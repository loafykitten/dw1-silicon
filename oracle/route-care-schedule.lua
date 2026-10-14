-- Extra schedule for DW1ROUTE_PROFILE=care-fixture: appended to the continue route (fixture card
-- in the cwd's memcard1.mcd, loaded into File City by VBlank 3800). Derived with
-- oracle/explore.lua (turn-based, screens read from screenshots; docs/phase4-plan.md): open the
-- Triangle menu, ITEM, sort the inventory ("Raise"), the DIGIMON status and Tech tabs, close the
-- menu, then no input while the game clock runs 07:04 to 14:16 (more than seven game hours) and
-- the partner's food, energy and poop level change. The route stops at VBlank 31000: the poop
-- condition (0x8) only sets at ~34000, but the pinned Redux's Lua hooks break down at VBlank
-- 32768 (docs/phase4-plan.md). Taps are 4 VBlanks.
local PRESSES = {
    { 3801, 3801, 1, 4, 'TRIANGLE' },
    { 3861, 3861, 1, 4, 'CROSS' },
    { 3921, 3921, 1, 4, 'CROSS' },
    { 3981, 3981, 1, 4, 'CROSS' },
    { 4101, 4101, 1, 4, 'DOWN' },
    { 4131, 4131, 1, 4, 'DOWN' },
    { 4161, 4161, 1, 4, 'CROSS' },
    { 4221, 4221, 1, 4, 'DOWN' },
    { 4251, 4251, 1, 4, 'CROSS' },
    { 4341, 4341, 1, 4, 'CIRCLE' },
    { 4371, 4371, 1, 4, 'CIRCLE' },
    { 4431, 4431, 1, 4, 'TRIANGLE' },
    { 4491, 4491, 1, 4, 'RIGHT' },
    { 4521, 4521, 1, 4, 'CROSS' },
    { 4611, 4611, 1, 4, 'R1' },
    { 4681, 4681, 1, 4, 'RIGHT' },
    { 4751, 4751, 1, 4, 'TRIANGLE' },
    { 4781, 4781, 1, 4, 'TRIANGLE' },
    { 4841, 4841, 1, 4, 'TRIANGLE' },
    { 4901, 4901, 1, 4, 'TRIANGLE' },
    { 4991, 4991, 1, 4, 'CIRCLE' },
    { 5081, 5081, 1, 4, 'TRIANGLE' },
}
local MILESTONES = {
    { 3860, 'care_menu' },
    { 4340, 'care_items_sorted' },
    { 4610, 'care_status' },
    { 4750, 'care_tech' },
    { 5300, 'care_menu_closed' },
    { 20000, 'care_idle' },
    { 26000, 'care_late' },
    { 31000, 'care_done' },           -- target
}
return PRESSES, MILESTONES
