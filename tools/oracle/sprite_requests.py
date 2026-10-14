#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that exercises libgs's `GsSortSprite`, for
`crates/psyq/tests/libgs_sprite_oracle.rs` to compare against.

Usage: sprite_requests.py OUT_FILE [--seed N] [--scale K] [--set clock|battle]

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

`--set battle` (Phase 5, `captures/oracle-runs/libgs-sprite-btl`) covers what the first battle
draws (`captures/oracle-runs/p5-btl-calls`): scaled sprites with attribute 0 or 0x50000000
(semi-transparent, additive) in grey or other colours, scaled and rotated ones, and
`GsSortFastSprite` (which the battle calls with scale 0 and widths from 0 to 255), with the other
semi-transparency rates, colour modes and the brightness bit around them, then every attribute
combination in every form and sides of 0 and over 255.
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


def request(sprite, pri, func='GsSortSprite'):
    ins = [
        ('B+%x' % SPRITE, sprite),
        ('B+%x' % OT, struct.pack('<IIIII', OT_LENGTH, BUF + ORG, 0, 0, BUF + ORG + 4 * ((1 << OT_LENGTH) - 1)).hex()),
        ('B+%x' % ORG, struct.pack('<16I', *([0x00FFFFFF] * 16)).hex()),
        ('B+%x' % PACKETS, ('a5' * 0x80)),
        ('%x' % G_OUT_PACKET_P, struct.pack('<I', BUF + PACKETS).hex()),
    ]
    outs = [('B+%x' % PACKETS, 0x80), ('B+%x' % ORG, 64), ('%x' % G_OUT_PACKET_P, 4), ('B+%x' % SPRITE, 36)]
    parts = [func, 'B+%x' % SPRITE, 'B+%x' % OT, '%x' % pri, '0']
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


def gen_battle(seed, scale):
    r = random.Random(seed)
    lines = []
    attrs = [0, 0x50000000, 0x40000000, 0x60000000, 0x70000000]

    def rgb():
        return r.choice([(128, 128, 128), (64, 64, 64), (r.randint(0, 255),) * 3,
                         (r.randint(0, 255), r.randint(0, 255), r.randint(0, 255)), (0, 0, 0), (255, 255, 255)])

    def scale_value():
        return r.choice([4096, 512, 1055, 1504, 2048, 3413, 8192, r.randint(1, 4096), r.randint(1, 16384),
                         r.randint(-4096, 4096), 0, 1, -1])

    def any_sprite(rotate, sx, sy, attribute, colour):
        w, h = r.choice([8, 16, 24, 32, 64, r.randint(1, 255)]), r.choice([8, 16, 24, 32, 64, r.randint(1, 255)])
        mx, my = r.choice([(0, 0), (16, 16), (3, 13), (w // 2, h // 2), (r.randint(-20, 300), r.randint(-20, 300))])
        return sprite_bytes(r.randint(-300, 300), r.randint(-300, 300), w, h,
                            r.choice([0x1E, 0x3C, 0x1F, r.randint(0, 0x1FF)]), r.randint(0, 255), r.randint(0, 255),
                            r.randint(0, 1023) & ~15, r.randint(0, 511), mx, my, rotate, sx, sy, colour, attribute)

    # The battle's own sprites: the 32x32 additive glow, the 8x16 / 24x24 clock-hand-like ones.
    for _ in range(150 * scale):
        s = r.choice([1055, 1504, 2048, r.randint(1000, 4096)])
        g = r.choice([64, 70, 96, 121, 128])
        lines.append(request(sprite_bytes(r.randint(-160, 160), r.randint(-120, 120), 32, 32, 0x3C,
                                          r.randint(0, 224), r.randint(0, 224), r.randint(0, 63) * 16,
                                          r.randint(0, 511), 16, 16, 0, s, s, (g, g, g), 0x50000000), 9))
    for _ in range(100 * scale):
        s = r.choice([512, 1194, 1877, 2560, 3413, 3584, 3754, 3925, 4096])
        w, h, mx, my = r.choice([(8, 16, 3, 13), (24, 24, 11, 12)])
        rot = r.choice([5040, 5400, 147456, 638976, 663552, 688128, 712704, 737280, r.randint(0, 360 * 4096)])
        lines.append(request(sprite_bytes(r.randint(-160, 160), r.randint(-120, 120), w, h, 0x1E, r.randint(0, 255),
                                          r.randint(0, 255), 256, 499, mx, my, rot, s, s), 9))
    # Around them: scaled, rotated or both, every semi-transparency rate, any colour.
    for _ in range(300 * scale):
        rot = r.choice([0, 0, r.randint(0, 360 * 4096), r.randint(-720 * 4096, 720 * 4096)])
        sx = scale_value()
        sy = r.choice([sx, scale_value()])
        lines.append(request(any_sprite(rot, sx, sy, r.choice(attrs), rgb()), r.choice([9, 0, 15])))
    # GsSortFastSprite: as the battle calls it, then anything.
    for _ in range(100 * scale):
        lines.append(request(sprite_bytes(r.randint(-160, 160), r.randint(-120, 120), r.randint(0, 255), 3, 0x1F,
                                          r.randint(0, 255), r.randint(0, 255), r.randint(0, 63) * 16,
                                          r.randint(0, 511), 0, 0, 0, 0, 0,
                                          (r.randint(128, 255), 255, 128), 0), 7, 'GsSortFastSprite'))
    for _ in range(200 * scale):
        lines.append(request(any_sprite(r.choice([0, r.randint(-1000000, 1000000)]), scale_value(), scale_value(),
                                        r.choice(attrs + [0x01000000, 0x02000000, 0x40]), rgb()),
                             r.choice([7, 0, 15]), 'GsSortFastSprite'))
    # The other attribute bits on GsSortSprite (colour modes, brightness off), unscaled.
    for _ in range(100 * scale):
        lines.append(request(any_sprite(r.choice([0, r.randint(0, 360 * 4096)]), 4096, 4096,
                                        r.choice([0x01000000, 0x02000000, 0x40, 0x41000000, 0x80000000]), rgb()),
                             9))
    # Every attribute combination (brightness off, colour modes 0-3, the rate with and without
    # GsALON, display off) in each form (SPRT, scaled, rotated), and on GsSortFastSprite.
    combos = [b | m << 24 | r << 28 | a | d
              for b in (0, 0x40) for m in range(4) for r in range(4) for a in (0, 0x40000000)
              for d in (0, 0x80000000)]
    for attr in combos:
        for rot, sc in ((0, 4096), (0, r.choice([1055, 2048, 8192])), (r.randint(1, 360 * 4096), 4096)):
            lines.append(request(any_sprite(rot, sc, sc, attr, rgb()), 9))
        lines.append(request(any_sprite(0, 4096, 4096, attr, rgb()), 7, 'GsSortFastSprite'))
    # Sides of 0 and over 255, shown and hidden, on both entry points.
    for func in ('GsSortSprite', 'GsSortFastSprite'):
        for w, h in ((0, 3), (0, 0), (0, 300), (8, 0), (256, 8), (8, 256), (300, 300)):
            for attr in (0, 0x80000000, 0x50000000):
                for rot in ((0, r.randint(1, 360 * 4096)) if func == 'GsSortSprite' else (0,)):
                    lines.append(request(sprite_bytes(r.randint(-100, 100), r.randint(-100, 100), w, h, 0x1E,
                                                      r.randint(0, 255), r.randint(0, 255), 256, 499, 3, 5, rot,
                                                      4096, 4096, rgb(), attr), 9, func))
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--seed', type=int, default=20261014)
    ap.add_argument('--scale', type=int, default=1)
    ap.add_argument('--set', choices=['clock', 'battle'], default='clock')
    a = ap.parse_args()
    lines = (gen_battle if a.set == 'battle' else gen)(a.seed, a.scale)
    with open(a.out, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(len(lines), 'requests')


if __name__ == '__main__':
    main()
