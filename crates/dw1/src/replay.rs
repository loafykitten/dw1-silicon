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
    /// Format 2's CD timing per read (`psyq::libcd`); None for a format 1 recording.
    pub cd_reads: Option<Vec<psyq::libcd::RecordedRead>>,
    next: usize,
    /// Redux counts VBlanks from power-on, the native clock from the game's ResetCallback.
    vblank_offset: i64,
    first_vblank_mismatch: Option<String>,
    /// The recording's `final_vblank` header (emulator numbering).
    final_vblank: Option<u32>,
    /// Native VBlank of the most recent poll.
    last_native: u32,
    /// The recording's initial cards: `card1_sha1` / `card2_sha1` headers (`fresh` = no card file given, the new card Redux makes,
    /// a fresh formatted card) and the `card1_fixture` name.
    cards: [Option<String>; 2],
    fixture: Option<String>,
}

/// SHA-1 of a file, hex.
fn file_sha1(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = sha1_smol::Sha1::new();
    hash.update(&bytes);
    Ok(hash.digest().to_string())
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
        let mut format = 0;
        let mut cd_reads = Vec::new();
        let mut declared_cd = None;
        let mut declared = None;
        let mut final_vblank = None;
        let mut cards = [None, None];
        let mut fixture = None;
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
                    ["format", "1"] => format = 1,
                    ["format", "2"] => format = 2,
                    ["cd_reads", n] => {
                        declared_cd = Some(n.parse::<usize>().map_err(|e| format!("{line}: {e}"))?)
                    }
                    ["cd", k, issue, busy, sectors] => {
                        let num = |s: &str| s.parse::<u32>().map_err(|e| format!("{line}: {e}"));
                        if num(k)? as usize != cd_reads.len() {
                            return Err(format!("{}: cd line out of order: {line}", path.display()));
                        }
                        cd_reads.push(psyq::libcd::RecordedRead {
                            issue: num(issue)?,
                            busy: num(busy)?,
                            sectors: num(sectors)?,
                        });
                    }
                    ["final_vblank", n] => {
                        final_vblank = Some(n.parse::<u32>().map_err(|e| format!("{line}: {e}"))?)
                    }
                    ["card1_sha1", h] => cards[0] = Some(h.to_string()),
                    ["card2_sha1", h] => cards[1] = Some(h.to_string()),
                    ["card1_fixture", name] => fixture = Some(name.to_string()),
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
        if format == 0 {
            return Err(format!("{}: not a format 1 or 2 pad replay", path.display()));
        }
        if format == 2 && declared_cd != Some(cd_reads.len()) {
            return Err(format!(
                "{}: the header declares {declared_cd:?} CD reads, the file has {}",
                path.display(),
                cd_reads.len()
            ));
        }
        if format == 1 && !cd_reads.is_empty() {
            return Err(format!("{}: cd lines in a format 1 replay", path.display()));
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
            cd_reads: (format == 2).then_some(cd_reads),
            next: 0,
            vblank_offset: 1144,
            first_vblank_mismatch: None,
            final_vblank,
            last_native: 0,
            cards,
            fixture,
        })
    }

    /// Checks the cards the run starts with (`DW1_CARD1`/`DW1_CARD2` files, or none = fresh
    /// formatted cards) against the recording's `card1_sha1`/`card2_sha1` headers. A recording
    /// without them is rejected: it doesn't say what card it was made on.
    pub fn check_cards(&self) -> Result<(), String> {
        let mut have = [String::new(), String::new()];
        for (i, var) in ["DW1_CARD1", "DW1_CARD2"].into_iter().enumerate() {
            have[i] = match std::env::var_os(var) {
                Some(path) => file_sha1(path.as_ref())?,
                None => "fresh".to_string(),
            };
        }
        self.check_card_hashes(&have)
    }

    /// [`check_cards`](Self::check_cards) on already-computed hashes (`fresh` = no card file).
    fn check_card_hashes(&self, have: &[String; 2]) -> Result<(), String> {
        for (i, var) in ["DW1_CARD1", "DW1_CARD2"].into_iter().enumerate() {
            let want = self.cards[i].as_deref().ok_or_else(|| {
                format!(
                    "the replay has no card{}_sha1 header (it doesn't name its initial card)",
                    i + 1
                )
            })?;
            let have = &have[i];
            if have != want {
                let name = match (&self.fixture, i) {
                    (Some(f), 0) => format!(" (fixture {f})"),
                    _ => String::new(),
                };
                return Err(format!(
                    "{var}: the recording started with card {want}{name} but this run has {have}"
                ));
            }
        }
        Ok(())
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
        self.last_native = vblank;
        Ok(p.mask)
    }

    /// What `VSync(-1)` returned on the PS1 at native VBlank `native`: the recording's VBlank at
    /// the last poll plus the native VBlanks since it, less `vblank_offset`, as for the polls
    /// (libetc's `Vcount` [obs: RAM dumps inside the GPU::Vsync events of route VBlanks 9640 and
    /// 8401 read 8494 and 7255, the event's own VBlank not yet counted; the game code after it
    /// sees one more: debug-gym, whose Seadramon spawns on `VSync(-1) / 3 % 20`, matches with
    /// this offset and diverges with one less]). FISH is the only reader (the bite and Seadramon
    /// checks). None before the first poll. Native loading takes no time, so the native counter
    /// alone runs ahead of the PS1's, which changed when fish bit.
    pub fn ps1_vcount(&self, native: u32) -> Option<u32> {
        let last = self.polls.get(self.next.checked_sub(1)?)?;
        // Wrapping, as the PS1's 32-bit counter does.
        let since = native.wrapping_sub(self.last_native);
        Some(last.vblank.wrapping_add(since).wrapping_sub(self.vblank_offset as u32))
    }

    /// The native VBlank at which the recording ends, once every poll has been consumed: the
    /// last poll's native VBlank plus the recording's distance from its last poll to
    /// `final_vblank`. None while polls remain, or if the header has no `final_vblank`.
    pub fn end_vblank(&self) -> Option<u32> {
        let last = self.polls.last()?;
        if self.next != self.polls.len() {
            return None;
        }
        let tail = self.final_vblank?.checked_sub(last.vblank)?;
        Some(self.last_native + tail)
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
        assert_eq!(ok.cd_reads, None);
        // Format 2: CD reads between the header and the polls, counted by the header.
        let polls = "0 1651 0 80010620\n1 1653 800 80010620\n";
        let v2 = "# test\nformat 2\npolls 2\ncd_reads 2\ncd 0 0 3 16\ncd 1 1 0 1\n";
        let read = |issue, busy, sectors| psyq::libcd::RecordedRead {
            issue,
            busy,
            sectors,
        };
        assert_eq!(
            load(&format!("{v2}{polls}")).unwrap().cd_reads,
            Some(vec![read(0, 3, 16), read(1, 0, 1)])
        );
        let short = "# test\nformat 2\npolls 2\ncd_reads 3\ncd 0 0 3 16\n";
        assert!(load(&format!("{short}{polls}")).err().unwrap().contains("CD reads"));
        let unordered = "# test\nformat 2\npolls 2\ncd_reads 1\ncd 1 0 3 16\n";
        assert!(load(&format!("{unordered}{polls}")).err().unwrap().contains("out of order"));
        assert!(
            load(&format!("{header}cd 0 0 3 16\n{polls}"))
                .err()
                .unwrap()
                .contains("format 1")
        );
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
            cd_reads: None,
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
            final_vblank: Some(1654),
            last_native: 0,
            cards: [Some("fresh".into()), Some("fresh".into())],
            fixture: None,
        };
        let none = ["fresh".to_string(), "fresh".to_string()];
        assert_eq!(replay.check_card_hashes(&none), Ok(()));
        let err = replay
            .check_card_hashes(&["fresh".into(), "abc".into()])
            .unwrap_err();
        assert!(err.contains("DW1_CARD2") && err.contains("abc"), "{err}");
        replay.cards[0] = Some("a414".into());
        replay.fixture = Some("ember-day2-gabumon".into());
        let err = replay.check_card_hashes(&none).unwrap_err();
        assert!(
            err.contains("a414") && err.contains("ember-day2-gabumon"),
            "{err}"
        );
        replay.cards[0] = None;
        assert!(
            replay
                .check_card_hashes(&none)
                .unwrap_err()
                .contains("no card1_sha1")
        );
        replay.cards[0] = Some("fresh".into());
        assert_eq!(replay.ps1_vcount(500), None);
        assert_eq!(replay.poll("MOV_playMovie", 507), Ok(0));
        // The PS1's count: the poll's 1651 less 1144, plus the native VBlanks since it.
        assert_eq!(replay.ps1_vcount(507), Some(507));
        assert_eq!(replay.ps1_vcount(520), Some(520));
        assert!(replay.timing_note().is_none());
        assert!(
            replay
                .poll("pollInputMenu", 509)
                .unwrap_err()
                .contains("divergence at poll 1")
        );
        assert_eq!(replay.poll("MOV_playMovie", 510), Ok(0x800));
        assert!(replay.timing_note().unwrap().contains("poll 1"));
        assert_eq!(replay.ps1_vcount(515), Some(1653 + 5 - 1144));
        // Across the native counter's wrap.
        replay.last_native = u32::MAX;
        assert_eq!(replay.ps1_vcount(2), Some(1653 + 3 - 1144));
        replay.last_native = 510;
        // Both polls used: the end is the last poll's frame plus 1654 - 1653.
        assert_eq!(replay.end_vblank(), Some(511));
        assert!(
            replay
                .poll("MOV_playMovie", 511)
                .unwrap_err()
                .contains("finished")
        );
    }
}
