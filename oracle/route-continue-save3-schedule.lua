-- Schedule for DW1ROUTE_PROFILE=continue-save3: the continue route (route-continue-schedule.lua)
-- but loading the card's third save (Down twice on the save list). For Master99's 2002 save
-- (captures/downloaded-saves/gamefaqs/master99-2002-1476.mcr, "Andrew", MetalGreymon, day 11 of
-- year 9), which sits in the card's second block but shows as save 3 (its file name ends in
-- DMR2); it loads into Jijimon's house (screen 205). Derived with oracle/explore.lua (Phase 9).
local PRESSES = {
    { 700, 1900, 150, 4, 'START' },   -- logos/FMV skip, as the new-game route
    { 2380, 2380, 1, 4, 'DOWN' },     -- title menu: NEW GAME -> CONTINUE GAME
    { 2430, 2430, 1, 4, 'CROSS' },    -- CONTINUE GAME
    { 2500, 2500, 1, 4, 'CROSS' },    -- CONTINUE SLOT 1
    { 2681, 2681, 1, 4, 'DOWN' },     -- save list (card read done by ~2680): save 1 -> 2
    { 2711, 2711, 1, 4, 'DOWN' },     -- -> save 3
    { 2751, 2751, 1, 4, 'CROSS' },    -- save 3
    { 2901, 2901, 1, 4, 'CROSS' },    -- "Continue adventure?" Yes
}
local MILESTONES = {
    { 2350, 'title_menu' },
    { 2420, 'continue_selected' },
    { 2500, 'slot_select' },
    { 2680, 'save_list' },
    { 2880, 'continue_prompt' },
    { 3100, 'loading' },
    { 4031, 'gameplay_start' },
}
return PRESSES, MILESTONES
