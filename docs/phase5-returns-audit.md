# Phase 5: missing returns in the game C (audit)

2026-10-14. gpt-6.1-sol (high), read-only, over the 110 `-Wreturn-type` warnings of the native build (taken before `*_startEFE` was fixed; see docs/phase5-plan.md). Not yet acted on except `*_startEFE`; the USED-* entries are the next slice. Claude has not yet re-checked every entry.


Paths below are relative to `decomp/src`. Caller searches exclude `*_eu.c` and `*_jp*.c`. References inside conditional compilation branches of other files are retained and identified where relevant.

**All 110 warnings concern falling off the end of a non-void function. None concerns a bare `return;`.**

“UNUSED” means the missing result is harmless to the inspected callers. It does not assess unrelated problems in those functions.

“USED-DETERMINED” fixes sometimes require a native-only local or extra argument. Those dependencies are specified below; `nativeFallback` is not an existing variable.

## Summary table

| File:line | Function | Class | Native return expression / action |
| --- | --- | --- | --- |
| btl/battle_effect.c:1875 | BTL_setupLoadedEFEFile | USED-UNCLEAR | See loader paths below |
| btl/battle_effect.c:2138 | BTL_offsetEFEPrimitiveUVs | UNUSED | — |
| btl/battle_effect.c:5037 | BTL_shiftRightInt32Variable | UNUSED | — |
| btl/battle_effect.c:5042 | BTL_shiftLeftInt32Variable | UNUSED | — |
| btl/battle_effect.c:5047 | BTL_moduloInt32Variable | UNUSED | — |
| btl/battle_effect.c:5052 | BTL_divideInt32Variable | UNUSED | — |
| btl/battle_effect.c:5057 | BTL_multiplyInt32Variable | UNUSED | — |
| btl/battle_effect.c:5062 | BTL_subtractInt32Variable | UNUSED | — |
| btl/battle_effect.c:5067 | BTL_addInt32Variable | UNUSED | — |
| btl/battle_effect.c:5072 | BTL_setInt32Variable | UNUSED | — |
| btl/battle_effect.c:5077 | BTL_shiftRightInt8Variable | UNUSED | — |
| btl/battle_effect.c:5082 | BTL_shiftLeftInt8Variable | UNUSED | — |
| btl/battle_effect.c:5087 | BTL_moduloInt8Variable | UNUSED | — |
| btl/battle_effect.c:5092 | BTL_divideInt8Variable | UNUSED | — |
| btl/battle_effect.c:5097 | BTL_multiplyInt8Variable | UNUSED | — |
| btl/battle_effect.c:5102 | BTL_subtractInt8Variable | UNUSED | — |
| btl/battle_effect.c:5107 | BTL_addInt8Variable | UNUSED | — |
| btl/battle_effect.c:5112 | BTL_setInt8Variable | UNUSED | — |
| btl/battle_effect.c:5117 | BTL_shiftRightInt16Variable | UNUSED | — |
| btl/battle_effect.c:5122 | BTL_shiftLeftInt16Variable | UNUSED | — |
| btl/battle_effect.c:5127 | BTL_moduloInt16Variable | UNUSED | — |
| btl/battle_effect.c:5132 | BTL_divideInt16Variable | UNUSED | — |
| btl/battle_effect.c:5137 | BTL_multiplyInt16Variable | UNUSED | — |
| btl/battle_effect.c:5142 | BTL_subtractInt16Variable | UNUSED | — |
| btl/battle_effect.c:5147 | BTL_addInt16Variable | UNUSED | — |
| btl/battle_effect.c:5152 | BTL_setInt16Variable | UNUSED | — |
| btl/battle_effect.c:5226 | BTL_calculateAttackHitPosition | UNUSED | — |
| btl/battle_main.c:3584 | BTL_calculateElementBonus | USED-DETERMINED | `(int16_t)nativeFallback`; caller supplies PS1 data address |
| dooa/dooa.c:1423 | DOOA_updateShards | UNUSED | — |
| dooa/dooa.c:1557 | DOOA_renderShards | UNUSED | — |
| fish/fish.c:1385 | FISH_openBaitMenu | UNUSED | — |
| fish/fish.c:1541 | FISH_showTextBox | UNUSED | — |
| kar/kar.c:1813 | KAR_computeSeparation | UNUSED | — |
| kar/kar.c:1913 | KAR_findWallContact | UNUSED | — |
| kar/kar.c:2445 | KAR_chooseOpponentShot | UNUSED | — |
| main/efe.c:977 | setEFEFlashOffset | UNUSED | — |
| main/effect_common.c:502 | addScreenPolyFT4 | UNUSED | — |
| main/file.c:78 | loadTextureFile | UNUSED | — |
| main/file.c:96 | readFile | UNUSED | — |
| main/file.c:114 | loadTIMFile | UNUSED | — |
| main/file.c:132 | loadStackedTIMEntry | USED-DETERMINED | `lastLoadImageResult`; capture existing calls |
| main/file.c:151 | readFileSectors | UNUSED | — |
| main/inventory.c:253 | getItemOptionColor | USED-UNCLEAR | Incoming `v0` for unsupported mode |
| main/math.c:25 | _sin | USED-DETERMINED | `0`, solely as unreachable warning suppression |
| main/script_common.c:2561 | flipTextboxPage | USED-UNCLEAR | Incoming `v0` |
| main/script_interp.c:2397 | popScriptStack | UNUSED | — |
| main/script_interp.c:2511 | setCardAmount | UNUSED | — |
| main/sound.c:648 | playSound | USED-UNCLEAR | Incoming `v0` for unsupported VAB ID |
| main/sound_async.c:103 | loadSoundFinishCallback | UNUSED | — |
| main/sound_async.c:168 | loadSoundCompleteCallback | USED-DETERMINED | `(int32_t)0x800E8CDCu` |
| main/tamer.c:1268 | getEntityFromScriptId | USED-UNCLEAR | Incoming `v0` when no entity matches |
| std/std_effect.c:1980 | STD_setupLoadedEFEFile | USED-UNCLEAR | See loader paths below |
| std/std_effect.c:2243 | STD_offsetEFEPrimitiveUVs | UNUSED | — |
| std/std_effect.c:5226 | STD_shiftRightInt32Variable | UNUSED | — |
| std/std_effect.c:5231 | STD_shiftLeftInt32Variable | UNUSED | — |
| std/std_effect.c:5236 | STD_moduloInt32Variable | UNUSED | — |
| std/std_effect.c:5241 | STD_divideInt32Variable | UNUSED | — |
| std/std_effect.c:5246 | STD_multiplyInt32Variable | UNUSED | — |
| std/std_effect.c:5251 | STD_subtractInt32Variable | UNUSED | — |
| std/std_effect.c:5256 | STD_addInt32Variable | UNUSED | — |
| std/std_effect.c:5261 | STD_setInt32Variable | UNUSED | — |
| std/std_effect.c:5266 | STD_shiftRightInt8Variable | UNUSED | — |
| std/std_effect.c:5271 | STD_shiftLeftInt8Variable | UNUSED | — |
| std/std_effect.c:5276 | STD_moduloInt8Variable | UNUSED | — |
| std/std_effect.c:5281 | STD_divideInt8Variable | UNUSED | — |
| std/std_effect.c:5286 | STD_multiplyInt8Variable | UNUSED | — |
| std/std_effect.c:5291 | STD_subtractInt8Variable | UNUSED | — |
| std/std_effect.c:5296 | STD_addInt8Variable | UNUSED | — |
| std/std_effect.c:5301 | STD_setInt8Variable | UNUSED | — |
| std/std_effect.c:5306 | STD_shiftRightInt16Variable | UNUSED | — |
| std/std_effect.c:5311 | STD_shiftLeftInt16Variable | UNUSED | — |
| std/std_effect.c:5316 | STD_moduloInt16Variable | UNUSED | — |
| std/std_effect.c:5321 | STD_divideInt16Variable | UNUSED | — |
| std/std_effect.c:5326 | STD_multiplyInt16Variable | UNUSED | — |
| std/std_effect.c:5331 | STD_subtractInt16Variable | UNUSED | — |
| std/std_effect.c:5336 | STD_addInt16Variable | UNUSED | — |
| std/std_effect.c:5341 | STD_setInt16Variable | UNUSED | — |
| std/std_effect.c:5415 | STD_calculateAttackHitPosition | UNUSED | — |
| std/std_main.c:1591 | STD_getEntityIndex | USED-DETERMINED | `nativeFallback`; caller supplies placement X or camera Y |
| std/std_main.c:5491 | STD_getEntityIndex2 | USED-DETERMINED | `(int16_t)0xD640`; preserves sole caller’s stored byte |
| std/std_main.c:7374 | STD_calculateElementBonus | USED-DETERMINED | `(int16_t)nativeFallback`; caller supplies PS1 data address |
| vs/vs_effect.c:1611 | VS_setupLoadedEFEFile | USED-UNCLEAR | See loader paths below |
| vs/vs_effect.c:1874 | VS_offsetEFEPrimitiveUVs | UNUSED | — |
| vs/vs_effect.c:4811 | VS_shiftRightInt32Variable | UNUSED | — |
| vs/vs_effect.c:4816 | VS_shiftLeftInt32Variable | UNUSED | — |
| vs/vs_effect.c:4821 | VS_moduloInt32Variable | UNUSED | — |
| vs/vs_effect.c:4826 | VS_divideInt32Variable | UNUSED | — |
| vs/vs_effect.c:4831 | VS_multiplyInt32Variable | UNUSED | — |
| vs/vs_effect.c:4836 | VS_subtractInt32Variable | UNUSED | — |
| vs/vs_effect.c:4841 | VS_addInt32Variable | UNUSED | — |
| vs/vs_effect.c:4846 | VS_setInt32Variable | UNUSED | — |
| vs/vs_effect.c:4851 | VS_shiftRightInt8Variable | UNUSED | — |
| vs/vs_effect.c:4856 | VS_shiftLeftInt8Variable | UNUSED | — |
| vs/vs_effect.c:4861 | VS_moduloInt8Variable | UNUSED | — |
| vs/vs_effect.c:4866 | VS_divideInt8Variable | UNUSED | — |
| vs/vs_effect.c:4871 | VS_multiplyInt8Variable | UNUSED | — |
| vs/vs_effect.c:4876 | VS_subtractInt8Variable | UNUSED | — |
| vs/vs_effect.c:4881 | VS_addInt8Variable | UNUSED | — |
| vs/vs_effect.c:4886 | VS_setInt8Variable | UNUSED | — |
| vs/vs_effect.c:4891 | VS_shiftRightInt16Variable | UNUSED | — |
| vs/vs_effect.c:4896 | VS_shiftLeftInt16Variable | UNUSED | — |
| vs/vs_effect.c:4901 | VS_moduloInt16Variable | UNUSED | — |
| vs/vs_effect.c:4906 | VS_divideInt16Variable | UNUSED | — |
| vs/vs_effect.c:4911 | VS_multiplyInt16Variable | UNUSED | — |
| vs/vs_effect.c:4916 | VS_subtractInt16Variable | UNUSED | — |
| vs/vs_effect.c:4921 | VS_addInt16Variable | UNUSED | — |
| vs/vs_effect.c:4926 | VS_setInt16Variable | UNUSED | — |
| vs/vs_effect.c:5000 | VS_calculateAttackHitPosition | UNUSED | — |
| vs/vs_main.c:1702 | VS_calculateElementBonus | USED-DETERMINED | `(int16_t)nativeFallback`; caller supplies PS1 data address |
| vs/vs_scene.c:1556 | VS_getFirstSpecialMove | USED-DETERMINED | `nativeFallback`; caller supplies `fighter->moves[2]` |

## USED-* site notes

### BTL / STD / VS_setupLoadedEFEFile — USED-UNCLEAR

The three complete function bodies match after replacing their respective `BTL_`, `STD_`, and `VS_` prefixes. All three original game functions were inspected:

| Function | Original address | Callback registration |
| --- | --- | --- |
| BTL_setupLoadedEFEFile | `0x800650EC` | battle_effect.c:1887 |
| STD_setupLoadedEFEFile | `0x8006D7D4` | std_effect.c:1992 |
| VS_setupLoadedEFEFile | `0x800638A4` | vs_effect.c:1623 |

**Caller and use — from C:** These are registered by the corresponding `*_loadEFEFinishCallback` through `setFileReadCallback2`. The resulting invocation is `tickFileReadQueue`’s `req->waitCallback(req->waitCallbackParam) == 0`, in main/file_queue.c. The return controls request completion. Ordering-table references are not additional callers.

The missing-return paths are:

1. Texture allocation has no free slot: write `*ld->isLoaded = -2`, then `goto end`.
2. Case 2 finds the wrong effect ID: write `-3`, then `goto end`.
3. Case 2 finds no available effect-data slot: write `-1`, then `goto end`.
4. The load-state switch matches no case, including state 3.

**PS1 results — from disassembly:**

- The initial `*_getEFEFileId(data)` sets `v0` to the header’s 32-bit word at offset `0x30`, i.e. `fileId`.
- The allocation-failure path preserves that `fileId`. The allocation search and `-2` store use other registers.
- An unmatched load state preserves `fileId`.
- Entering directly in state 2 preserves `fileId` on either error path.
- Entering state 1 with `tmd == NULL` also preserves `fileId` on those error paths.
- If execution passes through `updateTMDTextureData`, that game function leaves `v0 = m->modelPtr->nobj`. Neither subsequent case-2 error path changes it.
- Entering state 0 with `tmd == NULL` can instead carry the residue of `GsGetTimInfo`, or the return of the subsequent palette `LoadImage`. The game wrapper preserves that residue on the case-2 error paths.

The inspected `updateTMDTextureData` is game code at `0x800A3E74`. It loads the model’s object count into `v0` and does not subsequently overwrite `v0`.

**What remains needed:** An allowed observation of `v0` after the void SDK `GsGetTimInfo` on the state-0/no-model path, or evidence that every relevant error path necessarily passes through `updateTMDTextureData`. Sony SDK code was not disassembled.

A blanket `return 0`, `return *ld->isLoaded`, or `return fileId` would not reproduce every path. A eventual fix needs path-specific preserved results. The known paths permit `return fileId` or `return m->modelPtr->nobj`, as specified above.

### BTL / STD / VS_calculateElementBonus — USED-DETERMINED

**All callers use the result:**

| Function | Caller | Call sites |
| --- | --- | --- |
| BTL_calculateElementBonus | BTL_selectPartnerMove | battle_main.c:3500, 3503 |
| STD_calculateElementBonus | STD_selectPartnerMove | std_main.c:6494, 6497 |
| VS_calculateElementBonus | VS_selectPartnerMove | vs_main.c:1618, 1621 |

Each result contributes to an `int16_t weights[]` entry.

**Missing path:** `TYPE_FACTORS[arg0][arg1]` is outside `{2, 5, 10, 15, 20}`.

**Reachability — from C:** All 49 initialized entries of `TYPE_FACTORS`, main/item.c:94, belong to that set. No source assignment modifying this table was found. Thus the missing path is not reached with intact table data and valid indices.

**PS1 result — from disassembly:** On an unmatched factor, all three helpers leave incoming `v0` untouched. The factor is loaded into `v1`, not `v0`.

Their callers establish:

```c
v0 = 0x8012CEB4u + 52u * defenderType;
```

Here `defenderType` is:

- First call: `digimon->entity.type`.
- Second call:
  `ENTITY_TABLE[COMBAT_DATA_PTR->player.entityIds[fighter->targetId]]->type`.

`0x8012CEB4` is the original US `DIGIMON_DATA` address. The original callers compute the entry address before loading its `special[0]` into `a1`.

Original helper addresses:

- BTL: `0x8005FAD0`
- STD: `0x80067304`
- VS: `0x8005EC90`

Original call instructions:

- BTL: `0x80060A44`, `0x80060AE0`
- STD: `0x80067E70`, `0x80067F0C`
- VS: `0x8005F3BC`, `0x8005F458`

**Exact native fix:** Add a native-only `uint32_t nativeFallback` argument, supplied with the address expression above at each call, then append:

```c
#ifdef DW1_NATIVE
    return (int16_t)nativeFallback;
#endif
```

The narrowing preserves the observed result: these callers ultimately store the weight as 16 bits. This is a reconstruction of the unmatched-factor behavior, not a claim that that path presently occurs. A native pointer cast would produce a host address and would not reconstruct the PS1 address.

### loadStackedTIMEntry — USED-DETERMINED

**Caller:** `loadCardImage`, main/game_menu.c:2383:

```c
return loadStackedTIMEntry(...);
```

That passes the result on, so it counts as a use. `loadCardImage`’s call at game_menu.c:2078 discards its eventual result.

**PS1 result — from disassembly:** The original function at `0x800A3564` returns the result of its last `LoadImage`:

- No CLUT: pixel-image `LoadImage`.
- CLUT present: palette `LoadImage`.

After the pixel call, the palette test uses `v1`. On the no-CLUT branch, `v0` survives unchanged to the epilogue. The CLUT branch finishes with another `LoadImage`.

`LoadImage` is declared to return `int` in the SDK header; its implementation was not disassembled.

**Exact native fix:** Under `DW1_NATIVE`, declare `int32_t lastLoadImageResult`, capture each **existing** `LoadImage` call into it, then append:

```c
#ifdef DW1_NATIVE
    return lastLoadImageResult;
#endif
```

Do not call `LoadImage` again in the return statement. Do not replace its result with an assumed constant: the native implementation currently returns zero, while its source notes observed PS1 queue-dependent returns.

### getItemOptionColor — USED-UNCLEAR

**Only caller:** `renderItemOption`, main/inventory.c:733. It passes the result to `renderString`.

**Missing path — from C:** `mode` is outside `0..3`. Modes 0–3 return on every path, including unrecognized `GAME_STATE` values.

**Current reachability — from C:** The sole caller loops with `i = 0; i < 4; ++i` and passes `i`, so it cannot reach the missing path.

**PS1 result — from disassembly:** Original function `0x800DAAF8`. Unsupported modes branch from `0x800DABE4` directly to the epilogue at `0x800DAC40`; that path never writes `v0`. It returns incoming `v0`, not the inventory item type.

**What remains needed:** If unsupported-mode behavior must be implemented, identify the register value supplied by a new caller, or specify an intentional fallback policy. There is no evidence of a missing-result defect in the existing caller.

### _sin — USED-DETERMINED, unreachable fall-through

**Caller use — from C:** Every actual call consumes the result. The complete caller inventory is below.

**Reachability — from C:** `input &= 0x1FF` restricts `input` to `0..511`. The four return branches cover:

- `0..127`
- `128..255`
- `256..383`
- `384..511`

No input can reach the closing brace. There is no PS1 missing-return value to reconstruct.

**Native action:** No runtime fix is required. If eliminating this warning is desired, an appended native-only `return 0;` is unreachable warning suppression:

```c
#ifdef DW1_NATIVE
    return 0; /* Unreachable after input &= 0x1FF. */
#endif
```

This zero is not asserted to be a PS1 fall-through result.

### flipTextboxPage — USED-UNCLEAR

**All callers use the result:**

- `tickNamingBox`, shop/script_menu.c:2797, tests it against zero.
- `isItemMenuBoxBusy`, main/script_common.c:1961, returns it onward.

**Missing path — from C:** A ready, registered, double-buffered textbox whose write/render counts agree completes:

```c
entry->backPage ^= 1;
entry->flipCount++;
entry->pageReady = 0;
```

and falls through.

**PS1 result — from disassembly:** Original function `0x800FE650`. The successful-flip instructions at `0x800FE6D8..0x800FE6F8` use `v1`; none writes `v0`. The epilogue also preserves `v0`. The result is therefore the incoming register value, not `flipCount`, `backPage`, or zero.

Both inspected direct callers call it without first establishing a fresh `v0` value:

- `tickNamingBox`: `jal` at `0x8010A02C`.
- `isItemMenuBoxBusy`: `jal` at `0x800FCFA0`.

**What remains needed:** Trace incoming `v0` through each caller’s invoking chain, preferably with an oracle observation at successful flips. A common constant return is not established by the inspected code.

### playSound — USED-UNCLEAR; current consuming call cannot fall through

There are **289 call expressions** in the searched source set. The complete grouped inventory appears below.

**Only return-value use:** `ENDI_tickEnding`, endi/endi.c:430:

```c
return playSound(8, 0);
```

All other calls discard the result.

**Missing path — from C:** `vabId` is not in `{0,1,3,4,5,6,7,8}`: for example 2, a negative ID, or an ID greater than 8.

**PS1 result — from disassembly:** Original function `0x800C6374`. Unsupported IDs bypass both `startSound` calls and preserve incoming `v0`. The ID checks use `a3` and `at`.

**Current consuming path — from C:** The sole consuming call uses literal ID 8, which takes an explicit `return startSound(...)`. It cannot observe a fall-through result.

**What remains needed:** Only if unsupported IDs will have consuming callers: establish their incoming register values or choose an intentional unsupported-ID policy. No missing-result defect is demonstrated for the current consuming call.

### loadSoundCompleteCallback — USED-DETERMINED

**Registration:** `loadSoundFinishCallback`, main/sound_async.c:102, installs it with `setFileReadCallback2`.

**Caller and use:** `tickFileReadQueue` invokes it as `req->waitCallback(...)` and tests for zero.

**Missing path — from C:** `LOAD_SOUND_COMPLETE_STATE` is outside `{0,4,5}`. The source state transitions assign only those three values, so this is an unsupported-state path.

**PS1 result — from disassembly:**

- Callback address: `0x800E8CDC`.
- Unsupported-state branch: `0x800E8D28` to epilogue `0x800E8E30`.
- That path never writes `v0`.
- The dispatcher at `0x800E38F8` loads the callback address into `v0`, then calls it with `jalr v0` at `0x800E3900`.
- Therefore the unsupported-state result is exactly `0x800E8CDC`, which is nonzero and keeps the request waiting.

**Exact native fix:**

```c
#ifdef DW1_NATIVE
    return (int32_t)0x800E8CDCu;
#endif
```

Returning the native function’s address would not reproduce the original numeric register value.

### getEntityFromScriptId — USED-UNCLEAR

**All callers assign the result:**

| Caller | File:lines |
| --- | --- |
| spawnSpriteAtEntity | main/overworld.c:997 |
| scriptUnloadEntity | main/overworld.c:1146 |
| setLoopCountToOne | main/overworld.c:1828 |
| tickEntityWalkTo | main/tamer.c:1010, 1023 |
| tickLookAtEntity | main/tamer.c:1085, 1090 |
| tickEntitySetRotation | main/tamer.c:1109 |
| tickEntityMoveTo | main/tamer.c:1131, 1139 |
| tickEntityMoveToAxis | main/tamer.c:1207 |

**Missing path — from C:** The ID is neither `0xFD` nor `0xFC`, and none of the eight NPC entries matches.

**PS1 result — from disassembly:** Original function `0x800AC2F8`. The unsuccessful search uses `v1`, argument registers, and temporaries, without writing `v0`. It returns the incoming register value.

It does **not** return null, the final entity-table entry, or loop index 8.

**What remains needed:** Caller-specific incoming-`v0` reconstruction, including preceding callees where relevant, or oracle observations of unsuccessful searches. A native pointer result also needs a deliberate translation if the residue represents a PS1 address.

### STD_getEntityIndex — USED-DETERMINED

**All caller uses:**

- `STD_placePlayer2`, std_main.c:1603: assigns the result to `id`.
- `STD_runIntro`, std_main.c:1910/1912 and 1942/1944: assigns the result. Each pair is conditional source; the US calls are at 1912 and 1944.

**Missing path — from C:** No entry in `ENTITY_TABLE[0..9]` matches `entity`.

**PS1 result — from disassembly:** The function at `0x80058634` leaves incoming `v0` untouched on an unsuccessful search. It does not return 10.

The inspected US callers establish:

| Caller path | Incoming `v0` |
| --- | --- |
| STD_placePlayer2, `which == 1` | `-0x708` |
| STD_placePlayer2, otherwise | `-0x9C4` |
| STD_runIntro after STD_startCameraChase | `GS_VIEWPOINT.vpy` |

`STD_placePlayer2` sets `v0` to its selected X coordinate before the call at `0x80058538`.

`STD_startCameraChase` finishes by computing/clamping `GS_VIEWPOINT.vpy` in `v0`, then returns without altering it. `STD_runIntro` preserves that value until its entity-index call.

The first `STD_runIntro` call passes `ENTITY_TABLE[1]`, so that call’s unsuccessful search is itself unreachable: the searched array contains its argument.

**Exact native fix:** Add a native-only `int16_t nativeFallback` argument, passing:

- `x` from `STD_placePlayer2`.
- `(int16_t)GS_VIEWPOINT.vpy` from each `STD_runIntro` call.

Append:

```c
#ifdef DW1_NATIVE
    return nativeFallback;
#endif
```

A single constant cannot preserve both placement and intro paths.

### STD_getEntityIndex2 — USED-DETERMINED

**Only caller:** `STD_initializeCombat`, std_main.c:4172, stores the result in `COMBAT_DATA_PTR->player.entityIds[1]`.

**Missing path — from C:** No entity-table entry matches `other`.

**PS1 result — from disassembly:**

- Helper address: `0x80062D5C`.
- Unsuccessful search preserves incoming `v0`.
- Immediately before the call at `0x8005E994`, the caller loads `COMBAT_DATA_PTR` into `v0`.
- The following result store is a byte store.

`COMBAT_DATA_PTR` is initialized to `&COMBAT_DATA` in main/main.c:4227; the US symbol address of `COMBAT_DATA` is `0x8013D640`. No other source assignment to this pointer was found.

Thus the raw PS1 register result is `0x8013D640`; the sole caller stores byte `0x40`.

**Exact native fix for the inspected caller behavior:**

```c
#ifdef DW1_NATIVE
    return (int16_t)0xD640;
#endif
```

The function is declared `int16_t`; this expression preserves the low 16 bits and therefore the caller’s byte. It does not claim to reproduce the original register’s upper 16 bits.

### VS_getFirstSpecialMove — USED-DETERMINED

**All callers:** `VS_loadFighterEntities`, vs_scene.c:1244 and 1260. Both assign the result to `stats->base.moves[3]`.

**Missing path — from C:** Searching all sixteen move entries finds no non-`0xFF` move at least `0x3A`.

**PS1 result — from disassembly:**

- Helper address: `0x800580E4`.
- Its unsuccessful search never writes `v0`.
- At the first call, `0x800570CC`, incoming `v0` is the unsigned byte `fighter1->moves[2]`.
- At the second call, `0x80057178`, incoming `v0` is `fighter2->moves[2]`.

Each caller has just loaded that third move into `v0` and stored it into `stats->base.moves[2]`; loading the digimon ID into `a0` does not alter `v0`.

The unsuccessful result is therefore the fighter’s third selected move, not loop index `-1`, `0x2D`, or necessarily `0xFF`.

**Exact native fix:** Add a native-only `int32_t nativeFallback` argument. Supply `fighter1->moves[2]` and `fighter2->moves[2]` at the respective calls, then append:

```c
#ifdef DW1_NATIVE
    return nativeFallback;
#endif
```

The fallback cannot reliably be derived from `index` alone: separate registered fighters of the same digimon type can have different selected moves.

## UNUSED caller inventory

All results in this section are discarded.

### Effect families: 78 UNUSED sites

The complete bodies of all 27 warning-bearing functions in each effect file match after prefix normalization. This includes the loader, UV-offset helper, 24 arithmetic helpers, and attack-hit-position helper.

For every prefix `P = BTL`, `STD`, or `VS`:

- `P_offsetEFEPrimitiveUVs` is called only by `P_tickEFEUVAnimation`:
  - BTL battle_effect.c:1907
  - STD std_effect.c:2012
  - VS vs_effect.c:1643
- `P_calculateAttackHitPosition` is called by:
  - `P_applyRadiusAttackHit`: BTL:2384, STD:2552, VS:2135
  - `P_applyLineAttackHit`: BTL:2444, STD:2612, VS:2195

Each arithmetic helper is stored in `P_EFE_VARIABLE_OPERATORS[5][8]`. Its indirect caller is `P_applyEFEVariableOperator`, whose two invocation paths both discard the result:

| Prefix | File | Indirect calls |
| --- | --- | --- |
| BTL | btl/battle_effect.c | 4946, 4948 |
| STD | std/std_effect.c | 5135, 5137 |
| VS | vs/vs_effect.c | 4720, 4722 |

This covers every arithmetic helper individually through the following table mapping:

| Helper suffix | Operator column |
| --- | --- |
| setInt8Variable / setInt16Variable / setInt32Variable | 0 |
| addInt8Variable / addInt16Variable / addInt32Variable | 1 |
| subtractInt8Variable / subtractInt16Variable / subtractInt32Variable | 2 |
| multiplyInt8Variable / multiplyInt16Variable / multiplyInt32Variable | 3 |
| divideInt8Variable / divideInt16Variable / divideInt32Variable | 4 |
| moduloInt8Variable / moduloInt16Variable / moduloInt32Variable | 5 |
| shiftLeftInt8Variable / shiftLeftInt16Variable / shiftLeftInt32Variable | 6 |
| shiftRightInt8Variable / shiftRightInt16Variable / shiftRightInt32Variable | 7 |

The `Int8` helpers occupy rows 0, 1, and 3; `Int16` helpers row 2; `Int32` helpers row 4. No other invoking use of these helper addresses was found.

The additional `*_functions` ordering arrays have no invoking references in `decomp/src`; their entries do not create result consumers.

### Other UNUSED functions

| Function | Every caller or registration |
| --- | --- |
| DOOA_updateShards | Registered by DOOA_addShardEffect through `addObject` at dooa/dooa.c:657; invoked by `tickObjects`, main/world_object.c:89, through a void `TickFunction` |
| DOOA_renderShards | Same registration at dooa/dooa.c:657; invoked by `renderObjects`, main/world_object.c:101, through a void `RenderFunction` |
| FISH_openBaitMenu | FISH_tickRod, fish/fish.c:3784 |
| FISH_showTextBox | FISH_tickRod, fish/fish.c:4063, 4078, 4083, 4088, 4300, 4305, 4346, 4363, 4369, 4381, 4385, 4390 |
| KAR_computeSeparation | KAR_collideRestingStone, kar/kar.c:1625; KAR_collideMovingStones:1679, 1680 |
| KAR_findWallContact | KAR_bounceOffWall, kar/kar.c:1502, 1507 |
| KAR_chooseOpponentShot | KAR_tickMatchState, kar/kar.c:3150 |
| setEFEFlashOffset | DOOA_tickDissolve, dooa/dooa.c:523 |
| popScriptStack | scriptInstruction10to27, main/script_interp.c:734; returnFromScriptFile:2692 |
| setCardAmount | openMeritShop, shop/script_menu.c:859; showCardTextbox:1351; scriptInstruction28to3F, main/script_interp.c:1148, 1156; tickSingleCardShop, main/script_common.c:1108; tickItemMenuAmountBox:1547 |
| loadSoundFinishCallback | Registered by loadFullVHB, main/sound_async.c:83, and loadFullVHBSection:125; invoked as `req->finishCallback(...)` in main/file_queue.c, with result discarded |

The two DOOA callback names also occur in ordering arrays; those arrays are not invoked. The callback dispatcher’s void function types establish that their results are unused.

`loadSoundFinishCallback` is a **finish** callback, whose result is discarded. Its installed `loadSoundCompleteCallback` is a **wait** callback, whose result is tested. These are distinct uses.

### addScreenPolyFT4

Every call discards the result:

| File | Caller:lines |
| --- | --- |
| vs/vs_effect.c | VS_render3DTexturedQuad:2725; VS_renderAuraProjectile:5961 |
| btl/battle_effect.c | BTL_render3DTexturedQuad:2984; BTL_renderAuraProjectile:6183; BTL_renderBuffTrails:6578 |
| std/std_effect.c | STD_render3DTexturedQuad:3142; STD_renderAuraProjectile:6377 |
| doo2/doo2.c | DOO2_renderQuadShard:324 |
| evl/evl.c | EVL_renderQuadShard:1160 |
| main/model.c | renderFlatDigimon:2787 |
| dooa/dooa.c | DOOA_renderShards:1551 |

### loadTextureFile

Every call discards the result:

- `MURD_tick`, murd/murd.c:778.
- `spawnBoulder`, main/map_object.c:624.
- `scriptInstruction64to7E`, main/script_interp.c:1849.
- `FISH_loadSpritesAndSeadramon`, fish/fish.c:2008.

### readFile

Every call discards the result, including calls that contain an assignment **inside an argument**:

| File | Caller:lines |
| --- | --- |
| vs/vs_hud.c | VS_loadVersusSceneModel:1325; VS_loadStageModels:1435 |
| vs/vs_scene.c | VS_loadArenaAssets:1687, 1695; VS_loadArenaTIMToVRAM:1818; VS_loadTIMToVRAM:1838 |
| std/std_hud.c | STD_loadVersusSceneModel:1144; STD_loadWinLoseModel:1250; STD_loadChampionModels:1295, 1350, 1352 |
| std/std_setup.c | STD_loadTIMToVRAM:508; STD_loadArenaTIMToVRAM:625; STD_loadArenaAssets:647, 655 |
| kar/kar.c | KAR_start:786 |
| main/utils2.c | loadDynamicLibrary:402; loadStackedTIMFile:518 |
| main/map_object.c | loadStaticTMD:298; initializeChest:450; loadWarpCrystals:507; spawnToyTownBoxes:672, 673 |
| main/model.c | loadMMD:3414, 3446; applyMMD:3684, 3692 |
| main/partner_impl.c | initializePoop:438 |
| main/sound.c | readVHBFile:399 |
| main/script_interp.c | initializeScripts:2203 |
| main/map.c | loadMap:4036; readMapTFS:4222 |
| main/file.c | loadTextureFile:63; loadTIMFile:103 |
| dooa/dooa.c | DOOA_tickDissolve:536, 548 |
| fish/fish_model.c | FISH_loadTMDModel:28 |

### loadTIMFile

Every call discards the result:

| File | Caller:lines |
| --- | --- |
| vs/vs_demo.c | VS_setupDemoBattle:112, 113, 114; VS_cleanupDemoBattle:152; VS_setupTrialBattle:352, 353, 354, 355, 356, 357; VS_initializeTrialBattle:493 |
| vs/vs_select.c | VS__loadTextures:282, 283, 284, 285, 286, 287; VS__initialize:474 |
| vs/vs_scene.c | VS_loadVSAssets:1181 |
| std/std_setup.c | STD_initializeTournament:269, 271; STD_deinitializeTournament:451 |
| main/main.c | main:3423, 3426; runMainMenu:3710 |

### readFileSectors

Every call discards the result:

- `loadDigimonTexture`, main/model.c:3301.
- `loadMMDAsync`, main/model.c:3625.
- `readVHBFileSectors`, main/sound.c:454.
- `loadStackedTIMEntry`, main/file.c:122.

## Complete `_sin` caller inventory

Every listed call consumes the result through assignment, arithmetic, or returning it onward.

| File | Caller:lines |
| --- | --- |
| btl/battle_main.c | BTL_applyStretchScale:2717, 2718; BTL_applySquashScale:2723, 2724 |
| std/std_main.c | STD_applyStretchScale:7042, 7043; STD_applySquashScale:7048, 7049 |
| vs/vs_main.c | VS_applyStretchScale:1123, 1124; VS_applySquashScale:1129, 1130 |
| btl/battle_effect.c | BTL_renderRingTube:2538; BTL_checkTargetCollision:3870; BTL_setTransformToSourceBone:3909, 3910; BTL_calculatePolarOffset:4126; BTL_setTransformToTargetBone:4594, 4595; BTL_renderPoisonBubble:5482; BTL_renderConfusionEffect:5587; BTL_renderFinisherAura:5963 |
| std/std_effect.c | STD_renderRingTube:2706; STD_checkTargetCollision:4052; STD_setTransformToSourceBone:4091, 4092; STD_calculatePolarOffset:4308; STD_setTransformToTargetBone:4783, 4784; STD_renderPoisonBubble:5676; STD_renderConfusionEffect:5781; STD_renderFinisherAura:6157 |
| vs/vs_effect.c | VS_renderRingTube:2289; VS_checkTargetCollision:3635; VS_setTransformToSourceBone:3674, 3675; VS_calculatePolarOffset:3891; VS_setTransformToTargetBone:4366, 4367; VS_renderPoisonBubble:5261; VS_renderConfusionEffect:5366; VS_renderFinisherAura:5741 |
| eab/eab.c | EAB_renderBuildupRing:271; EAB_renderSpawnRing:328; EAB_tickSpawn:408, 415 |
| main/math.c | _cos:29 |
| main/particle.c | tickHealingParticles:149 |
| main/butterfly.c | renderButterfly:120, 124 |
| main/bubble.c | tickConditionBubble:644 |
| main/efe.c | tickParticleFX:529; renderParticleFX:593; renderEFEFlash:949, 956 |
| dooa/dooa.c | DOOA_tickDissolve:522 |

## Complete `playSound` caller inventory

All calls below discard the result **except `ENDI_tickEnding` at endi/endi.c:430**, which returns it onward.

Shared source files contain some version-dependent alternatives; these are retained in the inventory.

| File | Caller:lines |
| --- | --- |
| vs/vs_effect.c | VS_playEFESound:3830 |
| vs/vs_hud.c | VS___tickVSInput:700, 707, 714, 726 |
| vs/vs_intro.c | VS__renderIntroStats:500 |
| vs/vs_select.c | VS__isAlreadySelected:376; VS__handleDigimonSelected:388; VS__tickSelectDigimonPlayer:561, 571, 613, 628; VS__tickSelectBox:1062, 1079, 1087, 1097, 1102 |
| vs/vs_combat.c | VS_setupDemoCombat:495, 515; VS__combatSetup:826, 840; VS___tickVSInput:2620, 2633, 2646, 2669 |
| eab/eab.c | EAB_tickSpawn:446; EAB_tick:707 |
| btl/battle_setup.c | BTL_handleBattleIntro:258, 322 |
| btl/battle_effect.c | BTL_playEFESound:4065 |
| btl/battle_main.c | BTL_handleVictorySequence:889, 904 |
| trn/trn_speed.c | TRN_tickSpeedTraining:73, 112 |
| trn/trn_hud.c | TRN_renderPostTrainingStatsBox:262 |
| trn/trn_slots.c | TRN_tickSlotMachine:120, 142, 172, 184, 187, 190, 194 |
| trn/trn_hp.c | TRN_tickHpTraining:92, 105, 121, 133 |
| trn/trn_mp.c | TRN_tickMpTraining:74, 115 |
| trn/trn_brain.c | TRN_tickBrainsTraining:83, 122, 132, 148 |
| trn/trn_def.c | TRN_tickDefenseTraining:117, 245 |
| trn/trn_off.c | TRN_tickOffenseTraining:110, 142, 171, 209, 237 |
| shop/script_menu.c | tickCardMenu:1860; createMeritCardTradeDialogue:1930, 1936; tickItemKeeperWindow:1986, 1989, 1992, 1996, 1999, 2002, 2021; tickJukeboxMenu:2432, 2435; tickBirdraTransportMenu:2505, 2507, 2511; tickMojyaTradeMenu:2581, 2583, 2587 |
| shop/script_menu.c | tickNamingBox:2816, 2832, 2846, 2861, 2869, 2873, 2879, 2884; namingDeleteLast:3060; namingSelectionLeft:3113; namingSelectionRight:3138; namingSelectionUp:3163; namingSelectionDown:3188 |
| doo2/doo2.c | DOO2_openEggBox:736; DOO2_tickEggInput:757, 761, 765 |
| dget/dget.c | tickTournamentSchedule:480, 491, 493, 505, 516, 527, 537, 547, 549; tickTournamentInfo:639 |
| std/std_hud.c | STD_handleCommands:507, 514, 521, 533 |
| std/std_effect.c | STD_playEFESound:4247 |
| std/std_main.c | STD_renderIntroStats:2024; STD_updateBracket:2995, 3018, 3089, 3108; STD_combatSetup:3993, 4007 |
| kar/kar.c | KAR_tickYesNoPrompt:2062, 2069; KAR_tickHintBox:2124 |
| evl/evl.c | EVL_tickEvoSequence:505 |
| main/game_menu.c | tickGameMenu:1660, 1665, 1667, 1674; tickDigimonMenu:1721, 1731, 1738, 1744, 1768, 1772, 1778, 1811, 1816 |
| main/game_menu.c | tickPlayerMenu:1881, 1891, 1896, 1901, 1907, 1966, 2012, 2016, 2033, 2066, 2076, 2108, 2113; equipMove:2325, 2358, 2368, 2377 |
| main/utils2.c | handlePause:459; createPauseBox:478 |
| main/inventory.c | tickInventoryUI:367; tickInventoryTop:539, 543, 548, 553; tickDropItemConfirmBox:796, 800; selectRegularInventory:934; tickInventoryOptionSelector:1027, 1031 |
| main/anim.c | tickAnimation:339 |
| main/partner.c | partnerTickSleep:297; partnerTickPraiseScold:389; partnerTickFeedItem:464 |
| main/main_menu.c | tickMainMenu:3437, 4420, 4448; tickMenuInput:4681, 4684, 4690, 4693, 4698, 4702; tickSelectSlotInput:4882, 4885, 4892, 4895, 4901, 4905 |
| main/battle_tick.c | handleCommands:88, 107, 122, 181 |
| main/tamer.c | tamerTickPickupItem:1447, 1458, 1467; tamerTickPraiseScold:1646; tamerTickAwardSomething:1811 |
| main/main.c | runLandingScreen:3643, 3656 |
| main/script_interp.c | scriptInstruction5Ato5F:1560 |
| main/ui.c | createAnimatedUIBox:223; renderUIBoxAnim:336 |
| main/battle_ui.c | tickBitBox:438, 448; renderPostBattleStatsBox:609 |
| main/script_textbox.c | tickSelectionDialogue:238, 248, 255, 260, 268, 275; tickConfirmDialogue:306, 309; tickCustomSizedTextbox:370, 387 |
| main/script_common.c | tickItemMenu:856, 873; readSelectedItemMerit:893, 898; tickItemMenuDescriptionBox:918; tickSingleCardShop:1085, 1091, 1097, 1119, 1122 |
| main/script_common.c | tickItemMenuAmountBox:1482, 1499, 1507, 1510; createItemMenuAmountBox:2038, 2042; createSingleCardShopMenu:2086, 2090 |
| main/script_common.c | itemMenuCursorUp:2124, 2126; itemMenuCursorDown:2156, 2159; playShopSoundOnlyInSavannah:2378; itemMenuCursorMoveToBottom:2874; itemMenuCursorMoveToTop:2883 |
| dooa/dooa.c | DOOA_tickDissolve:469, 502; DOOA_tickRebirth:1047, 1077, 1147 |
| fish/fish.c | FISH_tickBaitMenu:1172, 1184, 1196, 1204, 1211, 1219; FISH_tickTextBox:1466; FISH_bobberCommand:2405, 2417, 2429; FISH_tickHookedFish:3253, 3344, 3428 |
| fish/fish.c | FISH_tickRod:3905, 3955, 4200, 4298, 4334, 4345, 4351, 4362, 4368, 4378 |
| endi/endi.c | ENDI_updateEnding:194; **ENDI_tickEnding:430 — result returned onward** |
| trn2/trn2_hud.c | TRN2_renderPostTrainingStatsBox:465 |
| trn2/trn2_hp_map99.c | TRN2_tickHpTrainingMap99:94, 103 |
| trn2/trn2_def_map108.c | TRN2_tickDefenseTrainingMap108:120, 146 |
| trn2/trn2_def_map99.c | TRN2_tickDefenseTrainingMap99:61, 87, 113 |
| trn2/trn2_reward.c | TRN2_tickSpeedTraining:82, 116 |
| trn2/trn2_mp.c | TRN2_tickMpTraining:63, 93 |
| trn2/trn2_off.c | TRN2_tickOffenseTraining:108, 134, 157 |
| trn2/trn2_slots.c | TRN2_tickSlotMachine:120, 142, 172, 184, 187, 190, 194 |
| trn2/trn2_hp_map107.c | TRN2_tickHpTrainingMap107:86, 99, 115, 124 |

## Verification and limitations

- Matched all 110 supplied warning locations to their enclosing functions.
- Confirmed all locations are function-ending braces and found no bare `return;` sites.
- Searched named references across the requested source scope and inspected runtime function-pointer dispatches.
- Checked all 27 corresponding warning-bearing effect-function bodies across BTL, STD, and VS.
- Read the original executable’s load address, `0x80090800`, from its PS-X EXE header; decoded executable instructions from file offset `0x800`.
- Used overlay load address `0x80052AE0` from `decomp/config/us/overlay.ld`.
- Disassembled only named game functions and their game callers. No Sony SDK implementation was disassembled.
- Native return expressions are proposed fixes, not implemented or runtime-tested changes.