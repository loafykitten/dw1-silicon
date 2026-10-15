-- DW1ROUTE_PROFILE=rest-trash (Phase 9, stream rest): Trash Mountain's gym (screen 165, GOMI02: TRN2
-- loads with the map) and Sukamon's recruit. The fixture captures/fixtures/p9-rest-trash.mcd is
-- Master99's 2002 save (p9-arena-master99, the card's third save: continue-save3) moved to where
-- the warp from screen 164 lands (tools/oracle/place_fixture.py ... 165 164 0 189 -3428 239=0:
-- flag 239, Sukamon recruited = 200 + his species 39, cleared). Script 159 section 7 (the Sukamon
-- at about x 483, z 179): with 239 clear, flag 239, prosperity + 1 and "I'll visit you guys there
-- some day"; talked to again, the Sukamon Tribe's technique. Then the sign at Cross tile 82
-- ("Training: Hide In Trash", MP), Start, Normal: TRN2's session; MetalGreymon's MP is capped.
-- Derived with oracle/explore.lua (captures/oracle-runs/p9-rest-trash-x1); no battle on the way.
-- The first DOWN is blocked (the partner stands behind) and the walk to the sign takes two tries.
local PRESSES = {
    { 4032, 4032, 1, 10, 'UP' },
    { 4042, 4042, 1, 20, 'RIGHT' },
    { 4062, 4062, 1, 200, 'UP' },
    { 4262, 4262, 1, 4, 'CROSS' },
    { 4322, 4322, 1, 4, 'CROSS' },
    { 4382, 4382, 1, 4, 'CROSS' },
    { 4442, 4442, 1, 4, 'CROSS' },
    { 4502, 4502, 1, 4, 'CROSS' },
    { 4562, 4562, 1, 4, 'CROSS' },
    { 4622, 4622, 1, 4, 'CROSS' },
    { 4682, 4682, 1, 4, 'CROSS' },
    { 4742, 4742, 1, 4, 'CROSS' },
    { 4802, 4802, 1, 4, 'CROSS' },
    { 4862, 4862, 1, 25, 'DOWN' },
    { 4887, 4887, 1, 60, 'RIGHT' },
    { 4947, 4947, 1, 20, 'DOWN' },
    { 4967, 4967, 1, 30, 'RIGHT' },
    { 4997, 4997, 1, 4, 'CROSS' },
    { 5124, 5124, 1, 4, 'CROSS' },
    { 5184, 5184, 1, 4, 'CROSS' },
    { 5244, 5244, 1, 4, 'CROSS' },
    { 9474, 9474, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 4262, 'sukamon' },
    { 4502, 'recruited' },
    { 5124, 'sign' },
    { 6500, 'session' },
    { 9474, 'gains' },
    { 9800, 'trash_done' },
}
return PRESSES, MILESTONES
