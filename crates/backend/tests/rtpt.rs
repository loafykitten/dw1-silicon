//! RTPT must equal RTPS on V0, V1, V2 in turn, except that the depth-cue step (MAC0/IR0 from
//! DQA/DQB) runs for V2 only, so V0/V1 never contribute its flags. PCSX-Redux models it this way
//! (`rtps(last)` in its GTE); psx-spx only says RTPT "repeats RTPS".

use backend::gte::{ExecStatus, Gte, cmd, creg, dreg};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn random_state(r: &mut Rng, mode: u32) -> Gte {
    let mut g = Gte::new();
    for i in 0..31 {
        // mode 0: game-like ranges; mode 1: full range registers (overflow and saturation).
        let v = r.next();
        let v = if mode == 0 {
            match i {
                creg::OFX | creg::OFY => (((v % 0x400) as i32 - 0x200) << 16) as u32,
                creg::H => 0x100 + v % 0x300,
                creg::DQA => (-0x1062i32) as u32,
                creg::DQB => 0x140_0000,
                5..=7 => (v % 0x2000).wrapping_sub(0x1000),
                _ => v & 0x0FFF_0FFF,
            }
        } else {
            v
        };
        g.ctc2(i, v);
    }
    for i in 0..31 {
        if matches!(i, dreg::SXYP | dreg::IRGB | dreg::ORGB) {
            continue;
        }
        let v = r.next();
        let v = if mode == 0 && matches!(i, dreg::VXY0..=dreg::VZ2) {
            v & 0x03FF_03FF
        } else {
            v
        };
        g.mtc2(i, v);
    }
    g
}

#[test]
fn rtpt_equals_three_rtps() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    for case in 0..600 {
        let g0 = random_state(&mut r, case % 2);
        let sf_lm = r.next() & 3;
        let (sf, lm) = (sf_lm & 1, sf_lm >> 1);
        let rtpt = cmd::RTPT & !(1 << 19) | (sf << 19) | (lm << 10);
        let rtps = cmd::RTPS & !(1 << 19) | (sf << 19) | (lm << 10);

        let mut a = g0.clone();
        assert_eq!(a.execute(rtpt), ExecStatus::Done);

        let mut b = g0.clone();
        let mut flag = 0;
        let (dqa, dqb) = (g0.cfc2(creg::DQA), g0.cfc2(creg::DQB));
        for v in 0..3 {
            // RTPS reads V0: move vertex v there.
            let (xy, z) = (g0.mfc2(dreg::VXY0 + 2 * v), g0.mfc2(dreg::VZ0 + 2 * v));
            b.mtc2(dreg::VXY0, xy);
            b.mtc2(dreg::VZ0, z);
            // V0/V1: a depth cue that can't raise flags (DQA 0, IR0 = DQB >> 12 = 0x100), so the
            // FLAG only carries the transform and projection checks. V2: the real one.
            if v < 2 {
                b.ctc2(creg::DQA, 0);
                b.ctc2(creg::DQB, 0x10_0000);
            } else {
                b.ctc2(creg::DQA, dqa);
                b.ctc2(creg::DQB, dqb);
            }
            assert_eq!(b.execute(rtps), ExecStatus::Done);
            flag |= b.cfc2(creg::FLAG);
        }
        for reg in [
            dreg::SXY0,
            dreg::SXY1,
            dreg::SXY2,
            dreg::SZ0,
            dreg::SZ1,
            dreg::SZ2,
            dreg::SZ3,
            dreg::MAC0,
            dreg::MAC1,
            dreg::MAC2,
            dreg::MAC3,
            dreg::IR0,
            dreg::IR1,
            dreg::IR2,
            dreg::IR3,
        ] {
            assert_eq!(a.mfc2(reg), b.mfc2(reg), "case {case} reg {reg}");
        }
        assert_eq!(a.cfc2(creg::FLAG), flag, "case {case} FLAG");
    }
}

/// A case where V0/V1's depth cue would saturate IR0 and overflow MAC0 if it ran (found in
/// review): RTPT must leave FLAG clear.
#[test]
fn rtpt_skips_depth_cue_for_v0_and_v1() {
    let mut g = Gte::new();
    // Identity rotation (RT11, RT22, RT33 = 1.0), zero translation and screen offset.
    g.ctc2(0, 0x1000);
    g.ctc2(2, 0x1000);
    g.ctc2(4, 0x1000);
    g.ctc2(creg::H, 256);
    g.ctc2(creg::DQA, 32767);
    g.ctc2(creg::DQB, (-536_854_528i32) as u32);
    for (v, z) in [192u32, 512, 1024].into_iter().enumerate() {
        g.mtc2(dreg::VXY0 + 2 * v, 0);
        g.mtc2(dreg::VZ0 + 2 * v, z);
    }
    let rtpt = cmd::RTPT | (1 << 19); // sf = 1, lm = 0
    assert_eq!(g.execute(rtpt), ExecStatus::Done);
    assert_eq!(g.cfc2(creg::FLAG), 0);
}
