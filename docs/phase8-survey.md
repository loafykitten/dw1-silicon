# Digimon World USA: HD renderer survey

Static source survey, with read-only inspection of the extracted US movie headers. Paths below are relative to the repository root. Rectangles are `(x,y,w,h)` in 16-bit VRAM words unless stated otherwise. No game execution or rendering validation was performed.

**Result:** the surveyed code has **no confirmed framebuffer readback or framebuffer-as-texture effect**. The proposed exact 1x VRAM plus HD draw surfaces is a plausible fit. The significant complications are mid-ordering-table draw-offset changes, projected sprite/label anchors, pre-rendered map coverage and culling for widescreen, and 24-bit movie uploads. Data-driven texture rectangles/pages remain a static-analysis limitation.

## 1. Framebuffer reads and screen copies

Gameplay framebuffer union is `[0,320) × [0,480)` (`decomp/src/main/main.c:3577-3581`). I found **15 `StoreImage` call sites and 6 `MoveImage` call sites**, listed completely below. None has a demonstrated framebuffer source. Palette memory starting at y=480 is immediately below, not inside, the buffers.

### Every readback site

| Site | Source and purpose |
| --- | --- |
| `decomp/src/main/effect_common.c:358` | `(0,480,256,7)`, `downloadCLUT1`: save scene palettes for palette fading; CPU modification and re-upload at `:364-403`. |
| `decomp/src/main/effect_common.c:412`, `:414`, `:416` | `(272,480,16,1)`, `(96,501,16,1)`, `(224,488,16,24)`: `downloadCLUT2`, additional effect/model palettes. |
| `decomp/src/main/efe.c:987` | `(512,248,256,7)`, `downloadSomeImage`; dim those 1,792 palette/image words and reload the same rect at `:991-1016`. Used by STD/VS screen-fade commands (`decomp/src/std/std_effect.c:2322,2491`; `decomp/src/vs/vs_effect.c:1953,2074`). Its x starts well beyond the framebuffer. |
| `decomp/src/doo2/doo2.c:359` | `(32,488,16,24)`, `DOO2_saveModelClut`. |
| `decomp/src/doo2/doo2.c:368` | `((tile&63)*16,tile>>6,16,4)`, `DOO2_saveClutTile`: egg TIM palette backup; caller passes the TIM's `GetClut(cx,cy)` result (`decomp/src/dooa/dooa.c:538-547`). **Asset-dependent coordinates**, not a screen-copy operation. |
| `decomp/src/dooa/dooa.c:670` | `(CLUT.x,CLUT.y,16,24)`, `DOOA_saveEntityClut`, from `model->clutPage`. |
| `decomp/src/dooa/dooa.c:679`, `:688` | `(0,488,16,24)` and `(48,488,32,24)`, model/shard palette backups. |
| `decomp/src/evl/evl.c:278` | `(CLUT.x,CLUT.y,16,24)`, `EVL_storeDigimonClut`. |
| `decomp/src/evl/evl.c:288`, `:297` | `(0,488,16,24)` and `(32,488,48,24)`, evolution palette banks. |
| `decomp/src/murd/murd.c:354` | `(CLUT.x,CLUT.y,16,24)`, despite the name `MURD_storeDigimonTexture`, this reads `model->clutPage`. |
| `decomp/src/endi/endi.c:110` | `(0,488,16,24)`, ending palette backup. |

Normal actor CLUT allocation is y=488 (`0x7a00/0x7a01`, decoded by `>>6`), including NPC allocations (`decomp/src/main/model.c:3400-3403,3424-3435`). Thus the model-derived palette reads above do not normally overlap the buffers.

### Every VRAM copy site

| Site | Source → destination and purpose |
| --- | --- |
| `decomp/src/main/anim.c:115-116` | `(textureX,textureY+32+frame*32,variableWidth,32)` → `(textureX,textureY)`: animated actor textures. Base comes from model page/offset, not framebuffer (`:226-227`; model allocations `decomp/src/main/model.c:3400-3435`). |
| `decomp/src/main/anim.c:326-333` | Animation opcode `0x3000`: script-supplied source offset/size → destination offset, both relative to that actor texture base. **Script-dependent bounds**, no fixed screen source. |
| `decomp/src/main/map_object.c:1193-1200` | `(518+(medal%9)*6,432+(medal/9)*32,6,32)` → `(512,432)`: choose medal texture; size from `MEDAL_RECT` at `:67`. |
| `decomp/src/fish/fish.c:4611-4612` | `(448,484,28,8)` → `(448,476)`: better fishing rod/swimmer texture selection. Destination crosses y=480 but x=448 is outside gameplay buffers. |
| `decomp/src/std/std_hud.c:1389-1396` | `(888,0,6,48)` → `(840+i*6,48)`, nine copies: initialize winner/loser model-scene texture slots. |
| `decomp/src/std/std_effect.c:1273-1281` | `(888-(step+1)*2,0,6,48)` (or x=888 on final step) → `(840+i*6,48)`: animate those slots. |

### Texture pages, transitions and GP0

- No explicit textured page setup found with a base inside x=0..319. Common apparent near misses are **x=320**, immediately adjacent: UI corners (`decomp/src/main/ui.c:282-287`), items (`decomp/src/main/item.c:2493-2499`), and textbox art (`decomp/src/main/script_textbox.c:1008`). Map tiles are x=768..1023 (`decomp/src/main/map.c:4251-4257`); map overlay pages start at x=384 (`decomp/src/main/overworld.c:763-773`); normal actors start at x=320 (`decomp/src/main/model.c:3424-3435`).
- `getTPage(...,0,0)` in line/flat-overlay **draw-mode packets is not texture sampling**: the following primitives are untextured (`decomp/src/main/line.c:20,30-59`; `decomp/src/btl/battle_effect.c:2498-2521`).
- Battle entry renders actors and projected battle-start lettering, not a captured screen/swirl (`decomp/src/btl/battle_setup.c:224-226,260-300`; lettering projection `decomp/src/btl/battle_hud.c:665-706`). Death/rebirth iris effects render a texture mask and surrounding quads over the scene, not framebuffer feedback (`decomp/src/murd/murd.c:592-694`; `decomp/src/dooa/dooa.c:909-935,959-1007`).
- Pause renders the dim overlay into **both buffers**, then busy-waits on input, retaining their pictures; no capture (`decomp/src/main/utils2.c:443-455`; `decomp/src/main/utils.c:75-90`). Menu model views compose another OT into the current framebuffer, not an offscreen texture (`decomp/src/main/game_menu.c:3973-4011,4034-4045`).
- No separate raw GP0 transfer-packet construction or GPU-register copy path was found in executable game C. The native `LoadImage`/`StoreImage`/`MoveImage` bypass packet parsing and call GPU transfers directly (`crates/psyq/src/libgpu.rs:128-156`). Packet GP0 `80..9f` invokes VRAM copy, `a0..bf` uploads, and `c0..df` is currently logged/ignored (`crates/backend/src/gpu.rs:962-985`). **An HD implementation must hook both direct transfers and GP0, not only GP0.**
- Confidence limit: TIM/TMD pages, animation bytecode offsets and effect-script model pages are data-driven (`decomp/src/main/file.c:74`; `decomp/src/main/anim.c:326-333`; `decomp/src/btl/battle_effect.c:4168`). This is not proof that every asset/scene can never address framebuffer pixels; a runtime overlap log would close that gap.

## 2. Framebuffer layout and environment calls

These are **all direct game-C call sites** for the six requested APIs; comments/prototypes excluded.

| API / count | Site and environment |
| --- | --- |
| `GsInitGraph` / 1 | `decomp/src/main/main.c:3577`: `(320,240,4,0,0)`: non-interlaced, GPU draw offset, dither initially off; native interpretation at `crates/psyq/src/libgs/mod.rs:143-175`. |
| `GsDefDispBuff` / 1 | `decomp/src/main/main.c:3578`: `(0,0)` and `(0,240)`, each 320×240. Both cleared via `(0,0,320,480)` at `:3580-3581`. |
| `SetDefDispEnv` / 1 | `decomp/src/mov/mov.c:266`: initially `(0,dispFrame*240,480,240)`, then `disp.w *= 2/3` → **320**, `isrgb24=1` (`:267-268`). Physical scanout consumes **480 VRAM words per row**. |
| `PutDispEnv` / 1 | `decomp/src/mov/mov.c:269`: that movie environment. |
| `PutDrawEnv` / 1 | `decomp/src/mov/mov.c:270`: movie's otherwise unused `draw`; uninitialized on PS1, deliberately zero-initialized in native C (`:210-218`). It is not a deliberate movie render target. |
| `SetDefDrawEnv` / 0 | No game-C call found. |

`GsInitGraph`, `GsDefDispBuff`, and buffer swaps also send environments **inside libgs**. `put_draw_buffer` clips to `(buffer.x,buffer.y,320,240)` and sets offset to buffer corner plus origin (`crates/psyq/src/libgs/mod.rs:127-140`); initialization sends draw/display environments (`:178-181`); `GsSwapDispBuff` alternates display/draw buffers (`:222-234`). Movie playback restores gameplay via `initializeFramebuffer` (`decomp/src/main/utils2.c:538-539`). No US interlace mode change was found; `SetVideoMode(MODE_PAL)` belongs to the PAL-region branch (`decomp/src/main/main.c:3394-3396`).

**Offsets matter:** fixed offsets are `(160,120)` and `(160,360)` (`decomp/src/main/main.c:218-220,3593-3594`). World drawing uses changing `DRAWING_OFFSET_X/Y`, while an OT entry at index `0x20` switches to the fixed centered UI offset (`:3961-3975,4043-4046`). Preserve command ordering and offset changes in HD; screen coordinates alone do not identify world/UI space.

**Offscreen rendering:** no intentional alternate raster target followed by a screen copy was found. Vertices can extend beyond the viewport and are clipped; offscreen VRAM is populated through image uploads/copies. Movie output is the exception to the gameplay footprint: x=0..479 words, y=32..207 or 272..447 (see §6). Separately named `FRAMEBUFFER_OT` objects in menus are ordering tables, not additional VRAM surfaces (`decomp/src/main/game_menu.c:3968-3975,4004-4009`).

## 3. 3D geometry sources and projected 2D attachments

### libgs and the common GTE boundary

Actors/bones use `GsGetLws` → light/LS matrices → `GsSortObject4J`; special wireframe and flattened-sprite paths branch around it (`decomp/src/main/model.c:2900-2933`). Other examples: map models (`decomp/src/main/map_object.c:1080`), fishing models (`decomp/src/fish/fish.c:2603,3559`), arena models (`decomp/src/vs/vs_scene.c:1726-1798`; `decomp/src/std/std_setup.c:691,709`), effect models (`decomp/src/main/effect_common.c:609`; `decomp/src/btl/battle_effect.c:2805,2864,4691`), and menu models (`decomp/src/main/game_menu.c:4004`).

`GsSortObject4J` dispatches through `GsFCALL4` (`crates/psyq/src/libgs/sort4j.rs:105-158`). Current Rust handlers project TG3/TG4 and unlit textured quads with RTPT/RTPS, then write integer SXY into packets (`:318-339,403-428,465`). C inline GTE enters through `dw1_gte_cop2` (`crates/psyq/src/gte.rs:85-89`); `RotTransPers*`/`RotNclip*` wrappers also execute the shared GTE (`crates/psyq/src/libgte.rs:689-697,717-731,753-773,783-875`). **Hook the shared backend projection**, including every RTPT vertex (`crates/backend/src/gte.rs:415-454,586-591`), or libgs's direct calls will bypass a hook only on the C bridge.

### Game C doing its own projection: complete direct-call inventory

Counts are **static call expressions**, not projected vertices or runtime calls. Includes inline `gte_rtps/rtpt`, `RotTransPers[3/4]`, and **`RotNclip3/4`**; excludes declarations and wrapper callers. **74 US-source sites across 24 files**, including one unreachable debug site. `main/model.c` has four additional non-US duplicate sites, excluded. A loop counts once; `RotTransPers4` counts once.

| File | Sites | Functions / purpose, with projection lines |
| --- | ---: | --- |
| `decomp/src/main/map.c` | 12 | `entityCheckCombatArea` 3069,3077; `isTileOffScreen` 3663; `entityIsOffScreen` 3894: tests/culling. `initializeDrawingOffsets` 4368,4372; `tickCameraFollowPlayer` 4488,4492; `tickCameraMovement` 4886,4889; `moveCameraByDiff` 5145,5148: projected camera deltas. |
| `decomp/src/main/model.c` | 4 | US `renderWireframed`: 3180,3198,3220,3239; textured TG3/TG4 or line outlines. Non-US duplicates at 3048,3065,3084,3103 are not counted (`:3002,3128-3129`). |
| `decomp/src/main/effect_common.c` | 3 | `addScreenPolyFT3` 475; `addScreenPolyFT4` 492: effect/world polygons. `worldPosToScreenPos` 510: general sprite/label/camera helper. |
| `decomp/src/main/item.c` | 3 | `renderOverworldItem` 2484: billboard center and depth-scaled width; `renderDroppedItemShadow` 2539,2543: projected ground quad. |
| `decomp/src/main/overworld.c` | 2 | `calcMapObjectOrder` 446: depth sort; `getDrawPosition` 946: map-image position from world point. |
| `decomp/src/main/drop_shadow.c` | 1 | `renderDropShadow` 64: four-corner textured ground shadow. |
| `decomp/src/main/btl.c` | 1 | `renderFleeBubble` 577: actor point controls bubble/pointer orientation; also calls shared projection at 536. |
| `decomp/src/main/utils.c` | 1 | `getEntityScreenPos` 190: bone position, plus draw-origin adjustment at 192-193. |
| `decomp/src/main/tamer.c` | 1 | `worldPosToScreenPos2` 1286: script/textbox anchor helper. |
| `decomp/src/main/map_object.c` | 1 | `isBoxOffScreen` 1059: model bounds culling, not emitted geometry. |
| `decomp/src/main/aabb.c` | 1 | `renderAABB` 30: debug lines, **unreachable because of unconditional return at 23**. |
| `decomp/src/btl/battle_effect.c` | 5 | `BTL_renderParticleEmitters` 2071: line endpoint; `BTL_renderRadialWaves` 2720: quads; `BTL_renderWireframeBox` 3099: lines; `BTL_renderProjectedSprite` 4192: projected center; `BTL_renderRibbonStrip` 5376: quads. |
| `decomp/src/std/std_effect.c` | 5 | Same five `STD_` functions respectively: 2176,2878,3262,4374,5570. |
| `decomp/src/vs/vs_effect.c` | 5 | Same five `VS_` functions respectively: 1807,2461,2845,3957,5155. |
| `decomp/src/btl/battle_hud.c` | 6 | `BTL_renderBattleStartText` 665,669,702,706; `BTL_renderBattleStartTextBurst` 810,814: transformed textured letter quads. |
| `decomp/src/std/std_hud.c` | 6 | `STD_renderBattleStartText` 699,703,736,740; `STD_renderBattleStartTextBurst` 844,848. |
| `decomp/src/vs/vs_hud.c` | 6 | `VS_renderBattleStartText` 899,903,936,940; `VS_renderBattleStartTextBurst` 1052,1056. |
| `decomp/src/btl/battle_main.c` | 1 | `BTL_tickEnemyAI` 637: offscreen decision for AI charge mode, **not drawing** (`:639-642`). |
| `decomp/src/btl/command_menu.c` | 1 | `BTL_initializeFinisherChargeup` 379: gauge/overlay anchor above partner, clamped to screen (`:380-395`). |
| `decomp/src/std/std_main.c` | 1 | `STD_renderPlayerMarker` 5691: bone-anchored 2D marker (`:5694-5705`). |
| `decomp/src/vs/vs_combat.c` | 1 | `VS__renderPlayerMarker` 2571: corresponding player marker. |
| `decomp/src/fish/fish.c` | 2 | `FISH_panCameraTo` 978: camera positioning; `FISH_drawSprite3D` 1684: four projected corners of fishing sprites. |
| `decomp/src/kar/kar.c` | 1 | `KAR_renderAimArrow` 1053: projected trajectory points with 2D trail/arrow quads (`:1056-1090`). |
| `decomp/src/doo2/doo2.c` | 4 | `DOO2_renderWireframeModel` 461,487,503,532: textured/outline triangle and quad branches. |

Shared polygon helpers also cover flattened Digimon (`decomp/src/main/model.c:2787`), death/evolution shards and sparks (`decomp/src/doo2/doo2.c:271,324,584`; `decomp/src/dooa/dooa.c:1497,1551`; `decomp/src/evl/evl.c:948,1160,1261`) and combat auras/projectiles/trails (`decomp/src/btl/battle_effect.c:3014,6047,6213,6608,6770`). These are not extra direct projection sites.

### Projected point → 2D sprite/rectangle: PGXP alignment risks

An integer-XY lookup alone will miss corners computed as `projectedCenter ± width/2`, or anchors moved/clamped after projection. Carry a precise anchor through these paths, or deliberately keep that family integer-aligned.

| Attachment family | Evidence |
| --- | --- |
| Billboard items, condition bubbles, butterflies, ambient/entity particles, clouds/flashes | `decomp/src/main/item.c:2484-2499`; `decomp/src/main/bubble.c:486-508`; `decomp/src/main/butterfly.c:126-130`; `decomp/src/main/efe.c:565,685,761,922`; `decomp/src/main/effect_common.c:768`. |
| Combat damage/status text, enemy HP bars, death countdown, command shout, charge meter, flee bubbles and player markers | `decomp/src/main/entity_text.c:147-153,186-193`; `decomp/src/btl/battle_main.c:2280-2313`; `decomp/src/btl/command_menu.c:379-395,492`; `decomp/src/btl/command_shout.c:66`; `decomp/src/main/btl.c:536-602`; marker sites in table above. |
| Effect-script projected/scaled sprites; separately projected coordinates then later screen sprites; poison/stun icons and ring-tube centers | BTL `decomp/src/btl/battle_effect.c:2569,4192-4199,4262-4265,4289-4310,5510,5761`; STD counterparts `decomp/src/std/std_effect.c:2737,4374,4444,5704,5955`; VS `decomp/src/vs/vs_effect.c:2320,3957,4027,5289,5539`. |
| Actor-attached menus, inventory, dialogue origins and reward/stat boxes | `decomp/src/main/game_menu.c:1418,1697`; `decomp/src/main/inventory.c:515,679`; `decomp/src/main/script_textbox.c:938,949`; `decomp/src/main/battle_ui.c:286,535`; `decomp/src/shop/script_menu.c:2678,2733,2769`; fishing boxes `decomp/src/fish/fish.c:1363,1529`. |
| Pickup/chest/award anchors and training/map-image attachments | `decomp/src/main/tamer.c:1436,1478,1537,1584,1806,1822`; `decomp/src/main/overworld.c:946-966`; `decomp/src/trn/trn_mp.c:91-92`; `decomp/src/trn/trn_brain.c:163`; training spot/stat-box projection `decomp/src/trn/trn_hud.c:196,403`; `decomp/src/trn2/trn2_hud.c:399,606`. |
| Mood bubble, rebirth/death iris and lives-box positions | `decomp/src/main/script_common.c:1238-1246`; `decomp/src/murd/murd.c:444,465,592-620`; `decomp/src/dooa/dooa.c:909-935`. |

**Planning inference:** preserve integer GTE results for camera, gameplay and culling; add rendering precision alongside them. A wholesale float-coordinate replacement would also change game decisions (`BTL_tickEnemyAI`, above). The reference's value-keyed lookup explicitly faces ambiguous integer keys (`ref/DW2-PC/backend/pgxp.c:11-24`); DW1 additionally offsets and expands projected centers, as demonstrated above.

## 4. Maps, 2D layers and widescreen

- **Backgrounds are textured quads, not GTE geometry:** `renderMap` emits up to **4 columns × 3 rows = 12 `POLY_FT4`**, each 128×128 screen pixels, at OT depth `0xfff` (`decomp/src/main/map.c:3949-3966,3985-4009`). The loader near the supplied note is `readMapTFS` at `:4217`; the actual renderer is earlier.
- **8-bit tile cache:** 128×128 texels occupy 64×128 VRAM words. Slot x=`768+(tileX%4)*64`, y=`(tileY%3)*128`; hence cache footprint **(768,0,256,384)**. CLUT `(0,480)`; page mode 1 (`decomp/src/main/map.c:4250-4259,4342-4353,4402-4444`). Palette variants for time of day are loaded at y=481 onward, with the active one at y=480 (`:4237-4243,4300-4335`).
- **Scroll:** quad position combines tile image coordinate, `cameraX/Y`, and world draw origin (`decomp/src/main/map.c:4519-4530`). Camera deltas come from differences of projected world points (`:4488-4503`). Rows/columns are streamed into reused slots on tile-boundary changes (`:4664-4819,5210-5237`, `updateTileColumn` at `:5242`). UV height is deliberately 127 rather than 128 on the lower half of a texture page (`:3996-3999`): avoid introducing seams through filtering/coverage changes.
- **Extra image exists on larger maps**, because the image is a grid of `width*128 × height*128`, with a 512×384 cached window, not just a 320×240 image (`decomp/src/main/map.c:4250-4266,3963-3986`). This does **not** guarantee both 16:9 margins contain valid pixels at every scroll phase. Cache selection assumes the old viewport; map edges have no authored image beyond the grid, and `tileId==-1` is explicitly black (`:3990-3993,4419-4424`). I did not audit every map's artwork extent.
- **Edge clamp:** camera X is restricted to `0..width*128-320`, Y to `0..height*128-240`; corresponding draw offsets switch to their limits (`decomp/src/main/map.c:4381-4397,4823-4844`). Widening needs revised camera/cache coverage or a deliberate edge policy. Widening the GPU clip alone cannot supply missing tiles.
- **Foreground/map overlays:** animated `POLY_FT4` objects, separately depth ordered and sometimes semi-transparent, with map-coordinate scrolling; weather can be screen-relative (`decomp/src/main/overworld.c:487-524,732-773`). Overlay textures start at x=384 and use 4/8-bit pages (`:763-773`); mist uses x=704 (`:630,637`). Overlay visibility tests use camera X−40..X+360, Y−60..Y+300 (`:498-503`), insufficient for every widened case.
- **CPU culling must also widen:** NPC/model visibility is tested against 320×240 (`decomp/src/main/main.c:3823`; `decomp/src/main/map_object.c:1061`), and `renderDigimon` skips `isOnScreen==0` (`decomp/src/main/model.c:2894-2896`). Particle-flash culling similarly uses 320 (`decomp/src/main/efe.c:1138,1178,1218,1258`). Changing those tests can affect simulation elsewhere; separate visual visibility from gameplay visibility where needed.
- **UI is authored in centered 320×240 coordinates:** clock stops at x=−135 (`decomp/src/main/clock.c:260-280`); status bars slide to x=75, extend 75 pixels right, and contain 2×2 details (`decomp/src/main/partner_impl.c:2165-2195`). Textbox centering hardcodes 320/240 (`decomp/src/main/script_textbox.c:1269`). Keep or reposition such anchors intentionally. Movie letterboxing is established below; no separate named gameplay cinematic-letterbox routine was found.

## 5. Effects and pixel-sensitive rendering

| Concern | Concrete evidence / HD implication |
| --- | --- |
| Main fades | `decomp/src/main/fade.c:143-169`: 320×240 semi-transparent FT4 at (−160,−120), tiny texture patch, CLUT `(256,502)`. `FADE_MODE=2` subtracts for black, mode 1 adds for white (`:89-109,180-202`). Expand coverage for wide margins; retain 5-bit blend behavior. |
| Buffer clears | Normal frames insert `GsSortClear` (`decomp/src/main/main.c:3974`), a GP0 fill of the active clip rectangle (`crates/psyq/src/libgs/mod.rs:328-337`). Tournament teardown clears both gameplay buffers and separate texture regions (`decomp/src/std/std_setup.c:443-447`). Mirror framebuffer clears into HD surfaces, including wide margins; texture-region clears stay in 1x VRAM. |
| Effect-screen fades/fills | BTL offset-relative 320×240 textured subtractive fill `decomp/src/btl/battle_effect.c:2303-2324`; untextured blend-selectable overlay `:2498-2522`. STD equivalents `decomp/src/std/std_effect.c:2479-2481,2679-2688`; VS `decomp/src/vs/vs_effect.c:2062-2064,2262-2271`. Fixed 320×240 flash/backdrops also at `decomp/src/eab/eab.c:482,551`, `decomp/src/murd/murd.c:484`; pause fill `decomp/src/main/utils2.c:492-495`; bracket background/fade `decomp/src/std/std_main.c:3310-3311,3712-3726`. These need explicit wide coverage. |
| Palette fades | CPU read/modify/re-upload of CLUTs, not screen pixels (`decomp/src/main/effect_common.c:353-403,407-452`; `decomp/src/main/efe.c:980-1016`). Preserve ordering relative to texture sampling; defer HD work only if it retains the appropriate VRAM version. |
| Iris transitions | Texture mask + four surrounding rectangles, bounds explicitly ±160/±120; overlap thickness fills seams (`decomp/src/murd/murd.c:628-694`; DOOA counterpart `decomp/src/dooa/dooa.c:959-1007`). Widen outer bounds while preserving projected hole anchor. |
| Mist/weather and scrolling attack backgrounds | Mist draws eight overlapping, semi-transparent quads with 320×240 periods and mirrored geometry (`decomp/src/main/overworld.c:577-617,625-658`). Battle backgrounds wrap at 320 (`decomp/src/btl/battle_effect.c:3974,4017-4020`; STD `decomp/src/std/std_effect.c:4156,4199-4202`; VS `decomp/src/vs/vs_effect.c:3739,3782-3785`). Margins need enough repeated quads, not stretched weather pixels. |
| Camera shake/draw origin | Meramon changes `DRAWING_OFFSET_X` and grows a blended quad as large as 400×400 (`decomp/src/main/map.c:5505-5535`); normal world/UI OT offset switch described in §2. Keep offsets in PS1 units before surface scaling. No intentional extra clip-area/offscreen raster pass was found. |
| Dither | Initial graph disables it (`decomp/src/main/main.c:3577`), but line and flat-overlay draw-mode packets set dither=1 (`decomp/src/main/line.c:20`; `decomp/src/btl/battle_effect.c:2520`; STD `:2688`; VS `:2271`). Native `GsSortBoxFill` also emits `0xe1000200` (`crates/psyq/src/libgs/sort2d.rs:123-128`). Thus **not globally dither-free**. Keep command state and decide whether HD dithering is locked to native pixels; I cannot establish aesthetic dependence without viewing scenes. |
| Lines | `LINE_F2/F3` helpers `decomp/src/main/line.c:26-59`; projected particle streaks `decomp/src/btl/battle_effect.c:2064-2076`; wire boxes `:3099-3110`; wireframe actor outlines `decomp/src/main/model.c:3198-3211,3239-3258`; fishing line screen clipping `decomp/src/fish/fish.c:3661`. Define scaled line width and joins; polygon-only PGXP misses these. |
| Pixel-aligned UI / text | UI corners are 4×4 textured quads (`decomp/src/main/ui.c:279-287`); item cells use 15 rather than 16 UV extents at texture-page edges (`decomp/src/main/utils2.c:315-327`). Nearest sampling and consistent edge rules matter. |

**Text pipeline:** glyphs are CPU-expanded into **4-bit texels**, loaded into the 256×256 text atlas at **(704,256)** in **64×256 VRAM words**. Cells are 12×11, with US variable-width advance (`decomp/src/main/font.c:15-20,1621-1697,1702-1704,1761`). Font CLUT is `(208,488)` (`:30-32,1509-1510`). `renderString` draws an atlas rectangle as a `POLY_FT4` with matching screen/UV size, optionally another at `(x+1,y+1)` for shadow (`decomp/src/main/utils2.c:267-286`; tpage/CLUT setup `decomp/src/main/utils.c:206-210`). Numbers additionally use digit quads (`:230-235`). This is bitmap text at native pixel size, not vector text or framebuffer texturing.

**Not found:** framebuffer mosaic/blur feedback, per-scanline display changes, or a scanline-interrupt raster effect in surveyed game C. This is a static negative result, not a visual regression guarantee.

## 6. Movies

- **Decoded slices upload directly to the movie buffers**, without textured polygons: `MOV_onSliceDecoded` advances a 24-word-wide slice and calls `LoadImage` (`decomp/src/mov/mov.c:91-119`). At RGB24 this is **16 pixels per slice**. `MOV_initEnvironment` sets slice width=24 (`:85-88`).
- US buffer origins are **(0,32)** and **(0,272)** (`decomp/src/mov/mov.c:236`); displayed origins alternate **(0,0)** / **(0,240)**. `SetDefDispEnv(...480,240)` is reduced to logical **320×240**, RGB24 (`:265-270`). Decoder mode is 3 (`:252-253`). A decoded frame uses `width*3/2` VRAM words and header-provided height (`:194-202`).
- **All four local US STR files contain 320×176 video**, verified read-only by checking every 2336-byte sector's video magic at +8 and width/height at +24/+26: `decomp/disks/us/MOVIE/{OP1,OP2,ED2,EDR}.STR`. Video-sector counts checked: **16,271 / 14,289 / 11,603 / 15,103**, respectively; no other dimensions found. These are binary offsets, not source line numbers. Supporting source for header use: `decomp/src/mov/mov.c:194-202`; existing OP1 observation: `docs/phase2-movie.md:71-73`.
- Therefore each picture occupies **480×176 VRAM words** at y=32..207 or 272..447, centered with **32-pixel black bars above and below** in the 320×240 display. The code clears `(0,0,480,480)` before playback and on dimensions changing (`decomp/src/mov/mov.c:194-198,247-249`).
- **Renderer implication:** treat RGB24 scanout/upload as a movie path; do not interpret the 480-word row as a 480-pixel HD gameplay surface. Scale the decoded movie and retain its aspect ratio/letterbox policy. HD texture or PGXP reconstruction cannot add geometry detail to it.
