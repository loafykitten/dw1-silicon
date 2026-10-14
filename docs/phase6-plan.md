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
