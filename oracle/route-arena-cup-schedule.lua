-- Extra schedule for DW1ROUTE_PROFILE=arena-cup (Phase 9, arena stream; derived with
-- oracle/explore.lua), after route-arena-start-schedule.lua (Master99's save, fixture
-- p9-arena-master99, MetalGreymon "Ultima", day 11 08:25 outside Jijimon's house, screen 179).
-- Out of the house and south into the plaza (192), east along its south side into the Arena
-- Lobby (223, step trigger 57). At the ENTRY desk Greymon has the save's entry for tomorrow:
-- cancel it, then enter today's Grade C cup (Champion Digimon, three rounds; an Ultimate may
-- enter). Idle in the lobby until 12:00, when Greymon reminds us; walk through the arena door
-- (trigger 54): "Tamer Level went down!", the STD overlay (GAME_STATE 5): title, bracket, the
-- first match's intro, the fight on Auto with the finisher charged and fired with Square (FIN),
-- the win (WIN_LOSE.TMD), the bracket again with the other matches simulated, and the second
-- round's intro. A whole cup doesn't fit under VBlank ~32700 (each match is about 6000).
local PRESSES = {
    { 4031, 4031, 1, 25, 'RIGHT' },
    { 4056, 4056, 1, 90, 'DOWN' },
    { 4176, 4176, 1, 30, 'RIGHT' },
    { 4206, 4206, 1, 60, 'DOWN' },
    { 4296, 4296, 1, 20, 'UP' },
    { 4316, 4316, 1, 50, 'RIGHT' },
    { 4376, 4376, 1, 15, 'DOWN' },
    { 4396, 4396, 1, 15, 'DOWN' },
    { 4811, 4811, 1, 190, 'DOWN' },
    { 5402, 5402, 1, 60, 'DOWN' },
    { 5465, 5465, 1, 20, 'RIGHT' },
    { 5488, 5488, 1, 35, 'DOWN' },
    { 5524, 5524, 1, 274, 'DOWN' },
    { 5801, 5801, 1, 45, 'RIGHT' },
    { 5849, 5849, 1, 25, 'DOWN' },
    { 6274, 6274, 1, 110, 'UP' },
    { 6387, 6387, 1, 4, 'CROSS' },
    { 6567, 6567, 1, 4, 'CROSS' },
    { 6627, 6627, 1, 4, 'CROSS' },
    { 6687, 6687, 1, 4, 'CROSS' },
    { 6747, 6747, 1, 4, 'CROSS' },
    { 6807, 6807, 1, 4, 'DOWN' },
    { 6837, 6837, 1, 4, 'CROSS' },
    { 6927, 6927, 1, 4, 'CROSS' },
    { 7017, 7017, 1, 4, 'CROSS' },
    { 7197, 7197, 1, 4, 'CROSS' },
    { 7287, 7287, 1, 4, 'CROSS' },
    { 7407, 7407, 1, 4, 'CROSS' },
    { 7497, 7497, 1, 4, 'CROSS' },
    { 7587, 7587, 1, 4, 'CROSS' },
    { 19557, 19557, 1, 4, 'CROSS' },
    { 19647, 19647, 1, 4, 'DOWN' },
    { 19654, 19654, 1, 42, 'LEFT' },
    { 19699, 19699, 1, 70, 'UP' },
    { 20069, 20069, 1, 4, 'CROSS' },
    { 23499, 23499, 1, 4, 'SQUARE' },
    { 23729, 23729, 1, 10, 'SQUARE' },
    { 24539, 24539, 1, 4, 'SQUARE' },
}
local MILESTONES = {
    { 4811, 'outside' },
    { 5402, 'plaza' },
    { 6274, 'arena_lobby' },
    { 6567, 'greymon' },
    { 7497, 'grade_c' },
    { 7707, 'registered' },
    { 19557, 'noon' },
    { 20069, 'arena_door' },
    { 20900, 'bracket' },
    { 22899, 'match1_fight' },
    { 23499, 'finisher_ready' },
    { 25769, 'match1_late' },
    { 26269, 'match1_win' },
    { 28069, 'round2_intro' },        -- target
}
return PRESSES, MILESTONES
