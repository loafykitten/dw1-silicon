# Phase 9: reaching the unrecorded features (routes, fixtures, the debug room)

2026-10-15, session 6. How the parts of the game no recording reached yet are reached for oracle
recordings (docs/oracle-route.md has the route machinery; docs/phase5-plan.md the method).

## The debug room (the main tool)

The game's own debug room (TCRF, "Digimon World", Debug Room) is the yard outside Jijimon's house
(screen 204) with extra actors. It appears when four story flags are 0: RAM 0x801BDFD2,
0x801BDFD3, 0x801BDFD4 and 0x801BDFE6 (TCRF's GameShark codes). They are bytes 0xFA, 0xFB, 0xFC
and 0x10E of SCRIPT_STATE (0x801BDED8), which the save keeps at payload offset 0x290 (the
payload's playerName is at 0x467, Inventory is 91 bytes, scriptState 0x17C). So:

- `tools/oracle/debug_fixture.py IN.mcd OUT.mcd` zeroes payload bytes 0x38A, 0x38B, 0x38C and
  0x39E in every valid save copy and redoes the checksum. `captures/fixtures/debug-day2-gabumon.mcd`
  is Ember's day-2 Gabumon save so changed (only those four bytes differ from RAM after loading).
- The actors appear when screen 204 is entered, not when a save loads into it: the route
  (`oracle/route-debug-room-schedule.lua`, profile `debug-room`) walks from screen 168 down into
  the plaza (180) and up into the yard (204) by VBlank 6953.
- Actors (positions in the yard as the route arrives): top left Jijimon (Half dead, Opening mode,
  Fast forward time: one hour / six hours / ten days), bottom left Jijimon (Mr. Warp: Mt.
  Panorama, Tropical Jungle, Drill Tunnel, Gear Savanna, Grey Lord's Mansion, Great Canyon, the
  Ancient Dino Region, and "Unknown World": in front of the Curling Cave, Seadra fishing at
  Dragon Eye Lake, the Lava Cave), bottom left Tokomon (the lives left, then Mr. Digivolution:
  Numemon, Sukamon, Botamon), bottom right Jijimon (a shop: Auto Pilot, every digivolution item,
  food, the rods are not in it).

Recorded and certified with it: `debug-evo` (Mr. Digivolution, Gabumon to Numemon: EVL) and
`debug-death` (ten days twice, death of old age on day 21, the dissolve, the egg, Botamon:
DOOA/DOO2).

## Battle Mode

Needs two memory cards, each with a Digimon registered at the Battle Registration machine
("Competition Data for Player 1 in Slot 1", then Player 2 in Slot 2). The `register-save`
recording's card has no registered Digimon ("Digimon Not Registered"); the downloaded saves of
BeelzemontZero (`captures/fixtures/p9-arena-vs-p1.mcd`) and Bob (`p9-arena-vs-p2.mcd`) are
registered and load.

**Why player 2 seemed stuck (found 2026-10-15):** not the card. Both cards answer: the title
screen's `MemCardSync` results alternate between slot 1 and slot 2, both 0 ("present") after
the first 3 ("new card") [obs: `captures/oracle-runs/p9-arena-cardprobe1`]. It is the second
**pad**: Redux leaves pad 2 unplugged by default (`PadsImpl::setDefaults` in
`ref/pcsx-redux/src/core/pad.cc` gives only pad 1 `Connected`), so its port answers 0xFF and the
game never sees a `P2_` press; player 2's prompt simply never received its Cross. With
`PCSX.settings.pads[2].Connected = true` the same Cross opens Bob's record list
(`p9-arena-cardprobe2`). The route machinery now does this: a profile with `pad2 = true` in
`oracle/route-new-game.lua`'s EXTRA table plugs pad 2 in, and `oracle/replay.lua` writes a
`pad2 connected` header line that makes a replay plug it in too (one-player recordings are
unchanged; natively the line is ignored: `PadRead` returns the recorded 32 bits, pad 2 in the
high half). The `battle-mode` profile (`oracle/route-battle-mode-schedule.lua`) is the certified
`oracle/replays/battle-mode.txt`.

## Shared saves and cheat codes (research, 2026-10-15)

Downloaded to `captures/downloaded-saves/gamefaqs/` (gitignored; uploaders' claims, unverified):
BeelzemontZero 2010 (max-stat Piximon, all items/cards/medals/Digimon in the city, Digimon
registered for Battle Mode), Master99 2002 ("completed everything", 99 of every item), Master99
2001 (Digitamamon, 99 of everything), winolivas 2021 (all medals, finished), Bob 2001 (registered
for Battle Mode), and a European save (not usable). All are raw 128 KB cards after stripping the
DexDrive/VGS headers (`.mcr`).

Useful RAM addresses from the published CodeBreaker codes (Almar's Guides, NTSC-U): pad 1 at
0x80134EE4, bits at 0x80134EB8, time of day 0x80134EBE, partner stats from 0x801557E0, lives
0x80155824, inventory types 0x8013D474 + slot and counts 0x8013D492 + slot (Auto Pilot 0x16,
Old Fishrod 0x74, Amazing Rod 0x75), key items 0x801BDFD2/3. Recordings never poke RAM: a
change goes into a fixture save (like the debug flags) so both sides start from the same card.

## Tournaments (Greymon's Arena)

The arena is the Arena Lobby (screen 223, ROOM19) off the south-east corner of the developed
city bottom (screen 192, step trigger 57, x 1100..1600 z -3200..-2800); Master99's 2002 save
(`captures/fixtures/p9-arena-master99.mcd`, unchanged: the debug-room flags would turn the city
back to its first state, where the plaza is screen 191 and has no arena) reaches it from Jijimon's
house in about 1,400 VBlanks. Greymon at the ENTRY desk (cross trigger 81) shows the next five
days' cups; the first column is today. Registering for today sets `PSTAT_TOURNAMENT_DAY` to `DAY`
(a later day gets bit 0x80, cleared at the day change), and from 12:00 (`updateTournamentRegistration`)
Greymon reminds you; the arena door (trigger 54, "go in by eleven pm") starts STD. The save was
registered for tomorrow, so the route cancels that and enters today's Grade C (three rounds). A
match takes about 6,000 VBlanks and the cup starts at 12:00, so from a morning save only the first
match and the second round's start fit under VBlank ~32700 (`oracle/replays/arena-cup.txt`).

## Still to reach

Fishing (Mr. Warp, Seadra fishing; needs a rod: the 2002 Master99 save has 99 of every item),
curling (Mr. Warp, Curling Cave), recruiting (an early recruit, e.g. Agumon in the Native Forest,
or a Frigimon), the other gyms, the boss buildup and the ending (a completed save near the end), a
whole tournament (a save made in the arena lobby just before 12:00 would leave room for it).
Fishing's Seadramon, curling's stones, the arenas and the result scenes also need the
`GsTMDfast*` handlers the audit lists as missing (`docs/phase9-overlay-audit.md`, finding 9).
