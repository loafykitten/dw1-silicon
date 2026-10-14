#!/usr/bin/env python3
"""Request file for oracle/call-sdk.lua that exercises libgs's `GsSortOt(src, dst)`, for
`crates/psyq/tests/libgs_sortot_oracle.rs` to compare against.

Usage: sortot_requests.py OUT_FILE [--seed N] [--count K]

Run it on the emulator (one emulator at a time; a fresh empty directory under
captures/oracle-runs/, e.g. libgs-sortot, holding req.txt; see oracle/call-sdk.lua) and keep
req.txt and results.txt there (gitignored; `DW1_LIBGS_SORTOT_CAPTURE` points the test elsewhere).

Every request builds two GsOTs (the source at B+0x100 with its entries at B+0x200, the
destination at B+0x1000 with its entries at B+0x1100), chains the entries of each table (some through
three-word packets) as GsClearOt and the sort calls leave them (random links crashed the SDK,
which walks the chain), calls `GsSortOt(src, dst)` and reads back both tables, both structs and
the return value. Lengths 0 to 5, `point` and `offset` below half the destination's size.
"""
import argparse
import random
import struct

BUF = 0x801E0000
SRC, SRC_ORG, DST, DST_ORG = 0x100, 0x200, 0x1000, 0x1100


def ot_bytes(length, org, offset, point):
    tag = org + 4 * ((1 << length) - 1)
    return struct.pack('<IIIII', length, org, offset, point, tag)


PACKETS = 0x2000
SENTINEL = 0x00116BF8  # where an emptied table's entry 0 points (the SDK's own empty packets)


END = 0x00FFFFFF
# How entry 0 (the end of a table's chain) looks: empty, or through one or two packets, ending
# at the SDK's sentinel packets or at the end marker.
MODES = ['sentinel', 'end', 'pk-sentinel', 'pk-end', 'pk-pk-sentinel', 'pk-pk-end']


def table(r, org, n, packet_base, mode='sentinel'):
    """A well-formed table of n entries: each links to the one below, with some entries going
    through a three-word packet first; entry 0 as `mode` says. Returns the entry bytes and the
    packet area bytes (entry 0's packets first)."""
    end = END if mode.endswith('end') else SENTINEL
    packets = b''
    if mode.startswith('pk-pk'):
        a0, a1 = packet_base, packet_base + 16
        packets += struct.pack('<4I', 2 << 24 | (a1 & 0xFFFFFF), *[r.getrandbits(32) for _ in range(3)])
        packets += struct.pack('<4I', 3 << 24 | end, *[r.getrandbits(32) for _ in range(3)])
        entries = [a0 & 0xFFFFFF]
    elif mode.startswith('pk'):
        packets += struct.pack('<4I', 3 << 24 | end, *[r.getrandbits(32) for _ in range(3)])
        entries = [packet_base & 0xFFFFFF]
    else:
        entries = [end]
    for i in range(1, n):
        below = (org + 4 * (i - 1)) & 0xFFFFFF
        if r.random() < 0.4:
            at = packet_base + len(packets)
            packets += struct.pack('<I', 3 << 24 | below) + struct.pack('<3I', *[r.getrandbits(32) for _ in range(3)])
            entries.append(at & 0xFFFFFF)
        else:
            entries.append(below)
    return struct.pack('<%dI' % n, *entries), packets


def request(r, ls, ld, point, offset, dst_point=0, dst_offset=0, mode='sentinel'):
    ns, nd = 1 << ls, 1 << ld
    src_entries, src_packets = table(r, BUF + SRC_ORG, ns, BUF + PACKETS, mode)
    dst_entries, dst_packets = table(r, BUF + DST_ORG, nd, BUF + PACKETS + 0x400,
                                     r.choice(['sentinel', 'pk-sentinel']))
    ins = [
        ('B+%x' % SRC, ot_bytes(ls, BUF + SRC_ORG, offset, point).hex()),
        ('B+%x' % SRC_ORG, src_entries.hex()),
        ('B+%x' % DST, ot_bytes(ld, BUF + DST_ORG, dst_offset, dst_point).hex()),
        ('B+%x' % DST_ORG, dst_entries.hex()),
    ]
    if src_packets:
        ins.append(('B+%x' % PACKETS, src_packets.hex()))
    if dst_packets:
        ins.append(('B+%x' % (PACKETS + 0x400), dst_packets.hex()))
    outs = [('B+%x' % SRC, 20), ('B+%x' % SRC_ORG, 4 * ns), ('B+%x' % DST, 20), ('B+%x' % DST_ORG, 4 * nd)]
    if src_packets:
        outs.append(('B+%x' % PACKETS, len(src_packets)))
    if dst_packets:
        outs.append(('B+%x' % (PACKETS + 0x400), len(dst_packets)))
    parts = ['GsSortOt', 'B+%x' % SRC, 'B+%x' % DST, '0', '0']
    parts += ['in=%s:%s' % i for i in ins]
    parts += ['out=%s:%x' % o for o in outs]
    return ' '.join(parts)


def gen(seed, count):
    r = random.Random(seed)
    lines = []
    # Every request stays inside the destination table whichever way `point` and `offset` index
    # it (a wild write crashed the emulated game in the first attempt): point and offset are
    # below half the table size, so even point + offset is.
    # Entry 0 of the source in every shape, with and without packets in the other entries.
    for mode in MODES:
        for ls in range(0, 5):
            for _ in range(3):
                lines.append(request(r, ls, 3, r.randint(0, 3), 0, mode=mode))
    for ld in range(1, 6):
        for point in range(0, 1 << (ld - 1)):
            lines.append(request(r, r.randint(0, 4), ld, point, 0))
        for offset in range(0, 1 << (ld - 1)):
            lines.append(request(r, r.randint(0, 4), ld, 1, offset))
    for _ in range(count):
        ld = r.randint(1, 5)
        half = 1 << (ld - 1)
        lines.append(request(r, r.randint(0, 4), ld, r.randint(0, half - 1), r.randint(0, half - 1),
                             r.randint(0, 6), r.randint(0, 6), mode=r.choice(MODES)))
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--count', type=int, default=200)
    a = ap.parse_args()
    with open(a.out, 'w') as f:
        f.write('\n'.join(gen(a.seed, a.count)) + '\n')


if __name__ == '__main__':
    main()
