-- Turn-based pad exploration (a dev tool for deriving routes; not part of the oracle).
--
-- Runs a route profile (DW1ROUTE_PROFILE, default new-game) up to a milestone
-- (DW1EXPLORE_TARGET, default the profile's last one), then stops emulation at every command
-- boundary and waits for a command file. Emulation is blocked inside the VBlank event, so
-- nothing runs while it waits and the run stays deterministic: the emulator sees only the pad
-- states below, keyed to VBlank numbers, exactly like a route schedule.
--
-- Run from a fresh directory like the route (docs/oracle-route.md), with -dofile of this file.
-- Control: write commands, one per line, to ./cmd.txt (the script removes it when it has read
-- it). Commands:
--   hold BTN[+BTN...] N   hold the buttons for N VBlanks
--   wait N                no input for N VBlanks
--   tap BTN               hold BTN for 4 VBlanks, then wait 26 more (the route's usual tap)
--   autoreel N [HI [RUN]] FISH only: reel a hooked fish for at most N VBlanks with no stops,
--                         deciding every 3 VBlanks from RAM (FISHING_DATA at 0x8007AC58: the rod's
--                         state, the hooked fish's state and the tension): hold Cross unless the
--                         tension is above HI (7500), or above RUN (5000) while the fish runs
--                         (state 50); the line snaps at 10000 held for a while
--   autocurl N            KAR only: play our side of a curling match for N VBlanks with no stops:
--                         a 4-VBlank Cross (30 apart) whenever the match state (0x80135244)
--                         wants one: Start Game (1), Penguinmon's lines (15, 19), our stone and
--                         straight aim (5, 6), and the power gauge (8) once it reaches 1900
--   shot NAME             screenshot NAME.raw/.txt now
--   dump NAME             main RAM (2 MB) to NAME.bin
--   waitne ADDR VALUE N   no input until the 32-bit word at ADDR (hex) is not VALUE (hex), at
--                         most N VBlanks (for waiting on game state, e.g. a fish biting)
--   state                 print the state line again
--   quit                  end the run
-- It never stops for commands while the game is using the CD (a CdRead, CdReadSync or
-- CdSearchFile call in the last 60 VBlanks): runs that paused mid-read diverged from their
-- recordings (Redux's CD timing follows the host clock), so it runs on with no input until the
-- drive has been quiet that long, and the state line shows where it stopped.
-- After each batch it prints "EXPLORE state ..." and takes a screenshot s_<vblank>. Every
-- executed press is appended to ./inputs.log as a route-schedule line
-- "{ vblank, vblank, 1, hold, 'BTN' }," ready for PRESSES.

local HERE = (debug.getinfo(1, 'S').source:gsub('^@', ''):match('^(.*)/[^/]*$')) or '.'
local B = { SELECT = 0, START = 3, UP = 4, RIGHT = 5, DOWN = 6, LEFT = 7, L2 = 8, R2 = 9,
            L1 = 10, R1 = 11, TRIANGLE = 12, CIRCLE = 13, CROSS = 14, SQUARE = 15 }

local C = dofile(HERE .. '/canonical-state.lua')
local pad = PCSX.SIO0.slots[1].pads[1]
-- Buttons named P2_<name> (P2_CROSS) are pressed on the second pad (Battle Mode's player 2).
local pad2 = PCSX.SIO0.slots[2].pads[1]

local function status()
    local rec = C.collect()
    local v = {}
    for _, r in ipairs(rec) do v[r.field.name] = r.value end
    local function a(t) return type(t) == 'table' and table.concat(t, ',') or tostring(t) end
    return string.format(
        'screen=%d prev=%d exit=%d main=%d game=%d menu=%d tamer_state=%d/%d partner_state=%d/%d '
        .. 'tamer=(%s)@%d clock=d%d %02d:%02d frame=%d cond=0x%x food=%d poop=%d pooping_timer=%d '
        .. 'hunger_hr=%d energy=%d money=%d',
        v['screen.current_screen'], v['screen.previous_screen'], v['screen.current_exit'],
        v['mode.main_state'], v['mode.game_state'], v['mode.is_in_menu'],
        v['mode.tamer_state'], v['mode.tamer_sub_state'], v['mode.partner_state'],
        v['mode.partner_sub_state'], a(v['tamer.pos']), v['tamer.yaw'], v['clock.day'],
        v['clock.hour'], v['clock.minute'], v['clock.current_frame'], v['partner.para.condition'],
        v['partner.para.food_level'], v['partner.para.poop_level'], v['partner.para.pooping_timer'],
        v['partner.para.next_hunger_hour'], v['partner.para.energy_level'], v['tamer.money'])
end

local function shot(name)
    local ss = PCSX.GPU.takeScreenShot()
    local f = assert(io.open(name .. '.raw', 'wb'))
    f:write(ffi.string(ss.data.data, ss.data.size)); f:close()
    f = assert(io.open(name .. '.txt', 'wb'))
    f:write(string.format('%d %d %d', tonumber(ss.width), tonumber(ss.height), tonumber(ss.bpp))); f:close()
end

local function readCommands()
    while true do
        local f = io.open('cmd.txt', 'rb')
        if f then
            local text = f:read('*a'); f:close()
            if #text > 0 and text:sub(-1) == '\n' then
                os.remove('cmd.txt')
                local q = {}
                for line in text:gmatch('[^\n]+') do q[#q + 1] = line end
                return q
            end
        end
        os.execute('sleep 0.25')
    end
end

local vblank = 0
local queue = {}
local active = nil      -- { buttons = {bits}, last = vblank at which to release }
local waitUntil = nil
local waitWord = nil    -- { index into ram32, value, last vblank } for waitne
local ram32 = ffi.cast('uint32_t*', PCSX.getMemPtr())
local reel = nil        -- { start, last, pressed (the press's VBlank or false) } for autoreel
local curl = nil        -- { last, pressedAt, releaseAt, nextTap } for autocurl
local function ramI32(addr)
    local v = ram32[bit.rshift(bit.band(addr, 0x1FFFFF), 2)]
    return v >= 0x80000000 and v - 0x100000000 or v
end
-- CD activity (see the header): the VBlank of the last libcd call the game made.
-- Before the takeover the route's VBlanks are counted here too, so a read in flight at the target
-- still holds the first stop back.
local lastCd = -1000
local takenOver, preCount = false, 0
KEEP_EXPLORE_COUNT = PCSX.Events.createEventListener('GPU::Vsync', function()
    if not takenOver then preCount = preCount + 1 end
end)
local function cdTouch() lastCd = takenOver and vblank or preCount end
-- A blocking CdReadSync(0) can outlast the 60 VBlanks, so a call counts as CD activity until it
-- returns. Return breakpoints are persistent, one per caller address (Redux makes an ffi callback
-- per Lua breakpoint and LuaJIT allows only about 2000: oracle/replay.lua), and act only when
-- their address is the pending call's (the libcd calls don't nest).
local cdSyncRa, cdSyncRets = nil, {}
local function cdSyncEnter()
    cdTouch()
    local ra = PCSX.getRegisters().GPR.n.ra
    cdSyncRa = ra
    if not cdSyncRets[ra] then
        cdSyncRets[ra] = PCSX.addBreakpoint(ra, 'Exec', 4, string.format('explore CdReadSync return %08x', ra),
            function()
                if cdSyncRa == ra then cdTouch(); cdSyncRa = nil end
                return true
            end)
    end
end
PCSX.settings.emulator.Debug.Debug = true
KEEP_EXPLORE_CD = {
    PCSX.addBreakpoint(0x800B2BB4, 'Exec', 4, 'explore CdRead', cdTouch),
    PCSX.addBreakpoint(0x800B2CB4, 'Exec', 4, 'explore CdReadSync', cdSyncEnter),
    PCSX.addBreakpoint(0x800B23C0, 'Exec', 4, 'explore CdSearchFile', cdTouch),
}
local needState = true
local log = assert(io.open('inputs.log', 'ab'))

local function parseButtons(s)
    local bits = {}
    for name in s:gmatch('[^+]+') do
        local n = name:upper()
        local p, base = pad, n
        if n:sub(1, 3) == 'P2_' then p, base = pad2, n:sub(4) end
        assert(B[base], 'unknown button ' .. name)
        bits[#bits + 1] = { p, B[base] }
    end
    return bits
end

local function tick()
    vblank = vblank + 1
    -- release first, then press, as the route does
    if active and vblank >= active.last then
        for _, b in ipairs(active.bits) do b[1].clearOverride(b[2]) end
        active = nil
    end
    if waitUntil and vblank >= waitUntil then waitUntil = nil end
    if waitWord and (ram32[waitWord[1]] ~= waitWord[2] or vblank >= waitWord[3]) then
        waitWord = nil
    end
    -- autoreel and autocurl log a press when it ends, with the VBlanks it was really held (a
    -- deadline or the rod's state can cut it short), so inputs.log replays what ran.
    local function logPress(from)
        log:write(string.format("    { %d, %d, 1, %d, 'CROSS' },\n", from, from, vblank - from))
        log:flush()
    end
    if reel then
        local rodState = ramI32(0x8007B41C)
        if vblank >= reel.last or rodState ~= 0xB then
            if reel.pressed then pad.clearOverride(B.CROSS); logPress(reel.pressed) end
            reel = nil
        elseif (vblank - reel.start) % 3 == 0 then
            local fish, level = ramI32(0x8007B488), ramI32(0x8007B504)
            local press = level <= reel.hi and not (fish == 50 and level > reel.run)
            if reel.pressed then logPress(reel.pressed) end
            if press then
                pad.setOverride(B.CROSS)
            elseif reel.pressed then
                pad.clearOverride(B.CROSS)
            end
            reel.pressed = press and vblank or nil
        end
    end
    if curl then
        if curl.releaseAt and vblank >= curl.releaseAt then
            pad.clearOverride(B.CROSS); logPress(curl.pressedAt); curl.releaseAt = nil
        end
        if vblank >= curl.last then
            if curl.releaseAt then pad.clearOverride(B.CROSS); logPress(curl.pressedAt) end
            curl = nil
        elseif not curl.releaseAt and vblank >= curl.nextTap then
            local mem8 = ffi.cast('int8_t*', PCSX.getMemPtr())
            local mem16 = ffi.cast('int16_t*', PCSX.getMemPtr())
            local state, player = mem8[0x135244], mem8[0x13523C]
            local power = mem16[0x13523E / 2]
            local ours = player == 0
            if state == 1 or state == 15 or state == 19 or (ours and (state == 5 or state == 6))
                or (ours and state == 8 and power >= 1900) then
                pad.setOverride(B.CROSS)
                curl.pressedAt, curl.releaseAt, curl.nextTap = vblank, vblank + 4, vblank + 30
            end
        end
    end
    while not active and not waitUntil and not waitWord and not reel and not curl do
        if #queue == 0 and (cdSyncRa or vblank - lastCd < 60) then return end
        if #queue == 0 then
            if needState then
                print(string.format('EXPLORE state vblank=%d %s', vblank, status()))
                shot(string.format('s_%d', vblank))
                needState = false
            end
            io.stdout:flush()
            queue = readCommands()
            needState = true
        end
        local line = table.remove(queue, 1)
        local cmd, a1, a2 = line:match('^(%S+)%s*(%S*)%s*(%S*)')
        if cmd == 'hold' then
            local n = tonumber(a2)
            local bits = parseButtons(a1)
            for _, b in ipairs(bits) do b[1].setOverride(b[2]) end
            active = { bits = bits, last = vblank + n }
            for name in a1:gmatch('[^+]+') do
                log:write(string.format("    { %d, %d, 1, %d, '%s' },\n", vblank, vblank, n, name:upper()))
            end
            log:flush()
        elseif cmd == 'tap' then
            local bits = parseButtons(a1)
            for _, b in ipairs(bits) do b[1].setOverride(b[2]) end
            active = { bits = bits, last = vblank + 4 }
            waitUntil = vblank + 30
            log:write(string.format("    { %d, %d, 1, 4, '%s' },\n", vblank, vblank, a1:upper()))
            log:flush()
        elseif cmd == 'wait' then
            waitUntil = vblank + tonumber(a1)
        elseif cmd == 'waitne' then
            local addr, value = tonumber(a1, 16), tonumber(a2, 16)
            local n = tonumber(line:match('%S+%s+%S+%s+%S+%s+(%S+)'))
            waitWord = { bit.rshift(bit.band(addr, 0x1FFFFF), 2), value, vblank + n }
        elseif cmd == 'autoreel' then
            local hi, run = line:match('^%S+%s+%S+%s+(%S+)%s*(%S*)')
            reel = { start = vblank, last = vblank + tonumber(a1), pressed = false,
                     hi = tonumber(hi) or 7500, run = tonumber(run) or 5000 }
        elseif cmd == 'autocurl' then
            curl = { last = vblank + tonumber(a1), nextTap = vblank }
        elseif cmd == 'shot' then
            shot(a1)
        elseif cmd == 'dump' then
            -- the 2 MB of main RAM, for tools/oracle/map_triggers.py and the like
            local f = assert(io.open(a1 .. '.bin', 'wb'))
            f:write(ffi.string(PCSX.getMemPtr(), 0x200000)); f:close()
            print('EXPLORE dumped ' .. a1 .. '.bin')
        elseif cmd == 'state' then
            print(string.format('EXPLORE state vblank=%d %s', vblank, status()))
        elseif cmd == 'quit' then
            print('EXPLORE quit vblank=' .. vblank)
            PCSX.quit(0)
            return
        else
            print('EXPLORE unknown command: ' .. line)
        end
    end
end

DW1ROUTE_TARGET = DW1ROUTE_TARGET or os.getenv('DW1EXPLORE_TARGET')
DW1ROUTE_ON_TARGET = function(v)
    lastCd = v - (preCount - lastCd)
    takenOver = true
    vblank = v
    print('EXPLORE start vblank=' .. v)
    -- Detach the route: its listener must not fire its remaining scheduled presses (they would
    -- be missing from inputs.log), and any button it holds is released.
    DW1ROUTE_DETACHED = true
    for _, b in pairs(B) do pad.clearOverride(b); pad2.clearOverride(b) end
    KEEP_EXPLORE_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', tick)
    -- the route's own listener already ran for this VBlank; take over from the next one, but
    -- show the state now
    needState = true
    queue = {}
end
dofile(HERE .. '/route-new-game.lua')
