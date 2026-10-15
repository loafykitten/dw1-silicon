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
- every sector of a movie the port decoded and the emulator was never seen decoding (its samples
  changed between two VBlanks only when the sector's did) has the same samples as the sector
  before it (silence repeats), so nothing was skipped or added;
- the VBlank the port starts playing a sector (its first 44.1 kHz frame leaves the CD input
  FIFO), placed on the emulator's clock through the replay's polls (a movie played without
  polls, the credits, is lined up by its median offset and judged within itself), is within --slack VBlanks
  (default 2) of the VBlank at whose end the emulator's save state first shows it: a save state
  is taken once a VBlank, the port's sector starts somewhere in its VBlank, and a movie's polls
  are 2 VBlanks apart, so each side is known to about a VBlank (on the new-game replay's movie
  1,044 of 1,627 sectors land on the same VBlank, 581 one apart, 2 two apart).

- the port's CD input never runs dry inside a read (a movie's audio plays without a gap, as on
  the PS1; a gap at the end of a read, before another movie or a re-read, is expected).

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
    # Gaps in the port's playback: the CD input found its FIFO empty (the decoder's underrun count
    # rose) between two pop lines (written every VBlank while it plays, so between movies one
    # spans the whole wait). That's expected only at a read's end: no sector follows the last one
    # decoded before the gap, or the next is not the next audio sector of the same read (another
    # movie, or the game re-reading after a lost frame, which the emulator does too). Inside a
    # read it's a dropout the PS1 doesn't have.
    sector_vbs = [vb for _, _, _, vb, _ in sectors]
    for (_, vb0, u0), (_, vb, u) in zip(pops, pops[1:]):
        if u <= u0:
            continue
        k = bisect.bisect_right(sector_vbs, vb0)
        before = sectors[k - 1][0] if k > 0 else None
        after = sectors[k][0] if k < len(sectors) else None
        if before is None or after is None or after != before + 8:
            stats['gaps at a read\'s end'] += 1
        else:
            stats['gaps inside a read'] += 1
            if len(report) < a.show:
                report.append(f'the port\'s CD input ran dry for {u - u0} frames by its VBlank {vb}, '
                              f'between LBAs {before} and {after}')
    # The port's reads: audio sectors come every 8 sectors in a movie, so a larger step starts
    # another movie (or another stretch of one).
    run_of, run = {}, 0
    for i, lba in enumerate(audio_lbas):
        if i and lba - audio_lbas[i - 1] != 8:
            run += 1
        run_of[lba] = run

    opcm = open(os.path.join(a.oracle_dir, 'xa-pcm.bin'), 'rb').read()
    seen = []
    timed = []  # (lba, emulator VBlank, port heard VBlank, the port's poll count then)
    first = last = None
    for line in open(os.path.join(a.oracle_dir, 'xa-log.txt')):
        f = line.split()
        vb = int(f[0])
        at_start = first is None
        first = vb if first is None else first
        last = vb
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
    # Timing. The two clocks are tied at the pad polls; a movie the game plays without reading the
    # pad (the credits) has none, and the port's VBlanks from the poll before or after it include
    # loading the port does in no time. So a stretch of sectors placed from the same poll and
    # spanning more than 60 VBlanks is lined up by its median offset (reported) and judged on the
    # timing within it.
    offsets = collections.Counter()
    shifts = []
    groups = []
    for t in timed:
        if groups and groups[-1][-1][3] == t[3] and run_of[groups[-1][-1][0]] == run_of[t[0]]:
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
    # Within each of the port's reads, the sectors between the first and last the emulator showed:
    # an unseen one must repeat the one before it (its decode changed nothing the emulator could
    # see).
    seen_set = set(seen)
    runs = {}
    for lba in seen_set:
        lo, hi = runs.get(run_of[lba], (lba, lba))
        runs[run_of[lba]] = (min(lo, lba), max(hi, lba))
    runs = sorted(runs.values())
    for lo, hi in runs:
        prev = None
        for lba in audio_lbas[bisect.bisect_left(audio_lbas, lo):bisect.bisect_right(audio_lbas, hi)]:
            pcm = nat[lba][-1][1]
            if lba not in seen_set:
                if prev is not None and pcm == prev:
                    stats['unseen, repeats the sector before'] += 1
                else:
                    stats['unseen and different'] += 1
                    if len(report) < a.show:
                        report.append(f'LBA {lba}: decoded by the port, never seen on the emulator')
            prev = pcm
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
           + stats['gaps inside a read'] + stats['bad log records'])
    if stats['samples same'] == 0:
        print('no sector compared')
        bad += 1
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
