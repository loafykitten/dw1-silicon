//! The game's GTE: one `backend::gte::Gte` standing in for COP2, reached by the game C through
//! the native `<inline_n.h>` (`crates/decomp-c/include/inline_n.h`), whose macros turn each
//! `lwc2`/`swc2`/`mtc2`/`mfc2`/`ctc2`/`cfc2`/`cop2` into a call here. The SDK's libgte and
//! libgs functions, as they're written in Rust, use the same instance through [`with`].

use backend::gte::{ExecStatus, Gte};
use std::sync::{LazyLock, Mutex};

static GTE: LazyLock<Mutex<Gte>> = LazyLock::new(|| Mutex::new(at_game_start()));

/// GTE control registers when the game's `main()` starts: what the retail BIOS (SCPH-1001,
/// Ember's dump) leaves after drawing its boot logo [obs: PCSX-Redux, `p2-init` capture at
/// the game's first call, 2026-10-13]. The game inherits them, so the native GTE starts there.
const CONTROL_AT_GAME_START: [u32; 32] = [
    0x00000FFB, 0xFFB7FF44, 0xF9CA0EBC, 0x063700AD, 0x00000EB7, 0x00000000, 0xFFFFFEAC, 0x00001700,
    0x00000000, 0x00000FA0, 0x0000F060, 0x0000F060, 0x00000000, 0x00000640, 0x00000640, 0x00000640,
    0x0BB80FA0, 0x0FA00FA0, 0x0FA00BB8, 0x0BB80FA0, 0x00000FA0, 0x00000000, 0x00000000, 0x00000000,
    0x01400000, 0x00F00000, 0x00000400, 0xFFFFF9E9, 0x01400000, 0x00000155, 0x00000100, 0x00600000,
];

/// GTE data registers at the same moment [obs]. Registers 15 (SXYP, a mirror of SXY2) and
/// 28..31 (IRGB/ORGB/LZCS/LZCR, views of other registers) aren't restored: writing them has
/// side effects, and the emulator's array doesn't hold their hardware view.
const DATA_AT_GAME_START: [u32; 28] = [
    0x00000B50, 0x00000B50, 0xFDABFF8D, 0x0000FE51, 0xFDAB000A, 0x0000FDD4, 0x200808FF, 0x00000509,
    0x000000A4, 0x0000104A, 0x00000082, 0x00000082, 0x0041012D, 0x0063012D, 0x00690146, 0x00000000,
    0x00001441, 0x00001441, 0x00001486, 0x00001419, 0x200808FF, 0x200808FF, 0x200808FF, 0x00000000,
    0xFFFFFCAE, 0x0000104A, 0x00000082, 0x00000082,
];

fn at_game_start() -> Gte {
    let mut gte = Gte::new();
    for (reg, &value) in CONTROL_AT_GAME_START.iter().enumerate() {
        gte.ctc2(reg, value);
    }
    for (reg, &value) in DATA_AT_GAME_START.iter().enumerate() {
        if reg != 15 {
            gte.mtc2(reg, value);
        }
    }
    gte
}

/// Runs `f` on the game's GTE. The game is single-threaded; the lock only keeps this sound.
pub fn with<R>(f: impl FnOnce(&mut Gte) -> R) -> R {
    // A panic while holding the lock aborts the process (panic = "abort"), so no poisoning.
    f(&mut GTE.lock().unwrap())
}

/// `mtc2 value, $reg`: write data register `reg`.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_gte_mtc2(reg: u32, value: u32) {
    with(|gte| gte.mtc2(reg as usize, value));
}

/// `mfc2 $rt, $reg`: read data register `reg`.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_gte_mfc2(reg: u32) -> u32 {
    with(|gte| gte.mfc2(reg as usize))
}

/// `ctc2 value, $reg`: write control register `reg`.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_gte_ctc2(reg: u32, value: u32) {
    with(|gte| gte.ctc2(reg as usize, value));
}

/// `cfc2 $rt, $reg`: read control register `reg`.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_gte_cfc2(reg: u32) -> u32 {
    with(|gte| gte.cfc2(reg as usize))
}

/// `cop2 command`: run a GTE command word. A command the model doesn't implement stops the
/// program, like an unimplemented SDK call, rather than leaving stale results behind.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_gte_cop2(command: u32) {
    if let ExecStatus::Unimplemented(op) = with(|gte| gte.execute(command)) {
        eprintln!("psyq: GTE command {op:#09x} isn't modelled (crates/backend/src/gte.rs)");
        std::process::abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backend::gte::{creg, dreg};

    #[test]
    fn registers_and_commands_reach_the_model() {
        // LZCS/LZCR: leading zeros of a positive word.
        dw1_gte_mtc2(dreg::LZCS as u32, 0x0000_1000);
        assert_eq!(dw1_gte_mfc2(dreg::LZCR as u32), 19);

        // RTPS with an identity rotation, no translation, H = 0x100, no offset: SXY2 is the
        // vertex scaled by H / Z.
        for (reg, value) in [(0, 0x1000), (1, 0), (2, 0x1000), (3, 0), (4, 0x1000)] {
            dw1_gte_ctc2(reg, value);
        }
        for reg in 5..=7 {
            dw1_gte_ctc2(reg, 0);
        }
        dw1_gte_ctc2(creg::OFX as u32, 0);
        dw1_gte_ctc2(creg::OFY as u32, 0);
        dw1_gte_ctc2(creg::H as u32, 0x100);
        dw1_gte_mtc2(dreg::VXY0 as u32, (20u32 << 16) | 10); // x = 10, y = 20
        dw1_gte_mtc2(dreg::VZ0 as u32, 0x200);
        dw1_gte_cop2(0x0180001); // RTPS, sf = 1, as gte_rtps() issues it
        let sxy2 = dw1_gte_mfc2(dreg::SXY2 as u32);
        // Projection divides by Z and multiplies by H: x = 10 * 0x100 / 0x200 = 5, y = 10.
        assert_eq!((sxy2 & 0xFFFF, sxy2 >> 16), (5, 10));
    }
}
