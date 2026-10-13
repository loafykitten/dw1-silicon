//! Model of the PS1 SPU (sound processing unit): 512 KB sound RAM, the register file at
//! 0x1F801C00..0x1F801FFF, 24 ADPCM voices with pitch counter, 4-point Gaussian interpolation,
//! ADSR envelope, voice and main volume, pitch modulation, noise, reverb, and the CD audio input.
//! Output is 44.1 kHz stereo, the SPU's own rate.
//!
//! Translated from DW2-PC (github.com/Wyrelade/DW2-PC, CC0), `backend/psxspu.c` and
//! `backend/psxspu.h`, at commit 2d491b5. The model logic is DW2-PC's; the literal tables are
//! not: [`GAUSS`], [`REVERB_FIR`] and the ADPCM filter coefficients ([`FILTER_POS`],
//! [`FILTER_NEG`]) are transcribed from psx-spx, "Sound Processing Unit (SPU)" and "CDROM XA
//! Audio ADPCM Compression" (psx-spx.consoledev.net/soundprocessingunitspu/ and /cdromformat/).
//!
//! Modelled as in psx-spx: SPU-ADPCM decode (5 filters; flag bits loop start / end / repeat),
//! pitch counter with PMON, the ADSR and volume sweep envelope steps, noise generator, KON /
//! KOFF / ENDX, manual and DMA sound RAM transfers, ATTR / STATX, reverb (22050 Hz, FIR
//! resampled). Everything runs per 44.1 kHz sample in [`Spu::render`]. The CD input takes
//! frames from the caller (the XA decoder's [`pop`](crate::xa::XaDecoder::pop)); the external
//! input is not mixed.
//!
//! Differences from the C: the state lives in [`Spu`] instead of file-scope statics, the CD input
//! is a closure, and the one-time "volume sweep with negative phase not modelled" log line is
//! dropped.

/// Sound RAM size in bytes.
pub const RAM_SIZE: usize = 0x80000;

const RAM_MASK: u32 = RAM_SIZE as u32 - 1;

/// SPU-ADPCM prediction filter coefficients (previous sample, the one before it). Source:
/// psx-spx, "SPU ADPCM Samples" says the filters are those of CD-XA; the coefficients are the
/// "Pos/neg Tables" of "CDROM XA Audio ADPCM Compression" (psx-spx.consoledev.net/cdromformat/).
pub const FILTER_POS: [i32; 5] = [0, 60, 115, 98, 122];
/// See [`FILTER_POS`].
pub const FILTER_NEG: [i32; 5] = [0, 0, -52, -55, -60];

/// The 512-entry Gaussian interpolation table. Source: psx-spx, "4-Point Gaussian Interpolation"
/// (psx-spx.consoledev.net/soundprocessingunitspu/).
pub const GAUSS: [i16; 512] = [
    -0x0001, -0x0001, -0x0001, -0x0001, -0x0001, -0x0001, -0x0001, -0x0001, -0x0001, -0x0001,
    -0x0001, -0x0001, -0x0001, -0x0001, -0x0001, -0x0001, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0001, 0x0001, 0x0001, 0x0001, 0x0002, 0x0002, 0x0002, 0x0003, 0x0003, 0x0003,
    0x0004, 0x0004, 0x0005, 0x0005, 0x0006, 0x0007, 0x0007, 0x0008, 0x0009, 0x0009, 0x000A, 0x000B,
    0x000C, 0x000D, 0x000E, 0x000F, 0x0010, 0x0011, 0x0012, 0x0013, 0x0015, 0x0016, 0x0018, 0x0019,
    0x001B, 0x001C, 0x001E, 0x0020, 0x0021, 0x0023, 0x0025, 0x0027, 0x0029, 0x002C, 0x002E, 0x0030,
    0x0033, 0x0035, 0x0038, 0x003A, 0x003D, 0x0040, 0x0043, 0x0046, 0x0049, 0x004D, 0x0050, 0x0054,
    0x0057, 0x005B, 0x005F, 0x0063, 0x0067, 0x006B, 0x006F, 0x0074, 0x0078, 0x007D, 0x0082, 0x0087,
    0x008C, 0x0091, 0x0096, 0x009C, 0x00A1, 0x00A7, 0x00AD, 0x00B3, 0x00BA, 0x00C0, 0x00C7, 0x00CD,
    0x00D4, 0x00DB, 0x00E3, 0x00EA, 0x00F2, 0x00FA, 0x0101, 0x010A, 0x0112, 0x011B, 0x0123, 0x012C,
    0x0135, 0x013F, 0x0148, 0x0152, 0x015C, 0x0166, 0x0171, 0x017B, 0x0186, 0x0191, 0x019C, 0x01A8,
    0x01B4, 0x01C0, 0x01CC, 0x01D9, 0x01E5, 0x01F2, 0x0200, 0x020D, 0x021B, 0x0229, 0x0237, 0x0246,
    0x0255, 0x0264, 0x0273, 0x0283, 0x0293, 0x02A3, 0x02B4, 0x02C4, 0x02D6, 0x02E7, 0x02F9, 0x030B,
    0x031D, 0x0330, 0x0343, 0x0356, 0x036A, 0x037E, 0x0392, 0x03A7, 0x03BC, 0x03D1, 0x03E7, 0x03FC,
    0x0413, 0x042A, 0x0441, 0x0458, 0x0470, 0x0488, 0x04A0, 0x04B9, 0x04D2, 0x04EC, 0x0506, 0x0520,
    0x053B, 0x0556, 0x0572, 0x058E, 0x05AA, 0x05C7, 0x05E4, 0x0601, 0x061F, 0x063E, 0x065C, 0x067C,
    0x069B, 0x06BB, 0x06DC, 0x06FD, 0x071E, 0x0740, 0x0762, 0x0784, 0x07A7, 0x07CB, 0x07EF, 0x0813,
    0x0838, 0x085D, 0x0883, 0x08A9, 0x08D0, 0x08F7, 0x091E, 0x0946, 0x096F, 0x0998, 0x09C1, 0x09EB,
    0x0A16, 0x0A40, 0x0A6C, 0x0A98, 0x0AC4, 0x0AF1, 0x0B1E, 0x0B4C, 0x0B7A, 0x0BA9, 0x0BD8, 0x0C07,
    0x0C38, 0x0C68, 0x0C99, 0x0CCB, 0x0CFD, 0x0D30, 0x0D63, 0x0D97, 0x0DCB, 0x0E00, 0x0E35, 0x0E6B,
    0x0EA1, 0x0ED7, 0x0F0F, 0x0F46, 0x0F7F, 0x0FB7, 0x0FF1, 0x102A, 0x1065, 0x109F, 0x10DB, 0x1116,
    0x1153, 0x118F, 0x11CD, 0x120B, 0x1249, 0x1288, 0x12C7, 0x1307, 0x1347, 0x1388, 0x13C9, 0x140B,
    0x144D, 0x1490, 0x14D4, 0x1517, 0x155C, 0x15A0, 0x15E6, 0x162C, 0x1672, 0x16B9, 0x1700, 0x1747,
    0x1790, 0x17D8, 0x1821, 0x186B, 0x18B5, 0x1900, 0x194B, 0x1996, 0x19E2, 0x1A2E, 0x1A7B, 0x1AC8,
    0x1B16, 0x1B64, 0x1BB3, 0x1C02, 0x1C51, 0x1CA1, 0x1CF1, 0x1D42, 0x1D93, 0x1DE5, 0x1E37, 0x1E89,
    0x1EDC, 0x1F2F, 0x1F82, 0x1FD6, 0x202A, 0x207F, 0x20D4, 0x2129, 0x217F, 0x21D5, 0x222C, 0x2282,
    0x22DA, 0x2331, 0x2389, 0x23E1, 0x2439, 0x2492, 0x24EB, 0x2545, 0x259E, 0x25F8, 0x2653, 0x26AD,
    0x2708, 0x2763, 0x27BE, 0x281A, 0x2876, 0x28D2, 0x292E, 0x298B, 0x29E7, 0x2A44, 0x2AA1, 0x2AFF,
    0x2B5C, 0x2BBA, 0x2C18, 0x2C76, 0x2CD4, 0x2D33, 0x2D91, 0x2DF0, 0x2E4F, 0x2EAE, 0x2F0D, 0x2F6C,
    0x2FCC, 0x302B, 0x308B, 0x30EA, 0x314A, 0x31AA, 0x3209, 0x3269, 0x32C9, 0x3329, 0x3389, 0x33E9,
    0x3449, 0x34A9, 0x3509, 0x3569, 0x35C9, 0x3629, 0x3689, 0x36E8, 0x3748, 0x37A8, 0x3807, 0x3867,
    0x38C6, 0x3926, 0x3985, 0x39E4, 0x3A43, 0x3AA2, 0x3B00, 0x3B5F, 0x3BBD, 0x3C1B, 0x3C79, 0x3CD7,
    0x3D35, 0x3D92, 0x3DEF, 0x3E4C, 0x3EA9, 0x3F05, 0x3F62, 0x3FBD, 0x4019, 0x4074, 0x40D0, 0x412A,
    0x4185, 0x41DF, 0x4239, 0x4292, 0x42EB, 0x4344, 0x439C, 0x43F4, 0x444C, 0x44A3, 0x44FA, 0x4550,
    0x45A6, 0x45FC, 0x4651, 0x46A6, 0x46FA, 0x474E, 0x47A1, 0x47F4, 0x4846, 0x4898, 0x48E9, 0x493A,
    0x498A, 0x49D9, 0x4A29, 0x4A77, 0x4AC5, 0x4B13, 0x4B5F, 0x4BAC, 0x4BF7, 0x4C42, 0x4C8D, 0x4CD7,
    0x4D20, 0x4D68, 0x4DB0, 0x4DF7, 0x4E3E, 0x4E84, 0x4EC9, 0x4F0E, 0x4F52, 0x4F95, 0x4FD7, 0x5019,
    0x505A, 0x509A, 0x50DA, 0x5118, 0x5156, 0x5194, 0x51D0, 0x520C, 0x5247, 0x5281, 0x52BA, 0x52F3,
    0x532A, 0x5361, 0x5397, 0x53CC, 0x5401, 0x5434, 0x5467, 0x5499, 0x54CA, 0x54FA, 0x5529, 0x5558,
    0x5585, 0x55B2, 0x55DE, 0x5609, 0x5632, 0x565B, 0x5684, 0x56AB, 0x56D1, 0x56F6, 0x571B, 0x573E,
    0x5761, 0x5782, 0x57A3, 0x57C3, 0x57E2, 0x57FF, 0x581C, 0x5838, 0x5853, 0x586D, 0x5886, 0x589E,
    0x58B5, 0x58CB, 0x58E0, 0x58F4, 0x5907, 0x5919, 0x592A, 0x593A, 0x5949, 0x5958, 0x5965, 0x5971,
    0x597C, 0x5986, 0x598F, 0x5997, 0x599E, 0x59A4, 0x59A9, 0x59AD, 0x59B0, 0x59B2, 0x59B3,
];

/// The 39-tap FIR that resamples the reverb input and output between 44.1 and 22.05 kHz.
/// Source: psx-spx, "Reverb Buffer Resampling" (psx-spx.consoledev.net/soundprocessingunitspu/).
pub const REVERB_FIR: [i16; 39] = [
    -0x0001, 0x0000, 0x0002, 0x0000, -0x000A, 0x0000, 0x0023, 0x0000, -0x0067, 0x0000, 0x010A,
    0x0000, -0x0268, 0x0000, 0x0534, 0x0000, -0x0B90, 0x0000, 0x2806, 0x4000, 0x2806, 0x0000,
    -0x0B90, 0x0000, 0x0534, 0x0000, -0x0268, 0x0000, 0x010A, 0x0000, -0x0067, 0x0000, 0x0023,
    0x0000, -0x000A, 0x0000, 0x0002, 0x0000, -0x0001,
];

/// Envelope (ADSR phase or volume sweep) step parameters, psx-spx "Envelope Operation".
#[derive(Clone, Copy, Default)]
struct EnvRate {
    exp: bool,
    decrease: bool,
    shift: i32,
    /// 0..=3
    step: i32,
}

#[derive(Clone, Copy, Default)]
struct EnvState {
    counter: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy)]
struct Voice {
    /// Bytes, block being played.
    cur_addr: u32,
    /// Bytes.
    repeat_addr: u32,
    /// Pitch counter: bit 12 up = sample in block, bits 4..=11 interpolation index.
    counter: u32,
    /// 3 samples of the previous block, then the decoded block.
    s: [i16; 31],
    /// ADPCM filter history.
    hist1: i16,
    hist2: i16,
    /// Flag byte of the current block.
    flags: u8,
    /// LSAX written by software after key on.
    ignore_loop: bool,
    phase: Phase,
    /// ADSR level (ENVX).
    level: i32,
    env: EnvState,
    /// Current voice volume (VOLX), left and right.
    vol_level: [i32; 2],
    vol_env: [EnvState; 2],
}

impl Voice {
    const fn new() -> Voice {
        Voice {
            cur_addr: 0,
            repeat_addr: 0,
            counter: 0,
            s: [0; 31],
            hist1: 0,
            hist2: 0,
            flags: 0,
            ignore_loop: false,
            phase: Phase::Off,
            level: 0,
            env: EnvState { counter: 0 },
            vol_level: [0; 2],
            vol_env: [EnvState { counter: 0 }; 2],
        }
    }
}

fn clamp16(x: i32) -> i32 {
    x.clamp(-0x8000, 0x7FFF)
}

/// One envelope step: returns the new level.
fn env_tick(e: &mut EnvState, r: &EnvRate, mut level: i32) -> i32 {
    let mut step = 7 - r.step;
    if r.decrease {
        step = !step;
    }
    step = ((step as u32) << (11 - r.shift).max(0)) as i32;
    let mut inc = 0x8000i32 >> (r.shift - 11).max(0);
    if r.exp && !r.decrease && level > 0x6000 {
        if r.shift < 10 {
            step >>= 2;
        } else if r.shift >= 11 {
            inc >>= 2;
        } else {
            step >>= 1;
            inc >>= 1;
        }
    } else if r.exp && r.decrease {
        step = step.wrapping_mul(level) >> 15;
    }
    if (r.step | (r.shift << 2)) != 0x7F && inc < 1 {
        inc = 1;
    }
    e.counter = e.counter.wrapping_add(inc);
    if e.counter & 0x8000 == 0 {
        return level;
    }
    e.counter = 0;
    level = level.wrapping_add(step);
    if !r.decrease {
        level = clamp16(level);
    } else if level < 0 {
        level = 0;
    }
    level
}

/// Volume register: direct (bit 15 = 0, value * 2) or sweep.
fn volume_step(reg: u16, e: &mut EnvState, level: i32) -> i32 {
    if reg & 0x8000 == 0 {
        return i32::from((u32::from(reg) << 1) as u16 as i16);
    }
    let r = EnvRate {
        exp: (reg >> 14) & 1 != 0,
        decrease: (reg >> 13) & 1 != 0,
        shift: i32::from((reg >> 2) & 0x1F),
        step: i32::from(reg & 3),
    };
    env_tick(e, &r, level)
}

/// `a * b / 0x8000`, saturated (reverb multiply).
fn mul(a: i32, b: i32) -> i32 {
    clamp16((a * b) >> 15)
}

/// The SPU.
pub struct Spu {
    ram: Box<[u8; RAM_SIZE]>,
    /// Register file as written (halfwords at 0x1F801C00..).
    regs: [u16; 0x200],
    voices: [Voice; 24],
    main_level: [i32; 2],
    main_env: [EnvState; 2],
    endx: u32,
    transfer_addr: u32,
    noise_level: i32,
    noise_timer: i32,
    /// Reverb input and output rings at 44.1 kHz for the FIR.
    rev_in: [[i16; 64]; 2],
    rev_out: [[i16; 64]; 2],
    /// Ring position (44.1 kHz samples).
    rev_pos: i32,
    /// Current buffer address, bytes.
    rev_addr: u32,
    /// 22 kHz phase.
    rev_phase: bool,
}

impl Default for Spu {
    fn default() -> Self {
        Self::new()
    }
}

impl Spu {
    pub fn new() -> Spu {
        Spu {
            ram: vec![0u8; RAM_SIZE]
                .into_boxed_slice()
                .try_into()
                .expect("RAM_SIZE bytes"),
            regs: [0; 0x200],
            voices: [Voice::new(); 24],
            main_level: [0; 2],
            main_env: [EnvState::default(); 2],
            endx: 0,
            transfer_addr: 0,
            noise_level: 1,
            noise_timer: 0,
            rev_in: [[0; 64]; 2],
            rev_out: [[0; 64]; 2],
            rev_pos: 0,
            rev_addr: 0,
            rev_phase: false,
        }
    }

    /// Resets registers, voices and the reverb state. Sound RAM is kept (as `PsxSpu_Reset` does).
    pub fn reset(&mut self) {
        self.regs = [0; 0x200];
        self.voices = [Voice::new(); 24];
        self.main_level = [0; 2];
        self.main_env = [EnvState::default(); 2];
        self.endx = 0;
        self.transfer_addr = 0;
        self.noise_level = 1;
        self.noise_timer = 0;
        self.rev_in = [[0; 64]; 2];
        self.rev_out = [[0; 64]; 2];
        self.rev_pos = 0;
        self.rev_phase = false;
        self.rev_addr = 0;
    }

    pub fn ram(&self) -> &[u8; RAM_SIZE] {
        &self.ram
    }

    pub fn ram_mut(&mut self) -> &mut [u8; RAM_SIZE] {
        &mut self.ram
    }

    // --- envelope -------------------------------------------------------------------------

    fn adsr_rate(&self, idx: usize) -> EnvRate {
        let v = &self.voices[idx];
        let a1 = self.regs[idx * 8 + 4];
        let a2 = self.regs[idx * 8 + 5];
        match v.phase {
            Phase::Attack => EnvRate {
                exp: a1 >> 15 != 0,
                decrease: false,
                shift: i32::from((a1 >> 10) & 0x1F),
                step: i32::from((a1 >> 8) & 3),
            },
            Phase::Decay => EnvRate {
                exp: true,
                decrease: true,
                shift: i32::from((a1 >> 4) & 0xF),
                step: 0,
            },
            Phase::Sustain => EnvRate {
                exp: a2 >> 15 != 0,
                decrease: (a2 >> 14) & 1 != 0,
                shift: i32::from((a2 >> 8) & 0x1F),
                step: i32::from((a2 >> 6) & 3),
            },
            _ => EnvRate {
                exp: (a2 >> 5) & 1 != 0,
                decrease: true,
                shift: i32::from(a2 & 0x1F),
                step: 0,
            },
        }
    }

    fn adsr_step(&mut self, idx: usize) {
        if self.voices[idx].phase == Phase::Off {
            return;
        }
        let r = self.adsr_rate(idx);
        let sustain_reg = self.regs[idx * 8 + 4];
        let v = &mut self.voices[idx];
        v.level = env_tick(&mut v.env, &r, v.level);
        match v.phase {
            Phase::Attack => {
                if v.level >= 0x7FFF {
                    v.phase = Phase::Decay;
                    v.env.counter = 0;
                }
            }
            Phase::Decay => {
                let sustain = (i32::from(sustain_reg & 0xF) + 1) * 0x800;
                if v.level <= sustain {
                    v.phase = Phase::Sustain;
                    v.env.counter = 0;
                }
            }
            Phase::Release if v.level <= 0 => {
                v.level = 0;
                v.phase = Phase::Off;
            }
            _ => {}
        }
    }

    // --- ADPCM ----------------------------------------------------------------------------

    fn decode_block(&mut self, i: usize) {
        let v = &mut self.voices[i];
        let base = (v.cur_addr & RAM_MASK & !15) as usize;
        let b = &self.ram[base..base + 16];
        let mut shift = u32::from(b[0] & 0xF);
        let filter = usize::from((b[0] >> 4) & 7).min(4);
        if shift > 12 {
            shift = 9;
        }
        v.flags = b[1];
        // Keep the last 3 samples for the interpolation.
        v.s[0] = v.s[28];
        v.s[1] = v.s[29];
        v.s[2] = v.s[30];
        for k in 0..28 {
            let nib = u32::from((b[2 + k / 2] >> ((k & 1) * 4)) & 0xF);
            let mut x = i32::from(((nib << 12) as u16 as i16) >> shift);
            x += (i32::from(v.hist1) * FILTER_POS[filter]
                + i32::from(v.hist2) * FILTER_NEG[filter]
                + 32)
                >> 6;
            let x = clamp16(x);
            v.hist2 = v.hist1;
            v.hist1 = x as i16;
            v.s[3 + k] = x as i16;
        }
        if (v.flags & 4) != 0 && !v.ignore_loop {
            v.repeat_addr = v.cur_addr & !15;
        }
    }

    fn key_on(&mut self, i: usize) {
        let ssa = u32::from(self.regs[i * 8 + 3]) << 3;
        let v = &mut self.voices[i];
        v.cur_addr = ssa;
        v.counter = 0;
        v.hist1 = 0;
        v.hist2 = 0;
        v.s = [0; 31];
        v.ignore_loop = false;
        v.phase = Phase::Attack;
        v.level = 0;
        v.env.counter = 0;
        self.endx &= !(1u32 << i);
        self.decode_block(i);
    }

    fn key_off(&mut self, i: usize) {
        let v = &mut self.voices[i];
        if v.phase != Phase::Off {
            v.phase = Phase::Release;
            v.env.counter = 0;
        }
    }

    /// End of a block: loop end flag, else the next block.
    fn next_block(&mut self, i: usize) {
        let v = &mut self.voices[i];
        if v.flags & 1 != 0 {
            self.endx |= 1u32 << i;
            v.cur_addr = v.repeat_addr;
            if v.flags & 2 == 0 {
                v.phase = Phase::Release;
                v.level = 0;
            }
        } else {
            v.cur_addr = (v.cur_addr + 16) & RAM_MASK;
        }
        self.decode_block(i);
    }

    // --- registers ------------------------------------------------------------------------

    /// Register write, `off` from 0x1F801C00 (0..0x3FF), 16-bit.
    pub fn write16(&mut self, off: u32, v: u16) {
        let off = off & 0x3FE;
        self.regs[(off >> 1) as usize] = v;
        if off < 0x180 {
            let i = (off >> 4) as usize;
            if off & 0xF == 0xE {
                self.voices[i].repeat_addr = u32::from(v) << 3;
                if self.voices[i].phase != Phase::Off {
                    self.voices[i].ignore_loop = true;
                }
            } else if off & 0xF == 0xC {
                self.voices[i].level = i32::from(v as i16);
            }
            return;
        }
        match off {
            0x188 | 0x18A => {
                let base = if off == 0x188 { 0 } else { 16 };
                for b in 0..16 {
                    if base + b < 24 && v & (1 << b) != 0 {
                        self.key_on(base + b);
                    }
                }
            }
            0x18C | 0x18E => {
                let base = if off == 0x18C { 0 } else { 16 };
                for b in 0..16 {
                    if base + b < 24 && v & (1 << b) != 0 {
                        self.key_off(base + b);
                    }
                }
            }
            // Reverb work area start: also the current buffer address.
            0x1A2 => self.rev_addr = u32::from(v) << 3,
            // Transfer address.
            0x1A6 => self.transfer_addr = u32::from(v) << 3,
            // Manual write FIFO.
            0x1A8 => {
                let a = self.transfer_addr;
                self.ram[(a & RAM_MASK) as usize] = v as u8;
                self.ram[(a.wrapping_add(1) & RAM_MASK) as usize] = (v >> 8) as u8;
                self.transfer_addr = a.wrapping_add(2) & RAM_MASK;
            }
            _ => {}
        }
    }

    /// Register read, `off` from 0x1F801C00 (0..0x3FF), 16-bit.
    pub fn read16(&self, off: u32) -> u16 {
        let off = off & 0x3FE;
        if off < 0x180 {
            let i = (off >> 4) as usize;
            if off & 0xF == 0xC {
                return self.voices[i].level as u16;
            }
            if off & 0xF == 0xE {
                return (self.voices[i].repeat_addr >> 3) as u16;
            }
            return self.regs[(off >> 1) as usize];
        }
        match off {
            0x19C => return self.endx as u16,
            0x19E => return (self.endx >> 16) as u16,
            // STATX: ATTR bits 0-5 applied at once, never busy.
            0x1AE => {
                let attr = self.regs[0x1AA >> 1];
                return (attr & 0x3F) | if attr & 0x20 != 0 { 0x80 } else { 0 };
            }
            0x1B8 => return self.main_level[0] as u16,
            0x1BA => return self.main_level[1] as u16,
            _ => {}
        }
        if (0x200..0x260).contains(&off) {
            let i = ((off - 0x200) >> 2) as usize;
            return self.voices[i].vol_level[((off >> 1) & 1) as usize] as u16;
        }
        self.regs[(off >> 1) as usize]
    }

    /// DMA channel 4: words to sound RAM at the current transfer address.
    pub fn dma_write(&mut self, src: &[u32]) {
        for &w in src {
            for k in 0..4 {
                self.ram[(self.transfer_addr & RAM_MASK) as usize] = (w >> (k * 8)) as u8;
                self.transfer_addr = (self.transfer_addr + 1) & RAM_MASK;
            }
        }
    }

    /// DMA channel 4: words from sound RAM at the current transfer address.
    pub fn dma_read(&mut self, dst: &mut [u32]) {
        for d in dst {
            let mut w = 0u32;
            for k in 0..4 {
                w |= u32::from(self.ram[(self.transfer_addr & RAM_MASK) as usize]) << (k * 8);
                self.transfer_addr = (self.transfer_addr + 1) & RAM_MASK;
            }
            *d = w;
        }
    }

    // --- mixing ---------------------------------------------------------------------------

    fn noise_tick(&mut self) {
        let attr = self.regs[0x1AA >> 1];
        let shift = i32::from((attr >> 10) & 0xF);
        let step = i32::from((attr >> 8) & 3) + 4;
        let nl = self.noise_level;
        let parity = ((nl >> 15) ^ (nl >> 12) ^ (nl >> 11) ^ (nl >> 10) ^ 1) & 1;
        self.noise_timer -= step;
        if self.noise_timer < 0 {
            self.noise_level = i32::from((nl * 2 + parity) as i16);
            self.noise_timer += 0x20000 >> shift;
            if self.noise_timer < 0 {
                self.noise_timer += 0x20000 >> shift;
            }
        }
    }

    // --- reverb ---------------------------------------------------------------------------
    // psx-spx "SPU Reverb Formula": runs at 22050 Hz on the work area ESA..0x7FFFE; input and
    // output are resampled with the 39-tap FIR.

    fn rreg(&self, r: usize) -> i32 {
        i32::from(self.regs[(0x1C0 + r * 2) >> 1] as i16)
    }

    fn raddr(&self, r: usize) -> u32 {
        u32::from(self.regs[(0x1C0 + r * 2) >> 1]) << 3
    }

    fn rev_wrap(&self, a: u32) -> u32 {
        let esa = u32::from(self.regs[0x1A2 >> 1]) << 3;
        let size = 0x80000 - esa;
        if size == 0 {
            return esa;
        }
        let a = self.rev_addr.wrapping_add(a).wrapping_sub(esa);
        esa + (a % size)
    }

    fn rev_read(&self, off: u32) -> i32 {
        let a = (self.rev_wrap(off) & 0x7FFFE) as usize;
        i32::from(i16::from_le_bytes([self.ram[a], self.ram[a + 1]]))
    }

    fn rev_write(&mut self, off: u32, v: i32) {
        if self.regs[0x1AA >> 1] & 0x80 == 0 {
            return;
        }
        let a = (self.rev_wrap(off) & 0x7FFFE) as usize;
        let b = (clamp16(v) as i16).to_le_bytes();
        self.ram[a] = b[0];
        self.ram[a + 1] = b[1];
    }

    fn fir(h: &[i16; 64], pos: i32) -> i32 {
        let mut acc = 0i32;
        for (i, &c) in REVERB_FIR.iter().enumerate() {
            acc += i32::from(c) * i32::from(h[(pos.wrapping_sub(i as i32) & 63) as usize]);
        }
        acc >> 15
    }

    /// One 44.1 kHz step: `inl`, `inr` = reverb send; returns the reverb output before EVOL.
    fn reverb_step(&mut self, inl: i32, inr: i32) -> (i32, i32) {
        let p = (self.rev_pos & 63) as usize;
        self.rev_in[0][p] = clamp16(inl) as i16;
        self.rev_in[1][p] = clamp16(inr) as i16;
        self.rev_out[0][p] = 0;
        self.rev_out[1][p] = 0;
        if self.rev_phase {
            let esa = u32::from(self.regs[0x1A2 >> 1]) << 3;
            let lin = mul(
                clamp16(Self::fir(&self.rev_in[0], self.rev_pos)),
                self.rreg(0x1E),
            );
            let rin = mul(
                clamp16(Self::fir(&self.rev_in[1], self.rev_pos)),
                self.rreg(0x1F),
            );
            let (viir, vwall, vapf1, vapf2) =
                (self.rreg(2), self.rreg(7), self.rreg(8), self.rreg(9));
            let (dapf1, dapf2) = (self.raddr(0), self.raddr(1));
            // Same side reflection, then different side reflection.
            for (m, d, input) in [
                (0xA, 0x10, lin),
                (0xB, 0x11, rin),
                (0x12, 0x19, lin),
                (0x13, 0x18, rin),
            ] {
                let m = self.raddr(m);
                let t = self.rev_read(m.wrapping_sub(2));
                let dv = self.rev_read(self.raddr(d));
                self.rev_write(m, mul(clamp16(input + mul(dv, vwall) - t), viir) + t);
            }
            // Early echo.
            let comb = |s: &Spu, base: usize| {
                clamp16(
                    mul(s.rreg(3), s.rev_read(s.raddr(base)))
                        + mul(s.rreg(4), s.rev_read(s.raddr(base + 2)))
                        + mul(s.rreg(5), s.rev_read(s.raddr(base + 8)))
                        + mul(s.rreg(6), s.rev_read(s.raddr(base + 10))),
                )
            };
            let mut lo = comb(self, 0xC);
            let mut ro = comb(self, 0xD);
            // Late reverb APF1, APF2.
            for (m, vapf, dapf, is_left) in [
                (0x1A, vapf1, dapf1, true),
                (0x1B, vapf1, dapf1, false),
                (0x1C, vapf2, dapf2, true),
                (0x1D, vapf2, dapf2, false),
            ] {
                let m = self.raddr(m);
                let t = self.rev_read(m.wrapping_sub(dapf));
                let x = if is_left { &mut lo } else { &mut ro };
                *x = clamp16(*x - mul(vapf, t));
                let w = *x;
                *x = clamp16(mul(*x, vapf) + t);
                self.rev_write(m, w);
            }
            self.rev_out[0][p] = lo as i16;
            self.rev_out[1][p] = ro as i16;
            self.rev_addr = if self.rev_addr + 2 > 0x7FFFE {
                esa
            } else {
                self.rev_addr + 2
            };
            if self.rev_addr < esa {
                self.rev_addr = esa;
            }
        }
        // Zero-stuffed 22 kHz output back to 44.1 kHz: FIR gain x2.
        let ol = clamp16(Self::fir(&self.rev_out[0], self.rev_pos).wrapping_mul(2));
        let or = clamp16(Self::fir(&self.rev_out[1], self.rev_pos).wrapping_mul(2));
        self.rev_phase = !self.rev_phase;
        self.rev_pos = self.rev_pos.wrapping_add(1);
        (ol, or)
    }

    /// Produces `out.len() / 2` stereo frames (L, R interleaved) at 44100 Hz. `cd` supplies the
    /// CD audio input, one frame per output frame.
    pub fn render(&mut self, out: &mut [i16], cd: &mut dyn FnMut() -> (i16, i16)) {
        for frame in out.as_chunks_mut::<2>().0 {
            let attr = self.regs[0x1AA >> 1];
            let pmon = u32::from(self.regs[0x190 >> 1]) | u32::from(self.regs[0x192 >> 1]) << 16;
            let non = u32::from(self.regs[0x194 >> 1]) | u32::from(self.regs[0x196 >> 1]) << 16;
            let eon = u32::from(self.regs[0x198 >> 1]) | u32::from(self.regs[0x19A >> 1]) << 16;
            let (mut l, mut r, mut sl, mut sr) = (0i32, 0i32, 0i32, 0i32);
            let mut prev_out = 0i32;

            let (cd_l, cd_r) = cd();
            self.noise_tick();
            for i in 0..24 {
                let idx = (self.voices[i].counter >> 12) as usize;
                let ip = ((self.voices[i].counter >> 4) & 0xFF) as usize;
                let vs = &self.voices[i].s;
                // Interpolation over s[idx ..= idx + 3] (oldest .. new).
                let mut sample = (i32::from(GAUSS[0xFF - ip]) * i32::from(vs[idx])) >> 15;
                sample += (i32::from(GAUSS[0x1FF - ip]) * i32::from(vs[idx + 1])) >> 15;
                sample += (i32::from(GAUSS[0x100 + ip]) * i32::from(vs[idx + 2])) >> 15;
                sample += (i32::from(GAUSS[ip]) * i32::from(vs[idx + 3])) >> 15;
                if non & (1 << i) != 0 {
                    sample = i32::from(self.noise_level as i16);
                }
                sample = sample.wrapping_mul(self.voices[i].level) >> 15;
                let (reg_l, reg_r) = (self.regs[i * 8], self.regs[i * 8 + 1]);
                let v = &mut self.voices[i];
                v.vol_level[0] = volume_step(reg_l, &mut v.vol_env[0], v.vol_level[0]);
                v.vol_level[1] = volume_step(reg_r, &mut v.vol_env[1], v.vol_level[1]);
                let vl = sample.wrapping_mul(v.vol_level[0]) >> 15;
                let vr = sample.wrapping_mul(v.vol_level[1]) >> 15;
                l += vl;
                r += vr;
                if eon & (1 << i) != 0 {
                    sl += vl;
                    sr += vr;
                }
                // Pitch counter.
                let mut step = i32::from(self.regs[i * 8 + 2]);
                if pmon & (1 << i) != 0 && i > 0 {
                    let factor = prev_out + 0x8000;
                    step = (i32::from(step as i16).wrapping_mul(factor)) >> 15;
                    step &= 0xFFFF;
                }
                if step > 0x3FFF {
                    step = 0x4000;
                }
                let v = &mut self.voices[i];
                v.counter = v.counter.wrapping_add(step as u32);
                while (self.voices[i].counter >> 12) >= 28 {
                    self.voices[i].counter -= 28 << 12;
                    self.next_block(i);
                }
                self.adsr_step(i);
                prev_out = sample;
            }
            // CD input (XA through the CD controller): ATTR bit 0 mixes it, bit 2 also into the
            // reverb, at the CD volume (0x1B0 / 0x1B2). The CD keeps flowing either way.
            if attr & 1 != 0 {
                let vl = (i32::from(cd_l) * i32::from(self.regs[0x1B0 >> 1] as i16)) >> 15;
                let vr = (i32::from(cd_r) * i32::from(self.regs[0x1B2 >> 1] as i16)) >> 15;
                l += vl;
                r += vr;
                if attr & 4 != 0 {
                    sl += vl;
                    sr += vr;
                }
            }
            let (m0, m1) = (self.regs[0x180 >> 1], self.regs[0x182 >> 1]);
            self.main_level[0] = volume_step(m0, &mut self.main_env[0], self.main_level[0]);
            self.main_level[1] = volume_step(m1, &mut self.main_env[1], self.main_level[1]);
            let (rl, rr) = self.reverb_step(sl, sr);
            l = (clamp16(l) * self.main_level[0]) >> 15;
            r = (clamp16(r) * self.main_level[1]) >> 15;
            l += (rl * i32::from(self.regs[0x184 >> 1] as i16)) >> 15;
            r += (rr * i32::from(self.regs[0x186 >> 1] as i16)) >> 15;
            if attr & 0x8000 == 0 || attr & 0x4000 == 0 {
                l = 0;
                r = 0;
            }
            frame[0] = clamp16(l) as i16;
            frame[1] = clamp16(r) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauss_table_matches_psx_spx() {
        assert_eq!(GAUSS.len(), 512);
        // Spot values from the page: first, last of each 0x80 block, and the middle.
        assert_eq!(GAUSS[0], -1);
        assert_eq!(GAUSS[0x0F], -1);
        assert_eq!(GAUSS[0x10], 0);
        assert_eq!(GAUSS[0x17], 1);
        assert_eq!(GAUSS[0x7F], 0x019C);
        assert_eq!(GAUSS[0x80], 0x01A8);
        assert_eq!(GAUSS[0xFF], 0x12C7);
        assert_eq!(GAUSS[0x100], 0x1307);
        assert_eq!(GAUSS[0x17F], 0x3DEF);
        assert_eq!(GAUSS[0x180], 0x3E4C);
        assert_eq!(GAUSS[0x1FF], 0x59B3);
    }

    #[test]
    fn gauss_quadruples_sum_near_one() {
        // psx-spx: gauss[000h+i] + gauss[0FFh-i] + gauss[100h+i] + gauss[1FFh-i] is 7F7Fh..7F81h.
        for i in 0..256 {
            let s = i32::from(GAUSS[i])
                + i32::from(GAUSS[0xFF - i])
                + i32::from(GAUSS[0x100 + i])
                + i32::from(GAUSS[0x1FF - i]);
            assert!((0x7F7F..=0x7F81).contains(&s), "i = {i:#x}: {s:#x}");
        }
    }

    #[test]
    fn reverb_fir_matches_psx_spx() {
        assert_eq!(REVERB_FIR.len(), 39);
        assert_eq!(REVERB_FIR[19], 0x4000);
        assert_eq!(REVERB_FIR[18], 0x2806);
        assert_eq!(REVERB_FIR[16], -0x0B90);
        // Symmetric around the centre tap, zero at every odd tap except the centre.
        for i in 0..39 {
            assert_eq!(REVERB_FIR[i], REVERB_FIR[38 - i]);
            if i % 2 == 1 && i != 19 {
                assert_eq!(REVERB_FIR[i], 0);
            }
        }
    }

    #[test]
    fn adpcm_filters_match_psx_spx() {
        assert_eq!(FILTER_POS, [0, 60, 115, 98, 122]);
        assert_eq!(FILTER_NEG, [0, 0, -52, -55, -60]);
    }

    #[test]
    fn silent_spu_renders_silence() {
        let mut spu = Spu::new();
        let mut out = [1i16; 64];
        spu.render(&mut out, &mut || (0, 0));
        assert!(out.iter().all(|&s| s == 0));
    }
}
