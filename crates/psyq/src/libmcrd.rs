//! libmcrd: the memory cards.
//!
//! Two parts: [`Card`], a 128 KB card image with the PS1 card's file system, and the library's
//! command interface (`MemCardExist`, `MemCardAccept`, `MemCardReadFile`, ...) over two slots.
//!
//! Rules are marked `[doc]` (a public document), `[obs]` (observed in the oracle, the game's
//! own use, or the fresh card PCSX-Redux writes) or `[guess]` (to be settled by measuring the
//! emulator).
//!
//! Sources: Psy-Q's `libmcrd.h` and `kernel.h` (McFunc*/McErr* constants, `struct DIRENTRY`),
//! psx-spx "Memory Card Data Format" (the 128 KB layout), and how `main_menu.c` drives it.
//! Card writes stay in memory; Phase 7 adds persistence.
//!
//! # How the game drives it
//!
//! - Channels are the controller-port numbers: 0x00 is slot 1, 0x10 is slot 2 [doc: libmcrd
//!   `chan`; obs: `CHECKED_MEMORY_CARD ^= 0x10`].
//! - Asynchronous commands: `MemCardExist`, `MemCardAccept`, `MemCardReadFile`,
//!   `MemCardWriteFile`. Each returns 1 if started and 0 if not (a command is already running,
//!   or bad arguments) [obs: the game tests `== 1` after Read/Write and `== 0` after
//!   Exist/Accept]. `MemCardSync(mode, &cmd, &result)` then returns -1 with nothing pending, 0
//!   while running and 1 when a command finished, storing its McFunc code and McErr result
//!   (mode 1 polls; mode 0 waits) [obs: `updateMemoryCardState`].
//! - `MemCardCreateFile`, `MemCardDeleteFile`, `MemCardFormat` and `MemCardGetDirentry` return
//!   their McErr code directly (0 = ok); the game always calls `MemCardSync(0)` first
//!   [obs].

use std::ffi::{CStr, c_char};
use std::sync::Mutex;

use crate::libetc;

// ---- Constants (libmcrd.h) ----

pub const MC_FUNC_EXIST: u32 = 1;
pub const MC_FUNC_ACCEPT: u32 = 2;
pub const MC_FUNC_READ_FILE: u32 = 3;
pub const MC_FUNC_WRITE_FILE: u32 = 4;

pub const MC_ERR_NONE: i32 = 0;
pub const MC_ERR_CARD_NOT_EXIST: i32 = 1;
pub const MC_ERR_CARD_INVALID: i32 = 2;
pub const MC_ERR_NEW_CARD: i32 = 3;
pub const MC_ERR_NOT_FORMAT: i32 = 4;
pub const MC_ERR_FILE_NOT_EXIST: i32 = 5;
pub const MC_ERR_ALREADY_EXIST: i32 = 6;
pub const MC_ERR_BLOCK_FULL: i32 = 7;

/// VBlanks an asynchronous command takes from start to done. `[guess]` until measured on the
/// emulator (how many VBlanks pass between the game starting `MemCardExist`/`Accept`/
/// `ReadFile`/`WriteFile` and `MemCardSync(1)` first returning 1).
pub const COMMAND_VBLANKS: u32 = 1;

// ---- The card image ----

/// Bytes per frame (sector) [doc: psx-spx].
pub const FRAME: usize = 128;
/// Bytes per block [doc: psx-spx].
pub const BLOCK: usize = 8192;
/// Usable blocks [doc: psx-spx].
pub const BLOCKS: usize = 15;
/// Size of a card image.
pub const CARD_SIZE: usize = 128 * 1024;

const FREE: u8 = 0xA0;
const FIRST: u8 = 0x51;
const MIDDLE: u8 = 0x52;
const LAST: u8 = 0x53;
const NO_NEXT: u16 = 0xFFFF;
const NAME_LEN: usize = 20;

/// A 128 KB memory card image and its file system [doc: psx-spx "Memory Card Data Format"].
///
/// Frame 0 is the header (`"MC"`), frames 1-15 the directory (one per data block), frames
/// 16-35 the broken-sector list, frame 63 a write-test copy of the header; data block `n`
/// (1-15) starts at byte `n * 8192`. Every frame above ends with an XOR of its other 127
/// bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    image: Box<[u8; CARD_SIZE]>,
}

/// A file on the card, as the directory lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    /// The name without its zero padding.
    pub name: Vec<u8>,
    /// Size in bytes: 8192 per block [doc: psx-spx].
    pub size: u32,
    /// The file's blocks (0-based, 0..15) in chain order.
    pub blocks: Vec<usize>,
}

fn frame_checksum(frame: &[u8]) -> u8 {
    frame[..FRAME - 1].iter().fold(0, |a, b| a ^ b)
}

fn seal(frame: &mut [u8]) {
    frame[FRAME - 1] = frame_checksum(frame);
}

/// Whether a card name pattern matches a stored name: `?` matches one character and `*` the
/// rest [guess: the BIOS's `firstfile` wildcards; the game only uses a trailing `*`].
fn name_matches(pattern: &[u8], name: &[u8]) -> bool {
    for (i, &p) in pattern.iter().enumerate() {
        match p {
            b'*' => return true,
            b'?' if i < name.len() => {}
            _ if name.get(i) == Some(&p) => {}
            _ => return false,
        }
    }
    pattern.len() >= name.len()
}

impl Card {
    /// A freshly formatted, empty card: byte for byte what PCSX-Redux creates [obs: SHA-1
    /// 1b3c99be55273085e14b30983b636529b6dd7f46].
    pub fn formatted() -> Card {
        let mut image = Box::new([0u8; CARD_SIZE]);
        let mut header = [0u8; FRAME];
        header[..2].copy_from_slice(b"MC");
        seal(&mut header);
        image[..FRAME].copy_from_slice(&header);
        image[63 * FRAME..64 * FRAME].copy_from_slice(&header);
        for f in 1..=BLOCKS {
            let frame = &mut image[f * FRAME..(f + 1) * FRAME];
            frame[0] = FREE;
            frame[8..10].copy_from_slice(&NO_NEXT.to_le_bytes());
            seal(frame);
        }
        for f in 16..36 {
            let frame = &mut image[f * FRAME..(f + 1) * FRAME];
            frame[..4].copy_from_slice(&[0xFF; 4]);
            frame[8..10].copy_from_slice(&NO_NEXT.to_le_bytes());
            seal(frame);
        }
        Card { image }
    }

    /// A card from a 128 KB image (a `.mcd`/`.mcr` file); `None` if it isn't that size.
    /// The contents are not checked: an unformatted image is a valid unformatted card.
    pub fn from_bytes(bytes: &[u8]) -> Option<Card> {
        let image: Box<[u8; CARD_SIZE]> = bytes.to_vec().into_boxed_slice().try_into().ok()?;
        Some(Card { image })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.image[..]
    }

    /// Whether the header says the card is formatted [doc: psx-spx: "MC"]. [guess] that the
    /// library checks nothing else (the header checksum, the directory).
    pub fn is_formatted(&self) -> bool {
        &self.image[..2] == b"MC"
    }

    /// Erases everything to the fresh state.
    pub fn format(&mut self) {
        *self = Card::formatted();
    }

    fn dir_frame(&self, block: usize) -> &[u8] {
        &self.image[(block + 1) * FRAME..(block + 2) * FRAME]
    }

    fn dir_frame_mut(&mut self, block: usize) -> &mut [u8] {
        &mut self.image[(block + 1) * FRAME..(block + 2) * FRAME]
    }

    fn state(&self, block: usize) -> u8 {
        self.dir_frame(block)[0]
    }

    fn next(&self, block: usize) -> u16 {
        let f = self.dir_frame(block);
        u16::from_le_bytes([f[8], f[9]])
    }

    /// The chain from a first block, following next links; stops at the end, a bad link or a
    /// loop.
    fn chain(&self, first: usize) -> Vec<usize> {
        let mut blocks = vec![first];
        let mut cur = first;
        while blocks.len() < BLOCKS {
            let n = self.next(cur) as usize;
            if n >= BLOCKS || blocks.contains(&n) {
                break;
            }
            blocks.push(n);
            cur = n;
        }
        blocks
    }

    /// The files in directory order (by first block) [doc: psx-spx: state 0x51 starts a file].
    pub fn files(&self) -> Vec<FileInfo> {
        (0..BLOCKS)
            .filter(|&b| self.state(b) == FIRST)
            .map(|b| {
                let f = self.dir_frame(b);
                let size = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
                let raw = &f[10..10 + NAME_LEN];
                let end = raw.iter().position(|&c| c == 0).unwrap_or(NAME_LEN);
                FileInfo {
                    name: raw[..end].to_vec(),
                    size,
                    blocks: self.chain(b),
                }
            })
            .collect()
    }

    /// Files whose names match a pattern (`*` and `?`).
    pub fn find(&self, pattern: &[u8]) -> Vec<FileInfo> {
        self.files()
            .into_iter()
            .filter(|f| name_matches(pattern, &f.name))
            .collect()
    }

    fn find_exact(&self, name: &[u8]) -> Option<FileInfo> {
        let name = &name[..name.len().min(NAME_LEN)];
        self.files().into_iter().find(|f| f.name == name)
    }

    /// Blocks not used by a file (never used, or deleted) [doc: psx-spx: 0xA0-0xA3].
    pub fn free_blocks(&self) -> usize {
        (0..BLOCKS).filter(|&b| self.state(b) >> 4 == 0xA).count()
    }

    /// Creates a file of `blocks` blocks (the name is cut to 20 bytes). The data blocks are
    /// left as they were [guess].
    pub fn create(&mut self, name: &[u8], blocks: usize) -> Result<(), i32> {
        let name = &name[..name.len().min(NAME_LEN)];
        if !self.is_formatted() {
            return Err(MC_ERR_NOT_FORMAT);
        }
        if self.find_exact(name).is_some() {
            return Err(MC_ERR_ALREADY_EXIST);
        }
        if blocks == 0 || blocks > self.free_blocks() {
            return Err(MC_ERR_BLOCK_FULL);
        }
        let used: Vec<usize> = (0..BLOCKS)
            .filter(|&b| self.state(b) >> 4 == 0xA)
            .take(blocks)
            .collect();
        for (i, &b) in used.iter().enumerate() {
            let last = i + 1 == used.len();
            let f = self.dir_frame_mut(b);
            f.fill(0);
            f[0] = match (i, last) {
                (0, _) => FIRST,
                (_, false) => MIDDLE,
                (_, true) => LAST,
            };
            if i == 0 {
                f[4..8].copy_from_slice(&((blocks * BLOCK) as u32).to_le_bytes());
                f[10..10 + name.len()].copy_from_slice(name);
            }
            let next = if last { NO_NEXT } else { used[i + 1] as u16 };
            f[8..10].copy_from_slice(&next.to_le_bytes());
            seal(f);
        }
        Ok(())
    }

    /// Deletes a file: its blocks go to the deleted states (0xA1-0xA3) and are free again
    /// [doc: psx-spx].
    pub fn delete(&mut self, name: &[u8]) -> Result<(), i32> {
        if !self.is_formatted() {
            return Err(MC_ERR_NOT_FORMAT);
        }
        let file = self.find_exact(name).ok_or(MC_ERR_FILE_NOT_EXIST)?;
        for b in file.blocks {
            let f = self.dir_frame_mut(b);
            f[0] = 0xA0 | (f[0] & 0x0F);
            seal(f);
        }
        Ok(())
    }

    /// The card image ranges of a file's data for `[ofs, ofs + len)`.
    fn spans(file: &FileInfo, ofs: usize, len: usize) -> Option<Vec<(usize, usize)>> {
        if !ofs.is_multiple_of(FRAME)
            || !len.is_multiple_of(FRAME)
            || ofs + len > file.size as usize
        {
            return None;
        }
        let mut spans = Vec::new();
        let (mut pos, end) = (ofs, ofs + len);
        while pos < end {
            let block = *file.blocks.get(pos / BLOCK)?;
            let within = pos % BLOCK;
            let n = (BLOCK - within).min(end - pos);
            spans.push(((block + 1) * BLOCK + within, n));
            pos += n;
        }
        Some(spans)
    }

    /// Reads `buf.len()` bytes at `ofs` in the file. `ofs` and the length must be multiples
    /// of 128 [doc: Psy-Q `MemCardReadFile`] and inside the file [guess].
    pub fn read(&self, name: &[u8], ofs: usize, buf: &mut [u8]) -> Result<(), i32> {
        if !self.is_formatted() {
            return Err(MC_ERR_NOT_FORMAT);
        }
        let file = self.find_exact(name).ok_or(MC_ERR_FILE_NOT_EXIST)?;
        let spans = Self::spans(&file, ofs, buf.len()).ok_or(MC_ERR_CARD_INVALID)?;
        let mut at = 0;
        for (src, n) in spans {
            buf[at..at + n].copy_from_slice(&self.image[src..src + n]);
            at += n;
        }
        Ok(())
    }

    /// Writes `data` at `ofs` in the file; same rules as [`Card::read`].
    pub fn write(&mut self, name: &[u8], ofs: usize, data: &[u8]) -> Result<(), i32> {
        if !self.is_formatted() {
            return Err(MC_ERR_NOT_FORMAT);
        }
        let file = self.find_exact(name).ok_or(MC_ERR_FILE_NOT_EXIST)?;
        let spans = Self::spans(&file, ofs, data.len()).ok_or(MC_ERR_CARD_INVALID)?;
        let mut at = 0;
        for (dst, n) in spans {
            self.image[dst..dst + n].copy_from_slice(&data[at..at + n]);
            at += n;
        }
        Ok(())
    }
}

// ---- The command interface ----

/// `struct DIRENTRY` (`kernel.h`), with the native pointer: the layout `decomp-c` checks
/// against the C header. `name` holds the file name, `size` its bytes; the game reads nothing
/// else.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DirEntry {
    pub name: [c_char; 20],
    pub attr: i32,
    pub size: i32,
    pub next: *mut DirEntry,
    pub head: i32,
    pub system: [c_char; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Idle,
    /// Running until this VBlank.
    Busy {
        until: u32,
    },
    Done,
}

struct Slot {
    card: Option<Card>,
    /// Set when a card is inserted, cleared when `MemCardAccept` sees it: the "new card"
    /// result [guess: which calls clear it].
    new_card: bool,
}

/// The library state: two slots and the one command in flight.
pub struct Controller {
    slots: [Slot; 2],
    phase: Phase,
    cmd: u32,
    result: u32,
}

/// What `MemCardSync` returns [obs: the game's switch].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncState {
    /// Nothing pending (-1).
    Idle,
    /// A command is running (0).
    Busy,
    /// A command finished (1) with this McFunc code and McErr result.
    Done { cmd: u32, result: u32 },
}

impl Controller {
    /// Two freshly formatted cards, like the oracle emulator's [obs].
    pub fn new() -> Controller {
        let slot = || Slot {
            card: Some(Card::formatted()),
            new_card: true,
        };
        Controller {
            slots: [slot(), slot()],
            phase: Phase::Idle,
            cmd: 0,
            result: 0,
        }
    }

    fn slot_of(chan: i32) -> Option<usize> {
        match chan {
            0x00 => Some(0),
            0x10 => Some(1),
            _ => None,
        }
    }

    /// Settles a command whose time has come, so `phase` is current.
    fn update(&mut self, now: u32) {
        if let Phase::Busy { until } = self.phase
            && now.wrapping_sub(until) < 0x8000_0000
        {
            self.phase = Phase::Done;
        }
    }

    pub fn insert(&mut self, slot: usize, card: Option<Card>) {
        self.slots[slot] = Slot {
            new_card: card.is_some(),
            card,
        };
    }

    /// Starts an asynchronous command; false if one is running [guess: starting while busy
    /// fails; starting after a finished-but-unpolled one replaces it].
    fn start(&mut self, now: u32, cmd: u32, result: i32) -> bool {
        self.update(now);
        if matches!(self.phase, Phase::Busy { .. }) {
            return false;
        }
        self.cmd = cmd;
        self.result = result as u32;
        self.phase = Phase::Busy {
            until: now.wrapping_add(COMMAND_VBLANKS),
        };
        true
    }

    fn busy(&mut self, now: u32) -> bool {
        self.update(now);
        matches!(self.phase, Phase::Busy { .. })
    }

    /// `MemCardExist`: is a card there? Result 1 no card, 3 a new card, else 0 [doc: McErr
    /// codes; guess: Exist reports the new-card flag without clearing it].
    pub fn exist(&mut self, now: u32, chan: i32) -> bool {
        let Some(s) = Self::slot_of(chan) else {
            return false;
        };
        if self.busy(now) {
            return false;
        }
        let r = match &self.slots[s].card {
            None => MC_ERR_CARD_NOT_EXIST,
            Some(_) if self.slots[s].new_card => MC_ERR_NEW_CARD,
            Some(_) => MC_ERR_NONE,
        };
        self.start(now, MC_FUNC_EXIST, r)
    }

    /// `MemCardAccept`: checks the card is usable. Result 1 no card, 3 new card (once), 4
    /// unformatted, else 0 [obs: the game treats 3 like 0 and 4 as "unformatted"].
    pub fn accept(&mut self, now: u32, chan: i32) -> bool {
        let Some(s) = Self::slot_of(chan) else {
            return false;
        };
        if self.busy(now) {
            return false;
        }
        let slot = &mut self.slots[s];
        let r = match &slot.card {
            None => MC_ERR_CARD_NOT_EXIST,
            Some(_) if slot.new_card => {
                slot.new_card = false;
                MC_ERR_NEW_CARD
            }
            Some(c) if !c.is_formatted() => MC_ERR_NOT_FORMAT,
            Some(_) => MC_ERR_NONE,
        };
        self.start(now, MC_FUNC_ACCEPT, r)
    }

    /// `MemCardReadFile`. False if busy, the channel is bad or the range isn't valid.
    pub fn read_file(
        &mut self,
        now: u32,
        chan: i32,
        name: &[u8],
        ofs: usize,
        buf: &mut [u8],
    ) -> bool {
        let Some(s) = Self::slot_of(chan) else {
            return false;
        };
        if !ofs.is_multiple_of(FRAME) || !buf.len().is_multiple_of(FRAME) || self.busy(now) {
            return false;
        }
        let r = match &self.slots[s].card {
            None => MC_ERR_CARD_NOT_EXIST,
            Some(c) => c.read(name, ofs, buf).err().unwrap_or(MC_ERR_NONE),
        };
        self.start(now, MC_FUNC_READ_FILE, r)
    }

    /// `MemCardWriteFile`; as [`Controller::read_file`].
    pub fn write_file(
        &mut self,
        now: u32,
        chan: i32,
        name: &[u8],
        ofs: usize,
        data: &[u8],
    ) -> bool {
        let Some(s) = Self::slot_of(chan) else {
            return false;
        };
        if !ofs.is_multiple_of(FRAME) || !data.len().is_multiple_of(FRAME) || self.busy(now) {
            return false;
        }
        let r = match &mut self.slots[s].card {
            None => MC_ERR_CARD_NOT_EXIST,
            Some(c) => c.write(name, ofs, data).err().unwrap_or(MC_ERR_NONE),
        };
        self.start(now, MC_FUNC_WRITE_FILE, r)
    }

    /// Runs `f` on the card for the synchronous commands. -1 if a command is running or the
    /// channel is bad [guess], 1 for no card [doc: McErrCardNotExist].
    fn with_card(&mut self, now: u32, chan: i32, f: impl FnOnce(&mut Card) -> i32) -> i32 {
        let Some(s) = Self::slot_of(chan) else {
            return -1;
        };
        if self.busy(now) {
            return -1;
        }
        match &mut self.slots[s].card {
            None => MC_ERR_CARD_NOT_EXIST,
            Some(c) => f(c),
        }
    }

    pub fn create_file(&mut self, now: u32, chan: i32, name: &[u8], blocks: usize) -> i32 {
        self.with_card(now, chan, |c| c.create(name, blocks).err().unwrap_or(0))
    }

    pub fn delete_file(&mut self, now: u32, chan: i32, name: &[u8]) -> i32 {
        self.with_card(now, chan, |c| c.delete(name).err().unwrap_or(0))
    }

    pub fn format(&mut self, now: u32, chan: i32) -> i32 {
        self.with_card(now, chan, |c| {
            c.format();
            0
        })
    }

    /// `MemCardGetDirentry`: the matching files from index `ofs`, at most `max`; the McErr
    /// code on failure.
    pub fn dirents(
        &mut self,
        now: u32,
        chan: i32,
        pattern: &[u8],
        ofs: usize,
        max: usize,
    ) -> Result<Vec<FileInfo>, i32> {
        let mut found = Vec::new();
        let status = self.with_card(now, chan, |c| {
            if !c.is_formatted() {
                return MC_ERR_NOT_FORMAT;
            }
            found = c.find(pattern);
            0
        });
        if status != 0 {
            return Err(status);
        }
        Ok(found.into_iter().skip(ofs).take(max).collect())
    }

    /// `MemCardSync`. `wait` (mode 0) finishes a running command at once. A finished command
    /// is reported once, then the library is idle [obs: `updateMemoryCardState` restarts its
    /// `Exist` on -1].
    pub fn sync(&mut self, now: u32, wait: bool) -> SyncState {
        self.update(now);
        if wait && matches!(self.phase, Phase::Busy { .. }) {
            self.phase = Phase::Done;
        }
        match self.phase {
            Phase::Idle => SyncState::Idle,
            Phase::Busy { .. } => SyncState::Busy,
            Phase::Done => {
                self.phase = Phase::Idle;
                SyncState::Done {
                    cmd: self.cmd,
                    result: self.result,
                }
            }
        }
    }
}

impl Default for Controller {
    fn default() -> Self {
        Controller::new()
    }
}

static CONTROLLER: Mutex<Option<Controller>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Controller) -> T) -> T {
    f(CONTROLLER
        .lock()
        .unwrap()
        .get_or_insert_with(Controller::new))
}

/// Puts a card (or none) in slot 1 or 2 (`slot` 0 or 1). The default is a fresh formatted
/// card in each. An inserted card reads as new until `MemCardAccept` sees it.
pub fn insert_card(slot: usize, card: Option<Card>) {
    assert!(slot < 2, "memory card slot {slot}");
    with(|c| c.insert(slot, card));
}

/// A copy of the card in a slot, for the app to save it.
pub fn card(slot: usize) -> Option<Card> {
    with(|c| c.slots[slot].card.clone())
}

fn name_bytes<'a>(name: *const c_char) -> &'a [u8] {
    // SAFETY: the game passes NUL-terminated file names.
    unsafe { CStr::from_ptr(name) }.to_bytes()
}

// ---- The exported calls ----

/// `MemCardInit(flg)`: initialises the card library [doc: flg 1 shares the controller ports'
/// interrupts with the pads].
#[unsafe(no_mangle)]
pub extern "C" fn MemCardInit(_flg: i32) {}

/// `MemCardStart()`: starts card access [doc].
#[unsafe(no_mangle)]
pub extern "C" fn MemCardStart() {}

/// Starts a card check (result via `MemCardSync`); 1 if started.
#[unsafe(no_mangle)]
pub extern "C" fn MemCardExist(chan: i32) -> i32 {
    with(|c| c.exist(libetc::vblank_count(), chan)) as i32
}

/// Starts a card accept; 1 if started.
#[unsafe(no_mangle)]
pub extern "C" fn MemCardAccept(chan: i32) -> i32 {
    with(|c| c.accept(libetc::vblank_count(), chan)) as i32
}

/// # Safety
/// `fnam` is a C string; `adrs` has room for `bytes` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MemCardReadFile(
    chan: i32,
    fnam: *const c_char,
    adrs: *mut u8,
    ofs: i32,
    bytes: i32,
) -> i32 {
    if ofs < 0 || bytes < 0 {
        return 0;
    }
    let buf = unsafe { std::slice::from_raw_parts_mut(adrs, bytes as usize) };
    let name = name_bytes(fnam);
    with(|c| c.read_file(libetc::vblank_count(), chan, name, ofs as usize, buf)) as i32
}

/// # Safety
/// `fnam` is a C string; `adrs` holds `bytes` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MemCardWriteFile(
    chan: i32,
    fnam: *const c_char,
    adrs: *const u8,
    ofs: i32,
    bytes: i32,
) -> i32 {
    if ofs < 0 || bytes < 0 {
        return 0;
    }
    let data = unsafe { std::slice::from_raw_parts(adrs, bytes as usize) };
    let name = name_bytes(fnam);
    with(|c| c.write_file(libetc::vblank_count(), chan, name, ofs as usize, data)) as i32
}

/// # Safety
/// `fnam` is a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MemCardCreateFile(chan: i32, fnam: *const c_char, blocks: i32) -> i32 {
    let name = name_bytes(fnam);
    with(|c| c.create_file(libetc::vblank_count(), chan, name, blocks.max(0) as usize))
}

/// # Safety
/// `fnam` is a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MemCardDeleteFile(chan: i32, fnam: *const c_char) -> i32 {
    let name = name_bytes(fnam);
    with(|c| c.delete_file(libetc::vblank_count(), chan, name))
}

#[unsafe(no_mangle)]
pub extern "C" fn MemCardFormat(chan: i32) -> i32 {
    with(|c| c.format(libetc::vblank_count(), chan))
}

/// `MemCardSync(mode, &cmds, &rslt)`; see the module docs.
///
/// # Safety
/// `cmds` and `rslt` are null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MemCardSync(mode: i32, cmds: *mut u32, rslt: *mut u32) -> i32 {
    match with(|c| c.sync(libetc::vblank_count(), mode == 0)) {
        SyncState::Idle => -1,
        SyncState::Busy => 0,
        SyncState::Done { cmd, result } => {
            // SAFETY: the caller's contract.
            unsafe {
                if !cmds.is_null() {
                    *cmds = cmd;
                }
                if !rslt.is_null() {
                    *rslt = result;
                }
            }
            1
        }
    }
}

/// `MemCardGetDirentry(chan, name, pdir, &files, ofs, max)`: fills `pdir` with the files
/// matching `name`, from the `ofs`-th, up to `max`, and stores their count in `files`. Returns
/// 0, or an McErr code (-1 if a command is running). Per entry: `name`, `size` in bytes,
/// `head` the 1-based first block; `attr` 0 and `next` null [guess].
///
/// # Safety
/// `name` is a C string; `pdir` has room for `max` entries; `files` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MemCardGetDirentry(
    chan: i32,
    name: *const c_char,
    pdir: *mut DirEntry,
    files: *mut i32,
    ofs: i32,
    max: i32,
) -> i32 {
    let pattern = name_bytes(name);
    let r = with(|c| {
        c.dirents(
            libetc::vblank_count(),
            chan,
            pattern,
            ofs.max(0) as usize,
            max.max(0) as usize,
        )
    });
    let list = match r {
        Ok(l) => l,
        Err(e) => {
            if !files.is_null() {
                unsafe { *files = 0 };
            }
            return e;
        }
    };
    for (i, f) in list.iter().enumerate() {
        let mut e = DirEntry {
            name: [0; 20],
            attr: 0,
            size: f.size as i32,
            next: std::ptr::null_mut(),
            head: f.blocks[0] as i32 + 1,
            system: [0; 4],
        };
        for (d, s) in e.name.iter_mut().zip(&f.name) {
            *d = *s as c_char;
        }
        unsafe { pdir.add(i).write(e) };
    }
    if !files.is_null() {
        unsafe { *files = list.len() as i32 };
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small SHA-1, so the test needs no dependency.
    fn sha1(data: &[u8]) -> String {
        let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
        let mut msg = data.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
        for chunk in msg.chunks(64) {
            let mut w = [0u32; 80];
            for i in 0..16 {
                w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }
            let [mut a, mut b, mut c, mut d, mut e] = h;
            for (i, wi) in w.iter().enumerate() {
                let (f, k) = match i {
                    0..=19 => ((b & c) | (!b & d), 0x5A827999),
                    20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                    _ => (b ^ c ^ d, 0xCA62C1D6u32),
                };
                let t = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(*wi);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = t;
            }
            for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
                *x = x.wrapping_add(y);
            }
        }
        h.iter().map(|x| format!("{x:08x}")).collect()
    }

    #[test]
    fn sha1_helper_is_right() {
        assert_eq!(sha1(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn fresh_card_matches_redux() {
        let c = Card::formatted();
        assert_eq!(c.as_bytes().len(), CARD_SIZE);
        assert_eq!(
            sha1(c.as_bytes()),
            "1b3c99be55273085e14b30983b636529b6dd7f46"
        );
        assert!(c.is_formatted());
        assert_eq!(c.free_blocks(), 15);
        assert!(c.files().is_empty());
    }

    #[test]
    fn frames_checksum() {
        let mut c = Card::formatted();
        c.create(b"BASLUS-01032DMR0", 2).unwrap();
        c.delete(b"BASLUS-01032DMR0").unwrap();
        c.create(b"BASLUS-01032DMR1", 3).unwrap();
        for f in (0..36).chain([63]) {
            let fr = &c.as_bytes()[f * FRAME..(f + 1) * FRAME];
            assert_eq!(fr[127], frame_checksum(fr), "frame {f}");
        }
    }

    #[test]
    fn create_write_read_delete() {
        let mut c = Card::formatted();
        let name = b"BASLUS-01032DMR0";
        c.create(name, 2).unwrap();
        let files = c.files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, name);
        assert_eq!(files[0].size, 2 * BLOCK as u32);
        assert_eq!(files[0].blocks, [0, 1]);
        assert_eq!(c.dir_frame(0)[0], FIRST);
        assert_eq!(c.dir_frame(1)[0], LAST);
        assert_eq!(c.free_blocks(), 13);
        assert_eq!(c.create(name, 1), Err(MC_ERR_ALREADY_EXIST));

        // A write across the block boundary lands in each block's data.
        let data: Vec<u8> = (0..BLOCK + 256).map(|i| (i % 251) as u8).collect();
        c.write(name, 128, &data).unwrap();
        let mut back = vec![0u8; data.len()];
        c.read(name, 128, &mut back).unwrap();
        assert_eq!(back, data);
        assert_eq!(c.as_bytes()[BLOCK + 128], data[0]);
        assert_eq!(c.as_bytes()[2 * BLOCK], data[BLOCK - 128]);
        assert_eq!(c.write(name, 0, &[0; 100]), Err(MC_ERR_CARD_INVALID));
        assert_eq!(
            c.read(name, 0, &mut vec![0; 3 * BLOCK]),
            Err(MC_ERR_CARD_INVALID)
        );
        assert_eq!(
            c.read(b"nope", 0, &mut [0; 128]),
            Err(MC_ERR_FILE_NOT_EXIST)
        );

        c.delete(name).unwrap();
        assert!(c.files().is_empty());
        assert_eq!(c.free_blocks(), 15);
        assert_eq!(c.dir_frame(0)[0], 0xA1);
        assert_eq!(c.dir_frame(1)[0], 0xA3);
        assert_eq!(c.delete(name), Err(MC_ERR_FILE_NOT_EXIST));
        c.create(name, 15).unwrap();
        assert_eq!(c.create(b"x", 1), Err(MC_ERR_BLOCK_FULL));
    }

    #[test]
    fn wildcards_and_unformatted() {
        let mut c = Card::formatted();
        c.create(b"BASLUS-01032DMR0", 1).unwrap();
        c.create(b"BASLUS-01032DMR1", 1).unwrap();
        c.create(b"OTHER", 1).unwrap();
        assert_eq!(c.find(b"BASLUS-01032DMR*").len(), 2);
        assert_eq!(c.find(b"BASLUS-01032DMR1").len(), 1);
        assert_eq!(c.find(b"*").len(), 3);
        assert_eq!(c.find(b"BASLUS-01032DMR").len(), 0);
        assert_eq!(c.find(b"OTH?R").len(), 1);
        let mut blank = Card::from_bytes(&[0; CARD_SIZE]).unwrap();
        assert!(!blank.is_formatted());
        assert_eq!(blank.create(b"x", 1), Err(MC_ERR_NOT_FORMAT));
        blank.format();
        assert!(blank == Card::formatted());
        assert!(Card::from_bytes(&[0; 10]).is_none());
    }

    #[test]
    fn async_state_machine() {
        let mut m = Controller::new();
        assert_eq!(m.sync(0, false), SyncState::Idle);
        assert!(m.exist(0, 0));
        assert!(!m.accept(0, 0), "second start while busy fails");
        assert_eq!(m.sync(0, false), SyncState::Busy);
        assert_eq!(
            m.sync(COMMAND_VBLANKS, false),
            SyncState::Done {
                cmd: MC_FUNC_EXIST,
                result: 3
            }
        );
        assert_eq!(m.sync(COMMAND_VBLANKS, false), SyncState::Idle);
        // Waiting completes at once.
        assert!(m.accept(5, 0x10));
        assert_eq!(
            m.sync(5, true),
            SyncState::Done {
                cmd: MC_FUNC_ACCEPT,
                result: 3
            }
        );
        // Accept consumed the new-card flag.
        assert!(m.accept(6, 0x10));
        assert_eq!(
            m.sync(6, true),
            SyncState::Done {
                cmd: MC_FUNC_ACCEPT,
                result: 0
            }
        );
        assert!(m.exist(7, 0x10));
        assert_eq!(m.sync(7, true), SyncState::Done { cmd: 1, result: 0 });
        // Bad channel, no card, unformatted card.
        assert!(!m.exist(8, 3));
        m.insert(0, None);
        assert!(m.exist(8, 0));
        assert_eq!(m.sync(8, true), SyncState::Done { cmd: 1, result: 1 });
        m.insert(0, Card::from_bytes(&[0; CARD_SIZE]));
        assert!(m.accept(9, 0));
        m.sync(9, true);
        assert!(m.accept(9, 0));
        assert_eq!(m.sync(9, true), SyncState::Done { cmd: 2, result: 4 });
    }

    #[test]
    fn file_commands_through_the_controller() {
        let mut m = Controller::new();
        let name = b"BASLUS-01032DMR0";
        assert_eq!(m.create_file(0, 0, name, 1), 0);
        let data = vec![7u8; 0x2000];
        assert!(m.write_file(0, 0, name, 0, &data));
        assert!(!m.write_file(0, 0, name, 0, &data), "busy");
        assert_eq!(
            m.create_file(0, 0, b"y", 1),
            -1,
            "sync commands fail while busy"
        );
        assert_eq!(m.sync(0, true), SyncState::Done { cmd: 4, result: 0 });
        let mut back = vec![0u8; 0x2000];
        assert!(m.read_file(1, 0, name, 0, &mut back));
        assert_eq!(m.sync(1, true), SyncState::Done { cmd: 3, result: 0 });
        assert_eq!(back, data);
        assert!(m.read_file(2, 0, b"missing", 0, &mut back));
        assert_eq!(m.sync(2, true), SyncState::Done { cmd: 3, result: 5 });
        assert!(!m.read_file(3, 0, name, 1, &mut back), "unaligned offset");
        let d = m.dirents(3, 0, b"BASLUS-01032DMR*", 0, 15).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(m.delete_file(3, 0, name), 0);
        assert_eq!(m.format(3, 0), 0);
        assert_eq!(m.format(3, 0x20), -1);
        m.insert(1, None);
        assert_eq!(m.format(3, 0x10), MC_ERR_CARD_NOT_EXIST);
        assert_eq!(m.dirents(3, 0x10, b"*", 0, 15).unwrap_err(), 1);
    }
}
