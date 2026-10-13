//! Native stand-ins for PS1 memory the game C addresses directly.
//!
//! - **PS1 RAM** (2 MB): fixed addresses in the decomp (`DW1_PS1_ADDR`: `GENERAL_BUFFER` at
//!   0x80010000, assets at known places inside overlay images, the overlays' load addresses)
//!   point here at the same offset (`addr & 0x1FFFFF`), so loaded files and the game's
//!   fixed-address reads agree. Only those regions are used; the rest of the game's memory is
//!   ordinary native memory.
//! - **The scratchpad** (1 KB at 0x1F800000): `getScratchAddr` (native `libetc.h`).
//!
//! Both are statics in the app's data segment, inside the GPU's link window
//! (docs/phase2-plan.md).

#![allow(non_upper_case_globals)]

/// PS1 main RAM size.
pub const PS1_RAM_SIZE: usize = 2 << 20;

/// PS1 scratchpad size.
pub const SCRATCHPAD_SIZE: usize = 1024;

#[repr(C, align(16))]
pub struct Ps1Ram(pub [u8; PS1_RAM_SIZE]);

#[repr(C, align(16))]
pub struct Scratchpad(pub [u8; SCRATCHPAD_SIZE]);

/// The stand-in for PS1 RAM, `dw1_ps1_ram` to the C.
#[unsafe(no_mangle)]
pub static mut dw1_ps1_ram: Ps1Ram = Ps1Ram([0; PS1_RAM_SIZE]);

/// The scratchpad, `dw1_scratchpad` to the C.
#[unsafe(no_mangle)]
pub static mut dw1_scratchpad: Scratchpad = Scratchpad([0; SCRATCHPAD_SIZE]);

/// The native address of PS1 RAM address `addr` (any segment: KUSEG, KSEG0, KSEG1).
pub fn ps1_ram_ptr(addr: u32) -> *mut u8 {
    (&raw mut dw1_ps1_ram)
        .cast::<u8>()
        .wrapping_add((addr & 0x1F_FFFF) as usize)
}
