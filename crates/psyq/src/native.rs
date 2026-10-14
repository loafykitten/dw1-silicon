//! Choices the Mac app makes that the game C reads: additions to the game, not PS1 behaviour.
//! Each is off unless the app turns it on, so the oracle replays run the game as it was.

use std::sync::atomic::{AtomicI32, Ordering};

/// `dw1_pc_saves` (`crates/decomp-c/include/dw1_native.h`): nonzero when the game's save and
/// load screens use the app's save slots (Phase 7, `crates/dw1/src/saves.rs`) instead of the
/// two memory cards: no card choice, card checks or card wording. An `AtomicI32` has the
/// layout of the C side's `int32_t`.
#[allow(non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static dw1_pc_saves: AtomicI32 = AtomicI32::new(0);

/// Turns the save slots on or off (before the game starts).
pub fn set_pc_saves(on: bool) {
    dw1_pc_saves.store(on as i32, Ordering::Relaxed);
}

pub fn pc_saves() -> bool {
    dw1_pc_saves.load(Ordering::Relaxed) != 0
}
