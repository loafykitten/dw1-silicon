#!/usr/bin/env python3
"""Compare the port's movie XA audio with the emulator's on a replay (docs/phase6-plan.md, "XA
audio against the emulator").

    tools/oracle/compare_xa.py ORACLE_DIR NATIVE_PREFIX REPLAY [--show N] [--slack V]

ORACLE_DIR holds oracle/capture-xa.lua's xa-log.txt and xa-pcm.bin (the emulator's last decoded
XA sector, VBlank by VBlank); NATIVE_PREFIX is the port's DW1_XA_LOG (<prefix>.txt, .bin) from
the same replay. The emulator's samples are its CD controller's ADPCM decode (37.8 kHz, before
the 44.1 kHz interpolation), which is what the port's decoder's last_raw holds, so they compare
bit for bit:

- every sector the emulator decoded (its save state's read position says which: the latest
  audio sector at or below it) has exactly the samples the port decoded for that sector;
- every audio sector between the first and last the emulator showed in one of its reads was
  decoded by the port, and one the emulator was never seen decoding (its samples changed between
  two VBlanks only when the sector's did) has the same samples as the sector before it (silence
  repeats), so nothing was skipped or added;
- the VBlank the port starts playing a sector (its first 44.1 kHz frame leaves the CD input
  FIFO), placed on the emulator's clock through the replay's polls (a movie played without
  polls, the credits, is lined up by its median offset and judged within itself), is within
  --slack VBlanks (default 2) of the VBlank at whose end the emulator's save state first shows it: a save state
  is taken once a VBlank, the port's sector starts somewhere in its VBlank, and a movie's polls
  are 2 VBlanks apart, so each side is known to about a VBlank (on the new-game replay's movie
  1,044 of 1,627 sectors land on the same VBlank, 581 one apart, 2 two apart);
- every audio sector the emulator read (its read position, VBlank by VBlank, passed through it
  in one read) was decoded by the port, and the port's CD input never runs dry except after the
  last audio sector of one of the emulator's reads (a movie's end, a re-read): reads are taken
  from the emulator's side, not from what the port decoded.

Exits non-zero on any difference. Levels after the decoder aren't compared: Redux plays XA on
its own audio stream without the SPU's CD volume, which the port applies as psx-spx documents.
"""
import argparse
import bisect
import collections
import os
import sys


def replay_polls(path):
    vbs = []
    for line in open(path):
        f = line.split()
        if len(f) == 4 and f[0].isdigit() and f[1].isdigit():
            vbs.append(int(f[1]))
    return vbs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('oracle_dir')
    ap.add_argument('native_prefix')
    ap.add_argument('replay')
    ap.add_argument('--show', type=int, default=10)
    ap.add_argument('--slack', type=int, default=2)
    a = ap.parse_args()

    polls = replay_polls(a.replay)

    # The port's VBlank of each poll it logged ("poll vblank polls": polls so far, the latest at
    # that VBlank). An event is placed from the nearer of the polls either side of it on the
    # port's clock: the one before (plus the VBlanks since) or the one after (minus the VBlanks
    # to it). Loading takes the port no time, so the nearer poll is the better anchor (a movie
    # starts just after a load, and its sectors before its first poll are placed from that poll).
    poll_vb = []  # (polls, the port's VBlank of poll number `polls`)
    for line in open(a.native_prefix + '.txt'):
        f = line.split()
        if f[0] == 'poll':
            poll_vb.append((int(f[2]), int(f[1])))
    poll_counts = [p for p, _ in poll_vb]

    def on_oracle_clock(npolls, since, vb):
        """(the VBlank on the emulator's clock, the poll it was placed from), or None."""
        prev = (since, polls[npolls - 1] + since, npolls) if 0 < npolls <= len(polls) else None
        k = bisect.bisect_left(poll_counts, npolls + 1)
        nxt = None
        if k < len(poll_vb) and npolls < len(polls):
            to = poll_vb[k][1] - vb
            nxt = (to, polls[npolls] - to, npolls + 1)
        best = min((c for c in (prev, nxt) if c is not None), default=None)
        return (best[1], best[2]) if best else None

    report, stats = [], collections.Counter()
    npcm = open(a.native_prefix + '.bin', 'rb').read()
    nat = collections.defaultdict(list)  # lba -> [((heard on the oracle's clock, anchor), pcm)]
    sectors, pops = [], []
    for line in open(a.native_prefix + '.txt'):
        f = line.split()
        if f[0] == 'sector':
            vb, np_, since, lba, _arr, frames, off, pushed = map(int, f[1:])
            if off < 0 or frames <= 0 or off + frames * 4 > len(npcm):
                stats['bad log records'] += 1
                continue
            sectors.append((lba, npcm[off:off + frames * 4], pushed, vb,
                            on_oracle_clock(np_, since, vb)))
        elif f[0] == 'pop':
            pops.append((int(f[4]), int(f[1]), int(f[5])))
    popped = [p[0] for p in pops]
    for lba, pcm, pushed, vb, placed in sectors:
        # Heard from the first VBlank whose pops pass the frames queued before it: placed on the
        # emulator's clock from the sector's own decode (a VBlank or two before, so a poll in
        # between, maybe followed by a load the port skips, doesn't move it).
        k = bisect.bisect_right(popped, pushed)
        heard = None
        if k < len(pops) and placed is not None:
            heard = (placed[0] + pops[k][1] - vb, placed[1])
        nat[lba].append((heard, pcm))
    audio_lbas = sorted(nat)
    opcm = open(os.path.join(a.oracle_dir, 'xa-pcm.bin'), 'rb').read()
    seen = []
    timed = []  # (lba, emulator VBlank, port heard VBlank, the port's poll count then)
    trace = []  # the emulator's read position, VBlank by VBlank (the last sector read)
    first = last = None
    for line in open(os.path.join(a.oracle_dir, 'xa-log.txt')):
        f = line.split()
        vb = int(f[0])
        at_start = first is None
        first = vb if first is None else first
        last = vb
        if not at_start and f[1] in ('xa', 'pos'):
            trace.append((vb, int(f[2])))
        if f[1] != 'xa' or at_start:
            # The first VBlank's save state shows whatever was decoded before the capture began.
            continue
        pos, n, off = int(f[2]), int(f[12]), int(f[13])
        pcm = opcm[off:off + n * 4]
        if int(f[3]) != 128 or int(f[4]) != 0 or int(f[5]) != 128 or int(f[6]) != 0:
            stats['emulator ATV not unity'] += 1
        k = bisect.bisect_right(audio_lbas, pos) - 1
        lba = audio_lbas[k] if k >= 0 else None
        if lba is None or pos - lba > 7:
            stats['emulator sector the port never decoded'] += 1
            if len(report) < a.show:
                report.append(f'VBlank {vb}: the emulator decoded a sector near LBA {pos}, the port none')
            continue
        seen.append(lba)
        same = [h for h, p in nat[lba] if p == pcm]
        if not same:
            stats['samples differ'] += 1
            if len(report) < a.show:
                report.append(f'VBlank {vb}: LBA {lba} decodes differently')
            continue
        stats['samples same'] += 1
        heard = min((h for h in same if h is not None), key=lambda h: abs(h[0] - vb), default=None)
        if heard is None:
            stats['never played by the port'] += 1
            continue
        timed.append((lba, vb, heard[0], heard[1]))
    # The emulator's reads, from its own read position (not from what the port decoded): a step
    # of 1-8 sectors from one VBlank to the next is the same read going on; anything else (a seek,
    # a stop, a re-read) ends one.
    segments = []  # [first LBA, last LBA, first VBlank, last VBlank] of each emulator read
    for (_, p0), (v1, p1) in zip([(None, None)] + trace, trace):
        if p0 is not None and 0 < p1 - p0 <= 8:
            segments[-1][1], segments[-1][3] = p1, v1
        elif p0 != p1:
            segments.append([p1, p1, v1, v1])

    def seg_of(lba, t=None):
        """The emulator read holding `lba` (the one nearest VBlank `t` on its clock if given)."""
        hits = [i for i, (lo, hi, _, _) in enumerate(segments) if lo - 7 <= lba <= hi]
        if t is None or not hits:
            return hits[0] if hits else None
        return min(hits, key=lambda i: max(segments[i][2] - t, t - segments[i][3], 0))

    # Timing. The two clocks are tied at the pad polls; a movie the game plays without reading the
    # pad (the credits) has none, and the port's VBlanks from the poll before or after it include
    # loading the port does in no time. So a stretch of sectors placed from the same poll and
    # spanning more than 60 VBlanks is lined up by its median offset (reported) and judged on the
    # timing within it.
    offsets = collections.Counter()
    shifts = []
    groups = []
    for t in timed:
        if groups and groups[-1][-1][3] == t[3] and seg_of(groups[-1][-1][0]) == seg_of(t[0]):
            groups[-1].append(t)
        else:
            groups.append([t])
    for g in groups:
        shift = 0
        if g[-1][1] - g[0][1] > 60:
            ds = sorted(h - vb for _, vb, h, _ in g)
            shift = ds[len(ds) // 2]
            shifts.append((g[0][1], g[-1][1], shift))
        for lba, vb, h, _ in g:
            d = h - vb - shift
            offsets[d] += 1
            if abs(d) <= a.slack:
                stats['heard same VBlank' if d == 0 else 'heard within the slack'] += 1
            else:
                stats['heard late/early'] += 1
                if len(report) < a.show:
                    report.append(f'LBA {lba}: emulator decoded by VBlank {vb}, port heard at {h}'
                                  + (f' (stretch lined up by {shift})' if shift else ''))
    seen_set = set(seen)
    # Every audio sector the emulator read through must have been decoded by the port, and one the
    # emulator never showed decoding must repeat the sector before it (silence), between the first
    # and last the emulator showed in that read.
    runs = []
    for lo, hi, _, _ in segments:
        shown = [x for x in seen_set if lo - 7 <= x <= hi]
        # (two reads of the same sectors share them; each is checked over the same span)
        if not shown:
            continue
        runs.append((min(shown), max(shown)))
        prev = None
        for x in range(min(shown), max(shown) + 1, 8):
            if x not in nat:
                stats['read by the emulator, never decoded by the port'] += 1
                if len(report) < a.show:
                    report.append(f'LBA {x}: read by the emulator, never decoded by the port')
                prev = None
                continue
            pcm = nat[x][-1][1]
            if x not in seen_set:
                if prev is not None and pcm == prev:
                    stats['unseen, repeats the sector before'] += 1
                else:
                    stats['unseen and different'] += 1
                    if len(report) < a.show:
                        report.append(f'LBA {x}: decoded by the port, never seen on the emulator')
            prev = pcm
    # Gaps in the port's playback: the CD input found its FIFO empty (the decoder's underrun count
    # rose) between two pop lines (written every VBlank while it plays, so between movies one
    # spans the whole wait). That's allowed only where the emulator's read ended too: the last
    # sector the port decoded before the gap is the last audio sector of an emulator read. A gap
    # after a sector inside one of the emulator's reads is a dropout the PS1 doesn't have, and
    # one where the emulator's reads can't tell (outside the capture) isn't excused either.
    sector_vbs = [vb for _, _, _, vb, _ in sectors]
    for (_, vb0, u0), (_, vb, u) in zip(pops, pops[1:]):
        if u <= u0:
            continue
        k = bisect.bisect_right(sector_vbs, vb0)
        before, placed = (sectors[k - 1][0], sectors[k - 1][4]) if k > 0 else (None, None)
        t = placed[0] if placed else None
        if before is None or t is None or not first - 30 <= t <= last + 30:
            stats['gaps outside the capture'] += 1
            continue
        i = seg_of(before, t)
        if i is None:
            stats['gaps the emulator can\'t vouch for'] += 1
        elif before + 8 > segments[i][1]:  # the emulator's read stopped before the next one
            stats['gaps where the emulator\'s read ended'] += 1
            continue
        else:
            stats['gaps inside a read'] += 1
        if len(report) < a.show:
            report.append(f'the port\'s CD input ran dry for {u - u0} frames by its VBlank {vb}, '
                          f'after LBA {before}')
    for r in report:
        print(r)
    print(f'emulator VBlanks {first}-{last}, movie stretches seen (LBAs) {runs}')
    print('xa:', dict(stats))
    print('port heard - emulator decoded, VBlanks:', dict(sorted(offsets.items())))
    for lo, hi, shift in shifts:
        print(f'emulator VBlanks {lo}-{hi} have no poll natively: lined up by {shift} VBlanks')
    bad = (stats['samples differ'] + stats['emulator sector the port never decoded']
           + stats['heard late/early'] + stats['never played by the port']
           + stats['unseen and different'] + stats['emulator ATV not unity']
           + stats['gaps inside a read'] + stats['gaps the emulator can\'t vouch for']
           + stats['read by the emulator, never decoded by the port'] + stats['bad log records'])
    if stats['samples same'] == 0:
        print('no sector compared')
        bad += 1
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
