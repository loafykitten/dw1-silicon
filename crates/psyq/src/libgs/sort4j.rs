//! libgs: `GsSortObject4J`, which dispatches an object's primitives to the TMD handlers
//! (`tmd.rs`, `divide.rs`).
//!
//! Written clean-room from public documentation and black-box observation of the real SDK in an
//! emulator (`captures/oracle-runs/libgs-pin`, `s4j-*`; gitignored): psx-spx for the TMD format
//! (CDROM File Formats: TMD), the GTE commands and the GPU packet layouts; the API headers for
//! struct layouts, the `GsFCALL4` table layout and the handlers' documented signature
//! (`PACKET *handler(op, vp, np, pk, n, shift, ot, scratch)`). No SDK code was read or
//! translated.
//!
//! `GsSortObject4J` walks an object's primitive list, which `GsLinkObject4` grouped into runs of
//! equal kind, and calls the function the game installed in `GsFCALL4` for each run, exactly as
//! the SDK does; a kind whose slot is empty stops the program. The handlers DW1's models need
//! are in `tmd.rs` (non-dividing) and `divide.rs` (`GsDIV1`, the arenas' floors); the ones no
//! disc model reaches (the light-off and fog columns, untextured Gouraud triangles, the lit
//! dividing ones) are trapping stubs (`stubs.rs`).
//!
//! Natively the pieces are native objects: the `GsDOBJ2` has a native layout, its TMD record and
//! the primitive data hold 32-bit encoded pointers (`memory::ptr32_of`), the packet area is
//! `GsGetWorkBase()` (the SDK's `GsOUT_PACKET_P`), and ordering-table links are 24-bit offsets in
//! the link window (`memory::link_of`).
//!
//! Each rule below says where it comes from: `[doc]` psx-spx or the API headers, `[obs]` the
//! captures.

use super::tmd::Kind;
use super::{GsDobj2, GsGetWorkBase, GsOt, GsSetWorkBase, primitive_words};
use crate::globals::GsFCALL4;
use crate::memory::ptr_of_ptr32;
use crate::trap::unimplemented;

/// A TMD vertex or normal [doc: TMD "Vertex and normal lists": s16 x, y, z and 2 unused bytes].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vert {
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    pub pad: i16,
}

/// A `GsFCALL4` handler: one run of `n` primitives starting at `op` (their TMD headers), the
/// object's vertex and normal lists, the packet pointer, the ordering-table shift and table,
/// and the scratch area; returns the packet pointer after the packets it wrote [hdr: libgs.h].
pub type TmdHandler = unsafe extern "C" fn(
    op: *const u32,
    vp: *const Vert,
    np: *const Vert,
    pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
    scratch: *mut u32,
) -> *mut u32;

/// `GsDOBJ2.attribute` bits [hdr: libgs.h]: `GsDIV1` (1 << 9) the first divide level (the
/// handler table's divide row), `GsDOFF` (1 << 31) display off.
const ATTR_DIV1: u32 = 1 << 9;
const ATTR_HIDDEN: u32 = 1 << 31;

/// libgs's `GsNDIV`: the divide level of the object being sorted (0 or 1 here), which the
/// dividing handlers read [obs: GsSortObject4J writes it from the attribute; the handlers' work
/// area starts with it].
pub(crate) static NDIV: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Where each kind's slots start in `GsFCALL4` [hdr: libgs.h `_GsFCALL`: `f3[2][3]`, `nf3[2]`,
/// `g3[2][3]`, `ng3[2]`, `tf3`, `ntf3`, `tg3`, `ntg3`, the same for quads, then `f3g[3]`,
/// `g3g[3]`, `f4g[3]`, `g4g[3]`]. The lit kinds take `base + div * 3 + lmode`, the unlit ones
/// `base + div`.
fn slot_for(mode: u8, flag: u8, div: usize, lmode: usize) -> Option<usize> {
    if mode >> 5 != 1 {
        return None;
    }
    let quad = mode & 0x08 != 0;
    let textured = mode & 0x04 != 0;
    let gouraud = mode & 0x10 != 0;
    let lit = flag & 1 == 0;
    let gradation = flag & 4 != 0;
    if gradation {
        // f3g, g3g, f4g, g4g: untextured only.
        if textured {
            return None;
        }
        return Some(64 + 3 * (usize::from(quad) * 2 + usize::from(gouraud)) + lmode);
    }
    // Triangle kinds: f3 g3 tf3 tg3, then the four quad kinds, each (lit 6 slots, unlit 2).
    let kind = usize::from(quad) * 4 + usize::from(textured) * 2 + usize::from(gouraud);
    let base = kind * 8;
    Some(if lit {
        base + div * 3 + lmode
    } else {
        base + 6 + div
    })
}

/// `GsSortObject4J(objp, otp, shift, scratch)`: transforms, lights and clips the object's
/// polygons, writes GPU packets from the work base on, and links each into the ordering table
/// `otp` at index `OTZ >> shift` [doc: libgs reference; obs]. The GTE must already hold the
/// object's local-to-screen matrix and translation, the light matrices, the background colour,
/// OFX/OFY/H and ZSF3/ZSF4 (the game sets them before the call). `scratch` is the scratchpad
/// work area for the handlers that need one (the dividing ones [obs]). Returns
/// nothing [hdr]; the work base ends after the last packet [obs: `GsOUT_PACKET_P`].
///
/// # Safety
/// `objp` must be a `GsDOBJ2` linked to a mapped TMD (`GsMapModelingData`, `GsLinkObject4`),
/// `otp` an ordering table in the link window, and the work base a packet area in the same
/// window with room for the packets.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortObject4J(
    objp: *mut GsDobj2,
    otp: *mut GsOt,
    shift: i32,
    scratch: *mut u32,
) {
    unsafe {
        let obj = &*objp;
        let attribute = obj.attribute;
        // [obs: p9-render-s4j-attr] Display off: nothing is written, not even libgs's mode
        // globals, whatever the other bits.
        if attribute & ATTR_HIDDEN != 0 {
            return;
        }
        // [obs] The game's objects have attribute 0 or 0x200 (GsDIV1: the arenas' floors). The
        // divide level reaches the dividing handlers through libgs's GsNDIV (`NDIV`). Fog, light
        // off and the semi-transparency bits are never set, and other divide levels make the real
        // SDK run away on some kinds.
        let div = match attribute {
            0 => 0,
            ATTR_DIV1 => 1,
            _ => unimplemented("GsSortObject4J with an unobserved object attribute"),
        };
        NDIV.store(div as u32, std::sync::atomic::Ordering::Relaxed);
        let lmode = 0;

        // The object's TMD record: vertex, normal and primitive tables (address, count), scale.
        let tmd = obj.tmd;
        let vp = ptr_of_ptr32(tmd.read()).cast::<Vert>();
        let n_vert = tmd.add(1).read() as usize;
        let np = ptr_of_ptr32(tmd.add(2).read()).cast::<Vert>();
        let n_norm = tmd.add(3).read() as usize;
        let mut op = ptr_of_ptr32(tmd.add(4).read()).cast::<u32>().cast_const();
        let mut left = tmd.add(5).read() as usize;
        let lists = ObjectLists::new(tmd, vp, n_vert, np, n_norm);

        let mut pk = GsGetWorkBase().cast::<u32>();
        while left > 0 {
            let header = op.read();
            let (ilen, flag, mode) = (
                (header >> 8) as u8,
                (header >> 16) as u8,
                (header >> 24) as u8,
            );
            // [obs] A header with input size 0 starts a run (GsLinkObject4): its low 16 bits are
            // the run's primitive count. An ordinary header is a run of one.
            let n = if ilen == 0 { header & 0xFFFF } else { 1 } as usize;
            let Some(words) = primitive_words(flag, mode) else {
                unimplemented("GsSortObject4J on a TMD line or sprite primitive");
            };
            if n == 0 || n > left {
                unimplemented("GsSortObject4J on a TMD whose runs are not linked");
            }
            let Some(slot) = slot_for(mode, flag, div, lmode) else {
                unimplemented("GsSortObject4J on an untextured gradation primitive kind");
            };
            let Some(f) = GsFCALL4[slot] else {
                eprintln!("psyq: GsFCALL4 slot {slot} (TMD mode {mode:#04x} flag {flag}) is empty");
                unimplemented("GsSortObject4J without an installed handler");
            };
            check_indices(op, n, words, Kind::of(flag, mode), &lists);
            let handler: TmdHandler = std::mem::transmute(f);
            pk = handler(op, vp, np, pk, n as i32, shift, otp, scratch);
            op = op.add(n * (1 + words));
            left -= n;
        }
        GsSetWorkBase(pk.cast());
    }
}

/// An object's vertex and normal lists, for `check_indices`. The SDK reads an index past a
/// list's end as the 8 bytes that follow it [obs: Numemon's model, normal 22 of 22, after
/// Mr. Digivolution, Phase 9]. Natively the TMD file sits in memory with the PS1's layout
/// (`GsMapModelingData` rewrites only the object table), so such a read gives the PS1's bytes
/// while it stays inside the file's lists (any object's): `tmd_span`, from the table and object
/// count the game mapped.
struct ObjectLists {
    record: *const u32,
    vp: usize,
    n_vert: usize,
    np: usize,
    n_norm: usize,
}

const OBJECT_WORDS: usize = 7;

/// The end of `n` primitives from `op`, walking their headers as the sorter does; None when a
/// header isn't one the sorter knows.
///
/// # Safety
/// `op` must point at `n` primitives.
unsafe fn primitives_end(op: *const u32, n: usize) -> Option<usize> {
    let mut at = op;
    let mut left = n;
    while left > 0 {
        let header = unsafe { at.read() };
        let (ilen, flag, mode) = (
            (header >> 8) as u8,
            (header >> 16) as u8,
            (header >> 24) as u8,
        );
        let run = if ilen == 0 { header & 0xFFFF } else { 1 } as usize;
        let words = primitive_words(flag, mode)?;
        if run == 0 || run > left {
            return None;
        }
        at = unsafe { at.add(run * (1 + words)) };
        left -= run;
    }
    Some(at as usize)
}

/// The span of all the lists of the mapped TMD holding the object record `record` (the table
/// and object count `GsMapModelingData` saw); None for a record of no mapped table, or lists
/// the sorter can't walk.
///
/// # Safety
/// The table `model::mapped_table_of` gives must still hold that TMD (the game maps a model
/// again when it loads another into the buffer).
unsafe fn tmd_span(record: *const u32) -> Option<(usize, usize)> {
    let (table, count) = super::model::mapped_table_of(record)?;
    let (mut lo, mut hi) = (usize::MAX, 0usize);
    for i in 0..count {
        let o = unsafe { table.add(i * OBJECT_WORDS) };
        let w = |j: usize| unsafe { o.add(j).read() };
        let size = std::mem::size_of::<Vert>();
        let vp = ptr_of_ptr32(w(0)) as usize;
        let np = ptr_of_ptr32(w(2)) as usize;
        let op = ptr_of_ptr32(w(4)).cast::<u32>();
        let pend = unsafe { primitives_end(op, w(5) as usize) }?;
        for (start, end) in [
            (vp, vp + w(1) as usize * size),
            (np, np + w(3) as usize * size),
            (op as usize, pend),
        ] {
            lo = lo.min(start);
            hi = hi.max(end);
        }
    }
    Some((lo, hi))
}

impl ObjectLists {
    fn new(
        record: *const u32,
        vp: *const Vert,
        n_vert: usize,
        np: *const Vert,
        n_norm: usize,
    ) -> ObjectLists {
        ObjectLists {
            record,
            vp: vp as usize,
            n_vert,
            np: np as usize,
            n_norm,
        }
    }

    /// Whether entry `index` of the list at `base` with `count` entries reads bytes the PS1
    /// would also read: inside the list, or past it but inside the TMD's lists.
    fn readable(&self, base: usize, count: usize, index: usize) -> bool {
        if index < count {
            return true;
        }
        let at = base + index * std::mem::size_of::<Vert>();
        // SAFETY: the object came from GsLinkObject4 on a mapped TMD (GsSortObject4J's contract).
        unsafe { tmd_span(self.record) }
            .is_some_and(|(lo, hi)| at >= lo && at + std::mem::size_of::<Vert>() <= hi)
    }

    fn vertex(&self, index: usize) -> bool {
        self.readable(self.vp, self.n_vert, index)
    }

    fn normal(&self, index: usize) -> bool {
        self.readable(self.np, self.n_norm, index)
    }
}

/// Stops the program if a run reads outside its object's data: an index past its vertex or
/// normal list is what the SDK reads as the following bytes (`ObjectLists`), but beyond the
/// object's lists that could be any memory natively, so it is treated as unmodelled.
unsafe fn check_indices(op: *const u32, n: usize, words: usize, kind: Kind, lists: &ObjectLists) {
    for i in 0..n {
        let idx = unsafe { op.add(i * (1 + words) + kind.index_at()).cast::<u16>() };
        let read = |slot: usize| usize::from(unsafe { idx.add(slot).read() });
        for k in 0..kind.nv {
            let vertex = read(kind.vertex_slot(k));
            let normal = kind.lit.then(|| read(kind.normal_slot(k)));
            if !lists.vertex(vertex) || normal.is_some_and(|nn| !lists.normal(nn)) {
                eprintln!(
                    "psyq: TMD primitive {i} of a run: normal {normal:?} of {}, vertex {vertex} of {}",
                    lists.n_norm, lists.n_vert
                );
                unimplemented("GsSortObject4J on a primitive indexing past its TMD lists");
            }
        }
    }
}
