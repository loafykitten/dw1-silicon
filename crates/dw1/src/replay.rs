//! Pad replays recorded on the emulator (`oracle/replay.lua`, S0.8): `PadRead` returns the
//! recorded state of each poll, keyed by poll sequence number, and each native poll must come
//! from the same game function as the recorded one, or the run stops as diverged.
//!
//! The recording names its caller by a PS1 return address; the decomp's symbol lists
//! (`decomp/config/us/symbols*.txt`) turn that into function names (an overlay address can
//! belong to several overlays loaded at the same place, so it's a set of candidates). The
//! native caller is named from the stack.

use std::collections::BTreeMap;
use std::path::Path;

/// One recorded poll.
pub struct Poll {
    pub vblank: u32,
    pub mask: u32,
    /// Functions the recorded return address can be in.
    pub callers: Vec<String>,
}

pub struct Replay {
    pub polls: Vec<Poll>,
    next: usize,
    /// Redux counts VBlanks from power-on, the native clock from the game's ResetCallback.
    vblank_offset: i64,
    first_vblank_mismatch: Option<String>,
}

/// Symbol lists: address -> name, for main and every overlay.
fn symbols(dir: &Path) -> Result<Vec<BTreeMap<u32, String>>, String> {
    let mut tables = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !(name.starts_with("symbols") && name.ends_with(".txt")) {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut table = BTreeMap::new();
        for line in text.lines() {
            let Some((sym, rest)) = line.split_once('=') else {
                continue;
            };
            let value = rest.split(';').next().unwrap_or("").trim();
            if let Some(hex) = value.strip_prefix("0x")
                && let Ok(addr) = u32::from_str_radix(hex, 16)
            {
                table.insert(addr, sym.trim().to_string());
            }
        }
        tables.push(table);
    }
    Ok(tables)
}

/// The functions `ra` can be in: the nearest symbol at or below it in each list that covers
/// it (a list covers an address if it has symbols on both sides of it).
fn callers_of(ra: u32, tables: &[BTreeMap<u32, String>]) -> Vec<String> {
    let mut names = Vec::new();
    for table in tables {
        let below = table.range(..=ra).next_back();
        let above = table.range(ra..).next();
        if let (Some((_, name)), Some(_)) = (below, above)
            && !names.contains(name)
        {
            names.push(name.clone());
        }
    }
    names
}

impl Replay {
    pub fn load(path: &Path, symbols_dir: &Path) -> Result<Replay, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let tables = symbols(symbols_dir)?;
        let mut polls = Vec::new();
        let mut format_ok = false;
        let mut declared = None;
        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            if f[0].parse::<usize>().is_err() {
                // A header line ("key value..."), only before the polls.
                if !polls.is_empty() {
                    return Err(format!(
                        "{}: header line after the polls: {line}",
                        path.display()
                    ));
                }
                match f[..] {
                    ["format", "1"] => format_ok = true,
                    ["polls", n] => {
                        declared = Some(n.parse::<usize>().map_err(|e| format!("{line}: {e}"))?)
                    }
                    _ => {}
                }
                continue;
            }
            if f.len() != 4 {
                return Err(format!("{}: malformed poll line: {line}", path.display()));
            }
            let seq: usize = f[0].parse().unwrap();
            if seq != polls.len() {
                return Err(format!("{}: poll {seq} out of order", path.display()));
            }
            let num = |s: &str| u32::from_str_radix(s, 16).map_err(|e| format!("{line}: {e}"));
            let ra = num(f[3])?;
            polls.push(Poll {
                vblank: f[1].parse().map_err(|e| format!("{line}: {e}"))?,
                mask: num(f[2])?,
                callers: callers_of(ra, &tables),
            });
        }
        if !format_ok {
            return Err(format!("{}: not a format 1 pad replay", path.display()));
        }
        if declared != Some(polls.len()) {
            return Err(format!(
                "{}: the header declares {declared:?} polls, the file has {} (truncated?)",
                path.display(),
                polls.len()
            ));
        }
        Ok(Replay {
            polls,
            next: 0,
            vblank_offset: 1144,
            first_vblank_mismatch: None,
        })
    }

    /// The recorded state for the next poll, from native caller `caller` at native VBlank
    /// `vblank`; Err when the run has diverged or the recording is used up.
    pub fn poll(&mut self, caller: &str, vblank: u32) -> Result<u32, String> {
        let seq = self.next;
        let Some(p) = self.polls.get(seq) else {
            return Err(format!(
                "replay finished: all {} polls used",
                self.polls.len()
            ));
        };
        if !p.callers.iter().any(|c| c == caller) {
            return Err(format!(
                "divergence at poll {seq}: the recording polled from {:?}, the port from {caller} \
                 (native vblank {vblank}, recorded {})",
                p.callers, p.vblank
            ));
        }
        let expected = i64::from(p.vblank) - self.vblank_offset;
        if self.first_vblank_mismatch.is_none() && i64::from(vblank) != expected {
            self.first_vblank_mismatch = Some(format!(
                "first VBlank difference at poll {seq}: recorded {} (native {expected}), port {vblank}",
                p.vblank
            ));
        }
        self.next += 1;
        Ok(p.mask)
    }

    pub fn polls_done(&self) -> usize {
        self.next
    }

    pub fn timing_note(&self) -> Option<&str> {
        self.first_vblank_mismatch.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_rejects_truncated_and_malformed_replays() {
        let dir = std::env::temp_dir().join(format!("dw1-replay-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("replay.txt");
        let load = |text: &str| {
            std::fs::write(&file, text).unwrap();
            Replay::load(&file, &dir)
        };
        let header = "# test\nformat 1\npolls 2\n";
        let ok = load(&format!("{header}0 1651 0 80010620\n1 1653 800 80010620\n")).unwrap();
        assert_eq!(ok.polls.len(), 2);
        assert!(load(header).err().unwrap().contains("truncated"));
        assert!(
            load(&format!("{header}0 1651 0 80010620\n"))
                .err()
                .unwrap()
                .contains("truncated")
        );
        assert!(
            load(&format!("{header}0 1651 0\n"))
                .err()
                .unwrap()
                .contains("malformed")
        );
        assert!(
            load(&format!("{header}0 1651 0 80010620\npolls 2\n"))
                .err()
                .unwrap()
                .contains("after the polls")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn callers_resolve_per_overlay_and_polls_check_them() {
        let main: BTreeMap<u32, String> = [(0x800E_FE14, "pollInputMenu"), (0x800E_FF00, "next")]
            .into_iter()
            .map(|(a, n)| (a, n.to_string()))
            .collect();
        let mov: BTreeMap<u32, String> = [(0x8001_0620, "MOV_playMovie"), (0x8001_0900, "end")]
            .into_iter()
            .map(|(a, n)| (a, n.to_string()))
            .collect();
        let btl: BTreeMap<u32, String> = [(0x8005_2AE0, "BTL_x"), (0x8005_3000, "BTL_y")]
            .into_iter()
            .map(|(a, n)| (a, n.to_string()))
            .collect();
        let tables = [main, mov, btl];
        assert_eq!(callers_of(0x800E_FE2C, &tables), ["pollInputMenu"]);
        assert_eq!(callers_of(0x8001_08D8, &tables), ["MOV_playMovie"]);
        assert!(callers_of(0x8000_0000, &tables).is_empty());

        let mut replay = Replay {
            polls: vec![
                Poll {
                    vblank: 1651,
                    mask: 0,
                    callers: vec!["MOV_playMovie".into()],
                },
                Poll {
                    vblank: 1653,
                    mask: 0x800,
                    callers: vec!["MOV_playMovie".into()],
                },
            ],
            next: 0,
            vblank_offset: 1144,
            first_vblank_mismatch: None,
        };
        assert_eq!(replay.poll("MOV_playMovie", 507), Ok(0));
        assert!(replay.timing_note().is_none());
        assert!(
            replay
                .poll("pollInputMenu", 509)
                .unwrap_err()
                .contains("divergence at poll 1")
        );
        assert_eq!(replay.poll("MOV_playMovie", 510), Ok(0x800));
        assert!(replay.timing_note().unwrap().contains("poll 1"));
        assert!(
            replay
                .poll("MOV_playMovie", 511)
                .unwrap_err()
                .contains("finished")
        );
    }
}
