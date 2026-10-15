-- Extra schedule for DW1ROUTE_PROFILE=cup (Phase 9, stream cup; derived with oracle/explore.lua),
-- after route-arena-start-schedule.lua (its "house" milestone is the lobby here). Fixture
-- p9-cup-lobby: Master99's 2002 save moved by tools/oracle/place_fixture.py into the Arena Lobby
-- (screen 223, where the plaza's exit lands: exit 1, (0, -1383)) at 11:50 on day 11, registered for
-- today's Grade C cup (cup 1; trigger 40, overlevelled, as the game set it in arena-cup).
-- Up to Greymon's desk: "So you've entered today's competition, right? Go in by eleven pm.",
-- "You're not planning to cancel, are you?!", No; at 12:00 the reminder ("You're entered in today's
-- Grade C Tournament, right?"); then to the arena door (step trigger 54), which starts STD. Three
-- rounds on Auto with no input (the first opponent fires a finisher: STD_startQueuedMove's
-- finisher path), the champion scene and STD_deinitializeTournament's fade. The target is just
-- before the first gameLoop frame after the cup: that frame adds DR_OFFSETS[b] to an ordering-table
-- entry that already starts with it (the deinit's last frame put it there), the GPU list loops, and
-- the pinned Redux cuts the chain with a DMA bus error and hangs a few frames later
-- (docs/phase9-routes.md, "A whole cup").
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
    { 9000, 'round1_fight' },          -- the first match: GAME_STATE 4 from VBlank ~7030
    { 11500, 'round1_win' },
    { 15600, 'round2_fight' },         -- ~15140 to ~16270
    { 16500, 'round2_win' },
    { 20000, 'final_fight' },          -- ~18870 to ~23720
    { 24000, 'final_win' },
    { 25000, 'champion' },
    { 25130, 'cup_end' },              -- target: STD_deinitializeTournament's fade, before gameLoop
}
return PRESSES, MILESTONES
