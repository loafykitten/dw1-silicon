-- Black-box probe: call one SDK function in the emulator with many argument sets and record what
-- it returns, without reading its code (docs/provenance.md: the SDK is observed, not translated).
-- Boots the default route; at VBlank DW1_PROBE_AT, when the game next enters VSync, the CPU is
-- borrowed: registers are saved, the function is called with each argument line in turn
-- (returning to VSync's entry, where a breakpoint catches it), then the registers are put back
-- and the game carries on as if nothing happened.
--
--   DW1_PROBE_FUNC=SsPitchFromNote DW1_PROBE_ARGS=args.txt DW1_PROBE_AT=2300 \
--       pcsx-redux ... -dofile oracle/sdk-probe.lua -run
-- args.txt: one call per line, "a0 a1 a2 a3 [s0 s1 s2 s3]" in hex (fewer words leave the rest 0;
-- s0.. are the fifth argument on, stored at sp+16.. for the call and put back afterwards).
-- Writes probe-out.txt: the line's words and v0, in hex. Only for functions whose effects are
-- harmless to the game (pure ones, like a pitch calculation, or a voice keyed on).
-- DW1_PROBE_SPU=1 also logs the SPU register stores each call makes, after its line:
-- " spu=address:value,..." (hex). DW1_PROBE_THEN=f1,f2:a0,... calls those functions in turn after
-- each probe, their stores logged with the probe's (a library that only writes the hardware on
-- its tick, like libsnd, needs its tick called to show what a call did). After the name, ":" and
-- up to four "/"-separated arguments in hex, "a0" for the probe's first argument and "pN" for its
-- line's Nth word (SsUtKeyOffV:a0 frees the voice a key-on took; SsUtAutoVol:a0/p9/p10/p11 takes
-- a line's 9th to 11th words, past what the probed function reads); a function with none gets
-- zeros.
-- "*n" repeats the call n times (SsSeqCalledTbyT*12). Each call's stores are preceded by "|".
-- DW1_PROBE_PRE=... (same syntax, no "a0"/"pN") runs once before the first probe, its stores not
-- logged (SsSeqStop:0,SsSeqClose:0 frees the game's SEQ). A probe line may carry pokes,
-- "@address=hexbytes", written to RAM just before its call (the bytes they overwrite are put
-- back after the last probe). DW1_PROBE_NOIRQ=1 masks interrupts while probing, so the SDK's own
-- interrupt-driven tick can't run between the calls.

local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end

DW1ROUTE_TARGET = 'title_menu'
DW1ROUTE_ON_TARGET = function() end
dofile(here() .. '/route-new-game.lua')

local FUNC = os.getenv('DW1_PROBE_FUNC') or DW1ROUTE_FAIL('DW1_PROBE_FUNC?')
local AT = tonumber(os.getenv('DW1_PROBE_AT') or '') or 2300
local ARGS = os.getenv('DW1_PROBE_ARGS') or DW1ROUTE_FAIL('DW1_PROBE_ARGS?')

local function symbol(name)
    local f = io.open(here() .. '/../decomp/config/us/symbols.txt', 'r')
    for line in f:lines() do
        local n, addr = line:match('^([%w_]+)%s*=%s*0x(%x+);')
        if n == name then f:close() return tonumber(addr, 16) end
    end
    f:close()
    DW1ROUTE_FAIL('no symbol ' .. name)
end
local funcAddr, vsyncAddr = symbol(FUNC), symbol('VSync')
local thens = {}
for item in (os.getenv('DW1_PROBE_THEN') or ''):gmatch('[^,%s]+') do
    local body, times = item:match('^(.-)%*(%d+)$')
    body, times = body or item, tonumber(times or '1')
    local name, argstr = body:match('^([%w_]+):?(.*)$')
    local args = {}
    for a in argstr:gmatch('[^/]+') do
        args[#args + 1] = (a == 'a0' and { 1 }) or (a:match('^p(%d+)$') and { tonumber(a:sub(2)) })
            or tonumber(a, 16)
    end
    for _ = 1, times do thens[#thens + 1] = { addr = symbol(name), args = args } end
end
local thenIndex = 0

local probes = {}
for line in io.lines(ARGS) do
    local words, pokes = {}, {}
    line = line:gsub('@(%x+)=(%x+)', function(addr, hex)
        pokes[#pokes + 1] = { addr = tonumber(addr, 16), hex = hex }
        return ''
    end)
    for w in line:gmatch('%x+') do words[#words + 1] = tonumber(w, 16) end
    words.pokes = pokes
    if #words > 0 then probes[#probes + 1] = words end
end

local function parseCalls(spec)
    local list = {}
    for item in (spec or ''):gmatch('[^,%s]+') do
        local body, times = item:match('^(.-)%*(%d+)$')
        body, times = body or item, tonumber(times or '1')
        local name, argstr = body:match('^([%w_]+):?(.*)$')
        local args = {}
        for a in argstr:gmatch('[^/]+') do args[#args + 1] = tonumber(a, 16) end
        for _ = 1, times do list[#list + 1] = { addr = symbol(name), args = args } end
    end
    return list
end
local pre = parseCalls(os.getenv('DW1_PROBE_PRE'))
local preIndex = 0
local NOIRQ = os.getenv('DW1_PROBE_NOIRQ') == '1'
local poked = {} -- address -> original byte

local vblank, bp, saved, nextProbe, out = 0, nil, nil, 1, {}
local SPU = os.getenv('DW1_PROBE_SPU') == '1'
local mem = ffi.cast('uint8_t*', PCSX.getMemPtr())
local function stackWord(sp, i)
    return ffi.cast('uint32_t*', mem + ((sp + 16 + 4 * i) % 0x200000))
end
local spuBp, spuWrites = nil, {}

local function startProbe(regs)
    local p = probes[nextProbe]
    for _, pk in ipairs(p.pokes) do
        for i = 0, #pk.hex / 2 - 1 do
            local a = (pk.addr + i) % 0x200000
            if poked[a] == nil then poked[a] = mem[a] end
            mem[a] = tonumber(pk.hex:sub(2 * i + 1, 2 * i + 2), 16)
        end
    end
    regs.GPR.n.a0, regs.GPR.n.a1 = p[1] or 0, p[2] or 0
    regs.GPR.n.a2, regs.GPR.n.a3 = p[3] or 0, p[4] or 0
    for i = 0, 3 do stackWord(saved.r[29], i)[0] = p[5 + i] or 0 end
    regs.GPR.n.sp = saved.r[29]
    regs.GPR.n.ra = vsyncAddr
    regs.pc = funcAddr
    spuWrites = {}
end

local function onSpuWrite(address, width)
    local r = PCSX.getRegisters()
    local w = ffi.cast('uint32_t*', mem + (r.pc % 0x200000))[0]
    local v = r.GPR.r[bit.band(bit.rshift(w, 16), 31)]
    if width == 2 then v = bit.band(v, 0xFFFF) elseif width == 1 then v = bit.band(v, 0xFF) end
    spuWrites[#spuWrites + 1] = string.format('%x:%x', address, v)
    return true
end

local function onVSync()
    local regs = PCSX.getRegisters()
    if not saved then
        saved = { pc = regs.pc, r = {}, stack = {} }
        for i = 0, 33 do saved.r[i] = regs.GPR.r[i] end
        for i = 0, 3 do saved.stack[i] = stackWord(saved.r[29], i)[0] end
        saved.sr = regs.CP0.r[12]
        if NOIRQ then regs.CP0.r[12] = bit.band(saved.sr, bit.bnot(1)) end
    end
    if preIndex < #pre then
        preIndex = preIndex + 1
        local t = pre[preIndex]
        regs.GPR.n.a0, regs.GPR.n.a1 = t.args[1] or 0, t.args[2] or 0
        regs.GPR.n.a2, regs.GPR.n.a3 = t.args[3] or 0, t.args[4] or 0
        regs.GPR.n.sp = saved.r[29]
        regs.GPR.n.ra = vsyncAddr
        regs.pc = t.addr
        return true
    end
    if not saved.started then
        saved.started = true
        if SPU then spuBp = PCSX.addBreakpoint(0x1F801C00, 'Write', 0x200, 'probe SPU', onSpuWrite) end
        startProbe(regs)
        return true
    end
    local p = probes[nextProbe]
    if thenIndex < #thens then
        if thenIndex == 0 then p.v0 = regs.GPR.n.v0 end
        thenIndex = thenIndex + 1
        local t = thens[thenIndex]
        local function arg(i)
            local a = t.args[i]
            if type(a) == 'table' then return p[a[1]] or 0 end
            return a or 0
        end
        regs.GPR.n.a0, regs.GPR.n.a1, regs.GPR.n.a2, regs.GPR.n.a3 = arg(1), arg(2), arg(3), arg(4)
        spuWrites[#spuWrites + 1] = '|'
        regs.GPR.n.sp = saved.r[29]
        regs.GPR.n.ra = vsyncAddr
        regs.pc = t.addr
        return true
    end
    thenIndex = 0
    local words = {}
    for i = 1, math.max(4, #p) do words[i] = string.format('%x', p[i] or 0) end
    out[#out + 1] = string.format('%s %08x%s\n', table.concat(words, ' '), p.v0 or regs.GPR.n.v0,
        SPU and (' spu=' .. table.concat(spuWrites, ',')) or '')
    nextProbe = nextProbe + 1
    if nextProbe <= #probes then
        startProbe(regs)
        return true
    end
    for i = 0, 33 do regs.GPR.r[i] = saved.r[i] end
    for i = 0, 3 do stackWord(saved.r[29], i)[0] = saved.stack[i] end
    for a, v in pairs(poked) do mem[a] = v end
    regs.CP0.r[12] = saved.sr
    regs.pc = saved.pc
    bp:remove()
    if spuBp then spuBp:remove() end
    DW1ROUTE_WRITE('probe-out.txt', table.concat(out))
    print(string.format('DW1PROBE done %s probes=%d vblank=%d', FUNC, #out, vblank))
    DW1ROUTE_SUCCEED()
    return true
end

KEEP_PROBE_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    vblank = vblank + 1
    if vblank == AT then
        PCSX.settings.emulator.Debug.Debug = true
        bp = PCSX.addBreakpoint(vsyncAddr, 'Exec', 4, 'probe', onVSync)
    end
end)
