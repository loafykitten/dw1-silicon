//! The SEQ player inside libsnd: reads a SEQ's score and says, tick by tick, which MIDI events
//! are due. What the events do to voices is libsnd's (`libsnd.rs`).
//!
//! The format (psx-spx, "SEQ - Single Sequence"): "pQES", version (4 bytes), ticks per quarter
//! note (2 bytes, big-endian), tempo in microseconds per quarter note (3 bytes), rhythm (2
//! bytes), then Standard MIDI File style events: a variable-length delta time, then a MIDI event
//! with running status, ending with FF 2F. A tempo change is FF 51 and three bytes, with no
//! length byte [obs: the two in DW1's music, FAALL.VHB fonts 17 and 29, parse that way].
//!
//! Timing [obs: the SDK probed with synthetic SEQs, captures/oracle-runs/p6-probe-seqB, -seqD]:
//! the player counts in tenths of a MIDI tick. Each tick takes `step` tenths, `step` being the
//! tempo's MIDI ticks per 240 Hz tick, times 10, rounded (60 BPM at 480 ticks a quarter: 20);
//! an event is due once the tenths counted since the previous one cover its delta. Remainders
//! carry over. A tempo change applies to the tick it comes in: what that tick overshot is
//! rescaled to the new tempo.
//!
//! Loops [obs: -seqC, the battle music, which jumps back to the notes right after its NRPN 20,
//! and the field music, whose loop takes the delta after its NRPN 20 only the first time]: NRPN
//! 20 (controller 99) marks the loop start at the event after it, a jump back landing on that
//! event due at once; a data entry (controller 6), before or after it, gives the loop count;
//! NRPN 30 marks the loop end. A count of 127 loops forever and jumps back as soon as the loop end is read
//! (the only count DW1's music uses). A finite count plays the loop that many times, and each
//! jump back comes after the delta of the event that follows the loop end, which is skipped. At
//! the end of the score the SEQ starts again from the top as long as `SsSeqPlay`'s play count
//! allows (0 = forever), else it ends.

/// A MIDI event due now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Key on, or key off when `velocity` is 0. 0x80 is read as a key on too [obs].
    Note {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    Control {
        channel: u8,
        control: u8,
        value: u8,
    },
    Program {
        channel: u8,
        program: u8,
    },
    /// Pitch bend: only the high byte counts [obs].
    Bend {
        channel: u8,
        msb: u8,
    },
    /// The score is over (after the last play): libsnd stops the SEQ.
    End,
}

/// Where the score data starts: after the 15-byte header.
const SCORE: usize = 15;
/// The longest SEQ copied: DW1's SEQ buffer is 0x5800 bytes.
pub const MAX_SEQ_BYTES: usize = 0x5800;
/// The loop count that means forever.
const FOREVER: u8 = 127;

/// A SEQ being played.
#[derive(Clone, Debug)]
pub struct Player {
    data: Vec<u8>,
    resolution: u32,
    /// Tenths of a MIDI tick per 240 Hz tick: now, and as the header sets it.
    step: i64,
    header_step: i64,
    pos: usize,
    status: u8,
    /// Tenths of a MIDI tick until the next event is due (<= 0: due).
    wait: i64,
    /// `SsSeqPlay`'s play count left (0 = forever).
    plays_left: i32,
    /// NRPN number (controller 99) and the last data entry (controller 6) seen, for loops.
    nrpn: Option<u8>,
    data_entry: Option<u8>,
    /// The loop start: position and the running status there.
    loop_start: Option<(usize, u8)>,
    loop_count: u8,
    loops_done: u8,
    /// A finite loop's jump back, waiting for the next event's delta.
    jump_pending: bool,
    /// NRPN 20 was just read: the loop start is set once the next delta is.
    mark_pending: bool,
    /// A jump just landed: the event there is due at once.
    jumped: bool,
    ended: bool,
}

/// Tenths of a MIDI tick per 240 Hz tick: resolution x 1,000,000 / tempo / 240 x 10, rounded.
pub fn step(resolution: u32, tempo: u32) -> i64 {
    let num = u64::from(resolution) * 1_000_000 * 10;
    let den = u64::from(tempo.max(1)) * 240;
    ((num + den / 2) / den) as i64
}

/// Events one tick may run before the score counts as broken: a loop or replay that takes no
/// time would otherwise spin forever.
const MAX_EVENTS_PER_TICK: usize = 4096;

/// Copies the SEQ at `addr` (up to its end marker, at most [`MAX_SEQ_BYTES`]).
///
/// # Safety
/// `addr` must point to readable memory holding a SEQ, or at least [`MAX_SEQ_BYTES`] readable
/// bytes.
pub unsafe fn copy_seq(addr: *const u8) -> Option<Vec<u8>> {
    // Read a byte at a time so nothing past the end marker is touched.
    let mut buf = Vec::new();
    while buf.len() < MAX_SEQ_BYTES {
        // SAFETY: the caller's contract; each byte is read only while the SEQ hasn't ended.
        buf.push(unsafe { addr.add(buf.len()).read() });
        match seq_status(&buf) {
            Parse::Done(len) => {
                buf.truncate(len);
                return Some(buf);
            }
            Parse::Bad => return None,
            Parse::More => {}
        }
    }
    None
}

enum Parse {
    Done(usize),
    More,
    Bad,
}

fn seq_status(data: &[u8]) -> Parse {
    if data.len() >= 4 && &data[..4] != b"pQES" {
        return Parse::Bad;
    }
    match seq_len_inner(data) {
        Ok(len) => Parse::Done(len),
        Err(true) => Parse::More,
        Err(false) => Parse::Bad,
    }
}

/// The length of the SEQ at `data` (header, score, end marker), if it parses within `data`.
pub fn seq_len(data: &[u8]) -> Option<usize> {
    seq_len_inner(data).ok()
}

/// Err(true): runs off the end of `data`; Err(false): not a SEQ.
fn seq_len_inner(data: &[u8]) -> Result<usize, bool> {
    let r = seq_len_opt(data);
    match r {
        Some(Ok(n)) => Ok(n),
        Some(Err(())) => Err(false),
        None => Err(true),
    }
}

/// None: needs more data; Some(Err): not a SEQ.
fn seq_len_opt(data: &[u8]) -> Option<Result<usize, ()>> {
    if data.len() < SCORE {
        return None;
    }
    if &data[..4] != b"pQES" {
        return Some(Err(()));
    }
    let mut pos = SCORE;
    let mut status = 0u8;
    loop {
        read_vlq(data, &mut pos)?;
        let mut b = *data.get(pos)?;
        if b & 0x80 != 0 {
            pos += 1;
            if b != 0xff {
                status = b;
            }
        } else if status == 0 {
            return Some(Err(()));
        } else {
            b = status;
        }
        match b {
            0xff => {
                let kind = *data.get(pos)?;
                pos += 1;
                match kind {
                    0x2f => {
                        // The end marker's length byte.
                        data.get(pos)?;
                        return Some(Ok(pos + 1));
                    }
                    0x51 => pos += 3,
                    _ => return Some(Err(())),
                }
            }
            0xc0..=0xdf => pos += 1,
            0x80..=0xef => pos += 2,
            _ => return Some(Err(())),
        }
        if pos > data.len() {
            return None;
        }
    }
}

fn read_vlq(data: &[u8], pos: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    for _ in 0..4 {
        let c = *data.get(*pos)?;
        *pos += 1;
        v = (v << 7) | u32::from(c & 0x7f);
        if c & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

impl Player {
    /// A player for the SEQ in `data` (from its "pQES"), or None if it doesn't parse.
    pub fn new(data: &[u8]) -> Option<Player> {
        let len = seq_len(data)?;
        let resolution = u32::from(u16::from_be_bytes([data[8], data[9]]));
        let tempo = u32::from_be_bytes([0, data[10], data[11], data[12]]);
        Some(Player {
            data: data[..len].to_vec(),
            resolution,
            step: step(resolution, tempo),
            header_step: step(resolution, tempo),
            pos: SCORE,
            status: 0,
            wait: 0,
            plays_left: 0,
            nrpn: None,
            data_entry: None,
            loop_start: None,
            loop_count: 0,
            loops_done: 0,
            jump_pending: false,
            mark_pending: false,
            jumped: false,
            ended: false,
        })
    }

    /// `SsSeqPlay`: from the top, `plays` times (0 = forever).
    pub fn play(&mut self, plays: i32) {
        self.pos = SCORE;
        self.status = 0;
        self.step = self.header_step;
        self.plays_left = plays;
        self.nrpn = None;
        self.data_entry = None;
        self.loop_start = None;
        self.loops_done = 0;
        self.jump_pending = false;
        self.mark_pending = false;
        self.jumped = false;
        self.ended = false;
        self.wait = self.next_delta();
    }

    fn next_delta(&mut self) -> i64 {
        let mut pos = self.pos;
        let d = read_vlq(&self.data, &mut pos).unwrap_or(0);
        self.pos = pos;
        i64::from(d) * 10
    }

    /// One 240 Hz tick: the events that fall due, in order.
    pub fn tick(&mut self, out: &mut Vec<Event>) {
        if self.ended {
            return;
        }
        self.wait -= self.step;
        let mut budget = MAX_EVENTS_PER_TICK;
        while self.wait <= 0 && !self.ended {
            budget -= 1;
            if budget == 0 {
                eprintln!("psyq: a SEQ loops without taking time; stopped");
                self.ended = true;
                out.push(Event::End);
                return;
            }
            if self.jump_pending {
                self.jump_pending = false;
                let (pos, status) = self.loop_start.unwrap_or((SCORE, 0));
                (self.pos, self.status) = (pos, status);
                continue;
            }
            self.event(out);
            if self.ended {
                break;
            }
            if std::mem::take(&mut self.jumped) {
                continue;
            }
            self.wait += self.next_delta();
            if std::mem::take(&mut self.mark_pending) {
                self.loop_start = Some((self.pos, self.status));
            }
        }
    }

    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    /// Reads and handles the event at `pos` (its delta already read).
    fn event(&mut self, out: &mut Vec<Event>) {
        let mut b = self.byte();
        if b & 0x80 != 0 {
            // A meta event doesn't change the running status (Standard MIDI Files).
            if b != 0xff {
                self.status = b;
            }
        } else {
            self.pos -= 1;
            b = self.status;
        }
        let channel = b & 0x0f;
        match b {
            0xff => match self.byte() {
                0x51 => {
                    let t = [0, self.byte(), self.byte(), self.byte()];
                    let new = step(self.resolution, u32::from_be_bytes(t));
                    // What's overshot this tick counts at the new tempo [obs].
                    self.wait = self.wait * new / self.step.max(1);
                    self.step = new;
                }
                _ => self.end_of_score(out),
            },
            0x80..=0x9f => {
                let note = self.byte();
                let velocity = self.byte();
                out.push(Event::Note {
                    channel,
                    note,
                    velocity,
                });
            }
            0xb0..=0xbf => {
                let control = self.byte();
                let value = self.byte();
                self.control(channel, control, value, out);
            }
            0xc0..=0xcf => {
                let program = self.byte();
                out.push(Event::Program { channel, program });
            }
            0xe0..=0xef => {
                let _lsb = self.byte();
                let msb = self.byte();
                out.push(Event::Bend { channel, msb });
            }
            // Aftertouch and the rest: DW1's music has none. Skip their data.
            0xd0..=0xdf => self.pos += 1,
            _ => self.pos += 2,
        }
    }

    fn control(&mut self, channel: u8, control: u8, value: u8, out: &mut Vec<Event>) {
        match control {
            99 => {
                self.nrpn = Some(value);
                match value {
                    20 => {
                        self.mark_pending = true;
                        self.loops_done = 0;
                        if let Some(count) = self.data_entry {
                            self.loop_count = count;
                        }
                    }
                    30 => self.loop_end(),
                    _ => {}
                }
            }
            6 => {
                self.data_entry = Some(value);
                if self.nrpn == Some(20) {
                    self.loop_count = value;
                }
            }
            _ => out.push(Event::Control {
                channel,
                control,
                value,
            }),
        }
    }

    fn loop_end(&mut self) {
        let Some((pos, status)) = self.loop_start else {
            return;
        };
        if self.loop_count == FOREVER {
            (self.pos, self.status) = (pos, status);
            self.jumped = true;
            return;
        }
        self.loops_done += 1;
        if self.loops_done < self.loop_count {
            self.jump_pending = true;
        }
    }

    fn end_of_score(&mut self, out: &mut Vec<Event>) {
        if self.plays_left != 1 {
            if self.plays_left > 1 {
                self.plays_left -= 1;
            }
            self.pos = SCORE;
            self.status = 0;
            self.loop_start = None;
            return;
        }
        self.ended = true;
        out.push(Event::End);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(events: &[(u32, &[u8])], resolution: u16, tempo: u32) -> Vec<u8> {
        let mut b = b"pQES".to_vec();
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&resolution.to_be_bytes());
        b.extend_from_slice(&tempo.to_be_bytes()[1..]);
        b.extend_from_slice(&[1, 4]);
        for (d, e) in events {
            let mut v = vec![(d & 0x7f) as u8];
            let mut n = d >> 7;
            while n > 0 {
                v.insert(0, 0x80 | (n & 0x7f) as u8);
                n >>= 7;
            }
            b.extend(v);
            b.extend_from_slice(e);
        }
        b.extend_from_slice(&[0, 0xff, 0x2f, 0]);
        b
    }

    /// The ticks (from 0, the first after `SsSeqPlay`) at which each note event falls due.
    fn note_ticks(data: &[u8], plays: i32, ticks: usize) -> Vec<(usize, u8, u8)> {
        let mut p = Player::new(data).unwrap();
        p.play(plays);
        let mut out = Vec::new();
        let mut got = Vec::new();
        for t in 0..ticks {
            out.clear();
            p.tick(&mut out);
            for e in &out {
                if let Event::Note { note, velocity, .. } = *e {
                    got.push((t, note, velocity));
                }
            }
        }
        got
    }

    #[test]
    fn step_matches_the_sdk() {
        // [obs] the probes' tempos at 480 and 96 ticks a quarter.
        assert_eq!(step(480, 500_000), 40);
        assert_eq!(step(480, 600_000), 33);
        assert_eq!(step(480, 750_000), 27);
        assert_eq!(step(480, 461_538), 43);
        assert_eq!(step(96, 600_000), 7);
        assert_eq!(step(96, 750_000), 5);
    }

    #[test]
    fn events_fall_due_like_the_sdk() {
        // [obs] -seqB: note on at delta d, off d + 5 later; libsnd writes a tick's events at the
        // next tick, so the SDK's write tick is one more than these.
        for (tempo, res, d, on, off) in [
            (600_000, 480, 333, 101, 204),
            (750_000, 480, 47, 18, 37),
            (461_538, 96, 100, 112, 228),
            (500_000, 480, 0, 1, 2),
            (500_000, 480, 3, 1, 3),
        ] {
            let data = seq(
                &[(d, &[0x90, 60, 127]), (d + 5, &[0x90, 60, 0])],
                res,
                tempo,
            );
            let got = note_ticks(&data, 1, 400);
            assert_eq!(got[0].0 + 1, on, "{tempo} {res} {d} on");
            assert_eq!(got[1].0 + 1, off, "{tempo} {res} {d} off");
        }
    }

    #[test]
    fn loops_and_replays() {
        // [obs] -seqC group 9: loop count 3; notes 60 (looped) then 50.
        let body: &[(u32, &[u8])] = &[
            (0, &[0xb0, 99, 20]),
            (0, &[0xb0, 6, 3]),
            (8, &[0x90, 60, 127]),
            (8, &[0x90, 60, 0]),
            (8, &[0xb0, 99, 30]),
            (8, &[0xc0, 3]),
            (0, &[0x90, 50, 127]),
            (8, &[0x90, 50, 0]),
        ];
        let got: Vec<_> = note_ticks(&seq(body, 480, 500_000), 1, 60)
            .into_iter()
            .filter(|n| n.2 > 0)
            .map(|n| (n.0 + 1, n.1))
            .collect();
        assert_eq!(got, [(2, 60), (10, 60), (18, 60), (24, 50)]);
        // Forever: jumps back as soon as the loop end is read.
        let mut forever = body.to_vec();
        forever[1] = (0, &[0xb0, 6, 127]);
        let got: Vec<_> = note_ticks(&seq(&forever, 480, 500_000), 1, 22)
            .into_iter()
            .filter(|n| n.2 > 0)
            .map(|n| n.0 + 1)
            .collect();
        assert_eq!(got, [2, 8, 14, 20]);
        // Two plays of a whole score, then the end.
        let two = seq(&[(0, &[0x90, 60, 127]), (8, &[0x90, 60, 0])], 480, 500_000);
        let mut p = Player::new(&two).unwrap();
        p.play(2);
        let mut out = Vec::new();
        for _ in 0..10 {
            p.tick(&mut out);
        }
        assert_eq!(
            out.iter()
                .filter(|e| matches!(e, Event::Note { velocity: 127, .. }))
                .count(),
            2
        );
        assert_eq!(out.last(), Some(&Event::End));
        assert!(p.ended);
    }

    #[test]
    fn a_score_that_takes_no_time_ends() {
        // An empty score replayed forever, and a loop of nothing.
        for data in [
            seq(&[], 480, 500_000),
            seq(
                &[
                    (0, &[0xb0, 6, 127]),
                    (0, &[0xb0, 99, 20]),
                    (0, &[0xb0, 99, 30]),
                ],
                480,
                500_000,
            ),
        ] {
            let mut p = Player::new(&data).unwrap();
            p.play(0);
            let mut out = Vec::new();
            p.tick(&mut out);
            assert_eq!(out.last(), Some(&Event::End));
        }
    }

    #[test]
    fn a_new_play_starts_at_the_header_tempo() {
        let data = seq(
            &[(0, &[0xff, 0x51, 0x03, 0xd0, 0x90]), (40, &[0x90, 60, 127])],
            480,
            500_000,
        );
        let mut p = Player::new(&data).unwrap();
        p.play(1);
        let mut out = Vec::new();
        p.tick(&mut out);
        assert_eq!(p.step, 80);
        p.play(1);
        assert_eq!(p.step, 40);
    }

    #[test]
    fn tempo_change() {
        // [obs] -seqD: tempo 250000 at the start, a note 40 MIDI ticks later: written at tick 5
        // (due at 4), and 333333: written at 7.
        let data = seq(
            &[(0, &[0xff, 0x51, 0x03, 0xd0, 0x90]), (40, &[0x90, 60, 127])],
            480,
            500_000,
        );
        assert_eq!(note_ticks(&data, 1, 20)[0].0 + 1, 5);
        let data = seq(
            &[(0, &[0xff, 0x51, 0x05, 0x16, 0x15]), (40, &[0x90, 60, 127])],
            480,
            500_000,
        );
        assert_eq!(note_ticks(&data, 1, 20)[0].0 + 1, 7);
    }

    #[test]
    fn length_of_the_disc_shape() {
        let data = seq(
            &[(0, &[0xb0, 7, 127]), (30, &[0x90, 60, 100]), (0, &[60, 0])],
            480,
            500_000,
        );
        assert_eq!(seq_len(&data), Some(data.len()));
        assert_eq!(seq_len(&data[..data.len() - 2]), None);
        assert_eq!(seq_len(b"nope"), None);
    }
}
