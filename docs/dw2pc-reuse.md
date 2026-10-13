# S0.6: what we reuse from DW2-PC

2026-10-13. DW2-PC (github.com/Wyrelade/DW2-PC, CC0, commit 2d491b5) is the reference for
the hardware models. `docs/provenance.md` says which of its files we may use. This page
records what S0.6 decided for each piece: translate it into Rust now, link its C for now,
or only read it for behaviour.

**Result: everything we take is translated into Rust now. Nothing is linked as C.**
Translating each model turned out to be cheap, and checking each one bit for bit against
the C was straightforward. That also keeps the endgame (all Rust) from inheriting C later.

| Piece | DW2-PC source | Decision | Where | How it's checked |
|---|---|---|---|---|
| GTE model | `psyq/gte.c` 1-392, `gte_core.h` | Translated | `crates/backend/src/gte.rs` | 1,248 PCSX-Redux reference cases (13 command variants), RTPT, NCT/NCCS/NCCT/AVSZ3/AVSZ4 hand-worked tests, and 48 real libgs calls (S0.7) |
| Software GPU (1x) | `backend/psxgpu.c` | Translated, without the HD/PGXP/SDL_GPU paths | `crates/backend/src/gpu.rs` | `crates/gpu-diff`: 7,500 random GP0 streams, 0 mismatches against the C. Its known differences from hardware are listed in `docs/backend-gpu.md` |
| MDEC + movie bitstream | `backend/mdec.c` | Translated; tables re-derived from MPEG-1 and psx-spx | `crates/backend/src/mdec.rs` | `crates/decoder-diff`, bit-exact against the C |
| SPU model | `backend/psxspu.c` | Translated; Gaussian, reverb and ADPCM tables from psx-spx | `crates/backend/src/spu.rs` | `crates/decoder-diff`, bit-exact against the C |
| XA decoder | `backend/xadec.c` | Translated; tables from psx-spx | `crates/backend/src/xa.rs` | `crates/decoder-diff`, bit-exact against the C |
| Library adapters (`libcd`, `libetc`, `libgpu`, `libmcrd`, `libpad`, `libpress`, `ps1mem`) | `psyq/*.c` | Read for behaviour; written fresh in `crates/psyq` when each phase needs them | — | Against the emulator (oracle replays) |
| Host pieces (`card`, `vblank`, `audio`, `input`, `settings`, `shot`) | `host/*.c` | Read for behaviour; replaced by SDL3 (`docs/s06-platform.md`) and our own host crate | — | — |
| HD GPU | `backend/psxgpu_hw.c` | Not used: we render with wgpu | — | — |

**Not reusable at all** (see `docs/provenance.md`): its sound libraries (`libsnd`, `libspu`,
the sequencer and voice manager), `libgs.c`, `libgte.c` 57-307, and the DW2-specific pack
code. DW1's sequencer, VAB handling and libgs are ours to write, from public docs and
emulator captures (S0.7 showed the method works for libgs).

The `gpu-diff` and `decoder-diff` crates compile DW2-PC's C from the gitignored
`ref/DW2-PC` only for these comparisons. They're test-only and show as ignored without it.
