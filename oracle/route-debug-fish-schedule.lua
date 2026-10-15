-- DW1ROUTE_PROFILE=debug-fish (Phase 9, stream fishcurl): after route-master99-yard-schedule.lua
-- (Master99's save, rods kept, out in the debug yard at 08:25): the top Jijimon's "Fast forward
-- time" one hour, Mr. Warp to "Seadra fishing" (Dragon Eye Lake), FISH from the menu, Chain melon,
-- one cast towards the Digicatfish, the bite, the strike ("Hooked!"), reeling against the tension
-- gauge and the catch ("Digicatfish, Total length 1 m 16 cm, I got one!"). Derived with
-- oracle/explore.lua (waitne for the bite; the Cross holds while reeling kept the tension between
-- 5000 and 7500).
local PRESSES = {
    -- down past the door; the walk crosses the yard's step trigger 54 (x 100..400, z -2300..-2000), whose
    -- script is read from the CD: the pad reads stale (Down) until it is in, as on the PS1 (recorded masks)
    { 4812, 4812, 1, 22, 'DOWN' },
    { 4836, 4836, 1, 20, 'LEFT' },
    { 4899, 4899, 1, 2, 'DOWN' },
    { 4921, 4921, 1, 6, 'UP' },
    { 4932, 4932, 1, 3, 'LEFT' },
    { 4940, 4940, 1, 12, 'LEFT' },
    { 4956, 4956, 1, 95, 'UP' },
    { 5055, 5055, 1, 12, 'LEFT' },
    { 5071, 5071, 1, 8, 'UP' },
    -- the top Jijimon: Half dead / Opening mode / Fast forward time
    { 5083, 5083, 1, 4, 'CROSS' },
    { 5173, 5173, 1, 4, 'DOWN' },
    { 5203, 5203, 1, 4, 'DOWN' },
    { 5233, 5233, 1, 4, 'CROSS' },
    -- Forward one hour: 08:3x -> 09:31 (Seadramon only comes at hours 8 and 17)
    { 5323, 5323, 1, 4, 'CROSS' },
    { 5443, 5443, 1, 30, 'DOWN' },
    -- Mr. Warp (after his colours and time lines): Very far away, Unknown World, Seadra fishing
    { 5477, 5477, 1, 4, 'CROSS' },
    { 5567, 5567, 1, 4, 'CROSS' },
    { 5657, 5657, 1, 4, 'CROSS' },
    { 5747, 5747, 1, 4, 'CROSS' },
    { 5837, 5837, 1, 4, 'DOWN' },
    { 5867, 5867, 1, 4, 'DOWN' },
    { 5897, 5897, 1, 4, 'CROSS' },
    { 5987, 5987, 1, 4, 'DOWN' },
    { 6017, 6017, 1, 4, 'DOWN' },
    { 6047, 6047, 1, 4, 'CROSS' },
    { 6137, 6137, 1, 4, 'DOWN' },
    { 6167, 6167, 1, 4, 'CROSS' },
    -- Dragon Eye Lake (screen 6, FISH loaded with the map): menu, FISH
    { 6797, 6797, 1, 4, 'TRIANGLE' },
    { 6857, 6857, 1, 4, 'CROSS' },
    -- bait: Chain melon (Digicatfish bite it, 30%); turn the tamer (yaw 1024 -> 576)
    { 6947, 6947, 1, 4, 'CROSS' },
    { 7018, 7018, 1, 21, 'LEFT' },
    -- throw, then release three frames later (power 8)
    { 7049, 7049, 1, 4, 'CROSS' },
    { 7061, 7061, 1, 4, 'CROSS' },
    { 7810, 7810, 1, 4, 'CROSS' },
    { 7854, 7854, 1, 4, 'CROSS' },
    { 7884, 7884, 1, 9, 'CROSS' },
    { 7893, 7893, 1, 9, 'CROSS' },
    { 7902, 7902, 1, 9, 'CROSS' },
    { 7911, 7911, 1, 9, 'CROSS' },
    { 7920, 7920, 1, 9, 'CROSS' },
    { 7929, 7929, 1, 9, 'CROSS' },
    { 7938, 7938, 1, 9, 'CROSS' },
    { 7947, 7947, 1, 9, 'CROSS' },
    { 7956, 7956, 1, 9, 'CROSS' },
    { 7992, 7992, 1, 9, 'CROSS' },
    { 8001, 8001, 1, 9, 'CROSS' },
    { 8010, 8010, 1, 9, 'CROSS' },
    { 8019, 8019, 1, 9, 'CROSS' },
    { 8221, 8221, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 5173, 'time_menu' },
    { 5567, 'mr_warp' },
    { 6797, 'lake' },
    { 7155, 'cast' },
    { 7687, 'bite' },
    { 7834, 'hooked' },
    { 8221, 'caught' },
    { 8401, 'fish_done' },
}
return PRESSES, MILESTONES
