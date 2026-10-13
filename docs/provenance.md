# S0.2: provenance

2026-10-13. What this repo may take from upstream, and how.

## dw_decomp

Not copied: it's the `decomp/` submodule (Ember's fork `loafykitten/dw_decomp`). This repo
holds only the submodule pointer. Its nested Sony SDK headers (`external/psyq_headers`,
unlicensed) stay in the submodule and reach Rust only through bindgen at build time. Files
the decomp build generates from the disc (`disks/`, `asm/`, `assets/`, `build/`) are
gitignored inside the submodule.

## DW2-PC

gpt-6.1-sol inventoried every file (full table: `docs/provenance/dw2pc-inventory.md`).
Rule: only code DW2-PC's authors wrote (CC0) is eligible, translated into Rust with a credit
note in the file header. Totals: 79 original, 29 mixed, 11 retail/SDK, 1,666 third-party
(mostly vendored tools).

**Eligible (original), with the parts to leave behind:**

| Piece | Files | Notes |
|---|---|---|
| GTE hardware model | `psyq/gte.c` up to line 392, `psyq/gte_core.h` | Lines 394-488 are SDK macro sequences: leave. Its reciprocal table is generated, not copied. |
| GTE test data | `tests/gte_ref.bin`, `tools/gte_probe.py` | Synthetic, made by PCSX-Redux from generated inputs; no game data. `tests/gte_test.c` is mixed (uses the retail atan table). |
| Software GPU | `backend/psxgpu.c`, `psxgpu_hd.c` (+ headers) | The dither matrix is a hardware constant. |
| HD GPU on SDL_GPU | `backend/psxgpu_hw.c`, shaders | Reference only: we render with wgpu. |
| MDEC decoder | `backend/mdec.c` | Leave the literal `quant`/`scale` tables (Psy-Q defaults) and take standard tables (MPEG AC codes, zigzag) from their public specs. |
| SPU model | `backend/psxspu.c` | Leave `psxspu_gauss.inc` and the reverb/ADPCM coefficient tables; re-derive them from psx-spx. |
| XA decoder | `backend/xadec.c` | Same: re-derive the interpolation tables from psx-spx. |
| Library adapters | `psyq/libcd.c`, `libetc.c`, `libgpu.c`, `libmcrd.c`, `libpad.c`, `libpress.c`, `ps1mem.c` | Host-side models, not Sony code. Their headers are "original, unverified" (could be adapted SDK headers): write our own declarations. |
| Host pieces | `host/card.c`, `vblank.c`, `audio.c`, `input.c`, `settings.*`, `sha256.c`, `shot.c` | Mostly superseded by Rust crates; reference for behaviour. |

**Not eligible:**

- **Sound libraries:** `psyq/libsnd.c`, `libspu.c`, `snd_seq.c`, `snd_vm.c`, `snd_internal.h`
  are translated from retail Psy-Q assembly (their own headers say so), and `snd_hw.c`
  embeds retail data. DW1's SEQ sequencer, VAB handling and voice manager must be written
  by us.
- **`psyq/libgs.c`** (follows retail libgs) and **`psyq/libgte.c` lines 57-307**
  (SDK routines) plus its `.incbin` of the retail atan table.
- `host/overlay.c`, `pak.c`, `pakbuild.c`, `tools/dw2pack.py`: DW2-specific and embed or
  read retail data; the *design* (pack + hash manifest) is fine to follow.
- Vendored third-party code (Dear ImGui, miniz, monocypher, stb, GCC binaries, mkpsxiso):
  not copied. If we need one, we take it from its upstream under its own license.

## Sony SDK behaviour: decided (Ember, 2026-10-13)

DW1 calls 175 SDK functions (`docs/survey.md`). DW2-PC got its sound stack and libgs by
translating the SDK's disassembly, which we've ruled out for this repo. Rule: we reimplement
SDK behaviour from public documentation (psx-spx, file-format docs, the SDK's published
manuals' descriptions of the API) and from what the emulator shows, consulting the SDK
disassembly in `decomp/asm/` only to answer narrow questions (an argument order, a
constant, whether a call blocks), and never translating routines line by line into this
repo.
