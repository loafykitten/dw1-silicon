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
-- CD reads (format 2, Phase 5): native reads finish at once, but the game keeps running (and
-- polling the pad) while the PS1's drive reads, so how long a read takes changes the game's state
-- (battle loading moves the fighters every frame). Both modes hook libcd's CdRead (0x800B2BB4) and
-- CdReadSync (0x800B2CB4) and count, per read that CdRead started (returned 1), the poll it was
-- issued at, its sector count, and how many CdReadSync calls with a nonzero mode returned "still
-- reading" (> 0) before it finished. A recording writes them as
-- "cd <k> <issue poll> <busy count> <sectors>" lines
-- between the header and the polls; a replay checks its own against them. An error result
-- (< 0) fails the run (never seen). The port's CdReadSync answers "still reading" as many times
-- (crates/psyq/src/libcd.rs). Reads made inside CdSearchFile (0x800B23C0: the SDK reading the
-- disc's directory, which the port's CdSearchFile looks up in the pack instead) aren't counted.
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
-- DW1REPLAY_EVERY changes the every-N-polls checkpoints (the port's DW1_CHECKPOINT_EVERY), to find
-- where a run diverges.
local CHECKPOINT_EVERY = tonumber(os.getenv('DW1REPLAY_EVERY') or '') or 600

local PAD_READ = 0x80091BE8
local CD_READ, CD_READ_SYNC, CD_SEARCH_FILE = 0x800B2BB4, 0x800B2CB4, 0x800B23C0
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
local rec = { vblank = {}, value = {}, ra = {}, cd = {} }
local header = {}
if MODE == 'replay' then
    local f, err = io.open(FILE, 'rb')
    if not f then fail('cannot open ' .. FILE .. ': ' .. tostring(err)) return end
    for line in f:lines() do
        if line:sub(1, 1) ~= '#' and line ~= '' then
            local k, v = line:match('^(%a[%w_]*)%s+(.*)$')
            local seq, vb, val, ra = line:match('^(%d+) (%d+) (%x+) (%x+)$')
            local ck, cissue, cbusy, csectors = line:match('^cd (%d+) (%d+) (%d+) (%d+)$')
            if ck then
                if tonumber(ck) ~= #rec.cd then fail('replay file: cd lines out of order at ' .. ck) return end
                rec.cd[#rec.cd + 1] = { issue = tonumber(cissue), busy = tonumber(cbusy), sectors = tonumber(csectors) }
            elseif seq then
                local n = tonumber(seq)
                if n ~= #rec.value then fail('replay file: poll lines out of order at ' .. seq) return end
                rec.vblank[n + 1], rec.value[n + 1], rec.ra[n + 1] = tonumber(vb), tonumber(val, 16), tonumber(ra, 16)
            elseif k then
                header[k] = v
            end
        end
    end
    f:close()
    local fmt = tonumber(header.format)
    if fmt ~= 1 and fmt ~= 2 then fail('replay file: unsupported format ' .. tostring(header.format)) return end
    -- Format 1 recordings predate the CD lines: their reads aren't checked.
    rec.checkCd = fmt == 2
    if fmt == 2 and tonumber(header.cd_reads) ~= #rec.cd then fail('replay file: header says ' .. tostring(header.cd_reads) .. ' CD reads, found ' .. #rec.cd) return end
    if tonumber(header.polls) ~= #rec.value then fail('replay file: header says ' .. tostring(header.polls) .. ' polls, found ' .. #rec.value) return end
    if tonumber(header.final_vblank) == nil then fail('replay file: no final_vblank') return end
    -- A two-player recording (Battle Mode) had pad 2 plugged in: so must the replay, or the SIO
    -- traffic (and with it the CPU's timing) differs. Recordings without the line had it out.
    if header.pad2 == 'connected' then
        PCSX.settings.pads[2].Connected = true
    elseif header.pad2 ~= nil then
        fail('replay file: unknown pad2 ' .. tostring(header.pad2)) return
    end
end

-- The cards the run starts with: Redux reads memcard1.mcd and memcard2.mcd from its cwd, so
-- they are hashed here, before anything runs ('fresh' = the new card Redux makes itself when
-- there is none, SHA-1 ec2d2493...). A recording stores them (card1_sha1, card2_sha1, and the fixture's name in
-- DW1REPLAY_CARD1_FIXTURE for a card); a replay refuses to run on any other card.
local REDUX_FRESH_CARD = 'ec2d2493fa3b5f0a546b7b224ddef5b143771219'
ffi.cdef('unsigned char* CC_SHA1(const void* data, uint32_t len, unsigned char* md);')
local function cardSha(name)
    local f = io.open(name, 'rb')
    if not f then return 'absent' end
    local data = f:read('*a')
    f:close()
    local md = ffi.new('unsigned char[20]')
    ffi.C.CC_SHA1(data, #data, md)
    local hex = {}
    for i = 0, 19 do hex[#hex + 1] = string.format('%02x', md[i]) end
    local sha = table.concat(hex)
    return sha == REDUX_FRESH_CARD and 'fresh' or sha
end
local CARD1_SHA, CARD2_SHA = cardSha('memcard1.mcd'), cardSha('memcard2.mcd')
local CARD1_FIXTURE = os.getenv('DW1REPLAY_CARD1_FIXTURE')
print('DW1REPLAY cards card1_sha1=' .. CARD1_SHA .. ' card2_sha1=' .. CARD2_SHA)
if MODE == 'record' and CARD1_SHA ~= 'fresh' and not CARD1_FIXTURE then
    fail('recording on a card needs DW1REPLAY_CARD1_FIXTURE=<name> (a fixture whose SHA-1 is known)')
    return
end
if MODE == 'replay' then
    for _, c in ipairs({ { 'card1_sha1', CARD1_SHA }, { 'card2_sha1', CARD2_SHA } }) do
        if header[c[1]] ~= c[2] then
            fail(string.format('%s: the recording has %s, this run starts with %s', c[1], tostring(header[c[1]]), c[2]))
            return
        end
    end
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
local entryBp, cdReadBp, cdSyncBp, cdSearchBp
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

-- ---- CD read hooks (format 2) ----
local cdReads = {}        -- { issue = poll, busy = count } per read CdRead started
local cdReadRa, cdSyncRa, cdSyncMode, cdReadSectors = nil, nil, nil, nil
local cdBps = {}          -- persistent return breakpoints, by function and return address
local cdSearchRa = nil    -- set while CdSearchFile runs (its own directory reads aren't counted)

local function cdReturn(kind, ra)
    return guard(function()
        if finished or failed then return true end
        local v0 = tonumber(PCSX.getRegisters().GPR.n.v0)
        if kind == 'search' then
            if cdSearchRa == ra then cdSearchRa = nil end
            return true
        end
        if cdSearchRa then return true end
        if kind == 'read' then
            if cdReadRa ~= ra then return true end
            cdReadRa = nil
            if v0 == 1 then cdReads[#cdReads + 1] = { issue = polls, busy = 0, sectors = cdReadSectors } end
        else
            if cdSyncRa ~= ra then return true end
            cdSyncRa = nil
            if v0 >= 0x80000000 then fail(string.format('CdReadSync returned an error (%d) at poll %d', v0 - 0x100000000, polls)) return true end
            if cdSyncMode ~= 0 and v0 > 0 then
                if #cdReads == 0 then fail('CdReadSync busy before any CdRead') return true end
                cdReads[#cdReads].busy = cdReads[#cdReads].busy + 1
            end
        end
        return true
    end)
end

local function cdEntry(kind)
    return guard(function()
        if finished or failed then return true end
        local r = PCSX.getRegisters().GPR.n
        local ra = tonumber(r.ra)
        if kind == 'search' then
            if cdSearchRa then fail('CdSearchFile re-entered') return true end
            cdSearchRa = ra
        elseif cdSearchRa then
            return true
        elseif kind == 'read' then
            if cdReadRa then fail('CdRead re-entered') return true end
            cdReadRa, cdReadSectors = ra, tonumber(r.a0)
        else
            if cdSyncRa then fail('CdReadSync re-entered') return true end
            cdSyncRa, cdSyncMode = ra, tonumber(r.a0)
        end
        local key = kind .. ra
        if not cdBps[key] then
            cdBps[key] = PCSX.addBreakpoint(ra, 'Exec', 4, string.format('%s return %08x', kind, ra), cdReturn(kind, ra))
        end
        return true
    end)
end

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
    -- Zero polls is fine for targets before the game first polls the pad (VBlank 1651 on the
    -- new-game route); replay mode checks the poll count against the recording separately.
    if nret == 0 then print('DW1REPLAY note: no PadRead call was seen before the target') end
    if pendingRa then return fail(string.format('PadRead (caller 0x%08x) still pending at the end', pendingRa)) end
    if not cdReadBp:isEnabled() or not cdSyncBp:isEnabled() or not cdSearchBp:isEnabled() then
        return fail('a CD entry breakpoint is gone')
    end
    if cdSearchRa then return fail('CdSearchFile still running at the end') end
    for key, bp in pairs(cdBps) do
        if not bp:isEnabled() then return fail('CD return breakpoint ' .. key .. ' is gone') end
    end
    entryBp:remove()
    cdReadBp:remove()
    cdSyncBp:remove()
    cdSearchBp:remove()
    local cdLines = {}
    for k, r in ipairs(cdReads) do cdLines[k] = string.format('cd %d %d %d %d\n', k - 1, r.issue, r.busy, r.sectors) end
    if MODE == 'record' then
        local hdr = {
            '# dw1-silicon pad replay: PadRead (0x80091BE8) results keyed to poll sequence number.',
            '# Pad states only, no game data. Lines after the header: CD reads "cd k issue_poll busy_count sectors",',
            '# then polls "seq vblank mask(hex) caller_ra(hex)".',
            'format 2',
            'redux ' .. REDUX_COMMIT,
            'core interpreter',
            'gpu softgpu',
            'bios_sha1 ' .. BIOS_SHA1,
            'disc_sha1 ' .. DISC_SHA1,
            CARD1_FIXTURE and ('card1_fixture ' .. CARD1_FIXTURE) or '# card1_fixture none (the card Redux makes)',
            'card1_sha1 ' .. CARD1_SHA,
            'card2_sha1 ' .. CARD2_SHA,
            'route oracle/route-new-game.lua profile ' .. (DW1ROUTE_PROFILE or os.getenv('DW1ROUTE_PROFILE') or 'new-game'),
            'target ' .. TARGET,
            'final_vblank ' .. v,
            'polls ' .. #log,
            'cd_reads ' .. #cdReads,
        }
        -- Only written when pad 2 was plugged in, so one-player recordings stay as they were.
        if PCSX.settings.pads[2].Connected then table.insert(hdr, 'pad2 connected') end
        write(OUT, table.concat(hdr, '\n') .. '\n' .. table.concat(cdLines) .. table.concat(log))
        print(string.format('DW1REPLAY recorded polls=%d cd_reads=%d final_vblank=%d checkpoints=%d out=%s', #log, #cdReads, v, ckpt, OUT))
    else
        local want = #rec.value
        print(string.format('DW1REPLAY replayed polls=%d of %d final_vblank=%d checkpoints=%d vblank_mismatches=%d',
            polls, want, v, ckpt, vblankMismatches))
        if firstVblankMismatch then print('DW1REPLAY first vblank mismatch: ' .. firstVblankMismatch) end
        print('DW1REPLAY vblanks_aligned=' .. (vblankMismatches == 0 and 'yes' or 'NO'))
        if polls ~= want then
            return fail(string.format('divergence: %d polls consumed, %d recorded', polls, want))
        end
        if rec.checkCd then
            if #cdReads ~= #rec.cd then
                return fail(string.format('CD reads: %d here, %d recorded', #cdReads, #rec.cd))
            end
            for k, r in ipairs(cdReads) do
                local w = rec.cd[k]
                if r.issue ~= w.issue or r.busy ~= w.busy or r.sectors ~= w.sectors then
                    return fail(string.format('CD read %d: poll %d busy %d sectors %d, recorded %d busy %d sectors %d', k - 1, r.issue, r.busy, r.sectors, w.issue, w.busy, w.sectors))
                end
            end
            print(string.format('DW1REPLAY cd_reads=%d match the recording', #cdReads))
        end
    end
    succeed()
end

-- Turn the debugger on before anything runs (breakpoints are inert without it).
PCSX.settings.emulator.Debug.Debug = true
entryBp = PCSX.addBreakpoint(PAD_READ, 'Exec', 4, 'PadRead', onEntry)
cdReadBp = PCSX.addBreakpoint(CD_READ, 'Exec', 4, 'CdRead', cdEntry('read'))
cdSyncBp = PCSX.addBreakpoint(CD_READ_SYNC, 'Exec', 4, 'CdReadSync', cdEntry('sync'))
cdSearchBp = PCSX.addBreakpoint(CD_SEARCH_FILE, 'Exec', 4, 'CdSearchFile', cdEntry('search'))

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
