//! PS1 movie decoding: the STR frame bitstream to MDEC run-length codes (what Psy-Q's
//! `DecDCTvlc2` does on the CPU) and the MDEC itself (run-length codes, dequantise, IDCT, YCbCr
//! to RGB, 16x16 macroblocks).
//!
//! Translated from DW2-PC (github.com/Wyrelade/DW2-PC, CC0), `backend/mdec.c` and
//! `backend/mdec.h`, at commit 2d491b5. The decoder logic is DW2-PC's; the literal tables are
//! not, they are re-derived from public sources:
//!
//! - AC variable-length codes: the MPEG-1 `dct_coeff_next` table (ISO/IEC 11172-2, table B.5),
//!   in the layout psx-spx gives for BS v1/v2/v3 ("Huffman codes for AC values", on
//!   psx-spx.consoledev.net/cdromfileformats/): prefix, index bits, then the sign bit, with the
//!   values as MDEC halfwords.
//! - Zigzag scan order: generated from the standard's definition (the diagonals of the 8x8
//!   block, alternating direction), checked against psx-spx's `zigzag[]` in the tests.
//! - IDCT scale table: generated from the formula on psx-spx's MDEC page
//!   (psx-spx.consoledev.net/macroblockdecodermdec/, "set_scale_table"): `s_k = scalefactor[k] *
//!   sqrt(2) * 4000h`, with the sign pattern shown there; checked against the page's table.
//! - Default quantisation matrix: the MPEG-1 default intra matrix (ISO/IEC 11172-2, 2.4.4.1)
//!   with 2 for the DC entry, which is what Psy-Q's `DecDCTReset` loads; psx-spx's BS "fraquant"
//!   section quotes it as `02h,10h,10h,13h,..,53h` in zigzag order. See [`DEFAULT_QUANT`].
//!
//! Decoding is instant (no MDEC timing). Differences from the C: the state lives in [`Mdec`]
//! instead of file-scope statics; [`vlc`] reads zeros past the end of the bitstream instead of
//! out of bounds; [`Mdec::start`] clips the stream to the words actually given.

use std::sync::OnceLock;

/// One group of AC codes that share a prefix and differ in `idx_bits` index bits; each code is
/// followed by a sign bit (1 = negative level). `vals` are MDEC halfwords (run in bits 15-10,
/// level in bits 9-0, sign bit clear).
struct AcGroup {
    prefix: u16,
    prefix_len: u8,
    idx_bits: u8,
    vals: &'static [u16],
}

/// AC codes of BS v1/v2/v3 (MPEG-1 `dct_coeff_next` without the first-coefficient `1s` form).
/// Not listed: `10` (end of block, MDEC `FE00h`) and `000001` (escape, 6-bit run and 10-bit
/// level follow raw); see [`ac_lut`]. Source: psx-spx "Huffman codes for AC values BS
/// v1/v2/v3/iki", cross-checked with ISO/IEC 11172-2 table B.5.
const AC_GROUPS: [AcGroup; 14] = [
    AcGroup {
        prefix: 0b11,
        prefix_len: 2,
        idx_bits: 0,
        vals: &[0x0001],
    },
    AcGroup {
        prefix: 0b011,
        prefix_len: 3,
        idx_bits: 0,
        vals: &[0x0401],
    },
    AcGroup {
        prefix: 0b010,
        prefix_len: 3,
        idx_bits: 1,
        vals: &[0x0002, 0x0801],
    },
    AcGroup {
        prefix: 0b0011,
        prefix_len: 4,
        idx_bits: 1,
        vals: &[0x1001, 0x0C01],
    },
    AcGroup {
        prefix: 0b00101,
        prefix_len: 5,
        idx_bits: 0,
        vals: &[0x0003],
    },
    AcGroup {
        prefix: 0b00100,
        prefix_len: 5,
        idx_bits: 3,
        vals: &[
            0x3401, 0x0006, 0x3001, 0x2C01, 0x0C02, 0x0403, 0x0005, 0x2801,
        ],
    },
    AcGroup {
        prefix: 0b0001,
        prefix_len: 4,
        idx_bits: 2,
        vals: &[0x1C01, 0x1801, 0x0402, 0x1401],
    },
    AcGroup {
        prefix: 0b00001,
        prefix_len: 5,
        idx_bits: 2,
        vals: &[0x0802, 0x2401, 0x0004, 0x2001],
    },
    AcGroup {
        prefix: 0b0000001,
        prefix_len: 7,
        idx_bits: 3,
        vals: &[
            0x4001, 0x1402, 0x0007, 0x0803, 0x0404, 0x3C01, 0x3801, 0x1002,
        ],
    },
    AcGroup {
        prefix: 0b00000001,
        prefix_len: 8,
        idx_bits: 4,
        vals: &[
            0x000B, 0x2002, 0x1003, 0x000A, 0x0804, 0x1C02, 0x5401, 0x5001, 0x0009, 0x4C01, 0x4801,
            0x0405, 0x0C03, 0x0008, 0x1802, 0x4401,
        ],
    },
    AcGroup {
        prefix: 0b000000001,
        prefix_len: 9,
        idx_bits: 4,
        vals: &[
            0x2802, 0x2402, 0x1403, 0x0C04, 0x0805, 0x0407, 0x0406, 0x000F, 0x000E, 0x000D, 0x000C,
            0x6801, 0x6401, 0x6001, 0x5C01, 0x5801,
        ],
    },
    AcGroup {
        prefix: 0b0000000001,
        prefix_len: 10,
        idx_bits: 4,
        vals: &[
            0x001F, 0x001E, 0x001D, 0x001C, 0x001B, 0x001A, 0x0019, 0x0018, 0x0017, 0x0016, 0x0015,
            0x0014, 0x0013, 0x0012, 0x0011, 0x0010,
        ],
    },
    AcGroup {
        prefix: 0b00000000001,
        prefix_len: 11,
        idx_bits: 4,
        vals: &[
            0x0028, 0x0027, 0x0026, 0x0025, 0x0024, 0x0023, 0x0022, 0x0021, 0x0020, 0x040E, 0x040D,
            0x040C, 0x040B, 0x040A, 0x0409, 0x0408,
        ],
    },
    AcGroup {
        prefix: 0b000000000001,
        prefix_len: 12,
        idx_bits: 4,
        vals: &[
            0x0412, 0x0411, 0x0410, 0x040F, 0x1803, 0x4002, 0x3C02, 0x3802, 0x3402, 0x3002, 0x2C02,
            0x7C01, 0x7801, 0x7401, 0x7001, 0x6C01,
        ],
    },
];

const AC_NONE: u8 = 0;
const AC_CODE: u8 = 1;
const AC_EOB: u8 = 2;
const AC_ESCAPE: u8 = 3;

/// One entry of the 16-bit lookup: kind, code length (without the sign bit), run, level.
#[derive(Clone, Copy)]
struct AcEntry {
    kind: u8,
    len: u8,
    run: u8,
    level: u8,
}

/// Lookup indexed by the next 16 bits of the stream.
fn ac_lut() -> &'static [AcEntry] {
    static LUT: OnceLock<Vec<AcEntry>> = OnceLock::new();
    LUT.get_or_init(|| {
        let none = AcEntry {
            kind: AC_NONE,
            len: 0,
            run: 0,
            level: 0,
        };
        let mut lut = vec![none; 1 << 16];
        let mut fill = |code: u32, len: u32, e: AcEntry| {
            let base = code << (16 - len);
            for i in 0..1u32 << (16 - len) {
                lut[(base | i) as usize] = e;
            }
        };
        for g in &AC_GROUPS {
            let len = u32::from(g.prefix_len) + u32::from(g.idx_bits);
            for (x, &v) in g.vals.iter().enumerate() {
                let code = u32::from(g.prefix) << g.idx_bits | x as u32;
                let e = AcEntry {
                    kind: AC_CODE,
                    len: len as u8,
                    run: (v >> 10) as u8,
                    level: (v & 0x3FF) as u8,
                };
                fill(code, len, e);
            }
        }
        fill(
            0b10,
            2,
            AcEntry {
                kind: AC_EOB,
                len: 2,
                run: 0,
                level: 0,
            },
        );
        fill(
            0b000001,
            6,
            AcEntry {
                kind: AC_ESCAPE,
                len: 6,
                run: 0,
                level: 0,
            },
        );
        lut
    })
}

/// One AC code of the table, without its sign bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcCode {
    /// The code bits, right aligned.
    pub bits: u16,
    /// Length in bits.
    pub len: u8,
    pub run: u8,
    pub level: u8,
}

/// All 111 AC codes (not the end-of-block and escape codes). For encoders and tests.
pub fn ac_codes() -> Vec<AcCode> {
    let mut v = Vec::new();
    for g in &AC_GROUPS {
        for (x, &val) in g.vals.iter().enumerate() {
            v.push(AcCode {
                bits: g.prefix << g.idx_bits | x as u16,
                len: g.prefix_len + g.idx_bits,
                run: (val >> 10) as u8,
                level: (val & 0x3FF) as u8,
            });
        }
    }
    v
}

/// Bits come from 16-bit little-endian words, most significant bit first.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    /// Next bits, left-aligned.
    buf: u32,
    /// Valid bits in `buf`.
    have: u32,
}

impl Bits<'_> {
    fn byte(&self, i: usize) -> u32 {
        u32::from(self.data.get(i).copied().unwrap_or(0))
    }

    fn fill(&mut self) {
        while self.have <= 16 {
            let w = self.byte(self.pos) | self.byte(self.pos + 1) << 8;
            self.buf |= w << (16 - self.have);
            self.pos += 2;
            self.have += 16;
        }
    }

    fn peek16(&mut self) -> usize {
        self.fill();
        (self.buf >> 16) as usize
    }

    fn get(&mut self, n: u32) -> u32 {
        self.fill();
        let v = self.buf >> (32 - n);
        self.buf <<= n;
        self.have -= n;
        v
    }
}

/// A version 3 DC code: the difference (times 4) from the last DC of the same kind. A size
/// category as in MPEG-1's `dct_dc_size_luminance` / `_chrominance` (unary-ish prefix), then
/// that many bits, the first of which is 1 for a positive difference. `None` for the unused
/// codes. [doc: psx-spx "CDROM File Video BS Compression DC Values", DC v3]
///
/// Luma: `100` 0, `00` 1, `01` 2, `101` 3, `110` 4, `1110` 5, ... `1111110` 8.
/// Chroma: `00` 0, `01` 1, `10` 2, `110` 3, `1110` 4, ... `11111110` 8.
fn dc_diff_v3(b: &mut Bits, luma: bool) -> Option<i32> {
    let size = match (b.get(2), luma) {
        (0b00, true) => 1,
        (0b01, true) => 2,
        (0b10, true) => 3 * b.get(1),
        (0b00, false) => 0,
        (0b01, false) => 1,
        (0b10, false) => 2,
        (_, luma) => {
            // "11" and then one more 1 for each category above the first coded this way.
            let mut size = if luma { 4 } else { 3 };
            while b.get(1) == 1 {
                size += 1;
            }
            size
        }
    };
    if size > 8 {
        return None;
    }
    if size == 0 {
        return Some(0);
    }
    let bits = b.get(size) as i32;
    let diff = if bits >= 1 << (size - 1) {
        bits
    } else {
        bits - ((1 << size) - 1)
    };
    Some(diff * 4)
}

/// Why [`vlc`] refused a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VlcError {
    /// Not an STR bitstream header, unknown version, quantiser above 63, or no room in `out`.
    BadHeader,
    /// An unused code, or a block that runs past the size in the header.
    BadStream,
}

/// Decodes one STR frame to MDEC run-length codes. `bs` is the frame: an 8-byte header
/// (run-length size in words, 0x3800, quantiser scale, version), then the bitstream. Writes
/// `out[0] = 0x3800_nnnn` (nnnn = words that follow) and the run-length halfwords (two per word,
/// low half first); returns the words written. Versions 1, 2 and 3 (3 codes the DC values as
/// differences, [`dc_diff_v3`]). On error `out[0]` holds an empty
/// stream (`0x3800_0000`) and the rest of `out` is untouched.
pub fn vlc(bs: &[u8], out: &mut [u32]) -> Result<usize, VlcError> {
    let hb = |i: usize| u32::from(bs.get(i).copied().unwrap_or(0));
    let words = hb(0) | hb(1) << 8;
    let magic = hb(2) | hb(3) << 8;
    let q = hb(4) | hb(5) << 8;
    let version = hb(6) | hb(7) << 8;
    let want = (words * 2) as usize;

    if let Some(w) = out.first_mut() {
        *w = 0x3800_0000;
    }
    if magic != 0x3800 || !(1..=3).contains(&version) || q > 63 || words as usize + 1 > out.len() {
        return Err(VlcError::BadHeader);
    }
    let lut = ac_lut();
    let mut o = vec![0u16; want];
    let mut n = 0usize;
    let mut b = Bits {
        data: bs.get(8..).unwrap_or(&[]),
        pos: 0,
        buf: 0,
        have: 0,
    };

    let mut block = 0usize; // 0 = Cr, 1 = Cb, 2..=5 = Y1..Y4, as the MDEC wants them
    let mut dc_pred = [0i32; 3]; // version 3: the last Cr, Cb and Y DC values
    while n < want {
        let dc = if version == 3 {
            if b.peek16() >> 6 == 0x3FF {
                break; // end of frame
            }
            let class = block.min(2);
            let Some(diff) = dc_diff_v3(&mut b, class == 2) else {
                return Err(VlcError::BadStream);
            };
            dc_pred[class] += diff;
            // Newer decoders wrap the running value within the 10-bit field [doc: psx-spx].
            (dc_pred[class] & 0x3FF) as u32
        } else {
            let dc = b.get(10);
            if dc == 0x1FF {
                break; // end of frame
            }
            dc
        };
        block = (block + 1) % 6;
        o[n] = (q << 10 | dc) as u16;
        n += 1;
        loop {
            let e = lut[b.peek16()];
            if n >= want {
                return Err(VlcError::BadStream); // the block runs past the size in the header
            }
            let (run, level);
            match e.kind {
                AC_EOB => {
                    b.get(2);
                    o[n] = 0xFE00;
                    n += 1;
                    break;
                }
                AC_ESCAPE => {
                    b.get(6);
                    run = b.get(6);
                    level = b.get(10);
                }
                AC_CODE => {
                    b.get(u32::from(e.len));
                    run = u32::from(e.run);
                    level = if b.get(1) != 0 {
                        0u32.wrapping_sub(u32::from(e.level))
                    } else {
                        u32::from(e.level)
                    };
                }
                _ => return Err(VlcError::BadStream),
            }
            o[n] = (run << 10 | (level & 0x3FF)) as u16;
            n += 1;
        }
    }
    while n < want {
        o[n] = 0xFE00; // the size is whole 32-word units
        n += 1;
    }
    for (i, pair) in o.as_chunks::<2>().0.iter().enumerate() {
        out[1 + i] = u32::from(pair[0]) | u32::from(pair[1]) << 16;
    }
    out[0] = 0x3800_0000 | words;
    Ok(words as usize + 1)
}

/// The block positions in zigzag scan order: entry `k` is the raster index (row * 8 + column) of
/// the k-th coefficient. Generated from the standard's definition: walk the anti-diagonals of
/// the 8x8 block, odd diagonals towards increasing rows, even ones towards decreasing rows
/// (ISO/IEC 11172-2, figure 2-8).
pub const ZIGZAG: [u8; 64] = zigzag_scan();

const fn zigzag_scan() -> [u8; 64] {
    let mut t = [0u8; 64];
    let mut k = 0;
    let mut d = 0usize;
    while d < 15 {
        // Rows on this diagonal run from r_lo to r_hi.
        let r_lo = d.saturating_sub(7);
        let r_hi = if d < 7 { d } else { 7 };
        let mut i = 0;
        while i <= r_hi - r_lo {
            let r = if d % 2 == 1 { r_lo + i } else { r_hi - i };
            t[k] = (r * 8 + (d - r)) as u8;
            k += 1;
            i += 1;
        }
        d += 1;
    }
    t
}

/// The default quantisation matrix (`DecDCTReset`), row major, luma and chroma alike: the MPEG-1
/// default intra matrix (ISO/IEC 11172-2, 2.4.4.1) with 2 in place of the DC entry 8. psx-spx
/// ("BS fraquant", cdromfileformats) gives its start and end in zigzag order: `02h,10h,10h,13h,
/// ..,53h`.
pub const DEFAULT_QUANT: [u8; 64] = [
    2, 16, 19, 22, 26, 27, 29, 34, //
    16, 16, 22, 24, 27, 29, 34, 37, //
    19, 22, 26, 27, 29, 34, 34, 38, //
    22, 22, 26, 27, 29, 34, 37, 40, //
    22, 26, 27, 29, 32, 35, 40, 48, //
    26, 27, 29, 32, 35, 40, 48, 58, //
    26, 27, 29, 34, 38, 46, 56, 69, //
    27, 29, 35, 38, 46, 56, 69, 83,
];

/// Sign pattern of the IDCT matrix: row u, column x holds `+-(k + 1)` for `+-s_k`
/// (psx-spx MDEC page, "set_scale_table").
const SCALE_PATTERN: [[i8; 8]; 8] = [
    [1, 1, 1, 1, 1, 1, 1, 1],
    [2, 4, 6, 8, -8, -6, -4, -2],
    [3, 7, -7, -3, -3, -7, 7, 3],
    [4, -8, -2, -6, 6, 2, 8, -4],
    [5, -5, -5, 5, 5, -5, -5, 5],
    [6, -2, 8, 4, -4, -8, 2, -6],
    [7, -3, 3, -7, -7, 3, -3, 7],
    [8, -6, 4, -2, 2, -4, 6, -8],
];

/// `s_k` as a real number times 4000h: `scalefactor[k] * sqrt(2) * 4000h`, with
/// `scalefactor[0] = 1` and `scalefactor[k] = cos(k * 90 degrees / 8) * sqrt(2)` for k = 1..7
/// (psx-spx MDEC page). Unrounded.
fn scale_exact(k: usize) -> f64 {
    let sqrt2 = std::f64::consts::SQRT_2;
    let sf = if k == 0 {
        1.0
    } else {
        (k as f64 * std::f64::consts::PI / 16.0).cos() * sqrt2
    };
    sf * sqrt2 * 16384.0
}

/// The IDCT matrix (row u, column x) with a 14-bit fraction, the values `MDEC(3)` is given.
/// Entries are rounded towards minus infinity, which reproduces the table on psx-spx's page
/// (`5A82` for +s0 and `A57D` = -5A83 for -s0).
pub fn scale_table() -> [i16; 64] {
    let mut t = [0i16; 64];
    for (u, row) in SCALE_PATTERN.iter().enumerate() {
        for (x, &p) in row.iter().enumerate() {
            let k = (p.unsigned_abs() - 1) as usize;
            let v = if p < 0 {
                -scale_exact(k)
            } else {
                scale_exact(k)
            };
            t[u * 8 + x] = v.floor() as i16;
        }
    }
    t
}

fn signed10(v: u32) -> i32 {
    (v & 0x3FF) as i32 - ((v & 0x200) << 1) as i32
}

/// One 1-D pass, which also transposes: `out[x + y * 8] = sum over u of in[y + u * 8] *
/// scale[u][x]`, rounded and scaled down by 16 bits.
fn idct_pass(scale: &[i16; 64], src: &[i32; 64]) -> [i32; 64] {
    let mut res = [0i32; 64];
    for x in 0..8 {
        for y in 0..8 {
            let mut sum = 0i32;
            for u in 0..8 {
                sum = sum.wrapping_add(src[y + u * 8].wrapping_mul(i32::from(scale[x + u * 8])));
            }
            res[x + y * 8] = sum.wrapping_add(0x8000) >> 16;
        }
    }
    res
}

/// Two passes (each transposes, so the result is the 2-D transform in the original orientation).
fn idct(scale: &[i16; 64], blk: &mut [i32; 64]) {
    *blk = idct_pass(scale, &idct_pass(scale, blk));
}

/// The MDEC: decodes run-length streams to pixels.
pub struct Mdec {
    scale: [i16; 64],
    data: Vec<u16>,
    pos: usize,
    rgb24: bool,
    stp: u16,
}

impl Default for Mdec {
    fn default() -> Self {
        Self::new()
    }
}

impl Mdec {
    pub fn new() -> Mdec {
        Mdec {
            scale: scale_table(),
            data: Vec::new(),
            pos: 0,
            rgb24: false,
            stp: 0,
        }
    }

    /// `DecDCTin`: the next output reads macroblocks from this run-length stream (out of
    /// [`vlc`]: the header word, then the halfwords). `rgb24`: 24-bit output, else 15-bit with
    /// bit 15 = `stp`.
    pub fn start(&mut self, input: &[u32], rgb24: bool, stp: bool) {
        self.data.clear();
        self.pos = 0;
        if let Some((&hdr, rest)) = input.split_first() {
            let words = ((hdr & 0xFFFF) as usize).min(rest.len());
            for &w in &rest[..words] {
                self.data.push(w as u16);
                self.data.push((w >> 16) as u16);
            }
        }
        self.rgb24 = rgb24;
        self.stp = u16::from(stp);
    }

    /// One block from the stream (false if the stream has ended).
    fn read_block(&mut self, blk: &mut [i32; 64]) -> bool {
        *blk = [0; 64];
        let mut n;
        loop {
            if self.pos >= self.data.len() {
                return false;
            }
            n = u32::from(self.data[self.pos]);
            self.pos += 1;
            if n != 0xFE00 {
                break;
            }
        }
        let qs = (n >> 10) as i32;
        let mut k = 0i32;
        blk[0] = if qs == 0 {
            (signed10(n) * 2).clamp(-0x400, 0x3FF)
        } else {
            (signed10(n) * i32::from(DEFAULT_QUANT[0])).clamp(-0x400, 0x3FF)
        };
        while self.pos < self.data.len() {
            n = u32::from(self.data[self.pos]);
            self.pos += 1;
            if n == 0xFE00 {
                break;
            }
            k += (n >> 10) as i32 + 1;
            if k > 63 {
                break;
            }
            let k = k as usize;
            if qs == 0 {
                blk[k] = (signed10(n) * 2).clamp(-0x400, 0x3FF);
            } else {
                let z = usize::from(ZIGZAG[k]);
                blk[z] = ((signed10(n) * i32::from(DEFAULT_QUANT[z]) * qs + 4) >> 3)
                    .clamp(-0x400, 0x3FF);
            }
        }
        idct(&self.scale, blk);
        true
    }

    /// One 16x16 macroblock (Cr, Cb, Y top left, top right, bottom left, bottom right) into `out`
    /// (192 words for 24-bit, 128 for 15-bit).
    fn macroblock(&mut self, out: &mut [u32]) {
        let mut cr = [0i32; 64];
        let mut cb = [0i32; 64];
        let mut y = [[0i32; 64]; 4];
        let mut ok = self.read_block(&mut cr);
        ok &= self.read_block(&mut cb);
        for b in &mut y {
            ok &= self.read_block(b);
        }
        let mut bytes = [0u8; 768];
        for py in 0..16 {
            for px in 0..16 {
                let c = (px >> 1) + (py >> 1) * 8;
                let lum = y[(py >> 3) * 2 + (px >> 3)][(px & 7) + (py & 7) * 8];
                let (mut r, mut g, mut b) = (0i32, 0i32, 0i32);
                if ok {
                    r = (lum + ((cr[c] * 1436) >> 10)).clamp(-128, 127) + 128;
                    g = (lum - ((cb[c] * 352 + cr[c] * 731) >> 10)).clamp(-128, 127) + 128;
                    b = (lum + ((cb[c] * 1815) >> 10)).clamp(-128, 127) + 128;
                }
                if self.rgb24 {
                    let o = (py * 16 + px) * 3;
                    bytes[o] = r as u8;
                    bytes[o + 1] = g as u8;
                    bytes[o + 2] = b as u8;
                } else {
                    let v = (r >> 3) as u16
                        | ((g >> 3) as u16) << 5
                        | ((b >> 3) as u16) << 10
                        | self.stp << 15;
                    let o = (py * 16 + px) * 2;
                    bytes[o..o + 2].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        for (w, c) in out.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *w = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        }
    }

    /// `DecDCTout`: pixels into `out`, whole macroblocks in stream order (24-bit: 192 words each,
    /// 15-bit: 128); a tail shorter than a macroblock is left alone. Each macroblock is 16 rows
    /// of 16 pixels. Past the end of the stream the rest is black.
    pub fn out(&mut self, out: &mut [u32]) {
        let mb = if self.rgb24 { 192 } else { 128 };
        for chunk in out.chunks_exact_mut(mb) {
            self.macroblock(chunk);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// psx-spx MDEC page, `zigzag[0..63]` (indexed by raster position, gives the scan index).
    const PSX_SPX_ZIGZAG: [u8; 64] = [
        0, 1, 5, 6, 14, 15, 27, 28, 2, 4, 7, 13, 16, 26, 29, 42, 3, 8, 12, 17, 25, 30, 41, 43, 9,
        11, 18, 24, 31, 40, 44, 53, 10, 19, 23, 32, 39, 45, 52, 54, 20, 22, 33, 38, 46, 51, 55, 60,
        21, 34, 37, 47, 50, 56, 59, 61, 35, 36, 48, 49, 57, 58, 62, 63,
    ];

    /// psx-spx MDEC page, the scale table "which should be usually/always" loaded by MDEC(3).
    const PSX_SPX_SCALE: [u16; 64] = [
        0x5A82, 0x5A82, 0x5A82, 0x5A82, 0x5A82, 0x5A82, 0x5A82, 0x5A82, //
        0x7D8A, 0x6A6D, 0x471C, 0x18F8, 0xE707, 0xB8E3, 0x9592, 0x8275, //
        0x7641, 0x30FB, 0xCF04, 0x89BE, 0x89BE, 0xCF04, 0x30FB, 0x7641, //
        0x6A6D, 0xE707, 0x8275, 0xB8E3, 0x471C, 0x7D8A, 0x18F8, 0x9592, //
        0x5A82, 0xA57D, 0xA57D, 0x5A82, 0x5A82, 0xA57D, 0xA57D, 0x5A82, //
        0x471C, 0x8275, 0x18F8, 0x6A6D, 0x9592, 0xE707, 0x7D8A, 0xB8E3, //
        0x30FB, 0x89BE, 0x7641, 0xCF04, 0xCF04, 0x7641, 0x89BE, 0x30FB, //
        0x18F8, 0xB8E3, 0x6A6D, 0x8275, 0x7D8A, 0x9592, 0x471C, 0xE707,
    ];

    #[test]
    fn zigzag_matches_psx_spx() {
        // psx-spx: zagzig[zigzag[i]] = i, so ZIGZAG (scan -> raster) is the inverse of its table.
        for (raster, &scan) in PSX_SPX_ZIGZAG.iter().enumerate() {
            assert_eq!(usize::from(ZIGZAG[usize::from(scan)]), raster);
        }
        assert_eq!(&ZIGZAG[..8], &[0, 1, 8, 16, 9, 2, 3, 10]);
        assert_eq!(ZIGZAG[63], 63);
    }

    #[test]
    fn scale_table_matches_psx_spx() {
        let t = scale_table();
        for i in 0..64 {
            assert_eq!(t[i] as u16, PSX_SPX_SCALE[i], "entry {i}");
        }
    }

    #[test]
    fn scale_rounding_is_not_marginal() {
        // The floor() must not depend on the last bits of the cos() implementation.
        for k in 0..8 {
            let v = scale_exact(k);
            let frac = v - v.floor();
            assert!((0.01..0.99).contains(&frac), "s{k} = {v}");
        }
    }

    #[test]
    fn default_quant_matches_published_values() {
        let zz: Vec<u8> = ZIGZAG
            .iter()
            .map(|&r| DEFAULT_QUANT[usize::from(r)])
            .collect();
        // psx-spx: 02h,10h,10h,13h,..,53h in zigzag order.
        assert_eq!(&zz[..4], &[0x02, 0x10, 0x10, 0x13]);
        assert_eq!(zz[63], 0x53);
        // The matrix is not symmetric; check the corners and the last row.
        assert_eq!(&DEFAULT_QUANT[56..], &[27, 29, 35, 38, 46, 56, 69, 83]);
        assert_eq!(DEFAULT_QUANT[7], 34);
    }

    #[test]
    fn ac_table_is_prefix_free_and_complete() {
        // Kraft sum over the codes plus EOB and escape: every 16-bit pattern is claimed by at
        // most one code (fill would otherwise overwrite), so count the claimed patterns.
        let mut claimed: u64 = 0;
        let mut codes = 0;
        for g in &AC_GROUPS {
            assert_eq!(g.vals.len(), 1 << g.idx_bits);
            for _ in g.vals {
                claimed += 1 << (16 - (u32::from(g.prefix_len) + u32::from(g.idx_bits)));
                codes += 1;
            }
        }
        claimed += (1 << 14) + (1 << 10); // "10" and "000001"
        assert_eq!(codes, 111);
        // Everything but the unused "000000000000" prefix (12 bits) is assigned.
        assert_eq!(claimed, 0x10000 - (1 << 4));
        let lut = ac_lut();
        assert_eq!(lut.iter().filter(|e| e.kind == AC_NONE).count(), 16);
        // A few codes from ISO/IEC 11172-2 table B.5: (code, run, level).
        let spot = |bits: &str, run: u8, level: u8| {
            let v = bits
                .bytes()
                .fold(0usize, |a, c| a << 1 | usize::from(c - b'0'));
            let e = lut[v << (16 - bits.len())];
            assert_eq!(
                (e.kind, usize::from(e.len), e.run, e.level),
                (AC_CODE, bits.len(), run, level),
                "{bits}"
            );
        };
        spot("11", 0, 1);
        spot("011", 1, 1);
        spot("0100", 0, 2);
        spot("0101", 2, 1);
        spot("00101", 0, 3);
        spot("0000110", 0, 4);
        spot("0000001010", 0, 7);
        spot("000000011101", 0, 8);
        spot("0000000011010", 0, 12);
        spot("00000000011111", 0, 16);
        spot("000000000011000", 0, 32);
        spot("0000000000010011", 1, 15);
        spot("0000000000011011", 31, 1);
    }

    #[test]
    fn vlc_rejects_bad_headers() {
        let mut out = [0xFFFF_FFFFu32; 4];
        assert_eq!(vlc(&[0; 4], &mut out), Err(VlcError::BadHeader));
        assert_eq!(out[0], 0x3800_0000);
        let mut bs = [0u8; 16];
        bs[2] = 0x00;
        bs[3] = 0x38;
        bs[6] = 4; // version
        assert_eq!(vlc(&bs, &mut out), Err(VlcError::BadHeader));
    }

    #[test]
    fn version_3_dc_values_are_differences() {
        // One macroblock at quantiser 1. Cr and Cb: difference 0 (chroma `00`). Y1: luma `00s`
        // with s = 1, +1*4. Y2: the same again, 8. Y3: `01sn` with s = 0, n = 0: -3*4, so
        // -4 (a 10-bit wrap: 3FCh). Y4: `101snn` = `111`: +7*4, so 24. Then end of frame.
        let mut bits = String::new();
        for code in ["00", "00", "001", "001", "0100", "101111"] {
            bits += code;
            bits += "10"; // EOB
        }
        bits += "1111111111";
        while !bits.len().is_multiple_of(16) {
            bits.push('0');
        }
        let mut bs = vec![32, 0, 0x00, 0x38, 1, 0, 3, 0];
        for ch in bits.as_bytes().chunks(16) {
            let w = ch.iter().fold(0u16, |a, &c| a << 1 | u16::from(c - b'0'));
            bs.extend_from_slice(&w.to_le_bytes());
        }
        let mut stream = vec![0u32; 33];
        assert_eq!(vlc(&bs, &mut stream), Ok(33));
        let halves: Vec<u16> = stream[1..]
            .iter()
            .flat_map(|w| [*w as u16, (*w >> 16) as u16])
            .collect();
        let expected = [
            0x400, 0xFE00, 0x400, 0xFE00, 0x404, 0xFE00, 0x408, 0xFE00, 0x7FC, 0xFE00, 0x418,
            0xFE00,
        ];
        assert_eq!(&halves[..12], &expected);
        assert!(halves[12..].iter().all(|&h| h == 0xFE00));
    }

    #[test]
    fn version_3_dc_size_categories() {
        let bits = |s: &str| {
            let mut v: Vec<u8> = Vec::new();
            let mut padded = s.to_string();
            while !padded.len().is_multiple_of(16) {
                padded.push('0');
            }
            for ch in padded.as_bytes().chunks(16) {
                let w = ch.iter().fold(0u16, |a, &c| a << 1 | u16::from(c - b'0'));
                v.extend_from_slice(&w.to_le_bytes());
            }
            v
        };
        let decode = |s: &str, luma| {
            let data = bits(s);
            let mut b = Bits {
                data: &data,
                pos: 0,
                buf: 0,
                have: 0,
            };
            dc_diff_v3(&mut b, luma)
        };
        // Largest chroma and luma categories (8 bits), positive and negative.
        assert_eq!(decode("11111110111111111", false), Some(255 * 4));
        assert_eq!(decode("11111110000000000", false), Some(-255 * 4));
        assert_eq!(decode("11111101000000000", true), Some(128 * 4));
        assert_eq!(decode("11111100000000000", true), Some(-255 * 4));
        // Zero differences and the unused codes.
        assert_eq!(decode("100", true), Some(0));
        assert_eq!(decode("00", false), Some(0));
        assert_eq!(decode("111111110", true), None);
        assert_eq!(decode("111111110", false), None);
    }

    #[test]
    fn flat_gray_macroblock() {
        // One frame: six blocks, each a DC only, then the end-of-frame DC.
        // Header: words = 32 (one 32-word unit), magic, q = 1, version 2.
        let mut bits = String::new();
        for dc in [0u32, 0, 0x20, 0x20, 0x20, 0x20] {
            bits += &format!("{dc:010b}");
            bits += "10"; // EOB
        }
        bits += "0111111111";
        while !bits.len().is_multiple_of(16) {
            bits.push('0');
        }
        let mut bs = vec![32, 0, 0x00, 0x38, 1, 0, 2, 0];
        for ch in bits.as_bytes().chunks(16) {
            let w = ch.iter().fold(0u16, |a, &c| a << 1 | u16::from(c - b'0'));
            bs.extend_from_slice(&w.to_le_bytes());
        }
        let mut stream = vec![0u32; 33];
        assert_eq!(vlc(&bs, &mut stream), Ok(33));
        assert_eq!(stream[0], 0x3800_0020);
        let mut m = Mdec::new();
        m.start(&stream, true, false);
        let mut px = vec![0u32; 192];
        m.out(&mut px);
        // Luma DC 0x20 * quant[0] (=2 at q 1: 0x20*2 = 64), IDCT gives 64 * 0x5A82^2 / 2^32 = 32.
        let bytes: Vec<u8> = px.iter().flat_map(|w| w.to_le_bytes()).collect();
        assert!(bytes.iter().all(|&b| b == 128 + 8), "{:?}", &bytes[..6]);
    }
}
