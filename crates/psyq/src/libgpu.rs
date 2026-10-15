//! libgpu: VRAM transfers, drawing and display environments, primitive set-up, ordering-table
//! links.
//!
//! Clean-room: GPU commands from psx-spx ("GPU", GP0/GP1); API meaning from the Psy-Q headers
//! and library reference; details from emulator observation of DW1's own calls
//! (`oracle/capture-calls.lua`, captures `p2-init`, `p2-landing`, `p2-menu`, 2026-10-13,
//! gitignored). Each rule says which: `[doc]`, `[hdr]` (the header's macros and structs),
//! `[obs]`.

use std::ffi::c_void;
use std::sync::Mutex;

use crate::gpu;
use crate::memory::link_of;

/// libgpu `RECT`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
}

/// libgpu `DR_ENV`: the packet `PutDrawEnv` builds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DrEnv {
    pub tag: u32,
    pub code: [u32; 15],
}

/// libgpu `DRAWENV`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawEnv {
    pub clip: Rect,
    pub ofs: [i16; 2],
    pub tw: Rect,
    pub tpage: u16,
    pub dtd: u8,
    pub dfe: u8,
    pub isbg: u8,
    pub r0: u8,
    pub g0: u8,
    pub b0: u8,
    pub dr_env: DrEnv,
}

/// libgpu `DISPENV`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DispEnv {
    pub disp: Rect,
    pub screen: Rect,
    pub isinter: u8,
    pub isrgb24: u8,
    pub pad0: u8,
    pub pad1: u8,
}

/// libgpu `DR_OFFSET`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DrOffset {
    pub tag: u32,
    pub code: [u32; 2],
}

/// `SetGraphDebug`'s level.
static DEBUG_LEVEL: Mutex<i32> = Mutex::new(0);

// ---- reset and synchronisation ----

/// `ResetGraph(mode)`: 0 resets the GPU completely (drawing state, and the display goes off),
/// 3 resets the drawing state but keeps the display, 1 cancels drawing in progress, which
/// never exists here [doc: library reference; GP1(00h)/(01h) psx-spx]. Returns 0 [obs].
#[unsafe(no_mangle)]
pub extern "C" fn ResetGraph(mode: i32) -> i32 {
    match mode {
        0 => gpu::with(|g| g.reset(true)),
        3 => gpu::with(|g| g.reset(false)),
        _ => {}
    }
    0
}

/// `SetGraphDebug(level)`: only kept; returns the previous level [doc; obs: 0 after boot].
#[unsafe(no_mangle)]
pub extern "C" fn SetGraphDebug(level: i32) -> i32 {
    std::mem::replace(&mut *DEBUG_LEVEL.lock().unwrap(), level)
}

/// `DrawSync(mode)`: drawing is synchronous here, so nothing is ever left: 0 [doc: mode 0
/// waits, mode 1 returns the number of queued items].
#[unsafe(no_mangle)]
pub extern "C" fn DrawSync(_mode: i32) -> i32 {
    0
}

// ---- VRAM transfers ----

/// # Safety
/// `rect` must be valid.
unsafe fn rect(rect: *const Rect) -> Rect {
    unsafe { rect.read() }
}

/// `ClearImage(rect, r, g, b)`: fills a VRAM rectangle [doc; GP0(02h)]. Returns 0 [obs].
///
/// # Safety
/// `r` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ClearImage(r: *const Rect, red: u8, green: u8, blue: u8) -> i32 {
    let r = unsafe { rect(r) };
    let rgb = u32::from(red) | u32::from(green) << 8 | u32::from(blue) << 16;
    gpu::with(|g| g.fill(r.x.into(), r.y.into(), r.w.into(), r.h.into(), rgb));
    0
}

/// `LoadImage(rect, p)`: copies `w * h` halfwords from `p` into VRAM [doc; GP0(A0h)].
/// Returns 0 [obs: 0 or 1, its place in the SDK's queue; DW1 ignores it].
///
/// # Safety
/// `r` must be valid and `p` must hold `w * h` halfwords.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LoadImage(r: *const Rect, p: *const u32) -> i32 {
    let r = unsafe { rect(r) };
    let n = (r.w.max(0) as usize) * (r.h.max(0) as usize);
    let src = unsafe { std::slice::from_raw_parts(p.cast::<u16>(), n) };
    gpu::with(|g| g.load_image(r.x.into(), r.y.into(), r.w.into(), r.h.into(), src));
    0
}

/// `StoreImage(rect, p)`: copies a VRAM rectangle to `p` [doc; GP0(C0h)]. Returns 0.
///
/// # Safety
/// `r` must be valid and `p` must have room for `w * h` halfwords.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn StoreImage(r: *const Rect, p: *mut u32) -> i32 {
    let r = unsafe { rect(r) };
    let n = (r.w.max(0) as usize) * (r.h.max(0) as usize);
    let dst = unsafe { std::slice::from_raw_parts_mut(p.cast::<u16>(), n) };
    gpu::with(|g| g.store_image(r.x.into(), r.y.into(), r.w.into(), r.h.into(), dst));
    0
}

/// `MoveImage(rect, x, y)`: copies a VRAM rectangle to `(x, y)` [doc; GP0(80h)]. Returns 0.
///
/// # Safety
/// `r` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MoveImage(r: *const Rect, x: i32, y: i32) -> i32 {
    let r = unsafe { rect(r) };
    gpu::with(|g| g.move_image(r.x.into(), r.y.into(), x, y, r.w.into(), r.h.into()));
    0
}

/// `LoadClut(clut, x, y)`: copies a 256-entry CLUT into VRAM row `y` from column `x` and returns
/// its CLUT id, `GetClut(x, y)` [doc; obs: 256 halfwords of row 0x1E1..0x1E3 matched the source
/// and nothing past them changed].
///
/// # Safety
/// `clut` must hold 256 halfwords.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LoadClut(clut: *const u32, x: i32, y: i32) -> u16 {
    let r = Rect {
        x: x as i16,
        y: y as i16,
        w: 256,
        h: 1,
    };
    unsafe { LoadImage(&r, clut) };
    GetClut(x, y)
}

/// libgpu `TIM_IMAGE` [hdr: libgpu.h; pointers are native].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TimImage {
    pub mode: u32,
    pub crect: *mut Rect,
    pub caddr: *mut u32,
    pub prect: *mut Rect,
    pub paddr: *mut u32,
}

/// Where `ReadTIM` reads next: set by `OpenTIM`, moved past the image by `ReadTIM` (an address;
/// raw pointers aren't `Send`) [obs: libgpu's static, after `ReadTIM` it holds the next TIM].
static TIM_CURSOR: Mutex<usize> = Mutex::new(0);

/// The address `ReadTIM` would read next.
pub fn tim_cursor() -> usize {
    *TIM_CURSOR.lock().unwrap()
}

/// `OpenTIM(addr)`: starts reading the TIM file at `addr` (its ID word) and returns 0 [doc;
/// obs]. It replaces the cursor unconditionally and checks nothing: on a word that isn't the
/// TIM ID it still returns 0, and `ReadTIM` is what refuses [obs: oracle/call-sdk.lua, a 0x11
/// and a 0 word].
///
/// # Safety
/// `addr` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn OpenTIM(addr: *const u32) -> i32 {
    *TIM_CURSOR.lock().unwrap() = addr as usize;
    0
}

/// `ReadTIM(img)`: describes the TIM opened by `OpenTIM` in `img` and moves on to the next one;
/// returns `img` [doc: psx-spx TIM; obs]. `mode` is the TIM's flags word, `crect` and `caddr`
/// point at the CLUT block's rectangle and data, `prect` and `paddr` at the pixel block's, all
/// inside the TIM. A TIM without a CLUT (flag bit 3 clear) gets null `crect` and `caddr`
/// [obs]. If the word at the cursor isn't the TIM ID 0x10 (a bad `OpenTIM` address, or the
/// end of a run of TIMs) it returns null and leaves `img` and the cursor alone [obs].
///
/// # Safety
/// `img` must be valid and the cursor must point at readable memory (a TIM, or a word that
/// is not 0x10).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ReadTIM(img: *mut TimImage) -> *mut TimImage {
    let mut cursor = TIM_CURSOR.lock().unwrap();
    unsafe {
        let tim = *cursor as *const u32;
        if tim.read() != 0x10 {
            return std::ptr::null_mut();
        }
        let flags = tim.add(1).read();
        // Each block is [length in bytes incl. this word][rect (x, y, w, h)][data].
        let (crect, caddr, pixels) = if flags & 8 != 0 {
            let clut = tim.add(2);
            let pixels = clut.cast::<u8>().add(clut.read() as usize).cast::<u32>();
            (
                clut.add(1).cast_mut().cast(),
                clut.add(3).cast_mut(),
                pixels,
            )
        } else {
            (std::ptr::null_mut(), std::ptr::null_mut(), tim.add(2))
        };
        *img = TimImage {
            mode: flags,
            crect,
            caddr,
            prect: pixels.add(1).cast_mut().cast(),
            paddr: pixels.add(3).cast_mut(),
        };
        *cursor = pixels.cast::<u8>().add(pixels.read() as usize) as usize;
    }
    img
}

// ---- environments ----

/// The drawing-environment commands for `env`, in the order libgpu sends them [obs: the
/// `dr_env` packet is E3 E4 E5 E1 E2 E6; GP0(E1h..E6h) psx-spx].
pub fn draw_env_words(env: &DrawEnv) -> Vec<u32> {
    let c = env.clip;
    let x0 = i32::from(c.x);
    let y0 = i32::from(c.y);
    let x1 = x0 + i32::from(c.w) - 1;
    let y1 = y0 + i32::from(c.h) - 1;
    let area = |x: i32, y: i32| ((y as u32 & 0x3FF) << 10) | (x as u32 & 0x3FF);
    let offset = ((env.ofs[1] as u32 & 0x7FF) << 11) | (env.ofs[0] as u32 & 0x7FF);
    // [hdr: _get_mode]
    let mode = 0xE100_0000
        | if env.dtd != 0 { 0x200 } else { 0 }
        | if env.dfe != 0 { 0x400 } else { 0 }
        | (u32::from(env.tpage) & 0x9FF);
    let tw = env.tw;
    if tw != Rect::default() {
        // DW1 never sets a texture window (checked when this traps).
        crate::trap::unimplemented("PutDrawEnv with a texture window");
    }
    let mut words = vec![
        0xE300_0000 | area(x0, y0),
        0xE400_0000 | area(x1, y1),
        0xE500_0000 | offset,
        mode,
        0xE200_0000,
        0xE600_0000,
    ];
    if env.isbg != 0 {
        // Clear the clip area to the background colour [doc]; the packet layout past the six
        // mode words is a guess, the effect isn't.
        words.extend([
            0x0200_0000 | u32::from(env.r0) | u32::from(env.g0) << 8 | u32::from(env.b0) << 16,
            ((c.y as u16 as u32) << 16) | c.x as u16 as u32,
            ((c.h as u16 as u32) << 16) | c.w as u16 as u32,
        ]);
    }
    words
}

/// `PutDrawEnv(env)`: sets the drawing environment; builds its packet in `env->dr_env` as the
/// SDK does [obs] and runs it. Returns `env`.
///
/// # Safety
/// `env` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn PutDrawEnv(env: *mut DrawEnv) -> *mut DrawEnv {
    let e = unsafe { &mut *env };
    let words = draw_env_words(e);
    e.dr_env.tag = (words.len() as u32) << 24 | 0x00FF_FFFF;
    e.dr_env.code[..words.len()].copy_from_slice(&words);
    gpu::with(|g| g.gp0(&words));
    env
}

/// `PutDispEnv(env)`: shows `env->disp` of VRAM [doc; GP1(05h..08h)]. Interlace and the
/// screen position on the TV don't change the 1x picture. Returns `env`.
///
/// # Safety
/// `env` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn PutDispEnv(env: *mut DispEnv) -> *mut DispEnv {
    let e = unsafe { &*env };
    let d = e.disp;
    gpu::with(|g| {
        g.set_display(
            d.x.into(),
            d.y.into(),
            d.w.into(),
            d.h.into(),
            e.isrgb24 != 0,
        )
    });
    env
}

/// `SetDefDispEnv(env, x, y, w, h)`: a display environment showing that rectangle, no
/// interlace, 15-bit [doc]. Returns `env`.
///
/// # Safety
/// `env` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SetDefDispEnv(
    env: *mut DispEnv,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> *mut DispEnv {
    unsafe {
        env.write(DispEnv {
            disp: Rect {
                x: x as i16,
                y: y as i16,
                w: w as i16,
                h: h as i16,
            },
            ..DispEnv::default()
        });
    }
    env
}

/// `SetDispMask(mask)`: display on (1) or off (0) [doc; GP1(03h)].
#[unsafe(no_mangle)]
pub extern "C" fn SetDispMask(mask: i32) {
    gpu::with(|g| g.set_display_enable(mask != 0));
}

// ---- primitives ----

/// `GetTPage(tp, abr, x, y)`: a texture page value [hdr: getTPage].
#[unsafe(no_mangle)]
pub extern "C" fn GetTPage(tp: i32, abr: i32, x: i32, y: i32) -> u16 {
    (((tp & 3) << 7)
        | ((abr & 3) << 5)
        | ((y & 0x100) >> 4)
        | ((x & 0x3FF) >> 6)
        | ((y & 0x200) << 2)) as u16
}

/// `GetClut(x, y)`: a CLUT value [hdr: getClut].
#[unsafe(no_mangle)]
pub extern "C" fn GetClut(x: i32, y: i32) -> u16 {
    ((y << 6) | ((x >> 4) & 0x3F)) as u16
}

/// Sets a primitive's length (tag bits 24-31) and command code (byte 7) [hdr: setlen,
/// setcode].
///
/// # Safety
/// `p` must point to a primitive.
unsafe fn set_len_code(p: *mut c_void, len: u8, code: u8) {
    let b = p.cast::<u8>();
    unsafe {
        b.add(3).write(len);
        b.add(7).write(code);
    }
}

macro_rules! primitive_setters {
    ($($name:ident: $len:expr, $code:expr $(, pad $pad:expr)?;)*) => {
        $(
            /// Sets up a primitive's length and code [hdr].
            ///
            /// # Safety
            /// `p` must point to the primitive.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(p: *mut c_void) {
                unsafe { set_len_code(p, $len, $code) };
                $( unsafe { p.cast::<u32>().add($pad).write(0x5555_5555) }; )?
            }
        )*
    };
}

primitive_setters! {
    SetPolyF3: 4, 0x20;
    SetPolyFT3: 7, 0x24;
    SetPolyG3: 6, 0x30;
    SetPolyGT3: 9, 0x34;
    SetPolyF4: 5, 0x28;
    SetPolyFT4: 9, 0x2C;
    SetPolyG4: 8, 0x38;
    SetPolyGT4: 12, 0x3C;
    SetLineF2: 3, 0x40;
    // LINE_F3/F4 end with the polyline terminator word.
    SetLineF3: 5, 0x48, pad 5;
    SetLineF4: 6, 0x4C, pad 6;
}

/// `SetSemiTrans(p, abe)`: semi-transparency on or off (code bit 1) [hdr: setSemiTrans].
///
/// # Safety
/// `p` must point to a primitive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SetSemiTrans(p: *mut c_void, abe: i32) {
    let code = unsafe { p.cast::<u8>().add(7) };
    unsafe {
        code.write(if abe != 0 {
            code.read() | 2
        } else {
            code.read() & !2
        })
    };
}

/// `SetDrawOffset(p, ofs)`: a `DR_OFFSET` primitive that moves the drawing offset to
/// `(ofs[0], ofs[1])` [doc; obs: length 2, code[0] = E5 command, code[1] = 0; GP0(E5h)].
///
/// # Safety
/// `p` must point to a `DR_OFFSET` and `ofs` to two halfwords.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SetDrawOffset(p: *mut DrOffset, ofs: *const u16) {
    let (x, y) = unsafe { (u32::from(ofs.read()), u32::from(ofs.add(1).read())) };
    unsafe {
        p.cast::<u8>().add(3).write(2);
        (*p).code = [0xE500_0000 | (y & 0x7FF) << 11 | (x & 0x7FF), 0];
    }
}

/// libgpu `DR_LOAD`: a CPU-to-VRAM copy of up to 13 words.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DrLoad {
    pub tag: u32,
    pub code: [u32; 3],
    pub p: [u32; 13],
}

/// `SetDrawLoad(p, r)`: a `DR_LOAD` primitive that copies the pixels the caller puts in `p.p`
/// into VRAM rectangle `r` [doc: libgpu reference; GP0(A0h) copy CPU to VRAM, GP0(01h) clear
/// cache]. [obs: p9-render-drawload, 160 rectangles] The command words are GP0(A0h), then the
/// rectangle's x and y, then its width and height (each halfword as given); the pixel data
/// (`(w * h + 1) / 2` words) is left to the caller, and the word after it becomes GP0(01h); the
/// tag's length byte counts all of that (the link is kept). The game's rectangles are 16 x 1 (a
/// CLUT row: 8 words). For more than 12 words the SDK instead writes a one-word tag with a null
/// link, which nothing in the game asks for, so it stops the program.
///
/// # Safety
/// `p` must point to a `DR_LOAD` and `r` to a `RECT`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SetDrawLoad(p: *mut DrLoad, r: *const Rect) {
    unsafe {
        let r = &*r;
        let words = (i32::from(r.w) * i32::from(r.h) + 1) >> 1;
        if words as u32 > 12 {
            crate::trap::unimplemented("SetDrawLoad with more than 12 words of pixels");
        }
        let words = words as usize;
        let w = p.cast::<u32>();
        w.cast::<u8>().add(3).write(4 + words as u8);
        w.add(1).write(0xA000_0000);
        w.add(2)
            .write(u32::from(r.x as u16) | u32::from(r.y as u16) << 16);
        w.add(3)
            .write(u32::from(r.w as u16) | u32::from(r.h as u16) << 16);
        w.add(4 + words).write(0x0100_0000);
    }
}

/// `AddPrim(ot, p)`: puts primitive `p` at the head of ordering-table entry `ot`: `p` links to
/// what `ot` linked to, `ot` links to `p`; both keep their length bytes [hdr: addPrim; obs].
///
/// # Safety
/// `ot` and `p` must point to a tag word each, inside the link window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn AddPrim(ot: *mut u32, p: *mut u32) {
    unsafe {
        let ot_tag = ot.read();
        p.write((p.read() & 0xFF00_0000) | (ot_tag & 0x00FF_FFFF));
        ot.write((ot_tag & 0xFF00_0000) | link_of(p.cast()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TIM word stream: [id, flags, (clut block)?, pixel block]; a block is
    /// [bytes incl. this word, x|y<<16, w|h<<16, data...] with one data word.
    fn tim(flags: u32, clut: bool, tag: u32) -> Vec<u32> {
        let mut v = vec![0x10, flags];
        if clut {
            v.extend([16, 0, 0x0001_0002, tag | 0xC]);
        }
        v.extend([16, 0, 0x0001_0002, tag | 0xD]);
        v
    }

    /// `OpenTIM`/`ReadTIM` as measured in Redux (oracle/call-sdk.lua, captures/oracle-runs/
    /// p3-tim): OpenTIM always returns 0; ReadTIM walks concatenated TIMs, returns null without
    /// writing at a word that isn't 0x10, and gives a CLUT-less TIM null `crect`/`caddr`.
    #[test]
    fn tim_reading_matches_the_sdk() {
        let mut words = tim(0xA, true, 0x100);
        words.extend(tim(0xB, true, 0x200));
        words.extend([0, 0, 0, 0]); // terminator
        let base = words.as_ptr();
        let mut img = TimImage {
            mode: 0xDEAD,
            crect: std::ptr::dangling_mut::<Rect>(),
            caddr: std::ptr::dangling_mut::<u32>(),
            prect: std::ptr::dangling_mut::<Rect>(),
            paddr: std::ptr::dangling_mut::<u32>(),
        };
        unsafe {
            assert_eq!(OpenTIM(base), 0);
            assert_eq!(ReadTIM(&mut img), &raw mut img);
            assert_eq!(img.mode, 0xA);
            assert_eq!(img.crect.cast::<u32>(), base.add(3).cast_mut());
            assert_eq!(img.caddr, base.add(5).cast_mut());
            assert_eq!(img.prect.cast::<u32>(), base.add(7).cast_mut());
            assert_eq!(img.paddr, base.add(9).cast_mut());
            assert_eq!(tim_cursor(), base.add(10) as usize);
            assert_eq!(ReadTIM(&mut img), &raw mut img);
            assert_eq!(img.mode, 0xB);
            assert_eq!(img.paddr, base.add(19).cast_mut());
            // The terminator: null, `img` and the cursor untouched.
            let (mode, cursor) = (img.mode, tim_cursor());
            assert!(ReadTIM(&mut img).is_null());
            assert_eq!((img.mode, tim_cursor()), (mode, cursor));

            // A bad ID: OpenTIM still returns 0 and moves the cursor; ReadTIM refuses.
            let bad = [0x11u32, 0xA, 0, 0, 0, 0];
            assert_eq!(OpenTIM(bad.as_ptr()), 0);
            assert_eq!(tim_cursor(), bad.as_ptr() as usize);
            assert!(ReadTIM(&mut img).is_null());
            assert_eq!(img.mode, 0xB);

            // Without a CLUT (flag bit 3 clear): null crect and caddr, pixels at word 2+.
            let mut nc = tim(0x2, false, 0x300);
            nc.extend([0, 0]);
            assert_eq!(OpenTIM(nc.as_ptr()), 0);
            assert!(!ReadTIM(&mut img).is_null());
            assert_eq!(img.mode, 2);
            assert!(img.crect.is_null() && img.caddr.is_null());
            assert_eq!(img.prect.cast::<u32>(), nc.as_ptr().add(3).cast_mut());
            assert_eq!(img.paddr, nc.as_ptr().add(5).cast_mut());
            assert!(ReadTIM(&mut img).is_null());
        }
    }

    #[test]
    fn values_match_the_captures() {
        // [obs] p2-landing: GetTPage(1, 0, 0x300, 0) = 0x8C, GetClut(0, 0x1E0) = 0x7800.
        assert_eq!(GetTPage(1, 0, 0x300, 0), 0x8C);
        assert_eq!(GetTPage(1, 0, 0x380, 0), 0x8E);
        assert_eq!(GetClut(0, 0x1E0), 0x7800);
        // [obs] p2-init: GsInitGraph's 320x240 environment at the origin.
        let env = DrawEnv {
            clip: Rect {
                x: 0,
                y: 0,
                w: 320,
                h: 240,
            },
            ..DrawEnv::default()
        };
        assert_eq!(
            draw_env_words(&env),
            [
                0xE300_0000,
                0xE403_BD3F,
                0xE500_0000,
                0xE100_0000,
                0xE200_0000,
                0xE600_0000
            ]
        );
        // [obs] p2-landing: clip (0, 240, 320, 240), offset (160, 360).
        let env = DrawEnv {
            clip: Rect {
                x: 0,
                y: 240,
                w: 320,
                h: 240,
            },
            ofs: [160, 360],
            ..DrawEnv::default()
        };
        assert_eq!(
            draw_env_words(&env)[..3],
            [0xE303_C000, 0xE407_7D3F, 0xE50B_40A0]
        );
    }

    static mut OT: [u32; 2] = [0x00FF_FFFF, 0];
    static mut PRIMS: [[u32; 10]; 2] = [[0; 10]; 2];

    #[test]
    fn primitives_and_links() {
        unsafe {
            let ot = (&raw mut OT).cast::<u32>();
            let a = (&raw mut PRIMS[0]).cast::<u32>();
            let b = (&raw mut PRIMS[1]).cast::<u32>();
            SetPolyFT4(a.cast());
            SetPolyFT4(b.cast());
            SetSemiTrans(b.cast(), 1);
            assert_eq!(a.read() >> 24, 9);
            assert_eq!(a.add(1).read() >> 24, 0x2C);
            assert_eq!(b.add(1).read() >> 24, 0x2E);
            AddPrim(ot, a);
            AddPrim(ot, b);
            // ot -> b -> a -> end
            assert_eq!(crate::memory::ptr_of_link(ot.read()), b);
            assert_eq!(crate::memory::ptr_of_link(b.read()), a);
            assert_eq!(a.read() & 0x00FF_FFFF, 0x00FF_FFFF);
            assert_eq!(a.read() >> 24, 9);

            let mut off = DrOffset::default();
            SetDrawOffset(&raw mut off, [160u16, 360].as_ptr());
            assert_eq!(off.tag >> 24, 2);
            assert_eq!(off.code, [0xE50B_40A0, 0]);
        }
    }
}
