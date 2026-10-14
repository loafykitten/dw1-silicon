-- Schedule for the CONTINUE route (DW1ROUTE_PROFILE=continue): boot with a fixture card in the
-- cwd's memcard1.mcd (one DW1 save in slot 1), skip the logos, title menu CONTINUE GAME, slot 1,
-- save 1, "Continue adventure?" Yes, load into File City, then no input. Same format as the
-- tables in route-new-game.lua. Screens were read from screenshots (docs/oracle-route.md).
local PRESSES = {
    { 700, 1900, 150, 4, 'START' },   -- logos/FMV skip, as the new-game route
    { 2380, 2380, 1, 4, 'DOWN' },     -- title menu: NEW GAME -> CONTINUE GAME
    { 2430, 2430, 1, 4, 'CROSS' },    -- CONTINUE GAME
    { 2500, 2500, 1, 4, 'CROSS' },    -- CONTINUE SLOT 1
    { 2700, 2700, 1, 4, 'CROSS' },    -- save 1 (card read done by ~2680)
    { 2900, 2900, 1, 4, 'CROSS' },    -- "Continue adventure?" Yes
}
local MILESTONES = {
    { 2350, 'title_menu' },
    { 2420, 'continue_selected' },
    { 2500, 'slot_select' },
    { 2680, 'save_list' },
    { 2850, 'continue_prompt' },
    { 3100, 'loading' },
    { 3800, 'gameplay_start' },
    { 5000, 'gameplay' },             -- target: no input after 2900
}
return PRESSES, MILESTONES
