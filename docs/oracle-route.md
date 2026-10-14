# Oracle route: new game to first gameplay frame

2026-10-13. `oracle/route-new-game.lua`, run on the pinned PCSX-Redux (`ref/pcsx-redux` at
26bfd693; the pin and why are in `docs/s08-oracle.md`). Always use `ref/pcsx-redux/pcsx-redux`,
never a copy, and never run two instances at once (concurrent runs of this build stalled).
Inputs are keyed to VBlank numbers (counted from the first `GPU::Vsync` event); no game state is
read.

An earlier version of this route was derived on Redux 5811e22, whose GTE register-write bug
(PR #2283) garbled every 3D character and made the dialogue stall; its schedule and VBlanks are
obsolete.

## Run it

```
R=/Users/fenn/Developer/dw1-silicon
mkdir -p captures/oracle-runs/route-run && cd captures/oracle-runs/route-run   # fresh and empty
"$R/ref/pcsx-redux/pcsx-redux" -cli -portable -interpreter -softgpu -bios "$R/scph1001.bin" \
    -iso "$R/disc/Digimon World (USA).cue" -dofile "$R/oracle/route-new-game.lua" -run > run.log
python3 "$R/oracle/raw2png.py" .
grep DW1ROUTE run.log
```

19400 VBlanks, about 380 s of wall time (the interpreter runs a little faster than real time).
The script quits itself with `PCSX.quit(0)`. A fresh directory matters: a memory card carried over
changes the save-slot screens. Another script can stop the route earlier or hook it by setting
`DW1ROUTE_TARGET` (a milestone name) and `DW1ROUTE_ON_TARGET` (called at that VBlank instead of
quitting) before `dofile`ing it.

## Screen sequence

Milestone VBlanks are screenshot times, not the moment a screen first appears.

| VBlank | Milestone | Screen / input |
|---|---|---|
| 700-1900, every 150 | | Start tap (4 VBlanks), to get past the logos/FMV before the title. Not tested whether all are needed |
| 800 | `bios_logo` | PlayStation logo |
| ~2100 | | Title menu appears |
| 2350 | `title_menu` | NEW GAME / CONTINUE / DELETE / BATTLE MODE, cursor on NEW GAME |
| 2400 | | Cross: NEW GAME |
| 2500 | `memcard_select` | START SLOT 1 / SLOT 2 / NO MEMORY CARD |
| 2600 | | Cross: START SLOT 1 ("Checking Memory Card" for ~300 VBlanks) |
| 3000 | `save_slot_select` | Five save slots, slot 1 highlighted. Cross at 3000 |
| 3190 | `record_new_adventure` | "Record new adventure?" Yes/No, Yes default. Cross at 3200 |
| 3600-19080, every 60 | | Cross tap (4 VBlanks) for everything below |
| 3700 | `jijimon_intro_dialogue` | Jijimon "I'm Jijimon. Tell me about yourself." |
| ~4100 | | "What's your name?" then name entry |
| 4400 | `name_entry` | Name grid, cursor on A, nothing preset. Each Cross types an A; after six the cursor jumps to OK and the next Cross confirms. Name is **AAAAAA** |
| 6000 | `intro_fmv` | Intro FMV (roughly 4800 to 12500; Cross taps did not skip it) |
| 13000 | `caption_file_city` | "File City" caption on black |
| 14000 | `first_partner_dialogue` | Field scene: tamer, Agumon, Jijimon, Tokomon and others (unchanged from the pinned-build capture) |
| 16000 | `caption_jijimons_house` | "Jijimon's house" caption |
| 18000 | `house_dialogue` | Inside Jijimon's house, "Thanks, you're our only hope." |
| 18300 | `lets_go_dialogue` | "Let's go AAAAAA!" |
| 18700 | `fade_to_gameplay` | Black |
| 19100 | `gameplay_start` | First gameplay frame: tamer standing at the house entrance, clock/day HUD top left, two gauges bottom right, no dialogue. Agumon is behind the tamer and hidden |
| 19300 | | Down held 90 VBlanks (walks a few steps so Agumon steps out) |
| 19400 | `gameplay` | **Target.** Tamer and Agumon both on screen, HUD up |

BIOS rand state (RAM 0x9010) at the target: `0xb738fdb3` (`DW1ROUTE rand_state=0xb738fdb3
vblank=19400`). It was the same at 19100 and does not move while idle, but it does depend on the
walk: with a 60-VBlank Down hold the state at 19400 was `0x0c02bf70`, so it is a sensitive check of
the input schedule.

## Reproducibility

Two fresh runs one after the other (G then H, separate empty directories) produced identical
`DW1ROUTE` output (every milestone at the same VBlank, same rand state) and byte-identical
screenshots for all 16 milestones. Final screenshot:
`captures/oracle-runs/route/runG/gameplay.png` (also `.raw`/`.txt`).

## Fragile spots

- **Blind inputs.** The route does not read the screen; from 3600 on it is Cross every 60 VBlanks
  and depends on every line taking the number of presses it did. A different disc image, BIOS,
  emulator build, or a non-empty memory card can shift it. Prefer re-deriving from screenshots over
  trusting the numbers.
- **Press spacing is no longer an issue.** On the buggy build the dialogue stalled at "What?"
  unless presses were 200 VBlanks apart; on the pinned build plain taps every 60 work all the way.
- **Walk length changes the partner's position and the rand state.** Down held 60 VBlanks left
  Agumon still hidden behind the tamer at 19400; 90 shows him.
- **Savestates are not usable for iterating.** Loading a state saved in-game
  (`PCSX.createSaveState` / `PCSX.loadSaveState`) sent the game back to the intro movie (seen on
  the buggy build; not re-tested).
- Screenshots are 320x240, 16-bit (bpp 0), converted by `oracle/raw2png.py`.

## Replays

`oracle/replay.lua` records the pad polls this route produces (`oracle/replays/new-game.txt`, 7131
polls to VBlank 19400) and replays them with no pad overrides; two fresh-process replays gave
canonical state dumps byte-identical to the recording's. The recording run with breakpoints on
reached the same BIOS rand state (`0xb738fdb3`) and the same 16 screenshots as the plain route.
See `docs/s08-oracle.md`.

## Continue route (Phase 3)

`DW1ROUTE_PROFILE=continue` swaps in `oracle/route-continue-schedule.lua`: boot with a card in
the cwd's `memcard1.mcd` (the fixture), CONTINUE GAME, slot 1, save 1, Yes, then no input.
Target 5000 VBlanks. Schedule and screens: `docs/phase3-plan.md`. Record with
`DW1ROUTE_PROFILE=continue DW1REPLAY_MODE=record` and `-dofile oracle/replay.lua`; replay with
`DW1REPLAY_MODE=replay DW1REPLAY_FILE=oracle/replays/continue-fixture.txt`. The route is blind
like the other and depends on the card's contents.

## Phase 4 routes

Two more profiles (`docs/phase4-plan.md` has the schedules, checkpoints and results):

- `DW1ROUTE_PROFILE=file-city-walk`: the new-game route, then `oracle/route-file-city-walk-schedule.lua`
  (the yard, the plaza, back, Jijimon's house and out, the plaza again; five map changes, to
  VBlank 23380). Recorded as `oracle/replays/file-city-walk.txt` with fresh cards.
- `DW1ROUTE_PROFILE=care-fixture`: the continue route, then `oracle/route-care-schedule.lua` (the
  game menu: item sort, status and tech screens; then idle, game clock 07:00 to 14:16; to VBlank
  31000). Recorded as `oracle/replays/care-fixture.txt` with the fixture card
  (`DW1REPLAY_CARD1_FIXTURE=ember-day2-gabumon`, the fixture as `memcard1.mcd` in the cwd).

Record: `DW1ROUTE_PROFILE=<profile> DW1REPLAY_MODE=record DW1REPLAY_TARGET=walk_done|care_done
-dofile oracle/replay.lua`, in a fresh directory like the other routes. **Keep routes under VBlank
~32700:** in the pinned Redux the Lua hooks stop working near VBlank 32768 (breakpoints are
deleted, listeners throw "stack overflow").

**Finding a route** (`oracle/explore.lua`): the same route with `DW1EXPLORE_TARGET=<milestone>`
instead of a target, then turn-based control. Emulation is stopped inside the VBlank event while
it waits for `./cmd.txt` (`hold UP+RIGHT 30`, `tap CROSS`, `wait 60`, `shot name`, `state`,
`quit`), so what the game sees is a pure function of VBlank number, and `./inputs.log` collects
the schedule lines to paste into a route file. After each batch it prints
`EXPLORE state vblank=... screen=... tamer=(x,y,z) clock=... cond=... ...` and writes `s_<vblank>.raw`
(convert with `oracle/raw2png.py`). A new profile is the base profile plus an extra schedule file
(`EXTRA` in `route-new-game.lua`).

Controls in the game (US): Triangle opens the menu and cancels, Cross confirms, D-pad moves.
Map changes show a black caption ("File City", "Jijimon's house") for about 300 VBlanks before
control comes back. Walking is 16 game units per VBlank; the yard (screen 204) ends at
z about -4200 (south) and the plaza (180) starts at z 3370 (its north edge).
