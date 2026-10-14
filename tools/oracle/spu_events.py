#!/usr/bin/env python3
"""Turn an SPU register write log (oracle/capture-spu.lua's spu-writes.txt, or the port's) into
the events that decide what's heard (docs/phase6-plan.md): per voice key-on with the parameters
the voice holds at that moment, key-off, and later pitch/volume changes; plus a summary.

    tools/oracle/spu_events.py spu-writes.txt [--summary] [--from V] [--to V]

Registers (psx-spx, SPU): voice n at 0x1F801C00 + 16 n: volume left/right (+0, +2), pitch (+4),
start address (+6, x8 bytes), ADSR (+8, +A), repeat address (+E); 0x1F801D88/8A key on (voices
0-15, 16-23), 0x1F801D8C/8E key off, 0x1F801D90/92 pitch modulation, 0x1F801D94/96 noise,
0x1F801D98/9A reverb (echo) on; 0x1F801D80/82 main volume, 0x1F801D84/86 reverb depth.
"""
import argparse
from collections import Counter, defaultdict

VOICE_FIELDS = {0: 'voll', 2: 'volr', 4: 'pitch', 6: 'start', 8: 'adsr1', 0xA: 'adsr2', 0xE: 'repeat'}
GLOBAL = {0x180: 'mvoll', 0x182: 'mvolr', 0x184: 'rvoll', 0x186: 'rvolr', 0x190: 'pmon_lo',
          0x192: 'pmon_hi', 0x194: 'non_lo', 0x196: 'non_hi', 0x198: 'eon_lo', 0x19A: 'eon_hi',
          0x1A2: 'rev_base', 0x1AA: 'spucnt', 0x1B0: 'cdvoll', 0x1B2: 'cdvolr'}


def vblank_stamp(f):
    return int(f[0])


def events(path, lo=0, hi=1 << 31, stamp=vblank_stamp):
    """The log's events in order, each a tuple starting (stamp, kind, ...); `stamp(fields)` gives
    a line's time (default its VBlank). Voices past 23 (the key registers' spare bits) are
    dropped."""
    voice = defaultdict(dict)
    glob = {}
    out = []
    for line in open(path):
        f = line.split()
        if f[1] == 'vh':
            continue
        if f[1] == 'call':
            # capture-spu.lua's DW1_SPU_CALLS: an SDK entry, kept in order with the stores
            if lo <= int(f[0]) < hi:
                out.append((stamp(f), 'call', f[2], [int(x, 16) for x in f[3:6]]))
            continue
        vb, addr, width, val = f[:4]
        vb, addr, width, val = int(vb), int(addr, 16), int(width), int(val, 16)
        t = stamp(f)
        keep = lo <= vb < hi
        off = addr - 0x1F801C00
        regs = [(off, val & 0xFFFF)] if width <= 2 else [(off, val & 0xFFFF), (off + 2, val >> 16)]
        for off, v in regs:
            if off < 0x180:
                n, f = off // 16, off % 16
                if f in VOICE_FIELDS:
                    name = VOICE_FIELDS[f]
                    old = voice[n].get(name)
                    voice[n][name] = v
                    if name in ('pitch', 'voll', 'volr') and old is not None and old != v and keep:
                        out.append((t, 'set', n, name, v))
            elif off in (0x188, 0x18A, 0x18C, 0x18E):
                base = 0 if off in (0x188, 0x18C) else 16
                kind = 'keyon' if off in (0x188, 0x18A) else 'keyoff'
                for b in range(16):
                    n = base + b
                    if v & (1 << b) and n < 24 and keep:
                        p = voice[n]
                        out.append((t, kind, n, p.get('start', 0) * 8, p.get('pitch', 0),
                                    p.get('voll', 0), p.get('volr', 0), p.get('adsr1', 0), p.get('adsr2', 0))
                                   if kind == 'keyon' else (t, kind, n))
            elif off in GLOBAL:
                name = GLOBAL[off]
                if glob.get(name) != v and keep:
                    out.append((t, 'global', name, v))
                glob[name] = v
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log')
    ap.add_argument('--summary', action='store_true')
    ap.add_argument('--from', dest='lo', type=int, default=0)
    ap.add_argument('--to', dest='hi', type=int, default=1 << 31)
    a = ap.parse_args()
    ev = events(a.log, a.lo, a.hi)
    if a.summary:
        kinds = Counter(e[1] for e in ev)
        voices = Counter(e[2] for e in ev if e[1] == 'keyon')
        samples = Counter(e[3] for e in ev if e[1] == 'keyon')
        print('events', dict(kinds))
        print('key-ons per voice', dict(sorted(voices.items())))
        print('distinct sample start addresses', len(samples))
        return
    for e in ev:
        if e[1] == 'keyon':
            vb, _, n, start, pitch, vl, vr, a1, a2 = e
            print(f'{vb} keyon v{n:02} start {start:#07x} pitch {pitch:#06x} vol {vl:#06x}/{vr:#06x} '
                  f'adsr {a1:#06x}/{a2:#06x}')
        elif e[1] == 'keyoff':
            print(f'{e[0]} keyoff v{e[2]:02}')
        elif e[1] == 'call':
            print(f'{e[0]} call {e[2]} ' + ' '.join(f'{x:x}' for x in e[3]))
        elif e[1] == 'set':
            print(f'{e[0]} set v{e[2]:02} {e[3]} {e[4]:#06x}')
        else:
            print(f'{e[0]} global {e[2]} {e[3]:#06x}')


if __name__ == '__main__':
    main()
