//! The movie stream behind libcd's `St*` calls: STR video frames assembled from the sectors the
//! drive delivers, timed on the virtual VBlank clock. Pure model (no globals, no unsafe); the
//! C entry points are in `libcd`.
//!
//! **Frames [doc: psx-spx "CDROM File Formats", STR video sectors].** A video sector's 2048 data
//! bytes start with a 32-byte header (`0x0160`, chunk type `0x8001`, this sector's index in the
//! frame, the sector count, the frame number, the demuxed size in bytes, width, height, the
//! first words of the bitstream header and a `CdlLOC` slot) followed by 2016 bytes of the
//! frame's bitstream. A frame's sectors come in order and the bitstream is the concatenation of
//! their payloads, cut to the demuxed size. XA audio sectors (submode bit 2) and anything else
//! without the video header sit between them and are skipped for now (sound is Phase 6).
//!
//! **Ring [doc: libcd `StSetRing`: a ring buffer of 2048-byte sectors the library receives into].**
//! The game hands the SDK `ring_sectors` sectors of memory. Natively a frame is assembled into
//! storage this model owns (the pointers `StGetNext` returns point into it) and the game's own
//! array is left alone; what's kept from the real ring is the accounting: a frame occupies its
//! sector count times 2048 bytes of a circular buffer from the moment its last sector arrives
//! until `StFreeRing`, and when it doesn't fit it is lost [guess: the SDK also drops what it
//! can't store; its overflow callbacks are not modelled]. The game then times out, asks
//! `StGetBackloc` where to resume and restarts the read there, as on the PS1.
//!
//! **Time.** Every sector has an arrival time on the VBlank clock: the n-th sector since the
//! read began arrives at `t0 + ceil(n * 225,792 / 564,398)` VBlanks at double speed (a sector
//! every 225,792 cycles = 33,868,800 / 150 [doc]; 564,398 cycles per VBlank [obs: PLAN.md "Field
//! rate", Redux], so 2.4996 sectors per VBlank; 75 a second at single speed doubles the
//! sector time). `t0` is the VBlank the read was issued at plus [`START_LATENCY`]. Sectors are
//! processed lazily when a call looks at the stream ([`Stream::sync`]); a poll that finds no
//! frame ready runs ahead to the next frame's last sector and reports the VBlank it arrives at,
//! and the caller advances the clock there. The game's code takes no time, so a poll never gives
//! up just because it ran many times.

use std::collections::VecDeque;

/// Bytes in a sector's user data, and in the video header at its start.
pub const SECTOR_BYTES: usize = 2048;
pub const HEADER_BYTES: usize = 32;
/// Bitstream bytes in one video sector.
pub const PAYLOAD_BYTES: usize = SECTOR_BYTES - HEADER_BYTES;

/// Cycles a sector takes at double speed, 33,868,800 / 150 [doc: psx-spx "CDROM Overview"].
const SECTOR_CYCLES_2X: u64 = 225_792;
/// CPU cycles per VBlank in the emulator the oracle runs [obs: PLAN.md, "Field rate"].
const VBLANK_CYCLES: u64 = 564_398;

/// VBlanks from the read command to the first sector: the drive seeks to the Setloc position
/// and spins up. [guess; compared with PCSX-Redux in docs/phase2-movie.md]
pub const START_LATENCY: u32 = 4;

/// Most sectors a poll looks ahead for a frame before deciding none is coming (a read past the
/// end of the disc, or a region with no video).
const MAX_LOOKAHEAD: u32 = 20_000;

/// The raw sector's offsets: submode is the third byte of the subheader after the 12 sync and
/// 4 header bytes; user data starts at 24 [doc: psx-spx "CDROM Sector Encoding"].
const SUBMODE_OFFSET: usize = 18;
const DATA_OFFSET: usize = 24;
const RAW_BYTES: usize = 2352;
const SUBMODE_AUDIO: u8 = 0x04;
const SUBMODE_FORM2: u8 = 0x20;

/// A frame's StHEADER, as the game reads it [doc: libcd.h `StHEADER`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StHeader {
    pub sector: u16,
    pub sectors: u16,
    pub frame: u32,
    pub size: u32,
    pub width: u16,
    pub height: u16,
}

impl StHeader {
    /// The header at the start of a video sector's data, or `None` when it isn't one.
    pub fn parse(data: &[u8]) -> Option<StHeader> {
        let h = data.get(..HEADER_BYTES)?;
        let u16at = |i: usize| u16::from_le_bytes([h[i], h[i + 1]]);
        let u32at = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
        (u16at(0) == 0x0160 && u16at(2) == 0x8001).then(|| StHeader {
            sector: u16at(4),
            sectors: u16at(6),
            frame: u32at(8),
            size: u32at(12),
            width: u16at(16),
            height: u16at(18),
        })
    }
}

/// A circular buffer's bookkeeping: contiguous slots handed out in order, freed in any order,
/// space reclaimed from the oldest.
#[derive(Debug)]
struct Ring {
    bytes: usize,
    slots: VecDeque<RingSlot>,
}

#[derive(Debug)]
struct RingSlot {
    off: usize,
    len: usize,
    freed: bool,
}

impl Ring {
    fn new(bytes: usize) -> Ring {
        Ring {
            bytes,
            slots: VecDeque::new(),
        }
    }

    /// Reserves `len` contiguous bytes; the offset, or `None` when they don't fit.
    fn alloc(&mut self, len: usize) -> Option<usize> {
        let off = match (self.slots.front(), self.slots.back()) {
            (Some(front), Some(back)) => {
                let end = back.off + back.len;
                if end > front.off {
                    // Used: front..end. Free: end..bytes, then 0..front.
                    if end + len <= self.bytes {
                        end
                    } else if len <= front.off {
                        0
                    } else {
                        return None;
                    }
                } else if end + len <= front.off {
                    end // Wrapped: used front..bytes and 0..end. Free: end..front.
                } else {
                    return None;
                }
            }
            _ if len <= self.bytes => 0,
            _ => return None,
        };
        self.slots.push_back(RingSlot {
            off,
            len,
            freed: false,
        });
        Some(off)
    }

    fn free(&mut self, off: usize) {
        if let Some(s) = self.slots.iter_mut().find(|s| s.off == off) {
            s.freed = true;
        }
        while self.slots.front().is_some_and(|s| s.freed) {
            self.slots.pop_front();
        }
    }
}

/// A frame that is in the ring.
#[derive(Clone, Copy, Debug)]
struct Frame {
    header: StHeader,
    /// Where its StHEADER sits in the storage; the bitstream follows, [`HEADER_BYTES`] on.
    off: usize,
    data_len: usize,
    last_lba: i32,
}

/// A frame being collected from its sectors.
struct Partial {
    header: StHeader,
    head: [u8; HEADER_BYTES],
    data: Vec<u8>,
    first_lba: i32,
}

/// Where a read is: the next sector, the clock it runs on, the frame being collected.
struct Reading {
    next_lba: i32,
    t0: u32,
    sector_cycles: u64,
    taken: u32,
    partial: Option<Partial>,
}

/// What `StGetNext` hands out: the frame's pointers into the stream's storage, and the VBlank
/// the caller must advance the clock to before returning it (never earlier than now).
#[derive(Clone, Copy, Debug)]
pub struct Delivered {
    pub header: *const u8,
    pub data: *const u8,
    pub frame: u32,
    pub until: u32,
}

/// A movie stream: the ring, the stream filter `StSetStream` sets and the read in progress.
pub struct Stream {
    /// Frame storage: the ring's bytes, 4-aligned so the bitstream reads as words.
    storage: Vec<u32>,
    ring: Ring,
    start_frame: u32,
    end_frame: Option<u32>,
    reading: Option<Reading>,
    ready: VecDeque<Frame>,
    held: Vec<Frame>,
    /// Where to resume after a loss: the first sector and number of the frame the game hasn't
    /// received (the one lost, else the one after the last handed out).
    backloc: Option<(i32, u32)>,
    lost: bool,
    raw: Box<[u8; RAW_BYTES]>,
    /// Sectors skipped because they weren't video, and frames lost to a full ring.
    pub skipped: u64,
    pub lost_frames: u64,
}

impl Stream {
    /// `StSetRing`: a ring of `sectors` sectors. Streams all frames from 1 until `StSetStream`
    /// says otherwise.
    pub fn new(sectors: usize) -> Stream {
        Stream {
            storage: vec![0; sectors * SECTOR_BYTES / 4],
            ring: Ring::new(sectors * SECTOR_BYTES),
            start_frame: 0,
            end_frame: None,
            reading: None,
            ready: VecDeque::new(),
            held: Vec::new(),
            backloc: None,
            lost: false,
            raw: Box::new([0; RAW_BYTES]),
            skipped: 0,
            lost_frames: 0,
        }
    }

    /// `StSetStream`: frames before `start` are dropped; `end` (`None` for the SDK's -1) ends it.
    pub fn set_range(&mut self, start: u32, end: Option<u32>) {
        self.start_frame = start;
        self.end_frame = end;
    }

    /// Starts reading at `lba` at time `now` (`CdRead2`). Frames not yet taken are dropped; a
    /// frame the game still holds stays valid.
    pub fn start(&mut self, lba: i32, now: u32, double_speed: bool) {
        self.drop_ready();
        self.reading = Some(Reading {
            next_lba: lba,
            t0: now + START_LATENCY,
            sector_cycles: if double_speed {
                SECTOR_CYCLES_2X
            } else {
                SECTOR_CYCLES_2X * 2
            },
            taken: 0,
            partial: None,
        });
    }

    /// Stops reading (`CdlPause`, or any other command that ends the stream).
    pub fn stop(&mut self) {
        self.reading = None;
    }

    pub fn is_reading(&self) -> bool {
        self.reading.is_some()
    }

    /// The sector the read would deliver next.
    pub fn next_lba(&self) -> Option<i32> {
        self.reading.as_ref().map(|r| r.next_lba)
    }

    fn drop_ready(&mut self) {
        while let Some(f) = self.ready.pop_front() {
            self.ring.free(f.off);
        }
    }

    /// The VBlank the next sector arrives at.
    pub fn next_arrival(&self) -> Option<u32> {
        let r = self.reading.as_ref()?;
        Some(arrival(r.t0, r.taken + 1, r.sector_cycles))
    }

    /// Takes the next sector from the drive. `Some(arrival, completed)`: when it arrived and
    /// whether it finished a frame into the ring. `None` when the read can't go on.
    fn step(
        &mut self,
        read: &mut dyn FnMut(i32, &mut [u8; RAW_BYTES]) -> bool,
    ) -> Option<(u32, bool)> {
        let r = self.reading.as_mut()?;
        let lba = r.next_lba;
        if !read(lba, &mut self.raw) {
            self.reading = None;
            return None;
        }
        r.next_lba += 1;
        r.taken += 1;
        let when = arrival(r.t0, r.taken, r.sector_cycles);
        let submode = self.raw[SUBMODE_OFFSET];
        let data = &self.raw[DATA_OFFSET..DATA_OFFSET + SECTOR_BYTES];
        let header = (submode & (SUBMODE_AUDIO | SUBMODE_FORM2) == 0)
            .then(|| StHeader::parse(data))
            .flatten();
        let Some(h) = header else {
            self.skipped += 1;
            return Some((when, false));
        };
        // Collect: sector 0 starts a frame; the rest must follow in order.
        if h.sector == 0 {
            let mut head = [0; HEADER_BYTES];
            head.copy_from_slice(&data[..HEADER_BYTES]);
            r.partial = Some(Partial {
                header: h,
                head,
                data: Vec::new(),
                first_lba: lba,
            });
        }
        let Some(p) = r.partial.as_mut() else {
            return Some((when, false)); // the middle of a frame whose start we never saw
        };
        if p.header.frame != h.frame || p.data.len() != usize::from(h.sector) * PAYLOAD_BYTES {
            r.partial = None; // a sector went missing or the frame changed: drop what we have
            return Some((when, false));
        }
        p.data.extend_from_slice(&data[HEADER_BYTES..]);
        if usize::from(h.sector) + 1 < usize::from(p.header.sectors) {
            return Some((when, false));
        }
        let p = r.partial.take()?;
        Some((when, self.complete(p, lba)))
    }

    /// A frame's last sector arrived: into the ring, or lost. True when it is now ready.
    fn complete(&mut self, p: Partial, last_lba: i32) -> bool {
        let h = p.header;
        if h.frame < self.start_frame || self.end_frame.is_some_and(|e| h.frame > e) {
            return false;
        }
        let want = h.size as usize;
        let data_len = if want > 0 && want <= p.data.len() {
            want
        } else {
            p.data.len()
        };
        let len = usize::from(h.sectors) * SECTOR_BYTES;
        let Some(off) = self.ring.alloc(len) else {
            self.lost_frames += 1;
            if !self.lost {
                self.lost = true;
                self.backloc = Some((p.first_lba, h.frame));
            }
            return false;
        };
        let bytes = self.storage_bytes_mut();
        bytes[off..off + HEADER_BYTES].copy_from_slice(&p.head);
        bytes[off + HEADER_BYTES..off + HEADER_BYTES + data_len]
            .copy_from_slice(&p.data[..data_len]);
        // Zero to a word boundary so the bitstream reads as whole words.
        let pad = data_len.next_multiple_of(4) - data_len;
        bytes[off + HEADER_BYTES + data_len..off + HEADER_BYTES + data_len + pad].fill(0);
        self.ready.push_back(Frame {
            header: h,
            off,
            data_len,
            last_lba,
        });
        true
    }

    fn storage_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.storage.len() * 4;
        // SAFETY: a u32 slice is valid as bytes; the length is the same in bytes.
        unsafe { std::slice::from_raw_parts_mut(self.storage.as_mut_ptr().cast(), len) }
    }

    /// Takes in every sector that has arrived by `now`.
    pub fn sync(&mut self, now: u32, read: &mut dyn FnMut(i32, &mut [u8; RAW_BYTES]) -> bool) {
        while self.next_arrival().is_some_and(|t| t <= now) {
            if self.step(read).is_none() {
                break;
            }
        }
    }

    /// `StGetNext`: the next whole frame, or `None` when none is on the way. When the frame
    /// hasn't arrived by `now`, runs ahead to its last sector and says when that is in
    /// `until`; the caller advances the clock there.
    ///
    /// No running ahead while the game holds a frame: whether a later frame fits the ring
    /// depends on when the game frees it, so the poll just reports nothing ready yet. (DW1's
    /// player frees each frame before polling again.)
    pub fn poll(
        &mut self,
        now: u32,
        read: &mut dyn FnMut(i32, &mut [u8; RAW_BYTES]) -> bool,
    ) -> Option<Delivered> {
        self.sync(now, read);
        if self.ready.is_empty() && !self.held.is_empty() {
            return None;
        }
        let mut until = now;
        let mut looked = 0;
        while self.ready.is_empty() {
            looked += 1;
            if looked > MAX_LOOKAHEAD {
                return None;
            }
            let (when, completed) = self.step(read)?;
            if completed {
                until = until.max(when);
            }
        }
        let f = self.ready.pop_front()?;
        // The resume point moves past this frame, unless an earlier frame is still lost.
        if !self.lost {
            self.backloc = Some((f.last_lba + 1, f.header.frame + 1));
        }
        self.held.push(f);
        let base = self.storage.as_ptr().cast::<u8>();
        // SAFETY: `off` is inside the storage.
        let (header, data) = unsafe { (base.add(f.off), base.add(f.off + HEADER_BYTES)) };
        Some(Delivered {
            header,
            data,
            frame: f.header.frame,
            until,
        })
    }

    fn held_at(&self, data: *const u8) -> Option<&Frame> {
        let base = self.storage.as_ptr() as usize;
        let at = (data as usize).checked_sub(base + HEADER_BYTES)?;
        self.held.iter().find(|f| f.off == at)
    }

    /// `StFreeRing` at VBlank `now`: takes in the sectors that arrived while the frame was
    /// held (they found the ring as it was), then gives back the frame whose bitstream starts
    /// at `data`. False if it isn't one handed out.
    pub fn free_at(
        &mut self,
        data: *const u8,
        now: u32,
        read: &mut dyn FnMut(i32, &mut [u8; RAW_BYTES]) -> bool,
    ) -> bool {
        self.sync(now, read);
        self.free(data)
    }

    /// Gives back the frame whose bitstream starts at `data`. False if it isn't one handed out.
    pub fn free(&mut self, data: *const u8) -> bool {
        let Some(&f) = self.held_at(data) else {
            return false;
        };
        self.held.retain(|h| h.off != f.off);
        self.ring.free(f.off);
        true
    }

    /// The bitstream length of a frame handed out, from its pointer.
    pub fn frame_len(&self, data: *const u8) -> Option<usize> {
        self.held_at(data).map(|f| f.data_len)
    }

    /// `StGetBackloc`: the first sector and number of the frame to resume from, `None` before
    /// any frame was seen.
    pub fn backloc(&self) -> Option<(i32, u32)> {
        self.backloc
    }

    /// A restart ends the loss: the resume point is whatever arrives next.
    pub fn clear_loss(&mut self) {
        self.lost = false;
    }
}

/// The VBlank sector `n` (1-based) arrives at.
fn arrival(t0: u32, n: u32, sector_cycles: u64) -> u32 {
    t0 + (u64::from(n) * sector_cycles).div_ceil(VBLANK_CYCLES) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A raw sector holding a video header and payload (or audio when `video` is None).
    fn sector(video: Option<(u16, u16, u32, u32, u8)>) -> [u8; RAW_BYTES] {
        let mut s = [0u8; RAW_BYTES];
        match video {
            Some((i, n, frame, size, fill)) => {
                s[SUBMODE_OFFSET] = 0x08;
                let d = &mut s[DATA_OFFSET..];
                d[0..2].copy_from_slice(&0x0160u16.to_le_bytes());
                d[2..4].copy_from_slice(&0x8001u16.to_le_bytes());
                d[4..6].copy_from_slice(&i.to_le_bytes());
                d[6..8].copy_from_slice(&n.to_le_bytes());
                d[8..12].copy_from_slice(&frame.to_le_bytes());
                d[12..16].copy_from_slice(&size.to_le_bytes());
                d[16..18].copy_from_slice(&320u16.to_le_bytes());
                d[18..20].copy_from_slice(&240u16.to_le_bytes());
                d[HEADER_BYTES..SECTOR_BYTES].fill(fill);
            }
            None => s[SUBMODE_OFFSET] = 0x64, // XA audio, form 2
        }
        s
    }

    /// A disc of `video` and audio sectors, read by LBA from 0.
    fn disc(sectors: Vec<[u8; RAW_BYTES]>) -> impl FnMut(i32, &mut [u8; RAW_BYTES]) -> bool {
        move |lba, out| match usize::try_from(lba).ok().and_then(|i| sectors.get(i)) {
            Some(s) => {
                *out = *s;
                true
            }
            None => false,
        }
    }

    /// Frames of 3 sectors (two video, one audio between) numbered from 1.
    fn movie(frames: u32) -> Vec<[u8; RAW_BYTES]> {
        let mut v = Vec::new();
        for f in 1..=frames {
            v.push(sector(Some((0, 2, f, 3000, f as u8))));
            v.push(sector(None));
            v.push(sector(Some((1, 2, f, 3000, 0x80 + f as u8))));
        }
        v
    }

    #[test]
    fn header_parses_the_documented_fields() {
        let s = sector(Some((1, 8, 77, 15_000, 0)));
        let h = StHeader::parse(&s[DATA_OFFSET..]).unwrap();
        assert_eq!(
            h,
            StHeader {
                sector: 1,
                sectors: 8,
                frame: 77,
                size: 15_000,
                width: 320,
                height: 240
            }
        );
        assert_eq!(StHeader::parse(&sector(None)[DATA_OFFSET..]), None);
    }

    #[test]
    fn sector_arrival_follows_the_drive_speed() {
        // 2.4996 sectors per VBlank at double speed: 150 sectors take 60 VBlanks and 6,000
        // cycles, so the last one arrives in the 61st.
        assert_eq!(arrival(0, 149, SECTOR_CYCLES_2X), 60);
        assert_eq!(arrival(0, 150, SECTOR_CYCLES_2X), 61);
        assert_eq!(arrival(10, 1, SECTOR_CYCLES_2X), 11);
        assert_eq!(arrival(10, 3, SECTOR_CYCLES_2X), 11 + 1);
        assert_eq!(arrival(0, 74, SECTOR_CYCLES_2X * 2), 60);
    }

    #[test]
    fn frames_assemble_across_audio_and_come_out_in_order() {
        let mut s = Stream::new(24);
        let mut read = disc(movie(3));
        s.start(0, 100, true);
        let d = s.poll(100, &mut read).unwrap();
        assert_eq!(d.frame, 1);
        // The frame's 3000 bytes: 2016 of 0x01 and 984 of 0x81, header kept beside it.
        let data = unsafe { std::slice::from_raw_parts(d.data, 3000) };
        assert!(data[..2016].iter().all(|&b| b == 1));
        assert!(data[2016..].iter().all(|&b| b == 0x81));
        assert_eq!(s.frame_len(d.data), Some(3000));
        let hdr = unsafe { std::slice::from_raw_parts(d.header, HEADER_BYTES) };
        assert_eq!(StHeader::parse(hdr).unwrap().frame, 1);
        // It arrived with its third sector: start latency plus ceil(3 * 225792 / 564398) = 2.
        assert_eq!(d.until, 100 + START_LATENCY + 2);
        assert!(s.free(d.data));
        assert!(!s.free(d.data), "freed twice");
        let d2 = s.poll(d.until, &mut read).unwrap();
        assert_eq!(d2.frame, 2);
        assert_eq!(s.skipped, 2);
    }

    #[test]
    fn a_frame_that_has_already_arrived_does_not_move_the_clock() {
        let mut s = Stream::new(24);
        let mut read = disc(movie(4));
        s.start(0, 0, true);
        let late = 1000;
        let d = s.poll(late, &mut read).unwrap();
        assert_eq!((d.frame, d.until), (1, late));
        s.free(d.data);
        // Frames 2-4 arrived long ago too; each comes out without waiting.
        for f in 2..=4 {
            let d = s.poll(late, &mut read).unwrap();
            assert_eq!((d.frame, d.until), (f, late));
            s.free(d.data);
        }
        // Past the end of the disc nothing more is coming, however often it is polled.
        assert!(s.poll(late, &mut read).is_none());
        assert!(!s.is_reading());
        assert!(s.poll(late, &mut read).is_none());
    }

    #[test]
    fn a_poll_before_arrival_reports_when_the_frame_comes() {
        let mut s = Stream::new(24);
        let mut read = disc(movie(40));
        s.start(0, 0, true);
        let mut now = 0;
        let mut times = Vec::new();
        for _ in 0..40 {
            let d = s.poll(now, &mut read).unwrap();
            assert!(d.until >= now);
            now = d.until;
            times.push(now);
            s.free(d.data);
        }
        // 3 sectors a frame at 2.4996 a VBlank: 120 sectors take 48.008 VBlanks (+ latency).
        assert_eq!(times[39], START_LATENCY + 49);
    }

    #[test]
    fn start_frame_skips_earlier_frames() {
        let mut s = Stream::new(24);
        s.set_range(3, None);
        let mut read = disc(movie(5));
        s.start(0, 0, true);
        assert_eq!(s.poll(0, &mut read).unwrap().frame, 3);
    }

    #[test]
    fn a_missing_sector_drops_the_frame() {
        let mut v = movie(3);
        v.remove(2); // frame 1 loses its second sector
        let mut s = Stream::new(24);
        let mut read = disc(v);
        s.start(0, 0, true);
        assert_eq!(s.poll(0, &mut read).unwrap().frame, 2);
    }

    #[test]
    fn a_full_ring_loses_frames_and_backloc_says_where_to_resume() {
        // 4-sector ring; every frame is 2 sectors, so only two fit unread.
        let mut s = Stream::new(4);
        let mut read = disc(movie(6));
        s.start(0, 0, true);
        s.sync(1000, &mut read); // frames 1-2 fit, 3-6 are lost
        assert_eq!(s.lost_frames, 4);
        assert_eq!(s.backloc(), Some((6, 3)), "frame 3 starts at sector 6");
        let d = s.poll(1000, &mut read).unwrap();
        assert_eq!(d.frame, 1);
        assert_eq!(s.backloc(), Some((6, 3)), "the loss stays the resume point");
        s.free(d.data);
        // Restart there (what the game does): frame 2, never taken, is dropped and the stream
        // carries on from frame 3.
        s.clear_loss();
        s.start(6, 1000, true);
        let d = s.poll(1000, &mut read).unwrap();
        assert_eq!(d.frame, 3);
        assert_eq!(s.backloc(), Some((9, 4)));
    }

    #[test]
    fn a_held_frame_blocks_lookahead_and_arrivals_while_held_find_the_ring_full() {
        // A 2-sector ring holds one frame. Frames arrive at VBlanks 6, 7, 8, 9, 11, 12, ...
        let mut s = Stream::new(2);
        let mut read = disc(movie(8));
        s.start(0, 0, true);
        let d = s.poll(0, &mut read).unwrap();
        assert_eq!((d.frame, d.until), (1, 6));
        // Polling again while frame 1 is held: nothing ready, nothing consumed ahead.
        assert!(s.poll(6, &mut read).is_none());
        assert_eq!(s.lost_frames, 0);
        // Freed at VBlank 6: frame 2 (VBlank 7) still fits.
        assert!(s.free_at(d.data, 6, &mut read));
        let d = s.poll(6, &mut read).unwrap();
        assert_eq!((d.frame, d.until), (2, 7));
        // Held until VBlank 12: frames 3-6 arrived meanwhile and were lost.
        assert!(s.free_at(d.data, 12, &mut read));
        assert_eq!(s.lost_frames, 4);
        assert_eq!(s.backloc(), Some((6, 3)));
    }

    #[test]
    fn ring_allocation_wraps_and_reclaims() {
        let mut r = Ring::new(10);
        assert_eq!(r.alloc(4), Some(0));
        assert_eq!(r.alloc(4), Some(4));
        assert_eq!(r.alloc(4), None);
        assert_eq!(r.alloc(2), Some(8));
        r.free(4); // out of order: nothing reclaimed yet
        assert_eq!(r.alloc(2), None);
        r.free(0); // 0..8 is free now
        assert_eq!(r.alloc(4), Some(0)); // wraps: 8..10 is used, 0..4 reused
        assert_eq!(r.alloc(5), None);
        assert_eq!(r.alloc(4), Some(4));
    }
}
