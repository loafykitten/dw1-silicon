-- DW1ROUTE_PROFILE=rest-ending236 (Phase 9, stream rest): the other ending map, 236 (TEND01).
-- Script 184 (screen 225, section 0x33) ends Machinedramon's scene with
-- `IF trig220 | trig214 | pstat1 >= 50 THEN WARP 237 ELSE WARP 236`, after adding 7 to the
-- prosperity (pstat 1); MAPHEAD gives both maps script 192 and the same Digimon. The fixture
-- captures/fixtures/p9-rest-ending236.mcd is Master99's 2001 save placed like the ending's
-- (tools/oracle/place_fixture.py ... 222 254 0 -400 -4100 326=1 214=0 p1=40: flag 214 cleared and
-- the prosperity 71 -> 40, so 47 after the battle), and this is the ending route's schedule
-- (route-ending-schedule.lua) up to the credits.
local PRESSES = {
    { 4000, 4000, 1, 540, 'UP' },        -- the corridor (222) to the boss room's warp
    { 4940, 4940, 1, 140, 'UP' },        -- into Analogman's trigger (225)
    { 5180, 8880, 50, 4, 'CROSS' },      -- the scene's lines, the buildup and spawn, to the battle
    { 13510, 13510, 1, 4, 'SQUARE' },    -- the finisher ("FIN")
    { 13740, 13740, 1, 4, 'SQUARE' },
    { 14170, 19620, 50, 4, 'CROSS' },    -- results, the scene, the warp, the clear save (Yes)
    { 19730, 20430, 50, 4, 'CROSS' },    -- the last lines, the ending
}
local MILESTONES = {
    { 4940, 'boss_room' },
    { 5930, 'analogman' },
    { 7430, 'machinedramon_spawn' },
    { 9530, 'battle' },
    { 14170, 'battle_won' },
    { 18420, 'ending_map' },
    { 19420, 'clear_save' },
    { 20230, 'ending' },
    { 20650, 'credits' },
    { 21200, 'credits_running' },
}
return PRESSES, MILESTONES
