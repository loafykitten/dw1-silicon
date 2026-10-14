//! The GPU model against the real thing: primitives drawn by the SDK's libgpu inside PCSX-Redux
//! (`-softgpu`) and read back from VRAM, from the cases `tools/oracle/gpu_requests.py` writes.
//!
//! The files are the emulator's outputs, so they stay local: `cases.txt` and `results.txt`
//! under `DW1_GPU_CAPTURE` (default `captures/oracle-runs/gpu-prims`, gitignored). Without them
//! the test shows as ignored. Each case is replayed on a fresh model whose VRAM holds the same
//! textures (see the generator's header), and the test rectangle must match pixel for pixel.
//!
//! A small residue is tolerated, case by case (`gpu_oracle_known.txt`): line rounding ties and
//! one blend-mode case (see the list's check below). Set `DW1_GPU_STRICT=1` to require an exact
//! match. Opt-in sets (`--only tiny`, `--only spans`: thin triangles that tell the gradient rule
//! apart) run with `DW1_GPU_CAPTURE` pointing at their directory and `DW1_GPU_EXACT=1`: no
//! known list, every pixel must match.
//!
//! Debugging a failure: `DW1_GPU_SHOW=N` prints the first N mismatching cases with their words
//! and the differing pixels.

#[cfg(not(have_gpu_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/gpu-prims (or DW1_GPU_CAPTURE); local only"]
fn gpu_matches_the_emulator() {}

#[cfg(have_gpu_capture)]
mod oracle {
    use backend::gpu::{Gpu, VRAM_W};
    use std::path::PathBuf;

    fn dir() -> PathBuf {
        match std::env::var_os("DW1_GPU_CAPTURE") {
            Some(d) => PathBuf::from(d),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../captures/oracle-runs/gpu-prims"),
        }
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    /// A model with the generator's textures: A (640,0) texel = u, B (768,0) texel = v, both 8-bit
    /// 128 x 256 halfwords; CLUT at (640,300) entry i = (i & 31) | (i >> 5) << 5 | 1 << 10, and one at (640,301) with bit 15 added to odd entries.
    fn textured_gpu() -> Gpu {
        let mut g = Gpu::new();
        let vram = g.vram_mut();
        for y in 0..256 {
            for k in 0..128 {
                let a = (2 * k) | ((2 * k + 1) << 8);
                vram[y * VRAM_W + 640 + k] = a as u16;
                vram[y * VRAM_W + 768 + k] = (y | y << 8) as u16;
            }
        }
        for i in 0..256usize {
            let c = (i & 31) | ((i >> 5) << 5) | (1 << 10);
            vram[300 * VRAM_W + 640 + i] = c as u16;
            // a second CLUT at (640,301): bit 15 (semi-transparent texel) on odd entries
            vram[301 * VRAM_W + 640 + i] = (c | if i & 1 == 1 { 0x8000 } else { 0 }) as u16;
            // a third CLUT at (640,302): colours spread over all channels, bit 15 set
            vram[302 * VRAM_W + 640 + i] =
                ((i & 31) | (((i * 7) & 31) << 5) | (((i * 3 + 5) & 31) << 10) | 0x8000) as u16;
        }
        g
    }

    #[test]
    fn gpu_matches_the_emulator() {
        let dir = dir();
        let cases = std::fs::read_to_string(dir.join("cases.txt")).unwrap();
        let results = std::fs::read_to_string(dir.join("results.txt")).unwrap();
        // The readback of each case: the lines that carry the StoreImage destination.
        let outs: Vec<Vec<u8>> = results
            .lines()
            .filter_map(|l| l.split("801e8010=").nth(1))
            .map(|s| unhex(s.split_whitespace().next().unwrap_or("")))
            .collect();
        let show: usize = std::env::var("DW1_GPU_SHOW")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let mut bad = 0;
        let mut bad_px = 0;
        let mut n = 0;
        let mut per_case: Vec<(usize, usize)> = Vec::new();
        for (i, line) in cases.lines().enumerate() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let num = |k: usize| f[k].parse::<usize>().unwrap();
            let (rx, ry, rw, rh) = (num(1), num(2), num(3), num(4));
            let words: Vec<u32> = f[5..]
                .iter()
                .map(|w| u32::from_str_radix(w, 16).unwrap())
                .collect();
            let out = &outs[i];
            assert_eq!(out.len(), rw * rh * 2, "case {i}: readback size");
            let mut g = textured_gpu();
            g.gp0(&words);
            let mut diffs = Vec::new();
            for y in 0..rh {
                for x in 0..rw {
                    let want =
                        u16::from_le_bytes([out[(y * rw + x) * 2], out[(y * rw + x) * 2 + 1]]);
                    let got = g.vram()[(ry + y) * VRAM_W + rx + x];
                    if got != want {
                        diffs.push((x, y, got, want));
                    }
                }
            }
            n += 1;
            if !diffs.is_empty() {
                bad += 1;
                bad_px += diffs.len();
                per_case.push((i, diffs.len()));
                if bad <= show {
                    eprintln!(
                        "case {i}: {} pixels differ; words {:x?}",
                        diffs.len(),
                        words
                    );
                    for d in diffs.iter().take(12) {
                        eprintln!("  x={} y={} got {:04x} want {:04x}", d.0, d.1, d.2, d.3);
                    }
                }
            }
        }
        eprintln!("{bad} of {n} cases differ ({bad_px} pixels)");
        if std::env::var_os("DW1_GPU_EXACT").is_some() {
            // An opt-in set: no known list, every pixel must match, and the cases must draw.
            let drawn = outs
                .iter()
                .filter(|o| !o.chunks(2).all(|c| c == [0xFF, 0x7F]))
                .count();
            assert!(
                n > 0 && drawn * 10 >= n * 8,
                "only {drawn} of {n} cases draw anything"
            );
            assert_eq!(bad, 0, "{bad} of {n} cases differ ({bad_px} pixels)");
            return;
        }
        // Lines and rectangles must actually draw (an earlier version of the cases drew
        // nothing, and passed): nearly all of them leave a pixel other than the fill.
        let (mut prims, mut drawn) = (0, 0);
        for (i, line) in cases.lines().enumerate() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let prim_op = u32::from_str_radix(f[5 + 8], 16).unwrap() >> 24;
            if (0x40..0x80).contains(&prim_op) {
                prims += 1;
                let blank = outs[i].chunks(2).all(|c| c == [0xFF, 0x7F]);
                drawn += usize::from(!blank);
            }
        }
        assert!(
            prims >= 50 && drawn * 10 >= prims * 8,
            "only {drawn} of {prims} line and rectangle cases draw anything"
        );
        // Known residue (`gpu_oracle_known.txt`): cases that still differ, with their exact
        // pixel counts: lines whose minor coordinate lands on a tie (cases 558-615), and one
        // semi-transparent Gouraud textured quad in blend mode B + F/4 (5481, green one step
        // off). Every other polygon is exact. Any
        // case not listed, or a listed one with more pixels, fails; fewer pixels is reported
        // so the list gets tightened. `DW1_GPU_UPDATE=1` prints the list for the current run.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for b in cases.bytes() {
            hash = (hash ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
        let header = format!("cases {n} {hash:016x}");
        if std::env::var_os("DW1_GPU_UPDATE").is_some() {
            println!("{header}");
            for (i, px) in &per_case {
                println!("{i} {px}");
            }
            return;
        }
        let known_text = include_str!("gpu_oracle_known.txt");
        let mut lines = known_text.lines();
        assert_eq!(
            lines.next(),
            Some(header.as_str()),
            "the captures are not from the current tools/oracle/gpu_requests.py (or the \
             known list is stale): rerun it, then `DW1_GPU_UPDATE=1` for a new list"
        );
        let known: std::collections::BTreeMap<usize, usize> = lines
            .map(|l| {
                let mut it = l.split_whitespace().map(|x| x.parse::<usize>().unwrap());
                (it.next().unwrap(), it.next().unwrap())
            })
            .collect();
        let strict = std::env::var_os("DW1_GPU_STRICT").is_some();
        let mut failures = Vec::new();
        for (i, px) in &per_case {
            match known.get(i) {
                None => failures.push(format!("case {i}: {px} pixels differ (new)")),
                Some(k) if px > k || strict => {
                    failures.push(format!("case {i}: {px} pixels differ (known: {k})"))
                }
                Some(k) if px < k => eprintln!("case {i}: now {px} pixels, list says {k}"),
                _ => {}
            }
        }
        for i in known.keys() {
            if !per_case.iter().any(|(c, _)| c == i) {
                eprintln!("case {i} is exact now; drop it from the list");
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The game's fade quad (`tools/oracle/gpu_snap_requests.py`) redrawn in Redux over the game's
    /// own picture and read back; the model gets the same picture and must produce the same pixels.
    #[test]
    fn gpu_matches_the_emulator_on_the_game_picture() {
        let dir = dir();
        let (Ok(cases), Ok(results)) = (
            std::fs::read_to_string(dir.join("snap_cases.txt")),
            std::fs::read_to_string(dir.join("snap_results.txt")),
        ) else {
            eprintln!("no snapshot run; skipped");
            return;
        };
        let grab = |key: &str| -> Vec<Vec<u8>> {
            results
                .lines()
                .filter_map(|l| l.split(key).nth(1))
                .map(|s| unhex(s.split_whitespace().next().unwrap_or("")))
                .collect()
        };
        let words16 = |b: &[u8]| -> Vec<u16> {
            b.chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect()
        };
        let tex = words16(&grab("801ed000=")[0]);
        let clut = words16(&grab("801ed100=")[0]);
        let pre = grab("801e1000=");
        let post = grab("801e5000=");
        let mut bad = 0;
        for (k, line) in cases.lines().enumerate() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (wx, wy) = (
                f[0].parse::<usize>().unwrap(),
                f[1].parse::<usize>().unwrap(),
            );
            let words: Vec<u32> = f[2..]
                .iter()
                .map(|w| u32::from_str_radix(w, 16).unwrap())
                .collect();
            let mut g = Gpu::new();
            let v = g.vram_mut();
            for (i, p) in words16(&pre[k]).iter().enumerate() {
                v[(wy + i / 64) * VRAM_W + wx + i % 64] = *p;
            }
            for (i, p) in tex.iter().enumerate() {
                v[(509 + i / 8) * VRAM_W + 952 + i % 8] = *p;
            }
            for (i, p) in clut.iter().enumerate() {
                v[502 * VRAM_W + 256 + i] = *p;
            }
            g.gp0(&words);
            let want = words16(&post[k]);
            let n = want
                .iter()
                .enumerate()
                .filter(|(i, w)| g.vram()[(wy + i / 64) * VRAM_W + wx + i % 64] != **w)
                .count();
            eprintln!("game-picture draw {k}: {n} pixels differ");
            if std::env::var_os("DW1_GPU_SHOW").is_some() {
                let pre16 = words16(&pre[k]);
                for (i, w) in want.iter().enumerate() {
                    let got = g.vram()[(wy + i / 64) * VRAM_W + wx + i % 64];
                    if got != *w {
                        eprintln!(
                            "  ({}, {}) pre {:04x} got {:04x} want {:04x}",
                            wx + i % 64,
                            wy + i / 64,
                            pre16[i],
                            got,
                            w
                        );
                    }
                }
            }
            bad += n;
        }
        assert_eq!(bad, 0, "the model differs on the game's own fade");
    }
}
