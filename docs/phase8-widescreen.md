# Phase 8.6: widescreen feasibility (report)

2026-10-14, session 5. PLAN.md: "Widescreen is a feasibility milestone, not a promise". This is
the report; nothing is built. Evidence (file:line, every map's size from the disc's headers):
`docs/phase8-widescreen-survey.md` (gpt-6.1-sol, read-only; sampled against the code).

## Short answer

16:9 can be added faithfully for some scenes, never by just widening the picture, and the PS1
game must not notice. Best first: the 3D tournament and Battle Mode arenas. Ordinary maps work
in their middles but show black at their edges. Title screens, menus and movies should stay 4:3
(pillarboxed). Worth doing only scene by scene, after the rest of Phase 8.

## What 16:9 means here

At 240 lines, 16:9 is 426.67 PS1 pixels wide: 53.33 more on each side of the 320 the game
draws. The plan is DW2-PC's: the HD surfaces get those margins, the camera and every number the
game computes stay as they are, and only the HD side draws into the margins. The 1x VRAM, the
replays and the RNG must be identical with it on or off.

## What's in the way

- **Map backgrounds are pre-drawn 128x128 tiles.** 241 maps: 19 are 3 tiles wide (384 pixels,
  too narrow for 426 at any camera position), the rest 4 to 10 wide. The camera stops 320
  pixels from each map edge, so at the edges a 16:9 view has 53 pixels with no art at all
  (black, or pillarbox there).
- **The game keeps 4 tile columns in VRAM; 16:9 can need 5.** The HD side would have to draw the
  fifth itself, from the map's tile pixels captured when the map loads (the game's copy in RAM
  is overwritten by battles and sound loading). Possible without touching what the game writes.
- **"On screen" feeds the game.** The game's 320x240 visibility tests decide which NPCs join a
  battle (and so which random numbers are drawn), enemy AI charge modes (a `randomLimit` call),
  collisions that start scripts, the partner's off-screen catch-up, battle intro and exit
  timing, and effect targets. None of these may change. Extra actors in the margins would have
  to be drawn by the HD side from existing state, and they'd still vanish, teleport or stop at
  the original 4:3 edges, which 16:9 would make visible.
- **Full-screen effects are authored 320 wide:** fades, battle flashes, iris wipes, mist and
  snow, battle backdrops, the pause dim. Each needs its margins covered (without running the
  weather simulation more: snow uses the shared RNG).
- **Title screens and menus** are 320-wide pictures and layouts; **movies** are 320x176.
  Nothing more exists to show.

## Recommendation

1. Don't build widescreen now. Finish Phase 8 (HD fixes, Ember's look at scale and no wobble),
   then Phase 9's playthrough.
2. If Ember wants it later: start with the STD/VS arenas (real 3D stages, the margins fill
   naturally), then an allowlist of large maps with captured tiles, pillarboxing everything
   else and every unverified scene. Gate each scene on identical replays with it on.
3. Labelled an addition, off by default, with pillarbox as the fallback everywhere.
