//! Test-only: DW2-PC's `backend/psxgpu.c` (CC0, github.com/Wyrelade/DW2-PC at commit 2d491b5)
//! compiled by `build.rs` and wrapped for the differential tests in `tests/`. Present only
//! when `ref/DW2-PC` is checked out (`cfg(have_dw2_c)`).

/// True when the C reference was compiled in.
pub const HAVE_C: bool = cfg!(have_dw2_c);

#[cfg(have_dw2_c)]
pub use c::CGpu;

#[cfg(have_dw2_c)]
mod c {
    use std::sync::{Mutex, MutexGuard};

    const W: usize = 1024;
    const H: usize = 512;

    unsafe extern "C" {
        static mut PsxGpu_Vram: [[u16; W]; H];
        fn PsxGpu_Reset(display: i32);
        fn PsxGpu_Gp0(words: *const u32, count: i32);
        fn PsxGpu_Fill(x: i32, y: i32, w: i32, h: i32, rgb24: u32);
        fn PsxGpu_LoadImage(x: i32, y: i32, w: i32, h: i32, src: *const u16);
        fn PsxGpu_StoreImage(x: i32, y: i32, w: i32, h: i32, dst: *mut u16);
        fn PsxGpu_MoveImage(sx: i32, sy: i32, dx: i32, dy: i32, w: i32, h: i32);
        fn PsxGpu_SetDisplay(x: i32, y: i32, w: i32, h: i32, rgb24: i32);
        fn PsxGpu_SetDisplayEnable(on: i32);
        fn PsxGpu_ReadDisplay(out: *mut u32, w: *mut i32, h: *mut i32) -> i32;
    }

    /// The C does plain signed `int` arithmetic on transfer and display arguments (`x + i`,
    /// `w * h`), so anything outside a range where that can't overflow is a test bug, not an
    /// input to feed it. Coordinates the PS1 can express are far inside this.
    fn bounded(args: &[i32]) {
        for &a in args {
            assert!(
                (-0x8000..=0x8000).contains(&a),
                "argument {a} out of the C's safe range"
            );
        }
    }

    // The C GPU is one set of globals: only one test may use it at a time.
    static LOCK: Mutex<()> = Mutex::new(());

    /// Exclusive handle on the C GPU (its state is global). Dropping it releases the lock.
    pub struct CGpu {
        _guard: MutexGuard<'static, ()>,
    }

    impl CGpu {
        /// Takes the lock, clears the VRAM and resets the drawing state, display off.
        pub fn new() -> Self {
            let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let g = CGpu { _guard: guard };
            // SAFETY: the lock gives exclusive access to the C globals.
            unsafe {
                std::ptr::write_bytes(&raw mut PsxGpu_Vram as *mut u16, 0, W * H);
                PsxGpu_Reset(1);
            }
            g
        }

        pub fn reset(&mut self, display: bool) {
            // SAFETY: lock held.
            unsafe { PsxGpu_Reset(display as i32) }
        }

        pub fn gp0(&mut self, words: &[u32]) {
            // SAFETY: pointer and count describe `words`.
            unsafe { PsxGpu_Gp0(words.as_ptr(), words.len() as i32) }
        }

        pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, rgb24: u32) {
            bounded(&[x, y, w, h]);
            // SAFETY: lock held; arguments bounded so the C's int arithmetic can't overflow.
            unsafe { PsxGpu_Fill(x, y, w, h, rgb24) }
        }

        /// `src` must hold at least `w * h` pixels (the C reads exactly that many).
        pub fn load_image(&mut self, x: i32, y: i32, w: i32, h: i32, src: &[u16]) {
            bounded(&[x, y, w, h]);
            assert!(src.len() as i64 >= w.max(0) as i64 * h.max(0) as i64);
            // SAFETY: `src` is long enough, checked above.
            unsafe { PsxGpu_LoadImage(x, y, w, h, src.as_ptr()) }
        }

        /// `dst` must hold at least `w * h` pixels.
        pub fn store_image(&self, x: i32, y: i32, w: i32, h: i32, dst: &mut [u16]) {
            bounded(&[x, y, w, h]);
            assert!(dst.len() as i64 >= w.max(0) as i64 * h.max(0) as i64);
            // SAFETY: `dst` is long enough, checked above.
            unsafe { PsxGpu_StoreImage(x, y, w, h, dst.as_mut_ptr()) }
        }

        pub fn move_image(&mut self, sx: i32, sy: i32, dx: i32, dy: i32, w: i32, h: i32) {
            bounded(&[sx, sy, dx, dy, w, h]);
            // SAFETY: lock held; arguments bounded.
            unsafe { PsxGpu_MoveImage(sx, sy, dx, dy, w, h) }
        }

        pub fn set_display(&mut self, x: i32, y: i32, w: i32, h: i32, rgb24: bool) {
            bounded(&[x, y, w, h]);
            // SAFETY: lock held; arguments bounded.
            unsafe { PsxGpu_SetDisplay(x, y, w, h, rgb24 as i32) }
        }

        pub fn set_display_enable(&mut self, on: bool) {
            // SAFETY: lock held.
            unsafe { PsxGpu_SetDisplayEnable(on as i32) }
        }

        /// `out` must hold 640 x 480 words (the C may write that many).
        pub fn read_display(&self, out: &mut [u32]) -> Option<(usize, usize)> {
            assert!(out.len() >= 640 * 480);
            let (mut w, mut h) = (0, 0);
            // SAFETY: `out` is large enough, checked above.
            let on = unsafe { PsxGpu_ReadDisplay(out.as_mut_ptr(), &mut w, &mut h) };
            (on != 0).then_some((w as usize, h as usize))
        }

        /// The whole VRAM, row-major.
        pub fn vram(&self) -> &[u16] {
            // SAFETY: lock held; the static is W * H contiguous u16.
            unsafe { std::slice::from_raw_parts(&raw const PsxGpu_Vram as *const u16, W * H) }
        }

        pub fn vram_mut(&mut self) -> &mut [u16] {
            // SAFETY: lock held (and `&mut self`).
            unsafe { std::slice::from_raw_parts_mut(&raw mut PsxGpu_Vram as *mut u16, W * H) }
        }
    }

    impl Default for CGpu {
        fn default() -> Self {
            Self::new()
        }
    }
}
