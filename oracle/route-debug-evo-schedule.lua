-- Extra schedule for DW1ROUTE_PROFILE=debug-evo, after route-debug-room-schedule.lua (Phase 9,
-- derived with oracle/explore.lua): over to the Tokomon ("Life three", then "I'm
-- Mr.Digivolution"), choose Numemon: the digivolution sequence (the EVL overlay), Gabumon becomes
-- Numemon, control back by ~9270. The script's SET_STAT lives 3 leaves 1 (STATS_LIMIT_ARRAY).
local PRESSES = {
    { 6953, 6953, 1, 70, 'UP' },
    { 7023, 7023, 1, 90, 'LEFT' },
    { 7123, 7123, 1, 18, 'DOWN' },
    { 7141, 7141, 1, 30, 'LEFT' },
    { 7171, 7171, 1, 4, 'CROSS' },     -- Tokomon: "Life three"
    { 7261, 7261, 1, 4, 'CROSS' },     -- "I'm Mr.Digivolution"
    { 7351, 7351, 1, 4, 'CROSS' },     -- Numemon / Sukamon / Botamon
    { 7441, 7441, 1, 4, 'CROSS' },     -- Numemon
}
local MILESTONES = {
    { 7441, 'evo_menu' },
    { 8371, 'evo_shards' },
    { 9300, 'evo_done' },               -- target: Numemon, control back
}
return PRESSES, MILESTONES
