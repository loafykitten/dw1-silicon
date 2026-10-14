#!/usr/bin/env python3
"""The current screen's tile triggers and warps, from a DW1STATE dump (oracle/capture-calls.lua with
DW1_CAPTURE_STATES=1, taken while the screen is loaded). For finding routes: where to stand to
use a sign, a door or an exit.

    tools/oracle/map_triggers.py call0000_in.bin

Tile triggers (MAP_COLLISION_DATA, 100 x 100 bytes at 0x801AF398; checkMapInteraction in
decomp/src/main/tamer.c): 51-79 run a map script section when stepped on, 80-109 when Cross is
pressed on them, 110-119 are warps (exit trigger - 110 indexes MAP_WARPS), 120 the toilet. A tile
(tx, tz) covers world x from (tx - 50) * 100 and z down from (50 - tz) * 100 (getTileTrigger in
decomp/src/main/map_collision.c). MAP_WARPS (0x80138730): spawnX/Y/Z, rotation, targetMap,
targetExit, ten halfwords each.
"""
import struct
import sys
from collections import defaultdict

REGS = 131 * 4


def main(path):
    b = open(path, 'rb').read()
    assert b[:8] == b'DW1STATE', path
    ram = b[8 + REGS + 1024:]

    def rd(addr, fmt):
        return struct.unpack_from('<' + fmt, ram, addr & 0x1FFFFF)

    screen = ram[0x134DA8]
    print(f'screen {screen}')
    grid = ram[0x1AF398:0x1AF398 + 10000]
    tiles = defaultdict(list)
    for tz in range(100):
        for tx in range(100):
            v = grid[tx + tz * 100]
            if 50 < v <= 120:
                tiles[v].append((tx, tz))
    warps = rd(0x80138730, '60h')
    target_map, target_exit = warps[40:50], warps[50:60]
    for v in sorted(tiles):
        xs = [(tx - 50) * 100 for tx, _ in tiles[v]]
        zs = [(50 - tz) * 100 for _, tz in tiles[v]]
        kind = ('step' if v < 80 else 'cross' if v < 110 else 'warp' if v < 120 else 'toilet')
        extra = ''
        if kind == 'warp':
            extra = f' -> screen {target_map[v - 110]} exit {target_exit[v - 110]}'
        print(f'trigger {v:3} {kind:6} {len(tiles[v]):3} tiles  x {min(xs)}..{max(xs) + 100}'
              f'  z {min(zs) - 100}..{max(zs)}{extra}')


if __name__ == '__main__':
    main(sys.argv[1])
