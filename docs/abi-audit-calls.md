# Phase 2 slice 1: unprototyped calls and missing returns

2026-10-13. gpt-6.1-sol (high) read-only audit of the 958 calls through `()` declarations and
the 113 functions without a return on some path (`docs/phase1-report.md`, "Warnings"), against
clang ASTs of the 119 native files. Claude checked the actionable cases against the code. Its
report follows unchanged below the action list.

## Actions

| Finding | Status |
|---|---|
| `src/shop/script_menu.c:3005`: `strlen((int32_t)ptr + 1)` truncates a pointer | **Fixed** in the fork under `DW1_NATIVE` |
| 8 `renderString` calls in `src/main/game_menu.c` omit the 9th argument (`shadow`), which the callee reads; garbage on the PS1 too | **Open, Phase 4:** capture what the PS1 passes at each site (the stack slot's value) and pass it explicitly under `DW1_NATIVE` |
| 16 missing-return functions whose callers use the value: `BTL/STD/VS_startEFE` (stored as `efeSubEffect`), `getEntityFromScriptId`, `STD_getEntityIndex(2)`, `VS_getFirstSpecialMove`, `flipTextboxPage`; the rest can't reach the missing path or discard the result | **Open:** for each, capture the PS1's `v0` on the fall-through path when its scene runs (Phase 4: `flipTextboxPage`, `getEntityFromScriptId`; Phase 5: the rest) and return it explicitly under `DW1_NATIVE` |
| Surplus arguments (10 calls) and `exit(1)` into the zero-argument stub | Harmless natively (the callee ignores them); left as is |

---

## The audit

- **958 named calls to 32 functions:** **932 OK, 26 WRONG** under the requested argument-count rule.
  - **1** integer-to-pointer mismatch with pointer truncation.
  - **8** calls missing a parameter that the callee reads.
  - **17** calls passing surplus arguments. The current native callees ignore those arguments.
- **113 missing-return functions:** **16 have consumed results**: 15 through direct C calls, plus one through a callback. No result consumer was found for the other **97**.
- All 958 named warning sites and 113 return warnings were resolved against Clang ASTs from the **119 native US translation units**. No files were changed.

Paths below are relative to `decomp/`, except paths beginning with `crates/`.

**Two corrections to the supplied assumptions matter:**

1. **K&R definitions receive promoted parameters.** Apple Clang emits, for example:

   ```llvm
   define void @startAnimation(ptr, i32)
   define void @drawString(ptr, i32, i32)
   define i32 @tickEntityMoveTo(i32, i32, i32, i32, i32, i32)
   define i32 @removeObject(i32, i32)
   ```

   These callees then truncate their narrow source parameters internally. Thus signedness changes or out-of-range narrow values do **not** create the described caller-extension hazard for these definitions. This distinction affects **680** warning-listed calls.

2. **`dw1_exit` is currently a zero-argument trapping stub**, instantiated at `crates/psyq/src/stubs.rs:11` by `crates/psyq/src/trap.rs:24`. It is not implemented in `libc.rs`. Its seven `exit(1)` calls therefore count as argument-count mismatches against the current implementation, although the stub ignores the argument and aborts.

## A. Complete WRONG list

### Integer passed as a pointer: 1 call

| Location | Quoted code | Argument and reason |
|---|---|---|
| [src/shop/script_menu.c:3005](../decomp/src/shop/script_menu.c:3005) | `len = strlen((int32_t)MAIN_D_801345F4 + 1);` | `dw1_strlen` argument **1** is an `int32_t`, but Rust expects `const char *`. The cast first truncates the pointer to 32 bits; the unprototyped call then passes an integer rather than a native pointer. Casting that integer back to a pointer afterward would not recover the lost bits. |

### Missing argument: 8 calls

Every following call supplies **8 arguments** to `renderString`, whose definition has **9**. Argument **9**, `int32_t shadow`, is missing. The callee reads it in `if (shadow != 0)` at `src/main/utils2.c:260`.

| Location | Quoted code |
|---|---|
| [src/main/game_menu.c:2398](../decomp/src/main/game_menu.c:2398) | `renderString(0, 0x6b, 0x4c, 0xc, 0xc, 0xd8, 0xc, 0);` |
| [src/main/game_menu.c:3633](../decomp/src/main/game_menu.c:3633) | `renderString(3, -0x14, -0x4f, 0x24, 0xc, 0, 0x18, 4);` |
| [src/main/game_menu.c:3636](../decomp/src/main/game_menu.c:3636) | `renderString(0, -0x5b, 0x3a, 0x2e, 0xc, 0x24, 0x18, 4);` |
| [src/main/game_menu.c:3639](../decomp/src/main/game_menu.c:3639) | `renderString(0, -0x63, 0x3a, 0x3e, 0xc, 0x52, 0x18, 4);` |
| [src/main/game_menu.c:3642](../decomp/src/main/game_menu.c:3642) | `renderString(0, -0x5b, 0x3a, 0x2f, 0xc, 0, 0x24, 4);` |
| [src/main/game_menu.c:3645](../decomp/src/main/game_menu.c:3645) | `renderString(0, -0x64, 0x3a, 0x41, 0xc, 0x2f, 0x24, 4);` |
| [src/main/game_menu.c:3648](../decomp/src/main/game_menu.c:3648) | `renderString(0, -0x64, 0x3a, 0x41, 0xc, 0, 0x3c, 4);` |
| [src/main/game_menu.c:3655](../decomp/src/main/game_menu.c:3655) | `renderString(0, -0x5c - ((len - 4) * 6), 0x4d, 0x78, 0xc, 0, 0x30, 4);` |

These missing arguments were unspecified on PS1 as well; pointer size alone does not make them harmless there.

### Surplus arguments to game functions: 10 calls

These are **WRONG by the requested count-mismatch criterion**, but their surplus arguments do not shift the arguments that the current arm64 callees actually consume.

| Location | Quoted code | Argument and reason |
|---|---|---|
| `src/main/battle_ui.c:423` | `BTL_tickBattleEndText(instanceId);` | Argument **1** is surplus; definition is `void BTL_tickBattleEndText(void)`. |
| `src/main/tamer.c:456` | `tamerTickWalkingState(instanceId);` | Argument **1** is surplus; definition is `void tamerTickWalkingState(void)`. |
| `src/std/std_setup.c:266` | `STD_initializeLighting(arena);` | Argument **1** is surplus; definition is `void STD_initializeLighting(void)`. |
| `src/trn/trn_brain.c:171` | `removeObject(0xfb0, instanceId, TRN_tickBrainsTraining, NULL);` | Arguments **3–4** are surplus; `removeObject` takes two parameters. |
| `src/trn/trn_def.c:280` | `removeObject(0xfae, instanceId, TRN_tickDefenseTraining, NULL);` | Arguments **3–4** are surplus. |
| `src/trn/trn_hp.c:169` | `removeObject(0xfab, instanceId, TRN_tickHpTraining, NULL);` | Arguments **3–4** are surplus. |
| `src/trn/trn_mp.c:141` | `removeObject(0xfaf, instanceId, TRN_tickMpTraining, NULL);` | Arguments **3–4** are surplus. |
| `src/trn/trn_off.c:198` | `removeObject(0xfac, instanceId, TRN_tickOffenseTraining, NULL);` | Arguments **3–4** are surplus. |
| `src/trn/trn_speed.c:139` | `removeObject(0xfad, instanceId, TRN_tickSpeedTraining, NULL);` | Arguments **3–4** are surplus. |
| `src/trn2/trn2_mp.c:127` | `removeObject(0xfaf, instanceId, TRN_tickMpTraining, NULL);` | Arguments **3–4** are surplus. The cross-overlay function reference is already documented beside this call; it is never invoked by `removeObject`. |

### Surplus arguments to the current `dw1_exit` stub: 7 calls

Each call below passes argument **1**, status `1`, to the current zero-argument Rust stub.

| Location | Quoted code |
|---|---|
| `src/btl/battle_effect.c:5330` | `exit(1);` |
| `src/fish/fish_model.c:24` | `exit(1);` |
| `src/fish/fish_model.c:34` | `exit(1);` |
| `src/fish/fish_model.c:40` | `exit(1);` |
| `src/fish/fish_model.c:55` | `exit(1);` |
| `src/std/std_effect.c:5524` | `exit(1);` |
| `src/vs/vs_effect.c:5109` | `exit(1);` |

If audited against the intended Psy-Q `exit(int)` interface instead of the present Rust stub, these seven calls are **OK**: the corresponding totals would be **939 OK / 19 WRONG**.

## B. Missing-return functions with consumed results

**16 functions**, covering **87 direct C consumption sites** and one indirect callback-consumption site.

The table distinguishes a consumed result from a caller actually reaching the missing-return path. PS1 return guesses below are source-level guesses, not verified retail register values.

| Function and definition | Caller locations and use | Missing path and likely PS1 result |
|---|---|---|
| `loadStackedTIMEntry`, `src/main/file.c:116` | `src/main/game_menu.c:2383`: `return loadStackedTIMEntry(...);` | Falls through unconditionally. **Guess:** residue of the last `LoadImage` call. The wrapper `loadCardImage` forwards it, although its game caller at `game_menu.c:2078` discards the wrapper’s result. |
| `flipTextboxPage`, `src/main/script_common.c:2536` | `src/main/script_common.c:1961`: returned by `isItemMenuBoxBusy`; `src/shop/script_menu.c:2779`: compared with zero | Successful page-flip path falls through after updating counters and clearing `pageReady`. No reliable source-level `v0` guess. |
| `BTL_startEFE`, `src/btl/battle_effect.c:2181` | `src/btl/battle_main.c:2572`: assigned to `digimon->stats.current.efeSubEffect` | Successful start falls through after `BTL_runEFESlotScript(i)`. **Guess:** residue from that call. Failure paths explicitly return `-1`. |
| `STD_startEFE`, `src/std/std_effect.c:2294` | `src/std/std_main.c:6773`: assigned to `efeSubEffect` | Same pattern; **guess:** residue from `STD_runEFESlotScript(i)`. |
| `VS_startEFE`, `src/vs/vs_effect.c:1925` | `src/vs/vs_main.c:955`: assigned to `efeSubEffect` | Same pattern; **guess:** residue from `VS_runEFESlotScript(i)`. |
| `getEntityFromScriptId`, `src/main/tamer.c:1249` | `src/main/overworld.c:997, 1146, 1825`; `src/main/tamer.c:1010, 1023, 1085, 1090, 1109, 1131, 1139, 1207`: stored as an `Entity *` and subsequently used | Unrecognized script ID falls through. The source does not specify a null or sentinel result; no reliable `v0` guess. |
| `STD_getEntityIndex`, `src/std/std_main.c:1582` | `src/std/std_main.c:1603, 1912, 1944`: assigned to entity-index variables | Entity absent from `ENTITY_TABLE[0..9]` falls through. **Guess:** exhausted loop index `10`, depending on register allocation. |
| `STD_getEntityIndex2`, `src/std/std_main.c:5482` | `src/std/std_main.c:4172`: assigned to `COMBAT_DATA_PTR->player.entityIds[1]` | Same missing-entity path; **guess:** `10`. |
| `VS_getFirstSpecialMove`, `src/vs/vs_scene.c:1545` | `src/vs/vs_scene.c:1244, 1260`: assigned to `stats->base.moves[3]` | No qualifying move falls through. **Guess:** possibly the exhausted loop index `-1`; not guaranteed. |
| `getItemOptionColor`, `src/main/inventory.c:209` | `src/main/inventory.c:733`: passed as `renderString`’s color | Missing return requires a mode outside `0..3`. The caller’s loop supplies exactly `0..3`, so this caller does not reach the missing path. |
| `BTL_calculateElementBonus`, `src/btl/battle_main.c:3570` | `src/btl/battle_main.c:3500, 3503`: added to move-selection weights | Missing return requires an unmatched `TYPE_FACTORS` value. The initialized table at `src/main/item.c:94` contains only `20, 15, 10, 5, 2`, all covered by explicit returns. |
| `STD_calculateElementBonus`, `src/std/std_main.c:7360` | `src/std/std_main.c:6494, 6497`: added to weights | Same covered-table condition. |
| `VS_calculateElementBonus`, `src/vs/vs_main.c:1688` | `src/vs/vs_main.c:1618, 1621`: added to weights | Same covered-table condition. |
| `playSound`, `src/main/sound.c:641` | `src/endi/endi.c:430`: `return playSound(8, 0);` | This use passes VAB ID `8`, which takes an explicit `return startSound(...)` path. It does not consume a missing return. |
| `loadSoundCompleteCallback`, `src/main/sound_async.c:128` | **Indirect:** `src/main/file_queue.c:170`: `req->waitCallback(req->waitCallbackParam) == 0` | Registered at `sound_async.c:102` through `setFileReadCallback2`, then transferred into `req->waitCallback` at `file_queue.c:162`. Missing return requires a state outside `0, 4, 5`; all assignments to `LOAD_SOUND_COMPLETE_STATE` establish those three states, whose cases explicitly return. |
| `_sin`, `src/main/math.c:13` | All locations below consume its result | `input &= 0x1FF` restricts input to `0..511`; the four returning branches cover that entire range. The final closing brace is unreachable. |

All **56 direct C uses of `_sin`**:

| File | Lines |
|---|---|
| `src/main/bubble.c` | 644 |
| `src/main/butterfly.c` | 120, 124 |
| `src/main/efe.c` | 524, 588, 944, 951 |
| `src/main/math.c` | 29 |
| `src/main/particle.c` | 149 |
| `src/btl/battle_effect.c` | 2504, 3824, 3863, 3864, 4080, 4540, 4541, 5419, 5524, 5900 |
| `src/btl/battle_main.c` | 2717, 2718, 2723, 2724 |
| `src/dooa/dooa.c` | 522 |
| `src/eab/eab.c` | 271, 328, 408, 415 |
| `src/std/std_main.c` | 7042, 7043, 7048, 7049 |
| `src/std/std_effect.c` | 2672, 4006, 4045, 4046, 4262, 4729, 4730, 5613, 5718, 6094 |
| `src/vs/vs_effect.c` | 2255, 3589, 3628, 3629, 3845, 4312, 4313, 5198, 5303, 5678 |
| `src/vs/vs_main.c` | 1123, 1124, 1129, 1130 |

There is also a Rust test consumer at `crates/decomp-c/src/lib.rs:84`.

The indirect-dispatch checks matter:

- The **72** `BTL_*`, `STD_*`, and `VS_*` variable-operator functions have their results discarded by dispatch calls at `battle_effect.c:4883, 4885`, `std_effect.c:5072, 5074`, and `vs_effect.c:4657, 4659`.
- `loadSoundFinishCallback` is invoked as a finish callback at `file_queue.c:156`, where its result is discarded. Its registration of `loadSoundCompleteCallback` creates the consumed callback case listed above.
- Function addresses in the `void *` ordering arrays are not themselves return-value uses.

Falling off a non-void function when its result is consumed is C undefined behavior. Native behavior therefore need not be limited to whatever happens to remain in `x0`.

## C. Minimal fixes preserving the decomp build

These are proposed changes only. `DW1_NATIVE` guards retain the original non-native source branches. No byte-identical PS1 rebuild was performed.

### 1. Fix the truncated-pointer `strlen` call

At `src/shop/script_menu.c:3005`:

```c
#ifdef DW1_NATIVE
        len = strlen(NAMING_PROMPT_DIGIMON);
#else
        len = strlen((int32_t)MAIN_D_801345F4 + 1);
#endif
```

For US, `NAMING_PROMPT_DIGIMON` is already defined as `(MAIN_D_801345F4 + 1)` at line 569. This preserves pointer arithmetic without passing through a 32-bit integer.

A native full prototype for `dw1_strlen` is also appropriate, but **a prototype alone cannot repair the existing pointer truncation**:

```c
int dw1_strlen(const char *src);
```

### 2. Supply `renderString`’s missing `shadow` parameter

Replace the empty declaration at `src/main/game_menu.c:182` conditionally:

```c
#ifdef DW1_NATIVE
void renderString(uint8_t color, int16_t x, int16_t y,
                  int16_t w, int16_t h, uint8_t u, uint8_t v,
                  long layer, int32_t shadow);
#else
void renderString();
#endif
```

At each of the eight listed sites, preserve the original call in the non-native branch and supply an explicit ninth argument natively. For example:

```c
#ifdef DW1_NATIVE
    renderString(0, 0x6b, 0x4c, 0xc, 0xc, 0xd8, 0xc, 0, 0);
#else
    renderString(0, 0x6b, 0x4c, 0xc, 0xc, 0xd8, 0xc, 0);
#endif
```

**The final `0` is a proposed “no shadow” choice.** The missing argument in the source does not establish the retail shadow behavior; use `1` instead if retail evidence establishes shadows at a particular site.

The full prototype also makes Clang perform the required narrow-parameter conversions for this prototype-style definition.

### 3. Remove surplus arguments natively

| Sites | Native call |
|---|---|
| `battle_ui.c:423` | `BTL_tickBattleEndText();` |
| `tamer.c:456` | `tamerTickWalkingState();` |
| `std_setup.c:266` | `STD_initializeLighting();` |
| All seven listed four-argument `removeObject` calls | Keep only the original object ID and `instanceId`. |

Example:

```c
#ifdef DW1_NATIVE
            removeObject(0xfb0, instanceId);
#else
            removeObject(0xfb0, instanceId, TRN_tickBrainsTraining, NULL);
#endif
```

Use native `(void)` declarations for the three zero-parameter functions.

For `removeObject`, a prototype compatible with its **existing K&R definition** is:

```c
#ifdef DW1_NATIVE
int32_t removeObject(int objectId, int instanceId);
#else
int32_t removeObject(/* int32_t objectId, int32_t instanceId */);
#endif
```

The promoted `int` parameters are deliberate. The K&R body performs the `int16_t` normalization. A native prototype using `int16_t` would require changing the native definition to prototype style too. Clang accepted the promoted-type prototype against the existing K&R definition in a read-only syntax check.

### 4. Make the native exit stub accept the intended status argument

For all seven `exit(1)` sites, the cleanest fix is to correct the Rust stub’s signature while leaving the decomp calls untouched.

Remove `dw1_exit` from the zero-argument stub invocation and provide:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn dw1_exit(_status: core::ffi::c_int) {
    crate::trap::unimplemented("dw1_exit");
}
```

Optionally add `void dw1_exit(int status);` to the native header. This preserves the present trapping behavior, matches the intended C interface, and does not change the PS1 decomp build.

## A. Definition, declaration, and count inventory

**K** means K&R definition: narrow source parameters enter as promoted `int` values and are converted inside the function. Parameter types below are the types written in the definition. `long` becomes `int` under the forced native header.

The OK counts classify every warning-listed call not present in the WRONG list.

| Function | Definition: location and parameters | Empty declaration(s) | OK / WRONG |
|---|---|---|---:|
| `startAnimation` **K** | `src/main/anim.c:201`: `Entity *, uint8_t` | `include/dw/anim.h:9` | 224 / 0 |
| `drawString` **K** | `src/main/font.c:1728`: `char *, uint16_t, uint16_t` | `include/dw/font.h:26` | 208 / 0 |
| `removeObject` **K** | `src/main/world_object.c:51`: `int16_t, int16_t` | `include/dw/world_object.h:23` | 177 / 7 |
| `renderString` | `src/main/utils2.c:247`: `uint8_t, int16_t, int16_t, int16_t, int16_t, uint8_t, uint8_t, long, int32_t` | `src/main/game_menu.c:182` | 64 / 8 |
| `dw1_memcpy` | `crates/psyq/src/libc.rs:26`: `void *, const void *, int` | `external/psyq_headers/mw_lib41/include/memory.h:25`, as `memcpy` | 49 / 0 |
| `dw1_strcpy` | `crates/psyq/src/libc.rs:88`: `char *, const char *` | `external/psyq_headers/mw_lib41/include/strings.h:32`, as `strcpy` | 48 / 0 |
| `dw1_strlen` | `crates/psyq/src/libc.rs:71`: `const char *` | `external/psyq_headers/mw_lib41/include/strings.h:34`, as `strlen` | 46 / 1 |
| `tickEntityWalkTo` **K** | `src/main/tamer.c:991`: `uint8_t, uint8_t, int16_t, int16_t, int8_t` | Expanded declaration list below | 34 / 0 |
| `setEntityRotation` **K** | `src/main/model.c:2986`: `int32_t, int16_t, int16_t, int16_t` | `include/dw/model.h:46` | 15 / 0 |
| `moveMapObjectsWithLimit` | `src/main/overworld.c:844`: `uint8_t, int16_t, int16_t, int16_t, int16_t, int16_t` | `src/trn/trn_hp.c:21`; `src/trn2/trn2_hp_map99.c:18`; `src/trn2/trn2_hp_map107.c:21` | 12 / 0 |
| `loadMapObjectPosition` | `src/main/overworld.c:832`: `int16_t *, int16_t *, int16_t, int16_t` | Expanded declaration list below | 10 / 0 |
| `storeMapObjectPosition` | `src/main/overworld.c:790`: `int16_t *, int16_t *, uint8_t, int16_t` | Expanded declaration list below | 8 / 0 |
| `dw1_exit` | `crates/psyq/src/stubs.rs:11`, expanded by `crates/psyq/src/trap.rs:24`: **no parameters** | `external/psyq_headers/mw_lib41/include/stdlib.h:23`, as `exit` | 0 / 7 |
| `uploadMapTileImages` | `src/main/map.c:4398`: `MapTileData *, int16_t` | `src/kar/kar.c:113` | 5 / 0 |
| `setInt16WithStride` | `src/main/effect_common.c:567`: `int16_t *, int16_t, int16_t, int16_t` | `src/main/efe.c:134` | 4 / 0 |
| `tickEntityMoveTo` **K** | `src/main/tamer.c:1114`: `uint8_t, uint8_t, int16_t, int32_t, int8_t, int8_t` | `include/dw/tamer.h:27` | 4 / 0 |
| `createParticleFX` | `src/main/efe.c:370`: `int32_t, int32_t, SVECTOR *, Entity *, int32_t` | `src/std/std_main.c:100`; `src/vs/vs_main.c:47` | 4 / 0 |
| `convertAsciiToJis` **K** | `src/main/sjis.c:63`: `uint8_t` | `include/dw/sjis.h:8` | 3 / 0 |
| `addWithLimit` | `src/main/item.c:2929`: `int16_t *, int16_t, int16_t` | `src/btl/battle_main.c:139`; `src/std/std_main.c:110`; `src/vs/vs_main.c:91` | 3 / 0 |
| `drawGlyph` **K** | `src/main/font.c:1549`: `uint16_t, uint16_t, uint16_t` | `include/dw/font.h:24` | 2 / 0 |
| `tickEntityMoveToAxis` **K** | `src/main/tamer.c:1192`: `uint8_t, int32_t, int8_t, int8_t, int8_t` | `include/dw/tamer.h:30` | 2 / 0 |
| `setMapObjectsFlag` | `src/main/overworld.c:930`: `int16_t, int16_t, int8_t` | `src/trn2/trn2_hp_map99.c:19` | 2 / 0 |
| `VS_addTargetCursor` **K** | `src/vs/vs_hud.c:610`: `int16_t, int32_t` | `include/dw/vs.h:181` | 2 / 0 |
| `BTL_tickBattleEndText` | `src/btl/battle_hud.c:482`: `void` | `src/main/battle_ui.c:21` | 0 / 1 |
| `loadMapSounds2` | `src/main/sound_async.c:233`: `int32_t` | `src/main/evl.c:16` | 1 / 0 |
| `isSoundLoaded` | `src/main/sound_async.c:248`: `int32_t, int32_t` | `src/main/evl.c:17` | 1 / 0 |
| `loadVLALL` | `src/main/sound_async.c:269`: `int32_t, uint8_t *` | `src/main/evl.c:18` | 1 / 0 |
| `dw1_strcmp` | `crates/psyq/src/libc.rs:156`: `const char *, const char *` | `external/psyq_headers/mw_lib41/include/strings.h:30`, as `strcmp` | 1 / 0 |
| `tamerTickWalkingState` | `src/main/tamer.c:510`: `void` | `src/main/tamer.c:245, 275` | 0 / 1 |
| `STD_initializeLighting` | `src/std/std_setup.c:474`: `void` | `src/std/std_setup.c:71` | 0 / 1 |
| `VS_selectMoveByPower` **K** | `src/vs/vs_main.c:1757`: `int32_t, int16_t *` | `include/dw/vs.h:214` | 1 / 0 |
| `VS_selectMoveByMpCost` **K** | `src/vs/vs_main.c:1778`: `int32_t, int16_t *` | `include/dw/vs.h:213` | 1 / 0 |
| **Total** | | | **932 / 26** |

The commented parameter lists in headers are still empty declarations after preprocessing. `drawGlyph` also has empty declarations at `src/main/script_textbox.c:651, 655`, but both are excluded from the native US build by their version/compiler guards.

### Expanded empty-declaration locations

| Function | Locations |
|---|---|
| `tickEntityWalkTo` | `src/main/partner.c:69`; `src/main/tamer.c:195`; `src/trn/trn_brain.c:22`; `src/trn/trn_def.c:27`; `src/trn/trn_hp.c:23`; `src/trn/trn_mp.c:25`; `src/trn/trn_off.c:28`; `src/trn/trn_speed.c:19`; `src/trn2/trn2_def_map108.c:25`; `src/trn2/trn2_def_map99.c:25`; `src/trn2/trn2_hp_map107.c:23`; `src/trn2/trn2_hp_map99.c:21`; `src/trn2/trn2_mp.c:19`; `src/trn2/trn2_off.c:26`; `src/trn2/trn2_reward.c:25` |
| `storeMapObjectPosition` | `src/trn/trn_def.c:19`; `src/trn/trn_hp.c:19`; `src/trn/trn_mp.c:19`; `src/trn/trn_off.c:18`; `src/trn2/trn2_def_map99.c:16`; `src/trn2/trn2_hp_map107.c:19`; `src/trn2/trn2_hp_map99.c:16`; `src/trn2/trn2_off.c:17` |
| `loadMapObjectPosition` | `src/trn/trn_def.c:20`; `src/trn/trn_hp.c:20`; `src/trn/trn_mp.c:20`; `src/trn/trn_off.c:19`; `src/trn2/trn2_def_map108.c:18`; `src/trn2/trn2_def_map99.c:17`; `src/trn2/trn2_hp_map107.c:20`; `src/trn2/trn2_hp_map99.c:17` |

### Why the remaining calls are OK

- **K&R functions:** all supplied arguments match the promoted entry-point widths/classes. In particular, signed animation IDs, `int32_t` rotations or coordinates, unsigned movement speeds, and negative text coordinates are narrowed inside these callees.
- **Renamed libc:** all 49 `memcpy` calls pass two pointers and a 32-bit length; signed/unsigned 32-bit length differences do not change width/class. All 48 `strcpy` calls, the one `strcmp` call, and the other 46 `strlen` calls pass pointers.
- **Prototype-style narrow parameters:** constants fit, typed values fit, or caller bounds establish fitting values:
  - `setInt16WithStride` uses fitting constants; `addWithLimit` receives fitting `int16_t` values and limits.
  - Training start indices are initialized to fitting values by the TRN/TRN2 setup functions before the listed `storeMapObjectPosition` and `moveMapObjectsWithLimit` calls.
  - The five `uploadMapTileImages` index expressions use bounded tile coordinates and signed-byte map dimensions.
  - `renderString` tab colors are initialized to `0/1`; its loops, fixed sprite tables, panel values `0/1`, sleep-cycle guard, and tamer-level range keep narrow arguments representable.
  - The move-selection range expression at `game_menu.c:3153` selects ordinary move IDs, whose ranges are `1..4`. The sole zero-range table entry is ID `110`, outside that selector. The expression at line 2783 also has an explicit `range != 0` guard.
- No named warning-listed call passes a floating-point value, struct by value, or a 64-bit integer argument. The sole pointer/integer class mismatch is the listed `strlen` call.

The six unnamed function-pointer warnings are outside the requested 958 named-call inventory; their EFE dispatchers were nevertheless traced when checking return-value consumption.