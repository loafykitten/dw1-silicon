-- Phase 9 (sound): what the emulator's CD controller does with the movies' XA audio, VBlank by
-- VBlank, while a certified replay plays (docs/phase6-plan.md, "XA audio against the emulator").
--
-- The XA ADPCM sectors never reach RAM (the drive sends them to the SPU's CD input), so no
-- breakpoint sees them. Redux's save state does: its SPU message keeps the last decoded XA
-- sector (the samples handed to the SPU, before the 37.8 -> 44.1 kHz interpolation) and its
-- CD-ROM message the header of the last sector read, the audio volume matrix (ATV) and the mute
-- flags. So from DW1_XA_FROM to DW1_XA_TO the script takes a save state at every VBlank, walks
-- its protobuf to those fields, and logs:
--   xa-log.txt   "vblank lba atv0 atv1 atv2 atv3 muted adpcmMuted mode freq stereo nsamples
--                 offset" per VBlank whose decoded sector changed (offset into xa-pcm.bin, -1 for
--                 unchanged), plus "vblank pos lba" when only the read position moved;
--   xa-pcm.bin   each changed sector's samples, interleaved 16-bit little-endian L/R frames.
-- (The save state is only read; emulation is unchanged.)
--
--   DW1_XA_FROM=4700 DW1_XA_TO=12600 DW1REPLAY_MODE=replay DW1REPLAY_FILE=oracle/replays/new-game.txt \
--       pcsx-redux ... -dofile oracle/capture-xa.lua -run
-- Run from a fresh directory under captures/oracle-runs/ (with the replay's fixture card as
-- memcard1.mcd if it has one). The replay runs to its end as usual.

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end

local FROM = tonumber(os.getenv('DW1_XA_FROM') or '') or error('DW1_XA_FROM?')
local TO = tonumber(os.getenv('DW1_XA_TO') or '') or error('DW1_XA_TO?')

-- Protobuf: a varint at p (a uint8_t*), returned as a Lua number (low 53 bits) and the next p.
local function varint(p)
    local v, mul = 0, 1
    while true do
        local b = p[0]
        p = p + 1
        v = v + (b % 128) * mul
        if b < 128 then return v, p end
        mul = mul * 128
    end
end

-- The fields of the message in [p, e): number -> { wire, value or pointer, length }.
local function fields(p, e)
    local out = {}
    while p < e do
        local key
        key, p = varint(p)
        local num, wire = math.floor(key / 8), key % 8
        if wire == 0 then
            local v
            v, p = varint(p)
            out[num] = { v = v }
        elseif wire == 2 then
            local len
            len, p = varint(p)
            out[num] = { p = p, len = len }
            p = p + len
        elseif wire == 5 then
            p = p + 4
        elseif wire == 1 then
            p = p + 8
        else
            error('protobuf wire type ' .. wire)
        end
    end
    return out
end

local function int32(f)
    if not f then return 0 end
    local v = f.v % 4294967296
    if v >= 2147483648 then v = v - 4294967296 end
    return v
end

local lines, pcmFile = {}, io.open('xa-pcm.bin', 'wb')
local offset, lastHash, lastLba = 0, nil, nil
local vblank = 0

local function msfToLba(b)
    local function bcd(x) return math.floor(x / 16) * 10 + x % 16 end
    return (bcd(b[0]) * 60 + bcd(b[1])) * 75 + bcd(b[2]) - 150
end

local function snapshot()
    local slice = PCSX.createSaveState()
    local base = ffi.cast('const uint8_t*', slice.data)
    local top = fields(base, base + slice.size)
    local spu = fields(top[6].p, top[6].p + top[6].len)
    local cd = fields(top[15].p, top[15].p + top[15].len)
    local lba = cd[38] and msfToLba(cd[38].p) or -1
    local atv = cd[26] and cd[26].p
    local xa = spu[3] and fields(spu[3].p, spu[3].p + spu[3].len) or {}
    local n = xa[4] and xa[4].v or 0
    local stereo = xa[3] and xa[3].v or 0
    local hash, pcm, bytes = nil, nil, 0
    if xa[7] and n > 0 then
        -- The pcm field is stereo-interleaved (the CD side expands mono) and 32768 bytes long.
        -- A sector counts as new when anything in it differs from the last one logged: its
        -- samples (all of them: a quiet sector can differ in a handful), length or history.
        bytes = math.min(n * 4, xa[7].len)
        pcm = xa[7].p
        hash = string.format('%d:%d:%d:', n, int32(xa[5] and fields(xa[5].p, xa[5].p + xa[5].len)[1]),
            int32(xa[6] and fields(xa[6].p, xa[6].p + xa[6].len)[1])) .. ffi.string(pcm, bytes)
    end
    if hash and hash ~= lastHash then
        pcmFile:write(ffi.string(pcm, bytes))
        lines[#lines + 1] = string.format('%d xa %d %d %d %d %d %d %d %d %d %d %d %d\n', vblank, lba,
            atv and atv[0] or -1, atv and atv[1] or -1, atv and atv[2] or -1, atv and atv[3] or -1,
            cd[18] and cd[18].v or 0, cd[28] and cd[28].v or 0, cd[29] and cd[29].v or 0,
            xa[1] and xa[1].v or 0, stereo, n, offset)
        offset = offset + bytes
        lastHash = hash
    elseif lba ~= lastLba then
        lines[#lines + 1] = string.format('%d pos %d\n', vblank, lba)
    end
    lastLba = lba
end

KEEP_XA_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if vblank >= FROM and vblank <= TO then
        snapshot()
        if vblank == TO then
            pcmFile:close()
            local f = io.open('xa-log.txt', 'w')
            f:write(table.concat(lines))
            f:close()
            print(string.format('DW1XA done vblank=%d sectors=%d bytes=%d', vblank, #lines, offset))
        end
    end
end)

dofile(here() .. '/replay.lua')
