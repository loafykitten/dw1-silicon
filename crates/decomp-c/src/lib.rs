//! The decomp's game C, compiled for arm64 (see `build.rs`), and the Rust view of it: extern
//! declarations for the C functions Rust calls, and bindgen bindings for the decomp structs
//! Rust reads, each checked against its PS1 size.

use core::ffi::c_long;

// Keep the psyq symbols the C needs (`dw1_bios_rand`) in the link.
use psyq as _;

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]
pub mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

/// PS1 layout sizes. A struct shared with saves, disc data or Rust must keep its PS1 size;
/// growing one (a pointer or `long` member) fails the build here.
const _: () = {
    assert!(size_of::<bindings::DigimonPara>() == 0x34);
    assert!(size_of::<bindings::PartnerPara>() == 0x84);
};

unsafe extern "C" {
    // src/main/rng.c. Declared with `long` in the decomp, which is 64-bit on macOS; to become
    // `int32_t` under DW1_NATIVE (S0.5 survey, `long` inventory).
    pub fn customRandom(min: c_long, max: c_long) -> i32;
    pub static mut CUSTOM_RNG_VAL1: u32;
    pub static mut CUSTOM_RNG_VAL2: u32;

    // src/main/math.c
    pub fn randomLimit(limit: i32) -> i32;
    pub fn _sin(input: i32) -> i32;
    pub fn _cos(input: i32) -> i32;
    pub fn _atan(dy: i32, dx: i32) -> i32;

    // src/main/file_table_us.c (via dw/sin_lookup_table.h)
    pub static SIN_LOOKUP_TABLE: [i16; 130];
}

#[cfg(test)]
mod tests {
    use super::*;
    use psyq::bios;

    /// `_sin` as math.c computes it: a quarter-wave table over a 512-step circle.
    fn sin_model(input: i32) -> i32 {
        let table = unsafe { &SIN_LOOKUP_TABLE };
        let i = (input & 0x1FF) as usize;
        match i {
            0..128 => table[i].into(),
            128..256 => table[256 - i].into(),
            256..384 => -i32::from(table[i - 256]),
            _ => -i32::from(table[512 - i]),
        }
    }

    #[test]
    fn trig_uses_the_linked_tables() {
        for input in -600..1200 {
            assert_eq!(unsafe { _sin(input) }, sin_model(input), "_sin({input})");
            assert_eq!(
                unsafe { _cos(input) },
                sin_model(input + 128),
                "_cos({input})"
            );
        }
        // _atan reads ATAN_LOOKUP_TABLE; the four axis directions are a smoke test that it
        // links and stays in range, not a model of it.
        for (dy, dx) in [(0, 100), (100, 0), (0, -100), (-100, 0), (37, -91)] {
            let angle = unsafe { _atan(dy, dx) };
            assert!((0..4096).contains(&angle), "_atan({dy}, {dx}) = {angle}");
        }
    }

    const CUSTOM_RNG_FACTOR: u32 = 0x41C6_50AD;
    const CUSTOM_RNG_VALUE: u32 = 0x3039;

    /// `customRandom` as the PS1 computes it (32-bit `long`, unsigned modulo).
    fn custom_random_model(state: &mut (u32, u32), min: i32, max: i32) -> i32 {
        if min == max {
            return min;
        }
        let (min, max) = if max < min { (max, min) } else { (min, max) };
        state.0 = state
            .0
            .wrapping_mul(CUSTOM_RNG_FACTOR)
            .wrapping_add(CUSTOM_RNG_VALUE);
        state.1 = state
            .1
            .wrapping_mul(CUSTOM_RNG_FACTOR)
            .wrapping_add(CUSTOM_RNG_VALUE);
        let mixed = (state.0 >> 16) | (state.1 << 16);
        let range = (max.wrapping_sub(min).wrapping_add(1)) as u32;
        min.wrapping_add((mixed % range) as i32)
    }

    /// Both generators are global state in C and Rust, so everything stateful runs in this one
    /// test, in order.
    #[test]
    fn game_c_rngs_match_the_ps1_models() {
        // customRandom: C against the model, from the game's initial state.
        let mut model = (0u32, 0x0013_CC25u32);
        unsafe {
            CUSTOM_RNG_VAL1 = model.0;
            CUSTOM_RNG_VAL2 = model.1;
        }
        for (min, max) in [(0, 99), (5, 5), (10, 1), (-50, 50), (0, 0x7FFF), (1, 3)] {
            for _ in 0..100 {
                let c = unsafe { customRandom(min.into(), max.into()) };
                assert_eq!(
                    c,
                    custom_random_model(&mut model, min, max),
                    "customRandom({min}, {max})"
                );
            }
        }

        // randomLimit: the C calls rand(), which must land in psyq's BIOS generator.
        bios::dw1_bios_srand(0x1234_5678);
        let mut state = 0x1234_5678;
        for limit in [1i32, 2, 10, 100, 1000, 0x7FFF, -7] {
            let (next, r) = bios::rand_step(state);
            state = next;
            let expected = limit.wrapping_mul(r) >> 15;
            assert_eq!(
                unsafe { randomLimit(limit) },
                expected,
                "randomLimit({limit})"
            );
        }
    }
}
