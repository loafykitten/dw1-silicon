# Task A: SDK surface and GTE

## Scope and counting

Surveyed the **120 C files listed in the US makefile**, with US-active conditionals. The build macro is **`VERSION_US`**, not `VER_US`; it selects version ID 4, date 19990331, and NTSC-U. Sources: `mk/version/us.mk:19`, `include/dw/version.h:35–38,51–54`. SDK-function counts were unchanged when comparing the Metrowerks and GCC conditional branches.

Path conventions below:

- Game paths are relative to **`ref/dw_decomp/`**.
- DW2-PC coverage paths are relative to **`ref/DW2-PC/psyq/`**, unless explicitly qualified.
- `file.c:10,20` means both lines; ranges are inclusive.
- Counts are **static source occurrences**, excluding declarations, comments, inactive regional branches, and function definitions. They are not runtime invocation counts.
- Functions with at most five calls have all call sites listed; larger counts have representative sites.
- **“Has it” means an implementation with that public name exists**, not that its behavior or ABI is sufficient for DW1.

**Verified inventory:** 175 directly called SDK/runtime function names, **2,939 call sites**; 44 call-like SDK macros; 27 additional TMD dispatch targets; five SDK globals and eight used Metrowerks/linker globals.

**Repository limitations:** the configured SDK header submodule is absent, and generated SDK assembly is absent. Its configuration is recorded at `mk/version/us.mk:3`, `.gitmodules:7–9`, and `config/us/main.yaml:10,64`. Thus the C inventory and DW2-PC implementation coverage are verifiable, but the exact SDK transitive call graph and SDK-internal GTE instruction counts remain **unverified**.

No files were modified. Nothing under `disc/` was read.

## A1. SDK functions and macro-wrapped calls

### libapi — 3 functions, 13 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `InitHeap3` | 1 | `src/main/main.c:3539` | **Missing** from `psyq/`; DW2 uses its game allocator, `ref/DW2-PC/src/main/mem.c:85` |
| `malloc3` | 7 | `src/main/model.c:2829,2830,3299` | **Missing** from `psyq/`; different game API `Mem_Alloc`, `ref/DW2-PC/src/main/mem.c:143` |
| `free3` | 5 | `src/main/model.c:2952,2953,3304,3472`; `src/std/std_setup.c:660` | **Missing** from `psyq/`; different game API `Mem_Free`, `ref/DW2-PC/src/main/mem.c:55` |

`InitHeap` is declared at `src/main/_psstart.c:16`, but its startup call at `:169` is inside `__cplusplus`, inactive for this C build. It is not counted.

### libetc — 4 functions, 32 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `PadInit` | 1 | `src/main/main.c:3389` | **Different API:** `PadInitDirect`, `PadStartCom`, `libpad.c:18,24` |
| `PadRead` | 14 | `src/main/btl.c:143`; `src/main/main.c:4040,4061` | **Different API:** receive buffers populated by `Psyq_PadVBlank`, `libpad.c:18,39`; `PadGetState`, `:31` |
| `ResetCallback` | 2 | `src/main/main.c:3385`; `src/main/sound.c:521` | **Has it:** `libetc.c:47`; only clears its VSync callback |
| `VSync` | 15 | `src/main/btl.c:153`; `src/main/main.c:3656,3675` | **Has it:** `libetc.c:25` |

### libgpu — 28 functions, 773 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `AddPrim` | 174 | `src/main/aabb.c:43`; `src/main/battle_ui.c:633`; `src/main/btl.c:150` | **Has it:** `libgpu.c:198` |
| `ClearImage` | 13 | `src/main/font.c:1531`; `src/main/main.c:3567`; `src/main/map.c:4414` | **Has it:** `libgpu.c:61` |
| `DrawSync` | 134 | `src/main/battle_ui.c:528`; `src/main/btl.c:152`; `src/main/efe.c:983` | **Has it:** `libgpu.c:56`; immediate completion |
| `GetClut` | 105 | `src/main/btl.c:590`; `src/main/clock.c:269,272` | **Missing** |
| `GetTPage` | 33 | `src/main/clock.c:228,269,272` | **Missing** |
| `LoadClut` | 7 | `src/main/map.c:4239,4312,4317` | **Missing**; underlying `LoadImage` exists, `libgpu.c:74` |
| `LoadImage` | 62 | `src/main/efe.c:1011`; `src/main/effect_common.c:403,446` | **Has it:** `libgpu.c:74` |
| `MoveImage` | 6 | `src/main/anim.c:116,331`; `src/main/map_object.c:1200` | **Has it:** `libgpu.c:86` |
| `OpenTIM` | 2 | `src/main/overworld.c:384,401` | **Missing**; different TIM API `GsGetTimInfo`, `libgs.c:474` |
| `PutDispEnv` | 1 | `src/mov/mov.c:262` | **Has it:** `libgpu.c:157` |
| `PutDrawEnv` | 1 | `src/mov/mov.c:263` | **Has it:** `libgpu.c:133` |
| `ReadTIM` | 2 | `src/main/overworld.c:385,402` | **Missing**; different TIM API `GsGetTimInfo`, `libgs.c:474` |
| `ResetGraph` | 6 | `src/main/main.c:3386,3657,3676` | **Has it:** `libgpu.c:34` |
| `SetDefDispEnv` | 1 | `src/mov/mov.c:259` | **Has it:** `libgpu.c:186` |
| `SetDispMask` | 3 | `src/main/main.c:3562,3584`; `src/mov/mov.c:264` | **Has it:** `libgpu.c:48` |
| `SetDrawLoad` | 3 | `src/btl/battle_effect.c:2717`; `src/std/std_effect.c:2875`; `src/vs/vs_effect.c:2458` | **Missing** |
| `SetDrawOffset` | 2 | `src/main/main.c:3579,3580` | **Missing**; draw-environment offsets handled by `PutDrawEnv`, `libgpu.c:133` |
| `SetGraphDebug` | 2 | `src/main/main.c:3387`; `src/main/utils2.c:396` | **Has it:** `libgpu.c:43` |
| `SetLineF2` | 5 | `src/btl/battle_effect.c:5181`; `src/eab/eab.c:638`; `src/evl/evl.c:1206`; `src/std/std_effect.c:5370`; `src/vs/vs_effect.c:4955` | **Missing** |
| `SetLineF3` | 2 | `src/main/main_menu.c:5262,5266` | **Missing** |
| `SetLineF4` | 4 | `src/main/aabb.c:33`; `src/btl/battle_effect.c:3046`; `src/std/std_effect.c:3209`; `src/vs/vs_effect.c:2792` | **Missing** |
| `SetPolyF4` | 14 | `src/main/script_common.c:2293,2297`; `src/main/ui.c:170` | **Has it:** `libgpu.c:207` |
| `SetPolyFT3` | 9 | `src/btl/battle_effect.c:5938,6657`; `src/doo2/doo2.c:241` | **Missing** |
| `SetPolyFT4` | 117 | `src/main/btl.c:587`; `src/main/drop_shadow.c:58`; `src/main/efe.c:681` | **Missing** |
| `SetPolyG4` | 1 | `src/main/main_menu.c:1583` | **Missing** |
| `SetPolyGT4` | 2 | `src/std/std_main.c:1860`; `src/vs/vs_intro.c:334` | **Missing** |
| `SetSemiTrans` | 47 | `src/main/btl.c:591`; `src/main/drop_shadow.c:59`; `src/main/efe.c:682` | **Missing** |
| `StoreImage` | 15 | `src/main/efe.c:982`; `src/main/effect_common.c:358,412` | **Has it:** `libgpu.c:80` |

#### libgpu macros — 32 names, 846 occurrences

All 32 macro spellings below are **missing from DW2-PC’s `psyq/` headers**. Its public GPU surface is declared at `libgpu.h:69–91`; `AddPrim` and `SetPolyF4` implementations can support corresponding adapters at `libgpu.c:198,207`.

Exact original SDK macro expansions are **unverified**, because the configured `mw_lib41` headers are absent.

| Macro | Count | Game sites |
|---|---:|---|
| `addPrim` | 19 | `src/main/line.c:22,38,56` |
| `getClut` | 32 | `src/main/efe.c:567,686,939` |
| `getTPage` | 93 | `src/main/btl.c:570,589`; `src/main/drop_shadow.c:60` |
| `setClut` | 46 | `src/main/drop_shadow.c:61`; `src/main/item.c:2521`; `src/main/main.c:4128` |
| `setDrawTPage` | 4 | `src/main/line.c:20`; `src/btl/battle_effect.c:2456`; `src/std/std_effect.c:2624`; `src/vs/vs_effect.c:2207` |
| `setLineF2` | 3 | `src/main/line.c:53`; `src/main/model.c:3252`; `src/doo2/doo2.c:540` |
| `setLineF3` | 1 | `src/main/line.c:35` |
| `setLineF4` | 2 | `src/doo2/doo2.c:492,536` |
| `setPolyF4` | 3 | `src/btl/battle_effect.c:2449`; `src/std/std_effect.c:2617`; `src/vs/vs_effect.c:2200` |
| `setPolyFT4` | 4 | `src/fish/fish.c:1295,1654,1700,2283` |
| `setPolyGT3` | 2 | `src/main/model.c:3190`; `src/doo2/doo2.c:475` |
| `setPolyGT4` | 2 | `src/main/model.c:3232`; `src/doo2/doo2.c:520` |
| `setRECT` | 185 | `src/main/anim.c:115,326`; `src/main/battle_ui.c:284` |
| `setRGB0` | 166 | `src/main/aabb.c:34`; `src/main/battle_ui.c:629,724` |
| `setRGB1` | 3 | `src/main/main_menu.c:1585`; `src/std/std_main.c:1864`; `src/vs/vs_intro.c:338` |
| `setRGB2` | 3 | `src/main/main_menu.c:1586`; `src/std/std_main.c:1865`; `src/vs/vs_intro.c:339` |
| `setRGB3` | 3 | `src/main/main_menu.c:1587`; `src/std/std_main.c:1866`; `src/vs/vs_intro.c:340` |
| `setSemiTrans` | 19 | `src/main/line.c:36,54`; `src/btl/battle_effect.c:2450` |
| `setShadeTex` | 9 | `src/btl/battle_effect.c:2451,2675,5312` |
| `setTPage` | 11 | `src/main/main.c:4126,4136,4144` |
| `setUV0` | 1 | `src/fish/fish.c:2279` |
| `setUV3` | 12 | `src/main/model.c:3187`; `src/btl/battle_effect.c:5943,6662` |
| `setUV4` | 8 | `src/main/game_menu.c:1614`; `src/main/model.c:3229`; `src/shop/script_menu.c:3235` |
| `setUVWH` | 91 | `src/main/btl.c:592`; `src/main/drop_shadow.c:62`; `src/main/efe.c:687` |
| `setWH` | 16 | `src/main/btl.c:560`; `src/main/clock.c:227`; `src/main/game_menu.c:2006` |
| `setXY0` | 2 | `src/std/std_hud.c:1081`; `src/vs/vs_hud.c:1298` |
| `setXY2` | 10 | `src/main/line.c:52`; `src/main/model.c:3256`; `src/btl/battle_effect.c:2537` |
| `setXY3` | 3 | `src/main/line.c:34`; `src/main/main_menu.c:5264,5268` |
| `setXY4` | 29 | `src/btl/battle_effect.c:2255,3048,3166` |
| `setXYWH` | 57 | `src/main/battle_ui.c:456,475`; `src/main/btl.c:597` |
| `setcode` | 2 | `src/main/model.c:3204,3244` |
| `setlen` | 5 | `src/main/model.c:3203,3243`; `src/btl/battle_effect.c:2672`; `src/std/std_effect.c:2830`; `src/vs/vs_effect.c:2413` |

The game’s `setUVDataPolyFT4` and `setPosDataPolyFT4` are game helpers, not SDK entry points; their bodies wrap `setUVWH` and `setXYWH` at `src/main/utils.c:297–305`.

### libgs — 35 functions, 962 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `GsClearOt` | 32 | `src/main/btl.c:147`; `src/main/game_menu.c:3975,4036` | **Missing** |
| `GsDefDispBuff` | 1 | `src/main/main.c:3564` | **Missing** |
| `GsDrawOt` | 10 | `src/main/btl.c:158`; `src/main/main.c:3660,3679` | **Missing**; lower-level `DrawOTag`, `libgpu.c:106` |
| `GsGetActiveBuff` | 11 | `src/main/btl.c:144`; `src/main/main.c:3648,3667` | **Missing** |
| `GsGetLs` | 15 | `src/main/effect_common.c:606`; `src/main/game_menu.c:4002`; `src/main/map_object.c:1077` | **Has it:** `libgs.c:497` |
| `GsGetLw` | 15 | `src/main/effect_common.c:603`; `src/main/game_menu.c:4000`; `src/main/map_object.c:1074` | **Different API:** implementation is private/static, `libgs.c:277` |
| `GsGetLws` | 9 | `src/main/model.c:2904`; `src/dooa/dooa.c:1187`; `src/std/std_hud.c:1202` | **Missing** |
| `GsGetTimInfo` | 20 | `src/main/file.c:65,105,123` | **Has it:** `libgs.c:474` |
| `GsGetWorkBase` | 178 | `src/main/aabb.c:31`; `src/main/battle_ui.c:627`; `src/main/btl.c:586` | **Missing** |
| `GsInit3D` | 1 | `src/main/main.c:3574` | **Has it:** `libgs.c:389` |
| `GsInitCoordinate2` | 23 | `src/main/effect_common.c:593`; `src/main/map_object.c:303,455` | **Has it:** `libgs.c:417` |
| `GsInitGraph` | 1 | `src/main/main.c:3563` | **Has it:** `libgs.c:341` |
| `GsLinkObject4` | 30 | `src/main/effect_common.c:592`; `src/main/map_object.c:301,459` | **Missing** |
| `GsMapModelingData` | 35 | `src/main/buff_model.c:83`; `src/main/map_object.c:300,453` | **Missing** |
| `GsMulCoord0` | 21 | `src/btl/battle_effect.c:2625,2786,2860` | **Missing** |
| `GsMulCoord3` | 1 | `src/main/vecmath.c:275` | **Different API:** private/static helper, `libgs.c:266` |
| `GsSetAmbient` | 4 | `src/main/map.c:4176`; `src/main/new_game.c:92`; `src/std/std_setup.c:497`; `src/vs/vs_scene.c:1541` | **Has it:** `libgs.c:467` |
| `GsSetFlatLight` | 16 | `src/main/game_menu.c:3980,4014`; `src/main/map.c:4169` | **Has it:** `libgs.c:437` |
| `GsSetLightMatrix` | 41 | `src/main/effect_common.c:604`; `src/main/game_menu.c:4001`; `src/main/map_object.c:1075` | **Missing**; GTE register setter exists as `GteC_SetLightMatrix`, `gte.c:461`, but does not establish equivalent libgs behavior |
| `GsSetLightMode` | 2 | `src/std/std_setup.c:498`; `src/vs/vs_scene.c:1542` | **Has it:** `libgs.c:461` |
| `GsSetLsMatrix` | 63 | `src/main/effect_common.c:508,607`; `src/main/game_menu.c:4003` | **Has it:** `libgs.c:426` |
| `GsSetNearClip` | 1 | `src/main/main.c:4199` | **Missing** |
| `GsSetOrign` | 9 | `src/main/btl.c:155`; `src/main/main.c:3658,3677` | **Missing**; different offset API `GsSetOffset`, `libgs.c:399`; equivalence **unverified** |
| `GsSetProjection` | 49 | `src/main/game_menu.c:3973,4010,4034` | **Has it:** `libgs.c:432` |
| `GsSetRefView2` | 35 | `src/main/game_menu.c:3974,4011,4035` | **Has it:** `libgs.c:551` |
| `GsSetView2` | 14 | `src/std/std_setup.c:429`; `src/std/std_main.c:2221,2263` | **Missing**; `GsSetRefView2` takes a different view representation, `libgs.c:551` |
| `GsSetWorkBase` | 169 | `src/main/aabb.c:45`; `src/main/battle_ui.c:635`; `src/main/btl.c:146` | **Missing** |
| `GsSortBoxFill` | 19 | `src/main/battle_ui.c:679,687,707` | **Missing** |
| `GsSortClear` | 8 | `src/main/btl.c:157`; `src/main/main.c:3715,3764` | **Missing** |
| `GsSortFastSprite` | 1 | `src/btl/battle_main.c:2313` | **Missing** |
| `GsSortObject4J` | 42 | `src/main/effect_common.c:609`; `src/main/game_menu.c:4004`; `src/main/map_object.c:1080` | **Missing** |
| `GsSortOt` | 21 | `src/main/game_menu.c:4009,4042`; `src/dooa/dooa.c:857` | **Missing** |
| `GsSortPoly` | 10 | `src/main/battle_ui.c:457,476`; `src/main/script_common.c:2236` | **Missing** |
| `GsSortSprite` | 45 | `src/main/btl.c:602`; `src/main/clock.c:283`; `src/main/effect_common.c:532` | **Missing** |
| `GsSwapDispBuff` | 10 | `src/main/btl.c:156`; `src/main/main.c:3659,3678` | **Missing** |

#### TMD dispatch targets — 27 additional functions

Each has **zero direct C calls and one dispatch-table assignment**. Runtime dispatch counts are **unverified**. All are **missing** from DW2-PC.

They belong to the libgs object-rendering interface, but this executable physically places their code in the extracted **`libgte` section**: `config/us/main.yaml:69`, `config/us/symbols.txt:262–288`.

| Target | Assignment |
|---|---|
| `GsTMDfastF3L` | `src/main/graphics.c:62` |
| `GsTMDfastTF3L` | `src/main/graphics.c:63` |
| `GsTMDfastTF3NL` | `src/main/graphics.c:64` |
| `GsTMDfastTNF3` | `src/main/graphics.c:65` |
| `GsTMDfastG3L` | `src/main/graphics.c:66` |
| `GsTMDfastTG3L` | `src/main/graphics.c:67` |
| `GsTMDfastTG3NL` | `src/main/graphics.c:68` |
| `GsTMDfastTNG3` | `src/main/graphics.c:69` |
| `GsTMDfastF4L` | `src/main/graphics.c:70` |
| `GsTMDfastTF4L` | `src/main/graphics.c:71` |
| `GsTMDfastTF4NL` | `src/main/graphics.c:72` |
| `GsTMDfastTNF4` | `src/main/graphics.c:73` |
| `GsTMDfastG4L` | `src/main/graphics.c:74` |
| `GsTMDfastTG4L` | `src/main/graphics.c:75` |
| `GsTMDfastTG4NL` | `src/main/graphics.c:76` |
| `GsTMDfastTNG4` | `src/main/graphics.c:77` |
| `GsTMDdivTF4L` | `src/main/graphics.c:78` |
| `GsTMDdivTF4NL` | `src/main/graphics.c:79` |
| `GsTMDdivTF3NL` | `src/main/graphics.c:80` |
| `GsTMDdivTG4NL` | `src/main/graphics.c:81` |
| `GsTMDdivTG3NL` | `src/main/graphics.c:82` |
| `GsTMDdivTNG4` | `src/main/graphics.c:83` |
| `GsTMDdivTNG3` | `src/main/graphics.c:84` |
| `GsTMDdivTNF4` | `src/main/graphics.c:85` |
| `GsTMDdivTNF3` | `src/main/graphics.c:86` |
| `GsTMDfastF4NL` | `src/main/graphics.c:87` |
| `GsTMDfastNF4` | `src/main/graphics.c:88` |

### libgte — 29 directly called functions, 567 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `ApplyMatrix` | 8 | `src/main/vecmath.c:61`; `src/fish/fish.c:1641,2158` | **Missing** |
| `ApplyMatrixLV` | 23 | `src/main/anim.c:480`; `src/main/battle_tick.c:160,226` | **Has it:** `libgte.c:143` |
| `ApplyMatrixSV` | 103 | `src/main/efe.c:365,780`; `src/main/effect_common.c:314` | **Has it:** `libgte.c:194` |
| `ApplyTransposeMatrixLV` | 3 | `src/fish/fish.c:3509,3977,4125` | **Missing** |
| `CompMatrix` | 2 | `src/main/model.c:3171`; `src/doo2/doo2.c:453` | **Missing** |
| `MulMatrix` | 2 | `src/fish/fish.c:3494,3526` | **Different API:** private/static implementation, `libgs.c:113` |
| `MulMatrix0` | 9 | `src/main/model.c:3167,3169`; `src/main/vecmath.c:293` | **Missing**; private multiplication core exists, `libgs.c:87` |
| `NormalColorCol` | 2 | `src/main/model.c:3228`; `src/doo2/doo2.c:515` | **Missing** |
| `NormalColorCol3` | 4 | `src/main/model.c:3183,3224`; `src/doo2/doo2.c:468,511` | **Missing** |
| `PopMatrix` | 12 | `src/btl/battle_effect.c:3015,3041,3115` | **Has it:** `libgte.c:182` |
| `PushMatrix` | 12 | `src/btl/battle_effect.c:3006,3023,3096` | **Has it:** `libgte.c:170` |
| `RotMatrix` | 51 | `src/main/anim.c:58,125`; `src/main/battle_tick.c:159` | **Missing** |
| `RotMatrixYXZ` | 44 | `src/main/vecmath.c:113,291,292` | **Has it:** `libgte.c:281` |
| `RotMatrixZYX` | 44 | `src/main/efe.c:364,779`; `src/main/particle.c:122` | **Missing** |
| `RotNclip3` | 4 | `src/main/model.c:3180,3198`; `src/doo2/doo2.c:461,487` | **Missing** |
| `RotNclip4` | 4 | `src/main/model.c:3220,3239`; `src/doo2/doo2.c:503,532` | **Missing** |
| `RotTransPers` | 4 | `src/main/aabb.c:30`; `src/btl/battle_effect.c:4116`; `src/std/std_effect.c:4298`; `src/vs/vs_effect.c:3881` | **Has it:** `libgte.c:248` |
| `RotTransPers3` | 1 | `src/main/effect_common.c:475` | **Missing** |
| `RotTransPers4` | 9 | `src/main/drop_shadow.c:64`; `src/main/effect_common.c:492`; `src/btl/battle_effect.c:2656` | **Missing** |
| `ScaleMatrix` | 39 | `src/main/anim.c:59,126`; `src/main/effect_common.c:348` | **Has it:** `libgte.c:208` |
| `SetLightMatrix` | 2 | `src/main/model.c:3170`; `src/doo2/doo2.c:452` | **Different API:** `GteC_SetLightMatrix`, `gte.c:461`, exposed as `gte_SetLightMatrix`, `gte_native.h:35` |
| `SetRotMatrix` | 16 | `src/main/effect_common.c:473,490`; `src/main/map.c:4362` | **Has it:** `libgte.c:222` |
| `SetTransMatrix` | 16 | `src/main/effect_common.c:474,491`; `src/main/map.c:4363` | **Has it:** `libgte.c:229` |
| `SquareRoot0` | 3 | `src/kar/kar.c:1537`; `src/std/std_main.c:2238`; `src/vs/vs_camera.c:193` | **Different API:** private/static implementation, `libgs.c:140` |
| `TransMatrix` | 37 | `src/main/anim.c:55,124`; `src/main/effect_common.c:349` | **Missing** |
| `TransposeMatrix` | 7 | `src/main/vecmath.c:60`; `src/btl/battle_effect.c:2734,2810` | **Different API:** private/static implementation, `libgs.c:123` |
| `ratan2` | 43 | `src/main/vecmath.c:122,124,188` | **Has it:** `libgte.c:84` |
| `rcos` | 30 | `src/main/vecmath.c:197,199,238` | **Has it:** `libgte.c:72` |
| `rsin` | 33 | `src/main/vecmath.c:196,198,237` | **Has it:** `libgte.c:66` |

The **11 used `gte_*` macros and their 181 occurrences** are exhaustively listed in A3.

`getScratchAddr` is another SDK macro: **31 direct C expressions**, plus **1,086 occurrences through game scratch macros**. Its definition is absent with the SDK headers. DW2-PC supplies a **different API**, `PS1_SCRATCHPAD`, at `ps1mem.h:30`. Full sites are in A4.

### libcd — 16 functions, 44 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `CdControl` | 9 | `src/main/efe.c:1032`; `src/main/file.c:88,144` | **Has it:** `libcd.c:314` |
| `CdControlB` | 1 | `src/mov/mov.c:269` | **Has it:** `libcd.c:354` |
| `CdInit` | 2 | `src/main/main.c:3390`; `src/main/utils2.c:394` | **Has it:** `libcd.c:261`; initialization stub |
| `CdIntToPos` | 5 | `src/main/efe.c:1052`; `src/main/file.c:31,141`; `src/main/file_queue.c:272`; `src/main/script_interp.c:2811` | **Has it:** `libcd.c:276` |
| `CdPosToInt` | 4 | `src/main/efe.c:1033`; `src/main/file.c:141`; `src/main/file_queue.c:272`; `src/main/script_interp.c:2804` | **Has it:** `libcd.c:271` |
| `CdRead` | 4 | `src/main/file.c:91,147`; `src/main/file_queue.c:143`; `src/main/script_interp.c:2815` | **Missing**; DW2 has lower-level `CdRead2`/`CdGetSector`, `libcd.c:400,363` |
| `CdRead2` | 1 | `src/mov/mov.c:132` | **Has it:** `libcd.c:400` |
| `CdReadSync` | 6 | `src/main/efe.c:1023`; `src/main/efe_table.c:31`; `src/main/file.c:93` | **Missing**; `CdSync` concerns command completion, `libcd.c:290` |
| `CdSearchFile` | 5 | `src/main/efe.c:1030`; `src/main/efe_table.c:38`; `src/main/file_queue.c:133`; `src/main/script_interp.c:2797`; `src/mov/mov.c:225` | **Missing** |
| `StCdInterrupt` | 1 | `src/mov/mov.c:91` | **Has it:** `libcd.c:522`; no-op |
| `StFreeRing` | 1 | `src/mov/mov.c:150` | **Has it:** `libcd.c:443`; no-op |
| `StGetBackloc` | 1 | `src/mov/mov.c:250` | **Missing** |
| `StGetNext` | 1 | `src/mov/mov.c:184` | **Has it:** `libcd.c:482`; real/stub movie paths |
| `StSetRing` | 1 | `src/mov/mov.c:115` | **Has it:** `libcd.c:418` |
| `StSetStream` | 1 | `src/mov/mov.c:116` | **Has it:** `libcd.c:439`; logs arguments without implementing their full semantics |
| `StUnSetRing` | 1 | `src/mov/mov.c:268` | **Has it:** `libcd.c:425` |

### libspu — 6 functions, 7 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `SpuClearReverbWorkArea` | 1 | `src/main/sound.c:544` | **Has it:** `libspu.c:988` |
| `SpuGetKeyStatus` | 2 | `src/main/sound.c:575,658` | **Missing** |
| `SpuSetReverb` | 1 | `src/main/sound.c:547` | **Has it:** `libspu.c:799` |
| `SpuSetReverbDepth` | 1 | `src/main/sound.c:545` | **Different API:** `SsUtSetReverbDepth(short,short)`, `libsnd.c:868`; no same-named SPU entry point |
| `SpuSetReverbModeParam` | 1 | `src/main/sound.c:543` | **Different API:** same name accepts a 32-bit emulated address, `libspu.c:831`, rather than the game’s `SpuReverbAttr *` |
| `SpuSetReverbVoice` | 1 | `src/main/sound.c:546` | **Has it:** `libspu.c:976` |

DW2-PC’s SPU implementation depends on retail-address state and its address-conversion macros: `libspu.c:8–55`, `snd_native.h:32–47`. These existing functions require adaptation for the proposed macOS memory model.

### libsnd — 21 functions, 41 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `SsEnd` | 1 | `src/main/sound.c:563` | **Missing** |
| `SsInit` | 2 | `src/main/main.c:3384`; `src/main/sound.c:522` | **Has it:** `libsnd.c:61` |
| `SsQuit` | 1 | `src/main/sound.c:564` | **Missing** |
| `SsSeqClose` | 1 | `src/main/sound.c:435` | **Different API:** `SsSepClose`, `libsnd.c:698` |
| `SsSeqOpen` | 1 | `src/main/sound.c:485` | **Different API:** `SsSepOpen`, `libsnd.c:494`; SEP parsing is not interchangeable with SEQ parsing |
| `SsSeqPlay` | 2 | `src/main/sound.c:500,502` | **Different API:** `SsSepPlay`, `libsnd.c:732` |
| `SsSeqSetVol` | 1 | `src/main/sound.c:494` | **Different API:** `SsSepSetVol`, `libsnd.c:776` |
| `SsSeqStop` | 1 | `src/main/sound.c:426` | **Different API:** `SsSepStop`, `libsnd.c:770` |
| `SsSetMVol` | 1 | `src/main/sound.c:535` | **Has it:** `libsnd.c:266` |
| `SsSetTableSize` | 1 | `src/main/sound.c:523` | **Has it:** `libsnd.c:229` |
| `SsSetTickMode` | 1 | `src/main/sound.c:524` | **Has it:** `libsnd.c:184` |
| `SsStart` | 1 | `src/main/sound.c:534` | **Different API:** `SsStart2`, `libsnd.c:92`; common internal `_SsStart`, `:98` |
| `SsUtAllKeyOff` | 1 | `src/main/sound.c:655` | **Has it:** `libsnd.c:814` |
| `SsUtAutoVol` | 2 | `src/main/sound.c:635,660` | **Missing** |
| `SsUtGetVBaddrInSB` | 4 | `src/main/sound.c:415,470`; `src/main/sound_async.c:165,196` | **Missing** |
| `SsUtKeyOffV` | 2 | `src/main/sound.c:636,661` | **Missing**; sequence-oriented private voice routines exist, `snd_vm.c:444,709` |
| `SsUtKeyOnV` | 1 | `src/main/sound.c:621` | **Missing**; sequence-oriented private voice routines exist, `snd_vm.c:343,739` |
| `SsVabClose` | 5 | `src/main/sound.c:404,459,560`; `src/main/sound_async.c:144,183` | **Has it:** `libsnd.c:476` |
| `SsVabOpenHeadSticky` | 4 | `src/main/sound.c:405,460`; `src/main/sound_async.c:145,184` | **Different API:** allocating `SsVabOpenHead`, `libsnd.c:336`; internal mode function `:342` |
| `SsVabTransBody` | 4 | `src/main/sound.c:410,465`; `src/main/sound_async.c:152,190` | **Has it:** `libsnd.c:449` |
| `SsVabTransCompleted` | 4 | `src/main/sound.c:414,469`; `src/main/sound_async.c:160,195` | **Has it:** `libsnd.c:470` |

### libmcrd — 11 functions, 61 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `MemCardAccept` | 6 | `src/main/main_menu.c:3488,3690,3811` | **Has it:** `libmcrd.c:188` |
| `MemCardCreateFile` | 1 | `src/main/main_menu.c:3561` | **Has it:** `libmcrd.c:254` |
| `MemCardDeleteFile` | 1 | `src/main/main_menu.c:3871` | **Missing** |
| `MemCardExist` | 4 | `src/main/main_menu.c:3294,3903,4262,4748` | **Has it:** `libmcrd.c:183` |
| `MemCardFormat` | 1 | `src/main/main_menu.c:3409` | **Has it:** `libmcrd.c:290` |
| `MemCardGetDirentry` | 4 | `src/main/main_menu.c:3932,4291,4770,4805` | **Missing** |
| `MemCardInit` | 1 | `src/main/main.c:3391` | **Has it:** `libmcrd.c:173` |
| `MemCardReadFile` | 3 | `src/main/main_menu.c:3751,4151,4828` | **Has it:** `libmcrd.c:234` |
| `MemCardStart` | 1 | `src/main/main.c:3392` | **Has it:** `libmcrd.c:178` |
| `MemCardSync` | 36 | `src/main/main_menu.c:3292,3408,3487` | **Has it:** `libmcrd.c:242` |
| `MemCardWriteFile` | 3 | `src/main/main_menu.c:3623,3999,4358` | **Has it:** `libmcrd.c:238` |

### libpress — 6 functions, 8 calls

| Function | Count | Game sites | DW2-PC coverage |
|---|---:|---|---|
| `DecDCTReset` | 1 | `src/mov/mov.c:113` | **Has it:** `libpress.c:21`; no-op |
| `DecDCTin` | 1 | `src/mov/mov.c:245` | **Has it:** `libpress.c:26` |
| `DecDCTout` | 2 | `src/mov/mov.c:100,246` | **Has it:** `libpress.c:31` |
| `DecDCToutCallback` | 2 | `src/mov/mov.c:114,267` | **Has it:** `libpress.c:49` |
| `DecDCTvlc2` | 1 | `src/mov/mov.c:149` | **Has it:** `libpress.c:55`; ignores the supplied decode table and assumes DW2’s buffer capacity |
| `DecDCTvlcBuild` | 1 | `src/mov/mov.c:232` | **Missing** |

### libc/libmath/stdlib — 16 functions, 431 calls

All sixteen are **missing as Psy-Q implementations in `psyq/`**. DW2-PC uses host C-library facilities, for example `libgs.c:1–3`, `gte.c:1`, and `libcd.c:359`. Native libc can supply many of these names, but that does not verify compatibility with the PS1 runtime.

| Function | Count | Game sites |
|---|---:|---|
| `abs` | 25 | `src/main/anim.c:415`; `src/main/efe.c:1091,1094` |
| `bzero` | 1 | `src/main/fish.c:132` |
| `exit` | 7 | `src/btl/battle_effect.c:5330`; `src/fish/fish_model.c:24,34` |
| `longjmp` | 37 | `src/main/script_interp.c:520,618,708` |
| `memcpy` | 52 | `src/main/anim.c:68,69,70` |
| `memset` | 2 | `src/main/script_interp.c:2205`; `src/dget/dget.c:677` |
| `printf` | 3 | `src/btl/battle_effect.c:3575`; `src/std/std_effect.c:3757`; `src/vs/vs_effect.c:3340` |
| `rand` | 154 | `src/main/efe.c:359,360,361` |
| `setjmp` | 1 | `src/main/script_interp.c:601` |
| `sprintf` | 7 | `src/main/game_menu.c:4143`; `src/main/item.c:3064`; `src/main/main_menu.c:3228` |
| `strcat` | 33 | `src/main/efe.c:1029`; `src/main/efe_table.c:37`; `src/main/file_queue.c:132` |
| `strcmp` | 1 | `src/main/file.c:29` |
| `strcpy` | 48 | `src/main/efe.c:1028`; `src/main/efe_table.c:36`; `src/main/file_queue.c:131` |
| `strlen` | 50 | `src/main/game_menu.c:2439,3016,3651` |
| `strncpy` | 3 | `src/main/main_menu.c:4850,4852,4854` |
| `strrchr` | 7 | `src/main/file.c:22`; `src/main/model.c:3292`; `src/main/sound.c:392` |

`randomLimit` explicitly scales `rand()` with `>>15`, so host `rand()` cannot be substituted without checking its range and sequence: `src/main/math.c:5–10`. No SDK `srand`, floating-point libmath call, or separate linked libmath section was found. `_sin`, `_cos`, and `_atan` are game implementations at `src/main/math.c:13,27,32`.

### libds and other libraries

- **libds:** zero direct game-C function/global uses. Its extracted section contains `DsReadyCallback` and `DsDataCallback`, `config/us/symbols.txt:1229–1230`, and is linked at `config/us/main.yaml:115`.
- **Other SDK libraries:** no additional direct function or global uses found in the US C set.
- Metrowerks startup/register operations and linker globals are listed below and in A4.

### SDK globals

| Global | C references | Sites | DW2-PC coverage |
|---|---:|---|---|
| `GsWSMATRIX` | 108, including two declarations | Full grouped list below | **Has it:** `libgs.c:25`, `libgs.h:69` |
| `GsLIGHTWSMATRIX` | 2 | `src/main/model.c:3169`; `src/doo2/doo2.c:451` | **Has it:** `libgs.c:26`, `libgs.h:70` |
| `GsIDMATRIX` | 4 | `src/std/std_main.c:2258,2273`; `src/vs/vs_camera.c:213,228` | **Different API:** private/static, `libgs.c:27` |
| `GsFCALL4` | 27 assignments, one declaration | `src/main/graphics.c:8,62–88` | **Missing** |
| `StCdIntrFlag` | 2 accesses, one declaration | `src/mov/mov.c:16,90,92` | **Different declaration:** DW1 array of `int32_t`; DW2 scalar `u_long`, `libcd.c:31`, `libcd.h:62` |

Full `GsWSMATRIX` references:

- `src/main/drop_shadow.c:63`
- `src/main/effect_common.c:17,473,474,490,491,508`
- `src/main/game_menu.c:3996,4028`
- `src/main/item.c:2482,2516`
- `src/main/map.c:184,3057,3323,3614,3884,4362,4363,4479,4480,4874,4875,5134,5135`
- `src/main/map_object.c:1052`
- `src/main/model.c:3171`
- `src/main/overworld.c:429,430,943,944`
- `src/main/partner_impl.c:544,545`
- `src/main/tamer.c:240,1275,1276`
- `src/main/utils.c:184`
- `src/main/vecmath.c:57,58,59,60`
- `src/btl/battle_effect.c:2625,2734,2786,2810,2860,3016,3033,4113,4114,4593,5268`
- `src/btl/battle_main.c:631,2259`
- `src/btl/command_menu.c:373`
- `src/doo2/doo2.c:453`
- `src/dooa/dooa.c:844`
- `src/fish/fish.c:974,975,1092,1669,2261,2596,3544,3640`
- `src/kar/kar.c:1050`
- `src/murd/murd.c:519`
- `src/std/std_main.c:5685`
- `src/std/std_hud.c:1198`
- `src/std/std_effect.c:1358,1496,1582,2783,2892,2944,2968,3018,3174,3196,3365,3366,3367,3378,4295,4296,4782,5462`
- `src/vs/vs_combat.c:2565`
- `src/vs/vs_effect.c:2366,2475,2527,2551,2601,2757,2779,2948,2949,2950,2961,3878,3879,4367,5047`
- `src/vs/vs_hud.c:840,958,993,1085,1380,1531`

Original SDK-global addresses are recorded at `config/us/symbols.txt:5079,5081–5082,5092,5128`.

### Metrowerks/linker globals

All eight are **missing from DW2-PC’s Psy-Q replacement layer**; they need native startup/linker treatment.

| Global | All used sites |
|---|---|
| `_gp_addr` | declaration `src/main/_psstart.c:29`; startup load `:85` |
| `_stack_addr` | `src/main/_psstart.c:77`; `src/main/main.c:3538` |
| `_fsbss` | `src/main/_psstart.c:88` |
| `_sbss_size` | `src/main/_psstart.c:89` |
| `_fbss` | `src/main/_psstart.c:100` |
| `_bss_size` | `src/main/_psstart.c:101` |
| `_end` | `src/main/main.c:3537` |
| `_stack_size` | `src/main/main.c:3538` |

The C++ static-initializer array and C++ heap initialization are inactive: `src/main/_psstart.c:150–171,208–217`.

### Installed callbacks and timing

| Mechanism | Verified installation/use | Timing implication |
|---|---|---|
| MDEC output callback | `MOV_onSliceDecoded` passed at `src/mov/mov.c:231`, installed through `DecDCToutCallback` at `:114`, removed at `:267` | Slice-completion callback advances double buffers, starts the next output slice, and uploads pixels: `:83–107`. DW2 calls callbacks synchronously and drains pending completions before returning, `libpress.c:31–46`; equivalent timing is **unverified**. |
| Sequencer tick selection | `SsSetTickMode(SS_TICK240)`, `src/main/sound.c:524`; `SsStart`, `:534` | Requests the SDK’s 240-tick mode. Exact DW1 timer setup is **unverified** without assembly. DW2’s mode decoding selects 240 at `libsnd.c:210–212`; its internal start installs timer/interrupt callbacks at `:98–163`. |
| CD completion | Queue polls `CdReadSync(1,NULL)`, `src/main/file_queue.c:152`, then calls the game completion callback at `:155–156` | Completion becomes visible through the game’s queue tick, registered at `:87`, rather than a game-installed CD SDK callback. Blocking reads also poll `CdReadSync(0,NULL)`, `src/main/file.c:93`. |
| SPU transfer completion | `SsVabTransCompleted(0)`, `src/main/sound_async.c:160,195` | Sound loading has completion-dependent states. DW2 forwards this to `SpuIsTransferCompleted`, `libsnd.c:470–472`. |
| GPU completion | `DrawSync(0)` before VSync/presentation, e.g. `src/main/btl.c:152–158` | DW2’s `DrawSync` always reports completion because submission is synchronous, `libgpu.c:56–59`. |

**No US game-C installation** of `VSyncCallback`, `CdReadCallback`, `CdSyncCallback`, `CdReadyCallback`, or `DrawSyncCallback` was found. The SDK may install internal callbacks; those are a separate, **unverified DW1 transitive dependency**.

All 15 `VSync` sites:

| Mode | All sites |
|---|---|
| `0` — next VBlank | `src/main/main.c:3656,3675,3712`; `src/mov/mov.c:257` |
| `3` — paced multi-VBlank wait | `src/main/btl.c:153`; `src/main/main.c:3761,3958`; `src/main/utils.c:86`; `src/btl/battle_main.c:2606`; `src/mov/mov.c:131`; `src/std/std_main.c:6205`; `src/vs/vs_main.c:1008` |
| `-1` — counter query | `src/fish/fish.c:2975,3067,3073` |

DW2’s implementation tracks the last wait and pumps its host clock, `libetc.c:25–44`; its VBlank also advances pads, cards, sound, and CD, `:14–22`. This couples more than presentation timing.

## A2. Extracted libraries and indirect SDK reachability

### Libraries linked into the US executable

All twelve text sections are explicitly linked. Ten also have generated BSS objects.

| Library | `config/us/main.yaml` text | `config/us/main.ld` text | Generated BSS: makefile / linker |
|---|---|---|---|
| libapi | `:64` | `:21` | `mk/version/us.mk:6`; `main.ld:839` |
| libc | `:65` | `:23` | None listed |
| libetc | `:66` | `:25` | `us.mk:7`; `main.ld:841` |
| libgpu | `:67` | `:27` | `us.mk:8`; `main.ld:843` |
| libgs | `:68` | `:29` | `us.mk:9`; `main.ld:845` |
| libgte | `:69` | `:31` | `us.mk:10`; `main.ld:847` |
| libcd | `:82` | `:57` | `us.mk:11`; `main.ld:861` |
| libspu | `:91` | `:75` | `us.mk:12`; `main.ld:873` |
| libsnd | `:92` | `:77` | `us.mk:13`; `main.ld:875` |
| libpress | `:114` | `:120` | None listed |
| libds | `:115` | `:122` | `us.mk:14`; `main.ld:897` |
| libmcrd | `:117` | `:126` | `us.mk:15`; `main.ld:901` |

Additional extracted sections in `config/us/main.yaml`:

- **rodata:** libc `:136`; libetc `:137`; libgpu `:138`; libgs `:139`; libcd `:142`; libspu `:146`; libsnd `:147`; libpress `:152`; libmcrd `:153`.
- **data:** libapi `:164`; libc `:165`; libetc `:166`; libgpu `:167`; libgte `:168`; libcd `:177`; libspu `:183`; libsnd `:184`; libpress `:198`; libmcrd `:200`.
- **BSS address partitions:** `config/us/bss.yaml:5–9,16,22–23,34,36`.

Library extraction is coarse: the makefile explicitly says a library file is retained whenever anything in it is referenced, `mk/version/us.mk:81–82`. **Linked presence therefore does not prove runtime reachability of every symbol.**

### Indirect SDK internals

**Verified from game C:** all 27 handler addresses in A1 enter `GsFCALL4`; `GsSortObject4J` is called 42 times. The precise table-dispatch implementation remains **unverified**, because its assembly body is absent. Its symbol is at `config/us/symbols.txt:220`.

The following are the specific internal dependencies/candidates exposed by the symbol map and the corresponding DW2 implementations. **DW1’s exact transitive edges are unverified.**

| Pipeline | Internals to account for | Evidence |
|---|---|---|
| TMD subdivision | `RCpolyFT3A`, `Add_Prim_FT3`, `RCpolyGT3A`, `Add_Prim_GT3`, `RCpolyFT4A`, `Add_Prim_FT4`, `RCpolyGT4A`, `Add_Prim_GT4` | Symbols `config/us/symbols.txt:289–296`; divided handlers installed at `src/main/graphics.c:78–86`; all missing in DW2 |
| Coordinate chains/cache | `GsMulCoord2`, `GsMulCoord3`, `GsGetLw`, matrix multiplication/transposition | DW1 symbols `config/us/symbols.txt:190–191,216`; DW2 private implementations `libgs.c:113,118,123,256,266,277` |
| View construction | `scale_view_param`, `select_max_param`, `len_param`, `Gssub_make_matrix`, Z-rotation helper | DW1 symbols `config/us/symbols.txt:205,208,211–215`; DW2 view implementation `libgs.c:551–634` |
| Lighting state | `gte_set_lc`, `gte_read_lc`, color/background/far-color register setters | DW1 symbols `config/us/symbols.txt:197–201,239,244–245`; DW2 helpers `libgs.c:67,73,79`, light setup `:437–469` |
| Packet generation and GPU queue | `_make_packet`, `_addque`, `_addque2`, `_exeque`, `_sync`, GPU initialization | DW1 symbols `config/us/symbols.txt:137–141,172,177–178`; SDK bodies absent |
| Sequencer interrupts | `_SsStart`, sequencer tick callback and root-counter/interrupt installation | DW1 `_SsStart` symbol `config/us/symbols.txt:829`; game starts it through `SsStart`, `src/main/sound.c:534`; DW2 implementation `libsnd.c:98–163` |
| SPU DMA completion | SPU transfer state and internal DMA callback handling | Game polls completion at `src/main/sound_async.c:160,195`; DW2 hardware layer `snd_hw.c:44–55,111–113` |
| Pad/card/CD interrupt support | libapi BIOS/event routines and libetc callback machinery | Linked symbols `config/us/symbols.txt:5–21,75–99`; exact DW1 edges unavailable |

An exhaustive, proven **SDK-internal call graph cannot be produced from this checkout**. The entries above distinguish installed addresses and C calls from inferred SDK internals.

## A3. GTE

### Every used `gte_*` macro

There are **11 macro names and 181 occurrences**. Only **RTPS and RTPT** are directly issued GTE commands in the game C; the other macros transfer registers or read results.

| Macro | Count | Metrowerks / GCC definition | DW2-PC |
|---|---:|---|---|
| `gte_ldv0` | 42 | `include/mwcc/inline_n.h:45`; `include/gcc/inline_n.h:9` | **Has it:** `gte_native.h:28`, `gte.c:409` |
| `gte_rtps` | 42 | MW `:540`; GCC `:498` | **Has it:** `gte_native.h:36`, `gte.c:463` |
| `gte_stsxy` | 42 | MW `:1082`; GCC `:976` | **Has it:** `gte_native.h:42`, `gte.c:470` |
| `gte_stszotz` | 19 | MW `:1272`; GCC `:1152` | **Has it:** `gte_native.h:43`, `gte.c:471` |
| `gte_SetRotMatrix` | 1 | MW `:373`; GCC `:294` | **Has it:** `gte_native.h:33`, `gte.c:448` |
| `gte_SetTransMatrix` | 1 | MW `:436`; GCC `:362` | **Has it:** `gte_native.h:34`, `gte.c:455` |
| `gte_ldv3` | 10 | MW `:66`; GCC `:30` | **Missing macro**; register model can support an adapter |
| `gte_rtpt` | 10 | MW `:547`; GCC `:504` | **Missing macro and command implementation** |
| `gte_stsxy3` | 10 | MW `:1088`; GCC `:978` | **Missing macro**; register model can support an adapter |
| `gte_ldlzc` | 2 | MW `:290`; GCC `:228` | **Different API:** `Gte_Mtc2(GTE_LZCS,…)`, `gte.c:319,340–343` |
| `gte_stlzc` | 2 | MW `:1511`; GCC `:1382` | **Different API:** `Gte_Mfc2`, `gte.c:350` |

`gte_stszotz` reads **SZ3 shifted right by two**, not the OTZ register: `include/mwcc/inline_n.h:1275–1279`; DW2 matches that operation at `gte.c:471`.

#### All RTPS-family sites

The following table gives each `gte_ldv0` line. **At every listed site, `gte_rtps` is the next line and `gte_stsxy` is the line after that.** Thus, for example, `src/main/btl.c:576` denotes all three references at `:576,577,578`.

| File | `gte_ldv0` lines |
|---|---|
| `src/main/btl.c` | `576` |
| `src/main/effect_common.c` | `509` |
| `src/main/item.c` | `2483,2542` |
| `src/main/map.c` | `3068,3076,3662,3893,4367,4371,4482,4486,4880,4883,5139,5142` |
| `src/main/map_object.c` | `1058` |
| `src/main/overworld.c` | `445,945` |
| `src/main/tamer.c` | `1280` |
| `src/main/utils.c` | `189` |
| `src/btl/battle_effect.c` | `2022,3034` |
| `src/btl/battle_hud.c` | `668,705,813` |
| `src/btl/battle_main.c` | `636` |
| `src/btl/command_menu.c` | `378` |
| `src/fish/fish.c` | `977` |
| `src/kar/kar.c` | `1052` |
| `src/std/std_main.c` | `5690` |
| `src/std/std_hud.c` | `702,739,847` |
| `src/std/std_effect.c` | `2127,3197` |
| `src/vs/vs_combat.c` | `2570` |
| `src/vs/vs_effect.c` | `1758,2780` |
| `src/vs/vs_hud.c` | `902,939,1055` |

#### All RTPT-family sites

Each `gte_ldv3` is followed immediately by `gte_rtpt`, then `gte_stsxy3`.

| File | `gte_ldv3` lines | `gte_rtpt` lines | `gte_stsxy3` lines |
|---|---|---|---|
| `src/main/item.c` | `2538` | `2539` | `2540` |
| `src/btl/battle_hud.c` | `664,701,809` | `665,702,810` | `666,703,811` |
| `src/std/std_hud.c` | `698,735,843` | `699,736,844` | `700,737,845` |
| `src/vs/vs_hud.c` | `898,935,1051` | `899,936,1052` | `900,937,1053` |

#### All remaining macro sites

| Macro | All sites |
|---|---|
| `gte_stszotz` | `src/main/effect_common.c:512`; `src/main/item.c:2486,2541`; `src/main/overworld.c:448`; `src/btl/battle_effect.c:2025`; `src/btl/battle_hud.c:667,704,812`; `src/kar/kar.c:1055`; `src/std/std_main.c:5693`; `src/std/std_hud.c:701,738,846`; `src/std/std_effect.c:2130`; `src/vs/vs_combat.c:2573`; `src/vs/vs_effect.c:1761`; `src/vs/vs_hud.c:901,938,1054` |
| `gte_SetRotMatrix` | `src/main/graphics.c:56` |
| `gte_SetTransMatrix` | `src/main/graphics.c:57` |
| `gte_ldlzc` | `src/main/vecmath.c:149,162` |
| `gte_stlzc` | `src/main/vecmath.c:154,165` |

### Commands needed inside reached SDK routines

DW2-PC implements exactly **RTPS, NCLIP, MVMVA, NCS, GPF**, selected in `gte.c:286–296`. An unsupported command logs and clears/updates FLAG without computing the missing operation.

The following SDK command mappings are **unverified for DW1’s extracted library bodies**. They describe the expected Psy-Q operations; they are not measured instruction counts.

| Command | Direct game-C command count | Expected SDK use / file evidence | DW2-PC |
|---|---:|---|---|
| **RTPS** | **42** | Single projection; fourth vertex in quad paths. Game wrapper calls include `src/main/drop_shadow.c:64`; SDK symbol `config/us/symbols.txt:252` | **Implemented**, `gte.c:290` |
| **RTPT** | **10** | Triangle transforms in `RotTransPers3`, `RotNclip3/4`, and TMD handlers. `src/main/effect_common.c:475`; `src/main/model.c:3180,3220`; `src/main/graphics.c:62–88` | **Missing** |
| **NCLIP** | 0 | `RotNclip3/4` and TMD face rejection. `src/main/model.c:3180,3198,3220,3239`; symbols `config/us/symbols.txt:254–255` | **Implemented**, `gte.c:291` |
| **MVMVA** | 0 | Matrix/coordinate multiplication and vector transforms. `src/main/model.c:3167,3169,3171`; `src/main/vecmath.c:275,293`; analogous DW2 matrix core `libgs.c:87–111` | **Implemented**, `gte.c:292` |
| **NCCS** | 0 | Expected `NormalColorCol` operation. `src/main/model.c:3228`; `src/doo2/doo2.c:515`; SDK symbol `config/us/symbols.txt:248` | **Missing** |
| **NCCT** | 0 | Expected `NormalColorCol3` operation. `src/main/model.c:3183,3224`; `src/doo2/doo2.c:468,511`; symbol `config/us/symbols.txt:249` | **Missing** |
| **NCS / NCT** | 0 / 0 | Possible normal-lighting operations in lit TMD handlers, `src/main/graphics.c:62–78`; exact handler selection **unverified** | NCS implemented `gte.c:293`; NCT **missing** |
| **AVSZ3 / AVSZ4** | 0 / 0 | Possible ordering-depth calculation inside SDK sorting/projection helpers; **unverified**, not implied merely by `gte_stszotz`. Relevant calls `src/main/model.c:3180,3220` | **Both missing** |
| **GPF / GPL** | 0 / 0 | Possible subdivision/interpolation operations in divided TMD handlers, `src/main/graphics.c:78–86`, and `RCpoly*`, `config/us/symbols.txt:289–296`; **unverified** | GPF implemented `gte.c:294`; GPL **missing** |

Additional fog/depth-cue commands **NCDS, NCDT, DPCS, DPCT, CDP, CC, DCPL, INTPL** exist in the local GTE macro definitions, but their execution through this game’s SDK paths is **unverified**. Definition anchors: `include/gte_macros.inc:23,28,33,38,53,58,73,78`. They should not be counted as established requirements solely because those definitions exist.

Command roles and the distinction between register transfers and command execution were cross-checked against the [PSX-SPX GTE specification](https://psx-spx.consoledev.net/geometrytransformationenginegte/). That specification does not establish DW1 SDK reachability.

**Established immediate gap:** RTPT is directly issued ten times by game C, while DW2-PC’s command dispatcher lacks it. SDK lighting also needs investigation beyond DW2’s NCS-only lighting command coverage.

## A4. Hardware, scratchpad, DMA, and fixed RAM

### Direct hardware-register access and DMA

**No executable US game-C MMIO dereference of a `0x1F80xxxx` literal, and no direct DMA-register programming, was found.**

The only matching eight-digit `0x1F80xxxx` literals are two **data words**, both `0x1f800000`:

- `src/mov/mov_bss_us.c:12423`
- `src/mov/mov_bss_us.c:22682`

Their meaning as addresses is **unverified**; neither is a C load/store through that address.

The Metrowerks startup enables COP2 through **COP0 status-register access**, not MMIO: `src/main/_psstart.c:174–177`. Game GTE macros perform COP2 register transfers and commands as listed in A3.

DMA is reached through SDK operations rather than game-C register writes: movie output `src/mov/mov.c:100,246`, VRAM uploads `:106`, and SPU transfers `src/main/sound_async.c:152,160`. Exact DW1 DMA-register operations inside those SDK routines are **unverified**.

DW2’s reusable hardware layer directly models SPU DMA4 registers and DPCR/DICR at `snd_hw.c:119–148`; this is DW2 implementation evidence, not a direct DW1-C access.

### Scratchpad use — exhaustive

There are **31 direct `getScratchAddr` expressions**, plus **1,086 game-macro occurrences** expanding to scratchpad addresses: **1,117 total source-level scratch expressions**. These are macro occurrence counts; compiler elimination and actual access counts are **unverified**.

The SDK definition of `getScratchAddr` is missing. Conventional word-offset interpretation gives offsets 0, 136, 168, and 176 bytes for arguments 0, 34, 42, and 44, respectively; that exact header expansion is **unverified**.

#### Direct `getScratchAddr` expressions — 31

- `src/main/effect_common.c:609`
- `src/main/game_menu.c:4005`
- `src/main/map_object.c:1080`
- `src/main/model.c:2919,2927,2930`
- `src/dooa/dooa.c:852`
- `src/kar/kar.c:927,932`
- `src/murd/murd.c:527`
- `src/std/std_setup.c:691,709,712,730,746,750`
- `src/std/std_hud.c:1205`
- `src/std/std_effect.c:1365,1393,1503,1589`
- `src/vs/vs_hud.c:1388,1537`
- `src/vs/vs_scene.c:1726,1737,1740,1757,1768,1772,1795,1798`

#### `FISH_SCRATCH` — 313 occurrences on 231 lines

Definition: `src/fish/fish.c:31`, wrapping `getScratchAddr(0)`.

All use lines:

`src/fish/fish.c:1088–1090,1092,1094,1099,1103,1628–1629,1631–1632,1635–1638,1641–1645,1648–1651,1665–1670,1673–1674,1676–1677,1681,1684–1685,1687,1689,1702,1731,1735,2229,2231–2233,2504,2508–2511,2513–2516,2529,2533–2536,2541–2545,2582,2587–2590,2592–2594,2596,2601–2603,2605,2607–2610,2612,2614,2636,2638–2639,2650–2654,2666,2668–2671,2673,2675–2681,2683,2686,3195–3203,3205–3211,3222–3230,3232–3238,3266–3270,3281–3284,3290–3292,3355–3358,3364–3366,3413,3416,3418,3421–3425,3482–3483,3492–3497,3506–3509,3525–3531,3544–3547,3553–3557,3559,3561–3566,3568–3569,3637–3642,3645–3649,3932–3933,3944–3945,3949–3950,4031–4032,4035,4038,4154–4158,4405,4410`.

#### `EFE_SCRATCH` — 290 occurrences on 233 lines

Definition: `include/dw/efe.h:24`, wrapping `getScratchAddr(0)`.

- `src/btl/battle_effect.c:2612–2618,2622–2627,2724–2741,2776–2787,2791,2795–2800,4585–4594,4598,4602–4607,5232,5262–5270`
- `src/std/std_effect.c:2770–2776,2780–2785,2882–2899,2934–2945,2949,2953–2958,4774–4783,4787,4791–4796,5426,5456–5464`
- `src/vs/vs_effect.c:2353–2359,2363–2368,2465–2482,2517–2528,2532,2536–2541,4357–4368,4372,4376–4381,5011,5041–5049`

#### `EFE_SPRITE_SCRATCH` — 126 occurrences on 87 lines

Definition: `include/dw/efe.h:25`, wrapping `getScratchAddr(0)`.

- `src/btl/battle_effect.c:4092–4107,4109–4112,4115–4123`
- `src/std/std_effect.c:4274–4289,4291–4294,4297–4305`
- `src/vs/vs_effect.c:3857–3872,3874–3877,3880–3888`

#### `EFE_PROJ_SCRATCH` — 15 occurrences on 15 lines

Definition: `include/dw/efe.h:26`, wrapping `getScratchAddr(42)`.

- `src/btl/battle_effect.c:2656,2659–2661,2676`
- `src/std/std_effect.c:2814,2817–2819,2834`
- `src/vs/vs_effect.c:2397,2400–2402,2417`

#### `EFE_WAVE_SCRATCH` — 186 occurrences on 120 lines

Definition: `include/dw/efe.h:27`, wrapping `getScratchAddr(34)`.

- `src/btl/battle_effect.c:2610–2611,2619,2621,2628–2642,2644–2655,2657–2658,2663–2666,2681–2683`
- `src/std/std_effect.c:2768–2769,2777,2779,2786–2800,2802–2813,2815–2816,2821–2824,2839–2841`
- `src/vs/vs_effect.c:2351–2352,2360,2362,2369–2383,2385–2396,2398–2399,2404–2407,2422–2424`

#### `EFE_RIBBON_SCRATCH` — 147 occurrences on 111 lines

Definition: `include/dw/efe.h:28`, wrapping `getScratchAddr(42)`.

- `src/btl/battle_effect.c:2569,2571,5231,5240–5244,5246,5248,5250–5251,5253,5255–5260,5273–5278,5283–5285,5287–5290,5295–5296,5302–5303,5313`
- `src/std/std_effect.c:2727,2729,5425,5434–5438,5440,5442,5444–5445,5447,5449–5454,5467–5472,5477–5479,5481–5484,5489–5490,5496–5497,5507`
- `src/vs/vs_effect.c:2310,2312,5010,5019–5023,5025,5027,5029–5030,5032,5034–5039,5052–5057,5062–5064,5066–5069,5074–5075,5081–5082,5092`

#### `EFE_SORT_WORKSPACE` — 9 occurrences on 9 lines

Definition: `include/dw/efe.h:30`, wrapping `getScratchAddr(44)`.

- `src/btl/battle_effect.c:2741,2800,4607`
- `src/std/std_effect.c:2899,2958,4796`
- `src/vs/vs_effect.c:2482,2541,4381`

### Fixed RAM-address macros

These are **22 address-bearing macro definitions**: 19 in US C and three in included game headers. All use sites are listed.

| Macro / value | Definition | All use sites |
|---|---|---|
| `GENERAL_BUFFER` / `0x80010000` | `include/dw/main.h:8` | `src/main/main.c:232`; `src/main/sound.c:526,530,686,703,714,729,742–743`; `src/main/sound_async.c:212,228,241,303`; `src/main/utils2.c:497`; `src/endi/endi.c:37–38`; `src/std/std_setup.c:271,507,624`; `src/vs/vs_scene.c:1181,1817,1837`; `src/vs/vs_select.c:282–287` |
| `TEXTURE_BUFFER` / `0x80088800` | `include/dw/main.h:9` | `src/main/file.c:62`; `src/main/game_menu.c:2383`; `src/main/main.c:223,225,228–229`; `src/main/main_menu.c:648,650`; `src/main/script_common.c:251` |
| `BOSS_EFE_TMD_BUFFER` / `0x80058000` | `include/dw/main.h:10` | `src/main/script_interp.c:1843`; `src/eab/eab.c:121` |
| `BTL_FINISHER_TIM` / `0x80052ae0` | `src/main/btl.c:23` | `src/main/btl.c:431` |
| `BTL_FINISHER_MODEL` / `0x80053800` | `src/main/btl.c:24` | `src/main/btl.c:432` |
| `BTL_CONFUSION_MODEL` / `0x80054838` | `src/main/btl.c:25` | `src/main/btl.c:438` |
| `BTL_STUN_MODEL` / `0x80054d00` | `src/main/btl.c:26` | `src/main/btl.c:443` |
| `BTL_BUFF_MODEL` / `0x80055328` | `src/main/btl.c:27` | `src/main/btl.c:448` |
| `shop_START` / `0x80080800` | `src/main/utils2.c:24` | `src/main/utils2.c:100` |
| `DOOA_MMD_BUFFER` / `0x80020000` | `src/dooa/dooa.c:33` | `src/dooa/dooa.c:1074` |
| `DOOA_SHARD_BUFFER` / `0x80044800` | `src/dooa/dooa.c:34` | `src/dooa/dooa.c:369,1076` |
| `DOOA_ORDERING_TABLE_0` / `0x8008c000` | `src/dooa/dooa.c:36` | `src/dooa/dooa.c:597` |
| `DOOA_ORDERING_TABLE_1` / `0x8008e000` | `src/dooa/dooa.c:37` | `src/dooa/dooa.c:599` |
| `EVL_MMD_BUFFER` / `0x80020000` | `src/evl/evl.c:28` | `src/evl/evl.c:199` |
| `MURD_ORDERING_TABLE_0` / `0x8008c000` | `src/murd/murd.c:28` | `src/murd/murd.c:340,342` |
| `STD_FINISHER_TIM` / `0x80052ae0` | `src/std/std_setup.c:27` | `src/std/std_setup.c:272` |
| `STD_FINISHER_MODEL` / `0x80053800` | `src/std/std_setup.c:28` | `src/std/std_setup.c:272` |
| `STD_CONFUSION_MODEL` / `0x80054838` | `src/std/std_setup.c:29` | `src/std/std_setup.c:274` |
| `STD_STUN_MODEL` / `0x80054d00` | `src/std/std_setup.c:30` | `src/std/std_setup.c:275` |
| `STD_BUFF_MODEL` / `0x80055328` | `src/std/std_setup.c:31` | `src/std/std_setup.c:276` |
| `STD_TMD_BUFFER` / `0x80038000` | `src/std/std_hud.c:29` | `src/std/std_hud.c:1143,1348` |
| `VS_TMD_BUFFER` / `0x80038000` | `src/vs/vs_hud.c:20` | `src/vs/vs_hud.c:1324` |

### Remaining `0x80xxxxxx` literals — exhaustive classification

US C contains **119 matching literal occurrences on 115 lines**:

- **19** fixed-address macro definitions, listed above.
- **13** sign/flag/attribute constants, not pointers.
- **87** initialized-data occurrences, including four explicit pointer casts. The semantics of the other 83 data words are **unverified**.

#### Sign/flag/attribute constants — 13

All are `0x80000000`:

- `src/main/vecmath.c:151,174`
- `src/btl/battle_effect.c:2661,4117,5288`
- `src/fish/fish.c:1689`
- `src/std/std_hud.c:1307`
- `src/std/std_effect.c:2819,4299,5482`
- `src/vs/vs_effect.c:2402,3882,5067`

#### Initialized data — 87 occurrences

| File | Count | All matching lines |
|---|---:|---|
| `src/doo2/doo2_bss_us.c` | 14 | `143,454,524,1427,1440,1629,2483–2486` |
| `src/dooa/dooa_bss_us.c` | 4 | `1015,1020,1032,1042` |
| `src/endi/endi_bss_us.c` | 1 | `145` |
| `src/fish/fish_bss_us.c` | 1 | `1032` |
| `src/kar/kar_bss_us.c` | 7 | `1004,1159,1895,2221,2228,2232,2244` |
| `src/mov/mov_bss_us.c` | 59 | `176,286,302,3985,3989,3990,4010,4024,4031,4051,4052,4055,4057,4058,4385,4937,6984,8007,13725,15247,15249,16977,20081,20388,20396,20404,20419,20490,20812,20822,20826,20840,20842,20845,20852,20859,20865,20868,20871,20873,20876,20878,20881,20887,20894,20909,20939,20949,20954,20968,20990,20993,21035,21219,21347,22124,23984,24698,24699` |
| `src/std/std_bss_us.c` | 1 | `280` |

Explicit pointer initializers:

- `(GsOT_TAG *)0x80040000`: `src/dooa/dooa_bss_us.c:1015,1020`
- `(void *)0x80040000`: `src/dooa/dooa_bss_us.c:1032`
- `(Entity *)0x80820000`: `src/endi/endi_bss_us.c:145`

The last value lies outside ordinary 2 MB PS1 RAM. Whether it is meaningful live state, packed retail data, or a type interpretation artifact is **unverified**. Likewise, address-shaped words in the large initialized blobs must not automatically be treated as pointer fixups.

**Port consequence:** the verified C surface requires new libgs packet/TMD sorting, expanded GTE support beginning with RTPT, SEQ/sticky-VAB/card-directory APIs, and explicit scratchpad/fixed-address handling. DW2-PC provides useful lower-level implementations, but matching names and linked SDK symbols alone do not establish behavioral compatibility.