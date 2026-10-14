#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that exercises every libgte function (and GsSetRefView2)
on varied inputs, for `crates/psyq/tests/libgte_oracle.rs` to compare against.

Usage: gte_requests.py OUT_FILE [--seed N] [--scale K]

Run the file on the emulator (booklab, nothing else running; about 20 minutes for 56,000
requests; see the header of oracle/call-sdk.lua for the options, and set DW1_PROGRESS_EVERY to
keep a results.partial.txt as it goes):

    ssh booklab 'mkdir -p ~/Developer/dw1-oracle/runs/gte-final && cd ~/Developer/dw1-oracle/runs/gte-final && cat > req.txt' < OUT_FILE
    ssh booklab 'cd ~/Developer/dw1-oracle/runs/gte-final && R=~/Developer/dw1-oracle && \
        DW1_CALLS=$PWD/req.txt DW1_SYMBOLS=$R/symbols.txt caffeinate -dim $R/pcsx-redux -cli \
        -portable -interpreter -softgpu -bios $R/scph1001.bin -iso "$R/disc/Digimon World (USA).cue" \
        -dofile $R/oracle/call-sdk.lua -run > run.log 2>&1; tail -2 run.log'

then copy req.txt and results.txt to captures/oracle-runs/gte-final/ (gitignored: the results
are Sony's code's outputs; `DW1_GTE_CAPTURE` points the test elsewhere). --scale multiplies the
number of requests per function (default 1).

Every request that touches GTE state first sets all 64 registers (`gtein`) to a hardware-
consistent random state and records them afterwards (`gteout`), so the Rust side can start from
the same state and compare every register. Requests that don't need the GTE only do so one time
in eight, to show the function leaves it alone. Inputs the real SDK has no usable answer for are
left out: a negative SquareRoot0 argument (it reads before its table), ratan2(0, INT_MIN) and
ratan2(INT_MIN, 0) (they never return), and GsSetRefView2 views whose squared lengths overflow.
CompMatrix sums that overflow fault the CPU; the oracle records those as `v0=EXC12` and the test
skips them.
"""
import argparse
import random
import struct

M32 = 0xFFFFFFFF


def h(v):
    return '%x' % (v & M32)


class Gen:
    def __init__(self, seed):
        self.r = random.Random(seed)
        self.lines = []
        # A well-behaved scene (identity-ish rotation, vertices well inside the screen), so the
        # projection functions also run without saturating anything.
        self.tame = False

    # ---- random values ----
    def mag(self):
        """A random bit width, biased to the small and to the 16- and 32-bit edges."""
        return self.r.choice([0, 1, 4, 8, 11, 12, 12, 13, 14, 15, 16, 16, 20, 24, 28, 31, 31])

    def s16(self, wide=0.15):
        r = self.r
        if r.random() < wide:
            return r.choice([0, 1, -1, 0x7FFF, -0x8000, 0x1000, -0x1000, 0x7FFE, -0x7FFF])
        w = self.r.choice([4, 8, 10, 12, 12, 13, 14, 15, 16])
        return max(-0x8000, min(0x7FFF, r.randint(-(1 << (w - 1)), (1 << (w - 1)) - 1)))

    def s32(self):
        r = self.r
        w = self.mag()
        if w == 0:
            return r.choice([0, 1, -1, 0x7FFFFFFF, -0x80000000])
        if w == 31 and r.random() < 0.3:
            return r.choice([0x7FFFFFFF, -0x80000000, 0x7FFFFFFE, -0x7FFFFFFF])
        return r.randint(-(1 << (w - 1)) if w > 1 else -1, (1 << (w - 1)) - 1 if w > 1 else 1)

    def angle(self):
        r = self.r
        k = r.random()
        if k < 0.1:
            return r.choice([0, 0x400, 0x800, 0xC00, 0x1000, -0x400, -0x800, -0x1000, 0x7FFF, -0x8000])
        if k < 0.25:
            return r.randint(-0x8000, 0x7FFF)
        return r.randint(-0x1000, 0x1000)

    def rot16(self):
        """A rotation-like matrix entry (4.12, mostly within +-0x1000)."""
        return self.r.choice([self.r.randint(-0x1000, 0x1000)] * 6 + [self.s16()])

    # ---- encoders ----
    @staticmethod
    def sv(x, y, z, pad=0):
        return struct.pack('<4h', x, y, z, pad).hex()

    @staticmethod
    def vv(x, y, z, pad=0):
        return struct.pack('<4i', x, y, z, pad).hex()

    def matrix(self, kind=None):
        r = self.r
        kind = kind or r.choice(['rot', 'rot', 'any', 'small'])
        if kind == 'rot':
            m = [self.rot16() for _ in range(9)]
        elif kind == 'small':
            m = [r.randint(-64, 64) for _ in range(9)]
        else:
            m = [self.s16() for _ in range(9)]
        t = [self.s32() if r.random() < 0.5 else r.randint(-0x20000, 0x20000) for _ in range(3)]
        return m, t

    @staticmethod
    def mat_bytes(m, t):
        return struct.pack('<9h', *m).hex() + '0000' + struct.pack('<3i', *t).hex()

    def rand_mat_hex(self, kind=None):
        m, t = self.matrix(kind)
        return self.mat_bytes(m, t)

    def vec_sv(self):
        r = self.r
        k = r.random()
        if k < 0.6:
            return self.sv(r.randint(-0x1000, 0x1000), r.randint(-0x1000, 0x1000), r.randint(-0x1000, 0x1000))
        return self.sv(self.s16(), self.s16(), self.s16(), self.s16())

    def vec_lv(self):
        r = self.r
        k = r.random()
        if k < 0.3:
            return self.vv(r.randint(-0x8000, 0x7FFF), r.randint(-0x8000, 0x7FFF), r.randint(-0x8000, 0x7FFF))
        return self.vv(self.s32(), self.s32(), self.s32(), self.s32())

    def vec_pers(self, z_bias=True):
        """A vertex for projection: usually in front of the camera, sometimes anywhere."""
        r = self.r
        if self.tame:
            return self.sv(r.randint(-200, 200), r.randint(-200, 200), r.randint(100, 1500))
        if r.random() < 0.75:
            return self.sv(r.randint(-0x800, 0x800), r.randint(-0x800, 0x800), r.randint(0, 0x2000))
        return self.sv(self.s16(), self.s16(), self.s16(), self.s16())

    def color(self):
        return '%08x' % self.r.getrandbits(32)

    # ---- GTE preset ----
    def gte_state(self):
        r = self.r
        realistic = r.random() < 0.7

        def sx16(v):
            return v & M32

        d = [0] * 32
        c = [0] * 32
        for n in (0, 2, 4):  # VXY
            d[n] = ((self.s16() & 0xFFFF) << 16 | (self.s16() & 0xFFFF)) & M32
        for n in (1, 3, 5):
            d[n] = sx16(self.s16())
        d[6] = r.getrandbits(32)
        d[7] = r.getrandbits(16)
        for n in (8, 9, 10, 11):
            d[n] = sx16(self.s16())
        for n in (12, 13, 14):
            d[n] = ((self.s16() & 0xFFFF) << 16 | (self.s16() & 0xFFFF)) & M32
        for n in (16, 17, 18, 19):
            d[n] = r.getrandbits(16)
        for n in (20, 21, 22):
            d[n] = r.getrandbits(32)
        d[23] = 0
        for n in (24, 25, 26, 27):
            d[n] = self.s32() & M32
        d[30] = r.choice([self.s32(), r.randint(0, 1 << 20)]) & M32
        d[31] = lzcr(d[30])
        # control: rotation / light / colour matrices
        for base in (0, 8, 16):
            vals = [self.rot16() if realistic else self.s16() for _ in range(9)]
            for k in range(4):
                c[base + k] = (vals[2 * k + 1] & 0xFFFF) << 16 | (vals[2 * k] & 0xFFFF)
            c[base + 4] = sx16(vals[8])  # the lone 16-bit register, sign-extended
        for n in (5, 6, 7):
            c[n] = (r.randint(-0x4000, 0x4000) if realistic else self.s32()) & M32
        for n in (13, 14, 15):
            c[n] = (r.randint(0, 0x1000) if realistic else self.s32()) & M32
        for n in (21, 22, 23):
            c[n] = (r.randint(0, 0xFFFF) if realistic else self.s32()) & M32
        for n in (24, 25):
            c[n] = (r.randint(-0x8000, 0x7FFF) << 16 if realistic else self.s32()) & M32
        c[26] = sx16(r.randint(50, 0x2000) if realistic else r.getrandbits(16) - (0x10000 if r.random() < 0.5 else 0))
        c[27] = sx16(self.s16())
        c[28] = self.s32() & M32
        c[29] = sx16(self.s16())
        c[30] = sx16(self.s16())
        if self.tame:
            # near-identity rotation, a camera a little behind the origin, a plain projection
            rot = [0x1000, r.randint(-60, 60), r.randint(-60, 60),
                   r.randint(-60, 60), 0x1000, r.randint(-60, 60),
                   r.randint(-60, 60), r.randint(-60, 60), 0x1000]
            for k in range(4):
                c[k] = (rot[2 * k + 1] & 0xFFFF) << 16 | (rot[2 * k] & 0xFFFF)
            c[4] = sx16(rot[8])
            c[5], c[6] = r.randint(-100, 100) & M32, r.randint(-100, 100) & M32
            c[7] = r.randint(0, 800)
            c[24] = (r.randint(-160, 160) << 16) & M32
            c[25] = (r.randint(-120, 120) << 16) & M32
            c[26] = r.randint(200, 1200)
            c[29], c[30] = 0x155, 0x100
            # lighting that stays in range: a mild light matrix, a near-identity colour matrix,
            # a dim ambient
            light = [r.randint(-0x500, 0x500) for _ in range(9)]
            colour = [0x1000 if i % 4 == 0 else r.randint(-120, 120) for i in range(9)]
            for base, vals in ((8, light), (16, colour)):
                for k in range(4):
                    c[base + k] = (vals[2 * k + 1] & 0xFFFF) << 16 | (vals[2 * k] & 0xFFFF)
                c[base + 4] = sx16(vals[8])
            for n in (13, 14, 15):
                c[n] = r.randint(0, 0x300)
            for n in (21, 22, 23):
                c[n] = r.randint(0, 0xFF)
        # FLAG: only the bits a command sets; bit 31 is the OR of the error bits
        f = r.getrandbits(32) & 0x7FFFF000 if r.random() < 0.5 else 0
        if f & 0x7F87E000:
            f |= 0x80000000
        c[31] = f
        words = []
        for n in range(32):
            words.append('x' if n in (15, 28, 29) else '%08x' % d[n])
        for n in range(32):
            words.append('%08x' % c[n])
        return 'gtein=' + ','.join(words)

    # ---- emitting ----
    def add(self, fn, args, ins=(), outs=(), stack=(), gte=True, *, pure_gte_every=8):
        parts = [fn] + [a if isinstance(a, str) else h(a) for a in (list(args) + ['0'] * 4)[:4]]
        for addr, hexbytes in ins:
            parts.append(f'in={addr}:{hexbytes}')
        for addr, n in outs:
            parts.append(f'out={addr}:{n:x}')
        if stack:
            parts.append('stack=' + ','.join(stack))
        if gte is True or (gte == 'sometimes' and self.r.randrange(pure_gte_every) == 0):
            parts.append(self.gte_state())
            parts.append('gteout')
        self.lines.append(' '.join(parts))


# ratan2(0, INT_MIN) and ratan2(INT_MIN, 0) never return on the PS1 [obs].
HANGS = {(0, -0x80000000), (-0x80000000, 0)}


def lzcr(v):
    v &= M32
    if v >> 31:
        v = ~v & M32
    n = 0
    while n < 32 and not (v >> (31 - n)) & 1:
        n += 1
    return n


def gen(seed, scale):
    g = Gen(seed)
    r = g.r
    N = 400 * scale

    # ---- pure integer functions ----
    for a in list(range(-4200, 4200)) + [r.randint(-(1 << 31), (1 << 31) - 1) for _ in range(300 * scale)]:
        g.add('rsin', [a], gte=False)
    for a in list(range(-1, 4200)) + [r.randint(-(1 << 31), (1 << 31) - 1) for _ in range(300 * scale)]:
        g.add('rcos', [a], gte=False)
    for i in range(0, 1025):  # every table cell, exactly on its lower edge
        g.add('ratan2', [i, 1024], gte=False)
        g.add('ratan2', [1024, i], gte=False)
        g.add('ratan2', [-i, -1024], gte=False)
    for y in range(-24, 25):
        for x in range(-24, 25):
            g.add('ratan2', [y, x], gte=False)
    for _ in range(4000 * scale):
        s = r.choice([4, 8, 10, 12, 16, 20, 22, 24, 30, 31])
        y = r.randint(-(1 << s), (1 << s) - 1)
        x = r.randint(-(1 << s), (1 << s) - 1)
        if r.random() < 0.1:
            x = y * r.choice([1, -1])
        if (y, x) not in HANGS and (x, y) not in HANGS:
            g.add('ratan2', [y, x], gte=False)
    for y in (0x7FFFFFFF, -0x80000000, 0x7FFFFFFE, 1, -1, 0):
        for x in (0x7FFFFFFF, -0x80000000, 0x7FFFFFFE, 1, -1, 0):
            if (y, x) not in HANGS and (x, y) not in HANGS:
                g.add('ratan2', [y, x], gte=False)
    for a in list(range(0, 3000)) + [k * k for k in range(0, 3000, 3)]:
        g.add('SquareRoot0', [a], gte='sometimes')
    for _ in range(4000 * scale):
        s = r.choice([6, 10, 16, 20, 24, 26, 28, 30, 31])
        g.add('SquareRoot0', [r.randint(0, (1 << s) - 1)], gte='sometimes')
    # Negative arguments make the real function read past its table (no formula; the game never
    # passes one), so they are not requested.
    for a in (0x7FFFFFFF, 0x7FFFFFFE, 0x40000000, 0x3FFFFFFF, 0, 1):
        g.add('SquareRoot0', [a], gte=True)

    # ---- set / get ----
    for _ in range(N // 2):
        g.add('SetRotMatrix', ['B+0'], ins=[('B+0', g.rand_mat_hex())])
    for _ in range(N // 2):
        g.add('SetTransMatrix', ['B+0'], ins=[('B+0', g.rand_mat_hex())])
    for _ in range(N // 2):
        g.add('SetLightMatrix', ['B+0'], ins=[('B+0', g.rand_mat_hex())])
    for _ in range(N // 2):
        m, t = g.matrix()
        v = g.vv(g.s32(), g.s32(), g.s32(), g.s32())
        g.add('TransMatrix', ['B+0', 'B+100'], ins=[('B+0', g.mat_bytes(m, t)), ('B+100', v)],
              outs=[('B+0', 0x20)], gte='sometimes')

    # ---- matrix stack: runs of pushes and pops (the stack is shared state) ----
    depth = 0
    for _ in range(N):
        if depth > 0 and (r.random() < 0.45 or depth >= 18):
            g.add('PopMatrix', [])
            depth -= 1
        else:
            g.add('PushMatrix', [])
            depth += 1
    for _ in range(depth):
        g.add('PopMatrix', [])
    # the stack holds 20: a push on a full stack does nothing, a pop on an empty one neither
    for _ in range(3):
        for _ in range(26):
            g.add('PushMatrix', [])
        for _ in range(27):
            g.add('PopMatrix', [])

    # ---- angle matrices ----
    for fn in ('RotMatrix', 'RotMatrixYXZ', 'RotMatrixZYX'):
        for _ in range(N * 3):
            m, t = g.matrix()
            g.add(fn, ['B+0', 'B+100'],
                  ins=[('B+0', g.sv(g.angle(), g.angle(), g.angle(), g.s16())), ('B+100', g.mat_bytes(m, t))],
                  outs=[('B+100', 0x20)], gte='sometimes')

    # ---- matrix arithmetic ----
    for _ in range(N):
        m, t = g.matrix()
        v = g.vv(*[g.s32() if r.random() < 0.5 else r.randint(-0x4000, 0x4000) for _ in range(3)])
        g.add('ScaleMatrix', ['B+0', 'B+100'], ins=[('B+0', g.mat_bytes(m, t)), ('B+100', v)],
              outs=[('B+0', 0x20)], gte='sometimes')
    for _ in range(N):
        g.add('TransposeMatrix', ['B+0', 'B+100'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex())],
              outs=[('B+100', 0x20)], gte='sometimes')
    for _ in range(N * 2):
        g.add('MulMatrix0', ['B+0', 'B+100', 'B+200'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex()), ('B+200', g.rand_mat_hex())],
              outs=[('B+200', 0x20)])
    for _ in range(N * 2):
        g.add('MulMatrix', ['B+0', 'B+100'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex())], outs=[('B+0', 0x20)])
    for _ in range(N * 2):
        g.add('CompMatrix', ['B+0', 'B+100', 'B+200'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex()), ('B+200', g.rand_mat_hex())],
              outs=[('B+200', 0x20)])
    for _ in range(N):
        g.add('CompMatrix', ['B+0', 'B+100', 'B+0'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex())], outs=[('B+0', 0x20)])
    for _ in range(N):
        g.add('MulMatrix0', ['B+0', 'B+100', 'B+100'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex())], outs=[('B+100', 0x20)])
    pad = 'a5a5a5a5' * 4
    # the same buffer as input and output
    for _ in range(N // 2):
        g.add('ApplyMatrixSV', ['B+0', 'B+100', 'B+100'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.vec_sv())], outs=[('B+100', 8)])
    for _ in range(N // 2):
        g.add('ApplyMatrixLV', ['B+0', 'B+100', 'B+100'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.vec_lv())], outs=[('B+100', 0x10)])
    for _ in range(N // 2):
        g.add('ApplyTransposeMatrixLV', ['B+0', 'B+100', 'B+100'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.vec_lv())], outs=[('B+100', 0x10)])
    for _ in range(N // 2):
        g.add('TransposeMatrix', ['B+0', 'B+0'], ins=[('B+0', g.rand_mat_hex())], outs=[('B+0', 0x20)])
    for _ in range(N // 2):
        g.add('MulMatrix0', ['B+0', 'B+100', 'B+0'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex())], outs=[('B+0', 0x20)])
    for _ in range(N // 2):
        g.add('MulMatrix', ['B+0', 'B+0'], ins=[('B+0', g.rand_mat_hex())], outs=[('B+0', 0x20)])
    for _ in range(N // 2):
        g.add('CompMatrix', ['B+0', 'B+100', 'B+100'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.rand_mat_hex())], outs=[('B+100', 0x20)])
    for _ in range(N * 2):
        g.add('ApplyMatrix', ['B+0', 'B+100', 'B+200'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.vec_sv()), ('B+200', pad)], outs=[('B+200', 0x10)])
    for _ in range(N * 2):
        g.add('ApplyMatrixSV', ['B+0', 'B+100', 'B+200'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.vec_sv()), ('B+200', pad)], outs=[('B+200', 8)])
    for _ in range(N * 3):
        g.add('ApplyMatrixLV', ['B+0', 'B+100', 'B+200'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.vec_lv()), ('B+200', pad)], outs=[('B+200', 0x10)])
    for _ in range(N * 3):
        g.add('ApplyTransposeMatrixLV', ['B+0', 'B+100', 'B+200'],
              ins=[('B+0', g.rand_mat_hex()), ('B+100', g.vec_lv()), ('B+200', pad)], outs=[('B+200', 0x10)])

    # ---- projection, normals ----
    for _ in range(N * 3):
        g.tame = r.random() < 0.5
        g.add('RotTransPers', ['B+0', 'B+100', 'B+110', 'B+120'],
              ins=[('B+0', g.vec_pers()), ('B+100', pad), ('B+110', pad), ('B+120', pad)],
              outs=[('B+100', 4), ('B+110', 4), ('B+120', 4)])
    for _ in range(N * 3):
        g.tame = r.random() < 0.5
        g.add('RotTransPers3', ['B+0', 'B+10', 'B+20', 'B+100'],
              ins=[('B+0', g.vec_pers()), ('B+10', g.vec_pers()), ('B+20', g.vec_pers()),
                   ('B+100', pad), ('B+110', pad), ('B+120', pad), ('B+130', pad), ('B+140', pad)],
              stack=['B+110', 'B+120', 'B+130', 'B+140'],
              outs=[('B+100', 4), ('B+110', 4), ('B+120', 4), ('B+130', 4), ('B+140', 4)])
    for _ in range(N * 3):
        g.tame = r.random() < 0.5
        g.add('RotTransPers4', ['B+0', 'B+10', 'B+20', 'B+30'],
              ins=[('B+0', g.vec_pers()), ('B+10', g.vec_pers()), ('B+20', g.vec_pers()),
                   ('B+30', g.vec_pers())] + [(f'B+{0x100 + 0x10 * k:x}', pad) for k in range(6)],
              stack=[f'B+{0x100 + 0x10 * k:x}' for k in range(6)],
              outs=[(f'B+{0x100 + 0x10 * k:x}', 4) for k in range(6)])
    for _ in range(N * 3):
        g.tame = r.random() < 0.5
        g.add('RotNclip3', ['B+0', 'B+10', 'B+20', 'B+100'],
              ins=[('B+0', g.vec_pers()), ('B+10', g.vec_pers()), ('B+20', g.vec_pers())]
              + [(f'B+{0x100 + 0x10 * k:x}', pad) for k in range(5)],
              stack=[f'B+{0x110 + 0x10 * k:x}' for k in range(5)],
              outs=[(f'B+{0x100 + 0x10 * k:x}', 4) for k in range(6)])
    for _ in range(N * 3):
        g.tame = r.random() < 0.5
        g.add('RotNclip4', ['B+0', 'B+10', 'B+20', 'B+30'],
              ins=[('B+0', g.vec_pers()), ('B+10', g.vec_pers()), ('B+20', g.vec_pers()),
                   ('B+30', g.vec_pers())] + [(f'B+{0x100 + 0x10 * k:x}', pad) for k in range(7)],
              stack=[f'B+{0x100 + 0x10 * k:x}' for k in range(7)],
              outs=[(f'B+{0x100 + 0x10 * k:x}', 4) for k in range(7)])
    for _ in range(N * 2):
        g.tame = r.random() < 0.5
        g.add('NormalColorCol', ['B+0', 'B+100', 'B+200'],
              ins=[('B+0', g.vec_sv()), ('B+100', g.color()), ('B+200', pad)], outs=[('B+200', 4)])
    for _ in range(N * 2):
        g.tame = r.random() < 0.5
        g.add('NormalColorCol3', ['B+0', 'B+10', 'B+20', 'B+100'],
              ins=[('B+0', g.vec_sv()), ('B+10', g.vec_sv()), ('B+20', g.vec_sv()), ('B+100', g.color()),
                   ('B+200', pad), ('B+210', pad), ('B+220', pad)],
              stack=['B+200', 'B+210', 'B+220'],
              outs=[('B+200', 4), ('B+210', 4), ('B+220', 4)])

    g.tame = False

    # ---- libgs: GsSetRefView2 ----
    g.tame = False
    # A view whose direction has squared lengths past 2^31 makes the real function take the
    # square root of a negative number, which reads past SquareRoot0's table: not requested.
    def w32(x):
        x &= M32
        return x - (1 << 32) if x >> 31 else x

    def view_ok(vp, vr):
        k = 0
        while any(abs(x) >> k >= 1 << 15 for x in vp + vr if x != -(1 << 31)):
            k += 1
        d = [w32((vr[i] >> k) - (vp[i] >> k)) for i in range(3)]
        sq = [w32(x * x) for x in d]
        return w32(sq[0] + sq[1] + sq[2]) >= 0 and w32(sq[0] + sq[2]) >= 0

    def comp(m=None):
        m = m or r.choice([1, 3, 10, 100, 1000, 3000, 10000, 20000, 30000, 40000, 70000, 200000,
                           1 << 20, 1 << 24, 1 << 28, 1 << 30, 1 << 31])
        v = r.randint(-m, m)
        return max(-(1 << 31), min((1 << 31) - 1, v))

    def view(vp, vr, rz=0):
        if not view_ok(vp, vr):
            return
        body = struct.pack('<7i', *vp, *vr, rz).hex() + '00000000'
        g.add('GsSetRefView2', ['B+0'], ins=[('B+0', body)],
              outs=[('80136f84', 0x20), ('80136fa4', 0x20)])

    for _ in range(N * 2):  # camera at the origin, any direction
        view([0, 0, 0], [comp(), comp(), comp()], r.choice([0, 0, r.randint(-4200, 4200)]))
    for _ in range(N * 2):  # camera anywhere, target anywhere
        view([comp(), comp(), comp()], [comp(), comp(), comp()], r.choice([0, 0, comp()]))
    for _ in range(N * 2):  # camera far out, target near it
        vp = [comp(), comp(), comp()]
        view(vp, [max(-(1 << 31), min((1 << 31) - 1, vp[i] + r.randint(-8000, 8000))) for i in range(3)])
    for _ in range(N):  # game-like: a camera a few thousand units out, looking at the map
        vp = [r.randint(-6000, 6000), r.randint(-3000, 3000), r.randint(-9000, 9000)]
        view(vp, [r.randint(-3000, 3000), r.randint(-500, 500), r.randint(-3000, 3000)],
             r.choice([0, 0, r.randint(-300000, 300000)]))
    edges = [0, 1, -1, 32766, 32767, 32768, 32769, -32767, -32768, -32769, 65535, 65536, 65537, -65536,
             -65537, 131071, 131072, -131072, 0x7FFFFFFF, -0x80000000, -0x7FFFFFFF]
    for _ in range(N):  # the 16-bit edges of the scaling
        vp = [r.choice(edges + [0, 0]) for _ in range(3)]
        vr = [r.choice(edges + [0, 0]) for _ in range(3)]
        view(vp, vr)
    for rz in list(range(-3000, 3001, 61)) + [r.randint(-(1 << 31), (1 << 31) - 1) for _ in range(40)] + \
            [-(1 << 31), (1 << 31) - 1, 359, 360, 361, -359, -360, -361]:
        view([100, -200, 300], [800, 100, 1400], rz)
        view([0, 0, 0], [0, 5000, 0], rz)
    for d in [(0, 0, 0), (0, 0, 1), (0, 0, -1), (1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 100, 0), (0, -100, 0),
              (0, 5000, 0), (0, 1 << 30, 0), (1, 1, 1), (-1, -1, -1), (100, 100, 100), (0, 0, 2000),
              (0, 0, -2000), (2000, 0, 0), (-2000, 0, 0), (0, -(1 << 30), 0)]:
        view([0, 0, 0], list(d))
        view([5, -7, 11], [5 + d[0], -7 + d[1], 11 + d[2]])
    g.tame = False
    return g.lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--seed', type=int, default=20261013)
    ap.add_argument('--scale', type=int, default=1)
    a = ap.parse_args()
    lines = gen(a.seed, a.scale)
    with open(a.out, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(len(lines), 'requests')


if __name__ == '__main__':
    main()
