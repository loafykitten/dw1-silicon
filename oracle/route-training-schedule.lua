-- Extra schedule for DW1ROUTE_PROFILE=training-fixture: appended to continue-save2 (fixture
-- ember-save2-prefight; screen 109, control at ~3860). Derived with oracle/explore.lua
-- (docs/phase5-plan.md): up into the Green Gym (screen 112, which loads the TRN overlay), walk to
-- the "Training: Running" sign (tile trigger 85, at about x 25, z -99), read it, Begin, Normal:
-- Gabumon runs laps for a game hour (TRN's speed training; the clock stops while it runs and
-- jumps by an hour), the results show Speed 60 -> 67; close them, the sign asks again, Quit;
-- control back by ~9785.
local PRESSES = {
    { 3861, 3861, 1, 60, 'UP' },      -- into screen 112
    { 4221, 4221, 1, 205, 'UP' },
    { 4436, 4436, 1, 29, 'LEFT' },    -- onto the sign's trigger tiles
    { 4485, 4485, 1, 4, 'CROSS' },    -- "Sign: Training: Running"
    { 4605, 4605, 1, 4, 'CROSS' },    -- Begin / Explain / Quit
    { 4665, 4665, 1, 4, 'CROSS' },    -- Begin -> Normal / Bonus Try / Quit
    { 4995, 4995, 1, 4, 'CROSS' },    -- Normal: training starts
    { 9085, 9085, 1, 4, 'CROSS' },    -- close the results
    { 9415, 9415, 1, 4, 'CROSS' },    -- the sign again
    { 9475, 9475, 1, 4, 'DOWN' },
    { 9515, 9515, 1, 4, 'DOWN' },
    { 9555, 9555, 1, 4, 'CROSS' },    -- Quit
}
local MILESTONES = {
    { 4485, 'trn_sign' },
    { 5325, 'trn_start' },
    { 6825, 'trn_laps' },
    { 9025, 'trn_results' },
    { 9900, 'trn_done' },             -- target: control back
}
return PRESSES, MILESTONES
