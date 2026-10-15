-- DW1ROUTE_PROFILE=rest-egg (Phase 9, stream rest): the post-game Digitamamon egg. With flag 50
-- (Machinedramon beaten) set and 256 clear, script 184 (screen 225, the top of Mt. Infinity,
-- section 0x33) offers the egg instead of Machinedramon: "!!!", the predefined battle (pstat 0xFB
-- = 8), and after a win flag 256 (Digitamamon recruited: recalculatePPandArena counts flag 200 +
-- species) and prosperity + 3. The fixture captures/fixtures/p9-rest-egg.mcd is BeelzemontZero's
-- save (p9-arena-vs-p1: Piximon, all stats 999, flag 50 set; the card's only save: the continue
-- base) moved to the last corridor like the ending's (tools/oracle/place_fixture.py ... 222 254 0
-- -400 -4100 326=1 256=0). Master99's MetalGreymon (2002 save, flag 256 cleared) won one
-- exploration narrowly but lost all three recordings (two kept: captures/oracle-runs/
-- p9-rest-egg-r300, -r900); Piximon won the exploration (captures/oracle-runs/p9-rest-eggpix-x1)
-- and the recording, losing little HP. The
-- battle is played by the controller below, so it doesn't depend on the fight lasting as long as
-- it did in the exploration.
local PRESSES = {
    { 4031, 4031, 1, 540, 'UP' },     -- the corridor (222) to the boss room's warp
    { 4971, 4971, 1, 140, 'UP' },     -- into the room's trigger (225): "!!!"
    { 5311, 5761, 90, 4, 'CROSS' },   -- the lines, to the battle
}
local MILESTONES = {
    { 4972, 'boss_room' },
    { 5400, 'egg' },
    { 7500, 'battle' },
    { 11000, 'battle_late' },
    { 15000, 'egg_done' },
}

-- The battle and what follows, from the emulated RAM: in the battle (GAME_STATE 0x80134F0A = 1) a
-- 4-VBlank Square (the finisher, "FINISH!! PUSH") every 60 VBlanks while the partner's finisher
-- gauge is full (COMBAT_DATA 0x8013D640, fighter 0: finisherProgress +0x1A == finisherGoal
-- +0x18); after it, a 4-VBlank Cross every 60 VBlanks (the results, the lines) until the tamer is
-- back to normal (TAMER_STATE 0x80134C91 = 0) out of the battle.
local FROM, SQUARE, CROSS = 5800, 15, 14
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
    elseif seenBattle then
        if game == 0 and u8[0x134C91] == 0 then done = true; return end
        if vblank >= nextTap then
            held = CROSS; pad.setOverride(CROSS); releaseAt, nextTap = vblank + 4, vblank + 60
        end
    end
end
return PRESSES, MILESTONES, controller
