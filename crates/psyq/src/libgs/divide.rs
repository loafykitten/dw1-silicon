//! libgs: the dividing TMD handlers, `GsTMDdivTNF3`, `GsTMDdivTNF4`, `GsTMDdivTNG3` and
//! `GsTMDdivTNG4` (unlit flat or Gouraud textured triangles and quads under attribute `GsDIV1`:
//! the arenas' floors in tournaments and Battle Mode).
//!
//! Written clean-room from black-box observation of the real SDK in an emulator: synthetic objects
//! sorted by the real `GsSortObject4J` with attribute 0x200 through `oracle/call-sdk.lua`
//! (`tools/oracle/tmd_requests.py --set div|explore`, runs `p9-render-s4j-div` and
//! `p9-render-explore`), reading back the packets, the ordering table and the scratchpad work
//! area. No SDK code was read or translated.
//!
//! What a dividing handler does with each primitive [obs]:
//! - It works in the scratchpad area the sort was given (`scratch`): a header (the divide level,
//!   the screen width and height, the CLUT and texture page halves of the first two texture
//!   words, the colour word with the command, the ordering-table entry's address), then one
//!   24-byte record per vertex: the local position (x, y; z), the texture word, the colour word,
//!   the screen XY and depth. The originals come first (their words as stored), then the
//!   midpoints the division makes (only the bytes that carry a value are written: the rest keeps
//!   what the scratchpad held), then the addresses of the original records.
//! - The whole polygon is projected and tested as the non-dividing handlers do (FLAG errors, the
//!   back-face test on its first three vertices), and its depth gives one ordering-table entry
//!   for all its pieces.
//! - Level 1 halves every edge: a midpoint's position, texture coordinates and colour are the
//!   halves of its ends' sums, and it is projected like a vertex. A triangle makes four
//!   triangles, a quad four quads, each drawn as the GPU primitive of its kind, in a fixed order.
//! - A piece's first texture word carries the CLUT, its second the texture page, the others
//!   nothing in their high halves; a Gouraud piece's first colour word gets the command in its
//!   high byte (written into that vertex's record), the others are the records' words as they
//!   stand, whose high bytes are whatever the scratchpad held there.
//!
//! The scratchpad bytes are the game's own natively (`dw1_scratchpad`), so the pieces and the
//! work area come out as the PS1 has them. Addresses in the work area are written as the PS1's.

use super::tmd::{Kind, ot_index, pair};
use super::{GsOt, Vert};
use crate::memory::{PS1_RAM_SIZE, dw1_ps1_ram, dw1_scratchpad, link_of};
use crate::trap::unimplemented;
use backend::gte::{Gte, cmd, dreg};

const SCRATCHPAD_PS1: u32 = 0x1F80_0000;
const HEADER: usize = 0x18;
const RECORD: usize = 0x18;

/// Record fields' offsets.
const R_XY: usize = 0;
const R_Z: usize = 4;
const R_UV: usize = 8;
const R_RGB: usize = 12;
const R_SXY: usize = 16;
const R_SZ: usize = 20;

/// The scratchpad work area.
struct Work(*mut u8);

impl Work {
    unsafe fn w32(&self, off: usize, v: u32) {
        unsafe { self.0.add(off).cast::<u32>().write_unaligned(v) }
    }
    unsafe fn r32(&self, off: usize) -> u32 {
        unsafe { self.0.add(off).cast::<u32>().read_unaligned() }
    }
    unsafe fn w16(&self, off: usize, v: u16) {
        unsafe { self.0.add(off).cast::<u16>().write_unaligned(v) }
    }
    unsafe fn s16(&self, off: usize) -> i32 {
        i32::from(unsafe { self.0.add(off).cast::<i16>().read_unaligned() })
    }
    unsafe fn w8(&self, off: usize, v: u8) {
        unsafe { self.0.add(off).write(v) }
    }
    unsafe fn r8(&self, off: usize) -> i32 {
        i32::from(unsafe { self.0.add(off).read() })
    }
    fn rec(i: usize) -> usize {
        HEADER + RECORD * i
    }
}

/// The PS1 address of an ordering table's first entry, as the SDK stores entry addresses: its
/// address when the table is in the PS1 RAM stand-in (as in the oracle tests), else its link with
/// KSEG0's top bit (the game's tables are native globals; nothing reads the value back).
fn ps1_address(slot: *mut u32) -> u32 {
    let base = (&raw mut dw1_ps1_ram).cast::<u8>() as usize;
    let at = slot as usize;
    if (base..base + PS1_RAM_SIZE).contains(&at) {
        0x8000_0000 | (at - base) as u32
    } else {
        0x8000_0000 | link_of(slot.cast())
    }
}

/// Writes the midpoint of records `a` and `b` into record `m`: position, texture coordinates
/// and (Gouraud) colour, each half the ends' sum; only those bytes.
unsafe fn midpoint(work: &Work, gouraud: bool, a: usize, b: usize, m: usize) {
    unsafe {
        let half = |x: i32, y: i32| (x + y) >> 1;
        for off in [R_XY, R_XY + 2, R_Z] {
            work.w16(m + off, half(work.s16(a + off), work.s16(b + off)) as u16);
        }
        for off in [R_UV, R_UV + 1] {
            work.w8(m + off, half(work.r8(a + off), work.r8(b + off)) as u8);
        }
        if gouraud {
            for off in [R_RGB, R_RGB + 1, R_RGB + 2] {
                work.w8(m + off, half(work.r8(a + off), work.r8(b + off)) as u8);
            }
        }
    }
}

/// Whether a polygon's screen positions are all left of, right of, above or below the screen:
/// further than half the screen's width or height from the GTE's screen offset (OFX, OFY).
fn off_screen(gte: &Gte, sxy: &[u32], width: u32, height: u32) -> bool {
    let (ofx, ofy) = (gte.cfc2(24) as i32 >> 16, gte.cfc2(25) as i32 >> 16);
    let (hw, hh) = (width as i32 / 2, height as i32 / 2);
    let xs = sxy.iter().map(|&p| i32::from(p as i16) - ofx);
    let ys = sxy.iter().map(|&p| i32::from((p >> 16) as i16) - ofy);
    xs.clone().all(|x| x < -hw)
        || xs.clone().all(|x| x > hw)
        || ys.clone().all(|y| y < -hh)
        || ys.clone().all(|y| y > hh)
}

/// Writes a quad's centre into record 8: each value a quarter of the four corners' sum.
unsafe fn centre(work: &Work, gouraud: bool) {
    unsafe {
        let m = Work::rec(8);
        let corners = [Work::rec(0), Work::rec(1), Work::rec(2), Work::rec(3)];
        for off in [R_XY, R_XY + 2, R_Z] {
            let sum: i32 = corners.iter().map(|&c| work.s16(c + off)).sum();
            work.w16(m + off, (sum >> 2) as u16);
        }
        let bytes: &[usize] = if gouraud {
            &[R_UV, R_UV + 1, R_RGB, R_RGB + 1, R_RGB + 2]
        } else {
            &[R_UV, R_UV + 1]
        };
        for &off in bytes {
            let sum: i32 = corners.iter().map(|&c| work.r8(c + off)).sum();
            work.w8(m + off, (sum >> 2) as u8);
        }
    }
}

/// One run of dividing primitives of `kind` (flag 1, mode `base_mode` or with the
/// semi-transparency bit).
#[allow(clippy::too_many_arguments)]
unsafe fn div_run(
    gte: &mut Gte,
    name: &str,
    kind: Kind,
    base_mode: u8,
    mut op: *const u32,
    vp: *const Vert,
    mut pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
    scratch: *mut u32,
) -> *mut u32 {
    unsafe {
        let level = super::sort4j::NDIV.load(std::sync::atomic::Ordering::Relaxed);
        if level != 1 {
            unimplemented("GsTMDdiv* with a divide level other than 1");
        }
        if scratch != (&raw mut dw1_scratchpad).cast() {
            unimplemented("GsTMDdiv* with a work area other than the scratchpad");
        }
        let work = Work(scratch.cast());
        let (width, height) = super::display_size();
        let ot_entries = 1u32.checked_shl((*ot).length).unwrap_or(0);
        let ot_org = (*ot).org;
        let offset = (*ot).offset;
        let nv = kind.nv;
        let words = kind.index_at() + (kind.vertex_slot(nv - 1) + 2) / 2;
        for _ in 0..n {
            let header = op.read();
            let (flag, mode) = ((header >> 16) as u8, (header >> 24) as u8);
            if flag != 1 || mode & !0x02 != base_mode {
                eprintln!("psyq: TMD mode {mode:#04x} flag {flag:#04x} in the {name} handler");
                unimplemented("GsTMDdiv* with unobserved mode or flag bits");
            }
            let p = op;
            op = op.add(words);
            let code = mode & !1;
            let tex = p.add(1);
            let colours = p.add(kind.colour_at());
            let idx = p.add(kind.index_at()).cast::<u16>();
            let vertex = |k: usize| &*vp.add(usize::from(idx.add(k).read()));

            let count = nv + if nv == 4 { 5 } else { 3 };
            // [obs] First the level and screen size, the originals' positions and the stack of
            // their records' addresses; then the back-face test.
            work.w32(0, level);
            work.w32(4, width);
            work.w32(8, height);
            for k in 0..nv {
                let r = Work::rec(k);
                let v = vertex(k);
                work.w32(r + R_XY, pair(v).0);
                work.w32(
                    r + R_Z,
                    u32::from(v.vz as u16) | u32::from(v.pad as u16) << 16,
                );
                work.w32(Work::rec(count) + 4 * k, SCRATCHPAD_PS1 + r as u32);
            }
            for k in 0..3 {
                let (xy, z) = pair(vertex(k));
                gte.mtc2(dreg::VXY0 + 2 * k, xy);
                gte.mtc2(dreg::VZ0 + 2 * k, z);
            }
            gte.execute(cmd::RTPT);
            let mut sxy = [0u32; 4];
            let mut sz = [0u32; 4];
            for k in 0..3 {
                sxy[k] = gte.mfc2(dreg::SXY0 + k);
                sz[k] = gte.mfc2(dreg::SZ1 + k);
            }
            gte.execute(cmd::NCLIP);
            if gte.mfc2(dreg::MAC0) as i32 <= 0 {
                continue;
            }
            if nv == 4 {
                let (xy, z) = pair(vertex(3));
                gte.mtc2(dreg::VXY0, xy);
                gte.mtc2(dreg::VZ0, z);
                gte.execute(cmd::RTPS);
                sxy[3] = gte.mfc2(dreg::SXY2);
                sz[3] = gte.mfc2(dreg::SZ3);
                gte.execute(cmd::AVSZ4);
            } else {
                gte.execute(cmd::AVSZ3);
            }
            // Then the rest of the header and of the originals' records, and the
            // ordering-table entry's address.
            work.w32(0x0C, (tex.read() >> 16) | (tex.add(1).read() & 0xFFFF_0000));
            if kind.gouraud {
                work.w8(0x13, code);
            } else {
                work.w32(0x10, u32::from(code) << 24 | (colours.read() & 0x00FF_FFFF));
            }
            for k in 0..nv {
                let r = Work::rec(k);
                work.w32(r + R_UV, tex.add(k).read());
                if kind.gouraud {
                    work.w32(r + R_RGB, colours.add(k).read());
                }
                work.w32(r + R_SXY, sxy[k]);
                work.w32(r + R_SZ, sz[k]);
            }
            let index = ot_index(gte.mfc2(dreg::OTZ), offset, shift);
            let slot = ot_org.wrapping_add(index as usize);
            work.w32(
                0x14,
                ps1_address(ot_org).wrapping_add(index.wrapping_mul(4)),
            );
            // [obs] A polygon wholly off the screen, or wholly nearer than H/2 (rounded down), is
            // dropped. Projection errors (FLAG) don't drop it, unlike the non-dividing handlers.
            let h = gte.cfc2(26) & 0xFFFF;
            let near = sz[..nv].iter().all(|&z| z < h >> 1);
            if off_screen(gte, &sxy[..nv], width, height) || near {
                continue;
            }
            if index >= ot_entries {
                unimplemented("GsTMDdiv* with OTZ past the ordering table");
            }

            // The midpoints (record numbers after the originals), then a quad's centre (a quarter
            // of the corners' sums).
            let mids: &[(usize, usize)] = if nv == 4 {
                &[(0, 1), (0, 2), (1, 3), (2, 3)]
            } else {
                &[(0, 1), (1, 2), (0, 2)]
            };
            for (m, &(a, b)) in mids.iter().enumerate() {
                midpoint(
                    &work,
                    kind.gouraud,
                    Work::rec(a),
                    Work::rec(b),
                    Work::rec(nv + m),
                );
            }
            if nv == 4 {
                centre(&work, kind.gouraud);
            }
            for i in nv..count {
                let r = Work::rec(i);
                gte.mtc2(dreg::VXY0, work.r32(r + R_XY));
                gte.mtc2(dreg::VZ0, work.r32(r + R_Z) & 0xFFFF);
                gte.execute(cmd::RTPS);
                work.w32(r + R_SXY, gte.mfc2(dreg::SXY2));
                // [obs] Which midpoints' depths are kept differs by handler.
                let keep = match (nv, kind.gouraud) {
                    (3, _) => false,
                    (_, false) => !matches!(i, 6 | 7),
                    (_, true) => true,
                };
                if keep {
                    work.w32(r + R_SZ, gte.mfc2(dreg::SZ3));
                }
            }
            // The pieces, in the SDK's order (record numbers).
            let pieces: &[[usize; 4]] = if nv == 4 {
                // v0 v1 v2 v3, m01 m02 m13 m23, centre
                &[[0, 4, 5, 8], [1, 6, 4, 8], [2, 5, 7, 8], [3, 7, 6, 8]]
            } else {
                // v0 v1 v2, m01 m12 m02
                &[[1, 4, 3, 0], [3, 4, 5, 0], [0, 3, 5, 0], [2, 5, 4, 0]]
            };
            let clut_tpage = work.r32(0x0C);
            let len = if kind.gouraud { 3 * nv } else { 2 * nv + 1 };
            for piece in pieces {
                let mut w = pk.add(1);
                let mut put = |v: u32| {
                    w.write(v);
                    w = w.add(1);
                };
                if kind.gouraud {
                    work.w8(Work::rec(piece[0]) + R_RGB + 3, code);
                } else {
                    put(work.r32(0x10));
                }
                for (k, &rv) in piece.iter().take(nv).enumerate() {
                    let r = Work::rec(rv);
                    if kind.gouraud {
                        put(work.r32(r + R_RGB));
                    }
                    put(work.r32(r + R_SXY));
                    let uv = work.r32(r + R_UV) & 0xFFFF;
                    put(match k {
                        0 => uv | clut_tpage << 16,
                        1 => uv | (clut_tpage & 0xFFFF_0000),
                        _ => uv,
                    });
                }
                let old = slot.read();
                pk.write((len as u32) << 24 | (old & 0x00FF_FFFF));
                slot.write((old & 0xFF00_0000) | link_of(pk.cast()));
                pk = pk.add(1 + len);
            }
        }
        pk
    }
}

macro_rules! div_handlers {
    ($($(#[$doc:meta])* $name:ident: $mode:expr;)*) => {
        $(
            $(#[$doc])*
            ///
            /// # Safety
            /// As `GsSortObject4J`'s; `scratch` is the scratchpad.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(
                op: *const u32,
                vp: *const Vert,
                _np: *const Vert,
                pk: *mut u32,
                n: i32,
                shift: i32,
                ot: *mut GsOt,
                scratch: *mut u32,
            ) -> *mut u32 {
                let kind = Kind::of(1, $mode);
                unsafe {
                    crate::gte::with(|gte| {
                        div_run(gte, stringify!($name), kind, $mode, op, vp, pk, n, shift, ot, scratch)
                    })
                }
            }
        )*
    };
}

div_handlers! {
    /// `GsTMDdivTNF3`: unlit flat textured triangles, divided (mode 0x25 or 0x27, flag 1).
    GsTMDdivTNF3: 0x25;
    /// `GsTMDdivTNF4`: unlit flat textured quads, divided (mode 0x2D or 0x2F).
    GsTMDdivTNF4: 0x2D;
    /// `GsTMDdivTNG3`: unlit Gouraud textured triangles, divided (mode 0x35 or 0x37).
    GsTMDdivTNG3: 0x35;
    /// `GsTMDdivTNG4`: unlit Gouraud textured quads, divided (mode 0x3D or 0x3F).
    GsTMDdivTNG4: 0x3D;
}
