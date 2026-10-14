# Phase 7: saving (plan and progress)

2026-10-14, session 4. PLAN.md's Phase 7: "Writing cards. Back up any card before a build
writes to it; write atomically; keep unrelated blocks and unknown bytes. Done when emulator ->
port -> emulator round-trips work for both slots, a failed write leaves the old card intact,
and saves made in the port load on the emulator with equal state." Plus Ember's addition: save
slots in the app's own format instead of memory cards (below, done the same session).

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

## Save slots (Ember's addition, 2026-10-14)

Ember: "I want to change the saving system to move away from a Memory Card system to a standard
save slot system. One slot per character as usual, but no memory cards. I want the saving system
itself to make sense for the PC rather than for the console", and "change the loading/saving to
our own optimized format. Make sure you're able to convert the saves so they can still be useful
for you." Chosen with Ember: 15 slots; New Game on a used slot asks, then overwrites.

**How the game saves:** it already thinks in slots. New Game picks a card, then an empty slot
(15 per card); Continue picks a card and a slot; saving during play ("Save and go to sleep" at
bedtime, the Battle Registration machine) writes back to the slot the game came from
(`MEMORY_CARD_ID`, `MEMORY_CARD_SLOT`) without asking. The title menu also has Delete Game and
Battle Mode (each player picks a save holding registered Digimon).

**The design:**

- **Screens** (the decomp's `main_menu.c`, under `DW1_NATIVE`, on when the app sets
  `dw1_pc_saves`, `crates/psyq/src/native.rs`): each title menu item opens its slot list at once
  (no "START SLOT 1 / 2 / NO MEMORY CARD", no "Checking Memory Card"); the list is titled NEW
  GAME, CONTINUE GAME, DELETE GAME or BATTLE: PLAYER 1/2; New Game can take a used slot after
  "Overwrite adventure?" (No first) and shows used slots as selectable; backing out goes to the
  title; Delete reloads the list; Battle Mode's player 2 picks from the same slots (on pad 2, as
  on the PS1). The operation and error boxes lose their memory card lines ("Saving data file";
  "Save Data / Data Error"); a partner in training "cannot be registered" without the card
  wording. Everything else (the sleep menu, the registration machine, the boxes and font) is the
  game's own. With the flag off the code is the PS1's: the nine certified replays still match.
- **Inside**, the slots are the 15 saves of one memory card that is always there, with every
  card command instant (`libmcrd::set_instant`) and the second card absent, so the game's own
  save logic, checksums and error paths run unchanged.
- **On disk** (`crates/dw1/src/saves.rs`): `~/Library/Application Support/dw1-silicon/saves/
  slot-01.dw1save` .. `slot-15.dw1save`, in the app's format: a 32-byte header (`DW1SAVE\0`,
  version 1, length, CRC-32, time written) and the game's `SavegamePayload` (3,840 bytes) once:
  3,872 bytes instead of an 8 KB card file holding the card's icon and title and a second copy
  of the save (the PS1's guard against a failed card write; here writes are atomic). The file
  name is the slot. A change is written atomically and durably; the save it replaces (or a
  deletion removes) is kept first in `saves/backups/slot-NN.dw1save`. A file that can't be read
  stops the game; one that isn't a valid save is reported, its slot shows as free, and it is
  backed up before anything replaces it.
- **Converting** (so emulator saves and oracle runs still work): the card file is rebuilt by
  `dw1_card_file_from_save` (C, the same steps the game uses when it saves; only title bytes the
  game would have left from an earlier save differ). `dw1 export-card <file>` writes every slot
  to a 128 KB memory card for an emulator; `dw1 import-card <file>` copies a card's saves into free
  slots (keeping their slot number when it's free). The first run seeds the slots from the cards
  earlier builds kept (`card1.mcd`, `card2.mcd`), else `DW1_CARD1`/`DW1_CARD2`: `dw1-mac` points
  at Ember's DuckStation card (read only), so Save1 and Save2 arrive as slots 1 and 2.
- The window build uses slots; headless runs use the two cards the emulator comparisons need
  (`DW1_SAVES=slots|cards` overrides; a replay refuses slots).

**Checked (2026-10-14):** headless runs with scripted pads (`DW1_PAD_SCRIPT`, new) through
Continue, New Game into an empty slot, New Game over a used slot (backup kept), Delete Game,
Battle Mode (to "Digimon Not Registered" and back to the list), and an in-game save: Ember's
save idled to bedtime, Sleep from the Triangle menu, "Save and go to sleep" (slot file updated,
old save in `backups/`), then Continue from it. Unit tests: the format, card rebuild (Ember's
DuckStation save round-trips), the keeper's writes, backups and deletions, damaged files,
merging, libmcrd's instant mode. **Saves made in the port load on the emulator with equal
state:** that bedtime save, exported with `export-card`
(`captures/fixtures/p7-exported-sleep-save.mcd`), recorded in PCSX-Redux with the continue
route (`captures/oracle-runs/p7-exported-continue`) and replayed natively from the same card:
9 of 9 canonical-state checkpoints identical.

Reviewed twice by gpt-6.1-sol. The first review found: an import could replace a slot file the
load had left out (damaged, or a newer format) without backing it up; a save with a valid
checksum but an out-of-range partner number would index the game's name table out of bounds
while the virtual card was built; an interrupted first-run seed (or an `import-card` before
the first launch) left a `saves/` directory that ended seeding for good; a write retried after
its rename went through backed the new save up over the previous one; `slot-1.dw1save`-style
aliases loaded but were never written or deleted. Fixed: imports go through the keeper's
backup-aware write; saves need a partner the game knows (and the C helper checks too); seeding
writes `saves.seeding/` and renames it into place (`saves::open`, used by the game and the
commands); a slot file already holding the save being written isn't backed up again; only the
keeper's own names count. The re-review's points, fixed too: the save directory is synced
before a backup's original goes, deletions sync on retry, the parent of `saves/` is synced on
every start, and a kept card that can't be checked stops the seed instead of being skipped.

Not done: a recorded emulator replay of the in-game save itself (bedtime is about 70,000 VBlanks
after Ember's save, past the pinned Redux's Lua limit near 32,768; the card write path is the one
the new-game replays already check byte for byte), the slot list's second line (empty in the US
game; it could show the day and place), multi-generation backups.

## Review (gpt-6.1-sol, 2026-10-14)

The first version's review found: two processes could corrupt each other's saves (shared temp
name, no lock), a failed write was only printed, the backup was overwritten non-atomically
before the save, the rename's directory entry wasn't synced, unreadable saved cards silently
turned saving off, a missing launcher fixture stopped the game before a saved card could win,
replays could start from a kept card their check didn't see, and a card dump could overwrite
the kept cards. All fixed as above. The re-review's remaining points, fixed too: a save that didn't reach
the disk now shows in the window's title until a later one succeeds (and a run that ends with
cards unsaved exits with status 3); dumps refuse the default save directory as well as
`DW1_SAVE_DIR` (an empty `DW1_OUT` counts as "."); a saved card whose metadata can't be read
stops the program (only "not found" seeds it); the directory sync covers bare file names. Not
done: the game's own save-error screen for a failed write (its timing isn't measured in the
emulator), backups beyond one generation.
