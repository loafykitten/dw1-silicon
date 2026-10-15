-- DW1ROUTE_PROFILE=rest-agumon (Phase 9, stream rest): Agumon's recruit in the Native Forest
-- (screen 109). The fixture captures/fixtures/p9-rest-agumon.mcd is BeelzemontZero's save
-- (p9-arena-vs-p1: Piximon, the card's only save: the continue base) moved to where the warp from
-- the Green Gym (screen 112) lands (tools/oracle/place_fixture.py ... 109 112 0 264 3585 203=0:
-- flag 203, Agumon recruited = 200 + his species 3, cleared). Entering the map with 203 clear
-- starts script 105's Agumon scene at once ("Wow!"): his lines, a predefined battle (pstat 0xFB =
-- 6), won, the scene after it, flag 203 and prosperity + 1 (Agumon goes to File City). Derived with
-- oracle/explore.lua (captures/oracle-runs/p9-rest-agumon-x1, won by VBlank 5609); the battle and
-- the lines are played by the controller below, so they don't depend on the fight lasting as
-- long as it did in the exploration.
local PRESSES = {
    { 3801, 3981, 60, 4, 'CROSS' },   -- "Wow!" and Agumon's challenge
}
local MILESTONES = {
    { 3803, 'agumon' },
    { 4600, 'battle' },
    { 5600, 'battle_late' },
    { 8000, 'agumon_done' },
}

-- From VBlank 4040, from the emulated RAM: in the battle (GAME_STATE 0x80134F0A = 1) a 4-VBlank
-- Square (the finisher) every 60 VBlanks while the partner's finisher gauge is full (COMBAT_DATA
-- 0x8013D640, fighter 0: finisherProgress +0x1A == finisherGoal +0x18); otherwise a 4-VBlank Cross
-- every 60 VBlanks (the battle's start, the results, the lines) until, after the battle, the tamer
-- is back to normal (TAMER_STATE 0x80134C91 = 0) with no battle on (GAME_STATE 0).
local FROM, SQUARE, CROSS = 4040, 15, 14
local seenBattle, done, held, releaseAt, nextTap = false, false, nil, nil, 0
local function controller(vblank, pad, ram)
    if vblank < FROM or done then return end
    local u8 = ffi.cast('uint8_t*', ram)
    local i16 = ffi.cast('int16_t*', ram)
    if releaseAt and vblank >= releaseAt then pad.clearOverride(held); releaseAt = nil end
    if releaseAt then return end
    local game = u8[0x134F0A]
    if game == 1 then
        seenBattle = true
        local goal, progress = i16[(0x13D640 + 0x18) / 2], i16[(0x13D640 + 0x1A) / 2]
        if goal > 0 and progress == goal and vblank >= nextTap then
            held = SQUARE; pad.setOverride(SQUARE); releaseAt, nextTap = vblank + 4, vblank + 60
        end
        return
    end
    if seenBattle and game == 0 and u8[0x134C91] == 0 then done = true; return end
    if vblank >= nextTap then
        held = CROSS; pad.setOverride(CROSS); releaseAt, nextTap = vblank + 4, vblank + 60
    end
end
return PRESSES, MILESTONES, controller
