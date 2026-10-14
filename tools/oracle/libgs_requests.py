#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that exercises the libgs light, view and coordinate
functions (GsSetFlatLight, GsSetAmbient, GsSetLightMode, GsSetLightMatrix, GsSetLsMatrix,
GsSetView2, GsGetLw, GsGetLs, GsGetLws, GsMulCoord0, GsMulCoord3), for
`crates/psyq/tests/libgs_oracle.rs` to compare against.

Usage: libgs_requests.py OUT_FILE [--seed N] [--scale K]

Run the file on the emulator the way tools/oracle/gte_requests.py describes (one emulator at a
time; a fresh empty directory under captures/oracle-runs/, e.g. libgs-light, holding req.txt;
about 10 minutes for the default size), then keep req.txt and results.txt there (gitignored:
the results are Sony's code's outputs; `DW1_LIBGS_LIGHT_CAPTURE` points the test elsewhere).

Every request that touches libgs state first writes the globals it depends on (`in=` at the
PS1 addresses of GsLIGHTWSMATRIX, the light colour matrix _LC, GsWSMATRIX, GsWSMATRIX_ORG and
GsLIGHT_MODE, from decomp/config/us/symbols.txt) and records all of them afterwards, so the
Rust side starts each request from the same values and compares every byte it could have
touched. All 64 GTE registers are preset (`gtein`) and recorded (`gteout`) on every request.

GsCOORDINATE2 hierarchies live in the scratch buffer with the PS1's 32-bit pointers (0x50 bytes
each: flg, coord, workm, param, super, sub); the test builds native structs (8-byte pointers)
from the same values. Hierarchies are 1 to 4 deep (and a few deeper: 8, 32) with random
matrices, flags and stale `workm`, and sequences that call the functions again on a hierarchy
the previous request left cached, with one `flg` reset in the middle.
Inputs the real SDK has no usable answer for are left out: a GsSetFlatLight vector whose sum of
squares wraps negative (the square root reads before its table), a hierarchy deeper than 32
(runs over libgs's own globals). A `super` of SCREEN (1) faults the real code (CPU exception 4)
and is requested a few times: the Rust functions must stop the program there.
"""
import argparse
import itertools
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gte_requests import Gen, M32  # noqa: E402

BUF = 0x801E0000
G_PSDCNT = 0x80136E08
G_MODE, G_LW, G_LC, G_WS, G_ORG = 0x80136F1C, 0x80136F24, 0x80136F44, 0x80136F84, 0x80136FA4
GLOBAL_OUTS = [(G_PSDCNT, 4), (G_MODE, 4), (G_LW, 32), (G_LC, 32), (G_WS, 32), (G_ORG, 32)]
COORD0 = 0x1000  # first coordinate system, B+0x1000, 0x80 apart
VIEW = 0x200  # a GsVIEW2
MAT_A, MAT_B, MAT_C = 0x400, 0x500, 0x600
LIGHT = 0x300


def w32(x):
    x &= M32
    return x - (1 << 32) if x >> 31 else x


class Req(Gen):
    def hexs(self, v):
        return '%x' % (v & M32)

    def pad(self):
        return self.r.choice([0, 0, 0xFFFF, self.r.getrandbits(16)])

    def mat(self, kind=None, pad=None, t=None):
        m, tt = self.matrix(kind)
        if t is not None:
            tt = t
        pad = self.pad() if pad is None else pad
        return struct.pack('<9h', *m).hex() + '%04x' % pad + struct.pack('<3i', *tt).hex()

    def small_t(self):
        r = self.r
        return [r.randint(-3000, 3000) for _ in range(3)]

    def game_mat(self):
        """A rotation-ish matrix with a game-sized translation."""
        return self.mat('rot', t=self.small_t())

    @staticmethod
    def coord_hex(flg, coord, workm, sup, param=0, sub=0):
        return struct.pack('<I', flg).hex() + coord + workm + struct.pack('<3I', param, sup, sub).hex()

    stamp = 0

    def new_stamp(self):
        """A new value for libgs's swap counter PSDCNT, which coordinate systems are stamped with
        (GsGetLw writes it into `flg`, and a `flg` equal to it means something): returns the `in=`
        for it and makes `flg()` produce it often."""
        r = self.r
        self.stamp = r.choice([0, 1, 2, 0x16A, 0x200, 0x7FFFFFFF, 0xFFFFFFFF, r.getrandbits(16), r.getrandbits(16)])
        return (self.hexs(G_PSDCNT), struct.pack('<I', self.stamp).hex())

    def flg(self):
        s = self.stamp
        return self.r.choice([0, 0, 0, 1, 1, s, s, 2, 0x16A, 0x80, 0xFFFFFFFF, self.r.getrandbits(32)])

    def seeds(self, lw=True, lc=True, ws=True, org=True, mode=True):
        """`in=` for the libgs globals the request depends on."""
        r = self.r
        ins = []
        if lw:
            ins.append((self.hexs(G_LW), self.mat(r.choice(['rot', 'rot', 'any']))))
        if lc:
            ins.append((self.hexs(G_LC), self.mat(r.choice(['any', 'small']))))
        if ws:
            ins.append((self.hexs(G_WS), self.mat(r.choice(['rot', 'rot', 'any']), t=self.small_t())))
        if org:
            ins.append((self.hexs(G_ORG), self.mat('any')))
        if mode:
            ins.append((self.hexs(G_MODE), struct.pack('<I', r.choice([0, 1, 2, 3, 0, 1, 2, 3, 4, 0x7FFFFFFF, 0xFFFFFFFF])).hex()))
        return ins

    def emit(self, fn, args, ins=(), outs=(), extra_outs=()):
        o = [(self.hexs(a), n) for a, n in GLOBAL_OUTS] + list(outs) + list(extra_outs)
        self.add(fn, list(args), ins=list(ins), outs=o)

    def b(self, off):
        return 'B+%x' % off


def build_chain(g, depth, base=COORD0, flgs=None, stale=True, new_stamp=True):
    """`in=` entries for a chain of `depth` coordinate systems from leaf (base) to root, each
    0x80 apart; returns (ins, addresses of the nodes leaf first)."""
    r = g.r
    ins, addrs = [], []
    if flgs is None or new_stamp:
        ins.append(g.new_stamp())
    for k in range(depth):
        off = base + k * 0x80
        sup = BUF + off + 0x80 if k < depth - 1 else 0
        flg = g.flg() if flgs is None else flgs[k]
        if flg == 'S':
            flg = g.stamp
        coord = g.game_mat() if r.random() < 0.7 else g.mat()
        workm = g.mat() if stale else coord
        ins.append((g.b(off), g.coord_hex(flg, coord, workm, sup)))
        addrs.append(off)
    return ins, addrs


def coord_outs(g, offs):
    return [(g.b(o), 0x50) for o in offs]


def gen(seed, scale):
    g = Req(seed)
    r = g.r
    N = 300 * scale

    # The emulator's menu leaves libgs's globals at some values (the game's camera in GsWSMATRIX);
    # write them all once so every later request starts from known state.
    g.emit('GsSetAmbient', ['800', '800', '800'], ins=g.seeds() + [g.new_stamp()])

    # ---- GsSetFlatLight ----
    def light_bytes(v, c):
        return struct.pack('<3i3Bx', *v, *c).hex()

    def sum_ok(v):
        s = w32(w32(v[0] * v[0]) + w32(v[1] * v[1]) + w32(v[2] * v[2]))
        return s >= 0

    def vec():
        k = r.random()
        if k < 0.25:
            return [r.randint(-200, 200) for _ in range(3)]
        if k < 0.45:
            return [r.randint(-46340, 46340) for _ in range(3)]
        if k < 0.6:
            return [r.choice([0, 0, 1, -1, 100, -100, 4096, -4096, 46340, -46340, 30, 0x64]) for _ in range(3)]
        if k < 0.7:  # wrapping sums of squares
            return [r.randint(-100000, 100000) for _ in range(3)]
        if k < 0.78:  # near the 16-bit and 32-bit edges
            return [r.choice([0, 65536, -65536, 32768, 0x7FFFFFFF, -0x80000000, 1 << 20, 46341, 65535, 46340])
                    for _ in range(3)]
        if k < 0.9:  # game-like lights
            return [r.randint(-100, 100), r.randint(0, 120), r.randint(-100, 100)]
        return [r.randint(-(1 << r.choice([8, 16, 24, 31])), (1 << r.choice([8, 16, 24, 31])) - 1) for _ in range(3)]

    for _ in range(N * 6):
        v = vec()
        if not sum_ok(v):
            continue
        c = [r.choice([0, 255, r.randint(0, 255), r.randint(0, 255)]) for _ in range(3)]
        idv = r.choice([0, 1, 2, 0, 1, 2, 0, 1, 2, 3, 4, -1, 7, 255, 0x80000000, 0x7FFFFFFF, 1 << 16])
        g.emit('GsSetFlatLight', [g.hexs(idv), g.b(LIGHT)], ins=g.seeds(ws=False, org=False, mode=False) +
               [(g.b(LIGHT), light_bytes(v, c))], outs=[(g.b(LIGHT), 16)])
    # the game's own lights, in order, as it sets them
    for lights in ([(0x1E, 0x64, 0x1E, 0x40, 0x40, 0x40), (-0x1E, 0x64, 0, 0x28, 0x28, 0x28),
                    (0, 0x64, -0x1E, 0x26, 0x26, 0x26)],):
        for i, l in enumerate(lights):
            g.emit('GsSetFlatLight', [str(i), g.b(LIGHT)], ins=[(g.b(LIGHT), light_bytes(l[:3], l[3:]))],
                   outs=[(g.b(LIGHT), 16)])
    for c in range(256):
        g.emit('GsSetFlatLight', [str(c % 3), g.b(LIGHT)],
               ins=[(g.b(LIGHT), light_bytes([5, 9, -2], [c, 255 - c, (c * 7) & 255]))])
    # zero vectors and zero-root sums (the sum wraps to 0)
    for v in [(0, 0, 0), (65536, 0, 0), (0, 65536, 0), (32768, 32768, 0), (-65536, 65536, 0), (0, 0, -65536),
              (46340, 46340, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1)]:
        if not sum_ok(v):
            continue
        for idv in (0, 1, 3):
            g.emit('GsSetFlatLight', [str(idv), g.b(LIGHT)], ins=g.seeds(ws=False, org=False, mode=False) +
                   [(g.b(LIGHT), light_bytes(v, [10, 20, 30]))])

    # ---- GsSetAmbient ----
    edges = [0, 1, 15, 16, 17, 0x7FF, 0x800, 0x801, 0xFFF, 0x1000, 0x1001, 0xFFFF, 0x10000, -1, -16, -17,
             0x7FFFFFFF, -0x80000000, 0x12345678, -0x12345678]
    for _ in range(N):
        g.emit('GsSetAmbient', [g.hexs(r.choice(edges + [r.getrandbits(32), r.randint(0, 0x1000)])) for _ in range(3)])
    g.emit('GsSetAmbient', ['800', '800', '800'])

    # ---- GsSetLightMode ----
    for m in [0, 1, 2, 3, 4, 5, 6, 7, 8, 15, 16, 0xFF, 0x100, 0x10000, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF,
              0xFFFFFFFE, 0xFFFFFFFD]:
        g.emit('GsSetLightMode', [g.hexs(m)], ins=g.seeds(lw=False, lc=False, ws=False, org=False))
    for _ in range(N // 2):
        g.emit('GsSetLightMode', [g.hexs(r.choice([0, 1, 2, 3, r.getrandbits(32), r.randint(-4, 8)]))],
               ins=g.seeds(lw=False, lc=False, ws=False, org=False))

    # ---- GsSetLightMatrix / GsSetLsMatrix ----
    for i in range(N * 3):
        kind = r.choice(['rot', 'rot', 'any', 'small'])
        ins = g.seeds(lc=False, ws=False, org=False, mode=False) + [(g.b(MAT_A), g.mat(kind))]
        g.emit('GsSetLightMatrix', [g.b(MAT_A)], ins=ins, outs=[(g.b(MAT_A), 32)])
    for i in range(N * 2):
        kind = r.choice(['rot', 'rot', 'any', 'small'])
        ins = [(g.b(MAT_A), g.mat(kind))]
        g.emit('GsSetLsMatrix', [g.b(MAT_A)], ins=ins, outs=[(g.b(MAT_A), 32)])

    # ---- GsMulCoord0 / GsMulCoord3 ----
    def mc_mat():
        k = r.random()
        if k < 0.5:
            return g.mat(r.choice(['rot', 'rot', 'any', 'small']), t=g.small_t())
        if k < 0.8:
            return g.mat(r.choice(['rot', 'any']))
        edge = [0, 1, -1, 0x7FFFFFFF, -0x80000000, 0x7FFF, 0x8000, -0x8000, 0xFFFF, 0x10000, 0x3FFFFFFF]
        return g.mat(r.choice(['rot', 'any']), t=[r.choice(edge) for _ in range(3)])

    for i in range(N * 4):
        a, b_ = mc_mat(), mc_mat()
        g.emit('GsMulCoord0', [g.b(MAT_A), g.b(MAT_B), g.b(MAT_C)],
               ins=[(g.b(MAT_A), a), (g.b(MAT_B), b_), (g.b(MAT_C), g.mat())],
               outs=[(g.b(o), 32) for o in (MAT_A, MAT_B, MAT_C)])
        g.emit('GsMulCoord3', [g.b(MAT_A), g.b(MAT_B)], ins=[(g.b(MAT_A), a), (g.b(MAT_B), b_)],
               outs=[(g.b(o), 32) for o in (MAT_A, MAT_B)])
    for i in range(N // 2):  # in-place variants
        a, b_ = mc_mat(), mc_mat()
        for dst in (MAT_A, MAT_B):
            g.emit('GsMulCoord0', [g.b(MAT_A), g.b(MAT_B), g.b(dst)], ins=[(g.b(MAT_A), a), (g.b(MAT_B), b_)],
                   outs=[(g.b(o), 32) for o in (MAT_A, MAT_B)])
        g.emit('GsMulCoord0', [g.b(MAT_A), g.b(MAT_A), g.b(MAT_A)], ins=[(g.b(MAT_A), a)], outs=[(g.b(MAT_A), 32)])
        g.emit('GsMulCoord0', [g.b(MAT_A), g.b(MAT_A), g.b(MAT_B)], ins=[(g.b(MAT_A), a), (g.b(MAT_B), b_)],
               outs=[(g.b(o), 32) for o in (MAT_A, MAT_B)])
        g.emit('GsMulCoord0', [g.b(MAT_A), g.b(MAT_B), g.b(MAT_B)], ins=[(g.b(MAT_A), a), (g.b(MAT_B), b_)],
               outs=[(g.b(o), 32) for o in (MAT_A, MAT_B)])
        g.emit('GsMulCoord3', [g.b(MAT_A), g.b(MAT_A)], ins=[(g.b(MAT_A), a)], outs=[(g.b(MAT_A), 32)])

    # ---- coordinate systems ----
    def get_requests(depth, flgs=None, calls=None):
        ins, offs = build_chain(g, depth, flgs=flgs)
        ins = ins + g.seeds(lw=False, lc=False, org=False, mode=False)
        fn = calls or r.choice(['GsGetLw', 'GsGetLs', 'GsGetLws'])
        node = r.randrange(depth)  # the leaf, or a node in the middle (its parents are the chain)
        # (a node in the middle sees only its own parents)
        args = [g.b(offs[node])]
        outs = coord_outs(g, offs)
        if fn == 'GsGetLws':
            args += [g.b(MAT_A), g.b(MAT_B)]
            outs += [(g.b(MAT_A), 32), (g.b(MAT_B), 32)]
        else:
            args += [g.b(MAT_A)]
            outs += [(g.b(MAT_A), 32)]
        g.emit(fn, args, ins=ins, outs=outs)
        return offs, fn, node

    for i in range(N * 3):
        depth = r.choice([1, 2, 2, 3, 3, 4, 4])
        get_requests(depth)
    for depth in (1, 2, 3, 4, 8, 16, 32):
        for fn in ('GsGetLw', 'GsGetLs', 'GsGetLws'):
            for flg in (0, 1, 'S'):
                ins, offs = build_chain(g, depth, flgs=[flg] * depth)
                args = [g.b(offs[0]), g.b(MAT_A)] + ([g.b(MAT_B)] if fn == 'GsGetLws' else [])
                g.emit(fn, args, ins=ins + g.seeds(lw=False, lc=False, org=False, mode=False),
                       outs=coord_outs(g, offs) + [(g.b(MAT_A), 32), (g.b(MAT_B), 32)])
    # every combination of flags {0, 1, stamp, other} on chains of 2 and 3, and 4 with a smaller set
    for depth, vals in ((2, [0, 1, 'S', 2]), (3, [0, 1, 'S', 2]), (4, [0, 'S', 3])):
        for combo in itertools.product(vals, repeat=depth):
            ins, offs = build_chain(g, depth, flgs=list(combo))
            fn = r.choice(['GsGetLw', 'GsGetLw', 'GsGetLs', 'GsGetLws'])
            node = r.choice([0, 0, 0, depth - 1]) if depth > 2 else 0
            args = [g.b(offs[node]), g.b(MAT_A)] + ([g.b(MAT_B)] if fn == 'GsGetLws' else [])
            g.emit(fn, args, ins=ins + g.seeds(lw=False, lc=False, org=False, mode=False),
                   outs=coord_outs(g, offs) + [(g.b(MAT_A), 32), (g.b(MAT_B), 32)])
    # the flag's caching: call, call again (nothing in `in`), flip one flag, call again
    for i in range(N // 2):
        depth = r.choice([2, 3, 4])
        ins, offs = build_chain(g, depth, flgs=[0] * depth)
        ws = g.seeds(lw=False, lc=False, org=False, mode=False)
        outs = coord_outs(g, offs) + [(g.b(MAT_A), 32)]
        g.emit('GsGetLw', [g.b(offs[0]), g.b(MAT_A)], ins=ins + ws, outs=outs)
        g.emit('GsGetLw', [g.b(offs[0]), g.b(MAT_A)], outs=outs)
        mid = r.randrange(depth)
        # reset one coordinate system's flag and give it a new local matrix
        new = g.coord_hex(0, g.game_mat(), g.mat(), BUF + offs[mid] + 0x80 if mid < depth - 1 else 0)
        g.emit('GsGetLs', [g.b(offs[0]), g.b(MAT_A)], ins=[(g.b(offs[mid]), new)], outs=outs)
        g.emit('GsGetLws', [g.b(offs[r.randrange(depth)]), g.b(MAT_A), g.b(MAT_B)], outs=outs + [(g.b(MAT_B), 32)])
        # a changed local matrix with the flag left set: the cached workm stays
        g.emit('GsGetLw', [g.b(offs[0]), g.b(MAT_A)],
               ins=[(g.b(offs[0] + 4), g.game_mat())], outs=outs)
    # a tree: two children under one parent
    for i in range(N // 4):
        ins, offs = build_chain(g, 3, flgs=[0, 0, 0])
        sib = COORD0 + 0x400
        sup = BUF + offs[1]
        ins.append((g.b(sib), g.coord_hex(g.flg(), g.game_mat(), g.mat(), sup)))
        outs = coord_outs(g, offs + [sib])
        g.emit('GsGetLw', [g.b(offs[0]), g.b(MAT_A)], ins=ins, outs=outs + [(g.b(MAT_A), 32)])
        g.emit('GsGetLs', [g.b(sib), g.b(MAT_A)], ins=g.seeds(lw=False, lc=False, org=False, mode=False),
               outs=outs + [(g.b(MAT_A), 32)])
    # extreme matrices through the chain
    for i in range(N // 2):
        depth = r.choice([2, 3])
        ins, offs = [], []
        for k in range(depth):
            off = COORD0 + k * 0x80
            sup = BUF + off + 0x80 if k < depth - 1 else 0
            ins.append((g.b(off), g.coord_hex(0, g.mat('any'), g.mat('any'), sup)))
            offs.append(off)
        g.emit(r.choice(['GsGetLw', 'GsGetLs', 'GsGetLws']),
               [g.b(offs[0]), g.b(MAT_A), g.b(MAT_B)], ins=ins + g.seeds(lw=False, lc=False, org=False, mode=False),
               outs=coord_outs(g, offs) + [(g.b(MAT_A), 32), (g.b(MAT_B), 32)])

    # ---- GsSetView2 ----
    def view_hex(sup):
        return g.mat(r.choice(['rot', 'rot', 'any']), t=g.small_t()) + struct.pack('<I', sup).hex()

    for i in range(N * 2):  # no parent
        g.emit('GsSetView2', [g.b(VIEW)],
               ins=[(g.b(VIEW), view_hex(0))] + g.seeds(lw=False, lc=False, mode=False),
               outs=[(g.b(VIEW), 36)])
    for i in range(N * 3):  # parent chains
        depth = r.choice([1, 1, 2, 3, 4])
        ins, offs = build_chain(g, depth)
        g.emit('GsSetView2', [g.b(VIEW)],
               ins=ins + [(g.b(VIEW), view_hex(BUF + offs[0]))] + g.seeds(lw=False, lc=False, mode=False),
               outs=[(g.b(VIEW), 36)] + coord_outs(g, offs))
    for depth in (1, 2, 3, 4, 8, 32):  # cached and not
        for flg in (0, 1, 'S'):
            ins, offs = build_chain(g, depth, flgs=[flg] * depth)
            g.emit('GsSetView2', [g.b(VIEW)],
                   ins=ins + [(g.b(VIEW), view_hex(BUF + offs[0]))] + g.seeds(lw=False, lc=False, mode=False),
                   outs=[(g.b(VIEW), 36)] + coord_outs(g, offs))
    for i in range(N // 2):  # the same parent again
        ins, offs = build_chain(g, r.choice([1, 2, 3]), flgs=None)
        g.emit('GsSetView2', [g.b(VIEW)],
               ins=ins + [(g.b(VIEW), view_hex(BUF + offs[0]))] + g.seeds(lw=False, lc=False, mode=False),
               outs=[(g.b(VIEW), 36)] + coord_outs(g, offs))
        g.emit('GsSetView2', [g.b(VIEW)], outs=[(g.b(VIEW), 36)] + coord_outs(g, offs))
    for i in range(N // 2):  # extreme views and parents
        depth = r.choice([1, 2])
        ins = []
        offs = []
        for k in range(depth):
            off = COORD0 + k * 0x80
            sup = BUF + off + 0x80 if k < depth - 1 else 0
            ins.append((g.b(off), g.coord_hex(0, g.mat('any'), g.mat('any'), sup)))
            offs.append(off)
        g.emit('GsSetView2', [g.b(VIEW)],
               ins=ins + [(g.b(VIEW), g.mat('any') + struct.pack('<I', BUF + offs[0]).hex())] + g.seeds(lw=False, lc=False, mode=False),
               outs=[(g.b(VIEW), 36)] + coord_outs(g, offs))

    # ---- a super of SCREEN (1): the real code faults ----
    for fn in ('GsGetLw', 'GsGetLs', 'GsGetLws'):
        ins = [(g.b(COORD0), g.coord_hex(0, g.game_mat(), g.mat(), 1))]
        g.emit(fn, [g.b(COORD0), g.b(MAT_A), g.b(MAT_B)], ins=ins, outs=[(g.b(MAT_A), 32)])
        ins = [(g.b(COORD0), g.coord_hex(0, g.game_mat(), g.mat(), BUF + COORD0 + 0x80)),
               (g.b(COORD0 + 0x80), g.coord_hex(0, g.game_mat(), g.mat(), 1))]
        g.emit(fn, [g.b(COORD0), g.b(MAT_A), g.b(MAT_B)], ins=ins, outs=[(g.b(MAT_A), 32)])
    g.emit('GsSetView2', [g.b(VIEW)], ins=[(g.b(VIEW), view_hex(1))], outs=[(g.b(VIEW), 36)])
    return g.lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--seed', type=int, default=20261014)
    ap.add_argument('--scale', type=int, default=1)
    a = ap.parse_args()
    lines = gen(a.seed, a.scale)
    with open(a.out, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(len(lines), 'requests')


if __name__ == '__main__':
    main()
