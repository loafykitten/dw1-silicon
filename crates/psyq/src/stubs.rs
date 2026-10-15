//! Every SDK function the game C calls that has no Rust implementation yet, by library. Each
//! one traps with its name (`trap::stubs!`). Implementing one means deleting it here and
//! writing it in its library's module.
//!
//! The list is what linking main and every overlay needs (Phase 1, 2026-10-13); a call the
//! game gains later shows up as an undefined symbol at link time.

use crate::trap::stubs;

// libc: process exit (returns to the BIOS on the PS1). Renamed `dw1_exit` by dw1_native.h.
stubs! { dw1_exit }

// libgs: the TMD primitive handlers GsSortObject4J dispatches to through GsFCALL4.
stubs! {
    GsTMDdivTF3NL GsTMDdivTF4L GsTMDdivTF4NL GsTMDdivTG3NL GsTMDdivTG4NL GsTMDfastF4NL
    GsTMDfastG3L GsTMDfastNF4 GsTMDfastTF3NL GsTMDfastTF4NL GsTMDfastTG3NL GsTMDfastTG4NL
}
