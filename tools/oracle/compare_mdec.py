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
  credits), and the differences must not lean: each channel's mean signed difference over all of
  that channel's compared samples stays within --bias (default 0.1 of a level; measured at most
  0.04; rounding the colour conversion down instead of to the nearest leaned about 0.5). Needs the port's pixels for every sampled frame
  (DW1_MDEC_LOG_PIXELS); a frame without them fails, as does a log record whose bytes aren't all
  in its binary file or a slice whose size differs from the other side's.

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

    stats, report = collections.Counter(), []

    def bad_record(what):
        stats['bad log records'] += 1
        if len(report) < a.show:
            report.append(what)

    def payload(data, off, n, where):
        """`n` bytes at `off` of a log's binary file, or None (counted) if they aren't all there."""
        if off < 0 or n <= 0 or off + n > len(data):
            bad_record(f'{where}: {n} bytes at {off} outside the {len(data)}-byte file')
            return None
        return data[off:off + n]

    obin = open(a.oracle_dir + '/mdec.bin', 'rb').read()
    ora = []  # [vblank, ordinal, codes, [slices]]
    for line in open(a.oracle_dir + '/mdec-log.txt'):
        f = line.split()
        if f[1] == 'frame':
            ora.append([int(f[0]), int(f[2]), payload(obin, int(f[4]), int(f[5]), 'emulator codes'),
                        []])
        elif f[1] == 'slice' and ora and int(f[2]) == ora[-1][1]:
            w, h, off, n = int(f[5]), int(f[6]), int(f[7]), int(f[8])
            if n != w * h * 2:
                bad_record(f'emulator slice of {w}x{h} halfwords logged with {n} bytes')
                ora[-1][3].append(None)
            else:
                ora[-1][3].append(payload(obin, off, n, 'emulator slice'))

    nbin = open(a.native_prefix + '.bin', 'rb').read()
    nat = []  # [codes, [(bytes logged, hash, pixels or None)]]
    for line in open(a.native_prefix + '.txt'):
        f = line.split()
        if f[0] == 'frame':
            nat.append([payload(nbin, int(f[3]), int(f[4]), 'port codes'), []])
        elif f[0] == 'slice' and nat:
            off, n, h = int(f[2]), int(f[3]), int(f[4], 16)
            nat[-1][1].append((n, h, payload(nbin, off, n, 'port slice') if off >= 0 else None))

    diff_bytes, max_diff = 0, 0
    signed, counted = [0, 0, 0], [0, 0, 0]
    hist = collections.Counter()
    j = 0
    for vb, k, codes, slices in ora:
        if codes is None or any(sl is None for sl in slices):
            continue  # counted as a bad record
        if not slices:
            stats['frame without pixels'] += 1
            if len(report) < a.show:
                report.append(f'VBlank {vb} frame {k}: the emulator logged no slices')
            continue
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
        if any(n != len(sl) for sl, (n, _, _) in zip(slices, nslices)):
            stats['slice size differs'] += 1
            if len(report) < a.show:
                report.append(f'VBlank {vb} frame {k}: slice sizes differ')
            continue
        if any(nb is None for _, _, nb in nslices):
            # Without the port's pixels neither the tolerance nor the lean can be judged.
            stats['frame without pixels'] += 1
            if len(report) < a.show:
                report.append(f'VBlank {vb} frame {k}: the port logged no pixels for it')
            continue
        same, close = True, True
        for sl, (_, h, nb) in zip(slices, nslices):
            for c in range(3):
                counted[c] += len(sl[c::3])
            if fnv(sl) == h and sl == nb:
                continue
            same = False
            for i, (x, y) in enumerate(zip(sl, nb)):
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
    # Each channel's mean over that channel's samples (a slice is whole RGB pixels).
    lean = [v / n if n else 0 for v, n in zip(signed, counted)]
    print('mean signed difference (port - emulator) per channel R, G, B:',
          ', '.join(f'{v:+.3f}' for v in lean))
    bad = (stats['codes not found natively'] + stats['slice count differs'] + stats['frame differs']
           + stats['slice size differs'] + stats['frame without pixels'] + stats['bad log records'])
    if any(abs(v) > a.bias for v in lean):
        print(f'the port\'s pictures lean against the emulator\'s by more than {a.bias}')
        bad += 1
    if stats['codes same'] == 0:
        print('no frame compared')
        bad += 1
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
