//! libgs: `GsSortObject4J` and the TMD primitive handlers it dispatches to.
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
//! the SDK does; a kind whose slot is empty stops the program. The handlers live here too:
//! `GsTMDfastTG3L` and `GsTMDfastTG4L` (light-calculated, Gouraud, textured triangles and quads)
//! are what the characters' models use; `GsTMDfastTNF4` (unlit, flat, textured quads) is what
//! the battle's effect models use (Phase 5, `s4j-btl-effect`). The other `GsTMD*` handlers are
//! trapping stubs (`stubs.rs`) until a capture exercises them.
//!
//! Natively the pieces are native objects: the `GsDOBJ2` has a native layout, its TMD record and
//! the primitive data hold 32-bit encoded pointers (`memory::ptr32_of`), the packet area is
//! `GsGetWorkBase()` (the SDK's `GsOUT_PACKET_P`), and ordering-table links are 24-bit offsets in
//! the link window (`memory::link_of`).
//!
//! Each rule below says where it comes from: `[doc]` psx-spx or the API headers, `[obs]` the
//! captures.

use super::{GsDobj2, GsGetWorkBase, GsOt, GsSetWorkBase, primitive_words};
use crate::globals::GsFCALL4;
use crate::memory::{link_of, ptr_of_ptr32};
use crate::trap::unimplemented;
use backend::gte::{Gte, cmd, dreg};

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

/// `GsDOBJ2.attribute` bits [hdr: libgs.h]: `GsFOG` (1 << 3) and `GsLOFF` (1 << 6) choose the
/// light mode column of the handler table, `GsDIV1`..`GsDIV5` (n << 9) the divide row.
const ATTR_FOG: u32 = 1 << 3;
const ATTR_LOFF: u32 = 1 << 6;
const ATTR_DIV: u32 = 7 << 9;

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
/// work area for the handlers that need one (the two implemented don't touch it [obs]). Returns
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
        // [obs] every object so far has attribute 0 (normal light, no divide).
        let attribute = obj.attribute;
        if attribute & !(ATTR_FOG | ATTR_LOFF | ATTR_DIV) != 0 {
            unimplemented("GsSortObject4J with an unobserved object attribute");
        }
        if attribute & (ATTR_FOG | ATTR_LOFF | ATTR_DIV) != 0 {
            unimplemented("GsSortObject4J with fog, light off or divide (unobserved)");
        }
        let (div, lmode) = (0, 0);

        // The object's TMD record: vertex, normal and primitive tables (address, count), scale.
        let tmd = obj.tmd;
        let vp = ptr_of_ptr32(tmd.read()).cast::<Vert>();
        let n_vert = tmd.add(1).read() as usize;
        let np = ptr_of_ptr32(tmd.add(2).read()).cast::<Vert>();
        let n_norm = tmd.add(3).read() as usize;
        let mut op = ptr_of_ptr32(tmd.add(4).read()).cast::<u32>().cast_const();
        let mut left = tmd.add(5).read() as usize;

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
            check_indices(op, n, words, mode, flag, n_vert, n_norm);
            let handler: TmdHandler = std::mem::transmute(f);
            pk = handler(op, vp, np, pk, n as i32, shift, otp, scratch);
            op = op.add(n * (1 + words));
            left -= n;
        }
        GsSetWorkBase(pk.cast());
    }
}

/// Stops the program if a run of a kind implemented here indexes past its object's vertex or
/// normal list. The SDK would read whatever lies beyond; natively that can be any memory, so an
/// out-of-range index is treated as unmodelled. Kinds without a native handler trap in their
/// stub anyway.
unsafe fn check_indices(
    op: *const u32,
    n: usize,
    words: usize,
    mode: u8,
    flag: u8,
    n_vert: usize,
    n_norm: usize,
) {
    // Lit Gouraud textured triangles and quads: (normal, vertex) halfword pairs after the
    // header and one texture word per vertex. [doc: TMD primitive layouts]
    if (mode & !0x03, flag & 1) == (0x2C, 1) {
        // Unlit flat textured quads: four vertex halfwords after the header, four texture
        // words and the colour word. [doc]
        for i in 0..n {
            let idx = unsafe { op.add(i * (1 + words) + 6).cast::<u16>() };
            for k in 0..4 {
                let vertex = usize::from(unsafe { idx.add(k).read() });
                if vertex >= n_vert {
                    eprintln!("psyq: TMD primitive {i} of a run: vertex {vertex} of {n_vert}");
                    unimplemented("GsSortObject4J on a primitive indexing past its TMD lists");
                }
            }
        }
        return;
    }
    let nv = match (mode & !0x03, flag & 1) {
        (0x34, 0) => 3,
        (0x3C, 0) => 4,
        _ => return,
    };
    for i in 0..n {
        let idx = unsafe { op.add(i * (1 + words) + 1 + nv).cast::<u16>() };
        for k in 0..nv {
            let (normal, vertex) = unsafe {
                (
                    usize::from(idx.add(2 * k).read()),
                    usize::from(idx.add(2 * k + 1).read()),
                )
            };
            if normal >= n_norm || vertex >= n_vert {
                eprintln!(
                    "psyq: TMD primitive {i} of a run: normal {normal} of {n_norm}, vertex {vertex} \
                     of {n_vert}"
                );
                unimplemented("GsSortObject4J on a primitive indexing past its TMD lists");
            }
        }
    }
}

/// `GsTMDfastTG3L`: a run of light-calculated, Gouraud, textured triangles (mode 0x34, or 0x36
/// with the semi-transparent bit; flag 0).
///
/// # Safety
/// As [`GsSortObject4J`]'s: `op` is `n` linked TMD primitives, `vp`/`np` their object's lists.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsTMDfastTG3L(
    op: *const u32,
    vp: *const Vert,
    np: *const Vert,
    pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
    _scratch: *mut u32,
) -> *mut u32 {
    unsafe { crate::gte::with(|gte| tg_run(gte, 3, op, vp, np, pk, n, shift, ot)) }
}

/// `GsTMDfastTG4L`: the same for quads (mode 0x3C, flag 0).
///
/// # Safety
/// As [`GsTMDfastTG3L`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsTMDfastTG4L(
    op: *const u32,
    vp: *const Vert,
    np: *const Vert,
    pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
    _scratch: *mut u32,
) -> *mut u32 {
    unsafe { crate::gte::with(|gte| tg_run(gte, 4, op, vp, np, pk, n, shift, ot)) }
}

/// `GsTMDfastTNF4`: a run of unlit, flat, textured quads (flag 1: no light calculation,
/// one-sided).
///
/// [doc] A primitive is a header word, the four vertices' texture words (u, v, and the CLUT in
/// the first's high half, the texture page in the second's), the colour word (r, g, b), then
/// the four vertex indices as halfwords, the first at the lower address. [obs] (`s4j-btl-effect`:
/// 20 primitives of mode 0x2F, 12 drawn) The first three vertices go through RTPT and NCLIP, a
/// result of 0 or less drops the quad, the fourth through RTPS, then AVSZ4 gives the OTZ. The
/// packet is a `POLY_FT4`: the colour with command 0x2E (mode 0x2F loses its bit 0), then per
/// vertex its screen XY and its texture word as stored; it goes at the head of entry
/// `OTZ >> shift`. Only mode 0x2F has been observed; any other mode or flag stops the program.
///
/// # Safety
/// As [`GsTMDfastTG3L`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsTMDfastTNF4(
    op: *const u32,
    vp: *const Vert,
    _np: *const Vert,
    pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
    _scratch: *mut u32,
) -> *mut u32 {
    unsafe { crate::gte::with(|gte| tnf4_run(gte, op, vp, pk, n, shift, ot)) }
}

unsafe fn tnf4_run(
    gte: &mut Gte,
    mut op: *const u32,
    vp: *const Vert,
    mut pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
) -> *mut u32 {
    unsafe {
        let ot_entries = 1u32.checked_shl((*ot).length).unwrap_or(0);
        let ot_org = (*ot).org;
        for _ in 0..n {
            let header = op.read();
            let tex = op.add(1);
            let color = op.add(5).read();
            let idx = op.add(6).cast::<u16>();
            op = op.add(8);
            if header >> 16 != 0x2F01 {
                eprintln!(
                    "psyq: TMD mode {:#04x} flag {:#04x} in the TNF4 handler",
                    header >> 24,
                    (header >> 16) & 0xFF
                );
                unimplemented("GsTMDfastTNF4 with an unobserved mode or flag");
            }
            let vertex = |k: usize| &*vp.add(usize::from(idx.add(k).read()));
            for k in 0..3 {
                let (xy, z) = pair(vertex(k));
                gte.mtc2(dreg::VXY0 + 2 * k, xy);
                gte.mtc2(dreg::VZ0 + 2 * k, z);
            }
            gte.execute(cmd::RTPT);
            gte.execute(cmd::NCLIP);
            if gte.mfc2(dreg::MAC0) as i32 <= 0 {
                continue;
            }
            let mut sxy = [0u32; 4];
            for (k, s) in sxy.iter_mut().take(3).enumerate() {
                *s = gte.mfc2(dreg::SXY0 + k);
            }
            let (xy, z) = pair(vertex(3));
            gte.mtc2(dreg::VXY0, xy);
            gte.mtc2(dreg::VZ0, z);
            gte.execute(cmd::RTPS);
            sxy[3] = gte.mfc2(dreg::SXY2);
            gte.execute(cmd::AVSZ4);
            let ot_index = gte.mfc2(dreg::OTZ) >> shift;
            if ot_index >= ot_entries {
                unimplemented("GsTMDfastTNF4 with OTZ past the ordering table");
            }
            pk.add(1).write(0x2E00_0000 | (color & 0x00FF_FFFF));
            for (k, &xy) in sxy.iter().enumerate() {
                pk.add(2 + 2 * k).write(xy);
                pk.add(3 + 2 * k).write(tex.add(k).read());
            }
            let slot = ot_org.add(ot_index as usize);
            let old = slot.read();
            pk.write(9 << 24 | (old & 0x00FF_FFFF));
            slot.write((old & 0xFF00_0000) | link_of(pk.cast()));
            pk = pk.add(10);
        }
        pk
    }
}

/// The transformation of one vertex from a TMD list as the GTE's packed (XY, Z) register pair.
fn pair(v: &Vert) -> (u32, u32) {
    (
        u32::from(v.vx as u16) | u32::from(v.vy as u16) << 16,
        u32::from(v.vz as u16),
    )
}

/// Lit Gouraud textured polygons with `nv` vertices (3 or 4), `n` of them from `op`.
///
/// [doc] A primitive is a header word, one texture word per vertex (u, v and the CLUT or
/// texture page), then one (normal, vertex) index pair per vertex, each a halfword, normal at
/// the lower address. [obs] No colour word: the colour comes from the lighting.
#[allow(clippy::too_many_arguments)]
unsafe fn tg_run(
    gte: &mut Gte,
    nv: usize,
    mut op: *const u32,
    vp: *const Vert,
    np: *const Vert,
    mut pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
) -> *mut u32 {
    unsafe {
        let base_mode: u32 = if nv == 4 { 0x3C } else { 0x34 };
        let ot_entries = 1u32.checked_shl((*ot).length).unwrap_or(0);
        let ot_org = (*ot).org;
        for _ in 0..n {
            let header = op.read();
            let tex = op.add(1);
            let idx = op.add(1 + nv).cast::<u16>();
            op = op.add(1 + 2 * nv);
            let normal_ix = |k: usize| usize::from(idx.add(2 * k).read());
            let vertex_ix = |k: usize| usize::from(idx.add(2 * k + 1).read());
            // Only the observed combinations, flag 0 (one-sided, lit, no gradation) and the
            // mode exactly 0x34/0x3C, or with the semi bit (0x36/0x3E): the command byte is the header's own, so the
            // semi-transparent bit (0x02) rides along into the packet [obs: mode 0x36
            // triangles in File City; mode 0x3E quads in the loaded save's File City]. The
            // raw-texture bit and other flags are unobserved. The game installs this handler in
            // a slot those bits share.
            let mode = header >> 24;
            if (header >> 16) & 0xFF != 0 || (mode != base_mode && mode != base_mode | 0x02) {
                eprintln!(
                    "psyq: TMD mode {mode:#04x} flag {:#04x} in the TG{nv}L handler",
                    (header >> 16) & 0xFF
                );
                unimplemented("GsTMDfastTG*L with unobserved mode or flag bits");
            }

            // Vertices 0..2 through RTPT, then the back-face test on them. [doc: RTPT, NCLIP]
            for k in 0..3 {
                let (xy, z) = pair(&*vp.add(vertex_ix(k)));
                gte.mtc2(dreg::VXY0 + 2 * k, xy);
                gte.mtc2(dreg::VZ0 + 2 * k, z);
            }
            gte.execute(cmd::RTPT);
            gte.execute(cmd::NCLIP);
            let nclip = gte.mfc2(dreg::MAC0) as i32;
            // Back-face culling [obs: flag 0 = "clip back"]: NCLIP on the first three vertices;
            // a result of 0 or less is dropped (a quad too, though its fourth vertex might face
            // front). The GTE state a culled primitive leaves is not part of the contract.
            if nclip <= 0 {
                continue;
            }

            let mut sxy = [0u32; 4];
            for (k, s) in sxy.iter_mut().take(3).enumerate() {
                *s = gte.mfc2(dreg::SXY0 + k);
            }
            if nv == 4 {
                let (xy, z) = pair(&*vp.add(vertex_ix(3)));
                gte.mtc2(dreg::VXY0, xy);
                gte.mtc2(dreg::VZ0, z);
                gte.execute(cmd::RTPS);
                sxy[3] = gte.mfc2(dreg::SXY2);
                gte.execute(cmd::AVSZ4);
            } else {
                gte.execute(cmd::AVSZ3);
            }
            let ot_index = gte.mfc2(dreg::OTZ) >> shift;
            if ot_index >= ot_entries {
                // What the SDK does here is unobserved.
                unimplemented("GsTMDfastTG*L with OTZ past the ordering table");
            }

            // Lighting: RGBC = code | 0x808080 [obs: RGBC after each call]; normals through NCCT
            // (and NCCS for a fourth). [obs: packet colours]
            gte.mtc2(dreg::RGBC, mode << 24 | 0x80_8080);
            for k in 0..3 {
                let (xy, z) = pair(&*np.add(normal_ix(k)));
                gte.mtc2(dreg::VXY0 + 2 * k, xy);
                gte.mtc2(dreg::VZ0 + 2 * k, z);
            }
            gte.execute(cmd::NCCT);
            let mut rgb = [0u32; 4];
            rgb[0] = gte.mfc2(dreg::RGB0);
            rgb[1] = gte.mfc2(dreg::RGB1);
            rgb[2] = gte.mfc2(dreg::RGB2);
            if nv == 4 {
                let (xy, z) = pair(&*np.add(normal_ix(3)));
                gte.mtc2(dreg::VXY0, xy);
                gte.mtc2(dreg::VZ0, z);
                gte.execute(cmd::NCCS);
                rgb[3] = gte.mfc2(dreg::RGB2);
            }

            // Packet: tag, then (colour, XY, texture word) per vertex. [doc: GP0(34h/3Ch)]
            let len = 3 * nv as u32;
            for k in 0..nv {
                let w = pk.add(1 + 3 * k);
                w.write(rgb[k]);
                w.add(1).write(sxy[k]);
                w.add(2).write(tex.add(k).read());
            }
            // OT insertion at the head of the list. [obs: changed OT words]
            let slot = ot_org.add(ot_index as usize);
            let old = slot.read();
            pk.write(len << 24 | (old & 0x00FF_FFFF));
            slot.write((old & 0xFF00_0000) | link_of(pk.cast()));
            pk = pk.add(1 + len as usize);
        }
        pk
    }
}
