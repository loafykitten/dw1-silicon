-- From Master99's save in Jijimon's house (continue-save3; debug fixture made with
-- tools/oracle/debug_fixture.py --keep 45 --keep 46, so the fishing rods stay) out into the yard
-- (screen 204), which then holds the debug actors (docs/phase9-routes.md). Shared by the
-- Master99 profiles (Phase 9, stream fishcurl); derived with oracle/explore.lua. A few presses
-- walk into the partner and do nothing.
local PRESSES = {
    { 4031, 4031, 1, 25, 'RIGHT' },
    { 4056, 4056, 1, 90, 'DOWN' },
    { 4176, 4176, 1, 30, 'RIGHT' },
    { 4206, 4206, 1, 60, 'DOWN' },
    { 4296, 4296, 1, 20, 'UP' },
    { 4316, 4316, 1, 50, 'RIGHT' },
    { 4376, 4376, 1, 15, 'DOWN' },
    { 4396, 4396, 1, 15, 'DOWN' },    -- onto the door's step trigger: out to the yard
}
local MILESTONES = {
    { 4300, 'house' },
    { 4811, 'yard' },
}
return PRESSES, MILESTONES
