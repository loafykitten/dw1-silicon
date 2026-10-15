-- Extra schedule for DW1ROUTE_PROFILE=tour-master99-2001 (Phase 9, stream tour, derived with
-- oracle/explore.lua): after the continue-save3 route, with Master99's 2001 GameFAQs save (fixture
-- p9-tour-master99-2001: Digitamamon, 941599 bits) in the late-game Jijimon's house (205): out
-- to the top of File City (179), the plaza (192), west into the Native Forest (109), a forest
-- Digimon walks into Digitamamon: a battle, won. The exploration's battle ended at ~9000 and it
-- went on through the crossroads (111) to screen 0 and a ModokiBetamon; in the recording
-- (oracle/replays/tour-master99-2001.txt) the battle runs to ~11740 and the walk after it stays in
-- the forest, where it ends at 15000.
local PRESSES = {
    { 4232, 4232, 1, 45, 'RIGHT' },
    { 4277, 4277, 1, 30, 'DOWN' },
    { 4307, 4307, 1, 45, 'RIGHT' },
    { 4352, 4352, 1, 30, 'DOWN' },
    { 4352, 4352, 1, 30, 'RIGHT' },
    { 5084, 5084, 1, 200, 'DOWN' },
    { 5641, 5641, 1, 148, 'DOWN' },
    { 5789, 5789, 1, 210, 'LEFT' },
    { 6413, 6413, 1, 60, 'UP' },
    { 6473, 6473, 1, 65, 'LEFT' },
    { 6538, 6538, 1, 4, 'CROSS' },
    { 8995, 8995, 1, 90, 'DOWN' },
    { 8995, 8995, 1, 90, 'LEFT' },
    { 9085, 9085, 1, 80, 'LEFT' },
    { 9165, 9165, 1, 150, 'DOWN' },
    { 9315, 9315, 1, 100, 'LEFT' },
    { 9815, 9815, 1, 80, 'LEFT' },
    { 9895, 9895, 1, 120, 'DOWN' },
    { 10015, 10015, 1, 35, 'LEFT' },
    { 10050, 10050, 1, 170, 'DOWN' },
    { 10620, 10620, 1, 25, 'LEFT' },
    { 10645, 10645, 1, 60, 'DOWN' },
    { 11105, 11105, 1, 140, 'DOWN' },
    { 11245, 11245, 1, 25, 'LEFT' },
    { 11270, 11270, 1, 4, 'CROSS' },
    { 11360, 11360, 1, 15, 'DOWN' },
    { 11375, 11375, 1, 4, 'CROSS' },
    { 11465, 11465, 1, 12, 'RIGHT' },
    { 11477, 11477, 1, 4, 'CROSS' },
    { 11567, 11567, 1, 4, 'CROSS' },
    { 12020, 12020, 1, 4, 'CROSS' },
    { 12110, 12110, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 4232, 'house_205' },
    { 4700, 'city_179' },
    { 5400, 'plaza_192' },
    { 6413, 'forest_109' },
    { 7047, 'battle' },
    { 8995, 'after_battle' },
    { 9815, 'late_crossroads_111' },
    { 11105, 'late_screen_0' },
    { 11567, 'late_modokibetamon' },
    { 12800, 'late_battle2' },
    { 15000, 'master99_2001_done' },
}
return PRESSES, MILESTONES
