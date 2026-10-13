//! Test-only support for the decoder differential tests: a xorshift generator, and (when
//! `ref/DW2-PC` is present, `cfg(have_dw2_c)`) safe wrappers around DW2-PC's C decoders.
//!
//! The C keeps its state in file-scope statics, and `psxspu.c` pulls frames from `xadec.c`, so
//! every test that calls it holds [`guard`] for its whole body.

/// xorshift64* (no crates).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    pub fn u16(&mut self) -> u16 {
        (self.next_u64() >> 48) as u16
    }

    pub fn u8(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }

    /// Uniform in `0..n` (n > 0).
    pub fn below(&mut self, n: u32) -> u32 {
        ((u64::from(self.u32()) * u64::from(n)) >> 32) as u32
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.below(hi - lo + 1)
    }

    /// True once in `n`.
    pub fn one_in(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }

    pub fn fill(&mut self, buf: &mut [u8]) {
        for b in buf {
            *b = self.u8();
        }
    }
}

/// Case count: `default`, scaled by `DECODER_DIFF_SCALE` (percent) when set.
pub fn cases(default: usize) -> usize {
    match std::env::var("DECODER_DIFF_SCALE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
    {
        Some(p) => (default * p / 100).max(1),
        None => default,
    }
}

#[cfg(have_dw2_c)]
pub use c::*;

#[cfg(have_dw2_c)]
mod c {
    use std::sync::{Mutex, MutexGuard};

    static LOCK: Mutex<()> = Mutex::new(());

    /// Serialises access to the C decoders' global state.
    pub fn guard() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    unsafe extern "C" {
        fn XaDec_Reset();
        fn XaDec_Sector(body: *const u8);
        fn XaDec_SetSpeed(n: i32);
        fn XaDec_Pop(l: *mut i16, r: *mut i16);
        fn XaDec_Queued() -> i32;
        fn XaDec_Underruns() -> i32;
        fn XaDec_Stop();
        fn XaDec_LastRaw(frames: *mut i32) -> *const i16;
        fn Mdec_Vlc(bs: *const u8, out: *mut u32, out_words: i32) -> i32;
        fn Mdec_Start(input: *const u32, rgb24: i32, stp: i32);
        fn Mdec_Out(out: *mut u32, words: i32);
        fn PsxSpu_Reset();
        fn PsxSpu_Write16(off: u32, v: u16);
        fn PsxSpu_Read16(off: u32) -> u16;
        fn PsxSpu_DmaWrite(src: *const u32, words: u32);
        fn PsxSpu_DmaRead(dst: *mut u32, words: u32);
        fn PsxSpu_Render(out: *mut i16, n: i32);
        static mut PsxSpu_Ram: [u8; 0x80000];
    }

    // Each wrapper passes pointers to live, correctly sized buffers.

    pub fn xa_reset() {
        unsafe { XaDec_Reset() }
    }

    /// `body` must hold 2336 bytes.
    pub fn xa_sector(body: &[u8]) {
        assert!(body.len() >= 2336);
        unsafe { XaDec_Sector(body.as_ptr()) }
    }

    pub fn xa_set_speed(n: i32) {
        unsafe { XaDec_SetSpeed(n) }
    }

    pub fn xa_pop() -> (i16, i16) {
        let (mut l, mut r) = (0, 0);
        unsafe { XaDec_Pop(&mut l, &mut r) };
        (l, r)
    }

    pub fn xa_queued() -> i32 {
        unsafe { XaDec_Queued() }
    }

    pub fn xa_underruns() -> i32 {
        unsafe { XaDec_Underruns() }
    }

    pub fn xa_stop() {
        unsafe { XaDec_Stop() }
    }

    pub fn xa_last_raw() -> Vec<[i16; 2]> {
        let mut n = 0;
        let p = unsafe { XaDec_LastRaw(&mut n) };
        (0..n as usize)
            .map(|i| unsafe { [*p.add(i * 2), *p.add(i * 2 + 1)] })
            .collect()
    }

    /// Returns the C's return value and the `out_words` words of `out` it left behind. The
    /// bitstream is copied with zero padding (the C reads whole words past a short stream).
    pub fn mdec_vlc(bs: &[u8], out_words: usize) -> (i32, Vec<u32>) {
        let mut padded = bs.to_vec();
        padded.resize(bs.len() + 256, 0);
        let mut out = vec![0xDEAD_BEEFu32; out_words.max(1)];
        let r = unsafe { Mdec_Vlc(padded.as_ptr(), out.as_mut_ptr(), out_words as i32) };
        (r, out)
    }

    /// `Mdec_Start` then `Mdec_Out` of `words` words. The C trusts the header's word count and
    /// reads `in[0]` unconditionally, so it gets an owned copy whose count is clipped to the
    /// payload actually present (an empty stream becomes a zero-length header), which is what
    /// the Rust `Mdec::start` does with the same input.
    pub fn mdec_decode(stream: &[u32], rgb24: bool, stp: bool, words: usize) -> Vec<u32> {
        let input: Vec<u32> = match stream.split_first() {
            None => vec![0],
            Some((&hdr, rest)) => {
                let n = ((hdr & 0xFFFF) as usize).min(rest.len());
                std::iter::once((hdr & !0xFFFF) | n as u32)
                    .chain(rest[..n].iter().copied())
                    .collect()
            }
        };
        let mut out = vec![0xDEAD_BEEFu32; words];
        unsafe {
            Mdec_Start(input.as_ptr(), i32::from(rgb24), i32::from(stp));
            Mdec_Out(out.as_mut_ptr(), words as i32);
        }
        out
    }

    pub fn spu_reset() {
        unsafe { PsxSpu_Reset() }
    }

    pub fn spu_write16(off: u32, v: u16) {
        unsafe { PsxSpu_Write16(off, v) }
    }

    pub fn spu_read16(off: u32) -> u16 {
        unsafe { PsxSpu_Read16(off) }
    }

    pub fn spu_dma_write(src: &[u32]) {
        unsafe { PsxSpu_DmaWrite(src.as_ptr(), src.len() as u32) }
    }

    pub fn spu_dma_read(dst: &mut [u32]) {
        unsafe { PsxSpu_DmaRead(dst.as_mut_ptr(), dst.len() as u32) }
    }

    pub fn spu_render(out: &mut [i16]) {
        unsafe { PsxSpu_Render(out.as_mut_ptr(), (out.len() / 2) as i32) }
    }

    pub fn spu_ram() -> Vec<u8> {
        unsafe {
            std::slice::from_raw_parts((&raw const PsxSpu_Ram).cast::<u8>(), 0x80000).to_vec()
        }
    }

    pub fn spu_ram_set(data: &[u8]) {
        assert!(data.len() <= 0x80000);
        unsafe {
            (&raw mut PsxSpu_Ram)
                .cast::<u8>()
                .copy_from_nonoverlapping(data.as_ptr(), data.len())
        }
    }
}
