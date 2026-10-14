#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that draws primitives on the real GPU (PCSX-Redux's
software renderer) and reads VRAM back, for `crates/backend/tests/gpu_oracle.rs` to compare the
Rust GPU model against. Black-box: only the results of what the emulator draws are used.

Usage: gpu_requests.py OUT_DIR [--seed N] [--scale K]

Writes OUT_DIR/req.txt (the requests) and OUT_DIR/cases.txt (what each case drew). Run req.txt
on the emulator (booklab, nothing else running; see the header of oracle/call-sdk.lua), then
keep req.txt, cases.txt and the emulator's results.txt together in
captures/oracle-runs/gpu-prims/ (gitignored; `DW1_GPU_CAPTURE` points the test elsewhere):

    ssh booklab 'mkdir -p ~/Developer/dw1-oracle/runs/gpu-prims && cd ~/Developer/dw1-oracle/runs/gpu-prims && cat > req.txt' < OUT_DIR/req.txt
    ssh booklab 'cd ~/Developer/dw1-oracle/runs/gpu-prims && R=~/Developer/dw1-oracle && \
        DW1_CALLS=$PWD/req.txt DW1_SYMBOLS=$R/symbols.txt caffeinate -dim $R/pcsx-redux -cli \
        -portable -interpreter -softgpu -bios $R/scph1001.bin -iso "$R/disc/Digimon World (USA).cue" \
        -dofile $R/oracle/call-sdk.lua -run > run.log 2>&1; tail -2 run.log'

How a case works. The game's own libgpu runs the calls: DrawOTag with a chain of GP0 packets
(draw mode E1, texture window E2, draw area E3/E4, offset E5, a fill of the test area, then the
primitive), DrawSync(0), StoreImage of the test rectangle, DrawSync(0) with `out=` on the copy.
VRAM layout (all of it is ours: the game is only there to host the SDK calls):
  texture A  (640,0)   128 x 256 halfwords, 8-bit texels, texel(u, v) = u        (tpage x 10)
  texture B  (768,0)   128 x 256 halfwords, 8-bit texels, texel(u, v) = v        (tpage x 12)
  CLUT       (640,300) 256 halfwords, entry i = (i & 31) | (i >> 5) << 5 | 1 << 10
  test area  (0,256)   256 x 48, filled with 0x7FFF (white; a drawn pixel never has bit 15 set, and white is rare) before every case
Texture A reads out u, texture B reads out v, so a textured case shows the interpolated
coordinates directly (the CLUT index is recoverable from the pixel).
cases.txt, one case per line: `id rx ry rw rh word word ...`: the test rectangle (in VRAM) and
the chain's GP0 words (hex, tags left out) from E1 on. req.txt has four calls per case.
"""
import argparse
import os
import random
import struct

BUF = 0x801E0000
AREA = (0, 256, 256, 48)  # x, y, w, h of the test area in VRAM
FILL = 0x7FFF
TPAGE_A = 10 | (1 << 7)  # x 640, y 0, 8-bit
TPAGE_B = 12 | (1 << 7)  # x 768
CLUT = (640 // 16) | (300 << 6)  # CLUT word (x/16 in bits 0-5, y in bits 6-14)

M32 = 0xFFFFFFFF


def w_xy(x, y):
    return ((y & 0xFFFF) << 16) | (x & 0xFFFF)


def chain_bytes(packets, base):
    """GP0 packets (lists of words) as an ordered-table chain at BUF + base."""
    out = b""
    addr = base
    for i, p in enumerate(packets):
        nxt = 0xFFFFFF if i == len(packets) - 1 else (BUF + addr + 4 + 4 * len(p)) & 0xFFFFFF
        out += struct.pack("<I", (len(p) << 24) | nxt)
        out += b"".join(struct.pack("<I", w & M32) for w in p)
        addr += 4 + 4 * len(p)
    return out


def hexb(b):
    return b.hex()


class Gen:
    def __init__(self, seed):
        self.r = random.Random(seed)
        self.req = []
        self.cases = []

    # ---- request lines ----
    def draw(self, packets):
        data = chain_bytes(packets, 0)
        assert len(data) <= 0x8000, len(data)
        self.req.append("DrawOTag B+0 0 0 0 in=B+0:%s" % hexb(data))
        self.req.append("DrawSync 0 0 0 0")

    def setup(self):
        def rows(page_x, gen):
            # 256 rows of 128 halfwords; chunks of 3 rows per packet, as many packets as fit
            packets = []
            size = 0
            for y0 in range(0, 256, 3):
                n = min(3, 256 - y0)
                words = [0xA0000000, w_xy(page_x, y0), w_xy(128, n)]
                for yy in range(y0, y0 + n):
                    words += gen(yy)
                packets.append(words)
                size += 4 * (len(words) + 1)
                if size > 0x7000 or y0 + n >= 256:
                    self.draw(packets)
                    packets, size = [], 0

        # texel(u, v) = u: byte pairs (2k, 2k+1)
        rows(640, lambda y: [(((2 * k + 1) << 8 | 2 * k) | ((2 * k + 3) << 24 | (2 * k + 2) << 16)) for k in range(0, 128, 2)])
        rows(768, lambda y: [y | y << 8 | y << 16 | y << 24] * 64)
        clut = [0xA0000000, w_xy(640, 300), w_xy(256, 1)]
        for i in range(0, 256, 2):
            c = lambda i: (i & 31) | ((i >> 5) << 5) | (1 << 10)
            clut.append(c(i) | c(i + 1) << 16)
        self.draw([clut])

    def case(self, rect, words):
        """words: the chain's GP0 words, as one list per packet."""
        rx, ry, rw, rh = rect
        assert rw * rh % 2 == 0 and rw * rh * 2 <= 0x7000
        i = len(self.cases)
        flat = [w & M32 for p in words for w in p]
        self.cases.append("%d %d %d %d %d %s" % (i, rx, ry, rw, rh, " ".join("%x" % w for w in flat)))
        self.draw(words)
        self.req.append(
            "StoreImage B+8000 B+8010 0 0 in=B+8000:%s" % hexb(struct.pack("<4h", rx, ry, rw, rh))
        )
        self.req.append("DrawSync 0 0 0 0 out=B+8010:%x" % (rw * rh * 2))

    # ---- case construction ----
    def prelude(self, tpage, dither, clip, ofs):
        """E1 draw mode, E2 window off, E3/E4 draw area (inclusive, VRAM coordinates), E5
        offset, then the fill of the test area."""
        x0, y0, x1, y1 = clip
        ax, ay, aw, ah = AREA
        return [
            [0xE1000000 | (tpage & 0x9FF) | (dither << 9)],
            [0xE2000000],
            [0xE3000000 | x0 | (y0 << 10)],
            [0xE4000000 | x1 | (y1 << 10)],
            [0xE5000000 | (ofs[0] & 0x7FF) | ((ofs[1] & 0x7FF) << 11)],
            [0x02000000 | ((FILL & 31) << 3) | (((FILL >> 5) & 31) << 11) | (((FILL >> 10) & 31) << 19),
             w_xy(ax, ay), w_xy(aw, ah)],
        ]

    @staticmethod
    def poly(kind, verts, cols, uvs, page, raw):
        """kind 'F3' 'F4' 'G3' 'G4' 'FT3' 'FT4' 'GT3' 'GT4'. verts: (x, y) relative to the
        draw offset."""
        quad = kind.endswith("4")
        tex = "T" in kind
        gour = kind[0] == "G"
        op = 0x20 | (0x10 if gour else 0) | (0x08 if quad else 0) | (0x04 if tex else 0) | (1 if tex and raw else 0)
        n = 4 if quad else 3
        words = []
        for k in range(n):
            c = cols[k] if gour else cols[0]
            if k == 0:
                words.append((op << 24) | c)
            elif gour:
                words.append(c)
            words.append(w_xy(*verts[k]))
            if tex:
                u, v = uvs[k]
                extra = CLUT << 16 if k == 0 else (page << 16 if k == 1 else 0)
                words.append(u | v << 8 | extra)
        return words

    def poly_case(self, kind, verts, cols, uvs, page, raw, dither, clip, ofs, rect=None):
        ax, ay, aw, ah = AREA
        if rect is None:
            rx0, ry0, rx1, ry1 = clip
            rx0, ry0 = max(ax, rx0 - 2), max(ay, ry0 - 2)
            rx1, ry1 = min(ax + aw - 1, rx1 + 2), min(ay + ah - 1, ry1 + 2)
            w = rx1 - rx0 + 1
            if w % 2:
                w += 1 if rx0 + w < ax + aw else 0
                if w % 2:
                    rx0 -= 1
                    w += 1
            rect = (rx0, ry0, w, ry1 - ry0 + 1)
        prim = self.poly(kind, verts, cols, uvs, page, raw)
        # a textured polygon's own tpage word sets the draw-mode page
        self.case(rect, self.prelude(page if "T" in kind else 0, dither, clip, ofs) + [prim])

    # ---- generators ----
    def rand_clip(self, full=False):
        ax, ay, aw, ah = AREA
        if full or self.r.random() < 0.5:
            return (ax, ay, ax + aw - 1, ay + ah - 1)
        x0 = self.r.randint(ax, ax + aw - 8)
        y0 = self.r.randint(ay, ay + ah - 8)
        x1 = self.r.randint(x0 + 2, min(ax + aw - 1, x0 + 120))
        y1 = self.r.randint(y0 + 2, min(ay + ah - 1, y0 + 40))
        return (x0, y0, x1, y1)

    def rand_case(self):
        r = self.r
        kind = r.choice(["F3", "F4", "G3", "G4", "FT3", "FT4", "GT3", "GT4"] * 2 + ["GT3", "GT4", "FT3", "FT4", "G3", "G4"])
        big = r.random() < 0.15
        span = r.choice([8, 16, 32, 64, 120]) if not big else r.choice([200, 255])
        clip = self.rand_clip(full=big)
        ofs = (r.randint(-100, 300), r.randint(-100, 300))
        # place the polygon around a point inside the clip (relative to the offset)
        cx = r.randint(clip[0], clip[2]) - ofs[0]
        cy = r.randint(clip[1], clip[3]) - ofs[1]
        n = 4 if kind.endswith("4") else 3
        sx = span
        sy = max(4, min(span, 60))
        verts = [(cx + r.randint(-sx, sx), cy + r.randint(-sy, sy)) for _ in range(n)]
        cols = [r.randint(0, 255) | r.randint(0, 255) << 8 | r.randint(0, 255) << 16 for _ in range(n)]
        if r.random() < 0.2:
            cols = [0x80 | 0x80 << 8 | 0x80 << 16] * n
        us = r.choice([32, 64, 255, 255])
        uvs = [(r.randint(0, 255), r.randint(0, 255)) for _ in range(n)]
        if r.random() < 0.3:
            # modest texture gradients, as sprites use
            u0, v0 = r.randint(0, 255 - us), r.randint(0, 255 - us)
            uvs = [(u0 + r.randint(0, us), v0 + r.randint(0, us)) for _ in range(n)]
        page = r.choice([TPAGE_A, TPAGE_B])
        raw = r.random() < 0.7
        self.poly_case(kind, verts, cols, uvs, page, raw, r.randint(0, 1), clip, ofs)

    def hand(self):
        ax, ay, aw, ah = AREA
        full = (ax, ay, ax + aw - 1, ay + ah - 1)
        # 1. the landing screen's FT4: x -160..96 at offset 160, u 0..255, 8-bit texture, 16 rows
        for page in (TPAGE_A, TPAGE_B):
            for v1 in (15, 240, 255):
                verts = [(-160, 0), (96, 0), (-160, 16), (96, 16)]
                uvs = [(0, 0), (255, 0), (0, v1), (255, v1)]
                self.poly_case("FT4", verts, [0x808080] * 4, uvs, page, True, 0, (0, 256, 255, 271), (160, 256))
                self.poly_case("FT4", verts, [0x808080] * 4, uvs, page, False, 1, (0, 256, 255, 271), (160, 256))
        # widths and u spans: right triangles with the gradient along x, both windings
        for w in (1, 2, 3, 5, 7, 8, 13, 16, 31, 64, 100, 127, 128, 200, 255):
            for u1 in (1, 2, 7, 16, 100, 128, 255):
                for flip in (0, 1):
                    verts = [(0, 0), (w, 0), (0, 8)]
                    uvs = [(0, 0), (u1, 0), (0, 0)]
                    if flip:
                        verts = [verts[0], verts[2], verts[1]]
                        uvs = [uvs[0], uvs[2], uvs[1]]
                    self.poly_case("FT3", verts, [0] * 3, uvs, TPAGE_A, True, 0, full, (4, 256 + 4))
        # the same going down (v along y) and with a negative slope (u falls from the left)
        for h in (3, 8, 16, 33, 60):
            for v1 in (5, 60, 128, 255):
                verts = [(0, 0), (6, 0), (0, h)]
                self.poly_case("FT3", verts, [0] * 3, [(0, 0), (0, 0), (0, v1)], TPAGE_B, True, 0, full, (4, 256 + 1))
                self.poly_case("FT3", verts, [0] * 3, [(0, v1), (0, v1), (0, 0)], TPAGE_B, True, 0, full, (4, 256 + 1))
        for w in (7, 40, 100, 255):
            self.poly_case("FT3", [(0, 0), (w, 0), (0, 8)], [0] * 3, [(255, 0), (0, 0), (255, 0)], TPAGE_A, True, 0, full, (0, 256))
        # Gouraud: ramps, dither on and off, both windings and the quad split
        for w in (4, 8, 16, 32, 64, 100, 200, 255):
            for c1 in (0xFF, 0x80, 0x10, 0x01):
                for d in (0, 1):
                    cols = [0x000000, c1 | c1 << 8 | c1 << 16, 0x000000]
                    self.poly_case("G3", [(0, 0), (w, 0), (0, 8)], cols, [(0, 0)] * 3, 0, False, d, full, (0, 256))
        for d in (0, 1):
            for flip in (0, 1):
                v = [(0, 0), (60, 0), (0, 20), (60, 20)]
                c = [0x000000, 0x00A000, 0x400000, 0xFF8040]
                if flip:
                    v = [v[1], v[0], v[3], v[2]]
                self.poly_case("G4", v, c, [(0, 0)] * 4, 0, False, d, full, (3, 256 + 3))
        # the menu's dark blue box: a vertical blue gradient
        for d in (0, 1):
            for (b0, b1) in ((0x20, 0x60), (0x00, 0x40), (0x30, 0x58), (0x18, 0x4C)):
                cols = [b0 << 16, b0 << 16, b1 << 16, b1 << 16]
                self.poly_case("G4", [(0, 0), (60, 0), (0, 40), (60, 40)], cols, [(0, 0)] * 4, 0, False, d, full, (2, 256 + 2))
        # clip at the left and top edges
        for dx in range(0, 12):
            clip = (8 + dx, 256 + 4, 100, 256 + 30)
            self.poly_case("FT4", [(0, 0), (90, 0), (0, 30), (90, 30)], [0] * 4,
                           [(0, 0), (255, 0), (0, 255), (255, 255)], TPAGE_A, True, 0, clip, (4, 256))
            self.poly_case("FT4", [(0, 0), (90, 0), (0, 30), (90, 30)], [0] * 4,
                           [(0, 0), (255, 0), (0, 255), (255, 255)], TPAGE_B, True, 0, clip, (4, 256))
            self.poly_case("G3", [(0, 0), (90, 0), (0, 30)], [0xFF, 0xFF00, 0xFF0000], [(0, 0)] * 3, 0, False, 1,
                           clip, (4, 256))
        # tiny and degenerate triangles; edges on pixel centres
        for k in range(60):
            v = [(self.r.randint(0, 6), self.r.randint(0, 6)) for _ in range(3)]
            uvs = [(self.r.randint(0, 255), self.r.randint(0, 255)) for _ in range(3)]
            self.poly_case("FT3", v, [0] * 3, uvs, TPAGE_A, True, 0, (0, 256, 15, 271), (4, 260))
            self.poly_case("GT3", v, [0xFF, 0xFF00, 0xFF0000], uvs, TPAGE_B, False, 1, (0, 256, 15, 271), (4, 260))
            self.poly_case("F3", v, [0x808080] * 3, uvs, 0, False, 0, (0, 256, 15, 271), (4, 260))

    def lines_rects(self):
        r = self.r
        ax, ay, aw, ah = AREA
        for _ in range(60):
            clip = self.rand_clip()
            ofs = (r.randint(-50, 200), r.randint(-50, 200))
            pts = [(r.randint(-20, 130), r.randint(-20, 50)) for _ in range(2)]
            gour = r.random() < 0.6
            cols = [r.randint(0, 0xFFFFFF) for _ in range(2)]
            op = 0x40 | (0x10 if gour else 0)
            prim = [(op << 24) | cols[0], w_xy(*pts[0])] + ([cols[1]] if gour else []) + [w_xy(*pts[1])]
            rect = (ax, ay, 128, ah)
            self.case(rect, self.prelude(0, r.randint(0, 1), clip, ofs) + [prim])
        for _ in range(40):
            clip = self.rand_clip()
            ofs = (r.randint(-50, 200), r.randint(-50, 200))
            tex = r.random() < 0.6
            size = r.choice([0, 1, 2, 3])
            x, y = r.randint(-10, 120), r.randint(-10, 40)
            op = 0x60 | (size << 3) | (0x04 if tex else 0) | (1 if tex and r.random() < 0.5 else 0)
            words = [(op << 24) | r.randint(0, 0xFFFFFF), w_xy(x, y)]
            if tex:
                words.append(r.randint(0, 255) | r.randint(0, 255) << 8 | CLUT << 16)
            if size == 0:
                words.append(w_xy(r.randint(1, 80), r.randint(1, 30)))
            page = r.choice([TPAGE_A, TPAGE_B])
            self.case((ax, ay, 128, ah), self.prelude(page, 0, clip, ofs) + [words])


    def perms(self, count):
        """Triangles whose attributes are small integers over big spans (so many pixels sit
        exactly on a half), each drawn with all six vertex orders: shows which vertex the
        hardware measures from, and whether the order matters at all."""
        import itertools
        r = self.r
        ax, ay, aw, ah = AREA
        full = (ax, ay, ax + aw - 1, ay + ah - 1)
        for n in range(count):
            w = r.choice([20, 30, 40, 60, 90, 120])
            h = r.choice([10, 16, 24, 30, 40])
            v = [(r.randint(0, w), r.randint(0, h // 3)), (r.randint(0, w), r.randint(h // 3, h)),
                 (r.randint(0, w), r.randint(0, h))]
            kind = n % 3
            if kind == 0:
                us = [(r.randint(0, 6), r.randint(0, 6)) for _ in range(3)]
                page = TPAGE_A if n % 2 else TPAGE_B
            else:
                cs = [4 * r.randint(0, 6) for _ in range(3)]
                cols = [c | r.choice([0, c]) << 8 | c << 16 for c in cs]
            for order in itertools.permutations(range(3)):
                vv = [v[k] for k in order]
                if kind == 0:
                    self.poly_case("FT3", vv, [0] * 3, [us[k] for k in order], page, True, 0, full, (4, 260))
                else:
                    self.poly_case("G3", vv, [cols[k] for k in order], [(0, 0)] * 3, 0, False, kind - 1, full, (4, 260))

    def modcases(self, count):
        """Textured polygons whose texture coordinate is the same at every vertex (one texel
        all over), so only the modulation by the interpolated colour is left to see."""
        r = self.r
        ax, ay, aw, ah = AREA
        full = (ax, ay, ax + aw - 1, ay + ah - 1)
        for n in range(count):
            kind = ["GT3", "GT4", "FT3"][n % 3]
            m = 4 if kind.endswith("4") else 3
            idx = r.randint(0, 255)
            uv = (idx, idx)
            verts = [(r.randint(0, 80), r.randint(0, 40)) for _ in range(m)]
            if r.random() < 0.5:
                cols = [r.randint(0, 255) | r.randint(0, 255) << 8 | r.randint(0, 255) << 16 for _ in range(m)]
            else:
                c0 = r.randint(0, 200)
                cols = [(c0 + r.randint(0, 50)) * 0x010101 for _ in range(m)]
            page = TPAGE_A if n % 2 else TPAGE_B
            self.poly_case(kind, verts, cols, [uv] * m, page, False, (n // 3) % 2, full, (4, 260))

    def sprites(self, count):
        """Axis-aligned quads like the game's sprites and menu boxes, up to 320 x 240 with
        texture spans of any size, seen through a window of the test area at a random offset."""
        r = self.r
        ax, ay, aw, ah = AREA
        full = (ax, ay, ax + aw - 1, ay + ah - 1)
        for n in range(count):
            w = r.choice([8, 16, 24, 32, 40, 64, 100, 128, 160, 200, 256, 300, 320])
            h = r.choice([8, 16, 24, 32, 48, 64, 100, 120, 160, 200, 240])
            x0 = r.randint(-w // 2, 8)
            y0 = r.randint(-h + 8, 0) if r.random() < 0.7 else 0
            verts = [(x0, y0), (x0 + w, y0), (x0, y0 + h), (x0 + w, y0 + h)]
            u0, v0 = r.randint(0, 255), r.randint(0, 255)
            if r.random() < 0.5:
                u0, v0 = 0, 0
            u1 = r.choice([255, r.randint(0, 255), min(255, u0 + w), r.randint(0, 255)])
            v1 = r.choice([240, 255, r.randint(0, 255), min(255, v0 + h)])
            kind = ["FT4", "FT4", "GT4", "G4"][n % 4]
            cols = [r.randint(0, 0xFFFFFF) for _ in range(4)]
            if kind == "FT4" and r.random() < 0.5:
                cols = [0x808080] * 4
            if kind == "G4":
                b0, b1 = r.randint(0, 120), r.randint(0, 120)
                cols = [b0 << 16, b0 << 16, b1 << 16, b1 << 16]
            uvs = [(u0, v0), (u1, v0), (u0, v1), (u1, v1)]
            # the window: the polygon's origin lands at (ox, oy) in VRAM so that different rows show
            ox = ax + r.randint(0, 40)
            oy = ay + r.randint(-60, 40)
            self.poly_case(kind, verts, cols, uvs, r.choice([TPAGE_A, TPAGE_B]), r.random() < 0.6, r.randint(0, 1), full, (ox, oy))

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--scale", type=int, default=1)
    a = ap.parse_args()
    g = Gen(a.seed)
    g.setup()
    g.hand()
    g.lines_rects()
    g.perms(150 * a.scale)
    g.modcases(300 * a.scale)
    g.sprites(500 * a.scale)
    for _ in range(3000 * a.scale):
        g.rand_case()
    os.makedirs(a.out, exist_ok=True)
    with open(os.path.join(a.out, "req.txt"), "w") as f:
        f.write("\n".join(g.req) + "\n")
    with open(os.path.join(a.out, "cases.txt"), "w") as f:
        f.write("\n".join(g.cases) + "\n")
    print("%d cases, %d requests" % (len(g.cases), len(g.req)))


if __name__ == "__main__":
    main()
