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
        if flags & 1 != 0 {
            return;
        }
        let count = p.add(1).read() as usize;
        let table = p.add(2);
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

/// `GsLinkObject4(tmd_base, objp, n)`: points `objp` at object `n` of the mapped TMD whose
/// object table is at `tmd_base` [doc], and groups that object's primitives for
/// `GsSortObject4J`: in the first primitive of each run with the same flag and mode bytes, the
/// low halfword (olen, ilen) becomes the run's length [obs: the medal's 22 lit GT4 primitives
/// gave 0x3C00080C -> 0x3C000016; the poop model's 1 + 8 gave two runs].
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
                let ilen = ((prim.read() >> 8) & 0xFF) as usize;
                prim = prim.add(1 + ilen);
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
    static mut TMD: [u32; 32] = [0; 32];

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
            // Primitives at table + 0x50 = word 22: a 2-word GT4-ish run of two (ilen 1), then
            // one of another kind.
            let p = t.add(2 + 0x50 / 4);
            p.write(0x3C00_0101);
            p.add(2).write(0x3C00_0101);
            p.add(4).write(0x3600_0101);
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
            assert_eq!(p.add(2).read(), 0x3C00_0101);
            assert_eq!(p.add(4).read(), 0x3600_0001);
        }
    }
}
