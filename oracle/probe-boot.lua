-- S0.8 boot probe for PCSX-Redux (-cli -interpreter, Ember's retail BIOS).
--
-- Boots the disc, taps Start/Cross on a fixed schedule to get past the title, and watches the
-- BIOS random generator: rand() in DW1 is the SDK stub into BIOS A(2Fh), whose state lives in
-- kernel RAM at 0x9010 (BIOS 0xBFC021F0: x = x * 0x41C64E6D + 0x3039). Each VBlank it reads the
-- state and, if it moved, counts how many rand() calls happened by stepping the generator. No
-- breakpoints (they slow the interpreter ~20x).
--
-- Output lines start with "DW1PROBE". Env: DW1_PROBE_VBLANKS (default 7200).

local SEED_ADDR = 0x9010
local MAX_STEPS = 1000000
local MAX_VBLANKS = tonumber(os.getenv('DW1_PROBE_VBLANKS') or '') or 60 * 120

-- PS1 pad bits (as PCSX-Redux numbers them).
local START, CROSS = 3, 14

-- Press a button for 4 VBlanks every 90, alternating Start and Cross, from VBlank 600 on.
local function scheduledButton(vblank)
    if vblank < 600 then return nil end
    local phase = (vblank - 600) % 90
    if phase >= 4 then return nil end
    if math.floor((vblank - 600) / 90) % 2 == 0 then return START end
    return CROSS
end

local mem = ffi.cast('uint32_t*', PCSX.getMemPtr())
local function readSeed() return mem[SEED_ADDR / 4] end

local function step(x)
    -- 32-bit wrapping multiply in Lua's 64-bit LuaJIT number space via bit ops.
    local lo = bit.band(x, 0xFFFF)
    local hi = bit.rshift(x, 16)
    local a = 0x41C64E6D
    local alo, ahi = bit.band(a, 0xFFFF), bit.rshift(a, 16)
    local p = lo * alo + bit.lshift(bit.band(lo * ahi + hi * alo, 0xFFFF), 16)
    return bit.tobit(p + 0x3039) % 0x100000000
end

local vblanks, totalCalls, events = 0, 0, 0
local lastSeed = readSeed()
local pad = PCSX.SIO0.slots[1].pads[1]
local held = nil

local function out(fmt, ...) print(string.format('DW1PROBE ' .. fmt, ...)) end

KEEP_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblanks = vblanks + 1

    local want = scheduledButton(vblanks)
    if want ~= held then
        if held then pad.clearOverride(held) end
        if want then pad.setOverride(want) end
        held = want
    end

    local seed = readSeed()
    if seed ~= lastSeed then
        local x, n = lastSeed, 0
        while x ~= seed and n < MAX_STEPS do
            x = step(x)
            n = n + 1
        end
        if x ~= seed then
            out('seed jump vblank=%d from=0x%08x to=0x%08x (not reachable by rand steps)', vblanks,
                lastSeed, seed)
        else
            totalCalls = totalCalls + n
            events = events + 1
            if events <= 40 then
                out('rand vblank=%d calls=%d total=%d seed=0x%08x', vblanks, n, totalCalls, seed)
            end
        end
        lastSeed = seed
    end

    if vblanks % 600 == 0 then out('heartbeat vblank=%d total_rand=%d', vblanks, totalCalls) end
    if vblanks >= MAX_VBLANKS then
        out('done vblanks=%d total_rand=%d initial_seed_seen=0x%08x', vblanks, totalCalls, INITIAL)
        PCSX.quit(0)
    end
end)

INITIAL = lastSeed
out('start seed=0x%08x max_vblanks=%d', lastSeed, MAX_VBLANKS)
