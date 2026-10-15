# Digimon World USA: 16:9 feasibility

**Feasible selectively, with a separate HD drawing path; widening the surface/clip alone is insufficient.** Large map interiors have real extra background art. Their borders do not. Ordinary BTL battles retain those 2D backgrounds; STD/VS arenas are stronger 3D candidates. Preserve the original camera, integer projections, visibility flags, simulation and RNG decisions. Pillarbox authored 4:3 screens and unsupported maps/scenes.

Read-only source/data investigation, 2026-10-14. No game execution, screenshots, code changes or disc-file copies. All 242 extracted US `.MAP` headers and corresponding TFS layouts were parsed in memory. Findings establish available tile rectangles, not that every pixel/composition outside the original view is finished art. Source shorthand below: `main/x.c` = `decomp/src/main/x.c`; `btl/`, `std/`, `vs/`, `mov/`, `trn/` likewise live under `decomp/src/`.

## 1. Map dimensions and complete inventory

**Format:** `MAP_ENTRIES[255]` supplies filenames (`main/map.c:458-2754`); `buildMapPath` chooses `MAP/MAP{mapId/15+1}/{filename}` (`:4101-4140`). The `.MAP` first little-endian u32 is the setup offset (`:4033-4040`). Within setup, skip 6 camera s32s, 18 light s32s, 3 ambient s32s, 1 projection s32 and 8 region s32s: **width/height are little-endian s32 at setup+144/+148**, followed by `width*height` s32 tile IDs at +152 (`:4154-4206`). These are 128-pixel tile dimensions, not collision-grid dimensions.

`readMapTFS` reads the companion `.TFS`, which supplies pixels, **not width/height** (`main/map.c:4217-4222`). TFS layout from `setupMap`: one skipped u32, u32 CLUT count, `count*512` palette bytes, then for each non-`-1` map cell one skipped u32 and **16,384 bytes of 128x128 8-bit pixels**; missing cells consume no image record (`:4234-4261`). All 242 file lengths matched `8 + 512*CLUTs + 16388*nonemptyCells` exactly.

| Width tiles | Pixel width | Distinct disc maps |
| ---: | ---: | ---: |
| 3 | 384 | 19 |
| 4 | 512 | 17 |
| 5 | 640 | 76 |
| 6 | 768 | 100 |
| 7 | 896 | 25 |
| 8 | 1024 | 1 |
| 9 | 1152 | 1 |
| 10 | 1280 | 3 |
| **Total** | | **242** |

- **19/242 (7.85%) meet the requested `width*128 <= 320+2*53 = 426` test.** All are width 3. **223** have width >=4; **206** have width >=5 and can potentially require five visible columns.
- The phrase “nothing beyond the 4:3 view at all” needs qualification: **zero maps are <=320 pixels wide**. A 384-wide map has 64 pixels beyond any single 320-wide view, but cannot fill 426.67 pixels at once. Its original camera range is only 0..64. Even optimally centered it lacks **42.67 pixels total**, or **21.33 per side**; at the original left clamp it lacks 53.33 left and has 10.67 spare pixels on the right.
- The table has **244 nonempty slots and 11 empty slots**. **242 slots resolve to existing assets, representing 241 distinct disc maps**: IDs 065 and 232 both name `YAKA25`. ID 065's constructed MAP5 path has no local file, but the game's lookup discards directories and matches the filename (`main/file.c:16-39`); `YAKA25` is present in MAP16 and the US file table (`main/file_table_us.c:504-505`). IDs **050 YAKA01** and **061 YAKA21** have no extracted file or US file-table entry; dimensions are unavailable, not guessed. **MGEN17** exists on disc but has no map-table slot (`main/file_table_us.c:488-489`). Thus counting resolvable slots instead of distinct disc files gives widths 3:19, 4:17, 5:77, 6:99, 7:25, 8:1, 9:1, 10:3.
- **35 `-1` cells across 9 maps:** 008 MAYO10:7; 014 TROP03:2; 025 TUNN02:5; 026 TUNN03:5; 040 GCAN05:5; 126 TUNN03_2:5; 164 GOMI01:2; 208 ROOM13:2; 223 ROOM19:2. Dimensions alone therefore overstate usable art on these maps.

Full compact inventory follows: decimal ID, filename, **width x height in 128-pixel tiles**. Group labels follow the map-table path rule; 065 is the filename-resolution exception above. Empty IDs: 239-246 and 250-252. The extra disc file is explicitly listed.

```text
MAP1:
  000 MAYO01 6x5; 001 MAYO02 7x4; 002 MAYO03 6x5; 003 MAYO04A 6x5
  004 MAYO04B 6x5; 005 MAYO05 5x3; 006 MAYO06 10x2; 007 MAYO11 6x5
  008 MAYO10 6x5; 009 MAYO08A 5x6; 010 MAYO08B 5x6; 011 TROP00 5x3
  012 TROP01 8x3; 013 TROP02 6x5; 014 TROP03 5x6
MAP2:
  015 TROP04 6x5; 016 TROP05 5x6; 017 TROP06 5x6; 018 MIHA00 3x6
  019 MIHA01 10x3; 020 MIHA02 5x6; 021 MIHA03 6x5; 022 MIHA04A 6x5
  023 MIHA04B 6x5; 024 TUNN01 5x4; 025 TUNN02 7x5; 026 TUNN03 7x5
  027 TUNN04 5x4; 028 TUNN05 7x4; 029 TUNN06 3x2
MAP3:
  030 TUNN07 7x4; 031 TUNN08 7x4; 032 TUNN09 3x2; 033 TUNN10 6x4
  034 DGHA01 5x6; 035 DGHA02 5x6; 036 GCAN01 5x6; 037 GCAN02 5x2
  038 GCAN03 5x6; 039 GCAN04 5x6; 040 GCAN05 7x5; 041 GCAN06 6x5
  042 GCAN07 5x4; 043 GCAN08_1 6x4; 044 GCAN09 7x2
MAP4:
  045 OGRE00 3x3; 046 OGRE01 5x3; 047 OGRE02 5x3; 048 OGRE03 5x4
  049 GCAN11 3x2; 050 YAKA01 unavailable; 051 YAKA02 6x5; 052 YAKA11A 6x5
  053 YAKA11B 6x5; 054 YAKA12 5x5; 055 YAKA13 5x6; 056 YAKA14 6x5
  057 YAKA15 5x5; 058 YAKA16 5x5; 059 YAKA17 5x4
MAP5:
  060 YAKA18 5x5; 061 YAKA21 unavailable; 062 YAKA22 6x5; 063 YAKA23 6x5
  064 YAKA24 6x4; 065 YAKA25 5x4; 066 CHKA01 5x4; 067 SAIB01 6x5
  068 SAIB02 5x6; 069 GIAS00 9x3; 070 GIAS01 5x6; 071 GIAS02 5x6
  072 GIAS03 6x5; 073 GIAS04 6x5; 074 GIAS05 6x5
MAP6:
  075 GIAS06A 5x6; 076 GIAS07 5x6; 077 GIAS08 5x6; 078 GIAS09 6x5
  079 KODA00 5x3; 080 KODA01 5x5; 081 KODA02 6x5; 082 KODA03 4x4
  083 KODA04 6x5; 084 KODA05 3x2; 085 KODA06 5x5; 086 KODA07 3x3
  087 KODA08 5x6; 088 FRZL01 6x5; 089 FRZL02 5x6
MAP7:
  090 FRZL03 6x5; 091 FRZL04 4x7; 092 FRZL05 3x2; 093 FRZL06 6x4
  094 FRZL07 7x4; 095 FRZL08 7x4; 096 FRZL12 6x5; 097 ICSA01 5x3
  098 ICSA02 4x7; 099 ICSA03 6x5; 100 ICSA04 5x6; 101 ICSA05 6x5
  102 ICSA06 3x4; 103 ICSA07 6x5; 104 ICSA08 6x5
MAP8:
  105 BETL01 6x5; 106 BETL02 7x4; 107 BETL03 5x4; 108 BETL04 5x4
  109 MAYO00 5x6; 110 MAYO01_2 5x6; 111 MAYO02_2 5x6; 112 TRAI00 5x6
  113 LEOM01 5x4; 114 LEOM02 5x2; 115 MIST01 5x6; 116 MIST02 6x5
  117 MIST03 6x5; 118 MIST04 5x3; 119 MIST05 6x5
MAP9:
  120 MIST06 5x3; 121 MIST07 4x7; 122 TUNN07_2 7x4; 123 TUNN07_3 7x4
  124 TUNN08_2 7x4; 125 TUNN08_3 7x4; 126 TUNN03_2 7x5; 127 GCAN04_2 5x6
  128 GCAN08_2 6x4; 129 GCAN10 5x6; 130 OGRE04 5x6; 131 GIAS06B 5x5
  132 FRZL13 5x6; 133 FRZL14 7x4; 134 FRZL15 6x2
MAP10:
  135 FRZL16 5x6; 136 FRZL17 3x5; 137 FRZL18 3x2; 138 STIC01 6x5
  139 STIC02 5x6; 140 GKYO01 7x4; 141 GKYO02 5x4; 142 OGRE10 5x3
  143 OGRE11 5x3; 144 OMOC01 5x6; 145 OMOC02 5x4; 146 OMOC03 6x5
  147 OMOC04 4x3; 148 OMOC05 3x2; 149 OMOC06 4x2
MAP11:
  150 OMOC07 3x2; 151 OMOC08 3x3; 152 FACT01 7x4; 153 FACT02 5x6
  154 FACT03 6x5; 155 FACT04 5x3; 156 FACT05 3x3; 157 FACT06 4x4
  158 FACT07 5x6; 159 FACT08A 5x6; 160 FACT08B 5x6; 161 FACT09 7x4
  162 FACT10 5x4; 163 FACT11A 7x4; 164 GOMI01 4x8
MAP12:
  165 GOMI02 5x6; 166 MGEN01 6x5; 167 MGEN02 6x5; 168 TWNA02 6x5
  169 TWNA03 6x5; 170 TWNA04 6x5; 171 TWNA05 6x5; 172 TWNA06 6x5
  173 TWNA07 6x5; 174 TWNA08 6x5; 175 TWNA09 6x5; 176 TWNA10 6x5
  177 TWNA11 6x5; 178 TWNA12 6x5; 179 TWNA13 6x5
MAP13:
  180 TWNB01 6x5; 181 TWNB02 6x5; 182 TWNB03 6x5; 183 TWNB04 6x5
  184 TWNB05 6x5; 185 TWNB06 6x5; 186 TWNB07 6x5; 187 TWNB08 6x5
  188 TWNB09 6x5; 189 TWNB10 6x5; 190 TWNB11 6x5; 191 TWNB12 6x5
  192 TWNB13 6x5; 193 TWNB14 6x5; 194 TWNB15 6x5
MAP14:
  195 TWNB16 6x5; 196 TWNB17 6x5; 197 TWNB18 6x5; 198 TWNB19 6x5
  199 TWNB20 6x5; 200 TWNB21 6x5; 201 TWNB22 6x5; 202 TWNB23 6x5
  203 TWNB24 6x5; 204 TWNA01 6x5; 205 ROOM10 4x3; 206 ROOM11 3x2
  207 ROOM12 3x2; 208 ROOM13 5x6; 209 ROOM14 4x3
MAP15:
  210 MGEN03 6x5; 211 ROOM02 4x3; 212 MGEN04 6x5; 213 ROOM04 4x4
  214 ROOM05A 4x3; 215 ROOM05B 4x3; 216 ROOM06 5x2; 217 ROOM07 4x2
  218 ROOM08 5x3; 219 MGEN05 6x5; 220 FACT11B 7x4; 221 SAIB03 7x3
  222 MGEN98 4x7; 223 ROOM19 5x6; 224 YAKA26 3x2
MAP16:
  225 MGEN99 6x4; 226 MGEN11 10x3; 227 MGEN12 6x5; 228 MGEN13 6x5
  229 MGEN14 6x4; 230 MGEN15 6x4; 231 MGEN16 4x4; 232 YAKA25 5x4
  233 MIHA05 7x4; 234 MIHA06 5x6; 235 ROOM20 3x2; 236 TEND01 6x5
  237 TEND02 6x5; 238 TOPN01 6x5
MAP17:
  247 MGEN06 7x2; 248 MGEN07 6x5; 249 MGEN08 6x5; 253 MGEN09 6x5
  254 MGEN10 6x5
Disc-only: MAP16/MGEN17 6x5 (no MAP_ENTRIES slot)
```

## 2. Cache capacity, streaming, and drawing missing columns

**Worst phase needs five columns, versus four cached.** For view width W and left-edge phase p in `[0,128)`, intersected columns = `ceil((p+W)/128)`. With **W=426.6667**, this is 4 or **5** (five when p>85.3333); with W=320 it is at most 4. Example on a sufficiently large map: cameraX=150 makes the wide interval **[96.6667,523.3333)**, intersecting tile columns **0..4**. Height remains 240 and needs at most **3 rows**. Worst coverage is **5x3=15 tiles**, versus the **4x3=12** cache; width-4 maps themselves need only four columns.

Each tile uses 64x128 VRAM words (8-bit). Slots are **x=768+(tileX%4)*64, y=(tileY%3)*128**, so the cache is **512x384 texels / 256x384 words**, x768..1023 (`main/map.c:4250-4259`, `:4342-4352`). Columns separated by four alias the same slot. VRAM alone cannot supply all five simultaneously; increasing the draw loop would sample overwritten/stale slots. Even a four-column wide interval can fail if it starts before the currently selected cache window. `renderMap` emits only the selected four columns and three rows (`:3962-3966`, `:3985-4009`).

**What selects the window:** initial `MAP_TILE_X=cameraX/128`, clamped to 0 for widths<5 and otherwise to width-4; Y similarly to height-3 (`main/map.c:4276-4295`). During scrolling, `handleTileUpdate(input,force)` uses controller direction bits, comparison with previous camera coordinates, map dimensions and hardcoded coordinate bands, rather than the widened view bounds (`:4664-4820`). Horizontal example: moving right selects first column 1 at cameraX **0x8a=138**, column 2 at **0x10a=266**; moving left uses different bands (column 0 for **10..128**, column 1 for **138..256**), and the right edge forces `width-4` (`:4734-4814`). Vertical bands are 128 apart, with bottom-edge `height-3` (`:4666-4730`).

On a changed first row, `updateTileRow` uploads four or width columns from the new top/bottom row, then updates PREV_TILE_Y (`main/map.c:5210-5237`). On a changed first column, `updateTileColumn` uploads the new left column or `start=3` right column across up to three rows, then updates PREV_TILE_X (`:5252-5278`). `force==1` uploads the entire selected window (`:4817-4819`). None considers ±53.33 margins. Preserve these original writes, including existing indexing quirks, for 1x fidelity.

**RAM availability: yes after loading, but not permanently.** The full TFS goes into `GENERAL_BUFFER_PTR` (`main/map.c:4221-4222`), normally `GENERAL_BUFFER` (`main/main.c:246`), PS1 address **0x80010000** (`decomp/include/dw/main.h:8`). `MAP_TILE_DATA.tiles[cell].imagePtr` points directly into that buffer for **every nonempty map cell**, not only cached cells; `MAP_CLUTS[]` also points there (`main/map.c:4234-4261`). Metadata has 35 cells (`decomp/include/dw/map.h:49-57`). `.MAP` setup/collision/entities are consumed before the same buffer is overwritten by `.TFS`; there is no separately allocated permanent full-map texture.

**Critical battle lifetime:** BTL initializes EFE slots, emitter/flash/aura storage and an effect heap starting at this same general buffer (`main/btl.c:455`, `btl/battle_effect.c:2170-2196`). Sound loading also uses it (`main/sound_async.c:217-246`). Thus imagePtr still existing does **not** prove its bytes remain TFS during battles. Victory explicitly reloads TFS (`main/tamer.c:861-869`). Ordinary battle camera follow is stopped (`main/tamer.c:837-846`; normal map-follow updates require GAME_STATE==0, `main/map.c:4477-4500`), so the original can retain its existing VRAM window while reusing RAM.

**HD solution (proposal):** create renderer-owned tile textures from valid TFS data at load/setup time, before shared-buffer reuse; retain them through battle. Submit extra background quads only to HD, preserving original position/OT depth and nearest-sampled indexed pixels. Alternatively accumulate native texture data as uploads occur, but that cannot guarantee unseen columns are available on first exposure. Use the **live active CLUT at (0,480)**, not an immutable daytime palette: time changes and interpolation update it (`main/map.c:4311-4326`, `:4588-4606`). Match tile UV/edge behavior (`:3996-3999`). This needs no extra PS1 VRAM writes, but does need new HD-only submissions; packet replay alone has no primitive for uncached tiles. Capturing tile pixels at load is necessary for dependable BTL support.

## 3. Map edges and missing art

The camera clamps to **X=0..(width*128-320)** and **Y=0..(height*128-240)** and changes drawing offsets to matching limits (`main/map.c:4381-4394`, `:4823-4844`). Geometry/background alignment uses these original offsets (`:4518-4526`). With unchanged camera the wide interval is **[cameraX-53.3333, cameraX+373.3333)**. At the left/right clamps, respectively, **53.3333 pixels** extend beyond the map. Both margins fit the rectangle only when **53.3333 <= cameraX <= width*128-373.3333**; no width-3 map can meet this.

What appears there:

- **Outside the grid:** no authored background tile, no automatic border extrusion/repetition, and no generated `tileId=-1` for out-of-bounds coordinates. Current drawing only addresses its selected tile grid (`main/map.c:3962-4009`). An HD extension must bounds-check and leave/paint the absent region black. Do not extend tile-array indexing beyond width/height.
- **Inside the grid, `tileId==-1`:** imagePtr=NULL, its cache rect is cleared black, and the emitted tile is black-modulated (`main/map.c:4250-4253`, `:4418-4431`, `:3990-3993`; streaming equivalents `:5228-5231`, `:5269-5272`). Texture transparency can expose the clear; this is not hidden background art.
- **Base/other layers:** normal frames insert a **black clear** (`main/main.c:3974`; initial buffers `:3580-3581`). If that clear covers the widened HD surface, uncovered margins are black. Actors, doors, foreground quads, weather and effects may still draw over this black after their visual culling is widened; they do not create missing terrain. Untouched 320-wide clears/fades can instead leave stale/exposed side pixels.

Changing the gameplay clamp to 426-wide limits would alter original offsets and screen-based decisions. A separate visual camera is possible, but shifts the original center composition and requires moving background, 3D and anchored effects together. It still cannot create art outside the map. Safest faithful policy: retain camera and accept black boundaries or pillarbox the scene.

## 4. Culling: drawing versus state

`entityIsOffScreen` projects eight bounding-box corners and calls `isOffScreen`; the latter tests offset-corrected coordinates against **±width/2, ±height/2**, normally **±160/±120** (`main/map.c:3871-3919`). It has a special always-visible type 0x98/animation 0x1c case (`:3880-3882`). Do not widen this helper globally.

| Requested site | What it controls | Would a drawing-only widening alter state? |
| --- | --- | --- |
| `main/main.c:3823` | Inside `VERSION_EQUAL_OR_OLDER(JP_BOMBOM)`; **not active in USA**. US initializes nonzero/non--1 NPC types as on-screen at `:3825-3827`. US date is newer (`decomp/include/dw/version.h:5-9`, `:53-54`). | No US test here to widen. The shared flag is stateful elsewhere. |
| `main/map_object.c:1061` (`isBoxOffScreen`) | Only callers found are renderDoors (`:398-418`), renderChest (`:484-495`), renderWarpCrystals (`:570-580`); rejection skips projection/model submission. | **Suitable for an HD-only visual predicate.** No RNG, chest-taking, door-opening or crystal gameplay state is changed by these branches; rendering matrices/packets do change. Keep the original helper result/calls for 1x. |
| `main/model.c:2894` (`renderDigimon`) | Read-only gate on **shared `isOnScreen`**, after isOnMap checks; isOnMap==2 bypasses both. Remaining work draws model/flat model and shadows (`:2890-2942`). | An additional geometric HD visibility decision can draw more without assigning the shared flag. **Changing isOnScreen globally is unsafe** (below). Preserve deliberate hiding, not just geometry culling: menus explicitly set these flags (`main/game_menu.c:1818-1819`, `:2127-2128`). |
| `main/efe.c:1138,1178,1218,1258` | Four quadrant loops in renderParticleFlash; cells outside **[-DRAWING_OFFSET_X,320-DRAWING_OFFSET_X]** are skipped. Y remains 240 (`:1131-1287`). | **Drawing only:** local geometry, packet allocation/OT links; no RNG or particle lifetime updates in these branches. Add HD cells without re-ticking the effect or changing original packet output. |
| `main/overworld.c:498-503` | Ordinary overlay rectangle test **cameraX-40..cameraX+360**, Y-60..Y+300; flag&0x80 and orderValue<20 bypass it. Actual visual margins need cameraX-53.33..cameraX+373.33 (13.33 extra each side over existing allowance). | **Ordinary overlay gate is drawing only.** Timer/currentFrame advance outside it (`:526-544`). Snow is a special caution: flag&0x80 already bypasses culling and calls buildSnowflakePrim (`:512-514`), which consumes RNG and changes x/y/currentFrame (`:688-729`). Widening only the rectangle test does not add snow calls. Re-running this renderer or spawning extra flakes would change state/RNG. |

**Confirmed downstream state/RNG dependencies of shared visibility and projection tests:**

- US NPC initialization/ticks write isOnScreen from the 320x240 helper (`main/overworld.c:1115-1116`, `:1355-1356`); partner ticks do likewise (`main/partner.c:260-264`). NPC AI is **not wholly paused by visibility**: AI/tracking runs earlier (`main/overworld.c:1227-1309`).
- **Collision and script activation:** `isInvisible` includes isOnScreen (`main/utils2.c:387-393`). Collision excludes invisible entities and can restore movement position on collision (`main/map.c:3439-3456`). Tamer collision then starts NPC script sections (`main/tamer.c:665-678`); NPC collision likewise feeds autotalk and script calls (`main/overworld.c:1309-1339`). Ground triggers are world/tile based (`main/tamer.c:688`), not an invitation to widen their regions.
- **Partner teleports/waypoints:** isOnScreen plus `isTileOffScreen` controls offscreen catch-up and position/animation-coordinate changes (`main/map.c:3324-3332`, `:3606-3645`). isTileOffScreen has separate **±200/±160** thresholds (`:3649-3674`); this is gameplay, not draw clipping. Speed training also repositions an offscreen partner periodically (`trn/trn_speed.c:95-103`). Widescreen can reveal those original teleports even with identical simulation.
- **Battle participants and RNG:** handleBattleStart removes offscreen NPCs from the map (`main/btl.c:251-253`, `:276-278`); only survivors proceed to chance draws at `:305` and sometimes `:335`. Widening isOnScreen changes participants/entity state **and which randomLimit calls execute**. Battle teardown also toggles isOnMap/anim flags based on visibility (`btl/battle_main.c:793-809`).
- **Explicit AI/RNG trap:** BTL_tickEnemyAI tests a projected point against **200x160**, not 320x240. Offscreen sets chargeMode=2; inside sets chargeMode=`randomLimit(2)` (`btl/battle_main.c:631-644`). Widening that test changes both **charge mode and RNG consumption**. Keep it exactly as-is.
- **Battle exit timing/entity animation:** intro waits for nonparticipants to leave 320x240 (`btl/battle_setup.c:260-300`); victory/flee sequences wait for enemies to leave (`btl/battle_main.c:943-963`); partner fleeing changes animation/flags on leaving (`:1216-1224`). Wider tests alter frames simulated and subsequent state/RNG timing.
- **Effect-script conditions/targets:** BTL_selectNextTargetEntity skips isOnScreen==0 and changes targetEntity/output index (`btl/battle_effect.c:4327-4361`); STD/VS have corresponding gates (`std/std_effect.c:4536`, `vs/vs_effect.c:4119`). This is effect-script execution, not rendering. Animation sound opcodes also gate playSound on the shared flag (`main/anim.c:335-345`).
- **Main script conditions:** no direct viewport-visibility condition was found in the main script IF dispatcher: it reads triggers, pstats and stat/card/move/condition/item/money comparisons (`main/script_interp.c:907-979`). The coupling above is indirect but real: battle outcome is written to pstat 0xff (`:1917-1918`), script waits depend on movement completion (`:562-584`), and collision starts script sections. I have not proven every disc script's eventual dependency.
- **Combat movement limits are separate gameplay:** entityCheckCombatArea/hasMovedOutsideCombatArea projects movement into a bounded screen-space area and collision reverts it (`main/map.c:3042-3104`, `:3428-3431`). Do not enlarge it with the visual frustum.

Therefore keep all gameplay predicates/flags and original GTE results intact. Generate extra HD-only model/overlay/effect primitives from existing state, without calling tick routines or replaying stateful render callbacks. More visible actors can still pop, stop, flee or teleport at the original 4:3 thresholds; fixing that behavior would no longer be faithful simulation.

## 5. Scene coverage

| Scene | What 16:9 could actually show | Policy |
| --- | --- | --- |
| Overworld | More **pre-rendered 2D tiles**, depth-ordered 2D foregrounds and 3D actors/doors; not a complete 3D landscape (`main/map.c:3949-4014`; `main/overworld.c:498-524`; `main/model.c:2900-2942`). | Selectively support larger-map interiors with captured tile data and HD visual culling; pillarbox width-3 maps and scenes with missing side art. No map is guaranteed art-filled at every original camera position. |
| **BTL ordinary encounter** | Same map background plus 3D combatants/effects. startBattle stops camera follow and loads BTL, not an arena map (`main/tamer.c:832-846`); the registered map renderer remains in the object render path (`main/map.c:3944`, `main/btl.c:141-158`). EFE can disable the overlay map layer (`btl/battle_effect.c:2298-2300`; layer gate `main/overworld.c:477-479`). | Conditional, with the same map-border/cache constraints and **TFS lifetime capture**. Do not treat BTL as an inherently full-3D arena. |
| **STD tournament / VS battle arenas** | Actual TMD stage geometry: STD removes overworld objects and loads arena models (`std/std_setup.c:262-267`, `:639-655`), draws through GsSortObject4J (`:682-712`); VS loads TMD stages and sorts stage objects (`vs/vs_scene.c:1681-1695`, `:1780-1803`). | **Best widescreen candidates**, especially normal full-arena combat views. Existing geometry can extend into margins with the same focal length. Model completeness, special/split views, effects and cameras still need visual validation; source alone does not prove every side angle is complete. |
| Gameplay menus / HUD | Authored 320x240 layout, generally centered. In-world menus may retain a widened map behind centered panels; UI/model previews add no new scenery. Menu model previews have their own projection/OT (`main/game_menu.c:3968-4011`). | Center existing UI; pillarbox standalone menus or expand only a deliberately neutral backdrop. Do not stretch panels or call preview render functions twice: they temporarily change animations/positions, and medals advance rotation (`:3987-4021`, `:4038-4041`). |
| Title / landing / main menu | Fixed raster backgrounds: **256+64=320 pixels wide**, 240 high (`main/main.c:4146-4160`, `:4172-4186`); loaded TITLE256/TITLE2 assets (`:3426`, `:3710`). Read-only TIM headers confirm TITLE256 is **320x256** (extra bottom rows include UI imagery), TITLE2 **320x240**, both 8-bit; neither contains horizontal overscan art. | **Pillarbox.** More surface only reveals empty sides; horizontal stretch or invented extensions are not faithful. |
| Movies | All four US STRs are **320x176**, verified over every video sector: OP1 16,271; OP2 14,289; ED2 11,603; EDR 15,103. Magic at sector+8, dimensions +24/+26 in 2336-byte sectors. RGB24 direct uploads/display use 480 words per 320-pixel row (`mov/mov.c:194-202`, `:266-269`), with y=32/272 picture origins (`:236`), so original output has **32-line bars above/below**. | Preserve decoded picture aspect; no extra imagery exists. Default retain original 320x240 presentation inside pillarbox. An explicit movie zoom could remove the baked presentation bars and fit the 320:176 picture (aspect **1.818:1**) inside 16:9, leaving small top/bottom bars; it is a presentation option, not wider game rendering. |

## 6. Recommendation and main risks

1. **Start with full-view STD/VS arenas**, then an allowlist of large overworld/BTL map scenes. Retain original camera/focal length and center coordinates; widen only HD clipping/composition. A 240-high 16:9 surface is **426.6667 wide, ±53.3333** beyond the original, or **(1280/3)*S x 240*S** in HD pixels (round consistently when S is not a multiple of 3).
2. For maps, capture indexed tiles at TFS setup, retain through shared-buffer reuse, and draw the independently selected **up to five columns** at background OT depth 0xfff. Do not alter cache selection, uploads, CPU flags, AI/combat bounds or RNG. Keep intentional entity hiding and layer controls. Use current palettes and original offset/foreground ordering.
3. **Pillarbox title/landing, standalone menus, width-3 maps, and unvalidated scenes/transitions.** Larger maps still hit black side gaps at their original camera clamps. Either accept those honest gaps for a whole wide scene or keep that scene 4:3; automatic aspect changes at every edge would be distracting. Empty `-1` tiles and unfinished-looking edges require scene checks, not just width eligibility.
4. Cover the new margins in every screen-space operation. Known examples: main 320x240 fade (`main/fade.c:144-167`), battle 320x240 fades (`btl/battle_effect.c:2303-2324`), wrapping battle backdrops (`:4017-4020`), mist/weather (`main/overworld.c:550-658`), pause fills (`main/utils2.c:492-495`) and iris bounds (`decomp/src/murd/murd.c:628-694`). **Do not fix weather coverage by increasing shared-RNG simulation calls.** Also preserve changing world versus fixed UI draw offsets (`main/main.c:3961-3970`, `:4043-4046`).
5. Before enabling a scene, verify map interiors/edges, tile-boundary scroll phases, battle start/end/flee, hidden actors, palette fades, foreground occlusion and full-screen effects. Require identical replay state/RNG and 1x VRAM with wide on/off. No such runtime verification was done in this investigation.

**Main risks:** missing border art; five-column coverage and stale cache slots; TFS pointers outliving their data; changing RNG or entity state through shared culling; revealing original offscreen tricks; 320-wide fades/weather/UI; incomplete arena geometry/compositions. The architecture can support a faithful selective extension, but the source/data do not support a promise of seamless universal 16:9.
