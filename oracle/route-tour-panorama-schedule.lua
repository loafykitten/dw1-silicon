-- Extra schedule for DW1ROUTE_PROFILE=tour-panorama, after route-debug-room-schedule.lua (Phase 9,
-- stream tour, derived with oracle/explore.lua): over to the bottom-left Jijimon (Mr. Warp:
-- "White White White..." colour test, the date, "Where do you want to go?"), Near, the peak of Mt.
-- Panorama (screen 18), north into two Goburimon ("I'm going to take all you have!"): a battle
-- against two enemies, the finisher with Square, Gabumon loses; the MURD scene and Jijimon's house
-- (218, "You're back after losing."), control back by ~14680.
local PRESSES = {
    { 6954, 6954, 1, 70, 'UP' },
    { 7024, 7024, 1, 40, 'LEFT' },
    { 7064, 7064, 1, 4, 'CROSS' },     -- Mr. Warp: colour test
    { 7154, 7154, 1, 4, 'CROSS' },     -- the date
    { 7244, 7244, 1, 4, 'CROSS' },     -- "I'm Mr.Warp"
    { 7334, 7334, 1, 4, 'CROSS' },     -- Near / Far away / Very far away
    { 7424, 7424, 1, 4, 'CROSS' },     -- Near: Mt. Panorama / Tropical Jungle / Drill Tunnel
    { 7514, 7514, 1, 4, 'CROSS' },     -- the peak of Mt. Panorama
    { 8044, 8044, 1, 30, 'RIGHT' },
    { 8074, 8074, 1, 250, 'UP' },      -- into the Goburimon
    { 8324, 8324, 1, 4, 'CROSS' },
    { 8654, 8654, 1, 4, 'CROSS' },     -- the battle starts
    { 11504, 11504, 1, 4, 'SQUARE' },  -- finisher
    { 14234, 14234, 1, 4, 'CROSS' },   -- Jijimon's house
    { 14324, 14324, 1, 4, 'CROSS' },
    { 14414, 14414, 1, 4, 'CROSS' },
    { 14504, 14504, 1, 4, 'CROSS' },
    { 14594, 14594, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 7424, 'warp_menu' },
    { 8044, 'panorama_peak' },
    { 8324, 'goburimon' },
    { 10784, 'battle' },
    { 14234, 'jijimon_house' },
    { 14800, 'panorama_done' },        -- target: control back in Jijimon's house
}
return PRESSES, MILESTONES
