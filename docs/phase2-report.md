# Phase 2: boot to the main menu

2026-10-14. PLAN.md's gate for Phase 2: the landing screen matches the emulator's at 1x, and a
replay reaches the main menu with no stub trapped. `docs/phase2-plan.md` added the S0.8
checkpoints on the way and re-entry (back to the landing screen and through again).

**Result: done, and further.** The native `dw1` boots the US game from Ember's disc image
through the intro movie, the landing screen, the main menu and the memory-card screens, and
into the new-game scene, where it now draws the 3D characters. Against PCSX-Redux (the oracle):

- **Frames:** every frame from the end of the intro movie to the main menu (298 frames with a
  reference: landing screen, main menu and the fades between them) is pixel-identical at
  15-bit depth. Frames are paired by replay poll, not VBlank (see "Time" below).
- **Replay:** `oracle/replays/new-game.txt` (7,131 recorded polls) runs with every poll from the
  same game function as recorded through the menus, the memory-card save and into
  `newGameScene`. It stops at poll 1088 on the next SDK call not yet written
  (`GsSortBoxFill`, a 2D sort: Phase 4 territory).
- **Checkpoints:** all four reachable canonical-state checkpoints (schema v2; polls 0 and 600)
  are byte-identical to the emulator's.
- **Re-entry:** with no input the game loops intro, landing screen, intro, landing screen
  (16,000 VBlanks) with no trap.
- **Movies:** all four play through the game's own player and Start skips them; frames come
  in the emulator's order and cadence; MDEC colour rounding is within 4/255 of Redux's
  (`docs/phase2-movie.md`).

## What was built (slices in `docs/phase2-plan.md`)

| Library | How it was fitted | Check |
|---|---|---|
| Heap, libc, BIOS bits | psx-spx, observation | unit tests |
| libcd + `pack` (disc) | ISO9660 on the verified image; completions on the clock | game reads equal extracted files |
| libgpu, libgs buffers/OT/clear | Redux captures (S0.7 method) | packets, then pixels |
| `backend::gpu` rasteriser | refitted to Redux by black-box draws (5,974 cases; semi-transparency and the game's own fade on its own picture) | landing/menu/fades 0 px |
| libetc (clock, pads) | virtual VBlank clock; replay host with caller checks | replay |
| libgte (29 functions) | `oracle/call-sdk.lua`: the real SDK run on chosen inputs, 56,160 calls | bit-exact |
| libgs camera, lights, coordinates | same, 12,093 calls | bit-exact |
| libgs `GsSortObject4J` (TG3/TG4) | 228 captured calls with whole-machine state | bit-exact packets |
| libmcrd | measured card timings in Redux (exist, accept, directory, create, write; periodic stall) | replay through the save |
| libsnd/libspu | silent but stateful | the game's waits finish |
| libpress, St* streaming | MDEC BS v2/v3; sectors on the clock | movies play, cadence equal |

Unmodelled inputs trap with a message (e.g. an overflowing `CompMatrix`, a negative
`SquareRoot0`, a coordinate chain under SCREEN) rather than return a guess.

## Time

The virtual clock advances exactly by the VBlanks each `VSync` waits. The game's code takes no
time natively, and the disc's ordinary reads complete at once, so native VBlank numbers run
ahead of the emulator's (the movie starts at native 13, Redux 507 after `main()`). Replays are
keyed by poll sequence, so this doesn't matter where the game waits by frames; where it waits
on hardware (the memory card's blocking calls and busy polls), the wait has to take the same
number of VBlanks and polls, so those timings are measured and modelled. `DW1_REPLAY_LOG=1`
prints each poll's native and recorded VBlank.

## Fork changes (`loafykitten/dw_decomp`, branch `dw1-native`)

All under `DW1_NATIVE` (the decomp's GCC build stays byte-identical): 558 pointer-truncating
casts respelled `intptr_t`/`uintptr_t`; fixed PS1 addresses through `DW1_PS1_ADDR`; TMD
address fields through `DW1_TMD_ADDR`; an unterminated `char[16]` path given its 17th byte;
`MOV_playMovie`'s uninitialised `DRAWENV` zeroed; `setupNewGameDialogueBox` skips empty entity
slots (the PS1 reads them harmlessly from low RAM).

## Reviews

Every slice was reviewed by gpt-6.1-sol (high), the GPU/memory work by GPT 6 Astra (high), and
every finding was checked and fixed. Open at the end of the phase: Astra on the rasteriser
refit and on `GsSortObject4J`; the card-timing follow-ups (reads, deletes, format, error paths,
the periodic stall's exact schedule) are being measured.

## Known differences and open items

- MDEC rounding (above). The oracle's residue in `gpu_oracle`: 249 of 5,974 synthetic cases
  differ by one-step rounding ties (1,573 pixels in all; thin triangles and random textured
  Gouraud); none occurs in the game's frames so far.
- The battle-effect interpreter's `int32_t` stack (`EFE_PUSH`) holds addresses: Phase 5.
- Integer-address macros in overlays (`EVL_MMD_BUFFER` and others) as their overlays run.
- 16 functions without a `return` whose callers use the value, and `renderString`'s ninth
  argument: capture the PS1 values when their scenes run (`docs/abi-audit-calls.md`).
- Phase 3 needs one of Ember's fixture saves (a memory card image from an emulator).
