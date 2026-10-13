//! libgs `GsGetTimInfo`: reading a TIM image's header.

/// libgs `GsIMAGE`.
#[repr(C)]
#[derive(Debug)]
pub struct GsImage {
    pub pmode: u32,
    pub px: i16,
    pub py: i16,
    pub pw: u16,
    pub ph: u16,
    pub pixel: *mut u32,
    pub cx: i16,
    pub cy: i16,
    pub cw: u16,
    pub ch: u16,
    pub clut: *mut u32,
}

/// `GsGetTimInfo(im, tim)`: `im` points at a TIM's flags word, after its ID. Fills `tim` with
/// the mode, the CLUT block (if flags bit 3 says there is one) and the pixel block: each block
/// is a length word, x/y, w/h halfwords, then the data [doc: psx-spx TIM; obs: flags 8 gave
/// pmode 8, the CLUT at +16 and the pixels after the CLUT block].
///
/// # Safety
/// `im` must point to a TIM's flags word, and `tim` be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsGetTimInfo(im: *mut u32, tim: *mut GsImage) {
    unsafe {
        let t = &mut *tim;
        let flags = im.read();
        t.pmode = flags;
        let mut block = im.add(1);
        // A block: [length in bytes, incl. this header][x, y][w, h][data].
        let header = |b: *mut u32| {
            let xy = b.add(1).read();
            let wh = b.add(2).read();
            (xy as i16, (xy >> 16) as i16, wh as u16, (wh >> 16) as u16)
        };
        if flags & 8 != 0 {
            let (x, y, w, h) = header(block);
            (t.cx, t.cy, t.cw, t.ch) = (x, y, w, h);
            t.clut = block.add(3);
            block = block.cast::<u8>().add(block.read() as usize).cast();
        }
        let (x, y, w, h) = header(block);
        (t.px, t.py, t.pw, t.ph) = (x, y, w, h);
        t.pixel = block.add(3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_blocks_like_the_capture() {
        // [obs] p2-init3d: flags 8; CLUT 16x16 at (0x100, 0x1F0); pixels 64x64 at (0x380, 0x1C0).
        let mut tim = vec![0u32; 3 + 128 + 3 + 4];
        tim[0] = 8;
        tim[1] = 12 + 0x200; // CLUT block: header + 16 * 16 * 2 bytes
        tim[2] = (0x1F0 << 16) | 0x100;
        tim[3] = (16 << 16) | 16;
        let p = 1 + (12 + 0x200) / 4;
        tim[p] = 12 + 0x2000;
        tim[p + 1] = (0x1C0 << 16) | 0x380;
        tim[p + 2] = (64 << 16) | 64;
        let mut out: GsImage = unsafe { std::mem::zeroed() };
        unsafe { GsGetTimInfo(tim.as_mut_ptr(), &mut out) };
        assert_eq!(out.pmode, 8);
        assert_eq!((out.cx, out.cy, out.cw, out.ch), (0x100, 0x1F0, 16, 16));
        assert_eq!(out.clut, unsafe { tim.as_mut_ptr().add(4) });
        assert_eq!((out.px, out.py, out.pw, out.ph), (0x380, 0x1C0, 64, 64));
        assert_eq!(out.pixel, unsafe { tim.as_mut_ptr().add(p + 3) });
    }
}
