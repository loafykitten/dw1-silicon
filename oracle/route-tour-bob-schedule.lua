-- Extra schedule for DW1ROUTE_PROFILE=tour-bob (Phase 9, stream tour, derived with
-- oracle/explore.lua): after the continue route, with Bob's GameFAQs save (fixture
-- p9-arena-vs-p2: MegaSeadramon, day 11, 20:02) in the late-game Jijimon's house (205): the back
-- room (206: Monzaemon, "It's empty."), back into the house. The exploration went on out at night
-- through File City (179) and the Green Gym (112) into the Native Forest (109) and a battle with
-- Dokunemon; in the recording (oracle/replays/tour-bob.txt) the walk back from 206 takes longer
-- and the later presses land in the house, where it ends at 11400.
local PRESSES = {
    { 5002, 5002, 1, 80, 'UP' },
    { 5002, 5002, 1, 80, 'LEFT' },
    { 5482, 5482, 1, 10, 'UP' },
    { 5492, 5492, 1, 6, 'RIGHT' },
    { 5498, 5498, 1, 8, 'RIGHT' },
    { 5506, 5506, 1, 50, 'UP' },
    { 5556, 5556, 1, 4, 'CROSS' },
    { 5646, 5646, 1, 4, 'CROSS' },
    { 5706, 5706, 1, 100, 'DOWN' },
    { 6206, 6206, 1, 50, 'RIGHT' },
    { 6256, 6256, 1, 30, 'DOWN' },
    { 6286, 6286, 1, 45, 'RIGHT' },
    { 6331, 6331, 1, 30, 'DOWN' },
    { 6331, 6331, 1, 30, 'RIGHT' },
    { 6761, 6761, 1, 60, 'DOWN' },
    { 6821, 6821, 1, 190, 'LEFT' },
    { 7411, 7411, 1, 80, 'LEFT' },
    { 7491, 7491, 1, 90, 'DOWN' },
    { 7981, 7981, 1, 30, 'LEFT' },
    { 8011, 8011, 1, 50, 'DOWN' },
    { 8461, 8461, 1, 45, 'DOWN' },
    { 8506, 8506, 1, 10, 'RIGHT' },
    { 8516, 8516, 1, 4, 'CROSS' },
    { 8606, 8606, 1, 4, 'CROSS' },
    { 8696, 8696, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 5002, 'house_205' },
    { 5482, 'back_room_206' },
    { 5646, 'box' },
    { 6416, 'house_again' },
    { 6896, 'late_city_179' },
    { 7411, 'late_gym_112' },
    { 8461, 'late_forest_109' },
    { 8606, 'late_dokunemon' },
    { 9326, 'late_battle' },
    { 10826, 'late_battle_menu' },
    { 11400, 'bob_done' },
}
return PRESSES, MILESTONES
