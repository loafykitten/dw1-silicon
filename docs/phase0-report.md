# Phase 0: go / no-go

2026-10-13. Phase 0 asked one question: can Digimon World be ported to Apple Silicon as a
native app, built from the decomp's C with the PS1 libraries replaced by Rust, and checked
against the original game? This report gives each spike's answer, what's still risky,
and a recommendation. Ember decides.

## Recommendation: go

None of the spikes found a blocker. The two things most likely to sink the project both
worked on real game data:
- **The 3D library (libgs) can be rewritten cleanly.** Our Rust version matched Sony's
  output exactly on 48 real calls.
- **The original game can be the test suite.** PCSX-Redux replays the game
  deterministically, on this Mac and on booklab.

The memory problem (PS1 code storing pointers in 32 bits) has a design, and its size is
counted. What's left is a lot of work, but known work.

## What each spike found

| Spike | Result | Details |
|---|---|---|
| S0.1 Tools | Everything builds on this Mac: Rust 1.99, the decomp's container, PCSX-Redux from source | `docs/toolchain.md` |
| S0.2 Provenance | We may use DW2-PC's hardware models (CC0), not its sound libraries or libgs (translated from Sony's code). The decomp is a submodule, so this repo holds none of its files | `docs/provenance.md` |
| S0.3 Decomp build | The decomp's GCC build passes on this Mac (through its container) | `docs/s03-decomp-gcc-build.md` |
| S0.4 Memory | macOS won't let anything live below 4 GB, so PS1 pointers can't be faked. Design: native pointers, fixed-width file and save records, widen the ~1,000 places that store pointers in 32-bit ints | `docs/memory.md` |
| S0.5 Survey | DW1 calls 175 SDK functions; DW2-PC covers about a third. The riskiest code is the battle effect interpreter (pointer tricks, three copies) and the script interpreter (`longjmp`) | `docs/survey.md` |
| S0.6 Rust skeleton | Game C compiles natively and links to Rust. GTE, GPU, SPU, XA and MDEC are in Rust and match their references bit for bit. Window/input/audio: SDL3, built in statically (provisional) | `docs/dw2pc-reuse.md`, `docs/s06-platform.md` |
| S0.7 libgs | Clean Rust `GsSortObject4J` matches 48 real calls exactly (7,246 packet words) and draws the same pixels | `docs/s07-libgs-spike.md` |
| S0.8 Oracle | S08_RESULTS | `docs/s08-oracle.md`, `docs/canonical-state.md` |

Along the way we found and reported a bug in PCSX-Redux (grumpycoders/pcsx-redux#2322). It
broke character models in its newest builds, so the oracle is pinned to the build before it.

## Risks left, biggest first

1. **Size of the library layer.** About 120 SDK functions have no reference
   implementation. That includes all of libgs beyond `GsSortObject4J`, the sound
   sequencer (SEQ, VAB, 240 Hz ticks) and CD/memory card. Each is written from public
   docs and checked against emulator captures, the way S0.7 did libgs. It works, but it's
   the bulk of the project.
2. **Sound.** No usable reference exists for DW1's sequencer: DW2-PC's was translated from
   Sony's code, so we can't take it. It has to be built from the SEQ/VAB formats and
   compared against the emulator's audio output, which is harder to diff than pixels.
3. **Pointer widening in the effect interpreter.** About 1,000 sites, mostly mechanical, in
   code that runs every battle. If one is missed, the bug shows up only in specific
   effects, so replays need to cover battles early.
4. **Oracle timing.** Redux runs at about 60.01 Hz; real hardware is about 59.82 Hz. Game
   logic counts frames, so replays still match. But anything timed by CPU cycles (CD
   reads, movie timeouts) won't be hardware-exact. The oracle is "Redux's behaviour",
   which is the right target, but it isn't the PS1 itself.
5. **The decomp's C under clang.** It compiles, but whether it means the same thing natively
   is unproven until scenes run against replays (Phase 2 onward). There are 233 K&R
   definitions, about 15 prototype mismatches, signed `char`, and a few known undefined
   behaviours.
6. **Young libraries.** The `sdl3` crate is at 0.20. Gamepads, audio latency and 59.94 Hz
   pacing are untested on real hardware with you at the screen.

## Not done in Phase 0

- OpenBIOS as the oracle's BIOS. Its `rand()` has the same recurrence and seed 0 at boot
  (from its source), but it keeps the state at a different address and wasn't run. We
  stay on your retail BIOS.
- Gamepad, audio and display-pacing checks on hardware. They need you at the screen.

## If go: what's next

Phase 1: all of main's C plus the Rust `psyq` crate (with trapping stubs) builds and links
for arm64, and every overlay's C compiles. Then Phase 2: boot to the main menu, matched
against the emulator at 1x by replay.
