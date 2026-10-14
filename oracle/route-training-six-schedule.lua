-- Extra schedule for DW1ROUTE_PROFILE=training-six: training-all (control back at 21100 at
-- (-274, -1150)), then Bonus Tries at the last two signs: "Save2's Classroom" (trigger 86, named
-- after the tamer; MP +5, Brains +4) and "Waterfall" (82, along the top of the gym; MP +40,
-- Defense +1). With training-fixture and training-bonus that is all six of TRN's kinds. Derived
-- with oracle/explore.lua started from training-all's end (route-driven up to there, so it
-- matches the recording).
local PRESSES = {
    { 21101, 21101, 1, 66, 'LEFT' },
    { 21172, 21172, 1, 75, 'UP' },
    { 21252, 21252, 1, 60, 'UP' },
    { 21332, 21332, 1, 4, 'CROSS' },
    { 21452, 21452, 1, 4, 'CROSS' },
    { 21512, 21512, 1, 4, 'CROSS' },
    { 21842, 21842, 1, 4, 'DOWN' },
    { 21882, 21882, 1, 4, 'CROSS' },
    { 22312, 22312, 1, 4, 'CROSS' },
    { 22402, 22402, 1, 4, 'CROSS' },
    { 22472, 22472, 1, 4, 'CROSS' },
    { 22542, 22542, 1, 4, 'CROSS' },
    { 22832, 22832, 1, 4, 'CROSS' },
    { 23262, 23262, 1, 43, 'DOWN' },
    { 23325, 23325, 1, 181, 'RIGHT' },
    { 23526, 23526, 1, 4, 'CROSS' },
    { 23646, 23646, 1, 4, 'CROSS' },
    { 23706, 23706, 1, 4, 'CROSS' },
    { 24036, 24036, 1, 4, 'DOWN' },
    { 24076, 24076, 1, 4, 'CROSS' },
    { 24506, 24506, 1, 4, 'CROSS' },
    { 24596, 24596, 1, 4, 'CROSS' },
    { 24666, 24666, 1, 4, 'CROSS' },
    { 24736, 24736, 1, 4, 'CROSS' },
    { 25026, 25026, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 21452, 'six_classroom' },
    { 22772, 'six_classroom_results' },
    { 23646, 'six_waterfall' },
    { 24966, 'six_waterfall_results' },
    { 25600, 'six_done' },              -- target: control back
}
return PRESSES, MILESTONES
