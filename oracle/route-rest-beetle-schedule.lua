-- DW1ROUTE_PROFILE=rest-beetle (Phase 9, stream rest): Beetle Land's screen 108 (BETL04: TRN2 loads
-- with the map) and Kuwagamon's recruit. The fixture captures/fixtures/p9-rest-beetle.mcd is
-- Master99's 2002 save (p9-arena-master99, the card's third save: continue-save3) moved to where
-- the warp from screen 106 lands (tools/oracle/place_fixture.py ... 108 106 0 603 -1853 251=0:
-- flag 251, Kuwagamon recruited = 200 + his species 51, cleared, so he waits by the machines;
-- MAPHEAD's script for 108 loads him only while it is clear). Script 104 section 5: "I want to beat
-- Kabuterimon", three answers; the first ("You gotta get stronger") ends it, the second ("there is
-- a sure fire way") brings "Kuwagamon went to the city!", flag 251 and prosperity + 2. Then the
-- treadmill sign (Cross tile 85) with a Bonus Try (TRN2's slot game: Cross starts and stops the
-- three reels) and the hammer machine sign (84) with a Normal session (TRN2_tickDefenseTraining
-- Map108); MetalGreymon's stats are capped, so the gains are none. Derived with oracle/explore.lua
-- (captures/oracle-runs/p9-rest-beetle-x1); no battle on the way.
local PRESSES = {
    { 4032, 4032, 1, 30, 'UP' },
    { 4062, 4062, 1, 38, 'LEFT' },
    { 4100, 4100, 1, 180, 'UP' },
    { 4282, 4282, 1, 8, 'UP' },
    { 4290, 4290, 1, 4, 'CROSS' },
    { 4350, 4350, 1, 4, 'CROSS' },
    { 4410, 4410, 1, 4, 'CROSS' },
    { 4470, 4470, 1, 4, 'CROSS' },
    { 4530, 4530, 1, 4, 'CROSS' },
    { 4590, 4590, 1, 4, 'CROSS' },
    { 4650, 4650, 1, 4, 'DOWN' },
    { 4680, 4680, 1, 4, 'CROSS' },
    { 4740, 4740, 1, 4, 'CROSS' },
    { 4800, 4800, 1, 4, 'CROSS' },
    { 4860, 4860, 1, 4, 'CROSS' },
    { 4920, 4920, 1, 4, 'CROSS' },
    { 4980, 4980, 1, 4, 'CROSS' },
    { 5040, 5040, 1, 4, 'CROSS' },
    { 5100, 5100, 1, 4, 'CROSS' },
    { 5160, 5160, 1, 4, 'CROSS' },
    { 5220, 5220, 1, 4, 'CROSS' },
    { 5280, 5280, 1, 4, 'CROSS' },
    { 5340, 5340, 1, 4, 'CROSS' },
    { 5400, 5400, 1, 4, 'CROSS' },
    { 5460, 5460, 1, 4, 'CROSS' },
    { 5520, 5520, 1, 4, 'CROSS' },
    { 5580, 5580, 1, 4, 'CROSS' },
    { 5640, 5640, 1, 16, 'RIGHT' },
    { 5656, 5656, 1, 4, 'CROSS' },
    { 5785, 5785, 1, 4, 'CROSS' },
    { 5845, 5845, 1, 4, 'CROSS' },
    { 5905, 5905, 1, 4, 'DOWN' },
    { 5935, 5935, 1, 4, 'CROSS' },
    { 6165, 6165, 1, 4, 'CROSS' },
    { 6255, 6255, 1, 4, 'CROSS' },
    { 6325, 6325, 1, 4, 'CROSS' },
    { 6395, 6395, 1, 4, 'CROSS' },
    { 6725, 6725, 1, 4, 'CROSS' },
    { 6875, 6875, 1, 10, 'UP' },
    { 6885, 6885, 1, 30, 'LEFT' },
    { 6915, 6915, 1, 4, 'CROSS' },
    { 6975, 6975, 1, 6, 'UP' },
    { 6981, 6981, 1, 4, 'CROSS' },
    { 7051, 7051, 1, 4, 'CROSS' },
    { 7111, 7111, 1, 4, 'CROSS' },
    { 7171, 7171, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 4282, 'kuwagamon' },
    { 4410, 'first_answer' },
    { 4740, 'second_answer' },
    { 5640, 'recruited' },
    { 5845, 'treadmill_sign' },
    { 6165, 'slots' },
    { 6725, 'treadmill_gains' },
    { 7051, 'hammer_sign' },
    { 9300, 'hammer_session' },
    { 11800, 'beetle_done' },
}
return PRESSES, MILESTONES
