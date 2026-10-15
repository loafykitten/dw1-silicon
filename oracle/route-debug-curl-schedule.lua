-- DW1ROUTE_PROFILE=debug-curl (Phase 9, stream fishcurl): after route-master99-yard-schedule.lua
-- and route-master99-warp-schedule.lua, on the fixture p9-fishcurl-debug-master99-penguin
-- (tools/oracle/debug_fixture.py --keep 45 --keep 46 --clear 257 --clear 357 on Master99's 2002
-- save: Penguinmon not recruited and his free match not used, so he waits outside the Curling
-- Cave, map script 206 section 6). Mr. Warp to the Curling Cave, Penguinmon's offer ("Digimon
-- Curling! ... Nothing the first time"), "Let's go inside", the rink (screen 136, the KAR
-- overlay), the match, and back outside. The walk and the talk were derived with oracle/explore.lua;
-- the match is played by the controller below (explorer-derived match inputs did not reproduce in
-- recordings: the opponent's turns take varying time), from the emulated RAM: KAR's match state
-- (0x80135244), whose turn (0x8013523C) and the power gauge (0x8013523E).
local PRESSES = {
    { 5740, 5740, 1, 4, 'CROSS' },    -- Mr. Warp: In front of Curling Cave (screen 134)
    -- to Penguinmon, by the cave mouth (x 1792, z 1185), and through his offer
    { 6801, 6801, 1, 15, 'RIGHT' },
    { 6818, 6818, 1, 100, 'UP' },
    { 6920, 6920, 1, 12, 'UP' },
    { 6936, 6936, 1, 4, 'CROSS' },
    { 7026, 7026, 1, 4, 'CROSS' },
    { 7116, 7116, 1, 4, 'CROSS' },
    { 7206, 7206, 1, 4, 'CROSS' },
    { 7296, 7296, 1, 4, 'CROSS' },
    { 7386, 7386, 1, 4, 'CROSS' },
    { 7476, 7476, 1, 4, 'CROSS' },
    { 7566, 7566, 1, 4, 'CROSS' },
    { 7656, 7656, 1, 4, 'CROSS' },
    { 7746, 7746, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 6800, 'cave_front' },
    { 6936, 'penguinmon' },
    { 8236, 'rink' },
    { 9000, 'first_throw' },
    { 13000, 'mid_match' },
    { 17000, 'late_match' },     -- the last scores: us 0, Penguinmon 5
    { 17500, 'back_outside' },   -- screen 134 again from VBlank 17356
    { 18600, 'curl_done' },
}

-- From the rink on: a 4-VBlank Cross, at least 30 VBlanks apart, whenever the match wants one:
-- Start Game (state 1), Penguinmon's lines (15, 19), our stone and straight aim (5, 6, our
-- turn), the power gauge (8) at 1900 or more; then, back outside, up to three more while his
-- closing lines' script runs. Cross is pad bit 14.
local FROM, CROSS = 8237, 14
-- The power at which each of our throws stops the gauge (DW1CURL_POWERS=a,b,... overrides it
-- for trying other matches; the recording used the default).
local POWERS = {}
for p in (os.getenv('DW1CURL_POWERS') or '1900'):gmatch('%d+') do POWERS[#POWERS + 1] = tonumber(p) end
local throws = 0
local releaseAt, nextTap, closes, wasInRink = nil, 0, 0, false
local function controller(vblank, pad, ram)
    if vblank < FROM then return end
    if releaseAt and vblank >= releaseAt then pad.clearOverride(CROSS); releaseAt = nil end
    if releaseAt or vblank < nextTap then return end
    local u8 = ffi.cast('uint8_t*', ram)
    local i8 = ffi.cast('int8_t*', ram)
    local i16 = ffi.cast('int16_t*', ram)
    local screen = u8[0x134DA8]
    local tap = false
    if screen == 136 then
        wasInRink = true
        local state, ours, power = i8[0x135244], i8[0x13523C] == 0, i16[0x13523E / 2]
        local want = POWERS[math.min(throws + 1, #POWERS)]
        tap = state == 1 or state == 15 or state == 19 or (ours and (state == 5 or state == 6))
            or (ours and state == 8 and power >= want)
        if ours and state == 8 and tap then throws = throws + 1 end
    elseif screen == 134 and wasInRink and closes < 3 and u8[0x134FF4] == 0 then
        -- a map script is running (IS_SCRIPT_PAUSED 0): his lines are up
        closes = closes + 1
        tap = true
    end
    if tap then
        pad.setOverride(CROSS)
        releaseAt, nextTap = vblank + 4, vblank + (screen == 136 and 30 or 120)
    end
end
return PRESSES, MILESTONES, controller
