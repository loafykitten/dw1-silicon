//! backend::spu against DW2-PC's psxspu.c: random register writes, key-ons with random ADPCM
//! blocks in sound RAM, reverb on and off, noise, pitch modulation, volume sweeps and the CD
//! input, compared sample by sample.

#[cfg(have_dw2_c)]
mod diff {
    use backend::spu::{RAM_SIZE, Spu};
    use backend::xa::{SECTOR_BODY_SIZE, XaDecoder};
    use decoder_diff::*;

    /// Both implementations, driven in lockstep.
    struct Pair {
        spu: Spu,
        xa: XaDecoder,
    }

    impl Pair {
        fn new() -> Pair {
            spu_reset();
            xa_reset();
            xa_set_speed(1);
            let mut xa = XaDecoder::new();
            xa.reset();
            xa.set_speed(1);
            Pair {
                spu: Spu::new(),
                xa,
            }
        }

        fn w(&mut self, off: u32, v: u16) {
            spu_write16(off, v);
            self.spu.write16(off, v);
        }

        fn render(&mut self, frames: usize, ctx: &str) -> usize {
            let mut c = vec![0i16; frames * 2];
            let mut r = vec![0i16; frames * 2];
            spu_render(&mut c);
            let xa = &mut self.xa;
            self.spu.render(&mut r, &mut || xa.pop());
            if let Some(i) = (0..c.len()).find(|&i| c[i] != r[i]) {
                panic!(
                    "{ctx}: frame {} channel {}: C {} Rust {}",
                    i / 2,
                    i % 2,
                    c[i],
                    r[i]
                );
            }
            c.iter().filter(|&&s| s != 0).count()
        }

        fn compare_regs(&self, ctx: &str) {
            for off in (0..0x400u32).step_by(2) {
                assert_eq!(
                    spu_read16(off),
                    self.spu.read16(off),
                    "{ctx}: read16({off:#x})"
                );
            }
        }

        fn compare_ram(&self, ctx: &str) {
            let c = spu_ram();
            if let Some(i) = (0..RAM_SIZE).find(|&i| c[i] != self.spu.ram()[i]) {
                panic!(
                    "{ctx}: RAM[{i:#x}]: C {:#x} Rust {:#x}",
                    c[i],
                    self.spu.ram()[i]
                );
            }
        }
    }

    /// Random sound RAM: ADPCM samples of 3..30 blocks (loop start flag on the first, loop end
    /// on the last, repeat or mute), then noise. Returns the block-aligned start addresses.
    fn make_ram(rng: &mut Rng, sample_bytes: usize) -> (Vec<u8>, Vec<u32>) {
        let mut ram = vec![0u8; RAM_SIZE];
        let mut starts = Vec::new();
        let mut a = 0usize;
        while a + 16 * 3 < sample_bytes {
            let blocks = rng.range(3, 30) as usize;
            starts.push(a as u32);
            for b in 0..blocks {
                if a + 16 > sample_bytes {
                    break;
                }
                let shift = if rng.one_in(6) {
                    rng.below(16)
                } else {
                    rng.range(5, 12)
                } as u8;
                let filter = if rng.one_in(10) {
                    rng.below(8)
                } else {
                    rng.below(5)
                } as u8;
                ram[a] = shift | filter << 4;
                let mut flags = 0u8;
                if b == 0 && rng.one_in(2) {
                    flags |= 4;
                }
                if b + 1 == blocks {
                    flags |= if rng.one_in(2) { 3 } else { 1 };
                } else if rng.one_in(40) {
                    flags |= rng.below(8) as u8;
                }
                ram[a + 1] = flags;
                rng.fill(&mut ram[a + 2..a + 16]);
                a += 16;
            }
        }
        // The rest (reverb work area and anything else): small random samples, so the reverb
        // has something to chew on.
        for i in (sample_bytes..RAM_SIZE).step_by(2) {
            if rng.one_in(3) {
                let v = (rng.u16() as i16 >> rng.below(10)).to_le_bytes();
                ram[i] = v[0];
                ram[i + 1] = v[1];
            }
        }
        (ram, starts)
    }

    fn direct_or_sweep(rng: &mut Rng) -> u16 {
        if rng.below(10) < 6 {
            rng.u16() & 0x3FFF | if rng.one_in(8) { 0x4000 } else { 0 }
        } else {
            rng.u16() | 0x8000
        }
    }

    /// One random case; returns (frames rendered, nonzero samples).
    fn run_case(rng: &mut Rng, seed_ctx: &str, use_cd: bool) -> (usize, usize) {
        let mut p = Pair::new();
        let sample_bytes = rng.range(0x2000, 0x30000) as usize & !15;
        let (ram, starts) = make_ram(rng, sample_bytes);
        spu_ram_set(&vec![0u8; RAM_SIZE]);

        // Load sound RAM: DMA for the bulk, manual FIFO writes for the first 64 bytes.
        let words: Vec<u32> = ram
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        p.w(0x1A6, 8);
        for i in 0..32 {
            p.w(
                0x1A8,
                u16::from_le_bytes([ram[16 + i * 2], ram[17 + i * 2]]),
            );
        }
        p.w(0x1A6, 0x20); // byte 0x100
        spu_dma_write(&words[0x40..]);
        p.spu.dma_write(&words[0x40..]);
        p.w(0x1A6, 0);
        spu_dma_write(&words[..4]);
        p.spu.dma_write(&words[..4]);
        // Fix up what the two partial loads skipped (bytes 0x50..0x100 and the 0x10..0x50 region
        // written by FIFO already match): write them by FIFO too.
        p.w(0x1A6, 0x0A);
        for i in 0x50 / 2..0x100 / 2 {
            p.w(0x1A8, u16::from_le_bytes([ram[i * 2], ram[i * 2 + 1]]));
        }
        p.compare_ram(&format!("{seed_ctx}: after load"));
        let mut back_c = vec![0u32; 8];
        let mut back_r = vec![0u32; 8];
        p.w(0x1A6, 0x40);
        spu_dma_read(&mut back_c);
        p.spu.dma_read(&mut back_r);
        assert_eq!(back_c, back_r, "{seed_ctx}: DMA read back");

        // Reverb area and registers.
        let esa = rng.range(sample_bytes as u32 / 8 + 0x100, 0xE000);
        p.w(0x1A2, esa as u16);
        let reverb_on = rng.below(3) != 0;
        for r in 0..32u32 {
            let v = match r {
                0 | 1 => rng.below(0x400) as u16,
                0x1E | 0x1F => rng.u16() >> 1,
                2..=9 => rng.u16() >> rng.below(3),
                _ => {
                    // Buffer addresses (8-byte units) inside the work area, sometimes garbage.
                    if rng.one_in(12) {
                        rng.u16()
                    } else {
                        rng.range(esa, 0xFFFF) as u16
                    }
                }
            };
            p.w(0x1C0 + r * 2, v);
        }
        // ATTR: enable, unmute, CD and reverb bits, noise clock.
        let mut attr = 0xC000u16 | (rng.u16() & 0x3F3F) | if reverb_on { 0x80 } else { 0 };
        if rng.one_in(10) {
            attr = rng.u16();
        }
        if use_cd {
            attr |= 1 | if rng.one_in(2) { 4 } else { 0 };
        }
        p.w(0x1AA, attr);
        p.w(0x180, direct_or_sweep(rng));
        p.w(0x182, direct_or_sweep(rng));
        p.w(0x184, rng.u16() >> 1);
        p.w(0x186, rng.u16() >> 1);
        p.w(0x1B0, rng.u16() & 0x7FFF);
        p.w(0x1B2, rng.u16() & 0x7FFF);

        let density = |rng: &mut Rng, one_in: u32| -> u32 {
            let mut m = 0u32;
            for i in 0..24 {
                if rng.one_in(one_in) {
                    m |= 1 << i;
                }
            }
            m
        };
        let setup_voice = |p: &mut Pair, rng: &mut Rng, i: u32| {
            let o = i * 16;
            p.w(o, direct_or_sweep(rng));
            p.w(o + 2, direct_or_sweep(rng));
            let pitch = match rng.below(6) {
                0 => rng.u16(),
                1 => rng.below(0x4800) as u16,
                _ => (0x1000u32 * rng.range(1, 400) / 100) as u16,
            };
            p.w(o + 4, pitch);
            let start = starts[rng.below(starts.len() as u32) as usize];
            p.w(o + 6, (start >> 3) as u16);
            let a1 = if rng.one_in(4) {
                rng.u16()
            } else {
                (rng.u16() & 0x80FF) | (rng.below(10) as u16) << 10
            };
            p.w(o + 8, a1);
            p.w(o + 10, rng.u16());
            if rng.one_in(12) {
                p.w(
                    o + 14,
                    (starts[rng.below(starts.len() as u32) as usize] >> 3) as u16,
                );
            }
            if rng.one_in(30) {
                p.w(o + 12, rng.u16());
            }
        };
        let pm = density(rng, 5);
        let no = density(rng, 12);
        let eo = if reverb_on {
            density(rng, 2)
        } else {
            density(rng, 6)
        };
        for (off, m) in [(0x190, pm), (0x194, no), (0x198, eo)] {
            p.w(off, m as u16);
            p.w(off + 2, (m >> 16) as u16);
        }
        let kon = density(rng, 3) | 1 << rng.below(24);
        for i in 0..24 {
            setup_voice(&mut p, rng, i);
        }
        p.w(0x188, kon as u16);
        p.w(0x18A, (kon >> 16) as u16);

        let mut frames = 0;
        let mut nonzero = 0;
        for round in 0..rng.range(3, 7) {
            if use_cd && rng.one_in(2) {
                let mut body = vec![0u8; SECTOR_BODY_SIZE];
                rng.fill(&mut body);
                body[3] = [0x00, 0x01, 0x04, 0x05, 0x10, 0x11][rng.below(6) as usize];
                for g in 0..18 {
                    for u in 0..8 {
                        body[8 + g * 128 + 4 + u] =
                            rng.range(8, 12) as u8 | (rng.below(4) as u8) << 4;
                    }
                }
                xa_sector(&body);
                p.xa.decode_sector(&body);
            }
            let n = rng.range(1, 700) as usize;
            nonzero += p.render(n, &format!("{seed_ctx}: round {round}"));
            frames += n;
            p.compare_regs(&format!("{seed_ctx}: round {round}"));
            // Mid-run poking: key off, re-key, volumes, pitch, ATTR toggles.
            match rng.below(5) {
                0 => {
                    let m = density(rng, 3);
                    p.w(0x18C, m as u16);
                    p.w(0x18E, (m >> 16) as u16);
                }
                1 => {
                    let m = density(rng, 4);
                    for i in 0..24 {
                        if m & 1 << i != 0 {
                            setup_voice(&mut p, rng, i);
                        }
                    }
                    p.w(0x188, m as u16);
                    p.w(0x18A, (m >> 16) as u16);
                }
                2 => {
                    p.w(0x180, direct_or_sweep(rng));
                    p.w(0x1AA, rng.u16() | 0xC000);
                }
                3 => {
                    for _ in 0..4 {
                        let i = rng.below(24);
                        p.w(i * 16 + 4, rng.u16());
                        p.w(i * 16 + 8, rng.u16());
                        p.w(i * 16 + 10, rng.u16());
                    }
                }
                _ => {}
            }
        }
        p.compare_ram(&format!("{seed_ctx}: end"));
        (frames, nonzero)
    }

    #[test]
    fn random_voices_reverb_noise_cd() {
        let _g = guard();
        let mut rng = Rng::new(0x5B0_0001);
        let n = cases(200);
        let (mut frames, mut nonzero) = (0, 0);
        for case in 0..n {
            let (f, nz) = run_case(&mut rng, &format!("case {case}"), case % 4 == 3);
            frames += f;
            nonzero += nz;
        }
        println!(
            "spu: {n} cases, {frames} frames ({nonzero} nonzero samples) compared sample-exact"
        );
        assert!(nonzero > frames, "the cases should make sound");
    }
}

/// Shows up as ignored (not passed) in a checkout without `ref/DW2-PC`.
#[cfg(not(have_dw2_c))]
#[test]
#[ignore = "ref/DW2-PC is absent: the spu differential tests did not run"]
fn skipped_without_dw2_pc() {}
