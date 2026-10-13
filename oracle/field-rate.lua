-- Field-rate measurement (S0.8): plays the new-game route to gameplay (VBlank 19400, no
-- debugger), then reads PCSX.getCPUCycles() at each GPU::Vsync and collects the cycle
-- differences between consecutive events: exactly N real intervals (env DW1RATE_VBLANKS, a
-- positive integer, default 1000). Prints mean/min/max, a histogram, and Hz = 33868800 / mean.
-- Redux's source predicts 2146 * 263 = 564398 cycles (about 60.009 Hz).
-- Same command line as the route (docs/oracle-route.md); output lines start with "DW1RATE".
-- Exits 1 on bad input, a callback error, or a quit before the measurement finished.
local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end
local done = false
local function die(msg)
    print('DW1RATE error: ' .. msg)
    PCSX.quit(1)
end

local raw = os.getenv('DW1RATE_VBLANKS') or '1000'
local N = tonumber(raw)
if not (N and N >= 1 and N == math.floor(N)) then
    die('DW1RATE_VBLANKS must be a positive integer, got ' .. raw)
    return
end

-- Our listener is registered before the route's, so for the VBlank event that reaches the target
-- it has already stored that event's cycle count as `last` by the time the route arms us; the
-- first interval is then target event to the next one, never a duplicate sample.
local armed, last, deltas = false, nil, {}
local function onVsync()
    local cyc = tonumber(PCSX.getCPUCycles())
    if armed and last then
        deltas[#deltas + 1] = cyc - last
        if #deltas == N then
            armed = false
            local sum, min, max, hist = 0, math.huge, 0, {}
            for _, d in ipairs(deltas) do
                sum = sum + d
                if d < min then min = d end
                if d > max then max = d end
                hist[d] = (hist[d] or 0) + 1
            end
            local mean = sum / N
            print(string.format('DW1RATE vblanks=%d mean=%.3f min=%d max=%d hz=%.5f', N, mean, min, max, 33868800 / mean))
            local keys = {}
            for k in pairs(hist) do keys[#keys + 1] = k end
            table.sort(keys)
            for _, k in ipairs(keys) do print(string.format('DW1RATE cycles=%d count=%d', k, hist[k])) end
            done = true
            DW1ROUTE_SUCCEED()
        end
    end
    last = cyc
end
KEEP_RATE_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    local ok, err = xpcall(onVsync, debug.traceback)
    if not ok then die('callback error: ' .. tostring(err)) end
end)
DW1ROUTE_TARGET = 'gameplay'
DW1ROUTE_ON_TARGET = function(v) armed = true end
dofile(here() .. '/route-new-game.lua')
KEEP_RATE_QUIT = PCSX.Events.createEventListener('Quitting', function()
    if not done then die('quit before the measurement finished') end
end)
