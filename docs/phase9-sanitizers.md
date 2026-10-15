# Phase 9: sanitizer runs of the certified replays (2026-10-15)

UBSan and AddressSanitizer builds of the game C (`DW1_SANITIZE=undefined|address`,
`crates/decomp-c/build.rs`; link the runtime to the final binary only, with `cargo rustc --release
-p dw1 --bin dw1 -- -C link-arg=<libclang_rt...dylib> -C link-arg=-Wl,-rpath,<its dir>` and a
separate `CARGO_TARGET_DIR`) ran all twelve replays; gpt-6.1-sol traced every report to the PS1
layout. Fixed in decomp f5decbe (Astra reviewed twice); still open, accepted as low risk because
arm64 tolerates them: the ItemMenuBox arena alignment and one EfeAura alignment. With the fixes,
the replays stay identical and run to the end under ASan (before, the tamer span crashed it).

## UBSan analysis (gpt-6.1-sol)

**The priority sites are the two out-of-bounds writers: `BTL_BUFF_TRAILS` and `MAP_CLUTS`.** The reported reads mostly reach fields whose layouts remain unchanged, values that cannot affect the subsequent decision, or drawing constants. Neither NULL report actually reads low RAM.

One additional issue surfaced in the alignment groups: **`ItemMenuBox` needs 40 bytes natively but receives 32**, allowing its row data to overwrite the following allocation’s header.

This investigation was read-only. I checked the requested architecture documents, native-fix conventions, retail assembly, symbol/layout files, and bytes in the extracted US executable and assets. Clang checks through stdin confirmed the relevant native structure sizes. I did not run new gameplay or emulator comparisons; the replay results below are those supplied in the request.

## Ranked non-misaligned sites

Ranking reflects remaining potential to change gameplay, not a demonstrated replay divergence.

| Rank | Reports | PS1 access | Native consequence and assessment |
|---|---|---|---|
| 1 | `battle_effect.c:6458` | Writes eight frame halfwords through a one-element `BTL_BUFF_TRAILS` array | Expanded stride writes into unrelated native storage. **High concern**, even though the PS1 collateral writes are harmless during normal initialization. |
| 2 | `map.c:4240` | Fourth/fifth `MAP_CLUTS` stores overwrite `CAMERA_TARGET.vx/vy` | Eight-byte native stores have different collateral effects. **Conditional gameplay risk** on three maps. |
| 3 | `battle_main.c:2116,2163` | `moves[-10]` reads the radius high byte | Controls counterattack behavior, but the relevant record layout and bytes are unchanged. **Low current divergence risk; remove bounds UB explicitly.** |
| 4 | `script_menu.c:2930`, both columns | Reads two halfwords from `NAMING_ROLLOVER_CHARS`: `0x8000,0x8000` | Native adjacency is unspecified. **Drawing divergence possible; no direct gameplay-state dependency.** |
| 5 | `overworld.c:490,530,540` | Reads `y` or the first two animation-duration bytes | Same native fields; an explicit sentinel/bound condition forces the outcome regardless. **Semantically harmless reads.** |
| 6 | `partner_impl.c:713` | `hungerTimes[8]` reads `energyCap` | Same native field; `i == 7` forces the branch anyway. **Semantically harmless.** |
| 7 | `battle_effect.c:6469` | Reads an uninitialized stack halfword beyond `pool[8]` | Copied only into a slot excluded from every subsequent selection. **Dead value; harmless to the generated permutation.** |
| 8 | `battle_setup.c:230`; `btl.c:387` | Forms a NULL first-member address, then calls a NULL-aware predicate | No PS1 memory access occurs. **Harmless intent expressed using undefined C member access.** |

Proposed changes below should be guarded by `DW1_NATIVE`, preserving the retail build.

## Non-misaligned details

### 1. `BTL_initializeBuffTrails`: out-of-bounds stores

**Location:** [battle_effect.c:6437](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_effect.c:6437), report at line 6458.

**Object and path.** `BTL_BUFF_TRAILS` contains one `EfeTrailEffect`, but initialization writes `.frame = -1` for indices **0–7**, including invalid indices **1–7**. Every normal battle-data initialization calls it through `BTL_initializeUnk3`; [btl.c:429](/Users/fenn/Developer/dw1-silicon/decomp/src/main/btl.c:429) then immediately calls `BTL_initializeUnk2`. Both battle replays hit it.

**PS1 evidence.** [symbols_btl.txt:467](/Users/fenn/Developer/dw1-silicon/decomp/config/us/symbols_btl.txt:467) places the array at `0x80075CA0`, size `0x12C`. Retail assembly uses exactly that stride and stores `FF FF` at:

| Index | PS1 address | Destination |
|---|---|---|
| 0 | `0x80075CA0` | Actual trail’s frame |
| 1 | `0x80075DCC` | `BTL_BUFF_TRAIL_A_X[0]` |
| 2 | `0x80075EF8` | `BTL_BUFF_RING_SPARKS[16].vx` |
| 3 | `0x80076024` | Beyond the BTL image |
| 4 | `0x80076150` | Beyond the BTL image |
| 5 | `0x8007627C` | Beyond the BTL image |
| 6 | `0x800763A8` | Beyond the BTL image |
| 7 | `0x800764D4` | Beyond the BTL image |

The BTL splat configuration and actual file both end at **`0x80075F7C`**. The initialized data preceding that end includes:

- `BTL_BUFF_TRAIL_A_X`, `0x80075DCC`, size `0x10`.
- `BTL_BUFF_RING_SPARKS`, `0x80075E78`, size `0x100`.
- The overlay trailer after the sparks.

This agrees with [overlay.ld:3](/Users/fenn/Developer/dw1-silicon/decomp/config/us/overlay.ld:3) and [btl.yaml](/Users/fenn/Developer/dw1-silicon/decomp/config/us/btl.yaml).

These are stores, so their previous bytes do not determine the result. For completeness, the file initially contains `00 02` at the index-1 destination and `F8 18` at index 2; these become runtime effect state.

**Why the PS1 writes are harmless here.** Index 1 is overwritten by this function’s subsequent randomized trail-coordinate initialization. Index 2 is overwritten by the immediately following `BTL_initializeUnk2`, which initializes the spark coordinates. Indices 3–7 land outside allocated BTL objects; the inspected linker/buffer layout does not assign live battle objects there.

**Native behavior.** Clang confirms `sizeof(EfeTrailEffect) == 312`, or **`0x138`**, instead of `0x12C`. The invalid stores therefore occur at native base plus `0x138*i`, reaching unrelated native storage. The PS1 explanation does not justify those stores natively.

**Fix.** Under `DW1_NATIVE`, initialize only the real element, `BTL_BUFF_TRAILS[0].frame`. Leave the coordinate generation and RNG calls unchanged. Enlarging this array to eight would prevent corruption, but seven unused native records are unnecessary.

### 2. `setupMap`: extra CLUT stores reach camera state

**Location:** [map.c:4225](/Users/fenn/Developer/dw1-silicon/decomp/src/main/map.c:4225), report at line 4240.

**Object and range.** `MAP_CLUTS[3]` receives indices `0 .. numCluts-1`. Inspecting all **242 TFS files** found:

| CLUT count | Files |
|---|---|
| 1 | 109 files |
| 3 | 130 files |
| 4 | `MIST02.TFS`, map **116 / `0x74`** |
| 5 | `MAYO01_2.TFS`, map **110 / `0x6E`**; `TWNA02.TFS`, map **168 / `0xA8`** |

Thus the actual invalid indices are **3 and 4**. The supplied reports occur on care, death/evolution debug, keeper, and registration routes.

**PS1 evidence.** [symbols.txt:5233](/Users/fenn/Developer/dw1-silicon/decomp/config/us/symbols.txt:5233) gives:

```text
MAP_CLUTS     0x80150C70  size 0x0C
CAMERA_TARGET 0x80150C7C  size 0x10
CLOCK_SPRITE  0x80150C8C
```

`bss.yaml` and `main.ld` place these in resident map BSS. Consequently:

- `MAP_CLUTS[3]` aliases `CAMERA_TARGET.vx`.
- `MAP_CLUTS[4]` aliases `CAMERA_TARGET.vy`.

`GENERAL_BUFFER_PTR` is initialized to PS1 address `0x80010000`; no reassignment was found. CLUT `i` starts at buffer offset `8 + 512*i`. The exact collateral stores are therefore:

| Index | Stored PS1 word | Little-endian bytes |
|---|---|---|
| 3 | `0x80010608` | `08 06 01 80` |
| 4 | `0x80010808` | `08 08 01 80` |

These are **runtime pointer values written into runtime camera state**, not constant camera coordinates.

**Native behavior and significance.** Native stores are eight bytes, and their destinations beyond the array are unspecified. They can corrupt more than the two PS1 camera words.

Only CLUT entries **0–2** are subsequently read by the palette/daytime code. Camera consumers include `tickCameraMovement`; ordinary camera entry points assign target coordinates before scheduling it. I found no demonstrated route consuming these poisoned PS1 coordinates, but the arbitrary native writes remain unjustified.

**Fix.** Preserve every `LoadClut`, store pointers only for indices 0–2, and explicitly reproduce the PS1 collateral writes for indices 3–4:

- Index 3: `CAMERA_TARGET.vx = (int32_t)0x80010608`.
- Index 4: `CAMERA_TARGET.vy = (int32_t)0x80010808`.

Trap on an unsupported larger count. This models the known retail behavior without depending on native adjacency. A five-element pointer array alone would remove the PS1 camera side effects.

### 3. `BTL_tickAttackHits`: negative move-table lookup

**Locations:** [battle_main.c:2116](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_main.c:2116) and [battle_main.c:2163](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_main.c:2163).

**Object and paths.** Both evaluate:

```c
DIGIMON_DATA[entity->type].moves[
    DW1_WRAP_INDEX(entity->anim.animId - 0x2e)]
```

The first check runs when a living fighter has a queued incoming attack. The second is reached after a miss, for an eligible unblocked melee attack, before searching for a counterattack move.

The supplied observed index is **−10**, corresponding to animation **`0x24`**. Normal battle animation states in the `0x20–0x2D` band produce **−14 through −1**; attack animations `0x2E–0x3D` produce valid indices 0–15. There is no local bounds guard: the full `uint8_t` animation domain mathematically permits −46 through 209, although those extremes are not established gameplay inputs here.

**PS1 evidence.** `DIGIMON_DATA` is at **`0x8012CEB4`**, size **`0x2490`**, comprising 180 records of **52 bytes**. Its declaration is [params.h:18](/Users/fenn/Developer/dw1-silicon/decomp/src/../include/dw/params.h:18):

```text
radius: offset 24
height: offset 26
type:   offset 28
level:  offset 29
moves:  offset 35, length 16
```

Therefore `moves[-10]` is byte **25**, the **high byte of `radius`**, at:

```text
0x8012CEB4 + 52*species + 25
```

Retail radii yield only **`00` or `01`** in that byte. For example, Agumon’s radius is 120, bytes `78 00`; this lookup reads `00`.

More generally, the negative normal-battle band reads existing fields in the same record. An exhaustive check of the retail records found **no `0x2D` byte anywhere in that band**.

**Native behavior.** Clang-generated bindings confirm the native record remains **52 bytes**, with the same offsets. It contains no pointers. The reported −10 access consequently reaches the same radius byte, rather than an unrelated native symbol.

**Effect and fix.** The result matters: comparison with `0x2D` controls counterattack handling. However, this reported access has **no identified PS1/native byte mismatch**.

Use a native accessor that reads the appropriate byte from the object representation of the **whole `DIGIMON_DATA` array**, with a checked signed byte offset. This preserves unusual valid cross-record reads too. Trap if the computed address leaves the modeled table; do not silently substitute a move value for a potentially runtime-state neighbor. Simply rejecting every negative index would discard the retail behavior without documenting why it is equivalent.

### 4. `renderNamingBox`: two constant halfwords beyond the table

**Location:** [script_menu.c:2898](/Users/fenn/Developer/dw1-silicon/decomp/src/shop/script_menu.c:2898), both reports at line 2930.

**Object and range.** US `NAMING_LABELS` is 3. The loop consumes table entries 0–8, leaving `lbl == &NAMING_CTRL_BOXES[9]`. The final call reads entries **9, 10 and 11**; **10–11** exceed the ten-element array.

This executes while drawing tamer/partner naming screens, including the supplied new-game, walking, and debug-death routes.

**PS1 evidence.** [symbols.txt:3655](/Users/fenn/Developer/dw1-silicon/decomp/config/us/symbols.txt:3655) and the retail data listing establish:

```text
NAMING_CTRL_BOXES     0x8013078C  size 0x14
NAMING_ROLLOVER_CHARS 0x801307A0
```

The three final values are:

| Table index | Address | Bytes | Signed value |
|---|---|---|---|
| 9 | `0x8013079E` | `00 00` | 0 |
| 10 | `0x801307A0` | `00 80` | −32768 |
| 11 | `0x801307A2` | `00 80` | −32768 |

These are constant initialized data, also visible at [script_menu.c:469](/Users/fenn/Developer/dw1-silicon/decomp/src/shop/script_menu.c:469).

**Native behavior and significance.** The two out-of-object halfwords are unspecified. They become the drawing call’s Y offset and width. [utils2.c:267](/Users/fenn/Developer/dw1-silicon/decomp/src/main/utils2.c:267) emits a fixed number of drawing primitives; these arguments do not control naming selection, buffer contents, or game rules.

**Fix.** Give the US native table **12 entries**, appending `(int16_t)0x8000` twice. Preserve the odd retail drawing parameters rather than replacing them with plausible dimensions.

### 5. `renderMapOverlays`: neighboring fields read before forced checks

**Locations:** [overworld.c:490](/Users/fenn/Developer/dw1-silicon/decomp/src/main/overworld.c:490), [overworld.c:530](/Users/fenn/Developer/dw1-silicon/decomp/src/main/overworld.c:530), and [overworld.c:540](/Users/fenn/Developer/dw1-silicon/decomp/src/main/overworld.c:540).

**Object and paths.**

- Line 490 reads `animSprites[-1]` for inactive instances whose `currentFrame == -1`. All twelve replays report it.
- Lines 530/540 read `animSprites[8]` when an eight-frame animation advances from frame 7. They are the drawn and hidden/placeholder branches of the same animation update.
- The maintained frame domain is inactive **−1**, active **0–7**, plus temporary **8** immediately before wrap.

**PS1 evidence.** [map.h:36](/Users/fenn/Developer/dw1-silicon/decomp/include/dw/map.h:36) defines a 34-byte record:

```text
y             offset 4
animSprites   offset 6, 16 bytes
animTimes     offset 22, 8 bytes
```

Thus:

- `animSprites[-1]` reads the runtime **`y`** halfword.
- `animSprites[8]` reads bytes **`animTimes[0], animTimes[1]`**, interpreted as one little-endian halfword.

These are fields inside `MAP_TILE_DATA`, at `0x8014F084`, size `0x1BBC`; the PS1 instance array begins at `0x8014F340`, with stride 34. Map loading supplies the coordinates/durations, and inactive-record initialization supplies the sentinel. There is no single constant byte value.

**Native behavior and significance.** The individual record remains 34 bytes with identical field offsets, despite its enclosing structure moving because tile pointers grow.

Neither loaded value can affect the decision:

```c
animSprites[-1] == -1 || startFrame == -1
animSprites[8]  == -1 || currentFrame >= 8
```

The right operand is necessarily true for each reported invalid access.

**Fix.** Put the sentinel/bounds operand first:

```c
startFrame == -1 || instances->animSprites[startFrame] == -1
instances->currentFrame >= 8 ||
    instances->animSprites[instances->currentFrame] == -1
```

Apply the second change at both sites. This removes the reads without changing timer advancement or animation state.

### 6. `setFoodTimer`: the ninth hunger byte is `energyCap`

**Location:** [partner_impl.c:713](/Users/fenn/Developer/dw1-silicon/decomp/src/main/partner_impl.c:713). The analogous inner-loop expression at line 725 deserves the same fix.

**Object and paths.** `i` runs 0–7; the lookahead uses `hungerTimes[i+1]`, indices **1–8**. It exceeds the array only at **i = 7**, when searching or wrapping a complete hunger schedule. Calls occur during partner initialization, care/feeding updates, evolution-related initialization, and script updates.

The retail species with eight populated slots are:

| Species | Hunger hours | Following byte |
|---|---|---|
| Numemon | 0,3,6,9,12,15,18,21 | `50` hex |
| Gabumon | 2,5,8,11,14,17,20,23 | `28` hex |
| Sukamon | 0,3,6,9,12,15,18,21 | `50` hex |
| Digitamamon | 0,3,6,9,12,15,18,21 | `50` hex |

**PS1 evidence.** `RAISE_DATA` is **`0x801225BC`**, size **`0x738`**: 66 records of 28 bytes. [partner.h:6](/Users/fenn/Developer/dw1-silicon/decomp/include/dw/partner.h:6) puts `energyCap` immediately after `hungerTimes[8]`, at offset 8. The invalid read is therefore:

```text
0x801225BC + 28*species + 8
```

The initial table’s energy-cap bytes are `00`, `19`, `28`, or `50`, depending on species.

**Native behavior, effect and fix.** The pointer-free record has the same layout natively. Moreover, `i == 7` makes the OR expression true regardless of that byte. Put `i == 7` first, and `j == 7` first in the inner loop. Hunger timing remains unchanged.

### 7. `BTL_initializeBuffTrails`: stack-pool overread

**Location:** [battle_effect.c:6469](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_effect.c:6469).

**Object and range.** `pool[8]` holds a shrinking permutation. At outer iteration `i`, `r` is `0 .. 7-i`, and the shift reads indices `r+1 .. 8-i`. The only actual array overread is **`pool[8]`**, during **i = 0**, at the shift’s final iteration. Both battle replays hit it.

**PS1 evidence.** The retail listing places `pool` at **`sp+0x28`**. Its ninth halfword, **`sp+0x38`**, is the storage later initialized as **`rot.vx`**. That halfword is uninitialized at this point: its exact bytes are runtime stack residue, not a linker symbol or constant.

**Native behavior and significance.** Native stack adjacency is unspecified, but the fetched value is written only into the last slot of the current active pool. That slot is excluded by the next iteration’s smaller random-selection bound. The residue propagates only through similarly discarded end slots; it never becomes selected `k`, a rotation, an RNG bound, or game state.

**Fix.** Under `DW1_NATIVE`, shift only while `j + 1 < 8 - i`. This removes the dead final copy and preserves all RNG calls and selected values. No stack-value reconstruction or native stop is needed.

### 8. NULL member-address expressions in battle setup

**Locations:**

- [battle_setup.c:230](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_setup.c:230), `BTL_handleBattleIntro`.
- [btl.c:387](/Users/fenn/Developer/dw1-silicon/decomp/src/main/btl.c:387), `loadBattleData`.

Both scan `ENTITY_TABLE` indices **2–9**, including empty NPC slots, and form an address such as `&npc->digimonEntity.entity` before calling `isInvisible`.

**PS1 evidence.** Each referenced member is the first member of its containing structure, so the expression passes address zero when the slot is NULL. Crucially, [retail `isInvisible` assembly](/Users/fenn/Developer/dw1-silicon/decomp/asm/us/main/utils2.s:773) begins:

```text
beqz a0, invisible
nop
lb   v0, 0x35(a0)
```

It returns “invisible” before loading anything when the argument is zero. The C implementation retains that guard at [utils2.c:389](/Users/fenn/Developer/dw1-silicon/decomp/src/main/utils2.c:389).

**Native behavior, effect and fix.** UBSan reports undefined member-address formation, not an actual NULL load. There are **no low-RAM bytes to reproduce**. Add an explicit non-NULL guard before forming the member address at each caller. Empty slots remain skipped.

## Misaligned groups

Ordinary arm64 loads/stores tolerate these unaligned addresses, so **alignment alone ranks below the out-of-bounds writers**. The C accesses remain undefined, and hardware tolerance does not make misaligned pointers valid for Rust typed access.

### EfeSlot arena: all reported battle-effect accesses, plus `main/efe.c:1349`

**Origin:** [battle_effect.c:2170](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_effect.c:2170), `BTL_initializeEFEEngine`.

The engine carves sixteen native `EfeSlot` records out of `GENERAL_BUFFER_PTR`. It advances the cursor using:

```c
base += 4 - ((intptr_t)base & 3);
```

That guarantees only four-byte alignment and **adds four even to an already aligned address**. The normally aligned general buffer therefore produces a slot table at four modulo eight.

Clang confirms native `EfeSlot` is **88 bytes, aligned to 8**. Its stride preserves the initial misalignment, explaining the reports throughout initialization, interpretation, unloading, and rendering at `battle_effect.c:1485–4803`.

The [efe.c:1349](/Users/fenn/Developer/dw1-silicon/decomp/src/main/efe.c:1349) store is the same defect: `dw1EfeSubEffects` writes through `end`, supplied as a field address inside the misaligned slot. Its static native subeffect table is not the source of this misalignment.

**Fix:** carve the table at `_Alignof(EfeSlot)`, reserve `16 * sizeof(EfeSlot)`, and align subsequent pointer-bearing allocations to their own requirements. Alternatively, use a dedicated typed native slot array with the existing initialization/lifetime behavior. Account for padding in arena capacity calculations.

### ItemMenuBox: `script_common.c` and `shop/script_menu.c`

**Origin:** [script_common.c:1729](/Users/fenn/Developer/dw1-silicon/decomp/src/main/script_common.c:1729), `allocateItemMenuBox`, and [script_common.c:3111](/Users/fenn/Developer/dw1-silicon/decomp/src/main/script_common.c:3111), `allocateArray`.

The allocator rounds sizes to four bytes and returns storage immediately after a **four-byte allocation header** in the texture-buffer arena. It cannot guarantee the eight-byte alignment required by native `ItemMenuBox`. Both source files operate on these same allocated boxes; they are one storage problem.

**Additional size defect:** the allocation requests literal **`0x20`**. Clang confirms:

```text
sizeof(ItemMenuBox)          = 40
offsetof(ItemMenuBox,itemRow) = 28
```

With six visible rows, initialization writes `itemRow[4]` and `[5]` at offsets **32–33**, beyond the allocated payload. The next allocation’s size header begins at offset **32**, so these writes corrupt it. Later `freeArray` reads that header and updates `ARRAY_SECTION_OFFSET`. This can affect subsequent allocations and menu behavior even on hardware that tolerates every misaligned access.

**Fix:** request `sizeof(ItemMenuBox)` under `DW1_NATIVE` and provide aligned native allocation. If changing the arena allocator, update allocation and freeing together so padding/header bookkeeping remains correct. **Treat the size overrun as high priority independently of the lower alignment severity.**

### Sound sequence-header loads

**Locations:** [sound.c:480](/Users/fenn/Developer/dw1-silicon/decomp/src/main/sound.c:480) and [sound.c:485](/Users/fenn/Developer/dw1-silicon/decomp/src/main/sound.c:485), `seqOpen`.

`SEQ_BUFFER` is declared as `uint8_t[0x5800]`, then cast to `uint32_t *`. Its declaration guarantees only byte alignment; native placement need not preserve the PS1 address **`0x80148574`**, which is four-byte aligned.

The loads read the sequence-offset table’s first word and the selected track’s offset. The bytes themselves are unchanged; there is no neighboring-object dependency in these reports.

**Fix:** give native `SEQ_BUFFER` explicit four-byte alignment, or read the words with `memcpy` into aligned `uint32_t` locals. Alignment alone is a **low gameplay-divergence risk** here.

## AddressSanitizer analysis (gpt-6.1-sol)

**Read-only ASan investigation**

Cases **1 and 4 share a real PS1 storage span** that the native build currently represents as separate arrays. Case **3 is sector padding spilling into a table that is immediately replaced**. Case **2 is a downstream effect of the previously understood buff-trail write**, rather than an independent overrun of the layout table.

I inspected the source, retail assembly listings, US configurations, existing ASan logs, native Mach-O symbols/data, and extracted retail files. I also compared the relevant extracted files with sector payloads read directly from the disc image. **No files were changed, and no new gameplay or replay run was performed.** Native addresses below describe the inspected artifacts; the working tree already contains some newer fixes.

**1. `getTileTrigger`: the reported access is before the collision buffer**

`getTileTrigger` converts world coordinates into a nominal 100×100 grid index:

```c
tx = x / 100 + 50;       /* with the original negative-coordinate adjustment */
tz = 50 - z / 100;      /* likewise */
index = tx + tz * 100;
```

It then reads the byte without checking the index. Zero means ordinary ground, `0x80` means impassable, and other bytes become signed trigger values. Both the C and retail listing establish this behavior: [map_collision.c:32](/Users/fenn/Developer/dw1-silicon/decomp/src/main/map_collision.c:32), [getTileTrigger.s:38](/Users/fenn/Developer/dw1-silicon/decomp/asm/us/main/matchings/map_collision/getTileTrigger.s:38).

There are two important consumers:

- `renderDropShadow` suppresses the shadow only when the result is `-1`: [drop_shadow.c:37](/Users/fenn/Developer/dw1-silicon/decomp/src/main/drop_shadow.c:37).
- The tamer uses trigger values for warps and scripted interactions: [tamer.c:688](/Users/fenn/Developer/dw1-silicon/decomp/src/main/tamer.c:688).

The recorded battle failure is the first path: `BTL_deinitializeCombat → BTL_battleTickFrame → renderDigimon → renderDropShadow → getTileTrigger`. ASan reports **index `-44`**, not a read after the array: [battle log:927](/Users/fenn/Developer/dw1-silicon/captures/native-runs/asan-battle-fixture/log.txt:927). Battle-end animation can move enemies offscreen before their render objects disappear: [battle_main.c:962](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_main.c:962).

The relevant PS1 layout is:

| Object | PS1 address | Reserved span |
|---|---:|---:|
| `TAMER_MODEL_BUFFER` | `0x801A1398` | `0x7800` |
| `MAIN_D_801A8B98` | `0x801A8B98` | `0x6800` |
| `MAP_COLLISION_DATA` | `0x801AF398` | `0x2800` |
| `STDVS_CAMERA` | `0x801B1B98` | `0x8C` |

Evidence: [symbols.txt:5313](/Users/fenn/Developer/dw1-silicon/decomp/config/us/symbols.txt:5313), [main.c:3328](/Users/fenn/Developer/dw1-silicon/decomp/src/main/main.c:3328), [generated main BSS:49](/Users/fenn/Developer/dw1-silicon/decomp/build/us/psx/generated/main.bss.s:49).

The symbol’s `0x2710` describes the **10,000-byte collision grid**. Its reserved storage is `0x2800`: there are another `0xF0` bytes before the camera. `loadMapCollisionData` copies exactly `0x2710` bytes and leaves that tail alone: [map_collision.c:23](/Users/fenn/Developer/dw1-silicon/decomp/src/main/map_collision.c:23).

For the reported access:

```text
MAP_COLLISION_DATA - 44 = 0x801AF36C
                       = MAIN_D_801A8B98 + 0x67D4
                       = TAMER_MODEL_BUFFER + 0xDFD4
```

As case 4 establishes, the tamer load fills the complete `0xE000` span preceding collision data. Offset `0xDFD4` lies in `BOYS.MMD`’s sector padding, which I verified is **zero**. Thus the PS1 reads **`0x00`**, returns zero, and permits the shadow.

The ordinary debug and release binaries retain the same array adjacency. The ASan binary inserts `0x1A00` bytes between the scratch array’s end and collision data; its `-44` access therefore lands in poisoned padding. The recorded “6612 bytes after `MAIN_D_801A8B98`” agrees exactly with that gap.

**Divergence:** ordinary native layout currently supplies the expected zero on this path, but this depends on separate global objects retaining their order and spacing. Other out-of-grid positions can read meaningful preceding animation bytes. A universal “outside the grid means zero” or clamp would change PS1 behavior. The observed consumer affects rendering; the tamer consumer can affect game state.

**Fix:** share the backing storage proposed for case 4 and read collision indices through its encompassing byte array. Preserve the original signed index arithmetic. Stop diagnostically for accesses outside the represented PS1 span until their backing bytes are established. If a future access reaches `STDVS_CAMERA`, reproduce its **PS1 byte representation** explicitly; native pointer-bearing camera storage is not automatically byte-equivalent.

**2. `BTL_renderCommandMenu`: the layout table is corrupted by the earlier trail initializer**

The actual expression is:

```c
BTL_COMMAND_LABEL_U[BTL_COMMAND_MENU_LAYOUTS[layout][i]]
BTL_COMMAND_LABEL_W[BTL_COMMAND_MENU_LAYOUTS[layout][i]]
```

The layout table contains valid label indices followed by `0xFF` entries: [command_menu.c:113](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/command_menu.c:113). Rendering performs these lookups for every command: [command_menu.c:295](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/command_menu.c:295).

In the ASan artifact:

```text
BTL_COMMAND_LABEL_U       = 0x1009EB440
BTL_COMMAND_LABEL_U + 255 = 0x1009EB53F
BTL_COMMAND_MENU_LAYOUTS  = 0x1009EB540
```

Consequently, **`LABEL_U[255]` is the access ASan describes as one byte before the layout table**. The width lookup lands inside another allocated object:

```text
BTL_COMMAND_LABEL_W + 255 = BTL_COMMAND_MENU_LAYOUTS + 0x1F
```

ASan need not report that second invalid lookup. See [battle log:489](/Users/fenn/Developer/dw1-silicon/captures/native-runs/asan-battle-fixture/log.txt:489).

The source of `255` in this replay is identifiable:

```text
recorded BTL_BUFF_TRAILS base = 0x10528B060
native record size           = 0x138
iteration 4 frame address    = base + 4 * 0x138
                             = 0x10528B540
                             = BTL_COMMAND_MENU_LAYOUTS
```

The old initializer writes a halfword `-1` there, changing layout 0’s first two bytes from `00 01` to **`FF FF`**. That write itself lands in an allocated object, so it is not separately diagnosed. The preceding reports establish the trail base and native size: [battle log:159](/Users/fenn/Developer/dw1-silicon/captures/native-runs/asan-battle-fixture/log.txt:159). The record starts with its halfword `frame`: [btl.h:25](/Users/fenn/Developer/dw1-silicon/decomp/include/dw/btl.h:25).

The replay has brain 71 and three commands, which selects layout 0 and should read only `00 01 03`: [checkpoint:63](/Users/fenn/Developer/dw1-silicon/captures/native-runs/asan-battle-fixture/ckpt_0009_game1.txt:63), [checkpoint:173](/Users/fenn/Developer/dw1-silicon/captures/native-runs/asan-battle-fixture/ckpt_0008_game2.txt:173), [battle_setup.c:124](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_setup.c:124), [command_menu.c:163](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/command_menu.c:163).

**The working tree already restricts native trail initialization to element zero**: [battle_effect.c:6461](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_effect.c:6461). The recorded ASan run predates that repair.

On PS1, the trail stride is `0x12C`, and iteration 4 writes `0x80076150`, beyond the overlay image—not the command layouts at `0x80072E7C`. The retail stride is explicit in [BTL_initializeBuffTrails.s:14](/Users/fenn/Developer/dw1-silicon/decomp/asm/us/btl/matchings/battle_effect/BTL_initializeBuffTrails.s:14); the overlay ends at file offset `0x2349C`: [btl.yaml:113](/Users/fenn/Developer/dw1-silicon/decomp/config/us/btl.yaml:113).

If label index `255` is reached on PS1, its reads are nevertheless exactly identifiable:

| Lookup | PS1 address | Actual object byte | Retail value |
|---|---:|---|---:|
| `LABEL_U[255]` | `0x80134827` | `STD_INTRO_CAMERA_STAGE1_ROT + 7` | `00` |
| `LABEL_W[255]` | `0x8013482F` | `STD_CAMERA_CHASE_OFFSET + 7` | `00` |

These are the high bytes of zero `SVECTOR` pad fields. Symbols and source agree: [symbols.txt:4205](/Users/fenn/Developer/dw1-silicon/decomp/config/us/symbols.txt:4205), [symbols.txt:4245](/Users/fenn/Developer/dw1-silicon/decomp/config/us/symbols.txt:4245), [std_main.c:725](/Users/fenn/Developer/dw1-silicon/decomp/src/std/std_main.c:725). I verified both zero bytes in the retail executable.

Native layout does **not** reproduce these neighbours. Before runtime corruption, the inspected ordinary binaries have:

```text
LABEL_U[255] → BTL_STR_SHOUT_DEFENSE + 7 = 0x21
LABEL_W[255] → BTL_STR_SHOUT_CHANGE  + 3 = 0x6E
```

That produces different texture coordinates from the PS1’s zero-width label.

There is also a separate conditional retail path worth preserving: high-brain command construction can produce **six commands** when no ordinary move slot is populated, but menu initialization has no case 6 and retains its preceding layout. That can expose `0xFF` entries: [battle_setup.c:162](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/battle_setup.c:162), [command_menu.c:163](/Users/fenn/Developer/dw1-silicon/decomp/src/btl/command_menu.c:163).

**Divergence:** the reported fixture’s `255` is native corruption. If `255` occurs through a genuine retail path, ordinary native builds can render different label UVs. These lookups themselves do not modify combat state.

**Fix:** retain the existing trail repair and rerun the fixture first. Do not enlarge `BTL_COMMAND_MENU_LAYOUTS`. For genuine sentinel lookups, a native accessor returning **U = 0, width = 0 for `0xFF`** reproduces the established US bytes. Stop on other unexpected indices. Adding a new six-command layout would change retail behavior.

**3. `MAPHEAD_SCRIPT_BUFFER`: 1,624 bytes of sector padding overwrite the offset table**

This occurs at **process initialization**, before the main menu, through `initializeScripts`: [main.c:3415](/Users/fenn/Developer/dw1-silicon/decomp/src/main/main.c:3415). It is not performed afresh for every map load.

The US filenames are `\SCN\MAPHEAD.SCN` and `\SCN\DG.SCN`: [script_interp.c:154](/Users/fenn/Developer/dw1-silicon/decomp/src/main/script_interp.c:154).

| File/read | Logical file size | Transfer size | Destination |
|---|---:|---:|---|
| Entire `MAPHEAD.SCN` | `0x606E` = 24,686 | 13 sectors, `0x6800` = 26,624 | `MAPHEAD_SCRIPT_BUFFER` |
| Initial section of `DG.SCN` | Whole file `0xC4800` = 804,864 | 4 sectors, `0x2000` = 8,192 | `SCRIPT_OFFSET_TABLE` |

The sizes and LBAs are in [file_table_us.c:754](/Users/fenn/Developer/dw1-silicon/decomp/src/main/file_table_us.c:754). Extracted sizes match, and I compared both files against the disc image. **All `MAPHEAD.SCN` bytes from logical EOF through the sector-rounded end are zero.**

`readFile` deliberately rounds upward to whole sectors: [file.c:87](/Users/fenn/Developer/dw1-silicon/decomp/src/main/file.c:87). The SDK implementation copies those full sectors; it does not know the C destination’s capacity: [libcd.rs:351](/Users/fenn/Developer/dw1-silicon/crates/psyq/src/libcd.rs:351), [libcd.rs:541](/Users/fenn/Developer/dw1-silicon/crates/psyq/src/libcd.rs:541).

The PS1 write is therefore:

```text
MAPHEAD_SCRIPT_BUFFER: 0x801B1D30 .. 0x801B7ED7
overflow:             0x801B7ED8 .. 0x801B852F
                    = SCRIPT_OFFSET_TABLE[0 .. 0x657]
                    = 0x658 / 1,624 zero bytes
```

It reaches neither `MAP_SCRIPT_BUFFER` nor `SCRIPT_STATE`: [symbols.txt:5322](/Users/fenn/Developer/dw1-silicon/decomp/config/us/symbols.txt:5322).

The next statements read all `0x2000` bytes of the offset table from `DG.SCN`, then clear `SCRIPT_STATE`: [script_interp.c:2203](/Users/fenn/Developer/dw1-silicon/decomp/src/main/script_interp.c:2203). The retail listing confirms the same sequence: [initializeScripts.s:23](/Users/fenn/Developer/dw1-silicon/decomp/asm/us/main/matchings/script_interp/initializeScripts.s:23).

Thus **the PS1 performs the overflow, but no persistent dependency on those overwritten zeros was found**. There is no script consumer between the two reads. Actual later map-script loads use offsets from that table and read into `MAP_SCRIPT_BUFFER`: [script_interp.c:647](/Users/fenn/Developer/dw1-silicon/decomp/src/main/script_interp.c:647).

Today, ordinary native binaries happen to place the arrays consecutively in declaration order, so the same zeros reach the same table prefix. In the ASan artifact, the 1,624 bytes instead land entirely in the gap before the table. Both versions subsequently load the correct complete table. The recorded write is [debug-evo log:51](/Users/fenn/Developer/dw1-silicon/captures/native-runs/asan-debug-evo/log.txt:51).

**Divergence:** no persistent gameplay difference is established for successful initialization on this disc. The native transfer still violates its allocation and Rust’s slice contract.

**Fix:** for exact overlapping-write behavior, use one native byte allocation of **`0x81A8`**, with the offset table beginning at **`+0x61A8`**. Alternatively, enlarge only the native maphead buffer to `0x6800`; that preserves the observed post-initialization state because the table is immediately replaced, though it removes the transient overlap. Do not truncate the SDK’s sector transfer.

**4. `TAMER_MODEL_BUFFER`: `MAIN_D_801A8B98` contains the rest of the same loaded MMD**

New game, load game, and post-ending continuation initialize tamer type zero: [main.c:3459](/Users/fenn/Developer/dw1-silicon/decomp/src/main/main.c:3459), [main.c:3475](/Users/fenn/Developer/dw1-silicon/decomp/src/main/main.c:3475), [main.c:3493](/Users/fenn/Developer/dw1-silicon/decomp/src/main/main.c:3493).

`initializeTamer` calls `loadMMD(type, 2)`, selecting `TAMER_MODEL_BUFFER`; the loader reads the whole MMD there and derives model/animation pointers from its first two words: [tamer.c:336](/Users/fenn/Developer/dw1-silicon/decomp/src/main/tamer.c:336), [model.c:3422](/Users/fenn/Developer/dw1-silicon/decomp/src/main/model.c:3422).

The file is **`CHDAT\MMD0\BOYS.MMD`**. Its table entry specifies:

```text
LBA:                   0xAD5
logical size:          0xDECC = 57,036 bytes
sector-rounded size:   0xE000 = 57,344 bytes
```

Evidence: [file_table_us.c:16](/Users/fenn/Developer/dw1-silicon/decomp/src/main/file_table_us.c:16), [model.c:2695](/Users/fenn/Developer/dw1-silicon/decomp/src/main/model.c:2695). Extracted file bytes match the disc; the final 308 transferred bytes are zero padding.

The read exactly fills both reserved buffers:

```text
TAMER_MODEL_BUFFER[0x7800]
MAIN_D_801A8B98[0x6800]
--------------------------------
combined size: 0xE000
```

Therefore **yes: during ordinary tamer gameplay, `MAIN_D_801A8B98` is part of the same loaded data**. It receives `0x66CC` bytes of actual MMD contents followed by `0x134` bytes of padding.

The MMD header is:

```text
08 00 00 00 D0 3A 00 00
model offset = 0x0008
animation table offset = 0x3AD0
```

The animation table has 40 entries. Animation `0x12` starts at file offset **`0x8AF8`**, exactly **4,856 bytes beyond `0x7800`**. Its first bytes are:

```text
1B 00 00 00 00 FC 00 00 00 00 CE FF ...
```

That matches the first reported `startAnimation` access precisely: [new-game log:264](/Users/fenn/Developer/dw1-silicon/captures/native-runs/asan-new-game/log.txt:264). The call comes through `scriptStartAnimation`, which starts the tamer’s requested animation: [script_interp.c:893](/Users/fenn/Developer/dw1-silicon/decomp/src/main/script_interp.c:893).

Subsequent accesses are ordinary animation decoding:

- Initial frame count and bone transforms: [anim.c:216](/Users/fenn/Developer/dw1-silicon/decomp/src/main/anim.c:216).
- Frame instructions, loops, texture changes and sound: [anim.c:276](/Users/fenn/Developer/dw1-silicon/decomp/src/main/anim.c:276).
- Keyframe/momentum operands: [anim.c:359](/Users/fenn/Developer/dw1-silicon/decomp/src/main/anim.c:359).

The required extension is **much larger than the observed 5 KB**. Animation `0x27` starts at `0xD85A`, 24,666 bytes past the declared buffer boundary.

Ordinary native binaries currently retain the PS1 adjacency, so the oversized load populates the following array and animation decoding finds the intended bytes. ASan moves the scratch array from `+0x7800` to `+0x9600`, breaking that alias relationship and placing animation accesses in poisoned storage. The loader’s full-file write is itself out of bounds before the animation readers execute.

The scratch name also has another legitimate lifetime: Battle Mode uses it for `1P2PWIN.TMD`, while reusing the tamer buffer for arena models: [vs_hud.c:1448](/Users/fenn/Developer/dw1-silicon/decomp/src/vs/vs_hud.c:1448), [vs_scene.c:1707](/Users/fenn/Developer/dw1-silicon/decomp/src/vs/vs_scene.c:1707). A fix must preserve that shared-storage behavior.

**Divergence:** animation bytes drive transforms, root movement, completion flags and script timing. Incorrect storage can change gameplay or crash. This is required data, not harmless excess capacity.

**Fix:** represent the complete native `0xE000` MMD/scratch span as one aligned byte array, retaining the scratch alias at `+0x7800`. Prefer extending that allocation through collision data as described below.

**Native adjacency and its limits**

Measured symbol deltas:

| Relationship | PS1 | Ordinary native | ASan native |
|---|---:|---:|---:|
| Tamer → scratch | `0x7800` | `0x7800` | `0x9600` |
| Scratch → collision | `0x6800` | `0x6800` | `0x8200` |
| Collision → camera | `0x2800` | `0x2800` | `0x3200` |
| Maphead → offset table | `0x61A8` | `0x61A8` | `0x7A20` |
| Offset table → map script | `0x2000` | `0x2000` | `0x2800` |

The ordinary adjacency follows declarations in [main.c:3328](/Users/fenn/Developer/dw1-silicon/decomp/src/main/main.c:3328) and [script_interp.c:349](/Users/fenn/Developer/dw1-silicon/decomp/src/main/script_interp.c:349). Native compilation uses Clang and `-fno-common`, not the PS1 linker script: [build.rs:108](/Users/fenn/Developer/dw1-silicon/crates/decomp-c/build.rs:108), [build.rs:125](/Users/fenn/Developer/dw1-silicon/crates/decomp-c/build.rs:125). Declaration order is not a storage contract; sanitizer instrumentation already demonstrates the failure.

**Ranked fix list**

1. **Combine tamer, scratch and collision backing storage under `DW1_NATIVE`.** Use one aligned **`0x10800` byte array**, with aliases at `0`, `0x7800` and `0xE000`. Load the MMD through the encompassing allocation and resolve collision reads within it. This addresses cases 4 and 1 while retaining loaded bytes, zero padding and Battle Mode reuse. Separate array members in a struct are insufficient if code still indexes beyond a member’s bounds.

2. **Combine maphead and offset-table storage**, preserving the table at `+0x61A8`; alternatively use a native `0x6800` maphead allocation if post-initialization equivalence is the chosen requirement. Preserve full-sector reads.

3. **Rebuild and rerun battle-fixture with the existing buff-trail repair.** Its fifth store explains the reported menu lookup completely. Treat disappearance of that downstream error as verification of the root repair.

4. **Handle genuine menu index `0xFF` explicitly as U = 0, width = 0.** Preserve the retail six-command/stale-layout behavior; stop on unexplained indices rather than borrowing arbitrary native neighbours.

Verification should include ASan new-game, battle-fixture and debug-evo, canonical replay comparisons, and coverage of later tamer animations and the six-command menu path. The existing checkpoint schema does not directly prove menu UV or shadow equivalence.