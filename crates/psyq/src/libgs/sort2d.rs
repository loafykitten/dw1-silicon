//! libgs's 2D sort calls: `GsSortBoxFill`, `GsSortPoly`, `GsSortSprite` and `GsSortFastSprite`.
//!
//! Clean-room like the rest of libgs: the struct layouts and attribute bits from the Psy-Q
//! headers, the GPU packet formats from psx-spx, the details from calls captured in PCSX-Redux
//! (`captures/oracle-runs/g2d-box-*` and `g2d-spr`, gitignored; `oracle/capture-calls.lua`).
//! Rules are tagged `[doc]`, `[hdr]` or `[obs]`.
//!
//! Each call writes one packet at the work base (`GsGetWorkBase`), advances the work base past
//! it and links it into the ordering table at the head of entry `pri`. Over the route to Jijimon's
//! name entry (Redux VBlanks 3570-5240) the game makes 941 `GsSortBoxFill` and 164 `GsSortPoly`
//! calls, and in File City's clock (VBlank 12773 on) `GsSortSprite`. The first battle (Phase 5,
//! `captures/oracle-runs/p5-btl-calls`) adds scaled, semi-transparent and coloured sprites and
//! `GsSortFastSprite`.

use super::{GsGetWorkBase, GsOt, GsSetWorkBase};
use crate::memory::link_of;
use crate::trap::unimplemented;
use backend::gte::{cmd, dreg};

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

/// Reads a caller-built argument struct. The game builds these on its stack or in globals and may
/// leave fields it doesn't care about unset; a volatile read keeps the compiler from treating
/// those bytes as uninitialised-and-therefore-anything (they only matter if a field the checks
/// below look at is garbage, and then the checks stop the program). Strictly, reading unset
/// bytes is still undefined in Rust's model, volatile or not; the memory comes from C across
/// the FFI boundary, and the volatile read is what stops the compiler from exploiting it.
unsafe fn read_arg<T: Copy>(p: *const T) -> T {
    unsafe { std::ptr::read_volatile(p) }
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
        let b = read_arg(bp);
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
/// [obs] The copy is the tag's length in words after the tag, byte for byte (but for the
/// GPU-ignored pad halfwords, zeroed); the caller's link
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
        // Only the length byte of the tag is read typed: the game's `SetPolyFT4` writes just the
        // length and command bytes, so the link bytes and the primitive's pad halfwords can be
        // uninitialised stack. The payload is copied as bytes.
        let len = pp.cast::<u8>().add(3).read();
        if len != 9 {
            unimplemented("GsSortPoly with a primitive other than POLY_FT4 (unobserved)");
        }
        let slot = entry("GsSortPoly", ot, pri);
        let pk = GsGetWorkBase().cast::<u32>();
        std::ptr::copy_nonoverlapping(
            pp.cast::<u8>().add(4),
            pk.cast::<u8>().add(4),
            4 * len as usize,
        );
        // POLY_FT4's two pad halfwords (the high halves of words 7 and 9) are ignored by the GPU;
        // zeroed so the packet holds no uninitialised bytes.
        pk.cast::<u16>().add(2 * 7 + 1).write(0);
        pk.cast::<u16>().add(2 * 9 + 1).write(0);
        link_in(slot, pk, len as u32);
        GsSetWorkBase(pk.add(1 + len as usize).cast());
    }
}

/// `GsSPRITE` attribute bits [hdr: libgs.h]: brightness (colour modulation) off, the colour mode
/// (0 4-bit, 1 8-bit, 2 15-bit CLUT-less), the semi-transparency rate, semi-transparency on, and
/// display off.
const ATTR_BRIGHT_OFF: u32 = 1 << 6;
const ATTR_COLOR_MODE: u32 = 3 << 24;
const ATTR_ALPHA_RATE: u32 = 3 << 28;
const ATTR_DISPLAY_OFF: u32 = 1 << 31;

/// The attribute's draw-mode bits for a sprite's packets: the command's flags and the texture
/// page word.
///
/// [obs] (`libgs-sprite-btl`: every combination of brightness off, colour modes 0-3, the four
/// rates with and without `GsALON`, on both entry points and in every form): the packet's command
/// gets bit 0 (raw texture) from brightness off and bit 1 (semi-transparent) from `GsALON`; the
/// texture page word takes the page's position bits (0-4) from `tpage` and its rate (bits 5-6) and
/// colour mode (bits 7-8) from the attribute, whether or not `GsALON` is set.
fn sprite_mode(attribute: u32, tpage: u16) -> (u32, u32) {
    let rate = (attribute & ATTR_ALPHA_RATE) >> 28;
    let color_mode = (attribute & ATTR_COLOR_MODE) >> 24;
    let mut flags = 0;
    if attribute & ATTR_BRIGHT_OFF != 0 {
        flags |= 1;
    }
    if attribute & ATTR_ALPHA_ON != 0 {
        flags |= 2;
    }
    (flags, u32::from(tpage & 0x1F) | rate << 5 | color_mode << 7)
}

/// Whether a sprite draws at all, or `Err` for what hasn't been observed (the caller stops the
/// program).
///
/// [obs] Display off (attribute bit 31) or a side of 0 draws nothing and leaves the work base, on
/// both entry points, whatever the other fields. Attribute bits outside the ones above (bits
/// 0-5, 7-23 and 26-27) and sides over 300 (the largest observed) haven't been observed.
fn sprite_drawn(s: &GsSprite) -> Result<bool, &'static str> {
    let known = ATTR_BRIGHT_OFF | ATTR_COLOR_MODE | ATTR_ALPHA_RATE | ATTR_ALPHA_ON | ATTR_DISPLAY_OFF;
    if s.attribute & !known != 0 {
        return Err("an unobserved attribute bit");
    }
    if s.w > 300 || s.h > 300 {
        return Err("a side over 300 (unobserved)");
    }
    Ok(s.attribute & ATTR_DISPLAY_OFF == 0 && s.w != 0 && s.h != 0)
}

/// Writes one `SPRT` packet with its draw-mode word (six words with the tag) and links it at
/// `slot`: `[len 5 | link]`, `E1000200 | page`, `(64 | flags) BBGGRR`, `yyyyxxxx`,
/// `clut << 16 | v << 8 | u`, `h << 16 | w` [obs].
unsafe fn write_sprt(slot: *mut u32, s: &GsSprite, flags: u32, page: u32, x: i32, y: i32) {
    unsafe {
        let pk = GsGetWorkBase().cast::<u32>();
        let clut = u32::from(crate::libgpu::GetClut(s.cx.into(), s.cy.into()));
        let rgb = u32::from(s.b) << 16 | u32::from(s.g) << 8 | u32::from(s.r);
        pk.add(1).write(0xE100_0200 | page);
        pk.add(2).write((0x64 | flags) << 24 | rgb);
        pk.add(3).write((y as u16 as u32) << 16 | x as u16 as u32);
        pk.add(4)
            .write(clut << 16 | u32::from(s.v) << 8 | u32::from(s.u));
        pk.add(5).write(u32::from(s.h) << 16 | u32::from(s.w));
        link_in(slot, pk, 5);
        GsSetWorkBase(pk.add(6).cast());
    }
}

/// `GsSortSprite(sp, ot, pri)`: a textured sprite `w` by `h` whose reference point `(mx, my)`
/// sits at `(x, y)`, from texture page `tpage` at `(u, v)` with the CLUT at `(cx, cy)`, drawn at
/// priority `pri`, scaled by `scalex`/`scaley` (4096 is 1.0) and turned by `rotate` (1/4096
/// degrees, clockwise on screen) around that point [doc; obs].
///
/// [obs] A sprite that isn't drawn (`sprite_drawn`) leaves the work base. Otherwise a sprite
/// with `rotate` 0 and both scales 4096 is one `SPRT` (`write_sprt`) at `(x - mx, y - my)`; any
/// other is one `POLY_FT4` of nine words: `[len 9 | link]`, `(2C | flags) BBGGRR`, then per corner
/// (top left, top right, bottom left, bottom right) its XY and a texture word: the CLUT with
/// `(u, v)`, then the texture page word (`sprite_mode`) << 16 with `(u + w - 1, v)`, then
/// `(u, v + h - 1)`, then `(u + w - 1, v + h - 1)`. The corner `(dx, dy)` from the reference point
/// (`-mx` or `w - mx`, `-my` or `h - my`) goes through the GTE (below) with the matrix of the turn
/// by `a = rotate / 360` (truncating, in libgte's 4096-per-turn units: `rcos`/`rsin`), and when
/// either scale isn't 4096, that matrix's columns multiplied by `scalex`, `scaley` and 0
/// (`(m * s) >> 12`; so the third row's 4096 becomes 0). Checked against all 60 angles of the
/// clock hand and 600 random rotations (`libgs-sprite`), 1,525 battle-set requests with scales,
/// colours, every attribute combination and odd sizes (`libgs-sprite-btl`;
/// `tests/libgs_sprite_oracle.rs`), and 49 captured calls.
///
/// What `sprite_drawn` rejects hasn't been observed and stops the program.
///
/// # Safety
/// `sp` must be valid, `ot` an ordering table in the link window, and the work base a packet
/// area in the same window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortSprite(sp: *const GsSprite, ot: *mut GsOt, pri: u16) {
    unsafe {
        let s = read_arg(sp);
        match sprite_drawn(&s) {
            Ok(true) => {}
            Ok(false) => return,
            Err(what) => unimplemented(&format!("GsSortSprite with {what}")),
        }
        let (flags, page) = sprite_mode(s.attribute, s.tpage);
        let slot = entry("GsSortSprite", ot, pri);
        let scaled = s.scalex != 4096 || s.scaley != 4096;
        if s.rotate == 0 && !scaled {
            write_sprt(
                slot,
                &s,
                flags,
                page,
                i32::from(s.x) - i32::from(s.mx),
                i32::from(s.y) - i32::from(s.my),
            );
            return;
        }
        let pk = GsGetWorkBase().cast::<u32>();
        let clut = u32::from(crate::libgpu::GetClut(s.cx.into(), s.cy.into()));
        let rgb = u32::from(s.b) << 16 | u32::from(s.g) << 8 | u32::from(s.r);
        let (u, v) = (u32::from(s.u), u32::from(s.v));
        // [obs] The corners go through the GTE: RT is the (scaled) turn about the Z axis, TR is
        // the sprite's position with Z set to the projection distance H (so H / SZ3 is exactly
        // 1, and with OFX = OFY = 0, as the game leaves them, SXY is the transformed point), RTPT
        // on the first three corners, RTPS on the fourth. The GTE registers it leaves (RT, TR,
        // the vertices, the screen FIFO) are the SDK's: the rotation matrix and translation are
        // compared against the captures, the data registers are scratch. No wobble doesn't record
        // these corners: the sprite is 2D and keeps its integer ones on the HD surfaces.
        let angle = s.rotate / 360;
        let (c, sn) = (crate::libgte::rcos(angle), crate::libgte::rsin(angle));
        let mut m = [[c, -sn, 0], [sn, c, 0], [0, 0, 0x1000]];
        if scaled {
            let scale = [i32::from(s.scalex), i32::from(s.scaley), 0];
            for row in &mut m {
                for (e, k) in row.iter_mut().zip(scale) {
                    *e = (*e * k) >> 12;
                }
            }
        }
        let (w, h) = (i32::from(s.w), i32::from(s.h));
        let (mx, my) = (i32::from(s.mx), i32::from(s.my));
        let corners = [(-mx, -my), (w - mx, -my), (-mx, h - my), (w - mx, h - my)];
        let half = |lo: i32, hi: i32| (hi as u16 as u32) << 16 | lo as u16 as u32;
        let sxy = backend::pgxp::without(|| {
            crate::gte::with(|g| {
                g.ctc2(0, half(m[0][0], m[0][1]));
                g.ctc2(1, half(m[0][2], m[1][0]));
                g.ctc2(2, half(m[1][1], m[1][2]));
                g.ctc2(3, half(m[2][0], m[2][1]));
                g.ctc2(4, m[2][2] as u16 as u32);
                g.ctc2(5, i32::from(s.x) as u32);
                g.ctc2(6, i32::from(s.y) as u32);
                let proj = g.cfc2(26);
                g.ctc2(7, proj);
                for (k, &(dx, dy)) in corners[..3].iter().enumerate() {
                    g.mtc2(dreg::VXY0 + 2 * k, half(dx, dy));
                    g.mtc2(dreg::VZ0 + 2 * k, 0);
                }
                g.execute(cmd::RTPT);
                let mut sxy = [0u32; 4];
                for (k, v) in sxy.iter_mut().take(3).enumerate() {
                    *v = g.mfc2(dreg::SXY0 + k);
                }
                g.mtc2(dreg::VXY0, half(corners[3].0, corners[3].1));
                g.mtc2(dreg::VZ0, 0);
                g.execute(cmd::RTPS);
                sxy[3] = g.mfc2(dreg::SXY2);
                sxy
            })
        });
        let (u1, v1) = (
            u.wrapping_add(w as u32 - 1) & 0xFF,
            v.wrapping_add(h as u32 - 1) & 0xFF,
        );
        pk.add(1).write((0x2C | flags) << 24 | rgb);
        pk.add(2).write(sxy[0]);
        pk.add(3).write(clut << 16 | v << 8 | u);
        pk.add(4).write(sxy[1]);
        pk.add(5).write(page << 16 | v << 8 | u1);
        pk.add(6).write(sxy[2]);
        pk.add(7).write(v1 << 8 | u);
        pk.add(8).write(sxy[3]);
        pk.add(9).write(v1 << 8 | u1);
        link_in(slot, pk, 9);
        GsSetWorkBase(pk.add(10).cast());
    }
}

/// `GsSortFastSprite(sp, ot, pri)`: `GsSortSprite` without rotation, scaling or a reference
/// point [doc; obs].
///
/// [obs] (`libgs-sprite-btl`, 470 requests): always one `SPRT` (`write_sprt`) with its top left
/// corner at `(x, y)`; `mx`, `my`, `scalex`, `scaley` and `rotate` are ignored, the attribute bits
/// act as in `GsSortSprite`, and the same sprites aren't drawn (`sprite_drawn`; the battle's
/// gauges pass a width of 0 when empty). What `sprite_drawn` rejects stops the program.
///
/// # Safety
/// As `GsSortSprite`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortFastSprite(sp: *const GsSprite, ot: *mut GsOt, pri: u16) {
    unsafe {
        let s = read_arg(sp);
        match sprite_drawn(&s) {
            Ok(true) => {}
            Ok(false) => return,
            Err(what) => unimplemented(&format!("GsSortFastSprite with {what}")),
        }
        let (flags, page) = sprite_mode(s.attribute, s.tpage);
        let slot = entry("GsSortFastSprite", ot, pri);
        write_sprt(slot, &s, flags, page, s.x.into(), s.y.into());
    }
}
