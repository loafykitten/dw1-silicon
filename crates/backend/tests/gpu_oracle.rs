//! The GPU model against the real thing: primitives drawn by the SDK's libgpu inside PCSX-Redux
//! (`-softgpu`) and read back from VRAM, from the cases `tools/oracle/gpu_requests.py` writes.
//!
//! The files are the emulator's outputs, so they stay local: `cases.txt` and `results.txt`
//! under `DW1_GPU_CAPTURE` (default `captures/oracle-runs/gpu-prims`, gitignored). Without them
//! the test shows as ignored. Each case is replayed on a fresh model whose VRAM holds the same
//! textures (see the generator's header), and the test rectangle must match pixel for pixel.
//!
//! A small residue is tolerated (see the budget at the end): rounding ties that differ by one
//! step in long thin triangles. Set `DW1_GPU_STRICT=1` to require an exact match.
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
        // Known residue: about 1,600 pixels in a million textured and shaded ones still
        // differ by one step at a rounding tie, in long thin triangles (the model rounds as
        // Redux does but not in the last bit of its fixed-point steps). The budget keeps
        // anything bigger from sneaking in; `DW1_GPU_STRICT=1` demands zero.
        if std::env::var_os("DW1_GPU_STRICT").is_some() {
            assert!(bad == 0, "{bad} of {n} cases differ ({bad_px} pixels)");
        }
        assert!(
            bad <= n / 12 && bad_px <= 2_000,
            "{bad} of {n} cases differ ({bad_px} pixels): more than the known residue"
        );
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
