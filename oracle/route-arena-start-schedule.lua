-- The start shared by the tournament profiles (Phase 9, arena stream), after the 'title' base:
-- CONTINUE GAME with the fixture p9-arena-master99 in the cwd's memcard1.mcd (Master99's 2002
-- GameFAQs save, unchanged: "completed everything", MetalGreymon, day 11 08:08 in Jijimon's
-- house). Its one save is the card's third (file DMR2): Down twice on the save list.
local PRESSES = {
    { 2380, 2380, 1, 4, 'DOWN' },     -- title menu: NEW GAME -> CONTINUE GAME
    { 2430, 2430, 1, 4, 'CROSS' },    -- CONTINUE GAME
    { 2500, 2500, 1, 4, 'CROSS' },    -- CONTINUE SLOT 1
    { 2681, 2681, 1, 4, 'DOWN' },     -- save list: save 1 -> 2
    { 2711, 2711, 1, 4, 'DOWN' },     -- -> 3
    { 2751, 2751, 1, 4, 'CROSS' },    -- save 3
    { 2901, 2901, 1, 4, 'CROSS' },    -- "Continue adventure?" Yes
}
local MILESTONES = {
    { 2680, 'save_list' },
    { 2880, 'continue_prompt' },
    { 4000, 'house' },
}
return PRESSES, MILESTONES
