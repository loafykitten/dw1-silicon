//! libcd: the CD-ROM drive, as far as the game's file layer uses it (`CdInit`, `CdSearchFile`,
//! `CdControl`/`CdControlB`, `CdRead`, `CdReadSync`, `CdIntToPos`, `CdPosToInt`), plus the movie
//! streaming calls (`CdRead2`, `St*`), whose frame assembly and timing live in `stream`.
//!
//! How the game drives it (src/main/file.c, file_queue.c, script_interp.c, efe.c):
//! `CdControl(CdlSetloc, &pos, NULL)` until it returns non-zero, then `CdRead(sectors, buf,
//! CdlModeSpeed)` until non-zero, then `CdReadSync(0, NULL)` until it returns 0 (the file queue
//! polls `CdReadSync(1, NULL)` once per frame instead). Positions come from its own file table
//! (`CdIntToPos(sector)`) or from `CdSearchFile`. `CdControl(CdlSetmode, &mode, ...)` sets the
//! drive mode, and `CdRead`'s `mode` argument sets it too.
//!
//! **Timing:** every command completes at once and deterministically: `CdRead` copies its
//! sectors into the caller's buffer before returning and `CdReadSync` reports done. The
//! PS1's drive takes tens of milliseconds to seek and reads at 75 (or 150) sectors a second,
//! and the game's frames between a request and its completion (the file queue's states) depend
//! on that; a replay that has to stay in step with Redux's CD timing will need completions
//! scheduled on the VBlank clock instead (docs/phase2-plan.md, "Time").
//!
//! Rules are tagged `[doc]` (psx-spx's CDROM chapters, the API reference and header comments)
//! or `[obs]` (what the game's call sites show it relies on).

use std::ffi::{CStr, c_char};
use std::sync::Mutex;

use pack::{Disc, SECTOR_DATA};

use crate::stream::Stream;

/// `CdlLOC`: a position as BCD minute, second, frame, plus a track byte. [doc: libcd.h]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CdlLoc {
    pub minute: u8,
    pub second: u8,
    pub sector: u8,
    pub track: u8,
}

/// `CdlFILE`: what `CdSearchFile` fills in. [doc: libcd.h]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CdlFile {
    pub pos: CdlLoc,
    pub size: u32,
    pub name: [c_char; 16],
}

// Commands (CdControl's `com`). [doc: libcd.h]
pub const CDL_NOP: u8 = 0x01;
pub const CDL_SETLOC: u8 = 0x02;
pub const CDL_STANDBY: u8 = 0x07;
pub const CDL_STOP: u8 = 0x08;
pub const CDL_PAUSE: u8 = 0x09;
pub const CDL_MUTE: u8 = 0x0b;
pub const CDL_DEMUTE: u8 = 0x0c;
pub const CDL_SETMODE: u8 = 0x0e;

/// Mode bit 5: deliver 2340 bytes a sector (everything after the sync bytes) instead of 2048.
/// [doc: psx-spx "CDROM Control Commands", Setmode bit 5]
pub const CDL_MODE_SIZE1: u8 = 0x20;
/// Mode bit 7: double speed (150 sectors a second). [doc: psx-spx Setmode]
pub const CDL_MODE_SPEED: u8 = 0x80;
const WHOLE_SECTOR: usize = 2340;
/// Where the user data starts in a raw sector, and where the post-sync part does.
const DATA_OFFSET: usize = 24;
const SYNC_LEN: usize = 12;

/// The status byte every command and `CdReadSync` reports: motor on, nothing else. [doc:
/// psx-spx "CDROM Status Code"]
const STATUS_IDLE: u8 = 0x02;

/// The frames the lead-in adds to a position's MSF: LBA 0 is 00:02:00. [doc: psx-spx "CDROM
/// Disk Images"]
const LEAD_IN: i32 = 150;

/// `CdIntToPos` in Rust: LBA to a BCD position. Track is 0. [doc: API reference]
pub fn int_to_pos(lba: i32) -> CdlLoc {
    let frames = lba + LEAD_IN;
    let bcd = |v: i32| (((v / 10) << 4) | (v % 10)) as u8;
    CdlLoc {
        minute: bcd(frames / 75 / 60),
        second: bcd(frames / 75 % 60),
        sector: bcd(frames % 75),
        track: 0,
    }
}

/// `CdPosToInt` in Rust: a BCD position to LBA, or `None` when a byte isn't valid BCD.
pub fn pos_to_int(pos: CdlLoc) -> Option<i32> {
    let un_bcd =
        |b: u8| (b >> 4 < 10 && b & 15 < 10).then(|| i32::from(b >> 4) * 10 + i32::from(b & 15));
    let (m, s, f) = (
        un_bcd(pos.minute)?,
        un_bcd(pos.second)?,
        un_bcd(pos.sector)?,
    );
    Some((m * 60 + s) * 75 + f - LEAD_IN)
}

/// Why a drive operation failed; the C entry points turn these into a message and abort, since
/// a retry loop in the game (`while (CdRead(...) == 0)`) would spin forever.
#[derive(Debug, PartialEq, Eq)]
pub enum DriveError {
    BadPosition(CdlLoc),
    Unsupported(u8),
    Disc(String),
}

impl std::fmt::Display for DriveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriveError::BadPosition(p) => write!(
                f,
                "position {:02x}:{:02x}:{:02x} isn't valid BCD",
                p.minute, p.second, p.sector
            ),
            DriveError::Unsupported(c) => write!(f, "CdControl command {c:#04x} isn't implemented"),
            DriveError::Disc(m) => f.write_str(m),
        }
    }
}

/// The drive's state: the inserted disc, the mode and the read position.
pub struct Drive {
    disc: Disc,
    mode: u8,
    /// The sector the next read starts at (set by `Setloc`, advanced by reads).
    pos: i32,
    /// The movie stream, once `StSetRing` has made one.
    stream: Option<Stream>,
}

impl Drive {
    pub fn new(disc: Disc) -> Drive {
        Drive {
            disc,
            mode: 0,
            pos: 0,
            stream: None,
        }
    }

    /// `CdInit`: resets the drive to its power-on state (mode 0, at the start, not reading).
    /// The ring buffer the game set up for streaming stays [guess: `CdInit` is only called
    /// before a movie's `StSetRing`].
    pub fn reset(&mut self) {
        self.mode = 0;
        self.pos = 0;
        if let Some(s) = self.stream.as_mut() {
            s.stop();
        }
    }

    /// The movie stream, or a stop with a message when the game never called `StSetRing`.
    fn stream_mut(&mut self, call: &str) -> Result<&mut Stream, DriveError> {
        self.stream.as_mut().ok_or_else(|| {
            DriveError::Disc(format!("{call} before StSetRing: there is no ring buffer"))
        })
    }

    /// `StFreeRing` at `now`: the stream catches up with the sectors that arrived meanwhile,
    /// then the frame is freed. False if `base` isn't a frame handed out.
    pub fn free_stream_frame(&mut self, base: *const u8, now: u32) -> Result<bool, DriveError> {
        let Drive { disc, stream, .. } = self;
        let s = stream
            .as_mut()
            .ok_or_else(|| DriveError::Disc("StFreeRing before StSetRing".into()))?;
        let freed = s.free_at(base, now, &mut |lba, out| {
            u32::try_from(lba).is_ok_and(|lba| disc.read_raw(lba, out).is_ok())
        });
        if let Some(lba) = s.next_lba() {
            self.pos = lba;
        }
        Ok(freed)
    }

    /// `CdRead2(mode)`: sets the mode and starts streaming from the `Setloc` position, at `now`
    /// VBlanks. [doc: libcd; obs: MOV passes Stream|Speed|RT|Size1 after Setloc and Setmode]
    pub fn start_stream(&mut self, mode: u8, now: u32) -> Result<(), DriveError> {
        self.mode = mode;
        let (lba, double) = (self.pos, mode & CDL_MODE_SPEED != 0);
        let s = self.stream_mut("CdRead2")?;
        s.clear_loss();
        s.start(lba, now, double);
        Ok(())
    }

    /// `StGetNext`: the next movie frame, with the VBlank to advance the clock to first.
    pub fn poll_stream(
        &mut self,
        now: u32,
    ) -> Result<Option<crate::stream::Delivered>, DriveError> {
        let Drive { disc, stream, .. } = self;
        let s = stream
            .as_mut()
            .ok_or_else(|| DriveError::Disc("StGetNext before StSetRing".into()))?;
        let delivered = s.poll(now, &mut |lba, out| {
            u32::try_from(lba).is_ok_and(|lba| disc.read_raw(lba, out).is_ok())
        });
        if let Some(lba) = s.next_lba() {
            self.pos = lba;
        }
        Ok(delivered)
    }

    pub fn mode(&self) -> u8 {
        self.mode
    }

    pub fn position(&self) -> i32 {
        self.pos
    }

    /// `CdControl`: runs a command with its parameter bytes. Returns the status byte it reports.
    /// Only the commands the game's file layer issues are implemented; the rest need the
    /// movie/XA slices.
    pub fn command(&mut self, com: u8, param: &[u8]) -> Result<u8, DriveError> {
        match com {
            // [doc] Setloc takes minute, second, frame as BCD; the next read or seek uses it.
            CDL_SETLOC => {
                let loc = CdlLoc {
                    minute: param[0],
                    second: param[1],
                    sector: param[2],
                    track: 0,
                };
                self.pos = pos_to_int(loc).ok_or(DriveError::BadPosition(loc))?;
            }
            // [doc] Setmode takes one mode byte (the CdlMode* bits).
            CDL_SETMODE => self.mode = param[0],
            // [doc] These only change motor, audio or reading state, which a synchronous
            // drive with no audio doesn't have; they report status.
            // [doc] Pause (and Stop) end any read in progress, a movie stream included; the head
            // stays where it is.
            CDL_PAUSE | CDL_STOP => {
                if let Some(s) = self.stream.as_mut() {
                    s.stop();
                }
            }
            CDL_NOP | CDL_STANDBY | CDL_MUTE | CDL_DEMUTE => {}
            other => return Err(DriveError::Unsupported(other)),
        }
        Ok(STATUS_IDLE)
    }

    /// Bytes of parameter `com` reads, so the C wrapper knows how much to take from `param`.
    pub fn param_len(com: u8) -> usize {
        match com {
            CDL_SETLOC => 3,
            CDL_SETMODE => 1,
            _ => 0,
        }
    }

    /// The bytes one sector occupies in a read buffer under `mode` (`0` = the current mode).
    pub fn sector_size(&self, mode: u8) -> usize {
        let mode = if mode != 0 { mode } else { self.mode };
        if mode & CDL_MODE_SIZE1 != 0 {
            WHOLE_SECTOR
        } else {
            SECTOR_DATA
        }
    }

    /// `CdRead`: reads `sectors` sectors from the current position into `out` and advances the
    /// position. A nonzero `mode` replaces the drive's mode first [obs: the game passes
    /// `CdlModeSpeed` every time]. The sector size is 2048 bytes of user data, or 2340
    /// (everything after the sync bytes) with `CDL_MODE_SIZE1`. [doc: psx-spx Setmode]
    pub fn read(&mut self, sectors: usize, mode: u8, out: &mut [u8]) -> Result<(), DriveError> {
        if mode != 0 {
            self.mode = mode;
        }
        if let Some(s) = self.stream.as_mut() {
            s.stop(); // a data read replaces the stream
        }
        let size = self.sector_size(0);
        let start = if size == WHOLE_SECTOR {
            SYNC_LEN
        } else {
            DATA_OFFSET
        };
        let mut raw = [0u8; pack::RAW_SECTOR];
        for (i, chunk) in out.chunks_exact_mut(size).take(sectors).enumerate() {
            let lba = u32::try_from(self.pos + i as i32)
                .map_err(|_| DriveError::Disc(format!("read at negative sector {}", self.pos)))?;
            self.disc
                .read_raw(lba, &mut raw)
                .map_err(|e| DriveError::Disc(e.to_string()))?;
            chunk.copy_from_slice(&raw[start..start + size]);
        }
        self.pos += sectors as i32;
        Ok(())
    }

    /// `CdSearchFile`: the file at `path`, or `None`. Directories don't count.
    pub fn search(&self, path: &str) -> Result<Option<CdlFile>, DriveError> {
        let found = self
            .disc
            .find(path)
            .map_err(|e| DriveError::Disc(e.to_string()))?;
        Ok(found.filter(|e| !e.is_dir).map(|e| {
            let mut name = [0; 16];
            // [doc] `name` holds the file name as stored, in a 16-byte field.
            for (dst, src) in name.iter_mut().zip(e.name.bytes().take(15)) {
                *dst = src as c_char;
            }
            CdlFile {
                pos: int_to_pos(e.lba as i32),
                size: e.size,
                name,
            }
        }))
    }
}

static DRIVE: Mutex<Option<Drive>> = Mutex::new(None);

/// Puts a disc in the drive. The app calls this before running the game; until then every CD
/// call stops the program with a message.
pub fn insert_disc(disc: Disc) {
    *DRIVE.lock().unwrap() = Some(Drive::new(disc));
}

/// Runs `f` on the drive, or stops the program when there's no disc or `f` fails.
fn with_drive<T>(call: &str, f: impl FnOnce(&mut Drive) -> Result<T, DriveError>) -> T {
    let mut guard = DRIVE.lock().unwrap();
    let Some(drive) = guard.as_mut() else {
        eprintln!(
            "psyq: {call}: no disc inserted (the app opens it from DW1_DISC before the game runs)"
        );
        std::process::abort();
    };
    match f(drive) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("psyq: {call}: {e}");
            std::process::abort();
        }
    }
}

/// `CdInit()`: resets the drive; returns 1 for success. [doc: API reference]
#[unsafe(no_mangle)]
pub extern "C" fn CdInit() -> i32 {
    with_drive("CdInit", |d| {
        d.reset();
        Ok(1)
    })
}

/// `CdIntToPos(i, p)`: fills `*p` with the position of sector `i` and returns `p`.
///
/// # Safety
/// `p` must be valid for a `CdlLOC` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CdIntToPos(i: i32, p: *mut CdlLoc) -> *mut CdlLoc {
    unsafe { p.write(int_to_pos(i)) };
    p
}

/// `CdPosToInt(p)`: the sector a position names.
///
/// # Safety
/// `p` must point to a `CdlLOC`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CdPosToInt(p: *const CdlLoc) -> i32 {
    let pos = unsafe { p.read() };
    pos_to_int(pos).unwrap_or_else(|| {
        eprintln!(
            "psyq: CdPosToInt: {:02x}:{:02x}:{:02x} isn't valid BCD",
            pos.minute, pos.second, pos.sector
        );
        std::process::abort();
    })
}

/// `CdSearchFile(fp, name)`: looks `name` (`\DIR\FILE.EXT;1`) up on the disc; fills `*fp` and
/// returns it, or returns NULL when there's no such file. [doc: API reference]
///
/// # Safety
/// `fp` must be valid for a `CdlFILE` write and `name` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CdSearchFile(fp: *mut CdlFile, name: *const c_char) -> *mut CdlFile {
    let path = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    match with_drive("CdSearchFile", |d| d.search(&path)) {
        Some(file) => {
            unsafe { fp.write(file) };
            fp
        }
        None => std::ptr::null_mut(),
    }
}

/// Shared by `CdControl` and `CdControlB`: both finish at once here.
unsafe fn control(call: &str, com: u8, param: *const u8, result: *mut u8) -> i32 {
    let need = Drive::param_len(com);
    if need > 0 && param.is_null() {
        eprintln!("psyq: {call}: command {com:#04x} needs a parameter");
        std::process::abort();
    }
    let param = if need > 0 {
        unsafe { std::slice::from_raw_parts(param, need) }
    } else {
        &[]
    };
    let status = with_drive(call, |d| d.command(com, param));
    // [doc] `result` receives the command's result bytes; every command here answers with
    // the status byte.
    if !result.is_null() {
        unsafe { result.write(status) };
    }
    1
}

/// `CdControl(com, param, result)`: issues a command; returns 1 when it was accepted.
///
/// # Safety
/// `param` must point to the command's parameter bytes (or be NULL for a command with none),
/// `result` to at least one byte (or be NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CdControl(com: u8, param: *const u8, result: *mut u8) -> i32 {
    unsafe { control("CdControl", com, param, result) }
}

/// `CdControlB(com, param, result)`: `CdControl` that waits for the command to finish; the
/// same thing here, where commands finish at once.
///
/// # Safety
/// As [`CdControl`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CdControlB(com: u8, param: *const u8, result: *mut u8) -> i32 {
    unsafe { control("CdControlB", com, param, result) }
}

/// `CdRead(sectors, buf, mode)`: reads `sectors` sectors from the position `Setloc` set into
/// `buf`; returns 1 when the read was started. Here the data is in `buf` on return.
///
/// # Safety
/// `buf` must have room for `sectors` sectors of the size the mode selects (2048 bytes, or
/// 2340 with `CdlModeSize1`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CdRead(sectors: i32, buf: *mut u8, mode: i32) -> i32 {
    let sectors = usize::try_from(sectors).unwrap_or(0);
    with_drive("CdRead", |d| {
        let mode = mode as u8;
        let out = unsafe { std::slice::from_raw_parts_mut(buf, sectors * d.sector_size(mode)) };
        d.read(sectors, mode, out)?;
        Ok(1)
    })
}

/// `CdReadSync(mode, result)`: 0 when the last read has finished (always, here), else the
/// sectors still to read, or -1 on an error. `result` receives the status byte. [doc: API
/// reference]
///
/// # Safety
/// `result` must be NULL or point to at least one byte.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CdReadSync(_mode: i32, result: *mut u8) -> i32 {
    with_drive("CdReadSync", |_| Ok(()));
    if !result.is_null() {
        unsafe { result.write(STATUS_IDLE) };
    }
    0
}

// ---- movie streaming ----
//
// How MOV drives it (src/mov/mov.c): `MOV_startStream` calls `StSetRing(ring, 24)` and
// `StSetStream(1, 1, -1, NULL, NULL)`, then `MOV_startRead` issues Setloc, Setmode and, after
// `VSync(3)`, `CdRead2(Stream|Speed|RT|Size1)`, repeating until it returns 1. `MOV_readFrame`
// polls `StGetNext(&bitstream, &header)` until it returns 0 (at most 2000 times), reads the
// header's frame number, width and height, and `MOV_decodeFrame` frees the bitstream with
// `StFreeRing` right after `DecDCTvlc2`. After a timeout `StGetBackloc(&loc)` says where to
// restart. `MOV_playMovie` ends with `StUnSetRing` and `CdControlB(CdlPause)`.

/// `CdRead2(mode)`: starts reading the sectors from the `Setloc` position as a stream, with
/// `mode` (stream, double speed, whole sectors...); returns 1 when it started. [doc: libcd]
#[unsafe(no_mangle)]
pub extern "C" fn CdRead2(mode: i32) -> i32 {
    let now = crate::libetc::vblank_count();
    with_drive("CdRead2", |d| {
        // The SDK's CdlModeStream (0x100) is above the drive's mode byte.
        d.start_stream(mode as u8, now)?;
        Ok(1)
    })
}

/// `StSetRing(ring, sectors)`: gives the stream `sectors` sectors of ring buffer. Natively the
/// frames are assembled in storage the stream owns, so `ring` is not touched. [doc: libcd]
///
/// # Safety
/// `ring` is not dereferenced.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn StSetRing(_ring: *mut u32, sectors: u32) {
    with_drive("StSetRing", |d| {
        d.stream = Some(Stream::new(sectors as usize));
        Ok(())
    });
}

/// `StSetStream(mode, start_frame, end_frame, func1, func2)`: which frames to deliver. `mode`
/// (24 or 16 bit MDEC output) and the two callbacks (ring overflow and end of stream) only
/// matter to the SDK's own decoding path, which the game doesn't use; `end_frame` -1 means no
/// end. [doc: libcd]
#[unsafe(no_mangle)]
pub extern "C" fn StSetStream(
    _mode: u32,
    start_frame: u32,
    end_frame: u32,
    _func1: Option<unsafe extern "C" fn()>,
    _func2: Option<unsafe extern "C" fn()>,
) {
    with_drive("StSetStream", |d| {
        let end = (end_frame != u32::MAX).then_some(end_frame);
        d.stream_mut("StSetStream")?.set_range(start_frame, end);
        Ok(())
    });
}

/// `StUnSetRing()`: ends the stream and frees the ring. [doc: libcd]
#[unsafe(no_mangle)]
pub extern "C" fn StUnSetRing() {
    with_drive("StUnSetRing", |d| {
        d.stream = None;
        Ok(())
    });
}

/// `StGetNext(&bitstream, &header)`: 0 and the next frame's pointers, or 1 when none is ready.
///
/// Frames arrive on the virtual clock (`stream`): when none has arrived yet, the clock runs on
/// to the VBlank its last sector arrives at (host VBlank callbacks included), so the caller's
/// poll loop finds it on the first try; returns 1 only when no frame is coming (the drive
/// stopped, or the read left the disc). [doc: libcd]
///
/// # Safety
/// `addr` and `header` must be valid for a pointer write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn StGetNext(addr: *mut *const u8, header: *mut *const u8) -> i32 {
    let now = crate::libetc::vblank_count();
    // The clock advances after the drive lock is released: host callbacks run in there.
    match with_drive("StGetNext", |d| d.poll_stream(now)) {
        Some(f) => {
            crate::libetc::advance_to(f.until);
            unsafe {
                addr.write(f.data);
                header.write(f.header);
            }
            0
        }
        None => 1,
    }
}

/// `StFreeRing(bitstream)`: gives a frame's space in the ring back. [doc: libcd]
///
/// # Safety
/// `base` must be a pointer `StGetNext` returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn StFreeRing(base: *const u8) {
    let now = crate::libetc::vblank_count();
    with_drive("StFreeRing", |d| {
        if !d.free_stream_frame(base, now)? {
            eprintln!("psyq: StFreeRing: {base:p} isn't a frame StGetNext handed out");
            std::process::abort();
        }
        Ok(())
    });
}

/// `StGetBackloc(&loc)`: where to restart after a loss, as the position of the first sector of
/// the first frame the game has not received, and that frame's number; 0 (and `loc` untouched)
/// before any frame arrived. [doc: libcd, "frame number to resume from"]
///
/// # Safety
/// `loc` must be valid for a `CdlLOC` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn StGetBackloc(loc: *mut CdlLoc) -> i32 {
    match with_drive("StGetBackloc", |d| {
        Ok(d.stream_mut("StGetBackloc")?.backloc())
    }) {
        Some((lba, frame)) => {
            unsafe { loc.write(int_to_pos(lba)) };
            frame as i32
        }
        None => 0,
    }
}

/// `StCdInterrupt()`: processes a sector-arrival interrupt the library had to defer; the game
/// calls it from the MDEC callback when `StCdIntrFlag` is set. Sectors are taken in when a call
/// looks at the stream and `StCdIntrFlag` stays 0, so there is nothing to do. [guess]
#[unsafe(no_mangle)]
pub extern "C" fn StCdInterrupt() {}

/// The length in bytes of a frame `StGetNext` handed out, from its bitstream pointer, for
/// `DecDCTvlc2` (which gets only the pointer).
pub fn stream_frame_len(data: *const u8) -> Option<usize> {
    let guard = DRIVE.lock().unwrap();
    guard.as_ref()?.stream.as_ref()?.frame_len(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pack::testing::synthetic_image;

    fn drive() -> Drive {
        Drive::new(Disc::from_memory(synthetic_image()).unwrap())
    }

    fn msf(lba: i32) -> [u8; 3] {
        let p = int_to_pos(lba);
        [p.minute, p.second, p.sector]
    }

    #[test]
    fn positions_convert_both_ways() {
        // [doc] LBA 0 is 00:02:00 (the lead-in); positions are BCD.
        let p = int_to_pos(0);
        assert_eq!((p.minute, p.second, p.sector, p.track), (0, 2, 0, 0));
        let p = int_to_pos(75 * 60 * 12 + 75 * 7 + 33 - 150);
        assert_eq!((p.minute, p.second, p.sector), (0x12, 0x07, 0x33));
        for lba in [0, 1, 74, 75, 4499, 4500, 20_000, 333_000] {
            assert_eq!(pos_to_int(int_to_pos(lba)), Some(lba), "{lba}");
        }
        let bad = CdlLoc {
            second: 0x1a,
            ..CdlLoc::default()
        };
        assert_eq!(pos_to_int(bad), None);
    }

    #[test]
    fn setloc_then_read_copies_sectors() {
        let mut d = drive();
        assert_eq!(d.command(CDL_SETLOC, &msf(20)), Ok(STATUS_IDLE));
        assert_eq!(d.position(), 20);
        let mut out = vec![0u8; 2 * SECTOR_DATA];
        d.read(2, 0x80, &mut out).unwrap();
        assert_eq!(d.mode(), 0x80);
        assert_eq!(d.position(), 22);
        assert!(out[..SECTOR_DATA].iter().all(|&b| b == 0x10));
        assert!(out[SECTOR_DATA..].iter().all(|&b| b == 0x11));
        // Reading carries on from where the last read stopped.
        let mut next = vec![0u8; SECTOR_DATA];
        d.read(1, 0, &mut next).unwrap();
        assert!(next.iter().all(|&b| b == 0x12));
        assert_eq!(d.mode(), 0x80, "mode 0 keeps the current mode");
    }

    #[test]
    fn whole_sector_mode_starts_after_the_sync_bytes() {
        let mut d = drive();
        d.command(CDL_SETMODE, &[CDL_MODE_SIZE1]).unwrap();
        d.command(CDL_SETLOC, &msf(23)).unwrap();
        let mut out = vec![0u8; WHOLE_SECTOR];
        d.read(1, 0, &mut out).unwrap();
        // The 2340 bytes start 12 bytes into the raw sector, so the user data (at 24) is 12 in.
        assert_eq!(&out[12..22], b"0123456789");
    }

    #[test]
    fn errors_are_reported_not_swallowed() {
        let mut d = drive();
        let bad = CdlLoc {
            second: 0x6a,
            ..CdlLoc::default()
        };
        assert_eq!(
            d.command(CDL_SETLOC, &[0, 0x6a, 0]),
            Err(DriveError::BadPosition(bad))
        );
        assert_eq!(d.command(0x03, &[]), Err(DriveError::Unsupported(0x03)));
        d.command(CDL_SETLOC, &msf(24)).unwrap(); // past the end of the 24-sector image
        let mut out = vec![0u8; SECTOR_DATA];
        assert!(matches!(d.read(1, 0, &mut out), Err(DriveError::Disc(_))));
    }

    #[test]
    fn search_fills_position_size_and_name() {
        let d = drive();
        let f = d.search("\\sub\\b.bin;1").unwrap().unwrap();
        assert_eq!(f.pos, int_to_pos(23));
        assert_eq!(f.size, 10);
        let name: Vec<u8> = f
            .name
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect();
        assert_eq!(name, b"B.BIN;1");
        assert_eq!(d.search("\\NOPE;1").unwrap(), None);
        assert_eq!(d.search("\\SUB").unwrap(), None, "a directory isn't a file");
    }

    #[test]
    fn the_c_interface_reads_a_file() {
        // The only unit test touching the global drive.
        insert_disc(Disc::from_memory(synthetic_image()).unwrap());
        assert_eq!(CdInit(), 1);
        let mut file = CdlFile {
            pos: CdlLoc::default(),
            size: 0,
            name: [0; 16],
        };
        let found = unsafe { CdSearchFile(&mut file, c"\\A.TXT;1".as_ptr()) };
        assert_eq!(found, &raw mut file);
        assert_eq!(file.size, 5000);
        assert_eq!(unsafe { CdPosToInt(&file.pos) }, 20);
        let mut status = 0u8;
        let loc = file.pos;
        let issued = unsafe { CdControl(CDL_SETLOC, (&raw const loc).cast(), &mut status) };
        assert_eq!((issued, status), (1, STATUS_IDLE));
        let mut buf = vec![0u8; 3 * SECTOR_DATA];
        assert_eq!(unsafe { CdRead(3, buf.as_mut_ptr(), 0x80) }, 1);
        assert_eq!(unsafe { CdReadSync(0, std::ptr::null_mut()) }, 0);
        assert_eq!((buf[0], buf[2048], buf[4096]), (0x10, 0x11, 0x12));
        let mut pos = CdlLoc::default();
        assert_eq!(unsafe { CdIntToPos(23, &mut pos) }, &raw mut pos);
        assert_eq!(pos, int_to_pos(23));
        assert!(unsafe { CdSearchFile(&mut file, c"\\NOPE;1".as_ptr()) }.is_null());
    }
}
