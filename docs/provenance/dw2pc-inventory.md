# DW2-PC provenance inventory (S0.2)

gpt-6.1-sol (high), read-only, 2026-10-13, DW2-PC at 2d491b5. Summary and decisions in
`docs/provenance.md`.


| Directory / scope | ORIGINAL | MIXED | RETAIL/SDK | THIRD-PARTY | Total |
|---|---:|---:|---:|---:|---:|
| `psyq/` | 24 | 5 | 5 | 0 | 34 |
| `backend/` | 17 | 3 | 1 | 0 | 21 |
| `host/` | 21 | 14 | 0 | 24 | 59 |
| `tests/` | 1 | 1 | 0 | 0 | 2 |
| `tools/` | 14 | 4 | 5 | 1,632 | 1,655 |
| `include/`, immediate files only | 0 | 1 | 0 | 5 | 6 |
| Top-level build/configuration files | 2 | 1 | 0 | 5 | 8 |
| **Total** | **79** | **29** | **11** | **1,666** | **1,785** |

Audited checkout: **`2d491b578002c04e52994d97882f8835c397707b`**. No files were modified, and no files beneath `disc/` or `decomp/disks/` were read.

**The main finding: do not treat `psyq/` as uniformly original.** Several files expressly translate Sony routines from retail disassembly. Removing their embedded tables does not make those implementations eligible under your rule.

Paths below are relative to `ref/DW2-PC/`. `—` means no non-original portion identified. **Unverified entries are not cleared for copying.** Positive ORIGINAL classifications rely on repository attribution and inspected source; the shallow history limits independent authorship verification.

The exhaustive **1,655 per-file `tools/` rows** were emitted in the nine numbered Markdown inventory outputs. The tool decisions relevant to reuse are repeated below.

**`psyq/`**

| Path | Class | Reason | Non-original parts / evidence |
|---|---|---|---|
| `psyq/gte.c` | MIXED | Original hardware model plus SDK macro register sequences. | `gte.c:394–488`: compatibility section; `GteC_*` sequences begin at `gte.c:409`. Core model through `gte.c:392` is the useful original portion. |
| `psyq/gte_core.h` | ORIGINAL | Hardware registers, command encoding and model interface. | — |
| `psyq/gte_native.h` | ORIGINAL, **unverified** | Port-specific C declarations/macros for SDK compatibility; independent header authorship not established. | Hold whole header pending provenance confirmation. |
| `psyq/libcd.c` | ORIGINAL | Host CD command/callback/stream model, rather than translated Sony implementation. | —; consumes player-supplied sectors at runtime. |
| `psyq/libcd.h` | ORIGINAL, **unverified** | Narrow SDK-compatible declarations and types. | Independent reconstruction versus SDK-header adaptation unverified. |
| `psyq/libetc.c` | ORIGINAL | Host VBlank scheduling and callback implementation. | — |
| `psyq/libetc.h` | ORIGINAL, **unverified** | Three SDK-compatible declarations. | Header source independence unverified. |
| `psyq/libgpu.c` | ORIGINAL | Native packet helpers and emulated GPU dispatch. | —; packet formats are hardware/API definitions. |
| `psyq/libgpu.h` | ORIGINAL, **unverified** | Narrow SDK-compatible packet/environment declarations. | Header source independence unverified. |
| `psyq/libgs.c` | MIXED | Explicitly follows retail libgs routines, with original generation/diagnostic additions. | `libgs.c:9–23` establishes origin. Library implementations across `libgs.c:25–523` and `libgs.c:551–640` are not cleared. Generated square-root loop `libgs.c:142–156` and diagnostics `libgs.c:526–549` are separable additions. |
| `psyq/libgs.h` | ORIGINAL, **unverified** | Narrow SDK-compatible structures and declarations. | Header source independence unverified. |
| `psyq/libgte.c` | MIXED | Retail-derived library implementations, extracted atan table, original sine/cosine generation. | `libgte.c:25–32`: `Psyq_RatanTable` `.incbin`; `libgte.c:57–307`: SDK-following routines, established at `libgte.c:8–19`. Original mathematical table generation is `libgte.c:34–55`. |
| `psyq/libgte.h` | ORIGINAL, **unverified** | Narrow SDK-compatible vector/matrix definitions and declarations. | Header source independence unverified. |
| `psyq/libmcrd.c` | ORIGINAL | Newly implemented card filesystem and asynchronous call model from documented card format. | —; provenance description at `libmcrd.c:9–26`. |
| `psyq/libmcrd.h` | ORIGINAL, **unverified** | Narrow SDK-compatible card declarations/constants. | Header source independence unverified. |
| `psyq/libpad.c` | ORIGINAL | Synthesizes controller replies from host input. | — |
| `psyq/libpad.h` | ORIGINAL, **unverified** | Narrow SDK-compatible declarations/constants. | Header source independence unverified. |
| `psyq/libpress.c` | ORIGINAL | Native MDEC adapter, rather than translated libpress implementation. | —; DW2 buffer sizing at `libpress.c:14` should be replaced for DW1. |
| `psyq/libpress.h` | ORIGINAL, **unverified** | Narrow SDK-compatible declarations. | Header source independence unverified. |
| `psyq/libsnd.c` | RETAIL/SDK | Expressly translated from retail Psy-Q assembly. | **Whole implementation**, `libsnd.c:1–1064`; origin at `libsnd.c:5–7`. |
| `psyq/libsnd.h` | ORIGINAL, **unverified** | Narrow SDK-compatible declarations. | Header source independence unverified. |
| `psyq/libspu.c` | RETAIL/SDK | Expressly translated retail Psy-Q functions. | **Whole implementation**, `libspu.c:1–1416`; origin at `libspu.c:1–7`. |
| `psyq/ps1mem.c` | ORIGINAL | Allocates native RAM and scratchpad arenas. | —; DW2 heap addresses in comments at `ps1mem.c:4–5` are integration metadata. |
| `psyq/ps1mem.h` | ORIGINAL | Native address/link conversion helpers. | —; DW2-specific assumptions documented at `ps1mem.h:8–18` need adaptation. |
| `psyq/psyq.h` | ORIGINAL | Port-specific umbrella header. | —; included SDK-compatible headers remain unverified as noted above. |
| `psyq/psyq_log.c` | ORIGINAL | Port diagnostic logging. | — |
| `psyq/psyq_log.h` | ORIGINAL | Port logging interface/macros. | — |
| `psyq/psyq_types.h` | ORIGINAL, **unverified** | Native-width adaptation of SDK type names. | Independent reconstruction versus adapted SDK typedefs unverified. |
| `psyq/psyq_vblank.h` | ORIGINAL | Internal native VBlank hooks. | — |
| `psyq/snd_hw.c` | MIXED | Original memory/DMA/service adapter embeds retail initialization images. | `snd_hw.c:22–40`: `Snd_RetailRodata`, `Snd_RetailData`, initialization and retail callback address; `snd_hw.c:103–105`: retail-global dump addresses/extents. |
| `psyq/snd_internal.h` | RETAIL/SDK | Generated retail SDK call-closure inventory. | Whole declaration list, `snd_internal.h:4–139`. |
| `psyq/snd_native.h` | MIXED | Original native memory helpers include the extracted SDK function inventory. | `snd_native.h:75`: `#include "snd_internal.h"`; retail memory layout described at `snd_native.h:8–22`. |
| `psyq/snd_seq.c` | RETAIL/SDK | Expressly translated retail Psy-Q sequencer. | Whole implementation, `snd_seq.c:1–1193`; origin at `snd_seq.c:1–3`. |
| `psyq/snd_vm.c` | RETAIL/SDK | Expressly translated retail Psy-Q voice manager. | Whole implementation, `snd_vm.c:1–1149`; origin at `snd_vm.c:1–3`. |

`gte.c` generates its UNR reciprocal table mathematically at `gte.c:123–134`; it does **not** embed a retail GTE table. Likewise, `libgte.c` generates sine/cosine tables, and `libgs.c` generates its square-root table. These generators must be distinguished from the literal/extracted `Psyq_RatanTable`.

**`backend/`**

| Path | Class | Reason | Non-original parts / evidence |
|---|---|---|---|
| `backend/mdec.c` | MIXED | Original decoder with literal SDK/default and standardized decoding tables. | `mdec.c:193–202`: `quant`, identified as Psy-Q default; `mdec.c:205–214`: `scale`, identified as Psy-Q scale table. Also `mdec.c:11–53`: ISO MPEG `ac_codes`; `mdec.c:184–189`: standard zigzag ordering. Literal-table source/licensing independence **unverified**. |
| `backend/mdec.h` | ORIGINAL | Native decoder interface and format documentation. | — |
| `backend/pgxp.c` | ORIGINAL | Port-specific precise-vertex tracking and frame pairing; “PGXP-style” does not identify vendored PGXP code. | — |
| `backend/pgxp.h` | ORIGINAL | Precise-vertex interface. | — |
| `backend/psxgpu.c` | ORIGINAL | Native software GPU implementation. | —; literal hardware dither matrix at `psxgpu.c:121–126`, not identified as retail/SDK extraction. |
| `backend/psxgpu.h` | ORIGINAL | GPU interface. | — |
| `backend/psxgpu_hd.c` | ORIGINAL | Port-specific HD software rasterizer. | —; hardware dither matrix at `psxgpu_hd.c:61–66`. |
| `backend/psxgpu_hd.h` | ORIGINAL | HD renderer interface. | — |
| `backend/psxgpu_hd_int.h` | ORIGINAL | Internal renderer structures/interface. | — |
| `backend/psxgpu_hw.c` | ORIGINAL | SDL_GPU renderer implementation. | —; external SDL dependency does not make this vendored SDL code. |
| `backend/psxgpu_hw_spv.h` | ORIGINAL | Generated from the repository’s GLSL shaders. | `psxgpu_hw_spv.h:1` identifies generator; arrays at `:7–381`, `:383–452`, `:454–649`, `:651–688`. Exact regeneration was not performed. |
| `backend/psxspu.c` | MIXED | Original SPU model with literal documented hardware coefficient tables. | `psxspu.c:17`: includes Gaussian table; `psxspu.c:352–356`: `rev_fir`; small ADPCM coefficients at `psxspu.c:177–178`. Table origin/clearance **unverified**; do not assume authors’ CC0 covers them. |
| `backend/psxspu.h` | ORIGINAL | SPU model interface. | — |
| `backend/psxspu_gauss.inc` | RETAIL/SDK, **unverified** | Entire file is the documented PS1 Gaussian coefficient table, not demonstrated author-generated data. | `psxspu_gauss.inc:2–67`: `gauss_tbl[512]`. Direct SDK/ROM extraction versus documentation transcription unverified. |
| `backend/shaders/hd.frag` | ORIGINAL | Port GPU fragment shader. | —; hardware dither matrix at `hd.frag:31`. |
| `backend/shaders/hd.vert` | ORIGINAL | Port GPU vertex shader. | — |
| `backend/shaders/sharpen.frag` | ORIGINAL | Port sharpening shader. | — |
| `backend/shaders/sharpen.vert` | ORIGINAL | Port sharpening vertex shader. | — |
| `backend/sharpen.c` | ORIGINAL | Native software sharpening implementation. | — |
| `backend/xadec.c` | MIXED | Original XA decoder with literal controller interpolation coefficients. | `xadec.c:19–41`: `zigzag[7][29]`, expressly from psx-spx Table1–Table7; small prediction tables at `xadec.c:14–15`. Coefficient origin/clearance **unverified**. |
| `backend/xadec.h` | ORIGINAL | Native XA decoder interface. | — |

The Gaussian, reverb FIR and XA interpolation tables are documented hardware values; that establishes a reference source, **not authorship by DW2-PC**. Their underlying provenance remains unverified for your stricter copying rule. [SPU reference](https://psx-spx.consoledev.net/soundprocessingunitspu/), [XA reference](https://psx-spx.consoledev.net/cdromdrive/), [MDEC reference](https://psx-spx.consoledev.net/macroblockdecodermdec/).

**`host/`**

| Path | Class | Reason | Non-original parts / evidence |
|---|---|---|---|
| `host/audio.c` | ORIGINAL | SDL audio output adapter. | — |
| `host/card.c` | ORIGINAL | Persistent raw card-image files and save-directory handling. | — |
| `host/devdata.c` | MIXED | Original dev-data loader with DW2 identifiers, encoding mappings and references to retail tables. | `devdata.c:20–30`: file IDs and retail table declarations; `:69–89`: glyph mapping; `:268`: model-file IDs; `:408–434`: system-text indices/map IDs; `DevData_InitConst` copies game constants; `:664–678`: game-specific slot/category labels, exact text origin **unverified**. DW2 record decoders throughout require separate adaptation. |
| `host/devdata.h` | MIXED | Original host records with DW2-specific limits and identifiers. | `devdata.h:15–17`, `:34–37`: game ID ranges/mode metadata; independent derivation of remaining game record details **unverified**. |
| `host/devedit.c` | MIXED | Original edit queue/UI support plus DW2 encoding and scene/state conventions. | `devedit.c:14`: game header; `:43`: domain count; `:109–135`: encoding mapping; `:176–241`: scene/task conventions; `:745–771`: game modes. Game-state setter conventions remain DW2-derived. |
| `host/devedit.h` | MIXED | Original edit interface containing game-specific schema/range information. | `devedit.h:17–33`, `:49`, `:80–82`: game limits, nickname width and encoding metadata; provenance of these values **unverified**. |
| `host/devedit40.c` | MIXED | Original adapter reads a decompiled game structure with a DW2 overlay ID. | `devedit40.c:5`: game header; `:14`: overlay ID `1`. |
| `host/devsnap.c` | MIXED | Original snapshot machinery plus DW2 state access and reproduced flag conventions. | `devsnap.c:13`: game header; `:277`: retail RNG table read; `:317–384`: `DevSnap_FlagTest`, expressly reproducing `Flag_Test` ranges. |
| `host/devsnap.h` | MIXED | Original snapshot structures sized around DW2 state. | `devsnap.h:15–21`: game counts/layout size; `:40–47`: game roster widths. |
| `host/devui.cpp` | MIXED | Original ImGui UI with DW2-specific ranges/default IDs/mode conventions. | `devui.cpp:731–734`, `:1285`, `:1340`, `:1353`, `:1396–1429`; broader game-specific labels/schema source independence **unverified**. |
| `host/devui.h` | ORIGINAL | Port dev-overlay interface. | — |
| `host/dw2.rc` | ORIGINAL | References the port’s generated original icon. | `dw2.rc:3` references `misc/icon/dw2.ico`; generator declares original artwork and no game data. |
| `host/host.h` | ORIGINAL | Port-specific host interfaces. | — |
| `host/host_sdl.h` | MIXED | Original host interfaces with DW2 pack file count. | `host_sdl.h:38`: `PAK_FILES = 0xE5B`; pad bits `:20–35` are hardware protocol definitions. |
| `host/icon_rgba.h` | ORIGINAL | Generated original port artwork, not a dumped retail icon. | Array `icon_rgba.h:4–1029`; origin declaration at `:1`, supported by `misc/icon/make_icon.py:1–12`. Generator uses a system font; exact rendered-font provenance **unverified**. |
| `host/imgui/LICENSE.txt` | THIRD-PARTY | Dear ImGui MIT license. | Whole file, `LICENSE.txt:1–EOF`. |
| `host/imgui/README.txt` | ORIGINAL | DW2-PC’s vendor/version inventory and update instructions. | —; distinguishes this authored file from unchanged upstream files at `README.txt:7–10`. |
| `host/imgui/backends/imgui_impl_sdl3.cpp` | THIRD-PARTY | Dear ImGui SDL3 backend, MIT. | Whole file, `imgui_impl_sdl3.cpp:1–EOF`. |
| `host/imgui/backends/imgui_impl_sdl3.h` | THIRD-PARTY | Dear ImGui SDL3 backend, MIT. | Whole file, `imgui_impl_sdl3.h:1–EOF`. |
| `host/imgui/backends/imgui_impl_sdlrenderer3.cpp` | THIRD-PARTY | Dear ImGui SDL renderer backend, MIT. | Whole file, `imgui_impl_sdlrenderer3.cpp:1–EOF`. |
| `host/imgui/backends/imgui_impl_sdlrenderer3.h` | THIRD-PARTY | Dear ImGui SDL renderer backend, MIT. | Whole file, `imgui_impl_sdlrenderer3.h:1–EOF`. |
| `host/imgui/imconfig.h` | THIRD-PARTY | Dear ImGui, MIT. | Whole file, `imconfig.h:1–EOF`. |
| `host/imgui/imgui.cpp` | THIRD-PARTY | Dear ImGui, MIT. | Whole file, `imgui.cpp:1–EOF`. |
| `host/imgui/imgui.h` | THIRD-PARTY | Dear ImGui, MIT. | Whole file, `imgui.h:1–EOF`. |
| `host/imgui/imgui_demo.cpp` | THIRD-PARTY | Dear ImGui, MIT. | Whole file, `imgui_demo.cpp:1–EOF`. |
| `host/imgui/imgui_draw.cpp` | THIRD-PARTY | Dear ImGui, MIT; includes upstream font/other contributed material. | Whole file, `imgui_draw.cpp:1–EOF`; do not relabel its embedded arrays CC0. |
| `host/imgui/imgui_internal.h` | THIRD-PARTY | Dear ImGui, MIT. | Whole file, `imgui_internal.h:1–EOF`. |
| `host/imgui/imgui_tables.cpp` | THIRD-PARTY | Dear ImGui, MIT. | Whole file, `imgui_tables.cpp:1–EOF`. |
| `host/imgui/imgui_widgets.cpp` | THIRD-PARTY | Dear ImGui, MIT. | Whole file, `imgui_widgets.cpp:1–EOF`. |
| `host/imgui/imstb_rectpack.h` | THIRD-PARTY | ImGui-modified Sean Barrett stb; MIT or Unlicense upstream, with ImGui MIT modifications. | Whole file, `imstb_rectpack.h:1–EOF`. |
| `host/imgui/imstb_textedit.h` | THIRD-PARTY | ImGui-modified Sean Barrett stb; MIT or Unlicense upstream, with ImGui MIT modifications. | Whole file, `imstb_textedit.h:1–EOF`. |
| `host/imgui/imstb_truetype.h` | THIRD-PARTY | ImGui-modified Sean Barrett stb; MIT or Unlicense upstream, with ImGui MIT modifications. | Whole file, `imstb_truetype.h:1–EOF`. |
| `host/input.c` | ORIGINAL | Native input adapter. | — |
| `host/main.c` | MIXED | Original native entry point with DW2 memory-layout assumptions. | `main.c:219`: retail heap bounds `0x80075000`/`0x801FF000`; game linkage/integration assumptions must be adapted. |
| `host/miniz/LICENSE` | THIRD-PARTY | miniz MIT license. | Whole file, `LICENSE:1–EOF`. |
| `host/miniz/miniz.c` | THIRD-PARTY | Rich Geldreich/miniz, MIT notices in this copy. | Whole file, `miniz.c:1–EOF`. |
| `host/miniz/miniz.h` | THIRD-PARTY | miniz header; project ships MIT license, header also contains historical public-domain wording. | Whole file, `miniz.h:1–EOF`; exact historical upstream snapshot **unverified**. |
| `host/monocypher/LICENCE.md` | THIRD-PARTY | Monocypher, BSD-2-Clause **or CC0**. | Whole file, `LICENCE.md:1–EOF`. |
| `host/monocypher/monocypher-ed25519.c` | THIRD-PARTY | Monocypher Ed25519, BSD-2-Clause or CC0. | Whole file, `monocypher-ed25519.c:1–EOF`. |
| `host/monocypher/monocypher-ed25519.h` | THIRD-PARTY | Monocypher Ed25519, BSD-2-Clause or CC0. | Whole file, `monocypher-ed25519.h:1–EOF`. |
| `host/monocypher/monocypher.c` | THIRD-PARTY | Monocypher, BSD-2-Clause or CC0. | Whole file, `monocypher.c:1–EOF`. |
| `host/monocypher/monocypher.h` | THIRD-PARTY | Monocypher, BSD-2-Clause or CC0. | Whole file, `monocypher.h:1–EOF`. |
| `host/overlay.c` | MIXED | Original overlay-reset machinery embeds retail boot image and overlay ordering. | `overlay.c:35`: `Ovl_LoadArea` `INCLUDE_BIN`; `:46–49`: retail overlay-ID order. |
| `host/pak.c` | MIXED | Original pack reader consumes retail file tables and embeds disc-derived manifest metadata. | `pak.c:19–21`: `Cd_FileLba`/`Cd_FileSectors`; `:26–32`: `Pak_Manifest` `.incbin`. Manifest contains hashes, **not retail payload bytes**. |
| `host/pakbuild.c` | MIXED | Original image/pack I/O with DW2 executable table offsets. | `pakbuild.c:21–22`: `EXE_LBA_TABLE`, `EXE_SECTOR_TABLE`; `:691–695`: reads those tables; file count comes from `host_sdl.h:38`. |
| `host/sdl.c` | ORIGINAL | Port window/presentation/thread adapter. | — |
| `host/settings.c` | ORIGINAL | Port settings implementation. | — |
| `host/settings.h` | ORIGINAL | Port settings interface/schema. | — |
| `host/settingsui.cpp` | ORIGINAL | Port settings UI. | —; uses external ImGui/SDL APIs. |
| `host/sha256.c` | ORIGINAL | Native SHA-256 implementation with standard FIPS constants. | —; `sha256.c:5–14` is the standardized round-constant array. |
| `host/sha256.h` | ORIGINAL | SHA-256 interface. | — |
| `host/shot.c` | ORIGINAL | Authored PNG/deflate screenshot encoder. | —; arrays at `shot.c:203–210` describe standard deflate coding. |
| `host/trace.c` | MIXED | Original tracing with DW2 scene identifiers. | `trace.c:6`: game header; `:30`: scene table IDs; `:39`: title mode/overlay IDs. |
| `host/ui.cpp` | ORIGINAL | Port ImGui integration layer. | —; upstream implementation remains separately licensed. |
| `host/ui.h` | ORIGINAL | Port UI interface. | — |
| `host/update.c` | ORIGINAL | Port updater; its public-key array is project signing metadata. | —; `update.c:59–62`: `update_pubkey[32]`, not retail data. |
| `host/update.h` | ORIGINAL | Port updater interface/version metadata. | — |
| `host/vblank.c` | ORIGINAL | Native pacing and host-event scheduling. | — |
| `host/winconsole.c` | ORIGINAL | Native Windows console attachment. | — |

The ImGui pin and unchanged-file list are explicit in `host/imgui/README.txt:3–10`: `ocornut/imgui`, tag `v1.92.9b`, commit `f1cc2ae15e53a861a874c3034aae6798fde194ab`. SDL is an external dependency here; there are no vendored SDL implementation files in these audited directories.

**`tests/`, immediate `include/` files, and top-level files**

| Path | Class | Reason | Non-original parts / evidence |
|---|---|---|---|
| `tests/gte_ref.bin` | ORIGINAL, **unverified reproduction** | Synthetic command results generated through PCSX-Redux, expressly without game data. | Whole binary generation described at `tools/gte_probe.py:1–13`; exact regeneration/hash verification not performed. It is **not established as a retail dump**. |
| `tests/gte_test.c` | MIXED | Original randomized test harness plus retail-table comparison and SDK defaults. | `gte_test.c:28`: extracted atan-table dependency; `:176–198`: retail executable/table addresses; `:219–221`: SDK initialization values. Synthetic command-case logic is separable. |
| `include/common.h` | THIRD-PARTY, **unverified** | Standard decompilation scaffold with port additions; inherited authorship boundary not established. | Hold whole file; common scaffold at `common.h:12–29`. |
| `include/gte.h` | MIXED | Native wrapper plus expressly identified SDK `inline_n.h` assembly sequences. | `gte.h:14–34`: SDK forms; custom unaligned load at `gte.h:19` is separately identified. |
| `include/gte_macros.inc` | THIRD-PARTY, **unverified inheritance** | Shared GAS decompilation macros, also present in credited PE2 project; ultimate authorship/license chain unverified. | Whole file, `gte_macros.inc:1–415`. |
| `include/include_asm.h` | THIRD-PARTY | Inherited decompilation include machinery with substantial port additions; upstream PE2 project declares CC0. | Shared machinery around `include_asm.h:69–98`; exact inherited boundary **unverified**. Generic `INCLUDE_BIN` macro definitions themselves contain no retail bytes. |
| `include/labels.inc` | THIRD-PARTY | Shared decompilation label macros with local changes; PE2 project declares CC0. | Whole scaffold, `labels.inc:1–EOF`; exact inherited boundary **unverified**. |
| `include/macro.inc` | THIRD-PARTY | Shared GAS decompilation scaffold; PE2 project declares CC0. | Whole file, `macro.inc:1–EOF`; ultimate origin **unverified**. |
| `.clang-format` | THIRD-PARTY, **unverified inheritance** | Formatting scaffold shared with credited PE2 project. | Whole file, `.clang-format:1–EOF`; copying direction/historical origin unverified. |
| `.clang-tidy` | THIRD-PARTY, **unverified inheritance** | Decompilation naming/configuration scaffold; original authorship not established. | Whole file, `.clang-tidy:1–EOF`. |
| `.gitattributes` | ORIGINAL | Repository merge/vendor classification rules. | —; explicitly identifies Sony SDK assembly/data elsewhere at `.gitattributes:18–22`. |
| `.gitignore` | THIRD-PARTY, **unverified inheritance** | Decompilation/build ignore scaffold; inherited boundary not established. | Whole file, `.gitignore:1–EOF`. |
| `.recomp.json` | MIXED | Authored project metadata includes retail identity/hash. | `.recomp.json:7–11`: original region/serial/SHA-1 metadata. |
| `CMakeLists.txt` | ORIGINAL | Authored native build recipe; dependencies do not turn recipe text into retail code. | External dependencies at `CMakeLists.txt:226–234`: manifest, atan table, retail sound images. Those generated inputs remain excluded. |
| `Dockerfile` | THIRD-PARTY | Shared PE2 build-container recipe; upstream project declares CC0. | Whole file, `Dockerfile:1–EOF`; exact historical copying direction **unverified**. |
| `requirements.txt` | THIRD-PARTY, **unverified inheritance** | Decompilation dependency scaffold plus project additions. | Whole file, `requirements.txt:1–EOF`; package names are not vendored implementations. |

The shared-source findings are supported by the credited upstream’s [Dockerfile](https://raw.githubusercontent.com/GabeRealB/parasite-eve-2-decomp/main/Dockerfile), [include machinery](https://raw.githubusercontent.com/GabeRealB/parasite-eve-2-decomp/main/include/include_asm.h), [GTE macros](https://raw.githubusercontent.com/GabeRealB/parasite-eve-2-decomp/main/include/gte_macros.inc), and [CC0 license](https://raw.githubusercontent.com/GabeRealB/parasite-eve-2-decomp/main/LICENSE). Shared presence establishes that these cannot confidently be credited solely to DW2-PC; it does not establish every historical copying boundary.

**Project-owned and standalone `tools/` decisions**

| Path | Class | Reason | Non-original parts / evidence |
|---|---|---|---|
| `tools/asm_normalizer.py` | ORIGINAL | Generic target-guided assembly rewriter; no embedded game/SDK corpus identified. | —; inputs and rewritten outputs may still be retail-derived. |
| `tools/asm_normalizer_manifest.json` | ORIGINAL | Empty `{}` manifest. | — |
| `tools/asm_restore.py` | ORIGINAL | Authored extraction/restoration workflow. | —; restored game/SDK assembly output is not original authors’ code. |
| `tools/build_dw2.py` | MIXED | Authored matching build driver with retail metadata and extraction integration. | `build_dw2.py:35`, `:48–56`, `:65`, `:103–125`: hashes/sizes/compiler-layout metadata. `assemble_bins`, `extract_bins`, `patch_image_bytes` at `:632–726` consume excluded retail inputs. |
| `tools/build_native.py` | ORIGINAL | Native build orchestration. | —; `build_native.py:51–66` extracts externally supplied native blobs, which remain excluded. |
| `tools/build_shaders.py` | ORIGINAL | GLSL-to-SPIR-V header generator. | — |
| `tools/check_asm_guard.py` | ORIGINAL | Source-policy check. | — |
| `tools/decomp-permuter-objdump.patch` | THIRD-PARTY | Patch includes MIT decomp-permuter source context. | Whole patch, `:1–EOF`. |
| `tools/decomp-permuter-sizeof.patch` | THIRD-PARTY | Patch includes MIT decomp-permuter source context. | Whole patch, `:1–EOF`. |
| `tools/dw2pack.py` | MIXED | Authored raw-sector/pack/manifest builder with DW2 constants. | `dw2pack.py:28`, `:31–33`, `:45–54`: counts/offsets/hashes/file IDs; `check_known_content` at `:147–177`, particularly `:170–175`, contains game-specific validation values. |
| `tools/gte_probe.py` | ORIGINAL | Synthetic probe generator and emulator automation. | —; no game data involved per `gte_probe.py:1–13`; BIOS is only an external runtime input. |
| `tools/m2ctx.py` | THIRD-PARTY | Shared PE2 context-generation helper, upstream CC0. | Whole file; inherited boundary **unverified**. |
| `tools/maspsx-lo-load-nop.patch` | THIRD-PARTY | Patch includes MIT maspsx source context. | Whole patch, `:1–EOF`. |
| `tools/match_status.json` | MIXED | Authored status metadata includes retail-address-bearing symbol names. | `match_status.json:783–862`, `:962`, `:995–997`, `:1052`, `:1631`. No instruction/data payload identified. |
| `tools/objdiff/gen_objdiff_json.py` | ORIGINAL | Authored comparison configuration generator. | — |
| `tools/ovl_setup.py` | MIXED | Authored overlay analyzer with DW2 load address/tail details. | `ovl_setup.py:8–9`, `:15`: retail file extent/load address. |
| `tools/package_release.py` | ORIGINAL | Authored packaging/signature workflow. | — |
| `tools/rename.py` | ORIGINAL | Authored symbol-renaming logic. | —; game symbol examples are integration metadata. |
| `tools/rename_field.py` | ORIGINAL | Authored compiler-guided renaming logic. | — |
| `tools/rodata_own.py` | ORIGINAL | Authored reference/ownership analysis. | —; produced rodata/assembly is separately excluded. |
| `tools/score_functions.py` | THIRD-PARTY | Shared PE2 scoring implementation, upstream CC0. | Whole file; inherited boundary **unverified**. |
| `tools/strict_report.py` | ORIGINAL | Authored comparison/status report generator. | — |
| `tools/update_readme_progress.py` | ORIGINAL | Authored progress updater. | — |

The shared tool implementations can be compared with upstream [m2ctx.py](https://raw.githubusercontent.com/GabeRealB/parasite-eve-2-decomp/main/tools/m2ctx.py) and [score_functions.py](https://raw.githubusercontent.com/GabeRealB/parasite-eve-2-decomp/main/tools/score_functions.py).

For the remaining tools, every file is excluded under your “DW2-PC authors only” rule, regardless of whether its separate license is permissive:

| Subtree / files | Classification and origin |
|---|---|
| `tools/asm-differ/` | THIRD-PARTY; Simon Lindholm’s asm-differ, **Unlicense**. Screenshot embedded-content provenance unverified. |
| `tools/decomp-permuter/` | THIRD-PARTY; Simon Lindholm and contributors, **MIT**. Nested pycparser/PLY have **BSD-3-Clause** licenses; nested systray-portable fork has **MIT**. Prebuilt dependency/artwork origins unverified. |
| `tools/m2c/` | THIRD-PARTY; `matt-kempster/m2c`, **GPL-3.0-only** (`pyproject.toml:12`). Nested pycparser and PLY are **BSD-3-Clause**. Individual fixture source independence unverified. |
| `tools/maspsx/` | THIRD-PARTY; Mark Street’s `mkst/maspsx`, **MIT**. Local modifications retain upstream provenance. |
| Linux/macOS GCC binaries | THIRD-PARTY; GNU GCC 2.x/old-gcc builds, expected **GPL-2.0-or-later**; exact binary/patch chain unverified. |
| Linux/macOS/Windows mkpsxiso binaries | THIRD-PARTY; Lameguy64/mkpsxiso, **GPL-2.0**; exact binary release/dependencies unverified. [Upstream license](https://raw.githubusercontent.com/Lameguy64/mkpsxiso/master/LICENSE.md). |
| Windows binutils and modern MinGW GCC binaries | THIRD-PARTY; GNU tools, expected **GPL-3.0-or-later**; exact binary provenance unverified. |
| Windows mcpp | THIRD-PARTY; Kiyoshi Matsui, **BSD-2-Clause**, local `LICENSE`. |
| Windows iconv distribution | THIRD-PARTY; Michele Locati wrapper **MIT**, iconv/gettext **GPL-3.0+**, CLDR **Unicode License V3**; binary provenance unverified. |
| `tools/windows/gcc-2.7.2-win/CC1PSX.EXE` | RETAIL/SDK, **unverified**; whole binary, bytes `0–EOF`. |
| `tools/windows/gcc-psx/CC1PLPSX.EXE` | RETAIL/SDK, **unverified**; whole binary, bytes `0–EOF`. |
| `tools/windows/gcc-psx/CC1PSX.EXE` | RETAIL/SDK, **unverified**; whole binary, bytes `0–EOF`. |
| `tools/windows/gcc-psx/CCPSX.EXE` | RETAIL/SDK, **unverified**; whole binary, bytes `0–EOF`. |
| `tools/windows/gcc-psx/CPPPSX.EXE` | RETAIL/SDK, **unverified**; whole binary, bytes `0–EOF`. |

The SDK-distributed compiler executables may contain GNU-derived components; their exact patch/distribution licensing was not established. The RETAIL/SDK label is a conservative exclusion, not a finding that every byte is proprietary Sony code. Binary entries in the appendix have no meaningful textual line numbers; the exclusion is the whole byte sequence.

**Binary inclusion and extraction inventory**

These are the manifest-declared sources, reproduced without opening the source disc files. All resulting blobs are **RETAIL/SDK**, not eligible CC0 assets.

| Asset / symbol | Retail source offset and size | Inclusion / manifest evidence |
|---|---|---|
| `Stg11_CardIconTim1` | `STAG1100.PRO`, `0x4ED0`, `0xC0` | `src/stag1100/card.c:10`; `configs/USA/include_bin.txt:6` |
| `Stg11_CardIconTim2` | `STAG1100.PRO`, `0x4F90`, `0xC0` | `src/stag1100/card.c:11`; manifest `:7` |
| `Stg11_CardIconTim3` | `STAG1100.PRO`, `0x5050`, `0xC0` | `src/stag1100/card.c:12`; manifest `:8` |
| `Stg40_AutomapClut` | `STAG4000.PRO`, `0xF5E8`, `0x40` | `src/stag4000/automap.c:23`; manifest `:9` |
| `Stg00_FontGlyphs` | `STAG0000.PRO`, `0x5770`, `0x200` | `src/stag0000/font.c:7`; manifest `:10` |
| `Stg10_VlcTablePacked` | `STAG1000.PRO`, `0x2078`, `0xE10` | `src/stag1000/stag1000_tail.c:5`; manifest `:11` |
| `Rand_Table` | `SLUS_011.93`, `0x31F04`, `0x2000` | `src/main/sys.c:68`; manifest `:12` |
| `Cd_FileLba` | `SLUS_011.93`, `0x33F94`, `0x396C` | `src/main/cd.c:48`; manifest `:13` |
| `Cd_FileSectors`, matching build | `SLUS_011.93`, `0x37900`, `0x810` | `src/main/cd.c:53`; manifest `:14` |
| `Cd_FileSectors`, native build | `SLUS_011.93`, `0x37900`, `0x1CB6` | `src/main/cd.c:51`; `include_bin_native.txt:8` |
| `Psyq_RatanTable` | `SLUS_011.93`, `0x3E5C0`, `0x802` | `psyq/libgte.c:30`; native manifest `:11` |
| `Snd_RetailRodata` | `SLUS_011.93`, `0x800`, `0x1000` | `psyq/snd_hw.c:27`; native manifest `:15` |
| `Snd_RetailData` | `SLUS_011.93`, `0x28800`, `0x2B360` | `psyq/snd_hw.c:31`; native manifest `:16` |
| `Ovl_LoadArea` | Boot TIM/initial overlay image extracted by splat; builder references executable offset `0x53B60` | `host/overlay.c:35`; `tools/dw2pack.py:33` |

Additional inclusion findings:

- `host/pak.c:29` embeds **hash metadata** from `configs/USA/pack_manifest.txt`, not game payload.
- `include/include_asm.h:23–31` and `:119–127` define generic binary-inclusion mechanisms.
- `tools/build_dw2.py:651` generates `.incbin` wrappers for external extracted assets.
- `tools/m2c/tests/end_to_end/incbin/manual.s:3,6,9` is a third-party synthetic inclusion fixture. Its `test.bin` is the eight bytes `01 02 03 04 05 06 07 08`, not identified retail content.
- No `#embed` occurrence was found in the scanned tree.

The large native arrays were individually checked: Gaussian/FIR/XA/MDEC tables are held as described above; SPIR-V, icon pixels, SHA constants, deflate tables and signing key have different stated origins. Vendored parser tables, ImGui/font data and cryptographic tables remain THIRD-PARTY.

**History coverage**

I ran the requested deletion search, also with `--all`:

```text
git -C ref/DW2-PC log --diff-filter=D --name-only
git -C ref/DW2-PC log --all --diff-filter=D --name-only
```

Both returned **no deletion entries** in the available history.

The clone contains exactly **50 reachable commits** and is shallow. Its boundary is:

```text
178ab4d51bf42f3c60a6c87f8e719a8f9e375354
2026-10-09 — Dev overlay: game names and a Data tab
```

Therefore I could check deletions represented inside that available history, but **could not check earlier removals, the boundary commit’s unavailable parent diff, or the original decompilation repository’s full history**. No history was fetched or checkout files changed. “No deletions found” does not establish that retail data was never committed and removed.

**Most useful pieces for DW1**

| Piece | Classification / reuse boundary |
|---|---|
| **GTE model** | `psyq/gte.c` is MIXED; core through `:392` and `gte_core.h` are the original candidates. Exclude SDK-sequence wrappers and retail-following `libgte`/`libgs` routines. |
| **GPU rasterizer** | `backend/psxgpu*`, PGXP support and shaders: ORIGINAL candidates; keep external SDL licensing separate. |
| **SPU/reverb** | `backend/psxspu.c`: MIXED; remove/replace the Gaussian and literal FIR tables. `psyq/libsnd.c`, `libspu.c`, `snd_seq.c`, `snd_vm.c`: RETAIL/SDK. |
| **XA decoder** | `backend/xadec.c`: MIXED; original decoding/FIFO logic, literal interpolation coefficients unverified. |
| **MDEC decoder** | `backend/mdec.c`: MIXED; original decoder logic, literal Psy-Q/default and standardized tables need separate replacement/provenance. |
| **Pack/manifest builder** | `host/pakbuild.c`, `host/pak.c`, `tools/dw2pack.py`: MIXED; reuse original I/O/hash/container logic with independently supplied DW1 offsets, counts, tables and manifest. |
| **Card files** | `host/card.c` and `psyq/libmcrd.c`: ORIGINAL implementation candidates; SDK-compatible header authorship remains unverified. |