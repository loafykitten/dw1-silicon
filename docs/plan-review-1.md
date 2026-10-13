# Plan review 1: GPT 6 Astra (high), 2026-10-13

Adversarial review of PLAN.md (pre-revision) against dw_decomp 09c174c and DW2-PC 2d491b5.
Claude spot-checked findings 1-6, 9, 11, 13, 14 and 18 against the code: all held. Paths are
to the local reference clones under `ref/`.


Verdict: revise the plan before treating Phase 0 as an executable handoff.  
The port remains plausible, but DW2-PC is a useful reference rather than a mostly reusable platform layer.  
The largest gaps are memory representation, DW1’s rendering libraries, overlay state lifetimes, and publication-safe imports.  
Several concrete claims are wrong: DW2 builds require disc data, low addresses do not preserve struct layouts, and the US overlay count needs qualification.  
This was a static review of both pinned clones and relevant upstream documentation; no files were modified, no disc data was read, and runtime behavior remains unverified.

1. **The preferred memory model is unsuitable for ordinary arm64 macOS—and is misdescribed.**

   **Severity:** blocker

   **Claim in plan:** “Game memory is kept below 2 GB so 32-bit structures work unchanged”; S0.4 proposes choosing the architecture through a small `mmap` experiment.

   **What’s actually true:** DW2-PC combines several distinct mechanisms:

   - A fixed executable base at `0x01000000`, with globals and its game heap inside one **16 MiB window**, to reconstruct **24-bit GPU ordering-table links**.
   - Addresses below **2 GiB** so pointers stored in signed 32-bit integers round-trip.
   - A separate 2 MiB `Ps1_Ram` arena translating original PS1 addresses.
   - On Linux, a `MAP_32BIT` stack entered through `makecontext`/`swapcontext`; on Windows, PE flags disabling address randomization, including high-entropy VA.

   See [DW2 CMakeLists.txt:25](ref/DW2-PC/CMakeLists.txt:25), [ps1mem.h:8](ref/DW2-PC/psyq/ps1mem.h:8), and `ref/DW2-PC/host/main.c:198–216,321–341`. None makes native pointers four bytes. DW2 explicitly acknowledges enlarged structures and doubles its scratchpad allocation at `psyq/ps1mem.h:17–19`.

   Apple’s loader requires a hard 4 GiB page-zero region for ordinary ARM64 executables. It enforces this through the VM map’s minimum address, not merely a removable `PROT_NONE` mapping. Shrinking `__PAGEZERO` or using `MAP_FIXED` is therefore not a supported escape. The check is independent of hardened-runtime status; ad-hoc signing does not waive it. See [Apple XNU’s loader](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/mach_loader.c), especially lines 651, 807–838 and 2231–2241. Exact behavior on this installed macOS/toolchain was **unverified**.

   **Suggested change to PLAN.md:** Replace the ranked alternatives with a representation design: native pointers for native objects; explicit 32-bit disk records; checked guest-address/offset translation for fixed memory; and handles or relative links for GPU packets and integer-stored references. A high-address arena with 32-bit offsets is another useful component. Treat the low-memory probe as bounded confirmation, not the architecture-selection gate.

2. **An unconditional subtree import violates the plan’s own publication rule.**

   **Severity:** blocker

   **Claim in plan:** “Code from the decomp is fine to commit”; import dw_decomp through `git subtree`; never publish disc-derived data.

   **What’s actually true:** The checkout contains more than independently authored implementation code:

   - [src/main/_psstart.c:1](ref/dw_decomp/src/main/_psstart.c:1) explicitly carries a Metrowerks copyright and “All Rights Reserved.”
   - `text/main/main.yaml:14` onward contains game names; other `text/` files contain game text. `mk/text.mk:1–10` compiles these into generated headers.
   - [src/mov/mov_bss_us.c:6](ref/dw_decomp/src/mov/mov_bss_us.c:6) contains initialized movie state and enormous raw integer arrays. `config/us/mov.yaml:66` places this file in the retail overlay’s `.data`.
   - DW2’s source also embeds extracted retail data into its executable through `.incbin`, including a boot TIM and sound-library data: `host/overlay.c:33–35`, `psyq/snd_hw.c:20–38`. Its README’s broad “no game data” wording is not sufficient evidence otherwise.
   - DW2 includes screenshots and third-party components with their own licenses: `README.md:33–43,327–330`.

   The top-level MIT/CC0 labels do not establish provenance or publication rights for every embedded item. Legal clearance is **unverified**; the conflict with this plan’s stricter no-disc-data policy is directly visible.

   **Suggested change to PLAN.md:** Add a provenance/import gate **before S0.2**. Keep upstream as an ignored reference or locally fetched dependency until an allowlisted import is prepared. Extract text, assets, original initialized blobs, and required SDK tables locally at runtime or pack-build time. Audit both Git history and release binaries; deleting a file after importing its history is insufficient.

3. **S0.3 cannot compile the full DW2 port without DW2 disc-derived files.**

   **Severity:** blocker

   **Claim in plan:** “Doesn’t need DW2 data to compile; running it needs a DW2 disc.”

   **What’s actually true:** The README explicitly says the build needs extracted disc files at `dumps/disc/` (`ref/DW2-PC/README.md:290–293`). The build script calls extraction with `allow_missing=False` and rejects a missing `assets/Ovl_LoadArea.bin`: [tools/build_native.py:135](ref/DW2-PC/tools/build_native.py:135).

   Required inputs include `ratan_tbl.bin`, retail sound data, file tables, and other `INCLUDE_BIN` assets. Even the supplied `gte_test` target includes `psyq/libgte.c`, whose assembly directly includes the extracted atan table (`CMakeLists.txt:229–243`; `psyq/libgte.c:25–30`). The synthetic `tests/gte_ref.bin` itself is game-data-free, but that does not make the target’s build independent of disc data.

   **Suggested change to PLAN.md:** Make S0.3 a **component portability harness** for GPU packets, GTE commands, SPU, XA and MDEC using synthetic inputs. A full DW2 build is optional and requires a legitimately available DW2 extraction. Do not make it a mandatory go/no-go prerequisite.

4. **DW1 needs substantially more libgs and GTE implementation than DW2 provides.**

   **Severity:** major

   **Claim in plan:** Adapt DW2’s graphics libraries and software renderer; rank shallow reuse fourth among risks.

   **What’s actually true:** DW2’s public libgs shim supplies 12 functions, chiefly coordinates, lighting, view setup and TIM parsing: [psyq/libgs.h:8](ref/DW2-PC/psyq/libgs.h:8).

   DW1 relies on the higher-level rendering machinery: `GsMapModelingData`, `GsLinkObject4`, `GsSortObject4J`, `GsSortSprite`, work-base allocation, ordering-table management and display-buffer helpers. It also installs approximately 27 TMD rendering dispatch entries in `GsFCALL4`: [src/main/graphics.c:60](ref/dw_decomp/src/main/graphics.c:60). Having a GPU packet rasterizer does not generate these missing packets.

   DW2’s GTE command switch implements only RTPS, NCLIP, MVMVA, NCS and GPF; other commands merely log: `psyq/gte.c:286–296`. DW1 directly uses `gte_rtpt`, including `src/main/item.c:2539` and `src/btl/battle_hud.c:665`. Its missing libgs routines will introduce further GTE requirements.

   **Suggested change to PLAN.md:** Promote libgs/TMD/GTE coverage to a top-three risk. Before the go/no-go, demonstrate a locally extracted DW1 model traversing mapping, animation, lighting, sorting and packet rendering. Produce a function-and-command coverage table, including library internals reached indirectly through dispatch tables.

5. **The ABI survey must distinguish serialized offsets, guest addresses and native pointers.**

   **Severity:** major

   **Claim in plan:** Disc data “may hold PS1 addresses”; use native pointers with fixups or translate accesses into a 2 MiB block.

   **What’s actually true:** Several different cases already exist:

   - MMD loading reads two **32-bit offsets** through `long *`: [model.c:3414](ref/dw_decomp/src/main/model.c:3414). Native macOS `long` is eight bytes, so this breaks independently of pointer address.
   - Animation offsets are added to a pointer narrowed to `int32_t`: `src/main/anim.c:216–220`.
   - MAP and script loaders also use relative offsets: `src/main/map.c:4033–4061`; `src/main/script_interp.c:650–680`.
   - Runtime pointers enter 32-bit interpreter stacks through `EFE_PUSH`: [include/dw/efe.h:14](ref/dw_decomp/include/dw/efe.h:14), with return pointers pushed at `src/btl/battle_effect.c:4778`.
   - Fixed guest addresses are explicit in `include/dw/main.h:8–10` and `src/main/btl.c:23–27`.
   - `PositionDataRaw` overlays a 136-byte retail representation with a pointer-bearing native struct: `include/dw/entity.h:11–23`.
   - TMD primitive addresses are read from `obj->primtop` after model mapping: `src/main/graphics.c:170–178`.

   Original executables/overlays necessarily carry linked addresses; ordinary asset formats above demonstrate offsets instead. I did **not** establish that the save payload contains live PS1 pointers. Its declared fields largely contradict that blanket assumption.

   **Suggested change to PLAN.md:** Require a schema for every loaded format, with explicit field widths, alignment, address classification and bounds checks. Keep disk structures separate from expanded native structures. Include interpreter stacks, packet links, scratchpad layouts and callbacks—not just disk/save boundaries.

6. **Overlay resets must follow original storage lifetimes, not source directories.**

   **Severity:** major

   **Claim in plan:** Link overlays together and reset each overlay’s data as DW2 does.

   **What’s actually true:** DW1 places small-data globals from overlay source files in the **resident main executable**. For example, BTL `.sdata` is selected by [config/us/main.ld:634](ref/dw_decomp/config/us/main.ld:634), and BTL `.sbss` by line 774. These survive an overlay-file reload.

   The reload itself copies file bytes into selected addresses, synchronously or through the file queue: [utils2.c:376](ref/dw_decomp/src/main/utils2.c:376). Overlays occupy several different, overlapping ranges (`config/us/overlay.ld:3,83,108`), and include assets as well as code (`config/us/btl.yaml:49–95`). “BSS” files can contain nonzero retail initialization, as MOV demonstrates.

   DW2 instead groups each overlay’s native `.data`/`.bss` and snapshots those ranges (`CMakeLists.txt:77–119`; `host/overlay.c:51–79`). Applying that grouping indiscriminately to DW1 changes persistence.

   Symbol collisions appear less serious than the plan implies: the existing DW1 build already links main and overlay sections into one ELF. A lexical scan of the 120 US-listed C files found no confirmed active duplicate function definition; its one candidate was region-dependent. This is **not** a complete object-symbol audit.

   **Suggested change to PLAN.md:** Generate a per-symbol lifetime map from original linker placement. Specify resident state, reloadable state, asset bytes, scratch buffers and overlapping regions separately. Test repeated loads, alternating overlays, async completion and persistent globals before adopting a reset implementation.

7. **Other SDK differences are concrete API and timing gaps, not simply different library names.**

   **Severity:** major

   **Claim in plan:** Adapt DW2’s “graphics, sound, CD, pad, card” layer.

   **What’s actually true:** DW1 US selects `mw_lib41` headers (`mk/version/us.mk:3`); DW2’s shim describes Psy-Q 4.7. Both use **libgs, libsnd, libmcrd and libpress**. DW2 is not merely libgpu plus a custom sound driver, nor is its card layer merely libcard.

   Important missing DW1-facing entries include:

   | Surface | DW1 examples | DW2 difference |
   |---|---|---|
   | CD | `CdRead`, `CdReadSync`, `CdSearchFile` in `src/main/file.c:85–94`, `script_interp.c:2797–2817` | Absent from the supplied shim |
   | Sound | `SsSeqOpen/Play/Stop`, `SsVabOpenHeadSticky`, per-voice calls in `src/main/sound.c:405,426,485,621` | Public API centers on `SsSep*`, `SsStart2`, `SsVabOpenHead` |
   | Sequencer clock | `SsSetTickMode(SS_TICK240)` and `SsStart`, `sound.c:524–534` | DW2 exposes a manually driven sequencer path |
   | Pad | `PadInit`, `PadRead` | Different libpad interface |
   | Card | `MemCardDeleteFile`, `MemCardGetDirentry`, `main_menu.c:3871,3932` | Not supplied by DW2’s shim |
   | Movies | `DecDCTvlcBuild`, `StGetBackloc`, `mov.c:232,250` | Missing entries |

   DW2’s sound implementation also uses **DW2-specific retail addresses and extracted initialization blocks**, not a completely independent portable sequencer: [psyq/libsnd.c:5](ref/DW2-PC/psyq/libsnd.c:5), `psyq/snd_hw.c:20–40`.

   **Suggested change to PLAN.md:** Separate hardware-backend reuse from SDK implementation reuse. Audit prototypes, callback behavior, tick sources and data dependencies. Record libds as present in DW1’s extracted SDK sections, but do not infer a required new game-facing API without call evidence.

8. **A full DW2 macOS build is itself a substantial port.**

   **Severity:** major

   **Claim in plan:** S0.3 “separates Mac problems from DW1 problems.”

   **What’s actually true:** The non-Windows branch assumes Linux/ELF, not arbitrary Unix:

   - `-m64`, `-fno-pie`, `-no-pie`, `-Ttext-segment`, GNU linker scripts and `ld -r -T`: `CMakeLists.txt:25–58,101–116,217–224`.
   - ELF `__start_`/`__stop_` section symbols: `host/overlay.c:17–22`.
   - `MAP_32BIT` and Linux-oriented context switching: `host/main.c:321–341`.
   - Linux shared-library naming and release selection: `host/update.c:46–55,217`.
   - A shader path accepting SPIR-V only: `host/sdl.c:163`; `backend/psxgpu_hw.c:85,104`.

   SDL_GPU itself supports Metal, but Metal requires appropriate shader formats and resource bindings. SDL documents offline shader conversion as the preferred approach: [SDL GPU documentation](https://wiki.libsdl.org/SDL3/CategoryGPU).

   **Suggested change to PLAN.md:** Replace “build DW2 natively” with a bounded portability inventory and synthetic harness. Define a new Mach-O build/reset strategy and MSL/metallib shader pipeline. Do not spend Phase 0 porting DW2’s entire host, updater and game merely to validate selected backends.

9. **Compiler flags will not repair several already visible native failures.**

   **Severity:** major

   **Claim in plan:** Mitigate CodeWarrior-shaped behavior using `-fwrapv -fno-strict-aliasing`, low optimization and UBSan.

   **What’s actually true:** There are concrete issues beyond optimizer sensitivity:

   - `addFileReadRequestPath` passes `NULL` for `loc`, then `addFileReadRequest` unconditionally evaluates `*loc`: [file_queue.c:191](ref/dw_decomp/src/main/file_queue.c:191), line 250. The later path lookup replaces that position, but the native null read must still be eliminated.
   - `main.c:3789` redeclares `memcpy` with an incompatible signature.
   - `_psstart.c:222–242`’s non-Metrowerks branch is MIPS assembly, not portable startup.
   - `include/gcc/inline_n.h:9` onward emits MIPS coprocessor instructions.
   - K&R definitions occur, for example, at `anim.c:201–204`; they require a deliberate C dialect and prototype cleanup.
   - The script interpreter uses `setjmp`/`longjmp`: `script_interp.c:364,600–629`. Host ABI, stack lifetime and modified automatic-variable rules need review.

   Actual Apple-clang diagnostics were **unverified**. GCC compilation is evidence of MIPS-GCC compatibility, not arm64 ABI correctness.

   **Suggested change to PLAN.md:** Add a native ABI/compiler checklist with explicit fixes and small semantic tests. Include `long` width, signedness, truncation, shifts, unaligned access and callback signatures. Both machines are little-endian; alignment and field width are the larger problems. Keep original fixed-point/GTE behavior; do not replace it with floating point as a routine cleanup.

10. **“Same input each frame” is not yet a replay specification.**

   **Severity:** major

   **Claim in plan:** Record pad state each emulator frame and replay it in the port from boot or a fixture save.

   **What’s actually true:** DW1 can execute differing numbers of VBlanks between input polls and game updates. It uses `VSync(0)` in the landing screen and `VSync(3)` in the new-game loop (`main.c:3656,3761`). Async CD completion changes control flow (`file_queue.c:127–175`), and movie code contains CPU-iteration timeouts (`mov.c:140–186`).

   DW2’s wall-clock scheduler explicitly performs catch-up VBlanks and restarts after long stalls: [host/vblank.c:10](ref/DW2-PC/host/vblank.c:10). That behavior cannot serve unchanged as a deterministic test clock.

   PCSX-Redux does expose pad overrides, VSync events and savestates. Its documentation also distinguishes deferred savestate loading from completion: [Redux API](https://pcsx-redux.consoledev.net/Lua/redux-basics/). API availability does not prove DW1 replay determinism.

   **Suggested change to PLAN.md:** Define replay events against emulated VBlank and pad-poll sequence numbers, with named game-update checkpoints. Add a deterministic host clock, explicit CD/SPU completion scheduling and divergence traces. Pin emulator revision, CPU core, BIOS/OpenBIOS, settings, initial RAM/card state and controller configuration. Certify replay across fresh emulator processes, not just two continuations of one state.

11. **State comparison needs canonical serialization and both RNG implementations.**

   **Severity:** major

   **Claim in plan:** Dump state structs and compare fields, including “RNG state.”

   **What’s actually true:** `Entity` and `EntityAnim` contain pointers and change layout (`include/dw/entity.h:33–61`). Padding, addresses, scratch memory, overlay initialization bytes and `jmp_buf` cannot be compared as native struct dumps.

   DW1 uses both `customRandom` and SDK `rand`. [rng.c:8](ref/dw_decomp/src/main/rng.c:8) defines the custom stream; [math.c:5](ref/dw_decomp/src/main/math.c:5) assumes `rand()` has a 15-bit result when computing `randomLimit`. Substituting macOS libc `rand` would alter gameplay.

   I found no direct game-C `srand`, `GetRCnt` or `SetRCnt` calls. A claim that DW1 seeds RNG from timers is therefore **unverified**, not an established defect. SDK-internal behavior still needs extraction or observation.

   **Suggested change to PLAN.md:** Create a versioned semantic checkpoint schema. Serialize fixed-width values, convert references to stable entity/resource IDs and exclude only explicitly documented non-semantic fields. Implement and test the original `rand` stream, its initial state and the custom stream. Capture RNG call counts at divergence checkpoints.

12. **59.94 Hz should not be presented as proven DW1 timing.**

   **Severity:** major

   **Claim in plan:** Phase 2 requires VSync pacing at 59.94 Hz.

   **What’s actually true:** DW2 hardcodes `60000/1001` (`host/vblank.c:10,29–30`). That is not evidence for DW1’s exact hardware timing. DW1 initializes a 320×240 non-interlaced-style display path (`main.c:3562–3564`); its clock advances on game ticks, with 20 ticks per game minute (`clock.c:96–136`).

   PS1 timing documentation distinguishes approximately **59.826 Hz non-interlaced NTSC** from **59.940 Hz interlaced NTSC**. See [PS1 GPU timings](https://psx-spx.consoledev.net/ps1/gpu/timings/). The selected PCSX-Redux revision’s exact behavior is **unverified**.

   **Suggested change to PLAN.md:** Specify VBlank/field timing, game-update cadence and presentation cadence separately. Measure DW1’s display modes and wait semantics; preserve `VSync(n)` behavior. Document any deliberate nominal-59.94 compatibility mode instead of labeling it exact hardware timing.

13. **Saving and save-format validation must happen before fixture-dependent phases.**

   **Severity:** major

   **Claim in plan:** Phase 6 implements saves; earlier phases use fixture saves.

   **What’s actually true:** The save payload is an explicit serialization layout, not a dump of the live pointer-bearing entity graph: [main_menu.c:88](ref/dw_decomp/src/main/main_menu.c:88). Loading reconstructs state field by field at lines 5016–5068. Checksums use fixed byte extents—`0x4E4` and `0xA00`—at lines 5004–5013. Incorrect SDK type widths can corrupt this even without embedded pointers.

   DW2 supports raw 128 KiB cards and temporary-file replacement (`host/card.c:7–9,73–94`), but this does not establish DW1 payload compatibility.

   **Suggested change to PLAN.md:** Move read-only card import, save parsing and fixture loading into early bring-up. Before any writes, assert retail offsets/sizes and test emulator → native → emulator round-trips on copies. Add checksum rejection, unknown-byte preservation, two-slot coverage, failed-write handling and preservation of unrelated card blocks. Restrict “as-is” compatibility to explicitly tested raw card formats.

14. **Audio and movies cannot simply wait until Phase 5.**

   **Severity:** major

   **Claim in plan:** Boot and early gameplay first; sound, XA and movies later.

   **What’s actually true:** US boot calls `playMovie(0, 1)` before the landing screen, then initializes music; new-game progression calls another movie: `main.c:3427–3443`.

   Sound loading is part of control flow. [sound_async.c:128](ref/dw_decomp/src/main/sound_async.c:128) waits for `SsVabTransCompleted`, while the file queue withholds completion until the wait callback finishes (`file_queue.c:151–175`). Synchronous sound loading also blocks at `sound.c:414`.

   **Suggested change to PLAN.md:** Bring up CD, sound-transfer completion, sequencer timing and movie completion/skip semantics with the first running slice. Audible output and fidelity can remain later. Any temporary bypass must be explicit in the fixture contract and cannot count as a faithful replay from original boot.

15. **The pack format needs a DW1-specific design, including executable-resident data.**

   **Severity:** major

   **Claim in plan:** Adapt DW2’s pack builder and manifest.

   **What’s actually true:** DW2’s extractor uses a game-specific file-ID table and data outside the ISO filesystem: [doc/PACK_FORMAT.md:12](ref/DW2-PC/doc/PACK_FORMAT.md:12). It preserves 2336-byte sector bodies for Form 2 content, rather than reducing everything to 2048-byte files (lines 58–63).

   DW1 uses filename lookup through its own table (`src/main/file.c:16–33`), ISO searches and partial reads (`script_interp.c:2783–2818`), and textures/models embedded in overlay files (`config/us/btl.yaml:49–95`). Its native build must also obtain data currently represented by generated assembly or C initializers.

   **Suggested change to PLAN.md:** Define a DW1 manifest covering filenames, LBAs, logical sizes, sector formats and required slices of main/overlay data. Preserve XA/STR subheaders and stream ordering. Include bounds validation, atomic pack replacement, a format version and a clean-checkout build that does not embed game assets into distributable binaries.

16. **The GCC route is valid, but its prerequisites and evidentiary value need sharper wording.**

   **Severity:** major

   **Claim in plan:** S0.2 uses `make regenerate`, then `make TOOLCHAIN=gcc`; CI does GCC after matching; GCC validates preserved meaning.

   **What’s actually true:**

   - GCC C compilation does not invoke `cc_mips.dll`: `mk/toolchain/gcc.mk:1–9`.
   - `regenerate` invokes splat generation, not the matching compiler: `Makefile:47–54`; `mk/platform/psx.mk:85–87`.
   - Splat **does require extracted original executables/overlays**, specified by `target_path` in the YAMLs.
   - The build still assembles extracted Psy-Q code/data. It is not an SDK-free compilation of game C.
   - It produces a MIPS ELF, a PS-X executable and overlay binaries under `build/us/psx/gcc`: [mk/platform/psx.mk:9](ref/dw_decomp/mk/platform/psx.mk:9), lines 47–57.
   - The Makefile does not require a matching build first. CI’s ordering reuses generation; it is not evidence of that dependency.
   - CI only builds GCC binaries; it does not run or compare them (`.github/workflows/build.yaml:60–61`). Successful compilation does not establish preserved behavior.

   S0.1 also omits explicit Python dependencies, recursive SDK headers and sufficiently recent GNU make; grouped targets appear at `mk/platform/psx.mk:77`. The checked-in downloader fetches Linux binaries, and the devcontainer explicitly targets `linux/amd64`.

   **Suggested change to PLAN.md:** Document the complete clean GCC path, preferably initially through the existing container, and label it a compilation/link regression check. State that matching compilation is unnecessary for generation. Keep `disks/us/`: that location and its `.gitignore` coverage are correct. PR #28 and the `local.mk` override are also correctly reported, but do not imply every dependency is a native Mac binary. [PR #28](https://github.com/jype0/dw_decomp/pull/28)

17. **“Everything links before anything runs” delays the most useful failures.**

   **Severity:** major

   **Claim in plan:** Phase 1 compiles all game C with stubs; later phases establish behavior.

   **What’s actually true:** A link can succeed with incorrect widths, useless SDK stubs, wrong overlay resets and deadlocked completion paths. Conversely, “all C” is not the complete program: generated text, extracted data, SDK globals and linker-defined symbols also matter (`mk/text.mk:1–10`; `config/us/main.ld:20–31`; `config/us/overlay.ld:8–21`).

   Existing phase gates are uneven: “title screen matches” does not test input or progress; one replay does not establish an overlay; “sounds right” is useful subjective acceptance but not a timing test. Phases 6–8 lack similarly concrete technical gates.

   **Suggested change to PLAN.md:** Replace the monolithic Phase 1 with runnable slices: host/synthetic backends → asset loading/model draw → title/input → save fixture → first map → battle. Build untouched units separately for coverage. For each slice require named fixtures, expected checkpoints, no unexpected stub calls, bounded completion, and tested re-entry. Add explicit gates for save round-trips, audio duration/event timing, repeat overlay loads and launching the packaged app without Homebrew paths.

18. **Overlay count and size figures need precise scope.**

   **Severity:** minor

   **Claim in plan:** Main plus 16 overlays; approximately 932 KB code and 103 KB data; “twice DW2’s count.”

   **What’s actually true:** There are **16 `*_REL.BIN` entries** in the US file table (`src/main/file_table_us.c:807–822`), but only **15 rebuilt overlay outputs** in `mk/version/us.mk`. SHOP code resides in main (`config/us/main.yaml:132`; `config/us/main.ld:156`), while legacy SHOP file/load references remain. Some VS code also resides in main (`main.yaml:125–127`). DW2 has **seven**, not eight, stage overlays (`CMakeLists.txt:52`).

   Summing C-labeled US YAML ranges gives **931,800 bytes**, consistent with approximately 932 decimal KB of original game machine code. That excludes SDK assembly. The meaning of **103 KB data is unverified**; it is not total initialized content. MOV alone has approximately **469,724 bytes** from its first `.data` offset to file end (`config/us/mov.yaml:65–68`).

   **Suggested change to PLAN.md:** Distinguish on-disc REL files, rebuilt overlays, resident code and active load paths. Label size metrics with their inclusion rules and units. Do not use decompilation progress figures as total porting or asset-volume estimates.

19. **Widescreen is a separate feasibility problem for DW1’s backgrounds.**

   **Severity:** minor

   **Claim in plan:** Widen the camera to 16:9 while leaving menus 4:3.

   **What’s actually true:** DW1’s environment path includes tiled background images, palettes and screen-positioned map layers, not solely a 3D scene that can expose additional geometry. See [map.c:4217](ref/dw_decomp/src/main/map.c:4217), especially tile setup at 4246–4259, and `overworld.c:468` onward. Widening 3D projection alone can misalign actors, backgrounds, occlusion and screen boundaries.

   **Suggested change to PLAN.md:** Make widescreen an optional later feasibility milestone. Test map-edge coverage, background/actor alignment, occlusion and scripted camera movement before promising it. Preserve pillarboxing wherever additional scene content cannot be shown faithfully.

Things the plan gets right and should keep:

- The native-decompilation direction, original-behavior baseline and optional enhancements.
- The GCC route without obtaining CodeWarrior from an unofficial source.
- Software rendering before Metal enhancements.
- Local, hash-verified packs and private oracle captures.
- PCSX-Redux as an instrumentable reference, with repeatability certification before comparison.
- Save backups, bounded implementation slices, human playtesting and a Phase 0 go/no-go—after the gates above are revised.