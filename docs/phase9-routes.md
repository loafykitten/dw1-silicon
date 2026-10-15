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

## The end of the game: Machinedramon, the ending, the credits, the return

Found 2026-10-15 (ending stream) from the scripts (`decomp/disks/us/SCN`: `MAPHEAD.SCN`'s
`SET_SCRIPT` pairs give each screen's script; `DG.SCN` starts with a table of script offsets) and
then on the emulator.

- **Where the overlays are used.** Only script 184 (screen 225, MGEN99, the top of Mt. Infinity)
  calls EAB: section 0x33 (step trigger 51, the middle of the room) runs Analogman's scene,
  `CALL_ROUTINE 0x33` (reads `BOSS_EFE.TMD`, loads EAB), `0x28` (`EAB_startBuildup`: the rings
  around Machinedramon), `CALL_DIGIMON_ROUTINE 8` (tamer state 18, `EAB_tick`: the flash, the
  wireframe spawn), then the predefined battle (pstat 0xFA = 1, enemy slot 5). After it the script
  sets flags 50 (Machinedramon beaten), 206, 207, 212, adds 7 prosperity and warps to the ending
  map, 236 or 237 (TEND01/02, picked by flags 214 and 220 and the prosperity; this save went to 237). There script 192 section
  0x33: `CALL_ROUTINE 0x34` (loads ENDI), the farewell lines, `0x36` (`gameClearSave`: the game-clear
  save prompt, MAIN_STATE 2, CURRENT_SCREEN becomes the city top), `0x35` (tamer state 16,
  `ENDI_tickEnding`: the tamer dissolves into particles that fly up). Then MAIN_STATE 3 ends the
  game loop: the credits movie (movie 3, which `MOV_playMovie` never lets the pad skip, about 7,800
  VBlanks), the title again; CONTINUE GAME with the clear save plays movie 2 (skippable) and puts
  you back in the city ("I'm back!", the Machinedramon card). With flag 50 set the room only offers
  the post-game Digitamamon egg (flag 256), so a finished save can't replay the ending.
- **The way up.** Mt. Infinity's entrance is the yard's north-east corner (step trigger 56 of
  script 162, x 3000..3300 z 3300..3500, after the step-58 crossing): screens 166, 167, 210, 212,
  219, 247, 248, 249, 253, 254, 222, 225, each with roaming enemies (one fight on the first floor
  took 2,000 VBlanks with a Digitamamon), and MetalGreymon (flag 326) blocks 222. Too long for one
  recording, and there's no save point on the mountain.
- **The fixture.** Master99's 2001 GameFAQs save (`captures/fixtures/p9-ending-master99-2001.mcd`,
  unchanged: Digitamamon, prosperity 71, flags 88 and 354 set, 50 not; the card's third save) moved
  with the new `tools/oracle/place_fixture.py` to the spot where the warp from 254 lands in the last
  corridor (screen 222, exit 0, (-400, -4100); the partner 200 behind, the waypoint trail reset as a
  warp leaves it) with MetalGreymon's flag 326 set: `captures/fixtures/p9-ending-mt-infinity.mcd`.
  Loading a save is generic (main.c's continue path), so the game starts there like from any save.
- **The replay** (`oracle/replays/ending.txt`, profile `ending` = the `title` base,
  `oracle/route-ending-continue-schedule.lua`, `oracle/route-ending-schedule.lua`; 10,198 polls, 99
  CD reads, to VBlank 31,900): up the corridor into 225 (4,940), Analogman (5,180), the buildup and
  spawn (about 6,290-6,620: `EAB_initializeRings` at 6,289, `EAB_tick`'s first call at 6,618),
  the battle (7,930-14,170, the finisher at 13,510), the ending map (18,420), the clear save written
  to the card (19,420), ENDI (about 19,980-20,480; the particles released at 20,184), the credits,
  the title (28,560), CONTINUE GAME with the clear save, movie 2 skipped, File City (31,240).
  Certified: a fresh Redux replay matched every poll, VBlank and CD read
  (`captures/oracle-runs/p9-ending-dense`); natively the 33 standard checkpoints and all 220 at
  every 50 polls are identical, and the card the game writes (the clear save) is byte-identical to
  Redux's (`DW1_CARD_DUMP`). The written card is kept as `captures/fixtures/p9-ending-clear-save.mcd`
  (a post-game save: the Digitamamon egg).
- **Frames** (spot checks at 1x against the recording's screenshots): the battle 0 pixels differ;
  the spawn 565, all on the green EAB particle lines (`LINE_F2`, the line rasteriser's known ties)
  and the model edges; the ending 45 at the screen's left edge.

## Fishing, curling, the gyms and DGET (stream fishcurl, 2026-10-15)

All three start from Master99's 2002 save made into debug fixtures (`tools/oracle/debug_fixture.py`
gained `--keep N`, `--clear N` and `--item ID`): the save is the card's third (file DMR2) and
loads in Jijimon's house (screen 205), so the routes use a `continue-save3` base, then
`route-master99-yard-schedule.lua` (out of the house into the debug yard) and
`route-master99-warp-schedule.lua` (to Mr. Warp, x -161 z -2451, and his "Unknown World" menu).

- **Fishing** (`debug-fish`, fixture `p9-fishcurl-debug-master99-rods`: the debug flags' bytes also
  hold the rod triggers 45 and 46, so `--keep 45 --keep 46`). FISH loads with the lake's map
  (screens 6 and 8, `checkFishingMap`); FISH in the triangle menu needs a rod trigger and a food
  item. The fight: Cross throws, Cross again releases (the frames between are the power), the bite
  check runs every 20 frames for fish within 200 units of the bobber at the bait's chance
  (`FISH_BITE_CHANCE`, per bait and kind: Chain melon gives Digicatfish 30%), Cross within
  `catchingTime / 2` frames of the bite strikes, then Cross held raises the tension and reels.
  Seadramon swims at hours 8 and 17 (`seadramonTime1/2` 1700 and 800 at Dragon Eye Lake) and
  bites any bait at 20% within 200 units every 8 frames; the route skips an hour first (the top
  debug Jijimon) so it fishes without him. `debug-seadramon` fishes at 08:36 instead: a cast at
  power 12 onto his first waypoint (x -1188, z -2202, where he slows), his bite, a fight played by
  a controller (Cross while the tension is under 4500 and he isn't running), the catch, "It's the
  Lake Guardian!" and the ride to Beetle Land. He spawns only while fewer than 32 fish swim, on a
  frame where `VSync(-1) / 3 % 20` is 0, and only at those hours (the clock stops while fishing). `oracle/explore.lua` gained `waitne ADDR VALUE N` (wait for the bite) and
  `autoreel` (the tension rule, run in Lua with no stops).
- **The PS1's VBlank counter:** FISH is the one place the game reads `VSync(-1)` (the bite and
  Seadramon checks: `VSync(-1) / 3 % 20`). Natively the counter ran ahead of the PS1's (loading
  takes no time), so fish bit at other frames; a replay now answers the PS1's count
  (`Replay::ps1_vcount`: libetc's `Vcount` = the recorded VBlank − 1144, the polls' pairing [obs]).
- **Curling** (`debug-curl`, fixture `p9-fishcurl-debug-master99-penguin`: `--clear 257 --clear
  357`). Penguinmon waits outside the Curling Cave only while not recruited (trigger 257) and
  offers the free first match while 357 is clear (map script 206, section 6; `DG.SCN` holds the
  map scripts indexed by the script id `MAPHEAD.SCN` gives each screen). KAR loads on screen 136.
  The match is played by a controller the schedule file returns (route-new-game.lua now calls it
  each VBlank): turns don't come at fixed VBlanks, and inputs found with the explorer started one
  VBlank apart from the route's and did not reproduce. Penguinmon wins 5 to 0. `debug-curl-win`
  throws at powers 2000, 1600, 2200, 1800 and 2400 (found by trying schedules): we win (KAR writes
  pstat 0x79 = 1) and outside Penguinmon keeps his word: trigger 257 is set and he leaves for the
  city, a recruit (DGET, despite its name, is the tournament window: below).
- **The gyms (TRN2)** (`debug-gym`, fixture `p9-fishcurl-debug-master99-flute`: `--item 115`, a
  Blue Flute). TRN2 loads on screens 99 (Ice Sanctuary), 107 and 108 (Beetle Land) and 165 (Trash
  Mountain) (`loadTrainingLibrary`). Beetle Land is reached from Dragon Eye Lake: Cross on the
  shore tiles (81) with a Blue Flute calls the Lake Guardian, who carries you there. A Tentomon at
  its entrance starts a battle; screen 106 east leads to the tree stump gym (107), whose signs are
  Cross tiles 81 and 83 ("Training: Striking", offense).
- **DGET is not recruiting:** it is the tournament schedule and registration window, loaded on
  the Arena Lobby screens 208 and 223 (`checkArenaMap`, `initTournamentSchedule`). The arena
  stream's `arena-cup` replay goes through it (screen 223, Greymon's ENTRY desk) and is certified.
- **Explorer findings:** the pad reads stale while the game reads the CD synchronously (a script
  file: the yard's step trigger 54), and a run that stopped mid-read diverged from its recording
  (Redux's CD timing follows the host clock), so the explorer now never stops within 60 VBlanks
  of a libcd call.

## Certified areas (stream tour, 2026-10-15)

Before Ember's playthrough, a tour of the world: Mr. Warp's destinations from the debug room
(fixture `debug-day2-gabumon`, Ember's day-2 Gabumon) and short walks from the downloaded saves.
Each is a recording certified by a fresh dense replay (a checkpoint every 50 polls) and run
natively with every checkpoint identical (`tools/replay-check.sh`, profiles `tour-*` in
`oracle/route-new-game.lua`, schedules `oracle/route-tour-*-schedule.lua`). Checkpoints are the
standard / dense counts.

| Replay | Areas (screens) | What happens | Checkpoints |
|---|---|---|---|
| `tour-panorama` | Mt. Panorama's peak (18), Jijimon's house (218) | Mr. Warp's script; two Goburimon, a two-enemy battle with the finisher, lost; MURD | 25 / 144 |
| `tour-jungle` | Tropical Jungle (12, 13, 34) | Darkrizamon, lost | 29 / 171 |
| `tour-drill` | Drill Tunnel (24, 25, 27) | Drimogemon's scripted battle ("It's hot!"), won, results, his lines | 32 / 218 |
| `tour-savanna` | Gear Savanna (69, 70, 71, 72) | Elecmon's line; Patamon, lost | 35 / 225 |
| `tour-mansion` | Grey Lord's Mansion (52, 53, 56, 57) | the entry cutscene, locked doors upstairs and down, three Soulmon (a three-enemy battle), lost | 31 / 174 |
| `tour-canyon` | Great Canyon (44) | Gotsumon, won (the recording; the exploration lost) | 26 / 176 |
| `tour-dino` | Ancient Dino Region gate (79), Jijimon's house | Tyrannomon at the gate, lost; MURD | 33 / 240 |
| `tour-dino-back` | Ancient Dino Region back (86, 87) | the "meteorite", the bone arch, Saberdramon, lost | 26 / 142 |
| `tour-lava` | Lava Cave (31, 30, 29, 28), Drill Tunnel hub (25) | two Drimogemon scenes ("We hit a big rock!"), through to the Drill Tunnel | 22 / 121 |
| `tour-freezeland` | Freezeland (134, 132, 134's lower part) | in front of the Curling Cave (Penguinmon left alone: talking starts the match), the hub, a Mojyamon | 22 / 143 |
| `tour-beelzemontzero` | late-game Jijimon's house (205), File City top (179), plaza (192), restaurant (215) | BeelzemontZero's save (Piximon, prosperity 100): Jijimon, Yuramon, Tyrannomon's menu, Dragon Noodle eaten | 17 / 103 |
| `tour-bob` | late-game house (205) and back room (206) | Bob's save (MegaSeadramon, night): Monzaemon's box | 17 / 124 |
| `tour-master99-2001` | house (205), city top (179), plaza (192), Native Forest (109) | Master99's 2001 save (Digitamamon): a forest battle, won | 24 / 139 |
| `tour-master99-2002` | house (205), city top (179), plaza (192), tool shop (216), Secret Item Shop (217) | Master99's 2002 save (MetalGreymon) | 18 / DENSE2002 |
| `tour-winolivas` | house (205), city top (179), Item Keeper's (211) | winolivas's save (year 17): the game menu's items, Sirloin eaten, Poyomon's tip, the storage window with a full inventory | 14 / DENSEWINO |

All five US downloaded saves load and were walked (each is its card's only save; Master99's two
sit in the third slot, `continue-save3`); the European save is for another release and was not
used.

One native fix came out of it: a scripted battle (Drimogemon's) starts with
`handleBattleStart`'s predefined-battle loop reading `ENTITY_TABLE[i]->isOnScreen` for empty
slots, which the PS1 read from low RAM (0) and wrote a 0 back to; natively it was a NULL read
(decomp `src/main/btl.c`, the same fix the ending stream made for Machinedramon). Everything else
matched with no change.

**Exploring vs recording.** Battles make an exploration and its recording drift apart (Redux's CD
timing follows the host clock even with the explorer's 60-VBlank margin around libcd calls), so a
battle may run longer in the recording and the presses after it land elsewhere. Six recordings
end differently from their exploration (canyon, dino, freezeland, bob, master99-2001 and -2002); each
schedule's header says what the recording does. Walks without battles reproduced exactly
(winolivas, lava). In-game directions are camera-relative: many screens turn the camera 45 or 90
degrees, so probe a direction with a short hold before walking blind.

## Still to reach

Other recruits, the Ice Sanctuary and Trash Mountain gyms (TRN2 on screens 99 and 165; Beetle
Land's 108 is a short walk from the `debug-gym` route), a whole tournament (a save made in the
arena lobby just before 12:00 would leave room for it; `tools/oracle/place_fixture.py` can put a
save there), the other ending map (236: the same script and ENDI path), the post-game
Digitamamon egg fight.
