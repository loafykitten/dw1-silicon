-- Field-rate measurement (S0.8): plays the new-game route to gameplay (VBlank 19400, no
-- debugger), then reads PCSX.getCPUCycles() at each of the next 1000 GPU::Vsync events and
-- prints cycles per VBlank (mean/min/max, a histogram of distinct values) and
-- Hz = 33868800 / mean. Redux's source predicts 2146 * 263 = 564398 cycles (about 60.009 Hz).
-- Same command line as the route (docs/oracle-route.md); output lines start with "DW1RATE".
local function here()
    local src = debug.getinfo(1, 'S').source:gsub('^@', '')
    return src:match('^(.*)/[^/]*$') or '.'
end
local N = tonumber(os.getenv('DW1RATE_VBLANKS') or '1000')
local samples, armed = {}, false
DW1ROUTE_TARGET = 'gameplay'
DW1ROUTE_ON_TARGET = function(v) armed = true; samples[1] = tonumber(PCSX.getCPUCycles()) end
dofile(here() .. '/route-new-game.lua')
KEEP_RATE_VSYNC = PCSX.Events.createEventListener('GPU::Vsync', function()
    if not armed then return end
    samples[#samples + 1] = tonumber(PCSX.getCPUCycles())
    if #samples < N + 1 then return end
    armed = false
    local sum, min, max, hist = 0, math.huge, 0, {}
    for i = 2, #samples do
        local d = samples[i] - samples[i - 1]
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
    DW1ROUTE_SUCCEED()
end)
