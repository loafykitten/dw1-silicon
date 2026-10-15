-- Extra schedule for DW1ROUTE_PROFILE=cup-lost (Phase 9, stream cup; derived with
-- oracle/explore.lua), after route-arena-start-schedule.lua and route-cup-lobby-schedule.lua: the
-- same cup lost in the first round. A controller (below) waits for the match (GAME_STATE 4, at
-- 0x80134F0A) and 200 VBlanks into it moves the command cursor once to the left, from Auto to
-- "Defensive" (STD_handleCommands: the pad's left raises hoveredCommand, wrapping to 1; MetalGreymon's
-- brains give the seven-command list, whose entry 1 is command 6), and confirms with Cross. The
-- partner only guards; the opponent's finisher wears it down, the timer runs out with our HP lower
-- and the match is lost: the lose scene, the cup over, STD_deinitializeTournament's fade. Like
-- `cup`, the target is just before the first gameLoop frame after the cup (the GPU list loop that
-- hangs the pinned Redux).
local PRESSES = {}
local MILESTONES = {
    { 7500, 'round1_defensive' },
    { 9000, 'round1_fight' },
    { 11400, 'round1_late' },
    { 13000, 'round1_timeout' },        -- the timer runs out at about 13330 (GAME_STATE 5)
    { 13500, 'lose_scene' },
    { 13775, 'cup_end' },               -- target: STD_deinitializeTournament's fade, before gameLoop
}
local LEFT, CROSS = 7, 14
local start, step = nil, 0
local function controller(vblank, pad, ram)
    if step >= 4 then return end
    local u8 = ffi.cast('uint8_t*', ram)
    if not start then
        if vblank > 5000 and u8[0x134F0A] == 4 then start = vblank end
        return
    end
    local t = vblank - start
    if step == 0 and t >= 200 then pad.setOverride(LEFT); step = 1
    elseif step == 1 and t >= 204 then pad.clearOverride(LEFT); step = 2
    elseif step == 2 and t >= 240 then pad.setOverride(CROSS); step = 3
    elseif step == 3 and t >= 244 then pad.clearOverride(CROSS); step = 4
    end
end
return PRESSES, MILESTONES, controller
