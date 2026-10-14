#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that exercises libgs's `GsSortSprite`, for
`crates/psyq/tests/libgs_sprite_oracle.rs` to compare against.

Usage: sprite_requests.py OUT_FILE [--seed N] [--scale K]

Run the file on the emulator (one emulator at a time; a fresh empty directory under
captures/oracle-runs/, e.g. libgs-sprite, holding req.txt; see oracle/call-sdk.lua), then keep
req.txt and results.txt there (gitignored: the results are Sony's code's outputs;
`DW1_LIBGS_SPRITE_CAPTURE` points the test elsewhere).

Every request builds a GsSPRITE at B+0x100, a 16-entry ordering table (GsOT at B+0x200, entries
at B+0x300, all ending the list) and a work base (GsOUT_PACKET_P) at B+0x400 over a 0xA5-filled
area, calls `GsSortSprite(sprite, ot, pri)`, and reads back the packet area, the table, the work
base and the GTE registers.

The sprites are the clock hand of File City (DW1's only rotating sprite so far: attribute 0,
8 by 16 pixels, reference point (3, 13), scale 4096 = 1.0, `rotate` = minute * 0x6000), at every
one of its 60 angles and several positions, then random rotations, positions, sizes, reference
points and textures with the other fields kept at the values the game uses (attribute 0, scale
4096, colour 128/128/128) because the native function stops on anything else.
"""
import argparse
import math
import random
import struct

BUF = 0x801E0000
SPRITE, OT, ORG, PACKETS = 0x100, 0x200, 0x300, 0x400
G_OUT_PACKET_P = 0x80136FE4
OT_LENGTH = 4  # 16 entries


def sprite_bytes(x, y, w, h, tpage, u, v, cx, cy, mx, my, rotate, scalex=4096, scaley=4096, rgb=(128, 128, 128),
                 attribute=0):
    return struct.pack('<IhhHHHBBhhBBBxhhhhi', attribute, x, y, w, h, tpage, u, v, cx, cy, *rgb, mx, my,
                       scalex, scaley, rotate).hex()


def request(sprite, pri):
    ins = [
        ('B+%x' % SPRITE, sprite),
        ('B+%x' % OT, struct.pack('<IIIII', OT_LENGTH, BUF + ORG, 0, 0, BUF + ORG + 4 * ((1 << OT_LENGTH) - 1)).hex()),
        ('B+%x' % ORG, struct.pack('<16I', *([0x00FFFFFF] * 16)).hex()),
        ('B+%x' % PACKETS, ('a5' * 0x80)),
        ('%x' % G_OUT_PACKET_P, struct.pack('<I', BUF + PACKETS).hex()),
    ]
    outs = [('B+%x' % PACKETS, 0x80), ('B+%x' % ORG, 64), ('%x' % G_OUT_PACKET_P, 4), ('B+%x' % SPRITE, 36)]
    parts = ['GsSortSprite', 'B+%x' % SPRITE, 'B+%x' % OT, '%x' % pri, '0']
    parts += ['in=%s:%s' % i for i in ins]
    parts += ['out=%s:%x' % o for o in outs]
    parts.append('gteout')
    return ' '.join(parts)


def gen(seed, scale):
    r = random.Random(seed)
    lines = []
    # The clock hand: every minute at a few positions (x is CLOCK_OFFSET_X + 23 for a few offsets).
    for x, y in [(-197, -66), (-162, -66), (-112, -66), (0, 0), (100, -50)]:
        for minute in range(60):
            lines.append(request(sprite_bytes(x, y, 8, 16, 0x1E, 120, 192, 256, 499, 3, 13, minute * 0x6000), 9))
    # Random sprites: full range of angles (and beyond one turn, and negative).
    for _ in range(600 * scale):
        w, h = r.choice([8, 16, 32, 64, r.randint(1, 255)]), r.choice([8, 16, 32, 64, r.randint(1, 255)])
        rot = r.choice([r.randint(0, 360 * 4096 - 1)] * 4 + [r.randint(-720 * 4096, 720 * 4096),
                       r.choice([0, 90, 180, 270, 360, -90, 45, 1, 2]) * 4096, r.randint(-5, 5)])
        mx, my = r.choice([(0, 0), (3, 13), (w // 2, h // 2), (r.randint(-20, 300), r.randint(-20, 300))])
        lines.append(request(sprite_bytes(
            r.randint(-300, 300), r.randint(-300, 300), w, h, r.choice([0x1E, 0x1C, 0x8C, r.randint(0, 0x1FF)]),
            r.randint(0, 255), r.randint(0, 255), r.randint(0, 1023) & ~15, r.randint(0, 511), mx, my, rot),
            r.choice([9, 9, 0, 1, 7, 15])))
    # Unrotated ones too (the SPRT path).
    for _ in range(100 * scale):
        w, h = r.randint(1, 255), r.randint(1, 255)
        lines.append(request(sprite_bytes(
            r.randint(-300, 300), r.randint(-300, 300), w, h, r.randint(0, 0x1FF), r.randint(0, 255),
            r.randint(0, 255), r.randint(0, 1023) & ~15, r.randint(0, 511), r.randint(-5, 20), r.randint(-5, 20), 0),
            r.choice([9, 0, 15])))
    return lines


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
