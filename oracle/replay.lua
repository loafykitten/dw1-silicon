-- Oracle pad replays keyed to pad polls (S0.8). docs/s08-oracle.md has the certification results.
--
-- The game reads the controller only through libetc PadRead (0x80091BE8; 14 call sites). Both
-- modes hook PadRead's entry (to read $ra) and keep a persistent breakpoint on each distinct
-- caller's return address (at most 14) to see v0 at return; the entry hook arms the one
-- matching its $ra. Breakpoints only fire with the debugger on, so the script switches it on from Lua
-- (don't pass -debugger).
--
-- Mode (global DW1REPLAY_MODE or env DW1REPLAY_MODE; default 'record'):
--   record  Plays the new-game route (oracle/route-new-game.lua, whose pad overrides feed the
--           game) up to DW1REPLAY_TARGET (default 'gameplay'), logging every PadRead return:
--           poll sequence number, VBlank number, returned mask, caller return address. Writes
--           the replay to $DW1REPLAY_OUT (default ./replay.txt).
--   replay  No pad overrides at all. Reads $DW1REPLAY_FILE (default oracle/replays/new-game.txt)
--           and at poll k's return overwrites v0 with the recorded mask. A different caller
--           address, or a poll after the recorded ones, fails the run with exit 1 and the
--           sequence number. VBlank numbers are compared with the recording and reported (they
--           should match; Redux is deterministic) but a mismatch is not an error by itself.
--           Stops at the recorded final VBlank and fails if the poll count differs.
--
-- Both modes write canonical state dumps (oracle/canonical-state.lua) into the cwd as
-- ckpt_NNNN_<why>.bin/.txt: at every change of MAIN_STATE, GAME_STATE or CURRENT_SCREEN (checked
-- at each poll return), every 600 polls, and "final". Run from a fresh empty directory under
-- captures/oracle-runs/ (see docs/s08-oracle.md for the command line).
-- The run only exits 0 through the real completion path (which first checks that the hooks are
-- still alive: debugger on, entry and return breakpoints enabled, no PadRead pending); a Quitting
-- event before that forces exit 1.
-- Output lines start with "DW1REPLAY".

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end
local HERE = here()

local MODE = DW1REPLAY_MODE or os.getenv('DW1REPLAY_MODE') or 'record'
local TARGET = DW1REPLAY_TARGET or os.getenv('DW1REPLAY_TARGET') or 'gameplay'
local OUT = os.getenv('DW1REPLAY_OUT') or 'replay.txt'
local FILE = os.getenv('DW1REPLAY_FILE') or (HERE .. '/replays/new-game.txt')
local CHECKPOINT_EVERY = 600

local PAD_READ = 0x80091BE8
local REDUX_COMMIT = '26bfd693'
local BIOS_SHA1 = '649895efd79d14790eabb362e94eb0622093dfb9'
local DISC_SHA1 = '5611645da66183e30d11fb6c11a2784ff47e1fdb'

local failed, succeeded = false, false
local function fail(msg)
    if failed then return end
    failed = true
    if DW1ROUTE_FAIL then DW1ROUTE_FAIL(msg) else print('DW1ROUTE error: ' .. msg); PCSX.quit(1) end
    print('DW1REPLAY FAIL ' .. msg)
end
local function succeed()
    if failed then return end
    succeeded = true
    if DW1ROUTE_SUCCEED then DW1ROUTE_SUCCEED() else PCSX.quit(0) end
end
local function write(name, data)
    local f, err = io.open(name, 'wb')
    if not f then return fail(err) end
    local ok, werr = f:write(data)
    local cok, cerr = f:close()
    if not ok or not cok then fail(name .. ': ' .. tostring(werr or cerr)) end
end

local C = dofile(HERE .. '/canonical-state.lua')
local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function ru8(addr) return mem[addr - 0x80000000] end

-- Replay data (replay mode).
local rec = { vblank = {}, value = {}, ra = {} }
local header = {}
if MODE == 'replay' then
    local f, err = io.open(FILE, 'rb')
    if not f then fail('cannot open ' .. FILE .. ': ' .. tostring(err)) return end
    for line in f:lines() do
        if line:sub(1, 1) ~= '#' and line ~= '' then
            local k, v = line:match('^(%a[%w_]*)%s+(.*)$')
            local seq, vb, val, ra = line:match('^(%d+) (%d+) (%x+) (%x+)$')
            if seq then
                local n = tonumber(seq)
                if n ~= #rec.value then fail('replay file: poll lines out of order at ' .. seq) return end
                rec.vblank[n + 1], rec.value[n + 1], rec.ra[n + 1] = tonumber(vb), tonumber(val, 16), tonumber(ra, 16)
            elseif k then
                header[k] = v
            end
        end
    end
    f:close()
    if tonumber(header.format) ~= 1 then fail('replay file: unsupported format ' .. tostring(header.format)) return end
    if tonumber(header.polls) ~= #rec.value then fail('replay file: header says ' .. tostring(header.polls) .. ' polls, found ' .. #rec.value) return end
    if tonumber(header.final_vblank) == nil then fail('replay file: no final_vblank') return end
end

-- Run state.
local vblank = 0
local polls = 0           -- completed polls so far == sequence number of the next poll
local log = {}            -- record mode: "seq vblank value ra" lines
local vblankMismatches, firstVblankMismatch = 0, nil
local ckpt = 0
local lastMain, lastGame, lastScreen = nil, nil, nil
local finished = false

local function checkpoint(why)
    local name = string.format('ckpt_%04d_%s', ckpt, why)
    ckpt = ckpt + 1
    local ok, err = pcall(C.dump, name)
    if not ok then return fail('canonical dump ' .. name .. ': ' .. tostring(err)) end
    print(string.format('DW1REPLAY checkpoint %s poll=%d vblank=%d', name, polls, vblank))
end

-- Checkpoint triggers, evaluated at each poll return (before the poll count is advanced, so the
-- label carries the sequence number of the poll that noticed).
local function pollCheckpoints()
    local main, game, screen = ru8(C.ADDR.MAIN_STATE), ru8(C.ADDR.GAME_STATE), ru8(C.ADDR.CURRENT_SCREEN)
    if main ~= lastMain then
        checkpoint(string.format('main%d', main)); lastMain = main
    end
    if game ~= lastGame then
        checkpoint(string.format('game%d', game)); lastGame = game
    end
    if screen ~= lastScreen then
        checkpoint(string.format('screen%d', screen)); lastScreen = screen
    end
    if polls > 0 and polls % CHECKPOINT_EVERY == 0 then checkpoint(string.format('every%d', polls)) end
end

-- ---- PadRead hooks ----
-- Redux makes an ffi callback for every Lua breakpoint, and LuaJIT allows only about 2000 of
-- them in a process, so the return breakpoints are persistent: one per distinct caller address
-- (at most the 14 call sites), created the first time that caller is seen. A return breakpoint
-- acts only when its address is the one the pending call will return to. Handles are kept
-- referenced for the whole run: Redux removes a Lua breakpoint when its handle is
-- garbage-collected.
local entryBp
local returnBps = {}
local pendingRa = nil -- the caller address of the PadRead call whose return we are waiting for

-- A Lua error inside a breakpoint callback makes Redux silently delete the breakpoint (the game
-- then runs on unobserved), so turn it into a failed run.
local function guard(fn)
    return function(...)
        local ok, res = xpcall(fn, debug.traceback, ...)
        if not ok then fail('error in breakpoint callback: ' .. tostring(res)) return true end
        return res
    end
end

local function onReturn(ra)
    return guard(function()
        if finished or failed or pendingRa ~= ra then return true end
        local v0 = tonumber(PCSX.getRegisters().GPR.n.v0)
        local seq = polls
        if MODE == 'replay' then
            local want = rec.value[seq + 1]
            if want == nil then fail(string.format('divergence: poll %d beyond the %d recorded polls (vblank %d)', seq, #rec.value, vblank)) return true end
            if rec.vblank[seq + 1] ~= vblank then
                vblankMismatches = vblankMismatches + 1
                firstVblankMismatch = firstVblankMismatch or string.format('poll %d recorded vblank %d, got %d', seq, rec.vblank[seq + 1], vblank)
            end
            PCSX.getRegisters().GPR.n.v0 = want
            v0 = want
        else
            log[#log + 1] = string.format('%d %d %08x %08x\n', seq, vblank, v0, ra)
        end
        pollCheckpoints()
        polls = polls + 1
        pendingRa = nil
        return true
    end)
end

local onEntry = guard(function()
    if finished or failed then return true end
    local ra = tonumber(PCSX.getRegisters().GPR.n.ra)
    local seq = polls
    if pendingRa then
        fail(string.format('PadRead re-entered before the previous call returned (poll %d)', seq)) return true
    end
    if MODE == 'replay' then
        local wantRa = rec.ra[seq + 1]
        if wantRa == nil then
            fail(string.format('divergence: poll %d beyond the %d recorded polls (vblank %d, caller 0x%08x)', seq, #rec.value, vblank, ra)) return true
        end
        if ra ~= wantRa then
            fail(string.format('divergence at poll %d: caller 0x%08x, recorded 0x%08x (vblank %d)', seq, ra, wantRa, vblank)) return true
        end
    end
    if not returnBps[ra] then
        returnBps[ra] = PCSX.addBreakpoint(ra, 'Exec', 4, string.format('PadRead return %08x', ra), onReturn(ra))
    end
    pendingRa = ra
    return true
end)

-- ---- run control ----
local function finish(v)
    if finished then return end
    finished = true
    checkpoint('final')
    if failed then return end
    -- Hook health: a deleted breakpoint or a disabled debugger would leave a partial recording
    -- that still looks successful.
    if not PCSX.settings.emulator.Debug.Debug then return fail('debugger was switched off during the run') end
    if not entryBp or not entryBp:isEnabled() then return fail('PadRead entry breakpoint is gone') end
    local nret = 0
    for ra, bp in pairs(returnBps) do
        nret = nret + 1
        if not bp:isEnabled() then return fail(string.format('return breakpoint %08x is gone', ra)) end
    end
    if nret == 0 then return fail('no PadRead call was ever seen') end
    if pendingRa then return fail(string.format('PadRead (caller 0x%08x) still pending at the end', pendingRa)) end
    entryBp:remove()
    if MODE == 'record' then
        local hdr = {
            '# dw1-silicon pad replay: PadRead (0x80091BE8) results keyed to poll sequence number.',
            '# Pad states only, no game data. Lines after the header: seq vblank mask(hex) caller_ra(hex).',
            'format 1',
            'redux ' .. REDUX_COMMIT,
            'core interpreter',
            'gpu softgpu',
            'bios_sha1 ' .. BIOS_SHA1,
            'disc_sha1 ' .. DISC_SHA1,
            'route oracle/route-new-game.lua',
            'target ' .. TARGET,
            'final_vblank ' .. v,
            'polls ' .. #log,
        }
        write(OUT, table.concat(hdr, '\n') .. '\n' .. table.concat(log))
        print(string.format('DW1REPLAY recorded polls=%d final_vblank=%d checkpoints=%d out=%s', #log, v, ckpt, OUT))
    else
        local want = #rec.value
        print(string.format('DW1REPLAY replayed polls=%d of %d final_vblank=%d checkpoints=%d vblank_mismatches=%d',
            polls, want, v, ckpt, vblankMismatches))
        if firstVblankMismatch then print('DW1REPLAY first vblank mismatch: ' .. firstVblankMismatch) end
        print('DW1REPLAY vblanks_aligned=' .. (vblankMismatches == 0 and 'yes' or 'NO'))
        if polls ~= want then
            return fail(string.format('divergence: %d polls consumed, %d recorded', polls, want))
        end
    end
    succeed()
end

-- Turn the debugger on before anything runs (breakpoints are inert without it).
PCSX.settings.emulator.Debug.Debug = true
entryBp = PCSX.addBreakpoint(PAD_READ, 'Exec', 4, 'PadRead', onEntry)

if MODE == 'record' then
    DW1ROUTE_TARGET = TARGET
    DW1ROUTE_ON_TARGET = function(v)
        if v ~= vblank then return fail(string.format('VBlank counters disagree: route %d, ours %d', v, vblank)) end
        local ok, err = xpcall(finish, debug.traceback, vblank)
        if not ok then fail('error finishing: ' .. tostring(err)) end
    end
    -- One VBlank counter: ours, registered before the route's listener so it has advanced
    -- before the route's callbacks (which can end the run) run for the same event.
    KEEP_REPLAY_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function() vblank = vblank + 1 end)
    dofile(HERE .. '/route-new-game.lua')
elseif MODE == 'replay' then
    local finalVblank = tonumber(header.final_vblank)
    KEEP_REPLAY_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
        vblank = vblank + 1
        if vblank == finalVblank then
            local ok, err = xpcall(finish, debug.traceback, vblank)
            if not ok then fail('error finishing: ' .. tostring(err)) end
        end
    end)
    print(string.format('DW1REPLAY replaying %s: %d polls, final vblank %d, target %s', FILE, #rec.value, finalVblank, tostring(header.target)))
else
    fail('unknown DW1REPLAY_MODE ' .. tostring(MODE))
end

-- Closing Redux or quitting before the real completion path must not look like success.
KEEP_REPLAY_QUIT = PCSX.Events.createEventListener('Quitting', function()
    if not succeeded then
        print('DW1REPLAY FAIL quit before the run completed')
        PCSX.quit(1)
    end
end)
