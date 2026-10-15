-- Extra schedule for DW1ROUTE_PROFILE=tour-bob (Phase 9, stream tour, derived with
-- oracle/explore.lua): after the continue route, with Bob's GameFAQs save (fixture p9-arena-vs-p2:
-- MegaSeadramon, day 11, 20:02) in the late-game Jijimon's house (205): the back room (206,
-- Monzaemon, "It's empty."), back through the house and out at night to the top of File City
-- (179), west to the Green Gym (112), south into the Native Forest (109), Dokunemon ("What do you
-- want?!"): a battle with the late-game partner (its command menu up), over by ~11120.
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
    { 6896, 'city_179' },
    { 7411, 'gym_112' },
    { 8461, 'forest_109' },
    { 8606, 'dokunemon' },
    { 9326, 'battle' },
    { 10826, 'battle_menu' },
    { 11400, 'bob_done' },
}
return PRESSES, MILESTONES
