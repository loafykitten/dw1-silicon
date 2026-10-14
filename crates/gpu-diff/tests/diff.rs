//! Differential tests: `backend::gpu::Gpu` against DW2-PC's `backend/psxgpu.c` (compiled by
//! build.rs from the gitignored `ref/DW2-PC`) on pseudo-random GP0 streams. After each stream
//! all of VRAM, every VRAM read-back and the display read-out must match bit for bit. A
//! mismatch is shrunk to a minimal op list and printed.
//!
//! The model no longer follows the C everywhere: it was fitted to PCSX-Redux's software
//! renderer (`tests/gpu_oracle.rs` in `crates/backend`), which rounds interpolated colours and
//! texture coordinates instead of truncating them, steps them from the left edge of each row,
//! draws a quad's second triangle first, and dithers only Gouraud polygons (and, when it is
//! on, ignores the raw flag of a textured one). So the polygons generated here give every
//! vertex the same colour and the same texture coordinate (nothing to interpolate, the two
//! agree), a quad is sent as its two triangles in the model's order (they can read each other's
//! pixels as texture), and the dither bit of E1 stays off. A modulated textured polygon is never semi-transparent (Redux blends
//! its pixels on 8 bits at the end of an odd row, and quarters the texel in mode B + F/4). Coverage, blending, masking, texture lookup,
//! rectangles, lines and transfers are still compared bit for bit.
//!
//! Case count per test: env `GPU_DIFF_CASES` (default 2500).

/// Shows up as ignored (not passed) in a checkout without `ref/DW2-PC`.
#[cfg(not(have_dw2_c))]
#[test]
#[ignore = "ref/DW2-PC/backend/psxgpu.c not found: GPU differential tests not run"]
fn skipped_without_dw2_pc() {}

#[cfg(have_dw2_c)]
mod diff {
    use backend::gpu::Gpu;
    use gpu_diff::CGpu;
    use std::collections::BTreeMap;
    use std::fmt::Write as _;

    const VW: usize = 1024;

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11
        }
        fn u32(&mut self) -> u32 {
            (self.next() >> 8) as u32
        }
        fn below(&mut self, n: u32) -> u32 {
            (self.next() % n as u64) as u32
        }
        /// Inclusive range.
        fn range(&mut self, lo: i32, hi: i32) -> i32 {
            lo + self.below((hi - lo + 1) as u32) as i32
        }
        /// True `pct` times in 100.
        fn pct(&mut self, pct: u32) -> bool {
            self.below(100) < pct
        }
    }

    #[derive(Clone)]
    enum Op {
        Gp0(Vec<u32>),
        Fill(i32, i32, i32, i32, u32),
        Load(i32, i32, i32, i32, Vec<u16>),
        Move(i32, i32, i32, i32, i32, i32),
        Store(i32, i32, i32, i32),
    }

    impl Op {
        fn show(&self) -> String {
            match self {
                Op::Gp0(w) => {
                    let mut s = String::from("gp0[");
                    for (i, x) in w.iter().enumerate() {
                        let _ = write!(s, "{}{x:08X}", if i > 0 { " " } else { "" });
                        if i > 24 {
                            let _ = write!(s, " ... {} words", w.len());
                            break;
                        }
                    }
                    s + "]"
                }
                Op::Fill(a, b, c, d, e) => format!("fill({a},{b},{c},{d},{e:06X})"),
                Op::Load(a, b, c, d, p) => format!(
                    "load_image({a},{b},{c},{d}, {} pixels, first {:04X?})",
                    p.len(),
                    &p[..p.len().min(4)]
                ),
                Op::Move(a, b, c, d, e, f) => format!("move_image({a},{b},{c},{d},{e},{f})"),
                Op::Store(a, b, c, d) => format!("store_image({a},{b},{c},{d})"),
            }
        }
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Mix {
        General,
        /// Semi-transparency and mask bit heavy.
        Blend,
        /// Textured primitives, texture windows and uploads heavy.
        Textured,
    }

    struct Case {
        init_seed: u64,
        ops: Vec<Op>,
        display: [Disp; 3],
    }

    #[derive(Clone, Copy)]
    struct Disp {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        rgb24: bool,
        on: bool,
    }

    // ---- generation ----

    struct Gen {
        r: Rng,
        mix: Mix,
        /// Current draw area and offset as emitted (for placing vertices).
        area: (i32, i32, i32, i32),
        ofs: (i32, i32),
        counts: BTreeMap<String, usize>,
    }

    impl Gen {
        fn count(&mut self, k: &str) {
            *self.counts.entry(k.to_string()).or_default() += 1;
        }

        fn junk(&mut self, bits: u32, shift: u32) -> u32 {
            if self.r.pct(50) {
                (self.r.u32() & ((1 << bits) - 1)) << shift
            } else {
                0
            }
        }

        /// A vertex word for screen position (sx, sy): the drawing offset is taken off, the 11
        /// bit fields get random junk above them half the time.
        fn vword(&mut self, sx: i32, sy: i32) -> u32 {
            let x = (sx - self.ofs.0).clamp(-1024, 1023);
            let y = (sy - self.ofs.1).clamp(-1024, 1023);
            loop {
                let w = ((x as u32) & 0x7FF)
                    | self.junk(5, 11)
                    | (((y as u32) & 0x7FF) << 16)
                    | self.junk(5, 27);
                if w & 0xF000_F000 != 0x5000_5000 {
                    return w;
                }
            }
        }

        /// A point: mostly in the draw area, sometimes anywhere.
        fn point(&mut self) -> (i32, i32) {
            let (x0, y0, x1, y1) = self.area;
            if self.r.pct(88) {
                let (x0, x1) = (x0.min(x1), x0.max(x1));
                let (y0, y1) = (y0.min(y1), y0.max(y1));
                (self.r.range(x0 - 8, x1 + 8), self.r.range(y0 - 8, y1 + 8))
            } else {
                (self.r.range(-1100, 1100), self.r.range(-1100, 1100))
            }
        }

        fn spread(&mut self) -> i32 {
            match self.r.below(100) {
                0..=39 => self.r.range(1, 8),
                40..=79 => self.r.range(8, 40),
                80..=95 => self.r.range(40, 120),
                _ => self.r.range(120, 1200),
            }
        }

        fn near(&mut self, c: (i32, i32), s: i32) -> (i32, i32) {
            (c.0 + self.r.range(-s, s), c.1 + self.r.range(-s, s))
        }

        fn colour(&mut self) -> u32 {
            match self.r.below(10) {
                0..=2 => 0x80_8080,
                3 => 0xFF_FFFF,
                4 => 0,
                _ => self.r.u32() & 0xFF_FFFF,
            }
        }

        fn pick(&mut self, base: u32, pct_semi: u32) -> u32 {
            if self.r.pct(pct_semi) {
                base | 2
            } else {
                base & !2
            }
        }

        fn polygon(&mut self) -> Vec<u32> {
            let (tex_pct, semi_pct) = match self.mix {
                Mix::General => (55, 40),
                Mix::Blend => (45, 85),
                Mix::Textured => (92, 40),
            };
            let mut op = self.r.below(32) | 0x20;
            op = if self.r.pct(tex_pct) { op | 4 } else { op & !4 };
            op = self.pick(op, semi_pct);
            let quad = op & 8 != 0;
            let tex = op & 4 != 0;
            // A modulated textured polygon isn't semi-transparent here: the model blends its
            // pixels differently from the C (see the header).
            if tex && op & 1 == 0 {
                op &= !2;
            }
            let gouraud = op & 0x10 != 0;
            let nv = if quad { 4 } else { 3 };
            let c = self.point();
            let s = self.spread();
            let mut pts = [(0, 0); 4];
            let axis = quad && self.r.pct(40);
            if axis {
                let (a, b) = (self.near(c, s), self.near(c, s));
                pts = [(a.0, a.1), (b.0, a.1), (a.0, b.1), (b.0, b.1)];
            } else {
                for p in pts.iter_mut().take(nv) {
                    *p = (c.0 + self.r.range(-s, s), c.1 + self.r.range(-s, s));
                }
                if self.r.pct(5) {
                    pts[1] = pts[0]; // degenerate
                }
            }
            // One texture coordinate and one colour for every vertex (see the header).
            let mut uvs = [(0i32, 0i32); 4];
            if tex {
                uvs = [(self.r.range(0, 255), self.r.range(0, 255)); 4];
            }
            let clut = self.r.u32() & 0xFFFF;
            let tpage = self.r.u32() & 0xFFFF;
            let col = self.colour();
            let mut vwords = [0u32; 4];
            let mut spare = [0u32; 4];
            for k in 0..nv {
                vwords[k] = self.vword(pts[k].0, pts[k].1);
                spare[k] = self.r.u32() & 0xFFFF;
            }
            // A triangle from vertices `ix`; the first carries the CLUT, the second the page.
            let emit = |op: u32, ix: [usize; 3]| {
                let mut w = Vec::new();
                for (j, &k) in ix.iter().enumerate() {
                    if j == 0 {
                        w.push((op << 24) | col);
                    } else if gouraud {
                        w.push(col);
                    }
                    w.push(vwords[k]);
                    if tex {
                        let hi = [clut, tpage, spare[k]][j];
                        w.push((hi << 16) | ((uvs[k].1 as u32) << 8) | uvs[k].0 as u32);
                    }
                }
                w
            };
            // A quad goes to both sides as the two triangles the model draws, in its order
            // (the second one first): where they overlap, a pixel can be read as texture by the
            // other, so the order shows. (A quad of the C is the other order; the oracle test
            // covers quads.)
            let w = if quad {
                let mut w = emit(op & !8, [1, 2, 3]);
                w.extend(emit(op & !8, [0, 1, 2]));
                w
            } else {
                emit(op, [0, 1, 2])
            };
            self.count(&format!(
                "poly {} {} {}",
                if quad { "quad" } else { "tri" },
                if gouraud { "gouraud" } else { "flat" },
                if tex {
                    if op & 1 != 0 { "tex-raw" } else { "tex-mod" }
                } else {
                    "untex"
                }
            ));
            if op & 2 != 0 {
                self.count("poly semi");
            }
            w
        }

        fn rect(&mut self) -> Vec<u32> {
            let (tex_pct, semi_pct) = match self.mix {
                Mix::General => (55, 40),
                Mix::Blend => (45, 85),
                Mix::Textured => (92, 40),
            };
            let mut op = self.r.below(32) | 0x60;
            op = if self.r.pct(tex_pct) { op | 4 } else { op & !4 };
            op = self.pick(op, semi_pct);
            let tex = op & 4 != 0;
            let size = (op >> 3) & 3;
            let p = self.point();
            let mut w = vec![(op << 24) | self.colour(), self.vword(p.0, p.1)];
            if tex {
                let clut = self.r.u32() & 0xFFFF;
                w.push((clut << 16) | (self.r.u32() & 0xFFFF));
            }
            if size == 0 {
                let (rw, rh) = if self.r.pct(92) {
                    (self.r.range(0, 60), self.r.range(0, 60))
                } else {
                    (self.r.range(0, 1023), self.r.range(0, 511))
                };
                w.push(((rh as u32) << 16) | rw as u32 | self.junk(6, 10));
            }
            self.count(&format!(
                "rect {} {}",
                ["var", "1x1", "8x8", "16x16"][size as usize],
                if tex { "tex" } else { "untex" }
            ));
            w
        }

        fn line(&mut self) -> Vec<u32> {
            let semi_pct = if self.mix == Mix::Blend { 85 } else { 40 };
            let mut op = self.r.below(32) | 0x40;
            op &= !4; // bit 2 is unused for lines
            op = self.pick(op, semi_pct);
            let gouraud = op & 0x10 != 0;
            let poly = op & 8 != 0;
            let nv = if poly { self.r.range(2, 6) } else { 2 };
            let mut c = self.point();
            let s = if self.r.pct(90) {
                self.r.range(1, 150)
            } else {
                self.r.range(150, 1300)
            };
            let mut w = Vec::new();
            for k in 0..nv {
                let col = self.colour();
                if k == 0 {
                    w.push((op << 24) | col);
                } else if gouraud {
                    w.push(col);
                }
                c = self.near(c, s);
                let vw = self.vword(c.0, c.1);
                w.push(vw);
            }
            if poly {
                w.push(0x5000_5000 | (self.r.u32() & 0x0FFF_0FFF));
            }
            self.count(&format!(
                "{} {}",
                if poly { "polyline" } else { "line" },
                if gouraud { "gouraud" } else { "flat" }
            ));
            if op & 2 != 0 {
                self.count("line semi");
            }
            w
        }

        fn state(&mut self) -> Vec<u32> {
            let which = match self.mix {
                Mix::Blend => [1, 1, 3, 4, 5, 6, 6, 6, 6, 2][self.r.below(10) as usize],
                Mix::Textured => [1, 1, 2, 2, 2, 3, 4, 5, 6, 2][self.r.below(10) as usize],
                Mix::General => self.r.range(1, 6) as u32,
            };
            self.state_of(which)
        }

        fn state_of(&mut self, which: u32) -> Vec<u32> {
            let w = match which {
                1 => {
                    // Bit 9 (dither) stays off: the model dithers differently from the C (see
                    // the header).
                    self.r.u32() & 0x3DFF
                }
                2 => {
                    if self.r.pct(25) {
                        0
                    } else {
                        self.r.u32() & 0xF_FFFF
                    }
                }
                3 | 4 => {
                    let (x, y) = if self.r.pct(70) {
                        let p = self.point();
                        (p.0.clamp(0, 1023), p.1.clamp(0, 511))
                    } else {
                        (self.r.range(0, 1023), self.r.range(0, 511))
                    };
                    (x as u32) | ((y as u32) << 10)
                }
                5 => {
                    let (x, y) = if self.r.pct(80) {
                        (self.r.range(-40, 40), self.r.range(-40, 40))
                    } else {
                        (self.r.range(-1024, 1023), self.r.range(-1024, 1023))
                    };
                    ((x as u32) & 0x7FF) | (((y as u32) & 0x7FF) << 11)
                }
                _ => self.r.u32() & 3,
            };
            self.count(&format!("E{which}"));
            let word = 0xE000_0000 | (which << 24) | (w & 0x00FF_FFFF);
            match which {
                3 => {
                    self.area.0 = (word & 0x3FF) as i32;
                    self.area.1 = ((word >> 10) & 0x1FF) as i32;
                }
                4 => {
                    self.area.2 = (word & 0x3FF) as i32;
                    self.area.3 = ((word >> 10) & 0x1FF) as i32;
                }
                5 => {
                    let s11 = |v: u32| ((v << 21) as i32) >> 21;
                    self.ofs = (s11(word & 0x7FF), s11((word >> 11) & 0x7FF));
                }
                _ => {}
            }
            vec![word]
        }

        fn rect_args(&mut self, fill: bool) -> (i32, i32, i32, i32) {
            let p = self.point();
            let (x, y) = if self.r.pct(70) {
                (p.0.rem_euclid(1024), p.1.rem_euclid(512))
            } else {
                (self.r.range(0, 1023), self.r.range(0, 511))
            };
            let (w, h) = match self.r.below(100) {
                0..=79 => (
                    self.r.range(if fill { 0 } else { 1 }, 64),
                    self.r.range(if fill { 0 } else { 1 }, 64),
                ),
                80..=97 => (self.r.range(1, 400), self.r.range(1, 200)),
                _ => (self.r.range(1, 1023), self.r.range(1, 511)),
            };
            (x, y, w, h)
        }

        fn pixels(&mut self, n: usize) -> Vec<u16> {
            let mode = self.r.below(4);
            (0..n)
                .map(|_| match mode {
                    0 => self.r.u32() as u16,
                    1 => (self.r.u32() as u16) & 0x7FFF, // no mask bits
                    2 => (self.r.u32() as u16) | 0x8000,
                    _ => {
                        if self.r.pct(30) {
                            0
                        } else {
                            self.r.u32() as u16
                        }
                    }
                })
                .collect()
        }

        fn op(&mut self) -> Op {
            let (w_state, w_poly, w_rect, w_line, w_xfer) = match self.mix {
                Mix::General => (13, 36, 16, 9, 26),
                Mix::Blend => (18, 30, 20, 12, 20),
                Mix::Textured => (16, 30, 22, 4, 28),
            };
            let total = w_state + w_poly + w_rect + w_line + w_xfer + 3;
            let mut n = self.r.below(total);
            if n < w_state {
                return Op::Gp0(self.state());
            }
            n -= w_state;
            if n < w_poly {
                return self.packet(|g| g.polygon());
            }
            n -= w_poly;
            if n < w_rect {
                return self.packet(|g| g.rect());
            }
            n -= w_rect;
            if n < w_line {
                return self.packet(|g| g.line());
            }
            n -= w_line;
            if n >= w_xfer {
                // commands the C ignores: nop, unhandled, and a cut-off one
                let op =
                    [0x00u32, 0x01, 0x03, 0x1F, 0xE0, 0xE7, 0xFF, 0x04][self.r.below(8) as usize];
                self.count("other/ignored");
                return Op::Gp0(vec![(op << 24) | (self.r.u32() & 0xFF_FFFF)]);
            }
            self.transfer()
        }

        /// One GP0 packet: usually a single command, sometimes several, sometimes cut off.
        fn packet(&mut self, mut f: impl FnMut(&mut Gen) -> Vec<u32>) -> Op {
            let mut w = f(self);
            if self.r.pct(12) {
                for _ in 0..self.r.range(1, 3) {
                    let more = match self.r.below(4) {
                        0 => self.state(),
                        1 => self.polygon(),
                        2 => self.rect(),
                        _ => self.line(),
                    };
                    w.extend(more);
                }
            }
            if self.r.pct(3) && w.len() > 1 {
                w.truncate(w.len() - 1);
                self.count("cut-off packet");
            }
            Op::Gp0(w)
        }

        fn transfer(&mut self) -> Op {
            match self.r.below(9) {
                0 => {
                    let (x, y, w, h) = self.rect_args(true);
                    let c = self.r.u32() & 0xFF_FFFF;
                    if self.r.pct(50) {
                        self.count("fill (gp0)");
                        let wf = (w as u32) & 0x3FF;
                        Op::Gp0(vec![
                            0x0200_0000 | c,
                            (x as u32 & 0x3F0) | (((y as u32) & 0x1FF) << 16) | self.junk(6, 10),
                            wf | ((h as u32) << 16),
                        ])
                    } else {
                        self.count("fill (api)");
                        Op::Fill(x, y, w, h, c | (self.r.u32() & 0xFF00_0000))
                    }
                }
                1 | 2 => {
                    let (x, y, mut w, mut h) = self.rect_args(false);
                    w = w.min(300);
                    h = h.min(300);
                    let px = self.pixels((w * h) as usize);
                    if self.r.pct(50) {
                        self.count("load (api)");
                        Op::Load(x, y, w, h, px)
                    } else {
                        self.count("load (gp0 A0)");
                        let mut v = vec![
                            0xA000_0000 | (self.r.u32() & 0xFF_FFFF),
                            (x as u32) | ((y as u32) << 16) | self.junk(6, 10),
                            (w as u32) | ((h as u32) << 16),
                        ];
                        for c in px.chunks(2) {
                            v.push(c[0] as u32 | ((*c.get(1).unwrap_or(&0) as u32) << 16));
                        }
                        Op::Gp0(v)
                    }
                }
                3 => {
                    let (x, y, w, h) = self.rect_args(false);
                    self.count("store (api)");
                    Op::Store(x, y, w, h)
                }
                4 => {
                    self.count("store (gp0 C0, ignored)");
                    Op::Gp0(vec![0xC000_0000, self.r.u32() & 0x01FF_03FF, 0x0004_0004])
                }
                _ => {
                    let (sx, sy, mut w, h) = self.rect_args(false);
                    let (dx, dy) = if self.r.pct(50) {
                        (sx + self.r.range(-20, 20), sy + self.r.range(-20, 20))
                    } else {
                        (self.r.range(0, 1023), self.r.range(0, 511))
                    };
                    if self.r.pct(5) {
                        w = 1024;
                    }
                    if self.r.pct(50) {
                        self.count("move (api)");
                        Op::Move(sx, sy, dx, dy, w, h)
                    } else {
                        self.count("move (gp0 80)");
                        let wv = if w >= 1024 { 0 } else { w as u32 };
                        let hv = if h >= 512 { 0 } else { h as u32 };
                        Op::Gp0(vec![
                            0x8000_0000,
                            (sx as u32 & 0x3FF) | (((sy as u32) & 0x1FF) << 16) | self.junk(6, 10),
                            (dx as u32 & 0x3FF) | (((dy as u32) & 0x1FF) << 16) | self.junk(6, 10),
                            wv | (hv << 16),
                        ])
                    }
                }
            }
        }

        fn disp(&mut self, rgb24: bool, on: bool) -> Disp {
            Disp {
                x: self.r.range(0, 1023),
                y: self.r.range(0, 511),
                w: [256, 320, 368, 512, 640, 700, 1, self.r.range(0, 800)]
                    [self.r.below(8) as usize],
                h: [240, 224, 480, 1, self.r.range(0, 600)][self.r.below(5) as usize],
                rgb24,
                on,
            }
        }

        fn case(&mut self, init_seed: u64) -> Case {
            self.area = (0, 0, 1023, 511);
            self.ofs = (0, 0);
            let mut ops = Vec::new();
            // Draw area and offset first, so most primitives land inside it.
            for e in [3u32, 4, 5] {
                if self.r.pct(85) {
                    let w = self.state_of(e);
                    ops.push(Op::Gp0(w));
                }
            }
            let n = self.r.range(6, 40);
            for _ in 0..n {
                let op = self.op();
                ops.push(op);
            }
            let d = [
                self.disp(false, true),
                self.disp(true, true),
                self.disp(false, false),
            ];
            Case {
                init_seed,
                ops,
                display: d,
            }
        }
    }

    // ---- execution ----

    fn init_vram(seed: u64, v: &mut [u16]) {
        let mut r = Rng(seed | 1);
        let mode = r.below(3);
        for p in v.iter_mut() {
            let x = r.u32() as u16;
            *p = match mode {
                0 => x,
                1 if x & 3 == 0 => 0,
                1 => x,
                _ => x & 0x7FFF,
            };
        }
    }

    fn first_diff(a: &[u16], b: &[u16]) -> Option<String> {
        let n = a.iter().zip(b).filter(|(x, y)| x != y).count();
        let i = a.iter().zip(b).position(|(x, y)| x != y)?;
        Some(format!(
            "{n} VRAM pixels differ, first at ({}, {}): rust {:04X}, c {:04X}",
            i % VW,
            i / VW,
            a[i],
            b[i]
        ))
    }

    /// Runs the case on both; `Err` describes the first difference.
    fn run(case: &Case, c: &mut CGpu, scratch: &mut Scratch) -> Result<(), String> {
        let mut g = Gpu::new();
        c.reset(true);
        init_vram(case.init_seed, g.vram_mut());
        c.vram_mut().copy_from_slice(g.vram());
        for (i, op) in case.ops.iter().enumerate() {
            match op {
                Op::Gp0(w) => {
                    g.gp0(w);
                    c.gp0(w);
                }
                Op::Fill(x, y, w, h, col) => {
                    g.fill(*x, *y, *w, *h, *col);
                    c.fill(*x, *y, *w, *h, *col);
                }
                Op::Load(x, y, w, h, p) => {
                    g.load_image(*x, *y, *w, *h, p);
                    c.load_image(*x, *y, *w, *h, p);
                }
                Op::Move(a, b, cc, d, e, f) => {
                    g.move_image(*a, *b, *cc, *d, *e, *f);
                    c.move_image(*a, *b, *cc, *d, *e, *f);
                }
                Op::Store(x, y, w, h) => {
                    let n = (*w * *h) as usize;
                    let (mut a, mut b) = (vec![0xBEEFu16; n], vec![0xBEEFu16; n]);
                    g.store_image(*x, *y, *w, *h, &mut a);
                    c.store_image(*x, *y, *w, *h, &mut b);
                    if a != b {
                        return Err(format!("op {i}: store_image differs"));
                    }
                }
            }
        }
        if let Some(d) = first_diff(g.vram(), c.vram()) {
            return Err(d);
        }
        for (k, d) in case.display.iter().enumerate() {
            g.set_display(d.x, d.y, d.w, d.h, d.rgb24);
            c.set_display(d.x, d.y, d.w, d.h, d.rgb24);
            g.set_display_enable(d.on);
            c.set_display_enable(d.on);
            scratch.a.fill(0xDEAD_BEEF);
            scratch.b.fill(0xDEAD_BEEF);
            let ra = g.read_display(&mut scratch.a);
            let rb = c.read_display(&mut scratch.b);
            if ra != rb {
                return Err(format!(
                    "display {k}: size/on differs, rust {ra:?}, c {rb:?}"
                ));
            }
            if scratch.a != scratch.b {
                let i = scratch
                    .a
                    .iter()
                    .zip(&scratch.b)
                    .position(|(x, y)| x != y)
                    .unwrap_or(0);
                return Err(format!(
                    "display {k} ({}): pixel {i} rust {:06X} c {:06X}",
                    if d.rgb24 { "24-bit" } else { "15-bit" },
                    scratch.a[i],
                    scratch.b[i]
                ));
            }
        }
        Ok(())
    }

    struct Scratch {
        a: Vec<u32>,
        b: Vec<u32>,
    }

    /// Removes ops while the case still fails.
    fn shrink(case: &mut Case, c: &mut CGpu, s: &mut Scratch) {
        loop {
            let mut changed = false;
            let mut i = 0;
            while i < case.ops.len() {
                let removed = case.ops.remove(i);
                if run(case, c, s).is_err() {
                    changed = true;
                } else {
                    case.ops.insert(i, removed);
                    i += 1;
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn cases() -> usize {
        std::env::var("GPU_DIFF_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2500)
    }

    fn campaign(name: &str, mix: Mix, seed: u64, min_each: usize) {
        let mut c = CGpu::new();
        let mut s = Scratch {
            a: vec![0; 640 * 480],
            b: vec![0; 640 * 480],
        };
        let mut g = Gen {
            r: Rng(seed),
            mix,
            area: (0, 0, 1023, 511),
            ofs: (0, 0),
            counts: BTreeMap::new(),
        };
        let n = cases();
        let mut failures = Vec::new();
        for i in 0..n {
            let init = g.r.next();
            let mut case = g.case(init);
            if let Err(e) = run(&case, &mut c, &mut s) {
                let orig = case.ops.len();
                shrink(&mut case, &mut c, &mut s);
                let e2 = run(&case, &mut c, &mut s).err().unwrap_or(e);
                let mut m = format!(
                    "case {i} (init VRAM seed {:#x}, mix {name}) fails: {e2}\nminimal stream ({} of {orig} ops):\n",
                    case.init_seed,
                    case.ops.len()
                );
                for o in &case.ops {
                    m += &format!("  {}\n", o.show());
                }
                failures.push(m);
                if failures.len() >= 3 {
                    break;
                }
            }
        }
        let mut report = format!("[{name}] {n} cases, {} mismatching\n", failures.len());
        for (k, v) in &g.counts {
            report += &format!("  {k:<28} {v}\n");
        }
        println!("{report}");
        assert!(
            failures.is_empty(),
            "GPU mismatch(es) against the C:\n{}",
            failures.join("\n")
        );
        if n >= 2000 {
            let low: Vec<_> = g
                .counts
                .iter()
                .filter(|(k, v)| **v < min_each && !k.ends_with("semi"))
                .collect();
            assert!(low.is_empty(), "too few cases for: {low:?}");
        }
    }

    #[test]
    fn random_general() {
        campaign("general", Mix::General, 0x1234_5678_9ABC_DEF1, 200);
    }

    #[test]
    fn random_blend_and_mask() {
        campaign("blend", Mix::Blend, 0xFEED_FACE_CAFE_0001, 50);
    }

    #[test]
    fn random_textured() {
        campaign("textured", Mix::Textured, 0x0BAD_C0DE_1357_9BDF, 20);
    }
}
