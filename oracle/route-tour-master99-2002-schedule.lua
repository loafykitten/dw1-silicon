-- Extra schedule for DW1ROUTE_PROFILE=tour-master99-2002 (Phase 9, stream tour, derived with
-- oracle/explore.lua): after the continue-save3 route, with Master99's 2002 GameFAQs save (fixture
-- p9-arena-master99: MetalGreymon, day 11, hungry) in the late-game Jijimon's house (205): out to
-- the top of File City (179), down to the plaza (192), into the tool shop (216). The exploration
-- bought cards at the Digimon card vending machine and walked back out; in the recording
-- (oracle/replays/tour-master99-2002.txt) the walk to the machine slides onto the step to the
-- Secret Item Shop (217), and the rest goes back and forth between 216 and 217; it ends at 9700.
local PRESSES = {
    { 4232, 4232, 1, 45, 'RIGHT' },
    { 4277, 4277, 1, 30, 'DOWN' },
    { 4307, 4307, 1, 45, 'RIGHT' },
    { 4352, 4352, 1, 30, 'DOWN' },
    { 4352, 4352, 1, 30, 'RIGHT' },
    { 4782, 4782, 1, 200, 'DOWN' },
    { 5337, 5337, 1, 120, 'DOWN' },
    { 5457, 5457, 1, 100, 'LEFT' },
    { 5957, 5957, 1, 15, 'DOWN' },
    { 5972, 5972, 1, 70, 'LEFT' },
    { 6042, 6042, 1, 20, 'UP' },
    { 6462, 6462, 1, 10, 'UP' },
    { 6472, 6472, 1, 75, 'UP' },
    { 6547, 6547, 1, 15, 'LEFT' },
    { 6562, 6562, 1, 3, 'UP' },
    { 6565, 6565, 1, 4, 'CROSS' },
    { 6655, 6655, 1, 3, 'UP' },
    { 6658, 6658, 1, 4, 'CROSS' },
    { 6748, 6748, 1, 4, 'CROSS' },
    { 6838, 6838, 1, 4, 'CROSS' },
    { 6958, 6958, 1, 4, 'CROSS' },
    { 7078, 7078, 1, 4, 'CROSS' },
    { 7198, 7198, 1, 4, 'CROSS' },
    { 7318, 7318, 1, 4, 'CROSS' },
    { 7438, 7438, 1, 4, 'CROSS' },
    { 7558, 7558, 1, 4, 'DOWN' },
    { 7588, 7588, 1, 4, 'CROSS' },
    { 7678, 7678, 1, 4, 'CROSS' },
    { 7768, 7768, 1, 4, 'CROSS' },
    { 7858, 7858, 1, 4, 'CROSS' },
    { 7948, 7948, 1, 4, 'CROSS' },
    { 8068, 8068, 1, 4, 'DOWN' },
    { 8098, 8098, 1, 4, 'CROSS' },
    { 8248, 8248, 1, 78, 'RIGHT' },
    { 8326, 8326, 1, 130, 'DOWN' },
    { 8856, 8856, 1, 38, 'LEFT' },
    { 8894, 8894, 1, 60, 'DOWN' },
}
local MILESTONES = {
    { 4232, 'house_205' },
    { 4900, 'city_179' },
    { 5600, 'plaza_192' },
    { 6462, 'tool_shop_216' },
    { 6748, 'late_card_machine' },
    { 7558, 'late_try_again' },
    { 7948, 'late_vegiemon_card' },
    { 8248, 'late_closed' },
    { 9396, 'late_plaza_again' },
    { 9700, 'master99_2002_done' },
}
return PRESSES, MILESTONES
