-- Extra schedule for DW1ROUTE_PROFILE=battle-mode (Phase 9, derived with oracle/explore.lua):
-- from the title menu with two cards (memcard1.mcd p9-arena-vs-p1, BeelzemontZero's save;
-- memcard2.mcd p9-arena-vs-p2, Bob's), BATTLE MODE, player 1's card prompt Yes, record 1,
-- "Read record?" Yes; player 2 (pad 2, which the profile plugs in) does the same on card 2.
-- Then 1 vs 1, the first stage, each player picks with Cross, both press Start: the VS overlay's
-- intro (left to play out), the fight on Auto (player 1's 9999 HP Digimon wins at ~8900), "1P
-- WIN", and the title menu again by ~9700.
local PRESSES = {
    { 2351, 2351, 1, 4, 'DOWN' },
    { 2381, 2381, 1, 4, 'DOWN' },
    { 2411, 2411, 1, 4, 'DOWN' },
    { 2461, 2461, 1, 4, 'CROSS' },     -- BATTLE MODE
    { 2611, 2611, 1, 4, 'CROSS' },     -- player 1's card: Yes
    { 2841, 2841, 1, 4, 'CROSS' },     -- record 1
    { 3021, 3021, 1, 4, 'CROSS' },     -- "Read record?" Yes
    { 3551, 3551, 1, 4, 'P2_CROSS' },  -- player 2's card: Yes
    { 3801, 3801, 1, 4, 'P2_CROSS' },  -- record 1 (Bob, MegaSeadramon)
    { 3891, 3891, 1, 4, 'P2_CROSS' },  -- "Read record?" Yes
    { 4421, 4421, 1, 4, 'CROSS' },     -- BATTLE MODE SELECT: 1 vs 1
    { 4511, 4511, 1, 4, 'CROSS' },     -- STAGE SELECT: the first coliseum
    { 4631, 4631, 1, 4, 'CROSS' },     -- player 1 picks
    { 4661, 4661, 1, 4, 'P2_CROSS' },  -- player 2 picks
    { 4721, 4721, 1, 4, 'START' },     -- player 1: Start
    { 5051, 5051, 1, 4, 'P2_START' },  -- player 2: Start
}
local MILESTONES = {
    { 3540, 'p2_card_prompt' },
    { 3800, 'p2_record_list' },
    { 4420, 'mode_select' },
    { 4720, 'fighter_select' },
    { 5681, 'intro' },
    { 6881, 'fight' },
    { 8381, 'fight_late' },
    { 9381, 'result' },
    { 9800, 'vs_done' },               -- target: back at the title menu
}
return PRESSES, MILESTONES
