#!/usr/bin/env python3
"""Show what each SDK call in an oracle/capture-calls.lua capture did: its arguments, return
value, and every RAM / scratchpad / GTE register range that changed between entry and return.

Usage: calldiff.py CAPTURE_DIR [--calls N,M,...] [--names A,B] [--words N] [--sym]

--words N  print up to N changed 32-bit words per range (default 16)
--sym      label addresses with the nearest symbol from decomp/config/us/symbols.txt
"""
import argparse
import bisect
import struct
from pathlib import Path

from dumpstate import load

ROOT = Path(__file__).resolve().parents[2]


def symbols():
    out = []
    for line in (ROOT / 'decomp/config/us/symbols.txt').read_text().splitlines():
        parts = line.split('=')
        if len(parts) < 2:
            continue
        name = parts[0].strip()
        val = parts[1].split(';')[0].strip()
        if name.isidentifier() and val.startswith('0x'):
            out.append((int(val, 16) & 0x1FFFFF, name))
    out.sort()
    return out


def label(syms, addr):
    if not syms:
        return ''
    keys = [a for a, _ in syms]
    i = bisect.bisect_right(keys, addr & 0x1FFFFF) - 1
    if i < 0:
        return ''
    a, n = syms[i]
    off = (addr & 0x1FFFFF) - a
    return f' <{n}+{off:#x}>' if off else f' <{n}>'


def ranges(a, b):
    """Changed word ranges between two equal-length byte strings: [(start, end)] in bytes."""
    out = []
    n = len(a) // 4
    wa = struct.unpack(f'<{n}I', a)
    wb = struct.unpack(f'<{n}I', b)
    i = 0
    while i < n:
        if wa[i] != wb[i]:
            j = i
            while j < n and (wa[j] != wb[j] or (j + 1 < n and wa[j + 1] != wb[j + 1])):
                j += 1
            out.append((i * 4, j * 4))
            i = j
        i += 1
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument('dir')
    p.add_argument('--calls')
    p.add_argument('--names')
    p.add_argument('--words', type=int, default=16)
    p.add_argument('--sym', action='store_true')
    args = p.parse_args()
    d = Path(args.dir)
    syms = symbols() if args.sym else None
    want = set(int(x) for x in args.calls.split(',') if x) if args.calls else None
    names = set(args.names.split(',')) if args.names else None
    for line in (d / 'calls.txt').read_text().splitlines()[1:]:
        f = line.split()
        n, vb, name = int(f[0]), f[1], f[2]
        if want is not None and n not in want:
            continue
        if names is not None and name not in names:
            continue
        a0, a1, a2, a3, sp, ra = (int(x, 16) for x in f[3:9])
        v0 = f[9] if len(f) > 9 else '-'
        print(f'#{n:04d} vb={vb} {name}(a0={a0:08x}{label(syms, a0)}, a1={a1:08x}, a2={a2:08x}, '
              f'a3={a3:08x}) sp={sp:08x} ra={ra:08x}{label(syms, ra)} -> v0={v0}')
        pin, pout = d / f'call{n:04d}_in.bin', d / f'call{n:04d}_out.bin'
        if not (pin.exists() and pout.exists()):
            continue
        si, so = load(pin), load(pout)
        # Stack arguments 5..8 (O32: sp+16..)
        stk = struct.unpack_from('<4I', si['ram'], (sp & 0x1FFFFF) + 16)
        print('    stack args: ' + ' '.join(f'{x:08x}' for x in stk))
        for what, ba, bb, base in (('ram', si['ram'], so['ram'], 0x80000000),
                                   ('scratch', si['scratch'], so['scratch'], 0x1F800000)):
            for s, e in ranges(ba, bb):
                addr = base + s
                if what == 'ram' and (sp & 0x1FFFFF) - 0x400 <= s < (sp & 0x1FFFFF):
                    continue  # the callee's own stack frame
                words = [struct.unpack_from('<I', bb, k)[0] for k in range(s, e + 4, 4)]
                old = [struct.unpack_from('<I', ba, k)[0] for k in range(s, e + 4, 4)]
                shown = min(len(words), args.words)
                print(f'    {what} {addr:08x}..{base + e + 4:08x}{label(syms, addr)}:')
                print('      new ' + ' '.join(f'{w:08x}' for w in words[:shown])
                      + (' ...' if shown < len(words) else ''))
                print('      old ' + ' '.join(f'{w:08x}' for w in old[:shown]))
        for nm, ka, kb in (('cp2d', si['cp2d'], so['cp2d']), ('cp2c', si['cp2c'], so['cp2c'])):
            ch = [f'{i}:{ka[i]:08x}->{kb[i]:08x}' for i in range(32) if ka[i] != kb[i]]
            if ch:
                print(f'    {nm} ' + ' '.join(ch))


if __name__ == '__main__':
    main()
