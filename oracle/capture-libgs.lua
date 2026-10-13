-- S0.7 capture: play the new-game route to a milestone, then record every GsSortObject4J call
-- for a few VBlanks: the whole machine state at entry and at return (docs/s07-libgs-spike.md).
--
-- Breakpoints need Redux's debugger, which slows every instruction, so the script switches it
-- on only at the milestone (don't pass -debugger). Run from a fresh, empty directory under
-- captures/ (the dumps hold game data from the disc):
--   R=/Users/fenn/Developer/dw1-silicon
--   "$R/ref/pcsx-redux/pcsx-redux" -cli -portable -interpreter -softgpu \
--       -bios "$R/scph1001.bin" -iso "$R/disc/Digimon World (USA).cue" \
--       -dofile "$R/oracle/capture-libgs.lua" -run > run.log
-- Env: DW1_CAPTURE_AT (milestone, default first_partner_dialogue), DW1_CAPTURE_VBLANKS (window,
-- default 4), DW1_CAPTURE_MAX (calls, default 48).
--
-- Writes callNN_in.bin / callNN_out.bin (state format below), vram_first.bin / vram_last.bin
-- (1 MiB, VRAM at the first entry and when the window closes) and calls.txt (one line per call).
--
-- State file: "DW1STATE" then the registers as Redux's psxRegisters prefix, little-endian u32s:
-- GPR[34] (r0..ra, lo, hi), CP0[32], CP2D[32], CP2C[32], pc; then the 1 KiB scratchpad, then
-- the 2 MiB of RAM.

local GS_SORT_OBJECT4J = 0x80099D88

local AT = os.getenv('DW1_CAPTURE_AT') or 'first_partner_dialogue'
local WINDOW = tonumber(os.getenv('DW1_CAPTURE_VBLANKS') or '4')
local MAX_CALLS = tonumber(os.getenv('DW1_CAPTURE_MAX') or '48')
-- After the window closes, how long to wait for calls still in flight to return; and how long
-- after arming to wait for the first call.
local GRACE = 60
local FIRST_CALL_TIMEOUT = 600

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end
DW1ROUTE_TARGET = AT -- read when the route loads
dofile(here() .. '/route-new-game.lua') -- defines DW1ROUTE_FAIL and DW1ROUTE_WRITE
if not (WINDOW and WINDOW >= 1 and MAX_CALLS and MAX_CALLS >= 1) then
    DW1ROUTE_FAIL('DW1_CAPTURE_VBLANKS and DW1_CAPTURE_MAX must be at least 1')
end

local function writeState(name)
    DW1ROUTE_WRITE(name, 'DW1STATE'
        .. ffi.string(PCSX.getRegisters(), ffi.sizeof('psxRegisters'))
        .. ffi.string(PCSX.getScratchPtr(), 1024)
        .. ffi.string(PCSX.getMemPtr(), 2 * 1024 * 1024))
end

local function writeVram(name)
    local v = PCSX.GPU.getVRAM()
    DW1ROUTE_WRITE(name, ffi.string(v.data, v.size))
end

local index = {}
local vblank, armedAt, stopAt, calls, entryBp = 0, nil, nil, 0, nil
local done = false
-- Return breakpoints still waiting, by call number. Holding the handles keeps them alive: Redux
-- removes a Lua breakpoint when its handle is garbage-collected.
-- Finished ones move to `retired` and stay referenced until the run ends.
local pending, npending, retired = {}, 0, {}

local function finish(why)
    if done then return end
    done = true
    if entryBp then entryBp:remove() end
    writeVram('vram_last.bin')
    DW1ROUTE_WRITE('calls.txt', '# call vblank objp otp shift scratch ra\n' .. table.concat(index))
    if npending > 0 then
        local left = {}
        for n in pairs(pending) do left[#left + 1] = string.format('%02d', n) end
        table.sort(left)
        DW1ROUTE_FAIL(string.format('capture incomplete: calls %s never returned', table.concat(left, ' ')))
        return
    end
    print(string.format('DW1CAPTURE done calls=%d vblank=%d (%s)', calls, vblank, why))
    DW1ROUTE_SUCCEED()
end

local function onEntry()
    if done or calls >= MAX_CALLS or (stopAt and vblank >= stopAt) then return true end
    local n = calls
    calls = calls + 1
    local r = PCSX.getRegisters().GPR.n
    local ra = r.ra
    if n == 0 then
        writeVram('vram_first.bin')
        stopAt = vblank + WINDOW
    end
    index[#index + 1] = string.format('%02d %d %08x %08x %d %08x %08x\n', n, vblank, r.a0, r.a1, r.a2, r.a3, ra)
    writeState(string.format('call%02d_in.bin', n))
    -- One-shot breakpoint on the return address. GsSortObject4J doesn't recurse, so the next
    -- execution of `ra` is this call's return.
    npending = npending + 1
    pending[n] = PCSX.addBreakpoint(ra, 'Exec', 4, 'GsSortObject4J return', function()
        writeState(string.format('call%02d_out.bin', n))
        retired[n], pending[n] = pending[n], nil
        npending = npending - 1
        if calls >= MAX_CALLS and npending == 0 then finish('max calls') end
        return false
    end)
    return true
end

KEEP_CAPTURE_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if armedAt and not stopAt and vblank >= armedAt + FIRST_CALL_TIMEOUT then
        DW1ROUTE_FAIL(string.format('no GsSortObject4J call within %d VBlanks', FIRST_CALL_TIMEOUT))
    end
    if stopAt and vblank >= stopAt and (npending == 0 or vblank >= stopAt + GRACE) then
        finish('window')
    end
end)

DW1ROUTE_ON_TARGET = function(v)
    print(string.format('DW1CAPTURE arming at vblank=%d', v))
    armedAt = v
    PCSX.settings.emulator.Debug.Debug = true
    entryBp = PCSX.addBreakpoint(GS_SORT_OBJECT4J, 'Exec', 4, 'GsSortObject4J', onEntry)
end
