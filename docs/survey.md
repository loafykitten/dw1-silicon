# S0.5: survey of the decomp

2026-10-13. dw_decomp at 09c174c, US build only (`VERSION_US`; the 120 C files in
`mk/version/us.mk`). Three gpt-6.1-sol (high) read-only runs did the counting; Claude checked
samples against the code (EFE slot table, TFS parser, script `setjmp`/`longjmp`, both RNGs,
clang and `-fexec-charset`): all held. The full reports, with every file:line list, are in
`docs/survey/`:

- `a-sdk-surface.md`: every SDK call and DW2-PC's coverage, GTE, scratchpad, fixed addresses
- `b-abi-checklist.md`: `long`, pointer/int casts, layouts, K&R, prototypes, UB, asm
- `c-formats-overlays-pacing.md`: loaded formats, overlay lifetimes, VSync, scripts, RNG

Paths below are relative to `ref/dw_decomp/` (later `decomp/`).

**Limit:** the clone had no submodules, so the SDK headers (`external/psyq_headers`, public
but unlicensed: they're Sony's) and the splat-generated assembly were absent. SDK typedefs,
SDK-internal call graphs and the GTE commands inside SDK routines are unverified until S0.3
runs `make regenerate`. Sony's headers stay out of this repo: the Rust side gets them through
bindgen at build time from the submodule, never as committed copies.

## What it means for the plan

1. **The library layer is mostly new work.** DW1 makes 2,939 SDK calls through 175
   functions, plus 44 macros and 27 TMD handlers reached through `GsFCALL4`. DW2-PC has
   about a third of the names, and "has the name" isn't "has the behaviour":
   - **libgs** (35 functions, 962 calls): DW2-PC has 11. Missing: the whole object pipeline
     (`GsLinkObject4`, `GsMapModelingData`, `GsSortObject4J` and the 27 `GsTMDfast*`/
     `GsTMDdiv*` handlers), ordering tables (`GsClearOt`, `GsSortOt`, `GsDrawOt`), sprites
     (`GsSortSprite`, 45 calls), `GsGetWorkBase`/`GsSetWorkBase` (347 calls), double buffers.
     S0.7's spike is exactly this.
   - **libgpu**: the `Set*`/`set*` primitive macros (846 uses) and `GetTPage`/`GetClut`.
     Small, mechanical, but must be bit-exact.
   - **libgte**: 29 functions, 13 missing (`RotMatrix`, `TransMatrix`, `RotMatrixZYX`,
     `RotNclip3/4`, `NormalColorCol*`...). GTE commands DW2-PC lacks: **RTPT** (issued 10
     times directly by game C), and likely NCCS/NCCT, NCT, AVSZ3/4, GPL inside the SDK.
   - **libsnd**: DW1 plays **SEQ** (`SsSeqOpen`/`Play`/`Stop`), DW2-PC only SEP; DW1 uses
     sticky VAB headers, per-voice key on/off, `SsSetTickMode(SS_TICK240)` with `SsStart`
     (root counter 2 at 240 Hz, not VBlank). DW2-PC's sound code is tied to its own retail
     addresses.
   - **libcd**: `CdRead`, `CdReadSync`, `CdSearchFile` missing (the game's whole file layer
     uses them). **libmcrd**: `MemCardGetDirentry`, `MemCardDeleteFile` missing.
     **libpress**: `DecDCTvlcBuild` missing; DW2-PC's `DecDCTvlc2` ignores the table.
   - **libc**: `rand` (154 direct calls, 168 more through `randomLimit`) must be the SDK's
     exact generator; `setjmp`/`longjmp` can be the host's.
   - No game-C hardware register or DMA access, and no game-installed VSync/CD/DrawSync
     callbacks. The scratchpad is used heavily (1,117 expressions, mostly the battle effect
     interpreter and fishing): a 1 KB native buffer serves it.
2. **Pointers stored in 32 bits are a bigger, more mechanical job than the plan said.**
   - The battle/tournament/versus **effect interpreter (EFE)**, triplicated in BTL, STD and
     VS, is the hot spot: `EFE_PUSH` narrows pointers (including addresses of stack locals)
     to `int32_t` (353 sites), 464 explicit casts, 159 integer-address restorations.
   - Arithmetic like `(char *)((int32_t)base + 0x280)` is everywhere in that code. A plain
     C cast back from `int32_t` can't restore the high half, so **the 4 GB window from
     S0.4 doesn't make these sites work unchanged**; every round trip must be rewritten.
   - So the design changes (to settle in `docs/memory.md`): widen in-memory stacks and
     integer-address variables to `intptr_t` (`EFE_PUSH` and the stack arrays are a few
     header lines), change arithmetic-only casts to `intptr_t` mechanically (ast-grep), and
     keep the window only where a 32-bit value must live inside a loaded buffer.
   - Since the EFE interpreter is self-contained, duplicated three times and full of 32-bit
     tricks, it's a strong early candidate for the Rust migration once the hybrid runs it.
3. **Loaded data is mostly offset-based; a few formats are rewritten in place.**
   - Offsets, not addresses: MMD (two 32-bit offsets read through `long *`), animations,
     MAP (an `s32` offset directory), SCN scripts, VHB, SEQ tables, EFE headers, TIM, STR.
   - **The save has no pointers** (`main_menu.c:88-152`): fixed-width fields, a 0x200-byte
     header plus two 0xF00 payloads, only the first 0x17C bytes of the script state saved.
     Native code must keep the payload a fixed-width record (`VECTOR` fields, the 28-byte
     copy spanning `reserved` and `pocketStation`).
   - **In-place rewrites:** `GsMapModelingData` turns TMD offsets into 32-bit addresses
     inside 28-byte object records (35 sites), and the game then edits primitives through
     them; EFE files become mutable runtime storage with seven-pointer records written into
     the loaded buffer. Both need native side tables (the Rust libgs keeps its own mapped
     model; EFE needs a per-slot native mirror).
   - `long`/`u_long` width: 521 lines, 249 touching data; e.g. the TFS parser walks the
     file in `u_long` steps (`map.c:4234`). Fix: map the SDK's `u_long`/`long` to 32-bit
     types (DW2-PC's `psyq_u_long` approach) and fix the game's own `long *` reads.
4. **Overlays: no name clashes, but lifetimes are subtle.**
   - 5,594 external definitions across main and overlays, no duplicates. Linking everything
     into one program is fine name-wise.
   - 540 objects from overlay source files live in the resident executable (small data),
     12 of 14 `*_bss_us.c` files hold non-zero initial data, BTL/STD/VS carry a TIM and four
     TMDs each, and load ranges overlap (sector rounding makes STD overwrite DOOA's first
     0x2E0 bytes). Overlays are loaded by `loadDynamicLibrary` (`utils2.c:376`), sync or
     queued, with no teardown step; resident world objects keep overlay function pointers.
   - So "reset an overlay" means: re-initialize its reloadable `.data`/`.bss` from the
     original image, leave its resident small data alone, and model overlap where it
     matters. The per-symbol map is in report C.
5. **Pacing:** `VSync(3)` (20 fps) in gameplay, battle and menus; `VSync(0)` on the landing
   screen and movies; fishing reads the VSync counter to time random decisions. The game
   clock is 20 ticks per game minute; care timers gate on changed ticks and run in world
   object slot order. Movies use CPU-iteration timeouts (`mov.c:140-187`) that must become
   time- or frame-based.
6. **Scripts:** the SCN interpreter (`script_interp.c`) uses one global `jmp_buf`, 37
   `longjmp`s, and a `longjmp` at :520 that can run before this call's `setjmp` (:601).
   There's also a cycle `tickScript → changeMap → runMapHeadScript → tickScript` sharing
   that buffer. Needs a careful native review (S0.4/Phase 2), and makes the interpreter
   another early Rust candidate: a loop with an explicit result instead of `longjmp`.
7. **RNG:** `customRandom` (57 callers) is two 32-bit LCGs (`×0x41C650AD + 0x3039`) with
   fixed initial state; the SDK `rand()` (322 uses with `randomLimit`) needs its exact
   recurrence and seed taken from the SDK code once S0.3 extracts it. No `srand`, no
   timer-based seeding.
8. **Compiler:** no floating point anywhere. 233 K&R definitions, 197 empty parameter lists,
   ~15 real prototype mismatches (`memcpy` redeclared, two- vs three-argument
   `fadeoutCLUT1`...), MIPS inline asm only in `_psstart.c` and the GTE macros (both
   replaced). Apple clang rejects `-fexec-charset=CP932`, so game text needs explicit
   encoding at build time. Plain `char` signedness matters (`entity.h:30` is numeric):
   pin it to what CodeWarrior did. Known UB to fix: the null `*loc` read
   (`file_queue.c:250`), an uninitialized pop (`script_interp.c:2396`), unchecked stacks.

## Riskiest 15 sites (from report B)

1. `include/dw/efe.h:15`: `EFE_PUSH` truncates pointers, including stack addresses
2. `src/btl/battle_effect.c:2133`: 0x280 bytes reserved for 16 `EfeSlot`s that need 0x500
3. `src/btl/battle_effect.c:1801`: loaded bytes used as seven-pointer records
4. `src/main/model.c:3415`: MMD 32-bit offsets read through `long *`
5. `src/main/map.c:4234`: TFS parsed in host-width `u_long` steps
6. `src/main/drop_shadow.c:64`: packed screen coordinates passed as `long *`
7. `src/main/main_menu.c:5010`: save checksum over an exact byte layout
8. `src/btl/battle_effect.c:3359`: 136-byte position stride and field offsets hardcoded
9. `src/doo2/doo2.c:626`: TMD bytes read as a pointer-bearing descriptor
10. `src/btl/btl_bss_us.c:1001`: initialized non-address bytes in pointer fields
11. `src/evl/evl.c:457`: numeric timer read through a pointer-sized alias
12. `src/main/file_queue.c:250`: null `loc` dereference
13. `src/main/script_interp.c:2396`: uninitialized stack entry copied on empty pop
14. `src/main/script_interp.c:601`: one `jmp_buf` across a possible re-entry cycle
15. `src/shop/script_menu.c:3005`: string address truncated, then passed to `strlen`
