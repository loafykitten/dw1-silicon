//! Overlay data lifetimes (run side; the build side is `overlays.rs` next to `build.rs`, which
//! explains the rule). Each overlay's initialised data sits in a section of its own and its
//! zero-initialised and uninitialised data in a second; [`snapshot`] copies every first section
//! before the game starts, and `dw1_overlay_reset`, which the game's `loadDynamicLibrary` calls
//! under `DW1_NATIVE`, puts an overlay's copy back and zeroes its second section as the PS1's
//! load of its file would, keeping the variables the PS1 had in the resident executable or in
//! its overlay `.bss`.

use std::sync::Mutex;

/// One resident variable of an overlay file: its address and native size (the generated
/// wrappers' `dw1_keep_t`; a list ends with a null address).
#[repr(C)]
struct Keep {
    addr: *mut u8,
    size: usize,
}

include!(concat!(env!("OUT_DIR"), "/overlays_table.rs"));

struct Section {
    id: u32,
    start: *mut u8,
    len: usize,
    initial: Vec<u8>,
    zero_start: *mut u8,
    zero_len: usize,
    keeps: Vec<(*mut u8, usize)>,
}

// The pointers are the game's statics; the game is single-threaded.
unsafe impl Send for Section {}

static SECTIONS: Mutex<Vec<Section>> = Mutex::new(Vec::new());

/// Copies every overlay's initialised data as linked (the values its file holds). Call once,
/// before the game runs.
pub fn snapshot() {
    let mut sections = SECTIONS.lock().unwrap();
    if !sections.is_empty() {
        return;
    }
    for (id, start, end, zero_start, zero_end, keep_lists) in overlay_sections() {
        let len = end as usize - start as usize;
        let zero_len = zero_end as usize - zero_start as usize;
        let initial = unsafe { std::slice::from_raw_parts(start, len) }.to_vec();
        let inside = |a: *mut u8, n: usize| {
            (a >= start && a as usize + n <= end as usize)
                || (a >= zero_start && a as usize + n <= zero_end as usize)
        };
        let mut keeps = Vec::new();
        for list in keep_lists {
            let mut k = list;
            unsafe {
                while !(*k).addr.is_null() {
                    let (addr, size) = ((*k).addr, (*k).size);
                    // The linker may drop an unused variable; a kept one must be in a section.
                    assert!(inside(addr, size), "overlay {id}: kept variable outside its sections");
                    keeps.push((addr, size));
                    k = k.add(1);
                }
            }
        }
        sections.push(Section {
            id,
            start,
            len,
            initial,
            zero_start,
            zero_len,
            keeps,
        });
    }
}

/// `dw1_overlay_reset(lib)`: the game is loading overlay `lib` (its `Overlay` number): its
/// initialised data goes back to the file's values and its zero-initialised data to zero, but
/// for its kept variables (resident, or without an initialiser). Overlays without data sections
/// (SHOP, built into the main executable) are left alone.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_overlay_reset(lib: i32) {
    let sections = SECTIONS.lock().unwrap();
    assert!(
        !sections.is_empty() || overlay_sections().is_empty(),
        "dw1_overlay_reset before decomp_c::overlays::snapshot"
    );
    let Some(s) = sections.iter().find(|s| s.id as i32 == lib) else {
        return;
    };
    unsafe {
        let kept: Vec<Vec<u8>> = s
            .keeps
            .iter()
            .map(|&(a, n)| std::slice::from_raw_parts(a, n).to_vec())
            .collect();
        std::ptr::copy_nonoverlapping(s.initial.as_ptr(), s.start, s.len);
        std::ptr::write_bytes(s.zero_start, 0, s.zero_len);
        for (&(a, n), v) in s.keeps.iter().zip(&kept) {
            std::ptr::copy_nonoverlapping(v.as_ptr(), a, n);
        }
    }
}

/// Each overlay section's (Overlay number, size in bytes, resident variables kept), for the
/// tests and diagnostics.
pub fn sections() -> Vec<(u32, usize, usize)> {
    SECTIONS
        .lock()
        .unwrap()
        .iter()
        .map(|s| (s.id, s.len, s.keeps.len()))
        .collect()
}

/// A section's start and length, and its kept variables' addresses and sizes.
pub type SectionInfo = (*mut u8, usize, Vec<(*mut u8, usize)>);

/// For tests: overlay `id`'s first section and its kept variables.
#[doc(hidden)]
pub fn section_info(id: u32) -> Option<SectionInfo> {
    let s = SECTIONS.lock().unwrap();
    let sec = s.iter().find(|s| s.id == id)?;
    Some((sec.start, sec.len, sec.keeps.clone()))
}
