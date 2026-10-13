//! libgs: `GsSortObject4J`, the subset DW1's character models need (S0.7 spike).
//!
//! Written clean-room from public documentation and black-box observation of the real SDK in an
//! emulator (`captures/oracle-runs/libgs-pin`, gitignored): psx-spx for the TMD format
//! (CDROM File Formats: TMD), the GTE commands and the GPU packet layouts; the API headers for
//! struct layouts only. No SDK code was read or translated.
//!
//! What the 48 captured calls use: every object has `attribute` 0 and only two primitive kinds,
//! both light-calculated, single-sided, Gouraud textured polygons: mode 0x34 (GT3, 6 input
//! words) and mode 0x3C (GT4, 8 input words), flag 0. Anything else returns
//! [`SortError::Unsupported`].
//!
//! Each rule below says where it comes from: `[doc]` psx-spx, `[obs]` the captures.

use backend::gte::{Gte, cmd, dreg};

/// Address of the game's `GsOUT_PACKET_P` (u32): where the next packet is written. [obs] The
/// value advances by each packet's size during a call. (This is a US-build symbol; in the port
/// it becomes a field of the Rust libgs state.)
pub const GS_OUT_PACKET_P: u32 = 0x8013_6FE4;

/// The GP0 command byte of the 0x34 and 0x3C polygon modes [doc: GP0(30h..3Fh)].
const MODE_GT3: u8 = 0x34;
const MODE_GT4: u8 = 0x3C;

/// A PS1 address space as the SDK sees it.
pub trait Memory {
    fn read8(&self, addr: u32) -> u8;
    fn read16(&self, addr: u32) -> u16;
    fn read32(&self, addr: u32) -> u32;
    fn write32(&mut self, addr: u32, value: u32);
}

/// Main RAM (2 MiB, mirrored through KUSEG/KSEG0/KSEG1) and the 1 KiB scratchpad. A spike
/// fixture: an access outside both panics (the port works on native memory instead).
pub struct Ps1Memory {
    pub ram: Vec<u8>,
    pub scratch: [u8; 1024],
}

pub const RAM_SIZE: usize = 2 << 20;
const SCRATCH_BASE: u32 = 0x1F80_0000;

impl Default for Ps1Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Ps1Memory {
    pub fn new() -> Self {
        Ps1Memory {
            ram: vec![0; RAM_SIZE],
            scratch: [0; 1024],
        }
    }

    /// The backing bytes at `addr` (`len` of them): RAM through any segment, or the scratchpad.
    fn slice(&self, addr: u32, len: usize) -> &[u8] {
        let phys = addr & 0x1FFF_FFFF;
        if phys < 0x0080_0000 {
            let o = phys as usize & (RAM_SIZE - 1);
            &self.ram[o..o + len]
        } else if (SCRATCH_BASE..SCRATCH_BASE + 0x400).contains(&phys) {
            let o = (phys - SCRATCH_BASE) as usize;
            &self.scratch[o..o + len]
        } else {
            panic!("libgs memory view: address {addr:#010x} is neither RAM nor scratchpad")
        }
    }

    fn slice_mut(&mut self, addr: u32, len: usize) -> &mut [u8] {
        let phys = addr & 0x1FFF_FFFF;
        if phys < 0x0080_0000 {
            let o = phys as usize & (RAM_SIZE - 1);
            &mut self.ram[o..o + len]
        } else if (SCRATCH_BASE..SCRATCH_BASE + 0x400).contains(&phys) {
            let o = (phys - SCRATCH_BASE) as usize;
            &mut self.scratch[o..o + len]
        } else {
            panic!("libgs memory view: address {addr:#010x} is neither RAM nor scratchpad")
        }
    }
}

impl Memory for Ps1Memory {
    fn read8(&self, addr: u32) -> u8 {
        self.slice(addr, 1)[0]
    }
    fn read16(&self, addr: u32) -> u16 {
        u16::from_le_bytes(self.slice(addr, 2).try_into().unwrap())
    }
    fn read32(&self, addr: u32) -> u32 {
        u32::from_le_bytes(self.slice(addr, 4).try_into().unwrap())
    }
    fn write32(&mut self, addr: u32, value: u32) {
        self.slice_mut(addr, 4)
            .copy_from_slice(&value.to_le_bytes());
    }
}

/// Why a sort stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortError {
    /// A primitive kind outside the subset (mode and flag bytes of its header).
    Unsupported { mode: u8, flag: u8 },
    /// A header's input size disagrees with the primitive kind's size, or a run's count is 0 or
    /// runs past the object's primitive count.
    BadHeader { addr: u32, header: u32 },
    /// An object attribute outside the subset (only 0 is observed).
    UnsupportedAttribute(u32),
    /// A vertex or normal index past its list.
    BadIndex { prim: u32, index: u32 },
    /// A primitive's `OTZ >> shift` is past the end of the ordering table. What the SDK does here
    /// is unobserved, so the sort stops instead of writing outside the table.
    OtOverflow { index: u32, entries: u32 },
}

/// One decoded primitive of a TMD object.
struct Prim {
    mode: u8,
    /// Address of the packet data (after the header word).
    data: u32,
}

impl Prim {
    fn is_quad(&self) -> bool {
        self.mode == MODE_GT4
    }
}

/// Input words of a lit, Gouraud, textured polygon: one texture word per vertex, then one
/// (normal, vertex) index pair word per vertex. [doc: TMD "Packet Data (for Polygons)"; obs: no
/// command/colour word, because the colour comes from the lighting. GT3 6 words, GT4 8 words,
/// matching the headers' size byte.]
fn input_words(mode: u8, flag: u8) -> Result<u32, SortError> {
    match (mode, flag) {
        (MODE_GT3, 0) => Ok(6),
        (MODE_GT4, 0) => Ok(8),
        _ => Err(SortError::Unsupported { mode, flag }),
    }
}

/// Decodes a TMD object's primitive list.
///
/// [obs] The list is stored in runs of equal primitives. A header is `olen | ilen << 8 |
/// flag << 16 | mode << 24` [doc]. A header whose ilen is 0 starts a run: its low 16 bits are the
/// run's primitive count and the first primitive's data follows at once (its size comes from the
/// mode); the other primitives of the run each have an ordinary header. The object's primitive
/// count (`n_prim`) counts primitives, not headers.
fn read_prims<M: Memory>(mem: &M, prim_top: u32, n_prim: u32) -> Result<Vec<Prim>, SortError> {
    let mut prims = Vec::with_capacity(n_prim as usize);
    let mut addr = prim_top;
    let mut left = n_prim;
    while left > 0 {
        let header = mem.read32(addr);
        let (ilen, flag, mode) = (
            (header >> 8) as u8,
            (header >> 16) as u8,
            (header >> 24) as u8,
        );
        let (count, first_is_run_header) = if ilen == 0 {
            (header & 0xFFFF, true)
        } else {
            (1, false)
        };
        if count == 0 || count > left {
            return Err(SortError::BadHeader { addr, header });
        }
        for k in 0..count {
            let want = input_words(mode, flag)?;
            if !(first_is_run_header && k == 0) {
                // An ordinary header: check it says what the mode implies.
                let h = mem.read32(addr);
                if (h >> 8) as u8 != want as u8
                    || (h >> 16) as u8 != flag
                    || (h >> 24) as u8 != mode
                {
                    return Err(SortError::BadHeader { addr, header: h });
                }
            }
            prims.push(Prim {
                mode,
                data: addr + 4,
            });
            addr += 4 + 4 * want;
            left -= 1;
        }
    }
    Ok(prims)
}

/// `GsSortObject4J(objp, otp, shift, scratch)`: transforms, lights and clips the object's
/// polygons, writes GPU packets from `GsOUT_PACKET_P` on, and links each into the ordering table
/// `otp` at index `OTZ >> shift`. The GTE must already hold the object's local-to-screen matrix
/// and translation, the light matrices, the background colour, OFX/OFY/H and ZSF3/ZSF4 (the game
/// sets them before the call). `scratch` (the scratchpad work area) is not needed by this subset.
pub fn sort_object4j<M: Memory>(
    mem: &mut M,
    gte: &mut Gte,
    objp: u32,
    otp: u32,
    shift: u32,
    _scratch: u32,
) -> Result<(), SortError> {
    // GsDOBJ2 { attribute, coord2, tmd, id }; tmd points at the TMD object-table entry.
    let attribute = mem.read32(objp);
    if attribute != 0 {
        return Err(SortError::UnsupportedAttribute(attribute));
    }
    let tmd = mem.read32(objp + 8);
    let vert_top = mem.read32(tmd);
    let n_vert = mem.read32(tmd + 4);
    let normal_top = mem.read32(tmd + 8);
    let n_normal = mem.read32(tmd + 12);
    let prim_top = mem.read32(tmd + 16);
    let n_prim = mem.read32(tmd + 20);
    // GsOT { length, org, offset, point, tag }: 2^length entries from org.
    let ot_entries = 1u32.checked_shl(mem.read32(otp)).unwrap_or(0);
    let ot_org = mem.read32(otp + 4);

    // Indices are checked before anything is written.
    let prims = read_prims(&*mem, prim_top, n_prim)?;
    for p in &prims {
        let nv = if p.is_quad() { 4 } else { 3 };
        for k in 0..nv {
            let w = mem.read32(p.data + 4 * (nv + k));
            let (normal, vertex) = (w & 0xFFFF, w >> 16);
            if normal >= n_normal || vertex >= n_vert {
                return Err(SortError::BadIndex {
                    prim: p.data - 4,
                    index: w,
                });
            }
        }
    }

    let mut out = mem.read32(GS_OUT_PACKET_P);

    for p in &prims {
        let nv: usize = if p.is_quad() { 4 } else { 3 };
        let tex = p.data;
        let idx = p.data + 4 * nv as u32;
        // [doc] each index word is (normal, vertex) u16s, normal at the lower address.
        let normal_ix = |k: usize| u32::from(mem.read16(idx + 4 * k as u32));
        let vertex_ix = |k: usize| u32::from(mem.read16(idx + 4 * k as u32 + 2));

        // Vertices 0..2 through RTPT, then the back-face test on them. [doc: RTPT, NCLIP]
        for k in 0..3 {
            let (xy, z) = vec3(mem, vert_top, vertex_ix(k));
            gte.mtc2(dreg::VXY0 + 2 * k, xy);
            gte.mtc2(dreg::VZ0 + 2 * k, z);
        }
        gte.execute(cmd::RTPT);
        gte.execute(cmd::NCLIP);
        let nclip = gte.mfc2(dreg::MAC0) as i32;
        // Back-face culling [obs: flag 0 = "clip back"]: NCLIP on the first three vertices;
        // a result of 0 or less is dropped (a quad too, though its fourth vertex might face front).
        // The GTE state left behind by a culled primitive is not part of the contract.
        if nclip <= 0 {
            continue;
        }

        let mut sxy: Vec<u32> = (0..3).map(|k| gte.mfc2(dreg::SXY0 + k)).collect();
        if p.is_quad() {
            let (xy, z) = vec3(mem, vert_top, vertex_ix(3));
            gte.mtc2(dreg::VXY0, xy);
            gte.mtc2(dreg::VZ0, z);
            gte.execute(cmd::RTPS);
            sxy.push(gte.mfc2(dreg::SXY2));
            gte.execute(cmd::AVSZ4);
        } else {
            gte.execute(cmd::AVSZ3);
        }
        let otz = gte.mfc2(dreg::OTZ);
        let ot_index = otz >> shift;
        if ot_index >= ot_entries {
            // Packets already linked stay live: keep the pointer past them.
            mem.write32(GS_OUT_PACKET_P, out);
            return Err(SortError::OtOverflow {
                index: ot_index,
                entries: ot_entries,
            });
        }

        // Lighting: RGBC = code | 0x808080 [obs: RGBC after each call]; normals through NCCT
        // (and NCCS for a fourth). [obs: packet colours]
        gte.mtc2(dreg::RGBC, u32::from(p.mode) << 24 | 0x80_8080);
        for k in 0..3 {
            let (xy, z) = vec3(mem, normal_top, normal_ix(k));
            gte.mtc2(dreg::VXY0 + 2 * k, xy);
            gte.mtc2(dreg::VZ0 + 2 * k, z);
        }
        gte.execute(cmd::NCCT);
        let mut rgb = vec![
            gte.mfc2(dreg::RGB0),
            gte.mfc2(dreg::RGB1),
            gte.mfc2(dreg::RGB2),
        ];
        if p.is_quad() {
            let (xy, z) = vec3(mem, normal_top, normal_ix(3));
            gte.mtc2(dreg::VXY0, xy);
            gte.mtc2(dreg::VZ0, z);
            gte.execute(cmd::NCCS);
            rgb.push(gte.mfc2(dreg::RGB2));
        }

        // Packet: tag, then (colour, XY, texture word) per vertex. [doc: GP0(34h/3Ch)]
        let len = 3 * nv as u32;
        let base = out;
        for k in 0..nv {
            let w = base + 4 + 12 * k as u32;
            mem.write32(w, rgb[k]);
            mem.write32(w + 4, sxy[k]);
            mem.write32(w + 8, mem.read32(tex + 4 * k as u32));
        }
        // OT insertion at the head of the list. [obs: changed OT words]
        let slot = ot_org + 4 * ot_index;
        let old = mem.read32(slot);
        mem.write32(base, len << 24 | (old & 0x00FF_FFFF));
        mem.write32(slot, (old & 0xFF00_0000) | (base & 0x00FF_FFFF));
        out = base + 4 + 4 * len;
    }

    mem.write32(GS_OUT_PACKET_P, out);
    Ok(())
}

/// Entry `i` of a vertex or normal list (8 bytes: s16 x, y, z and 2 unused [doc: TMD]) as the
/// GTE's packed (XY, Z) register pair.
fn vec3<M: Memory>(mem: &M, top: u32, i: u32) -> (u32, u32) {
    let a = top + 8 * i;
    (
        u32::from(mem.read16(a)) | u32::from(mem.read16(a + 2)) << 16,
        u32::from(mem.read16(a + 4)),
    )
}
