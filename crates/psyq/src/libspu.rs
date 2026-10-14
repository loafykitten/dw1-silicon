//! libspu: the SPU as the game and libsnd drive it, with the SPU model (`backend::spu`) behind
//! it.
//!
//! Everything reaches the hardware model as register writes, the way the SDK reaches the real
//! SPU, so the port's sound can be checked against the emulator's register traffic
//! (`oracle/capture-spu.lua`, `tools/oracle/spu_events.py`; docs/phase6-plan.md). With
//! `DW1_SPU_LOG=<file>` every write is logged in the capture's format ("vblank address width
//! value cycle", the VBlank in the port's numbering, cycle 0), followed by the pad polls so far
//! and the VBlanks since the latest, which is how a replay lines the two up. Sound RAM transfers (DMA) are
//! not register traffic and aren't logged. `DW1_WAV=<file>` writes everything rendered to a
//! 44.1 kHz stereo WAV file. Both are flushed when the program exits.
//!
//! Time: the model runs on the virtual clock. [`frame`] (called by libsnd once per VBlank)
//! renders the VBlank's samples, so the voices' envelopes, and with them `SpuGetKeyStatus`,
//! advance with the game whether or not anything is listening; the samples go to the host's
//! sink ([`set_audio_sink`]), if there is one.
//!
//! libsnd and libspu share one SPU. Lock order when both are needed: libsnd's state first,
//! then this one.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::{LazyLock, Mutex, MutexGuard};

use backend::spu::{RAM_SIZE, Spu};

/// Voices on the SPU.
pub const VOICES: usize = 24;
/// Bits of the voices that exist, in a voice mask.
pub const ALL_VOICES: u32 = (1 << VOICES) - 1;

pub const SPU_SUCCESS: i32 = 0;
pub const SPU_ERROR: i32 = -1;
pub const SPU_OFF: i32 = 0;
pub const SPU_ON: i32 = 1;
/// Voice status: key is off but the envelope is still releasing.
pub const SPU_OFF_ENV_ON: i32 = 2;
/// Voice status: key is on but the envelope has finished.
pub const SPU_ON_ENV_OFF: i32 = 3;
/// `SpuSetReverbVoice` mode: the bit pattern replaces the current one.
pub const SPU_BIT: i32 = 8;

/// `SpuReverbAttr.mask` bits.
pub const SPU_REV_MODE: u32 = 1 << 0;
pub const SPU_REV_DEPTHL: u32 = 1 << 1;
pub const SPU_REV_DEPTHR: u32 = 1 << 2;
pub const SPU_REV_DELAYTIME: u32 = 1 << 3;
pub const SPU_REV_FEEDBACK: u32 = 1 << 4;
/// Every reverb attribute (what a zero mask means).
const REVERB_ALL: u32 =
    SPU_REV_MODE | SPU_REV_DEPTHL | SPU_REV_DEPTHR | SPU_REV_DELAYTIME | SPU_REV_FEEDBACK;
/// `SpuReverbAttr.mode` flag: clear the work area when the mode is set.
pub const SPU_REV_MODE_CLEAR_WA: i32 = 0x100;
/// Reverb modes are `0..SPU_REV_MODE_MAX`.
pub const SPU_REV_MODE_MAX: i32 = 10;

/// Reverb work area size by mode, at the top of sound RAM. [doc] Psy-Q Run-Time Library
/// Reference 4.4, "SpuSetReverbModeParam" table (mode "off" uses 0 or 0x80; 0 here).
pub const REVERB_AREA_SIZE: [usize; SPU_REV_MODE_MAX as usize] = [
    0, 0x26c0, 0x1f40, 0x4840, 0x6fe0, 0xade0, 0xf6c0, 0x18040, 0x18040, 0x3c00,
];

/// The reverb registers 0x1F801DC0-0x1F801DFF a mode sets, for the modes the game uses: mode 3
/// (`SPU_REV_MODE_STUDIO_B`) [obs: `SpuSetReverbModeParam` in the emulator,
/// captures/oracle-runs/p6-spu-calls2; the same values as psx-spx's "Studio Medium" preset].
fn reverb_preset(mode: i32) -> Option<[u16; 32]> {
    match mode {
        3 => Some([
            0x00b1, 0x007f, 0x70f0, 0x4fa8, 0xbce0, 0x4510, 0xbef0, 0xb4c0, 0x5280, 0x4ec0, 0x0904,
            0x076b, 0x0824, 0x065f, 0x07a2, 0x0616, 0x076c, 0x05ed, 0x05ec, 0x042e, 0x050f, 0x0305,
            0x0462, 0x02b7, 0x042f, 0x0265, 0x0264, 0x01b2, 0x0100, 0x0080, 0x8000, 0x8000,
        ]),
        _ => None,
    }
}

/// Register offsets from 0x1F801C00 (psx-spx, "SPU Control and Status Register Summary").
pub mod reg {
    /// Voice `n`'s registers start at `16 n`: volume left/right, pitch, start address (in
    /// 8-byte units), ADSR low/high, ADSR level, repeat address.
    pub const fn voice(n: usize, field: u16) -> u16 {
        (n as u16) * 16 + field
    }
    pub const VOL_L: u16 = 0x0;
    pub const VOL_R: u16 = 0x2;
    pub const PITCH: u16 = 0x4;
    pub const START: u16 = 0x6;
    pub const ADSR1: u16 = 0x8;
    pub const ADSR2: u16 = 0xA;
    pub const ENVX: u16 = 0xC;
    pub const REPEAT: u16 = 0xE;
    pub const MAIN_VOL_L: u16 = 0x180;
    pub const MAIN_VOL_R: u16 = 0x182;
    pub const REVERB_VOL_L: u16 = 0x184;
    pub const REVERB_VOL_R: u16 = 0x186;
    pub const KON: u16 = 0x188;
    pub const KOFF: u16 = 0x18C;
    pub const PMON: u16 = 0x190;
    pub const NON: u16 = 0x194;
    pub const EON: u16 = 0x198;
    pub const ENDX: u16 = 0x19C;
    pub const REVERB_BASE: u16 = 0x1A2;
    pub const TRANSFER_ADDR: u16 = 0x1A6;
    pub const TRANSFER_FIFO: u16 = 0x1A8;
    pub const SPUCNT: u16 = 0x1AA;
    pub const TRANSFER_CTRL: u16 = 0x1AC;
    pub const CD_VOL_L: u16 = 0x1B0;
    pub const CD_VOL_R: u16 = 0x1B2;
    pub const EXT_VOL_L: u16 = 0x1B4;
    pub const EXT_VOL_R: u16 = 0x1B6;
    pub const REVERB_PARAMS: u16 = 0x1C0;
}

/// The looping silent block `SsInit` writes, and what libsnd's first VAB is loaded after.
const SILENT_BLOCK: u16 = 0x1000;

/// SPUCNT bits.
const SPUCNT_ENABLE: u16 = 0x8000;
const SPUCNT_UNMUTE: u16 = 0x4000;
const SPUCNT_REVERB: u16 = 0x0080;
const SPUCNT_CD_AUDIO: u16 = 0x0001;

/// CPU cycles per VBlank in the emulator [obs: PLAN.md, "Field timing"; `stream.rs`].
pub const CYCLES_PER_VBLANK: u64 = 564_398;
/// CPU cycles per 44.1 kHz sample: 33.8688 MHz / 44100 [doc: psx-spx, SPU].
pub const CYCLES_PER_SAMPLE: u64 = 768;

/// `SpuVolume` (libspu.h).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpuVolume {
    pub left: i16,
    pub right: i16,
}

/// `SpuReverbAttr` (libspu.h).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpuReverbAttr {
    pub mask: u32,
    pub mode: i32,
    pub depth: SpuVolume,
    pub delay: i32,
    pub feedback: i32,
}

/// The reverb settings as libspu keeps them (the registers hold the rest).
#[derive(Clone, Debug, Default)]
pub struct Reverb {
    pub enabled: bool,
    pub mode: i32,
    pub depth: SpuVolume,
    pub delay: i32,
    pub feedback: i32,
}

enum LogEntry {
    Write(u16, u16),
    Call(&'static str, [u32; 3]),
}

/// Where rendered samples go: 44.1 kHz stereo, L and R interleaved.
pub type AudioSink = Box<dyn FnMut(&[i16]) + Send>;

pub struct SpuState {
    pub hw: Spu,
    /// Keyed voices as libspu counts them: set by a key on, cleared by a key off.
    keyed: u32,
    pub reverb: Reverb,
    log: Option<BufWriter<File>>,
    /// This VBlank's writes and SDK calls, logged when it ends.
    log_pending: Vec<LogEntry>,
    /// The VBlank logged writes are stamped with.
    pub log_vblank: u32,
    /// Cycles rendered into the current VBlank, and the cycles left over below one sample.
    frame_cycles: u64,
    sample_carry: u64,
    sink: Option<AudioSink>,
    buf: Vec<i16>,
}

impl SpuState {
    pub fn new() -> Self {
        Self {
            hw: Spu::new(),
            keyed: 0,
            reverb: Reverb::default(),
            log: None,
            log_pending: Vec::new(),
            log_vblank: 0,
            frame_cycles: 0,
            sample_carry: 0,
            sink: None,
            buf: Vec::new(),
        }
    }

    /// Logs register writes to `path` from now on.
    pub fn open_log(&mut self, path: &str) -> std::io::Result<()> {
        self.log = Some(BufWriter::new(File::create(path)?));
        Ok(())
    }

    pub fn flush_log(&mut self) {
        self.write_log_pending();
        if let Some(log) = &mut self.log {
            let _ = log.flush();
        }
    }

    /// Writes the VBlank's logged writes, each followed by the pad polls so far and the VBlanks
    /// since the latest: the port's VBlanks don't line up with the emulator's (loading is
    /// instant), its polls do. Stamped at the end of the VBlank, so a write the game makes
    /// before a poll in the same VBlank counts after it, like the libsnd tick that follows.
    fn write_log_pending(&mut self) {
        let Some(log) = &mut self.log else { return };
        let polls = crate::libetc::polls();
        let since = self
            .log_vblank
            .saturating_sub(crate::libetc::last_poll_vblank());
        for entry in self.log_pending.drain(..) {
            let _ = match entry {
                LogEntry::Write(off, v) => writeln!(
                    log,
                    "{} {:08x} 2 {v:x} 0 {polls} {since}",
                    self.log_vblank,
                    0x1F80_1C00 + u32::from(off)
                ),
                LogEntry::Call(name, args) => writeln!(
                    log,
                    "{} call {name} {} 0 0 0 0 0 0 {polls} {since}",
                    self.log_vblank,
                    args.iter()
                        .map(|a| format!("{a:08x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            };
        }
    }

    /// Logs an SDK call with its first three arguments (the capture's `DW1_SPU_CALLS` lines
    /// have more; the comparison reads the first ones).
    pub fn log_call(&mut self, name: &'static str, args: [u32; 3]) {
        if self.log.is_some() {
            self.log_pending.push(LogEntry::Call(name, args));
        }
    }

    /// A register write, `off` from 0x1F801C00.
    pub fn write(&mut self, off: u16, v: u16) {
        self.hw.write16(u32::from(off), v);
        if self.log.is_some() {
            self.log_pending.push(LogEntry::Write(off, v));
        }
    }

    pub fn read(&self, off: u16) -> u16 {
        self.hw.read16(u32::from(off))
    }

    /// Back to power-on: the model reset (sound RAM kept, as on the hardware), every voice
    /// off, reverb off.
    pub fn reset(&mut self) {
        self.hw.reset();
        self.keyed = 0;
        self.reverb = Reverb::default();
    }

    /// Copies `data` to sound RAM at `addr` (a DMA transfer). False (nothing written) if it
    /// won't fit.
    pub fn write_ram(&mut self, addr: usize, data: &[u8]) -> bool {
        match addr.checked_add(data.len()) {
            Some(end) if end <= RAM_SIZE => {
                self.hw.ram_mut()[addr..end].copy_from_slice(data);
                true
            }
            _ => false,
        }
    }

    pub fn ram(&self) -> &[u8] {
        self.hw.ram()
    }

    /// Writes a 24-bit voice mask to a pair of registers (low 16 voices, then the high 8).
    /// Halves with no bit set aren't written when `skip_zero` (a zero write to KON or KOFF does
    /// nothing).
    fn write_mask(&mut self, off: u16, mask: u32, skip_zero: bool) {
        let (lo, hi) = (mask as u16, ((mask >> 16) & 0xff) as u16);
        if lo != 0 || !skip_zero {
            self.write(off, lo);
        }
        if hi != 0 || !skip_zero {
            self.write(off + 2, hi);
        }
    }

    /// Keys on the voices in `mask`.
    pub fn key_on(&mut self, mask: u32) {
        let mask = mask & ALL_VOICES;
        self.write_mask(reg::KON, mask, true);
        self.keyed |= mask;
    }

    /// Keys off the voices in `mask`.
    pub fn key_off(&mut self, mask: u32) {
        let mask = mask & ALL_VOICES;
        self.write_mask(reg::KOFF, mask, true);
        self.keyed &= !mask;
    }

    /// The voices that feed the reverb (EON).
    pub fn set_reverb_voices(&mut self, mask: u32) {
        self.write_mask(reg::EON, mask & ALL_VOICES, false);
    }

    pub fn reverb_voices(&self) -> u32 {
        u32::from(self.read(reg::EON)) | (u32::from(self.read(reg::EON + 2)) & 0xff) << 16
    }

    /// The voices using noise (NON).
    pub fn set_noise_voices(&mut self, mask: u32) {
        self.write_mask(reg::NON, mask & ALL_VOICES, false);
    }

    /// Voice `n`'s envelope level (ENVX).
    pub fn envelope(&self, n: usize) -> u16 {
        self.read(reg::voice(n, reg::ENVX))
    }

    /// `SpuGetKeyStatus`: the status of the voice in `voice_bit`; with several bits set, the
    /// lowest-numbered voice [doc: Psy-Q reference]. -1 if there's no valid voice [doc]: no bit
    /// set, or the lowest above voice 23 [guess]. The four states [doc]: key on or off, by
    /// libspu's own count, and the envelope (ENVX) at 0 or not.
    pub fn key_status(&self, voice_bit: u32) -> i32 {
        let n = voice_bit.trailing_zeros() as usize;
        if voice_bit == 0 || n >= VOICES {
            return SPU_ERROR;
        }
        let env = self.envelope(n) != 0;
        match (self.keyed & (1 << n) != 0, env) {
            (true, true) => SPU_ON,
            (true, false) => SPU_ON_ENV_OFF,
            (false, true) => SPU_OFF_ENV_ON,
            (false, false) => SPU_OFF,
        }
    }

    /// The SPU control register as `SsInit` leaves it: on, unmuted [obs: 0xC000].
    pub fn spucnt(&self) -> u16 {
        self.read(reg::SPUCNT)
    }

    /// The reverb work area for `mode`: (start, length) at the top of sound RAM.
    pub fn reverb_area(mode: i32) -> Option<(usize, usize)> {
        let size = *REVERB_AREA_SIZE.get(usize::try_from(mode).ok()?)?;
        Some((RAM_SIZE - size, size))
    }

    /// Zeroes the work area of `mode`. False for an invalid mode.
    pub fn clear_reverb_area(&mut self, mode: i32) -> bool {
        let Some((start, len)) = Self::reverb_area(mode) else {
            return false;
        };
        self.hw.ram_mut()[start..start + len].fill(0);
        true
    }

    /// `SpuSetReverbModeParam`. [doc] Psy-Q reference: SPU_ERROR for an invalid mode (then
    /// nothing is set), else 0. The "work area allocated with SpuMalloc" error needs SpuMalloc,
    /// which the game doesn't call. Setting the mode writes its parameters and the work area's
    /// start, and zeroes the reverb depth whatever the mask says [obs: the game passes depth
    /// 0x7000 here and the emulator shows 0 written, then 0x7000 from `SpuSetReverbDepth`].
    pub fn set_reverb_mode_param(&mut self, attr: &SpuReverbAttr) -> i32 {
        // [doc] A zero mask selects every attribute.
        let mask = if attr.mask == 0 {
            REVERB_ALL
        } else {
            attr.mask
        };
        let mode = attr.mode & 0xff;
        if mask & SPU_REV_MODE != 0 && !(0..SPU_REV_MODE_MAX).contains(&mode) {
            return SPU_ERROR;
        }
        if mask & SPU_REV_MODE != 0 {
            let Some(params) = reverb_preset(mode) else {
                crate::trap::unimplemented("SpuSetReverbModeParam: a reverb mode DW1 doesn't use");
            };
            self.reverb.mode = mode;
            self.write(reg::REVERB_VOL_L, 0);
            self.write(reg::REVERB_VOL_R, 0);
            self.reverb.depth = SpuVolume::default();
            for (i, &v) in params.iter().enumerate() {
                self.write(reg::REVERB_PARAMS + 2 * i as u16, v);
            }
            if attr.mode & SPU_REV_MODE_CLEAR_WA != 0 {
                self.clear_reverb_area(mode);
            }
            let (start, _) = Self::reverb_area(mode).expect("valid mode");
            self.write(reg::REVERB_BASE, (start / 8) as u16);
        }
        if mask & SPU_REV_DELAYTIME != 0 {
            self.reverb.delay = attr.delay;
        }
        if mask & SPU_REV_FEEDBACK != 0 {
            self.reverb.feedback = attr.feedback;
        }
        SPU_SUCCESS
    }

    /// `SpuSetReverbDepth`: the left/right depth the mask selects, to the reverb volume
    /// registers [obs: depth 0x7000 writes 0x7000]. [doc] A zero mask selects both channels.
    pub fn set_reverb_depth(&mut self, attr: &SpuReverbAttr) {
        let mask = if attr.mask == 0 {
            REVERB_ALL
        } else {
            attr.mask
        };
        if mask & SPU_REV_DEPTHL != 0 {
            self.reverb.depth.left = attr.depth.left;
            self.write(reg::REVERB_VOL_L, attr.depth.left as u16);
        }
        if mask & SPU_REV_DEPTHR != 0 {
            self.reverb.depth.right = attr.depth.right;
            self.write(reg::REVERB_VOL_R, attr.depth.right as u16);
        }
    }

    /// `SpuSetReverbVoice`: returns the reverb voice bits (low 24) after the change.
    pub fn set_reverb_voice(&mut self, on_off: i32, voice_bit: u32) -> u32 {
        let bits = voice_bit & ALL_VOICES;
        let now = self.reverb_voices();
        let new = match on_off {
            SPU_ON => now | bits,
            SPU_OFF => now & !bits,
            SPU_BIT => bits,
            _ => now,
        };
        self.set_reverb_voices(new);
        new
    }

    /// `SpuSetReverb`: the reverb master switch (SPUCNT bit 7) [doc: psx-spx].
    pub fn set_reverb(&mut self, on: bool) {
        self.reverb.enabled = on;
        let cnt = self.spucnt();
        self.write(
            reg::SPUCNT,
            if on {
                cnt | SPUCNT_REVERB
            } else {
                cnt & !SPUCNT_REVERB
            },
        );
    }

    /// `SsInit`'s writes [obs: the emulator at the game's `SsInit`, in this order]: SPU off,
    /// volumes and voice masks cleared, a 16-byte looping block at 0x1000 (eight halfwords
    /// 0x0707 through the transfer FIFO), every voice set to play it at volume 0 and keyed on
    /// then off (which leaves each voice's hardware state settled), then the SPU on and unmuted,
    /// the reverb work area at the top of sound RAM (0xFFFE x 8), every voice's registers reset
    /// (pitch 0x1000, sample 0x18000, ADSR 0x00BF / 0), main volume 0x3FFF and the voice masks
    /// cleared. libsnd then resets its voices as `SsUtAllKeyOff` does.
    pub fn init_registers(&mut self) {
        self.reset();
        self.write(reg::SPUCNT, 0);
        self.write(reg::MAIN_VOL_L, 0);
        self.write(reg::MAIN_VOL_R, 0);
        self.write(reg::TRANSFER_CTRL, 4);
        self.write(reg::REVERB_VOL_L, 0);
        self.write(reg::REVERB_VOL_R, 0);
        self.write(reg::KOFF, 0xffff);
        self.write(reg::KOFF + 2, 0xffff);
        self.write_mask(reg::EON, 0, false);
        self.write_mask(reg::PMON, 0, false);
        self.write_mask(reg::NON, 0, false);
        for off in [reg::CD_VOL_L, reg::CD_VOL_R, reg::EXT_VOL_L, reg::EXT_VOL_R] {
            self.write(off, 0);
        }
        self.write(reg::TRANSFER_ADDR, SILENT_BLOCK / 8);
        for _ in 0..8 {
            self.write(reg::TRANSFER_FIFO, 0x0707);
        }
        self.write(reg::SPUCNT, 0x10);
        self.write(reg::SPUCNT, 0);
        for n in 0..VOICES {
            self.write(reg::voice(n, reg::VOL_L), 0);
            self.write(reg::voice(n, reg::VOL_R), 0);
            self.write(reg::voice(n, reg::PITCH), 0x3fff);
            self.write(reg::voice(n, reg::START), SILENT_BLOCK / 8);
            self.write(reg::voice(n, reg::ADSR1), 0);
            self.write(reg::voice(n, reg::ADSR2), 0);
        }
        self.write_mask(reg::KON, ALL_VOICES, false);
        self.write_mask(reg::KOFF, ALL_VOICES, false);
        self.keyed = 0;
        self.write(reg::SPUCNT, SPUCNT_ENABLE | SPUCNT_UNMUTE);
        self.write(reg::REVERB_BASE, 0xfffe);
        for n in 0..VOICES {
            for (field, v) in [
                (reg::VOL_L, 0),
                (reg::VOL_R, 0),
                (reg::PITCH, 0x1000),
                (reg::START, 0x3000),
                (reg::ADSR1, 0xbf),
                (reg::ADSR2, 0),
                (reg::ENVX, 0),
                (reg::REPEAT, 0),
            ] {
                self.write(reg::voice(n, field), v);
            }
        }
        self.write(reg::MAIN_VOL_L, 0x3fff);
        self.write(reg::MAIN_VOL_R, 0x3fff);
        self.write(reg::REVERB_VOL_L, 0);
        self.write(reg::REVERB_VOL_R, 0);
        for off in [
            reg::KON,
            reg::KOFF,
            reg::PMON,
            reg::NON,
            reg::EON,
            reg::ENDX,
        ] {
            self.write_mask(off, 0, false);
        }
    }

    /// What libcd's `CdInit` writes to the SPU once the drive has reset [obs: the emulator, 7
    /// VBlanks after each of the game's two `CdInit` calls]: main and CD audio volume full, CD
    /// audio on. (The game's `SsInit` later turns CD audio off again.)
    pub fn cd_init_volume(&mut self) {
        self.write(reg::MAIN_VOL_L, 0x3fff);
        self.write(reg::MAIN_VOL_R, 0x3fff);
        self.write(reg::CD_VOL_L, 0x3fff);
        self.write(reg::CD_VOL_R, 0x3fff);
        let cnt = self.spucnt();
        self.write(reg::SPUCNT, cnt | SPUCNT_CD_AUDIO);
    }

    /// Renders the samples up to `cycle` cycles into the current VBlank.
    pub fn render_to(&mut self, cycle: u64) {
        if cycle <= self.frame_cycles {
            return;
        }
        let cycles = cycle - self.frame_cycles + self.sample_carry;
        self.frame_cycles = cycle;
        let samples = (cycles / CYCLES_PER_SAMPLE) as usize;
        self.sample_carry = cycles % CYCLES_PER_SAMPLE;
        if samples == 0 {
            return;
        }
        self.buf.resize(samples * 2, 0);
        // No CD audio input yet (XA is Phase 6's slice 5).
        self.hw.render(&mut self.buf, &mut || (0, 0));
        if let Some(sink) = &mut self.sink {
            sink(&self.buf);
        }
    }

    /// Finishes the VBlank: renders the rest of it, logs its writes.
    pub fn end_frame(&mut self) {
        self.render_to(CYCLES_PER_VBLANK);
        self.frame_cycles = 0;
        self.write_log_pending();
    }
}

impl Default for SpuState {
    fn default() -> Self {
        Self::new()
    }
}

/// Tests that touch the global SPU or libsnd state hold this, since they share it.
#[cfg(test)]
pub(crate) static GLOBAL_TEST: Mutex<()> = Mutex::new(());

static SPU: LazyLock<Mutex<SpuState>> = LazyLock::new(|| {
    let mut spu = SpuState::new();
    if let Ok(path) = std::env::var("DW1_SPU_LOG")
        && let Err(e) = spu.open_log(&path)
    {
        eprintln!("psyq: DW1_SPU_LOG {path}: {e}");
    }
    if let Ok(path) = std::env::var("DW1_WAV") {
        match Wav::create(&path) {
            Ok(wav) => {
                let wav = std::sync::Arc::new(Mutex::new(wav));
                *WAV.lock().unwrap() = Some(wav.clone());
                spu.sink = Some(Box::new(move |s| wav.lock().unwrap().push(s)));
            }
            Err(e) => eprintln!("psyq: DW1_WAV {path}: {e}"),
        }
    }
    // SAFETY: registering a plain extern "C" function.
    unsafe { atexit(flush_at_exit) };
    Mutex::new(spu)
});

unsafe extern "C" {
    fn atexit(f: extern "C" fn()) -> i32;
}

/// Flushes the log and the WAV file (the game's exits are `exit` calls). Skips anything whose
/// lock is held, rather than hang the exit.
extern "C" fn flush_at_exit() {
    if let Ok(mut spu) = SPU.try_lock() {
        spu.flush_log();
    }
    if let Ok(wav) = WAV.try_lock()
        && let Some(wav) = wav.as_ref()
        && let Ok(mut wav) = wav.try_lock()
    {
        wav.finish();
    }
}

static WAV: Mutex<Option<std::sync::Arc<Mutex<Wav>>>> = Mutex::new(None);

/// A 16-bit stereo 44.1 kHz WAV file being written; the header's sizes are filled in by
/// [`Wav::finish`].
struct Wav {
    file: BufWriter<File>,
    bytes: u32,
}

impl Wav {
    fn create(path: &str) -> std::io::Result<Wav> {
        let mut wav = Wav {
            file: BufWriter::new(File::create(path)?),
            bytes: 0,
        };
        wav.header()?;
        Ok(wav)
    }

    fn header(&mut self) -> std::io::Result<()> {
        let f = &mut self.file;
        f.write_all(b"RIFF")?;
        f.write_all(&36u32.saturating_add(self.bytes).to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        // PCM, 2 channels, 44100 Hz, 4 bytes a frame, 16 bits.
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&1u16.to_le_bytes())?;
        f.write_all(&2u16.to_le_bytes())?;
        f.write_all(&44100u32.to_le_bytes())?;
        f.write_all(&(44100u32 * 4).to_le_bytes())?;
        f.write_all(&4u16.to_le_bytes())?;
        f.write_all(&16u16.to_le_bytes())?;
        f.write_all(b"data")?;
        f.write_all(&self.bytes.to_le_bytes())
    }

    fn push(&mut self, samples: &[i16]) {
        // A RIFF file can't describe more than 4 GiB (6 h 45 min of audio): stop there.
        let add = 2 * samples.len() as u32;
        if self.bytes.checked_add(add + 36).is_none() {
            if self.bytes != u32::MAX {
                eprintln!("psyq: DW1_WAV: 4 GiB reached, the rest isn't written");
                self.finish();
                self.bytes = u32::MAX;
            }
            return;
        }
        for s in samples {
            let _ = self.file.write_all(&s.to_le_bytes());
        }
        self.bytes += add;
    }

    fn finish(&mut self) {
        use std::io::Seek;
        let _ = self.file.flush();
        if self.file.seek(std::io::SeekFrom::Start(0)).is_ok() {
            let _ = self.header();
            let _ = self.file.flush();
        }
        let _ = self.file.seek(std::io::SeekFrom::End(0));
    }
}

/// The SPU state, locked. For libsnd, tests, and the host's audio side.
pub fn state() -> MutexGuard<'static, SpuState> {
    SPU.lock().unwrap()
}

/// Runs `f` on the SPU state.
pub fn with_state<R>(f: impl FnOnce(&mut SpuState) -> R) -> R {
    f(&mut state())
}

/// Sends the rendered samples to `sink` from now on (the window's audio stream, a WAV file).
pub fn set_audio_sink(sink: AudioSink) {
    state().sink = Some(sink);
}

/// Flushes the register log, if there is one (at exit).
pub fn flush_log() {
    state().flush_log();
}

/// `SpuGetKeyStatus(voice_bit)`: see [`SpuState::key_status`].
#[unsafe(no_mangle)]
pub extern "C" fn SpuGetKeyStatus(voice_bit: u32) -> i32 {
    state().key_status(voice_bit)
}

/// `SpuSetReverb(on_off)`: [doc] returns the reverb on/off value.
#[unsafe(no_mangle)]
pub extern "C" fn SpuSetReverb(on_off: i32) -> i32 {
    let on = on_off == SPU_ON;
    state().set_reverb(on);
    if on { SPU_ON } else { SPU_OFF }
}

/// `SpuSetReverbModeParam(attr)`.
///
/// # Safety
/// `attr` must point to a valid `SpuReverbAttr` (or be NULL, which returns `SPU_ERROR`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SpuSetReverbModeParam(attr: *const SpuReverbAttr) -> i32 {
    // SAFETY: the caller's contract.
    match unsafe { attr.as_ref() } {
        Some(attr) => state().set_reverb_mode_param(attr),
        None => SPU_ERROR,
    }
}

/// `SpuClearReverbWorkArea(rev_mode)`: [doc] 0, or SPU_ERROR for a bad mode.
#[unsafe(no_mangle)]
pub extern "C" fn SpuClearReverbWorkArea(rev_mode: i32) -> i32 {
    if state().clear_reverb_area(rev_mode) {
        SPU_SUCCESS
    } else {
        SPU_ERROR
    }
}

/// `SpuSetReverbDepth(attr)`: [doc] returns 0.
///
/// # Safety
/// `attr` must point to a valid `SpuReverbAttr` (or be NULL, which does nothing).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SpuSetReverbDepth(attr: *const SpuReverbAttr) -> i32 {
    // SAFETY: the caller's contract.
    if let Some(attr) = unsafe { attr.as_ref() } {
        state().set_reverb_depth(attr);
    }
    SPU_SUCCESS
}

/// `SpuSetReverbVoice(on_off, voice_bit)`: [doc] returns the voices' reverb bits afterwards.
/// libsnd's tick rewrites the bits from its own voices' settings [obs: the emulator shows EON
/// rewritten on every tick], so what the game sets here lasts until the next tick.
#[unsafe(no_mangle)]
pub extern "C" fn SpuSetReverbVoice(on_off: i32, voice_bit: u32) -> u32 {
    state().set_reverb_voice(on_off, voice_bit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A voice playing a looping block of silence-but-nonzero samples at full envelope.
    fn keyed_voice(spu: &mut SpuState, n: usize) {
        // One ADPCM block at 0x1000 that loops on itself (flags: start, repeat, end).
        let mut block = [0u8; 16];
        block[1] = 0x07;
        block[2..].fill(0x11);
        spu.write_ram(0x1000, &block);
        spu.write(reg::voice(n, reg::START), 0x1000 / 8);
        spu.write(reg::voice(n, reg::PITCH), 0x1000);
        // Fastest attack, sustain at max.
        spu.write(reg::voice(n, reg::ADSR1), 0x00ff);
        spu.write(reg::voice(n, reg::ADSR2), 0x0000);
        spu.key_on(1 << n);
    }

    #[test]
    fn key_status_follows_keys_and_the_envelope() {
        let mut spu = SpuState::new();
        assert_eq!(spu.key_status(1 << 10), SPU_OFF);
        keyed_voice(&mut spu, 10);
        // Keyed, but no sample rendered yet: the envelope is still 0.
        assert_eq!(spu.key_status(1 << 10), SPU_ON_ENV_OFF);
        spu.render_to(CYCLES_PER_SAMPLE * 64);
        assert_eq!(spu.key_status(1 << 10), SPU_ON);
        assert_eq!(spu.key_status(1 << 11), SPU_OFF);
        // Several bits: the lowest voice.
        assert_eq!(spu.key_status(0b11 << 10), SPU_ON);
        assert_eq!(spu.key_status(0), SPU_ERROR);
        assert_eq!(spu.key_status(1 << 24), SPU_ERROR);
        spu.key_off(1 << 10);
        assert_eq!(spu.key_status(1 << 10), SPU_OFF_ENV_ON);
    }

    #[test]
    fn rendering_follows_the_cycle_clock() {
        let mut spu = SpuState::new();
        let got = std::sync::Arc::new(Mutex::new(0usize));
        let g = got.clone();
        spu.sink = Some(Box::new(move |s| *g.lock().unwrap() += s.len() / 2));
        for _ in 0..60 {
            spu.end_frame();
        }
        // 60 VBlanks of 564,398 cycles at 768 cycles a sample.
        assert_eq!(
            *got.lock().unwrap() as u64,
            60 * CYCLES_PER_VBLANK / CYCLES_PER_SAMPLE
        );
    }

    #[test]
    fn ram_writes_are_bounds_checked() {
        let mut spu = SpuState::new();
        assert!(spu.write_ram(RAM_SIZE - 4, &[1, 2, 3, 4]));
        assert_eq!(&spu.ram()[RAM_SIZE - 4..], &[1, 2, 3, 4]);
        assert!(!spu.write_ram(RAM_SIZE - 3, &[1, 2, 3, 4]));
        assert!(!spu.write_ram(usize::MAX, &[1]));
    }

    #[test]
    fn reverb_setup_as_the_game_does_it() {
        let mut spu = SpuState::new();
        spu.init_registers();
        let (start, len) = SpuState::reverb_area(3).unwrap();
        assert_eq!((start, len), (RAM_SIZE - 0x4840, 0x4840));
        spu.hw.ram_mut()[start] = 0xaa;
        spu.hw.ram_mut()[start - 1] = 0xbb;
        let attr = SpuReverbAttr {
            mask: SPU_REV_MODE | SPU_REV_DEPTHL | SPU_REV_DEPTHR,
            mode: SPU_REV_MODE_CLEAR_WA | 3,
            depth: SpuVolume {
                left: 0x7000,
                right: 0x7000,
            },
            delay: 0,
            feedback: 0,
        };
        assert_eq!(spu.set_reverb_mode_param(&attr), SPU_SUCCESS);
        assert_eq!(spu.reverb.mode, 3);
        assert_eq!(
            spu.read(reg::REVERB_VOL_L),
            0,
            "setting the mode zeroes the depth"
        );
        assert_eq!(spu.read(reg::REVERB_BASE), 0xf6f8);
        assert_eq!(spu.read(reg::REVERB_PARAMS), 0x00b1);
        assert_eq!(spu.read(reg::REVERB_PARAMS + 62), 0x8000);
        assert_eq!(spu.ram()[start], 0, "work area cleared");
        assert_eq!(spu.ram()[start - 1], 0xbb, "nothing below it touched");
        spu.set_reverb_depth(&attr);
        assert_eq!(
            (spu.read(reg::REVERB_VOL_L), spu.read(reg::REVERB_VOL_R)),
            (0x7000, 0x7000)
        );
        assert_eq!(spu.set_reverb_voice(SPU_ON, 0x7fffff), 0x7fffff);
        assert_eq!((spu.read(reg::EON), spu.read(reg::EON + 2)), (0xffff, 0x7f));
        spu.set_reverb(true);
        assert_eq!(spu.spucnt(), 0xc080);
        // A bad mode sets nothing.
        let bad = SpuReverbAttr {
            mask: SPU_REV_MODE,
            mode: 10,
            ..attr
        };
        assert_eq!(spu.set_reverb_mode_param(&bad), SPU_ERROR);
        assert_eq!(spu.reverb.mode, 3);
        assert!(!spu.clear_reverb_area(10));
        assert!(!spu.clear_reverb_area(-1));
    }

    #[test]
    fn reverb_voice_bits() {
        let mut spu = SpuState::new();
        assert_eq!(spu.set_reverb_voice(SPU_ON, 0x7fffff), 0x7fffff);
        assert_eq!(spu.set_reverb_voice(SPU_OFF, 0x3), 0x7ffffc);
        assert_eq!(spu.set_reverb_voice(SPU_BIT, 0xff00_0001), 1);
    }

    #[test]
    fn writes_are_logged_in_the_capture_format() {
        let dir = std::env::temp_dir().join(format!("dw1-spu-log-{}", std::process::id()));
        let mut spu = SpuState::new();
        spu.open_log(dir.to_str().unwrap()).unwrap();
        spu.log_vblank = 7;
        spu.key_on(1 << 17);
        spu.flush_log();
        let text = std::fs::read_to_string(&dir).unwrap();
        std::fs::remove_file(&dir).unwrap();
        assert!(text.starts_with("7 1f801d8a 2 2 0 "), "{text}");
    }

    #[test]
    fn c_interface_reverb_and_key_status() {
        let _guard = GLOBAL_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let attr = SpuReverbAttr {
            mask: SPU_REV_DEPTHL,
            depth: SpuVolume { left: 5, right: 9 },
            ..Default::default()
        };
        // SAFETY: `attr` is a valid SpuReverbAttr.
        unsafe { SpuSetReverbDepth(&attr) };
        assert_eq!(
            with_state(|s| s.reverb.depth),
            SpuVolume { left: 5, right: 0 }
        );
        assert_eq!(SpuSetReverb(SPU_ON), SPU_ON);
        assert!(with_state(|s| s.reverb.enabled));
        assert_eq!(SpuSetReverb(SPU_OFF), SPU_OFF);
        assert_eq!(SpuClearReverbWorkArea(3), SPU_SUCCESS);
        assert_eq!(SpuClearReverbWorkArea(99), SPU_ERROR);
        assert_eq!(SpuGetKeyStatus(1 << 12), SPU_OFF);
        // SAFETY: NULL is allowed.
        assert_eq!(
            unsafe { SpuSetReverbModeParam(std::ptr::null()) },
            SPU_ERROR
        );
    }
}
