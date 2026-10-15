-- The walk into the debug room, shared by the debug-* profiles (Phase 9): appended to the
-- continue route, with the debug fixture in the cwd's memcard1.mcd (tools/oracle/debug_fixture.py
-- on ember-day2-gabumon: the four story flags that open the game's debug room are 0). Screen 168,
-- control at 3800. Up, then down off the screen into the plaza (180) and up again into the yard
-- (204), which now holds the debug actors (TCRF, "Digimon World", Debug Room).
local PRESSES = {
    { 5003, 5003, 1, 40, 'RIGHT' },
    { 5043, 5043, 1, 400, 'UP' },
    { 5593, 5593, 1, 330, 'DOWN' },
    { 5923, 5923, 1, 80, 'LEFT' },
    { 6003, 6003, 1, 120, 'DOWN' },
    { 6323, 6323, 1, 80, 'UP' },
}
local MILESTONES = {
    { 6323, 'plaza' },
    { 6953, 'debug_room' },
}
return PRESSES, MILESTONES
