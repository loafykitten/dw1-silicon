# Phase 4: the walk around File City and the care replay (oracle gate)

2026-10-14. PLAN.md's Phase 4 oracle: "a replay from boot to the first walk around File City,
state equal at each map change, and a fixture-started replay of an hour of care with the clock
and care state equal". Both replays exist, both are certified on the emulator, and the native
`dw1` runs both to their last poll with every canonical checkpoint identical.

## Results

| Replay | Start | Final | Polls | Checkpoints | Native |
|---|---|---|---|---|---|
| `oracle/replays/file-city-walk.txt` | new game, fresh cards | VBlank 23380 | 8,819 | 27 | 27 of 27 identical |
| `oracle/replays/care-fixture.txt` | Ember's save (`ember-day2-gabumon`) | VBlank 31000 | 18,734 | 37 | 37 of 37 identical |

Native: `DW1_REPLAY=oracle/replays/<name>.txt [DW1_CARD1=<fixture>] DW1_OUT=<dir>
./target/debug/dw1`, then `tools/oracle/compare_canon.py <dir> <oracle dir> --allow-unpaired`.
The card identity is in the headers (Phase 3 follow-up):
the walk has `card1_sha1 fresh`, the care replay `card1_fixture ember-day2-gabumon` and the
fixture's SHA-1.

**Certification (like S0.8).** A fresh Redux process replaying each recording (no pad overrides,
the pinned build, one emulator at a time) ran to the last poll with 0 VBlank mismatches and
dumps byte-identical to the recording's: walk 27 of 27, care 37 of 37
(`captures/oracle-runs/p4-walk-record`, `p4-walk-replay1`, `p4-care-record`, `p4-care-replay1`).

## Walk route (`DW1ROUTE_PROFILE=file-city-walk`)

The new-game route to VBlank 19400, then `oracle/route-file-city-walk-schedule.lua` (15 holds;
inputs derived with `oracle/explore.lua`, below, from screenshots and the position and screen
the game reports). Screen ids: 204 is the yard outside Jijimon's house, where the game starts;
180 is the File City plaza south of it; 218 is the inside of Jijimon's house.

| VBlank (recording) | Input | Screen change (checkpoint) |
|---|---|---|
| 19401 | Down 210 | 204 to 180 (`screen180`, poll 7213; "File City" caption, control back ~19800) |
| 20011 | Up 90 | 180 to 204 (`screen204`, poll 7409) |
| 20431-20741 | Up 200, Left 40, Up 40 | 204 to 218 (`screen218`, poll 7635: "Jijimon's house" caption) |
| 21061-21471 | steps inside, then out of the door | 218 to 204 (`screen204`, poll 7975) |
| 21871-23320 | Down 200, 150, 200 | 204 to 180 (`screen180`, poll 8475) |
| 23321 | Down 200 | a few steps across the plaza; target 23380 |

Five map changes after the first gameplay frame (checkpoints 19, 20, 21, 23, 25 of 27; the
first 18 are the new-game route's own). The tamer and partner positions, script state, clock and BIOS rand state are
in every dump. **Not reached:** the plaza's west exit (x about -3000, z about 400) leads to
screen 109 and a scripted meeting with a wild Agumon that turns into a battle; the route
avoids it. The shops were not visited (exits not found; that is the next step of a longer
walk).

## Care route (`DW1ROUTE_PROFILE=care-fixture`)

The continue route (CONTINUE GAME, slot 1, save 1, Yes; File City at 3800), then
`oracle/route-care-schedule.lua`: Triangle opens the game menu; ITEM, Use on the first item
(a no-op at full HP), Sort ("Raise"; the slot order changes in the canonical state); the
DIGIMON screen with its Status and Tech tabs; the menu closed (the game clock stops while it is
open); then no input. Checkpoints every 600 polls plus `main1`/`screen168`/`final`.

State at the checkpoints: game clock day 1 07:00 (frame 8400) to 14:16 (17130), more than seven
game hours (a game minute is 60 VBlanks, an hour 3,600). In that time food level 960 to 526,
energy 15 to 0, weight 15 to 1, poop level 48 to 5, the inventory slot order re-sorted;
`condition` stays 0. Native equals Redux at every one of them.

**Why it stops at 31000.** The partner's poop condition (bit 8) sets at about VBlank 34000 and
the partner relieves itself at about 36400 (seen in an explorer run; poop level 48 resets), but
the pinned Redux's Lua hooks break near VBlank 32768: a recording with the PadRead breakpoints
lost them at ~32,700 ("Lua Breakpoint invoker threw an exception, deleting breakpoint"), and the
explorer's VBlank listener threw "stack overflow" at 32,801 (again at ~36,500). Neither run could
be certified, so the route ends before. A longer care replay needs another hook design or the
poop to come sooner (the fixture has no food, so the partner can't be fed).

## Tools added

- `oracle/explore.lua` (dev tool): runs a profile to a milestone, then stops emulation at each
  command boundary and reads commands from `./cmd.txt` (`hold BTN[+BTN] N`, `tap BTN`, `wait N`,
  `shot NAME`, `state`, `quit`). The pad states are keyed to VBlanks exactly like a route, and
  every press is written to `./inputs.log` as a ready-to-paste schedule line. At the handoff it
  detaches the route's own listener (`DW1ROUTE_DETACHED`) and releases the buttons the route
  holds, so no scheduled press fires after it. It prints the
  screen, positions, clock and care fields after each batch and takes a screenshot `s_<vblank>`.
  How the two routes were found: send a command, read the state line and the screenshot, repeat.
- `oracle/route-new-game.lua`: profiles `file-city-walk` (new-game + the walk schedule) and
  `care-fixture` (continue + the care schedule); `oracle/replay.lua` records them with
  `DW1ROUTE_PROFILE=<profile> DW1REPLAY_MODE=record DW1REPLAY_TARGET=<last milestone>` (the care
  recording also needs `DW1REPLAY_CARD1_FIXTURE=ember-day2-gabumon` and the fixture as the cwd's
  `memcard1.mcd`).
- Menu input in the game (US build): Triangle opens and cancels, Cross confirms, D-pad moves
  (the Redux override bit numbers are the hardware ones, `docs/oracle-route.md`). Taps closer
  than ~30 VBlanks apart are sometimes dropped by the menus.

## Native fixes

- **`GsSortOt`** (libgs, `crates/psyq/src/libgs/mod.rs`), reached by the DIGIMON status screen
  (the partner model is drawn into its own table, then sorted into the frame's). Sony's
  function, observed: `dst`'s entry `src->point - dst->offset` is set to link to `src->tag`; the
  end of `src`'s chain takes that entry's old link; nothing else changes; the return value is
  `dst`. The end of the chain is the last element of the chain that starts at `src`'s entry 0:
  entry 0 itself when it is empty, else the last packet hanging off bucket 0 (its length byte
  stays). Measured after review (gpt-6.1-sol found that the first version overwrote entry 0
  even with packets behind it): 352 `call-sdk.lua` requests
  (`tools/oracle/sortot_requests.py`, `libgs-sortot`: sources of 1 to 16 entries, destinations
  of 2 to 32, entries going through packets, bucket 0 empty, with one packet or with two), 141
  compared with every table and packet word of both tables checked, 40 or more with packets in
  bucket 0. On the PS1 an emptied table's entry 0 links to the SDK's own empty packets
  (`0x116bf8`) and the chain ends there; natively it ends at `LINK_END`, and the test maps one to
  the other. Fed a chain ending in the `0xFFFFFF` marker instead, the SDK overwrote entry 0
  whatever hung off it (97 requests); the game never has that state and it isn't copied.
  Also 13 captured calls from the care route (`captures/oracle-runs/g2d-sortot`,
  `oracle/capture-calls.lua` with `DW1ROUTE_PROFILE=care-fixture`), all words equal
  (`crates/psyq/tests/libgs_sortot.rs`). **An index outside the destination** (`point - offset`
  below 0 or past the table; the SDK writes outside it) **stops the program** like an
  unimplemented call (`crates/psyq/tests/sortot_traps.rs`, subprocess cases). A first request
  set with random link words crashed the emulated game (the SDK walks the chain), so the
  entries are chained like the game's.
- **decomp fork, local branch `dw1-native-phase4`** (one commit on `1f6a0d1`, `d29807d`, **not
  pushed**): the digivice's two framebuffer ordering tables (`FRAMEBUFFER_OT`, in `main.c`) sat
  at `TEXTURE_BUFFER` and `+0x14` (the PS1 `sizeof(GsOT)`); natively the struct is 32 bytes, so
  the second table was misaligned (a Rust panic in `GsClearOt`) and overlapped its tag array.
  Under `DW1_NATIVE` they are at `+0` and `+0x20` and the tag arrays move to `+0x40` and
  `+0x2040`.

## Left

- A longer care replay with a need that triggers (the poop at ~34000), which needs the hooks to
  survive VBlank 32768 (a different breakpoint design or a Redux fix) or a fixture closer to a
  poop, and feeding (the fixture has no food).
- The shops and other map exits of File City (the exits are not mapped yet), and the route's
  unvisited scripted scenes (the wild Agumon meeting leads to a battle).
- Frames of both replays against Redux's (only the canonical state was compared).
