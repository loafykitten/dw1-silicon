//! libsnd: VAB banks, SEQ playback handles and voice key-on, silent.
//!
//! The game only needs the library to keep its books: VAB ids open and close, a VAB's sample
//! body lands in sound RAM at the address the game chose (so Phase 6's SPU can play it), SEQ
//! access numbers are handed out, and the waits the game does (`SsVabTransCompleted`) finish.
//! Nothing ticks: there is no sequencer yet (Phase 6), so a SEQ that was told to play stays
//! "playing" and never ends. The game doesn't wait on a SEQ ending during boot (it never calls
//! `SsIsEos`), so nothing is faked. Voices are the SPU's, in [`crate::libspu`].
//!
//! Semantics are from the Psy-Q Run-Time Library Reference 4.4 (the `[doc]` marks); anything it
//! doesn't settle is a `[guess]`. The VAB header layout is the one libsnd.h declares (`VabHdr`,
//! `ProgAtr`, `VagAtr`), followed by the 256-entry table of sample sizes in 8-byte units.
//!
//! Lock order: this module's state, then libspu's.

use std::sync::{LazyLock, Mutex, MutexGuard};

use backend::spu::RAM_SIZE;

use crate::libspu::{self, AutoVol, KeyOn, SpuState, VOICES};

/// VABs open at once. [doc] ids 0-15.
pub const MAX_VABS: usize = 16;
/// [doc] SEQ/SEP data open at once, and the most `SsSetTableSize` accepts.
pub const MAX_SEQ: usize = 32;
/// Programs in a VAB header.
const VAB_PROGRAMS: usize = 128;
const VAB_TONES_PER_PROGRAM: usize = 16;
const PROG_ATTR_SIZE: usize = 16;
const TONE_ATTR_SIZE: usize = 32;
/// Entries in the sample size table that ends the VAB header.
const VAG_TABLE_ENTRIES: usize = 256;
/// `VabHdr.form`: "VABp".
const VAB_FORM: [u8; 4] = *b"VABp";

/// `VabHdr` (libsnd.h), 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VabHdr {
    pub form: i32,
    pub ver: i32,
    pub id: i32,
    pub fsize: u32,
    pub reserved0: u16,
    /// Programs in the bank.
    pub ps: u16,
    /// Tones in the bank.
    pub ts: u16,
    /// Samples (VAGs) in the bank.
    pub vs: u16,
    pub mvol: u8,
    pub pan: u8,
    pub attr1: u8,
    pub attr2: u8,
    pub reserved1: u32,
}

/// What the library needs from a VAB header (VH).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VhInfo {
    pub programs: usize,
    pub tones: usize,
    pub samples: usize,
    /// Bytes of the VH: header, program table, tone table, sample size table.
    pub vh_size: usize,
    /// Bytes of sample data (VB) that follow the VH on disc.
    pub body_size: usize,
}

/// Size of a VH with `programs` programs.
pub fn vh_size(programs: usize) -> usize {
    size_of::<VabHdr>()
        + VAB_PROGRAMS * PROG_ATTR_SIZE
        + programs * VAB_TONES_PER_PROGRAM * TONE_ATTR_SIZE
        + VAG_TABLE_ENTRIES * 2
}

/// The header's program count, if `head` begins with a valid-looking `VabHdr`. [doc] "the VAB
/// header doesn't contain valid data" is an open failure; validity here is the "VABp" tag and
/// counts within the format's limits (128 programs, 16 tones each, 256 samples) [guess].
pub fn vh_program_count(head: &[u8]) -> Option<usize> {
    let hdr = head.get(..size_of::<VabHdr>())?;
    if hdr[..4] != VAB_FORM {
        return None;
    }
    let u16_at = |at: usize| usize::from(u16::from_le_bytes([hdr[at], hdr[at + 1]]));
    let (ps, ts, vs) = (u16_at(18), u16_at(20), u16_at(22));
    (ps <= VAB_PROGRAMS && ts <= VAB_PROGRAMS * VAB_TONES_PER_PROGRAM && vs <= VAG_TABLE_ENTRIES)
        .then_some(ps)
}

/// Parses a whole VH (`head` must hold all `vh_size` bytes). The body size is the sum of the
/// sample size table, which counts 8-byte units; entry 0 is included, so either numbering of
/// the samples (from 0 or from 1) gives the right total.
pub fn parse_vh(head: &[u8]) -> Option<VhInfo> {
    let programs = vh_program_count(head)?;
    let size = vh_size(programs);
    let head = head.get(..size)?;
    let u16_at = |at: usize| usize::from(u16::from_le_bytes([head[at], head[at + 1]]));
    let samples = u16_at(22);
    let table = size - VAG_TABLE_ENTRIES * 2;
    let units: usize = (0..=samples.min(VAG_TABLE_ENTRIES - 1))
        .map(|i| u16_at(table + i * 2))
        .sum();
    Some(VhInfo {
        programs,
        tones: u16_at(20),
        samples,
        vh_size: size,
        body_size: units * 8,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vab {
    /// Where the body goes in sound RAM (the game's `sbaddr`).
    pub sb_addr: u32,
    pub body_size: usize,
    pub info: VhInfo,
    /// `SsVabTransBody` has run.
    pub transferred: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqStatus {
    Opened,
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seq {
    /// Address of the SEQ data in the game's memory. Not read here: the game reuses the
    /// buffer, so Phase 6 must copy the data when it opens the SEQ for real.
    pub addr: usize,
    pub vab_id: i16,
    pub status: SeqStatus,
    pub vol_left: i16,
    pub vol_right: i16,
    /// `SsSeqPlay`'s play count; 0 (`SSPLAY_INFINITY`) loops forever.
    pub loops: i16,
}

pub struct SndState {
    pub initialized: bool,
    /// `SsStart` has run (and `SsEnd` hasn't).
    pub started: bool,
    pub tick_mode: i32,
    pub master_vol: (i16, i16),
    pub vabs: [Option<Vab>; MAX_VABS],
    /// SEQ access numbers are indexes into this; its length is `SsSetTableSize`'s `s_max`.
    pub seqs: Vec<Option<Seq>>,
    /// `SsSetTableSize`'s `t_max` (SEQs per SEP). Kept; SEP isn't used.
    pub seq_tunes: i16,
}

impl SndState {
    pub fn new() -> Self {
        Self {
            initialized: false,
            started: false,
            tick_mode: 0,
            master_vol: (0, 0),
            vabs: [None; MAX_VABS],
            seqs: Vec::new(),
            seq_tunes: 0,
        }
    }

    fn vab(&self, id: i16) -> Option<&Vab> {
        self.vabs.get(usize::try_from(id).ok()?)?.as_ref()
    }

    /// `SsInit`: libsnd and the SPU back to power-on.
    pub fn init(&mut self, spu: &mut SpuState) {
        *self = Self::new();
        self.initialized = true;
        spu.reset();
    }

    /// `SsQuit`: closes everything. Sound RAM keeps its contents.
    pub fn quit(&mut self, spu: &mut SpuState) {
        *self = Self::new();
        spu.all_key_off();
    }

    /// `SsSetTableSize`. [guess] it drops SEQs already open; the game calls it once, right
    /// after `SsInit`.
    pub fn set_table_size(&mut self, s_max: i16, t_max: i16) {
        self.seqs = vec![None; usize::try_from(s_max).unwrap_or(0).min(MAX_SEQ)];
        self.seq_tunes = t_max;
    }

    /// `SsVabOpenHeadSticky`: `head` is the VH (at least `vh_size` bytes, unless the header is
    /// invalid and then anything). Returns the VAB id, or -1 in every case [doc] lists:
    /// id already open, id outside 0-15 (-1 asks for any free id), no free id, invalid header,
    /// body past the end of sound RAM.
    pub fn open_head_sticky(&mut self, head: &[u8], vab_id: i16, sb_addr: u32) -> i16 {
        let id = if vab_id == -1 {
            match self.vabs.iter().position(Option::is_none) {
                Some(i) => i,
                None => return -1,
            }
        } else {
            match usize::try_from(vab_id) {
                Ok(i) if i < MAX_VABS && self.vabs[i].is_none() => i,
                _ => return -1,
            }
        };
        let Some(info) = parse_vh(head) else {
            return -1;
        };
        let start = sb_addr as usize;
        if start + info.body_size > RAM_SIZE {
            return -1;
        }
        for (other, vab) in self.vabs.iter().enumerate() {
            let Some(vab) = vab else { continue };
            let (a, b) = (vab.sb_addr as usize, vab.body_size);
            if start < a + b && a < start + info.body_size {
                // Real sound RAM would hold garbage; say so, and carry on like the SDK.
                eprintln!(
                    "psyq: VAB {id} at SPU {start:#x}+{:#x} overlaps VAB {other} at {a:#x}+{b:#x}",
                    info.body_size
                );
            }
        }
        self.vabs[id] = Some(Vab {
            sb_addr,
            body_size: info.body_size,
            info,
            transferred: false,
        });
        id as i16
    }

    /// `SsVabTransBody`: copies the body to sound RAM at the VAB's `sb_addr`. Silent, so the
    /// transfer is done when this returns. Returns the VAB id; -1 if that VAB isn't open or
    /// its body was already sent [guess for the second case]. `body` must hold `body_size`
    /// bytes.
    pub fn trans_body(&mut self, spu: &mut SpuState, body: &[u8], vab_id: i16) -> i16 {
        let Some(vab) = usize::try_from(vab_id)
            .ok()
            .and_then(|i| self.vabs.get_mut(i)?.as_mut())
        else {
            return -1;
        };
        if vab.transferred || body.len() < vab.body_size {
            return -1;
        }
        if !spu.write_ram(vab.sb_addr as usize, &body[..vab.body_size]) {
            return -1;
        }
        vab.transferred = true;
        vab_id
    }

    /// `SsVabClose`. An id that isn't open is ignored. Sound RAM is left as it is.
    pub fn close_vab(&mut self, vab_id: i16) {
        if let Some(slot) = usize::try_from(vab_id)
            .ok()
            .and_then(|i| self.vabs.get_mut(i))
        {
            *slot = None;
        }
    }

    /// `SsUtGetVBaddrInSB`: the VAB's sound RAM address, or `u32::MAX` (-1) if it isn't open.
    pub fn vab_addr(&self, vab_id: i16) -> u32 {
        self.vab(vab_id).map_or(u32::MAX, |v| v.sb_addr)
    }

    /// `SsSeqOpen`: the lowest free SEQ access number, or -1 if the table is full (or was
    /// never sized: [guess], the SDK needs `SsSetTableSize` first).
    pub fn seq_open(&mut self, addr: usize, vab_id: i16) -> i16 {
        match self.seqs.iter().position(Option::is_none) {
            Some(i) => {
                self.seqs[i] = Some(Seq {
                    addr,
                    vab_id,
                    status: SeqStatus::Opened,
                    vol_left: 0,
                    vol_right: 0,
                    loops: 0,
                });
                i as i16
            }
            None => -1,
        }
    }

    fn seq_mut(&mut self, access: i16) -> Option<&mut Seq> {
        self.seqs.get_mut(usize::try_from(access).ok()?)?.as_mut()
    }

    pub fn seq_close(&mut self, access: i16) {
        if let Some(slot) = usize::try_from(access)
            .ok()
            .and_then(|i| self.seqs.get_mut(i))
        {
            *slot = None;
        }
    }

    /// `SsSeqPlay`: `SSPLAY_PLAY` (1) plays, `SSPLAY_PAUSE` (0) pauses. With no tick source
    /// a playing SEQ never advances or ends.
    pub fn seq_play(&mut self, access: i16, mode: i8, loops: i16) {
        if let Some(seq) = self.seq_mut(access) {
            seq.status = if mode == 1 {
                SeqStatus::Playing
            } else {
                SeqStatus::Paused
            };
            seq.loops = loops;
        }
    }

    pub fn seq_stop(&mut self, access: i16) {
        if let Some(seq) = self.seq_mut(access) {
            seq.status = SeqStatus::Stopped;
        }
    }

    pub fn seq_set_vol(&mut self, access: i16, left: i16, right: i16) {
        if let Some(seq) = self.seq_mut(access) {
            (seq.vol_left, seq.vol_right) = (left, right);
        }
    }

    /// `SsUtKeyOnV`. [doc] ranges: voice 0-23, prog 0-127, tone 0-15, note, fine and volumes
    /// 0-127; the VAB must be open. Returns the voice, or -1.
    pub fn key_on_v(&self, spu: &mut SpuState, voice: i16, key: KeyOn) -> i16 {
        let in_range = |v: i16, max: i16| (0..=max).contains(&v);
        let ok = in_range(voice, VOICES as i16 - 1)
            && self.vab(key.vab).is_some()
            && in_range(key.prog, 127)
            && in_range(key.tone, 15)
            && in_range(key.note, 127)
            && in_range(key.fine, 127)
            && in_range(key.vol_left, 127)
            && in_range(key.vol_right, 127);
        if ok && spu.key_on(voice as usize, key) {
            voice
        } else {
            -1
        }
    }
}

impl Default for SndState {
    fn default() -> Self {
        Self::new()
    }
}

static SND: LazyLock<Mutex<SndState>> = LazyLock::new(|| Mutex::new(SndState::new()));

/// The libsnd state, locked. Take it before libspu's if you need both.
pub fn state() -> MutexGuard<'static, SndState> {
    SND.lock().unwrap()
}

/// Runs `f` on the libsnd state.
pub fn with_state<R>(f: impl FnOnce(&mut SndState) -> R) -> R {
    f(&mut state())
}

/// `SsInit`: [doc] initializes libsnd (and the SPU under it).
#[unsafe(no_mangle)]
pub extern "C" fn SsInit() {
    let mut snd = state();
    snd.init(&mut libspu::state());
}

/// `SsQuit`: closes libsnd.
#[unsafe(no_mangle)]
pub extern "C" fn SsQuit() {
    let mut snd = state();
    snd.quit(&mut libspu::state());
}

/// `SsStart`: starts sound processing (the tick source). Nothing ticks yet.
#[unsafe(no_mangle)]
pub extern "C" fn SsStart() {
    state().started = true;
}

/// `SsEnd`: stops sound processing.
#[unsafe(no_mangle)]
pub extern "C" fn SsEnd() {
    state().started = false;
}

/// `SsSetTickMode(mode)`: the SEQ tick source (`SS_TICK240`...). Kept for Phase 6.
#[unsafe(no_mangle)]
pub extern "C" fn SsSetTickMode(mode: i32) {
    state().tick_mode = mode;
}

/// `SsSetTableSize(table, s_max, t_max)`. The caller's `table` memory is not used: the SEQ
/// table lives here.
#[unsafe(no_mangle)]
pub extern "C" fn SsSetTableSize(_table: *mut u8, s_max: i16, t_max: i16) {
    state().set_table_size(s_max, t_max);
}

/// `SsSetMVol(left, right)`: master volume.
#[unsafe(no_mangle)]
pub extern "C" fn SsSetMVol(left: i16, right: i16) {
    state().master_vol = (left, right);
}

/// `SsVabOpenHeadSticky(addr, vabid, sbaddr)`: opens a VAB from its header in memory. Returns
/// the VAB id or -1.
///
/// # Safety
/// `addr` must point to a VAB header in readable memory (NULL returns -1).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SsVabOpenHeadSticky(addr: *const u8, vabid: i16, sbaddr: u32) -> i16 {
    if addr.is_null() {
        return -1;
    }
    // The header says how long it is, so read the fixed part first.
    // SAFETY: the caller's contract: a VAB header, which is at least a `VabHdr` long.
    let fixed = unsafe { std::slice::from_raw_parts(addr, size_of::<VabHdr>()) };
    let Some(programs) = vh_program_count(fixed) else {
        return -1;
    };
    // SAFETY: a valid header is followed by its program, tone and sample size tables.
    let head = unsafe { std::slice::from_raw_parts(addr, vh_size(programs)) };
    state().open_head_sticky(head, vabid, sbaddr)
}

/// `SsVabTransBody(addr, vabid)`: sends the VAB's sample data to sound RAM. Returns the VAB id
/// or -1.
///
/// # Safety
/// `addr` must point to the VAB's body (the VB data), as long as the header's sample sizes say.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SsVabTransBody(addr: *const u8, vabid: i16) -> i16 {
    let mut snd = state();
    let Some(size) = snd.vab(vabid).map(|v| v.body_size) else {
        return -1;
    };
    if addr.is_null() {
        return -1;
    }
    // SAFETY: the caller's contract.
    let body = unsafe { std::slice::from_raw_parts(addr, size) };
    snd.trans_body(&mut libspu::state(), body, vabid)
}

/// `SsVabTransCompleted(mode)`: [doc] 1 when the transfer is done. A silent transfer is done
/// as soon as `SsVabTransBody` returns, so this is always 1 and never waits.
#[unsafe(no_mangle)]
pub extern "C" fn SsVabTransCompleted(_mode: i16) -> i16 {
    1
}

/// `SsVabClose(vabid)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsVabClose(vabid: i16) {
    state().close_vab(vabid);
}

/// `SsUtGetVBaddrInSB(vabid)`: the VAB's sound RAM address, -1 if it isn't open.
#[unsafe(no_mangle)]
pub extern "C" fn SsUtGetVBaddrInSB(vabid: i16) -> u32 {
    state().vab_addr(vabid)
}

/// `SsSeqOpen(addr, vab_id)`: the SEQ access number, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqOpen(addr: *const u32, vab_id: i16) -> i16 {
    if addr.is_null() {
        return -1;
    }
    state().seq_open(addr as usize, vab_id)
}

/// `SsSeqClose(access)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqClose(access: i16) {
    state().seq_close(access);
}

/// `SsSeqPlay(access, mode, l_count)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqPlay(access: i16, mode: i8, l_count: i16) {
    state().seq_play(access, mode, l_count);
}

/// `SsSeqStop(access)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqStop(access: i16) {
    state().seq_stop(access);
}

/// `SsSeqSetVol(access, left, right)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqSetVol(access: i16, left: i16, right: i16) {
    state().seq_set_vol(access, left, right);
}

/// `SsUtKeyOnV(voice, vabid, prog, tone, note, fine, voll, volr)`: keys a voice on. [doc]
/// returns the voice number, -1 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn SsUtKeyOnV(
    voice: i16,
    vab: i16,
    prog: i16,
    tone: i16,
    note: i16,
    fine: i16,
    vol_left: i16,
    vol_right: i16,
) -> i16 {
    let key = KeyOn {
        vab,
        prog,
        tone,
        note,
        fine,
        vol_left,
        vol_right,
    };
    state().key_on_v(&mut libspu::state(), voice, key)
}

/// `SsUtKeyOffV(voice)`: [doc] 0, or -1 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn SsUtKeyOffV(voice: i16) -> i16 {
    match usize::try_from(voice) {
        Ok(v) if libspu::state().key_off(v) => 0,
        _ => -1,
    }
}

/// `SsUtAllKeyOff(mode)`: every voice off.
#[unsafe(no_mangle)]
pub extern "C" fn SsUtAllKeyOff(_mode: i16) {
    libspu::state().all_key_off();
}

/// `SsUtAutoVol(vc, start_vol, end_vol, delta_time)`: [doc] a volume sweep on a voice over
/// `delta_time` ticks; 0, or -1 for arguments out of range (voice 0-23, volumes 0-127, time
/// 0-10800).
#[unsafe(no_mangle)]
pub extern "C" fn SsUtAutoVol(voice: i16, start_vol: i16, end_vol: i16, delta_time: i16) -> i16 {
    let ok = (0..VOICES as i16).contains(&voice)
        && (0..=127).contains(&start_vol)
        && (0..=127).contains(&end_vol)
        && (0..=10800).contains(&delta_time);
    if !ok {
        return -1;
    }
    libspu::state().voices[voice as usize].auto_vol = Some(AutoVol {
        start: start_vol,
        end: end_vol,
        ticks: delta_time,
    });
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A VH with `programs` programs, `samples` samples (numbered from 1) of the given sizes
    /// in bytes.
    fn make_vh(programs: u16, sizes: &[usize]) -> Vec<u8> {
        let mut vh = vec![0u8; vh_size(usize::from(programs))];
        vh[..4].copy_from_slice(b"VABp");
        vh[18..20].copy_from_slice(&programs.to_le_bytes());
        vh[20..22].copy_from_slice(&(programs * 2).to_le_bytes());
        vh[22..24].copy_from_slice(&(sizes.len() as u16).to_le_bytes());
        let table = vh.len() - VAG_TABLE_ENTRIES * 2;
        for (i, size) in sizes.iter().enumerate() {
            let at = table + (i + 1) * 2;
            vh[at..at + 2].copy_from_slice(&((size / 8) as u16).to_le_bytes());
        }
        vh
    }

    fn fresh() -> (SndState, SpuState) {
        let mut snd = SndState::new();
        let mut spu = SpuState::new();
        snd.init(&mut spu);
        (snd, spu)
    }

    #[test]
    fn header_layout_and_body_size() {
        assert_eq!(size_of::<VabHdr>(), 32);
        let vh = make_vh(3, &[0x100, 0x2000, 0x18]);
        let info = parse_vh(&vh).unwrap();
        assert_eq!(info.programs, 3);
        assert_eq!(info.samples, 3);
        assert_eq!(info.vh_size, 32 + 2048 + 3 * 512 + 512);
        assert_eq!(info.body_size, 0x100 + 0x2000 + 0x18);
        assert_eq!(parse_vh(&vh[..100]), None, "truncated");
        let mut bad = vh.clone();
        bad[0] = b'X';
        assert_eq!(parse_vh(&bad), None);
        bad = vh;
        bad[18] = 129;
        assert_eq!(parse_vh(&bad), None, "too many programs");
    }

    #[test]
    fn vab_open_transfer_close_cycle() {
        let (mut snd, mut spu) = fresh();
        let vh = make_vh(2, &[0x40, 0x80]);
        let body: Vec<u8> = (0..0xc0).map(|i| i as u8 | 1).collect();
        assert_eq!(snd.open_head_sticky(&vh, 1, 0xe800), 1);
        assert_eq!(snd.vab_addr(1), 0xe800);
        // Already open: the game closes first for this reason.
        assert_eq!(snd.open_head_sticky(&vh, 1, 0xe800), -1);
        assert_eq!(snd.trans_body(&mut spu, &body, 1), 1);
        assert_eq!(&spu.ram[0xe800..0xe8c0], &body[..]);
        assert_eq!(spu.ram[0xe7ff], 0);
        assert_eq!(spu.ram[0xe8c0], 0);
        assert_eq!(snd.trans_body(&mut spu, &body, 1), -1, "sent twice");
        assert_eq!(snd.trans_body(&mut spu, &body, 2), -1, "not open");
        snd.close_vab(1);
        snd.close_vab(1);
        snd.close_vab(-3);
        snd.close_vab(99);
        assert_eq!(snd.vab_addr(1), u32::MAX);
        assert_eq!(snd.open_head_sticky(&vh, 1, 0xe800), 1);
        // The data stays in sound RAM across a close, like the SPU's.
        assert_eq!(spu.ram[0xe800], body[0]);
    }

    #[test]
    fn vab_open_failures_and_auto_ids() {
        let (mut snd, _) = fresh();
        let vh = make_vh(1, &[0x1000]);
        assert_eq!(snd.open_head_sticky(&vh, 16, 0x1010), -1);
        assert_eq!(snd.open_head_sticky(&vh, -2, 0x1010), -1);
        assert_eq!(snd.open_head_sticky(&vh[..40], 0, 0x1010), -1);
        // The body would pass the end of sound RAM.
        assert_eq!(snd.open_head_sticky(&vh, 0, RAM_SIZE as u32 - 0xfff), -1);
        assert_eq!(snd.open_head_sticky(&vh, 0, RAM_SIZE as u32 - 0x1000), 0);
        // -1 takes the lowest free id.
        assert_eq!(snd.open_head_sticky(&vh, -1, 0x1010), 1);
        for want in 2..16 {
            assert_eq!(snd.open_head_sticky(&vh, -1, 0x1010), want);
        }
        assert_eq!(snd.open_head_sticky(&vh, -1, 0x1010), -1, "all 16 open");
    }

    #[test]
    fn seq_table_ids_and_state() {
        let (mut snd, _) = fresh();
        assert_eq!(snd.seq_open(0x1000, 2), -1, "no table yet");
        snd.set_table_size(1, 1);
        let a = snd.seq_open(0x1000, 2);
        assert_eq!(a, 0);
        assert_eq!(snd.seq_open(0x2000, 2), -1, "table of one is full");
        snd.seq_set_vol(a, 0x50, 0x50);
        snd.seq_play(a, 1, 0);
        let seq = snd.seqs[0].unwrap();
        assert_eq!(
            (seq.status, seq.vol_left, seq.vol_right, seq.loops),
            (SeqStatus::Playing, 0x50, 0x50, 0)
        );
        snd.seq_stop(a);
        assert_eq!(snd.seqs[0].unwrap().status, SeqStatus::Stopped);
        // Bad access numbers do nothing.
        snd.seq_play(-1, 1, 0);
        snd.seq_stop(7);
        snd.seq_close(-1);
        snd.seq_close(a);
        assert_eq!(snd.seq_open(0x3000, 2), 0, "the number is free again");
        snd.set_table_size(100, 1);
        assert_eq!(snd.seqs.len(), MAX_SEQ);
    }

    #[test]
    fn key_on_checks_arguments_and_sets_the_spu_voice() {
        let (mut snd, mut spu) = fresh();
        let vh = make_vh(1, &[0x40]);
        snd.open_head_sticky(&vh, 0, 0x1010);
        let key = KeyOn {
            vab: 0,
            prog: 3,
            tone: 2,
            note: 60,
            fine: 0,
            vol_left: 0x7f,
            vol_right: 0x7f,
        };
        assert_eq!(snd.key_on_v(&mut spu, 12, key), 12);
        assert!(spu.voices[12].key_on);
        assert_eq!(spu.voices[12].key, key);
        assert_eq!(snd.key_on_v(&mut spu, 24, key), -1);
        assert_eq!(snd.key_on_v(&mut spu, -1, key), -1);
        assert_eq!(snd.key_on_v(&mut spu, 13, KeyOn { vab: 5, ..key }), -1);
        assert_eq!(snd.key_on_v(&mut spu, 13, KeyOn { tone: 16, ..key }), -1);
        assert_eq!(
            snd.key_on_v(
                &mut spu,
                13,
                KeyOn {
                    vol_left: 128,
                    ..key
                }
            ),
            -1
        );
        assert!(!spu.voices[13].key_on);
    }

    #[test]
    fn init_and_quit() {
        let (mut snd, mut spu) = fresh();
        snd.set_table_size(2, 1);
        let vh = make_vh(1, &[0x40]);
        snd.open_head_sticky(&vh, 0, 0x1010);
        snd.trans_body(&mut spu, &[7; 0x40], 0);
        spu.key_on(3, KeyOn::default());
        snd.quit(&mut spu);
        assert!(snd.vabs.iter().all(Option::is_none) && snd.seqs.is_empty());
        assert!(!spu.voices[3].key_on);
        assert_eq!(spu.ram[0x1010], 7, "quit leaves sound RAM alone");
        snd.init(&mut spu);
        assert_eq!(spu.ram[0x1010], 0, "init clears it");
    }

    #[test]
    fn c_interface_loads_a_vab_and_reports_completion() {
        let _guard = libspu::GLOBAL_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let vh = make_vh(1, &[0x40]);
        let body = [0x55u8; 0x40];
        SsInit();
        SsSetTableSize(std::ptr::null_mut(), 1, 1);
        SsSetTickMode(2);
        SsVabClose(0);
        // SAFETY: `vh` is a whole VH, `body` the whole body.
        unsafe {
            assert_eq!(SsVabOpenHeadSticky(vh.as_ptr(), 0, 0x1010), 0);
            assert_eq!(SsVabTransBody(body.as_ptr(), 0), 0);
            assert_eq!(SsVabOpenHeadSticky(std::ptr::null(), 1, 0x2000), -1);
            assert_eq!(SsVabTransBody(body.as_ptr(), 7), -1);
        }
        assert_eq!(SsVabTransCompleted(1), 1);
        assert_eq!(SsVabTransCompleted(0), 1);
        assert_eq!(SsUtGetVBaddrInSB(0), 0x1010);
        assert_eq!(libspu::with_state(|s| s.ram[0x1010]), 0x55);
        assert_eq!(SsUtKeyOnV(10, 0, 0, 0, 60, 0, 0x7f, 0x7f), 10);
        assert_eq!(libspu::SpuGetKeyStatus(1 << 10), libspu::SPU_ON);
        assert_eq!(SsUtAutoVol(10, 0x7f, 0, 6), 0);
        assert_eq!(SsUtAutoVol(10, 0x7f, 0, -1), -1);
        assert_eq!(SsUtKeyOffV(10), 0);
        assert_eq!(libspu::SpuGetKeyStatus(1 << 10), libspu::SPU_OFF);
        SsUtAllKeyOff(0);
        let seq = [0u32; 4];
        assert_eq!(SsSeqOpen(seq.as_ptr(), 0), 0);
        SsSeqSetVol(0, 0x50, 0x50);
        SsSeqPlay(0, 1, 0);
        SsSeqStop(0);
        SsSeqClose(0);
        SsSetMVol(127, 127);
        SsStart();
        assert!(with_state(|s| s.started));
        SsEnd();
        SsQuit();
        assert!(with_state(|s| !s.initialized));
    }
}
