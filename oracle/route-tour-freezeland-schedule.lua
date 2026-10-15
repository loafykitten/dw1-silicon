-- Extra schedule for DW1ROUTE_PROFILE=tour-freezeland (Phase 9, stream tour, derived with
-- oracle/explore.lua): after route-debug-room-schedule.lua: Mr. Warp, Very far away, Unknown
-- World, in front of the Curling Cave (Freezeland, screen 134; Penguinmon left alone: talking to
-- him starts a curling match), west into the Freezeland hub (screen 132). The exploration met
-- Hyogamon there and lost; in the recording (oracle/replays/tour-freezeland.txt) the tamer misses
-- him and the walk south takes it back into screen 134's lower part, by a Mojyamon (the explorer's
-- pauses change timing: docs/phase5-plan.md). No battle; ends at 14000.
local PRESSES = {
    { 6954, 6954, 1, 70, 'UP' },
    { 7024, 7024, 1, 40, 'LEFT' },
    { 7064, 7064, 1, 4, 'CROSS' },
    { 7154, 7154, 1, 4, 'CROSS' },
    { 7244, 7244, 1, 4, 'CROSS' },
    { 7334, 7334, 1, 4, 'CROSS' },
    { 7424, 7424, 1, 4, 'DOWN' },
    { 7454, 7454, 1, 4, 'DOWN' },
    { 7484, 7484, 1, 4, 'CROSS' },
    { 7574, 7574, 1, 4, 'DOWN' },
    { 7604, 7604, 1, 4, 'DOWN' },
    { 7634, 7634, 1, 4, 'CROSS' },
    { 7724, 7724, 1, 4, 'CROSS' },
    { 8254, 8254, 1, 60, 'DOWN' },
    { 8714, 8714, 1, 10, 'LEFT' },
    { 8724, 8724, 1, 5, 'UP' },
    { 8729, 8729, 1, 40, 'LEFT' },
    { 8769, 8769, 1, 200, 'DOWN' },
    { 8969, 8969, 1, 160, 'LEFT' },
    { 9129, 9129, 1, 4, 'CROSS' },
    { 9219, 9219, 1, 4, 'CROSS' },
    { 13049, 13049, 1, 4, 'CROSS' },
    { 13139, 13139, 1, 4, 'CROSS' },
    { 13229, 13229, 1, 4, 'CROSS' },
    { 13319, 13319, 1, 4, 'CROSS' },
    { 13409, 13409, 1, 4, 'CROSS' },
    { 13499, 13499, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 7724, 'warp_menu_unknown' },
    { 8254, 'freezeland_134' },
    { 8714, 'freezeland_132' },
    { 9129, 'late_1' },
    { 10749, 'late_2' },
    { 12749, 'late_3' },
    { 14000, 'freezeland_done' },
}
return PRESSES, MILESTONES
