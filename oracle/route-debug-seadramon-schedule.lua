-- DW1ROUTE_PROFILE=debug-seadramon (Phase 9, stream fishcurl): fishing for Seadramon. After
-- route-master99-yard-schedule.lua and route-master99-warp-schedule.lua (fixture
-- p9-fishcurl-debug-master99-rods): Mr. Warp to Dragon Eye Lake at 08:36 (Seadramon swims at hour
-- 8), FISH, Sirloin, the tamer turned to yaw 1088 and a cast at power 12 onto Seadramon's first
-- waypoint (x -1188, z -2202, where he slows), his bite, the strike, the fight and the catch:
-- triggerSeadramonCutscene, which ends in Beetle Land. Derived with
-- oracle/explore.lua.
local PRESSES = {
    { 5740, 5740, 1, 4, 'DOWN' },     -- Mr. Warp, Unknown World: Seadra fishing
    { 5770, 5770, 1, 4, 'CROSS' },    -- Dragon Eye Lake (screen 6)
    { 6521, 6521, 1, 4, 'TRIANGLE' },
    { 6581, 6581, 1, 4, 'CROSS' },
    { 6671, 6671, 1, 4, 'CROSS' },
    { 6742, 6742, 1, 3, 'RIGHT' },
    { 6755, 6755, 1, 4, 'CROSS' },
    { 6826, 6826, 1, 3, 'RIGHT' },
    { 6839, 6839, 1, 4, 'CROSS' },
    { 6863, 6863, 1, 4, 'CROSS' },
    { 7513, 7513, 1, 4, 'CROSS' },
    -- "Hooked!" closed; the fight and the scene after it are the controller's (below)
    { 7558, 7558, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 6500, 'lake' },
    { 7000, 'cast' },
    { 7540, 'hooked' },
    { 11000, 'fight' },
    { 13000, 'late_fight' },
    { 14500, 'lake_guardian' },
    { 15700, 'beetle_land' },
    { 16300, 'seadramon_done' },
}

-- The fight, played from the emulated RAM (inputs found with the explorer's autoreel did not
-- reproduce here): every 3 VBlanks while the rod is in its fight state (0xB at 0x8007B41C), Cross
-- held while the tension (0x8007B504) is at most HI and Seadramon (the hooked fish's state at
-- 0x8007B488) is not running (50); once he is landed, a 4-VBlank Cross every 120 VBlanks while a
-- map script runs (IS_SCRIPT_PAUSED 0x80134FF4 is 0), until the screen changes (Beetle Land).
-- HI 4500 lands him (so does 3000; at 6000 and 7500 the line snapped).
local FROM, CROSS = 7561, 14
-- The tension under which Cross is held (DW1SEA_HI overrides it for trying other fights).
local HI = tonumber(os.getenv('DW1SEA_HI') or '') or 4500
local pressed, releaseAt, nextTap, start = false, nil, 0, nil
local function controller(vblank, pad, ram)
    if vblank < FROM then return end
    local i32 = ffi.cast('int32_t*', ram)
    local u8 = ffi.cast('uint8_t*', ram)
    local rod = i32[0x7B41C / 4]
    if rod == 0xB then
        start = start or vblank
        if (vblank - start) % 3 == 0 then
            local fish, level = i32[0x7B488 / 4], i32[0x7B504 / 4]
            local press = level <= HI and fish ~= 50
            if press then pad.setOverride(CROSS) elseif pressed then pad.clearOverride(CROSS) end
            pressed = press
        end
        return
    end
    if pressed then pad.clearOverride(CROSS); pressed = false end
    if releaseAt and vblank >= releaseAt then pad.clearOverride(CROSS); releaseAt = nil end
    if start and not releaseAt and vblank >= nextTap and u8[0x134DA8] == 6 and u8[0x134FF4] == 0 then
        pad.setOverride(CROSS)
        releaseAt, nextTap = vblank + 4, vblank + 120
    end
end
return PRESSES, MILESTONES, controller
