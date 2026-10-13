//! CD-XA ADPCM audio decoder and the 37.8 -> 44.1 kHz resampler of the PS1 CD controller.
//!
//! Translated from DW2-PC (github.com/Wyrelade/DW2-PC, CC0), `backend/xadec.c` and
//! `backend/xadec.h`, at commit 2d491b5. The decoder logic is DW2-PC's; the literal tables are
//! not: the zigzag interpolation table and the ADPCM prediction filters are transcribed from
//! psx-spx, "CDROM XA Audio ADPCM Compression"
//! (psx-spx.consoledev.net/cdromformat/), see [`XA_ZIGZAG_ROWS`] and [`XA_FILTER_POS`].
//!
//! A sector body is the 8-byte subheader (file, channel, submode, coding info twice) and 18
//! sound groups of 128 bytes: 16 header bytes (unit u's shift and filter at byte 4 + u) and 28
//! data words (unit u's sample j at byte 16 + j * 4 + u / 2, low nibble for even u; 8-bit: byte
//! 16 + j * 4 + u). 4-bit groups hold 8 units of 28 samples, 8-bit groups 4; stereo units
//! alternate left and right. 18.9 kHz samples enter the resampler twice. The 44.1 kHz frames
//! wait in a FIFO that the SPU's CD input reads one frame per output sample.
//!
//! Differences from the C: the state lives in [`XaDecoder`] instead of file-scope statics, and a
//! sector shorter than 2336 bytes is zero padded instead of read out of bounds.

/// 44.1 kHz stereo frames the FIFO holds (about 370 ms).
pub const FIFO_FRAMES: usize = 16384;

/// Bytes of a Mode 2 Form 2 sector body: subheader plus 2328 bytes.
pub const SECTOR_BODY_SIZE: usize = 2336;

const GROUPS: usize = 18;
const GROUP_SIZE: usize = 128;
const RAW_FRAMES_MAX: usize = GROUPS * 224;

/// ADPCM prediction filter coefficients applied to the previous sample ("old") and the one
/// before it ("older"). Source: psx-spx, "CDROM XA Audio ADPCM Compression", Pos/neg Tables
/// (psx-spx.consoledev.net/cdromformat/). The page lists five filters (the SPU's); XA uses the
/// first four.
pub const XA_FILTER_POS: [i32; 5] = [0, 60, 115, 98, 122];
/// See [`XA_FILTER_POS`].
pub const XA_FILTER_NEG: [i32; 5] = [0, 0, -52, -55, -60];

/// The controller's 29-tap zigzag interpolation: 7 output samples per 6 input samples.
///
/// Source: psx-spx, "CDROM XA Audio ADPCM Compression", "25-point Zigzag Interpolation"
/// (psx-spx.consoledev.net/cdromformat/). Laid out as on that page: row `i - 1` holds index
/// `i` (1..=29) of Table1..Table7 in columns 0..=6. Tap `i` weighs the input sample `i`
/// places before the newest (`ringbuf[(p - i) AND 1Fh]` on the page).
pub const XA_ZIGZAG_ROWS: [[i16; 7]; 29] = [
    [0x0000, 0x0000, 0x0000, 0x0000, -0x0001, 0x0002, -0x0005],
    [0x0000, 0x0000, 0x0000, -0x0001, 0x0003, -0x0008, 0x0011],
    [0x0000, 0x0000, -0x0001, 0x0003, -0x0008, 0x0010, -0x0023],
    [0x0000, -0x0002, 0x0003, -0x0008, 0x0011, -0x0023, 0x0046],
    [0x0000, 0x0000, -0x0002, 0x0006, -0x0010, 0x002B, -0x0017],
    [-0x0002, 0x0003, -0x0005, 0x0005, 0x000A, 0x001A, -0x0044],
    [0x000A, -0x0013, 0x001F, -0x001B, 0x006B, -0x00EB, 0x015B],
    [-0x0022, 0x003C, -0x004A, 0x00A6, -0x016D, 0x027B, -0x0347],
    [0x0041, -0x004B, 0x00B3, -0x01A8, 0x0350, -0x0548, 0x080E],
    [-0x0054, 0x00A2, -0x0192, 0x0372, -0x0623, 0x0AFA, -0x1249],
    [0x0034, -0x00E3, 0x02B1, -0x05BF, 0x0BCD, -0x16FA, 0x3C07],
    [0x0009, 0x0132, -0x039E, 0x09B8, -0x1780, 0x53E0, 0x53E0],
    [-0x010A, -0x0043, 0x04F8, -0x11B4, 0x6794, 0x3C07, -0x16FA],
    [0x0400, -0x0267, -0x05A6, 0x74BB, 0x234C, -0x1249, 0x0AFA],
    [-0x0A78, 0x0C9D, 0x7939, 0x0C9D, -0x0A78, 0x080E, -0x0548],
    [0x234C, 0x74BB, -0x05A6, -0x0267, 0x0400, -0x0347, 0x027B],
    [0x6794, -0x11B4, 0x04F8, -0x0043, -0x010A, 0x015B, -0x00EB],
    [-0x1780, 0x09B8, -0x039E, 0x0132, 0x0009, -0x0044, 0x001A],
    [0x0BCD, -0x05BF, 0x02B1, -0x00E3, 0x0034, -0x0017, 0x002B],
    [-0x0623, 0x0372, -0x0192, 0x00A2, -0x0054, 0x0046, -0x0023],
    [0x0350, -0x01A8, 0x00B3, -0x004B, 0x0041, -0x0023, 0x0010],
    [-0x016D, 0x00A6, -0x004A, 0x003C, -0x0022, 0x0011, -0x0008],
    [0x006B, -0x001B, 0x001F, -0x0013, 0x000A, -0x0005, 0x0002],
    [0x000A, 0x0005, -0x0005, 0x0003, -0x0001, 0x0000, 0x0000],
    [-0x0010, 0x0006, -0x0002, 0x0000, 0x0000, 0x0000, 0x0000],
    [0x0011, -0x0008, 0x0003, -0x0002, 0x0001, 0x0000, 0x0000],
    [-0x0008, 0x0003, -0x0001, 0x0000, 0x0000, 0x0000, 0x0000],
    [0x0003, -0x0001, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000],
    [-0x0001, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000],
];

fn clamp16(x: i32) -> i32 {
    x.clamp(-0x8000, 0x7FFF)
}

/// XA-ADPCM decoder, resampler and FIFO.
pub struct XaDecoder {
    /// ADPCM history per channel: old, older (0 = left or mono, 1 = right).
    hist: [[i32; 2]; 2],
    /// Resampler input per channel.
    ring: [[i16; 32]; 2],
    ring_pos: u32,
    sixstep: i32,
    fifo: Vec<[i16; 2]>,
    fifo_head: usize,
    fifo_count: usize,
    /// The last sector before the resampler.
    raw: Vec<[i16; 2]>,
    raw_frames: usize,
    /// A stream runs and has sent its first sector.
    counting: bool,
    underruns: i32,
    /// Frames per kept frame, 0 = drop all.
    speed: i32,
    sum_l: i32,
    sum_r: i32,
    summed: i32,
}

impl Default for XaDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl XaDecoder {
    pub fn new() -> XaDecoder {
        XaDecoder {
            hist: [[0; 2]; 2],
            ring: [[0; 32]; 2],
            ring_pos: 0,
            sixstep: 6,
            fifo: vec![[0; 2]; FIFO_FRAMES],
            fifo_head: 0,
            fifo_count: 0,
            raw: vec![[0; 2]; RAW_FRAMES_MAX],
            raw_frames: 0,
            counting: false,
            underruns: 0,
            speed: 1,
            sum_l: 0,
            sum_r: 0,
            summed: 0,
        }
    }

    /// New stream: clears the FIFO, the ADPCM history and the resampler. The speed setting stays.
    pub fn reset(&mut self) {
        self.hist = [[0; 2]; 2];
        self.ring = [[0; 32]; 2];
        self.ring_pos = 0;
        self.sixstep = 6;
        self.fifo_head = 0;
        self.fifo_count = 0;
        self.raw_frames = 0;
        self.counting = false;
        self.underruns = 0;
        self.sum_l = 0;
        self.sum_r = 0;
        self.summed = 0;
    }

    /// The game runs `n` times faster, so the sectors come `n` times faster. `n >= 2` keeps one
    /// averaged frame of every `n`; 0 (no limit) drops the sound; 1 = normal.
    pub fn set_speed(&mut self, n: i32) {
        if n != self.speed {
            self.speed = n;
            self.sum_l = 0;
            self.sum_r = 0;
            self.summed = 0;
        }
    }

    fn fifo_put(&mut self, mut l: i32, mut r: i32) {
        if self.speed != 1 {
            if self.speed == 0 {
                return;
            }
            self.sum_l = self.sum_l.wrapping_add(l);
            self.sum_r = self.sum_r.wrapping_add(r);
            self.summed += 1;
            if self.summed < self.speed {
                return;
            }
            l = self.sum_l / self.speed;
            r = self.sum_r / self.speed;
            self.sum_l = 0;
            self.sum_r = 0;
            self.summed = 0;
        }
        if self.fifo_count == FIFO_FRAMES {
            // The consumer stalled: drop the oldest frame.
            self.fifo_head = (self.fifo_head + 1) % FIFO_FRAMES;
            self.fifo_count -= 1;
        }
        let i = (self.fifo_head + self.fifo_count) % FIFO_FRAMES;
        self.fifo[i] = [l as i16, r as i16];
        self.fifo_count += 1;
    }

    fn interpolate(&self, ch: usize, k: usize) -> i32 {
        let mut sum = 0i32;
        for (i, row) in XA_ZIGZAG_ROWS.iter().enumerate() {
            let s =
                i32::from(self.ring[ch][(self.ring_pos.wrapping_sub(i as u32 + 1) & 31) as usize]);
            sum += (s * i32::from(row[k])) >> 15;
        }
        clamp16(sum)
    }

    /// One 37.8 kHz input frame into the resampler; every sixth adds seven 44.1 kHz frames.
    fn resample(&mut self, l: i32, r: i32) {
        let p = (self.ring_pos & 31) as usize;
        self.ring[0][p] = l as i16;
        self.ring[1][p] = r as i16;
        self.ring_pos = self.ring_pos.wrapping_add(1);
        self.sixstep -= 1;
        if self.sixstep == 0 {
            self.sixstep = 6;
            for k in 0..7 {
                let (a, b) = (self.interpolate(0, k), self.interpolate(1, k));
                self.fifo_put(a, b);
            }
        }
    }

    /// 28 samples of sound unit `u` in a group: 4-bit (nibble) or 8-bit.
    fn decode_unit(&mut self, grp: &[u8], u: usize, bits8: bool, ch: usize, out: &mut [i16; 28]) {
        let hdr = usize::from(grp[4 + u]);
        let mut shift = (hdr & 0xF) as u32;
        let filter = (hdr >> 4) & 3;
        let f0 = XA_FILTER_POS[filter];
        let f1 = XA_FILTER_NEG[filter];
        let mut old = self.hist[ch][0];
        let mut older = self.hist[ch][1];
        if shift > 12 {
            shift = 9;
        }
        for (j, o) in out.iter_mut().enumerate() {
            let t = if bits8 {
                i32::from(((u32::from(grp[16 + j * 4 + u]) << 8) as u16 as i16) >> shift)
            } else {
                let nib = (grp[16 + j * 4 + u / 2] >> ((u & 1) * 4)) & 0xF;
                i32::from(((u32::from(nib) << 12) as u16 as i16) >> shift)
            };
            let s = clamp16(t + ((old * f0 + older * f1 + 32) >> 6));
            *o = s as i16;
            older = old;
            old = s;
        }
        self.hist[ch][0] = old;
        self.hist[ch][1] = older;
    }

    /// Decodes one audio sector into the FIFO. `body` starts at the subheader; a short body is
    /// zero padded to [`SECTOR_BODY_SIZE`].
    pub fn decode_sector(&mut self, body: &[u8]) {
        let mut buf = [0u8; SECTOR_BODY_SIZE];
        let n = body.len().min(SECTOR_BODY_SIZE);
        buf[..n].copy_from_slice(&body[..n]);

        let ci = buf[3];
        let stereo = (ci & 3) == 1;
        let half = (ci >> 2) & 1;
        let bits8 = ((ci >> 4) & 3) == 1;
        let units = if bits8 { 4 } else { 8 };
        let mut s = [[0i16; 28]; 2];

        self.raw_frames = 0;
        self.counting = true;
        for g in 0..GROUPS {
            let grp = &buf[8 + g * GROUP_SIZE..8 + (g + 1) * GROUP_SIZE];
            let mut u = 0;
            while u < units {
                self.decode_unit(grp, u, bits8, 0, &mut s[0]);
                if stereo {
                    self.decode_unit(grp, u + 1, bits8, 1, &mut s[1]);
                }
                for (j, &l) in s[0].iter().enumerate() {
                    let r = if stereo { s[1][j] } else { l };
                    self.raw[self.raw_frames] = [l, r];
                    self.raw_frames += 1;
                    for _ in 0..=half {
                        self.resample(i32::from(l), i32::from(r));
                    }
                }
                u += if stereo { 2 } else { 1 };
            }
        }
    }

    /// Next 44.1 kHz stereo frame for the SPU CD input; zeros when the FIFO is empty.
    pub fn pop(&mut self) -> (i16, i16) {
        if self.fifo_count == 0 {
            if self.counting {
                // Saturating: a stream starved for hours must not overflow (found in review).
                self.underruns = self.underruns.saturating_add(1);
            }
            return (0, 0);
        }
        let [l, r] = self.fifo[self.fifo_head];
        self.fifo_head = (self.fifo_head + 1) % FIFO_FRAMES;
        self.fifo_count -= 1;
        (l, r)
    }

    /// Frames waiting in the FIFO.
    pub fn queued(&self) -> usize {
        self.fifo_count
    }

    /// Pops that found the FIFO empty since the first sector of the stream.
    pub fn underruns(&self) -> i32 {
        self.underruns
    }

    /// Ends the underrun count when the stream stops (frames already decoded still play out).
    pub fn stop(&mut self) {
        self.counting = false;
    }

    /// The 37.8 kHz decode before the resampler (frames, L R) of the last sector; 18.9 kHz
    /// sectors count each sample once.
    pub fn last_raw(&self) -> &[[i16; 2]] {
        &self.raw[..self.raw_frames]
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn underrun_count_saturates() {
        let mut xa = XaDecoder::new();
        xa.counting = true;
        xa.underruns = i32::MAX - 1;
        for _ in 0..3 {
            assert_eq!(xa.pop(), (0, 0));
        }
        assert_eq!(xa.underruns(), i32::MAX);
    }
    use super::*;

    #[test]
    fn zigzag_table_matches_psx_spx_spots() {
        assert_eq!(XA_ZIGZAG_ROWS.len(), 29);
        // Rows read off the psx-spx page (index 1, 12, 15, 29).
        assert_eq!(XA_ZIGZAG_ROWS[0], [0, 0, 0, 0, -1, 2, -5]);
        assert_eq!(
            XA_ZIGZAG_ROWS[11],
            [0x09, 0x132, -0x39E, 0x9B8, -0x1780, 0x53E0, 0x53E0]
        );
        assert_eq!(
            XA_ZIGZAG_ROWS[14],
            [-0xA78, 0xC9D, 0x7939, 0xC9D, -0xA78, 0x80E, -0x548]
        );
        assert_eq!(XA_ZIGZAG_ROWS[28], [-1, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn zigzag_table_properties() {
        // Table3 is symmetric around its centre tap (index 15).
        for i in 0..29 {
            assert_eq!(
                XA_ZIGZAG_ROWS[i][2],
                XA_ZIGZAG_ROWS[28 - i][2],
                "table3 tap {i}"
            );
        }
        // Table4 is Table2 reversed.
        for i in 0..29 {
            assert_eq!(
                XA_ZIGZAG_ROWS[i][3],
                XA_ZIGZAG_ROWS[28 - i][1],
                "table4 tap {i}"
            );
        }
    }

    #[test]
    fn filters_match_psx_spx() {
        assert_eq!(XA_FILTER_POS, [0, 60, 115, 98, 122]);
        assert_eq!(XA_FILTER_NEG, [0, 0, -52, -55, -60]);
    }

    #[test]
    fn silent_sector_gives_silence_and_right_count() {
        let mut xa = XaDecoder::new();
        let mut body = [0u8; SECTOR_BODY_SIZE];
        body[3] = 0x01; // stereo, 37.8 kHz, 4-bit
        xa.decode_sector(&body);
        // 18 groups * 4 stereo unit pairs * 28 = 2016 frames in, 7/6 out.
        assert_eq!(xa.last_raw().len(), 2016);
        assert_eq!(xa.queued(), 2016 / 6 * 7);
        assert_eq!(xa.pop(), (0, 0));
        xa.reset();
        assert_eq!(xa.queued(), 0);
    }
}
