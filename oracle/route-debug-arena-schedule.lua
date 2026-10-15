-- DW1ROUTE_PROFILE=debug-arena (Phase 9, stream fishcurl): after route-master99-yard-schedule.lua,
-- from the debug yard down into File City's plaza (screen 191 with Master99's buildings).
-- Derived with oracle/explore.lua; the first presses walk into the shed and the time Jijimon.
local PRESSES = {
    { 4812, 4812, 1, 60, 'LEFT' },
    { 4874, 4874, 1, 200, 'DOWN' },
    { 5374, 5374, 1, 10, 'RIGHT' },
    { 5386, 5386, 1, 35, 'DOWN' },
    { 5423, 5423, 1, 45, 'LEFT' },
    { 5470, 5470, 1, 160, 'DOWN' },
    { 5930, 5930, 1, 55, 'RIGHT' },
    { 5987, 5987, 1, 135, 'DOWN' },
    { 6124, 6124, 1, 75, 'LEFT' },
    { 6201, 6201, 1, 60, 'DOWN' },
}
local MILESTONES = {
    { 6900, 'plaza' },
}
return PRESSES, MILESTONES
