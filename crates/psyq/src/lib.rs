//! Rust implementations of the PS1 SDK and BIOS calls the game C makes.
//!
//! Each function keeps the original name and C signature (or a `dw1_` name where the original
//! would clash with the host libc; see `crates/decomp-c/include/dw1_native.h`), with 32-bit
//! integer types where the PS1 used `long`.

pub mod bios;
