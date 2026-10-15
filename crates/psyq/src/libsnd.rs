//! libsnd: VAB banks, SEQ handles, and the voices it keys on, through libspu to the SPU model.
//!
//! VAB ids open and close, a VAB's sample body lands in sound RAM at the address the game chose,
//! SEQ access numbers are handed out. Key-ons (`SsUtKeyOnV`, the game's sound effects) turn the
//! VAB's tone attributes into voice registers, and libsnd's tick writes them to the SPU, as the
//! SDK does: a call only records what to do, and the tick that follows does it (writes the
//! voice's pitch, volume, sample address and envelope, then key off, key on, the reverb and noise
//! voice masks) [obs: the emulator's SPU traffic, docs/phase6-plan.md]. The tick runs at 240 Hz
//! (`SsSetTickMode(SS_TICK240)`) on the virtual clock: [`frame`] runs each VBlank's ticks, with
//! the SPU rendering in between. There is no sequencer yet (Phase 6, slice 3): a SEQ told to play
//! stays "playing" and makes no sound. The game never waits on a SEQ ending (it never calls
//! `SsIsEos`).
//!
//! Semantics are from the Psy-Q Run-Time Library Reference 4.4 (the `[doc]` marks) and from the
//! SDK run as a black box in the emulator (`[obs]`: `oracle/sdk-probe.lua` calls an SDK function
//! with chosen arguments and records what it returns and writes; `oracle/capture-spu.lua` logs a
//! route's calls and SPU writes); anything neither settles is a `[guess]`. The VAB header layout
//! is the one libsnd.h declares (`VabHdr`, `ProgAtr`, `VagAtr`), followed by the 256-entry table
//! of sample sizes in 8-byte units.
//!
//! Lock order: this module's state, then libspu's.

use std::sync::{LazyLock, Mutex, MutexGuard};

use backend::spu::RAM_SIZE;

use crate::libspu::{self, CYCLES_PER_VBLANK, SPU_ON, SpuState, VOICES, reg};
use crate::sequencer::{Event, Player};

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
/// `VabHdr.form`: the word "VABp" in little-endian order, so the bytes read "pBAV" [obs: the
/// disc's banks; psx-spx VH format].
const VAB_FORM: [u8; 4] = *b"pBAV";

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
/// header doesn't contain valid data" is an open failure; validity here is the "VABp" tag (bytes "pBAV") and
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vab {
    /// Where the body goes in sound RAM (the game's `sbaddr`).
    pub sb_addr: u32,
    pub body_size: usize,
    pub info: VhInfo,
    /// `SsVabTransBody` has run.
    pub transferred: bool,
    /// The VH. "Sticky" [doc]: the SDK keeps reading the caller's header instead of copying it;
    /// DW1 never changes a header it has opened (it closes and reopens to load another bank),
    /// so a copy reads the same.
    head: Vec<u8>,
}

/// A tone's attributes (`VagAtr`, libsnd.h), the fields a key-on uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    /// Bit 2: the voice feeds the reverb [obs: tone mode 4 sets the voice's EON bit, mode 0
    /// clears it; 1,456 probes].
    pub mode: u8,
    pub vol: u8,
    pub pan: u8,
    /// The note the sample plays at its own rate, and a fine tune in 1/128 semitones.
    pub center: u8,
    pub shift: u8,
    pub adsr1: u16,
    pub adsr2: u16,
    /// Sample number: an index into the sample size table.
    pub vag: u16,
    /// The notes the tone plays (the sequencer keys every tone of the program whose range holds
    /// the note), and the pitch bend range down and up, in semitones.
    pub min: u8,
    pub max: u8,
    pub pb_min: u8,
    pub pb_max: u8,
}

impl Vab {
    fn u16_at(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.head[at], self.head[at + 1]])
    }

    /// The program's volume (`ProgAtr.mvol`), if the program has tones.
    fn program_volume(&self, prog: usize) -> Option<u8> {
        let at = size_of::<VabHdr>() + prog * PROG_ATTR_SIZE;
        (prog < VAB_PROGRAMS && self.head[at] != 0).then(|| self.head[at + 1])
    }

    /// The tone table holds 16 entries for each program in use, in program order [doc: psx-spx,
    /// VAB format]. DW1's banks number their programs from 0 without gaps but two of the music
    /// banks (FAALL.VHB), and those are never keyed directly.
    fn tone(&self, prog: usize, tone: usize) -> Option<Tone> {
        self.program_volume(prog)?;
        let slot = (0..prog)
            .filter(|&p| self.program_volume(p).is_some())
            .count();
        let at = size_of::<VabHdr>()
            + VAB_PROGRAMS * PROG_ATTR_SIZE
            + (slot * VAB_TONES_PER_PROGRAM + tone) * TONE_ATTR_SIZE;
        let t = self.head.get(at..at + TONE_ATTR_SIZE)?;
        Some(Tone {
            mode: t[1],
            vol: t[2],
            pan: t[3],
            center: t[4],
            shift: t[5],
            adsr1: u16::from_le_bytes([t[16], t[17]]),
            adsr2: u16::from_le_bytes([t[18], t[19]]),
            vag: u16::from_le_bytes([t[22], t[23]]),
            min: t[6],
            max: t[7],
            pb_min: t[12],
            pb_max: t[13],
        })
    }

    /// The program's tones whose note range holds `note`, in tone order.
    fn tones_for(&self, prog: usize, note: u8) -> Vec<Tone> {
        let at = size_of::<VabHdr>() + prog * PROG_ATTR_SIZE;
        if prog >= VAB_PROGRAMS {
            return Vec::new();
        }
        let count = usize::from(self.head[at]).min(VAB_TONES_PER_PROGRAM);
        (0..count)
            .filter_map(|t| self.tone(prog, t))
            .filter(|t| (t.min..=t.max).contains(&note))
            .collect()
    }

    /// The sample's sound RAM address: the body's, plus the sizes of the samples before it
    /// (table entry 0 counts) [obs: 103 key-ons in captures/oracle-runs/p6-spu-calls2].
    fn sample_address(&self, vag: u16) -> u32 {
        let table = self.info.vh_size - VAG_TABLE_ENTRIES * 2;
        let units: u32 = (0..usize::from(vag).min(VAG_TABLE_ENTRIES))
            .map(|i| u32::from(self.u16_at(table + i * 2)))
            .sum();
        self.sb_addr + units * 8
    }

    fn master(&self) -> (u8, u8) {
        (self.head[24], self.head[25])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqStatus {
    Opened,
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Debug)]
pub struct Seq {
    /// Address of the SEQ data in the game's memory. The data is copied when the SEQ opens
    /// (the game reuses its buffer).
    pub addr: usize,
    pub vab_id: i16,
    pub status: SeqStatus,
    pub vol_left: i16,
    pub vol_right: i16,
    /// `SsSeqPlay`'s play count; 0 (`SSPLAY_INFINITY`) loops forever.
    pub loops: i16,
    /// The score, if the data parsed as a SEQ.
    player: Option<Player>,
    pub channels: [Channel; 16],
}

/// A MIDI channel's state as the sequencer keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    pub program: u8,
    /// Controllers 7 (volume), 11 (expression) and 10 (pan).
    pub volume: u8,
    pub expression: u8,
    pub pan: u8,
}

impl Default for Channel {
    /// [guess] DW1's music sets program, volume and pan on every channel before its first note.
    fn default() -> Self {
        Self {
            program: 0,
            volume: 127,
            expression: 127,
            pan: 64,
        }
    }
}

/// Which note of which SEQ a voice was last keyed for. The SDK keeps this after the note ends,
/// and the channel's later controller changes, pitch bends and note-offs still reach the voice
/// [obs: -seqC, a note-off keys off every voice with that note, a volume change rewrites
/// voices already released], until the voice is keyed again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub seq: usize,
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
    pub tone: Tone,
    pub prog_vol: u8,
}

/// What `SsUtKeyOnV` asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyOn {
    pub vab: i16,
    pub prog: i16,
    pub tone: i16,
    pub note: i16,
    pub fine: i16,
    pub vol_left: i16,
    pub vol_right: i16,
}

/// A voice as libsnd keeps it: the register values its next tick writes, and what that tick
/// does to the voice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SndVoice {
    pub pitch: u16,
    pub vol: (u16, u16),
    pub start: u16,
    pub adsr: (u16, u16),
    /// Feeds the reverb (written to EON every tick).
    pub reverb: bool,
    /// Registers to write at the next tick.
    dirty_pitch: bool,
    dirty_vol: bool,
    dirty_sample: bool,
    /// Key on / key off at the next tick.
    pub key_on: bool,
    pub key_off: bool,
    /// A volume sweep (`SsUtAutoVol`) in progress.
    pub auto_vol: Option<AutoVol>,
    /// The SEQ note the voice belongs to, if the sequencer keyed it.
    pub owner: Option<Owner>,
    /// When it was last keyed (a count of key-ons): the sequencer takes the oldest voice when
    /// none is free.
    pub age: u32,
}

/// An `SsUtAutoVol` sweep: the voice's volume, as a fraction of what it was keyed with in
/// 127ths, moves from `start` to `end` by `step` a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoVol {
    pub level: i32,
    pub end: i32,
    pub step: i32,
}

/// The SPU's ticks per second in `SS_TICK240` mode, in CPU cycles: 33,868,800 / 240 [doc: the
/// tick mode's name; the CPU clock, psx-spx]. A free-running timer, not tied to the VBlank: 240
/// Hz is a little under 4 ticks a VBlank, so about one VBlank in 1,721 has 3 [obs: the
/// emulator's `SsSeqCalledTbyT` calls over 29,000 VBlanks of the care replay, 3-tick VBlanks
/// 1,720-1,722 apart, which is these two clocks exactly].
pub const CYCLES_PER_TICK: u64 = 141_120;
/// Where in the VBlank the first tick after `SsStart` falls. The emulator's phase isn't a fixed
/// thing (the 3-tick VBlanks fall at 3117 on one route and 3186 on another with the same boot),
/// so it isn't modelled; the port's ticks are at most one tick (1/240 s) away from the
/// emulator's.
const TICK_PHASE_AT_START: u64 = 0;
/// `SsSetTickMode` modes. [doc]
pub const SS_TICK240: i32 = 2;

pub struct SndState {
    pub initialized: bool,
    /// `SsStart` has run (and `SsEnd` hasn't).
    pub started: bool,
    /// `SsEnd` has run (and `SsInit` hasn't since).
    pub ended: bool,
    pub tick_mode: i32,
    pub master_vol: (i16, i16),
    pub vabs: [Option<Vab>; MAX_VABS],
    /// SEQ access numbers are indexes into this; its length is `SsSetTableSize`'s `s_max`.
    pub seqs: Vec<Option<Seq>>,
    /// `SsSetTableSize`'s `t_max` (SEQs per SEP). Kept; SEP isn't used.
    pub seq_tunes: i16,
    pub voices: [SndVoice; VOICES],
    /// Cycles into the next VBlank of the next 240 Hz tick.
    next_tick: u64,
    /// Key-ons so far (the voices' ages).
    key_ons: u32,
    /// The latest `SsUtKeyOnV` call's tone, its program's volume and its left/right volumes,
    /// which every `SsUtAutoVol` step uses, whichever voice it sweeps (see
    /// [`SndState::auto_vol`]).
    last_key_on: Option<(Tone, u8, i32, i32)>,
}

/// Pitch for a note `semitones` above the center, before octaves: one row per semitone, one
/// column per 1/16 semitone. [obs: `SsPitchFromNote` called 48,897 times in the emulator
/// (captures/oracle-runs/p6-probe-pitch); `SsUtKeyOnV` keys the same pitches. Each entry is
/// near 4096 x 2^(semitone/12 + column/192) but not a formula of it, so the measured table is
/// what's kept.]
#[rustfmt::skip]
const PITCH_TABLE: [[u16; 16]; 12] = [
    [4096, 4110, 4125, 4140, 4155, 4170, 4185, 4200, 4216, 4231, 4246, 4261, 4277, 4292, 4308, 4323],
    [4339, 4355, 4371, 4386, 4402, 4418, 4434, 4450, 4466, 4482, 4499, 4515, 4531, 4548, 4564, 4581],
    [4597, 4614, 4630, 4647, 4664, 4681, 4698, 4715, 4732, 4749, 4766, 4783, 4801, 4818, 4835, 4853],
    [4870, 4888, 4906, 4924, 4941, 4959, 4977, 4995, 5013, 5031, 5050, 5068, 5086, 5105, 5123, 5142],
    [5160, 5179, 5198, 5216, 5235, 5254, 5273, 5292, 5311, 5331, 5350, 5369, 5389, 5408, 5428, 5447],
    [5467, 5487, 5507, 5527, 5547, 5567, 5587, 5607, 5627, 5648, 5668, 5688, 5709, 5730, 5750, 5771],
    [5792, 5813, 5834, 5855, 5876, 5898, 5919, 5940, 5962, 5983, 6005, 6027, 6049, 6070, 6092, 6114],
    [6137, 6159, 6181, 6203, 6226, 6248, 6271, 6294, 6316, 6339, 6362, 6385, 6408, 6431, 6455, 6478],
    [6501, 6525, 6549, 6572, 6596, 6620, 6644, 6668, 6692, 6716, 6741, 6765, 6789, 6814, 6839, 6863],
    [6888, 6913, 6938, 6963, 6988, 7014, 7039, 7064, 7090, 7116, 7141, 7167, 7193, 7219, 7245, 7271],
    [7298, 7324, 7351, 7377, 7404, 7431, 7458, 7485, 7512, 7539, 7566, 7593, 7621, 7648, 7676, 7704],
    [7732, 7760, 7788, 7816, 7844, 7873, 7901, 7930, 7958, 7987, 8016, 8045, 8074, 8103, 8133, 8162],
];

/// `SsPitchFromNote(note, fine, center, shift)`: the SPU pitch (4096 = the sample's own rate)
/// for `note` on a sample recorded at `center`; `fine` and `shift` add 1/128 semitones each,
/// used in steps of 8. Octaves shift the table's value; the result is 16 bits. [obs: exact for
/// every probe with the note at most 60 semitones below the center; further down the SDK's
/// results are garbage (0, 31, 127...), which no DW1 bank asks for, so they trap.]
pub fn pitch_from_note(note: i16, fine: i16, center: u8, shift: u8) -> u16 {
    let fine = i32::from(fine) + i32::from(shift);
    let semitones = i32::from(note) - i32::from(center) + fine.div_euclid(128);
    if semitones < -60 {
        crate::trap::unimplemented("SsPitchFromNote: a note more than 5 octaves below the center");
    }
    let (octave, semitone) = (semitones.div_euclid(12), semitones.rem_euclid(12));
    let p = u32::from(PITCH_TABLE[semitone as usize][(fine.rem_euclid(128) >> 3) as usize]);
    (if octave >= 0 {
        p << octave
    } else {
        p >> -octave
    }) as u16
}

/// One side's share of a pan (0 = left, 64 = centre, 127 = right) in 63rds: full on its own
/// side and at the centre, falling to 0 at the far side. [obs]
fn pan_share(pan: i32) -> (i32, i32) {
    (
        if pan <= 64 { 63 } else { 127 - pan },
        if pan >= 64 { 63 } else { pan },
    )
}

/// `SsUtKeyOnV`'s left and right volumes as a volume (the louder) and a pan (0-127, 64 the
/// centre: the quieter side's share of the louder, in 64ths). [obs: the key-on probes]
fn key_on_pan(vol_left: i32, vol_right: i32) -> (i32, i32) {
    if vol_left == vol_right {
        (vol_left, 64)
    } else if vol_left > vol_right {
        (vol_left, vol_right * 64 / vol_left)
    } else {
        (vol_right, 127 - vol_left * 64 / vol_right)
    }
}

/// One side's share of a pan in 64ths, as `SsUtAutoVol`'s steps take it: full on its own side,
/// 63/64 at the centre, falling to 0 at the far side. [obs: below]
fn pan_share_64(pan: i32) -> (i32, i32) {
    ((127 - pan).min(64), pan.min(64))
}

/// The voice volume registers at one step of an `SsUtAutoVol` sweep of a voice `SsUtKeyOnV`
/// keyed. Not the key-on's formula: the step's `level` (0-127) takes the place of the key-on's
/// volume, tone volume x program volume x 0x3FFF / 127^3 in one division, then the pans in 64ths
/// (where the key-on's are in 63rds, so a centred voice steps at 63/64 per centred pan on the
/// left and full on the right): the tone's, then a third one at the centre (the program's or the
/// bank's: 64 in every DW1 bank, which `key_on_v` checks), then the voice's from the key-on's
/// left and right volumes, each rounded down.
/// [obs: exact for 500 probes of `SsUtKeyOnV` then `SsUtAutoVol` and the tick
/// (captures/oracle-runs/p9-sound-autovol-*: tone volumes 0-127, tone pans 0-127, program
/// volumes 64-127, key-on volumes 1-127 on either side, sweeps up and down over 3-15 ticks),
/// the 64 of p6-probe-autovol and the battle replay's fades; the other orders of the three pans
/// each miss some.]
pub fn auto_vol_volume(
    level: i32,
    vol_left: i32,
    vol_right: i32,
    tone: &Tone,
    prog_vol: u8,
) -> (u16, u16) {
    let (_, pan) = key_on_pan(vol_left, vol_right);
    let base =
        i64::from(level) * i64::from(tone.vol) * i64::from(prog_vol) * 0x3fff / (127 * 127 * 127);
    let (tl, tr) = pan_share_64(i32::from(tone.pan));
    let (cl, cr) = pan_share_64(64);
    let (vl, vr) = pan_share_64(pan);
    let side = |t: i32, c: i32, v: i32| {
        (base * i64::from(t) / 64 * i64::from(c) / 64 * i64::from(v) / 64) as u16
    };
    (side(tl, cl, vl), side(tr, cr, vr))
}

/// The voice volume registers for a key-on: `SsUtKeyOnV`'s left and right volumes become a
/// volume (the louder) and a pan (the quieter one's share of it), which with the tone's and the
/// program's volume and the tone's pan give each side, in 0x3FFF units. [obs: exact for all
/// 6,235 probes of `SsUtKeyOnV` (captures/oracle-runs/p6-probe-keyon, -keyon2), which cover
/// tone volumes 0-127, tone pans 1-127 and program volumes 100-127. Every bank on DW1's disc
/// has master volume 127 and pan 64, and the sound effect banks program pan 64, so those
/// don't enter; a bank that differed traps.]
pub fn key_on_volume(vol_left: i32, vol_right: i32, tone: &Tone, prog_vol: u8) -> (u16, u16) {
    let (vol, pan) = key_on_pan(vol_left, vol_right);
    // The probes fit several orders of these products; this one stays in 32 bits.
    let base = vol * i32::from(tone.vol) * 0x3fff / (127 * 127) * i32::from(prog_vol) / 127;
    let (tl, tr) = pan_share(i32::from(tone.pan));
    let (vl, vr) = pan_share(pan);
    (
        (base * tl / 63 * vl / 63) as u16,
        (base * tr / 63 * vr / 63) as u16,
    )
}

/// A SEQ note's pitch: like [`pitch_from_note`] with `fine` (a pitch bend, in 1/128 semitones)
/// and the tone's fine tune, except that a note more than 60 semitones below the tone's centre
/// gives 0 [obs: -seqA, note 20 on a tone centred on 81].
pub fn seq_pitch(note: u8, fine: i32, tone: &Tone) -> u16 {
    let total = (i32::from(note) - i32::from(tone.center)) * 128 + fine + i32::from(tone.shift);
    if total.div_euclid(128) < -60 {
        return 0;
    }
    pitch_from_note(i16::from(note), fine as i16, tone.center, tone.shift)
}

/// A SEQ voice's volume registers [obs: exact for 981 probes with synthetic SEQs,
/// captures/oracle-runs/p6-probe-seqvol, -seqvol2, -seqA, -seqD, -seqE]: a linear volume,
/// 0x3FFF x tone volume x program volume x (velocity x channel volume / 127, rounded down)
/// / 127^3 in one division, then the expression and the SEQ's volume for that side (127ths);
/// then the channel's pan share and the tone's (63rds, as for `SsUtKeyOnV`); then squared, in
/// 0x3FFF units. The squaring is what makes MIDI volumes sound even.
pub fn seq_volume(o: &Owner, ch: &Channel, seq_vol: (i16, i16)) -> (u16, u16) {
    let v7 = i64::from(o.velocity) * i64::from(ch.volume) / 127;
    let base = 0x3fff * i64::from(o.tone.vol) * i64::from(o.prog_vol) * v7 / (127 * 127 * 127)
        * i64::from(ch.expression)
        / 127;
    let (cl, cr) = pan_share(i32::from(ch.pan));
    let (tl, tr) = pan_share(i32::from(o.tone.pan));
    let side = |seq: i16, c: i32, t: i32| {
        let v = base * i64::from(seq.clamp(0, 127)) / 127 * i64::from(c) / 63 * i64::from(t) / 63;
        (v * v / 0x3fff) as u16
    };
    (side(seq_vol.0, cl, tl), side(seq_vol.1, cr, tr))
}

impl SndState {
    pub fn new() -> Self {
        Self {
            initialized: false,
            started: false,
            ended: false,
            tick_mode: 0,
            master_vol: (0, 0),
            vabs: std::array::from_fn(|_| None),
            seqs: Vec::new(),
            seq_tunes: 0,
            voices: [SndVoice::default(); VOICES],
            next_tick: 0,
            key_ons: 0,
            last_key_on: None,
        }
    }

    fn vab(&self, id: i16) -> Option<&Vab> {
        self.vabs.get(usize::try_from(id).ok()?)?.as_ref()
    }

    /// `SsInit`: libsnd and the SPU back to power-on.
    pub fn init(&mut self, spu: &mut SpuState) {
        *self = Self::new();
        self.initialized = true;
        spu.init_registers();
        self.all_key_off(spu);
    }

    /// `SsQuit`: closes everything. It writes nothing to the SPU [obs: no SPU writes between the
    /// game's `SsEnd`/`SsQuit` and its next `SsInit`]; sound RAM keeps its contents.
    pub fn quit(&mut self) {
        *self = Self::new();
    }

    /// `SsEnd`: stops the tick altogether [obs: as for `SsQuit`].
    pub fn end(&mut self) {
        self.started = false;
        self.ended = true;
    }

    /// `SsSetTableSize`. [guess] it drops SEQs already open; the game calls it once, right
    /// after `SsInit`.
    pub fn set_table_size(&mut self, s_max: i16, t_max: i16) {
        self.seqs = vec![None; usize::try_from(s_max).unwrap_or(0).min(MAX_SEQ)];
        self.seq_tunes = t_max;
    }

    /// `SsStart`: starts the tick timer.
    pub fn start(&mut self) {
        self.started = true;
        self.next_tick = TICK_PHASE_AT_START;
    }

    /// `SsSetMVol`: the main volume registers, 127 = 0x3FFF [obs: 127 writes 0x3FFF; other
    /// values are a guess on the same scale].
    pub fn set_master_volume(&mut self, spu: &mut SpuState, left: i16, right: i16) {
        self.master_vol = (left, right);
        let scale = |v: i16| (i32::from(v.clamp(0, 127)) * 0x3fff / 127) as u16;
        spu.write(reg::MAIN_VOL_L, scale(left));
        spu.write(reg::MAIN_VOL_R, scale(right));
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
            head: head[..info.vh_size].to_vec(),
        });
        id as i16
    }

    /// `SsVabTransBody`: copies the body to sound RAM at the VAB's `sb_addr`. The transfer is
    /// done when this returns. Returns the VAB id; -1 if that VAB isn't open or its body was
    /// already sent [guess for the second case]. `body` must hold `body_size` bytes.
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
    /// never sized: [guess], the SDK needs `SsSetTableSize` first). `data` is the SEQ, copied
    /// (None, or data that doesn't parse, opens a SEQ that never plays).
    pub fn seq_open(&mut self, data: Option<&[u8]>, addr: usize, vab_id: i16) -> i16 {
        match self.seqs.iter().position(Option::is_none) {
            Some(i) => {
                self.seqs[i] = Some(Seq {
                    addr,
                    vab_id,
                    status: SeqStatus::Opened,
                    vol_left: 0,
                    vol_right: 0,
                    loops: 0,
                    player: data.and_then(Player::new),
                    channels: [Channel::default(); 16],
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

    /// `SsSeqPlay`: `SSPLAY_PLAY` (1) plays from the top, `loops` times (0 = forever);
    /// `SSPLAY_PAUSE` (0) pauses.
    pub fn seq_play(&mut self, access: i16, mode: i8, loops: i16) {
        if let Some(seq) = self.seq_mut(access) {
            seq.loops = loops;
            if mode == 1 {
                seq.status = SeqStatus::Playing;
                if let Some(p) = seq.player.as_mut() {
                    p.play(i32::from(loops));
                }
            } else {
                seq.status = SeqStatus::Paused;
            }
        }
    }

    /// `SsSeqStop`: the SEQ's voices silenced (volume 0) and keyed off [obs: -seqC].
    pub fn seq_stop(&mut self, access: i16) {
        if let Some(seq) = self.seq_mut(access) {
            seq.status = SeqStatus::Stopped;
            self.silence_seq(access as usize);
        }
    }

    fn silence_seq(&mut self, seq: usize) {
        for v in &mut self.voices {
            if v.owner.is_some_and(|o| o.seq == seq) {
                v.vol = (0, 0);
                v.dirty_vol = true;
                v.key_off = true;
                v.key_on = false;
            }
        }
    }

    /// `SsSeqSetVol`: the SEQ's volume, applied to its voices at once [obs: the next tick
    /// rewrites their volume].
    pub fn seq_set_vol(&mut self, access: i16, left: i16, right: i16) {
        let Some(seq) = self.seq_mut(access) else {
            return;
        };
        (seq.vol_left, seq.vol_right) = (left, right);
        let (vol, channels) = ((left, right), seq.channels);
        for v in &mut self.voices {
            if let Some(o) = v.owner.filter(|o| o.seq == access as usize) {
                v.vol = seq_volume(&o, &channels[usize::from(o.channel & 15)], vol);
                v.dirty_vol = true;
            }
        }
    }

    /// The voice for a SEQ key-on: the lowest-numbered free one, else the one keyed longest
    /// ago [obs: -seqC, chords of up to 30 notes]. Free is its envelope finished and its note
    /// off (keyed off, or a key-off due at the next tick), with no key-on due: a voice still
    /// holding a note whose sample has ended isn't free, one whose note-off came this tick is
    /// [obs: battle-twice replay, both cases].
    fn allocate(&self, spu: &SpuState) -> usize {
        (0..VOICES)
            .find(|&n| {
                let v = &self.voices[n];
                !v.key_on && (v.key_off || !spu.is_keyed(n)) && spu.envelope(n) == 0
            })
            .unwrap_or_else(|| {
                (0..VOICES)
                    .min_by_key(|&n| {
                        std::cmp::Reverse(self.key_ons.wrapping_sub(self.voices[n].age))
                    })
                    .unwrap_or(0)
            })
    }

    /// One tick of every playing SEQ: its due events applied to the voices, for the next tick
    /// to write.
    fn run_sequencer(&mut self, spu: &SpuState) {
        let mut events = Vec::new();
        for access in 0..self.seqs.len() {
            let Some(seq) = self.seqs[access].as_mut() else {
                continue;
            };
            if seq.status != SeqStatus::Playing {
                continue;
            }
            let Some(player) = seq.player.as_mut() else {
                continue;
            };
            events.clear();
            player.tick(&mut events);
            for &e in &events {
                self.seq_event(spu, access, e);
            }
        }
    }

    fn seq_event(&mut self, spu: &SpuState, access: usize, e: Event) {
        let Some(seq) = self.seqs[access].as_ref() else {
            return;
        };
        let (vol, vab_id, channels) = ((seq.vol_left, seq.vol_right), seq.vab_id, seq.channels);
        match e {
            Event::Note {
                channel,
                note,
                velocity,
            } if velocity > 0 => {
                let ch = channels[usize::from(channel & 15)];
                let Some(vab) = self.vab(vab_id) else { return };
                let prog = usize::from(ch.program);
                let Some(prog_vol) = vab.program_volume(prog) else {
                    return;
                };
                let tones = vab.tones_for(prog, note);
                let addrs: Vec<u16> = tones
                    .iter()
                    .map(|t| (vab.sample_address(t.vag) / 8) as u16)
                    .collect();
                for (tone, start) in tones.into_iter().zip(addrs) {
                    let n = self.allocate(spu);
                    let owner = Owner {
                        seq: access,
                        channel,
                        note,
                        velocity,
                        tone,
                        prog_vol,
                    };
                    self.voices[n] = SndVoice {
                        pitch: seq_pitch(note, 0, &tone),
                        vol: seq_volume(&owner, &ch, vol),
                        start,
                        adsr: (tone.adsr1, tone.adsr2),
                        reverb: tone.mode & 4 != 0,
                        dirty_pitch: true,
                        dirty_vol: true,
                        dirty_sample: true,
                        key_on: true,
                        key_off: false,
                        auto_vol: None,
                        owner: Some(owner),
                        age: self.key_ons,
                    };
                    self.key_ons = self.key_ons.wrapping_add(1);
                }
            }
            Event::Note { channel, note, .. } => {
                for v in &mut self.voices {
                    if v.owner
                        .is_some_and(|o| o.seq == access && o.channel == channel && o.note == note)
                    {
                        v.key_off = true;
                        v.key_on = false;
                    }
                }
            }
            Event::Control {
                channel,
                control,
                value,
            } => {
                let mut ch = channels[usize::from(channel & 15)];
                match control {
                    7 => ch.volume = value,
                    10 => ch.pan = value,
                    11 => ch.expression = value,
                    _ => return,
                }
                if let Some(seq) = self.seqs[access].as_mut() {
                    seq.channels[usize::from(channel & 15)] = ch;
                }
                for v in &mut self.voices {
                    if let Some(o) = v.owner.filter(|o| o.seq == access && o.channel == channel) {
                        v.vol = seq_volume(&o, &ch, vol);
                        v.dirty_vol = true;
                    }
                }
            }
            Event::Program { channel, program } => {
                if let Some(seq) = self.seqs[access].as_mut() {
                    seq.channels[usize::from(channel & 15)].program = program;
                }
            }
            Event::Bend { channel, msb } => {
                for v in &mut self.voices {
                    let Some(o) = v.owner.filter(|o| o.seq == access && o.channel == channel)
                    else {
                        continue;
                    };
                    let range = i32::from(if msb >= 64 {
                        o.tone.pb_max
                    } else {
                        o.tone.pb_min
                    });
                    if range == 0 {
                        continue;
                    }
                    // In 1/128 semitones, rounded down [obs: a bend through every value,
                    // captures/oracle-runs/p6-probe-bend]; the low byte doesn't count.
                    let fine = ((i32::from(msb) - 64) * range * 128).div_euclid(63);
                    v.pitch = seq_pitch(o.note, fine, &o.tone);
                    v.dirty_pitch = true;
                }
            }
            Event::End => {
                if let Some(seq) = self.seqs[access].as_mut() {
                    seq.status = SeqStatus::Stopped;
                }
                self.silence_seq(access);
            }
        }
    }

    /// `SsUtKeyOnV`. [doc] ranges: voice 0-23, prog 0-127, tone 0-15, note, fine and volumes
    /// 0-127; the VAB must be open. A voice still sounding from an earlier key-on (keyed and
    /// its envelope not finished, or keyed at a tick still to come) is refused [obs: probes
    /// re-keying a voice playing a looped sample fail, and succeed once it's keyed off]. The
    /// registers are written at the next tick. Returns the voice, or -1.
    pub fn key_on_v(&mut self, spu: &SpuState, voice: i16, key: KeyOn) -> i16 {
        let in_range = |v: i16, max: i16| (0..=max).contains(&v);
        let ok = in_range(voice, VOICES as i16 - 1)
            && in_range(key.prog, 127)
            && in_range(key.tone, 15)
            && in_range(key.note, 127)
            && in_range(key.fine, 127)
            && in_range(key.vol_left, 127)
            && in_range(key.vol_right, 127);
        if !ok {
            return -1;
        }
        let Some(vab) = self.vab(key.vab) else {
            return -1;
        };
        let (prog, tone_index) = (key.prog as usize, key.tone as usize);
        let (Some(tone), Some(prog_vol)) = (vab.tone(prog, tone_index), vab.program_volume(prog))
        else {
            return -1;
        };
        if vab.master() != (127, 64)
            || vab.head[size_of::<VabHdr>() + prog * PROG_ATTR_SIZE + 4] != 64
        {
            crate::trap::unimplemented(
                "SsUtKeyOnV: a bank whose master or program pan isn't DW1's",
            );
        }
        let start = (vab.sample_address(tone.vag) / 8) as u16;
        // Recorded before the voice is checked: a refused key-on still sets the tone the next
        // SsUtAutoVol steps use [obs: p9-sound-autovol-last3].
        self.last_key_on = Some((
            tone,
            prog_vol,
            i32::from(key.vol_left),
            i32::from(key.vol_right),
        ));
        let n = voice as usize;
        if self.voices[n].key_on || spu.key_status(1 << n) == SPU_ON {
            return -1;
        }
        let vol = key_on_volume(
            i32::from(key.vol_left),
            i32::from(key.vol_right),
            &tone,
            prog_vol,
        );
        let pitch = pitch_from_note(key.note, key.fine, tone.center, tone.shift);
        let v = &mut self.voices[n];
        *v = SndVoice {
            pitch,
            vol,
            start,
            adsr: (tone.adsr1, tone.adsr2),
            reverb: tone.mode & 4 != 0,
            dirty_pitch: true,
            dirty_vol: true,
            dirty_sample: true,
            key_on: true,
            key_off: false,
            auto_vol: None,
            owner: None,
            age: self.key_ons,
        };
        self.key_ons = self.key_ons.wrapping_add(1);
        voice
    }

    /// `SsUtAutoVol`: from the next tick, the voice's volume moves from `start` to `end` (0-127)
    /// by `(start - end) / ticks` a tick (rounded down, at least 1), one step a tick, landing on
    /// `end` (so 127 to 0 over 6 ticks takes 7: 106, 85, 64, 43, 22, 1, 0), each step written as
    /// [`auto_vol_volume`] [obs: the probes there].
    ///
    /// The tone, program volume and key-on volumes in each step aren't the swept voice's but
    /// those of the latest `SsUtKeyOnV` call at that tick, whichever voice it was for, even a
    /// refused one [obs: captures/oracle-runs/p9-sound-autovol-last1..3: a key-on of another
    /// voice before the sweep, or two ticks into it, changes the steps from then on; the battle
    /// replay's fades at VBlanks 4905 and 5961 follow the key-on of another voice just before,
    /// with music playing, so the sequencer's notes don't count]. So a sweep after
    /// `SsUtAllKeyOff` (the game's stopSound) brings the silenced voice's volume back for its
    /// steps [obs: p9-sound-autovol-allkeyoff].
    pub fn auto_vol(&mut self, voice: i16, start: i16, end: i16, ticks: i16) {
        let v = &mut self.voices[voice as usize];
        let (start, end) = (i32::from(start), i32::from(end));
        let step = ((start - end).abs() / i32::from(ticks)).max(1);
        v.auto_vol = Some(AutoVol {
            level: start,
            end,
            step,
        });
    }

    /// `SsUtKeyOffV`: keys the voice off at the next tick. [doc] 0, or -1 for a voice out of
    /// range.
    pub fn key_off_v(&mut self, voice: i16) -> i16 {
        match usize::try_from(voice) {
            Ok(n) if n < VOICES => {
                // The last of a key-on and a key-off before the tick wins [obs: -seqC].
                self.voices[n].key_off = true;
                self.voices[n].key_on = false;
                0
            }
            _ => -1,
        }
    }

    /// `SsUtAllKeyOff`: every voice set at once to pitch 0x1000, volume 0, the silent block
    /// and ADSR 0x80FF / 0x4000, and keyed off at the next tick; key-ons not yet done are
    /// dropped [obs: the game's stopSound in the battle replay, written in the call].
    pub fn all_key_off(&mut self, spu: &mut SpuState) {
        for (n, v) in self.voices.iter_mut().enumerate() {
            *v = SndVoice {
                pitch: 0x1000,
                vol: (0, 0),
                start: 0x1000 / 8,
                adsr: (0x80ff, 0x4000),
                key_off: true,
                age: v.age,
                owner: v.owner,
                ..SndVoice::default()
            };
            spu.write(reg::voice(n, reg::PITCH), v.pitch);
            spu.write(reg::voice(n, reg::VOL_L), 0);
            spu.write(reg::voice(n, reg::VOL_R), 0);
            spu.write(reg::voice(n, reg::START), v.start);
            spu.write(reg::voice(n, reg::ADSR1), v.adsr.0);
            spu.write(reg::voice(n, reg::ADSR2), v.adsr.1);
        }
    }

    /// One 240 Hz tick: each voice's changed registers, then key off, key on, the reverb and
    /// noise masks, in the order the emulator shows the SDK writing them. [obs]
    pub fn tick(&mut self, spu: &mut SpuState) {
        let (mut on, mut off, mut reverb) = (0u32, 0u32, 0u32);
        let last_key_on = self.last_key_on;
        for (n, v) in self.voices.iter_mut().enumerate() {
            if let Some(sweep) = v.auto_vol.as_mut() {
                let Some((tone, prog_vol, vl, vr)) = last_key_on else {
                    crate::trap::unimplemented("SsUtAutoVol before any SsUtKeyOnV");
                };
                sweep.level = if sweep.level > sweep.end {
                    (sweep.level - sweep.step).max(sweep.end)
                } else {
                    (sweep.level + sweep.step).min(sweep.end)
                };
                let level = sweep.level;
                v.vol = auto_vol_volume(level, vl, vr, &tone, prog_vol);
                v.dirty_vol = true;
                if level == sweep.end {
                    v.auto_vol = None;
                }
            }
            if v.dirty_pitch {
                spu.write(reg::voice(n, reg::PITCH), v.pitch);
            }
            if v.dirty_vol {
                spu.write(reg::voice(n, reg::VOL_L), v.vol.0);
                spu.write(reg::voice(n, reg::VOL_R), v.vol.1);
            }
            if v.dirty_sample {
                spu.write(reg::voice(n, reg::START), v.start);
                spu.write(reg::voice(n, reg::ADSR1), v.adsr.0);
                spu.write(reg::voice(n, reg::ADSR2), v.adsr.1);
            }
            (v.dirty_pitch, v.dirty_vol, v.dirty_sample) = (false, false, false);
            if std::mem::take(&mut v.key_on) {
                on |= 1 << n;
            }
            if std::mem::take(&mut v.key_off) {
                off |= 1 << n;
            }
            if v.reverb {
                reverb |= 1 << n;
            }
        }
        spu.key_off(off);
        spu.key_on(on);
        spu.set_reverb_voices(reverb);
        spu.set_noise_voices(0);
        // The SEQs' events come after the writes: what they key goes out at the next tick
        // [obs: a note at delta 0 sounds at the second tick after SsSeqPlay].
        self.run_sequencer(spu);
    }

    /// Runs one VBlank (`vblank`, the port's numbering) of sound time: the ticks that fall in
    /// it, with the SPU rendering up to each, then the rest of the VBlank.
    pub fn frame(&mut self, spu: &mut SpuState, vblank: u32) {
        spu.log_vblank = vblank;
        if self.started {
            while self.next_tick < CYCLES_PER_VBLANK {
                spu.render_to(self.next_tick);
                self.tick(spu);
                self.next_tick += CYCLES_PER_TICK;
            }
            self.next_tick -= CYCLES_PER_VBLANK;
        } else if self.initialized && !self.ended {
            // Between SsInit and SsStart the SDK ticks once a VBlank [obs].
            self.tick(spu);
        }
        spu.end_frame();
        spu.log_vblank = vblank + 1;
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

/// One VBlank of sound time (libetc calls this as each VBlank ends; `vblank` is the one
/// ending).
pub fn frame(vblank: u32) {
    let mut snd = state();
    snd.frame(&mut libspu::state(), vblank);
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
    state().quit();
}

/// `SsStart`: starts the tick.
#[unsafe(no_mangle)]
pub extern "C" fn SsStart() {
    state().start();
}

/// `SsEnd`: stops the tick.
#[unsafe(no_mangle)]
pub extern "C" fn SsEnd() {
    state().end();
}

/// `SsSetTickMode(mode)`: the tick rate. DW1 asks for `SS_TICK240` only.
#[unsafe(no_mangle)]
pub extern "C" fn SsSetTickMode(mode: i32) {
    if mode != SS_TICK240 {
        crate::trap::unimplemented("SsSetTickMode: a tick mode other than SS_TICK240");
    }
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
    let mut snd = state();
    snd.set_master_volume(&mut libspu::state(), left, right);
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

/// `SsVabTransCompleted(mode)`: [doc] 1 when the transfer is done. The transfer is done as
/// soon as `SsVabTransBody` returns, so this is always 1 and never waits.
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
///
/// # Safety
/// `addr` must point to a SEQ in readable memory (NULL returns -1).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SsSeqOpen(addr: *const u32, vab_id: i16) -> i16 {
    if addr.is_null() {
        return -1;
    }
    // SAFETY: the caller's contract; the copy stops at the SEQ's end marker.
    let data = unsafe { crate::sequencer::copy_seq(addr.cast()) };
    if data.is_none() {
        eprintln!("psyq: SsSeqOpen: no SEQ at {addr:p}; it won't play");
    }
    state().seq_open(data.as_deref(), addr as usize, vab_id)
}

/// `SsSeqClose(access)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqClose(access: i16) {
    state().seq_close(access);
}

/// `SsSeqPlay(access, mode, l_count)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqPlay(access: i16, mode: i8, l_count: i16) {
    let mut snd = state();
    libspu::with_state(|spu| {
        spu.log_call("SsSeqPlay", [access as u32, mode as u32, l_count as u32])
    });
    snd.seq_play(access, mode, l_count);
}

/// `SsSeqStop(access)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqStop(access: i16) {
    let mut snd = state();
    libspu::with_state(|spu| spu.log_call("SsSeqStop", [access as u32, 0, 0]));
    snd.seq_stop(access);
}

/// `SsSeqSetVol(access, left, right)`.
#[unsafe(no_mangle)]
pub extern "C" fn SsSeqSetVol(access: i16, left: i16, right: i16) {
    state().seq_set_vol(access, left, right);
}

/// `SsUtKeyOnV(voice, vabid, prog, tone, note, fine, voll, volr)`: keys a voice on at the next
/// tick. [doc] returns the voice number, -1 on failure.
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
    let mut snd = state();
    let mut spu = libspu::state();
    spu.log_call("SsUtKeyOnV", [voice as u32, vab as u32, prog as u32]);
    snd.key_on_v(&spu, voice, key)
}

/// `SsUtKeyOffV(voice)`: [doc] 0, or -1 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn SsUtKeyOffV(voice: i16) -> i16 {
    state().key_off_v(voice)
}

/// `SsUtAllKeyOff(mode)`: every voice off.
#[unsafe(no_mangle)]
pub extern "C" fn SsUtAllKeyOff(_mode: i16) {
    let mut snd = state();
    let mut spu = libspu::state();
    spu.log_call("SsUtAllKeyOff", [0, 0, 0]);
    snd.all_key_off(&mut spu);
}

/// `SsUtAutoVol(vc, start_vol, end_vol, delta_time)`: [doc] a volume sweep on a voice over
/// `delta_time` ticks; 0, or -1 for arguments out of range (voice 0-23, volumes 0-127, time
/// 0-10800). See [`SndState::auto_vol`].
#[unsafe(no_mangle)]
pub extern "C" fn SsUtAutoVol(voice: i16, start_vol: i16, end_vol: i16, delta_time: i16) -> i16 {
    let ok = (0..VOICES as i16).contains(&voice)
        && (0..=127).contains(&start_vol)
        && (0..=127).contains(&end_vol)
        && (0..=10800).contains(&delta_time);
    if !ok {
        return -1;
    }
    if delta_time == 0 {
        // [obs: a probe of SsUtAutoVol with delta_time 0 never returned to its caller]
        crate::trap::unimplemented("SsUtAutoVol: a sweep over 0 ticks (the SDK never returns)");
    }
    state().auto_vol(voice, start_vol, end_vol, delta_time);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A VH with `programs` programs (each with 2 tones), `samples` samples (numbered from 1)
    /// of the given sizes in bytes. Program `p`'s tone `t` plays sample `t + 1` at volume 127,
    /// pan 64, centre note 60, with the ADSR values 0x80ff / 0x5fd5; programs are at volume 127.
    fn make_vh(programs: u16, sizes: &[usize]) -> Vec<u8> {
        let mut vh = vec![0u8; vh_size(usize::from(programs))];
        vh[..4].copy_from_slice(b"pBAV");
        vh[18..20].copy_from_slice(&programs.to_le_bytes());
        vh[20..22].copy_from_slice(&(programs * 2).to_le_bytes());
        vh[22..24].copy_from_slice(&(sizes.len() as u16).to_le_bytes());
        vh[24] = 127;
        vh[25] = 64;
        for p in 0..usize::from(programs) {
            let at = 32 + p * PROG_ATTR_SIZE;
            vh[at] = 2;
            vh[at + 1] = 127;
            vh[at + 4] = 64;
            for t in 0..2 {
                let at = 32 + VAB_PROGRAMS * PROG_ATTR_SIZE + (p * 16 + t) * TONE_ATTR_SIZE;
                vh[at + 2] = 127;
                vh[at + 3] = 64;
                vh[at + 4] = 60;
                vh[at + 16..at + 18].copy_from_slice(&0x80ffu16.to_le_bytes());
                vh[at + 18..at + 20].copy_from_slice(&0x5fd5u16.to_le_bytes());
                vh[at + 22..at + 24].copy_from_slice(&(t as u16 + 1).to_le_bytes());
            }
        }
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
        assert_eq!(&spu.ram()[0xe800..0xe8c0], &body[..]);
        assert_eq!(spu.ram()[0xe7ff], 0);
        assert_eq!(spu.ram()[0xe8c0], 0);
        assert_eq!(snd.trans_body(&mut spu, &body, 1), -1, "sent twice");
        assert_eq!(snd.trans_body(&mut spu, &body, 2), -1, "not open");
        snd.close_vab(1);
        snd.close_vab(1);
        snd.close_vab(-3);
        snd.close_vab(99);
        assert_eq!(snd.vab_addr(1), u32::MAX);
        assert_eq!(snd.open_head_sticky(&vh, 1, 0xe800), 1);
        // The data stays in sound RAM across a close, like the SPU's.
        assert_eq!(spu.ram()[0xe800], body[0]);
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
        assert_eq!(snd.seq_open(None, 0x1000, 2), -1, "no table yet");
        snd.set_table_size(1, 1);
        let a = snd.seq_open(None, 0x1000, 2);
        assert_eq!(a, 0);
        assert_eq!(snd.seq_open(None, 0x2000, 2), -1, "table of one is full");
        snd.seq_set_vol(a, 0x50, 0x50);
        snd.seq_play(a, 1, 0);
        let seq = snd.seqs[0].clone().unwrap();
        assert_eq!(
            (seq.status, seq.vol_left, seq.vol_right, seq.loops),
            (SeqStatus::Playing, 0x50, 0x50, 0)
        );
        snd.seq_stop(a);
        assert_eq!(snd.seqs[0].as_ref().unwrap().status, SeqStatus::Stopped);
        // Bad access numbers do nothing.
        snd.seq_play(-1, 1, 0);
        snd.seq_stop(7);
        snd.seq_close(-1);
        snd.seq_close(a);
        assert_eq!(snd.seq_open(None, 0x3000, 2), 0, "the number is free again");
        snd.set_table_size(100, 1);
        assert_eq!(snd.seqs.len(), MAX_SEQ);
    }

    #[test]
    fn pitch_matches_the_sdk() {
        // [obs] SsPitchFromNote in the emulator.
        assert_eq!(pitch_from_note(0x3c, 0, 0x52, 0), 0x47d);
        assert_eq!(pitch_from_note(0x3c, 0, 0x54, 10), 0x403);
        assert_eq!(pitch_from_note(60, 64, 60, 64), 4339);
        assert_eq!(pitch_from_note(72, 0, 60, 0), 8192);
        assert_eq!(pitch_from_note(60, 0, 99, 0), 430);
        assert_eq!(pitch_from_note(48, 0, 60, 120), 2161);
    }

    #[test]
    fn key_on_volume_matches_the_sdk() {
        let tone = |vol, pan| Tone {
            mode: 0,
            vol,
            pan,
            center: 60,
            shift: 0,
            adsr1: 0,
            adsr2: 0,
            vag: 1,
            min: 0,
            max: 127,
            pb_min: 0,
            pb_max: 0,
        };
        // [obs] SsUtKeyOnV probes: (tone volume, tone pan, program volume, voll, volr).
        assert_eq!(
            key_on_volume(127, 127, &tone(127, 64), 127),
            (0x3fff, 0x3fff)
        );
        assert_eq!(key_on_volume(1, 2, &tone(127, 64), 127), (131, 258));
        assert_eq!(key_on_volume(100, 63, &tone(127, 1), 127), (12900, 129));
        assert_eq!(key_on_volume(63, 127, &tone(100, 64), 127), (6347, 12900));
        assert_eq!(key_on_volume(1, 65, &tone(127, 64), 127), (0, 8385));
        assert_eq!(key_on_volume(1, 0, &tone(127, 1), 100), (101, 0));
    }

    #[test]
    fn auto_vol_steps_match_the_sdk() {
        let tone = |vol, pan| Tone {
            mode: 0,
            vol,
            pan,
            center: 60,
            shift: 0,
            adsr1: 0,
            adsr2: 0,
            vag: 1,
            min: 0,
            max: 127,
            pb_min: 0,
            pb_max: 0,
        };
        let steps = |vl, vr, t: &Tone, pv| {
            [106, 85, 64, 43, 22, 1, 0].map(|level| auto_vol_volume(level, vl, vr, t, pv))
        };
        // [obs] SsUtAutoVol(v, 127, 0, 6) after SsUtKeyOnV: a centred full tone
        // (p6-probe-autovol), the battle replay's tone of volume 95 (VBlank 4848), a tone panned
        // hard left keyed 127/64, and one panned right keyed 30/127 (p9-sound-autovol-*).
        assert_eq!(
            steps(127, 127, &tone(127, 64), 127),
            [
                (0x32f1, 0x356a),
                (0x28da, 0x2ad5),
                (0x1ec3, 0x2040),
                (0x14aa, 0x15ab),
                (0xa92, 0xb16),
                (0x7a, 0x81),
                (0, 0)
            ]
        );
        assert_eq!(
            steps(127, 127, &tone(95, 64), 127),
            [
                (0x261b, 0x27f4),
                (0x1e8d, 0x200a),
                (0x1701, 0x181f),
                (0xf75, 0x1035),
                (0x7e6, 0x84a),
                (0x5a, 0x60),
                (0, 0)
            ]
        );
        assert_eq!(
            steps(127, 64, &tone(127, 1), 127),
            [
                (0x3494, 0x6a),
                (0x2a29, 0x55),
                (0x1fbf, 0x40),
                (0x1554, 0x2b),
                (0xae9, 0x16),
                (0x7e, 1),
                (0, 0)
            ]
        );
        assert_eq!(
            steps(30, 127, &tone(127, 100), 127),
            [
                (1330, 13674),
                (1066, 10965),
                (803, 8256),
                (539, 5547),
                (276, 2838),
                (12, 129),
                (0, 0)
            ]
        );
        assert_eq!(auto_vol_volume(106, 127, 127, &tone(1, 0), 64), (52, 0));
    }

    #[test]
    fn a_sweep_steps_with_the_latest_key_on_tone() {
        let (mut snd, mut spu) = fresh();
        let mut vh = make_vh(1, &[0x40, 0x80]);
        // Tone 1 at volume 60 (tone 0 stays at 127).
        vh[32 + VAB_PROGRAMS * PROG_ATTR_SIZE + TONE_ATTR_SIZE + 2] = 60;
        snd.open_head_sticky(&vh, 0, 0x1010);
        let key = |tone| KeyOn {
            vab: 0,
            prog: 0,
            tone,
            note: 60,
            fine: 0,
            vol_left: 0x7f,
            vol_right: 0x7f,
        };
        let vol = |spu: &SpuState| {
            (
                spu.read(reg::voice(10, reg::VOL_L)),
                spu.read(reg::voice(10, reg::VOL_R)),
            )
        };
        assert_eq!(snd.key_on_v(&spu, 10, key(0)), 10);
        snd.tick(&mut spu);
        assert_eq!(vol(&spu), (0x3fff, 0x3fff));
        snd.auto_vol(10, 0x7f, 0, 6);
        snd.tick(&mut spu);
        snd.tick(&mut spu);
        assert_eq!(
            vol(&spu),
            (0x28da, 0x2ad5),
            "level 85 of the voice's own tone"
        );
        // Another voice's key-on mid-sweep: the next steps take its tone [obs:
        // p9-sound-autovol-last2: 0xe87 / 0xf3c at level 64 with a tone of volume 60].
        assert_eq!(snd.key_on_v(&spu, 11, key(1)), 11);
        snd.tick(&mut spu);
        assert_eq!(vol(&spu), (0xe87, 0xf3c));
        // SsUtAllKeyOff silences the voice; a sweep brings its volume back.
        snd.all_key_off(&mut spu);
        assert_eq!(vol(&spu), (0, 0));
        snd.auto_vol(10, 0x7f, 0, 6);
        snd.tick(&mut spu);
        assert_eq!(vol(&spu), (0x1811, 0x193c));
    }

    #[test]
    fn key_on_waits_for_the_tick_and_writes_the_voice() {
        let (mut snd, mut spu) = fresh();
        let vh = make_vh(2, &[0x40, 0x80]);
        snd.open_head_sticky(&vh, 0, 0x1010);
        let key = KeyOn {
            vab: 0,
            prog: 1,
            tone: 1,
            note: 48,
            fine: 0,
            vol_left: 0x7f,
            vol_right: 0x7f,
        };
        assert_eq!(snd.key_on_v(&spu, 12, key), 12);
        assert_eq!(
            spu.read(reg::voice(12, reg::PITCH)),
            0x1000,
            "SsInit's value until the tick"
        );
        assert_eq!(
            snd.key_on_v(&spu, 12, key),
            -1,
            "already keyed for the next tick"
        );
        snd.tick(&mut spu);
        assert_eq!(spu.read(reg::voice(12, reg::PITCH)), 2048);
        assert_eq!(spu.read(reg::voice(12, reg::VOL_L)), 0x3fff);
        // Tone 1 plays sample 2, after sample 0 (empty) and sample 1's 0x40 bytes.
        assert_eq!(spu.read(reg::voice(12, reg::START)), (0x1010 + 0x40) / 8);
        assert_eq!(spu.read(reg::voice(12, reg::ADSR1)), 0x80ff);
        assert_eq!(spu.key_status(1 << 12), libspu::SPU_ON_ENV_OFF);
        assert_eq!(snd.key_on_v(&spu, 24, key), -1);
        assert_eq!(snd.key_on_v(&spu, -1, key), -1);
        assert_eq!(snd.key_on_v(&spu, 13, KeyOn { vab: 5, ..key }), -1);
        assert_eq!(snd.key_on_v(&spu, 13, KeyOn { tone: 16, ..key }), -1);
        assert_eq!(
            snd.key_on_v(&spu, 13, KeyOn { prog: 2, ..key }),
            -1,
            "no such program"
        );
        assert_eq!(
            snd.key_on_v(
                &spu,
                13,
                KeyOn {
                    vol_left: 128,
                    ..key
                }
            ),
            -1
        );
        assert_eq!(snd.key_off_v(12), 0);
        assert_eq!(snd.key_off_v(24), -1);
        snd.tick(&mut spu);
        assert_eq!(spu.key_status(1 << 12), libspu::SPU_OFF);
    }

    #[test]
    fn ticks_run_at_240_hz_on_the_virtual_clock() {
        let (mut snd, mut spu) = fresh();
        snd.start();
        let vblanks = 2 * 1722;
        let mut ticks_per_vblank = std::collections::BTreeMap::new();
        for vb in 0..vblanks {
            let before = snd.next_tick;
            snd.frame(&mut spu, vb);
            let n = (CYCLES_PER_VBLANK - before).div_ceil(CYCLES_PER_TICK);
            *ticks_per_vblank.entry(n).or_insert(0) += 1;
        }
        // 4 a VBlank but about one VBlank in 1,722 with 3.
        assert_eq!(ticks_per_vblank.get(&4).copied().unwrap_or(0) + 2, vblanks);
        assert_eq!(ticks_per_vblank.get(&3).copied(), Some(2));
    }

    #[test]
    fn init_and_quit() {
        let (mut snd, mut spu) = fresh();
        snd.set_table_size(2, 1);
        let vh = make_vh(1, &[0x40]);
        snd.open_head_sticky(&vh, 0, 0x1010);
        snd.trans_body(&mut spu, &[7; 0x40], 0);
        snd.quit();
        assert!(snd.vabs.iter().all(Option::is_none) && snd.seqs.is_empty());
        assert_eq!(spu.ram()[0x1010], 7, "quit leaves sound RAM alone");
        snd.init(&mut spu);
        assert_eq!(spu.read(reg::MAIN_VOL_L), 0x3fff);
        assert_eq!(spu.spucnt(), 0xc000);
    }

    #[test]
    fn c_interface_loads_a_vab_and_keys_a_voice() {
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
        assert_eq!(libspu::with_state(|s| s.ram()[0x1010]), 0x55);
        SsStart();
        SsSetMVol(127, 127);
        assert_eq!(SsUtKeyOnV(10, 0, 0, 0, 60, 0, 0x7f, 0x7f), 10);
        frame(0);
        assert_eq!(
            libspu::with_state(|s| s.read(reg::voice(10, reg::PITCH))),
            4096
        );
        assert_eq!(SsUtAutoVol(10, 0x7f, 0, 6), 0);
        assert_eq!(SsUtAutoVol(10, 0x7f, 0, -1), -1);
        assert_eq!(SsUtKeyOffV(10), 0);
        frame(1);
        assert_eq!(libspu::SpuGetKeyStatus(1 << 10) & 1, libspu::SPU_OFF);
        SsUtAllKeyOff(0);
        let seq = [0u32; 4];
        // SAFETY: four readable words (not a SEQ: it opens, and never plays).
        assert_eq!(unsafe { SsSeqOpen(seq.as_ptr(), 0) }, 0);
        SsSeqSetVol(0, 0x50, 0x50);
        SsSeqPlay(0, 1, 0);
        SsSeqStop(0);
        SsSeqClose(0);
        assert!(with_state(|s| s.started));
        SsEnd();
        SsQuit();
        assert!(with_state(|s| !s.initialized));
    }
}
