//! Model of the PS1 GTE (geometry transformation engine, COP2).
//!
//! Translated from DW2-PC (github.com/Wyrelade/DW2-PC, CC0), `psyq/gte.c` lines 1-392 and
//! `psyq/gte_core.h`, at commit 2d491b5. That part is DW2-PC's original work; the SDK macro
//! sequences after line 392 are not ported. RTPT is added from psx-spx
//! (psx-spx.consoledev.net/geometrytransformationenginegte/) and shares RTPS's per-vertex code.
//!
//! The 32 data and 32 control registers keep their hardware view: a write stores what the
//! register holds (16-bit registers sign or zero extended), a read returns what `mfc2`/`cfc2`
//! would. Commands take the COP2 instruction word, so sf, lm and the MVMVA fields come from the
//! same bits as on the PS1. Results follow psx-spx: 44-bit MAC accumulation with overflow flags,
//! IR / colour / SZ / SXY saturation, the UNR divide, and FLAG bit 31 as the error summary.
//!
//! NCT, NCCS, NCCT, AVSZ3 and AVSZ4 are also from psx-spx (the lighting family shares NCS's
//! two matrix steps; NCCx then multiplies by RGBC). NCDS/NCDT and the other colour commands are
//! not modelled.
//!
//! Not ported: DW2-PC's PG.2 precise-projection hook and matrix hooks (host-side extras that
//! never change the hardware view), and `Lwc2`/`Swc2` (a caller does `mtc2(reg, word)`).

/// Data register indices.
pub mod dreg {
    pub const VXY0: usize = 0;
    pub const VZ0: usize = 1;
    pub const VXY1: usize = 2;
    pub const VZ1: usize = 3;
    pub const VXY2: usize = 4;
    pub const VZ2: usize = 5;
    pub const RGBC: usize = 6;
    pub const OTZ: usize = 7;
    pub const IR0: usize = 8;
    pub const IR1: usize = 9;
    pub const IR2: usize = 10;
    pub const IR3: usize = 11;
    pub const SXY0: usize = 12;
    pub const SXY1: usize = 13;
    pub const SXY2: usize = 14;
    pub const SXYP: usize = 15;
    pub const SZ0: usize = 16;
    pub const SZ1: usize = 17;
    pub const SZ2: usize = 18;
    pub const SZ3: usize = 19;
    pub const RGB0: usize = 20;
    pub const RGB1: usize = 21;
    pub const RGB2: usize = 22;
    pub const RES1: usize = 23;
    pub const MAC0: usize = 24;
    pub const MAC1: usize = 25;
    pub const MAC2: usize = 26;
    pub const MAC3: usize = 27;
    pub const IRGB: usize = 28;
    pub const ORGB: usize = 29;
    pub const LZCS: usize = 30;
    pub const LZCR: usize = 31;
}

/// Control register indices.
pub mod creg {
    pub const RT0: usize = 0;
    pub const TRX: usize = 5;
    pub const TRY: usize = 6;
    pub const TRZ: usize = 7;
    pub const LLM0: usize = 8;
    pub const RBK: usize = 13;
    pub const GBK: usize = 14;
    pub const BBK: usize = 15;
    pub const LCM0: usize = 16;
    pub const RFC: usize = 21;
    pub const GFC: usize = 22;
    pub const BFC: usize = 23;
    pub const OFX: usize = 24;
    pub const OFY: usize = 25;
    pub const H: usize = 26;
    pub const DQA: usize = 27;
    pub const DQB: usize = 28;
    pub const ZSF3: usize = 29;
    pub const ZSF4: usize = 30;
    pub const FLAG: usize = 31;
}

/// Command words as the Psy-Q macros and libgte emit them.
pub mod cmd {
    pub const RTPS: u32 = 0x4A18_0001;
    pub const RTPT: u32 = 0x4A28_0030;
    pub const NCLIP: u32 = 0x4B40_0006;
    pub const NCS: u32 = 0x4AC8_041E;
    pub const NCT: u32 = 0x4AD8_0420;
    pub const NCCS: u32 = 0x4B08_041B;
    pub const NCCT: u32 = 0x4B18_043F;
    pub const AVSZ3: u32 = 0x4B58_002D;
    pub const AVSZ4: u32 = 0x4B68_002E;
    pub const fn mvmva(sf: u32, mx: u32, v: u32, cv: u32, lm: u32) -> u32 {
        0x4A40_0012 | (sf << 19) | (mx << 17) | (v << 15) | (cv << 13) | (lm << 10)
    }
    pub const fn gpf(sf: u32) -> u32 {
        0x4B90_003D | (sf << 19)
    }
}

// FLAG bits.
const fn f_mac_pos(i: usize) -> u32 {
    1 << (31 - i)
}
const fn f_mac_neg(i: usize) -> u32 {
    1 << (28 - i)
}
const fn f_ir(i: usize) -> u32 {
    1 << (25 - i)
}
const fn f_color(i: usize) -> u32 {
    1 << (22 - i)
}
const F_SZ: u32 = 0x0004_0000;
const F_DIV: u32 = 0x0002_0000;
const F_MAC0_POS: u32 = 0x0001_0000;
const F_MAC0_NEG: u32 = 0x0000_8000;
const F_SX2: u32 = 0x0000_4000;
const F_SY2: u32 = 0x0000_2000;
const F_IR0: u32 = 0x0000_1000;
/// Bits 30..23 and 18..13: any of them set also sets bit 31.
const F_ERROR_BITS: u32 = 0x7F87_E000;

/// What `execute` did with a command word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecStatus {
    /// Computed with the hardware rules.
    Done,
    /// MVMVA with matrix select 3 (garbage matrix) or far-colour translation (hardware bug):
    /// computed the way the C model does (matrix 3 as RT, cv 2 as a plain FC translation), which
    /// is not what the hardware does.
    Approximated,
    /// Command not modelled: only FLAG was cleared. Carries the command word.
    Unimplemented(u32),
}

/// The GTE register file and command unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gte {
    d: [i32; 32],
    c: [i32; 32],
    flag: u32,
}

impl Default for Gte {
    fn default() -> Self {
        Self::new()
    }
}

fn lo16(v: i32) -> i16 {
    v as u16 as i16
}
fn hi16(v: i32) -> i16 {
    ((v as u32) >> 16) as u16 as i16
}
fn sx44(v: i64) -> i64 {
    (((v as u64) << 20) as i64) >> 20
}

/// UNR reciprocal table (257 entries), generated as DW2-PC does.
fn unr_table() -> &'static [u8; 0x101] {
    use std::sync::OnceLock;
    static T: OnceLock<[u8; 0x101]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u8; 0x101];
        for (i, e) in t.iter_mut().enumerate() {
            let v = (0x40000 / (i as i32 + 0x100) + 1) / 2 - 0x101;
            *e = if v < 0 { 0 } else { v as u8 };
        }
        t
    })
}

fn lzc(v: i32) -> i32 {
    let mut u = if v < 0 { !(v as u32) } else { v as u32 };
    let mut n = 0;
    while n < 32 && u & 0x8000_0000 == 0 {
        u <<= 1;
        n += 1;
    }
    n
}

fn sat5(ir: i32) -> u32 {
    (ir >> 7).clamp(0, 0x1F) as u32
}

impl Gte {
    /// All registers zero except LZCR = 32 (the count for LZCS = 0).
    pub fn new() -> Self {
        let mut d = [0; 32];
        d[dreg::LZCR] = 32;
        Gte {
            d,
            c: [0; 32],
            flag: 0,
        }
    }

    // ---- register access ----

    /// MTC2 / write to a data register.
    pub fn mtc2(&mut self, reg: usize, value: u32) {
        let d = &mut self.d;
        match reg & 31 {
            r @ (dreg::VZ0
            | dreg::VZ1
            | dreg::VZ2
            | dreg::IR0
            | dreg::IR1
            | dreg::IR2
            | dreg::IR3) => d[r] = i32::from(lo16(value as i32)),
            r @ (dreg::OTZ | dreg::SZ0 | dreg::SZ1 | dreg::SZ2 | dreg::SZ3) => {
                d[r] = (value & 0xFFFF) as i32;
            }
            dreg::SXYP => {
                d[dreg::SXY0] = d[dreg::SXY1];
                d[dreg::SXY1] = d[dreg::SXY2];
                d[dreg::SXY2] = value as i32;
            }
            dreg::IRGB => {
                d[dreg::IR1] = ((value & 0x1F) << 7) as i32;
                d[dreg::IR2] = (((value >> 5) & 0x1F) << 7) as i32;
                d[dreg::IR3] = (((value >> 10) & 0x1F) << 7) as i32;
            }
            dreg::ORGB | dreg::LZCR => {} // read only
            dreg::LZCS => {
                d[dreg::LZCS] = value as i32;
                d[dreg::LZCR] = lzc(value as i32);
            }
            r => d[r] = value as i32,
        }
    }

    /// MFC2 / read of a data register.
    pub fn mfc2(&self, reg: usize) -> u32 {
        match reg & 31 {
            dreg::SXYP => self.d[dreg::SXY2] as u32,
            dreg::IRGB | dreg::ORGB => {
                sat5(self.d[dreg::IR1])
                    | sat5(self.d[dreg::IR2]) << 5
                    | sat5(self.d[dreg::IR3]) << 10
            }
            r => self.d[r] as u32,
        }
    }

    /// CTC2 / write to a control register.
    pub fn ctc2(&mut self, reg: usize, value: u32) {
        let reg = reg & 31;
        match reg {
            // RT33, L33, LB3, H, DQA, ZSF3, ZSF4: 16 bits, sign extended (H reads back so too).
            4 | 12 | 20 | creg::H | creg::DQA | creg::ZSF3 | creg::ZSF4 => {
                self.c[reg] = i32::from(lo16(value as i32));
            }
            creg::FLAG => {
                let mut v = value & 0x7FFF_F000;
                if v & F_ERROR_BITS != 0 {
                    v |= 0x8000_0000;
                }
                self.c[creg::FLAG] = v as i32;
            }
            _ => self.c[reg] = value as i32,
        }
    }

    /// CFC2 / read of a control register.
    pub fn cfc2(&self, reg: usize) -> u32 {
        self.c[reg & 31] as u32
    }

    // ---- helpers over the register file ----

    fn vx(&self, n: usize) -> i16 {
        lo16(self.d[dreg::VXY0 + n * 2])
    }
    fn vy(&self, n: usize) -> i16 {
        hi16(self.d[dreg::VXY0 + n * 2])
    }
    fn vz(&self, n: usize) -> i16 {
        lo16(self.d[dreg::VZ0 + n * 2])
    }

    /// Matrix element (row r, column c) of the control matrix at base 0 (RT), 8 (LLM) or 16
    /// (LCM): five words of packed s16 pairs, the 9th element alone in the low half of word 4.
    fn mat(&self, base: usize, r: usize, c: usize) -> i16 {
        let k = r * 3 + c;
        let w = self.c[base + k / 2];
        if k & 1 != 0 { hi16(w) } else { lo16(w) }
    }

    /// MAC1..3 accumulation step: flags a 44-bit overflow, then wraps to 44 bits like the
    /// hardware adder.
    fn mac_step(&mut self, i: usize, v: i64) -> i64 {
        if v > 0x7FF_FFFF_FFFF {
            self.flag |= f_mac_pos(i);
        } else if v < -0x800_0000_0000 {
            self.flag |= f_mac_neg(i);
        }
        sx44(v)
    }

    fn set_mac(&mut self, i: usize, v: i64, shift: u32) {
        let v = self.mac_step(i, v);
        self.d[dreg::MAC0 + i] = (v >> shift) as i32;
    }

    fn set_mac0(&mut self, v: i64) {
        if v > 0x7FFF_FFFF {
            self.flag |= F_MAC0_POS;
        } else if v < -0x8000_0000 {
            self.flag |= F_MAC0_NEG;
        }
        self.d[dreg::MAC0] = v as i32;
    }

    fn set_ir(&mut self, i: usize, v: i32, lm: bool) {
        let lo = if lm { 0 } else { -0x8000 };
        let mut v = v;
        if v < lo {
            v = lo;
            self.flag |= f_ir(i);
        } else if v > 0x7FFF {
            v = 0x7FFF;
            self.flag |= f_ir(i);
        }
        self.d[dreg::IR0 + i] = v;
    }

    fn set_mac_ir(&mut self, i: usize, v: i64, shift: u32, lm: bool) {
        self.set_mac(i, v, shift);
        self.set_ir(i, self.d[dreg::MAC0 + i], lm);
    }

    fn push_sz(&mut self, v: i64) {
        let mut v = v;
        if v < 0 {
            v = 0;
            self.flag |= F_SZ;
        } else if v > 0xFFFF {
            v = 0xFFFF;
            self.flag |= F_SZ;
        }
        self.d[dreg::SZ0] = self.d[dreg::SZ1];
        self.d[dreg::SZ1] = self.d[dreg::SZ2];
        self.d[dreg::SZ2] = self.d[dreg::SZ3];
        self.d[dreg::SZ3] = v as i32;
    }

    fn sat_sxy(&mut self, v: i32, f: u32) -> i32 {
        if v < -0x400 {
            self.flag |= f;
            -0x400
        } else if v > 0x3FF {
            self.flag |= f;
            0x3FF
        } else {
            v
        }
    }

    fn push_sxy(&mut self, x: i32, y: i32) {
        self.d[dreg::SXY0] = self.d[dreg::SXY1];
        self.d[dreg::SXY1] = self.d[dreg::SXY2];
        self.d[dreg::SXY2] = (((y as u32) << 16) | (x as u32 & 0xFFFF)) as i32;
    }

    fn sat_color(&mut self, i: usize, v: i32) -> u32 {
        if v < 0 {
            self.flag |= f_color(i);
            0
        } else if v > 0xFF {
            self.flag |= f_color(i);
            0xFF
        } else {
            v as u32
        }
    }

    /// Colour FIFO push of MAC1..3 / 16 with RGBC's code byte.
    fn push_rgb_from_mac(&mut self) {
        let r = self.sat_color(1, self.d[dreg::MAC1] >> 4);
        let g = self.sat_color(2, self.d[dreg::MAC2] >> 4);
        let b = self.sat_color(3, self.d[dreg::MAC3] >> 4);
        self.d[dreg::RGB0] = self.d[dreg::RGB1];
        self.d[dreg::RGB1] = self.d[dreg::RGB2];
        self.d[dreg::RGB2] =
            (r | g << 8 | b << 16 | (self.d[dreg::RGBC] as u32 & 0xFF00_0000)) as i32;
    }

    /// H / SZ3 in 16.16 as the GTE's Newton-Raphson divider computes it, or 0x1FFFF and the
    /// divide overflow flag when H >= SZ3 * 2.
    fn unr_divide(&mut self, h: u32, sz3: u32) -> u32 {
        if h >= sz3.wrapping_mul(2) {
            self.flag |= F_DIV;
            return 0x1FFFF;
        }
        let mut z = 0u32;
        while (sz3.wrapping_shl(z)) & 0x8000 == 0 {
            z += 1;
        }
        let n = h.wrapping_shl(z);
        let mut d = sz3.wrapping_shl(z);
        let u = u32::from(unr_table()[(d.wrapping_sub(0x7FC0) >> 7) as usize]) + 0x101;
        d = 0x200_0080u32.wrapping_sub(d.wrapping_mul(u)) >> 8;
        d = 0x80u32.wrapping_add(d.wrapping_mul(u)) >> 8;
        let q = ((u64::from(n) * u64::from(d)) + 0x8000) >> 16;
        if q > 0x1FFFF { 0x1FFFF } else { q as u32 }
    }

    // ---- commands ----

    /// RTPS body for vertex `v` (0..2): one perspective transform. `depth_cue` runs the final
    /// MAC0/IR0 depth-cue step; RTPT does it for V2 only, so V0/V1 can't leave its flags.
    fn rtps_vertex(&mut self, v: usize, sf: bool, lm: bool, depth_cue: bool) {
        let shift = if sf { 12 } else { 0 };
        let (vx, vy, vz) = (self.vx(v), self.vy(v), self.vz(v));
        let mut m = [0i64; 3];
        for (i, mi) in m.iter_mut().enumerate() {
            let mut acc = i64::from(self.c[creg::TRX + i]) << 12;
            acc = self.mac_step(i + 1, acc + i64::from(self.mat(0, i, 0)) * i64::from(vx));
            acc = self.mac_step(i + 1, acc + i64::from(self.mat(0, i, 1)) * i64::from(vy));
            acc = self.mac_step(i + 1, acc + i64::from(self.mat(0, i, 2)) * i64::from(vz));
            *mi = acc;
            self.set_mac(i + 1, acc, shift);
        }
        self.set_ir(1, self.d[dreg::MAC1], lm);
        self.set_ir(2, self.d[dreg::MAC2], lm);
        // IR3: saturated from MAC3, but its flag comes from MAC3's value shifted by 12 whatever
        // sf says (hardware quirk).
        {
            let z12 = m[2] >> 12;
            let mut val = self.d[dreg::MAC3];
            let lo = if lm { 0 } else { -0x8000 };
            if !(-0x8000..=0x7FFF).contains(&z12) {
                self.flag |= f_ir(3);
            }
            if val < lo {
                val = lo;
            } else if val > 0x7FFF {
                val = 0x7FFF;
            }
            self.d[dreg::IR3] = val;
        }
        self.push_sz(m[2] >> 12);
        let q = self.unr_divide(self.c[creg::H] as u16 as u32, self.d[dreg::SZ3] as u32);
        let q = i64::from(q);
        let mut s = q * i64::from(self.d[dreg::IR1] as i16) + i64::from(self.c[creg::OFX]);
        self.set_mac0(s);
        let x = self.sat_sxy((s >> 16) as i32, F_SX2);
        s = q * i64::from(self.d[dreg::IR2] as i16) + i64::from(self.c[creg::OFY]);
        self.set_mac0(s);
        let y = self.sat_sxy((s >> 16) as i32, F_SY2);
        self.push_sxy(x, y);
        if !depth_cue {
            return;
        }
        s = q * i64::from(self.c[creg::DQA] as i16) + i64::from(self.c[creg::DQB]);
        self.set_mac0(s);
        let mut ir0 = (s >> 12) as i32;
        if ir0 < 0 {
            ir0 = 0;
            self.flag |= F_IR0;
        } else if ir0 > 0x1000 {
            ir0 = 0x1000;
            self.flag |= F_IR0;
        }
        self.d[dreg::IR0] = ir0;
    }

    fn op_nclip(&mut self) {
        let p = |r: usize| (i64::from(lo16(self.d[r])), i64::from(hi16(self.d[r])));
        let (x0, y0) = p(dreg::SXY0);
        let (x1, y1) = p(dreg::SXY1);
        let (x2, y2) = p(dreg::SXY2);
        self.set_mac0(x0 * y1 + x1 * y2 + x2 * y0 - x0 * y2 - x1 * y0 - x2 * y1);
    }

    /// MAC/IR = (T << 12 + M * V) >> shift, one adder step per term. `t` None: no translation.
    fn mul_mat_vec(&mut self, base: usize, t: Option<[i32; 3]>, v: [i16; 3], shift: u32, lm: bool) {
        for i in 0..3 {
            let mut acc = t.map_or(0, |t| i64::from(t[i]) << 12);
            for (j, &vj) in v.iter().enumerate() {
                acc = self.mac_step(i + 1, acc + i64::from(self.mat(base, i, j)) * i64::from(vj));
            }
            self.set_mac_ir(i + 1, acc, shift, lm);
        }
    }

    fn op_mvmva(&mut self, op: u32, sf: bool, lm: bool) -> ExecStatus {
        let shift = if sf { 12 } else { 0 };
        let mx = ((op >> 17) & 3) as usize;
        let vsel = ((op >> 15) & 3) as usize;
        let cv = ((op >> 13) & 3) as usize;
        let v = if vsel == 3 {
            [
                self.d[dreg::IR1] as i16,
                self.d[dreg::IR2] as i16,
                self.d[dreg::IR3] as i16,
            ]
        } else {
            [self.vx(vsel), self.vy(vsel), self.vz(vsel)]
        };
        let t = if cv == 3 {
            None
        } else {
            let tb = [creg::TRX, creg::RBK, creg::RFC][cv];
            Some([self.c[tb], self.c[tb + 1], self.c[tb + 2]])
        };
        let base = [creg::RT0, creg::LLM0, creg::LCM0][if mx == 3 { 0 } else { mx }];
        self.mul_mat_vec(base, t, v, shift, lm);
        if mx == 3 || cv == 2 {
            ExecStatus::Approximated
        } else {
            ExecStatus::Done
        }
    }

    /// NCS / NCCS body for normal `v` (0..2): `LLM * N`, then `BK << 12 + LCM * IR`, optionally
    /// scaled by RGBC (`color`: NCCx), then a colour FIFO push (psx-spx "GTE Color Calculation
    /// Commands").
    fn light_vertex(&mut self, v: usize, sf: bool, lm: bool, color: bool) {
        let shift = if sf { 12 } else { 0 };
        let bk = [self.c[creg::RBK], self.c[creg::GBK], self.c[creg::BBK]];
        let n = [self.vx(v), self.vy(v), self.vz(v)];
        self.mul_mat_vec(creg::LLM0, None, n, shift, lm);
        let ir = [
            self.d[dreg::IR1] as i16,
            self.d[dreg::IR2] as i16,
            self.d[dreg::IR3] as i16,
        ];
        self.mul_mat_vec(creg::LCM0, Some(bk), ir, shift, lm);
        if color {
            let rgbc = self.d[dreg::RGBC] as u32;
            for i in 0..3 {
                let c = i64::from((rgbc >> (8 * i)) & 0xFF);
                let m = (c * i64::from(self.d[dreg::IR1 + i])) << 4;
                self.set_mac_ir(i + 1, m, shift, lm);
            }
        }
        self.push_rgb_from_mac();
    }

    /// AVSZ3 / AVSZ4: OTZ = ZSF * (sum of the newest 3 or 4 SZ) / 4096, saturated to 16 bits.
    fn op_avsz(&mut self, quad: bool) {
        let sz = |g: &Self, i: usize| i64::from(g.d[dreg::SZ0 + i]);
        let (zsf, sum) = if quad {
            (
                self.c[creg::ZSF4],
                sz(self, 0) + sz(self, 1) + sz(self, 2) + sz(self, 3),
            )
        } else {
            (self.c[creg::ZSF3], sz(self, 1) + sz(self, 2) + sz(self, 3))
        };
        let mac0 = i64::from(zsf) * sum;
        self.set_mac0(mac0);
        let mut otz = mac0 >> 12;
        if otz < 0 {
            otz = 0;
            self.flag |= F_SZ;
        } else if otz > 0xFFFF {
            otz = 0xFFFF;
            self.flag |= F_SZ;
        }
        self.d[dreg::OTZ] = otz as i32;
    }

    fn op_gpf(&mut self, sf: bool, lm: bool) {
        let shift = if sf { 12 } else { 0 };
        let ir0 = i64::from(self.d[dreg::IR0] as i16);
        for i in 1..=3 {
            let ir = i64::from(self.d[dreg::IR0 + i] as i16);
            self.set_mac_ir(i, ir0 * ir, shift, lm);
        }
        self.push_rgb_from_mac();
    }

    /// Run the command encoded in the COP2 instruction word `op`. FLAG is cleared first and
    /// recomputed (bit 31 included), as on hardware.
    pub fn execute(&mut self, op: u32) -> ExecStatus {
        let sf = (op >> 19) & 1 != 0;
        let lm = (op >> 10) & 1 != 0;
        self.flag = 0;
        let status = match op & 0x3F {
            0x01 => {
                self.rtps_vertex(0, sf, lm, true);
                ExecStatus::Done
            }
            0x30 => {
                for v in 0..3 {
                    self.rtps_vertex(v, sf, lm, v == 2);
                }
                ExecStatus::Done
            }
            0x06 => {
                self.op_nclip();
                ExecStatus::Done
            }
            0x12 => self.op_mvmva(op, sf, lm),
            0x1E => {
                self.light_vertex(0, sf, lm, false);
                ExecStatus::Done
            }
            0x1B => {
                self.light_vertex(0, sf, lm, true);
                ExecStatus::Done
            }
            0x20 | 0x3F => {
                for v in 0..3 {
                    self.light_vertex(v, sf, lm, op & 0x3F == 0x3F);
                }
                ExecStatus::Done
            }
            0x2D | 0x2E => {
                self.op_avsz(op & 0x3F == 0x2E);
                ExecStatus::Done
            }
            0x3D => {
                self.op_gpf(sf, lm);
                ExecStatus::Done
            }
            _ => ExecStatus::Unimplemented(op),
        };
        if self.flag & F_ERROR_BITS != 0 {
            self.flag |= 0x8000_0000;
        }
        self.c[creg::FLAG] = self.flag as i32;
        status
    }
}
