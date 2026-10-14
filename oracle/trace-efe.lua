-- Dev tool (Phase 5): trace the battle effect interpreter (BTL's EFE scripts) in a window of
-- VBlanks, to compare with the port's run. Plays a route profile like capture-calls.lua, and from
-- DW1_TRACE_FROM to DW1_TRACE_TO keeps a breakpoint on each of the 18 opcode handlers in BTL's
-- jump table (BTL_jtbl_80073604, read from RAM once the overlay is in) and writes one line per
-- executed opcode to efe-trace.txt: "vblank opcode(hex) script_offset(hex) slot record", the
-- offset being EFE_SCRIPT_PTR - EFE_SCRIPT_HEAD (the script's place in its EFE file, the same
-- natively), slot the EFE slot (EFE_DATA_ITERATOR's index in EFE_DATA_PTR's table of 40-byte
-- entries) and record the sub-effect EFE_SCRIPT_CONTEXT is (its index in the slot's records).
--
--   DW1ROUTE_PROFILE=battle-fixture DW1_TRACE_FROM=4500 DW1_TRACE_TO=4800 \
--       pcsx-redux ... -dofile oracle/trace-efe.lua -run
-- Run from a fresh directory under captures/oracle-runs/ holding the profile's fixture card.

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end

DW1ROUTE_TARGET = 'title_menu'
DW1ROUTE_ON_TARGET = function() end
dofile(here() .. '/route-new-game.lua')

local FROM = tonumber(os.getenv('DW1_TRACE_FROM') or '') or DW1ROUTE_FAIL('DW1_TRACE_FROM?')
local TO = tonumber(os.getenv('DW1_TRACE_TO') or '') or DW1ROUTE_FAIL('DW1_TRACE_TO?')
local JTBL, HANDLERS = 0x80073604, 18
local PTR, CUR, HEAD, CTX, ITER, DATA_PTR = 0x80134D00, 0x80134D04, 0x80134D08, 0x80134CE8, 0x80134D0C, 0x80134D10

local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function r32(a) return ffi.cast('uint32_t*', mem + (a - 0x80000000))[0] end
local function r16(a) return ffi.cast('uint16_t*', mem + (a - 0x80000000))[0] end

local vblank, lines, bps = 0, {}, {}
local function onOp()
    local iter = tonumber(r32(ITER))
    local slot = math.floor((iter - tonumber(r32(DATA_PTR))) / 40)
    local rec = math.floor((tonumber(r32(CTX)) - tonumber(r32(iter + 16))) / 28)
    lines[#lines + 1] = string.format('%d %04x %x %d %d\n', vblank, r16(CUR), (r32(PTR) - r32(HEAD)) % 0x100000000,
        slot, rec)
    return true
end

KEEP_TRACE_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if vblank == FROM then
        PCSX.settings.emulator.Debug.Debug = true
        local seen = {}
        for i = 0, HANDLERS - 1 do
            local a = r32(JTBL + 4 * i)
            if not seen[a] then
                seen[a] = true
                bps[#bps + 1] = PCSX.addBreakpoint(a, 'Exec', 4, 'efe op ' .. i, onOp)
            end
        end
        print(string.format('DW1TRACE armed %d handlers at vblank %d', #bps, vblank))
    elseif vblank == TO then
        for _, bp in ipairs(bps) do bp:remove() end
        DW1ROUTE_WRITE('efe-trace.txt', table.concat(lines))
        print(string.format('DW1TRACE done ops=%d vblank=%d', #lines, vblank))
        DW1ROUTE_SUCCEED()
    end
end)
