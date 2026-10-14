-- Schedule for DW1ROUTE_PROFILE=continue-save2: the continue route (route-continue-schedule.lua)
-- but loading the card's second save (Down on the save list). For a fixture card with two DW1
-- saves in the cwd's memcard1.mcd, like ember-save2-prefight (save 2 stands right before a wild
-- ModokiBetamon on screen 109). Derived with oracle/explore.lua (docs/phase5-plan.md).
local PRESSES = {
    { 700, 1900, 150, 4, 'START' },   -- logos/FMV skip, as the new-game route
    { 2380, 2380, 1, 4, 'DOWN' },     -- title menu: NEW GAME -> CONTINUE GAME
    { 2430, 2430, 1, 4, 'CROSS' },    -- CONTINUE GAME
    { 2500, 2500, 1, 4, 'CROSS' },    -- CONTINUE SLOT 1
    { 2681, 2681, 1, 4, 'DOWN' },     -- save list (card read done by ~2680): save 1 -> save 2
    { 2731, 2731, 1, 4, 'CROSS' },    -- save 2
    { 2931, 2931, 1, 4, 'CROSS' },    -- "Continue adventure?" Yes
}
local MILESTONES = {
    { 2350, 'title_menu' },
    { 2420, 'continue_selected' },
    { 2500, 'slot_select' },
    { 2680, 'save_list' },
    { 2880, 'continue_prompt' },
    { 3100, 'loading' },
    { 3860, 'gameplay_start' },
}
return PRESSES, MILESTONES
