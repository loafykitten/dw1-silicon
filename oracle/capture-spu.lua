-- Phase 6 oracle: log every store to the SPU's registers (0x1F801C00-0x1F801DFF) while a route
-- plays (docs/phase6-plan.md). Redux checks write breakpoints before the store executes, so the
-- hook decodes the store instruction at the PC (SB/SH/SW) and reads the value from its source
-- register. Writes spu-writes.txt: "vblank address(hex) width value(hex)" per store, in order.
-- The SDK's code is only ever run, never read: the hook sees its stores to the hardware.
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
local TO = tonumber(os.getenv('DW1_SPU_TO') or '') or DW1ROUTE_FAIL('DW1_SPU_TO?')
local SPU_BASE, SPU_SIZE = 0x1F801C00, 0x200

local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function insn(pc)
    return ffi.cast('uint32_t*', mem + (pc % 0x200000))[0]
end

local vblank, lines, count, bp = 0, {}, 0, nil
local function onWrite(address, width)
    local r = PCSX.getRegisters()
    local w = insn(r.pc)
    local rt = bit.band(bit.rshift(w, 16), 31)
    local v = r.GPR.r[rt]
    if width == 1 then v = bit.band(v, 0xFF) elseif width == 2 then v = bit.band(v, 0xFFFF) end
    count = count + 1
    lines[#lines + 1] = string.format('%d %08x %d %x\n', vblank, address, width, v)
    return true
end

KEEP_SPU_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if vblank == FROM then
        PCSX.settings.emulator.Debug.Debug = true
        bp = PCSX.addBreakpoint(SPU_BASE, 'Write', SPU_SIZE, 'SPU writes', onWrite)
        print(string.format('DW1SPU armed at vblank %d', vblank))
    elseif vblank == TO then
        if bp then bp:remove() end
        DW1ROUTE_WRITE('spu-writes.txt', table.concat(lines))
        print(string.format('DW1SPU done writes=%d vblank=%d', count, vblank))
        DW1ROUTE_SUCCEED()
    end
end)
