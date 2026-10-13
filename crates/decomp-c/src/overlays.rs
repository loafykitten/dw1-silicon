//! The overlay load addresses `loadDynamicLibrary` (src/main/utils2.c) reads each
//! `*_REL.BIN` into, which the PS1 linker script defined.
//!
//! Natively every overlay's code is linked into the program already, and what reloading an
//! overlay must reset is the Phase 2/5 overlay-lifetime work (docs/survey.md, item 4). Until
//! then the addresses are empty arrays: the file layer they feed traps first (CdSearchFile,
//! CdRead), so nothing is ever written through them.

#![allow(non_upper_case_globals)]

macro_rules! load_addresses {
    ($($name:ident)*) => {
        $(
            #[unsafe(no_mangle)]
            pub static mut $name: [u8; 0] = [];
        )*
    };
}

load_addresses! {
    btl_START dget_START doo2_START dooa_START eab_START endi_START evl_START fish_START
    kar_START mov_START murd_START std_START trn_START trn2_START vs_START
}
