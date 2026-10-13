# Phase 0: go / no-go

2026-10-13. Phase 0 asked one question: can Digimon World be ported to Apple Silicon as a
native app, built from the decomp's C with the PS1 libraries replaced by Rust, and checked
against the original game? This report gives each spike's answer, what's still risky,
and a recommendation. Ember decides.

## Recommendation: go

None of the spikes found a blocker. The two things most likely to sink the project both
worked on real game data:
- **The 3D library (libgs) can be rewritten cleanly.** Our Rust version of its main drawing
  call matched Sony's output exactly on 48 real calls.
- **The original game can be the test suite.** PCSX-Redux replays the game
  deterministically, on this Mac and on booklab.

The memory problem (PS1 code storing pointers in 32 bits) has a design, and its size is
counted. What's left is a lot of work, with some important unknowns still open (below).

## What each spike found

| Spike | Result | Details |
|---|---|---|
| S0.1 Tools | Everything builds on this Mac: Rust 1.99, the decomp's container, PCSX-Redux from source | `docs/toolchain.md` |
| S0.2 Provenance | We may use DW2-PC's original hardware code, minus the parts the provenance check excludes. Its sound libraries, libgs and the Sony-derived parts of libgte must be rewritten. The decomp is a submodule, so this repo holds none of its files | `docs/provenance.md` |
| S0.3 Decomp build | The decomp's GCC build passes on this Mac (through its container) | `docs/s03-decomp-gcc-build.md` |
| S0.4 Memory | macOS won't let anything live below 4 GB, so PS1 pointers can't be faked. Design: native pointers, fixed-width file and save records, and widening the ~1,000 places that store pointers in 32-bit ints | `docs/memory.md` |
| S0.5 Survey | DW1 calls 175 SDK functions; DW2-PC has fewer than half of them by name, and some of that is code we can't reuse. The riskiest game code is the battle effect interpreter (pointer tricks, three copies) and the script interpreter (`longjmp`) | `docs/survey.md` |
| S0.6 Rust skeleton | Three of the game's C files compile natively and link to Rust, which proves the boundary; the rest is Phase 1. GTE, GPU, SPU, XA and MDEC are in Rust and match their references bit for bit. Window/input/audio: SDL3 (static build) provisionally chosen after standalone spikes; not in the app yet | `docs/dw2pc-reuse.md`, `docs/s06-platform.md` |
| S0.7 libgs | Our clean Rust `GsSortObject4J` matches 48 calls from one emulator frame exactly (7,246 packet words). Those calls cover lit textured triangles and quads only. Both sets of packets draw the same pixels through our GPU | `docs/s07-libgs-spike.md` |
| S0.8 Oracle | A new game played from boot to the first gameplay frame (7,131 pad reads, about 6½ minutes) is recorded as pad states keyed to the game's controller reads. It replays exactly: 19 state snapshots (partner, tamer, items, flags, clock, both RNGs, map) are byte-identical across the recording, two fresh replays on this Mac and one on booklab. A tampered replay fails. Redux runs at 60.009 Hz | `docs/s08-oracle.md`, `docs/canonical-state.md` |

Along the way we found and reported a bug in PCSX-Redux (grumpycoders/pcsx-redux#2322). It
broke character models in its newest builds, so the oracle is pinned to the build before it.

## Risks left, biggest first

1. **Size of the library layer.** Most of the SDK layer still has to be written: the rest
   of libgs (the other primitive types and other calls), the sound sequencer (SEQ, VAB,
   240 Hz ticks), CD and memory card. Each is written from public docs and checked against
   emulator captures, the way S0.7 did. That works, but it's the bulk of the project.
2. **Sound.** DW2-PC has no sequencer we may copy, so DW1's has to be written from the
   SEQ/VAB format docs and checked against the emulator's audio, which is harder to diff
   than pixels.
3. **Overlay state.** The game swaps code modules in and out of memory. Some of their data
   must survive a swap, some must be reloaded fresh, and some loads overlap. Getting a
   reset wrong could show up hours into a game.
4. **Pointers and loaded data.** About 1,000 sites store pointers in 32 bits, mostly in
   the effect interpreter, which runs every battle. Widening them is mostly mechanical, but
   models and effect files are also rewritten in place with pointers inside the loaded
   bytes. Those need native side tables, which aren't built or tested yet. A missed site
   only shows up in specific effects, so replays need to cover battles early.
5. **Picture accuracy.** Our GPU matches DW2-PC's, including its known differences from a
   real PS1 (blending order, polygon edges, transfers; `docs/backend-gpu.md`). Those still
   have to be checked against Redux's frames.
6. **The decomp's C under clang.** Only three files are compiled natively so far. The
   rest still has to be shown to mean the same thing natively. Known issues: 233 K&R
   definitions, about 15 prototype mismatches, `char` signedness to pin to the original
   compiler, game text needing an explicit encoding step, a few known undefined behaviours,
   and the script interpreter's shared `longjmp` buffer with a possible re-entry.
7. **Oracle timing.** Redux runs at 60.009 Hz; real hardware is about 59.82 Hz. The recorded
   route matches on the pinned Redux setup. But CD completion and the movie code's timeouts
   depend on CPU time, so other builds, cores or timings may diverge, and Redux may differ
   from hardware there. The oracle is "Redux's behaviour", which is the right target, but it
   isn't the PS1 itself.
8. **Young libraries.** The `sdl3` crate is at 0.20. Gamepads, audio latency and 59.94 Hz
   pacing are untested on real hardware with you at the screen.

## Not done in Phase 0

- OpenBIOS as the oracle's BIOS. From its source, its `rand()` uses the same recurrence and
  also starts at 0, but it wasn't run. We stay on your retail BIOS.
- Gamepad, audio and display-pacing checks on hardware. They need you at the screen.

## If go: what's next

Phase 1: all of main's C plus the Rust `psyq` crate (with trapping stubs) builds and links
for arm64, and every overlay's C compiles. Then Phase 2: boot to the main menu, matched
against the emulator at 1x by replay.
