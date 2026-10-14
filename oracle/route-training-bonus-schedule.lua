-- Extra schedule for DW1ROUTE_PROFILE=training-bonus: training-fixture (speed training, control
-- back at ~9785), then the HP sign ("Training: Boulder Moving", tile trigger 81, at about x 896,
-- z -1250; around the heart sign), Begin, Bonus Try: the slot game (TRN's trn_slots.c; three
-- Cross presses stop the reels), the session, HP +176 and Offense +4; results closed, control back
-- by ~11990 (a Bonus Try doesn't ask again). Derived with oracle/explore.lua.
local PRESSES = {
    { 9901, 9901, 1, 51, 'RIGHT' },
    { 9957, 9957, 1, 75, 'DOWN' },
    { 10052, 10052, 1, 20, 'LEFT' },   -- around the heart sign
    { 10077, 10077, 1, 40, 'DOWN' },
    { 10122, 10122, 1, 20, 'RIGHT' },  -- onto trigger 81
    { 10162, 10162, 1, 4, 'CROSS' },   -- the sign
    { 10282, 10282, 1, 4, 'CROSS' },   -- Begin / Explain / Quit
    { 10342, 10342, 1, 4, 'CROSS' },   -- Begin -> Normal / Bonus Try / Quit
    { 10672, 10672, 1, 4, 'DOWN' },
    { 10712, 10712, 1, 4, 'CROSS' },   -- Bonus Try: the slot game
    { 11142, 11142, 1, 4, 'CROSS' },   -- the slots: start, then three reels
    { 11232, 11232, 1, 4, 'CROSS' },
    { 11302, 11302, 1, 4, 'CROSS' },
    { 11372, 11372, 1, 4, 'CROSS' },
    { 11662, 11662, 1, 4, 'CROSS' },   -- close the results
}
local MILESTONES = {
    { 10282, 'bonus_sign' },
    { 11142, 'bonus_slots' },
    { 11602, 'bonus_results' },
    { 12100, 'bonus_done' },           -- target: control back
}
return PRESSES, MILESTONES
