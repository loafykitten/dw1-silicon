-- DW1ROUTE_PROFILE=debug-gym (Phase 9, stream fishcurl): TRN2, the gyms outside File City. After
-- route-master99-yard-schedule.lua and route-master99-warp-schedule.lua, on the fixture
-- p9-fishcurl-debug-master99-flute (tools/oracle/debug_fixture.py --keep 45 --keep 46 --item 115:
-- Master99's save with a Blue Flute added; the certified card was made before --item gave the
-- slot a name-texture index as giveItem does, so its flute's index is 0xFF where today's tool
-- writes 25; the replay is certified on the old card, so keep it). Mr. Warp to Dragon Eye Lake
-- (08:3x, so the fishing Seadramon swims there too), the Blue Flute at the shore, the Lake Guardian's ride to Beetle Land,
-- and a Normal session at the stump gym's Striking sign (BETL03: TRN2 loads on maps 99, 107, 108
-- and 165, map.c loadTrainingLibrary). Derived with oracle/explore.lua.
local PRESSES = {
    { 5740, 5740, 1, 4, 'DOWN' },     -- Mr. Warp, Unknown World: Seadra fishing
    { 5770, 5770, 1, 4, 'CROSS' },    -- Dragon Eye Lake (screen 6)
    -- along the shore onto its Cross tiles (81, map script 7 section 81)
    { 6501, 6501, 1, 50, 'LEFT' },
    { 6555, 6555, 1, 4, 'CROSS' },
    { 6645, 6645, 1, 30, 'UP' },
    { 6679, 6679, 1, 20, 'LEFT' },
    -- "Oh, yes! ... the Lake Guardian's Blue Flute?" I'll play: Seadramon rises, "Do you need help
    -- my friend?", and carries us to Beetle Land (screen 105)
    { 6703, 6703, 1, 4, 'CROSS' },
    { 6793, 6793, 1, 4, 'CROSS' },
    { 6883, 6883, 1, 4, 'CROSS' },
    { 6973, 6973, 1, 4, 'CROSS' },
    { 7882, 7882, 1, 4, 'CROSS' },
    { 7972, 7972, 1, 4, 'CROSS' },
    { 8062, 8062, 1, 4, 'CROSS' },
    { 8152, 8152, 1, 4, 'CROSS' },
    { 8242, 8242, 1, 4, 'CROSS' },
    { 8332, 8332, 1, 4, 'CROSS' },
    { 8506, 8506, 1, 4, 'CROSS' },
    -- north through Beetle Land's entrance; a Tentomon ("Reporting, reporting!") starts a battle, won
    -- on its own (BTL), results closed
    { 8866, 8866, 1, 60, 'UP' },
    { 8928, 8928, 1, 80, 'UP' },
    { 9010, 9010, 1, 80, 'UP' },
    { 9092, 9092, 1, 80, 'UP' },
    { 9174, 9174, 1, 80, 'UP' },
    { 9256, 9256, 1, 80, 'UP' },
    { 9338, 9338, 1, 80, 'UP' },
    { 9421, 9421, 1, 4, 'CROSS' },
    { 9926, 9926, 1, 4, 'CROSS' },
    { 10016, 10016, 1, 4, 'CROSS' },
    { 10106, 10106, 1, 4, 'CROSS' },
    { 10196, 10196, 1, 4, 'CROSS' },
    { 10286, 10286, 1, 4, 'CROSS' },
    { 10376, 10376, 1, 4, 'CROSS' },
    { 10466, 10466, 1, 4, 'CROSS' },
    { 10556, 10556, 1, 4, 'CROSS' },
    { 10646, 10646, 1, 4, 'CROSS' },
    { 13256, 13256, 1, 4, 'CROSS' },
    { 13376, 13376, 1, 4, 'CROSS' },
    { 13496, 13496, 1, 4, 'CROSS' },
    { 13616, 13616, 1, 4, 'CROSS' },
    { 13800, 13800, 1, 4, 'CROSS' },
    -- on north into screen 106, then east into the tree stump gym (screen 107, TRN2 loaded with the map)
    { 13920, 13920, 1, 200, 'UP' },
    { 14122, 14122, 1, 200, 'UP' },
    { 14324, 14324, 1, 100, 'UP' },
    { 14826, 14826, 1, 105, 'UP' },
    { 14933, 14933, 1, 125, 'RIGHT' },
    { 15503, 15503, 1, 40, 'UP' },
    -- onto the Striking sign (Cross tile 83): Sign "Training: Striking", Begin, Normal; the session
    -- (TRN2_tickOffenseTraining) runs its 1200 frames and the gains show (MetalGreymon's are capped)
    { 15545, 15545, 1, 120, 'UP' },
    { 15667, 15667, 1, 4, 'CROSS' },
    { 15757, 15757, 1, 4, 'CROSS' },
    { 15847, 15847, 1, 4, 'CROSS' },
    { 15937, 15937, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 6500, 'lake' },
    { 6793, 'flute_offer' },
    { 7882, 'lake_guardian' },
    { 8866, 'beetle_land' },
    { 9421, 'tentomon' },
    { 13256, 'battle_results' },
    { 15503, 'stump_gym' },
    { 15757, 'sign' },
    { 17000, 'training' },
    { 19967, 'gains' },
    { 20600, 'gym_done' },
}
return PRESSES, MILESTONES
