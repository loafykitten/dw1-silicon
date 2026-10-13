//! backend::mdec against DW2-PC's mdec.c: every 16-bit lookup of the AC table, random STR frames
//! with valid run-length/VLC streams, and random run-length streams through the MDEC.

#[cfg(have_dw2_c)]
mod diff {
    use backend::mdec::{Mdec, VlcError, ac_codes, vlc};
    use decoder_diff::*;

    /// Bits, MSB first, packed into 16-bit little-endian words as the STR bitstream is.
    #[derive(Default)]
    struct BitWriter {
        bits: Vec<bool>,
    }

    impl BitWriter {
        fn put(&mut self, v: u32, n: u32) {
            for i in (0..n).rev() {
                self.bits.push(v >> i & 1 != 0);
            }
        }

        fn bytes(&self) -> Vec<u8> {
            let mut out = Vec::new();
            for ch in self.bits.chunks(16) {
                let mut w = 0u16;
                for i in 0..16 {
                    w = w << 1 | u16::from(ch.get(i).copied().unwrap_or(false));
                }
                out.extend_from_slice(&w.to_le_bytes());
            }
            out
        }
    }

    fn header(words: u32, q: u32, version: u32) -> Vec<u8> {
        let mut h = Vec::new();
        for v in [words, 0x3800, q, version] {
            h.extend_from_slice(&(v as u16).to_le_bytes());
        }
        h
    }

    /// Runs both `vlc`s and compares; returns the Rust stream when it succeeded.
    fn compare_vlc(bs: &[u8], out_words: usize, ctx: &str) -> Option<Vec<u32>> {
        let (cr, cout) = mdec_vlc(bs, out_words);
        let mut rout = vec![0xDEAD_BEEFu32; out_words.max(1)];
        let rr = vlc(bs, &mut rout[..out_words]);
        match rr {
            Ok(n) => {
                assert_eq!(cr, n as i32, "{ctx}: return");
                assert_eq!(&cout[..n], &rout[..n], "{ctx}: output");
                Some(rout[..n].to_vec())
            }
            Err(e) => {
                assert_eq!(cr, -1, "{ctx}: C accepted what Rust rejected ({e:?})");
                if out_words > 0 {
                    assert_eq!(cout[0], rout[0], "{ctx}: empty stream marker");
                    assert_eq!(rout[0], 0x3800_0000);
                }
                None
            }
        }
    }

    /// Every 16-bit pattern as the start of an AC code: the code's length is probed through the
    /// Rust table, a tail (EOB, end of frame) is placed after it, and C must agree on the
    /// result. Patterns Rust calls unused must be rejected by C for every code length.
    #[test]
    fn every_ac_pattern() {
        let _g = guard();
        let codes = ac_codes();
        let mut by_pat = vec![None; 1 << 16];
        for c in &codes {
            let base = u32::from(c.bits) << (16 - c.len);
            for i in 0..1u32 << (16 - c.len) {
                assert!(
                    by_pat[(base | i) as usize].replace(*c).is_none(),
                    "overlap at {c:?}"
                );
            }
        }
        let build = |dc: u32, pat: u32, len: u32, sign: u32| {
            // DC, then `len` bits of the pattern + sign, then EOB, then end of frame.
            let mut w = BitWriter::default();
            w.put(dc, 10);
            w.put(pat >> (16 - len), len);
            w.put(sign, 1);
            w.put(0b10, 2);
            w.put(0x1FF, 10);
            let mut bs = header(32, 3, 2);
            bs.extend(w.bytes());
            bs
        };
        let (mut ok, mut rejected, mut checked) = (0, 0, 0);
        for pat in 0..1u32 << 16 {
            let dc = ((pat * 7 + 3) & 0x1FF).min(0x1FE);
            match by_pat[pat as usize] {
                Some(c) => {
                    for sign in 0..2 {
                        let bs = build(dc, pat, u32::from(c.len), sign);
                        let r = compare_vlc(&bs, 33, &format!("pattern {pat:#06x} sign {sign}"));
                        let s = r.expect("a valid code decodes");
                        let level = if sign == 1 {
                            (0x400 - u32::from(c.level)) & 0x3FF
                        } else {
                            u32::from(c.level)
                        };
                        // [header, dc, ac, eob] as halfwords.
                        assert_eq!(s[1] & 0xFFFF, 3 << 10 | dc);
                        assert_eq!(s[1] >> 16, u32::from(c.run) << 10 | level);
                        ok += 1;
                    }
                }
                None => {
                    // EOB ("10..") and escape ("000001..") are covered by the random frames
                    // below; the 12-zero prefix is unused and C must reject every length.
                    if pat >> 12 == 0 {
                        for len in 1..=16u32 {
                            let bs = build(dc, pat, len, 0);
                            let r = compare_vlc(&bs, 33, &format!("unused {pat:#06x} len {len}"));
                            let _ = r;
                            rejected += 1;
                        }
                    }
                }
            }
            checked += 1;
        }
        println!(
            "mdec: {checked} AC patterns, {ok} valid decodes, {rejected} unused-code probes compared with C"
        );
    }

    /// A random valid STR frame: macroblocks of six blocks with random DC and AC codes.
    fn random_frame(rng: &mut Rng, codes: &[backend::mdec::AcCode]) -> (Vec<u8>, usize) {
        let mut w = BitWriter::default();
        let blocks = 6 * rng.range(1, 4);
        let mut halfwords = 0usize;
        for _ in 0..blocks {
            let mut dc = rng.below(0x200);
            if rng.one_in(2) {
                dc = 0x3FF - rng.below(0x200); // negative DC
            }
            if dc == 0x1FF {
                dc = 0x1FE;
            }
            w.put(dc, 10);
            halfwords += 1;
            for _ in 0..rng.below(24) {
                if rng.one_in(8) {
                    // Escape: 6-bit run, 10-bit raw level (any value, including 0).
                    w.put(0b000001, 6);
                    w.put(rng.below(64), 6);
                    w.put(rng.below(1024), 10);
                } else {
                    let c = codes[rng.below(codes.len() as u32) as usize];
                    w.put(u32::from(c.bits), u32::from(c.len));
                    w.put(rng.below(2), 1);
                }
                halfwords += 1;
            }
            w.put(0b10, 2);
            halfwords += 1;
        }
        w.put(0x1FF, 10);
        (w.bytes(), halfwords)
    }

    fn rgb_compare(stream: &[u32], rng: &mut Rng, ctx: &str) -> usize {
        let mut compared = 0;
        for (rgb24, stp) in [(true, false), (false, false), (false, true), (true, true)] {
            let mut m = Mdec::new();
            m.start(stream, rgb24, stp);
            let per_mb = if rgb24 { 192 } else { 128 };
            // Whole macroblocks, plus some past the end of the stream, plus a ragged tail.
            let mbs = rng.range(1, 6) as usize;
            let words = mbs * per_mb + rng.below(per_mb as u32 - 1) as usize;
            let c = mdec_decode(stream, rgb24, stp, words);
            let mut r = vec![0xDEAD_BEEFu32; words];
            m.out(&mut r);
            assert_eq!(c, r, "{ctx}: rgb24 {rgb24} stp {stp}");
            compared += words;
        }
        compared
    }

    /// Empty and truncated run-length streams (header claiming more words than present) decode
    /// the same: the stream ends where the data ends.
    #[test]
    fn empty_and_truncated_streams() {
        let _g = guard();
        let mut rng = Rng::new(0x3DEC_00E7);
        let mut streams: Vec<Vec<u32>> = vec![vec![], vec![0x3800_0000], vec![0x3800_0020]];
        for _ in 0..200 {
            let present = rng.below(40) as usize;
            let claimed = present as u32 + rng.below(40);
            let mut s = vec![0x3800_0000 | claimed];
            s.extend((0..present).map(|_| rng.u32()));
            streams.push(s);
        }
        for (i, s) in streams.iter().enumerate() {
            rgb_compare(s, &mut rng, &format!("truncated stream {i}"));
        }
    }

    #[test]
    fn random_frames_and_macroblocks() {
        let _g = guard();
        let codes = ac_codes();
        let mut rng = Rng::new(0x3DEC_0001);
        let n = cases(1500);
        let (mut frames, mut rejected, mut words_cmp, mut pixels) = (0, 0, 0, 0);
        for case in 0..n {
            let (body, halfwords) = random_frame(&mut rng, &codes);
            // Header: 32-word units, enough (or sometimes too few); quantiser 0..=63 (and a few
            // out of range); version 1 or 2 (and a few bad ones).
            let units = halfwords.div_ceil(64);
            let mut units = units as u32;
            match rng.below(10) {
                0 => units = units.saturating_sub(1),
                1 => units += rng.below(3),
                _ => {}
            }
            let q = if rng.one_in(30) {
                rng.range(64, 100)
            } else if rng.one_in(6) {
                0
            } else {
                rng.below(64)
            };
            // (Version 3 is the Rust decoder's alone: DW2-PC's C does 1 and 2.)
            let version = if rng.one_in(30) {
                rng.below(3)
            } else {
                rng.range(1, 2)
            };
            let mut bs = header(units * 32, q, version);
            bs.extend(body);
            let out_words = if rng.one_in(20) {
                rng.below(units * 32 + 2) as usize
            } else {
                units as usize * 32 + 1
            };
            let ctx = format!("frame {case}");
            frames += 1;
            let Some(stream) = compare_vlc(&bs, out_words, &ctx) else {
                rejected += 1;
                continue;
            };
            words_cmp += stream.len();
            pixels += rgb_compare(&stream, &mut rng, &ctx);
        }
        println!(
            "mdec: {frames} random frames ({rejected} rejected by both), {words_cmp} run-length words, {pixels} output words compared"
        );
    }

    #[test]
    fn random_bitstreams_never_disagree() {
        let _g = guard();
        let mut rng = Rng::new(0x3DEC_0002);
        let n = cases(3000);
        let mut accepted = 0;
        for case in 0..n {
            let words = rng.range(1, 3) * 32;
            let mut bs = header(words, rng.below(64), rng.range(1, 2));
            let mut body = vec![0u8; rng.range(4, 600) as usize];
            rng.fill(&mut body);
            // Bias towards plausible streams: make the high bits of many bytes zero.
            if rng.one_in(2) {
                for b in &mut body {
                    *b &= rng.u8();
                }
            }
            bs.extend(body);
            if compare_vlc(&bs, words as usize + 1, &format!("noise {case}")).is_some() {
                accepted += 1;
            }
        }
        println!("mdec: {n} random bitstreams, {accepted} accepted, all agreed with C");
    }

    #[test]
    fn random_run_length_streams() {
        let _g = guard();
        let mut rng = Rng::new(0x3DEC_0003);
        let n = cases(1500);
        let mut pixels = 0;
        for case in 0..n {
            let words = rng.range(1, 90) as usize;
            let mut stream = vec![words as u32 | 0x3800_0000];
            for _ in 0..words {
                let mut w = 0u32;
                for h in 0..2 {
                    let v = match rng.below(8) {
                        0 | 1 => 0xFE00,
                        // A first word of a block: random quantiser scale (0 included) and DC.
                        2 => u32::from(rng.u16()),
                        // Mostly small runs, so blocks fill up without running past k = 63.
                        _ => rng.below(4) << 10 | (rng.below(1024)),
                    };
                    w |= v << (16 * h);
                }
                stream.push(w);
            }
            pixels += rgb_compare(&stream, &mut rng, &format!("rl stream {case}"));
        }
        println!("mdec: {n} random run-length streams, {pixels} output words compared");
    }

    #[test]
    fn vlc_error_kinds() {
        let mut out = [0u32; 8];
        assert_eq!(vlc(&header(2, 70, 2), &mut out), Err(VlcError::BadHeader));
    }
}

/// Shows up as ignored (not passed) in a checkout without `ref/DW2-PC`.
#[cfg(not(have_dw2_c))]
#[test]
#[ignore = "ref/DW2-PC is absent: the mdec differential tests did not run"]
fn skipped_without_dw2_pc() {}
