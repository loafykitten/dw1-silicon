# Phase 7: saving (plan and progress)

2026-10-14, session 4. PLAN.md's Phase 7: "Writing cards. Back up any card before a build
writes to it; write atomically; keep unrelated blocks and unknown bytes. Done when emulator ->
port -> emulator round-trips work for both slots, a failed write leaves the old card intact,
and saves made in the port load on the emulator with equal state." Plus Ember's addition: one
save slot instead of memory cards (below, not started: it needs Ember's call on the design).

## The port writes the emulator's bytes

libmcrd has kept a byte-level card model since Phase 3 (`crates/psyq/src/libmcrd.rs`: the
PS1 card file system, writes included), but kept it in memory. `DW1_CARD_DUMP=1` (headless)
now writes both card images next to the final checkpoint, to set beside the `memcard1.mcd` and
`memcard2.mcd` PCSX-Redux saves in its run directory after the same replay. All nine certified
replays: both cards byte-identical to the emulator's. Two of them write a card (`new-game` and
`file-city-walk` create the save when the new game starts); the other seven only read Ember's
fixtures and leave them as they were, on both sides.

Because the bytes are the emulator's, emulator -> port -> emulator holds on these paths: a card
the port wrote is the card the emulator would have written. Not yet covered: an in-game save
(the save machine, script instruction 48, `tickSaveMachine`), which needs a recorded route to
it; the write path is the same `MemCardWriteFile` with a different save in the buffer.

## Keeping the cards

- libmcrd keeps a slot's card in a file when the app says so (`set_card_file`): after each call
  that can change a card (write, create, delete, format), a card that differs from what was
  last written is written to `<file>.tmp`, synced, and renamed over the file, so a crash or a
  failed write leaves the old file whole. The first write of a run first copies the old file to
  `<file>.bak`. A write that fails is reported and tried again at the next change.
- The window build keeps cards in `~/Library/Application Support/dw1-silicon/card1.mcd` and
  `card2.mcd` (`DW1_SAVE_DIR` to put them elsewhere, empty to keep nothing). A card file there
  is the card; `DW1_CARD1`/`2` only seed a card that isn't there yet, so the launcher's fixture
  doesn't undo the player's saves. Headless runs keep nothing unless `DW1_SAVE_DIR` is set (the
  replays stay read-only).
- `dw1-mac` seeds card 1 from Ember's fixture the first time; `--fresh` keeps nothing.
- Tested: `libmcrd::persist_tests` (atomic write, backup, nothing written when nothing
  changed), and the new-game replay with `DW1_SAVE_DIR` set leaves a `card1.mcd` byte-identical
  to the emulator's (card 2, unchanged, isn't written).

## One save slot (Ember's addition): questions before starting

How the save and load screens and their card checks should map onto one slot is a design call
for Ember (PLAN.md, Phase 7): a native-only menu path under `DW1_NATIVE`, or a libmcrd that
presents one always-present card with one save and no free-space or format prompts; and how
`.mcd` import/export should look. The two-card model stays for the replays either way.
