-- Phase 9 (sound and movies): the emulator's movie decoding, frame by frame, while a certified
-- replay plays (docs/phase2-movie.md, "Pictures against PCSX-Redux"). For every DW1_MDEC_EVERY-th
-- DecDCTin call between VBlanks DW1_MDEC_FROM and DW1_MDEC_TO it records:
--   * the run-length codes handed to the MDEC (DecDCTin's buffer: what the SDK's DecDCTvlc2 made
--     of the frame's bitstream on the CPU; the header word's low half counts the words after it);
--   * every LoadImage until the next DecDCTin (MOV's slices: the MDEC's output, before VRAM).
-- Writes mdec-log.txt ("vblank frame k rle offset bytes mode" and "vblank slice k x y w h offset
-- bytes") and mdec.bin (the bytes). The SDK's code is only run; its arguments and the RAM it
-- points at are read at the call.
--
--   DW1_MDEC_FROM=5700 DW1_MDEC_TO=12300 DW1_MDEC_EVERY=25 DW1REPLAY_MODE=replay \
--       DW1REPLAY_FILE=oracle/replays/new-game.txt pcsx-redux ... -dofile oracle/capture-mdec.lua -run
-- Run from a fresh directory under captures/oracle-runs/; the replay runs to its end as usual.

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end

local FROM = tonumber(os.getenv('DW1_MDEC_FROM') or '') or error('DW1_MDEC_FROM?')
local TO = tonumber(os.getenv('DW1_MDEC_TO') or '') or error('DW1_MDEC_TO?')
local EVERY = tonumber(os.getenv('DW1_MDEC_EVERY') or '') or 25

local function symbol(name)
    local f = io.open(here() .. '/../decomp/config/us/symbols.txt', 'r')
    for line in f:lines() do
        local n, addr = line:match('^([%w_]+)%s*=%s*0x(%x+);')
        if n == name then f:close() return tonumber(addr, 16) end
    end
    f:close()
    error('no symbol ' .. name)
end

local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function ptr(addr) return mem + (addr % 0x200000) end
local function u16(addr) return ffi.cast('uint16_t*', ptr(addr))[0] end
local function u32(addr) return ffi.cast('uint32_t*', ptr(addr))[0] end

local vblank, calls, sampling, offset = 0, 0, false, 0
local bps = {}
local lines, bin = {}, io.open('mdec.bin', 'wb')
local function dump(addr, bytes)
    bin:write(ffi.string(ptr(addr), bytes))
    local at = offset
    offset = offset + bytes
    return at
end

local function onDecDCTin()
    if vblank < FROM or vblank > TO then return true end
    local r = PCSX.getRegisters().GPR.n
    calls = calls + 1
    sampling = (calls % EVERY) == 0
    -- LoadImage is watched only for a sampled frame's slices.
    if sampling then bps[2]:enable() else bps[2]:disable() end
    if sampling then
        local words = u32(r.a0) % 65536
        local bytes = (words + 1) * 4
        lines[#lines + 1] = string.format('%d frame %d rle %d %d %d\n', vblank, calls, dump(r.a0, bytes),
            bytes, r.a1)
    end
    return true
end

local function onLoadImage()
    if not sampling or vblank < FROM or vblank > TO then return true end
    local r = PCSX.getRegisters().GPR.n
    local x, y, w, h = u16(r.a0), u16(r.a0 + 2), u16(r.a0 + 4), u16(r.a0 + 6)
    local bytes = w * h * 2
    lines[#lines + 1] = string.format('%d slice %d %d %d %d %d %d %d\n', vblank, calls, x, y, w, h,
        dump(r.a1, bytes), bytes)
    return true
end

KEEP_MDEC_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if vblank == FROM then
        PCSX.settings.emulator.Debug.Debug = true
        bps[1] = PCSX.addBreakpoint(symbol('DecDCTin'), 'Exec', 4, 'DecDCTin', onDecDCTin)
        bps[2] = PCSX.addBreakpoint(symbol('LoadImage'), 'Exec', 4, 'LoadImage', onLoadImage)
        bps[2]:disable()
    elseif vblank == TO then
        for _, b in ipairs(bps) do b:remove() end
        bin:close()
        local f = io.open('mdec-log.txt', 'w')
        f:write(table.concat(lines))
        f:close()
        print(string.format('DW1MDEC done vblank=%d frames=%d bytes=%d', vblank, calls, offset))
    end
end)

dofile(here() .. '/replay.lua')
