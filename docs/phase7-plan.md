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
  last written is written to a new temporary file (`<file>.<pid>.<n>.tmp`, created exclusively),
  synced, renamed over the file, and the directory synced, so a crash, a power cut or a failed
  write leaves the old file whole. The first write of a run first keeps the card as it was on
  disk in `<file>.bak`, written the same way (a failure there leaves the old backup alone). A
  write that fails is reported loudly (the game thinks it saved) and tried again at the next
  change and at exit.
- The save directory is locked (`<dir>/lock`, exclusive) for the run: a second copy of the game
  refuses to start on it, rather than the two undoing each other's saves. A saved card file that
  can't be read, or isn't 128 KB, stops the program instead of playing on with a card that
  isn't kept. A replay with `DW1_SAVE_DIR` refuses to start if a kept card already exists (a
  replay starts from its recorded cards). `DW1_CARD_DUMP` writes atomically and never into the
  save directory.
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

## Review (gpt-6.1-sol, 2026-10-14)

The first version's review found: two processes could corrupt each other's saves (shared temp
name, no lock), a failed write was only printed, the backup was overwritten non-atomically
before the save, the rename's directory entry wasn't synced, unreadable saved cards silently
turned saving off, a missing launcher fixture stopped the game before a saved card could win,
replays could start from a kept card their check didn't see, and a card dump could overwrite
the kept cards. All fixed as above; still not done: making a failed save visible inside the game
(the game's own error path for a write isn't measured in the emulator), backups beyond one
generation.

## Open: where to save in the game

The in-game save is the "save machine" (script routine 48, `tickSaveMachine`), which the disc's
scripts call only on screens 165 (Trash Mountain) and 178 (a File City screen, TWNA12, not one
the replays reach from Ember's saves; `DG.SCN`'s script table). Routine 54 (the save menu) is on
screen 192 (TWNB13). Ember knows where they saved their DuckStation games: that's the route to
record (Open questions).
