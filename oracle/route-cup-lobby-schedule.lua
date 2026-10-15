-- The start of the cup profiles (Phase 9, stream cup; derived with oracle/explore.lua), after
-- route-arena-start-schedule.lua (its "house" milestone is the lobby here). Fixture
-- p9-cup-lobby: Master99's 2002 save moved by tools/oracle/place_fixture.py into the Arena Lobby
-- (screen 223, where the plaza's exit lands: exit 1, (0, -1383)) at 11:50 on day 11, registered for
-- today's Grade C cup (cup 1; trigger 40, overlevelled, as the game set it in arena-cup).
-- Up to Greymon's desk: "So you've entered today's competition, right? Go in by eleven pm.",
-- "You're not planning to cancel, are you?!", No; at 12:00 the reminder ("You're entered in today's
-- Grade C Tournament, right?"); then to the arena door (step trigger 54), which starts STD
-- (GAME_STATE 5) at about VBlank 5030.
local PRESSES = {
    { 4001, 4001, 1, 110, 'UP' },
    { 4116, 4116, 1, 4, 'CROSS' },    -- Greymon
    { 4266, 4266, 1, 4, 'CROSS' },
    { 4416, 4416, 1, 4, 'CROSS' },
    { 4506, 4506, 1, 4, 'CROSS' },    -- "No, I won't cancel"
    { 4626, 4626, 1, 4, 'CROSS' },    -- the noon reminder
    { 4716, 4716, 1, 4, 'DOWN' },
    { 4720, 4720, 1, 42, 'LEFT' },
    { 4762, 4762, 1, 70, 'UP' },      -- the arena door
}
local MILESTONES = {
    { 4266, 'greymon' },
    { 4626, 'noon_reminder' },
    { 5082, 'cup_title' },
    { 6082, 'bracket' },
}
return PRESSES, MILESTONES
