-- CONTINUE GAME with the card's third save (DMR2), appended to the 'title' base profile (Phase 9,
-- the ending stream): title menu CONTINUE GAME, slot 1, the third save in the list (the only one on
-- Master99's 2001 GameFAQs card and the fixtures made from it), "Continue adventure?" Yes. Timings
-- as route-continue-schedule.lua, read from screenshots.
local PRESSES = {
    { 2380, 2380, 1, 4, 'DOWN' },     -- title menu: NEW GAME -> CONTINUE GAME
    { 2430, 2430, 1, 4, 'CROSS' },    -- CONTINUE GAME
    { 2500, 2500, 1, 4, 'CROSS' },    -- CONTINUE SLOT 1
    { 2701, 2701, 1, 4, 'DOWN' },     -- save list (card read done by ~2680)
    { 2731, 2731, 1, 4, 'DOWN' },
    { 2761, 2761, 1, 4, 'CROSS' },    -- save 3
    { 2961, 2961, 1, 4, 'CROSS' },    -- "Continue adventure?" Yes
}
local MILESTONES = {
    { 2680, 'save_list' },
    { 2950, 'continue_prompt' },
    { 3900, 'save3_loaded' },
}
return PRESSES, MILESTONES
