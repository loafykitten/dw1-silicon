//! Data the game C links against that isn't a function: SDK globals and the few linker
//! symbols the PS1 build had. Each layout is checked against the C headers in
//! `crates/decomp-c` (bindgen size asserts).

#![allow(non_upper_case_globals)]

/// libgte `MATRIX`: a 3x3 rotation in 4.12 fixed point and a translation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Matrix {
    pub m: [[i16; 3]; 3],
    pub t: [i32; 3],
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix {
        m: [[0x1000, 0, 0], [0, 0x1000, 0], [0, 0, 0x1000]],
        t: [0; 3],
    };
}

// libgs. GsIDMATRIX, the unit matrix, is set by GsInitGraph [obs: zero until then]; the other
// two by the view and light calls (GsSetRefView2, GsSetFlatLight...) before use.
#[unsafe(no_mangle)]
pub static mut GsIDMATRIX: Matrix = Matrix {
    m: [[0; 3]; 3],
    t: [0; 3],
};
#[unsafe(no_mangle)]
pub static mut GsWSMATRIX: Matrix = Matrix {
    m: [[0; 3]; 3],
    t: [0; 3],
};
#[unsafe(no_mangle)]
pub static mut GsLIGHTWSMATRIX: Matrix = Matrix {
    m: [[0; 3]; 3],
    t: [0; 3],
};

/// Slots in libgs's `_GsFCALL` table (GsSortObject4J's handlers per primitive kind, divide
/// mode and light mode), all function pointers.
pub const GS_FCALL_SLOTS: usize = 76;

/// libgs `GsFCALL4`. Empty until the game fills the slots it uses (graphics.c, the libgs.h
/// `jt_init4` pattern).
#[unsafe(no_mangle)]
pub static mut GsFCALL4: [Option<unsafe extern "C" fn()>; GS_FCALL_SLOTS] = [None; GS_FCALL_SLOTS];

/// libcd streaming: set when a streamed sector arrives; MOV polls and clears it.
#[unsafe(no_mangle)]
pub static mut StCdIntrFlag: i32 = 0;

// CodeWarrior linker symbols read by main.c's initializeHeap(). The native heap doesn't come
// from them (InitHeap3 traps until Phase 2 gives the game its heap), so they're zero.
#[unsafe(no_mangle)]
pub static mut _end: u32 = 0;
#[unsafe(no_mangle)]
pub static mut _stack_addr: u32 = 0;
#[unsafe(no_mangle)]
pub static mut _stack_size: u32 = 0;
