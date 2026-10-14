-- Extra schedule for DW1ROUTE_PROFILE=keeper-fixture: appended to the continue route (fixture
-- ember-day2-gabumon in the cwd's memcard1.mcd; screen 168, control at 3800). Derived with
-- oracle/explore.lua (docs/phase5-plan.md): around the fence to the Item Keeper's door (warp 113,
-- x -1400..-1000, z -1300..-1100; tools/oracle/map_triggers.py), into screen 211, up to the
-- counter; Agumon's introduction ("My new business is 'Item Keeper'"), talk again, "any business?"
-- Yes: the storage window (Keeping / In hand; the item-menu code whose flipTextboxPage results
-- docs/phase5-returns-audit.md reasons about), a few presses in it, Triangle, the farewell line;
-- control back by ~7060.
local PRESSES = {
    { 3801, 3801, 1, 55, 'UP' },
    { 3861, 3861, 1, 62, 'LEFT' },
    { 4123, 4123, 1, 20, 'DOWN' },
    { 4148, 4148, 1, 62, 'LEFT' },
    { 4215, 4215, 1, 20, 'UP' },
    { 4435, 4435, 1, 40, 'DOWN' },
    { 4480, 4480, 1, 80, 'LEFT' },
    { 4580, 4580, 1, 62, 'UP' },
    { 4647, 4647, 1, 25, 'RIGHT' },
    { 4872, 4872, 1, 40, 'UP' },
    { 5112, 5112, 1, 10, 'LEFT' },
    { 5132, 5132, 1, 30, 'UP' },
    { 5192, 5192, 1, 22, 'RIGHT' },
    { 5219, 5219, 1, 50, 'UP' },
    { 5619, 5619, 1, 25, 'UP' },
    { 5649, 5649, 1, 25, 'RIGHT' },
    { 5684, 5684, 1, 4, 'CROSS' },
    { 5804, 5804, 1, 4, 'CROSS' },
    { 5894, 5894, 1, 4, 'CROSS' },
    { 5984, 5984, 1, 4, 'CROSS' },
    { 6104, 6104, 1, 4, 'CROSS' },
    { 6194, 6194, 1, 4, 'CROSS' },
    { 6374, 6374, 1, 4, 'CROSS' },
    { 6464, 6464, 1, 4, 'RIGHT' },
    { 6524, 6524, 1, 4, 'CROSS' },
    { 6614, 6614, 1, 4, 'DOWN' },
    { 6674, 6674, 1, 4, 'CROSS' },
    { 6764, 6764, 1, 4, 'TRIANGLE' },
    { 6884, 6884, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 5469, 'keeper_enter' },
    { 5804, 'keeper_intro' },
    { 6344, 'keeper_window' },
    { 6884, 'keeper_farewell' },
    { 7200, 'keeper_done' },            -- target: control back
}
return PRESSES, MILESTONES
