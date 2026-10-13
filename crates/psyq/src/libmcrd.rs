//! libmcrd: the memory cards.
//!
//! Only initialisation so far; the card operations come with card images (Phase 2/3).

/// `MemCardInit(flg)`: initialises the card library [doc: flg 1 shares the controller ports'
/// interrupts with the pads].
#[unsafe(no_mangle)]
pub extern "C" fn MemCardInit(_flg: i32) {}

/// `MemCardStart()`: starts card access [doc].
#[unsafe(no_mangle)]
pub extern "C" fn MemCardStart() {}
