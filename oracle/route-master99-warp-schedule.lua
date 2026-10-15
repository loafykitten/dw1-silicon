-- After route-master99-yard-schedule.lua: from the house door to the Jijimon that tells the
-- time and then is Mr. Warp (x -161, z -2451), and through his menu to "Very far away",
-- "Unknown World": In front of Curling Cave / Seadra fishing / Lava Cave, cursor on the first.
-- The tamer's walk down past x 256, z -2000..-2300 crosses a step trigger (54) whose script walks
-- him on south to about z -3185 by itself. Derived with oracle/explore.lua (Phase 9).
local PRESSES = {
    { 4811, 4811, 1, 40, 'DOWN' },
    { 4856, 4856, 1, 25, 'LEFT' },
    { 4881, 4881, 1, 15, 'DOWN' },
    { 4901, 4901, 1, 10, 'LEFT' },
    { 4936, 4936, 1, 10, 'RIGHT' },
    { 4951, 4951, 1, 35, 'UP' },
    { 5026, 5026, 1, 8, 'UP' },
    { 5034, 5034, 1, 30, 'LEFT' },
    { 5074, 5074, 1, 5, 'LEFT' },
    { 5079, 5079, 1, 4, 'CROSS' },    -- Jijimon: colours test
    { 5169, 5169, 1, 4, 'CROSS' },    -- "Right now, 8:30 on the 11th day..."
    { 5259, 5259, 1, 4, 'CROSS' },    -- "I'm Mr.Warp. Where do you want to go?"
    { 5349, 5349, 1, 4, 'CROSS' },    -- Near / Far away / Very far away
    { 5439, 5439, 1, 4, 'DOWN' },
    { 5469, 5469, 1, 4, 'DOWN' },
    { 5499, 5499, 1, 4, 'CROSS' },    -- Very far away: Anc. Dino Region gate / back / Unknown World
    { 5589, 5589, 1, 4, 'DOWN' },
    { 5619, 5619, 1, 4, 'DOWN' },
    { 5649, 5649, 1, 4, 'CROSS' },    -- Unknown World
}
local MILESTONES = {
    { 5349, 'mr_warp' },
    { 5739, 'unknown_world' },
}
return PRESSES, MILESTONES
