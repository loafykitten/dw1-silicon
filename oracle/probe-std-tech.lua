-- Measures what the PS1 uses for STD_startQueuedMove's `tech` on the finisher path that never sets
-- it (docs/phase9-overlay-audit.md, finding 11). Runs oracle/replay.lua (any mode; replay mode
-- reads DW1REPLAY_FILE) and breaks at 0x8006569C, the first use of `tech` after the branch
-- (MOVE_DATA[tech].unk3 & 2, `sll $v1, $s3, 4`): there `tech` is $s3, which the function only
-- assigns on the other branch, so on the finisher path it still holds the caller's $s3.
-- Writes std-tech.txt, one line per hit on that path:
--   vblank s3 digimon(s0) fighter(s1) target(s2) caller_ra queuedAnim entity_type unk3 timer
-- Run like replay.lua (fresh directory, the fixture card as memcard1.mcd):
--   DW1REPLAY_MODE=replay DW1REPLAY_FILE=... -dofile oracle/probe-std-tech.lua
local HERE = (debug.getinfo(1, 'S').source:gsub('^@', ''):match('^(.*)/[^/]*$')) or '.'
dofile(HERE .. '/replay.lua')

local USE_TECH = 0x8006569C
local NO_AI_FLAG, FINISHING_ENTITY, STD_FINISHER_TIMER = 0x80134D74, 0x80134D60, 0x80135174
local MOVE_DATA = 0x8012623C
local WORD_AT_USE = 0x3C028012 -- lui $v0, %hi(MOVE_DATA + 0xD): only STD's code is there

local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function r32(a) a = a - 0x80000000; return mem[a] + mem[a + 1] * 256 + mem[a + 2] * 65536 + mem[a + 3] * 16777216 end
local function r8(a) return mem[a - 0x80000000] end
local out = assert(io.open('std-tech.txt', 'wb'))
local vblank = 0
VBLANK_LISTENER_TECH = PCSX.Events.createEventListener('GPU::Vsync', function() vblank = vblank + 1 end)
STD_TECH_BP = PCSX.addBreakpoint(USE_TECH, 'Exec', 4, 'STD tech use', function()
    local ok, err = pcall(function()
        if r32(USE_TECH) ~= WORD_AT_USE then return end
        local g = PCSX.getRegisters().GPR.n
        local s0, s1, s2, s3 = tonumber(g.s0), tonumber(g.s1), tonumber(g.s2), tonumber(g.s3)
        local noAi = r32(NO_AI_FLAG)
        if noAi == 0 then return end -- the branch that set tech
        local sp = tonumber(g.sp)
        local ra = r32(sp + 0x20)
        local s3s = s3 >= 0x80000000 and s3 - 0x100000000 or s3
        local unk3 = '-'
        local idx = MOVE_DATA + 0xD + s3s * 16
        if idx >= 0x80000000 and idx < 0x80200000 then unk3 = string.format('%02x', r8(idx)) end
        out:write(string.format('%d s3=%08x s0=%08x s1=%08x s2=%08x ra=%08x queued=%02x type=%d unk3=%s fin=%08x timer=%d\n',
            vblank, s3, s0, s1, s2, ra, r8(s1 + 0x38), r32(s0), unk3, r32(FINISHING_ENTITY),
            r32(STD_FINISHER_TIMER)))
        out:flush()
    end)
    if not ok then print('probe-std-tech error: ' .. tostring(err)) end
end)
