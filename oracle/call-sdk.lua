-- Black-box oracle for SDK functions: plays the new-game route to the main menu, then, at a
-- PadRead call, runs a list of SDK calls with chosen arguments and memory, records what each
-- returns and writes, and quits. The SDK's code is only ever run, never read.
--
-- Run from a fresh, empty directory under captures/:
--   R=/Users/fenn/Developer/dw1-silicon
--   DW1_CALLS=requests.txt "$R/ref/pcsx-redux/pcsx-redux" -cli -portable -interpreter \
--       -softgpu -bios "$R/scph1001.bin" -iso "$R/disc/Digimon World (USA).cue" \
--       -dofile "$R/oracle/call-sdk.lua" -run > run.log
--
-- Requests (DW1_CALLS), one call per line, '#' comments:
--   <function> <a0> <a1> <a2> <a3> [in=<addr>:<hex bytes>]... [out=<addr>:<length>]...
--              [gte=<cN|dN>:<hex u32>]... [gtein=<w>,<w>,...] [stack=<a5>,<a6>,...] [gteout]
-- An argument or address is a hex number, or B+<hex>, an offset into a scratch buffer (64 KB of
-- stack RAM the game doesn't reach at the menu). `in` writes bytes before the call, `out`
-- reads bytes after it, `gte` sets a GTE control (c) or data (d) register before the call,
-- `gtein` sets all 64 (d0..d31, then c0..c31; a word of `x` leaves that register alone) so a
-- call is repeatable, `stack` gives the arguments after the fourth (hex or B+ offsets, written
-- at sp+16, sp+20, ... as the o32 ABI expects), and `gteout` records all 64 GTE registers after
-- the call. Function names come from decomp/config/us/symbols.txt.
-- Output (results.txt), one line per call: "<n> v0=<hex> [<addr>=<hex bytes>]... [gte=...]".
-- A call that faults the CPU (the SDK's trapping `add`s, on an overflow) is recorded as
-- "<n> v0=EXC<cause code>" with nothing else, and the run goes on.
-- Env: DW1_CALL_AT (VBlank to start at, default 2300: the main menu); DW1_SYMBOLS (the symbol
-- list, default decomp/config/us/symbols.txt next to this checkout); DW1_PROGRESS_EVERY (write
-- results.partial.txt every N calls, default 1000: a call that never returns hangs the
-- machine, and then this is all there is).

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end
local ROOT = here() .. '/..'

DW1ROUTE_TARGET = 'title_menu'
DW1ROUTE_ON_TARGET = function() end
dofile(here() .. '/route-new-game.lua')

local AT = tonumber(os.getenv('DW1_CALL_AT') or '2300')
local BUF = 0x801E0000     -- scratch buffer (stack RAM far below the menu's frames)
local CALL_SP = 0x801EFFF0 -- stack pointer for the calls, above the buffer
local TRIGGER = 0x80091BE8 -- PadRead entry: a clean call boundary every frame

local syms = {}
do
    local path = os.getenv('DW1_SYMBOLS') or (ROOT .. '/decomp/config/us/symbols.txt')
    local f = io.open(path, 'r')
    if not f then DW1ROUTE_FAIL('cannot read ' .. path .. ' (set DW1_SYMBOLS)') end
    for line in f:lines() do
        local name, addr = line:match('^([%w_]+)%s*=%s*0x(%x+);')
        if name then syms[name] = tonumber(addr, 16) end
    end
    f:close()
end

local function addr(s)
    local off = s:match('^B%+(%x+)$')
    if off then return BUF + tonumber(off, 16) end
    local v = tonumber(s, 16)
    if not v then DW1ROUTE_FAIL('bad address ' .. s) end
    return v
end

local calls = {}
do
    local path = os.getenv('DW1_CALLS')
    if not path then DW1ROUTE_FAIL('set DW1_CALLS') end
    local f = io.open(path, 'r')
    if not f then DW1ROUTE_FAIL('cannot read ' .. path) end
    for line in f:lines() do
        line = line:gsub('#.*$', '')
        local words = {}
        for w in line:gmatch('%S+') do words[#words + 1] = w end
        if #words > 0 then
            local fn = syms[words[1]]
            if not fn then DW1ROUTE_FAIL('no symbol ' .. words[1]) end
            local c = { fn = fn, args = {}, ins = {}, outs = {}, gte = {}, gteout = false, stack = {} }
            for i = 2, 5 do c.args[i - 1] = addr(words[i] or '0') end
            for i = 6, #words do
                local w = words[i]
                local k, v = w:match('^(%a+)=(.*)$')
                if k == 'in' then
                    local a, hex = v:match('^([^:]+):(%x*)$')
                    c.ins[#c.ins + 1] = { addr(a), hex }
                elseif k == 'out' then
                    local a, n = v:match('^([^:]+):(%x+)$')
                    c.outs[#c.outs + 1] = { addr(a), tonumber(n, 16) }
                elseif k == 'gte' then
                    local r, val = v:match('^([cd]%d+):(%x+)$')
                    c.gte[#c.gte + 1] = { r, tonumber(val, 16) }
                elseif k == 'gtein' then
                    local n = 0
                    for word in v:gmatch('[^,]+') do
                        local kind = n < 32 and 'd' or 'c'
                        if word ~= 'x' then
                            c.gte[#c.gte + 1] = { kind .. (n % 32), tonumber(word, 16) }
                        end
                        n = n + 1
                    end
                    if n ~= 64 then DW1ROUTE_FAIL('gtein needs 64 words') end
                elseif k == 'stack' then
                    for word in v:gmatch('[^,]+') do c.stack[#c.stack + 1] = addr(word) end
                elseif w == 'gteout' then
                    c.gteout = true
                else
                    DW1ROUTE_FAIL('bad field ' .. w)
                end
            end
            calls[#calls + 1] = c
        end
    end
    f:close()
end

local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function phys(a) return bit.band(a, 0x1FFFFF) end

local regs = PCSX.getRegisters()
local callSR = 0
local saved, current, results, skipNext = nil, 0, {}, false
local vblank = 0

local function startCall(i)
    local c = calls[i]
    for _, w in ipairs(c.ins) do
        local a, hex = w[1], w[2]
        for k = 0, #hex / 2 - 1 do
            mem[phys(a) + k] = tonumber(hex:sub(k * 2 + 1, k * 2 + 2), 16)
        end
    end
    for _, g in ipairs(c.gte) do
        local kind, n = g[1]:sub(1, 1), tonumber(g[1]:sub(2))
        local val = g[2]
        if val >= 0x80000000 then val = val - 0x100000000 end -- the register file is int32
        if kind == 'c' then regs.CP2C.r[n] = val else regs.CP2D.r[n] = val end
    end
    for k, w in ipairs(c.stack) do
        local p = phys(CALL_SP + 16 + 4 * (k - 1))
        for b = 0, 3 do mem[p + b] = bit.band(bit.rshift(w, 8 * b), 0xFF) end
    end
    local r = regs.GPR.n
    r.a0, r.a1, r.a2, r.a3 = c.args[1], c.args[2], c.args[3], c.args[4]
    r.sp = CALL_SP
    r.ra = TRIGGER
    callSR = regs.CP0.r[12]
    regs.pc = c.fn
end

-- A call that raises a CPU exception (the SDK uses trapping `add` and `addi`: a signed overflow
-- is a fault on the PS1, i.e. an input the game must never give it) is recorded as v0=EXC<code>
-- (the CP0 Cause exception code), with no memory or GTE output, and the machine carries on with
-- the next call.
local function finishCall(i, excCode)
    local c = calls[i]
    local parts = { string.format('%d v0=%08x', i, regs.GPR.n.v0) }
    if excCode then
        parts = { string.format('%d v0=EXC%d', i, excCode) }
        results[#results + 1] = table.concat(parts, ' ')
        return
    end
    for _, o in ipairs(c.outs) do
        local hex = {}
        for k = 0, o[2] - 1 do hex[#hex + 1] = string.format('%02x', mem[phys(o[1]) + k]) end
        parts[#parts + 1] = string.format('%08x=%s', o[1], table.concat(hex))
    end
    if c.gteout then
        local g = {}
        for n = 0, 31 do g[#g + 1] = string.format('%08x', regs.CP2D.r[n]) end
        for n = 0, 31 do g[#g + 1] = string.format('%08x', regs.CP2C.r[n]) end
        parts[#parts + 1] = 'gte=' .. table.concat(g, ',')
    end
    results[#results + 1] = table.concat(parts, ' ')
    -- A call that never returns hangs the machine: keep what was recorded so far on disk.
    if i % tonumber(os.getenv('DW1_PROGRESS_EVERY') or '1000') == 0 then
        print(string.format('DW1CALL progress %d/%d', i, #calls))
        DW1ROUTE_WRITE('results.partial.txt', table.concat(results, '\n') .. '\n')
    end
end

local function finish()
    DW1ROUTE_WRITE('results.txt', table.concat(results, '\n') .. '\n')
    print(string.format('DW1CALL done calls=%d', #results))
    DW1ROUTE_SUCCEED()
end

local bp, excBp
local function onTrigger(excCode)
    if skipNext then
        skipNext = false
        return true
    end
    if current == 0 then
        -- Save the game's state at this PadRead call; it's restored after the last call.
        saved = { pc = regs.pc, gpr = {}, cp2d = {}, cp2c = {} }
        for n = 0, 33 do saved.gpr[n] = regs.GPR.r[n] end
        for n = 0, 31 do saved.cp2d[n], saved.cp2c[n] = regs.CP2D.r[n], regs.CP2C.r[n] end
    else
        finishCall(current, excCode)
    end
    current = current + 1
    if current <= #calls then
        startCall(current)
        return true
    end
    -- All done: put the game back as it was, run on, and quit at the next VBlank.
    for n = 0, 33 do regs.GPR.r[n] = saved.gpr[n] end
    for n = 0, 31 do regs.CP2D.r[n], regs.CP2C.r[n] = saved.cp2d[n], saved.cp2c[n] end
    regs.pc = saved.pc
    skipNext = true
    bp:remove()
    excBp:remove()
    finish()
    return true
end

KEEP_CALL_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if vblank == AT then
        PCSX.settings.emulator.Debug.Debug = true
        -- Otherwise a CPU exception pauses the emulator (and the exception hook below never runs on).
        PCSX.settings.emulator.Debug.FirstChanceException = 0
        bp = PCSX.addBreakpoint(TRIGGER, 'Exec', 4, 'call-sdk trigger', function() return onTrigger(nil) end)
        -- The general exception vector: interrupts (code 0) and syscalls (8) are normal, anything
        -- else during a call ends the call.
        excBp = PCSX.addBreakpoint(0x80000080, 'Exec', 4, 'call-sdk exception', function()
            local ok, err = pcall(function()
                local code = bit.band(bit.rshift(regs.CP0.r[13], 2), 0x1F)
                if code ~= 0 and code ~= 8 and current >= 1 and current <= #calls and not skipNext then
                    regs.CP0.r[12] = callSR -- undo the exception's push of the interrupt/mode bits
                    onTrigger(code)
                end
            end)
            if not ok then print('DW1CALL exception handler error: ' .. tostring(err)) end
            return true
        end)
        print(string.format('DW1CALL armed at vblank=%d (%d calls)', vblank, #calls))
    end
end)
