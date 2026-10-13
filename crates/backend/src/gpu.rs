//! Model of the PS1 GPU: 1024x512 16-bit VRAM, GP0 drawing commands, display readout.
//!
//! Translated from DW2-PC (github.com/Wyrelade/DW2-PC, CC0), `backend/psxgpu.c` at commit
//! 2d491b5. That is DW2-PC's original work. Only the 1x software path is ported: the HD
//! renderer (`psxgpu_hd*`), the PGXP hooks and the SDL_GPU path are left out, and the places
//! where the C calls into them are marked below. Behaviour follows the C exactly, including
//! its quirks (see the notes on each function); `crates/gpu-diff` checks that bit for bit
//! against the C source.
//!
//! Rasterization rules (as in the C): pixel centres on integer coordinates, top and left
//! polygon edges included, right and bottom excluded; quads are the triangles (v0 v1 v2) and
//! (v1 v2 v3); polygons wider than 1023 or taller than 511 are dropped; texel 0 is
//! transparent; semi-transparency applies to untextured primitives with the semi bit and to
//! textured ones only where the texel's bit 15 is set; modulation is texel * colour / 16 per
//! channel (0x80 = unchanged), clamped. Pixel format: PS1 15-bit BGR plus the mask bit.

/// VRAM width in pixels.
pub const VRAM_W: usize = 1024;
/// VRAM height in pixels.
pub const VRAM_H: usize = 512;

const W_MASK: i32 = VRAM_W as i32 - 1;
const H_MASK: i32 = VRAM_H as i32 - 1;

/// Drawing state (E1-E6).
#[derive(Clone, Copy, Default)]
struct State {
    /// Draw area, inclusive.
    clip_x0: i32,
    clip_y0: i32,
    clip_x1: i32,
    clip_y1: i32,
    ofs_x: i32,
    ofs_y: i32,
    /// E1 bits 0-8 and 11.
    tpage: u32,
    dither: bool,
    flip_x: bool,
    flip_y: bool,
    tw_mask_x: i32,
    tw_mask_y: i32,
    tw_off_x: i32,
    tw_off_y: i32,
    set_mask: bool,
    check_mask: bool,
}

#[derive(Clone, Copy, Default)]
struct Display {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rgb24: bool,
    on: bool,
}

/// One polygon or line vertex in VRAM coordinates (drawing offset applied), 8 bits per colour
/// channel. (The C also carries PGXP positions here; not ported.)
#[derive(Clone, Copy, Default)]
struct Vtx {
    x: i32,
    y: i32,
    r: i32,
    g: i32,
    b: i32,
    u: i32,
    v: i32,
}

/// Texture of a primitive: mode 0 = 4-bit, 1 = 8-bit, 2 = 15-bit; page tx/ty, CLUT cx/cy.
#[derive(Clone, Copy, Default)]
struct Tex {
    mode: i32,
    tx: i32,
    ty: i32,
    cx: i32,
    cy: i32,
    raw: bool,
}

/// PS1 dither offsets by (y & 3, x & 3), added to the 8-bit colour before the cut to 5 bits.
const DITHER_TBL: [[i32; 4]; 4] = [
    [-4, 0, -3, 1],
    [2, -2, 3, -1],
    [-3, 1, -4, 0],
    [3, -1, 2, -2],
];

fn sign11(v: u32) -> i32 {
    (v << 21) as i32 >> 21
}

fn to5(c8: i32, d: i32) -> i32 {
    (c8 + d).clamp(0, 255) >> 3
}

/// The emulated GPU. The C keeps this in file-level statics; here it is a value.
pub struct Gpu {
    vram: Vec<u16>,
    st: State,
    disp: Display,
    /// Commands the C logs once ("logged once" lines): (opcode, message). Each opcode appears
    /// at most once.
    pub log: Vec<(u8, &'static str)>,
    seen: [bool; 256],
}

impl Default for Gpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Gpu {
    /// A GPU with zeroed VRAM, display off and the draw area at the whole VRAM.
    pub fn new() -> Self {
        let mut g = Gpu {
            vram: vec![0; VRAM_W * VRAM_H],
            st: State::default(),
            disp: Display::default(),
            log: Vec::new(),
            seen: [false; 256],
        };
        g.reset(true);
        g
    }

    /// The VRAM, row-major, `VRAM_W` pixels per row.
    pub fn vram(&self) -> &[u16] {
        &self.vram
    }

    /// The VRAM for direct writes (host setup; the C exposes `PsxGpu_Vram`).
    pub fn vram_mut(&mut self) -> &mut [u16] {
        &mut self.vram
    }

    fn log_once(&mut self, op: u32, what: &'static str) {
        let op = (op & 0xFF) as u8;
        if !self.seen[op as usize] {
            self.seen[op as usize] = true;
            self.log.push((op, what));
        }
    }

    /// GP1(0)-like reset of the drawing state; the display is switched off when `display` is
    /// set. VRAM and the display area are left as they are.
    pub fn reset(&mut self, display: bool) {
        self.st = State {
            clip_x1: VRAM_W as i32 - 1,
            clip_y1: VRAM_H as i32 - 1,
            ..State::default()
        };
        if display {
            self.disp.on = false;
        }
    }

    // ---- pixels ----

    fn idx(x: i32, y: i32) -> usize {
        (y as usize) * VRAM_W + x as usize
    }

    /// Writes one pixel (r, g, b in 0..31, or out of range when semi-transparent blending
    /// follows). `mask`: the texel's bit 15.
    #[allow(clippy::too_many_arguments)]
    fn put(
        &mut self,
        x: i32,
        y: i32,
        mut r: i32,
        mut g: i32,
        mut b: i32,
        semi: bool,
        abr: i32,
        mask: bool,
    ) {
        let d = &mut self.vram[Self::idx(x, y)];
        if self.st.check_mask && *d & 0x8000 != 0 {
            return;
        }
        if semi {
            let br = (*d & 31) as i32;
            let bg = ((*d >> 5) & 31) as i32;
            let bb = ((*d >> 10) & 31) as i32;
            match abr {
                0 => {
                    r = (br + r) >> 1;
                    g = (bg + g) >> 1;
                    b = (bb + b) >> 1;
                }
                1 => {
                    r += br;
                    g += bg;
                    b += bb;
                }
                2 => {
                    r = br - r;
                    g = bg - g;
                    b = bb - b;
                }
                _ => {
                    r = br + (r >> 2);
                    g = bg + (g >> 2);
                    b = bb + (b >> 2);
                }
            }
            r = r.clamp(0, 31);
            g = g.clamp(0, 31);
            b = b.clamp(0, 31);
        }
        let m = if mask || self.st.set_mask { 0x8000 } else { 0 };
        *d = (r | (g << 5) | (b << 10) | m) as u16;
    }

    fn texel(&self, t: &Tex, u: i32, v: i32) -> u16 {
        let st = &self.st;
        let u = ((u & !st.tw_mask_x) | (st.tw_off_x & st.tw_mask_x)) & 0xFF;
        let v = ((v & !st.tw_mask_y) | (st.tw_off_y & st.tw_mask_y)) & 0xFF;
        let y = (t.ty + v) & H_MASK;
        let at = |x: i32, y: i32| self.vram[Self::idx(x & W_MASK, y)];
        match t.mode {
            0 => {
                let w = at(t.tx + (u >> 2), y) as i32;
                at(t.cx + ((w >> ((u & 3) * 4)) & 0xF), t.cy)
            }
            1 => {
                let w = at(t.tx + (u >> 1), y) as i32;
                at(t.cx + ((w >> ((u & 1) * 8)) & 0xFF), t.cy)
            }
            _ => at(t.tx + u, y),
        }
    }

    /// One pixel of a primitive: colour r, g, b in 8 bits per channel (vertex colour). `dith`:
    /// the primitive is dithered (E1 bit 9 on a Gouraud or texture-modulated polygon; never
    /// rectangles or raw textures).
    #[allow(clippy::too_many_arguments)]
    fn shade(
        &mut self,
        x: i32,
        y: i32,
        t: Option<&Tex>,
        u: i32,
        v: i32,
        r: i32,
        g: i32,
        b: i32,
        semi: bool,
        abr: i32,
        dith: bool,
    ) {
        let d = if dith {
            DITHER_TBL[(y & 3) as usize][(x & 3) as usize]
        } else {
            0
        };
        if let Some(t) = t {
            let c = self.texel(t, u, v) as i32;
            if c == 0 {
                return;
            }
            let mut tr = c & 31;
            let mut tg = (c >> 5) & 31;
            let mut tb = (c >> 10) & 31;
            if !t.raw {
                // texel (5 bit) * colour (8 bit) / 16 = 8-bit result, 0x80 = unchanged
                tr = to5((tr * r) >> 4, d);
                tg = to5((tg * g) >> 4, d);
                tb = to5((tb * b) >> 4, d);
            }
            self.put(
                x,
                y,
                tr,
                tg,
                tb,
                semi && c & 0x8000 != 0,
                abr,
                c & 0x8000 != 0,
            );
        } else {
            self.put(x, y, to5(r, d), to5(g, d), to5(b, d), semi, abr, false);
        }
    }

    // ---- primitives ----

    #[allow(clippy::too_many_arguments)]
    fn triangle(
        &mut self,
        v0: &Vtx,
        v1: &Vtx,
        v2: &Vtx,
        gouraud: bool,
        t: Option<&Tex>,
        semi: bool,
        abr: i32,
    ) {
        fn edge(a: &Vtx, b: &Vtx, px: i32, py: i32) -> i64 {
            (b.x - a.x) as i64 * (py - a.y) as i64 - (b.y - a.y) as i64 * (px - a.x) as i64
        }
        // Top or left edge (included on ties) for a triangle with positive area under edge().
        fn top_left(a: &Vtx, b: &Vtx) -> bool {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            dy < 0 || (dy == 0 && dx > 0)
        }
        let dith = self.st.dither && (gouraud || t.is_some_and(|t| !t.raw));
        let mut minx = v0.x.min(v1.x).min(v2.x);
        let maxx = v0.x.max(v1.x).max(v2.x);
        let mut miny = v0.y.min(v1.y).min(v2.y);
        let maxy = v0.y.max(v1.y).max(v2.y);
        if maxx - minx >= 1024 || maxy - miny >= 512 {
            return;
        }
        let (mut v1, mut v2) = (v1, v2);
        let mut area = edge(v0, v1, v2.x, v2.y);
        if area == 0 {
            return;
        }
        if area < 0 {
            std::mem::swap(&mut v1, &mut v2);
            area = -area;
        }
        let st = self.st;
        minx = minx.max(st.clip_x0);
        miny = miny.max(st.clip_y0);
        let maxx = maxx.min(st.clip_x1);
        let maxy = maxy.min(st.clip_y1);
        let tl0 = top_left(v1, v2);
        let tl1 = top_left(v2, v0);
        let tl2 = top_left(v0, v1);
        for y in miny..=maxy {
            for x in minx..=maxx {
                let w0 = edge(v1, v2, x, y);
                let w1 = edge(v2, v0, x, y);
                let w2 = edge(v0, v1, x, y);
                if w0 < 0 || w1 < 0 || w2 < 0 {
                    continue;
                }
                if (w0 == 0 && !tl0) || (w1 == 0 && !tl1) || (w2 == 0 && !tl2) {
                    continue;
                }
                let (mut r, mut g, mut b, mut u, mut v) = (v0.r, v0.g, v0.b, 0, 0);
                let mix = |a: i32, b: i32, c: i32| {
                    ((a as i64 * w0 + b as i64 * w1 + c as i64 * w2) / area) as i32
                };
                if gouraud {
                    r = mix(v0.r, v1.r, v2.r);
                    g = mix(v0.g, v1.g, v2.g);
                    b = mix(v0.b, v1.b, v2.b);
                }
                if t.is_some() {
                    u = mix(v0.u, v1.u, v2.u);
                    v = mix(v0.v, v1.v, v2.v);
                }
                self.shade(x, y, t, u, v, r, g, b, semi, abr, dith);
            }
        }
    }

    fn make_tex(tpage: u32, clut: u32, raw: bool) -> Tex {
        let mut mode = ((tpage >> 7) & 3) as i32;
        if mode == 3 {
            mode = 2;
        }
        Tex {
            mode,
            tx: (tpage & 0xF) as i32 * 64,
            ty: ((tpage >> 4) & 1) as i32 * 256,
            cx: (clut & 0x3F) as i32 * 16,
            cy: ((clut >> 6) & 0x1FF) as i32,
            raw,
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn polygon(&mut self, w: &[u32], op: u32) {
        let quad = op & 0x08 != 0;
        let textured = op & 0x04 != 0;
        let semi = op & 0x02 != 0;
        let gouraud = op & 0x10 != 0;
        let nv = if quad { 4 } else { 3 };
        let mut v = [Vtx::default(); 4];
        let mut clut = 0u32;
        let mut tpage = self.st.tpage;
        let mut col = w[0];
        let mut i = 1;
        for k in 0..nv {
            if gouraud && k > 0 {
                col = w[i];
                i += 1;
            }
            v[k].r = (col & 0xFF) as i32;
            v[k].g = ((col >> 8) & 0xFF) as i32;
            v[k].b = ((col >> 16) & 0xFF) as i32;
            v[k].x = sign11(w[i] & 0xFFFF) + self.st.ofs_x;
            v[k].y = sign11(w[i] >> 16) + self.st.ofs_y;
            i += 1;
            if textured {
                v[k].u = (w[i] & 0xFF) as i32;
                v[k].v = ((w[i] >> 8) & 0xFF) as i32;
                if k == 0 {
                    clut = w[i] >> 16;
                } else if k == 1 {
                    tpage = w[i] >> 16;
                }
                i += 1;
            }
        }
        let mut tex = Tex::default();
        if textured {
            // A textured polygon's page becomes the current draw mode page (GPUSTAT bits 0-8, 11).
            self.st.tpage = (self.st.tpage & !0x9FF) | (tpage & 0x9FF);
            tex = Self::make_tex(tpage, clut, op & 0x01 != 0);
        }
        let t = textured.then_some(&tex);
        let abr = ((tpage >> 5) & 3) as i32;
        self.triangle(&v[0], &v[1], &v[2], gouraud, t, semi, abr);
        if quad {
            self.triangle(&v[1], &v[2], &v[3], gouraud, t, semi, abr);
        }
        // The C then draws the same triangles again on the HD surfaces (PGXP lookup,
        // PsxHd_Triangle) when HD is on. Not ported.
    }

    fn rectangle(&mut self, w: &[u32], op: u32) {
        const SIZES: [i32; 4] = [0, 1, 8, 16];
        let textured = op & 0x04 != 0;
        let semi = op & 0x02 != 0;
        let size = SIZES[((op >> 3) & 3) as usize];
        let r = (w[0] & 0xFF) as i32;
        let g = ((w[0] >> 8) & 0xFF) as i32;
        let b = ((w[0] >> 16) & 0xFF) as i32;
        let x0 = sign11(w[1] & 0xFFFF) + self.st.ofs_x;
        let y0 = sign11(w[1] >> 16) + self.st.ofs_y;
        let (mut u0, mut v0, mut i) = (0, 0, 2);
        let mut tex = Tex::default();
        if textured {
            u0 = (w[2] & 0xFF) as i32;
            v0 = ((w[2] >> 8) & 0xFF) as i32;
            tex = Self::make_tex(self.st.tpage, w[2] >> 16, op & 0x01 != 0);
            i = 3;
        }
        let (rw, rh) = if size == 0 {
            ((w[i] & 0x3FF) as i32, ((w[i] >> 16) & 0x1FF) as i32)
        } else {
            (size, size)
        };
        let st = self.st;
        let abr = ((st.tpage >> 5) & 3) as i32;
        let t = textured.then_some(&tex);
        for y in 0..rh {
            let py = y0 + y;
            if py < st.clip_y0 || py > st.clip_y1 {
                continue;
            }
            for x in 0..rw {
                let px = x0 + x;
                if px < st.clip_x0 || px > st.clip_x1 {
                    continue;
                }
                // E1 bits 12 / 13: texture x / y flip for rectangles
                let u = if st.flip_x { u0 - x } else { u0 + x };
                let v = if st.flip_y { v0 - y } else { v0 + y };
                self.shade(px, py, t, u, v, r, g, b, semi, abr, false);
            }
        }
        // HD: PsxHd_Rect on the HD surfaces. Not ported.
    }

    /// Line from a to b (psx-spx "Line Drawing"): both end points drawn, colour interpolated
    /// along the longer axis, dithered when Gouraud (E1 bit 9), clipped to the draw area per
    /// pixel. A segment of 1024 or more columns or 512 or more rows is not drawn.
    fn line_seg(&mut self, a: &Vtx, b: &Vtx, gouraud: bool, semi: bool, abr: i32) {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let adx = dx.abs();
        let ady = dy.abs();
        let n = adx.max(ady);
        let dith = self.st.dither && gouraud;
        if adx >= 1024 || ady >= 512 {
            return;
        }
        let st = self.st;
        for i in 0..=n {
            // 16.16 fixed point, rounded to the nearest pixel
            let (x, y) = if n == 0 {
                (a.x, a.y)
            } else {
                let step = |d: i32| ((d as i64 * i as i64 * 65536 / n as i64 + 32768) >> 16) as i32;
                (a.x + step(dx), a.y + step(dy))
            };
            if x < st.clip_x0 || x > st.clip_x1 || y < st.clip_y0 || y > st.clip_y1 {
                continue;
            }
            let (mut r, mut g, mut bl) = (a.r, a.g, a.b);
            if gouraud && n != 0 {
                r = a.r + (b.r - a.r) * i / n;
                g = a.g + (b.g - a.g) * i / n;
                bl = a.b + (b.b - a.b) * i / n;
            }
            let d = if dith {
                DITHER_TBL[(y & 3) as usize][(x & 3) as usize]
            } else {
                0
            };
            self.put(x, y, to5(r, d), to5(g, d), to5(bl, d), semi, abr, false);
        }
    }

    /// GP0 40-5F: single lines (3 words, Gouraud 4) and polylines (vertices up to the
    /// 0x5xxx5xxx terminator). Lines are never textured.
    fn line(&mut self, w: &[u32], op: u32) {
        let gouraud = op & 0x10 != 0;
        let polyline = op & 0x08 != 0;
        let semi = op & 0x02 != 0;
        let abr = ((self.st.tpage >> 5) & 3) as i32;
        let is_end = |x: u32| x & 0xF000_F000 == 0x5000_5000;
        let mut col = w[0];
        let mut v = [Vtx::default(); 2];
        let (mut i, mut k) = (1usize, 0usize);
        loop {
            if k > 0 && gouraud {
                // The C reads past the packet on a malformed polyline; stop instead.
                let Some(&x) = w.get(i) else { return };
                if is_end(x) && polyline {
                    break;
                }
                col = x;
                i += 1;
            }
            let Some(&x) = w.get(i) else { return };
            if k > 0 && polyline && is_end(x) {
                break;
            }
            let p = &mut v[k & 1];
            p.r = (col & 0xFF) as i32;
            p.g = ((col >> 8) & 0xFF) as i32;
            p.b = ((col >> 16) & 0xFF) as i32;
            p.x = sign11(x & 0xFFFF) + self.st.ofs_x;
            p.y = sign11(x >> 16) + self.st.ofs_y;
            // (C: PGXP source lookup when smooth frames are on; not ported.)
            i += 1;
            if k > 0 {
                let (a, b) = (v[(k - 1) & 1], v[k & 1]);
                self.line_seg(&a, &b, gouraud, semi, abr);
                // HD: PsxHd_Line. Not ported.
            }
            k += 1;
            if !polyline && k == 2 {
                break;
            }
        }
    }

    // ---- transfers (not clipped by the draw area, coordinates wrap) ----

    /// GP0 02 / libgpu ClearImage: fills a rect with a 24-bit colour (cut to 15 bits). Ignores
    /// the mask bit settings.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, rgb24: u32) {
        let c = (((rgb24 >> 3) & 31) | (((rgb24 >> 11) & 31) << 5) | (((rgb24 >> 19) & 31) << 10))
            as u16;
        for j in 0..h {
            let yy = y.wrapping_add(j) & H_MASK;
            for i in 0..w {
                self.vram[Self::idx(x.wrapping_add(i) & W_MASK, yy)] = c;
            }
        }
        // HD: PsxHd_Sync before, PsxHd_Fill after. Not ported.
    }

    fn load_iter(&mut self, x: i32, y: i32, w: i32, h: i32, mut src: impl Iterator<Item = u16>) {
        for j in 0..h {
            let yy = y.wrapping_add(j) & H_MASK;
            for i in 0..w {
                // The C reads `w * h` pixels from `src`; a short source ends the copy here.
                let Some(p) = src.next() else { return };
                self.vram[Self::idx(x.wrapping_add(i) & W_MASK, yy)] = p;
            }
        }
    }

    /// CPU to VRAM: `w * h` pixels row by row from `src`. Ignores the mask bit settings.
    pub fn load_image(&mut self, x: i32, y: i32, w: i32, h: i32, src: &[u16]) {
        self.load_iter(x, y, w, h, src.iter().copied());
    }

    /// VRAM to CPU: `w * h` pixels into `dst` (a short `dst` ends the copy there).
    pub fn store_image(&self, x: i32, y: i32, w: i32, h: i32, dst: &mut [u16]) {
        let mut k = 0;
        for j in 0..h {
            let yy = y.wrapping_add(j) & H_MASK;
            for i in 0..w {
                let Some(d) = dst.get_mut(k) else { return };
                *d = self.vram[Self::idx(x.wrapping_add(i) & W_MASK, yy)];
                k += 1;
            }
        }
    }

    /// VRAM to VRAM copy. Ignores the mask bit settings.
    pub fn move_image(&mut self, sx: i32, sy: i32, dx: i32, dy: i32, w: i32, h: i32) {
        let w = w.min(VRAM_W as i32);
        let mut line = [0u16; VRAM_W];
        // Row order chosen so an overlapping copy reads each row before it is overwritten.
        for j in 0..h {
            let jj = if dy > sy { h - 1 - j } else { j };
            let s = (sy.wrapping_add(jj) & H_MASK) as usize;
            let d = (dy.wrapping_add(jj) & H_MASK) as usize;
            for i in 0..w {
                line[i as usize] = self.vram[Self::idx(sx.wrapping_add(i) & W_MASK, s as i32)];
            }
            for i in 0..w {
                self.vram[Self::idx(dx.wrapping_add(i) & W_MASK, d as i32)] = line[i as usize];
            }
        }
        // HD: PsxHd_Sync before, PsxHd_Refresh after. Not ported.
    }

    // ---- GP0 ----

    /// Words of the command starting at `w[0]` (`w.len()` available), or 0 when it is cut off.
    fn command_len(w: &[u32]) -> usize {
        let n = w.len();
        let op = w[0] >> 24;
        match op {
            0x02 => 3,
            0x20..=0x3F => {
                let nv = if op & 0x08 != 0 { 4 } else { 3 };
                1 + nv * if op & 0x04 != 0 { 2 } else { 1 }
                    + if op & 0x10 != 0 { nv - 1 } else { 0 }
            }
            0x40..=0x5F => {
                if op & 0x08 == 0 {
                    return if op & 0x10 != 0 { 4 } else { 3 };
                }
                // polyline: up to the 0x5xxx5xxx terminator
                (3..n)
                    .find(|&i| w[i] & 0xF000_F000 == 0x5000_5000)
                    .map_or(0, |i| i + 1)
            }
            0x60..=0x7F => {
                2 + if op & 0x04 != 0 { 1 } else { 0 } + if (op >> 3) & 3 == 0 { 1 } else { 0 }
            }
            0x80..=0x9F => 4,
            0xA0..=0xBF => {
                if n < 3 {
                    return 0;
                }
                let (cw, ch) = Self::xfer_size(w[2]);
                3 + ((cw * ch + 1) / 2) as usize
            }
            0xC0..=0xDF => 3,
            _ => 1,
        }
    }

    /// Width and height of a transfer from its size word: 0 means the maximum.
    fn xfer_size(w: u32) -> (i32, i32) {
        (
            ((((w & 0xFFFF) as i32) - 1) & 0x3FF) + 1,
            ((((w >> 16) as i32) - 1) & 0x1FF) + 1,
        )
    }

    fn command(&mut self, w: &[u32]) {
        let c = w[0];
        let op = c >> 24;
        match op {
            0x20..=0x3F => self.polygon(w, op),
            0x60..=0x7F => self.rectangle(w, op),
            0x40..=0x5F => self.line(w, op),
            0x80..=0x9F => {
                let (cw, ch) = Self::xfer_size(w[3]);
                self.move_image(
                    (w[1] & 0x3FF) as i32,
                    ((w[1] >> 16) & 0x1FF) as i32,
                    (w[2] & 0x3FF) as i32,
                    ((w[2] >> 16) & 0x1FF) as i32,
                    cw,
                    ch,
                );
            }
            0xA0..=0xBF => {
                let (cw, ch) = Self::xfer_size(w[2]);
                // The pixel data are the following words, low half first (little endian).
                let px = w[3..].iter().flat_map(|&x| [x as u16, (x >> 16) as u16]);
                self.load_iter(
                    (w[1] & 0x3FF) as i32,
                    ((w[1] >> 16) & 0x1FF) as i32,
                    cw,
                    ch,
                    px,
                );
            }
            0xC0..=0xDF => self.log_once(op, "VRAM to CPU in a packet, ignored"),
            0x00 | 0x01 => {}
            0x02 => self.fill(
                (w[1] & 0x3F0) as i32,
                ((w[1] >> 16) & 0x1FF) as i32,
                (((w[2] & 0x3FF) + 15) & !15) as i32,
                ((w[2] >> 16) & 0x1FF) as i32,
                c & 0xFF_FFFF,
            ),
            0xE1 => {
                self.st.tpage = c & 0x9FF;
                self.st.dither = (c >> 9) & 1 != 0;
                // (bit 10, draw to display area: stored in the C, never read)
                self.st.flip_x = (c >> 12) & 1 != 0;
                self.st.flip_y = (c >> 13) & 1 != 0;
            }
            0xE2 => {
                self.st.tw_mask_x = (c & 31) as i32 * 8;
                self.st.tw_mask_y = ((c >> 5) & 31) as i32 * 8;
                self.st.tw_off_x = ((c >> 10) & 31) as i32 * 8;
                self.st.tw_off_y = ((c >> 15) & 31) as i32 * 8;
            }
            0xE3 => {
                self.st.clip_x0 = (c & 0x3FF) as i32;
                self.st.clip_y0 = ((c >> 10) & 0x1FF) as i32;
            }
            0xE4 => {
                self.st.clip_x1 = (c & 0x3FF) as i32;
                self.st.clip_y1 = ((c >> 10) & 0x1FF) as i32;
            }
            0xE5 => {
                self.st.ofs_x = sign11(c & 0x7FF);
                self.st.ofs_y = sign11((c >> 11) & 0x7FF);
            }
            0xE6 => {
                self.st.set_mask = c & 1 != 0;
                self.st.check_mask = (c >> 1) & 1 != 0;
            }
            _ => self.log_once(op, "unhandled command, ignored"),
        }
    }

    /// Runs GP0 command words (one OT packet body, or a few words). Several commands may
    /// follow each other; a command cut off at the end drops the rest (logged).
    pub fn gp0(&mut self, words: &[u32]) {
        let mut i = 0;
        while i < words.len() {
            let len = Self::command_len(&words[i..]);
            if len == 0 || i + len > words.len() {
                self.log_once(words[i] >> 24, "command cut off at the packet end, dropped");
                return;
            }
            self.command(&words[i..i + len]);
            i += len;
        }
    }

    // ---- display ----

    /// Display area (GP1 05 / 06 / 07 / 08 / 03 in one call): top-left in VRAM, size, 24-bit.
    pub fn set_display(&mut self, x: i32, y: i32, w: i32, h: i32, rgb24: bool) {
        // (C: PsxHd_Register for the HD surfaces when not 24-bit. Not ported.)
        self.disp = Display {
            x,
            y,
            w,
            h,
            rgb24,
            on: self.disp.on,
        };
    }

    /// Display on / off (GP1 03).
    pub fn set_display_enable(&mut self, on: bool) {
        self.disp.on = on;
    }

    /// Size of the current draw area (E3 / E4); the C uses it for the PGXP key.
    pub fn draw_area_size(&self) -> (i32, i32) {
        (
            self.st.clip_x1 - self.st.clip_x0 + 1,
            self.st.clip_y1 - self.st.clip_y0 + 1,
        )
    }

    /// The visible picture: `w * h` pixels of 0x00RRGGBB written row by row to `out`, which
    /// must hold at least that many (640 x 480 is always enough). Returns `(w, h)`, or `None`
    /// when the display is off (`out` untouched) or `out` is too small.
    pub fn read_display(&self, out: &mut [u32]) -> Option<(usize, usize)> {
        let d = &self.disp;
        let w = d.w.clamp(1, 640);
        let h = d.h.clamp(1, 480); // NTSC: at most 480 visible lines
        if !d.on || out.len() < (w * h) as usize {
            return None;
        }
        for y in 0..h {
            let ry = (d.y.wrapping_add(y) & H_MASK) as usize;
            let row = &self.vram[ry * VRAM_W..(ry + 1) * VRAM_W];
            let o = &mut out[(y * w) as usize..((y + 1) * w) as usize];
            for (x, px) in o.iter_mut().enumerate() {
                let x = x as i32;
                if d.rgb24 {
                    // Bytes of the row, little-endian halves; the row wraps at 2048 bytes.
                    let byte = |i: i32| {
                        let i = (i & 2047) as usize;
                        let hw = row[i >> 1];
                        (if i & 1 == 0 { hw } else { hw >> 8 } & 0xFF) as u32
                    };
                    // Masked first, so the byte offsets can't overflow for any display x.
                    let i = d.x.wrapping_mul(2).wrapping_add(x * 3) & 2047;
                    *px = (byte(i) << 16) | (byte(i + 1) << 8) | byte(i + 2);
                } else {
                    let c = row[((d.x.wrapping_add(x)) & W_MASK) as usize] as u32;
                    let e = |v: u32| (v << 3) | (v >> 2);
                    *px = (e(c & 31) << 16) | (e((c >> 5) & 31) << 8) | e((c >> 10) & 31);
                }
            }
        }
        Some((w as usize, h as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A display x far outside VRAM must wrap in 24-bit mode, not overflow (found in review;
    /// the original C overflows here).
    #[test]
    fn read_display_24bit_extreme_x_wraps() {
        let mut gpu = Gpu::new();
        gpu.set_display(0x3FFF_FFFF, 0, 1, 1, true);
        gpu.set_display_enable(true);
        let mut out = vec![0u32; 640 * 480];
        assert!(gpu.read_display(&mut out).is_some());
    }
}
