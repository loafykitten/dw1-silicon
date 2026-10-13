#!/usr/bin/env python3
"""Inspect DW1STATE dumps from oracle/capture-libgs.lua: one call's GsDOBJ2, coord chain, GTE."""
import struct, sys

REGS = 131 * 4

def load(path):
    b = open(path, 'rb').read()
    assert b[:8] == b'DW1STATE'
    regs = struct.unpack_from('<131I', b, 8)
    scratch = b[8 + REGS:8 + REGS + 1024]
    ram = b[8 + REGS + 1024:]
    assert len(ram) == 2 << 20
    gpr, cp0, cp2d, cp2c, pc = regs[0:34], regs[34:66], regs[66:98], regs[98:130], regs[130]
    return dict(gpr=gpr, cp2d=cp2d, cp2c=cp2c, pc=pc, ram=ram, scratch=scratch)

def rd(st, addr, fmt):
    a = addr & 0x1FFFFF
    return struct.unpack_from('<' + fmt, st['ram'], a)

def s16(v): return v - 0x10000 if v & 0x8000 else v

def matrix(st, addr):
    m = rd(st, addr, '9h2x3i')
    return [list(m[0:3]), list(m[3:6]), list(m[6:9])], list(m[9:12])

def gte_rt(c):
    r = [s16(c[0] & 0xFFFF), s16(c[0] >> 16), s16(c[1] & 0xFFFF), s16(c[1] >> 16),
         s16(c[2] & 0xFFFF), s16(c[2] >> 16), s16(c[3] & 0xFFFF), s16(c[3] >> 16), s16(c[4] & 0xFFFF)]
    return [r[0:3], r[3:6], r[6:9]]

def det(m):
    a = [[x / 4096 for x in row] for row in m]
    return (a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]))

def main(path):
    st = load(path)
    a0, a1, a2, a3, ra = st['gpr'][4], st['gpr'][5], st['gpr'][6], st['gpr'][7], st['gpr'][31]
    print(f'pc={st["pc"]:08x} objp={a0:08x} otp={a1:08x} shift={a2} scratch={a3:08x} ra={ra:08x}')
    attr, coord2, tmd, oid = rd(st, a0, '4I')
    print(f'GsDOBJ2 attr={attr:08x} coord2={coord2:08x} tmd={tmd:08x} id={oid:08x}')
    c = coord2
    depth = 0
    while c and depth < 12:
        flg, = rd(st, c, 'I')
        m, t = matrix(st, c + 4)
        w, wt = matrix(st, c + 0x24)
        param, sup, sub = rd(st, c + 0x44, '3I')
        print(f'  coord {c:08x} flg={flg} super={sup:08x} coord={m} t={t} det={det(m):.3f}')
        print(f'           workm={w} t={wt} det={det(w):.3f}')
        c, depth = sup, depth + 1
    rt = gte_rt(st['cp2c'])
    cc = st['cp2c']
    tr = [x - (1 << 32) if x & 0x80000000 else x for x in cc[5:8]]
    print(f'GTE RT={rt} det={det(rt):.4f} TR={tr} OFX={cc[24]:08x} OFY={cc[25]:08x} H={cc[26]}')
    if tmd:
        w = rd(st, tmd, '7I')
        print(f'TMD obj: vert={w[0]:08x} n_vert={w[1]} normal={w[2]:08x} n_normal={w[3]} prim={w[4]:08x} n_prim={w[5]} scale={w[6]}')
        n = min(w[1], 8)
        for i in range(n):
            print('   v', rd(st, w[0] + 8 * i, '3hh'))

if __name__ == '__main__':
    for p in sys.argv[1:]:
        print('==', p)
        main(p)
