//! backend::xa against DW2-PC's xadec.c: random sectors of every coding-info value, compared
//! frame by frame.

#[cfg(have_dw2_c)]
mod diff {
    use backend::xa::{SECTOR_BODY_SIZE, XaDecoder};
    use decoder_diff::*;

    /// One random sector body. `gentle` keeps shifts at 8..=12 so the output rarely clips.
    fn sector(rng: &mut Rng, ci: u8, gentle: bool) -> Vec<u8> {
        let mut b = vec![0u8; SECTOR_BODY_SIZE];
        rng.fill(&mut b);
        b[3] = ci;
        b[7] = ci;
        if gentle {
            for g in 0..18 {
                for u in 0..8 {
                    let filter = rng.below(4) as u8;
                    b[8 + g * 128 + 4 + u] = (rng.range(8, 12) as u8) | filter << 4;
                }
            }
        }
        b
    }

    fn compare_state(xa: &mut XaDecoder, ctx: &str) {
        assert_eq!(xa.queued() as i32, xa_queued(), "{ctx}: queued");
        assert_eq!(xa.underruns(), xa_underruns(), "{ctx}: underruns");
        assert_eq!(xa.last_raw(), xa_last_raw().as_slice(), "{ctx}: raw");
    }

    fn pop_both(xa: &mut XaDecoder, n: usize, ctx: &str) {
        for i in 0..n {
            assert_eq!(xa.pop(), xa_pop(), "{ctx}: pop {i}");
        }
    }

    #[test]
    fn random_sectors_every_coding_info() {
        let _g = guard();
        let mut rng = Rng::new(0x58A_0001);
        let per_ci = cases(40);
        let mut sectors = 0usize;
        let mut frames = 0usize;
        // Every coding-info byte that varies the format: stereo/mono (and reserved), rate,
        // bits per sample (and reserved), emphasis.
        let cis: Vec<u8> = (0..=255u8)
            .filter(|c| c & 0x80 == 0 && (c & 0x0F) < 8)
            .collect();
        for &ci in &cis {
            let mut xa = XaDecoder::new();
            xa_reset();
            xa.reset();
            let speed = [1, 1, 1, 2, 3, 0, 5][rng.below(7) as usize];
            xa_set_speed(speed);
            xa.set_speed(speed);
            for n in 0..per_ci {
                let body = sector(&mut rng, ci, n % 2 == 0);
                xa_sector(&body);
                xa.decode_sector(&body);
                sectors += 1;
                compare_state(&mut xa, &format!("ci {ci:#04x} sector {n}"));
                frames += xa.queued();
                // Drain a random amount, sometimes past the end (underruns), sometimes nothing
                // so the FIFO fills and drops.
                let take = match rng.below(4) {
                    0 => 0,
                    1 => xa.queued() + 3,
                    _ => rng.below(xa.queued() as u32 + 1) as usize,
                };
                pop_both(&mut xa, take, &format!("ci {ci:#04x} sector {n}"));
                if rng.one_in(9) {
                    xa_stop();
                    xa.stop();
                }
                if rng.one_in(13) {
                    let s = [1, 2, 0, 4][rng.below(4) as usize];
                    xa_set_speed(s);
                    xa.set_speed(s);
                }
                compare_state(&mut xa, &format!("ci {ci:#04x} after pop {n}"));
            }
            let q = xa.queued();
            pop_both(&mut xa, q + 2, &format!("ci {ci:#04x} drain"));
        }
        println!(
            "xa: {} coding-info values, {sectors} sectors, {frames} queued frames compared",
            cis.len()
        );
    }

    #[test]
    fn fifo_overflow_drops_oldest() {
        let _g = guard();
        let mut rng = Rng::new(0x58A_0002);
        let mut xa = XaDecoder::new();
        xa_reset();
        xa.reset();
        xa_set_speed(1);
        xa.set_speed(1);
        // 37.8 kHz mono 4-bit: 4032 raw frames, 4704 out per sector; five sectors overflow 16384.
        for n in 0..6 {
            let body = sector(&mut rng, 0x00, true);
            xa_sector(&body);
            xa.decode_sector(&body);
            compare_state(&mut xa, &format!("overflow sector {n}"));
        }
        assert_eq!(xa.queued(), 16384);
        let q = xa.queued();
        pop_both(&mut xa, q, "overflow drain");
    }

    #[test]
    fn short_body_is_zero_padded() {
        let _g = guard();
        let mut rng = Rng::new(0x58A_0003);
        let mut xa = XaDecoder::new();
        xa_reset();
        xa.reset();
        xa_set_speed(1);
        let mut body = sector(&mut rng, 0x01, true);
        body.truncate(1000);
        let mut padded = body.clone();
        padded.resize(SECTOR_BODY_SIZE, 0);
        xa_sector(&padded);
        xa.decode_sector(&body);
        compare_state(&mut xa, "short body");
    }
}

/// Shows up as ignored (not passed) in a checkout without `ref/DW2-PC`.
#[cfg(not(have_dw2_c))]
#[test]
#[ignore = "ref/DW2-PC is absent: the xa differential tests did not run"]
fn skipped_without_dw2_pc() {}
