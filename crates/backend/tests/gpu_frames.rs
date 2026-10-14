//! The GPU model against Redux on the polygons of whole frames of the game (3D models: small lit
//! Gouraud textured triangles and quads, semi-transparent objects, depth-sorted overlaps).
//!
//! `tools/oracle/gpu_frame_requests.py` turns a native run's polygon list and VRAM into emulator
//! requests; for each frame a directory under `captures/oracle-runs/gpu-frames/` (or
//! `DW1_GPU_FRAMES`, local only) holds `frame.txt` (the polygons with their draw state), `vram.bin`
//! (the game's VRAM, for the textures and CLUTs) and `results.txt` (the emulator's readback of the
//! cleared frame buffer after the polygons, in 6 bands). The model starts from the same VRAM with
//! the frame buffer cleared, draws the same words, and every pixel must match.
//!
//! Without the directory the test passes without doing anything.

use backend::gpu::{Gpu, VRAM_W};
use std::path::PathBuf;

#[test]
fn model_matches_redux_on_game_frames() {
    let root = match std::env::var_os("DW1_GPU_FRAMES") {
        Some(d) => PathBuf::from(d),
        None => {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs/gpu-frames")
        }
    };
    let Ok(rd) = std::fs::read_dir(&root) else {
        eprintln!("no frames in {}; skipped", root.display());
        return;
    };
    let mut dirs: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    dirs.sort();
    let mut failures = Vec::new();
    for d in dirs {
        let (Ok(frame), Ok(res), Ok(vram)) = (
            std::fs::read_to_string(d.join("frame.txt")),
            std::fs::read_to_string(d.join("results.txt")),
            std::fs::read(d.join("vram.bin")),
        ) else {
            continue;
        };
        let mut g = Gpu::new();
        for (i, c) in vram.chunks(2).enumerate() {
            g.vram_mut()[i] = u16::from_le_bytes([c[0], c[1]]);
        }
        let y0: usize = frame
            .lines()
            .find_map(|l| l.strip_prefix("Y "))
            .map_or(0, |v| v.parse().unwrap());
        g.fill(0, y0 as i32, 320, 240, 0);
        let trace: Option<(usize, usize)> = std::env::var("DW1_GPU_TRACE").ok().and_then(|s| {
            let mut it = s.split(',').map(|v| v.parse::<usize>().unwrap());
            Some((it.next()?, it.next()?))
        });
        for l in frame.lines().filter(|l| !l.starts_with('Y')) {
            let words: Vec<u32> = l[2..]
                .split_whitespace()
                .map(|w| u32::from_str_radix(w, 16).unwrap())
                .collect();
            let before = trace.map(|(x, y)| g.vram()[(y0 + y) * VRAM_W + x]);
            if l.starts_with('E') {
                for w in words {
                    g.gp0(&[w]);
                }
            } else {
                g.gp0(&words);
            }
            if let (Some((x, y)), Some(b)) = (trace, before) {
                let a = g.vram()[(y0 + y) * VRAM_W + x];
                if a != b {
                    eprintln!("trace ({x},{y}): {b:04x} -> {a:04x} by {l}");
                }
            }
        }
        if std::env::var_os("DW1_GPU_SAVE").is_some() {
            let mut out = Vec::new();
            for y in y0..y0 + 240 {
                for x in 0..320 {
                    out.extend_from_slice(&g.vram()[y * VRAM_W + x].to_le_bytes());
                }
            }
            std::fs::write(d.join("model.bin"), out).unwrap();
        }
        let bands: Vec<Vec<u8>> = res
            .lines()
            .filter_map(|l| l.split("801e8000=").nth(1))
            .map(|s| {
                let s = s.split_whitespace().next().unwrap_or("");
                (0..s.len() / 2)
                    .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(bands.len(), 6, "{}: bands", d.display());
        for (b, band) in bands.iter().enumerate() {
            assert_eq!(
                band.len(),
                320 * 40 * 2,
                "{}: band {b} is short",
                d.display()
            );
        }
        let mut bad = Vec::new();
        for (b, band) in bands.iter().enumerate() {
            for (i, c) in band.chunks(2).enumerate() {
                let want = u16::from_le_bytes([c[0], c[1]]);
                let (x, y) = (i % 320, y0 + b * 40 + i / 320);
                let got = g.vram()[y * VRAM_W + x];
                if got != want {
                    bad.push((x, y, got, want));
                }
            }
        }
        eprintln!("{}: {} pixels differ", d.display(), bad.len());
        if std::env::var_os("DW1_GPU_SAVE").is_some() {
            let t: String = bad
                .iter()
                .map(|p| format!("{} {}\n", p.0, p.1 - y0))
                .collect();
            std::fs::write(d.join("diffs.txt"), t).unwrap();
        }
        if std::env::var_os("DW1_GPU_SHOW").is_some() {
            for p in bad.iter().take(20) {
                eprintln!("  ({}, {}) got {:04x} want {:04x}", p.0, p.1, p.2, p.3);
                // which polygons draw this pixel in the model (each alone, on black)
                let mut state: Vec<u32> = Vec::new();
                for (n, l) in frame
                    .lines()
                    .enumerate()
                    .filter(|(_, l)| !l.starts_with('Y'))
                {
                    let words: Vec<u32> = l[2..]
                        .split_whitespace()
                        .map(|w| u32::from_str_radix(w, 16).unwrap())
                        .collect();
                    if l.starts_with('E') {
                        state = words;
                        continue;
                    }
                    let mut h = Gpu::new();
                    for (i, c) in vram.chunks(2).enumerate() {
                        h.vram_mut()[i] = u16::from_le_bytes([c[0], c[1]]);
                    }
                    h.fill(0, y0 as i32, 320, 240, 0);
                    for w in &state {
                        h.gp0(&[*w]);
                    }
                    h.gp0(&words);
                    let v = h.vram()[p.1 * VRAM_W + p.0];
                    if v != 0 {
                        eprintln!("    line {n}: model draws {v:04x}: {}", l);
                    }
                }
            }
        }
        // A `known.txt` in the frame directory holds an accepted residue (pixel count), if any;
        // none of the current frames has one (all exact).
        let known: usize = std::fs::read_to_string(d.join("known.txt"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        if bad.len() > known {
            failures.push(format!("{}: {} pixels", d.display(), bad.len()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
