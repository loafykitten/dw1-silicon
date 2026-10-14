-- Extra schedule for DW1ROUTE_PROFILE=battle-twice: battle-fixture (one won battle, back on the
-- map by ~9100), then the same fight again with BTL loaded a second time (Phase 5's reload test,
-- docs/phase5-plan.md). Derived with oracle/explore.lua: walk up to screen 112 and back down into
-- 109, where the ModokiBetamon has come back; walk into it, close its line; the battle loads
-- (GAME_STATE 1 by ~11100). Gabumon still has the 96 HP the first fight left and loses (0 HP by
-- ~11700, the death countdown); the game sends the tamer to Jijimon's house (screen 218, "Oh my
-- gosh! You're back after losing."); Cross through the lines; control back by ~13350.
local PRESSES = {
    { 9201, 9201, 1, 60, 'UP' },      -- to screen 112
    { 9611, 9611, 1, 40, 'DOWN' },
    { 9901, 9901, 1, 60, 'DOWN' },    -- back into 109
    { 10411, 10411, 1, 40, 'DOWN' },  -- into the ModokiBetamon
    { 10511, 10511, 1, 4, 'CROSS' },  -- its line; the battle loads
    { 12941, 12941, 1, 4, 'CROSS' },  -- Jijimon's lines
    { 13011, 13011, 1, 4, 'CROSS' },
    { 13081, 13081, 1, 4, 'CROSS' },
    { 13151, 13151, 1, 4, 'CROSS' },
    { 13221, 13221, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 9611, 'twice_screen112' },
    { 10411, 'twice_screen109' },
    { 11141, 'twice_battle' },
    { 11741, 'twice_down' },
    { 12941, 'twice_jijimon' },
    { 13500, 'twice_done' },          -- target: control back in Jijimon's house
}
return PRESSES, MILESTONES
