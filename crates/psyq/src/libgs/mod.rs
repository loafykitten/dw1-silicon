//! libgs: the display buffers, ordering tables and 2D sorting (this file), and object sorting
//! (`sort4j`, the S0.7 spike, still on a PS1 memory model).
//!
//! Clean-room like libgpu (`crate::libgpu`): API meaning from the Psy-Q headers and library
//! reference, the details from emulator observation of DW1's own calls (captures `p2-init`,
//! `p2-landing`, `p2-menu`, 2026-10-13). Rules are tagged `[doc]`, `[hdr]` or `[obs]`.

mod coord;
mod divide;
mod light;
mod model;
mod sort2d;
mod sort4j;
mod tim;
mod tmd;
mod view;

pub use coord::*;
pub use divide::*;
pub use light::*;
pub use model::*;
pub use sort2d::*;
pub use sort4j::*;
pub use tim::*;
pub use tmd::*;
pub use view::*;

use std::sync::Mutex;

use crate::globals::{GsIDMATRIX, Matrix, PSDCNT};
use crate::gpu;
use crate::libgpu::{self, DispEnv, DrawEnv, Rect};
use crate::memory::{LINK_END, link_of, ptr_of_link};
use crate::trap::unimplemented;

/// libgs `GsOT`.
#[repr(C)]
#[derive(Debug)]
pub struct GsOt {
    /// log2 of the number of entries.
    pub length: u32,
    /// The entries.
    pub org: *mut u32,
    pub offset: u32,
    pub point: u32,
    /// The entry drawing starts from.
    pub tag: *mut u32,
}

/// libgs's display state (its private globals on the PS1).
struct Display {
    width: i16,
    height: i16,
    /// The two buffers' top-left corners in VRAM (`GsDefDispBuff`).
    buffers: [(i16, i16); 2],
    /// The buffer being drawn (`GsGetActiveBuff`); the other one is displayed.
    active: usize,
    /// `GsSetOrign`: added to the buffer corner for the drawing offset.
    origin: (i16, i16),
    draw: DrawEnv,
    disp: DispEnv,
    /// `GsSetWorkBase`: where the next sorted packet goes (an address; raw pointers aren't
    /// `Send`).
    work_base: usize,
}

static DISPLAY: Mutex<Display> = Mutex::new(Display {
    width: 0,
    height: 0,
    buffers: [(0, 0); 2],
    active: 0,
    origin: (0, 0),
    draw: DrawEnv {
        clip: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        ofs: [0; 2],
        tw: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        tpage: 0,
        dtd: 0,
        dfe: 0,
        isbg: 0,
        r0: 0,
        g0: 0,
        b0: 0,
        dr_env: libgpu::DrEnv {
            tag: 0,
            code: [0; 15],
        },
    },
    disp: DispEnv {
        disp: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        screen: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        isinter: 0,
        isrgb24: 0,
        pad0: 0,
        pad1: 0,
    },
    work_base: 0,
});

/// `GsSortClear`'s fill packets, one per buffer, kept by libgs [obs: two 4-word packets in the
/// SDK's data; the one for the buffer being drawn is used]. In the link window.
static mut CLEAR_PACKETS: [[u32; 4]; 2] = [[0; 4]; 2];

fn display<R>(f: impl FnOnce(&mut Display) -> R) -> R {
    f(&mut DISPLAY.lock().unwrap())
}

impl Display {
    /// Points the drawing environment at the active buffer and sends it [obs: GsSwapDispBuff
    /// calls GsSetDrawBuffClip then GsSetDrawBuffOffset, each a PutDrawEnv].
    fn put_draw_buffer(&mut self) {
        let (x, y) = self.buffers[self.active];
        self.draw.clip = Rect {
            x,
            y,
            w: self.width,
            h: self.height,
        };
        self.draw.ofs = [x.wrapping_add(self.origin.0), y.wrapping_add(self.origin.1)];
        unsafe { libgpu::PutDrawEnv(&raw mut self.draw) };
    }
}

/// `GsInitGraph(w, h, intmode, dither, vrammode)`: resets the GPU and sets up a `w` x `h`
/// display at the VRAM origin [doc; obs]. DW1 passes (320, 240, 4, 0, 0): non-interlaced, the
/// GPU's own drawing offset (`GsOFSGPU`), no dither.
#[unsafe(no_mangle)]
pub extern "C" fn GsInitGraph(w: u16, h: u16, intmode: u16, dither: u16, _vrammode: u16) {
    if intmode & 1 != 0 {
        // Interlaced libgs (GsINTER) draws both fields; DW1 never asks for it.
        crate::trap::unimplemented("GsInitGraph with interlace");
    }
    libgpu::ResetGraph(0);
    unsafe { PSDCNT = 1 };
    display(|d| {
        d.width = w as i16;
        d.height = h as i16;
        d.active = 0;
        d.origin = (0, 0);
        d.draw = DrawEnv {
            clip: Rect {
                x: 0,
                y: 0,
                w: d.width,
                h: d.height,
            },
            dtd: u8::from(dither != 0),
            ..DrawEnv::default()
        };
        d.disp = DispEnv {
            disp: Rect {
                x: 0,
                y: 0,
                w: d.width,
                h: d.height,
            },
            ..DispEnv::default()
        };
        unsafe {
            libgpu::PutDrawEnv(&raw mut d.draw);
            libgpu::PutDispEnv(&raw mut d.disp);
        }
    });
    // [obs] the unit matrix, and four GTE registers: OFX = OFY = 0, H = 1000, DQA = -0x1062.
    unsafe { GsIDMATRIX = Matrix::IDENTITY };
    crate::gte::with(|g| {
        g.ctc2(24, 0);
        g.ctc2(25, 0);
        g.ctc2(26, 1000);
        g.ctc2(27, 0xFFFF_EF9E);
    });
}

/// `GsDefDispBuff(x0, y0, x1, y1)`: the two buffers' corners, and the drawing environment
/// re-sent for the active one [doc; obs].
#[unsafe(no_mangle)]
pub extern "C" fn GsDefDispBuff(x0: u16, y0: u16, x1: u16, y1: u16) {
    display(|d| {
        d.buffers = [(x0 as i16, y0 as i16), (x1 as i16, y1 as i16)];
        d.put_draw_buffer();
    });
}

/// `GsInit3D()`: sets up libgs's 3D state. Nothing it sets is visible on the PS1 in DW1's
/// boot [obs: no GTE register or libgs value DW1 reads changes], so nothing to do until the
/// 3D calls need state of their own.
#[unsafe(no_mangle)]
pub extern "C" fn GsInit3D() {}

/// `GsSetOrign(x, y)`: the screen origin, used from the next buffer swap [obs].
#[unsafe(no_mangle)]
pub extern "C" fn GsSetOrign(x: i32, y: i32) {
    display(|d| d.origin = (x as i16, y as i16));
}

/// `GsGetActiveBuff()`: the buffer being drawn, 0 or 1 [doc; obs].
#[unsafe(no_mangle)]
pub extern "C" fn GsGetActiveBuff() -> i32 {
    display(|d| d.active as i32)
}

/// `GsSwapDispBuff()`: flips the buffers: the one just drawn is displayed and drawing moves to
/// the other [obs: libgs's swap counter `PSDCNT` goes up by one; the active index toggles; the display shows the inactive buffer; then
/// PutDispEnv, SetDispMask(1), and the drawing environment for the new active buffer].
#[unsafe(no_mangle)]
pub extern "C" fn GsSwapDispBuff() {
    unsafe { PSDCNT = PSDCNT.wrapping_add(1) };
    display(|d| {
        d.active ^= 1;
        let (x, y) = d.buffers[d.active ^ 1];
        d.disp.disp.x = x;
        d.disp.disp.y = y;
        unsafe { libgpu::PutDispEnv(&raw mut d.disp) };
        libgpu::SetDispMask(1);
        d.put_draw_buffer();
    });
}

/// `GsSetWorkBase(p)`: where libgs writes the next packets [doc].
#[unsafe(no_mangle)]
pub extern "C" fn GsSetWorkBase(p: *mut u8) {
    display(|d| d.work_base = p as usize);
}

/// `GsGetWorkBase()`: the current packet area pointer [doc].
#[unsafe(no_mangle)]
pub extern "C" fn GsGetWorkBase() -> *mut u8 {
    display(|d| d.work_base as *mut u8)
}

/// `GsClearOt(offset, point, ot)`: empties an ordering table: each entry links to the one
/// below it, entry 0 ends the list, and drawing starts at the last entry, so higher entries
/// draw first [obs: entries hold the link to the entry 4 bytes lower, `tag` is
/// `org + (1 << length) - 1`; on the PS1 entry 0 links to two empty packets of the SDK's own,
/// which draw nothing].
///
/// # Safety
/// `ot` must be valid, with `org` holding `1 << length` entries in the link window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsClearOt(offset: u16, point: u16, ot: *mut GsOt) {
    let ot = unsafe { &mut *ot };
    let n = 1usize << ot.length;
    ot.offset = offset.into();
    ot.point = point.into();
    unsafe {
        ot.org.write(LINK_END);
        for i in 1..n {
            ot.org.add(i).write(link_of(ot.org.add(i - 1).cast()));
        }
        ot.tag = ot.org.add(n - 1);
    }
}

/// `GsDrawOt(ot)`: draws an ordering table from `ot->tag` [doc; obs].
///
/// # Safety
/// `ot` and every packet linked from it must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsDrawOt(ot: *mut GsOt) {
    unsafe { gpu::draw_chain((*ot).tag) };
}

/// `GsSortOt(src, dst)`: puts the whole of the ordering table `src` into `dst` as one chain and
/// returns `dst` [obs: 352 `call-sdk.lua` requests and 13 captured calls of the Digimon status
/// screen's partner model]. `dst`'s entry `src->point - dst->offset` is made to link to
/// `src->tag` (the top entry, so `src`'s contents draw in turn as that entry's), with a zero
/// length byte. The end of `src`'s chain, which ended its own list, is made to continue with what
/// the entry linked to before: that is the link of the last element of the chain that starts at
/// `src`'s entry 0 (the entry itself when no packet hangs off bucket 0, else the last packet,
/// whose length byte stays); nothing else is written.
///
/// On the PS1 an emptied table's entry 0 links to two empty packets of the SDK's own and the end
/// of the chain is found by that link; natively it is `LINK_END`, which is the same place in
/// every case measured (the SDK, given a chain ending in the `0xFFFFFF` marker, overwrote entry
/// 0 even with packets behind it; that state doesn't occur in the game and isn't copied).
///
/// An entry index outside `dst` (the SDK writes outside the table) stops the program.
///
/// # Safety
/// Both tables and the packets linked from `src`'s entry 0 must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortOt(src: *mut GsOt, dst: *mut GsOt) -> *mut GsOt {
    unsafe {
        let index = i64::from((*src).point as i32) - i64::from((*dst).offset as i32);
        if !(0..1i64 << ((*dst).length & 31)).contains(&index) {
            unimplemented("GsSortOt with an entry index outside the destination table");
        }
        let slot = (*dst).org.add(index as usize);
        // The last word of the chain that starts at entry 0.
        let mut end = (*src).org;
        for _ in 0..1 << 20 {
            let next = ptr_of_link(end.read());
            if next.is_null() {
                end.write((end.read() & 0xFF00_0000) | (slot.read() & LINK_END));
                slot.write(link_of((*src).tag.cast()));
                return dst;
            }
            end = next;
        }
        unimplemented("GsSortOt on a chain that doesn't end");
    }
}

/// `GsSortClear(r, g, b, ot)`: clears the buffer being drawn to (r, g, b) before anything else
/// in `ot` draws: a fill packet over the drawing area goes at the head of the list [obs: the
/// packet is `[len 3 | link] 02BBGGRR yyyyxxxx hhhhwwww`, linked in front of `ot->tag`'s
/// list, one packet per buffer].
///
/// # Safety
/// `ot` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSortClear(r: u8, g: u8, b: u8, ot: *mut GsOt) {
    let (index, clip) = display(|d| (d.active, d.draw.clip));
    unsafe {
        let head = (*ot).tag;
        let packet = (&raw mut CLEAR_PACKETS[index]).cast::<u32>();
        let words = [
            (3 << 24) | (head.read() & LINK_END),
            0x0200_0000 | u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16,
            (clip.y as u16 as u32) << 16 | clip.x as u16 as u32,
            (clip.h as u16 as u32) << 16 | clip.w as u16 as u32,
        ];
        for (i, w) in words.into_iter().enumerate() {
            packet.add(i).write(w);
        }
        head.write((head.read() & 0xFF00_0000) | link_of(packet.cast()));
    }
}
