-- Extra schedule for DW1ROUTE_PROFILE=register-save (Phase 7): appended to the continue route
-- (fixture ember-day2-gabumon in the cwd's memcard1.mcd; screen 168, in front of Jijimon's
-- house). Derived with oracle/explore.lua from the continue route's end (5000): up into
-- Jijimon's house (screen 218), to the Battle Registration machine, "What's this machine?",
-- Jijimon's explanation, "Do you want to use Save Machine for Memory Card Battle Data?" Yes; the
-- registration screen, "Do you want to register?" No; "Do you want to save?" Yes: the game saves
-- to the slot it was loaded from (main_menu.c's save machine, tickSaveMachine), the in-game save
-- path the other routes don't reach. Control back by ~7100. Ember's tip, 2026-10-14.
local PRESSES = {
    { 5001, 5001, 1, 90, 'UP' },
    { 5481, 5481, 1, 40, 'LEFT' },
    { 5541, 5541, 1, 20, 'UP' },
    { 5591, 5591, 1, 25, 'LEFT' },
    { 5631, 5631, 1, 6, 'DOWN' },
    { 5661, 5661, 1, 4, 'CROSS' },     -- "What's this machine?"
    { 5821, 5821, 1, 4, 'CROSS' },
    { 5881, 5881, 1, 4, 'CROSS' },
    { 5941, 5941, 1, 4, 'CROSS' },
    { 6001, 6001, 1, 4, 'CROSS' },
    { 6061, 6061, 1, 4, 'CROSS' },
    { 6121, 6121, 1, 4, 'CROSS' },
    { 6181, 6181, 1, 4, 'CROSS' },
    { 6411, 6411, 1, 4, 'CROSS' },     -- use the Save Machine: Yes
    { 6741, 6741, 1, 4, 'DOWN' },      -- "Do you want to register?" No
    { 6781, 6781, 1, 4, 'CROSS' },
    { 7011, 7011, 1, 4, 'CROSS' },     -- "Do you want to save?" Yes
}
local MILESTONES = {
    { 5421, 'jijimon_house' },
    { 5791, 'machine_talk' },
    { 6411, 'machine_prompt' },
    { 6741, 'registration' },
    { 7011, 'save_prompt' },
    { 7071, 'saving' },
    { 7400, 'saved' },                 -- target: control back
}
return PRESSES, MILESTONES
