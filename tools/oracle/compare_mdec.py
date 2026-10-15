#!/usr/bin/env python3
"""Compare the port's movie decoding with the emulator's on a replay (docs/phase2-movie.md).

    tools/oracle/compare_mdec.py ORACLE_DIR NATIVE_PREFIX [--show N] [--tolerance T] [--bias B]
    tools/oracle/compare_mdec.py --hashes ORACLE_DIR > hashes.txt   (for DW1_MDEC_LOG_PIXELS)

ORACLE_DIR holds oracle/capture-mdec.lua's mdec-log.txt and mdec.bin (sampled frames: the
run-length codes the SDK's DecDCTvlc2 made, and each slice the MDEC decoded, as MOV uploads it);
NATIVE_PREFIX is the port's DW1_MDEC_LOG from the same replay (every frame's codes, every slice's
hash, and with DW1_MDEC_LOG_PIXELS=1 the slices' bytes).

- Codes: each sampled frame's codes must be some native frame's, in order (frames line up by
  their codes, so a movie the two sides start at different VBlanks still pairs).
- Pictures: the frame's slices, in upload order, against the native ones (24-bit RGB). They aren't
  byte for byte the same and can't be from documentation alone: Redux's MDEC computes the IDCT
  with a fast (AAN) factorisation of its own, the port psx-spx's matrix form, and psx-spx says the
  hardware's exact rounding isn't known. So a channel may differ by --tolerance levels (default
  6; measured 2026-10-15: at most 4 on the new-game movie, 5 in two corner pixels of the
  credits), and the differences must not lean: the mean signed difference per channel over every
  compared byte stays within --bias (default 0.1 of a level; rounding the colour conversion down
  instead of to the nearest leaned about 0.5). Needs DW1_MDEC_LOG_PIXELS=1 natively; with only
  the hashes, any difference fails.

Exits non-zero if any code stream differs or isn't found, or the pictures differ beyond that.
"""
import argparse
import collections
import sys


def fnv(data):
    h = 0xcbf29ce484222325
    for b in data:
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


def hashes(oracle_dir):
    """The FNV-1a of each sampled frame's codes, for the port's DW1_MDEC_LOG_PIXELS."""
    obin = open(oracle_dir + '/mdec.bin', 'rb').read()
    for line in open(oracle_dir + '/mdec-log.txt'):
        f = line.split()
        if f[1] == 'frame':
            print(f'{fnv(obin[int(f[4]):int(f[4]) + int(f[5])]):016x}')


def main():
    if len(sys.argv) == 3 and sys.argv[1] == '--hashes':
        hashes(sys.argv[2])
        return
    ap = argparse.ArgumentParser()
    ap.add_argument('oracle_dir')
    ap.add_argument('native_prefix')
    ap.add_argument('--show', type=int, default=10)
    ap.add_argument('--tolerance', type=int, default=6)
    ap.add_argument('--bias', type=float, default=0.1)
    a = ap.parse_args()

    obin = open(a.oracle_dir + '/mdec.bin', 'rb').read()
    ora = []  # [vblank, ordinal, codes, [slices]]
    for line in open(a.oracle_dir + '/mdec-log.txt'):
        f = line.split()
        if f[1] == 'frame':
            off, n = int(f[4]), int(f[5])
            ora.append([int(f[0]), int(f[2]), obin[off:off + n], []])
        elif f[1] == 'slice' and ora and int(f[2]) == ora[-1][1]:
            off, n = int(f[7]), int(f[8])
            ora[-1][3].append(obin[off:off + n])

    nbin = open(a.native_prefix + '.bin', 'rb').read()
    nat = []  # [codes, [(hash, bytes or None)]]
    for line in open(a.native_prefix + '.txt'):
        f = line.split()
        if f[0] == 'frame':
            off, n = int(f[3]), int(f[4])
            nat.append([nbin[off:off + n], []])
        elif f[0] == 'slice' and nat:
            off, n, h = int(f[2]), int(f[3]), int(f[4], 16)
            nat[-1][1].append((h, nbin[off:off + n] if off >= 0 else None))

    stats, report = collections.Counter(), []
    diff_bytes, max_diff, compared = 0, 0, 0
    signed = [0, 0, 0]
    hist = collections.Counter()
    j = 0
    for vb, k, codes, slices in ora:
        m = next((i for i in range(j, len(nat)) if nat[i][0] == codes), None)
        if m is None:
            stats['codes not found natively'] += 1
            if len(report) < a.show:
                report.append(f'VBlank {vb} frame {k}: no native frame has these run-length codes')
            continue
        stats['codes same'] += 1
        j = m + 1
        nslices = nat[m][1]
        if len(nslices) != len(slices):
            stats['slice count differs'] += 1
            if len(report) < a.show:
                report.append(f'VBlank {vb} frame {k}: {len(slices)} slices, natively {len(nslices)}')
            continue
        same, close = True, True
        for s, (h, nb) in zip(slices, nslices):
            compared += len(s)
            if fnv(s) == h:
                continue
            same = False
            if nb is None:
                close = False
                continue
            for i, (x, y) in enumerate(zip(s, nb)):
                if x != y:
                    diff_bytes += 1
                    d = abs(x - y)
                    hist[d] += 1
                    max_diff = max(max_diff, d)
                    signed[i % 3] += y - x
                    if d > a.tolerance:
                        close = False
        stats['frame same' if same else 'frame within tolerance' if close else 'frame differs'] += 1
        if not close and len(report) < a.show:
            report.append(f'VBlank {vb} frame {k} (native frame {m + 1}): pixels differ')
    for r in report:
        print(r)
    print('mdec:', dict(stats))
    if diff_bytes:
        print(f'differing channel bytes: {diff_bytes}, largest difference {max_diff}, '
              f'by size {dict(sorted(hist.items()))}')
    lean = [v / compared for v in signed] if compared else [0, 0, 0]
    print('mean signed difference (port - emulator) per channel R, G, B:',
          ', '.join(f'{v:+.3f}' for v in lean))
    bad = (stats['codes not found natively'] + stats['slice count differs'] + stats['frame differs'])
    if any(abs(v) > a.bias for v in lean):
        print(f'the port\'s pictures lean against the emulator\'s by more than {a.bias}')
        bad += 1
    if stats['codes same'] == 0:
        print('no frame compared')
        bad += 1
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
