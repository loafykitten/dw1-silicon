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

/// libgpu `TIM_IMAGE`, native pointers [hdr].
#[repr(C)]
pub struct TimImage {
    pub mode: u32,
    pub crect: *mut Rect,
    pub caddr: *mut u32,
    pub prect: *mut Rect,
    pub paddr: *mut u32,
}

/// The TIM `OpenTIM` was last given (the SDK keeps it in a global) [doc].
static TIM: Mutex<usize> = Mutex::new(0);

/// `OpenTIM(addr)`: remembers a TIM file in memory for `ReadTIM`. Returns 0, or 1 if it
/// doesn't start with the TIM id (0x10) [doc: library reference; format: psx-spx "TIM"].
///
/// # Safety
/// `addr` must point to a TIM file.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn OpenTIM(addr: *const u32) -> i32 {
    if unsafe { addr.read_unaligned() } != 0x10 {
        return 1;
    }
    *TIM.lock().unwrap() = addr as usize;
    0
}

/// `ReadTIM(img)`: describes the TIM `OpenTIM` was given: pixel mode (the header's flag word),
/// the CLUT block's rectangle and data (null if the file has no CLUT) and the pixel block's
/// rectangle and data, all pointing into the file [doc: psx-spx "TIM": flag bit 3 = has CLUT;
/// each block is a size word, a RECT and halfwords]. Returns `img`; null if no TIM is open.
///
/// # Safety
/// `img` must be valid and the opened TIM still in memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ReadTIM(img: *mut TimImage) -> *mut TimImage {
    let base = *TIM.lock().unwrap() as *const u32;
    if base.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let flag = base.add(1).read_unaligned();
        let mut block = base.add(2);
        let (crect, caddr) = if flag & 8 != 0 {
            let words = block.read_unaligned() as usize;
            let c = (
                block.add(1).cast_mut().cast::<Rect>(),
                block.add(3).cast_mut(),
            );
            block = block.byte_add(words);
            c
        } else {
            (std::ptr::null_mut(), std::ptr::null_mut())
        };
        img.write(TimImage {
            mode: flag,
            crect,
            caddr,
            prect: block.add(1).cast_mut().cast::<Rect>(),
            paddr: block.add(3).cast_mut(),
        });
    }
    img
}

/// `LoadClut(clut, x, y)`: uploads 256 colours to row `y` at `x`; returns the CLUT id
/// (`GetClut(x, y)`) [doc].
///
/// # Safety
/// `clut` must hold 256 halfwords.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LoadClut(clut: *const u32, x: i32, y: i32) -> i32 {
    let r = Rect {
        x: x as i16,
        y: y as i16,
        w: 256,
        h: 1,
    };
    unsafe { LoadImage(&r, clut) };
    i32::from(GetClut(x, y))
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
