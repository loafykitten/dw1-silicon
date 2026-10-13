//! PS1 BIOS (kernel) calls.
//!
//! The SDK's `rand`/`srand` in DW1's executable are three-instruction stubs that jump into the
//! BIOS (A(2Fh) and A(30h); `decomp/asm/us/main/psyq/libc.s`), so the game's random stream is
//! the BIOS generator, documented in psx-spx "Kernel (BIOS)":
//! `x = x * 0x41C64E6D + 0x3039`, result `(x >> 16) & 0x7FFF`.
//!
//! The seed the BIOS holds when the game first calls `rand` has not been measured yet (S0.8
//! does it on the emulator, with the retail BIOS: OpenBIOS may differ). Until then it is 0.

use std::sync::atomic::{AtomicU32, Ordering};

const RAND_MULTIPLIER: u32 = 0x41C6_4E6D;
const RAND_INCREMENT: u32 = 0x3039;

/// Seed before the first `rand` call. Placeholder until measured (see module docs).
pub const INITIAL_RAND_SEED: u32 = 0;

static RAND_STATE: AtomicU32 = AtomicU32::new(INITIAL_RAND_SEED);

/// One step of the BIOS generator: the next state and the 15-bit value `rand` returns.
pub const fn rand_step(state: u32) -> (u32, i32) {
    let next = state
        .wrapping_mul(RAND_MULTIPLIER)
        .wrapping_add(RAND_INCREMENT);
    (next, ((next >> 16) & 0x7FFF) as i32)
}

/// BIOS A(2Fh) `rand()`, called by the game C as `rand` (renamed `dw1_bios_rand` there so it
/// can't bind to the host libc's).
#[unsafe(no_mangle)]
pub extern "C" fn dw1_bios_rand() -> i32 {
    // The game is single-threaded; the atomic only keeps this safe Rust.
    let (next, value) = rand_step(RAND_STATE.load(Ordering::Relaxed));
    RAND_STATE.store(next, Ordering::Relaxed);
    value
}

/// BIOS A(30h) `srand(seed)`. DW1's C never calls it (S0.5 survey); kept for tests and the
/// oracle, which sets the measured initial seed.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_bios_srand(seed: u32) {
    RAND_STATE.store(seed, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rand_step_matches_the_documented_recurrence() {
        // From seed 0: x1 = 0x3039, x2 = 0x3039 * 0x41C64E6D + 0x3039 (mod 2^32).
        let (x1, r1) = rand_step(0);
        assert_eq!((x1, r1), (0x3039, 0));
        let (x2, r2) = rand_step(x1);
        assert_eq!(x2, 0x3039u32.wrapping_mul(0x41C6_4E6D).wrapping_add(0x3039));
        assert_eq!(r2, ((x2 >> 16) & 0x7FFF) as i32);
    }

    #[test]
    fn rand_is_15_bit() {
        let mut state = 1;
        for _ in 0..10_000 {
            let (next, value) = rand_step(state);
            assert!((0..=0x7FFF).contains(&value));
            state = next;
        }
    }
}
