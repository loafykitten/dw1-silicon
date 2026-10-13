//! The headless host: runs the game on the virtual clock with no window, writing the display
//! as PNG at chosen VBlanks and stopping at a chosen one. Its numbering matches
//! `oracle/capture-calls.lua` (VBlanks since the game's `ResetCallback`; the emulator counts
//! from power-on, about 1144 more for DW1's boot), so native frames and emulator frames can be
//! put side by side.
//!
//! Env: `DW1_SHOTS=first:last:step` (screenshots `shot_<vblank>.png`), `DW1_STOP_AT=<vblank>`
//! (exit 0 there), `DW1_OUT=<dir>` (default `captures/native-runs/latest`), `DW1_REPLAY=<file>`
//! (pad input from an emulator replay, `oracle/replays/`; the run stops when it diverges or the
//! recording ends).

use std::path::PathBuf;

use crate::replay::Replay;

pub struct Headless {
    shots: Option<(u32, u32, u32)>,
    stop_at: Option<u32>,
    out: PathBuf,
    replay: Option<Replay>,
}

/// The name of the function containing `addr` (the native game code is in this binary).
fn function_name(addr: usize) -> String {
    #[repr(C)]
    struct DlInfo {
        fname: *const std::ffi::c_char,
        fbase: *mut std::ffi::c_void,
        sname: *const std::ffi::c_char,
        saddr: *mut std::ffi::c_void,
    }
    unsafe extern "C" {
        fn dladdr(addr: *const std::ffi::c_void, info: *mut DlInfo) -> i32;
    }
    let mut info = DlInfo {
        fname: std::ptr::null(),
        fbase: std::ptr::null_mut(),
        sname: std::ptr::null(),
        saddr: std::ptr::null_mut(),
    };
    // The return address can sit just past a call at the very end of a function; look one
    // byte back so it resolves to the caller.
    let found = unsafe { dladdr((addr - 1) as *const _, &mut info) };
    if found == 0 || info.sname.is_null() {
        return format!("{addr:#x}");
    }
    unsafe { std::ffi::CStr::from_ptr(info.sname) }
        .to_string_lossy()
        .into_owned()
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
        let replay = std::env::var_os("DW1_REPLAY").map(|path| {
            let symbols = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../decomp/config/us");
            Replay::load(path.as_ref(), &symbols).unwrap_or_else(|e| {
                eprintln!("dw1: {e}");
                std::process::exit(2);
            })
        });
        Headless {
            shots,
            stop_at: env_num("DW1_STOP_AT"),
            out,
            replay,
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

    fn pad(&mut self, caller: usize) -> u32 {
        let Some(replay) = self.replay.as_mut() else {
            return 0;
        };
        let name = function_name(caller);
        match replay.poll(&name, psyq::libetc::vblank_count()) {
            Ok(mask) => mask,
            Err(e) => {
                eprintln!("dw1: {e}");
                if let Some(note) = replay.timing_note() {
                    eprintln!("dw1: {note}");
                }
                let ok = replay.polls_done() == replay.polls.len();
                std::process::exit(if ok { 0 } else { 1 });
            }
        }
    }
}
