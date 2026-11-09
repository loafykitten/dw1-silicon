# Bugs found in play (Phase 9)

Ember's playthrough (Phase 9.5, `docs/phase9-plan.md`). One entry per bug, newest first. When
something looks wrong, press **F12** in the game: a bug snapshot goes to
`~/Library/Application Support/dw1-silicon/reports/<time>/` (the screen, the game's state, the
last minute of input, the settings, copies of the save slots and the log). Say which snapshot,
and roughly what happened; the snapshot folders stay on the Mac (they hold game data and are
never committed).

For each bug:

- **What happened** and what should have happened (on the PS1 / DuckStation, if known).
- **Where:** place, day, what you were doing.
- **Snapshot:** `reports/<time>`; the save slot just before it, if there is one.
- **Status:** open / reproduced / fixed in `<commit>` / the original game does it too.

## Open

(none yet)

## Fixed

### Techsheet skill-selection grid: box-fill blend rate (2026-11-09)

- **What happened:** entering the skill-selection grid, rather than merely opening the
  Techsheet summary, stopped with `GsSortBoxFill with an unobserved attribute bit (not GsALON)`.
  The call stack was `renderBox` -> `renderDigimonMoveBox` -> `renderDigimonMovesView`.
- **Where/repro:** from the field menu, open the partner's Techsheet and confirm to enter the
  selectable skill grid. The reported case had SpitFire equipped; the game day was not recorded.
- **Cause:** `renderDigimonMoveBox` calls `renderBox` with flags 3 for unavailable techniques.
  `renderBox` sets attribute `0x60000000` (GsALON | GsATWO), but the native box sorter allowed
  only GsALON and always wrote blend rate 0.
- **Fix proposed in this branch:** allow the documented rate bits 28-29 and map them to
  GP0(E1h) ABR bits 5-6. Rate 2 subtracts the foreground from the background. Rate-0 packets
  are unchanged; unrelated attribute bits still trap, with the attribute value in the message.
  The mapping follows the Psy-Q header's GsAZERO/GsAONE/GsATWO/GsATHREE definitions and
  [psx-spx's GPU draw-mode documentation](https://psx-spx.consoledev.net/graphicsprocessingunitgpu/#gp0e1h-draw-mode-setting-aka-tpage),
  not a translation of SDK disassembly.
- **Regression evidence:** `tests/libgs_boxfill.rs` first reproduced the old abort at
  `0x60000000`, then passed after the fix. It checks all four rates with alpha on/off, full
  packet contents, OT linking, work-base advance, a packet-boundary sentinel and 1x GPU
  pixels. `tests/libgs_traps.rs` also verifies that unrelated box attribute bits still stop.
  On the clean contribution branch, `cargo test --locked --release -p psyq -p backend -p dw1`
  reports 210 passed, no failures and 19 fixture-dependent tests ignored.
- **Player verification:** a supplied screenshot shows SpitFire equipped and the complete
  skill grid rendered, and the player confirmed normal operation after the targeted check.
  No exact repetition count or full-playthrough certification is claimed.
- **Snapshot/limits:** no F12 snapshot or original-SDK oracle capture is included. Disc data,
  saves, game screenshots and executables remain local. Original-SDK packet comparison and
  the full `tools/replay-check.sh` suite have not been run for this contribution.
