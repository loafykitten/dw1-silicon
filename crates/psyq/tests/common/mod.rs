//! Shared by the libgs capture tests: `DW1STATE` machine snapshots from
//! `oracle/capture-calls.lua` and the native PS1-RAM stand-in they are loaded into.

#![allow(dead_code, clippy::chunks_exact_to_as_chunks)]

use backend::gte::Gte;
use psyq::memory::{LINK_END, PS1_RAM_SIZE, dw1_ps1_ram, link_of, ps1_ram_ptr};
use std::path::{Path, PathBuf};

/// One machine snapshot (`DW1STATE`: GPR[34], CP0[32], CP2D[32], CP2C[32], pc, 1 KiB
/// scratchpad, 2 MiB RAM; all little-endian u32).
pub struct State {
    pub gpr: [u32; 34],
    pub cp2d: [u32; 32],
    pub cp2c: [u32; 32],
    pub scratch: Vec<u8>,
    pub ram: Vec<u8>,
}

pub fn word(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
}

impl State {
    pub fn load(path: &Path) -> State {
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(&b[..8], b"DW1STATE", "{}", path.display());
        let regs = &b[8..8 + 131 * 4];
        let mut s = State {
            gpr: [0; 34],
            cp2d: [0; 32],
            cp2c: [0; 32],
            scratch: Vec::new(),
            ram: Vec::new(),
        };
        for i in 0..34 {
            s.gpr[i] = word(regs, i);
        }
        for i in 0..32 {
            s.cp2d[i] = word(regs, 66 + i);
            s.cp2c[i] = word(regs, 98 + i);
        }
        let at = 8 + 131 * 4;
        s.scratch = b[at..at + 1024].to_vec();
        s.ram = b[at + 1024..at + 1024 + PS1_RAM_SIZE].to_vec();
        s
    }

    pub fn r32(&self, addr: u32) -> u32 {
        word(&self.ram, (addr as usize & (PS1_RAM_SIZE - 1)) / 4)
    }
}

pub fn root() -> PathBuf {
    match std::env::var_os("DW1_SORT_CAPTURES") {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs"),
    }
}

/// The captured GTE state loaded into a fresh `Gte`. Registers whose writes have side
/// effects or are derived (SXYP moves the FIFO, IRGB rewrites IR1-3, ORGB and LZCR) are
/// skipped.
pub fn load_gte(s: &State) -> Gte {
    let mut g = Gte::new();
    for r in 0..32 {
        if !matches!(r, 15 | 28 | 29 | 31) {
            g.mtc2(r, s.cp2d[r]);
        }
    }
    for r in 0..32 {
        g.ctc2(r, s.cp2c[r]);
    }
    g
}

/// A PS1 RAM address as a native pointer.
pub fn native(addr: u32) -> *mut u8 {
    ps1_ram_ptr(addr)
}

/// A 24-bit link as the PS1 captured it, translated to the native encoding.
pub fn translate_link(word: u32) -> u32 {
    let link = word & LINK_END;
    if link == LINK_END {
        return word;
    }
    (word & 0xFF00_0000) | link_of(native(0x8000_0000 | link))
}

pub fn ram() -> &'static mut [u8] {
    unsafe { std::slice::from_raw_parts_mut((&raw mut dw1_ps1_ram).cast::<u8>(), PS1_RAM_SIZE) }
}

pub fn ram_word(ram: &[u8], addr: u32) -> u32 {
    word(ram, (addr as usize & (PS1_RAM_SIZE - 1)) / 4)
}

pub fn set_ram_word(ram: &mut [u8], addr: u32, v: u32) {
    let o = addr as usize & (PS1_RAM_SIZE - 1);
    ram[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
