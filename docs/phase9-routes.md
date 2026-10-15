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
BeelzemontZero and Bob are registered and load. Player 2's card prompt keeps its Yes disabled on
Redux: the game polls slot 2 (`updateMemoryCardState`, `MemCardExist(0x10)`) and Redux's
second card never answers present, though it loads `memcard2.mcd`. Open: why (Redux's SIO port 2
setup), or another route (a card swap in slot 1?). The route tools accept `P2_<button>` presses
for the second pad.

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

## Still to reach

Fishing (Mr. Warp, Seadra fishing; needs a rod: the 2002 Master99 save has 99 of every item),
curling (Mr. Warp, Curling Cave), tournaments (Greymon's Arena: Prosperity 15 and the Greymon
fight first; a completed save), recruiting (an early recruit, e.g. Agumon in the Native Forest,
or a Frigimon), the other gyms, the boss buildup and the ending (a completed save near the end).
Fishing's Seadramon, curling's stones, the arenas and the result scenes also need the
`GsTMDfast*` handlers the audit lists as missing (`docs/phase9-overlay-audit.md`, finding 9).
