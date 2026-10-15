-- The end of the game (Phase 9, the ending stream), appended to continue-save3 with the fixture
-- captures/fixtures/p9-ending-mt-infinity.mcd (tools/oracle/place_fixture.py on Master99's 2001
-- GameFAQs save: moved to Mt. Infinity's last corridor, screen 222, where the warp from screen 254
-- lands, with MetalGreymon's flag 326 set; docs/phase9-routes.md). Up the corridor into the boss
-- room (225); Analogman's scene (script 184, section 0x33), Machinedramon's buildup and spawn (the
-- EAB overlay: EAB_startBuildup, then tamer state 18, EAB_tick), the battle (Square for the
-- finisher), the scene after it, the warp to the ending map (237), Jijimon's game-clear save
-- (written to the card), the ending (the ENDI overlay: tamer state 16), the credits movie
-- (it plays to its end: it can't be skipped), the title again, CONTINUE GAME with the clear save, the return movie
-- skipped, and back in File City ("I'm back!"). Found with oracle/explore.lua
-- (captures/oracle-runs/p9-ending-explore3). Ends at VBlank 31900, under the pinned Redux's Lua
-- limit near 32768.
local PRESSES = {
    { 4000, 4000, 1, 540, 'UP' },        -- the corridor (222) to the boss room's warp
    { 4940, 4940, 1, 140, 'UP' },        -- into Analogman's trigger (225)
    { 5180, 8880, 50, 4, 'CROSS' },      -- the scene's lines, the buildup and spawn, to the battle
    { 13510, 13510, 1, 4, 'SQUARE' },    -- the finisher ("FIN")
    { 13740, 13740, 1, 4, 'SQUARE' },
    { 14170, 19620, 50, 4, 'CROSS' },    -- results, the scene, the warp, the clear save (Yes)
    { 19730, 20430, 50, 4, 'CROSS' },    -- the last lines, the ending
    { 28230, 28230, 1, 4, 'START' },     -- (the credits movie can't be skipped: MOV_playMovie
                                         -- ignores the pad for movie 3; it ends at ~28300)
    { 28560, 28560, 1, 4, 'START' },     -- "Press Start Button"
    { 28790, 28790, 1, 4, 'DOWN' },      -- title menu: CONTINUE GAME
    { 28820, 28820, 1, 4, 'CROSS' },
    { 28890, 28890, 1, 4, 'CROSS' },     -- CONTINUE SLOT 1
    { 29120, 29120, 1, 4, 'DOWN' },      -- the save list: the third save (now the clear save)
    { 29150, 29150, 1, 4, 'DOWN' },
    { 29180, 29180, 1, 4, 'CROSS' },
    { 29380, 29380, 1, 4, 'CROSS' },     -- "Continue adventure?" Yes
    { 30410, 30410, 1, 4, 'START' },     -- skip the return movie (movie 2)
    { 31250, 31700, 50, 4, 'CROSS' },    -- "I'm back!"
}
local MILESTONES = {
    { 4940, 'boss_room' },
    { 5930, 'analogman' },
    { 7430, 'machinedramon_spawn' },
    { 9530, 'battle' },
    { 14170, 'battle_won' },
    { 18420, 'ending_map' },
    { 19420, 'clear_save' },
    { 20230, 'ending' },
    { 20650, 'credits' },
    { 28560, 'title_again' },
    { 28800, 'title_menu_again' },
    { 30400, 'return_movie' },
    { 31240, 'im_back' },
    { 31900, 'file_city' },
}
return PRESSES, MILESTONES
