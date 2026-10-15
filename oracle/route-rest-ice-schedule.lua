-- DW1ROUTE_PROFILE=rest-ice (Phase 9, stream rest): the Ice Sanctuary gym (screen 99, ICSA03: TRN2
-- loads with the map). The fixture captures/fixtures/p9-rest-ice.mcd is Master99's 2002 save
-- (p9-arena-master99, the card's third save: continue-save3) moved to where screen 98's warp lands
-- (tools/oracle/place_fixture.py ... 99 98 0 1529 -2524). The camera is turned 45 degrees here:
-- Up+Right walks north. The sign at Cross tile 84 ("Training: Icicle Fall", Defense), Start,
-- Normal: TRN2_tickDefenseTrainingMap99 counts only the frames the icicles fall (about 200 per
-- 600 VBlanks, to 1200), so the session is ended with Triangle (CANCEL_BUTTON sets the count to
-- 1200: the gains as for a full session); then the sign at tile 81 ("Training: Moving Ice", HP),
-- Start, Normal, run to its end (TRN2_tickHpTrainingMap99). Derived with oracle/explore.lua
-- (captures/oracle-runs/p9-rest-ice-x1); no battle on the way.
local PRESSES = {
    { 4032, 4032, 1, 10, 'UP' },
    { 4042, 4042, 1, 10, 'UP' },
    { 4042, 4042, 1, 10, 'RIGHT' },
    { 4052, 4052, 1, 95, 'UP' },
    { 4052, 4052, 1, 95, 'RIGHT' },
    { 4147, 4147, 1, 26, 'UP' },
    { 4147, 4147, 1, 26, 'RIGHT' },
    { 4173, 4173, 1, 10, 'RIGHT' },
    { 4173, 4173, 1, 10, 'DOWN' },
    { 4183, 4183, 1, 4, 'CROSS' },
    { 4311, 4311, 1, 4, 'CROSS' },
    { 4371, 4371, 1, 4, 'CROSS' },
    { 4431, 4431, 1, 4, 'CROSS' },
    { 10761, 10761, 1, 70, 'TRIANGLE' },
    { 11131, 11131, 1, 4, 'CROSS' },
    { 11311, 11311, 1, 50, 'LEFT' },
    { 11311, 11311, 1, 50, 'UP' },
    { 11361, 11361, 1, 57, 'UP' },
    { 11361, 11361, 1, 57, 'RIGHT' },
    { 11418, 11418, 1, 38, 'LEFT' },
    { 11418, 11418, 1, 38, 'UP' },
    { 11456, 11456, 1, 4, 'CROSS' },
    { 11526, 11526, 1, 4, 'CROSS' },
    { 11586, 11586, 1, 4, 'CROSS' },
    { 11646, 11646, 1, 4, 'CROSS' },
    { 15578, 15578, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 4311, 'icicle_sign' },
    { 5061, 'icicle_fall' },
    { 11131, 'icicle_gains' },
    { 11526, 'moving_ice_sign' },
    { 12576, 'moving_ice' },
    { 15578, 'moving_ice_gains' },
    { 15900, 'ice_done' },
}
return PRESSES, MILESTONES
