# Phase 8: the Mac-native renderer and extras (plan and progress)

2026-10-14, session 5. PLAN.md's Phase 8: "The `render` crate on wgpu/Metal: render scale, no
wobble, Retina, window modes, gamepads through SDL. Each option labelled original or addition;
1x with extras off still matches the emulator's picture. Widescreen is a feasibility milestone,
not a promise."

Ember asked (2026-10-14) whether to switch to Bevy now so the rendering code wouldn't be replaced
later. Answer, kept: no. The renderer consumes the PS1 draw commands the game makes, and the
Rust game (Phase 10) makes the same commands, so the renderer survives the migration; Bevy's
renderer would need the same custom wgpu code inside it, and its scheduling and frequent
breaking releases fight the deterministic, oracle-checked game loop. A remaster on an engine
could come after the Rust game, as its own project.

## The design: the 1x VRAM stays exact, HD is a side channel

This is DW2-PC's design (`ref/DW2-PC/backend/psxgpu_hd.c`, `pgxp.c`: both DW2-PC's own work,
CC0, eligible in `docs/provenance/dw2pc-inventory.md`), translated into Rust like the 1x GPU.

- `backend::gpu` keeps drawing every primitive into the 1x VRAM exactly as now. The game reads
  only that VRAM (textures, CLUTs, `StoreImage`), every replay compares against it, and with the
  extras off it is the picture.
- With a render scale S above 1, each primitive is also drawn into an **HD surface**: one of the
  game's frame buffers (the two 320x240 rects at (0,0) and (0,240)) kept at S x S pixels per VRAM
  pixel. Textures and CLUTs are always sampled from the 1x VRAM. The window shows the display
  buffer's surface instead of the 1x read-out.
- **No wobble** (PGXP-style): the GTE's projections keep their float result next to the integer
  one; a polygon whose vertices all match a projection of this frame is drawn into the HD surface
  at the precise positions, with perspective-correct texturing. The integer results the game sees
  are unchanged, so game decisions (culling, AI's on-screen test at `btl/battle_main.c:637`,
  camera deltas) are identical at every setting.
- **The gate that keeps this honest:** every certified replay matches with the extras on (scale
  4, no wobble on), not just off. The extras must never reach game state.

### What the survey found (Sol, 2026-10-14; checked samples against the code)

Full report: `docs/phase8-survey.md` (file:line evidence for everything below).

- **No frame buffer is ever read back or sampled.** 15 `StoreImage` and 6 `MoveImage` sites, all
  CLUTs or texture areas (palette fades, animated textures, medal and tournament slots); no
  texture page inside x 0-319; pause dims both buffers and waits, menus compose into the current
  buffer. So the HD surfaces never need to feed back into the 1x VRAM. (A runtime check closes
  the static gap: log any texture sample or transfer source inside a frame buffer.)
- **Layout:** `GsInitGraph(320, 240, non-interlaced)`, buffers (0,0) and (0,240), offsets
  (160,120)/(160,360). Movies display 320x240 RGB24 from (0,0)/(0,240), the picture 320x176 at
  y 32 (all four US STRs).
- **Draw offset changes inside an ordering table** (OT index 0x20 switches from the world origin
  to the fixed UI origin, `main.c:3961-3975`): HD primitives take the drawing state in force
  when each was sent, as the 1x path does.
- **Transfers bypass GP0:** native `LoadImage`/`StoreImage`/`MoveImage` call the GPU directly
  (`crates/psyq/src/libgpu.rs:128-156`). The HD hooks go on `Gpu::load_image`/`move_image`/`fill`,
  not only on the GP0 parser.
- **Projection:** everything goes through `backend::gte`'s `rtps_vertex` (libgs's Rust sorts, the
  C's inline GTE, `RotTransPers*`), so one hook there sees every projected vertex. 74 game-C
  projection sites across 24 files besides libgs.
- **2D things anchored to projected points** (billboard items, bubbles, damage numbers, HP bars,
  markers, shadows, menus over the partner): they stay at their integer positions. With no wobble
  on, a model may sit a fraction of a PS1 pixel from its shadow or bubble; whether that shows at
  4x is for Ember to judge, and the fix (carrying a sub-pixel anchor) is a later option.
- **Dithering is not off everywhere:** lines, flat overlays and `GsSortBoxFill` set it. HD keeps
  the PS1 dither pattern at PS1 pixel size unless a "full colour" option says otherwise.
- **Text** is 4-bit glyphs in an atlas at (704,256) drawn as `POLY_FT4` at native size: nearest
  sampling keeps it sharp-edged at any scale.
- **Widescreen** obstacles (for 8.6): the map is up to 12 `POLY_FT4` of 128x128 from a 512x384
  tile cache in VRAM x 768-1023; the camera is clamped to `0..width*128-320`; culling of NPCs,
  overlays and particles is against 320x240 (and some of it feeds game logic); fades, flashes,
  iris wipes, mist and battle backdrops are authored 320 wide; UI is centred 320x240.

## Slices

Each slice: a brief, implemented (main session or a Sonnet subagent), reviewed by gpt-6.1-sol
(high) until no finding is left (Astra for 8.2 and 8.3: the GPU layer is touchy), checked as
below. Order matters for 8.1-8.3; 8.4-8.6 can follow in any order.

### 8.1 wgpu presenter (`crates/render`)

Replace SDL's 2D canvas with wgpu 30 on Metal (SDL3 keeps the window, input and audio; the
surface comes from the window's raw handle, as the S0.6 spike did). Upload the displayed picture
as a texture, draw it with a nearest (integer scale) or sharp-bilinear (fit) shader.

- Window modes (labelled additions): windowed and fullscreen (Cmd-Ctrl-F or F11);
  integer scale or fit to the window; 4:3 kept (the PS1's 320x240 on a 4:3 TV: pixels are not
  square at 320 wide either, so "4:3" and "square pixels" are both offered, 4:3 the default).
- Retina: the drawable is the window's pixel size.
- Present mode chosen so the game's own field clock paces frames (no Fifo blocking at 59.817 on
  a 60/120/240 Hz display); measure what a 120 Hz ProMotion display does.
- **Check:** an offscreen render of a known picture at 1x with integer scale read back equals
  the input pixel for pixel (`render` test); the window shows what `read_display` gives.

### 8.2 HD surfaces, software (`backend::hd`)

Translate `psxgpu_hd.c`'s software path (surfaces, the primitive queue, drawing threads, fills,
refreshes from VRAM, display read-out), without the GPU renderer, 16:9, sharpening and smooth
frames. Hooks in `backend::gpu` after each 1x triangle, rectangle, line, fill, upload and copy
(direct calls included). Surfaces registered from `PutDrawEnv`/`PutDispEnv` and libgs's buffer
swaps. Movies (RGB24) scale the 1x read-out.

- Scale 1-8 (labelled addition), default from the window size once 8.4 lands.
- **Checks:** (a) all ten replays identical with scale 4 (`tools/replay-check.sh` with
  `DW1_SCALE=4`); (b) headless HD shots: on 2D-only frames (menus, text, the landing screen) the
  first pixel of every S x S block equals the 1x picture; on 3D frames, differences only on
  polygon edges (counted); (c) frame time at scale 4 and 8 in the release window build, with the
  game at 59.817 fields a second and room to spare on the M2 Max.

### 8.3 No wobble

The precise hook in `backend::gte` (each RTPS/RTPT vertex: integer SXY, float x/y, view depth),
the per-frame lookup by integer position (`pgxp.c`'s rules: several points at one position pick
the one whose depth fits the polygon's other corners), dropped at each `DrawOTag`. Polygons with
every vertex known go to the HD surface precise, with perspective-correct UVs.

- Labelled addition, needs scale above 1 to matter (at 1x it would change the original picture,
  so it's off there).
- **Checks:** replays identical with it on; a side-by-side of the new-game scene and a battle at
  4x, wobble on and off, for Ember; a test that the 1x VRAM is byte-identical with it on and off.

### 8.4 Settings and options

`~/Library/Application Support/dw1-silicon/settings.toml` (render scale or "match window", no
wobble, window mode, picture shape, full colour), with hotkeys to change them in play and the
window title or a short on-screen note saying what changed. Every option says whether it's
original or an addition. Gamepads stay SDL's (already in); add remapping only if Ember asks.
"Full colour" (HD only, addition): blend and shade at 8 bits per channel without the 15-bit cut
and the dither.

### 8.5 GPU renderer (only if 8.2 is too slow)

The HD queue drawn by wgpu instead of the CPU threads (`psxgpu_hw.c` as the reference: the 1x
VRAM uploaded as a texture by dirty rows, the PS1 blend modes and mask bit in the fragment
shader). Done only if 8.2 can't hold the field rate at the scales Ember wants.

### 8.6 Widescreen feasibility (a report, no promise)

Try 16:9 on the map, a battle and a menu with the HD surfaces widened (DW2-PC's margins): how
much of each map's art exists past the 320 columns, what the 4x3 tile cache and the camera clamp
cut, which culling feeds game logic (and so can't be widened without changing the game), what
the authored-320 effects do. Output: `docs/phase8-widescreen.md` with pictures for Ember and a
recommendation (likely: 16:9 where the art exists, pillarbox elsewhere, or not at all).

## Done when

1x with the extras off matches the emulator's picture (the presenter adds nothing); every replay
matches with every extra on; scale, no wobble, window modes and Retina work in `dw1-mac` and
Ember has looked at them; the widescreen report is written. Then Phase 9.

## Progress

- 2026-10-14: plan written from Sol's survey of the drawing code and DW2-PC's HD renderer.
- **8.1 done** (c68a300): `crates/render`, wgpu 30 on Metal. The picture is a Bgra8Unorm texture
  drawn with a sharp-bilinear shader into a non-sRGB window: whole-number scales are the picture
  pixel for pixel (offscreen readback tests at 1-4x). Fifo, one frame of latency; `ShowGate`
  drops frames rather than wait on displays slower than the field rate. The splash draws on an
  SDL software canvas through the same renderer. Sol reviewed it three times. Windows were
  "occluded" for part of the session (macOS reports them hidden; the S0.6 spike couldn't draw
  either), so frames went unshown then; when visible, 1 skipped frame in 545.
- **8.2 done** (aa4c969, Sonnet subagent): `backend::hd`. `DW1_SCALE=n`, `DW1_HD_THREADS`,
  `DW1_SHOTS` at scale with `DW1_SHOTS_1X=1`, `tools/hd_compare.py`. All ten replays identical
  at scale 4. Timing (release, new-game replay): 9.6 s at 1x, 14.8 s at 4x, 21.9 s at 8x; per
  field at 4x a 0.25 ms mean, 6.2 ms worst. Astra found eight issues (UVs on 2D quads such as map
  tiles floored instead of the 1x rounding, lines, odd-row tails, wrapped transfers, overflow,
  `vram_mut`, overlapping surfaces), all fixed in 9f08b6c: non-precise polygons and lines now
  take colour, texture coordinates and coverage from the 1x rasterizer per 1x pixel, so at scale
  4 the first pixel of every block equals the 1x picture on all 267 new-game shots (3D frames
  included; before: up to 18,000 differing). HD detail comes from edges and, with no wobble, from
  precise polygons. Astra's second pass: one overflow on huge precise triangles, fixed (128-bit
  attribute start). Timing after the fixes: 15.6 s at 4x (worst field 3.6 ms), 22.6 s at 8x
  (one field 19 ms).
- **8.3 done** (cfada45, c5afad7, 5b920a3): `backend::pgxp` and the GTE's report. Each corner is
  precise on its own (shared edges stay closed) and sprite-like mappings keep integer corners.
  The table is cleared after each drawn ordering table (DW1 projects everything, then draws once
  per frame; menu OTs are spliced into the main one). All ten replays identical with
  `DW1_SCALE=4 DW1_PGXP=1`. Astra's review fixed: per-vertex saturation in RTPT, the divider's
  own overflow test, depth in the duplicate test.
- **8.4 done** (d975a06): `settings.txt` with render_scale (auto from the window's size), no_wobble,
  fit, fullscreen; F2/F3/F4/F11 with a note in the corner. Defaults: auto, no wobble on, integer
  fit. "Full colour" is not built yet.
- **8.5 not needed** so far: software HD holds the field rate at 4x and mostly at 8x.
- **8.6 done:** `docs/phase8-widescreen.md` (selective, not now).
- **Ember's look (2026-10-14):** the defaults look great, the hotkeys and fit work. No wobble off
  at a high scale looked awful (3D models textured block by block at 1x, the 2D-exact rule
  applied to everything); now only untextured and sprite-like polygons keep it, other textured
  polygons interpolate smoothly (`smooth` in `hd.rs`). 2D frames still 0 differences from 1x;
  replays identical; Sol found nothing.
- **8.4 full colour done:** `full_colour` in `settings.txt`, F5, `DW1_FULL_COLOUR=1` (default
  on since Ember's look: "I can barely tell the difference in color so let's leave it on";
  headless runs keep it off unless asked). HD surfaces now hold 8 bits a channel and a mask bit (`u32`); with full
  colour off every pixel is the expanded 15-bit value and the 15-bit rules run on it, so the HD
  picture is byte-identical to the previous build (426 new-game shots at 4x with no wobble); with
  it on the same rules run on 8-bit channels without the cut and the dither (a rectangle's texel
  saturates before it blends, a polygon's after, as at 1x). The 1x VRAM is the same either way;
  all ten replays identical with `DW1_SCALE=4 DW1_PGXP=1 DW1_FULL_COLOUR=1`. The display read-out
  is now a plain copy, so a field at 8x costs less than before (about 1.2 ms against 1.65 ms,
  draw and read-out, new-game replay). Astra found two issues (a rectangle's texel blended
  unsaturated; a pixel written after the option was turned off could keep its full-colour value),
  both fixed with tests; second pass clean. Side-by-sides for Ember in the gitignored
  `captures/phase8-full-colour/`.
