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

### Native Start pause loop can freeze the host (2026-11-09)

- **Report:** Start/Enter during combat paused permanently. A local live-process sample
  consistently showed `BTL_battleMain -> handlePause -> PadRead` in the busy wait.
- **Cause:** on PS1, controller interrupts continue during the no-VSync pause loop. In this
  port the window host services input only on a virtual VBlank, while PadRead reads cached
  state. The loop cannot observe the release and next press of Start.
- **Proposed bounded correction:** the decomp submodule adds `VSync(0)` inside the shared
  `handlePause` and VS Battle Mode pause waits, under `DW1_NATIVE` only. This services input,
  sound, presentation and pacing without ticking the paused game. Original non-native
  branches, global PadRead semantics, saves and keyboard mapping are unchanged.
- **Related work:** [dw1-silicon PR #1](https://github.com/loafykitten/dw1-silicon/pull/1)
  addresses the same root cause at the SDK PadRead layer. This draft is a narrower,
  pause-loop-only alternative for maintainer comparison, not a separate newly discovered bug.
- **Synthetic regression:** `python3 tools/tests/pause_loop.py` extracts the actual shared
  pause, VS pause and VS rising-edge functions and supplies a cached event-driven pad.
  The old native body fails boundedly; the patched bodies pass 14 native and 14 asynchronous
  non-native-branch scenarios. Coverage includes long holds, release/press edges, unrelated
  buttons, field/battle guards, cleanup and both VS owners (including wrong-owner presses).
  The non-native checks reject added polling waits. No disc or save fixtures are used.
- **Live keyboard acceptance:** an isolated signed app, with a copy of the player's save,
  paused/resumed twice in a Native Forest Dokunemon battle, including a one-second held
  Enter. Cross did not resume. Two paused canonical state snapshots taken 17 seconds apart
  were byte-identical while VBlank time advanced; combat progressed after the next Start.
  Field pause/resume and subsequent movement also worked. These live checks used the local
  installed build, which also contains separate Techsheet, keyboard and map-row changes;
  none of those changes is included in this contribution. The player's save and controls
  hashes were unchanged. Screenshots, logs and state dumps remain local, not published.
- **Clean contribution branch validation:** `cargo test --workspace --locked` reports
  227 passed, 0 failed, 27 unavailable-fixture tests ignored. Release `dw1` builds. This
  branch is based directly on upstream `phase0`, excluding the other local fixes/features.
- **Dependency:** [dw_decomp draft PR #1](https://github.com/loafykitten/dw_decomp/pull/1)
  contains the nine-line native code change. Until that is merged upstream, `.gitmodules`
  temporarily points to the authorized contributor fork so a new checkout can fetch the
  pinned commit. Restore the upstream URL and repin to the accepted commit before this
  companion is made ready/merged. Neither draft depends on the separate SDK-layer #1.
- **Limits:** physical controllers and live VS Battle Mode were not tested. No fresh
  original-PS1 pause-timing capture or full-playthrough certification is claimed. The full
  oracle replay suite requires unavailable local fixture cards/captures. A before/after
  native replay of the existing new-game recording had 19/19 byte-identical checkpoints;
  this is not a new emulator certification. No independent model review is claimed.

(none yet)

## Fixed

(none yet)
