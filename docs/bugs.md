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

### Birdra Transport: blank destination names (native proposal, 2026-11-10)

- **Report/repro:** the player consistently saw fares but no destination names when opening
  Birdramon's transport list, including after restarting. A private F12 snapshot confirmed
  this at both 1x and 6x (screen 207, game day 14). It was not a new fatal crash. The snapshot,
  save copies, test fixtures and screenshots stay local and are not included here.
- **Cause:** US `drawString2` adds `adv` after a space, but only assigns it after drawing a
  non-space glyph. The destination names have leading padding, so the initial advance is
  uninitialised. The fare-column control resets the position, allowing numbers to survive.
- **Proposed correction:** native US only: initialise the advance to zero in variable-width
  mode (neutral leading padding) or twelve in fixed-cell mode. Leave the original declaration
  in non-native builds and all other-region paths unchanged. Once a glyph assigns an advance,
  keep the existing previous-glyph padding: resetting every space to the blank glyph's own
  width moves shop digits into the wrong source rectangle. No font, label, layout, GPU,
  save-format, input or gameplay-tick changes are proposed.
- **Policy/limits:** this intentionally removes native undefined behaviour; the chosen
  initial advance is NOT certified as original-PS1 spacing. Inspection of the original GAME
  routine (not Sony SDK code) showed incoming-register reuse there too. No SDK implementation
  was translated. Longer labels may still clip at the fare column; menu redesign is out of
  scope. Original-emulator pixels, the full oracle fixture suite and a fresh independent
  model review remain outstanding. Keep the contribution a draft for maintainer review.
- **Synthetic test:** `cargo test --locked -p decomp-c --test textbox_spaces` exercises the
  real linked game C and GPU with synthetic text only. Sixty combinations cover ASCII/JIS
  leading padding, repeated/internal/all-space/no-space strings, both width modes and two
  origins. Fixed cells are compared with direct glyph draws; variable leading padding with
  just the prefix removed. Thirty-two explicit numeric-padding cases compare direct glyph/
  cell positions after wide/narrow prefixes rather than using `drawString2` as their reference.
  The old code fails; debug/release pass after the correction. This is a native regression,
  not a PS1-oracle test. No disc/save fixtures are required for this test.
- **Clean contribution checks:** `cargo test --workspace --locked`: 228 passed, 0 failed,
  27 fixture-dependent ignored. `cargo build --release -p dw1 --locked` passes. Targeted
  rustfmt and `git diff --check` pass; unrelated pre-existing formatting is left unchanged.
  The retained non-native source is byte-identical when the new native declaration branch
  is removed. No matching retail/compiler build is claimed.
- **Clean runtime checks (local-only fixtures):** a private copied/relocated save fixture
  reproduces blank labels before the change and visible labels after it on this clean
  branch. Menu open/cancel works at 1x and 6x, with 12/12 before/after native canonical
  checkpoints identical and 12/12 identical between scales. The existing new-game recording
  completes with 19/19 native before/after checkpoints identical. These are native-to-native
  regressions, not newly recorded original-emulator/oracle comparisons. Runtime fixtures and
  pictures remain local.
- **Player acceptance:** after the correct candidate was explicitly launched, the player
  confirmed the destination list works. Only keyboard input was tested; no physical
  controller was available. The manually tested app also had separate Techsheet, keyboard
  editor, tile-row and pause fixes. Those are NOT part of this clean contribution; its
  automated checks are rerun without them. No full-playthrough or exact repetition count
  is claimed. Saves/preferences were unchanged by installing/launching that candidate.
- **Dependency:** source fix: https://github.com/loafykitten/dw_decomp/pull/2 (base `dw1-native`).
  A fresh bare-repository fetch of its unmerged commit through the upstream URL was rejected
  (`not our ref`), so `.gitmodules` TEMPORARILY points to `csonic0926/dw_decomp` and pins its
  published fix commit for reproducibility. Before ready/merge, accept/merge that fix,
  restore the original upstream URL, and repin to the accepted upstream commit.
- **Attribution:** Generated by Codex (GPT-6.1 sol).



## Fixed

(none yet)
