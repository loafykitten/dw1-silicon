//! The headless host: runs the game on the virtual clock with no window, writing the display
//! as PNG at chosen VBlanks and stopping at a chosen one. Its numbering matches
//! `oracle/capture-calls.lua` (VBlanks since the game's `ResetCallback`; the emulator counts
//! from power-on, about 1144 more for DW1's boot), so native frames and emulator frames can be
//! put side by side.
//!
//! Env: `DW1_SHOTS=first:last:step` (screenshots `shot_<vblank>.png`), `DW1_STOP_AT=<vblank>`
//! (exit 0 there), `DW1_OUT=<dir>` (default `captures/native-runs/latest`),
//! `DW1_START_AT=<vblank>` (pad 1 holds Start from there on).

use std::path::PathBuf;

pub struct Headless {
    shots: Option<(u32, u32, u32)>,
    stop_at: Option<u32>,
    start_at: Option<u32>,
    out: PathBuf,
}

fn env_num(name: &str) -> Option<u32> {
    let v = std::env::var(name).ok()?;
    match v.parse() {
        Ok(n) => Some(n),
        Err(_) => {
            eprintln!("dw1: {name} is not a number: {v}");
            std::process::exit(2);
        }
    }
}

impl Headless {
    pub fn from_env() -> Headless {
        let shots = std::env::var("DW1_SHOTS").ok().map(|s| {
            let parts: Vec<u32> = s.split(':').filter_map(|p| p.parse().ok()).collect();
            match parts[..] {
                [a, b, c] if c > 0 => (a, b, c),
                _ => {
                    eprintln!("dw1: DW1_SHOTS must be first:last:step");
                    std::process::exit(2);
                }
            }
        });
        let out = std::env::var_os("DW1_OUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../captures/native-runs/latest")
            });
        if shots.is_some() {
            std::fs::create_dir_all(&out).unwrap_or_else(|e| {
                eprintln!("dw1: {}: {e}", out.display());
                std::process::exit(2);
            });
        }
        Headless {
            shots,
            stop_at: env_num("DW1_STOP_AT"),
            start_at: env_num("DW1_START_AT"),
            out,
        }
    }

    fn shot(&self, vblank: u32) {
        let mut rgb = vec![0u32; 640 * 480];
        let Some((w, h)) = psyq::gpu::with(|g| g.read_display(&mut rgb)) else {
            eprintln!("dw1: vblank {vblank}: display off, no screenshot");
            return;
        };
        let bytes: Vec<u8> = rgb[..w * h]
            .iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
            .collect();
        let path = self.out.join(format!("shot_{vblank:05}.png"));
        let file = std::fs::File::create(&path).unwrap_or_else(|e| {
            eprintln!("dw1: {}: {e}", path.display());
            std::process::exit(2);
        });
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .and_then(|mut wr| wr.write_image_data(&bytes))
            .unwrap_or_else(|e| eprintln!("dw1: {}: {e}", path.display()));
    }
}

impl psyq::libetc::Host for Headless {
    fn vblank(&mut self, vblank: u32) {
        if let Some((first, last, step)) = self.shots
            && (first..=last).contains(&vblank)
            && (vblank - first).is_multiple_of(step)
        {
            self.shot(vblank);
        }
        if self.stop_at == Some(vblank) {
            eprintln!("dw1: stopped at vblank {vblank}");
            std::process::exit(0);
        }
    }

    fn pad(&mut self) -> u32 {
        match self.start_at {
            Some(at) if psyq::libetc::vblank_count() >= at => 0x800,
            _ => 0,
        }
    }
}
