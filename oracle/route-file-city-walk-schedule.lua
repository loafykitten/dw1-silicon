-- Extra schedule for DW1ROUTE_PROFILE=file-city-walk: appended to the new-game route (first
-- gameplay frame, tamer standing outside Jijimon's house, screen 204, VBlank 19400). Derived with
-- oracle/explore.lua (turn-based, screens read from screenshots; docs/phase4-plan.md). The walk,
-- with the screen changes it makes (screen 204 is the yard, 180 the File City plaza, 218 the
-- inside of Jijimon's house):
--   19401 Down 210       204 -> 180 (about 19600, "File City" caption, control back ~19800)
--   20011 Up 90          180 -> 204 (about 20130; control back ~20400)
--   20431 Up 200, Left 40, Up 40   into the house door: 204 -> 218 (about 20760, control ~21060)
--   21061-21470          a few steps inside the house, then out of the door: 218 -> 204
--   21871-23320          Down: through the yard and the passage again, 204 -> 180
--   23321 Down 200       a few steps across the plaza; the target is VBlank 23380
-- The plaza's west exit (screen 109) starts a battle with a wild Agumon: stay out of it.
local PRESSES = {
    { 19401, 19401, 1, 210, 'DOWN' },
    { 20011, 20011, 1, 90, 'UP' },
    { 20431, 20431, 1, 200, 'UP' },
    { 20661, 20661, 1, 40, 'LEFT' },
    { 20701, 20701, 1, 40, 'UP' },
    { 21061, 21061, 1, 60, 'UP' },
    { 21141, 21141, 1, 60, 'LEFT' },
    { 21201, 21201, 1, 40, 'DOWN' },
    { 21261, 21261, 1, 30, 'RIGHT' },
    { 21291, 21291, 1, 120, 'DOWN' },
    { 21431, 21431, 1, 40, 'RIGHT' },
    { 21471, 21471, 1, 60, 'DOWN' },
    { 21871, 21871, 1, 200, 'DOWN' },
    { 22521, 22521, 1, 150, 'DOWN' },
    { 23121, 23121, 1, 200, 'DOWN' },
}
local MILESTONES = {
    { 19700, 'walk_plaza_caption' },
    { 20200, 'walk_back_to_yard' },
    { 20800, 'walk_house_caption' },
    { 21100, 'walk_house_inside' },
    { 21700, 'walk_out_of_house' },
    { 23200, 'walk_plaza_again' },
    { 23380, 'walk_done' },            -- target
}
return PRESSES, MILESTONES
