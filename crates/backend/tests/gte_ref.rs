#![allow(clippy::needless_range_loop)]
//! Checks the Rust GTE model against tests/data/gte_ref.bin (PCSX-Redux outputs from DW2-PC,
//! CC0). The file holds outputs only; the inputs are regenerated with the same xorshift
//! generator and case layout as DW2-PC's tests/gte_test.c.

use backend::gte::{ExecStatus, Gte, cmd, creg, dreg};

const CASES: usize = 96;
const CASE_WORDS: usize = 64;
const OUT_WORDS: usize = 33;

const OPS: [u32; 13] = [
    cmd::RTPS,
    0x4A10_0001 | (1 << 10),
    cmd::NCLIP,
    cmd::mvmva(1, 0, 3, 3, 0),
    cmd::mvmva(1, 0, 0, 0, 0),
    cmd::mvmva(0, 0, 3, 3, 0),
    cmd::mvmva(1, 0, 0, 3, 0),
    cmd::mvmva(1, 1, 1, 1, 1),
    cmd::mvmva(0, 2, 2, 0, 0),
    cmd::NCS,
    0x4AC0_001E,
    cmd::gpf(1),
    0x4B90_003D | (1 << 10),
];

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo.wrapping_add((self.next() % (hi - lo + 1) as u32) as i32)
    }
    /// pair(range(a), range(b)); the C evaluates its two arguments right to left.
    fn pair_range(&mut self, lo: i32, hi: i32) -> u32 {
        let b = self.range(lo, hi);
        let a = self.range(lo, hi);
        pair(a, b)
    }
}

fn pair(a: i32, b: i32) -> u32 {
    ((b as u32) << 16) | (a as u32 & 0xFFFF)
}

fn is_mat(i: usize) -> bool {
    i < 5 || (8..13).contains(&i) || (16..21).contains(&i)
}
fn is_vec(i: usize) -> bool {
    (5..8).contains(&i) || (13..16).contains(&i) || (21..24).contains(&i)
}

fn make_case(r: &mut Rng, op_index: usize, wide: u32) -> [u32; CASE_WORDS] {
    let mut w = [0u32; CASE_WORDS];
    let m = if wide != 0 { 0x7FFF } else { 0x1000 };
    for (i, c) in w.iter_mut().take(31).enumerate() {
        *c = if is_mat(i) {
            r.pair_range(-m, m)
        } else if is_vec(i) {
            if wide != 0 {
                r.next()
            } else {
                r.range(-0x8000, 0x8000) as u32
            }
        } else if i == creg::OFX || i == creg::OFY {
            if wide != 0 {
                r.next()
            } else {
                (r.range(-0x200, 0x200) << 16) as u32
            }
        } else if i == creg::H {
            if wide != 0 {
                r.next() & 0xFFFF
            } else {
                r.range(0x100, 0x400) as u32
            }
        } else if i == creg::DQA {
            if wide != 0 {
                r.next() as i16 as i32 as u32
            } else {
                (-0x1062i32) as u32
            }
        } else if i == creg::DQB {
            if wide != 0 { r.next() } else { 0x140_0000 }
        } else {
            r.next() as i16 as i32 as u32
        };
    }
    for i in 0..31 {
        let v = if wide != 0 { 0x7FFF } else { 0x400 };
        if i == dreg::SXYP || i == dreg::IRGB || i == dreg::ORGB {
            continue;
        }
        w[31 + i] = if i == dreg::VXY0 || i == dreg::VXY1 || i == dreg::VXY2 {
            r.pair_range(-v, v)
        } else if i == dreg::VZ0 || i == dreg::VZ1 || i == dreg::VZ2 {
            if wide != 0 {
                r.next()
            } else {
                r.range(-v, v) as u32
            }
        } else if (dreg::IR0..=dreg::IR3).contains(&i) {
            if wide != 0 {
                r.next()
            } else {
                r.range(-0x1000, 0x1000) as u32
            }
        } else if (dreg::SXY0..=dreg::SXY2).contains(&i) {
            r.pair_range(-0x400, 0x3FF)
        } else {
            r.next()
        };
    }
    if wide == 2 {
        for i in 0..31 {
            let big = (0x7FF0_0000 | (r.next() & 0xFFFFF)) as i32;
            if is_mat(i) {
                w[i] = if r.next() & 1 != 0 {
                    let b = 0x7FFF - (r.next() & 0xFF) as i32;
                    let a = 0x7FFF - (r.next() & 0xFF) as i32;
                    pair(a, b)
                } else {
                    let b = -0x8000 + (r.next() & 0xFF) as i32;
                    let a = -0x8000 + (r.next() & 0xFF) as i32;
                    pair(a, b)
                };
            } else if is_vec(i) {
                w[i] = (if r.next() & 1 != 0 { big } else { -big }) as u32;
            }
        }
        for i in dreg::VXY0..=dreg::VZ2 {
            w[31 + i] = if r.next() & 1 != 0 {
                0x7FFF_7FFF
            } else {
                0x8000_8000
            };
        }
        for i in dreg::IR1..=dreg::IR3 {
            w[31 + i] = if r.next() & 1 != 0 {
                0x7FFF
            } else {
                0xFFFF_8000
            };
        }
    }
    w[CASE_WORDS - 2] = OPS[op_index];
    w[CASE_WORDS - 1] = op_index as u32;
    w
}

fn run_case(w: &[u32; CASE_WORDS]) -> ([u32; OUT_WORDS], ExecStatus) {
    let mut g = Gte::new();
    for i in 0..31 {
        g.ctc2(i, w[i]);
    }
    for i in 0..31 {
        if i == dreg::SXYP || i == dreg::IRGB || i == dreg::ORGB {
            continue;
        }
        g.mtc2(i, w[31 + i]);
    }
    let status = g.execute(OPS[w[CASE_WORDS - 1] as usize]);
    let mut out = [0u32; OUT_WORDS];
    for (i, o) in out.iter_mut().take(32).enumerate() {
        *o = g.mfc2(i);
    }
    out[32] = g.cfc2(creg::FLAG);
    (out, status)
}

#[test]
fn matches_pcsx_redux_reference() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/gte_ref.bin");
    let bytes = std::fs::read(path).expect("gte_ref.bin");
    let n = OPS.len() * CASES;
    assert_eq!(bytes.len(), n * OUT_WORDS * 4, "reference size");
    let ref_words: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();

    // Cases are generated in order with one shared generator.
    let mut rng = Rng(0x2545_F491);
    let mut bad = 0;
    let mut per_op = [0usize; 13];
    for k in 0..n {
        let wide = if (k % CASES) * 4 / CASES >= 3 {
            2
        } else {
            u32::from(k % CASES >= CASES / 2)
        };
        let w = make_case(&mut rng, k / CASES, wide);
        let (out, status) = run_case(&w);
        assert_ne!(status, ExecStatus::Unimplemented(OPS[k / CASES]));
        let r = &ref_words[k * OUT_WORDS..(k + 1) * OUT_WORDS];
        if out[..] != *r {
            bad += 1;
            per_op[k / CASES] += 1;
            if bad <= 12 {
                let diffs: Vec<String> = (0..OUT_WORDS)
                    .filter(|&i| out[i] != r[i])
                    .map(|i| {
                        format!(
                            "{}{} {:08X}/{:08X}",
                            if i == 32 { "FLAG" } else { "d" },
                            if i == 32 { 0 } else { i },
                            out[i],
                            r[i]
                        )
                    })
                    .collect();
                eprintln!(
                    "case {k} op {:08X}: {} (model/PS1)",
                    OPS[k / CASES],
                    diffs.join(" ")
                );
            }
        }
    }
    for (i, b) in per_op.iter().enumerate() {
        eprintln!("op {:08X}: {}/{} cases match", OPS[i], CASES - b, CASES);
    }
    eprintln!("{}/{} cases match the PS1 GTE reference", n - bad, n);
    assert_eq!(bad, 0, "{bad} reference cases differ");
}
