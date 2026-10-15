-- DW1ROUTE_PROFILE=debug-curl (Phase 9, stream fishcurl): after route-master99-yard-schedule.lua
-- and route-master99-warp-schedule.lua, on the fixture p9-fishcurl-debug-master99-penguin
-- (tools/oracle/debug_fixture.py --keep 45 --keep 46 --clear 257 --clear 357 on Master99's 2002
-- save: Penguinmon not recruited and his free match not used, so he waits outside the Curling
-- Cave, map script 206 section 6). Mr. Warp to the Curling Cave, Penguinmon's offer ("Digimon
-- Curling! ... Nothing the first time"), "Let's go inside", the rink (screen 136, the KAR
-- overlay), the match, which Penguinmon wins, and back outside. Derived with oracle/explore.lua.
local PRESSES = {
    { 5740, 5740, 1, 4, 'CROSS' },    -- Mr. Warp: In front of Curling Cave (screen 134)
    -- to Penguinmon, by the cave mouth (x 1792, z 1185), and through his offer
    { 6801, 6801, 1, 15, 'RIGHT' },
    { 6818, 6818, 1, 100, 'UP' },
    { 6920, 6920, 1, 12, 'UP' },
    { 6936, 6936, 1, 4, 'CROSS' },
    { 7026, 7026, 1, 4, 'CROSS' },
    { 7116, 7116, 1, 4, 'CROSS' },
    { 7206, 7206, 1, 4, 'CROSS' },
    { 7296, 7296, 1, 4, 'CROSS' },
    { 7386, 7386, 1, 4, 'CROSS' },
    { 7476, 7476, 1, 4, 'CROSS' },
    { 7566, 7566, 1, 4, 'CROSS' },
    { 7656, 7656, 1, 4, 'CROSS' },
    { 7746, 7746, 1, 4, 'CROSS' },
    -- the match (oracle/explore.lua autocurl): Start Game, then for each of our five stones Cross
    -- (stone), Cross (straight aim) and Cross at power 1900; Cross through his lines; his throws
    -- are the game's (KAR_chooseOpponentShot). Penguinmon wins.
    { 8238, 8238, 1, 4, 'CROSS' },
    { 8578, 8578, 1, 4, 'CROSS' },
    { 8608, 8608, 1, 4, 'CROSS' },
    { 8680, 8680, 1, 4, 'CROSS' },
    { 9235, 9235, 1, 4, 'CROSS' },
    { 9304, 9304, 1, 4, 'CROSS' },
    { 9334, 9334, 1, 4, 'CROSS' },
    { 10291, 10291, 1, 4, 'CROSS' },
    { 10357, 10357, 1, 4, 'CROSS' },
    { 10387, 10387, 1, 4, 'CROSS' },
    { 10459, 10459, 1, 4, 'CROSS' },
    { 11017, 11017, 1, 4, 'CROSS' },
    { 11086, 11086, 1, 4, 'CROSS' },
    { 11116, 11116, 1, 4, 'CROSS' },
    { 11812, 11812, 1, 4, 'CROSS' },
    { 11878, 11878, 1, 4, 'CROSS' },
    { 11908, 11908, 1, 4, 'CROSS' },
    { 11980, 11980, 1, 4, 'CROSS' },
    { 12697, 12697, 1, 4, 'CROSS' },
    { 12766, 12766, 1, 4, 'CROSS' },
    { 12796, 12796, 1, 4, 'CROSS' },
    { 13489, 13489, 1, 4, 'CROSS' },
    { 13558, 13558, 1, 4, 'CROSS' },
    { 13588, 13588, 1, 4, 'CROSS' },
    { 14458, 14458, 1, 4, 'CROSS' },
    { 14524, 14524, 1, 4, 'CROSS' },
    { 14554, 14554, 1, 4, 'CROSS' },
    { 14626, 14626, 1, 4, 'CROSS' },
    { 15256, 15256, 1, 4, 'CROSS' },
    { 15325, 15325, 1, 4, 'CROSS' },
    { 15355, 15355, 1, 4, 'CROSS' },
    { 16225, 16225, 1, 4, 'CROSS' },
    { 16291, 16291, 1, 4, 'CROSS' },
    { 16321, 16321, 1, 4, 'CROSS' },
    { 16393, 16393, 1, 4, 'CROSS' },
    { 16801, 16801, 1, 4, 'CROSS' },
    { 16867, 16867, 1, 4, 'CROSS' },
    { 16897, 16897, 1, 4, 'CROSS' },
    { 16927, 16927, 1, 4, 'CROSS' },
    { 16957, 16957, 1, 4, 'CROSS' },
    { 16987, 16987, 1, 4, 'CROSS' },
    -- back outside: "Sorry bro. Come back again to challenge me..." closed
    { 20297, 20297, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 6800, 'cave_front' },
    { 6936, 'penguinmon' },
    { 8236, 'rink' },
    { 9000, 'first_throw' },
    { 13000, 'mid_match' },
    { 17200, 'match_over' },
    { 20297, 'penguinmon_won' },
    { 20450, 'curl_done' },
}
return PRESSES, MILESTONES
