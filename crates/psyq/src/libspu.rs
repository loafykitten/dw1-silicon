//! libspu: the SPU's state as the game sees it (sound RAM, voice key status, reverb), silent.
//!
//! Nothing here makes sound yet (Phase 6). The state is what a real SPU would need: the
//! 512 KB sound RAM with VAB bodies copied into it by libsnd, each voice's key state and the
//! parameters it was keyed on with, and the reverb settings. Phase 6 puts the SPU model
//! (`backend::spu`) behind this by reading [`with_state`]; the C-facing functions stay as they
//! are.
//!
//! libsnd and libspu share one SPU. Lock order when both are needed: libsnd's state first,
//! then this one.

use std::sync::{LazyLock, Mutex, MutexGuard};

use backend::spu::RAM_SIZE;

/// Voices on the SPU.
pub const VOICES: usize = 24;

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

/// What a voice was keyed on with (`SsUtKeyOnV`), kept for the Phase 6 SPU model.
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

/// A volume sweep set by `SsUtAutoVol`, applied by the Phase 6 mixer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoVol {
    pub start: i16,
    pub end: i16,
    /// Sweep time in sequencer ticks.
    pub ticks: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Voice {
    /// Keyed on and not yet keyed off. A silent SPU has no envelope or sample end, so a keyed
    /// voice stays on until the game keys it off.
    pub key_on: bool,
    pub key: KeyOn,
    pub auto_vol: Option<AutoVol>,
}

#[derive(Clone, Debug)]
pub struct Reverb {
    pub enabled: bool,
    pub mode: i32,
    pub depth: SpuVolume,
    pub delay: i32,
    pub feedback: i32,
    /// Voices (low 24 bits) that feed the reverb.
    pub voices: u32,
}

pub struct SpuState {
    /// Sound RAM, `RAM_SIZE` bytes.
    pub ram: Vec<u8>,
    pub voices: [Voice; VOICES],
    pub reverb: Reverb,
}

impl SpuState {
    pub fn new() -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            voices: [Voice::default(); VOICES],
            reverb: Reverb {
                enabled: false,
                mode: 0,
                depth: SpuVolume::default(),
                delay: 0,
                feedback: 0,
                voices: 0,
            },
        }
    }

    /// Back to power-on: RAM cleared, every voice off, reverb off. [guess] that `SsInit`
    /// (which runs SpuInit) clears sound RAM; the game loads every VAB after it anyway.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Copies `data` to sound RAM at `addr`. False (nothing written) if it won't fit.
    pub fn write_ram(&mut self, addr: usize, data: &[u8]) -> bool {
        match addr.checked_add(data.len()) {
            Some(end) if end <= RAM_SIZE => {
                self.ram[addr..end].copy_from_slice(data);
                true
            }
            _ => false,
        }
    }

    /// Keys a voice on. False if `voice` is out of range.
    pub fn key_on(&mut self, voice: usize, key: KeyOn) -> bool {
        let Some(v) = self.voices.get_mut(voice) else {
            return false;
        };
        *v = Voice {
            key_on: true,
            key,
            auto_vol: None,
        };
        true
    }

    /// Keys a voice off. A silent SPU has no release phase, so it is off at once.
    pub fn key_off(&mut self, voice: usize) -> bool {
        let Some(v) = self.voices.get_mut(voice) else {
            return false;
        };
        v.key_on = false;
        true
    }

    pub fn all_key_off(&mut self) {
        for v in &mut self.voices {
            v.key_on = false;
        }
    }

    /// `SpuGetKeyStatus`: the status of the voice in `voice_bit`; with several bits set, the
    /// lowest-numbered voice [doc: Psy-Q reference]. -1 if there's no valid voice [doc]: no bit
    /// set, or the lowest above voice 23 [guess].
    pub fn key_status(&self, voice_bit: u32) -> i32 {
        if voice_bit == 0 || voice_bit.trailing_zeros() as usize >= VOICES {
            return SPU_ERROR;
        }
        if self.voices[voice_bit.trailing_zeros() as usize].key_on {
            SPU_ON
        } else {
            SPU_OFF
        }
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
        self.ram[start..start + len].fill(0);
        true
    }

    /// `SpuSetReverbModeParam`. [doc] Psy-Q reference: SPU_ERROR for an invalid mode (then
    /// nothing is set), else 0. The "work area allocated with SpuMalloc" error needs SpuMalloc,
    /// which the game doesn't call.
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
            self.reverb.mode = mode;
            if attr.mode & SPU_REV_MODE_CLEAR_WA != 0 {
                self.clear_reverb_area(mode);
            }
        }
        self.set_reverb_depth(&SpuReverbAttr { mask, ..*attr });
        if mask & SPU_REV_DELAYTIME != 0 {
            self.reverb.delay = attr.delay;
        }
        if mask & SPU_REV_FEEDBACK != 0 {
            self.reverb.feedback = attr.feedback;
        }
        SPU_SUCCESS
    }

    /// `SpuSetReverbDepth`: the left/right depth the mask selects.
    /// [doc] A zero mask selects both channels.
    pub fn set_reverb_depth(&mut self, attr: &SpuReverbAttr) {
        let mask = if attr.mask == 0 {
            REVERB_ALL
        } else {
            attr.mask
        };
        if mask & SPU_REV_DEPTHL != 0 {
            self.reverb.depth.left = attr.depth.left;
        }
        if mask & SPU_REV_DEPTHR != 0 {
            self.reverb.depth.right = attr.depth.right;
        }
    }

    /// `SpuSetReverbVoice`: returns the reverb voice bits (low 24) after the change.
    pub fn set_reverb_voice(&mut self, on_off: i32, voice_bit: u32) -> u32 {
        let bits = voice_bit & 0x00ff_ffff;
        match on_off {
            SPU_ON => self.reverb.voices |= bits,
            SPU_OFF => self.reverb.voices &= !bits,
            SPU_BIT => self.reverb.voices = bits,
            _ => {}
        }
        self.reverb.voices
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

static SPU: LazyLock<Mutex<SpuState>> = LazyLock::new(|| Mutex::new(SpuState::new()));

/// The SPU state, locked. For libsnd, tests, and the Phase 6 audio side.
pub fn state() -> MutexGuard<'static, SpuState> {
    SPU.lock().unwrap()
}

/// Runs `f` on the SPU state.
pub fn with_state<R>(f: impl FnOnce(&mut SpuState) -> R) -> R {
    f(&mut state())
}

/// `SpuGetKeyStatus(voice_bit)`: [`SPU_ON`] / [`SPU_OFF`] for the one voice named, -1 if the
/// voice is incorrect.
#[unsafe(no_mangle)]
pub extern "C" fn SpuGetKeyStatus(voice_bit: u32) -> i32 {
    state().key_status(voice_bit)
}

/// `SpuSetReverb(on_off)`: [doc] returns the reverb on/off value.
#[unsafe(no_mangle)]
pub extern "C" fn SpuSetReverb(on_off: i32) -> i32 {
    let on = on_off == SPU_ON;
    state().reverb.enabled = on;
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
#[unsafe(no_mangle)]
pub extern "C" fn SpuSetReverbVoice(on_off: i32, voice_bit: u32) -> u32 {
    state().set_reverb_voice(on_off, voice_bit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_status_follows_key_on_and_off() {
        let mut spu = SpuState::new();
        assert_eq!(spu.key_status(1 << 10), SPU_OFF);
        assert!(spu.key_on(10, KeyOn::default()));
        assert_eq!(spu.key_status(1 << 10), SPU_ON);
        assert_eq!(spu.key_status(1 << 11), SPU_OFF);
        // No valid voice.
        assert_eq!(spu.key_status(0), SPU_ERROR);
        // Several bits: the lowest voice (10 is on, 11 off).
        assert_eq!(spu.key_status(0b11 << 10), SPU_ON);
        assert_eq!(spu.key_status(1 << 24), SPU_ERROR);
        assert!(!spu.key_on(24, KeyOn::default()));
        assert!(spu.key_off(10));
        assert_eq!(spu.key_status(1 << 10), SPU_OFF);
        spu.key_on(0, KeyOn::default());
        spu.key_on(23, KeyOn::default());
        spu.all_key_off();
        assert!(spu.voices.iter().all(|v| !v.key_on));
    }

    #[test]
    fn ram_writes_are_bounds_checked() {
        let mut spu = SpuState::new();
        assert!(spu.write_ram(RAM_SIZE - 4, &[1, 2, 3, 4]));
        assert_eq!(&spu.ram[RAM_SIZE - 4..], &[1, 2, 3, 4]);
        assert!(!spu.write_ram(RAM_SIZE - 3, &[1, 2, 3, 4]));
        assert!(!spu.write_ram(usize::MAX, &[1]));
    }

    #[test]
    fn reverb_mode_param_as_the_game_sets_it() {
        let mut spu = SpuState::new();
        let (start, len) = SpuState::reverb_area(3).unwrap();
        assert_eq!((start, len), (RAM_SIZE - 0x4840, 0x4840));
        spu.ram[start] = 0xaa;
        spu.ram[start - 1] = 0xbb;
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
        assert_eq!(spu.reverb.depth, attr.depth);
        assert_eq!(spu.ram[start], 0, "work area cleared");
        assert_eq!(spu.ram[start - 1], 0xbb, "nothing below it touched");
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
