//! libgs's 2D sort calls: `GsSortBoxFill`, `GsSortPoly` and `GsSortSprite`.
//!
//! Clean-room like the rest of libgs: the struct layouts and attribute bits from the Psy-Q
//! headers, the GPU packet formats from psx-spx, the details from calls captured in PCSX-Redux
//! (`captures/oracle-runs/g2d-box-*` and `g2d-spr`, gitignored; `oracle/capture-calls.lua`).
//! Rules are tagged `[doc]`, `[hdr]` or `[obs]`.
//!
//! Each call writes one packet at the work base (`GsGetWorkBase`), advances the work base past
//! it and links it into the ordering table at the head of entry `pri`. Over the route to Jijimon's
//! name entry (Redux VBlanks 3570-5240) the game makes 941 `GsSortBoxFill` and 164 `GsSortPoly`
//! calls, and in File City's clock (VBlank 12773 on) `GsSortSprite`; no `GsSortFastSprite` or
//! `GsSortOt` call occurs anywhere on the route so far, so those stay trapping stubs
//! (`stubs.rs`) until a scene that uses them is captured.

use super::{GsGetWorkBase, GsOt, GsSetWorkBase};
use crate::memory::link_of;
use crate::trap::unimplemented;

/// libgs `GsBOXF` [hdr: libgs.h].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GsBoxf {
    pub attribute: u32,
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// libgs `GsSPRITE` [hdr: libgs.h].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GsSprite {
    pub attribute: u32,
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    pub tpage: u16,
    pub u: u8,
    pub v: u8,
    pub cx: i16,
    pub cy: i16,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub mx: i16,
    pub my: i16,
    pub scalex: i16,
    pub scaley: i16,
    pub rotate: i32,
}

/// `GsALON`: semi-transparency on [hdr: libgs.h `GsALON` (1 << 30)].
const ATTR_ALPHA_ON: u32 = 1 << 30;

/// Checks the ordering table is a plain one and returns the entry for `pri`.
///
/// [obs] `ot.offset` and `ot.point` are 0 in every captured call and `pri` indexes the table
/// directly (entry 6 for `pri` 6). Anything else is unobserved and stops the program.
unsafe fn entry(name: &str, ot: *mut GsOt, pri: u16) -> *mut u32 {
    unsafe {
        let ot = &*ot;
        if ot.offset != 0 || ot.point != 0 {
            unimplemented(&format!("{name} on an ordering table with offset or point"));
        }
        if u32::from(pri) >= 1u32.checked_shl(ot.length).unwrap_or(0) {
            unimplemented(&format!("{name} with a priority past the ordering table"));
        }
        ot.org.add(usize::from(pri))
    }
}

/// Links the packet at `pk` (tag word already holding its length) at the head of `slot`'s list
/// [obs: the packet's link becomes the entry's old link, the entry the packet's].
unsafe fn link_in(slot: *mut u32, pk: *mut u32, len: u32) {
    unsafe {
        let old = slot.read();
        pk.write(len << 24 | (old & 0x00FF_FFFF));
        slot.write((old & 0xFF00_0000) | link_of(pk.cast()));
    }
}

/// `GsSortBoxFill(bp, ot, pri)`: a filled rectangle at `(x, y)`, `w` by `h`, in the colour
/// `(r, g, b)`, drawn at priority `pri` [doc].
///
/// [obs] The packet is five words, a draw-mode word then a flat TILE: `[len 4 | link]`,
/// `E1000200`, `60BBGGRR` (`62BBGGRR` with `GsALON`), `yyyyxxxx`, `hhhhwwww`. The draw-mode word
/// is 0x200 (dither on, texture page 0) in every captured call, `GsALON` or not; the blend mode
/// bits 28-29 of the attribute were 0 in all of them, so any other attribute stops the program.
///
/// # Safety
/// `bp` must be valid, `ot` an ordering table in the link window, and the work base a packet area
/// in the same window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortBoxFill(bp: *const GsBoxf, ot: *mut GsOt, pri: u16) {
    unsafe {
        let b = *bp;
        if b.attribute & !ATTR_ALPHA_ON != 0 {
            unimplemented("GsSortBoxFill with an unobserved attribute bit (not GsALON)");
        }
        let slot = entry("GsSortBoxFill", ot, pri);
        let pk = GsGetWorkBase().cast::<u32>();
        let code = if b.attribute & ATTR_ALPHA_ON != 0 {
            0x62
        } else {
            0x60
        };
        pk.add(1).write(0xE100_0200);
        pk.add(2)
            .write(code << 24 | u32::from(b.b) << 16 | u32::from(b.g) << 8 | u32::from(b.r));
        pk.add(3)
            .write(u32::from(b.y as u16) << 16 | u32::from(b.x as u16));
        pk.add(4).write(u32::from(b.h) << 16 | u32::from(b.w));
        link_in(slot, pk, 4);
        GsSetWorkBase(pk.add(5).cast());
    }
}

/// `GsSortPoly(pp, ot, pri)`: registers a GPU primitive (a `POLY_*`, `SPRT`, ... built by the
/// caller, usually on its stack) in the table at `pri`. libgs copies it to the work area because
/// the caller's copy won't outlive the call [doc; obs].
///
/// [obs] The copy is the tag's length in words after the tag, byte for byte; the caller's link
/// is ignored and the copy's tag holds the length and the table's old link. Only `POLY_FT4`
/// (nine words, command 0x2E: semi-transparent) has been captured; other lengths follow the
/// same rule by construction but are stopped until seen.
///
/// # Safety
/// `pp` must point to a primitive, `ot` an ordering table in the link window, and the work base
/// a packet area in the same window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortPoly(pp: *const u32, ot: *mut GsOt, pri: u16) {
    unsafe {
        let len = pp.read() >> 24;
        if len != 9 {
            unimplemented("GsSortPoly with a primitive other than POLY_FT4 (unobserved)");
        }
        let slot = entry("GsSortPoly", ot, pri);
        let pk = GsGetWorkBase().cast::<u32>();
        for i in 1..=len as usize {
            pk.add(i).write(pp.add(i).read());
        }
        link_in(slot, pk, len);
        GsSetWorkBase(pk.add(1 + len as usize).cast());
    }
}

/// `GsSortSprite(sp, ot, pri)`: a textured sprite `w` by `h` whose reference point `(mx, my)`
/// sits at `(x, y)`, from texture page `tpage` at `(u, v)` with the CLUT at `(cx, cy)`, drawn at
/// priority `pri` [doc].
///
/// [obs] Unscaled, unrotated and unmodified sprites (`attribute` 0, `scalex` = `scaley` = 4096
/// meaning 1.0, `rotate` 0, colour 128/128/128) are one `SPRT` packet of five words:
/// `[len 5 | link]`, `E1000200 | tpage`, `64BBGGRR`, `(y - my) << 16 | (x - mx)`,
/// `clut << 16 | v << 8 | u`, `h << 16 | w`. Scaling and rotation (which turn the sprite into a
/// four-point polygon through the GTE), blend and colour-mode bits, other colours, and sprites
/// wider than 255 haven't been captured, and stop the program.
///
/// # Safety
/// `sp` must be valid, `ot` an ordering table in the link window, and the work base a packet
/// area in the same window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortSprite(sp: *const GsSprite, ot: *mut GsOt, pri: u16) {
    unsafe {
        let s = *sp;
        if s.attribute != 0 {
            unimplemented("GsSortSprite with an unobserved attribute (blend, colour mode, ...)");
        }
        if s.scalex != 4096 || s.scaley != 4096 || s.rotate != 0 {
            unimplemented("GsSortSprite with scaling or rotation (unobserved)");
        }
        if (s.r, s.g, s.b) != (128, 128, 128) || s.tpage > 0x1FF || s.w > 255 || s.h > 255 {
            unimplemented("GsSortSprite with an unobserved colour, texture page or size");
        }
        let slot = entry("GsSortSprite", ot, pri);
        let pk = GsGetWorkBase().cast::<u32>();
        let clut = crate::libgpu::GetClut(s.cx.into(), s.cy.into());
        pk.add(1).write(0xE100_0200 | u32::from(s.tpage));
        pk.add(2)
            .write(0x6400_0000 | u32::from(s.b) << 16 | u32::from(s.g) << 8 | u32::from(s.r));
        let (x, y) = (s.x.wrapping_sub(s.mx), s.y.wrapping_sub(s.my));
        pk.add(3)
            .write(u32::from(y as u16) << 16 | u32::from(x as u16));
        pk.add(4)
            .write(u32::from(clut) << 16 | u32::from(s.v) << 8 | u32::from(s.u));
        pk.add(5).write(u32::from(s.h) << 16 | u32::from(s.w));
        link_in(slot, pk, 5);
        GsSetWorkBase(pk.add(6).cast());
    }
}
