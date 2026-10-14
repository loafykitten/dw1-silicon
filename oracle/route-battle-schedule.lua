-- Extra schedule for DW1ROUTE_PROFILE=battle-fixture: appended to continue-save2 (fixture
-- ember-save2-prefight in the cwd's memcard1.mcd; screen 109, control at ~3860). Derived with
-- oracle/explore.lua (docs/phase5-plan.md): walk down into the wild ModokiBetamon ("This is my
-- territory!"), close its line (GAME_STATE 3, the BTL overlay loads), let the partner fight on
-- its own, Square for the finisher once "FIN" shows (the game's mask 0x80), win, close the
-- results with Cross, back on the map (GAME_STATE 0) by ~9100.
local PRESSES = {
    { 3861, 3861, 1, 40, 'DOWN' },
    { 3931, 3931, 1, 30, 'DOWN' },
    { 4021, 4021, 1, 4, 'CROSS' },    -- the ModokiBetamon's line; the battle loads
    { 7051, 7051, 1, 4, 'SQUARE' },   -- finisher (FIN on the HUD)
    { 7471, 7471, 1, 10, 'SQUARE' },
    { 8401, 8401, 1, 4, 'CROSS' },    -- close the results (stat gains, bits)
}
local MILESTONES = {
    { 4020, 'battle_dialogue' },
    { 4651, 'battle_start' },
    { 6151, 'battle_mid' },
    { 7981, 'battle_won' },
    { 8281, 'battle_results' },
    { 9200, 'battle_done' },          -- target: back on the map, no input after 8401
}
return PRESSES, MILESTONES
