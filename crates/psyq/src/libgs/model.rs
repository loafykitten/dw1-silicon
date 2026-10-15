//! libgs models: mapping a TMD in place, linking objects to it, coordinate systems.
//!
//! TMD layout from psx-spx ("CDROM File Formats", TMD); behaviour observed on DW1's boot
//! (capture `p2-init3d`: the medal and poop models, 2026-10-13).

use crate::globals::Matrix;
use crate::memory::{ptr_of_ptr32, ptr32_of};

/// libgs `GsCOORDINATE2`.
#[repr(C)]
#[derive(Debug)]
pub struct GsCoordinate2 {
    pub flg: u32,
    pub coord: Matrix,
    pub workm: Matrix,
    pub param: *mut u8,
    pub sup: *mut GsCoordinate2,
    pub sub: *mut GsCoordinate2,
}

/// libgs `GsDOBJ2`.
#[repr(C)]
#[derive(Debug)]
pub struct GsDobj2 {
    pub attribute: u32,
    pub coord2: *mut GsCoordinate2,
    /// The object's `TMD_STRUCT` record (7 words) in the mapped TMD.
    pub tmd: *mut u32,
    pub id: u32,
}

/// Words in a TMD object record: vertex, normal and primitive tables (address, count), scale.
const OBJECT_WORDS: usize = 7;

/// `GsMapModelingData(p)`: `p` points at a TMD's flags word, after its ID. Unless flags bit 0
/// says it's done already, every object's vertex, normal and primitive offsets (relative to
/// the object table, which follows the object count) become addresses, and bit 0 is set
/// [doc: psx-spx TMD, FIXP; obs: offset 0x334 from the table at 0x80153EAC became
/// 0x801541E0]. Natively the addresses are 32-bit encoded pointers (`memory::ptr32_of`).
///
/// # Safety
/// `p` must point to a TMD's flags word, with the whole TMD after it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsMapModelingData(p: *mut u32) {
    unsafe {
        let flags = p.read();
        let count = p.add(1).read() as usize;
        let table = p.add(2);
        remember_mapping(table, count);
        if flags & 1 != 0 {
            return;
        }
        for i in 0..count {
            let object = table.add(i * OBJECT_WORDS);
            for field in [0, 2, 4] {
                let offset = object.add(field).read() as usize;
                object
                    .add(field)
                    .write(ptr32_of(table.cast::<u8>().add(offset)));
            }
        }
        p.write(flags | 1);
    }
}

/// Object tables the game has mapped (`GsMapModelingData`), with their object counts: where a
/// linked object's TMD is, for `GsSortObject4J`'s reads past a list (`sort4j::ObjectLists`).
/// A table mapped again (a new model loaded into the same buffer) replaces its entry.
static MAPPED: std::sync::Mutex<Vec<(usize, usize)>> = std::sync::Mutex::new(Vec::new());
/// Most tables kept (the oldest goes first; DW1 maps a few dozen models).
const MAPPED_KEEP: usize = 1024;

fn remember_mapping(table: *const u32, count: usize) {
    let mut m = MAPPED.lock().unwrap();
    m.retain(|&(t, _)| t != table as usize);
    if m.len() == MAPPED_KEEP {
        m.remove(0);
    }
    m.push((table as usize, count));
}

/// The mapped object table holding object record `record`, and its object count: None if the
/// record isn't one of a table `GsMapModelingData` saw.
pub(crate) fn mapped_table_of(record: *const u32) -> Option<(*const u32, usize)> {
    let r = record as usize;
    let size = OBJECT_WORDS * 4;
    MAPPED
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|&&(t, n)| r >= t && r < t + n * size && (r - t) % size == 0)
        .map(|&(t, n)| (t as *const u32, n))
}

/// The number of packet words after a TMD primitive's header, from its flag and mode bytes
/// [doc: psx-spx "TMD", packet data for polygons: the command/colour word, texture
/// coordinates, extra colours for gradation or unlit Gouraud, then vertex and normal indices as
/// halfwords padded to a word; obs: lit textured polygons have no command/colour word, and with
/// that the rule gives the stored input length of all 82,359 primitives in the disc's TMDs].
/// None for lines and sprites, which DW1's models don't use.
pub fn primitive_words(flag: u8, mode: u8) -> Option<usize> {
    if mode >> 5 != 1 {
        return None;
    }
    let quad = mode & 0x08 != 0;
    let textured = mode & 0x04 != 0;
    let gouraud = mode & 0x10 != 0;
    let lit = flag & 0x01 == 0;
    let gradation = flag & 0x04 != 0;
    let n = if quad { 4 } else { 3 };
    let mut words = usize::from(!(lit && textured));
    if textured {
        words += n;
    }
    if gradation {
        words += n - 1;
    }
    let mut halves = n;
    if lit {
        halves += if gouraud { n } else { 1 };
    } else if gouraud && !gradation {
        words += n - 1;
    }
    Some(words + halves.div_ceil(2))
}

/// `GsLinkObject4(tmd_base, objp, n)`: points `objp` at object `n` of the mapped TMD whose
/// object table is at `tmd_base` [doc], and groups that object's primitives for
/// `GsSortObject4J`: in the first primitive of each run with the same flag and mode bytes, the
/// low halfword (olen, ilen) becomes the run's length [obs: the medal's 22 lit GT4 primitives
/// gave 0x3C00080C -> 0x3C000016; the poop model's 1 + 8 gave two runs]. Primitive sizes come
/// from their type ([`primitive_words`]), not the overwritten input length, so linking the
/// same object again (the game does, every draw) finds the same runs.
///
/// The game C calls this through the native `libgs.h`, which passes `tmd_base` at pointer
/// width (`dw1_GsLinkObject4`; the PS1 prototype takes `unsigned long`).
///
/// # Safety
/// `tmd_base` must be a mapped TMD's object table holding object `n`, and `objp` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_GsLinkObject4(tmd_base: usize, objp: *mut GsDobj2, n: i32) {
    unsafe {
        let object = (tmd_base as *mut u32).add(n as usize * OBJECT_WORDS);
        (*objp).tmd = object;
        let mut prim = ptr_of_ptr32(object.add(4).read()).cast::<u32>();
        let mut left = object.add(5).read() as usize;
        while left > 0 {
            let first = prim;
            let kind = first.read() & 0xFFFF_0000;
            let mut run = 0u32;
            while left > 0 && prim.read() & 0xFFFF_0000 == kind {
                let header = prim.read();
                let (flag, mode) = ((header >> 16) as u8, (header >> 24) as u8);
                let Some(words) = primitive_words(flag, mode) else {
                    crate::trap::unimplemented("GsLinkObject4 on a TMD line or sprite primitive");
                };
                prim = prim.add(1 + words);
                run += 1;
                left -= 1;
            }
            first.write(kind | run);
        }
    }
}

/// `GsInitCoordinate2(super, base)`: `base` becomes the unit matrix under `super`, unchanged
/// since [doc; obs: the matrix is set, the rest was already zero in the capture].
///
/// # Safety
/// `base` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsInitCoordinate2(sup: *mut GsCoordinate2, base: *mut GsCoordinate2) {
    unsafe {
        let base = &mut *base;
        base.flg = 0;
        base.coord = Matrix::IDENTITY;
        base.sup = sup;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TMD in the link window: flags, one object, then the object table and its data.
    static mut TMD: [u32; 64] = [0; 64];

    #[test]
    fn primitive_sizes_follow_the_disc() {
        // Input lengths seen on the disc (see primitive_words).
        for (flag, mode, ilen) in [
            (0, 0x20, 3),
            (0, 0x24, 5),
            (0, 0x28, 4),
            (0, 0x2C, 7),
            (0, 0x34, 6),
            (0, 0x38, 5),
            (0, 0x3C, 8),
            (1, 0x25, 6),
            (1, 0x2D, 7),
            (1, 0x35, 8),
            (1, 0x3D, 10),
            (1, 0x3F, 10),
        ] {
            assert_eq!(
                primitive_words(flag, mode),
                Some(ilen),
                "flag {flag} mode {mode:#x}"
            );
        }
        assert_eq!(primitive_words(0, 0x40), None);
    }

    #[test]
    fn mapping_and_linking_follow_the_captures() {
        unsafe {
            let t = (&raw mut TMD).cast::<u32>();
            // flags 0, nobj 1; object: vertop 0x40, vern 1, nortop 0x48, norn 1, primtop 0x50,
            // primn 3, scale 0. Offsets are from the object table (word 2).
            for (i, w) in [0u32, 1, 0x40, 1, 0x48, 1, 0x50, 3, 0]
                .into_iter()
                .enumerate()
            {
                t.add(i).write(w);
            }
            // Primitives at table + 0x50: two lit textured Gouraud quads (mode 3C, 8 words
            // each), then a lit textured Gouraud triangle (mode 36... 34 family, 6 words).
            let p = t.add(2 + 0x50 / 4);
            p.write(0x3C00_080C);
            p.add(9).write(0x3C00_080C);
            p.add(18).write(0x3600_0609);
            GsMapModelingData(t);
            assert_eq!(t.read(), 1);
            let table = t.add(2).cast::<u8>();
            assert_eq!(ptr_of_ptr32(t.add(2).read()), table.add(0x40));
            assert_eq!(ptr_of_ptr32(t.add(6).read()), table.add(0x50));
            // Mapped already: nothing changes.
            let before = t.add(2).read();
            GsMapModelingData(t);
            assert_eq!(t.add(2).read(), before);

            let mut obj = GsDobj2 {
                attribute: 0,
                coord2: std::ptr::null_mut(),
                tmd: std::ptr::null_mut(),
                id: 0,
            };
            dw1_GsLinkObject4(t.add(2) as usize, &mut obj, 0);
            assert_eq!(obj.tmd, t.add(2));
            assert_eq!(p.read(), 0x3C00_0002);
            assert_eq!(p.add(9).read(), 0x3C00_080C);
            assert_eq!(p.add(18).read(), 0x3600_0001);
            // Linking again finds the same runs.
            dw1_GsLinkObject4(t.add(2) as usize, &mut obj, 0);
            assert_eq!(p.read(), 0x3C00_0002);
            assert_eq!(p.add(9).read(), 0x3C00_080C);
            assert_eq!(p.add(18).read(), 0x3600_0001);
        }
    }
}

#[cfg(test)]
mod mapping_tests {
    use super::*;

    #[test]
    fn a_linked_objects_table_is_found_only_when_mapped() {
        // a TMD with no objects' lists needed: flags, count 2, two object records
        let mut tmd = vec![0u32; 2 + 2 * OBJECT_WORDS];
        tmd[1] = 2;
        let p = tmd.as_mut_ptr();
        let table = unsafe { p.add(2) };
        let second = unsafe { table.add(OBJECT_WORDS) };
        assert_eq!(mapped_table_of(second), None);
        remember_mapping(table, 2);
        assert_eq!(mapped_table_of(second), Some((table.cast_const(), 2)));
        // not a record boundary, or past the table
        assert_eq!(mapped_table_of(unsafe { table.add(1) }), None);
        assert_eq!(mapped_table_of(unsafe { table.add(2 * OBJECT_WORDS) }), None);
        // mapped again with another count (a new model in the buffer) replaces the entry
        remember_mapping(table, 1);
        assert_eq!(mapped_table_of(second), None);
    }
}
