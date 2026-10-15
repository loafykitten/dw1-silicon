#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that exercises libgs's `GsSortObject4J` and the TMD
primitive handlers the game installs in `GsFCALL4` (`initializeGsTMDMap`), on synthetic
objects, for `crates/psyq/tests/libgs_tmd_oracle.rs` to compare against.

Usage: tmd_requests.py OUT_FILE [--seed N] [--scale K] [--set fast|div|attr|explore]

Run the file on the emulator (a fresh empty directory under captures/oracle-runs/, e.g.
libgs-tmd-fast, holding req.txt; see oracle/call-sdk.lua), then keep req.txt and results.txt
there (gitignored: the results are Sony's code's outputs).

Every request builds, in call-sdk's scratch buffer (B = 0x801E0000):
- a `GsDOBJ2` at B+0 (attribute, coord2 0, the object record, id) and a `GsOT` at B+0x20;
- the object record at B+0x40 (vertex, normal and primitive addresses as `GsMapModelingData`
  leaves them, their counts, scale 0), vertices at B+0x100, normals at B+0x400;
- the primitives at B+0x800, grouped into runs as `GsLinkObject4` leaves them (the first header
  of a run of equal flag and mode carries the run's count in its low halfword);
- the ordering table's entries at B+0x2000 (1 << length of them, cleared as `GsClearOt` does:
  each links to the one before, the first to the end marker), followed by a guard area;
- the work base (`GsOUT_PACKET_P`) at B+0x3000 over a 0xA5-filled packet area, and the
  scratchpad filled with a pattern;
then calls `GsSortObject4J(obj, ot, shift, 0x1F800000)` (the scratch argument the game passes)
with all 64 GTE registers preset, and reads back the packet area, the table with its guard, the
work base, the object, the table header, the primitives (unchanged?), the scratchpad and the GTE.

The kinds are the ones the disc's TMDs use (a survey of all 82,411 primitives of every TMD on
the disc, including those inside other files: lit flat F3/F4, lit flat textured FT3/FT4, lit
Gouraud G4, lit Gouraud textured GT3/GT4, unlit flat textured FT3/FT4, unlit Gouraud textured
GT3/GT4), each with the semi-transparency and raw-texture bits varied, mixed in one object.

Sets:
- `fast`: attribute 0 (the non-dividing handlers), random lighting, matrices, positions
  (including behind the eye, off screen and huge), shifts, tables and `GsOT` offsets.
- `div`: attribute 0x200 (`GsDIV1`, the arenas' floor) on the unlit textured kinds the arenas
  use (DIV handlers TNF3/TNF4/TNG3/TNG4), random as above.
- `attr`: display off (bit 31) with and without other bits.
- `bits`: each kind with and without the semi-transparency bit, one small object per request.
- `bounds`: the dividing handlers' screen-edge and near-depth tests at their edges.
- `explore`: a few simple objects (identity rotation, one primitive) to see the shape of the
  output while implementing.
"""
import argparse
import random
import struct

BUF = 0x801E0000
OBJ, OT, REC, VERTS, NORMS, PRIMS, ORG, PACKETS = 0x0, 0x20, 0x40, 0x100, 0x400, 0x800, 0x2000, 0x3000
OT_GUARD = 0x100
SCRATCHPAD = 0x1F800000
G_OUT_PACKET_P = 0x80136FE4
G_FCALL4 = 0x80137000
# libgs's globals from HWD0 to GsTON (read back: which the sort writes).
G_LIBGS = 0x80136F10
M32 = 0xFFFFFFFF

# kind: (flag, base mode, vertices, lit, textured, gouraud, olen)
KINDS = {
    'F3L': (0, 0x20, 3, True, False, False, 4),
    'F4L': (0, 0x28, 4, True, False, False, 5),
    'G4L': (0, 0x38, 4, True, False, True, 8),
    'TF3L': (0, 0x24, 3, True, True, False, 7),
    'TF4L': (0, 0x2C, 4, True, True, False, 9),
    'TG3L': (0, 0x34, 3, True, True, True, 9),
    'TG4L': (0, 0x3C, 4, True, True, True, 12),
    'TNF3': (1, 0x25, 3, False, True, False, 7),
    'TNF4': (1, 0x2D, 4, False, True, False, 9),
    'TNG3': (1, 0x35, 3, False, True, True, 9),
    'TNG4': (1, 0x3D, 4, False, True, True, 12),
}
# The GPU packet's words after the tag, per kind.
GPU_WORDS = {'F3L': 4, 'F4L': 5, 'G4L': 8, 'TF3L': 7, 'TF4L': 9, 'TG3L': 9, 'TG4L': 12,
             'TNF3': 7, 'TNF4': 9, 'TNG3': 9, 'TNG4': 12}
DIV_KINDS = ['TNF3', 'TNF4', 'TNG3', 'TNG4']


def body_words(flag, mode):
    """TMD packet words after the header (as crates/psyq's primitive_words)."""
    quad, tex, gour = mode & 8, mode & 4, mode & 0x10
    lit = flag & 1 == 0
    n = 4 if quad else 3
    w = 0 if (lit and tex) else 1
    if tex:
        w += n
    halves = n
    if lit:
        halves += n if gour else 1
    elif gour:
        w += n - 1
    return w + (halves + 1) // 2


class Gen:
    def __init__(self, seed):
        self.r = random.Random(seed)

    def s16(self, lo, hi):
        return self.r.randint(lo, hi)

    def rotation(self):
        """A rotation matrix (4.12) from three random angles, sometimes scaled or arbitrary."""
        import math
        r = self.r
        k = r.random()
        if k < 0.1:
            return [r.randint(-0x8000, 0x7FFF) for _ in range(9)]
        a, b, c = (r.uniform(-math.pi, math.pi) for _ in range(3))
        ca, sa, cb, sb, cc, sc = math.cos(a), math.sin(a), math.cos(b), math.sin(b), math.cos(c), math.sin(c)
        m = [cb * cc, -cb * sc, sb,
             sa * sb * cc + ca * sc, -sa * sb * sc + ca * cc, -sa * cb,
             -ca * sb * cc + sa * sc, ca * sb * sc + sa * cc, ca * cb]
        s = 4096 * (r.choice([1, 1, 1, 0.5, 2, r.uniform(0.2, 4)]))
        return [max(-0x8000, min(0x7FFF, int(round(x * s)))) for x in m]

    def gte(self, tame):
        r = self.r
        m = self.rotation()
        if tame:
            tz = r.randint(300, 4000)
            tx, ty = r.randint(-300, 300), r.randint(-250, 250)
        else:
            tz = r.choice([r.randint(-2000, 20000), r.randint(-100, 300), r.randint(0, 0x10000),
                           r.randint(-0x80000, 0x80000)])
            tx, ty = (r.choice([r.randint(-1000, 1000), r.randint(-0x8000, 0x8000)]) for _ in range(2))
        light = [r.randint(-0x1000, 0x1000) for _ in range(9)]
        lcm = r.choice([[0xC00, 0, 0, 0xC00, 0, 0, 0xC00, 0, 0],
                        [r.randint(0, 0x1000) for _ in range(9)],
                        [r.randint(-0x8000, 0x7FFF) for _ in range(9)]])
        bk = r.choice([[0x800] * 3, [r.randint(0, 0x1000) for _ in range(3)],
                       [r.randint(-0x10000, 0x10000) for _ in range(3)]])
        fc = r.choice([[0, 0, 0], [r.randint(0, 0x1000) for _ in range(3)]])
        ofx, ofy = r.choice([(0, 0), (160 << 16, 120 << 16), (r.randint(-0x1000000, 0x1000000),) * 2])
        h = r.choice([0x3E8, 0x5E7, 0x715, 0x200, r.randint(100, 2000)])
        dqa, dqb = r.choice([(0xFFFFEF9E, 0x01400000), (r.getrandbits(16), r.getrandbits(32))])
        zsf3, zsf4 = r.choice([(0x155, 0x100), (0x155, 0x100), (r.randint(0, 0x1000), r.randint(0, 0x1000))])

        def pair(a, b):
            return (a & 0xFFFF) | (b & 0xFFFF) << 16

        def mat(v):
            return [pair(v[0], v[1]), pair(v[2], v[3]), pair(v[4], v[5]), pair(v[6], v[7]), v[8] & 0xFFFF]
        c = mat(m) + [tx & M32, ty & M32, tz & M32] + mat(light) + [x & M32 for x in bk] + mat(lcm) + \
            [x & M32 for x in fc] + [ofx & M32, ofy & M32, h, dqa & 0xFFFF, dqb & M32, zsf3, zsf4, 0]
        # Data registers: junk (working storage), but the ones whose writes have side effects in
        # the native GTE (SXYP, IRGB, ORGB, LZCR) are left alone.
        d = []
        for i in range(32):
            d.append('x' if i in (15, 28, 29, 31) else '%x' % r.getrandbits(32))
        return ','.join(d + ['%x' % (x & M32) for x in c])

    def vertex(self, tame):
        r = self.r
        if tame or r.random() < 0.8:
            return [r.randint(-400, 400) for _ in range(3)]
        return [r.randint(-0x8000, 0x7FFF) for _ in range(3)]

    def normal(self):
        import math
        r = self.r
        if r.random() < 0.05:
            return [r.randint(-0x8000, 0x7FFF) for _ in range(3)]
        x, y, z = (r.gauss(0, 1) for _ in range(3))
        n = math.sqrt(x * x + y * y + z * z) or 1
        return [int(round(4096 * v / n)) for v in (x, y, z)]

    def tex_word(self):
        r = self.r
        return r.choice([r.getrandbits(32), r.getrandbits(16) | (r.randint(0, 0x7FFF) << 16)])

    def colour(self, mode):
        r = self.r
        rgb = r.choice([0x808080, r.getrandbits(24), 0xFFFFFF, 0])
        hi = r.choice([0, mode, r.getrandbits(8)])
        return rgb | hi << 24

    def primitive(self, kind, nv_list, nn_list, mode_bits):
        flag, mode, nv, lit, tex, gour, olen = KINDS[kind]
        mode |= mode_bits
        r = self.r
        verts = [r.randrange(nv_list) for _ in range(nv)]
        norms = [r.randrange(nn_list) for _ in range(nv)]
        halves = []
        words = []
        if tex:
            words += [self.tex_word() for _ in range(nv)]
        if not (lit and tex):
            words.append(self.colour(mode))
        if not lit and gour:
            words += [self.colour(mode) for _ in range(nv - 1)]
        if lit:
            if gour:
                for k in range(nv):
                    halves += [norms[k], verts[k]]
            else:
                halves += [norms[0]] + verts
        else:
            halves += verts
        if len(halves) % 2:
            halves.append(r.choice([0, 0, 0, r.getrandbits(16)]))
        for k in range(0, len(halves), 2):
            words.append(halves[k] | halves[k + 1] << 16)
        ilen = body_words(flag, mode)
        assert len(words) == ilen, (kind, len(words), ilen)
        return flag, mode, olen, words

    def obj(self, kinds, count, tame, attribute, mode_bits_for):
        r = self.r
        nv_list, nn_list = r.randint(4, 40), r.randint(2, 40)
        verts = [self.vertex(tame) for _ in range(nv_list)]
        norms = [self.normal() for _ in range(nn_list)]
        # Runs: a sequence of kinds, each repeated a few times.
        prims = []
        while len(prims) < count:
            kind = r.choice(kinds)
            for _ in range(r.choice([1, 1, 2, 3, 5])):
                if len(prims) < count:
                    prims.append((kind, self.primitive(kind, nv_list, nn_list, mode_bits_for(kind))))
        # Link: the first header of each run of equal (flag, mode) holds the run's count.
        out = []
        i = 0
        gpu_words = 0
        while i < len(prims):
            fm = prims[i][1][:2]
            j = i
            while j < len(prims) and prims[j][1][:2] == fm:
                j += 1
            for k in range(i, j):
                flag, mode, olen, words = prims[k][1]
                ilen = len(words)
                low = (j - i) if k == i else (olen | ilen << 8)
                out.append(mode << 24 | flag << 16 | low)
                out += words
                gpu_words += 1 + GPU_WORDS[prims[k][0]]
            i = j
        return verts, norms, out, len(prims), gpu_words

    def request(self, verts, norms, prim_words, n_prim, gpu_words, attribute, shift, ot_len, gte, factor,
                offset=0, point=0, scratch=True, extra_ins=()):
        r = self.r
        n_ot = 1 << ot_len
        obj = struct.pack('<IIII', attribute, 0, BUF + REC, r.getrandbits(32))
        ot = struct.pack('<IIIII', ot_len, BUF + ORG, offset, point, BUF + ORG + 4 * (n_ot - 1))
        rec = struct.pack('<IIIIIIi', BUF + VERTS, len(verts), BUF + NORMS, len(norms), BUF + PRIMS, n_prim, 0)
        vb = b''.join(struct.pack('<4h', *v, 0) for v in verts)
        nb = b''.join(struct.pack('<4h', *n, 0) for n in norms)
        pb = struct.pack('<%dI' % len(prim_words), *prim_words)
        # GsClearOt: each entry links to the one before; the first ends the list.
        org = [0x00FFFFFF] + [(BUF + ORG + 4 * (i - 1)) & 0xFFFFFF for i in range(1, n_ot)]
        ob = struct.pack('<%dI' % n_ot, *org) + b'\x5a' * OT_GUARD
        pk_len = min(0xA000, 4 * gpu_words * factor + 0x40)
        assert len(vb) <= NORMS - VERTS and len(nb) <= PRIMS - NORMS and len(pb) <= ORG - PRIMS
        assert len(ob) <= PACKETS - ORG
        ins = [
            ('B+%x' % OBJ, obj.hex()), ('B+%x' % OT, ot.hex()), ('B+%x' % REC, rec.hex()),
            ('B+%x' % VERTS, vb.hex()), ('B+%x' % NORMS, nb.hex()), ('B+%x' % PRIMS, pb.hex()),
            ('B+%x' % ORG, ob.hex()), ('B+%x' % PACKETS, 'a5' * pk_len),
            ('%x' % G_OUT_PACKET_P, struct.pack('<I', BUF + PACKETS).hex()),
        ]
        ins += list(extra_ins)
        outs = [('B+%x' % PACKETS, pk_len), ('B+%x' % ORG, len(ob)), ('%x' % G_OUT_PACKET_P, 4),
                ('B+%x' % OBJ, 16), ('B+%x' % OT, 20), ('B+%x' % PRIMS, len(pb)), ('%x' % G_LIBGS, 0xF0)]
        if scratch:
            pattern = bytes((i * 7 + 3) & 0xFF for i in range(1024))
            ins.append(('%x' % SCRATCHPAD, pattern.hex()))
            outs.append(('%x' % SCRATCHPAD, 1024))
        parts = ['GsSortObject4J', 'B+%x' % OBJ, 'B+%x' % OT, '%x' % shift, '%x' % SCRATCHPAD]
        parts += ['in=%s:%s' % i for i in ins]
        parts += ['out=%s:%x' % o for o in outs]
        parts += ['gtein=' + gte, 'gteout']
        return ' '.join(parts)

    def bits(self, base_kinds, variants):
        """A mode-bit chooser: per kind, the semi-transparency bit (2) as the disc has it, or
        (`variants`) either way. The raw-texture bit (1) is as on the disc: set on every unlit
        primitive, clear on every lit one; the other way round sends the real GsSortObject4J
        off into the weeds (it runs away or faults: `--set bits` with it varied, 2026-10-15)."""
        r = self.r

        def choose(kind):
            flag = KINDS[kind][0]
            disc = [2, 0, 0] if flag == 0 else [0, 0, 2]
            semi = r.choice([0, 2]) if variants and r.random() < 0.25 else r.choice(disc)
            return semi
        return choose

    def shift_and_len(self, tame):
        r = self.r
        if tame:
            # OTZ is about z/4 with the game's ZSF3/ZSF4: keep the index in the table.
            return r.choice([(2, 8), (4, 8), (8, 8), (12, 2), (r.randint(6, 12), 8)])
        # Past the table's end only as far as the guard area reaches (OTZ is 16 bits).
        return r.choice([(r.randint(8, 16), 8), (12, 2), (r.randint(10, 16), 4)])


def gen_fast(seed, scale):
    g = Gen(seed)
    r = g.r
    lines = []
    kinds = list(KINDS)
    for i in range(1500 * scale):
        tame = r.random() < 0.7
        ks = r.sample(kinds, r.choice([1, 1, 2, 3, len(kinds)]))
        verts, norms, prims, n, gw = g.obj(ks, r.randint(1, 24), tame, 0, g.bits(ks, i % 3 == 0))
        shift, ot_len = g.shift_and_len(tame)
        # GsOT.offset is subtracted from OTZ: mostly 0 (the game's), sometimes small, rarely any.
        off = r.choice([0] * 8 + [r.randint(0, 400), r.getrandbits(16)])
        pt = r.choice([0, 0, r.getrandbits(16)])
        lines.append(g.request(verts, norms, prims, n, gw, 0, shift, ot_len, g.gte(tame), 1, off, pt,
                               scratch=i % 10 == 0))
    return lines


def gen_div(seed, scale):
    g = Gen(seed)
    r = g.r
    lines = []
    for i in range(800 * scale):
        tame = r.random() < 0.8
        # GsOT.offset 0 or small: an OTZ below the offset sends the dividing handlers' writes far
        # past the table, which can take the emulated machine down.
        off = r.choice([0] * 9 + [r.randint(0, 64)])
        ks = r.sample(DIV_KINDS, r.choice([1, 1, 2, 4]))
        verts, norms, prims, n, gw = g.obj(ks, r.randint(1, 12), tame, 0x200, g.bits(ks, i % 3 == 0))
        shift, ot_len = g.shift_and_len(tame)
        lines.append(g.request(verts, norms, prims, n, gw, 0x200, shift, ot_len, g.gte(tame), 4, off))
    return lines


def gen_attr(seed, scale):
    g = Gen(seed)
    r = g.r
    lines = []
    # The game's objects have attribute 0, 0x200 or 0x80000000 (display off: the champion's
    # models at first). Other bits with display off too; the other bits alone (divide levels
    # past 1, light modes, the semi-transparency rate) make the real SDK run away on some kinds
    # and the game never sets them.
    attrs = [0x80000000, 0x80000200, 0x80000040, 0xC0000000, 0x80000400, 0x80000008, 0xFFFFFFFF,
             0x8FFFFFFF]
    for attribute in attrs:
        for _ in range(4 * scale):
            ks = r.sample(list(KINDS), 3)
            verts, norms, prims, n, gw = g.obj(ks, r.randint(1, 8), True, attribute, g.bits(ks, False))
            shift, ot_len = g.shift_and_len(True)
            lines.append(g.request(verts, norms, prims, n, gw, attribute, shift, ot_len, g.gte(True), 32))
    return lines


def gen_bounds(seed, scale):
    """The dividing handlers' screen and near tests at their edges: identity rotation, H = 1000,
    z = 1000 (screen = local + offset exactly), small front-facing squares and triangles placed
    one pixel either side of each screen edge, under several screen offsets and libgs screen
    sizes (HWD0, VWD0); then depths either side of H/2."""
    g = Gen(seed)
    r = g.r
    lines = []

    def gte(ofx, ofy, tz, h=1000):
        c = [0x1000, 0, 0x1000, 0, 0x1000, 0, 0, tz] + [0, 0, 0, 0, 0] + [0x800] * 3 + \
            [0x1000, 0, 0x1000, 0, 0x1000] + [0, 0, 0] + [ofx & M32, ofy & M32, h, 0xEF9E, 0x01400000, 0x155,
                                                           0x100, 0]
        return ','.join(['0'] * 15 + ['x'] + ['0'] * 12 + ['x'] * 3 + ['0'] + ['%x' % x for x in c])

    def prim(kind, n):
        flag, mode, nv, lit, tex, gour, olen = KINDS[kind]
        mode |= r.choice([0, 2])
        words = [g.tex_word() for _ in range(nv)]
        words += [g.colour(mode) for _ in range(nv if gour else 1)]
        idx = list(range(nv)) + ([0] if nv == 3 else [])
        words += [idx[k] | idx[k + 1] << 16 for k in range(0, len(idx), 2)]
        return [mode << 24 | flag << 16 | 1] + words, 1 + GPU_WORDS[kind]

    def add(kind, x0, y0, d, ofx=0, ofy=0, tz=1000, z=(0, 0, 0, 0), size=None, h=1000):
        verts = [[x0, y0, z[0]], [x0 + d, y0, z[1]], [x0, y0 + d, z[2]], [x0 + d, y0 + d, z[3]]]
        p, gw = prim(kind, 1)
        extra = []
        if size:
            extra = [('%x' % G_LIBGS, struct.pack('<II', *size).hex())]
        lines.append(g.request(verts, [[0, 0, 4096]], p, 1, gw, 0x200, 2, 8, gte(ofx, ofy, tz, h), 4,
                               extra_ins=extra))

    for kind in DIV_KINDS:
        for d in (1, 5):
            for (ofx, ofy, w, h, size) in [(0, 0, 320, 240, None), (100, -50, 320, 240, None),
                                           (0, 0, 256, 200, (256, 200)), (37, 11, 320, 240, None)]:
                left, top = ofx - w // 2, ofy - h // 2
                right, bottom = left + w, top + h
                ys = top + h // 2
                xs = left + w // 2
                for edge in (-1, 0, 1):
                    add(kind, left - d + edge, ys, d, ofx << 16, ofy << 16, size=size)   # max x = left + edge
                    add(kind, right + edge, ys, d, ofx << 16, ofy << 16, size=size)      # min x = right + edge
                    add(kind, xs, top - d + edge, d, ofx << 16, ofy << 16, size=size)    # max y = top + edge
                    add(kind, xs, bottom + edge, d, ofx << 16, ofy << 16, size=size)     # min y = bottom + edge
            # Fractional screen offsets.
            for frac in (0x8000, 0xFFFF, 0x0001):
                for edge in (-1, 0, 1):
                    add(kind, -160 - d + edge, 0, d, frac, 0)
                    add(kind, 160 + edge, 0, d, -frac, 0)
        # Depths about H/2 (500): all equal, and one vertex either side.
        for sz in (498, 499, 500, 501, 502):
            add(kind, -10, -10, 20, tz=sz)
            add(kind, -10, -10, 20, tz=500, z=(sz - 500, 0, 0, 0))
            add(kind, -10, -10, 20, tz=500, z=(0, 0, 0, sz - 500))
        # And with an odd H (1001: is the depth compared with H/2 rounded down, or doubled?).
        for sz in (499, 500, 501):
            add(kind, -10, -10, 20, tz=sz, h=1001)
    return lines


def gen_explore(seed, scale):
    """Simple objects: identity rotation, z = 1000, H = 1000, the game's ZSF, one primitive (or a
    few), each kind; and the same through GsDIV1..2."""
    g = Gen(seed)
    r = g.r
    lines = []

    def gte_simple():
        c = [0x1000, 0, 0x1000, 0, 0x1000, 0, 0, 1000] + [0x1000, 0, 0x1000, 0, 0x1000] + [0x800] * 3 + \
            [0x1000, 0, 0x1000, 0, 0x1000] + [0, 0, 0] + [0, 0, 1000, 0xEF9E, 0x01400000, 0x155, 0x100, 0]
        return ','.join(['0'] * 15 + ['x'] + ['0'] * 12 + ['x'] * 3 + ['0'] + ['%x' % x for x in c])
    square = [[-100, -100, 0], [100, -100, 0], [-100, 100, 0], [100, 100, 0], [0, 0, 0]]
    norms = [[0, 0, -4096], [0, -4096, 0]]
    for kind in KINDS:
        flag, mode, nv, lit, tex, gour, olen = KINDS[kind]
        for attribute in (0, 0x200, 0x400) if kind in DIV_KINDS else (0,):
            p = g.primitive(kind, 5, 2, 0)
            # Fixed indices and tex words, so the output is easy to read.
            words = p[3]
            words = [w for w in words]
            hd = [flag, mode, olen]
            idx = [0, 1, 2, 3][:nv]
            halves = []
            if lit:
                halves = sum(([0, v] for v in idx), []) if gour else [0] + idx
            else:
                halves = idx[:]
            if len(halves) % 2:
                halves.append(0)
            iw = [halves[k] | halves[k + 1] << 16 for k in range(0, len(halves), 2)]
            tw = [0x7F000000 | 0x10 * k | 0x20 * k << 8 for k in range(nv)] if tex else []
            cw = []
            if not (lit and tex):
                cw.append(0x00402010 | mode << 24)
            if not lit and gour:
                cw += [0x00402010 + 0x10101 * k for k in range(1, nv)]
            words = tw + cw + iw
            prims = [mode << 24 | flag << 16 | 1] + words
            lines.append(g.request(square, norms, prims, 1, 1 + GPU_WORDS[kind], attribute, 2, 8, gte_simple(),
                                   64))
    # What the game installed in GsFCALL4, read along with the first request.
    lines[0] += ' out=%x:130' % G_FCALL4
    return lines


def gen_bits(seed, scale):
    """Each kind with and without the semi-transparency bit, one small object per request."""
    g = Gen(seed)
    r = g.r
    lines = []
    for kind in KINDS:
        for bits in (0, 2):
            for _ in range(2 * scale):
                verts, norms, prims, n, gw = g.obj([kind], r.randint(1, 4), True, 0, lambda k: bits)
                shift, ot_len = g.shift_and_len(True)
                lines.append(g.request(verts, norms, prims, n, gw, 0, shift, ot_len, g.gte(True), 1))
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--seed', type=int, default=20261015)
    ap.add_argument('--scale', type=int, default=1)
    ap.add_argument('--set', choices=['fast', 'div', 'attr', 'explore', 'bits', 'bounds'], default='fast')
    a = ap.parse_args()
    gen = {'fast': gen_fast, 'div': gen_div, 'attr': gen_attr, 'explore': gen_explore, 'bits': gen_bits,
           'bounds': gen_bounds}[a.set]
    lines = gen(a.seed, a.scale)
    with open(a.out, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(len(lines), 'requests')


if __name__ == '__main__':
    main()
