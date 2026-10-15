-- Extra schedule for DW1ROUTE_PROFILE=tour-canyon (Phase 9, stream tour, derived with
-- oracle/explore.lua): after route-debug-room-schedule.lua: Mr. Warp, Far away, Great Canyon
-- (screen 44, the camera turned: Up walks west), along the cliff to the canyon house's door,
-- Gotsumon ("I'll beat you up!"): the battle. The exploration lost it and went home; the
-- recording (oracle/replays/tour-canyon.txt) wins it with the Square finisher and ends on the
-- results (the explorer's pauses change CD timing: docs/phase5-plan.md).
local PRESSES = {
    { 6954, 6954, 1, 70, 'UP' },
    { 7024, 7024, 1, 40, 'LEFT' },
    { 7064, 7064, 1, 4, 'CROSS' },
    { 7154, 7154, 1, 4, 'CROSS' },
    { 7244, 7244, 1, 4, 'CROSS' },
    { 7334, 7334, 1, 4, 'CROSS' },
    { 7424, 7424, 1, 4, 'DOWN' },
    { 7454, 7454, 1, 4, 'CROSS' },
    { 7544, 7544, 1, 4, 'DOWN' },
    { 7574, 7574, 1, 4, 'DOWN' },
    { 7604, 7604, 1, 4, 'CROSS' },
    { 8134, 8134, 1, 10, 'LEFT' },
    { 8144, 8144, 1, 10, 'UP' },
    { 8154, 8154, 1, 85, 'UP' },
    { 8239, 8239, 1, 45, 'RIGHT' },
    { 8284, 8284, 1, 40, 'UP' },
    { 8324, 8324, 1, 4, 'CROSS' },
    { 8414, 8414, 1, 30, 'LEFT' },
    { 8444, 8444, 1, 4, 'CROSS' },
    { 8534, 8534, 1, 45, 'UP' },
    { 8579, 8579, 1, 4, 'CROSS' },
    { 8699, 8699, 1, 4, 'CROSS' },
    { 8789, 8789, 1, 4, 'CROSS' },
    { 12629, 12629, 1, 4, 'SQUARE' },
    { 15659, 15659, 1, 4, 'CROSS' },
    { 15749, 15749, 1, 4, 'CROSS' },
    { 15839, 15839, 1, 4, 'CROSS' },
    { 15929, 15929, 1, 4, 'CROSS' },
    { 16019, 16019, 1, 4, 'CROSS' },
    { 16109, 16109, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 7604, 'warp_menu_far' },
    { 8134, 'canyon_44' },
    { 8699, 'gotsumon' },
    { 10629, 'battle' },
    { 14459, 'battle_late' },
    { 16200, 'canyon_done' },
}
return PRESSES, MILESTONES
