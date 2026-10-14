local PRESSES = {
    { 700, 1900, 150, 4, 'START' },
    { 2380, 2380, 1, 4, 'DOWN' },
    { 2430, 2430, 1, 4, 'CROSS' },
    { 2500, 2500, 1, 4, 'CROSS' },
    { 2700, 2700, 1, 4, 'CROSS' },
    { 2900, 2900, 1, 4, 'CROSS' },
}
local MILESTONES = {}
for v = 2900, 7000, 100 do table.insert(MILESTONES, { v, string.format('x%05d', v) }) end
return PRESSES, MILESTONES
