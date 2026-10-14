-- Phase 6 oracle: log every store to the SPU's registers (0x1F801C00-0x1F801DFF) while a route
-- plays (docs/phase6-plan.md). Redux checks write breakpoints before the store executes, so the
-- hook decodes the store instruction at the PC (SB/SH/SW) and reads the value from its source
-- register. Writes spu-writes.txt: "vblank address(hex) width value(hex) cycle" per store, in
-- order; cycle counts CPU cycles since the VBlank began (the GPU::Vsync event).
-- The SDK's code is only ever run, never read: the hook sees its stores to the hardware.
--
-- DW1_SPU_CALLS=SsUtKeyOnV,SsSeqPlay,... also logs entries to those functions (main executable
-- symbols) in the same stream, so each call lines up with the stores it causes:
-- "vblank call name a0 a1 a2 a3 stack0 stack1 stack2 stack3 ra cycle" (the stack words are the
-- o32 ABI's fifth to eighth arguments, at sp+16). An SsVabOpenHeadSticky call is followed by
-- "vblank vh id hex": the VAB header the game passed (header, program and tone tables, sample
-- sizes), so the parameters keyed on can be checked against the bank they came from.
--
--   DW1ROUTE_PROFILE=battle-fixture DW1_SPU_FROM=2000 DW1_SPU_TO=9200 \
--       pcsx-redux ... -dofile oracle/capture-spu.lua -run
-- Run from a fresh directory under captures/oracle-runs/ holding the profile's fixture card.

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end

DW1ROUTE_TARGET = 'title_menu'
DW1ROUTE_ON_TARGET = function() end
dofile(here() .. '/route-new-game.lua')

local FROM = tonumber(os.getenv('DW1_SPU_FROM') or '') or 1
-- DW1_SPU_WRITES=0 logs only the DW1_SPU_CALLS entries (much faster: no write breakpoint)
local WRITES = os.getenv('DW1_SPU_WRITES') ~= '0'
local TO = tonumber(os.getenv('DW1_SPU_TO') or '') or DW1ROUTE_FAIL('DW1_SPU_TO?')
local SPU_BASE, SPU_SIZE = 0x1F801C00, 0x200

local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function insn(pc)
    return ffi.cast('uint32_t*', mem + (pc % 0x200000))[0]
end

local calls = {}
do
    local wanted = {}
    for name in (os.getenv('DW1_SPU_CALLS') or ''):gmatch('[^,%s]+') do wanted[name] = true end
    local f = io.open(here() .. '/../decomp/config/us/symbols.txt', 'r')
    if not f then DW1ROUTE_FAIL('cannot read decomp/config/us/symbols.txt') end
    for line in f:lines() do
        local name, addr = line:match('^([%w_]+)%s*=%s*0x(%x+);')
        if name and wanted[name] then
            calls[#calls + 1] = { name = name, addr = tonumber(addr, 16) }
            wanted[name] = nil
        end
    end
    f:close()
    for name in pairs(wanted) do DW1ROUTE_FAIL('no symbol ' .. name) end
end

local vblank, lines, count, bp, callBps = 0, {}, 0, nil, {}
local function word(addr)
    return ffi.cast('uint32_t*', mem + (addr % 0x200000))[0]
end

local frameStart = 0
local function cycle()
    return tonumber(PCSX.getCPUCycles() - frameStart)
end

local function onCall(fn)
    return function()
        local r = PCSX.getRegisters().GPR.n
        lines[#lines + 1] = string.format('%d call %s %08x %08x %08x %08x %08x %08x %08x %08x %08x %d\n',
            vblank, fn.name, r.a0, r.a1, r.a2, r.a3, word(r.sp + 16), word(r.sp + 20), word(r.sp + 24),
            word(r.sp + 28), r.ra, cycle())
        if fn.name == 'SsVabOpenHeadSticky' and r.a0 >= 0x80000000 then
            -- VabHdr (32 bytes, ps at +18) + 128 ProgAtr (16) + ps * 16 VagAtr (32) + 256 u16
            local ps = mem[(r.a0 + 18) % 0x200000] + 256 * mem[(r.a0 + 19) % 0x200000]
            local size = 32 + 128 * 16 + math.min(ps, 128) * 16 * 32 + 512
            local hex = {}
            for i = 0, size - 1 do hex[#hex + 1] = string.format('%02x', mem[(r.a0 + i) % 0x200000]) end
            lines[#lines + 1] = string.format('%d vh %d %s\n', vblank, r.a1, table.concat(hex))
        end
        return true
    end
end

local function onWrite(address, width)
    local r = PCSX.getRegisters()
    local w = insn(r.pc)
    local rt = bit.band(bit.rshift(w, 16), 31)
    local v = r.GPR.r[rt]
    if width == 1 then v = bit.band(v, 0xFF) elseif width == 2 then v = bit.band(v, 0xFFFF) end
    count = count + 1
    lines[#lines + 1] = string.format('%d %08x %d %x %d\n', vblank, address, width, v, cycle())
    return true
end

KEEP_SPU_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    frameStart = PCSX.getCPUCycles()
    if vblank == FROM then
        PCSX.settings.emulator.Debug.Debug = true
        if WRITES then bp = PCSX.addBreakpoint(SPU_BASE, 'Write', SPU_SIZE, 'SPU writes', onWrite) end
        for _, fn in ipairs(calls) do
            callBps[#callBps + 1] = PCSX.addBreakpoint(fn.addr, 'Exec', 4, fn.name, onCall(fn))
        end
        print(string.format('DW1SPU armed at vblank %d', vblank))
    elseif vblank == TO then
        if bp then bp:remove() end
        for _, b in ipairs(callBps) do b:remove() end
        DW1ROUTE_WRITE('spu-writes.txt', table.concat(lines))
        print(string.format('DW1SPU done writes=%d vblank=%d', count, vblank))
        DW1ROUTE_SUCCEED()
    end
end)
