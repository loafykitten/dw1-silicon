//! F12: a bug snapshot (Phase 9.3). Everything needed to look into a bug found in play goes into
//! `~/Library/Application Support/dw1-silicon/reports/<UTC time>/`: the screen (1x and as shown),
//! the game's state (the canonical record the oracle compares: place, day, the partner...), the
//! last minute of pad input (in `DW1_PAD_SCRIPT`'s format), the settings, copies of the save
//! slots, the log so far, and a note of what this is. Nothing leaves the Mac. It reads the game's
//! state and changes nothing.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use super::Window;
use super::splash::write_png;

/// Fields of pad input kept: a minute at the PS1's field rate.
const KEEP_FIELDS: u32 = 3600;

/// The pad bits the game read, as changes: (vblank, bits from then on).
#[derive(Default)]
pub(super) struct PadLog {
    changes: VecDeque<(u32, u32)>,
}

impl PadLog {
    /// The game read `bits` at `vblank`. Changes older than a minute go (the last one before the
    /// minute stays: it says what was held at its start).
    pub(super) fn push(&mut self, vblank: u32, bits: u32) {
        if self.changes.back().is_none_or(|&(_, b)| b != bits) {
            self.changes.push_back((vblank, bits));
        }
        let from = vblank.saturating_sub(KEEP_FIELDS);
        while self.changes.len() > 1 && self.changes[1].0 <= from {
            self.changes.pop_front();
        }
    }

    /// `<vblank> <mask in hex>` lines, as `DW1_PAD_SCRIPT` reads them, for the minute before
    /// `now` (the game may not have read the pad for a while: a movie).
    fn script(&self, now: u32) -> String {
        let from = now.saturating_sub(KEEP_FIELDS);
        // the last change at or before the minute's start says what was held then
        let first = self
            .changes
            .iter()
            .rposition(|&(v, _)| v <= from)
            .unwrap_or(0);
        self.changes
            .iter()
            .skip(first)
            .map(|(v, b)| format!("{v} {b:04x}\n"))
            .collect()
    }
}

impl Window {
    /// Writes a bug snapshot and says where in the corner note.
    pub(super) fn bug_snapshot(&mut self) {
        let message = match self.write_snapshot() {
            Ok(dir) => {
                eprintln!("dw1: bug snapshot written to {}", dir.display());
                let name = dir
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                // short enough to keep the whole name in a narrow window
                snapshot_note(&name)
            }
            Err(e) => {
                eprintln!("dw1: bug snapshot: {e}");
                format!("Bug snapshot failed: {e}")
            }
        };
        self.show_message(message);
    }

    fn write_snapshot(&mut self) -> Result<PathBuf, String> {
        let base = crate::default_save_dir().ok_or("no home directory")?;
        let stamp = crate::applog::utc_stamp(std::time::SystemTime::now());
        let mut dir = base.join("reports").join(&stamp);
        // reserved by creating it: two in one second (or two copies of the game) get suffixes
        std::fs::create_dir_all(base.join("reports")).map_err(|e| e.to_string())?;
        let mut n = 1;
        loop {
            match std::fs::create_dir(&dir) {
                Ok(()) => break,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && n < 100 => {
                    n += 1;
                    dir = base.join("reports").join(format!("{stamp}-{n}"));
                }
                Err(e) => return Err(format!("{}: {e}", dir.display())),
            }
        }
        let mut problems = Vec::new();
        let mut note = |what: &str, r: Result<(), String>| {
            if let Err(e) = r {
                problems.push(format!("{what}: {e}"));
            }
        };

        // the screen: the PS1's picture, and the one shown at the render scale
        let mut px = vec![0u32; 640 * 480];
        let one = psyq::gpu::with(|g| g.read_display(&mut px));
        match one {
            Some((w, h)) => note(
                "screen.png",
                write_png(&dir.join("screen.png"), &px[..w * h], w as u32, h as u32),
            ),
            None => note("screen.png", Err("the display is off".into())),
        }
        let mut hd = Vec::new();
        if let Some((w, h)) = psyq::gpu::with(|g| {
            (g.scale() > 1)
                .then(|| g.read_display_hd(&mut hd))
                .flatten()
        }) {
            note(
                "screen-hd.png",
                write_png(&dir.join("screen-hd.png"), &hd[..w * h], w as u32, h as u32),
            );
        }

        // the game's state
        let rec = crate::canon::collect();
        note(
            "state.txt",
            write(&dir.join("state.txt"), rec.render().as_bytes()),
        );
        note("state.bin", write(&dir.join("state.bin"), &rec.encode()));

        // the last minute of input
        let vblank = psyq::libetc::vblank_count();
        let pad = format!(
            "# The pad bits the game read in the last minute before vblank {vblank}: from each\n\
             # line's vblank on, the mask (DW1_PAD_SCRIPT's format).\n{}",
            self.pad_log.script(vblank)
        );
        note("pad.txt", write(&dir.join("pad.txt"), pad.as_bytes()));

        note(
            "settings.txt",
            write(
                &dir.join("settings.txt"),
                self.settings.to_text().as_bytes(),
            ),
        );
        note("saves", copy_saves(&dir.join("saves")));
        if let Some(log) = crate::applog::log_path() {
            note(
                "log.txt",
                std::fs::copy(log, dir.join("log.txt"))
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
            );
        }

        let about = format!(
            "dw1-silicon bug snapshot\n\
             time (UTC): {stamp}\n\
             version: {}\n\
             vblank: {vblank}\n\
             render scale: {}, no wobble: {}, full colour: {}\n\
             \n\
             screen.png      the PS1's picture (what the game drew, 1x)\n\
             screen-hd.png   the picture as shown at the render scale (if above 1)\n\
             state.txt/.bin  the game's state (place, day, partner, ...), as the replays compare it\n\
             pad.txt         the last minute of pad input\n\
             settings.txt    the picture settings\n\
             saves/          copies of the save slots\n\
             log.txt         what the game printed so far (app runs)\n\
             {}",
            env!("CARGO_PKG_VERSION"),
            psyq::gpu::with(|g| g.scale()),
            self.settings.no_wobble,
            self.settings.full_colour,
            if problems.is_empty() {
                String::new()
            } else {
                format!("\nNot written:\n{}\n", problems.join("\n"))
            }
        );
        write(&dir.join("about.txt"), about.as_bytes())?;
        Ok(dir)
    }
}

/// The corner note for a snapshot named `name`.
fn snapshot_note(name: &str) -> String {
    format!("Bug snapshot: {name}")
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// Copies the save slots' files (`slot-NN.dw1save`), not the backups.
fn copy_saves(to: &Path) -> Result<(), String> {
    let Some(from) = crate::save_dir().map(|d| d.join("saves")) else {
        return Err("no save directory".into());
    };
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    let entries = std::fs::read_dir(&from).map_err(|e| format!("{}: {e}", from.display()))?;
    for e in entries.flatten() {
        let name = e.file_name();
        if name.to_string_lossy().ends_with(".dw1save") {
            std::fs::copy(e.path(), to.join(&name)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pad_log_keeps_a_minute_of_changes() {
        let mut log = PadLog::default();
        log.push(100, 0);
        log.push(101, 0);
        log.push(102, 0x40);
        log.push(200, 0);
        assert_eq!(log.script(300), "100 0000\n102 0040\n200 0000\n");
        // with no reads since (a movie), the snapshot's own minute: what was held at its start
        assert_eq!(log.script(5000), "200 0000\n");
        // a minute later only what was held at its start and the changes since stay
        log.push(200 + KEEP_FIELDS + 5, 0x10);
        assert_eq!(log.script(3805), "200 0000\n3805 0010\n");
        log.push(10_000, 0x10);
        assert_eq!(log.script(10_000), "3805 0010\n");
    }

    #[test]
    fn the_note_keeps_the_whole_name_in_the_narrowest_window() {
        // a 320-pixel-wide window fits (320 / 8) - 3 = 37 characters (Window::present)
        let note = snapshot_note("20261015-055900-99");
        assert!(note.chars().count() <= 37, "{note}");
    }
}
