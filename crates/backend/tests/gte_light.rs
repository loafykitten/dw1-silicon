//! NCT, NCCS, NCCT, AVSZ3 and AVSZ4 against values worked out by hand from the formulas in
//! psx-spx ("GTE Color Calculation Commands", "GTE Coordinate Calculation Commands").

use backend::gte::{ExecStatus, Gte, cmd, creg, dreg};

/// Light matrix and colour matrix = identity (1.0 = 0x1000), background colour `bk`, RGBC `rgbc`.
fn lit(bk: u32, rgbc: u32) -> Gte {
    let mut g = Gte::new();
    for base in [creg::LLM0, creg::LCM0] {
        // Elements 11, 22 and 33 sit in the low halves of words 0, 2 and 4.
        for w in [0, 2, 4] {
            g.ctc2(base + w, 0x1000);
        }
    }
    for r in [creg::RBK, creg::GBK, creg::BBK] {
        g.ctc2(r, bk);
    }
    g.mtc2(dreg::RGBC, rgbc);
    g
}

fn set_v(g: &mut Gte, n: usize, x: i16, y: i16, z: i16) {
    g.mtc2(
        dreg::VXY0 + 2 * n,
        u32::from(x as u16) | u32::from(y as u16) << 16,
    );
    g.mtc2(dreg::VZ0 + 2 * n, z as u16 as u32);
}

#[test]
fn nct_lights_three_normals() {
    // BK = 0x400 (0.25). Normal (0.5, 0, 0): LLM*N = IR (0x800, 0, 0); + LCM*IR + BK gives
    // (0xC00, 0x400, 0x400); /16 = (0xC0, 0x40, 0x40). Normal (0, 1.0, 0): IR2 = 0x1000 gives
    // G = 0x1400/16 = 0x140, saturated to 0xFF (flag bit 20). Normal (0, 0, -1.0): IR3 saturates
    // to 0 with lm=1 (flag bit 22), so all channels = BK/16 = 0x40. The code byte comes from RGBC.
    let mut g = lit(0x400, 0x3C80_8080);
    set_v(&mut g, 0, 0x800, 0, 0);
    set_v(&mut g, 1, 0, 0x1000, 0);
    set_v(&mut g, 2, 0, 0, -0x1000);
    assert_eq!(g.execute(cmd::NCT), ExecStatus::Done);
    assert_eq!(g.mfc2(dreg::RGB0), 0x3C40_40C0);
    assert_eq!(g.mfc2(dreg::RGB1), 0x3C40_FF40);
    assert_eq!(g.mfc2(dreg::RGB2), 0x3C40_4040);
    assert_eq!(g.cfc2(creg::FLAG), 0x0050_0000);
}

#[test]
fn nccs_multiplies_by_rgbc() {
    // Same lighting as above for normal (0.5, 0, 0): IR = (0xC00, 0x400, 0x400). RGBC colour
    // 0x80 per channel: MAC = 0x80 * IR * 16 / 4096 = (0x600, 0x200, 0x200); /16 = (0x60, 0x20, 0x20).
    let mut g = lit(0x400, 0x3480_8080);
    set_v(&mut g, 0, 0x800, 0, 0);
    assert_eq!(g.execute(cmd::NCCS), ExecStatus::Done);
    assert_eq!(g.mfc2(dreg::RGB2), 0x3420_2060);
    assert_eq!(g.mfc2(dreg::IR1), 0x600);
    assert_eq!(g.cfc2(creg::FLAG), 0);
    // Without BK: (0x800, 0, 0) -> IR1 = 0x800 -> 0x80 * 0x800 * 16 / 4096 = 0x400 -> 0x40.
    let mut g = lit(0, 0x3480_8080);
    set_v(&mut g, 0, 0x800, 0, 0);
    g.execute(cmd::NCCS);
    assert_eq!(g.mfc2(dreg::RGB2), 0x3400_0040);
}

#[test]
fn ncct_runs_nccs_on_each_vector_and_shifts_the_fifo() {
    let mut g = lit(0, 0x3C80_8080);
    set_v(&mut g, 0, 0x800, 0, 0);
    set_v(&mut g, 1, 0, 0x400, 0);
    set_v(&mut g, 2, 0, 0, 0x1000);
    assert_eq!(g.execute(cmd::NCCT), ExecStatus::Done);
    // 0x80 * 0x800 * 16 >> 12 >> 4 = 0x40; 0x80 * 0x400 ... = 0x20; 0x80 * 0x1000 ... = 0x80.
    assert_eq!(g.mfc2(dreg::RGB0), 0x3C00_0040);
    assert_eq!(g.mfc2(dreg::RGB1), 0x3C00_2000);
    assert_eq!(g.mfc2(dreg::RGB2), 0x3C80_0000);
}

#[test]
fn avsz3_and_avsz4() {
    // ZSF3 = 341 (0x155): MAC0 = 341 * (0x31B5 + 0x31AB + 0x31BF) = 341 * 38175 = 13,017,675;
    // OTZ = MAC0 >> 12 = 3178.
    let mut g = Gte::new();
    g.ctc2(creg::ZSF3, 0x155);
    for (r, v) in [
        (dreg::SZ0, 0x1111),
        (dreg::SZ1, 0x31B5),
        (dreg::SZ2, 0x31AB),
        (dreg::SZ3, 0x31BF),
    ] {
        g.mtc2(r, v);
    }
    assert_eq!(g.execute(cmd::AVSZ3), ExecStatus::Done);
    assert_eq!(g.mfc2(dreg::MAC0), 13_017_675);
    assert_eq!(g.mfc2(dreg::OTZ), 3178);
    assert_eq!(g.cfc2(creg::FLAG), 0);

    // ZSF4 = 256: MAC0 = 256 * (0x31BC + 0x31A8 + 0x31A9 + 0x3193) = 256 * 50848 = 13,017,088;
    // OTZ = 3178 (exactly 13,017,088 / 4096 = 3178).
    g.ctc2(creg::ZSF4, 0x100);
    for (r, v) in [
        (dreg::SZ0, 0x31BC),
        (dreg::SZ1, 0x31A8),
        (dreg::SZ2, 0x31A9),
        (dreg::SZ3, 0x3193),
    ] {
        g.mtc2(r, v);
    }
    assert_eq!(g.execute(cmd::AVSZ4), ExecStatus::Done);
    assert_eq!(g.mfc2(dreg::MAC0), 13_017_088);
    assert_eq!(g.mfc2(dreg::OTZ), 3178);
}

#[test]
fn avsz_saturates_and_flags() {
    // 0x7FFF * 3 * 0xFFFF = 6.4e9 overflows MAC0's 31 bits (FLAG bit 16) and is far above
    // 0xFFFF after >> 12: OTZ = 0xFFFF, FLAG bit 18 (and so bit 31).
    let mut g = Gte::new();
    g.ctc2(creg::ZSF3, 0x7FFF);
    for r in [dreg::SZ1, dreg::SZ2, dreg::SZ3] {
        g.mtc2(r, 0xFFFF);
    }
    g.execute(cmd::AVSZ3);
    assert_eq!(g.mfc2(dreg::OTZ), 0xFFFF);
    assert_eq!(g.cfc2(creg::FLAG), 0x8005_0000);
    // A negative factor makes MAC0 negative (-256 * 196605, no overflow): OTZ = 0, FLAG bit 18.
    g.ctc2(creg::ZSF3, 0xFFFF_FF00);
    g.execute(cmd::AVSZ3);
    assert_eq!(g.mfc2(dreg::OTZ), 0);
    assert_eq!(g.cfc2(creg::FLAG), 0x8004_0000);
}
