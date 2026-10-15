//! HD surfaces: the draw / display buffers drawn a second time at S x S pixels per VRAM pixel,
//! next to the 1x VRAM, which stays exactly as `gpu.rs` leaves it.
//!
//! Translated from DW2-PC (github.com/Wyrelade/DW2-PC, CC0), `backend/psxgpu_hd.c` at commit
//! 2d491b5 (with `psxgpu_hd.h` and `psxgpu_hd_int.h`). That is DW2-PC's original work. Only the
//! software path is ported: the SDL_GPU renderer, the 16:9 margins, sharpening, smooth frames
//! (PR.34) and packets / layers are left out. Differences from the C are marked below.
//!
//! A surface is one draw / display buffer rect of VRAM (the game's two 320x240 buffers) kept at
//! S x S pixels per VRAM pixel in the PS1 format (15-bit BGR + mask bit). Primitives are drawn
//! into it at that resolution with the PS1 rules applied per pixel of the surface; textures and
//! CLUTs always come from the 1x VRAM. Pixel (X, Y) of a surface samples the VRAM point
//! (x + X / S, y + Y / S), so a 1x pixel is the S x S block whose first pixel samples the same
//! point as the PS1 does. Vertex positions are fixed point with `SUB` fraction bits.
//!
//! Primitives are queued with the drawing state they saw and drawn at the next flush: before
//! any VRAM transfer or fill, a surface change or a display read. A flush splits the surface
//! rows between threads (row Y goes to part Y % parts); every part walks the whole queue in
//! order, so each pixel sees the primitives in packet order and the result does not depend on
//! the thread count.
//!
//! Changes from the C: shading follows `gpu.rs` (the 1x rules fitted to the oracle: only Gouraud
//! polygons are dithered, the semi-transparent dithered path blends on 8 bits, blend mode
//! B + F/4 quarters the texel first), so a 2D picture matches the 1x one; the odd-row-tail quirk
//! of the 1x path is not reproduced. A quad's triangles go in the 1x order (second first).
//! Surfaces are made when registered, not at first use. Precise positions (`precise`, slice 8.3)
//! are ported but always off for now.

use crate::gpu::{DITHER_TBL, H_MASK, State, Tex, VRAM_W, Vtx, W_MASK, to5};

/// Fraction bits of the fixed-point surface positions.
const SUB: u32 = 4;
/// Most surfaces kept (the game has two buffers).
const MAX_SURF: usize = 4;
/// A queue covering fewer surface pixels than this is drawn on the calling thread alone (starting
/// threads costs more).
const PARALLEL_WORK: u64 = 20_000;
/// The queue is drawn when it reaches this many commands.
const MAX_QUEUE: usize = 1 << 16;

/// A surface: VRAM rect x, y, w, h; `px` is (w * S) x (h * S) pixels, empty until used.
struct Surf {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    px: Vec<u16>,
}

impl Surf {
    fn contains(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        x >= self.x && y >= self.y && x + w <= self.x + self.w && y + h <= self.y + self.h
    }
}

/// The part of a surface a command needs.
#[derive(Clone, Copy)]
struct Geo {
    si: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct TriCmd {
    v: [Vtx; 3],
    tex: Option<Tex>,
    gouraud: bool,
    semi: bool,
    abr: i32,
    precise: bool,
}

struct RectCmd {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    tex: Option<Tex>,
    u0: i32,
    v0: i32,
    r: i32,
    g: i32,
    b: i32,
    semi: bool,
    abr: i32,
}

struct LineCmd {
    a: Vtx,
    b: Vtx,
    gouraud: bool,
    semi: bool,
    abr: i32,
}

enum Kind {
    Tri(TriCmd),
    Rect(RectCmd),
    Line(LineCmd),
}

/// One queued primitive with the drawing state it saw.
struct Cmd {
    geo: Geo,
    st: State,
    kind: Kind,
}

/// The HD side channel of a `Gpu`.
pub struct Hd {
    scale: i32,
    surfs: Vec<Surf>,
    queue: Vec<Cmd>,
    threads: usize,
    missed: bool,
    /// Rough pixels the queued commands cover (1x), to decide whether threads pay.
    work: u64,
    /// Time spent drawing the queue so far, nanoseconds (for `DW1_HD_STATS`).
    pub(crate) flush_ns: u64,
}

/// The default number of drawing threads: one per logical core, at most 8.
pub fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, 8)
}

impl Hd {
    /// An HD output at `scale` (2..=8 is useful; 1 works but is pointless) drawing on `threads`
    /// threads (0 = automatic).
    pub fn new(scale: u32, threads: usize) -> Hd {
        Hd {
            scale: scale.clamp(1, 8) as i32,
            surfs: Vec::new(),
            queue: Vec::new(),
            threads: if threads == 0 {
                default_threads()
            } else {
                threads.clamp(1, 16)
            },
            missed: false,
            flush_ns: 0,
            work: 0,
        }
    }

    pub fn scale(&self) -> u32 {
        self.scale as u32
    }

    pub fn set_threads(&mut self, n: usize) {
        self.threads = if n == 0 {
            default_threads()
        } else {
            n.clamp(1, 16)
        };
    }

    // ---- surfaces ----

    /// Surface pixels from the 1x VRAM (nearest) for the VRAM rect x, y, w, h inside `s`.
    fn copy_from_vram(s: &mut Surf, scale: i32, vram: &[u16], x: i32, y: i32, w: i32, h: i32) {
        let sc = scale as usize;
        let pitch = s.w as usize * sc;
        for j in y..y + h {
            let src = &vram[(j as usize & H_MASK as usize) * VRAM_W..][..VRAM_W];
            let row0 = (j - s.y) as usize * sc * pitch;
            for i in x..x + w {
                let c = src[i as usize & W_MASK as usize];
                let at = row0 + (i - s.x) as usize * sc;
                s.px[at..at + sc].fill(c);
            }
            let from = row0 + (x - s.x) as usize * sc;
            for k in 1..sc {
                s.px.copy_within(from..from + w as usize * sc, from + k * pitch);
            }
        }
    }

    fn ensure(&mut self, si: usize, vram: &[u16]) {
        let scale = self.scale;
        let s = &mut self.surfs[si];
        if s.px.is_empty() {
            s.px = vec![0; (s.w * scale) as usize * (s.h * scale) as usize];
            let (x, y, w, h) = (s.x, s.y, s.w, s.h);
            Self::copy_from_vram(s, scale, vram, x, y, w, h);
        }
    }

    /// Forgets the surface pixels (the 1x VRAM was written behind our back); they are rebuilt
    /// from it at the next use.
    pub fn invalidate(&mut self) {
        self.queue.clear();
        self.work = 0;
        for s in &mut self.surfs {
            s.px = Vec::new();
        }
    }

    /// A draw or display buffer rect (PutDrawEnv clip, PutDispEnv area). Kept when an existing
    /// surface holds it; otherwise the surfaces it overlaps are dropped and it becomes one.
    pub fn register(&mut self, vram: &[u16], x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.flush(vram);
        if self.surfs.iter().any(|s| s.contains(x, y, w, h)) {
            return;
        }
        self.surfs
            .retain(|s| !(x < s.x + s.w && s.x < x + w && y < s.y + s.h && s.y < y + h));
        if self.surfs.len() == MAX_SURF {
            self.surfs.remove(0);
        }
        self.surfs.push(Surf {
            x,
            y,
            w,
            h,
            px: Vec::new(),
        });
        self.ensure(self.surfs.len() - 1, vram);
    }

    /// The surface holding the current draw area (allocated), or None.
    fn target(&mut self, vram: &[u16], st: &State) -> Option<Geo> {
        let (w, h) = (st.clip_x1 - st.clip_x0 + 1, st.clip_y1 - st.clip_y0 + 1);
        let si = self
            .surfs
            .iter()
            .position(|s| s.contains(st.clip_x0, st.clip_y0, w, h));
        let Some(si) = si else {
            if !self.missed {
                self.missed = true;
                eprintln!(
                    "[gpu] HD: draw area {},{}..{},{} is in no surface, drawn at 1x only (logged once)",
                    st.clip_x0, st.clip_y0, st.clip_x1, st.clip_y1
                );
            }
            return None;
        };
        self.ensure(si, vram);
        let s = &self.surfs[si];
        Some(Geo {
            si,
            x: s.x,
            y: s.y,
            w: s.w,
            h: s.h,
        })
    }

    fn push(&mut self, vram: &[u16], geo: Geo, st: &State, kind: Kind) {
        if self.queue.len() >= MAX_QUEUE {
            self.flush(vram);
        }
        let (cx0, cy0, cx1, cy1) = (st.clip_x0, st.clip_y0, st.clip_x1, st.clip_y1);
        let clipped = |x0: i32, y0: i32, x1: i32, y1: i32| {
            let (w, h) = (x1.min(cx1) - x0.max(cx0) + 1, y1.min(cy1) - y0.max(cy0) + 1);
            w.max(0) as u64 * h.max(0) as u64
        };
        self.work += match &kind {
            Kind::Tri(t) => {
                let x = t.v.iter().map(|p| p.x);
                let y = t.v.iter().map(|p| p.y);
                clipped(
                    x.clone().min().unwrap(),
                    y.clone().min().unwrap(),
                    x.max().unwrap(),
                    y.max().unwrap(),
                )
            }
            Kind::Rect(r) => clipped(r.x0, r.y0, r.x0 + r.w - 1, r.y0 + r.h - 1),
            Kind::Line(l) => {
                (l.b.x - l.a.x)
                    .unsigned_abs()
                    .max((l.b.y - l.a.y).unsigned_abs()) as u64
                    + 1
            }
        } * (self.scale * self.scale) as u64;
        self.queue.push(Cmd { geo, st: *st, kind });
    }

    // ---- queue ----

    /// Draws the queue into the surfaces.
    pub fn flush(&mut self, vram: &[u16]) {
        if self.queue.is_empty() {
            return;
        }
        let parts = if self.work < PARALLEL_WORK {
            1
        } else {
            self.threads
        };
        self.work = 0;
        let started = std::time::Instant::now();
        let scale = self.scale;
        let mut ps: Vec<Part> = (0..parts)
            .map(|part| Part {
                part: part as i32,
                parts: parts as i32,
                scale,
                rows: (0..self.surfs.len()).map(|_| Vec::new()).collect(),
            })
            .collect();
        for (si, s) in self.surfs.iter_mut().enumerate() {
            if s.px.is_empty() {
                continue;
            }
            let pitch = (s.w * scale) as usize;
            for (k, row) in s.px.chunks_mut(pitch).enumerate() {
                ps[k % parts].rows[si].push(row);
            }
        }
        let queue = &self.queue;
        if parts == 1 {
            run_part(&mut ps[0], vram, queue);
        } else {
            std::thread::scope(|sc| {
                let mut it = ps.iter_mut();
                let first = it.next().unwrap();
                for p in it {
                    sc.spawn(move || run_part(p, vram, queue));
                }
                run_part(first, vram, queue);
            });
        }
        self.queue.clear();
        self.flush_ns += started.elapsed().as_nanos() as u64;
    }

    // ---- primitives ----

    /// A triangle the 1x path just drew (it takes the same early rejections). `precise`: use
    /// px / py / pz (decided per polygon by the caller, so both triangles of a quad agree).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn triangle(
        &mut self,
        vram: &[u16],
        st: &State,
        v: [&Vtx; 3],
        gouraud: bool,
        tex: Option<&Tex>,
        semi: bool,
        abr: i32,
        precise: bool,
    ) {
        let minx = v.iter().map(|p| p.x).min().unwrap();
        let maxx = v.iter().map(|p| p.x).max().unwrap();
        let miny = v.iter().map(|p| p.y).min().unwrap();
        let maxy = v.iter().map(|p| p.y).max().unwrap();
        if (maxx - minx >= 1024 || maxy - miny >= 512) && !(precise && precise_fits(v)) {
            return;
        }
        // flat on its integer corners (a precise one may not be: draw_tri checks those)
        if !precise
            && (v[1].x - v[0].x) as i64 * (v[2].y - v[0].y) as i64
                - (v[1].y - v[0].y) as i64 * (v[2].x - v[0].x) as i64
                == 0
        {
            return;
        }
        // wholly outside the draw area: nothing to draw (the 1x path drops it the same way)
        if maxx < st.clip_x0 || minx > st.clip_x1 || maxy < st.clip_y0 || miny > st.clip_y1 {
            return;
        }
        let Some(geo) = self.target(vram, st) else {
            return;
        };
        self.push(
            vram,
            geo,
            st,
            Kind::Tri(TriCmd {
                v: [*v[0], *v[1], *v[2]],
                tex: tex.copied(),
                gouraud,
                semi,
                abr,
                precise,
            }),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rect(
        &mut self,
        vram: &[u16],
        st: &State,
        (x0, y0, w, h): (i32, i32, i32, i32),
        tex: Option<&Tex>,
        (u0, v0): (i32, i32),
        (r, g, b): (i32, i32, i32),
        semi: bool,
        abr: i32,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        let Some(geo) = self.target(vram, st) else {
            return;
        };
        self.push(
            vram,
            geo,
            st,
            Kind::Rect(RectCmd {
                x0,
                y0,
                w,
                h,
                tex: tex.copied(),
                u0,
                v0,
                r,
                g,
                b,
                semi,
                abr,
            }),
        );
    }

    pub(crate) fn line(
        &mut self,
        vram: &[u16],
        st: &State,
        a: &Vtx,
        b: &Vtx,
        gouraud: bool,
        semi: bool,
        abr: i32,
    ) {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        if dx <= -1024 || dx >= 1024 || dy <= -512 || dy >= 512 {
            return;
        }
        let Some(geo) = self.target(vram, st) else {
            return;
        };
        self.push(
            vram,
            geo,
            st,
            Kind::Line(LineCmd {
                a: *a,
                b: *b,
                gouraud,
                semi,
                abr,
            }),
        );
    }

    // ---- transfers ----

    /// Intersection of a VRAM rect with a surface (no wrap: the game's transfers stay inside
    /// VRAM).
    fn clip_rect(s: &Surf, x: i32, y: i32, w: i32, h: i32) -> Option<(i32, i32, i32, i32)> {
        let x0 = x.max(s.x);
        let y0 = y.max(s.y);
        let x1 = (x + w).min(s.x + s.w);
        let y1 = (y + h).min(s.y + s.h);
        (x1 > x0 && y1 > y0).then_some((x0, y0, x1 - x0, y1 - y0))
    }

    /// A fill (colour `c` in VRAM format). The queue is drawn first, so call this after the 1x
    /// fill only if `flush` ran before it (`Gpu` does).
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: u16) {
        let sc = self.scale as usize;
        for s in &mut self.surfs {
            let Some((fx, fy, fw, fh)) = Self::clip_rect(s, x, y, w, h) else {
                continue;
            };
            if s.px.is_empty() {
                continue;
            }
            let pitch = s.w as usize * sc;
            let k0 = (fx - s.x) as usize * sc;
            let k1 = k0 + fw as usize * sc;
            for j in (fy - s.y) as usize * sc..(fy + fh - s.y) as usize * sc {
                s.px[j * pitch + k0..j * pitch + k1].fill(c);
            }
        }
    }

    /// A rect the 1x VRAM just got from the CPU or a VRAM copy: copied into the surfaces.
    /// Call `flush` before the 1x write.
    pub fn refresh(&mut self, vram: &[u16], x: i32, y: i32, w: i32, h: i32) {
        let scale = self.scale;
        for s in &mut self.surfs {
            let Some((fx, fy, fw, fh)) = Self::clip_rect(s, x, y, w, h) else {
                continue;
            };
            if s.px.is_empty() {
                continue;
            }
            Self::copy_from_vram(s, scale, vram, fx, fy, fw, fh);
        }
    }

    // ---- display ----

    /// The display area (x, y, w, h in VRAM) from a surface: w*S x h*S pixels of 0x00RRGGBB
    /// into `out`. None when no surface holds the area.
    pub fn read_display(
        &mut self,
        vram: &[u16],
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        out: &mut Vec<u32>,
    ) -> Option<(usize, usize)> {
        self.flush(vram);
        let si = self.surfs.iter().position(|s| s.contains(x, y, w, h))?;
        self.ensure(si, vram);
        let sc = self.scale as usize;
        let s = &self.surfs[si];
        let pitch = s.w as usize * sc;
        let (ow, oh) = (w as usize * sc, h as usize * sc);
        out.clear();
        out.reserve(ow * oh);
        let x0 = (x - s.x) as usize * sc;
        for j in 0..oh {
            let row = &s.px[((y - s.y) as usize * sc + j) * pitch + x0..][..ow];
            out.extend(row.iter().map(|&c| rgb15(c)));
        }
        Some((ow, oh))
    }
}

/// 15-bit BGR to 0x00RRGGBB, the same expansion as `Gpu::read_display`.
pub(crate) fn rgb15(c: u16) -> u32 {
    let c = c as u32;
    let e = |v: u32| (v << 3) | (v >> 2);
    (e(c & 31) << 16) | (e((c >> 5) & 31) << 8) | e((c >> 10) & 31)
}

/// PG.10 b2: the GPU drops triangles 1024 or more columns wide or 512 or more rows tall; an HD
/// surface draws such a triangle when all three corners are precise and it is of a sane size.
fn precise_fits(v: [&Vtx; 3]) -> bool {
    if v.iter().any(|p| p.pz <= 0.0) {
        return false;
    }
    let ext = |f: fn(&Vtx) -> f32| {
        let mn = v.iter().map(|p| f(p)).fold(f32::INFINITY, f32::min);
        let mx = v.iter().map(|p| f(p)).fold(f32::NEG_INFINITY, f32::max);
        mx - mn
    };
    ext(|p| p.px) < 16384.0 && ext(|p| p.py) < 16384.0
}

// ---- drawing ----

/// One drawing thread's share of the surfaces: the rows Y with Y % parts == part.
struct Part<'a> {
    part: i32,
    parts: i32,
    scale: i32,
    /// per surface, its rows part, part + parts, ... in order
    rows: Vec<Vec<&'a mut [u16]>>,
}

impl Part<'_> {
    fn row(&mut self, si: usize, y: i32) -> &mut [u16] {
        debug_assert!(y % self.parts == self.part);
        &mut *self.rows[si][(y / self.parts) as usize]
    }

    /// First row >= y that belongs to this part.
    fn first_row(&self, y: i32) -> i32 {
        y + ((self.part - y % self.parts) % self.parts + self.parts) % self.parts
    }
}

fn run_part(p: &mut Part, vram: &[u16], queue: &[Cmd]) {
    for c in queue {
        match &c.kind {
            Kind::Tri(t) => draw_tri(p, vram, c, t),
            Kind::Rect(r) => draw_rect(p, vram, c, r),
            Kind::Line(l) => draw_line(p, c, l),
        }
    }
}

/// Clip rect of the draw area in surface pixels, inclusive, inside the surface.
fn hd_clip(st: &State, g: &Geo, scale: i32) -> (i32, i32, i32, i32) {
    (
        ((st.clip_x0 - g.x) * scale).max(0),
        ((st.clip_y0 - g.y) * scale).max(0),
        ((st.clip_x1 + 1 - g.x) * scale - 1).min(g.w * scale - 1),
        ((st.clip_y1 + 1 - g.y) * scale - 1).min(g.h * scale - 1),
    )
}

/// The texel at u, v (texture window applied) of a texture.
fn texel(st: &State, vram: &[u16], t: &Tex, u: i32, v: i32) -> u16 {
    let u = ((u & !st.tw_mask_x) | (st.tw_off_x & st.tw_mask_x)) & 0xFF;
    let v = ((v & !st.tw_mask_y) | (st.tw_off_y & st.tw_mask_y)) & 0xFF;
    let y = (t.ty + v) & H_MASK;
    let at = |x: i32, y: i32| vram[(y as usize) * VRAM_W + (x & W_MASK) as usize] as i32;
    (match t.mode {
        0 => {
            let w = at(t.tx + (u >> 2), y);
            at(t.cx + ((w >> ((u & 3) * 4)) & 0xF), t.cy)
        }
        1 => {
            let w = at(t.tx + (u >> 1), y);
            at(t.cx + ((w >> ((u & 1) * 8)) & 0xFF), t.cy)
        }
        _ => at(t.tx + u, y),
    }) as u16
}

/// Writes one pixel (r, g, b in 0..31, or out of range when blending follows); the PS1 rules of
/// `Gpu::put`.
#[allow(clippy::too_many_arguments)]
fn hput(
    st: &State,
    d: &mut u16,
    mut r: i32,
    mut g: i32,
    mut b: i32,
    semi: bool,
    abr: i32,
    mask: bool,
) {
    if st.check_mask && *d & 0x8000 != 0 {
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
    let m = if mask || st.set_mask { 0x8000 } else { 0 };
    *d = (r | (g << 5) | (b << 10) | m) as u16;
}

/// Semi-transparent pixel of a dithered polygon (`Gpu::put_semi8`): blended on 8-bit channels,
/// then the dither is added.
fn hput_semi8(st: &State, d: &mut u16, c8: [i32; 3], dith: i32, abr: i32, mask: bool) {
    let old = *d;
    if st.check_mask && old & 0x8000 != 0 {
        return;
    }
    let mut out = [0i32; 3];
    for k in 0..3 {
        let (b, s) = ((((old >> (5 * k)) & 31) as i32) << 3, c8[k]);
        let v = match abr {
            0 => (b + s) >> 1,
            1 => b + s,
            2 => b - s,
            _ => b + (s >> 2),
        };
        out[k] = to5(v.clamp(0, 255), dith);
    }
    let m = if mask || st.set_mask { 0x8000 } else { 0 };
    *d = (out[0] | (out[1] << 5) | (out[2] << 10) | m) as u16;
}

/// One pixel of a primitive (`Gpu::shade`, without the odd-row tail quirk). `dith`: the
/// primitive is dithered, `d` its dither offset at this pixel; `poly`: a polygon, not a
/// rectangle.
#[allow(clippy::too_many_arguments)]
/// The value `hshade` would write when it doesn't depend on what is there already (no
/// semi-transparency, no mask check; `dith` and `d`: the dither at this pixel): `Some(Some(pixel))`, or
/// `Some(None)` for a transparent texel; `None` when the destination matters.
fn direct_value(
    st: &State,
    tc: Option<(&Tex, i32)>,
    (r, g, b): (i32, i32, i32),
    semi: bool,
    dith: bool,
    d: i32,
) -> Option<Option<u16>> {
    if st.check_mask || (dith && semi) {
        return None;
    }
    let set = if st.set_mask { 0x8000 } else { 0 };
    let Some((t, c)) = tc else {
        if semi {
            return None;
        }
        return Some(Some(
            (to5(r, d) | (to5(g, d) << 5) | (to5(b, d) << 10) | set) as u16,
        ));
    };
    if c == 0 {
        return Some(None);
    }
    if semi && c & 0x8000 != 0 {
        return None;
    }
    let (tr, tg, tb) = (c & 31, (c >> 5) & 31, (c >> 10) & 31);
    let (or, og, ob) = if t.raw {
        (to5(tr << 3, d), to5(tg << 3, d), to5(tb << 3, d))
    } else {
        (
            to5((tr * r) >> 4, d),
            to5((tg * g) >> 4, d),
            to5((tb * b) >> 4, d),
        )
    };
    let m = if c & 0x8000 != 0 { 0x8000 } else { set };
    Some(Some((or | (og << 5) | (ob << 10) | m) as u16))
}

fn hshade(
    st: &State,
    p: &mut u16,
    tc: Option<(&Tex, i32)>,
    (r, g, b): (i32, i32, i32),
    semi: bool,
    abr: i32,
    dith: bool,
    d: i32,
    poly: bool,
) {
    let Some((t, c)) = tc else {
        if dith && semi {
            hput_semi8(st, p, [r, g, b], d, abr, false);
        } else {
            hput(st, p, to5(r, d), to5(g, d), to5(b, d), semi, abr, false);
        }
        return;
    };
    if c == 0 {
        return;
    }
    let (mut tr, mut tg, mut tb) = (c & 31, (c >> 5) & 31, (c >> 10) & 31);
    let tsemi = semi && c & 0x8000 != 0;
    if dith && tsemi {
        let c8 = if t.raw {
            [tr << 3, tg << 3, tb << 3]
        } else {
            [(tr * r) >> 4, (tg * g) >> 4, (tb * b) >> 4]
        };
        hput_semi8(st, p, c8, d, abr, true);
        return;
    }
    if poly && tsemi && abr == 3 && !dith {
        // blend mode B + F/4: the texel is quartered before it is modulated
        if st.check_mask && *p & 0x8000 != 0 {
            return;
        }
        let m = [r, g, b];
        let t5 = [tr, tg, tb];
        let mut out = [0i32; 3];
        for k in 0..3 {
            let sv = if t.raw {
                t5[k] >> 2
            } else {
                (((t5[k] >> 2) * m[k]) >> 4) >> 3
            };
            out[k] = (((*p >> (5 * k)) & 31) as i32 + sv).min(31);
        }
        *p = (out[0] | (out[1] << 5) | (out[2] << 10) | 0x8000) as u16;
        return;
    }
    if t.raw {
        tr = to5(tr << 3, d);
        tg = to5(tg << 3, d);
        tb = to5(tb << 3, d);
    } else {
        let blend = poly && tsemi && !dith;
        let cut = |v: i32| if blend { v >> 3 } else { to5(v, d) };
        tr = cut((tr * r) >> 4);
        tg = cut((tg * g) >> 4);
        tb = cut((tb * b) >> 4);
    }
    hput(st, p, tr, tg, tb, tsemi, abr, c & 0x8000 != 0);
}

// ---- triangles ----

fn fdiv(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

fn cdiv(a: i64, b: i64) -> i64 {
    -fdiv(-a, b)
}

#[derive(Clone, Copy, Default)]
struct Edge {
    ax: i64,
    ay: i64,
    dx: i64,
    dy: i64,
    /// 0 on a top / left edge (included on ties), else 1
    thr: i64,
}

impl Edge {
    fn new(ax: i64, ay: i64, bx: i64, by: i64) -> Edge {
        let (dx, dy) = (bx - ax, by - ay);
        Edge {
            ax,
            ay,
            dx,
            dy,
            thr: if dy < 0 || (dy == 0 && dx > 0) { 0 } else { 1 },
        }
    }

    /// The edge function at pixel column 0 of row `y`.
    fn row(&self, y: i64) -> i64 {
        self.dx * ((y << SUB) - self.ay) + self.dy * self.ax
    }

    /// Its step per column.
    fn step(&self) -> i64 {
        -(self.dy << SUB)
    }
}

/// The 1x rasterizer's colour interpolation (`Gpu::triangle`): the vertices taken top to
/// bottom, the step per pixel from the widest row, the row's start from its left edge.
struct Col1x {
    tv: Vtx,
    mv: Vtx,
    bv: Vtx,
    gx: [i64; 3],
    minx: i32,
    /// a textured Gouraud polygon without dither takes its colour once per pair of pixels
    pairs: bool,
}

/// Where a 1x row starts: the left edge's x in 16.16 and the three colours there.
struct Col1xRow {
    xe: i64,
    base: [i64; 3],
    xs0: i64,
}

impl Col1x {
    fn new(v: [&Vtx; 3], st: &State, pairs: bool) -> Option<Col1x> {
        let mut sv = [*v[0], *v[1], *v[2]];
        sv.sort_by_key(|p| (p.y, p.x));
        let (tv, mv, bv) = (sv[0], sv[1], sv[2]);
        if bv.y == tv.y {
            return None;
        }
        let ratio = (((mv.y - tv.y) as i64) << 16) / (bv.y - tv.y) as i64;
        let width = ratio * (bv.x - tv.x) as i64 + (((tv.x - mv.x) as i64) << 16);
        let mut gx = [0i64; 3];
        for (i, g) in gx.iter_mut().enumerate() {
            let da = ratio * (Self::attr(&bv, i) - Self::attr(&tv, i))
                + ((Self::attr(&tv, i) - Self::attr(&mv, i)) << 16);
            *g = if width == 0 { 0 } else { (da << 16) / width };
        }
        let minx = v.iter().map(|p| p.x).min().unwrap().max(st.clip_x0);
        Some(Col1x {
            tv,
            mv,
            bv,
            gx,
            minx,
            pairs,
        })
    }

    fn attr(p: &Vtx, i: usize) -> i64 {
        (match i {
            0 => p.r,
            1 => p.g,
            _ => p.b,
        }) as i64
    }

    fn row(&self, y: i32) -> Col1xRow {
        let (tv, mv, bv) = (self.tv, self.mv, self.bv);
        let short = if y < mv.y { (tv, mv) } else { (mv, bv) };
        let xnum = |p: &Vtx, q: &Vtx| p.x as i64 * (q.y - y) as i64 + q.x as i64 * (y - p.y) as i64;
        let (ep, eq) = if short.1.y == short.0.y {
            (tv, bv)
        } else {
            let xs = xnum(&short.0, &short.1) * (bv.y - tv.y) as i64;
            let xl = xnum(&tv, &bv) * (short.1.y - short.0.y) as i64;
            if xs < xl { short } else { (tv, bv) }
        };
        let den = (eq.y - ep.y) as i64;
        let xe = ((ep.x as i64) << 16) + (((eq.x - ep.x) as i64 * (y - ep.y) as i64) << 16) / den;
        let xs0 = ((xe + 0xFFFF) >> 16).max(self.minx as i64);
        let mut base = [0i64; 3];
        for (i, b) in base.iter_mut().enumerate() {
            let da = Self::attr(&eq, i) - Self::attr(&ep, i);
            *b = (Self::attr(&ep, i) << 16) + ((da * (y - ep.y) as i64) << 16) / den;
        }
        Col1xRow { xe, base, xs0 }
    }

    /// The colour at 1x pixel `x` of a row.
    fn value(&self, r: &Col1xRow, x: i32) -> (i32, i32, i32) {
        let xi = if self.pairs {
            r.xs0 + 2 * (x as i64 - r.xs0).div_euclid(2)
        } else {
            x as i64
        };
        let mut out = [0i32; 3];
        for i in 0..3 {
            let d = (self.gx[i] as i128 * ((xi << 16) - r.xe) as i128) >> 16;
            let q = r.base[i] as i128 + d + 0x8000;
            out[i] = ((q >> 16).clamp(i32::MIN.into(), i32::MAX.into()) as i32).clamp(0, 255);
        }
        (out[0], out[1], out[2])
    }
}

fn draw_tri(p: &mut Part, vram: &[u16], c: &Cmd, t: &TriCmd) {
    let st = &c.st;
    let g = c.geo;
    let scale = p.scale;
    let sc = scale as i64;
    let tex = t.tex.as_ref();
    let (gouraud, semi, abr) = (t.gouraud, t.semi, t.abr);
    // [obs] only Gouraud polygons are dithered (as in gpu.rs)
    let dith = st.dither && gouraud;
    let mut v = [&t.v[0], &t.v[1], &t.v[2]];

    // fixed-point positions; the orientation from them
    let mut px = [0i64; 3];
    let mut py = [0i64; 3];
    for i in 0..3 {
        if t.precise {
            px[i] = ((v[i].px as f64 - g.x as f64) * scale as f64 * (1 << SUB) as f64 + 0.5).floor()
                as i64;
            py[i] = ((v[i].py as f64 - g.y as f64) * scale as f64 * (1 << SUB) as f64 + 0.5).floor()
                as i64;
        } else {
            px[i] = ((v[i].x - g.x) as i64 * sc) << SUB;
            py[i] = ((v[i].y - g.y) as i64 * sc) << SUB;
        }
    }
    let a1 = (px[1] - px[0]) * (py[2] - py[0]) - (py[1] - py[0]) * (px[2] - px[0]);
    if a1 == 0 {
        return;
    }
    if a1 < 0 {
        v.swap(1, 2);
        px.swap(1, 2);
        py.swap(1, 2);
    }
    let e = [
        Edge::new(px[1], py[1], px[2], py[2]),
        Edge::new(px[2], py[2], px[0], py[0]),
        Edge::new(px[0], py[0], px[1], py[1]),
    ];
    let area = e[2].dx * (py[2] - py[0]) - e[2].dy * (px[2] - px[0]);
    if area <= 0 {
        return;
    }
    let step = [e[0].step(), e[1].step(), e[2].step()];

    // attributes: 0 r, 1 g, 2 b, 3 u, 4 v
    let mut att = [[0i64; 3]; 5];
    for i in 0..3 {
        att[0][i] = v[i].r as i64;
        att[1][i] = v[i].g as i64;
        att[2][i] = v[i].b as i64;
        att[3][i] = v[i].u as i64;
        att[4][i] = v[i].v as i64;
    }
    let mut dn = [0i64; 5];
    for k in 0..5 {
        dn[k] = att[k][0] * step[0] + att[k][1] * step[1] + att[k][2] * step[2];
    }
    let mut block = false;
    let mut persp = false;
    let (mut umin, mut umax) = (0, 0);
    let mut pw = [0f64; 3];
    let mut puw = [0f64; 3];
    let mut pvw = [0f64; 3];
    if tex.is_some() {
        // No cross terms (du / dy = dv / dx = 0): a 2D sprite-like mapping. Its texels are read
        // at the 1x pixel's position so pixel art stays exact at any scale.
        let dudy = att[3][0] * e[0].dx + att[3][1] * e[1].dx + att[3][2] * e[2].dx;
        block = !t.precise && dudy == 0 && dn[4] == 0;
        // PG.2: perspective-correct mapping from the precise depths (1 / z interpolates
        // linearly on screen, so do u / z, v / z)
        persp = t.precise && v.iter().all(|p| p.pz > 0.0);
        if persp {
            for i in 0..3 {
                pw[i] = 1.0 / v[i].pz as f64;
                puw[i] = v[i].u as f64 * pw[i];
                pvw[i] = v[i].v as f64 * pw[i];
            }
        }
        umin = v[0].u.min(v[1].u).min(v[2].u);
        umax = v[0].u.max(v[1].u).max(v[2].u);
    }
    // A polygon that is not precise takes its colours from the 1x rasterizer's arithmetic at
    // the 1x pixel (constant over its S x S block, as the dither is), so a gradient is the 1x
    // one at every scale; a precise one interpolates smoothly across the surface pixels.
    let col1x = (gouraud && !t.precise)
        .then(|| {
            Col1x::new(
                [&t.v[0], &t.v[1], &t.v[2]],
                st,
                tex.is_some() && !dith && !semi,
            )
        })
        .flatten();
    let natt = if gouraud && col1x.is_none() { 3 } else { 0 };
    // flat or per-1x-pixel colour and a texel read at the 1x pixel: constant over each 1x
    // pixel's S columns
    let uniform = natt == 0 && (tex.is_none() || block);

    let (cx0, cy0, cx1, cy1) = hd_clip(st, &g, scale);
    let ymin = *py.iter().min().unwrap();
    let ymax = *py.iter().max().unwrap();
    let miny = (cdiv(ymin, 1 << SUB) as i32).max(cy0);
    let maxy = (fdiv(ymax, 1 << SUB) as i32).min(cy1);
    if miny > maxy {
        return;
    }
    let mut y = p.first_row(miny);
    while y <= maxy {
        let row_y = y;
        y += p.parts;
        let (mut xl, mut xr) = (cx0 as i64, cx1 as i64);
        let mut nr = [0i64; 3];
        for i in 0..3 {
            nr[i] = e[i].row(row_y as i64);
            if step[i] > 0 {
                xl = xl.max(cdiv(e[i].thr - nr[i], step[i]));
            } else if step[i] < 0 {
                xr = xr.min(fdiv(nr[i] - e[i].thr, -step[i]));
            } else if nr[i] < e[i].thr {
                xl = 1;
                xr = 0;
            }
        }
        if xl > xr {
            continue;
        }
        // attribute numerators at xl: sum att * edge; value = numerator / area
        let num = |k: usize| {
            att[k][0] * (nr[0] + step[0] * xl)
                + att[k][1] * (nr[1] + step[1] * xl)
                + att[k][2] * (nr[2] + step[2] * xl)
        };
        let mut c24 = [0i64; 3];
        let mut d24 = [0i64; 3];
        for k in 0..natt {
            c24[k] = (fdiv(num(k) << 16, area) << 8) + 0x80;
            d24[k] = fdiv(dn[k] << 24, area);
        }
        let (mut u24, mut v24, mut du24, mut dv24) = (0i64, 0i64, 0i64, 0i64);
        let (mut qw, mut qu, mut qv, mut dqw, mut dqu, mut dqv) =
            (0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
        let mut vv = 0i32;
        if persp {
            let e0 = (nr[0] + step[0] * xl) as f64;
            let e1 = (nr[1] + step[1] * xl) as f64;
            let e2 = (nr[2] + step[2] * xl) as f64;
            let a = area as f64;
            let s = [step[0] as f64, step[1] as f64, step[2] as f64];
            qw = (pw[0] * e0 + pw[1] * e1 + pw[2] * e2) / a;
            qu = (puw[0] * e0 + puw[1] * e1 + puw[2] * e2) / a;
            qv = (pvw[0] * e0 + pvw[1] * e1 + pvw[2] * e2) / a;
            dqw = (pw[0] * s[0] + pw[1] * s[1] + pw[2] * s[2]) / a;
            dqu = (puw[0] * s[0] + puw[1] * s[1] + puw[2] * s[2]) / a;
            dqv = (pvw[0] * s[0] + pvw[1] * s[1] + pvw[2] * s[2]) / a;
        } else if tex.is_some() && !block {
            u24 = (fdiv(num(3) << 16, area) << 8) + 0x80;
            v24 = (fdiv(num(4) << 16, area) << 8) + 0x80;
            du24 = fdiv(dn[3] << 24, area);
            dv24 = fdiv(dn[4] << 24, area);
        } else if tex.is_some() {
            // v from the row of the 1x pixel (constant along the row)
            let yb = (row_y - row_y % scale) as i64;
            let n: i64 = (0..3).map(|i| att[4][i] * e[i].row(yb)).sum();
            vv = fdiv(n, area) as i32;
        }
        let dy = ((row_y / scale + g.y) & 3) as usize;
        let mut bx = (xl / sc) as i32;
        let mut sub = (xl % sc) as i32;
        let mut u = -1i32;
        let mut texel_c = 0i32;
        let mut have_c = false;
        let (mut r, mut gr, mut b) = (v[0].r, v[0].g, v[0].b);
        let row = p.row(g.si, row_y);
        let rc1x = col1x.as_ref().map(|c| c.row(row_y / scale + g.y));
        let mut x = xl;
        while x <= xr {
            let d = if dith {
                DITHER_TBL[dy][((bx + g.x) & 3) as usize]
            } else {
                0
            };
            if let (Some(c), Some(rc)) = (&col1x, &rc1x) {
                if sub == 0 || x == xl {
                    (r, gr, b) = c.value(rc, bx + g.x);
                }
            } else if natt != 0 {
                r = ((c24[0] >> 24) as i32).clamp(0, 255);
                gr = ((c24[1] >> 24) as i32).clamp(0, 255);
                b = ((c24[2] >> 24) as i32).clamp(0, 255);
            }
            if tex.is_some() {
                if block {
                    if sub == 0 || u < 0 {
                        let xb = bx as i64 * sc;
                        let n: i64 = (0..3).map(|i| att[3][i] * (nr[i] + step[i] * xb)).sum();
                        u = (fdiv(n, area) as i32).clamp(umin, umax);
                    }
                } else if persp {
                    let iw = 1.0 / qw;
                    u = (qu * iw + 1.0 / 512.0).floor() as i32;
                    vv = (qv * iw + 1.0 / 512.0).floor() as i32;
                } else {
                    u = (u24 >> 24) as i32;
                    vv = (v24 >> 24) as i32;
                }
            }
            if let Some(t) = tex
                && (!block || sub == 0 || !have_c)
            {
                texel_c = texel(st, vram, t, u, vv) as i32;
                have_c = true;
            }
            if uniform {
                // the colour is the same over the rest of this 1x pixel (the whole row when
                // untextured): write it as a run
                let run = if tex.is_none() && !gouraud {
                    xr - x + 1
                } else {
                    ((scale - sub) as i64).min(xr - x + 1)
                };
                let at = x as usize;
                let tc = tex.map(|t| (t, texel_c));
                match direct_value(st, tc, (r, gr, b), semi, dith, d) {
                    Some(Some(val)) => row[at..at + run as usize].fill(val),
                    Some(None) => {}
                    None => {
                        for k in 0..run as usize {
                            hshade(
                                st,
                                &mut row[at + k],
                                tc,
                                (r, gr, b),
                                semi,
                                abr,
                                dith,
                                d,
                                true,
                            );
                        }
                    }
                }
                x += run;
                let adv = sub as i64 + run;
                sub = (adv % sc) as i32;
                bx += (adv / sc) as i32;
                continue;
            }
            hshade(
                st,
                &mut row[x as usize],
                tex.map(|t| (t, texel_c)),
                (r, gr, b),
                semi,
                abr,
                dith,
                d,
                true,
            );
            x += 1;
            for k in 0..natt {
                c24[k] += d24[k];
            }
            u24 += du24;
            v24 += dv24;
            qw += dqw;
            qu += dqu;
            qv += dqv;
            sub += 1;
            if sub == scale {
                sub = 0;
                bx += 1;
            }
        }
    }
}

// ---- rectangles ----

fn draw_rect(p: &mut Part, vram: &[u16], c: &Cmd, r: &RectCmd) {
    let st = &c.st;
    let g = c.geo;
    let scale = p.scale;
    let (cx0, cy0, cx1, cy1) = hd_clip(st, &g, scale);
    let xs = ((r.x0 - g.x) * scale).max(cx0);
    let xe = ((r.x0 + r.w - g.x) * scale - 1).min(cx1);
    let ys = ((r.y0 - g.y) * scale).max(cy0);
    let ye = ((r.y0 + r.h - g.y) * scale - 1).min(cy1);
    if ys > ye || xs > xe {
        return;
    }
    let mut y = p.first_row(ys);
    while y <= ye {
        let row_y = y;
        y += p.parts;
        let yy = row_y / scale + g.y - r.y0;
        let tv = if st.flip_y { r.v0 - yy } else { r.v0 + yy };
        let row = p.row(g.si, row_y);
        let mut texel_c = 0i32;
        let mut x = xs;
        while x <= xe {
            // one 1x pixel's columns at a time (the whole row when untextured)
            let run = if r.tex.is_none() {
                xe - x + 1
            } else {
                (scale - x % scale).min(xe - x + 1)
            };
            if let Some(t) = r.tex.as_ref() {
                let xx = x / scale + g.x - r.x0;
                let u = if st.flip_x { r.u0 - xx } else { r.u0 + xx };
                texel_c = texel(st, vram, t, u, tv) as i32;
            }
            let tc = r.tex.as_ref().map(|t| (t, texel_c));
            let at = x as usize;
            match direct_value(st, tc, (r.r, r.g, r.b), r.semi, false, 0) {
                Some(Some(val)) => row[at..at + run as usize].fill(val),
                Some(None) => {}
                None => {
                    for k in 0..run as usize {
                        hshade(
                            st,
                            &mut row[at + k],
                            tc,
                            (r.r, r.g, r.b),
                            r.semi,
                            r.abr,
                            false,
                            0,
                            false,
                        );
                    }
                }
            }
            x += run;
        }
    }
}

// ---- lines ----

/// A line S pixels thick: along the longer axis every surface column (or row) from the first 1x
/// end pixel's block to the last one's, the other axis interpolated between the block centres.
/// Colour interpolated the same way, dithered by the 1x pixel.
fn draw_line(p: &mut Part, c: &Cmd, l: &LineCmd) {
    let st = &c.st;
    let g = c.geo;
    let scale = p.scale;
    let sc = scale as i64;
    let (a, b) = (&l.a, &l.b);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let xmajor = dx.abs() >= dy.abs();
    let dith = st.dither && l.gouraud;
    let (cx0, cy0, cx1, cy1) = hd_clip(st, &g, scale);
    // major axis centres ac -> bc, minor axis centres aw -> bw (surface pixels)
    let cen = |v: i32, o: i32| (v - o) as i64 * sc + sc / 2;
    let (ac, bc, aw, bw) = if xmajor {
        (cen(a.x, g.x), cen(b.x, g.x), cen(a.y, g.y), cen(b.y, g.y))
    } else {
        (cen(a.y, g.y), cen(b.y, g.y), cen(a.x, g.x), cen(b.x, g.x))
    };
    let len = bc - ac;
    let m0 = ac.min(bc) - sc / 2;
    let m1 = ac.max(bc) - sc / 2 + sc - 1;
    for m in m0..=m1 {
        let mut tn = if len == 0 { 0 } else { m - ac };
        let (mut r, mut gr, mut bl) = (a.r, a.g, a.b);
        let w;
        if len != 0 {
            let l2 = len.abs();
            if len < 0 {
                tn = -tn;
            }
            tn = tn.clamp(0, l2);
            w = aw + fdiv((bw - aw) * tn * 2 + l2, 2 * l2);
            if l.gouraud {
                r = a.r + ((b.r - a.r) as i64 * tn / l2) as i32;
                gr = a.g + ((b.g - a.g) as i64 * tn / l2) as i32;
                bl = a.b + ((b.b - a.b) as i64 * tn / l2) as i32;
            }
        } else {
            w = aw;
        }
        for j in 0..sc {
            let q = w - sc / 2 + j;
            let (x, y) = if xmajor { (m, q) } else { (q, m) };
            if x < cx0 as i64 || x > cx1 as i64 || y < cy0 as i64 || y > cy1 as i64 {
                continue;
            }
            let (x, y) = (x as i32, y as i32);
            if y % p.parts != p.part {
                continue;
            }
            let d = if dith {
                DITHER_TBL[((y / scale + g.y) & 3) as usize][((x / scale + g.x) & 3) as usize]
            } else {
                0
            };
            let row = p.row(g.si, y);
            hput(
                st,
                &mut row[x as usize],
                to5(r, d),
                to5(gr, d),
                to5(bl, d),
                l.semi,
                l.abr,
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gpu::{Gpu, VRAM_W};

    const S: usize = 3;

    fn px(rgb: u32) -> u32 {
        rgb
    }

    /// A 2D scene: fills, an upload then sprites / flat and textured quads that sample it.
    fn scene(g: &mut Gpu) {
        // the game's setup: draw area 320x240 at (0, 0), offset 0, display the same
        g.gp0(&[
            0xE300_0000,
            0xE400_0000 | (239 << 10) | 319,
            0xE500_0000,
            0xE100_0000,
        ]);
        g.set_display(0, 0, 320, 240, false);
        g.set_display_enable(true);
        g.fill(0, 0, 320, 240, 0x30_20_10);
        // upload a 16x16 15-bit texture at (320, 0) with a transparent corner
        let tex: Vec<u16> = (0..256u16)
            .map(|i| {
                if i % 16 < 3 {
                    0
                } else {
                    0x8000 | (i * 37) & 0x7FFF
                }
            })
            .collect();
        g.load_image(320, 0, 16, 16, &tex);
        // also a 4-bit page with a CLUT, to check CLUT sampling
        let clut: Vec<u16> = (0..16u16)
            .map(|i| if i == 0 { 0 } else { (i * 2113) & 0x7FFF })
            .collect();
        g.load_image(0, 480, 16, 1, &clut);
        let nib: Vec<u16> = (0..64u16)
            .map(|i| i.wrapping_mul(0x1357) ^ 0x2468)
            .collect();
        g.load_image(384, 0, 4, 16, &nib);
        // flat opaque quad
        g.gp0(&[
            0x2800_80C0,
            20 << 16 | 30,
            20 << 16 | 70,
            60 << 16 | 30,
            60 << 16 | 70,
        ]);
        // flat semi-transparent quad (average)
        g.gp0(&[0xE100_0000 | (0 << 5)]);
        g.gp0(&[
            0x2A60_C020,
            40 << 16 | 50,
            40 << 16 | 120,
            90 << 16 | 50,
            90 << 16 | 120,
        ]);
        // textured quad, 15-bit page at (320, 0): tpage x = 5, mode 2; 1:1 mapping, u/v 0..15
        let tpage = 5 | (2 << 7);
        g.gp0(&[
            0x2C80_8080,
            100 << 16 | 100,
            0,
            116 << 16 | 100,
            tpage << 16 | 0x1000,
            100 << 16 | 116,
            0x10,
            116 << 16 | 116,
            0x1010,
        ]);
        // sprite (rectangle) textured with the 4-bit page at (384, 0) via CLUT at (0, 480)
        g.gp0(&[0xE100_0000 | 6 | (0 << 7)]);
        g.gp0(&[0x7480_8080, 150 << 16 | 150, (480 << 6) << 16]);
        // a plain rectangle with an odd offset and a 1x1
        g.gp0(&[0x6000_00FF, 200 << 16 | 33, 17 << 16 | 21]);
        g.gp0(&[0x6800_FF00, 5 << 16 | 5]);
        // fill again over part, then a sprite on top
        g.fill(16, 160, 32, 32, 0x00_80_00);
        g.gp0(&[0x7580_8080, 20 << 16 | 164, (480 << 6) << 16]);
        // upload over the texture then draw again (the draw must see the new texels)
        let tex2: Vec<u16> = tex.iter().map(|c| c ^ 0x1F).collect();
        g.load_image(320, 0, 16, 16, &tex2);
        g.gp0(&[
            0x2D80_8080,
            100 << 16 | 200,
            0,
            116 << 16 | 200,
            tpage << 16 | 0x1000,
            100 << 16 | 216,
            0x10,
            116 << 16 | 216,
            0x1010,
        ]);
        // dithered Gouraud quad and triangle (a window's gradient; the triangle is not axis-aligned)
        g.gp0(&[0xE100_0200]);
        g.gp0(&[
            0x3800_6020,
            150 << 16 | 20,
            0x0030_00C0,
            150 << 16 | 123,
            0x00A0_2030,
            190 << 16 | 20,
            0x0010_D0E0,
            190 << 16 | 123,
        ]);
        g.gp0(&[
            0x3000_FF00,
            200 << 16 | 230,
            0x0000_FF00,
            230 << 16 | 300,
            0x0000_00FF,
            235 << 16 | 215,
        ]);
        // a move
        g.move_image(100, 100, 220, 180, 16, 16);
        // a vertical and a horizontal line
        g.gp0(&[0x4000_FFFF, 10 << 16 | 200, 10 << 16 | 230]);
    }

    fn compare(g1: &mut Gpu, gs: &mut Gpu) {
        let mut a = vec![0u32; 640 * 480];
        let (w, h) = g1.read_display(&mut a).unwrap();
        let mut b = Vec::new();
        let (bw, bh) = gs.read_display_hd(&mut b).unwrap();
        assert_eq!((bw, bh), (w * S, h * S));
        let mut bad = 0;
        for y in 0..h {
            for x in 0..w {
                if px(a[y * w + x]) != b[(y * S) * bw + x * S] {
                    if bad < 10 {
                        eprintln!(
                            "({x}, {y}): 1x {:06x} hd {:06x}",
                            a[y * w + x],
                            b[(y * S) * bw + x * S]
                        );
                    }
                    bad += 1;
                }
            }
        }
        assert_eq!(bad, 0, "pixels whose HD block's first pixel differs");
    }

    #[test]
    fn hd_block_first_pixel_equals_1x_on_2d_scene() {
        let mut g1 = Gpu::new();
        let mut gs = Gpu::new();
        gs.set_scale(S as u32);
        scene(&mut g1);
        scene(&mut gs);
        // the 1x VRAM is the same with HD on
        assert!(g1.vram() == gs.vram());
        compare(&mut g1, &mut gs);
        // something was drawn
        let mut b = Vec::new();
        gs.read_display_hd(&mut b).unwrap();
        assert!(b.iter().collect::<std::collections::HashSet<_>>().len() > 8);
        let _ = VRAM_W;
    }

    #[test]
    fn scale_1_is_the_1x_picture_and_24_bit_is_replicated() {
        let mut g = Gpu::new();
        assert_eq!(g.scale(), 1);
        scene(&mut g);
        let (mut one, mut hd) = (vec![0u32; 640 * 480], Vec::new());
        let (w, h) = g.read_display(&mut one).unwrap();
        assert_eq!(g.read_display_hd(&mut hd), Some((w, h)));
        assert!(hd[..] == one[..w * h]);
        // a 24-bit display (a movie frame) at scale 2: every pixel doubled both ways
        g.set_scale(2);
        g.set_display(0, 0, 160, 8, true);
        let (w, h) = g.read_display(&mut one).unwrap();
        assert_eq!(g.read_display_hd(&mut hd), Some((w * 2, h * 2)));
        for y in 0..h * 2 {
            for x in 0..w * 2 {
                assert_eq!(hd[y * w * 2 + x], one[(y / 2) * w + x / 2]);
            }
        }
    }

    #[test]
    fn hd_does_not_depend_on_thread_count() {
        let mut pics = Vec::new();
        for threads in [1, 4, 7] {
            let mut g = Gpu::new();
            g.set_hd_threads(threads);
            g.set_scale(S as u32);
            // enough commands to run in parallel
            scene(&mut g);
            for i in 0..40u32 {
                g.gp0(&[
                    0x2A60_C020 + i,
                    (10 + i) << 16 | 10,
                    5 << 16 | (60 + i),
                    90 << 16 | 20,
                    80 << 16 | 120,
                ]);
                g.gp0(&[0x4000_FF00 + i, 5 << 16 | (i * 3), 130 << 16 | 200]);
            }
            let mut out = Vec::new();
            g.read_display_hd(&mut out).unwrap();
            pics.push(out);
        }
        assert!(pics[0] == pics[1] && pics[0] == pics[2]);
    }

    #[test]
    fn hd_follows_draw_offset_changes() {
        // a rectangle drawn under two different offsets lands at both places
        let mut g = Gpu::new();
        g.set_scale(2);
        g.gp0(&[0xE300_0000, 0xE400_0000 | (239 << 10) | 319, 0xE500_0000]);
        g.set_display(0, 0, 320, 240, false);
        g.set_display_enable(true);
        g.fill(0, 0, 320, 240, 0);
        g.gp0(&[0x6000_00FF, 0, 4 << 16 | 4]);
        g.gp0(&[0xE500_0000 | 100 | (50 << 11)]);
        g.gp0(&[0x6000_FF00, 0, 4 << 16 | 4]);
        let mut out = Vec::new();
        let (w, _) = g.read_display_hd(&mut out).unwrap();
        assert_eq!(out[0], 0xFF0000);
        assert_eq!(out[50 * 2 * w + 100 * 2], 0x00FF00);
    }
}
