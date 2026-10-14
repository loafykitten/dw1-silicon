# Phase 5: the overlays (battles first)

2026-10-14, session 4. PLAN.md's Phase 5: "each overlay is done when its scene's replay matches the
oracle, including loading it twice, alternating with another overlay, and resident state surviving
the swap". This document starts with BTL, the field battle.

## The first battle: done

| Replay | Start | Final | Polls | CD reads | Checkpoints | Native |
|---|---|---|---|---|---|---|
| `oracle/replays/battle-fixture.txt` | Ember's Save2 (`ember-save2-prefight`) | VBlank 9200 | 3,961 | 40 | 17 (90 at every 50 polls) | all identical |

Ember's Save2 stands on screen 109 in front of a wild ModokiBetamon. The route
(`DW1ROUTE_PROFILE=battle-fixture`: `oracle/route-continue-save2-schedule.lua` then
`oracle/route-battle-schedule.lua`) loads that save, walks down into the ModokiBetamon ("This is my
territory!"), closes its line (the BTL overlay loads, GAME_STATE 3), lets Gabumon fight on its own,
fires the finisher with Square when "FIN" shows (the game's mask 0x80), wins, closes the results
with Cross and is back on the map (GAME_STATE 0) by VBlank ~9000: 100 bits and stat gains.

Certified like the earlier replays: a fresh Redux replaying the recording matched every poll,
VBlank and CD read (`captures/oracle-runs/p5-battle-replay1`). Natively
(`tools/replay-check.sh battle-fixture`) all 17 standard checkpoints are identical, and so are
all 90 of a run with a checkpoint every 50 polls (`DW1REPLAY_EVERY=50` on Redux,
`DW1_CHECKPOINT_EVERY=50` natively; `captures/oracle-runs/p5-battle-dense`), with the canonical
state's new battle and animation sections (below).

## Loading it twice, and another overlay: done

`oracle/replays/battle-twice.txt` (`DW1ROUTE_PROFILE=battle-twice`, 5,604 polls, 76 CD reads, to
VBlank 13500): the first battle, then up to screen 112 and back into 109, where the ModokiBetamon
has come back, and the fight again with BTL loaded a second time. Gabumon, on the 96 HP the first
fight left, loses: the death countdown, the MURD overlay's "lost a life" scene, and Jijimon's house
("Oh my gosh! You're back after losing."). Certified (`p5-twice-replay1`); natively 28 of 28
checkpoints identical. MURD needed three native fixes: `loadTextureFile` writing its results
through NULL, `MURD_storeDigimonTexture`'s buffer address passed as a 32-bit `u_long`, and empty
entity slots read and written by `MURD_setOtherEntitiesVisible` (the PS1 hit low RAM).

## Training (TRN): the first session done

`oracle/replays/training-fixture.txt` (`DW1ROUTE_PROFILE=training-fixture`, 3,195 polls, 36 CD
reads, to VBlank 9900): from Save2 up into the Green Gym (screen 112, `TRAI00`, which loads TRN as
the map loads, `loadTrainingLibrary`), onto the "Training: Running" sign, Begin, Normal: Gabumon
runs laps for a game hour, Speed 60 -> 67, results closed, Quit. Certified (`p5-trn-replay1`);
natively 12 of 12 checkpoints identical, and 70 of 70 at every 50 polls, with no native change
needed. How training starts: the gym's signs are tile triggers 81-86 (`checkMapInteraction`:
the tamer stands on a tile with trigger 80-109 and presses Cross; the map script runs; found from
`MAP_COLLISION_DATA` in a RAM dump of screen 112); 85 is Running, in the middle of the gym. The
others are at x 750..1150 z -1450..-1150 (81), x 1250..1650 z 650..950 (82), x -650..-250
z -2450..-2050 (83), x -950..-550 z -1350..-1150 (84) and x -2050..-950 z 850..1650 (86). One
session uses all the partner's energy (a second session still runs).

`oracle/replays/training-bonus.txt` (`DW1ROUTE_PROFILE=training-bonus`, 4,129 polls, to VBlank
12100): the speed session, then the HP sign ("Training: Boulder Moving", trigger 81, reached
around the heart sign), Begin, Bonus Try: the slot game (`trn_slots.c`), the session, HP +176 and
Offense +4. Certified (`p5-bonus-replay1`); natively 13 of 13, and 89 of 89 at every 50 polls.
`oracle/replays/training-six.txt` (`DW1ROUTE_PROFILE=training-six`, 11,030 polls, 41 CD reads, to
VBlank 25600): training-bonus, then Bonus Tries at "Punching Glove" (Defense), "Tree Striking"
(Offense), "Save2's Classroom" (Brains, named after the tamer) and "Waterfall" (MP), with Gabumon
getting hungry and relieving itself on the ground in between: all six of TRN's kinds. The final
stats equal the exploration's (Brains 76, MP 746, Defense 85). TRN2's gyms are elsewhere on the
map (still to do).

**Caveat found deriving it:** inputs found with `oracle/explore.lua` don't always reproduce in a
recording. Here the explorer's run diverged from the recording after the Tree Striking session
(the walk to the next sign slid elsewhere), although recordings are deterministic among
themselves and the explorer matches a recording exactly while the route drives it. The likely
cause is that the explorer pauses the emulator between commands for seconds, and something in
Redux's CD emulation runs on wall-clock time (that stretch has two more CD reads than the
recording up to it). So: check a new recording's milestone screenshots or final state against the
exploration, and if they differ, explore again from the recording's own end (route-driven up to
there), as `training-six` does after `training-all`.

## What it took

### Oracle and tools

- **Canonical state schema v3** (`docs/canonical-state.md`): appended, so the v2 fields keep their
  offsets: the four `COMBAT_DATA` fighters (status and effect slots, finisher goal and progress,
  every status timer, damage buffers, flags, targets, buffs), the player's command state, each
  enemy (`ENTITY_TABLE[entityIds[k]]`: species, HP, MP, position) and the animation state of the
  tamer, the partner and the enemies. All older oracle dumps were regenerated with it
  (`captures/oracle-runs/v3-*`).
- **Replay format 2: CD timing.** The PS1's drive takes real time and the game keeps running
  while it reads (battle loading moves the fighters and polls the pad every frame until the
  overlay is in), so a read's length is game state. `oracle/replay.lua` records, per `CdRead`,
  the poll it started at, its sector count and how many non-blocking `CdReadSync` calls answered
  "busy"; natively `CdReadSync` answers the same (`psyq::libcd::set_read_schedule`), and every
  read must start at its recorded poll, all busy answers must be used and every recorded read
  must happen, or the run stops as diverged. Reads inside `CdSearchFile` (the SDK reading the
  disc's directory; natively the pack answers) aren't counted. Format 1 replays still work (reads
  finish at once).
- `oracle/trace-efe.lua` (trace the battle effect interpreter's opcodes, with slot and
  sub-effect), `oracle/capture-calls.lua` now takes overlay functions and scratchpad arguments
  (`DW1_CAPTURE_ARG0_BYTES`), `DW1_RAND_LOG=1` and `DW1_CD_LOG=1` natively,
  `tools/replay-check.sh` (every certified replay, strict pairing).

### SDK (clean-room, black-box)

- `GsSortSprite` with scaling, every attribute combination (brightness off, colour modes,
  semi-transparency rates, display off) and odd sizes; `GsSortFastSprite`. 1,525 `call-sdk.lua`
  requests (`tools/oracle/sprite_requests.py --set battle`) plus the 1,000 older ones; what was
  never observed traps (`tests/sprite_traps.rs`).
- `GsTMDfastTNF4` (unlit flat textured quads, the effects' models; mode 0x2F only), checked on
  captured calls (`s4j-btl-effect`).

### Game C (decomp fork, `dw1-native`)

The native battle diverged in these places, each found by comparing dense checkpoints, then the
effect opcode streams, then the RNG callers:

1. `addFileReadRequest` read `*loc` with `loc` NULL (the queue overwrites it anyway).
2. The EFE effect interpreter (BTL, STD, VS) kept addresses in 32-bit stacks, registers and fields
   (a Sonnet subagent widened them; `docs/memory.md` item 3).
3. `EfeSubEffect` and `EfeUvAnim` records hold pointers but live inside the loaded EFE file on
   the PS1: natively each slot has side tables (sized from the disc: at most 149 sub-effects,
   War Cry; 19 UV animations), and the free-record search, bounded by the model pointer on the
   PS1, is bounded by the table.
4. `*_startEFE` has no return statement; the PS1 caller got `EFE_ACTIVE_SECTION` in v0.
5. `(uint32_t)(animId - 0x2E)` array indexes rely on 32-bit address wraparound
   (`DW1_WRAP_INDEX`).
6. `setInt16WithStride` strides were literal PS1 struct sizes for structs holding pointers
   (`DW1_SIZEOF`); the particle effects' slots 1-3 were never freed.
7. A stopped sub-effect that never started wrote through a null instance pointer.

### Overlay data lifetimes

Each load of an overlay on the PS1 copies its file over its load address, so its initialised data
(explicit zeros included: CodeWarrior put those in `.data`) restarts from the file's values, while
its `.bss` keeps whatever the RAM held and the small variables CodeWarrior put in the resident
executable keep theirs. Natively the overlays' C is linked once, so (`crates/decomp-c/overlays.rs`,
`src/overlays.rs`): every overlay file whose `.data` the original `overlay.ld` places in an overlay
is compiled through a generated wrapper that puts its initialised data in one section per overlay
(`__DATA,__dw1_btl` ...) and its zero-initialised and uninitialised data in a second
(`__dw1_btlz` ...); `dw1_overlay_reset`, called by `loadDynamicLibrary`, restores the first from a
startup snapshot and zeroes the second, but keeps the file's resident definitions (names
`symbols.txt` places at 0x80090000 or above) and its definitions without an initialiser. Those
come from the preprocessed file (a small declaration parser; gpt-6.1-sol checked its 1,138
definitions against clang's AST). Function-local statics would stop the build (there are none).
The one-battle replay is unchanged by it; the two-battle replay below tests it.

## Next

- More battle coverage if cheap (another enemy species, the tamer's command menu, an item), and
  a battle after an overlay other than BTL and MURD.
- The 110 non-void functions without a return (`-Wreturn-type`): gpt-6.1-sol's audit
  (`docs/phase5-returns-audit.md`): 94 unused, 9 used with a determinable PS1 value, 7 unclear
  (among them the EFE loaders and `playSound`). Check and fix the 16.
- TRN/TRN2 (training), then the rest (PLAN.md's order).
