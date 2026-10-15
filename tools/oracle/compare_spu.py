#!/usr/bin/env python3
"""Compare the port's SPU events with the emulator's on a replay (docs/phase6-plan.md).

    tools/oracle/compare_spu.py ORACLE_LOG NATIVE_LOG REPLAY [--voices 10-23] [--show N]

ORACLE_LOG is oracle/capture-spu.lua's spu-writes.txt from the replay's route, NATIVE_LOG the
port's DW1_SPU_LOG from the same replay. The two runs' VBlanks don't line up (the port loads
instantly) but their pad polls do, so a port event is placed at the emulator VBlank of the
latest poll before it plus the VBlanks since (from its log); between polls that are a VBlank
apart, which is most of the game, that's the emulator's own VBlank numbering.

Compared, per voice: key-ons with what the voice was keyed with (sample address, pitch, volume,
ADSR); key-offs of a voice that's sounding; pitch and volume changes while it sounds (changes
written just before a key-on belong to the key-on), the SsUtAutoVol fades' steps included (exact
since 2026-10-15: libsnd.rs, auto_vol_volume). A matching event a VBlank
away counts as a match "off by one": the port runs a VBlank's ticks after the game's code, the SDK whenever its
timer fires; one between the same two polls of a loading screen (polls more than 2 VBlanks
apart) as "same loading gap". Globally: the main and reverb volumes, the reverb voice mask (restricted to
--voices) and the SPU control bits that matter (enable, mute, reverb, CD audio), sampled at each
pad poll where the emulator's had held for 2 VBlanks either side (while the game loads, the
port does in one VBlank what takes the emulator many, so short-lived states can't be compared).
"""
import argparse
import bisect
import os
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import spu_events  # noqa: E402

SPUCNT_MASK = 0xC08F
# VBlanks after a key-off in which a voice's pitch and volume changes still count.
RELEASE = 30
# Events looked ahead to resynchronize after an insertion or deletion.
LOOKAHEAD = 8


def replay_polls(path):
    vbs = []
    for line in open(path):
        f = line.split()
        if len(f) == 4 and f[0].isdigit() and f[1].isdigit():
            vbs.append(int(f[1]))
    return vbs


def replay_final_vblank(path):
    for line in open(path):
        f = line.split()
        if len(f) == 2 and f[0] == 'final_vblank':
            return int(f[1])
    return 1 << 30


def parse_voices(spec):
    lo, _, hi = spec.partition('-')
    return set(range(int(lo), int(hi or lo) + 1))


def voice_streams(evs, voices):
    """Per (voice, stream): [(time, what)], and the global changes [(stamp, name, value)].

    Events carry (estimated emulator VBlank, the side's own VBlank). A voice's events belong to
    the sound effect stream ('sfx', timed by the estimate) if its key-on followed an SsUtKeyOnV
    for it, else to the song playing ('song', n) since the n-th SsSeqPlay, timed in VBlanks
    since that SsSeqPlay on the side's own clock: music runs on, unlike the game, while the
    emulator loads, so only the song's own timing is comparable.
    """
    per = defaultdict(list)
    on = set()
    released = {}  # voice -> the time it was keyed off (still releasing for RELEASE VBlanks)
    glob = []
    sfx_next, stream_of = set(), {}
    song, song_start = -1, 0
    stops, starts = {}, {}

    def add(n, t, what):
        st = stream_of.get(n, 'sfx')
        if st == 'sfx':
            per[(n, 'sfx')].append((t[0], what))
        else:
            per[(n, st[:2])].append((t[1] - st[2], what))

    for e in evs:
        t, kind = e[0], e[1]
        if kind == 'call':
            if e[2] == 'SsUtKeyOnV':
                sfx_next.add(e[3][0] & 0xffff)
            elif e[2] == 'SsSeqPlay':
                song, song_start = song + 1, t[1]
                starts[song] = song_start
            elif e[2] == 'SsSeqStop' and song >= 0:
                stops.setdefault(song, t[1] - song_start)
            continue
        if kind == 'keyon':
            n = e[2]
            # The SDK's SsInit keys every voice on at volume 0 with a zero ADSR and off again:
            # silent, skipped.
            if n not in voices or (e[5] == 0 and e[6] == 0 and e[7] == 0 and e[8] == 0):
                continue
            if n in sfx_next:
                sfx_next.discard(n)
                stream_of[n] = 'sfx'
            else:
                stream_of[n] = ('song', song, song_start)
            add(n, t, ('keyon',) + tuple(e[3:]))
            on.add(n)
        elif kind == 'keyoff':
            n = e[2]
            if n in voices and n in on:
                add(n, t, ('keyoff',))
                on.discard(n)
                released[n] = t[1]
        elif kind == 'set':
            n = e[2]
            # Changes while the voice sounds, or releases (DW1 fades a sound after keying it off).
            if n in voices and (n in on or (n in released and t[1] - released[n] <= RELEASE)):
                add(n, t, ('set', e[3], e[4]))
        elif kind == 'global':
            glob.append((t[0], e[2], e[3]))
    # Changes written just before a key-on (up to 2 VBlanks before: a VBlank's ticks fall on
    # either side of the boundary) belong to the key-on.
    for k, lst in per.items():
        keep = []
        for item in reversed(lst):
            if (item[1][0] == 'set' and keep and keep[-1][1][0] == 'keyon'
                    and keep[-1][0] - item[0] <= 2):
                continue
            keep.append(item)
        per[k] = list(reversed(keep))
    per['stops'] = stops
    per['starts'] = starts
    return per, glob


def global_changes(glob, voice_mask):
    """[(stamp, state)] after each change of the global registers that matter."""
    state, out = {}, []
    names = ('mvoll', 'mvolr', 'rvoll', 'rvolr', 'eon', 'spucnt')
    for t, name, v in glob:
        if name in ('eon_lo', 'eon_hi'):
            lo = state.get('_eon_lo', 0) if name == 'eon_hi' else v
            hi = v if name == 'eon_hi' else state.get('_eon_hi', 0)
            state['_eon_lo'], state['_eon_hi'] = lo, hi
            name, v = 'eon', (lo | hi << 16) & voice_mask
        elif name == 'spucnt':
            v &= SPUCNT_MASK
        if name not in names:
            continue
        state[name] = v
        out.append((t, tuple(state.get(k) for k in names)))
    return out


def state_at(changes, stamps, t):
    i = bisect.bisect_right(stamps, t)
    return changes[i - 1][1] if i else None


def compare_globals(ora_g, nat_g, polls, mask, report, show):
    oc, nc = global_changes(ora_g, mask), global_changes(nat_g, mask)
    os_, ns = [c[0] for c in oc], [c[0] for c in nc]
    stats = defaultdict(int)
    for k, vb in enumerate(polls):
        o = state_at(oc, os_, vb)
        if o is None or any(state_at(oc, os_, vb + d) != o for d in (-2, -1, 1, 2)):
            stats['unsettled'] += 1
            continue
        n = state_at(nc, ns, vb)
        if o == n:
            stats['same'] += 1
        else:
            stats['differ'] += 1
            if len(report) < show:
                report.append(f'global at poll {k} (vblank {vb}): oracle {o} | port {n}  '
                              '(mvol L/R, rvol L/R, eon, spucnt)')
    return stats


def close(a, b):
    return abs(a - b) <= 1


POLLS = []


GAP_POLLS = []


def gaps(t):
    """The loading gaps (polls more than 2 VBlanks apart) that VBlank t lies in, its ends
    included: the port stamps a VBlank's writes after its polls, and loads in no time, so an
    event of a gap can land on either of its polls."""
    p = GAP_POLLS  # the poll VBlanks without repeats (several polls can share a VBlank)
    i = bisect.bisect_left(p, t)
    out = set()
    for g in (i, i + 1):
        if 0 < g < len(p) and p[g - 1] <= t <= p[g] and p[g] - p[g - 1] > 2:
            out.add(g)
    return out


def same_gap(a, b):
    """Both in the same loading gap: while the game loads (or plays a movie) the port's VBlanks
    between polls aren't the emulator's."""
    return bool(gaps(a) & gaps(b))


def compare_lists(name, ora, nat, report, show, song=False):
    stats = defaultdict(int)
    i = j = 0
    # A song's events may drift slowly against the port's: the emulator misses a 240 Hz tick
    # now and then while the game is busy (23 instead of the clock's 11 over 19,000 VBlanks of
    # the training replay, 3 VBlanks), and where the SsSeqPlay call falls between two ticks
    # shifts a whole song by up to a tick: a song event may be off by 2 VBlanks plus one for
    # every 2,000 VBlanks of song time.
    while i < len(ora) or j < len(nat):
        o = ora[i] if i < len(ora) else None
        n = nat[j] if j < len(nat) else None
        if o and n and o[1] == n[1]:
            drift = n[0] - o[0]
            allowed = 2 + max(o[0], 0) // 2000
            if drift == 0:
                stats['same'] += 1
            elif close(o[0], n[0]):
                stats['off by one'] += 1
            elif song and abs(drift) <= allowed:
                stats['drifted'] += 1
            elif not song and same_gap(o[0], n[0]):
                stats['same loading gap'] += 1
            else:
                stats['late/early'] += 1
                if len(report) < show:
                    report.append(f'{name}: {o[1]} at oracle {o[0]}, port {n[0]}')
            i, j = i + 1, j + 1
            continue
        # An insertion or deletion: find the nearest event that matches again.
        skip = None
        if o and n:
            for d in range(1, LOOKAHEAD + 1):
                if j + d < len(nat) and nat[j + d][1] == o[1] and close(nat[j + d][0], o[0]):
                    skip = ('port', d)
                    break
                if i + d < len(ora) and ora[i + d][1] == n[1] and close(ora[i + d][0], n[0]):
                    skip = ('oracle', d)
                    break
        if skip:
            side, d = skip
            stats['only ' + side] += d
            if len(report) < show:
                extra = (nat[j:j + d] if side == 'port' else ora[i:i + d])
                report.append(f'{name}: only {side}: {extra}')
            if side == 'port':
                j += d
            else:
                i += d
            continue
        stats['differ' if o and n else ('only oracle' if o else 'only port')] += 1
        if len(report) < show:
            report.append(f'{name}: oracle {o} | port {n}')
        i, j = i + (1 if o else 0), j + (1 if n else 0)
    return stats


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('oracle')
    ap.add_argument('native')
    ap.add_argument('replay')
    ap.add_argument('--voices', default='0-23')
    ap.add_argument('--show', type=int, default=20)
    ap.add_argument('--oracle-end', type=int, help='the VBlank the oracle log stops at: '
                    'later events are left out on both sides')
    a = ap.parse_args()
    voices = parse_voices(a.voices)
    mask = sum(1 << v for v in voices)
    polls = replay_polls(a.replay)
    final_vblank = replay_final_vblank(a.replay)
    POLLS.extend(polls)
    GAP_POLLS.extend(sorted(set(polls)))

    def native_stamp(f):
        polls_done, since, raw = int(f[-2]), int(f[-1]), int(f[0])
        # Before the first poll: the port counts from ResetCallback, the emulator from power-on.
        return (polls[polls_done - 1] + since if polls_done else raw + 1144, raw)

    def oracle_stamp(f):
        return (int(f[0]), int(f[0]))

    # Before ResetCallback (Redux VBlank 1144) the emulator's log has the BIOS's own sounds.
    ora_ev = [e for e in spu_events.events(a.oracle, stamp=oracle_stamp) if e[0][0] >= 1144]
    nat_ev = spu_events.events(a.native, stamp=native_stamp)
    nat_polls = 0
    for line in open(a.native):
        f = line.split()
        if len(f) >= 7:
            nat_polls = max(nat_polls, int(f[-2]))
    if a.oracle_end:
        ora_ev = [e for e in ora_ev if e[0][0] < a.oracle_end - 1]
        nat_ev = [e for e in nat_ev if e[0][0] < a.oracle_end - 1]
    ora_v, ora_g = voice_streams(ora_ev, voices)
    nat_v, nat_g = voice_streams(nat_ev, voices)
    report, total, music = [], defaultdict(int), defaultdict(int)
    voices_soft = defaultdict(int)
    notes_o, notes_n = defaultdict(list), defaultdict(list)
    ora_stops, nat_stops = ora_v.pop('stops'), nat_v.pop('stops')
    # A song still playing when a log ends stops there.
    for stops, starts, evs in ((ora_stops, ora_v.pop('starts'), ora_ev), (nat_stops, nat_v.pop('starts'), nat_ev)):
        end = max(e[0][1] for e in evs)
        for song, start in starts.items():
            stops.setdefault(song, end - start)
    for key in sorted(set(ora_v) | set(nat_v), key=str):
        n, st = key
        is_song = st != 'sfx'
        label = f'v{n:02}' + (f' song {st[1]}' if is_song else '')
        ora_l, nat_l = ora_v.get(key, []), nat_v.get(key, [])
        if is_song:
            # The game stops a song by its own clock, and the emulator's songs run on through
            # its loading screens: from the earlier side's stop on (a VBlank's margin: the stop's
            # own key-offs land on either side of it), the song's tail. The stop's clean-up is
            # checked by the probes (docs/phase6-plan.md), not here.
            stop = min(ora_stops.get(st[1], 1 << 30), nat_stops.get(st[1], 1 << 30)) - 1
            tail = sum(1 for x in ora_l if x[0] >= stop) + sum(1 for x in nat_l if x[0] >= stop)
            ora_l = [x for x in ora_l if x[0] < stop]
            nat_l = [x for x in nat_l if x[0] < stop]
            music['song tail'] += tail
        stats = compare_lists(label, ora_l, nat_l, report if not is_song else [], a.show, is_song)
        for k, v in stats.items():
            if is_song and k in ('differ', 'only oracle', 'only port', 'late/early'):
                # Judged below, voice by voice only as a soft count: which voice a note gets
                # can follow a release ending a moment either side of a tick.
                voices_soft[k] += v
            else:
                (music if is_song else total)[k] += v
        if is_song:
            for x in ora_l:
                if x[1][0] == 'keyon':
                    notes_o[st[1]].append(x)
            for x in nat_l:
                if x[1][0] == 'keyon':
                    notes_n[st[1]].append(x)
    # Each song's notes, whatever voice they got: the same notes at the same song time.
    for song in sorted(set(notes_o) | set(notes_n)):
        nat_left = sorted(notes_n[song])
        used = [False] * len(nat_left)
        for t, what in sorted(notes_o[song]):
            allowed = 2 + max(t, 0) // 2000
            hit = None
            for k, (tn, wn) in enumerate(nat_left):
                if not used[k] and wn == what and abs(tn - t) <= allowed:
                    hit = k
                    break
            if hit is None:
                music['notes only oracle'] += 1
                if len(report) < a.show:
                    report.append(f'song {song}: note only in the emulator at {t}: {what}')
                continue
            used[hit] = True
            d = abs(nat_left[hit][0] - t)
            music['notes same' if d == 0 else 'notes off by one' if d == 1 else 'notes drifted'] += 1
        for k, u in enumerate(used):
            if not u:
                music['notes only port'] += 1
                if len(report) < a.show:
                    report.append(f'song {song}: note only in the port at {nat_left[k][0]}: {nat_left[k][1]}')
    if a.oracle_end:
        polls = [vb for vb in polls if vb < a.oracle_end - 2]
    # The reverb voice mask follows the voices keyed, and music runs on its own clock (see
    # voice_streams): only the sound effect voices' bits are comparable by poll.
    gst = compare_globals(ora_g, nat_g, [vb for vb in polls if vb >= 1146], mask & 0xfffc00,
                          report, a.show)
    for line in report:
        print(line)
    print('sound effects:', dict(total))
    print('music:', dict(music))
    if voices_soft:
        print('music voice by voice (not failures):', dict(voices_soft))
    print('globals:', dict(gst))
    bad = (total['differ'] + total['only oracle'] + total['only port'] + total['late/early']
           + music['notes only oracle'] + music['notes only port'] + gst['differ'])
    # An empty or truncated capture must not pass: the logs must reach the same point of the
    # replay (the song tails above take a song still playing at a log's end as stopped there).
    ora_last = 0
    for line in open(a.oracle):
        ora_last = max(ora_last, int(line.split(maxsplit=1)[0]))
    want = min(a.oracle_end, final_vblank) if a.oracle_end else final_vblank
    if ora_last < want - 5:
        print(f'the emulator log ends at VBlank {ora_last}, before the replay\'s {want}')
        bad += 1
    if nat_polls < len(polls):
        print(f'the port log ends after {nat_polls} of the replay\'s {len(polls)} polls')
        bad += 1
    if not any(e[1] == 'keyon' for e in ora_ev) or not any(e[1] == 'keyon' for e in nat_ev):
        print('no key-ons on one side: nothing compared')
        bad += 1
    if gst['same'] == 0:
        print('no global state compared')
        bad += 1
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
