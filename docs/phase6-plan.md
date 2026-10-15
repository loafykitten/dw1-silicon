# Phase 6: sound, streamed audio, movies (plan)

2026-10-14, session 4. PLAN.md's Phase 6: "SPU and the sequencer at its tick mode
(`SsSetTickMode(SS_TICK240)`), reverb, XA streams, MDEC movies at full quality. Done when event
timing and durations match the emulator (measured) and Ember says they sound right."

## Where it starts

- `backend::spu` (translated from DW2-PC, bit-exact against its C: voices, ADPCM, ADSR, pitch,
  noise, reverb, mixing), `backend::xa` and `backend::mdec` exist and are tested in isolation.
- `psyq::libsnd` and `psyq::libspu` keep the books silently: VAB headers and bodies land in a
  sound RAM image, voices are keyed with their parameters, nothing ticks (no sequencer), no audio
  leaves the program (`crates/psyq/src/libsnd.rs`, `libspu.rs`).
- Movies decode natively (MOV, Phase 2) but quality and sound haven't been compared.

## The oracle for sound: an SPU event log

Listening can't certify a port; the emulator's SPU traffic can. Redux checks write breakpoints
on every store, hardware registers included (`src/core/debug.cc`), before the store executes, so a
Lua hook on the SPU's register range (0x1F801C00-0x1F801DFF) can decode the store at the PC and
read the value from its source register. That gives the full SPU register traffic with the VBlank
(and CPU cycle) of every write.

From the raw writes, a decoder builds the events that matter for what's heard: per voice key-on
(start address, pitch, ADSR, volume, reverb flag), key-off, pitch and volume changes, and the
global state (main volume, reverb mode and depth, CD/XA volume). Natively the same events come
from libsnd/libspu's calls (keyed through the native SPU model). Comparing the two event lists,
keyed by VBlank relative to the replay's polls like the frames, is the gate for the library half.
Sound RAM is compared at checkpoints (VAB bodies, and what DMA put there).

## Slices

1. **The oracle.** `oracle/capture-spu.lua` (write log through the replays), a decoder
   (`tools/oracle/spu_events.py`) to events, and a first look at the battle replay: which SDK
   calls produce which events (key-on through `SsUtKeyOn`/`SsVoKeyOn`? the sequencer's
   `SsSeqPlay`?), how many per frame, what the tick rate is.
2. **Native events.** libspu's state behind `backend::spu` register writes; the same decoder's
   event list natively (`DW1_SPU_LOG`); compare on the replays. Sound effects (direct key-ons)
   first: menus, battles.
3. **The sequencer.** `SsSeqOpen`/`SsSeqPlay`/`SsSeqStop`/`SsSetTickMode`... clean-room from the
   Psy-Q Library Reference and the SEQ format (psx-spx; the disc's SEQs), ticked on the VBlank
   clock at SS_TICK240's rate as measured; checked event by event against the oracle on the replays
   (music starts on the title, the field, battles).
4. **Audio out.** SDL3 audio in the window build: `backend::spu` mixing at 44.1 kHz into an
   audio stream, paced by the emulated clock; latency and 59.94 Hz pacing checked with Ember (open
   since S0.6).
5. **XA streams and reverb**, then **movies**: XA through `backend::xa` from the CD stream, MDEC
   picture quality compared with Redux's frames.

Ember judges "sounds right" after slice 4.

## Slice 1: done (2026-10-14)

`oracle/capture-spu.lua` logs every SPU register store on a route (`DW1_SPU_FROM`/`DW1_SPU_TO`;
a write breakpoint over 0x1F801C00-0x1F801DFF, the value read from the store's source register),
and `tools/oracle/spu_events.py` turns the log into key-ons (with the voice's start address,
pitch, volume and ADSR at that moment), key-offs, parameter changes and global changes.

First measurements, battle replay, Redux VBlanks 4000-5000 (`captures/oracle-runs/p6-spu-test`):
33,824 stores. The key on/off, pitch modulation, noise and reverb-on registers (0x1D88-0x1D9A)
are each written 3,999 times, 4 per VBlank: the SDK's tick runs at 240 Hz (SS_TICK240) and
rewrites them every tick. 285 key-ons and 262 key-offs on 21 voices (voices 0-4 most: effects and
the battle music's busier parts), 19 distinct sample start addresses, 644 pitch/volume changes, 73
global changes. Logging slows Redux about threefold (1,000 VBlanks in 98 s).

Next (slice 2): which SDK calls produce these (capture `SsUtKeyOn`, `SsSeqPlay`... alongside), the
SDK's voice allocation as observed, then the native event log to compare.

## Slice 2: sound effects through the SPU model (2026-10-14)

libspu now drives `backend::spu` with register writes, as the SDK drives the real SPU, and
libsnd's key-ons go through a 240 Hz tick on the virtual clock (`libsnd::frame`, called by libetc
as each VBlank ends: the VBlank's ticks with the SPU rendering up to each, then the rest of the
VBlank). The SPU renders every VBlank whether or not anything listens, so voice envelopes, and
`SpuGetKeyStatus` (which the game's channel allocation reads), move with the game.
`DW1_SPU_LOG=<file>` logs the port's register writes in the capture's format plus the pad polls
so far and the VBlanks since the latest; `DW1_WAV=<file>` writes what's rendered.

How the SDK was measured, without reading its code:

- `oracle/sdk-probe.lua` borrows the emulator's CPU at a VBlank and calls one SDK function with
  many argument sets (stack arguments too), then follow-up calls (`DW1_PROBE_THEN`, e.g. the
  tick `SsSeqCalledTbyT`), recording v0 and every SPU store each call makes.
- `oracle/capture-spu.lua` logs a route's SPU stores with the cycle within the VBlank, plus chosen
  SDK calls (`DW1_SPU_CALLS`) in the same stream and the VAB headers the game opens.

Findings (all `[obs]` in the code):

- **Pitch** (`SsPitchFromNote`, 48,897 probes): a 12 x 16 table (semitone x 1/16 semitone) of
  measured values, shifted by octaves; `fine` and the tone's `shift` add in 1/128 semitones,
  used in steps of 8. Exact down to 60 semitones below the centre (the SDK returns garbage
  below that; it traps).
- **Volume** (`SsUtKeyOnV`, 6,235 probes): the louder of voll/volr is the volume, the quieter's
  share of it the pan; then tone volume x program volume, tone pan before voice pan, each pan a
  share in 63rds. Exact. Every bank on the disc has master volume 127 / pan 64 and the sound
  effect banks program pan 64 (`decomp/disks/us/SOUND/VHB`, 145 banks surveyed).
- Sample address = the VAB's sound RAM address + the sizes of the samples before it; ADSR is the
  tone's; tone mode bit 2 = reverb (EON is rewritten from libsnd's voices every tick).
- **The tick**: a free-running 240 Hz timer (141,120 cycles), so 4 ticks a VBlank but one VBlank
  in 1,721 with 3 (care replay, 29,000 VBlanks). Its phase differs between routes, so it isn't
  modelled; the port is at most a tick off. Before `SsStart` it ticks once a VBlank; `SsEnd` and
  `SsQuit` stop it and write nothing.
- `SsInit` writes a 16-byte looping block at 0x1000 and keys every voice on and off over it;
  `CdInit` sets main and CD volume full and CD audio on (which `SsInit` turns off again);
  `SpuSetReverbModeParam` writes the mode's 32 parameters (mode 3 recorded) and zeroes the depth.
- `SsUtKeyOnV` refuses a voice still sounding from an earlier key-on.
- `SsUtAutoVol` (the game fades a stopped sound over 6 ticks): steps of (start - end) / ticks a
  tick; the right side of a centred voice comes out exact, the left up to 5% lower and the first
  step depends on sub-tick timing, neither worked out. Approximated.

The gate is `tools/oracle/compare_spu.py` (emulator log, port log, replay): per voice, key-ons with
their parameters, key-offs of sounding voices and pitch/volume changes; globally the main and
reverb volumes, reverb voices and SPU control bits, sampled at each poll where the emulator's had
settled. Battle replay, sound effect voices 10-23, VBlanks to 5000: 104 events at the same VBlank,
2 a VBlank apart, 1 in the same loading gap, 3 fade steps with other values; globals 1,620 polls
the same. Music (voices 0-9) waits for the sequencer.

Later the same day, after the review (gpt-6.1-sol): the comparison counts a voice's pitch and
volume changes during its release too (30 VBlanks after the key-off), only volume differences
count as fade differences, it resynchronizes after an insertion or deletion, and it fails on an
empty capture. `SsInit`'s full reset was recorded (a second pass resets every voice's registers,
a third does what `SsUtAllKeyOff` does: pitch 0x1000, volume 0, the silent block, ADSR 0x80FF /
0x4000, written at once).

## Slice 3: the sequencer (2026-10-14)

`crates/psyq/src/sequencer.rs` reads a SEQ and says, tick by tick, which events are due; libsnd
turns them into voices. Measured with synthetic SEQs fed to the SDK in the emulator (the probe's
pokes; `captures/oracle-runs/p6-probe-seq*`, `-bend`):

- **Timing**: counted in tenths of a MIDI tick; each 240 Hz tick takes round(10 x resolution x
  1e6 / tempo / 240); an event is due once its delta is covered, remainders carry; a tick's
  events are written at the next tick. A tempo change (FF 51 + 3 bytes, no length byte) applies
  to the tick it comes in, its overshoot rescaled.
- **Loops**: NRPN 20 (controller 99) marks the loop start just after itself, a data entry
  (controller 6, before or after) the count; NRPN 30 jumps back, at once for 127 (forever, the
  only count DW1 uses); a finite count jumps after the next event's delta. The end of the score
  restarts it while `SsSeqPlay`'s count lasts (0 = forever), then the SEQ stops.
- **Voices**: every tone whose note range holds the note is keyed; the voice is the lowest free
  one (envelope finished), else the one keyed longest ago. The SDK remembers which SEQ note a
  voice was keyed for until it's keyed again: note-offs, volume and pan changes and bends still
  reach released voices. `SsSeqStop` and the end of a SEQ zero its voices' volume and key them off.
- **Volume** (981 probes, exact): 0x3FFF x tone volume x program volume x (velocity x channel
  volume / 127) / 127^3 in one division, x expression / 127, x the SEQ's side volume / 127, the
  channel's then the tone's pan share (63rds), then squared (v x v / 0x3FFF). Volume and pan
  changes rewrite the channel's voices.
- **Pitch**: the key-on ignores any earlier bend; a bend sets every voice of the channel to the
  note plus floor((msb - 64) x range x 128 / 63) 1/128 semitones (range = the tone's bend range
  down or up; the low byte doesn't count; range 0 = no change). A note more than 60 semitones
  below the centre gets pitch 0.

`SsUtAllKeyOff` (the game's stopSound) writes its voice resets at once, keying off at the next
tick.

The gate compares music in song time (VBlanks since the song's `SsSeqPlay`, each side on its own
clock): the emulator's music keeps playing through loading screens that take the port no time,
so poll-relative times only work for sound effects. Battle replay, whole run: sound effects 371
at the same VBlank, 57 a VBlank off, 5 in the same loading gap, 36 fade steps with other values;
music 1,965 the same, 19 a VBlank off, 0 different (78 events past the earlier side's song end);
globals 3,956 polls the same. All nine replays still match their checkpoints.

## Slice 4: sound out (2026-10-14)

The window build opens the default audio device (SDL3 audio stream, 16-bit stereo) and libspu
pushes each VBlank's samples into it. They're declared at 43,959 Hz: what libspu renders per
VBlank (734.89 samples, from the emulator's VBlank length) times the window's 59.817 Hz field
pace, so sound and picture keep pace; SDL resamples to the device (pitch 0.3%, 5 cents, flat).
More than a quarter second queued and new samples are dropped, so a stall doesn't leave the
sound lagging. `DW1_MUTE=1` opens no device. Headless runs can write the audio with
`DW1_WAV=<file>` (the battle replay's: `captures/native-runs/p6-listen-battle/`, local only).

Not yet: Ember's listen ("sounds right").

### More replays, and the review of slice 3 (2026-10-14)

gpt-6.1-sol's review of the sequencer found real problems, fixed: a score or loop that takes no
time now ends the SEQ instead of spinning (a tick may run at most 4,096 events), tempo math is
64-bit, a new `SsSeqPlay` starts at the header's tempo, loop jumps restore the running status,
the WAV cap keeps the real byte count; in the comparison, music volume differences always fail
(only sound effect fades are excused), `SsUtAllKeyOff` no longer ends a song's window, and only
SsInit's silent key-ons (volume and ADSR all 0) are skipped. The allocator it flagged (a voice
keyed this tick looking free) was already fixed.

Three more replays captured (`captures/oracle-runs/p6-spu-battle-twice`, `-training-six`,
`-new-game`) taught three more rules:

- A voice still holding a note isn't free even when its sample has ended; one whose note-off
  came this tick is (both seen in battle-twice).
- A loop jump lands on the event after NRPN 20, due at once: the delta before it counts only
  the first time (the field music's loop was 30 MIDI ticks long otherwise).
- `CdInit` sets SPUCNT outright to 0xC001, which turns the reverb off: on the new-game route a
  `CdInit` after the boot leaves the reverb off for the rest of the run.

And one emulator trait: Redux misses a 240 Hz tick now and then while the game is busy (23
instead of the clock's 11 over 19,000 VBlanks of the training replay), so long songs drift a few
VBlanks against the port; the comparison follows a song's offset as it drifts.

| replay | sound effects | music | globals |
|---|---|---|---|
| battle-fixture | 371 same, 57 off by one, 5 loading gap, 36 fade steps | 1,965 same, 19 off by one | 3,956 same |
| battle-twice | 524 same, 59 off by one, 6 loading gap, 36 fade steps | 2,764 same, 269 off by one | 5,599 same |
| training-six | 611 same, 12 off by one, 5 loading gap, 24 fade steps | 130 same, 118 off by one, 290 drifted | 11,025 same |
| new-game | 226 same, 75 loading gap | 991 same, 1,226 off by one, 167 drifted; 44 differ | 7,126 same |

The new-game differences are voice choices at the start of songs after a long release: the
emulator still sees the old note's envelope (its release takes about 1.5 s, and the song starts
1.45 s after the stop), the port's has just finished. Release timing that close to a tick
boundary isn't reproducible from VBlank-level timing.

## Slice 5 (part): XA audio for the movies (2026-10-14)

The movie player reads with Setmode's XA bit (`CdRead2(Stream|Speed|RT|Size1)`): the drive sends
every XA audio sector it reads to the SPU's CD input (no Setfilter: DW1 sets none). The stream
now keeps those sectors (`Stream::take_audio`), libcd hands them to `libspu`'s `XaDecoder`
after each stream call, and the SPU mixes the decoder's 44.1 kHz frames at the CD volume when
SPUCNT's CD bit is on (`CdInit` sets both). Sectors are taken in lazily, up to a frame ahead (since
2026-10-15 each reaches the decoder at its arrival: "XA audio against the emulator" below); on
the intro movie the decoder's FIFO stays between about 750 and 2,700 frames (17-60 ms) and never
runs dry after the first sector (`captures/native-runs/p6-movie-audio/intro-movie.wav`, local
only: 121 s, sound throughout but for a second of silence in the movie itself). Not compared
with the emulator yet: the SPU capture logs registers, and the CD input isn't register traffic.

### The comparison after the second review (2026-10-14)

gpt-6.1-sol's second review: a song's drift allowance grew with every matched event (10%
tempo errors could pass), the window's audio sink could outlive its stream, a log stopping
early could pass, the audio queue limit dropped new sound and kept the stale backlog, and
repeated poll VBlanks broke the loading-gap lookup. Fixed: a song event may now be 2 VBlanks
off plus one per 2,000 VBlanks of song time (the song-start tick plus the emulator's lost ticks),
on one clock for all voices; music is judged by its notes per song whatever voice each got, with
voice-by-voice differences reported apart (they follow releases ending a moment either side of
a tick); both logs must reach the replay's end; the window takes libspu's sink away before its
stream closes, and clears the queue when it falls a quarter second behind.

`tools/sound-check.sh` runs the four captured replays and compares them; all pass:

| replay | sound effects | music notes | globals |
|---|---|---|---|
| battle-fixture | 371 same, 57 off by one, 5 loading gap, 36 fade steps | 937 same | 3,956 same |
| battle-twice | 524 same, 59 off by one, 6 loading gap, 36 fade steps | 1,332 same, 118 off by one | 5,599 same |
| training-six | 611 same, 12 off by one, 5 loading gap, 24 fade steps | 70 same, 54 off by one, 146 drifted | 11,025 same |
| new-game | 226 same, 75 loading gap | 501 same, 629 off by one, 84 drifted | 7,126 same |

## The leftovers, closed (2026-10-15, Phase 9 sound stream)

### `SsUtAutoVol`: exact

The fade's steps were approximate (the left side up to 5% low, the first step "depending on
sub-tick timing"). Probed again with `oracle/sdk-probe.lua` (`SsUtKeyOnV`, the tick, `SsUtAutoVol`,
ticks; tone and program attributes poked into the VAB header the SDK keeps; interrupts masked so
the SDK's own tick can't interleave, which is what the "sub-tick timing" was):
`captures/oracle-runs/p9-sound-autovol-*`, 500 probes over tone volumes 0-127, tone pans 0-127,
program volumes 64-127, key-on volumes 1-127 on either side and sweeps up and down over 3-15 ticks.
What the SDK does, all exact (`libsnd.rs`, `auto_vol_volume`, `SndState::auto_vol`):

- The sweep's level moves by `(start - end) / ticks` a tick (rounded down, at least 1) and lands on
  `end`: 127 to 0 over 6 ticks writes 7 steps (106, 85, 64, 43, 22, 1, 0), as before.
- Each step is not the key-on's formula: level x tone volume x program volume x 0x3FFF / 127^3 in
  one division, then three pans in 64ths, each rounded down: the tone's, a centred one (the
  program's or the bank's, 64 in every DW1 bank), then the key-on's left/right balance. A centred
  pan in 64ths is 63/64 on the left, so a centred voice's left side steps at (63/64)^3 = 95.4% of
  its right: the "up to 5% lower" left.
- The tone, program volume and key-on volumes are those of the **latest `SsUtKeyOnV` call**, for
  whatever voice, even a refused one, looked up at each step (a key-on of another voice before
  the sweep or two ticks into it changes the steps from then on: `p9-sound-autovol-last1..3`).
  The game's two battle fades that differed (VBlanks 4905 and 5961 of the battle replay) were
  exactly this: another voice was keyed just before, with a louder tone.
- A sweep after `SsUtAllKeyOff` (the game's `stopSound` sweeps the effect voices still on after
  it) brings the silenced voice's volume back for its steps (`p9-sound-autovol-allkeyoff`); the
  port kept no tone for a silenced voice and wrote nothing.
- `SsUtAutoVol` with 0 ticks never returns to its caller (`p9-sound-autovol-zero`); the port stops
  there (DW1 always passes 6).

`compare_spu.py` no longer excuses fade steps ("set value differs" is gone): every step of the
four replays' fades is now the emulator's value, at the same VBlank or one apart.

| replay | sound effects | music notes | globals |
|---|---|---|---|
| battle-fixture | 387 same, 77 off by one, 5 loading gap | 937 same | 3,954 same |
| battle-twice | 540 same, 79 off by one, 6 loading gap | 1,332 same, 118 off by one | same as before |
| training-six | 615 same, 32 off by one, 5 loading gap | 70 same, 54 off by one, 146 drifted | same as before |
| new-game | 226 same, 75 loading gap | 501 same, 629 off by one, 84 drifted | same as before |

### XA audio against the emulator: exact after the decoder, and now gapless

The XA sectors never reach RAM (the drive sends them to the SPU), so no breakpoint sees them, but
Redux's save state keeps the last sector its CD controller decoded (the 37.8 kHz samples, before
the 44.1 kHz interpolation) with the read position, the audio volume matrix and the mute flags.
`oracle/capture-xa.lua` takes a save state every VBlank of a replay and logs each new decoded
sector; natively `DW1_XA_LOG` logs every sector decoded, the frames the SPU takes from the CD
input each VBlank and the polls; `tools/oracle/compare_xa.py` compares them
(`tools/sound-check.sh`: `new-game`, the new-game movie; `ending`, the credits, the title movie
and the continue movie). What "identical" means, and what was measured:

- **The decoded samples: bit for bit.** Every sector the emulator decoded (1,627 on the new-game
  movie, 2,534 around the credits) has exactly the samples the port decoded for that LBA, and
  every sector the port decoded between them that the emulator was never seen decoding repeats
  the one before it (silence), so none is skipped or added. DW1's four movies are all 37.8 kHz
  stereo 4-bit, one channel, a sector in 8; no Setfilter, ATV 0x80/0/0x80/0 (unity, as psx-spx
  says), never muted.
- **The interpolation to 44.1 kHz** is psx-spx's zigzag table, which is Redux's too; its output
  can't be read from Redux (it goes straight to Redux's audio stream), so it isn't compared.
- **The level after that isn't comparable**: Redux plays XA on its own audio stream without the
  SPU's CD volume (0x1DB0) or SPUCNT's CD bit; the port mixes it through both as psx-spx
  describes (CdInit's 0x3FFF, half scale). The SPU log comparison checks those registers and bits.
- **Timing: within a VBlank or two.** Each sector starts playing natively 0-2 VBlanks after the
  VBlank at whose end the emulator's save state first shows it decoded (mostly 0 or 1; the
  emulator's side is known to a VBlank, the port's to the VBlank its first frame plays in). The
  clocks are tied through the replay's polls; the credits have none (the game doesn't read the
  pad while they play), so their two halves are lined up by their median offset and judged within
  themselves.
- **Fixed: gaps.** The port's CD input used to play a movie's audio from the moment the stream
  first took a sector in, and took sectors in only when the game called a stream function: each
  movie's first second had two gaps (13 and 7 ms), the credits more, and sectors read ahead of a
  skip were still played after it. Now (`libcd.rs`, `Drive::play_xa`, `Drive::vblank`) the
  stream takes in what has arrived at the end of every VBlank (it would at the game's next call
  anyway, before anything that call does, so the game sees no difference: all replays identical),
  each XA sector reaches the decoder at its arrival and not before (a sector read ahead of a stop
  is never heard), and the CD input plays a VBlank of silence before a sector that finds it
  empty, which keeps the later sectors (arriving up to a VBlank after their moment, at VBlank
  resolution) on time. No gap remains within a movie, only at its end and at the credits' re-read
  that the emulator has too.
  `compare_xa.py` takes the reads from the emulator's side (its read position, VBlank by VBlank:
  steps of 1-8 sectors are one read), not from what the port decoded: every audio sector of a
  movie read, up to where the emulator's read position ended, must have been decoded by the port
  (and one the emulator never showed must hold the samples the emulator still held), and the
  port's CD input may run dry (its underrun count rising) only after the last sector of one of
  the emulator's reads. A gap after a sector decoded before or after the capture is only counted;
  anything the captures can't decide fails as "unverified". The logs show 3 gaps at reads' ends
  on new-game and 5 on ending (plus 2 after the boot's movie, before the ending capture starts),
  none inside a read, nothing unverified. `tools/oracle/test_sound_checks.py` (run first by
  `tools/sound-check.sh`) feeds both movie checks synthetic logs and requires them to fail on
  each bad case: a picture leaning 0.25 in one channel, pixel data cut short, missing or of the
  wrong size, a gap in the XA audio inside a read, a sector the port skipped inside a read or at
  its end where the emulator showed only its position (it repeated the one before), and gaps
  before the capture that must count as outside it.
