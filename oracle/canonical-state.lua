-- Canonical game-state dump, schema v1 (docs/canonical-state.md).
--
-- A versioned, fixed-width, little-endian serialization of the semantic game state, read from
-- PS1 RAM, plus a text rendering for diffs. Never a raw struct dump: padding, unused fields,
-- renderer/animation pointers and the script jmp_buf are excluded; pointers are either followed
-- (a position is read through the entity's posData) or normalised (script cursors become
-- buffer-relative offsets). Addresses are the US build's (decomp/config/us/symbols.txt).
--
-- Use inside Redux:  local C = dofile('.../canonical-state.lua'); local rec = C.collect()
--   C.encode(rec) -> binary string      C.render(rec) -> text string
--   C.dump(basename) -> writes basename.bin and basename.txt (via DW1ROUTE_WRITE if defined)
-- Also loadable outside Redux with a memory reader: C.collect(function(off, n) ... end) where
-- the function returns the n bytes at RAM offset `off` as a string.
--
-- Binary layout: "DW1CANON" (8 bytes), u32 schema version, u32 body length, then every field in
-- the order of C.fields, arrays inline, no padding. C.layout() lists offsets.

local C = {}
C.VERSION = 1

local SIZE = { u8 = 1, s8 = 1, u16 = 2, s16 = 2, u32 = 4, s32 = 4 }
local SIGNED = { s8 = true, s16 = true, s32 = true }

-- Reader over RAM. `getbytes(off, n)` returns n bytes at RAM offset off (0 = 0x80000000).
local R = {}
local getbytes

local function ramOffset(addr)
    local off = addr >= 0x80000000 and addr - 0x80000000 or addr
    if off < 0 or off + 1 > 0x200000 then error(string.format('address 0x%08x outside RAM', addr)) end
    return off
end

local function rd(addr, ty)
    local n = SIZE[ty]
    local s = getbytes(ramOffset(addr), n)
    local v = 0
    for i = n, 1, -1 do v = v * 256 + s:byte(i) end
    if SIGNED[ty] and v >= 2 ^ (8 * n - 1) then v = v - 2 ^ (8 * n) end
    return v
end

local function rdArray(addr, ty, count)
    local t = {}
    for i = 0, count - 1 do t[#t + 1] = rd(addr + i * SIZE[ty], ty) end
    return t
end

-- Field table. Each entry: { name=, ty=, count=, hex=, src=, read=function() -> number|table }.
C.fields = {}
local function add(name, ty, count, src, read, hex)
    C.fields[#C.fields + 1] = { name = name, ty = ty, count = count, src = src, read = read, hex = hex }
end

-- A value stored at an absolute address.
local function abs(name, ty, addr, symbol, hex)
    add(name, ty, 1, string.format('%s 0x%08X', symbol, addr), function() return rd(addr, ty) end, hex)
end
local function absArray(name, ty, addr, count, symbol, hex)
    add(name, ty, count, string.format('%s 0x%08X', symbol, addr),
        function() return rdArray(addr, ty, count) end, hex)
end
-- A derived value.
local function derived(name, ty, count, src, fn, hex) add(name, ty, count, src, fn, hex) end

-- Addresses: decomp/config/us/symbols.txt (all checked against it, 2026-10-13) plus member offsets
-- from the decomp headers (docs/canonical-state.md).
local A = {
    MAIN_STATE = 0x80134EB4, GAME_STATE = 0x80134F0A, IS_IN_MENU = 0x80134EF0,
    MAP_LAYER_ENABLED = 0x80134EC8, PAUSE_STATE = 0x80134E79,
    TAMER_STATE = 0x80134C91, TAMER_SUB_STATE = 0x80134C92,
    PARTNER_STATE = 0x80134C5B, PARTNER_SUB_STATE = 0x80134C5C,
    CURRENT_SCREEN = 0x80134DA8, PREVIOUS_SCREEN = 0x80134DA9,
    CURRENT_EXIT = 0x80134DAA, PREVIOUS_EXIT = 0x80134DAB,
    CURRENT_SCREEN_ID = 0x80134FFE, CURRENT_SCRIPT_ID = 0x80134FC0, ACTIVE_MAP_SCRIPT = 0x80134FC2,
    ACTIVE_SCRIPT = 0x80134FD8, SCRIPT_POINTER = 0x80134FDC, SCRIPT_SECTION = 0x80134FE4,
    SCRIPT_SECTION_IS_EVENT = 0x80134FE0, ACTIVE_INSTRUCTION = 0x80134FE8,
    IS_SCRIPT_PAUSED = 0x80134FF4, SCRIPT_STACK_OFFSET = 0x80134FC6, SCRIPT_PARAM_1 = 0x80134FF8,
    SCRIPT_STATE_2 = 0x80134FFA, DELAY_FRAMES = 0x80134FFC,
    MAPHEAD_SCRIPT_PTR = 0x80134FAC, MAP_SCRIPT_PTR = 0x80134FB4,
    SCRIPT_STATE = 0x801BDED8,
    CUSTOM_RNG_VAL1 = 0x80134E68, CUSTOM_RNG_VAL2 = 0x8013442C, BIOS_RAND_STATE = 0x9010,
    YEAR = 0x80134F02, DAY = 0x80134F04, HOUR = 0x80134EBC, MINUTE = 0x80134EBE,
    CURRENT_FRAME = 0x80134F08, LAST_HANDLED_FRAME = 0x80134F06, SUBFRAME_COUNT = 0x80134E1C,
    IS_GAMETIME_RUNNING = 0x80134E20, PLAYTIME_FRAMES = 0x80134F00,
    PLAYTIME_MINUTES = 0x80134EFC, PLAYTIME_HOURS = 0x80134EFE,
    MONEY = 0x80134EB8, TAMER_ENTITY = 0x8015576C, PARTNER_ENTITY = 0x801557A8,
    PARTNER_PARA = 0x80138460, INVENTORY = 0x8013D474,
    MERIT = 0x80134FC4, BATTLES_STARTED = 0x80134FC8, BATTLES_FLED = 0x80134FCA,
    TOURNAMENTS_WON = 0x80134FCC, TOURNAMENT_WINS = 0x80134FCE, TOURNAMENTS_LOST = 0x80134FD0,
}
C.ADDR = A

-- A pointer to a PositionData inside RAM (0x80xxxxxx, room for the 0x88-byte struct), or nil.
local function posPtr(entityAddr)
    local p = rd(entityAddr + 0x04, 'u32')
    if p >= 0x80000000 and p + 0x88 <= 0x80200000 then return p end
    return nil
end
local function posValid(entityAddr)
    return posPtr(entityAddr) and 1 or 0
end
local function posXYZ(entityAddr)
    local p = posPtr(entityAddr)
    if not p then return { 0, 0, 0 } end
    return rdArray(p + 0x78, 's32', 3)
end
local function posYaw(entityAddr)
    local p = posPtr(entityAddr)
    return p and rd(p + 0x72, 's16') or 0
end

-- Normalises a script pointer: which resident buffer it lies in (0 null, 1 maphead SCN buffer,
-- 2 cached map SCN buffer, 3 anywhere else) and the byte offset from that buffer's base. The
-- buffers are 25000 and 16384 bytes (MAPHEAD_SCRIPT_BUFFER, MAP_SCRIPT_BUFFER).
local function normScript(p)
    if p == 0 then return 0, 0 end
    local heads = { { 1, rd(A.MAPHEAD_SCRIPT_PTR, 'u32'), 25000 }, { 2, rd(A.MAP_SCRIPT_PTR, 'u32'), 16384 } }
    for _, h in ipairs(heads) do
        if h[2] ~= 0 and p >= h[2] and p - h[2] <= h[3] then return h[1], p - h[2] end
    end
    return 3, 0
end

-- ---- mode ----
abs('mode.main_state', 's8', A.MAIN_STATE, 'MAIN_STATE')
abs('mode.game_state', 's8', A.GAME_STATE, 'GAME_STATE')
abs('mode.is_in_menu', 's32', A.IS_IN_MENU, 'IS_IN_MENU')
abs('mode.map_layer_enabled', 'u8', A.MAP_LAYER_ENABLED, 'MAP_LAYER_ENABLED')
abs('mode.pause_state', 'u8', A.PAUSE_STATE, 'PAUSE_STATE')
abs('mode.tamer_state', 's8', A.TAMER_STATE, 'TAMER_STATE')
abs('mode.tamer_sub_state', 's8', A.TAMER_SUB_STATE, 'TAMER_SUB_STATE')
abs('mode.partner_state', 's8', A.PARTNER_STATE, 'PARTNER_STATE')
abs('mode.partner_sub_state', 's8', A.PARTNER_SUB_STATE, 'PARTNER_SUB_STATE')

-- ---- screen / script ----
abs('screen.current_screen', 'u8', A.CURRENT_SCREEN, 'CURRENT_SCREEN')
abs('screen.previous_screen', 'u8', A.PREVIOUS_SCREEN, 'PREVIOUS_SCREEN')
abs('screen.current_exit', 'u8', A.CURRENT_EXIT, 'CURRENT_EXIT')
abs('screen.previous_exit', 'u8', A.PREVIOUS_EXIT, 'PREVIOUS_EXIT')
abs('script.current_screen_id', 'u16', A.CURRENT_SCREEN_ID, 'CURRENT_SCREEN_ID')
abs('script.current_script_id', 'u16', A.CURRENT_SCRIPT_ID, 'CURRENT_SCRIPT_ID')
abs('script.active_map_script', 'u16', A.ACTIVE_MAP_SCRIPT, 'ACTIVE_MAP_SCRIPT')
derived('script.active_buffer', 'u8', 1, 'ACTIVE_SCRIPT 0x80134FD8, normalised',
    function() return (normScript(rd(A.ACTIVE_SCRIPT, 'u32'))) end)
derived('script.cursor_buffer', 'u8', 1, 'SCRIPT_POINTER 0x80134FDC, normalised',
    function() return (normScript(rd(A.SCRIPT_POINTER, 'u32'))) end)
derived('script.cursor_offset', 'u32', 1, 'SCRIPT_POINTER 0x80134FDC, normalised',
    function() return select(2, normScript(rd(A.SCRIPT_POINTER, 'u32'))) end)
abs('script.section', 'u8', A.SCRIPT_SECTION, 'SCRIPT_SECTION')
abs('script.section_is_event', 's32', A.SCRIPT_SECTION_IS_EVENT, 'SCRIPT_SECTION_IS_EVENT')
abs('script.active_instruction', 'u8', A.ACTIVE_INSTRUCTION, 'ACTIVE_INSTRUCTION')
abs('script.is_paused', 's32', A.IS_SCRIPT_PAUSED, 'IS_SCRIPT_PAUSED')
abs('script.param_1', 'u16', A.SCRIPT_PARAM_1, 'SCRIPT_PARAM_1')
abs('script.state_2', 'u16', A.SCRIPT_STATE_2, 'SCRIPT_STATE_2')
abs('script.delay_frames', 'u16', A.DELAY_FRAMES, 'DELAY_FRAMES')
abs('script.stack_depth', 'u16', A.SCRIPT_STACK_OFFSET, 'SCRIPT_STACK_OFFSET')
-- Script stack: 8 entries of { buffer, offset, script id, smth0, smth1 }; entries at or beyond
-- the depth are zero (they hold stale bytes), as are pointer parts that mean nothing.
local STACK = A.SCRIPT_STATE + 0x25C
local function stackEntries(field)
    return function()
        local depth = rd(A.SCRIPT_STACK_OFFSET, 'u16')
        local out = {}
        for i = 0, 7 do
            local v = 0
            if i < depth then
                local base = STACK + 8 * i
                if field == 'buffer' then v = (normScript(rd(base, 'u32')))
                elseif field == 'offset' then v = select(2, normScript(rd(base, 'u32')))
                elseif field == 'id' then v = rd(base + 4, 'u16')
                elseif field == 'smth0' then v = rd(base + 6, 'u8')
                else v = rd(base + 7, 'u8') end
            end
            out[#out + 1] = v
        end
        return out
    end
end
derived('script.stack_buffer', 'u8', 8, 'ScriptState.stack[i].scriptPtr 0x801BE134+8i, normalised', stackEntries('buffer'))
derived('script.stack_offset', 'u32', 8, 'ScriptState.stack[i].scriptPtr 0x801BE134+8i, normalised', stackEntries('offset'))
derived('script.stack_script_id', 'u16', 8, 'ScriptState.stack[i].scriptId 0x801BE138+8i', stackEntries('id'))
derived('script.stack_smth0', 'u8', 8, 'ScriptState.stack[i].smth[0] 0x801BE13A+8i', stackEntries('smth0'))
derived('script.stack_smth1', 'u8', 8, 'ScriptState.stack[i].smth[1] 0x801BE13B+8i', stackEntries('smth1'))

-- ---- rng ----
abs('rng.custom_val1', 'u32', A.CUSTOM_RNG_VAL1, 'CUSTOM_RNG_VAL1', true)
abs('rng.custom_val2', 'u32', A.CUSTOM_RNG_VAL2, 'CUSTOM_RNG_VAL2', true)
abs('rng.bios_rand_state', 'u32', A.BIOS_RAND_STATE, 'BIOS rand() state (RAM 0x9010)', true)

-- ---- clock ----
abs('clock.year', 'u8', A.YEAR, 'YEAR')
abs('clock.day', 's16', A.DAY, 'DAY')
abs('clock.hour', 's16', A.HOUR, 'HOUR')
abs('clock.minute', 's16', A.MINUTE, 'MINUTE')
abs('clock.current_frame', 'u16', A.CURRENT_FRAME, 'CURRENT_FRAME')
abs('clock.last_handled_frame', 'u16', A.LAST_HANDLED_FRAME, 'LAST_HANDLED_FRAME')
abs('clock.subframe_count', 'u16', A.SUBFRAME_COUNT, 'SUBFRAME_COUNT')
abs('clock.is_gametime_running', 's8', A.IS_GAMETIME_RUNNING, 'IS_GAMETIME_RUNNING')
abs('clock.time_speed', 'u8', A.SCRIPT_STATE + 0x159, 'ScriptState.pstats[0] (PSTAT_TIME_SPEED)')
abs('clock.playtime_frames', 'u16', A.PLAYTIME_FRAMES, 'PLAYTIME_FRAMES')
abs('clock.playtime_minutes', 'u16', A.PLAYTIME_MINUTES, 'PLAYTIME_MINUTES')
abs('clock.playtime_hours', 'u16', A.PLAYTIME_HOURS, 'PLAYTIME_HOURS')

-- ---- tamer ----
abs('tamer.money', 's32', A.MONEY, 'MONEY')
abs('tamer.level', 's8', A.TAMER_ENTITY + 0x38, 'TAMER_ENTITY.tamerLevel')
abs('tamer.raised_count', 'u8', A.TAMER_ENTITY + 0x39, 'TAMER_ENTITY.raisedCount')
abs('tamer.entity_type', 's32', A.TAMER_ENTITY, 'TAMER_ENTITY.entity.type')
derived('tamer.pos_valid', 'u8', 1, 'TAMER_ENTITY.entity.posData (non-null, inside RAM)',
    function() return posValid(A.TAMER_ENTITY) end)
derived('tamer.pos', 's32', 3, '*posData + 0x78 (PositionData.location x,y,z)',
    function() return posXYZ(A.TAMER_ENTITY) end)
derived('tamer.yaw', 's16', 1, '*posData + 0x72 (PositionData.rotation.vy)',
    function() return posYaw(A.TAMER_ENTITY) end)

-- ---- partner ----
local P = A.PARTNER_ENTITY
abs('partner.species', 's32', P, 'PARTNER_ENTITY.entity.type')
derived('partner.pos_valid', 'u8', 1, 'PARTNER_ENTITY.entity.posData (non-null, inside RAM)',
    function() return posValid(P) end)
derived('partner.pos', 's32', 3, '*posData + 0x78 (PositionData.location x,y,z)',
    function() return posXYZ(P) end)
derived('partner.yaw', 's16', 1, '*posData + 0x72 (PositionData.rotation.vy)',
    function() return posYaw(P) end)
abs('partner.base_off', 's16', P + 0x38, 'stats.base.off')
abs('partner.base_def', 's16', P + 0x3A, 'stats.base.def')
abs('partner.base_speed', 's16', P + 0x3C, 'stats.base.speed')
abs('partner.base_brain', 's16', P + 0x3E, 'stats.base.brain')
absArray('partner.moves_prio', 'u8', P + 0x40, 4, 'stats.base.movesPrio')
absArray('partner.moves', 'u8', P + 0x44, 4, 'stats.base.moves')
abs('partner.max_hp', 's16', P + 0x48, 'stats.base.hp')
abs('partner.max_mp', 's16', P + 0x4A, 'stats.base.mp')
abs('partner.cur_hp', 's16', P + 0x4C, 'stats.current.currentHP')
abs('partner.cur_mp', 's16', P + 0x4E, 'stats.current.currentMP')
absArray('partner.learned_moves', 'u32', P + 0x58, 2, 'PARTNER_ENTITY.learnedMoves', true)
absArray('partner.name', 'u8', P + 0x68, 20, 'PARTNER_ENTITY.name (game-encoded)')
abs('partner.lives', 's8', P + 0x7C, 'PARTNER_ENTITY.lives')
-- PartnerPara (care and evolution state). Excluded: unused1..unused12.
local PARA = A.PARTNER_PARA
abs('partner.para.condition', 'u32', PARA + 0x00, 'PARTNER_PARA.condition', true)
for _, f in ipairs({
    { 'sleepy_hour', 0x04 }, { 'sleepy_minute', 0x06 }, { 'wakeup_hour', 0x08 }, { 'wakeup_minute', 0x0A },
    { 'hours_awake_default', 0x0C }, { 'hours_asleep_default', 0x0E }, { 'time_awake_today', 0x10 },
    { 'sickness_counter', 0x12 }, { 'missed_sleep_hours', 0x14 }, { 'tiredness_sleep_timer', 0x16 },
    { 'poop_level', 0x18 }, { 'virus_bar', 0x1E }, { 'pooping_timer', 0x20 }, { 'tiredness', 0x22 },
    { 'sub_tiredness', 0x24 }, { 'tiredness_hunger_timer', 0x26 }, { 'discipline', 0x28 },
    { 'happiness', 0x2A }, { 'times_being_sick', 0x30 }, { 'sickness_tries', 0x32 },
    { 'area_effect_timer', 0x34 }, { 'sickness_timer', 0x36 }, { 'injury_timer', 0x38 },
    { 'next_hunger_hour', 0x3A }, { 'energy_level', 0x3C }, { 'food_level', 0x3E },
    { 'starvation_timer', 0x40 }, { 'weight', 0x42 }, { 'refused_fav_food', 0x44 },
    { 'empty_stomach_timer', 0x46 }, { 'remaining_lifetime', 0x48 }, { 'age', 0x4A },
    { 'train_boost_flag', 0x4C }, { 'train_boost_value', 0x4E }, { 'train_boost_timer', 0x50 },
    { 'care_mistakes', 0x52 }, { 'battles', 0x54 }, { 'evo_timer', 0x56 }, { 'fish_caught', 0x5E },
    { 'upgrade_hp_counter', 0x66 }, { 'upgrade_mp_counter', 0x68 }, { 'upgrade_off_counter', 0x6A },
    { 'bugged_upgrade_brains_counter', 0x6C }, { 'upgrade_defense_counter', 0x6E },
    { 'upgrade_speed_counter', 0x70 }, { 'upgrade_brain_counter', 0x72 },
    { 'suka_backup_hp', 0x76 }, { 'suka_backup_mp', 0x78 }, { 'suka_backup_off', 0x7A },
    { 'suka_backup_def', 0x7C }, { 'suka_backup_speed', 0x7E }, { 'suka_backup_brain', 0x80 },
}) do
    abs('partner.para.' .. f[1], 's16', PARA + f[2], string.format('PARTNER_PARA+0x%02X', f[2]))
end

-- ---- inventory ----
local INV = A.INVENTORY
abs('inventory.size', 'u8', INV + 0x5A, 'INVENTORY.size')
absArray('inventory.item_ids', 'u8', INV + 0x00, 30, 'INVENTORY.types (0xFF empty)')
absArray('inventory.counts', 'u8', INV + 0x1E, 30, 'INVENTORY.amounts')

-- ---- progress ----
local SS = A.SCRIPT_STATE
absArray('progress.cards', 'u8', SS + 0xD4, 33, 'ScriptState.cards')
absArray('progress.triggers', 'u8', SS + 0xF5, 100, 'ScriptState.triggers (800 bits)')
absArray('progress.pstats', 'u8', SS + 0x159, 256, 'ScriptState.pstats')
abs('progress.merit', 's16', A.MERIT, 'MERIT')
abs('progress.battles_started', 's16', A.BATTLES_STARTED, 'BATTLES_STARTED')
abs('progress.battles_fled', 's16', A.BATTLES_FLED, 'BATTLES_FLED')
abs('progress.tournaments_won', 's16', A.TOURNAMENTS_WON, 'TOURNAMENTS_WON')
abs('progress.tournament_wins', 's16', A.TOURNAMENT_WINS, 'TOURNAMENT_WINS')
abs('progress.tournaments_lost', 's16', A.TOURNAMENTS_LOST, 'TOURNAMENTS_LOST')

-- ---- collection / encoding / rendering ----

local function defaultGetBytes()
    if not C._mem then C._mem = ffi.cast('uint8_t*', PCSX.getMemPtr()) end
    return function(off, n) return ffi.string(C._mem + off, n) end
end

-- Reads every field. Returns an ordered list of { field=, value= (number or table) }.
function C.collect(reader)
    getbytes = reader or defaultGetBytes()
    local rec = {}
    for i, f in ipairs(C.fields) do rec[i] = { field = f, value = f.read() } end
    return rec
end

local function pack(v, ty)
    local n = SIZE[ty]
    if v < 0 then v = v + 2 ^ (8 * n) end
    local b = {}
    for i = 1, n do
        b[i] = string.char(v % 256)
        v = math.floor(v / 256)
    end
    return table.concat(b)
end

function C.encodeBody(rec)
    local out = {}
    for _, e in ipairs(rec) do
        local f = e.field
        if f.count == 1 and type(e.value) ~= 'table' then
            out[#out + 1] = pack(e.value, f.ty)
        else
            assert(#e.value == f.count, f.name)
            for _, v in ipairs(e.value) do out[#out + 1] = pack(v, f.ty) end
        end
    end
    return table.concat(out)
end

function C.encode(rec)
    local body = C.encodeBody(rec)
    return 'DW1CANON' .. pack(C.VERSION, 'u32') .. pack(#body, 'u32') .. body
end

local function fmtScalar(v, f)
    if f.hex then return string.format('0x%0' .. (2 * SIZE[f.ty]) .. 'x', v < 0 and v + 2 ^ (8 * SIZE[f.ty]) or v) end
    return string.format('%d', v)
end

function C.render(rec)
    local lines = { string.format('# dw1 canonical state schema v%d', C.VERSION) }
    for _, e in ipairs(rec) do
        local f = e.field
        local s
        if type(e.value) ~= 'table' then
            s = fmtScalar(e.value, f)
        else
            local parts = {}
            for _, v in ipairs(e.value) do
                if f.ty == 'u8' and f.count > 4 then parts[#parts + 1] = string.format('%02x', v)
                else parts[#parts + 1] = fmtScalar(v, f) end
            end
            s = table.concat(parts, ' ')
        end
        lines[#lines + 1] = f.name .. ' = ' .. s
    end
    return table.concat(lines, '\n') .. '\n'
end

-- Offsets of every field in the binary (from the start of the file).
function C.layout()
    local out, off = {}, 16
    for _, f in ipairs(C.fields) do
        local size = SIZE[f.ty] * f.count
        out[#out + 1] = { name = f.name, ty = f.ty, count = f.count, offset = off, size = size, src = f.src }
        off = off + size
    end
    return out, off
end

function C.dump(basename, reader)
    local rec = C.collect(reader)
    local write = DW1ROUTE_WRITE or function(name, data)
        local f = assert(io.open(name, 'wb')); f:write(data); f:close()
    end
    write(basename .. '.bin', C.encode(rec))
    write(basename .. '.txt', C.render(rec))
    return rec
end

function C.layoutMarkdown()
    local rows, total = C.layout()
    local out = { '| Offset | Size | Field | Type | Source |', '|---|---|---|---|---|' }
    for _, r in ipairs(rows) do
        out[#out + 1] = string.format('| 0x%04X | %d | `%s` | %s%s | %s |', r.offset, r.size, r.name, r.ty,
            r.count > 1 and ('[' .. r.count .. ']') or '', r.src)
    end
    out[#out + 1] = string.format('\nTotal: %d bytes (16-byte header + %d body).', total, total - 16)
    return table.concat(out, '\n') .. '\n'
end

return C
