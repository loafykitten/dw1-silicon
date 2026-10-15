#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that exercises libgpu's `SetDrawLoad(DR_LOAD *p, RECT *r)`,
for `crates/psyq/tests/libgpu_drawload_oracle.rs` to compare against.

Usage: drawload_requests.py OUT_FILE [--seed N]

Run the file on the emulator (a fresh empty directory under captures/oracle-runs/, e.g.
p9-render-drawload, holding req.txt; see oracle/call-sdk.lua), then keep req.txt and results.txt
there (gitignored: the results are Sony's code's outputs).

Each request fills a DR_LOAD (17 words) and the 16 words after it with 0xA5 at B+0x100, writes a
RECT at B+0x40 and calls SetDrawLoad(B+0x100, B+0x40), then reads back the 33 words and the
RECT. The rectangles: the game's (a 16 x 1 CLUT row, STD/VS's EFE sub-opcode 0x50), then other
sizes up to and past the 13 data words a DR_LOAD holds, odd widths, zero and negative sizes, and
positions across VRAM and past it.
"""
import argparse
import random
import struct


def request(x, y, w, h):
    rect = struct.pack('<4h', x, y, w, h).hex()
    return ('SetDrawLoad B+100 B+40 0 0 in=B+40:%s in=B+100:%s out=B+100:84 out=B+40:8'
            % (rect, 'a5' * 0x84))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--seed', type=int, default=20261015)
    a = ap.parse_args()
    r = random.Random(a.seed)
    lines = []
    for _ in range(40):
        lines.append(request(r.randint(0, 63) * 16, r.randint(0, 511), 16, 1))
    for w, h in [(1, 1), (2, 1), (16, 1), (24, 1), (26, 1), (27, 1), (28, 1), (32, 1), (16, 2), (8, 4), (13, 2),
                 (3, 3), (0, 0), (0, 1), (16, 0), (-1, 1), (16, -1), (1024, 512), (256, 256), (5, 5)]:
        for _ in range(3):
            lines.append(request(r.randint(0, 1023), r.randint(0, 511), w, h))
    for _ in range(60):
        lines.append(request(r.randint(-2048, 2047), r.randint(-1024, 1023), r.randint(-40, 64),
                             r.randint(-4, 16)))
    with open(a.out, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(len(lines), 'requests')


if __name__ == '__main__':
    main()
