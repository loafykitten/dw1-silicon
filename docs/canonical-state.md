# Canonical state schema v2

2026-10-13. `oracle/canonical-state.lua` serializes the game's semantic state from PS1 RAM into a
versioned, fixed-width, little-endian binary (834 bytes) and a text rendering for diffs. The
oracle writes one at every checkpoint of a replay (`docs/s08-oracle.md`); the port will write the
same record from its own memory and compare. It is the "canonical schema" of PLAN.md's oracle
section: never a raw struct dump (padding, `jmp_buf`, pointers), with documented exclusions.

Derived from a gpt-6.1-sol survey of the MIT decomp (`decomp/` at 09c174c, struct layouts under
the 32-bit PS1 ABI) and checked by us: every symbol address below was compared with
`decomp/config/us/symbols.txt` (all matched), and the values were read at the first gameplay
frame of the new-game route (below). No game data is in this document.

## File format

`"DW1CANON"` (8 bytes), u32 schema version (2), u32 body length (818), then every field in the
table order, arrays inline, no padding, no alignment. Signed types are two's complement. The
`.txt` rendering has one `name = value` line per field (arrays space-separated; byte arrays and
RNG words in hex). Any change to the field list or encoding bumps the version.

## Rules

- **Pointers are excluded or followed, never stored.** A position is read through the entity's
  `posData` pointer (`PositionData.location` at +0x78, `rotation.vy` at +0x72); if the pointer is
  null or outside RAM the `pos_valid` byte is 0 and the position and yaw are 0. Script cursors
  (`SCRIPT_POINTER`, `ACTIVE_SCRIPT`, the stack's `scriptPtr`) become a buffer kind (0 null, 1 the
  maphead SCN buffer, 2 the cached map SCN buffer, 3 anywhere else) plus a byte offset from that
  buffer's base (`MAPHEAD_SCRIPT_PTR`, `MAP_SCRIPT_PTR`); kind 3 and null have offset 0.
- **Script stack entries at or beyond the depth** (`SCRIPT_STACK_OFFSET`) are stale bytes and are
  written as zero. **Within an active entry, only the members its type sets are recorded** (v2):
  `smth[0]` is the type. Every pusher in `script_interp.c` sets: type 1 (call: `JUMP_AND_LINK`,
  `JUMP_TO_FILE_AND_LINK`, in both script interpreters) the script pointer, script id and `smth[0]`; type 4
  (`WARP_TO`, `TELEPORT`, `SECTION_ON_EXIT` and a battle-outcome return) `smth[0]` and `smth[1]` only (the section to run, 0xFF for the maphead); type 3 (opcode 0xFB,
  map reload) `smth[0]` only. So buffer, offset and script id are recorded only when `smth[0] == 1`,
  and `smth1` only when `smth[0] == 4`; everything else is zero (it is stale stack memory that
  differs between the PS1 and the native build).
- **Layout was verified from the code:** position stride 0x88, location +0x78, yaw +0x72 (the
  `initializeDigimonObject` and `setTamerDirection` assembly); `triggers` +0xF5, `pstats` +0x159 and
  the stack at +0x25C inside `ScriptState` (the `getTriggerOffset`, `readPStat` and
  `pushScriptStack` assembly).
- The partner's species is `PARTNER_ENTITY.entity.type` (the index into `DIGIMON_DATA`; 3 is
  Agumon, 0 the main character, 1 Botamon, 2 Koromon); `partner.moves` are animation IDs (0x2E and
  up, 0xFF empty), not technique IDs; technique `m` is learned iff `(learned_moves[m/32] >> (m%32)) & 1`.
- `progress.triggers` is an 800-bit field (flag `n` is byte `n/8`, mask `1 << (n%8)`); `pstats`
  are 256 byte-valued script variables (index 0 is the time speed, 1 prosperity).
- Clock: 20 ticks per game minute; `current_frame = hour*1200 + minute*20 + phase`, so the
  sub-minute phase that drives care updates is kept raw.
- `partner.para.condition` bits: 1 sleepy, 2 tired, 4 hungry, 8 poopy, 0x10 unhappy, 0x20
  injured, 0x40 sick. `partner_state` 3 is asleep, 6 toilet, 1 normal.
- RNGs: `custom_val1/2` are the game's `customRandom` states (`s = s * 0x41C650AD + 0x3039`,
  VAL2 starts at 0x0013CC25, VAL1 at 0); `bios_rand_state` is the BIOS `rand()` state at RAM
  0x9010 (`x = x * 0x41C64E6D + 0x3039`, see `docs/s08-oracle.md`).

## v2 changes

v1 recorded stale members of active script-stack entries (a warp or map-reload entry's pointer
and id, a call's `smth[1]`), which differ between PS1 and native memory. v2 zeroes them as above;
the version is 2 and nothing else changed (same 834 bytes and field order). The v1 and v2
records of the new-game route differ only where such an entry was pending at a checkpoint.

## Exclusions

| Excluded | Why |
|---|---|
| `Entity.posData`, `animPtr`, `anim.*Ptr`, `PositionData` pointers/matrices, `GsDOBJ2` | Pointers and renderer state. Positions are read through `posData` instead. |
| `STORED_TAMER_POS` (0x80138720) | Auxiliary position, not the live one. |
| `PartnerPara.unused1..12`, `PartnerEntity.unk1/unk2` and `pad`, `CurrentStats.unk1` and `field7`, `ScriptState.smth` (212 bytes), `SavedState.unk20` | Padding or unknown meaning; not guessed into the record. |
| Other `CurrentStats` (`efeSubEffect`, `isHit`, `vabId`, `chargeMode`) | Battle/animation scratch; add when a battle checkpoint needs them. |
| `INVENTORY.names` | UI render index (swapped with the slot on sort); slot order is kept via ids and counts. |
| `SCRIPT_JMP_BUF` | CPU context (PC/SP/registers). |
| Input words and repeat timers (`POLLED_INPUT` etc.) | The replay's pad polls already pin them; add to compare engines' input handling. |
| Animation state, entity table (`ENTITY_TABLE`), overlay-local battle and minigame machines | Out of scope for v1; a scoped survey is needed first. |
| `SavedState` (0x801555D0) | Save/load staging, filled from live state on load. |

Not covered by v1 and worth a later version: the whole `loadSavegame` staging only copies 0x17C
bytes of `ScriptState`, so a save is not a substitute for this record; `tamer.entity_type` (0) and
the other fields of `TamerEntity` are only the ones listed.

## Spot checks at the first gameplay frame

Checkpoint `final` of the new-game route (VBlank 19400, in Jijimon's house, screen 204): partner
species 3 (Agumon); name `82 60` x6 (Shift-JIS fullwidth A, the route's AAAAAA); HP/MP 800/600,
offense/defense/speed/brain 80/60/70/70; 3 lives; learned techniques bitset 0x4; moves
46/255/255/53; hunger/energy/weight 52/20/15; clock year 0 day 0 08:09 (`current_frame` 9788 =
8*1200 + 9*20 + 8); `time_speed` 0; tamer money 0, level 0, raised count 1; tamer and partner
positions valid (z -2735 and -2241); inventory size 10 and empty; both custom RNG states
unused (0 and 0x0013CC25: the first 19400 VBlanks never call `customRandom`); BIOS rand state
0xB738FDB3 (matches the route's value). Money 0: the decomp's default savegame
(`main_menu.c:4937`) says 50000, but that is a different path from the NEW GAME route; the
value in RAM is what the game holds.

## Layout

| Offset | Size | Field | Type | Source |
|---|---|---|---|---|
| 0x0010 | 1 | `mode.main_state` | s8 | MAIN_STATE 0x80134EB4 |
| 0x0011 | 1 | `mode.game_state` | s8 | GAME_STATE 0x80134F0A |
| 0x0012 | 4 | `mode.is_in_menu` | s32 | IS_IN_MENU 0x80134EF0 |
| 0x0016 | 1 | `mode.map_layer_enabled` | u8 | MAP_LAYER_ENABLED 0x80134EC8 |
| 0x0017 | 1 | `mode.pause_state` | u8 | PAUSE_STATE 0x80134E79 |
| 0x0018 | 1 | `mode.tamer_state` | s8 | TAMER_STATE 0x80134C91 |
| 0x0019 | 1 | `mode.tamer_sub_state` | s8 | TAMER_SUB_STATE 0x80134C92 |
| 0x001A | 1 | `mode.partner_state` | s8 | PARTNER_STATE 0x80134C5B |
| 0x001B | 1 | `mode.partner_sub_state` | s8 | PARTNER_SUB_STATE 0x80134C5C |
| 0x001C | 1 | `screen.current_screen` | u8 | CURRENT_SCREEN 0x80134DA8 |
| 0x001D | 1 | `screen.previous_screen` | u8 | PREVIOUS_SCREEN 0x80134DA9 |
| 0x001E | 1 | `screen.current_exit` | u8 | CURRENT_EXIT 0x80134DAA |
| 0x001F | 1 | `screen.previous_exit` | u8 | PREVIOUS_EXIT 0x80134DAB |
| 0x0020 | 2 | `script.current_screen_id` | u16 | CURRENT_SCREEN_ID 0x80134FFE |
| 0x0022 | 2 | `script.current_script_id` | u16 | CURRENT_SCRIPT_ID 0x80134FC0 |
| 0x0024 | 2 | `script.active_map_script` | u16 | ACTIVE_MAP_SCRIPT 0x80134FC2 |
| 0x0026 | 1 | `script.active_buffer` | u8 | ACTIVE_SCRIPT 0x80134FD8, normalised |
| 0x0027 | 1 | `script.cursor_buffer` | u8 | SCRIPT_POINTER 0x80134FDC, normalised |
| 0x0028 | 4 | `script.cursor_offset` | u32 | SCRIPT_POINTER 0x80134FDC, normalised |
| 0x002C | 1 | `script.section` | u8 | SCRIPT_SECTION 0x80134FE4 |
| 0x002D | 4 | `script.section_is_event` | s32 | SCRIPT_SECTION_IS_EVENT 0x80134FE0 |
| 0x0031 | 1 | `script.active_instruction` | u8 | ACTIVE_INSTRUCTION 0x80134FE8 |
| 0x0032 | 4 | `script.is_paused` | s32 | IS_SCRIPT_PAUSED 0x80134FF4 |
| 0x0036 | 2 | `script.param_1` | u16 | SCRIPT_PARAM_1 0x80134FF8 |
| 0x0038 | 2 | `script.state_2` | u16 | SCRIPT_STATE_2 0x80134FFA |
| 0x003A | 2 | `script.delay_frames` | u16 | DELAY_FRAMES 0x80134FFC |
| 0x003C | 2 | `script.stack_depth` | u16 | SCRIPT_STACK_OFFSET 0x80134FC6 |
| 0x003E | 8 | `script.stack_buffer` | u8[8] | ScriptState.stack[i].scriptPtr 0x801BE134+8i, normalised |
| 0x0046 | 32 | `script.stack_offset` | u32[8] | ScriptState.stack[i].scriptPtr 0x801BE134+8i, normalised |
| 0x0066 | 16 | `script.stack_script_id` | u16[8] | ScriptState.stack[i].scriptId 0x801BE138+8i |
| 0x0076 | 8 | `script.stack_smth0` | u8[8] | ScriptState.stack[i].smth[0] 0x801BE13A+8i |
| 0x007E | 8 | `script.stack_smth1` | u8[8] | ScriptState.stack[i].smth[1] 0x801BE13B+8i |
| 0x0086 | 4 | `rng.custom_val1` | u32 | CUSTOM_RNG_VAL1 0x80134E68 |
| 0x008A | 4 | `rng.custom_val2` | u32 | CUSTOM_RNG_VAL2 0x8013442C |
| 0x008E | 4 | `rng.bios_rand_state` | u32 | BIOS rand() state (RAM 0x9010) 0x00009010 |
| 0x0092 | 1 | `clock.year` | u8 | YEAR 0x80134F02 |
| 0x0093 | 2 | `clock.day` | s16 | DAY 0x80134F04 |
| 0x0095 | 2 | `clock.hour` | s16 | HOUR 0x80134EBC |
| 0x0097 | 2 | `clock.minute` | s16 | MINUTE 0x80134EBE |
| 0x0099 | 2 | `clock.current_frame` | u16 | CURRENT_FRAME 0x80134F08 |
| 0x009B | 2 | `clock.last_handled_frame` | u16 | LAST_HANDLED_FRAME 0x80134F06 |
| 0x009D | 2 | `clock.subframe_count` | u16 | SUBFRAME_COUNT 0x80134E1C |
| 0x009F | 1 | `clock.is_gametime_running` | s8 | IS_GAMETIME_RUNNING 0x80134E20 |
| 0x00A0 | 1 | `clock.time_speed` | u8 | ScriptState.pstats[0] (PSTAT_TIME_SPEED) 0x801BE031 |
| 0x00A1 | 2 | `clock.playtime_frames` | u16 | PLAYTIME_FRAMES 0x80134F00 |
| 0x00A3 | 2 | `clock.playtime_minutes` | u16 | PLAYTIME_MINUTES 0x80134EFC |
| 0x00A5 | 2 | `clock.playtime_hours` | u16 | PLAYTIME_HOURS 0x80134EFE |
| 0x00A7 | 4 | `tamer.money` | s32 | MONEY 0x80134EB8 |
| 0x00AB | 1 | `tamer.level` | s8 | TAMER_ENTITY.tamerLevel 0x801557A4 |
| 0x00AC | 1 | `tamer.raised_count` | u8 | TAMER_ENTITY.raisedCount 0x801557A5 |
| 0x00AD | 4 | `tamer.entity_type` | s32 | TAMER_ENTITY.entity.type 0x8015576C |
| 0x00B1 | 1 | `tamer.pos_valid` | u8 | TAMER_ENTITY.entity.posData (non-null, inside RAM) |
| 0x00B2 | 12 | `tamer.pos` | s32[3] | *posData + 0x78 (PositionData.location x,y,z) |
| 0x00BE | 2 | `tamer.yaw` | s16 | *posData + 0x72 (PositionData.rotation.vy) |
| 0x00C0 | 4 | `partner.species` | s32 | PARTNER_ENTITY.entity.type 0x801557A8 |
| 0x00C4 | 1 | `partner.pos_valid` | u8 | PARTNER_ENTITY.entity.posData (non-null, inside RAM) |
| 0x00C5 | 12 | `partner.pos` | s32[3] | *posData + 0x78 (PositionData.location x,y,z) |
| 0x00D1 | 2 | `partner.yaw` | s16 | *posData + 0x72 (PositionData.rotation.vy) |
| 0x00D3 | 2 | `partner.base_off` | s16 | stats.base.off 0x801557E0 |
| 0x00D5 | 2 | `partner.base_def` | s16 | stats.base.def 0x801557E2 |
| 0x00D7 | 2 | `partner.base_speed` | s16 | stats.base.speed 0x801557E4 |
| 0x00D9 | 2 | `partner.base_brain` | s16 | stats.base.brain 0x801557E6 |
| 0x00DB | 4 | `partner.moves_prio` | u8[4] | stats.base.movesPrio 0x801557E8 |
| 0x00DF | 4 | `partner.moves` | u8[4] | stats.base.moves 0x801557EC |
| 0x00E3 | 2 | `partner.max_hp` | s16 | stats.base.hp 0x801557F0 |
| 0x00E5 | 2 | `partner.max_mp` | s16 | stats.base.mp 0x801557F2 |
| 0x00E7 | 2 | `partner.cur_hp` | s16 | stats.current.currentHP 0x801557F4 |
| 0x00E9 | 2 | `partner.cur_mp` | s16 | stats.current.currentMP 0x801557F6 |
| 0x00EB | 8 | `partner.learned_moves` | u32[2] | PARTNER_ENTITY.learnedMoves 0x80155800 |
| 0x00F3 | 20 | `partner.name` | u8[20] | PARTNER_ENTITY.name (game-encoded) 0x80155810 |
| 0x0107 | 1 | `partner.lives` | s8 | PARTNER_ENTITY.lives 0x80155824 |
| 0x0108 | 4 | `partner.para.condition` | u32 | PARTNER_PARA.condition 0x80138460 |
| 0x010C | 2 | `partner.para.sleepy_hour` | s16 | PARTNER_PARA+0x04 0x80138464 |
| 0x010E | 2 | `partner.para.sleepy_minute` | s16 | PARTNER_PARA+0x06 0x80138466 |
| 0x0110 | 2 | `partner.para.wakeup_hour` | s16 | PARTNER_PARA+0x08 0x80138468 |
| 0x0112 | 2 | `partner.para.wakeup_minute` | s16 | PARTNER_PARA+0x0A 0x8013846A |
| 0x0114 | 2 | `partner.para.hours_awake_default` | s16 | PARTNER_PARA+0x0C 0x8013846C |
| 0x0116 | 2 | `partner.para.hours_asleep_default` | s16 | PARTNER_PARA+0x0E 0x8013846E |
| 0x0118 | 2 | `partner.para.time_awake_today` | s16 | PARTNER_PARA+0x10 0x80138470 |
| 0x011A | 2 | `partner.para.sickness_counter` | s16 | PARTNER_PARA+0x12 0x80138472 |
| 0x011C | 2 | `partner.para.missed_sleep_hours` | s16 | PARTNER_PARA+0x14 0x80138474 |
| 0x011E | 2 | `partner.para.tiredness_sleep_timer` | s16 | PARTNER_PARA+0x16 0x80138476 |
| 0x0120 | 2 | `partner.para.poop_level` | s16 | PARTNER_PARA+0x18 0x80138478 |
| 0x0122 | 2 | `partner.para.virus_bar` | s16 | PARTNER_PARA+0x1E 0x8013847E |
| 0x0124 | 2 | `partner.para.pooping_timer` | s16 | PARTNER_PARA+0x20 0x80138480 |
| 0x0126 | 2 | `partner.para.tiredness` | s16 | PARTNER_PARA+0x22 0x80138482 |
| 0x0128 | 2 | `partner.para.sub_tiredness` | s16 | PARTNER_PARA+0x24 0x80138484 |
| 0x012A | 2 | `partner.para.tiredness_hunger_timer` | s16 | PARTNER_PARA+0x26 0x80138486 |
| 0x012C | 2 | `partner.para.discipline` | s16 | PARTNER_PARA+0x28 0x80138488 |
| 0x012E | 2 | `partner.para.happiness` | s16 | PARTNER_PARA+0x2A 0x8013848A |
| 0x0130 | 2 | `partner.para.times_being_sick` | s16 | PARTNER_PARA+0x30 0x80138490 |
| 0x0132 | 2 | `partner.para.sickness_tries` | s16 | PARTNER_PARA+0x32 0x80138492 |
| 0x0134 | 2 | `partner.para.area_effect_timer` | s16 | PARTNER_PARA+0x34 0x80138494 |
| 0x0136 | 2 | `partner.para.sickness_timer` | s16 | PARTNER_PARA+0x36 0x80138496 |
| 0x0138 | 2 | `partner.para.injury_timer` | s16 | PARTNER_PARA+0x38 0x80138498 |
| 0x013A | 2 | `partner.para.next_hunger_hour` | s16 | PARTNER_PARA+0x3A 0x8013849A |
| 0x013C | 2 | `partner.para.energy_level` | s16 | PARTNER_PARA+0x3C 0x8013849C |
| 0x013E | 2 | `partner.para.food_level` | s16 | PARTNER_PARA+0x3E 0x8013849E |
| 0x0140 | 2 | `partner.para.starvation_timer` | s16 | PARTNER_PARA+0x40 0x801384A0 |
| 0x0142 | 2 | `partner.para.weight` | s16 | PARTNER_PARA+0x42 0x801384A2 |
| 0x0144 | 2 | `partner.para.refused_fav_food` | s16 | PARTNER_PARA+0x44 0x801384A4 |
| 0x0146 | 2 | `partner.para.empty_stomach_timer` | s16 | PARTNER_PARA+0x46 0x801384A6 |
| 0x0148 | 2 | `partner.para.remaining_lifetime` | s16 | PARTNER_PARA+0x48 0x801384A8 |
| 0x014A | 2 | `partner.para.age` | s16 | PARTNER_PARA+0x4A 0x801384AA |
| 0x014C | 2 | `partner.para.train_boost_flag` | s16 | PARTNER_PARA+0x4C 0x801384AC |
| 0x014E | 2 | `partner.para.train_boost_value` | s16 | PARTNER_PARA+0x4E 0x801384AE |
| 0x0150 | 2 | `partner.para.train_boost_timer` | s16 | PARTNER_PARA+0x50 0x801384B0 |
| 0x0152 | 2 | `partner.para.care_mistakes` | s16 | PARTNER_PARA+0x52 0x801384B2 |
| 0x0154 | 2 | `partner.para.battles` | s16 | PARTNER_PARA+0x54 0x801384B4 |
| 0x0156 | 2 | `partner.para.evo_timer` | s16 | PARTNER_PARA+0x56 0x801384B6 |
| 0x0158 | 2 | `partner.para.fish_caught` | s16 | PARTNER_PARA+0x5E 0x801384BE |
| 0x015A | 2 | `partner.para.upgrade_hp_counter` | s16 | PARTNER_PARA+0x66 0x801384C6 |
| 0x015C | 2 | `partner.para.upgrade_mp_counter` | s16 | PARTNER_PARA+0x68 0x801384C8 |
| 0x015E | 2 | `partner.para.upgrade_off_counter` | s16 | PARTNER_PARA+0x6A 0x801384CA |
| 0x0160 | 2 | `partner.para.bugged_upgrade_brains_counter` | s16 | PARTNER_PARA+0x6C 0x801384CC |
| 0x0162 | 2 | `partner.para.upgrade_defense_counter` | s16 | PARTNER_PARA+0x6E 0x801384CE |
| 0x0164 | 2 | `partner.para.upgrade_speed_counter` | s16 | PARTNER_PARA+0x70 0x801384D0 |
| 0x0166 | 2 | `partner.para.upgrade_brain_counter` | s16 | PARTNER_PARA+0x72 0x801384D2 |
| 0x0168 | 2 | `partner.para.suka_backup_hp` | s16 | PARTNER_PARA+0x76 0x801384D6 |
| 0x016A | 2 | `partner.para.suka_backup_mp` | s16 | PARTNER_PARA+0x78 0x801384D8 |
| 0x016C | 2 | `partner.para.suka_backup_off` | s16 | PARTNER_PARA+0x7A 0x801384DA |
| 0x016E | 2 | `partner.para.suka_backup_def` | s16 | PARTNER_PARA+0x7C 0x801384DC |
| 0x0170 | 2 | `partner.para.suka_backup_speed` | s16 | PARTNER_PARA+0x7E 0x801384DE |
| 0x0172 | 2 | `partner.para.suka_backup_brain` | s16 | PARTNER_PARA+0x80 0x801384E0 |
| 0x0174 | 1 | `inventory.size` | u8 | INVENTORY.size 0x8013D4CE |
| 0x0175 | 30 | `inventory.item_ids` | u8[30] | INVENTORY.types (0xFF empty) 0x8013D474 |
| 0x0193 | 30 | `inventory.counts` | u8[30] | INVENTORY.amounts 0x8013D492 |
| 0x01B1 | 33 | `progress.cards` | u8[33] | ScriptState.cards 0x801BDFAC |
| 0x01D2 | 100 | `progress.triggers` | u8[100] | ScriptState.triggers (800 bits) 0x801BDFCD |
| 0x0236 | 256 | `progress.pstats` | u8[256] | ScriptState.pstats 0x801BE031 |
| 0x0336 | 2 | `progress.merit` | s16 | MERIT 0x80134FC4 |
| 0x0338 | 2 | `progress.battles_started` | s16 | BATTLES_STARTED 0x80134FC8 |
| 0x033A | 2 | `progress.battles_fled` | s16 | BATTLES_FLED 0x80134FCA |
| 0x033C | 2 | `progress.tournaments_won` | s16 | TOURNAMENTS_WON 0x80134FCC |
| 0x033E | 2 | `progress.tournament_wins` | s16 | TOURNAMENT_WINS 0x80134FCE |
| 0x0340 | 2 | `progress.tournaments_lost` | s16 | TOURNAMENTS_LOST 0x80134FD0 |

Total: 834 bytes (16-byte header + 818 body).
