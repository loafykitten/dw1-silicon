-- Oracle route: boot Digimon World (USA) and play a NEW GAME up to the first frame of normal
-- gameplay (File City, HUD up, after the intro, naming and Jijimon's dialogue).
--
-- Run from a fresh, empty directory (Redux writes config and memory cards into the cwd):
--   R=/Users/fenn/Developer/dw1-silicon
--   "$R/ref/pcsx-redux/pcsx-redux" -cli -portable -interpreter -softgpu -bios "$R/scph1001.bin" \
--       -iso "$R/disc/Digimon World (USA).cue" -dofile "$R/oracle/route-new-game.lua" -run
-- Writes NAME.raw/NAME.txt screenshots into the cwd (convert with oracle/raw2png.py).
--
-- Inputs are keyed to VBlank numbers (Redux is deterministic). See docs/oracle-route.md.
-- Output lines: "DW1ROUTE milestone=<name> vblank=<n>", "DW1ROUTE reached <target> vblank=<n>".

-- PS1 pad bits as PCSX-Redux numbers them.
local HERE = (debug.getinfo(1, 'S').source:gsub('^@', ''):match('^(.*)/[^/]*$')) or '.'
local B = { SELECT = 0, START = 3, UP = 4, RIGHT = 5, DOWN = 6, LEFT = 7, L2 = 8, R2 = 9,
            L1 = 10, R1 = 11, TRIANGLE = 12, CIRCLE = 13, CROSS = 14, SQUARE = 15 }

-- Input schedule: { first_vblank, last_vblank, period, hold_vblanks, button }.
-- A press starts at first, first+period, ... (<= last) and is held for `hold` VBlanks.
local PRESSES = {
    { 700, 1900, 150, 4, 'START' },   -- skip the BIOS-to-title FMVs/logos
    { 2400, 2400, 1, 4, 'CROSS' },    -- title menu: NEW GAME
    { 2600, 2600, 1, 4, 'CROSS' },    -- START SLOT 1
    { 3000, 3000, 1, 4, 'CROSS' },    -- save slot 1
    { 3200, 3200, 1, 4, 'CROSS' },    -- "Record new adventure?" Yes
    -- Jijimon's questions, name entry (cursor starts on A: six presses type AAAAAA, then the
    -- cursor jumps to OK), intro FMV, File City and Jijimon's house dialogue, 4-VBlank taps 60
    -- apart.
    { 3600, 19100, 60, 4, 'CROSS' },
    -- First gameplay frame is 19100; the partner stands behind the tamer there, so walk Down a
    -- few steps so both are clearly on screen.
    { 19300, 19300, 1, 90, 'DOWN' },
}

-- Milestones: { vblank, name }. A screenshot NAME.raw/.txt is taken at that VBlank (after the
-- frame's input changes) and a DW1ROUTE line printed. The last one is the target.
local MILESTONES = {
    { 800, 'bios_logo' },
    { 2350, 'title_menu' },
    { 2500, 'memcard_select' },
    { 3000, 'save_slot_select' },
    { 3190, 'record_new_adventure' },
    { 3700, 'jijimon_intro_dialogue' },
    { 4400, 'name_entry' },
    { 6000, 'intro_fmv' },
    { 13000, 'caption_file_city' },
    { 14000, 'first_partner_dialogue' },
    { 16000, 'caption_jijimons_house' },
    { 18000, 'house_dialogue' },
    { 18300, 'lets_go_dialogue' },
    { 18700, 'fade_to_gameplay' },
    { 19100, 'gameplay_start' },
    { 19400, 'gameplay' },
}
-- Another script can stop the route elsewhere: set DW1ROUTE_TARGET (a milestone name) and
-- DW1ROUTE_ON_TARGET (called at that VBlank instead of quitting) before dofile'ing this one.
-- DW1ROUTE_PROFILE=continue (env or global) swaps in the CONTINUE GAME schedule, for a fixture
-- card in the cwd's memcard1.mcd (see docs/oracle-route.md, "Continue route"); file-city-walk and
-- care-fixture extend new-game and continue; continue-save2 loads the card's second save and
-- battle-fixture (Phase 5) extends it, battle-twice extends that.
local PROFILE = DW1ROUTE_PROFILE or os.getenv('DW1ROUTE_PROFILE') or 'new-game'
-- Profiles that continue a base profile with more input: the extra file returns PRESSES and
-- MILESTONES to append (its last milestone becomes the target). Both are Phase 4 routes.
local EXTRA = {
    ['file-city-walk'] = { base = 'new-game', file = '/route-file-city-walk-schedule.lua' },
    ['care-fixture'] = { base = 'continue', file = '/route-care-schedule.lua' },
    ['keeper-fixture'] = { base = 'continue', file = '/route-keeper-schedule.lua' },
    ['register-save'] = { base = 'continue', file = '/route-register-save-schedule.lua' },
    ['debug-room'] = { base = 'continue', file = '/route-debug-room-schedule.lua' },
    ['debug-evo'] = { base = 'continue',
                      file = { '/route-debug-room-schedule.lua', '/route-debug-evo-schedule.lua' } },
    ['debug-death'] = { base = 'continue',
                        file = { '/route-debug-room-schedule.lua', '/route-debug-death-schedule.lua' } },
    -- Phase 9, stream tour: Mr. Warp's destinations from the debug room, and walks from the
    -- downloaded saves.
    ['tour-panorama'] = { base = 'continue',
                          file = { '/route-debug-room-schedule.lua', '/route-tour-panorama-schedule.lua' } },
    ['tour-jungle'] = { base = 'continue',
                        file = { '/route-debug-room-schedule.lua', '/route-tour-jungle-schedule.lua' } },
    ['tour-drill'] = { base = 'continue',
                       file = { '/route-debug-room-schedule.lua', '/route-tour-drill-schedule.lua' } },
    ['battle-fixture'] = { base = 'continue-save2', file = '/route-battle-schedule.lua' },
    ['training-fixture'] = { base = 'continue-save2', file = '/route-training-schedule.lua' },
    ['training-bonus'] = { base = 'continue-save2',
                           file = { '/route-training-schedule.lua', '/route-training-bonus-schedule.lua' } },
    ['training-all'] = { base = 'continue-save2',
                         file = { '/route-training-schedule.lua', '/route-training-bonus-schedule.lua',
                                  '/route-training-all-schedule.lua' } },
    ['training-six'] = { base = 'continue-save2',
                         file = { '/route-training-schedule.lua', '/route-training-bonus-schedule.lua',
                                  '/route-training-all-schedule.lua', '/route-training-six-schedule.lua' } },
    ['battle-twice'] = { base = 'continue-save2',
                         file = { '/route-battle-schedule.lua', '/route-battle-twice-schedule.lua' } },
}
local base = PROFILE
if EXTRA[PROFILE] then base = EXTRA[PROFILE].base end
if base == 'continue' then
    PRESSES, MILESTONES = dofile(CONTINUE_SCHEDULE_FILE or (HERE .. '/route-continue-schedule.lua'))
elseif base == 'continue-save2' then
    PRESSES, MILESTONES = dofile(HERE .. '/route-continue-save2-schedule.lua')
elseif base ~= 'new-game' then
    error('unknown DW1ROUTE_PROFILE ' .. tostring(PROFILE))
end
if EXTRA[PROFILE] then
    local files = EXTRA[PROFILE].file
    if type(files) == 'string' then files = { files } end
    for _, file in ipairs(files) do
        local morePresses, moreMilestones = dofile(HERE .. file)
        for _, p in ipairs(morePresses) do PRESSES[#PRESSES + 1] = p end
        for _, m in ipairs(moreMilestones) do MILESTONES[#MILESTONES + 1] = m end
    end
end
local TARGET = DW1ROUTE_TARGET or MILESTONES[#MILESTONES][2]

local RAND_ADDR = 0x9010 -- BIOS rand() state (see oracle/probe-boot.lua)

local mem = ffi.cast('uint32_t*', PCSX.getMemPtr())
local pad = PCSX.SIO0.slots[1].pads[1]
-- Presses named P2_<button> (P2_CROSS) go to the second pad (Battle Mode's player 2).
local pad2 = PCSX.SIO0.slots[2].pads[1]

-- Expand PRESSES into press/release events keyed by VBlank: each event is {pad, bit}.
local pressAt, releaseAt = {}, {}
-- DW1ROUTE_PRESS_UNTIL=<vblank>: ignore presses scheduled after it (to watch an idle menu).
local PRESS_UNTIL = tonumber(os.getenv('DW1ROUTE_PRESS_UNTIL') or '') or math.huge
for _, p in ipairs(PRESSES) do
    local first, last, period, hold, name = p[1], p[2], p[3], p[4], p[5]
    last = math.min(last, PRESS_UNTIL)
    local p = pad
    if name:sub(1, 3) == 'P2_' then p, name = pad2, name:sub(4) end
    local bit = B[name]
    assert(bit, 'unknown button ' .. name)
    for t = first, last, period do
        pressAt[t] = pressAt[t] or {}
        table.insert(pressAt[t], { p, bit })
        releaseAt[t + hold] = releaseAt[t + hold] or {}
        table.insert(releaseAt[t + hold], { p, bit })
    end
end

local milestoneAt, targetKnown = {}, false
for _, m in ipairs(MILESTONES) do
    milestoneAt[m[1]] = m[2]
    if m[2] == TARGET then targetKnown = true end
end

-- Stops the run with exit status 1: a run that can't finish its job must not look successful.
-- PCSX.quit returns to the script, so DW1ROUTE_FAILED keeps a later DW1ROUTE_SUCCEED from
-- turning the failure into exit status 0.
DW1ROUTE_FAILED = false
function DW1ROUTE_FAIL(msg)
    print('DW1ROUTE error: ' .. msg)
    DW1ROUTE_FAILED = true
    PCSX.quit(1)
end
function DW1ROUTE_SUCCEED()
    if not DW1ROUTE_FAILED then PCSX.quit(0) end
end
if not targetKnown then DW1ROUTE_FAIL('unknown target milestone ' .. tostring(TARGET)) end

-- Writes `data` to a new file `name`, failing the run if any step fails.
function DW1ROUTE_WRITE(name, data)
    local f, err = io.open(name, 'wb')
    if not f then return DW1ROUTE_FAIL(err) end
    local ok, werr = f:write(data)
    local cok, cerr = f:close()
    if not ok or not cok then DW1ROUTE_FAIL(name .. ': ' .. tostring(werr or cerr)) end
end

local function shot(name)
    local ss = PCSX.GPU.takeScreenShot()
    DW1ROUTE_WRITE(name .. '.raw', ffi.string(ss.data.data, ss.data.size))
    DW1ROUTE_WRITE(name .. '.txt',
        string.format('%d %d %d', tonumber(ss.width), tonumber(ss.height), tonumber(ss.bpp)))
end

local vblank = 0
KEEP_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    -- oracle/explore.lua takes over the pad at the target and sets this: no scheduled press or
    -- milestone may fire after it.
    if DW1ROUTE_DETACHED then return end
    vblank = vblank + 1
    if releaseAt[vblank] then for _, b in ipairs(releaseAt[vblank]) do b[1].clearOverride(b[2]) end end
    if pressAt[vblank] then for _, b in ipairs(pressAt[vblank]) do b[1].setOverride(b[2]) end end

    local m = milestoneAt[vblank]
    if m then
        shot(m)
        print(string.format('DW1ROUTE milestone=%s vblank=%d', m, vblank))
        if m == TARGET then
            print(string.format('DW1ROUTE reached %s vblank=%d', m, vblank))
            print(string.format('DW1ROUTE rand_state=0x%08x vblank=%d', mem[RAND_ADDR / 4], vblank))
            if DW1ROUTE_ON_TARGET then DW1ROUTE_ON_TARGET(vblank) else DW1ROUTE_SUCCEED() end
        end
    end
end)
print('DW1ROUTE start')
