# Phase 9: static audit of the overlays not yet recorded (gpt-6.1-sol, 2026-10-15)

Read-only audit by gpt-6.1-sol (high) of std, vs, fish, evl, doo2, dooa, kar, dget, trn2, eab and
endi for the native bug classes that broke the earlier overlays. Findings 1-8, 10, 11, 13-16
were fixed or handled in the decomp fork (9102d79); finding 9 (missing `GsTMDfast*` handlers and
sorter restrictions for arenas, result scenes, Seadramon and curling) and `SetDrawLoad` were done
by the render stream (`docs/phase9-render.md`). Status per finding: `docs/phase9-plan.md`,
Progress.


**The main playthrough blockers are in EVL and DOOA/DOO2.** STD and VS also have definite initialization and rendering blockers. EAB can corrupt unrelated native globals, including an ending buffer pointer.

This was read-only. I read the requested documents first, checked the 40 translation units selected by the US build for these overlays, ran Clang syntax/AST/layout checks, inspected relevant SDK implementations, and read primitive headers from the local disc. **No emulator comparison or gameplay reproduction was performed.** “Confirmed” below means established from the current source/layout, not reproduced in play.

Paths below are relative to `decomp/src/` unless prefixed with `crates/`. Findings are ranked approximately by likelihood of disrupting a normal playthrough. Earlier blockers currently mask several later ones.

**Confirmed findings**

**1. Digivolution truncates addresses throughout its initialization and shard pipeline — classes 4, 8**

Locations and functions:

- [evl/evl.c:1297](/Users/fenn/Developer/dw1-silicon/decomp/src/evl/evl.c:1297), `EVL_initEvoSequence`: passes native addresses to the helpers below.
- `evl/evl.c:269,283,292`: `EVL_storeDigimonClut`, `EVL_storeClutBank0`, `EVL_storeClutBank1` accept `int32_t buffer`, then cast it back to a pointer for `StoreImage`.
- `evl/evl.c:301,318`: `EVL_initShardSets`, `EVL_setScratchTop` likewise accept addresses as `int32_t`.
- `evl/evl.c:215,1001,1066,1070`: the scratch cursor, shard address fields and primitive cursors remain 32-bit.
- `evl/evl.c:882`, `EVL_renderShardSet`: converts those truncated fields back into pointers.

**Consequence:** the first CLUT download already receives an invalid address. Widening the call-site cast to `intptr_t` has not fixed the receiving parameter. Later shard construction independently loses addresses.

**When/likelihood:** every ordinary digivolution; extremely high likelihood of stopping it.

**Fix direction:** propagate pointer types or `intptr_t` through declarations, definitions, globals, locals and shard fields. Follow the existing EFE widening pattern, including the storage and buffer changes in finding 2.

**2. Digivolution places expanded shard records in a PS1-sized byte array — classes 1, 2**

- [evl/evl.c:301](/Users/fenn/Developer/dw1-silicon/decomp/src/evl/evl.c:301), `EVL_initShardSets`
- `evl/evl.c:30`, `EvlShardSet`
- `evl/evl_bss_us.c:599`, `EVL_D_80065FA0[724]`

The initializer treats this byte array as 30 `EvlShardSet` records and reserves literal `0x2d0` bytes. The record was 24 bytes on PS1; **its current native size is 32 bytes**, even before widening its remaining address fields. Thirty current records require 960 bytes.

**Consequence:** after repairing the truncated base address, initialization itself writes beyond the array. Widening the remaining fields increases the required allocation further. Four-byte base alignment is also insufficient for its native pointer field.

**When/likelihood:** every digivolution initialization, masked by finding 1.

**Fix direction:** use a properly aligned native typed array/side table of 30 records under `DW1_NATIVE`, with `sizeof`-based bounds. Keep the original byte array/layout for the PS1 build.

**3. Digivolution and rebirth still pass raw KSEG addresses as native buffers — class 8**

- [evl/evl.c:199](/Users/fenn/Developer/dw1-silicon/decomp/src/evl/evl.c:199), `MAIN_D_801349E4 = EVL_MMD_BUFFER`, used by `EVL_tickEvoSequence` at line 408.
- [dooa/dooa.c:369](/Users/fenn/Developer/dw1-silicon/decomp/src/dooa/dooa.c:369), `DOOA_tickDissolve`: passes raw `DOOA_SHARD_BUFFER`, `0x80044800`, to `DOOA_initShardEffect`.
- `dooa/dooa.c:1074,1076`, `DOOA_tickRebirth`: passes raw `0x80020000` to `loadMMDAsync` and raw shard-buffer address to `DOO2_resetShardSets`.

These constants bypass `DW1_PS1_ADDR`. `loadMMDAsync` in `main/model.c:3580` accepts an address and directly uses it as a native destination; it does not translate KSEG addresses.

**Consequence:** invalid writes during shard initialization or texture/model loading. In particular, `DOOA_initShardEffect` writes through the raw address at line 651.

**When/likelihood:** normal evolution/death/rebirth; definite invalid destinations when reached.

**Fix direction:** translate these fixed addresses through `DW1_PS1_ADDR`, and use pointer-width storage wherever the translated address is retained. The already-translated DOOA ordering-table macros demonstrate the intended approach.

**4. Death/rebirth CLUT helpers truncate native pointers — class 4**

- [dooa/dooa.c:370](/Users/fenn/Developer/dw1-silicon/decomp/src/dooa/dooa.c:370), `DOOA_tickDissolve`
- `dooa/dooa.c:661,674,683`: `DOOA_saveEntityClut`, `DOOA_saveModelClut`, `DOOA_saveShardClut`
- `dooa/dooa.c:547,550`: calls into `DOO2_saveClutTile`, `DOO2_saveModelClut`
- `doo2/doo2.c:354,363`: those DOO2 helpers take `u_long pixels`.

The DOOA helpers receive `int32_t`; the DOO2 helpers receive native-build `u_long`, which is deliberately still 32-bit.

**Consequence:** `StoreImage` writes through a truncated address. The existing pointer-width casts at the callers do not prevent argument conversion from narrowing it again.

**When/likelihood:** ordinary death/rebirth, immediately after the earlier buffer-address blocker is repaired.

**Fix direction:** change the address-bearing parameters and corresponding declarations to pointers or pointer-width integers under the native build.

**5. Rebirth reads mapped TMD records through an incompatible native struct — classes 1, 2, 4**

- [doo2/doo2.c:597](/Users/fenn/Developer/dw1-silicon/decomp/src/doo2/doo2.c:597), `DOO2_buildShardSet`
- `doo2/doo2.c:29`, `Doo2ModelDesc`
- `doo2/doo2.c:626,641,657`: casts the loaded TMD object table to that struct and reads its fields.
- `dooa/dooa.c:1144`, `DOOA_tickRebirth`: passes the model address through the function’s `u_long modelList`.
- `doo2/doo2.c:165,173`; `include/dw/doo2.h:24`: additional 32-bit scratch cursors and shard address fields.

`Doo2ModelDesc` has native pointer fields and **is 40 bytes natively**, while the loaded TMD records remain seven 32-bit words, 28 bytes each. Its pointer fields also bypass `DW1_TMD_ADDR`, so they do not decode the SDK’s encoded references.

**Consequence:** wrong counts, wrong object stride and invalid vertex/primitive addresses. Merely widening `modelList` will not repair this.

**When/likelihood:** egg-model shattering during rebirth; definite failure when reached.

**Fix direction:** use the native replacement `struct TMD_STRUCT` and decode its three address fields with `DW1_TMD_ADDR`, as EVL already does. Separately widen runtime scratch cursors and `Doo2ShardSet` address fields.

**6. Boss buildup writes past a three-entry array and into unrelated native globals — classes 1, 8**

- [eab/eab.c:210](/Users/fenn/Developer/dw1-silicon/decomp/src/eab/eab.c:210), `EAB_initializeRings`
- `eab/eab_bss_us.c:8`, `EAB_D_800617D0[3]`

The initializer writes **18 entries**, although the array contains three. Other ring allocation functions correctly search only three entries.

This is especially harmful after native expansion: `EabModelFX` grows from 8 to 16 bytes. The existing debug binary places `ENDI_FADE_CLUT_BUFFER` at `EAB_D_800617D0 + 0x50`; iteration 5 writes `-1` into that pointer’s low halfword. The ending buffer pointer is resident state, so loading ENDI does not inherently restore it.

The retail game listing also performs 18 writes, at an eight-byte stride. Consequently, reproducing those writes through expanded C objects does not preserve even the original collateral writes.

**Consequence:** certain out-of-bounds writes; possible delayed ending failure and other cross-overlay corruption.

**When/likelihood:** every `EAB_startBuildup`; high concern for a full playthrough.

**Fix direction:** bound native initialization to the real three-entry table. If any retail overlapping initialization proves necessary, reproduce that specific state change explicitly rather than relying on writes past an object.

**7. Ending initialization truncates its CLUT buffer address — class 4**

- [endi/endi.c:423](/Users/fenn/Developer/dw1-silicon/decomp/src/endi/endi.c:423), `ENDI_tickEnding`
- `endi/endi.c:71,105`: `ENDI_setClutStp`, `ENDI_downloadClut`

Both helpers take `u_long clut`, then cast it back into a pointer.

**Consequence:** invalid `StoreImage` destination, followed by invalid CPU palette accesses and `LoadImage` source.

**When/likelihood:** every ending sequence; definite blocker independently of EAB’s corruption.

**Fix direction:** use pointer parameters throughout. Preserve `u_long` for palette words, not the address of the palette.

**8. STD and VS initialize their opcode tables using an eight-byte PS1 stride — classes 1, 9**

- [std/std_effect.c:5616](/Users/fenn/Developer/dw1-silicon/decomp/src/std/std_effect.c:5616), `STD_initializeEFESubOpcodeTable`
- [vs/vs_effect.c:5201](/Users/fenn/Developer/dw1-silicon/decomp/src/vs/vs_effect.c:5201), `VS_initializeEFESubOpcodeTable`

Both validate entries through:

```c
((int32_t (*)[2])...EFE_SUB_OPCODES)[i][0]
```

Each actual entry contains an integer and a function pointer, making it **16 bytes natively**. At `i == 1`, this reads part of the first handler address instead of the next opcode. The existing debug binary’s handler addresses fail the `< 0x61` validation.

**Consequence:** calls `exit(1)`, which resolves to the trapping `dw1_exit` stub.

**When/likelihood:** normal STD/VS effect-engine initialization; effectively certain in the inspected build.

**Fix direction:** copy BTL’s existing correction at `btl/battle_effect.c:5422`: read `table[i].opcode`. Implementing `exit` would not fix the underlying error.

**9. Required model-rendering SDK paths still trap — class 9**

These are confirmed from the source calls, installed handler table and actual disc primitive headers.

| Feature / function locations | Unsupported path and consequence | Likelihood |
|---|---|---|
| STD arena: `std/std_setup.c:691,709,730,746`; VS arena: `vs/vs_scene.c:1726,1737,1757,1768,1795` | Arena TMDs contain lit flat textured primitives requiring `GsTMDfastTF3L`/`TF4L`, unlit textured triangles requiring `GsTMDfastTNF3`, and unlit Gouraud primitives requiring `GsTMDfastTNG3`/`TNG4`. These remain stubs. Mode `0x2D` also reaches implemented `GsTMDfastTNF4`, which currently accepts only `0x2F`. | Normal arena rendering. |
| STD draw/result/champion: `std/std_hud.c:1219`; `std/std_effect.c:1365,1503,1589`; VS draw/result: `vs/vs_hud.c:1401,1551` | `DRAW.TMD`, `WIN_LOSE.TMD` and `CHAMP.TMD` contain lit flat textured triangles/quads. `1P2PWIN.TMD` adds other unsupported kinds. | Normal corresponding result scenes. |
| STD/VS arena attribute setters: `std/std_setup.c:693,733`; `vs/vs_scene.c:1727,1758,1785` | Set attribute `0x200`; `GsSortObject4J` rejects divide attributes at `crates/psyq/src/libgs/sort4j.rs:119`. | Certain on the specified renderer paths, once earlier model traps are fixed. |
| STD champion: `std/std_hud.c:1321` → `std/std_effect.c:1365` | Objects initially carry display-off bit `0x80000000`; the sorter rejects it at `sort4j.rs:116` instead of skipping hidden objects. | Beginning of champion animation. |
| Seadramon: `fish/fish.c:2603`, `FISH_drawSeadramon` | All five `SEA.TMD` objects contain lit flat textured triangles/quads, modes `0x26`/`0x2E`, requiring the trapping flat handlers. | Certain when Seadramon is drawn; optional progression. |
| Curling: `kar/kar.c:927,932`, `KAR_renderScene` | `KARRING.TMD` objects 6–8 contain untextured lit Gouraud quads, mode `0x38`, requiring stub `GsTMDfastG4L`. | When those type-2 stone models are drawn. |

**Fix direction:** implement and oracle-check the required handlers, mode variants and object attributes in `crates/psyq`. Preserve the game’s rendering choices.

There is one additional **conditional SDK blocker**:

- `std/std_effect.c:2939`, `STD_addClutLoadPrim`
- `vs/vs_effect.c:2522`, `VS_addClutLoadPrim`

EFE subopcode `0x50` calls the trapping `SetDrawLoad`. The stop is definite if invoked; this audit did not establish which tournament/Battle Mode moves execute it.

**10. Tournament commands use PS1 byte offsets into expanded position records — class 1**

- [std/std_main.c:4997](/Users/fenn/Developer/dw1-silicon/decomp/src/std/std_main.c:4997)
- `std/std_main.c:5012`
- Function: `STD_handlePartnerMoveCommand`

Both form a location pointer using `posData + 0x78` bytes. Clang confirms that native `PositionData.location` is at **`0x98`**, with total structure size 168 bytes.

**Consequence:** `entityLookAtLocation` receives pointer/scale storage interpreted as coordinates. This can produce incorrect facing and battle movement behavior.

**When/likelihood:** command 6, or command 5 after sufficient distance has been reached; high likelihood for players using these commands.

**Fix direction:** use `&posData->location`, matching the typed accesses already used elsewhere.

**11. Tournament and Battle Mode finishers read an uninitialized move ID — class 7**

- [std/std_main.c:6644](/Users/fenn/Developer/dw1-silicon/decomp/src/std/std_main.c:6644), `STD_startQueuedMove`
- [vs/vs_main.c:840](/Users/fenn/Developer/dw1-silicon/decomp/src/vs/vs_main.c:840), `VS_startFighterMove`

`tech` is assigned only when `NO_AI_FLAG == 0`. On a subsequent call for `FINISHING_ENTITY`, when the finisher timer has expired, execution can reach `MOVE_DATA[tech]` without assigning it.

**Consequence:** arbitrary move-table indexing, incorrect flags, or an invalid read. The retail STD listing uses incoming `$s3` on this path; native stack/register allocation cannot preserve that implicitly.

**When/likelihood:** executing finishers in STD/VS; substantial conditional risk.

**Fix direction:** reconstruct an explicit caller/state-derived value, following the missing-return fixes. Recomputing the queued technique is a possible native fix, but its equivalence should be checked against the emulator.

**Measured for STD (cup stream, 2026-10-15).** The `cup` recording reaches it: the first opponent's
ranged finisher, once its 0x50-frame charge is spent, comes back through
`STD_tickDigimonAttackRanged` into this path. `oracle/probe-std-tech.lua` breaks at 0x8006569C (the
`MOVE_DATA[tech]` load, `sll $v1, $s3, 4`) on a replay of the recording: one hit, VBlank 10904, the
enemy (`$s0` 0x80155828) called from 0x80061FE0, `$s3` = 0x57, its `move` argument (the finisher's
technique; `MOVE_DATA[0x57].unk3` is 1, so no timer is set). In the disassembly `$s3` is the
caller's: `STD_tickDigimonAttackRanged` and `STD_tickQueuedMove`/`STD_tickConfusedState` copy their
fourth argument (`move`, `arg3`) into it, and `STD_tickAttackHits` holds the attacker's address there.
Natively `STD_startQueuedMove` now takes that value from each caller (`callerS3`) and uses it as
`tech`; the `STD_tickAttackHits` call still stops (an address), as does a value outside
`MOVE_DATA`. `cup` is identical natively at all 33 standard and 259 dense checkpoints with it.
`VS_startFighterMove` keeps its stop: no recording reaches it (Battle Mode's finishers), so its
callers' `$s3` is unmeasured.

**12. Fishing appends native object records at insufficient alignment — class 8**

- [fish/fish_model.c:31](/Users/fenn/Developer/dw1-silicon/decomp/src/fish/fish_model.c:31), `FISH_loadTMDModel`
- `fish/fish_model.c:47`: passes those records to Rust `dw1_GsLinkObject4`.

The file end is rounded only to four bytes. `SAO.TMD` is 1,660 bytes, and the actual rod buffer is eight-byte aligned. Its appended `GsDOBJ2` array therefore starts at **four modulo eight**, although native `GsDOBJ2` requires eight-byte alignment.

**Consequence:** misaligned C object access and a violated Rust pointer-alignment requirement. Debug raw-pointer checks can stop the game; a release build tolerating the hardware access still has undefined behavior.

**When/likelihood:** loading the fishing rod; definite misalignment, build-dependent failure.

**Fix direction:** align the appended array to `_Alignof(GsDOBJ2)` and include that padding in the capacity check, or use separate native storage. Capacity itself is sufficient: both fishing models fit in their 4,096-byte buffers with native records.

**13. Ending particles read an uninitialized word beyond a matrix — classes 7, 8**

- [endi/endi.c:317](/Users/fenn/Developer/dw1-silicon/decomp/src/endi/endi.c:317), `ENDI_releaseParticles`

`MATRIX.t` has three elements. `baseMatrix.t[3]` reads beyond the matrix, before `boneMatrix` has been initialized.

**Consequence:** the released particles’ base Z depends on native stack contents and optimization. This is primarily a visual divergence, with undefined-behavior implications.

**When/likelihood:** frame 80 of the ending sequence; definite invalid read.

**Fix direction:** measure/reconstruct the retail stack value and represent it explicitly natively. The retail listing also reads the adjacent stack word, so simply changing all three accesses to `t[0..2]` would silently alter behavior.

**14. Battle Mode intro consumes an uninitialized previous-pad value — class 7**

- [vs/vs_intro.c:399](/Users/fenn/Developer/dw1-silicon/decomp/src/vs/vs_intro.c:399), `VS__runIntro`

The first iteration tests `pad & ~prev` before assigning `prev`.

**Consequence:** the first confirm edge can be lost or interpreted inconsistently, affecting intro skipping.

**When/likelihood:** each intro’s first polling iteration; likely minor rather than a playthrough stop.

**Fix direction:** supply an explicit initial previous-input value based on the existing input state and emulator behavior, without introducing an extra replay-visible poll.

**15. Death-shard initialization clears only half of its native pointer sentinel — classes 1, 8**

- [dooa/dooa.c:651](/Users/fenn/Developer/dw1-silicon/decomp/src/dooa/dooa.c:651), `DOOA_initShardEffect`
- `dooa/dooa.c:1341`, `DOOA_updateShards`

Initialization writes zero through `int32_t *`; traversal reads the sentinel through `TMD_P_TG4 **`, eight bytes natively.

**Consequence:** if the reused buffer’s upper four bytes are nonzero, an empty shard list appears nonempty and traversal interprets old bytes as a shard. A freshly zeroed arena can hide this.

**When/likelihood:** buffer reuse during death sequences; conditional but genuine width mismatch.

**Fix direction:** initialize a complete pointer sentinel, matching the pointer-sized termination store already used at line 1694. Use the same sentinel type consistently in rendering and updating.

**16. Three consumed missing-return functions remain unfixed — class 6**

These agree with the outstanding sites in `docs/phase5-returns-audit.md`.

| Location / function | Trigger and native consequence | Existing audit’s fix direction |
|---|---|---|
| `std/std_main.c:1582`, `STD_getEntityIndex` | Missing entity match; undefined returned index used for placement/intro operations. Expected table membership makes this low likelihood normally. | Native fallback argument: placement X or camera Y, depending on caller. |
| `std/std_main.c:5482`, `STD_getEntityIndex2` | Missing entity match; undefined result stored in combat entity IDs. Low likelihood with intact setup. | Native return `(int16_t)0xD640`, preserving the documented caller’s stored byte. |
| `vs/vs_scene.c:1545`, `VS_getFirstSpecialMove` | Species has no qualifying special move; undefined move stored by `VS_loadFighterEntities` at lines 1244/1260. | Pass the respective fighter’s `moves[2]` as the native fallback. |

These are confirmed missing returns; **ordinary reachability is less certain than the blockers above**. I found no additional consumed missing-return site beyond the prior audit.

**Suspicious patterns and patterns that appear safe**

| Locations / functions | Assessment and suggested direction |
|---|---|
| `evl/evl.c:457–485`, `EVL_tickEvoSequence` | Reads four-byte `timer` through `uint8_t **`, fetching eight bytes natively. The upper bytes are currently padding in a zero-initialized global, so this probably behaves correctly today. Replace the pun with `data->timer` under `DW1_NATIVE`; do not count it as a demonstrated camera failure. |
| `std/std_main.c:2827,2842`, `STD_startCameraIntro`; `vs/vs_camera.c:728` and the corresponding distance-table loop, `VS_startCameraIntro` | Index beyond declared four-element arrays into separately declared adjacent arrays. The inspected native binary retains the expected adjacency, but this remains C bounds UB and a linker-layout dependency. Native combined 22-element storage/accessors would remove it. |
| `trn2/trn2_hud.c:262`, `TRN2_calculateTrainingMultiplier` | Reads uninitialized `result` and `t`, but immediately overwrites `result` with the real calculation. Formally UB; no useful result depends on it. Native removal of this dead calculation is appropriate. |
| `std/std_main.c:3616`, `STD_tickBracketProjectile` | `d` is uninitialized for match 7+. Projectiles are created for simulated matches; ordinary final-match flow appears to avoid this path. Verify before choosing a fallback. |
| `kar/kar.c:2620`, `KAR_setOpponentShot` | `stone` is uninitialized if no unused stone is found. Both shot-plan rows cover all four stone types, so normal remaining-stone invariants appear to prevent it. A native invariant trap would make failures diagnosable. |
| `evl/evl.c:1437`, `EVL_applyEvolution`; STD/VS particle and stun renderers | Uninitialized warnings arise for unsupported primary-element/type/state values. Normal expected domains cover the initialized branches. No ordinary failing input was established. |
| STD/VS EFE division/modulo/shift helpers, e.g. `std/std_effect.c:5253–5270`, `vs/vs_effect.c:4838–4855` | Zero divisors, oversized shift counts and `INT_MIN / -1` remain potential MIPS/C mismatches. No triggering retail script was established. Use explicit MIPS semantics or native traps for unobserved inputs when these paths are measured. Do not assume native C division by zero necessarily produces a hardware trap. |
| `fish/fish.c:3057`, `FISH_findFishToBite` | Literal stride 56 is correct: `Fish` contains no pointers and remains 56 bytes natively. |
| `std/std_main.c:2830`; `vs/vs_camera.c:731`, calls to `initializeSomeArenaArrays` | Addresses narrow into 32-bit arguments, but those two parameters are unused by `main/effect_common.c:612`. No truncation consequence here. |
| `dooa/dooa.c:1103`, call to `reincarnatePartner` | The narrowed first argument is explicitly unused by `main/evolution.c:1054`. |
| STD/VS camera `view.super` assignments | These call implemented `GsSetView2`, whose parent-coordinate path exists. They do not establish a hit on `GsSetRefView2`’s unsupported-parent trap. |
| Initialized BSS records containing numeric “pointers” | Many contain old disc/debug bytes. Checked initialization paths replace or invalidate the relevant records before use; their appearance alone is not evidence of a live invalid pointer. |

**Coverage by bug class**

`F` = confirmed unresolved finding above. `S` = suspicious/conditional pattern only. `—` = checked; no additional unresolved problem found in that class. This is static-audit coverage, not emulator certification.

| Overlay | 1 Sizes/offsets | 2 File/table pointer records | 3 Unsigned address wrap | 4 Truncation | 5 NULL/low addresses | 6 Consumed missing returns | 7 Uninitialized | 8 MIPS/layout/alignment | 9 SDK stops |
|---|---|---|---|---|---|---|---|---|---|
| std | F | — | — | — | — | F | F | S | F |
| vs | F | — | — | — | — | F | F | S | F |
| fish | — | — | — | — | — | — | — | F | F |
| evl | F | F | — | F | — | — | S | F | — |
| doo2 | F | F | — | F | — | — | — | F¹ | — |
| dooa | F | — | — | F | — | — | — | F | — |
| kar | — | — | — | — | — | — | S | S | F |
| dget | — | — | — | — | — | — | — | — | — |
| trn2 | — | — | — | — | — | — | S | — | — |
| eab | F | — | — | — | — | — | — | F | — |
| endi | — | — | — | F | — | — | F | F | — |

¹ DOO2 receives the untranslated scratch address from DOOA.

In particular:

- **Class 3:** no new unsigned-wrap address/index defect found in any requested overlay. The risky animation-minus-`0x2e` sites in STD/VS already use `DW1_WRAP_INDEX`; remaining unsigned indexes inspected were bounded or reduced modulo table length.
- **Class 5:** no new confirmed NULL/small-address dereference found. Optional NULL arguments inspected were guarded or otherwise supported.
- **Class 6:** the previously documented unused returns in FISH, KAR, DOOA and EFE arithmetic dispatch remain unused.
- **Class 2:** STD/VS already use the EFE side tables. Their loaded-record issue is not still open; their opcode-table validation is a separate native-stride defect.
- **Class 9:** no additional direct trapping stub was found beyond `dw1_exit` and `SetDrawLoad`; the significant remaining surface is indirect TMD-handler dispatch and sorter restrictions. The rod’s `SAO.TMD` uses supported Gouraud textured kinds, and EAB’s `BOSS_EFE.TMD` uses supported flag-1/mode-`0x2F` quads.