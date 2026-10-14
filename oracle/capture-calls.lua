-- Phase 2 capture: play the new-game route and record chosen SDK calls in a window of VBlanks
-- (arguments, return values, and optionally the whole machine state at entry and return),
-- plus screenshots and VRAM. Generalises capture-libgs.lua (S0.7) to any function in the
-- decomp's symbol list. The SDK is black-box here: we watch calls, never read its code.
--
-- Run from a fresh, empty directory under captures/ (the dumps hold game data):
--   R=/Users/fenn/Developer/dw1-silicon
--   DW1_SHOTS=700:2350:25 "$R/ref/pcsx-redux/pcsx-redux" -cli -portable -interpreter \
--       -softgpu -bios "$R/scph1001.bin" -iso "$R/disc/Digimon World (USA).cue" \
--       -dofile "$R/oracle/capture-calls.lua" -run > run.log
--
-- Env (all optional):
--   DW1_SHOTS=first:last:step   screenshots shot_<vblank>.raw/.txt (oracle/raw2png.py)
--   DW1_CAPTURE_FUNCS=A,B,...   functions to record (names from decomp/config/us/symbols.txt)
--   DW1_CAPTURE_FROM=<vblank>   arm the breakpoints at this VBlank (default 2000)
--   DW1_CAPTURE_VBLANKS=<n>     window length (default 2)
--   DW1_CAPTURE_MAX=<n>         most calls recorded (default 400)
--   DW1_CAPTURE_STATES=1        also write callNNNN_in.bin / callNNNN_out.bin (DW1STATE format,
--                               see capture-libgs.lua; 2 MiB each)
--   DW1_CAPTURE_VRAM=0          skip the per-VBlank VRAM dumps (long windows)
--   DW1_CAPTURE_DEREF=1         append the words at a1 and a2 after each call (MemCardSync's cmd/result)
--   DW1_CAPTURE_STOP=<vblank>   end the run here (default: after the window and the last shot),
--                               once every recorded call has returned
--   DW1_CAPTURE_GRACE=<n>       VBlanks to wait for those returns before failing (default 600)
-- Writes calls.txt: "n vblank name a0 a1 a2 a3 sp ra v0" (v0 at return), and vram_<vblank>.bin
-- (1 MiB) at every VBlank of the window.

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end
local ROOT = here() .. '/..'

DW1ROUTE_TARGET = 'title_menu'
DW1ROUTE_ON_TARGET = function() end -- the route keeps going; this script decides when to stop
dofile(here() .. '/route-new-game.lua')

local function num(name, default)
    local v = os.getenv(name)
    if v == nil or v == '' then return default end
    local n = tonumber(v)
    if not n then DW1ROUTE_FAIL(name .. ' is not a number: ' .. v) end
    return n
end

local shotFirst, shotLast, shotStep
do
    local s = os.getenv('DW1_SHOTS')
    if s and s ~= '' then
        local a, b, c = s:match('^(%d+):(%d+):(%d+)$')
        if not a then DW1ROUTE_FAIL('DW1_SHOTS must be first:last:step') end
        shotFirst, shotLast, shotStep = tonumber(a), tonumber(b), tonumber(c)
    end
end

local FROM = num('DW1_CAPTURE_FROM', 2000)
local WINDOW = num('DW1_CAPTURE_VBLANKS', 2)
local MAX_CALLS = num('DW1_CAPTURE_MAX', 400)
local STATES = os.getenv('DW1_CAPTURE_STATES') == '1'
local DEREF = os.getenv('DW1_CAPTURE_DEREF') == '1'

-- Function addresses from the decomp's symbol list.
local funcs = {}
do
    local wanted = {}
    for name in (os.getenv('DW1_CAPTURE_FUNCS') or ''):gmatch('[^,%s]+') do wanted[name] = true end
    local f = io.open(ROOT .. '/decomp/config/us/symbols.txt', 'r')
    if not f then DW1ROUTE_FAIL('cannot read decomp/config/us/symbols.txt') end
    for line in f:lines() do
        local name, addr = line:match('^([%w_]+)%s*=%s*0x(%x+);')
        if name and wanted[name] then
            funcs[#funcs + 1] = { name = name, addr = tonumber(addr, 16) }
            wanted[name] = nil
        end
    end
    f:close()
    for name in pairs(wanted) do DW1ROUTE_FAIL('no symbol ' .. name) end
end

-- Default end: after the window and the last screenshot, whichever is later.
local STOP = num('DW1_CAPTURE_STOP', math.max(#funcs > 0 and FROM + WINDOW + 1 or 0, (shotLast or FROM) + 1))
-- Calls still running when the run should end get this many VBlanks to return (a blocking
-- CdReadSync can take a while); past that the capture fails rather than miss them.
local RETURN_GRACE = num('DW1_CAPTURE_GRACE', 600)

local function writeState(name)
    DW1ROUTE_WRITE(name, 'DW1STATE'
        .. ffi.string(PCSX.getRegisters(), ffi.sizeof('psxRegisters'))
        .. ffi.string(PCSX.getScratchPtr(), 1024)
        .. ffi.string(PCSX.getMemPtr(), 2 * 1024 * 1024))
end

local function shot(name)
    local ss = PCSX.GPU.takeScreenShot()
    DW1ROUTE_WRITE(name .. '.raw', ffi.string(ss.data.data, ss.data.size))
    DW1ROUTE_WRITE(name .. '.txt',
        string.format('%d %d %d', tonumber(ss.width), tonumber(ss.height), tonumber(ss.bpp)))
end

local index = {}
local vblank, calls, done = 0, 0, false
local entryBps, pending, retired, retBps = {}, {}, {}, {}

local function pendingCount()
    local n = 0
    for _ in pairs(pending) do n = n + 1 end
    return n
end

local function finish()
    if done then return end
    done = true
    for _, bp in ipairs(entryBps) do bp:remove() end
    local lines = { '# n vblank name a0 a1 a2 a3 sp ra v0\n' }
    for i = 1, #index do lines[#lines + 1] = index[i].line .. ' ' .. (index[i].v0 or '-') .. '\n' end
    DW1ROUTE_WRITE('calls.txt', table.concat(lines))
    print(string.format('DW1CAPTURE done calls=%d vblank=%d', calls, vblank))
    DW1ROUTE_SUCCEED()
end

local function onEntry(fn)
    return function()
        if done or calls >= MAX_CALLS or vblank >= FROM + WINDOW then return true end
        local n = calls
        calls = calls + 1
        local r = PCSX.getRegisters().GPR.n
        local rec = { line = string.format('%04d %d %s %08x %08x %08x %08x %08x %08x',
            n, vblank, fn.name, r.a0, r.a1, r.a2, r.a3, r.sp, r.ra) }
        index[#index + 1] = rec
        if STATES then writeState(string.format('call%04d_in.bin', n)) end
        -- A persistent breakpoint per return address (LuaJIT allows about 2,000 ffi callbacks,
        -- so one per call would run out); it retires the newest pending call from that address.
        pending[n] = rec
        rec.ra, rec.a1, rec.a2 = r.ra, r.a1, r.a2
        if not retBps[r.ra] then
            local ra = r.ra
            retBps[ra] = PCSX.addBreakpoint(ra, 'Exec', 4, 'return ' .. ra, function()
                local best
                for k, rc in pairs(pending) do
                    if rc.ra == ra and (not best or k > best) then best = k end
                end
                if best then
                    local rc = pending[best]
                    rc.v0 = string.format('%08x', PCSX.getRegisters().GPR.n.v0)
                    if DEREF then
                        local words = ''
                        for _, a in ipairs({ rc.a1, rc.a2 }) do
                            local o = a % 0x80000000
                            if a >= 0x80000000 and o + 4 <= 0x200000 then
                                words = words .. string.format(' [%08x]=%08x', a,
                                    ffi.cast('uint32_t*', ffi.cast('uint8_t*', PCSX.getMemPtr()) + o)[0])
                            end
                        end
                        rc.v0 = rc.v0 .. words
                    end
                    if STATES then writeState(string.format('call%04d_out.bin', best)) end
                    pending[best] = nil
                end
                return true
            end)
        end
        return true
    end
end

KEEP_CAPTURE_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if shotFirst and vblank >= shotFirst and vblank <= shotLast
        and (vblank - shotFirst) % shotStep == 0 then
        shot(string.format('shot_%05d', vblank))
    end
    if #funcs > 0 and vblank == FROM then
        print(string.format('DW1CAPTURE arming at vblank=%d', vblank))
        PCSX.settings.emulator.Debug.Debug = true
        for _, fn in ipairs(funcs) do
            entryBps[#entryBps + 1] = PCSX.addBreakpoint(fn.addr, 'Exec', 4, fn.name, onEntry(fn))
        end
    end
    if #funcs > 0 and vblank > FROM and vblank <= FROM + WINDOW and os.getenv('DW1_CAPTURE_VRAM') ~= '0' then
        local v = PCSX.GPU.getVRAM()
        DW1ROUTE_WRITE(string.format('vram_%05d.bin', vblank), ffi.string(v.data, v.size))
    end
    if vblank >= STOP then
        if pendingCount() == 0 then
            finish()
        elseif vblank >= STOP + RETURN_GRACE then
            local left = {}
            for n in pairs(pending) do left[#left + 1] = string.format('%04d', n) end
            table.sort(left)
            DW1ROUTE_FAIL('calls never returned: ' .. table.concat(left, ' '))
        end
    end
end)
