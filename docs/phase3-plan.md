# Phase 3: load a save (read only)

2026-10-14. Fixture: Ember's `captures/fixtures/ember-day2-gabumon.mcd` (local; notes in its
README.txt): a raw 128 KB DuckStation card, one DW1 save in slot 1, Gabumon, about day 2.

## Result

The canonical state after the load equals the emulator's. With the `continue-fixture` replay all
nine checkpoints (poll 0 to the final one at poll 1829) are byte-identical, including the one
at the load (`main_state` 0 to 1, poll 884) and the screen change to 168 (File City, poll 888).
That needed two temporary local changes that are **not** in the tree, because other work owns
them (below). Without them native stops at the first 2D sort, after checkpoint 5 (the screen
change), so checkpoints 0-5 are what a clean tree produces today.

## Card format

DuckStation's raw per-game card (128 KB, `MC` header, same frame layout) loads as-is in both
PCSX-Redux (it copied the file to `memcard1.mcd`, listed "Ember Gabumon" and continued) and the
native `Card::from_bytes`. Redux's own cards are the same raw format, so there is no difference
for this card. Only this one format is tested.

## Oracle route and recording

`oracle/route-new-game.lua` takes `DW1ROUTE_PROFILE=continue` and reads the schedule from
`oracle/route-continue-schedule.lua`. Run in a fresh directory with the fixture copied to
`memcard1.mcd` (Redux reads cards from its cwd with `-portable`); command line in
`docs/oracle-route.md`. Recorded with `oracle/replay.lua` (`DW1REPLAY_MODE=record`,
`DW1ROUTE_PROFILE=continue`) as `oracle/replays/continue-fixture.txt` (pad states only).

| VBlank | Input / screen |
|---|---|
| 700-1900, every 150 | Start tap (logos) |
| 2350 | title menu, cursor on NEW GAME |
| 2380 | Down (CONTINUE GAME) |
| 2430 | Cross: CONTINUE GAME |
| 2500 | Cross: CONTINUE SLOT 1 |
| 2680 | save list ("1 Ember Gabumon") |
| 2700 | Cross: save 1 |
| 2850 | "Continue adventure?" Yes/No |
| 2900 | Cross: Yes |
| about 3040 | the load (`main_state` 1) |
| 3100 | black loading screen |
| 3800 | File City, HUD up |
| 5000 | end, no input after 2900 |

1829 polls, final VBlank 5000. Checkpoints (VBlank of the recording): 0-2 initial values at 1651,
`every600` 2620, `main1` 3043 (the load), `screen168` 3530 (File City), `every1200` 4057,
`every1800` 4957, `final` 5000.

## Native

`DW1_CARD1=<fixture> DW1_REPLAY=oracle/replays/continue-fixture.txt DW1_OUT=<dir>
./target/debug/dw1`, then `tools/oracle/compare_canon.py <dir> <oracle dir> --allow-unpaired`.
Card reads, the game's own checksum code and `loadSavegame` needed nothing new (timing from the
libmcrd model, unchanged). Added:

- `libgpu`: `OpenTIM`, `ReadTIM`, `LoadClut` (the map loader's TIM images).
- decomp (fork branch `dw1-native-continue`, one commit on `62ff326`): `clearMapDigimon` writes
  a byte through a null `ENTITY_TABLE` slot (low RAM on the PS1); skipped natively.
- Tests (`crates/decomp-c`): the fixture's save passes `createSavegameChecksum` (the game's C,
  both slot payloads' layout 0x4FC checksum offset), and a flipped bit in either the 0x4E4 or
  the 0xA00 checksummed range no longer matches. Ignored when the fixture is absent
  (`DW1_FIXTURE_CARD` overrides the path).

## Left

- Past checkpoint 5 native stops at `GsSortSprite` (the 2D sorts: another agent).
- Then `GsSortObject4J` traps on TMD mode 0x3E (0x3C with the semi-transparency bit) in the
  TG4L handler, in `renderDigimon` (the partner): an unobserved case in `libgs/sort4j.rs`. With
  the 2D sorts as no-ops and that bit let through as 0x3C, the run reaches the final checkpoint
  and all nine are identical (the canonical state doesn't include the picture). The 0x3E case
  needs a capture (`oracle/capture-libgs.lua` on this route) before it is implemented.
- The picture of the loaded scene hasn't been compared yet.
