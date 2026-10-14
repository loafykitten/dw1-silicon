//! The headless host: runs the game on the virtual clock with no window, writing the display
//! as PNG at chosen VBlanks and stopping at a chosen one. Its numbering matches
//! `oracle/capture-calls.lua` (VBlanks since the game's `ResetCallback`; the emulator counts
//! from power-on, about 1144 more for DW1's boot), so native frames and emulator frames can be
//! put side by side.
//!
//! Env: `DW1_SHOTS=first:last:step` (screenshots `shot_<vblank>.png`), `DW1_STOP_AT=<vblank>`
//! (exit 0 there; `DW1_VRAM_DUMP=<file>` writes the VRAM there, and `DW1_GP0_DUMP=1` prints every
//! polygon packet with its draw state and a `V <n>` line per VBlank, for
//! `tools/oracle/gpu_frame_requests.py`), `DW1_OUT=<dir>` (default `captures/native-runs/latest`), `DW1_REPLAY=<file>`
//! (pad input from an emulator replay, `oracle/replays/`; the run stops when it diverges or the
//! recording ends), `DW1_START_AT=<vblank>` (without a replay, pad 1 holds Start from there on).
//!
//! Checkpoints (`DW1_CHECKPOINTS`, on by default with `DW1_REPLAY`; `0` turns them off, `1`
//! turns them on without a replay): the canonical game-state record (`canon.rs`,
//! `docs/canonical-state.md`) written to `DW1_OUT` as `ckpt_NNNN_<why>.bin` and `.txt`, at
//! the same triggers and with the same numbering as `oracle/replay.lua`: a change of
//! MAIN_STATE (`main<N>`), GAME_STATE (`game<N>`) or CURRENT_SCREEN (`screen<N>`) seen at a
//! pad poll, every 600th poll (`every<N>`), and `final`. The emulator writes `final` at the
//! recording's `final_vblank`, one frame after the last poll (19399 -> 19400); loading is
//! instant natively, so VBlank numbers don't carry over, and the native `final` is written at
//! the same distance after the last poll's native VBlank, only once every recorded poll has
//! been consumed (also at `DW1_STOP_AT`, whatever the poll count). The run exits 0 there.
//! Without a replay, the triggers run at every pad poll too. Stale `ckpt_NNNN_*` files in
//! `DW1_OUT` are deleted when the run starts (only those). `tools/oracle/compare_canon.py` compares them with the emulator's.

use std::path::PathBuf;

use crate::canon;
use crate::replay::Replay;

pub struct Headless {
    shots: Option<(u32, u32, u32)>,
    stop_at: Option<u32>,
    start_at: Option<u32>,
    out: PathBuf,
    replay: Option<Replay>,
    /// DW1_REPLAY_LOG: print each poll with its recorded and native VBlank.
    replay_log: bool,
    checkpoints: Option<Checkpoints>,
}

/// Checkpoint bookkeeping, mirroring `oracle/replay.lua`'s `pollCheckpoints`.
struct Checkpoints {
    next: u32,
    last: [Option<u8>; 3],
    /// Polls seen so far (the next poll's sequence number).
    polls: usize,
    final_done: bool,
}

/// Poll interval of the `every<N>` checkpoints.
const CHECKPOINT_EVERY: usize = 600;

impl Checkpoints {
    fn write(&mut self, out: &std::path::Path, why: &str, poll: usize, vblank: u32) {
        let name = format!("ckpt_{:04}_{why}", self.next);
        self.next += 1;
        let rec = canon::collect();
        let result = std::fs::create_dir_all(out)
            .and_then(|()| std::fs::write(out.join(format!("{name}.bin")), rec.encode()))
            .and_then(|()| std::fs::write(out.join(format!("{name}.txt")), rec.render()));
        if let Err(e) = result {
            eprintln!("dw1: {}: {e}", out.join(&name).display());
            std::process::exit(2);
        }
        eprintln!("dw1: checkpoint {name} poll={poll} native vblank={vblank}");
    }

    /// Evaluated at each poll (`poll` is its sequence number), in the Lua's order.
    fn at_poll(&mut self, out: &std::path::Path, vblank: u32) {
        let poll = self.polls;
        self.polls += 1;
        // The record's first fields are the three watched bytes.
        let rec = canon::collect();
        let value = |name: &str| {
            let i = canon::FIELDS.iter().position(|f| f.name == name).unwrap();
            rec.values[i][0] as u8
        };
        let now = [
            value("mode.main_state"),
            value("mode.game_state"),
            value("screen.current_screen"),
        ];
        for (i, label) in ["main", "game", "screen"].into_iter().enumerate() {
            if self.last[i] != Some(now[i]) {
                self.write(out, &format!("{label}{}", now[i]), poll, vblank);
                self.last[i] = Some(now[i]);
            }
        }
        if poll > 0 && poll.is_multiple_of(CHECKPOINT_EVERY) {
            self.write(out, &format!("every{poll}"), poll, vblank);
        }
    }
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

/// Removes the `ckpt_NNNN_*.bin/.txt` files an earlier run left in `out` (only those), so a
/// run that stops early can't be compared against its predecessor's checkpoints.
fn clear_stale_checkpoints(out: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(out) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let stale = name.strip_prefix("ckpt_").is_some_and(|r| {
            r.len() > 5
                && r.as_bytes()[..4].iter().all(u8::is_ascii_digit)
                && r.as_bytes()[4] == b'_'
                && (name.ends_with(".bin") || name.ends_with(".txt"))
        });
        if stale && let Err(e) = std::fs::remove_file(e.path()) {
            eprintln!("dw1: cannot remove stale {name}: {e}");
            std::process::exit(2);
        }
    }
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
        let checkpoints = match std::env::var("DW1_CHECKPOINTS").ok().as_deref() {
            Some("" | "0") => false,
            Some(_) => true,
            None => replay.is_some(),
        }
        .then_some(Checkpoints {
            next: 0,
            last: [None; 3],
            polls: 0,
            final_done: false,
        });
        if checkpoints.is_some() {
            clear_stale_checkpoints(&out);
        }
        Headless {
            checkpoints,
            shots,
            stop_at: env_num("DW1_STOP_AT"),
            start_at: env_num("DW1_START_AT"),
            replay_log: std::env::var("DW1_REPLAY_LOG").is_ok_and(|v| !v.is_empty() && v != "0"),
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
        if backend::gpu::dump_polygons() {
            eprintln!("V {vblank}");
        }
        if let Some(path) = std::env::var_os("DW1_VRAM_DUMP")
            && self.stop_at == Some(vblank)
        {
            let bytes: Vec<u8> =
                psyq::gpu::with(|g| g.vram().iter().flat_map(|w| w.to_le_bytes()).collect());
            std::fs::write(&path, bytes).unwrap_or_else(|e| eprintln!("dw1: {e}"));
        }
        if let Some((first, last, step)) = self.shots
            && (first..=last).contains(&vblank)
            && (vblank - first).is_multiple_of(step)
        {
            self.shot(vblank);
        }
        // The recording's end: the emulator writes `final` at the recording's final_vblank,
        // after the last poll. Natively the same distance after the last poll's frame.
        if let Some(replay) = self.replay.as_ref()
            && let Some(end) = replay.end_vblank()
            && vblank >= end
            && let Some(c) = self.checkpoints.as_mut()
            && !c.final_done
        {
            c.final_done = true;
            c.write(&self.out, "final", replay.polls_done(), vblank);
            if let Some(note) = replay.timing_note() {
                eprintln!("dw1: {note}");
            }
            std::process::exit(0);
        }
        if self.stop_at == Some(vblank) {
            eprintln!("dw1: stopped at vblank {vblank}");
            let polls = self.replay.as_ref().map_or(0, Replay::polls_done);
            if let Some(c) = self.checkpoints.as_mut()
                && !c.final_done
            {
                c.final_done = true;
                c.write(&self.out, "final", polls, vblank);
            }
            if let Some(note) = self.replay.as_ref().and_then(|r| r.timing_note()) {
                eprintln!("dw1: {note}");
            }
            std::process::exit(0);
        }
    }

    fn pad(&mut self, caller: usize) -> u32 {
        let Some(replay) = self.replay.as_mut() else {
            let vblank = psyq::libetc::vblank_count();
            if let Some(c) = self.checkpoints.as_mut() {
                c.at_poll(&self.out, vblank);
            }
            return match self.start_at {
                Some(at) if vblank >= at => 0x800,
                _ => 0,
            };
        };
        let name = function_name(caller);
        let vblank = psyq::libetc::vblank_count();
        if self.replay_log
            && let Some(p) = replay.polls.get(replay.polls_done())
        {
            eprintln!(
                "dw1: poll {} native vblank {vblank} recorded {} (native {}) mask {:#x} {name}",
                replay.polls_done(),
                p.vblank,
                i64::from(p.vblank) - 1144,
                p.mask
            );
        }
        match replay.poll(&name, vblank) {
            Ok(mask) => {
                if let Some(c) = self.checkpoints.as_mut() {
                    c.at_poll(&self.out, vblank);
                }
                mask
            }
            Err(e) => {
                eprintln!("dw1: {e}");
                // Normally `final` was written by the VBlank callback and we never get here;
                // a poll in the same frame as the last one is the fallback.
                if replay.polls_done() == replay.polls.len()
                    && let Some(c) = self.checkpoints.as_mut()
                    && !c.final_done
                {
                    c.final_done = true;
                    c.write(&self.out, "final", replay.polls_done(), vblank);
                }
                if let Some(note) = replay.timing_note() {
                    eprintln!("dw1: {note}");
                }
                let ok = replay.polls_done() == replay.polls.len();
                std::process::exit(if ok { 0 } else { 1 });
            }
        }
    }
}
