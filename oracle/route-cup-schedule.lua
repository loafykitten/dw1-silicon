-- Extra schedule for DW1ROUTE_PROFILE=cup (Phase 9, stream cup), after
-- route-arena-start-schedule.lua and route-cup-lobby-schedule.lua (the lobby, Greymon, the noon
-- reminder, the arena door). Three rounds on Auto with no input (the first opponent fires a
-- finisher: STD_startQueuedMove's finisher path), the champion scene and
-- STD_deinitializeTournament's fade. The target is just before the first gameLoop frame after the
-- cup: that frame adds DR_OFFSETS[b] to an ordering-table entry that already starts with it (the
-- deinit's last frame put it there), the GPU list loops, and the pinned Redux cuts the chain with a
-- DMA bus error and hangs a few frames later (docs/phase9-routes.md, "A whole cup").
local PRESSES = {}
local MILESTONES = {
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
