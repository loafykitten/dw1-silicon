//! libgs: the TMD primitive handlers `GsSortObject4J` dispatches to through `GsFCALL4`, the
//! non-dividing ("fast") ones. `divide.rs` has the dividing ones.
//!
//! Written clean-room from public documentation and black-box observation of the real SDK in an
//! emulator: psx-spx for the TMD format (CDROM File Formats: TMD), the GTE commands and the GPU
//! packet layouts; the captured calls of the S0.7 spike and Phase 2/5 (`libgs-pin`, `s4j-*`);
//! and synthetic objects sorted by the real `GsSortObject4J` through `oracle/call-sdk.lua`
//! (`tools/oracle/tmd_requests.py`, runs `p9-render-s4j-{fast,bits}`: every primitive kind the
//! disc's TMDs use, under random matrices, lights, positions and tables). No SDK code was read or
//! translated.
//!
//! The kinds, from the TMD header's mode byte (bit 0 raw texture, 1 semi-transparent, 2
//! textured, 3 four vertices, 4 Gouraud; bits 5-7 = 1, a polygon) and flag byte (bit 0 no light
//! calculation) [doc]:
//!
//! | handler | kind | disc modes, flag |
//! |---|---|---|
//! | `GsTMDfastF3L` / `F4L` | lit flat triangle / quad | 0x20 / 0x28, 0 |
//! | `GsTMDfastG4L` | lit Gouraud quad | 0x38, 0 |
//! | `GsTMDfastTF3L` / `TF4L` | lit flat textured | 0x24, 0x26 / 0x2C, 0x2E, 0 |
//! | `GsTMDfastTG3L` / `TG4L` | lit Gouraud textured | 0x34, 0x36 / 0x3C, 0x3E, 0 |
//! | `GsTMDfastTNF3` / `TNF4` | unlit flat textured | 0x25, 0x27 / 0x2D, 0x2F, 1 |
//! | `GsTMDfastTNG3` / `TNG4` | unlit Gouraud textured | 0x35 / 0x3D, 0x3F, 1 |
//!
//! Each handler takes those and the same with the semi-transparency bit flipped (all observed on
//! the emulator). The raw-texture bit is set on every unlit primitive on the disc and clear on
//! every lit one; the other way round the real `GsSortObject4J` runs away (`--set bits`, first
//! version), so it stops the program here, as does any other flag bit (two-sided, gradation).
//!
//! What every handler does to each primitive of its run [obs]:
//! - vertices 0-2 through RTPT, then NCLIP: 0 or less drops the primitive (a quad is judged on
//!   its first triangle);
//! - a quad's fourth vertex through RTPS, then AVSZ4; a triangle's AVSZ3. The packet goes at the
//!   head of ordering-table entry `(OTZ - GsOT.offset) >> shift`, whatever the table's length
//!   (an index past it, or an OTZ below the offset, writes outside the table: natively that
//!   stops the program); `GsOT.point` unused;
//! - the colours: lit kinds through NCCS (flat: the first normal) or NCCT and NCCS (Gouraud: one
//!   normal per vertex), with RGBC = the mode byte as the GPU command over the primitive's colour
//!   word's red, green and blue (textured: 0x80 each, they have no colour word); unlit kinds
//!   copy their colour words, the first with the command (the mode byte without its raw-texture
//!   bit) over its high byte, the others as stored;
//! - the packet: the GPU primitive of the same shape (`POLY_F3` .. `POLY_GT4`): flat ones the
//!   colour then per vertex its XY (and texture word, as stored); Gouraud ones per vertex colour,
//!   XY (and texture word).
//!
//! Natively the primitive data holds the PS1's bytes, the packet area is `GsGetWorkBase()`'s, and
//! ordering-table links are 24-bit offsets in the link window (`memory::link_of`).

use super::{GsOt, Vert};
use crate::memory::link_of;
use crate::trap::unimplemented;
use backend::gte::{Gte, cmd, dreg};

/// A TMD polygon's shape, from its mode and flag bytes [doc: TMD packet layouts].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Kind {
    pub nv: usize,
    pub lit: bool,
    pub textured: bool,
    pub gouraud: bool,
}

impl Kind {
    pub fn of(flag: u8, mode: u8) -> Kind {
        Kind {
            nv: if mode & 0x08 != 0 { 4 } else { 3 },
            lit: flag & 1 == 0,
            textured: mode & 0x04 != 0,
            gouraud: mode & 0x10 != 0,
        }
    }

    /// Word offset (from the header) of the colour words: after the texture words.
    fn colour_at(self) -> usize {
        1 + if self.textured { self.nv } else { 0 }
    }

    /// How many colour words: none for lit textured, one per vertex for unlit Gouraud, else one.
    fn colours(self) -> usize {
        if self.lit && self.textured {
            0
        } else if !self.lit && self.gouraud {
            self.nv
        } else {
            1
        }
    }

    /// Word offset of the indices (halfwords, the first at the lower address).
    pub fn index_at(self) -> usize {
        self.colour_at() + self.colours()
    }

    /// Halfword position of vertex `k`'s index: (normal, vertex) pairs for lit Gouraud; one
    /// normal then the vertices for lit flat; the vertices alone unlit.
    pub fn vertex_slot(self, k: usize) -> usize {
        match (self.lit, self.gouraud) {
            (true, true) => 2 * k + 1,
            (true, false) => 1 + k,
            (false, _) => k,
        }
    }

    /// Halfword position of the normal used for vertex `k` (lit kinds).
    pub fn normal_slot(self, k: usize) -> usize {
        if self.gouraud { 2 * k } else { 0 }
    }

    /// Word offset (from the tag) of vertex `k`'s texture word in the GPU packet.
    fn uv_at(self, k: usize) -> usize {
        self.xy_at(k) + 1
    }

    /// Word offset (from the tag) of vertex `k`'s XY in the GPU packet.
    fn xy_at(self, k: usize) -> usize {
        let per_vertex = 1 + usize::from(self.textured) + usize::from(self.gouraud);
        1 + usize::from(!self.gouraud) + usize::from(self.gouraud) + k * per_vertex
    }

    /// The GPU packet's words after its tag.
    fn packet_words(self) -> usize {
        let per_vertex = 1 + usize::from(self.textured) + usize::from(self.gouraud);
        self.nv * per_vertex + usize::from(!self.gouraud)
    }
}

/// The transformation of one vertex from a TMD list as the GTE's packed (XY, Z) register pair.
pub(crate) fn pair(v: &Vert) -> (u32, u32) {
    (
        u32::from(v.vx as u16) | u32::from(v.vy as u16) << 16,
        u32::from(v.vz as u16),
    )
}

/// Loads `vs` (up to three) into V0..V2.
fn load(gte: &mut Gte, vs: &[&Vert]) {
    for (k, v) in vs.iter().enumerate() {
        let (xy, z) = pair(v);
        gte.mtc2(dreg::VXY0 + 2 * k, xy);
        gte.mtc2(dreg::VZ0 + 2 * k, z);
    }
}

/// Stops on a header the handler for `kind` doesn't take: the flag must be the kind's (0 lit,
/// 1 unlit) and the mode its own with or without the semi-transparency bit.
fn check_header(name: &str, header: u32, flag: u8, base_mode: u8) {
    let (f, mode) = ((header >> 16) as u8, (header >> 24) as u8);
    if f != flag || mode & !0x02 != base_mode {
        eprintln!("psyq: TMD mode {mode:#04x} flag {f:#04x} in the {name} handler");
        unimplemented("GsTMDfast* with unobserved mode or flag bits");
    }
}

/// The ordering-table entry of a primitive at depth `otz`: `(OTZ - GsOT.offset) >> shift`
/// [obs: p9-render-s4j-fast, random offsets; the game's are always 0]. Below the offset it wraps
/// to a huge index, past the table.
pub(crate) fn ot_index(otz: u32, offset: u32, shift: i32) -> u32 {
    otz.wrapping_sub(offset).checked_shr(shift as u32).unwrap_or(0)
}

/// Packet words a handler writes before its tests [obs: the words a dropped primitive leaves]:
/// the texture words, the first three XYs, the first colour word (unlit: with the command).
/// `early` ones before the projection tests, `late` ones after the back-face test (before a
/// quad's fourth vertex is tested).
const EARLY_UV: u8 = 1;
const EARLY_XY: u8 = 2;
const EARLY_COLOUR: u8 = 4;

/// One run of `n` non-dividing primitives of `kind` (mode `base_mode` or with the
/// semi-transparency bit, flag `flag`), from `op`; returns the packet pointer after them.
#[allow(clippy::too_many_arguments)]
unsafe fn fast_run(
    gte: &mut Gte,
    name: &str,
    early: u8,
    late: u8,
    kind: Kind,
    flag: u8,
    base_mode: u8,
    mut op: *const u32,
    vp: *const Vert,
    np: *const Vert,
    mut pk: *mut u32,
    n: i32,
    shift: i32,
    ot: *mut GsOt,
) -> *mut u32 {
    unsafe {
        let ot_entries = 1u32.checked_shl((*ot).length).unwrap_or(0);
        let ot_org = (*ot).org;
        let offset = (*ot).offset;
        let words = kind.index_at() + (kind.vertex_slot(kind.nv - 1) + 2) / 2;
        let len = kind.packet_words();
        for _ in 0..n {
            let header = op.read();
            check_header(name, header, flag, base_mode);
            let p = op;
            op = op.add(words);
            let mode = header >> 24;
            let idx = p.add(kind.index_at()).cast::<u16>();
            let vertex = |k: usize| &*vp.add(usize::from(idx.add(kind.vertex_slot(k)).read()));
            let normal = |k: usize| &*np.add(usize::from(idx.add(kind.normal_slot(k)).read()));

            load(gte, &[vertex(0), vertex(1), vertex(2)]);
            gte.execute(cmd::RTPT);
            let flag_t = gte.cfc2(31);
            let mut sxy = [0u32; 4];
            for (k, s) in sxy.iter_mut().take(3).enumerate() {
                *s = gte.mfc2(dreg::SXY0 + k);
            }
            // [obs] Some words go into the packet before the tests below (a dropped primitive
            // leaves them past the work base, where the next packet may or may not cover them).
            let tex = p.add(1);
            if early & EARLY_UV != 0 {
                for k in 0..kind.nv {
                    pk.add(kind.uv_at(k)).write(tex.add(k).read());
                }
            }
            if early & EARLY_XY != 0 {
                for (k, &s) in sxy.iter().take(3).enumerate() {
                    pk.add(kind.xy_at(k)).write(s);
                }
            }
            if early & EARLY_COLOUR != 0 {
                pk.add(1).write((mode & !1) << 24 | (p.add(kind.colour_at()).read() & 0x00FF_FFFF));
            }
            if std::env::var_os("DW1_DIAG").is_some() {
                let sz: Vec<u32> = (0..4).map(|k| gte.mfc2(dreg::SZ0 + k)).collect();
                let mut g2 = gte.clone();
                g2.execute(cmd::NCLIP);
                let nclip = g2.mfc2(dreg::MAC0) as i32;
                let mut f4 = 0;
                if kind.nv == 4 {
                    load(&mut g2, &[vertex(3)]);
                    g2.execute(cmd::RTPS);
                    f4 = g2.cfc2(31);
                }
                eprintln!("DIAG mode={mode:02x} flagt={flag_t:08x} flag4={f4:08x} nclip={nclip} sz={sz:?} xy={:08x},{:08x},{:08x}", sxy[0], sxy[1], sxy[2]);
            }
            // [obs] A projection with an error (FLAG bit 31: a coordinate or depth out of range)
            // drops the primitive.
            if flag_t & 0x8000_0000 != 0 {
                continue;
            }
            gte.execute(cmd::NCLIP);
            if gte.mfc2(dreg::MAC0) as i32 <= 0 {
                continue;
            }
            if late & EARLY_XY != 0 {
                for (k, &s) in sxy.iter().take(3).enumerate() {
                    pk.add(kind.xy_at(k)).write(s);
                }
            }
            if late & EARLY_UV != 0 {
                for k in 0..kind.nv {
                    pk.add(kind.uv_at(k)).write(tex.add(k).read());
                }
            }
            if kind.nv == 4 {
                load(gte, &[vertex(3)]);
                gte.execute(cmd::RTPS);
                if gte.cfc2(31) & 0x8000_0000 != 0 {
                    continue;
                }
                sxy[3] = gte.mfc2(dreg::SXY2);
                gte.execute(cmd::AVSZ4);
            } else {
                gte.execute(cmd::AVSZ3);
            }
            let ot_index = ot_index(gte.mfc2(dreg::OTZ), offset, shift);
            if std::env::var_os("DW1_DIAG").is_some() {
                eprintln!("DIAGOTZ {} {}", gte.mfc2(dreg::OTZ), ot_index >= ot_entries);
                if ot_index >= ot_entries {
                    continue;
                }
            }
            if ot_index >= ot_entries {
                // The SDK writes past the table [obs]; natively that's memory we don't own.
                unimplemented("GsTMDfast* with OTZ past the ordering table");
            }

            let colours = p.add(kind.colour_at());
            let mut rgb = [0u32; 4];
            if kind.lit {
                // [obs] Textured: the mode byte over 0x808080. Untextured: the colour word as
                // stored, its high byte (the mode byte on the disc) becoming the command.
                let rgbc = if kind.textured {
                    mode << 24 | 0x80_8080
                } else {
                    colours.read()
                };
                gte.mtc2(dreg::RGBC, rgbc);
                if kind.gouraud {
                    load(gte, &[normal(0), normal(1), normal(2)]);
                    gte.execute(cmd::NCCT);
                    rgb[0] = gte.mfc2(dreg::RGB0);
                    rgb[1] = gte.mfc2(dreg::RGB1);
                    rgb[2] = gte.mfc2(dreg::RGB2);
                    if kind.nv == 4 {
                        load(gte, &[normal(3)]);
                        gte.execute(cmd::NCCS);
                        rgb[3] = gte.mfc2(dreg::RGB2);
                    }
                } else {
                    load(gte, &[normal(0)]);
                    gte.execute(cmd::NCCS);
                    rgb[0] = gte.mfc2(dreg::RGB2);
                }
            } else {
                rgb[0] = (mode & !1) << 24 | (colours.read() & 0x00FF_FFFF);
                if kind.gouraud {
                    for (k, c) in rgb.iter_mut().enumerate().take(kind.nv).skip(1) {
                        *c = colours.add(k).read();
                    }
                }
            }

            let mut w = pk.add(1);
            let mut put = |v: u32| {
                w.write(v);
                w = w.add(1);
            };
            if !kind.gouraud {
                put(rgb[0]);
            }
            for k in 0..kind.nv {
                if kind.gouraud {
                    put(rgb[k]);
                }
                put(sxy[k]);
                if kind.textured {
                    put(tex.add(k).read());
                }
            }
            let slot = ot_org.add(ot_index as usize);
            let old = slot.read();
            pk.write((len as u32) << 24 | (old & 0x00FF_FFFF));
            slot.write((old & 0xFF00_0000) | link_of(pk.cast()));
            pk = pk.add(1 + len);
        }
        pk
    }
}

macro_rules! fast_handlers {
    ($($(#[$doc:meta])* $name:ident: $flag:expr, $mode:expr, $early:expr, $late:expr;)*) => {
        $(
            $(#[$doc])*
            ///
            /// # Safety
            /// As `GsSortObject4J`'s: `op` is `n` linked TMD primitives, `vp`/`np` their object's
            /// lists, `pk` the packet area, `ot` the ordering table.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(
                op: *const u32,
                vp: *const Vert,
                np: *const Vert,
                pk: *mut u32,
                n: i32,
                shift: i32,
                ot: *mut GsOt,
                _scratch: *mut u32,
            ) -> *mut u32 {
                let kind = Kind::of($flag, $mode);
                unsafe {
                    crate::gte::with(|gte| {
                        fast_run(gte, stringify!($name), $early, $late, kind, $flag, $mode, op, vp, np, pk, n, shift, ot)
                    })
                }
            }
        )*
    };
}

fast_handlers! {
    /// `GsTMDfastF3L`: light-calculated flat triangles (mode 0x20, flag 0).
    GsTMDfastF3L: 0, 0x20, 0, 0;
    /// `GsTMDfastF4L`: light-calculated flat quads (mode 0x28).
    GsTMDfastF4L: 0, 0x28, 0, EARLY_XY;
    /// `GsTMDfastG4L`: light-calculated Gouraud quads (mode 0x38; KARRING.TMD's stones).
    GsTMDfastG4L: 0, 0x38, 0, EARLY_XY;
    /// `GsTMDfastTF3L`: light-calculated flat textured triangles (mode 0x24).
    GsTMDfastTF3L: 0, 0x24, EARLY_UV, 0;
    /// `GsTMDfastTF4L`: light-calculated flat textured quads (mode 0x2C).
    GsTMDfastTF4L: 0, 0x2C, 0, EARLY_XY | EARLY_UV;
    /// `GsTMDfastTG3L`: light-calculated Gouraud textured triangles (mode 0x34; the
    /// characters).
    GsTMDfastTG3L: 0, 0x34, 0, 0;
    /// `GsTMDfastTG4L`: light-calculated Gouraud textured quads (mode 0x3C).
    GsTMDfastTG4L: 0, 0x3C, 0, EARLY_XY;
    /// `GsTMDfastTNF3`: unlit flat textured triangles (mode 0x25, flag 1).
    GsTMDfastTNF3: 1, 0x25, EARLY_UV, 0;
    /// `GsTMDfastTNF4`: unlit flat textured quads (mode 0x2D; the battle effects' 0x2F).
    GsTMDfastTNF4: 1, 0x2D, EARLY_COLOUR, EARLY_XY | EARLY_UV;
    /// `GsTMDfastTNG3`: unlit Gouraud textured triangles (mode 0x35).
    GsTMDfastTNG3: 1, 0x35, EARLY_UV, 0;
    /// `GsTMDfastTNG4`: unlit Gouraud textured quads (mode 0x3D).
    GsTMDfastTNG4: 1, 0x3D, 0, EARLY_XY;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libgs::primitive_words;

    /// The layout's word count is the TMD input length (`primitive_words`) for every kind.
    #[test]
    fn layouts_fill_the_primitive() {
        for (flag, mode) in [
            (0, 0x20),
            (0, 0x28),
            (0, 0x38),
            (0, 0x24),
            (0, 0x2C),
            (0, 0x34),
            (0, 0x3C),
            (1, 0x25),
            (1, 0x2D),
            (1, 0x35),
            (1, 0x3D),
        ] {
            let k = Kind::of(flag, mode);
            let words = k.index_at() - 1 + (k.vertex_slot(k.nv - 1) + 2) / 2;
            assert_eq!(Some(words), primitive_words(flag, mode), "{mode:#x}");
        }
    }
}
