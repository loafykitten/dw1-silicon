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
--   shot NAME             screenshot NAME.raw/.txt now
--   dump NAME             main RAM (2 MB) to NAME.bin
--   state                 print the state line again
--   quit                  end the run
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
    while not active and not waitUntil do
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
