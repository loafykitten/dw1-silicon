-- Finds who writes the DMA interrupt register (DICR, 0x1F8010F4) with bit 15 set, the "DMA bus
-- error" the BIOS prints. Runs oracle/replay.lua (replay mode: DW1REPLAY_FILE) and, from VBlank
-- DW1_DICR_FROM (default 24000), logs each write to DICR: the VBlank, the PC, $ra and the value
-- (the stored register, decoded from the store instruction at the PC). Writes dicr.txt; at most
-- DW1_DICR_MAX (default 2000) lines.
local HERE = (debug.getinfo(1, 'S').source:gsub('^@', ''):match('^(.*)/[^/]*$')) or '.'
dofile(HERE .. '/replay.lua')

local FROM = tonumber(os.getenv('DW1_DICR_FROM') or '') or 24000
local MAX = tonumber(os.getenv('DW1_DICR_MAX') or '') or 2000
local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function r32(a)
    a = a % 0x200000
    return mem[a] + mem[a + 1] * 256 + mem[a + 2] * 65536 + mem[a + 3] * 16777216
end
local out = assert(io.open('dicr.txt', 'wb'))
local vblank, lines = 0, 0
local NAMES = { [0] = 'r0', 'at', 'v0', 'v1', 'a0', 'a1', 'a2', 'a3', 't0', 't1', 't2', 't3', 't4', 't5',
                't6', 't7', 's0', 's1', 's2', 's3', 's4', 's5', 's6', 's7', 't8', 't9', 'k0', 'k1', 'gp',
                'sp', 'fp', 'ra' }
local function hit()
    local ok, err = pcall(function()
        if lines >= MAX then return end
        local regs = PCSX.getRegisters()
        local pc = tonumber(regs.pc)
        local word = r32(pc)
        local rt = math.floor(word / 65536) % 32
        local value = rt == 0 and 0 or tonumber(regs.GPR.r[rt])
        out:write(string.format('%d pc=%08x ra=%08x word=%08x %s=%08x\n', vblank, pc,
            tonumber(regs.GPR.n.ra), word, NAMES[rt], value))
        out:flush()
        lines = lines + 1
    end)
    if not ok then print('probe-dicr error: ' .. tostring(err)) end
end
DICR_LISTENER = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if vblank == FROM then
        DICR_BP = PCSX.addBreakpoint(0x1F8010F4, 'Write', 4, 'DICR write', hit)
        DICR_BP8 = PCSX.addBreakpoint(0xBF8010F4, 'Write', 4, 'DICR write kseg1', hit)
        print('probe-dicr armed at ' .. vblank)
    end
end)
