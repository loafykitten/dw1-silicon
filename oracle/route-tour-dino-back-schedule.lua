-- Extra schedule for DW1ROUTE_PROFILE=tour-dino-back (Phase 9, stream tour, derived with
-- oracle/explore.lua): after route-debug-room-schedule.lua: Mr. Warp, Very far away, the Ancient
-- Dino Region's back (screen 86, the "meteorite" left alone), south into screen 87 under the bone
-- arch, Saberdramon ("Oh, My!"): the battle, lost; MURD and Jijimon's house (218), control back by
-- ~14600.
local PRESSES = {
    { 6954, 6954, 1, 70, 'UP' },
    { 7024, 7024, 1, 40, 'LEFT' },
    { 7064, 7064, 1, 4, 'CROSS' },
    { 7154, 7154, 1, 4, 'CROSS' },
    { 7244, 7244, 1, 4, 'CROSS' },
    { 7334, 7334, 1, 4, 'CROSS' },
    { 7424, 7424, 1, 4, 'DOWN' },
    { 7454, 7454, 1, 4, 'DOWN' },
    { 7484, 7484, 1, 4, 'CROSS' },
    { 7574, 7574, 1, 4, 'DOWN' },
    { 7604, 7604, 1, 4, 'CROSS' },
    { 8134, 8134, 1, 30, 'DOWN' },
    { 8564, 8564, 1, 10, 'DOWN' },
    { 8574, 8574, 1, 60, 'DOWN' },
    { 8634, 8634, 1, 50, 'DOWN' },
    { 8684, 8684, 1, 40, 'DOWN' },
    { 8724, 8724, 1, 4, 'CROSS' },
    { 8814, 8814, 1, 12, 'DOWN' },
    { 8814, 8814, 1, 12, 'LEFT' },
    { 8826, 8826, 1, 4, 'CROSS' },
    { 8916, 8916, 1, 70, 'DOWN' },
    { 8986, 8986, 1, 90, 'RIGHT' },
    { 9476, 9476, 1, 4, 'CROSS' },
    { 9566, 9566, 1, 4, 'CROSS' },
    { 9656, 9656, 1, 4, 'CROSS' },
    { 9746, 9746, 1, 4, 'CROSS' },
    { 14136, 14136, 1, 4, 'CROSS' },
    { 14226, 14226, 1, 4, 'CROSS' },
    { 14316, 14316, 1, 4, 'CROSS' },
    { 14406, 14406, 1, 4, 'CROSS' },
    { 14496, 14496, 1, 4, 'CROSS' },
    { 14586, 14586, 1, 4, 'CROSS' },
}
local MILESTONES = {
    { 7754, 'warp_menu_very_far' },
    { 8134, 'dino_86' },
    { 8484, 'dino_87' },
    { 8634, 'bone_arch' },
    { 9476, 'saberdramon' },
    { 11036, 'battle' },
    { 13836, 'jijimon_house' },
    { 15000, 'dino_back_done' },
}
return PRESSES, MILESTONES
