# dw1-silicon: Digimon World (PS1) as a native Apple Silicon app

Handoff plan, written 2026-10-13 for whichever agent continues this work, and revised the
same day after an adversarial review by GPT 6 Astra (high) (`docs/plan-review-1.md`). Read it
whole before starting, and don't re-ask Ember anything answered here. Keep the
"Where things stand" section current at the end of every session: it is the handoff.

## Where things stand (2026-10-14, session 3): start here

- **Phase 4 gate replays done (2026-10-14):** `oracle/replays/file-city-walk.txt` and
  `care-fixture.txt`, native checkpoints all identical; `GsSortOt` implemented (DIGIMON status
  screen); decomp fork branch `dw1-native-phase4` (local, to push) fixes the digivice's ordering
  tables. `oracle/explore.lua` finds routes turn-based. Keep Redux routes under VBlank ~32700
  (Lua hooks fail near 32768). See `docs/phase4-plan.md`.
- **Phases 2 and 3 are done; Phase 4 is under way** (`docs/phase2-report.md`,
  `docs/phase3-plan.md`, `docs/phase2-plan.md` for the slices). The `new-game` replay
  (`oracle/replays/new-game.txt`, 7,131 polls) runs natively from boot to its last poll, first
  gameplay with the tamer and Agumon, with all 19 canonical checkpoints byte-identical to the
  emulator's, in about 18 s. Frames from the landing screen to the main menu are
  pixel-identical; new-game and File City frames differ only on 3D models (8-211 px a frame,
  rasteriser rounding on Gouraud/semi-transparent polygons). Phase 3: Ember's save loads
  through CONTINUE GAME (`oracle/replays/continue-fixture.txt` with `DW1_CARD1=` the fixture):
  checkpoints 0-5, including the load and File City, are identical; the replay then stops at
  TMD mode 0x3E (a semi-transparent quad) in `GsTMDfastTG4L`, which an agent was implementing at
  the end of the session along with a survey of every TMD kind the routes use. **Next (Phase
  4):** the rest of the TMD kinds, frames of the models exact, then the Phase 4 gate (a replay to
  the first walk around File City with state equal at each map change, and a fixture-started
  hour of care). Reviews open at the session's end: gpt-6.1-sol on the 2D sorts (80a8e2d,
  0ffa5c6) and on Phase 3; GPT 6 Astra's second pass on the rasteriser (952738e, 925c0c0).
  - **Rasteriser:** `backend::gpu` now follows Redux's softgpu, fitted by black-box draws
    (`crates/backend/tests/gpu_oracle.rs`, `tools/oracle/gpu_requests.py`; local results in
    `captures/oracle-runs/gpu-prims`, `gpu-snap`): round-to-nearest interpolation from the left
    edge, quads drawn v1v2v3 then v0v1v2, the odd-row last-pixel rule for semi-transparent
    modulated textures. `crates/gpu-diff` still checks the rest against DW2-PC's C.
  - **Time:** native loading takes no time, so native VBlanks run ahead of Redux's; replays
    pair by poll. Waits on hardware must take the emulator's VBlanks: memory-card timings are
    measured (`crates/psyq/src/libmcrd.rs`). `DW1_REPLAY_LOG=1` shows each poll's native and
    recorded VBlank; frames pair as recorded + (native − native at the last poll).
  - **Fixture save (Phase 3):** `captures/fixtures/ember-day2-gabumon.mcd` (local, gitignored;
    notes and SHA-1 in `captures/fixtures/README.txt`): Ember's DuckStation card, one save,
    Gabumon on about day 2. DuckStation is set up for Ember (`~/Applications/DuckStation.app`,
    launched with `dw1-play`; cards in `~/Library/Application Support/DuckStation/memcards/`).
  - **Booklab** was unreachable after a power cut on 2026-10-13 (ssh timed out); emulator work
    ran on this Mac, one Redux at a time.
  - **Native canonical state:** `crates/dw1/src/canon.rs` collects the S0.8 record (schema v2,
    834 bytes) from the game's C globals; the headless replay (`DW1_REPLAY`, `DW1_OUT`) writes
    `ckpt_NNNN_<why>.{bin,txt}` at `oracle/replay.lua`'s triggers (`DW1_CHECKPOINTS=0` turns it
    off). Compare with `tools/oracle/compare_canon.py NATIVE_DIR ORACLE_DIR --allow-unpaired`.
    Checkpoints 0-3 (poll 0, poll 600) match the emulator's exactly, bytes included.
  - **3D models draw natively (2026-10-14):** `GsSortObject4J` and `GsTMDfastTG3L`/`TG4L` are
    native (`libgs/sort4j.rs`, bit-exact on 228 captured calls; only those two kinds occur up
    to Redux VBlank 5238). The new-game scene's frames equal Redux's up to rasteriser rounding.
    `DW1_CRASH_TRACE=1` prints a call
    stack on SIGSEGV (lldb can't attach in the agent sandbox). `oracle/capture-calls.lua` has
    `DW1_CAPTURE_KINDS=1` (per-call attribute and primitive kinds of `GsSortObject4J`).
  - **2D sorts, TIM and CLUT calls are native (2026-10-14, slice 6f):** `GsSortBoxFill`,
    `GsSortPoly`, `GsSortSprite` (`libgs/sort2d.rs`), `LoadClut`, `OpenTIM`, `ReadTIM`
    (`libgpu.rs`). **The replay now runs from boot to the last poll (7131) with no trap and all
    19 canonical-state checkpoints identical to the emulator's** (`compare_canon.py ... --allow-unpaired`:
    19 paired, 19 identical). Not reached by the route, so still trapping stubs: `GsSortFastSprite`,
    `GsSortOt`, `SetDrawLoad` (battle effects). `GsSortSprite` traps on scaling, attribute bits
    (blend, colour mode, perspective) and other colours. Tests: `tests/libgs_2d_capture.rs`
    (287 captured calls), `tests/libgpu_capture.rs`, `tests/libgs_sprite_oracle.rs` (1,000
    `call-sdk.lua` requests: `tools/oracle/sprite_requests.py`), `tests/libgs_capture.rs` (388 calls,
    now also `s4j-city*` and the semi-transparent 0x36 triangle). Shared loader: `tests/common`.
    Captures live in `captures/oracle-runs/{g2d-box-*,g2d-spr*,g2d-tim,g2d-clut,libgs-sprite,s4j-city*}`
    (a test directory name must start with `g2d-box-`/`g2d-spr`/`s4j-` to be picked up).
    Fork changes this took (`dw1-native`, commits da1da55, b32edc6 and 2889e7a on top of 62ff326;
    not pushed from the worktree that wrote them): null entity slots in `setupNameSelectorBox`,
    `setupNameDisplayBox` and `clearMapDigimon`, and `uploadMapTileImages`, which read
    `MapTiles.width` at the PS1 offset 0x1bb8 (wrong with 8-byte pointers; File City's ground was
    black and noisy until fixed). **Frames** (headless shots vs Redux, aligned by poll): 134
    new-game-scene frames (Redux VBlanks 3603-5199) differ in 8-20 pixels each, all on the 3D
    models; 14 File City frames (13203-18003) in 24-211 pixels, models only (the largest is
    the semi-transparent 0x36 object, blending rounding); no 2D difference anywhere.
    `compare_canon.py`: 19 of 19 identical. Redux runs for these are in `captures/oracle-runs/`
    (`shots-g2d`, `shots-city`).
  - **Emulator tools added:** `oracle/capture-calls.lua` (record chosen SDK calls with full
    machine state; `tools/oracle/calldiff.py` shows what each changed) and
    `oracle/call-sdk.lua` (run SDK functions on chosen inputs). Timeline of the route at boot:
    the game's `main()` starts at Redux VBlank 1144 (native VBlank 0), the intro movie plays
    ~1454-1800, the landing screen ~1825-1925, the main menu from ~2050.
- **Phase 1 is done** (`docs/phase1-report.md`). All 119 US game C files (main, SHOP, all 15
  overlays) compile with Apple clang and link whole into `dw1`, with every SDK call resolved
  (187 trapping stubs in `crates/psyq/src/stubs.rs`). `cargo run -p dw1` runs the game's own
  `main()` until its first SDK call, `InitHeap3`, and stops there with that name. **Next:
  Phase 2** (boot to the main menu), starting with the unprototyped-call audit (958 calls to
  32 functions; report, "Warnings") and the heap.
  - **How the native build keeps PS1 meaning** (`crates/decomp-c/include/dw1_native.h`,
    force-included): `long` is 32-bit (`#define long int`, after the host's `<stdint.h>`),
    `NULL` is a pointer, libc calls are renamed `dw1_*` (BIOS semantics from psx-spx, in
    `crates/psyq/src/libc.rs`), `main` is `dw1_game_main`. Native `inline_n.h` (GTE macros
    through `crates/psyq/src/gte.rs`), `setjmp.h` (host) and `__rts_info_t__.h` replace the
    PS1-only headers.
  - **Build** (`crates/decomp-c/build.rs`): file lists from `decomp/mk/version/us.mk`; text
    headers from the decomp's `gen_text.py`; non-ASCII string literals re-encoded as CP932 with
    Python's codec (58 strings checked byte-identical to `SLUS_010.32`); whole-archive link;
    fails if game C would bind to a macOS libc function. Needs `.venv` (or `DW1_PYTHON`).
    `DW1_WARNINGS=1` prints the ~3,570 clang warnings (triaged in the report).
  - **Fork:** `loafykitten/dw_decomp` branch `dw1-native` is pushed (a2ea978: two spelling
    fixes for `long long`/`long unsigned int`); the decomp's GCC build still passes.
  - Reviewed by gpt-6.1-sol (high), three passes, converged.
- **Phase 0 is done, and Ember said go (2026-10-13).** `docs/phase0-report.md` is the S0.9
  report. The go covers the plan as written: the hybrid first (decomp C running natively,
  Rust for everything around it), then module-by-module migration to Rust.
- **Repo:** public at `github.com/loafykitten/dw1-silicon`, branch `phase0` (the only branch
  so far, Phase 1 included; `main` comes later by merge). Claude commits and pushes at its
  discretion.
- **Plan:** reviewed adversarially (Astra, 19 findings, three blockers; a sample checked
  against the code, all held; `docs/plan-review-1.md`) and revised: this file.
- **Direction (Ember, 2026-10-13):** a full Rust rewrite is the endgame, reached through a
  hybrid (decomp C game + Rust for everything new, then module-by-module migration with
  side-by-side checks); not Bevy. Goal, Architecture, S0.6 and Phase 10 say how.
- **SDK rule set (Ember, 2026-10-13):** clean reimplementation of Sony's SDK (see Ground
  rules).
- **Phase 0 results** (each reviewed by gpt-6.1-sol until no finding was left):
  - **S0.1:** tools in `docs/toolchain.md` (Homebrew cmake, ninja, SDL3, `gmake`, MIPS
    binutils; `.venv/` with the decomp's Python requirements; dumpsxiso 2.20 in
    `~/.local/bin`; PCSX-Redux built from source at 26bfd693 in `ref/pcsx-redux`; the
    decomp's container `dw-build`, linux/amd64 under OrbStack; Rust 1.99).
  - **S0.2:** `docs/provenance.md` (full table in `docs/provenance/`). Eligible from
    DW2-PC: GTE hardware model, software GPU, MDEC/SPU/XA decoders minus their literal
    tables, CD/card/pad/VBlank/MDEC adapters, synthetic GTE test data. Not eligible: its
    whole sound stack (translated from retail Psy-Q), libgs, most of libgte.
  - **S0.3:** dw_decomp is the `decomp/` submodule (Ember's fork
    `loafykitten/dw_decomp`, based on upstream 09c174c, only `external/psyq_headers`
    initialized; port changes on branch `dw1-native`, pushed since Phase 1); the disc is extracted to the gitignored
    `decomp/disks/us/`; the GCC build passes (`docs/s03-decomp-gcc-build.md`).
  - **S0.4:** `docs/memory.md`. Below 4 GB is impossible on arm64 macOS; design per
    kind of reference; a fixed window at 0x1_8000_0000 is a measured fallback.
  - **S0.5:** `docs/survey.md` and `docs/survey/`. The PS1 library layer is mostly new
    work (DW2-PC has fewer than half of DW1's 175 SDK functions by name, little of libgs);
    the effect interpreter's 32-bit pointer tricks need widening; the save has no
    pointers; no overlay name clashes.
  - **S0.6:** Cargo workspace `crates/psyq`, `crates/decomp-c`, `crates/dw1`. The decomp's
    `rng.c`, `math.c` and `file_table_us.c` compile natively and pass tests against PS1
    models; bindgen bindings with PS1-size asserts. The game's `rand()` is the **BIOS**
    generator (A(2Fh)), in `psyq::bios`, seed 0 at boot (measured in S0.8). In
    `crates/backend`: the **GTE** (DW2-PC's model in Rust; all 1,248 PCSX-Redux reference
    cases, 13 command variants, plus RTPT, NCT, NCCS, NCCT, AVSZ3, AVSZ4), the **GPU**
    (DW2-PC's 1x software rasterizer; `docs/backend-gpu.md` lists its differences from
    hardware to check in Redux), **XA, MDEC, SPU** (every constant table re-derived from
    psx-spx or MPEG-1), each bit-exact against DW2-PC's C (test-only crates `gpu-diff`,
    `decoder-diff`, ignored without `ref/DW2-PC`). DW2-PC reuse: `docs/dw2pc-reuse.md`
    (everything translated, nothing linked as C). **Window, input and audio: SDL3**
    (`sdl3` crate 0.20, static build), provisionally, over winit + cpal + gilrs
    (`tools/spikes/s06/`, `docs/s06-platform.md`); not in the app yet. Gamepads, audio
    latency and 59.94 Hz pacing still need checking with Ember at the screen.
  - **S0.7:** `docs/s07-libgs-spike.md`. `oracle/capture-libgs.lua` records the machine
    around every `GsSortObject4J` call in one frame (48 calls); a clean Rust
    `psyq::libgs::sort_object4j` (lit GT3/GT4 only, from psx-spx and black-box captures)
    matches all 48 exactly (7,246 packet words) and draws the same pixels through
    `backend::gpu`. The rest of libgs is coverage: other primitive kinds and entry points,
    each needing a capture of a scene that uses it.
  - **S0.8:** `docs/s08-oracle.md`. Headless PCSX-Redux (`-cli -interpreter -softgpu`) on
    Ember's retail BIOS and disc. **Redux is pinned to 26bfd693**: PR #2283 (GTE write
    delay) breaks Sony's `MulMatrix`, so most DW1 bone matrices come out wrong. Filed
    upstream as grumpycoders/pcsx-redux#2322 (2026-10-13, from Ember's account, with an AI
    disclosure line); `tools/redux-2322/` is the test for when to unpin. **New-game route**
    (`oracle/route-new-game.lua`, `docs/oracle-route.md`): deterministic inputs from boot to
    first gameplay with the tamer and Agumon on screen (VBlank 19400). **Replays:**
    `oracle/replay.lua` records `PadRead` results (7,131 polls, `oracle/replays/new-game.txt`)
    and replays them with no pad overrides, failing on a caller or poll-count divergence.
    **State:** `oracle/canonical-state.lua`, schema v2 (`docs/canonical-state.md`, 834
    bytes), dumped at 19 checkpoints. **Certified:** the recording, two fresh replays on this
    Mac and one on booklab gave byte-identical dumps; tampered replays fail; the debugger
    doesn't perturb the run. **Field rate:** 564,398 CPU cycles per VBlank = 60.009 Hz
    (Redux) vs 59.817 Hz on hardware. OpenBIOS: same `rand` from its source, not run.
- **Working with the oracle** (learned the hard way; details in `docs/s08-oracle.md`):
  - Run **one emulator at a time** on a machine; concurrent runs stall at ~1% CPU.
  - Use absolute paths and a fresh empty directory under `captures/oracle-runs/` per run
    (Redux writes config and memory cards into the cwd).
  - Don't pass `-debugger` (it slows everything). Breakpoints need
    `PCSX.settings.emulator.Debug.Debug = true`, set from Lua when needed. Keep every
    breakpoint handle referenced (they're deleted on GC).
  - LuaJIT has about 2,000 ffi callbacks: never create a breakpoint per event. Make them
    persistent and reuse them.
  - After switching Redux commits, rebuild clean (`rm -rf objs/Release bins/Release`); an
    incremental build hung in the BIOS.
  - A full route or replay to gameplay takes about 385 s on this Mac and 389 s on booklab.
- **booklab** (`ssh booklab`, M1): `~/Developer/dw1-oracle` holds the pinned Redux binary,
  `scph1001.bin`, the disc (`disc/`), a copy of `oracle/` (refresh with
  `rsync -a --delete oracle/ booklab:Developer/dw1-oracle/oracle/`) and `runs/`. Homebrew
  capstone, freetype, ffmpeg and sdl3 were installed there for Redux. Check that nothing
  else (Wrath-Rust) is running before using it.
- **Helper tools:** `tools/oracle/dumpstate.py` (reads `DW1STATE` dumps from
  `capture-libgs.lua`), `tools/oracle/dis.py` (small R3000 disassembler over a dump's RAM),
  `tools/oracle/stalevz.py` (the #2322 bone-matrix check).
- **Local only (gitignored):** Ember's disc in `disc/` (`Digimon World (USA).bin` + `.cue`,
  MODE2/2352, and the original `.7z`; SHA-1 `5611645da66183e30d11fb6c11a2784ff47e1fdb`, the
  known-good redump of SLUS-01032, checked 2026-10-13); Ember's PS1 BIOS dump
  `scph1001.bin` in the repo root (512 KB, SHA-1 `649895efd79d14790eabb362e94eb0622093dfb9`,
  MD5 `dc2b9bf8da62ec93e868cfd29f0d067d`; kernel date 1994-09-22; exact revision to be read
  from PCSX-Redux at first boot, S0.8), the oracle's retail BIOS; reference clones `ref/dw_decomp`
  (09c174c), `ref/DW2-PC` (2d491b5), `ref/pcsx-redux` (26bfd693).

## Goal

Run Digimon World (USA, SLUS-01032) as a native arm64 macOS app, built from the game's
own decompiled C, with the PlayStation's libraries and hardware replaced by a Mac layer.
The game's rules, RNG, scripts, text, music and saves behave exactly as on the PS1.
How it looks and plays may improve (resolution, no wobble, gamepads, saves as files, and
widescreen if it proves feasible), and every improvement is an option that says whether
it's original or an addition, with the original picture available at 1x.

**Endgame: the whole game in Rust** (Ember, 2026-10-13), one uniform codebase that's easy to
maintain and to mod (e.g. new digivolutions). We get there in stages, never by rewriting
blind:
1. **Hybrid:** the decomp's C runs the game unchanged in meaning; everything new (the PS1
   library replacements, GPU, sound, CD/pack, cards, input, renderer, oracle tools) is
   written in Rust from the start.
2. **Migration:** game modules move from C to Rust one at a time. While both exist, the C
   and Rust versions run side by side in one process on the same state and any difference
   is a failure (in-process differential testing), on top of the emulator oracle. Done
   when no game C is left and the decomp is no longer a build dependency.
3. **Mods** come after, as opt-in layers over a "faithful" mode that stays oracle-checked.

Not a goal: an emulator, a recompiler, other regions (EU and JP may come later; the
decomp matches them too), or other platforms. Not Bevy: DW1 is a PS1 game loop feeding a
GPU, not an ECS game; rendering is `wgpu` on Metal.

## Ground rules

- **The repo is public** (Ember, 2026-10-13): `loafykitten/dw1-silicon` on GitHub, like
  pf-wotr-silicon. Everything committed must be safe to publish: the port reads all game
  data from a pack built on the player's machine from their own disc, checked against a
  manifest of hashes (DW2-PC's design). Never commit or upload anything from the disc,
  and no copyrighted toolchain files (`cc_mips.dll`).
- **Never commit disc data**: no disc image, no extracted files, no assets, no dumps, no
  oracle captures. They live under gitignored paths (`disc/`, `dumps/`, `build/`,
  `captures/`, `ref/`). Check `git status` before every commit; `.gitignore` only protects
  what it lists.
- **Upstream code isn't automatically safe to commit** (review finding 2). dw_decomp is MIT
  and DW2-PC is CC0, but both trees also hold things their authors didn't write: in
  dw_decomp, Metrowerks' copyrighted startup (`src/main/_psstart.c`), game text
  (`text/**/*.yaml`, compiled into headers by `mk/text.mk`) and large initialized data
  blobs (`src/mov/mov_bss_us.c`, 1.5 MB); in DW2-PC, `.incbin`s of retail data
  (`host/overlay.c`, `psyq/snd_hw.c`, `configs/USA/include_bin.txt`) and third-party
  components with their own licenses. Nothing from either tree enters this repo except
  through S0.2's import allowlist. Keep the licenses and credit both projects.
- **Sony's SDK is reimplemented clean** (Ember, 2026-10-13): from public documentation
  (psx-spx, file-format docs, the SDK manuals' API descriptions) and emulator observation.
  The SDK disassembly in `decomp/asm/` may answer narrow questions (a constant, an argument
  order, whether a call blocks) but is never translated routine by routine into this repo.
  Same for Sony-derived code in DW2-PC (`docs/provenance.md`).
- **Never modify Ember's disc image.** Read it; build derived files from it elsewhere.
- **Git:** commit on a work branch, never directly on `main`. Since 2026-10-13 Claude
  commits and pushes at its own discretion (Ember handed it over). Still check
  `git status` for disc data before every commit.
- Ask before anything outward-facing (issues or PRs upstream, publishing, Discord).
- **Model routing** (Ember's global CLAUDE.md, revised 2026-10-13 to match Wrath-Rust):
  - Claude implements, in the main session or through **Sonnet** subagents for large or
    parallel work, escalating to **Opus 5.5 (high)** only when Sonnet's work is bad or it
    can't solve the task. At most 3 implementation subagents at once, and every brief says
    "don't spawn sub-agents".
  - **gpt-6.1-sol (high) reviews every implementation** (`codex-review` skill, Codex Fast
    mode; pass `-m gpt-6.1-sol -c model_reasoning_effort="high"` explicitly).
    **GPT 6 Astra (high)** (`-m gpt-6-astra`) instead when Sol misses things or the work is
    touchy (the memory model, the GPU layer, anything that could break saves). Verify
    findings against the code before acting; fix what's real; re-review until no
    finding is left (Wrath-Rust's "reviews converged").
  - **Bulk investigation** (surveys of the decomp, grepping for patterns, reading DW2-PC)
    goes to gpt-6.1-sol: `codex exec -m gpt-6.1-sol -s read-only` with a self-contained
    prompt. **Computer use** (driving an emulator, comparing screenshots) goes to
    gpt-6.1-sol (high) through the `codex-computer-use` skill, or GPT 6 Astra (high) when
    it's particularly complex or Sol isn't good enough at it. Never Haiku.
- **Machines:** this Mac is the main one. **booklab** (`ssh booklab`, an M1 Ember keeps for
  this) is available whenever parallel work helps: long emulator recordings, a second
  oracle run to certify replays, builds while this Mac runs the game. Its setup notes are
  in `~/Developer/Wrath-Rust/docs/booklab.md`; don't disturb Wrath-Rust's work on it
  (check what's running first). Never use it for timing comparisons against this Mac.
- **Testing split:** agents get scenes running and prove them against the oracle; Ember
  plays. Ember makes fixture saves by playing, and is the judge of "plays right".

## Sources (checked 2026-10-13; corrected after review 1)

- **dw_decomp** (`github.com/jype0/dw_decomp`, MIT): matching decomp, 100% decompiled on
  decomp.dev for us, eu, jp (and tracks jp_rev1, jp_bombom, jp_trial). The US disc has 16
  overlay files (`*_REL.BIN`: BTL, DGET, DOO2, DOOA, EAB, ENDI, EVL, FISH, KAR, MOV, MURD,
  SHOP, STD, TRN, TRN2, VS; `src/main/file_table_us.c:807`), but the US build produces 15
  overlays: SHOP's code is built into the main executable (`mk/version/us.mk`, a `unit`, not
  an `overlay`), as is some VS code. The game's own C is about 932 KB of US machine code
  (summed from the splat yamls; excludes the SDK's assembly). US headers are Psy-Q
  `mw_lib41`; the SDK libraries in the image are libapi, libetc, libgpu, libgs, libgte,
  libcd, libspu, libsnd, libds, libmcrd (plus libpress for movies).
  - Built with CodeWarrior for PlayStation R4 (`cc_mips.dll` through wibo) and splat;
    `make compare` checks against the disc. macOS Apple Silicon support for the build
    merged in PR #28 (2026-09-01): `WIBO := bin/wibo-i686` in an untracked `local.mk`. The
    checked-in downloader fetches Linux binaries and the dev container is `linux/amd64`.
  - **The GCC build** (`make TOOLCHAIN=gcc`, `mk/toolchain/gcc.mk`) doesn't invoke
    `cc_mips.dll` and doesn't need the matching build first. It still needs splat's
    `make regenerate`, which reads the original executable and overlays extracted from the
    disc into `disks/us/` (covered by the decomp's `.gitignore`), and it assembles the
    extracted SDK code. It outputs a MIPS ELF, a PS-X EXE and overlay binaries under
    `build/us/psx/gcc`. CI only builds it; nobody runs or compares it. So it's a
    compile-and-link check that the C still builds with a second compiler, not proof that
    meaning is preserved. Needs a recent GNU make (grouped targets) and the Python
    requirements.
  - Still active (type and symbol cleanups, EU data matched 2026-10-10).
- **SydMontague's DW1-Code and DW1-SydPatches:** the reverse-engineering the decomp's
  symbols come from. Good for "what is this struct" questions.
- **DW2-PC** (`github.com/Wyrelade/DW2-PC`, CC0): a native Windows/Linux port of Digimon
  World 2 from its matching decomp, playable since v0.1.0 (2026-10-09). **A reference, not
  a platform we can lift whole** (review findings 3, 4, 7, 8):
  - Layout: `src/` (the game's C, 64-bit fixes under `DW2_NATIVE`), `psyq/` (PC versions
    of the Psy-Q 4.7 libraries DW2 calls), `backend/` (PS1 GPU, SPU with reverb, XA, MDEC;
    HD renderer on SDL_GPU with SPIR-V shaders only), `host/` (SDL3 window, input, audio,
    card files, data pack and manifest), `tests/` (a C GTE model checked against
    PCSX-Redux values). Seven stage overlays, linked into one program; each overlay's
    `.data`/`.bss` is grouped by the linker and snapshotted to reset it on reload.
  - **Its memory layout:** the executable at a fixed base (0x01000000, no ASLR) with its
    globals, a 2 MB `Ps1_Ram` arena and the heap in one 16 MB window below 2 GB. That gives
    it three things: 24-bit GPU ordering-table links resolve, pointers stored in 32-bit
    ints round-trip, and fixed PS1 addresses translate into `Ps1_Ram`. Linux adds a
    `MAP_32BIT` stack via `makecontext`. Native pointers are still 8 bytes in its 64-bit
    build; structs holding them grow (`psyq/ps1mem.h`). **None of this works on arm64
    macOS** (S0.4).
  - Its non-Windows build assumes Linux/ELF: `-Ttext-segment`, GNU linker scripts,
    `ld -r -T`, ELF `__start_`/`__stop_` section symbols, `MAP_32BIT`, SPIR-V shaders.
  - Building it needs DW2's disc extracted to `dumps/disc/` (`.incbin`s of retail tables),
    even for its `gte_test` target, so we can't do a full build without a DW2 disc.
  - Its libgs shim has 12 functions (coordinates, lighting, view, TIM parsing); its GTE
    model implements RTPS, NCLIP, MVMVA, NCS and GPF only. Its sound code depends on
    DW2-specific retail addresses and extracted data.
- **Digimon World Recompiled** (ethan4love, GitLab `psx-recomp-port`): a Windows-only
  static recompilation, not from the decomp. Useful only as a behaviour reference.
- **Wrath-Rust** (`~/Developer/Wrath-Rust`): Ember's other port. Process lessons taken from
  it are under "Process".

## Architecture (proposed; the spikes decide)

```
decomp/        dw_decomp: submodule of the fork loafykitten/dw_decomp. Port changes guarded by
               #ifdef DW1_NATIVE so upstream's builds are unaffected. Shrinks as modules
               move to Rust; gone at the end.
crates/
  dw1-game/    Rust ports of game modules (empty at first; grows during migration)
  psyq/        Rust implementations of the PS1 library calls DW1 makes, exported to the
               game C as `extern "C"` with the original names and signatures
  backend/     PS1 GPU (software first), GTE model, SPU, XA, MDEC
  host/        window, input, audio, card files, pack, deterministic clock for replays
  render/      wgpu on Metal: 1x faithful output first, then the extras
  pack/        pack builder (from Ember's .bin/.cue) and manifest
  oracle/      replay, canonical state schema, comparison tools
  decomp-c/    the game C from decomp/, built by `cc` with the port's flags and a forced
               `dw1_native.h`; bindgen bindings with PS1-size asserts
  dw1/         the app: links everything
oracle/        emulator-side scripts (PCSX-Redux Lua)
docs/          spike reports, per-phase notes, reviews, the HTML guide at the end
```

- **Build:** a Cargo workspace, arm64 only, Mach-O; the game C compiled by Apple clang
  through the `cc` crate with the port's flags. Window/input/audio through SDL3 (Rust
  bindings) or winit/cpal (S0.6 picks); rendering through `wgpu` on Metal. The `.app`
  bundles anything dynamic (no Homebrew paths at run time), ad-hoc signed. DW2-PC's ELF
  linker-script tricks don't apply; Mach-O needs its own way to group and reset overlay
  state (S0.5's lifetime map decides what has to be reset).
- **The C↔Rust boundary** is the PS1 SDK's API: the game C calls `GsSortObject4J`, `CdRead`,
  `SsSeqPlay`... and those are Rust. Shared state crossing it (game structs the Rust side
  reads, callbacks into C) gets `#[repr(C)]` mirrors generated or checked against the C
  headers (bindgen plus layout asserts), never written by hand and trusted.
- **DW2-PC's C** (GTE model, GPU rasterizer, SPU, XA, MDEC) is a reference to translate into
  Rust, checked by its own tests and PCSX-Redux values, or linked as C temporarily where
  that's faster to a running game; S0.6 decides piece by piece.
- **Overlays:** linked into one program. The decomp already links main and every overlay
  into one ELF, and a lexical scan found no duplicate function definitions, so symbol
  clashes look small (S0.5 confirms with a real symbol audit). The hard part is state
  lifetime, not names: overlay C files put `.sdata`/`.sbss` into the resident main
  executable (`config/us/main.ld:634,774`), overlays share overlapping load ranges, carry
  assets as well as code, and "bss" files can hold non-zero retail data. Resets follow the
  original linker placement per symbol, not the source directory.
- **Memory representation (S0.4 decides; measured part done):** arm64 macOS reserves the
  low 4 GB (`__PAGEZERO`); a smaller page zero gets the process killed at exec, and
  `MAP_FIXED` below 4 GB fails. So there's no single trick; the design is per kind of
  reference:
  1. **Native objects use native pointers.** Structs grow; that's fine where nothing reads
     them as bytes.
  2. **Disk and save records get explicit fixed-width schemas** (`int32_t`, not `long`:
     macOS `long` is 8 bytes and the decomp uses `long *` to read 32-bit offsets, e.g.
     `src/main/model.c:3414`). Disk structs stay separate from expanded native structs;
     loaders bounds-check. Most DW1 formats hold relative offsets (MMD, MAP, scripts,
     animations), not PS1 addresses.
  3. **Pointers stored in 32-bit ints** (the effect interpreter's `EFE_PUSH` in
     `include/dw/efe.h`, ~1,000 cast sites mostly in the triplicated BTL/STD/VS effect code;
     see `docs/survey.md` item 2). A plain C cast back from `int32_t` can't restore a high
     half, so no address trick makes these work unchanged. Plan: widen in-memory stacks and
     integer-address variables to `intptr_t`, turn arithmetic-only `(int32_t)ptr` casts into
     `intptr_t` mechanically (ast-grep), and only where a 32-bit value must sit inside a
     loaded buffer, use a **4 GB window** with explicit encode/decode: the executable loads
     at 0x1_xxxx_xxxx and a fixed arena at 0x1_8000_0000 maps (measured, 6 ASLR runs). To
     confirm in S0.4: whether any such site remains after widening, the ASLR slide range,
     and that the mapping holds in a signed `.app`.
  4. **Fixed PS1 addresses** (`include/dw/main.h:8`, `src/main/btl.c:23`, the scratchpad)
     translate into the PS1 RAM arena with checked accessors.
  5. **GPU packet links** (24-bit ordering-table links) can't rely on a 16 MB window around
     the executable: primitive buffers and OTs move into one arena and links become
     offsets from its base (libgpu/libgs replacements own this).
  6. Callbacks and `setjmp`/`longjmp` (the script interpreter,
     `src/main/script_interp.c:364,600`) get checked for the native ABI.

## The oracle: the original game as the test suite

The reference is the original game in **PCSX-Redux** (open source, runs on Apple Silicon,
Lua scripting, pad overrides, VSync events, savestates, memory access, GDB stub; DW2-PC
already uses it for GTE reference values). DuckStation is the fallback for visual
comparisons.

- **Replays are keyed to the game, not to wall-clock frames** (review finding 10). DW1 runs
  a varying number of VBlanks per update (`VSync(0)` on the landing screen, `VSync(3)` in
  the new-game loop, `main.c:3656,3761`), async CD completion changes control flow, and
  movie code has CPU-iteration timeouts. A replay is a list of pad states keyed by pad-poll
  sequence number, with emulated VBlank counts and named game-update checkpoints alongside.
  The port runs replays on a deterministic clock with CD/SPU completions scheduled
  explicitly (DW2-PC's wall-clock VBlank with catch-up is for play, not for tests), and
  writes a divergence trace when a checkpoint fails.
- **Pin the reference:** PCSX-Redux revision, CPU core (interpreter vs dynarec), BIOS or
  OpenBIOS, settings, initial RAM and card state, controller configuration. Certify a
  replay across fresh emulator processes (and on booklab), not two continuations of one
  state.
- **State comparison through a canonical schema** (finding 11): a versioned, fixed-width
  serialization of the semantic state (partner stats and care state, inventory, flags,
  clock, both RNG streams, the current map and script position), with references turned
  into stable entity/resource IDs and documented exclusions. Never raw native struct dumps
  (pointers, padding, `jmp_buf`). Which structs and where come from the decomp's symbols.
- **Both RNGs:** the game's `customRandom` (`src/main/rng.c`) and the SDK's `rand()`
  (`randomLimit` in `src/main/math.c` assumes a 15-bit result). The port implements the
  SDK `rand` exactly (macOS libc's would change gameplay), with its initial state, and
  checkpoints record RNG call counts.
- **Timing is measured, not assumed** (finding 12): the PS1's non-interlaced NTSC field
  rate is about 59.82 Hz (59.817 from the video clock; measured Redux: 60.009), interlaced 59.940 Hz; DW1 runs 320×240 non-interlaced. The plan
  keeps VBlank/field timing, game-update cadence (the clock: 20 ticks per game minute,
  `src/main/clock.c`) and presentation cadence separate, and preserves `VSync(n)`
  semantics. Any 59.94 or 60 Hz presentation mode is labelled as such.
- **Pictures:** at 1x with the extras off, compare frames with the emulator's, as
  DW2-PC does with reference screenshots.
- Captures stay local and gitignored. Small derived test cases with no game data can be
  committed.

## Phases

Each phase ends with a short report in `docs/` and a check-in with Ember. Phase 0 ends
with a go / no-go. Every runnable phase's gate includes: named fixtures, expected
checkpoints equal to the oracle, no unexpected stub calls (unimplemented SDK functions trap
and log), bounded completion (no hangs on CD/SPU/movie waits), and re-entry tested (the
scene run twice).

### Phase 0: setup and risk spikes

- **S0.1 Tools and repo:** `git init` on a work branch (first commit: PLAN.md, CLAUDE.md,
  `.gitignore`, `docs/`, when Ember says so). Homebrew `cmake`, `ninja`, `sdl3`, `make`
  (GNU make 4.3+), `python3` with the decomp's `requirements.txt` in a venv; MIPS
  binutils/GCC, or the decomp's dev container under OrbStack (`linux/amd64`); PCSX-Redux
  for this Mac; `dumpsxiso`/`mkpsxiso`. Codex CLI works with `-m gpt-6.1-sol`. Record
  versions in `docs/toolchain.md`.
- **S0.2 Provenance gate:** with dw_decomp as a submodule (Answered, 2026-10-13), this repo
  holds none of upstream's files, only a pointer to the fork; its nested Sony SDK headers
  (`external/psyq_headers`, unlicensed) stay there too and reach Rust only through bindgen
  at build time. What's left: before any DW2-PC code is translated or copied, a per-file
  list of what in DW2-PC is its authors' code (CC0, usable with a credit note) versus
  retail data, `.incbin`s, Sony-derived tables or third-party code (not usable). Output:
  `docs/provenance.md`. Generated files from the decomp build (text headers, overlay
  data) stay in the gitignored build tree.
- **S0.3 The decomp's GCC build on this Mac:** dw_decomp is the `decomp/` submodule (fork
  `loafykitten/dw_decomp`, local branch `dw1-native` for port changes, not pushed yet);
  extract the disc with `dumpsxiso` into the gitignored `decomp/disks/us/`, `make regenerate`, then
  `make TOOLCHAIN=gcc`, first through the dev container, then natively if the tools allow.
  It's a compile/link regression check for `DW1_NATIVE` changes, nothing more. No
  `cc_mips.dll`, so no matching build or `make compare`; if one turns out to be needed,
  ask Ember, and don't fetch the compiler from an unofficial source.
- **S0.4 Memory representation:** done 2026-10-13, `docs/memory.md`, spikes in
  `tools/spikes/s04/`. Below 4 GB is impossible (MAP_FIXED fails, a small page zero is
  killed at exec); a fixed window at 0x1_8000_0000 maps (200/200 runs; executable base
  slides within 0x1_0009_8000 to 0x1_04FF_4000). Design: native pointers for native
  objects, fixed-width disk/save records, widen 32-bit pointer storage to `intptr_t`, a
  2 MB PS1 RAM arena for fixed addresses and overlays, side tables for TMD mapping and EFE
  buffers, offset-based GPU packet links; the window is a measured fallback only.
- **S0.5 Survey of the decomp** (gpt-6.1-sol, read-only, in bulk; Claude checks samples).
  Output: `docs/survey.md` with counts and file:line lists:
  - **SDK surface:** every SDK function and global called, directly or through dispatch
    tables (`GsFCALL4`'s TMD handlers in `src/main/graphics.c:60`), with DW2-PC's
    coverage of each (has it / different API / missing), prototypes, callbacks and tick
    sources. Known gaps: libgs object/TMD sorting, CD (`CdRead`, `CdSearchFile`),
    sequencer (`SsSeqOpen`, `SsSetTickMode`), card (`MemCardGetDirentry`), movie
    (`DecDCTvlcBuild`, `StGetBackloc`), pad (`PadInit`/`PadRead`).
  - **GTE:** every command used, inline (`gte_rtpt` etc.) and inside the SDK routines DW1
    reaches.
  - **ABI checklist:** `long` width, pointer↔int casts and narrowing, `sizeof`
    assumptions, signedness, shifts, unaligned access, K&R definitions, incompatible
    redeclarations (`memcpy` in `main.c:3789`), null reads the PS1 tolerated
    (`file_queue.c:250`), MIPS inline asm (`include/gcc/inline_n.h`, `_psstart.c`),
    `setjmp`/`longjmp`, callback signatures.
  - **Every loaded format** (MMD/TMD, MAP, scripts, animations, TIMs, VABs/SEQs, saves):
    field widths and whether each field is an offset, a PS1 address or data.
  - **Overlay lifetime map:** per symbol, resident or reloadable, from the original linker
    placement; overlapping load ranges; assets inside overlay files; non-zero "bss".
  - The VSync/frame-pacing structure; the script engine's entry points.
- **S0.6 Rust skeleton and DW2-PC components:** a Cargo workspace that compiles a few
  decomp C files through the `cc` crate and links them against Rust `extern "C"` stubs
  (proves the boundary, the flags and bindgen layout checks). Pick SDL3-rs vs winit/cpal.
  For DW2-PC (not a full build; it needs DW2 data): build its GTE model and test, GPU
  packet rasterizer, SPU, XA and MDEC with Apple clang against synthetic inputs, and
  decide each: translate to Rust now, or link as C for now. Output:
  `docs/dw2pc-reuse.md`.
- **S0.7 libgs/TMD spike:** one DW1 model, extracted locally from Ember's disc, through
  model mapping, animation, lighting, sorting and packet generation into the software
  rasterizer, compared with the emulator's frame. This is the biggest unknown DW2-PC
  doesn't cover.
- **S0.8 Oracle bring-up:** PCSX-Redux running Ember's disc on this Mac, pinned per "The
  oracle" (retail BIOS for replays: the game's `rand()` is BIOS A(2Fh), so the BIOS's seed
  at boot is part of the game state; measure it, and check whether OpenBIOS matches); a Lua script that records pad input keyed by poll sequence and dumps the
  canonical state at a checkpoint; two fresh-process runs agree. Measure the field rate.
- **S0.9 Go / no-go:** `docs/phase0-report.md`: each spike's result, the memory design and
  why, what DW2-PC covers and what's new, the riskiest unknowns left, and a
  recommendation. Ember decides.

### Phase 1: main compiles and links natively

All of main's C (which includes SHOP and part of VS) plus the Rust `psyq` crate with
trapping stubs builds for arm64 and links; every overlay's C files compile on their own (warnings
counted and triaged). Done when it links, each overlay file compiles, and the decomp's GCC
build still builds with `DW1_NATIVE` off. Short by design: running code finds the real
problems.

### Phase 2: boot to the main menu

The pack (DW1's own manifest: filenames, LBAs, logical sizes, sector formats with XA/STR
subheaders kept, plus the slices of the executable and overlays the native build needs;
format version, bounds checks, atomic replace; no game data embedded in binaries), CD
reads and completions, the software GPU and libgs, pad input, the deterministic clock.
Boot plays the intro movie through MOV (`playMovie(0, 1)`, `main.c:3427`) and waits on
sound transfers (`sound_async.c:128`), so movie completion/skip and SPU transfer
completion work here, silently if need be. Done when the landing screen matches the
emulator's at 1x and a replay reaches the main menu with no stub trapped.

### Phase 3: load a save (read only)

Raw card images from PCSX-Redux / DuckStation read as-is (tested formats only), DW1's save
parsed against its retail layout (`main_menu.c:88`, checksums over 0x4E4 and 0xA00 bytes
at 5004), checksum rejection. Nothing writes yet. Done when one of Ember's fixture saves
loads and the canonical state after load equals the emulator's. From here on, fixtures
can start any replay.

### Phase 4: a new game, File City, Jijimon's house

The intro choices, the first partner, walking, map changes, the script engine, menus,
the in-game clock and care (hunger, sleep, toilet). Oracle: a replay from boot to the
first walk around File City, state equal at each map change, and a fixture-started replay
of an hour of care with the clock and care state equal.

**Gate met (2026-10-14, `docs/phase4-plan.md`):** `oracle/replays/file-city-walk.txt` (five map
changes after first gameplay, to VBlank 23380) and `oracle/replays/care-fixture.txt` (Ember's
save, more than seven game hours of care state, to VBlank 31000) are certified on Redux and
run natively with every checkpoint identical (27 of 27, 37 of 37). Open: a care replay that
reaches a need (the poop at ~34000 is past the pinned Redux's Lua limit near VBlank 32768),
the shops and other exits, frame comparison.

### Phase 5: the overlays, one at a time

Order: BTL (battles), TRN/TRN2 (training), then the rest (DGET, DOO2, DOOA, EAB, ENDI, EVL,
FISH, KAR, MURD, STD, VS); MOV is in from Phase 2, SHOP's code from Phase 1. Digivolution,
death and rebirth, recruitment. Each overlay is done when its scene's replay matches the
oracle, including loading it twice, alternating with another overlay, and resident state
surviving the swap.

### Phase 6: sound, streamed audio, movies (fidelity)

SPU and the sequencer at its tick mode (`SsSetTickMode(SS_TICK240)`), reverb, XA streams,
MDEC movies at full quality. Done when event timing and durations match the emulator
(measured) and Ember says they sound right.

### Phase 7: saving

Writing cards. Back up any card before a build writes to it; write atomically; keep
unrelated blocks and unknown bytes. Done when emulator → port → emulator round-trips work
for both slots, a failed write leaves the old card intact, and saves made in the port load
on the emulator with equal state.

### Phase 8: the Mac-native renderer and extras

The `render` crate on wgpu/Metal: render scale, no wobble, Retina, window
modes, gamepads through SDL. Each option labelled original or addition; 1x with extras off
still matches the emulator's picture. **Widescreen is a feasibility milestone, not a
promise** (finding 19): DW1's maps are pre-drawn backgrounds with screen-positioned layers
(`src/main/map.c:4217`), so test map-edge coverage, actor/background alignment, occlusion
and scripted cameras first, and pillarbox wherever more picture can't be shown faithfully.

### Phase 9: the long tail and a full playthrough

Ember plays start to finish (with fixture saves along the way); bugs go to `docs/bugs.md`
with save and repro. Then packaging (`.app` that runs on a Mac without Homebrew,
first-run pack build, settings) and the deliverable: a self-contained HTML guide
(`docs/guide.html`) on installing, building and playing. This is the first release: a
faithful hybrid.

### Phase 10: migrate the game to Rust

Module by module, leaves first (math, RNG, clock, care, inventory), then systems (scripts,
maps, battle), overlays last. Per module: translate, then run the C and Rust versions side
by side on the same inputs and state (every call, every frame of the replays) until they
never differ, then switch to Rust and delete the C. Game structs become Rust types as their
last C user goes; the memory-representation workarounds go with them. Done when no game C
is built and every replay still matches the oracle. Then mods (Goal, stage 3).

Modules can start migrating earlier where it pays (e.g. a module that needs heavy 64-bit
surgery anyway), but not before the hybrid runs the scene that exercises it: the running C
is what the Rust is checked against.

## Process (lessons from Wrath-Rust)

1. **Adversarial review of the plan before Phase 0:** done 2026-10-13
   (`docs/plan-review-1.md`), findings folded in above. Review the plan again before a
   large change of direction (e.g. after the go / no-go).
2. **The handoff lives in this file.** At the end of each session, update "Where things
   stand" (date, session number, what's done, what's next, with links to the phase doc)
   and the status line in `CLAUDE.md`. Record Ember's answers with dates under "Answered".
3. **Small bounded slices.** Each implementation slice has a brief (goal, files, the
   oracle check that proves it, "don't spawn sub-agents"), gets implemented, reviewed by
   Sol until no finding is left, and verified against the oracle before it counts.
4. **Measure, don't infer.** Facts that matter (memory layout, timings, what the original
   does) are measured on the emulator and written down with the date.
5. Codex runs can outlast Bash's 10-minute timeout: run them in the background with an
   explicit timeout and poll for the report file. Give them the `ref/` clones to read
   rather than relying on web fetches.
6. For unattended emulator runs keep `caffeinate -dim` going (Wrath-Rust lost runs to the
   display sleeping and locking the session).

## Risks (ranked, after review 1)

1. **Memory representation on arm64 macOS.** Low mapping is out (measured), so every kind
   of reference needs its own handling (Architecture). Biggest single cost; S0.4 and S0.5
   size it.
2. **libgs, TMD and the GTE:** DW1 leans on the high-level libgs object pipeline and the
   GTE commands behind it, which DW2-PC barely covers. S0.7 tests it.
3. **Overlay state lifetimes:** resident small data from overlay files, overlapping load
   ranges, assets in overlays, non-zero "bss". Wrong resets cause bugs that show up hours
   later.
4. **Publication safety:** upstream trees hold text, SDK code and retail data; one careless
   import puts them in a public repo, and history keeps them. S0.2 gates it.
5. **CodeWarrior-isms and ABI under clang:** undefined behaviour the original compiler
   tolerated, `long` width, null reads, K&R C. Mitigation: `-fwrapv -fno-strict-aliasing`,
   `-funsigned-char` if the original char is unsigned, `-O0`/`-O1` until stable, UBSan
   builds, the S0.5 checklist, oracle replays.
6. **Determinism of the oracle:** replays that diverge for timing reasons look like port
   bugs. Pinning and fresh-process certification (S0.8) come before trusting a replay.
7. **Sixteen overlay files**, more than twice DW2's seven, and DW1's script engine and care
   simulation run on a clock, so timing bugs can hide for hours of play.
8. **The long tail:** bugs only a full playthrough finds. Ember's playthrough is the last
   gate, and DW2-PC hasn't had one either.
9. **The C↔Rust boundary:** layout mismatches between C structs and their Rust mirrors,
   callbacks re-entering C, panics unwinding across `extern "C"`. Mitigation: generated
   bindings with layout asserts, `panic = "abort"` at the boundary, UBSan/ASan on the C side.
10. **Migration drift:** a Rust module that's subtly different (integer widths, wrapping,
    evaluation order, RNG call order). Mitigation: side-by-side differential runs before
    each switch, explicit `wrapping_*`/fixed-width types mirroring the C.

## Open questions for Ember

None right now. Add them here as they come up.

## Answered by Ember

- 2026-10-13: go ahead with the port; project lives in `~/Developer/dw1-silicon`; Ember
  owns the game and has time to test.
- 2026-10-13: the disc is Ember's US image, now in `disc/` (redump-verified).
- 2026-10-13: no `cc_mips.dll`; use the GCC build (see S0.3).
- 2026-10-13: subagents as in Wrath-Rust (Sonnet first, Opus on escalation); reviews and
  computer use through Codex, Sol (high), Astra (high) for particularly complex work.
  The global CLAUDE.md was changed to say so.
- 2026-10-13: the repo will be public.
- 2026-10-13: this Mac is the main machine; booklab is available for parallel work.
- 2026-10-13: the endgame is a full Rust rewrite (uniform codebase, easier to mod, e.g. new
  digivolutions), reached through a hybrid first: decomp C for the game, Rust for
  everything new, then module-by-module migration. Not Bevy.
- 2026-10-13: dw_decomp comes in as a fork, `loafykitten/dw_decomp`, added as a git
  submodule at `decomp/`, with port changes on a branch in the fork (Ember creates the fork;
  Claude Code's auto mode won't). Credit upstream projects in the README.
- 2026-10-13: this repo is licensed MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`,
  README section). Code adapted from DW2-PC notes its CC0 origin; don't copy code from
  GPL projects (PCSX-Redux, mkpsxiso) into this repo.
- 2026-10-13: Claude commits and pushes as needed, at its discretion.
- 2026-10-13: Ember supplied a retail BIOS dump (`scph1001.bin`, repo root, gitignored) for
  the oracle.
- 2026-10-13: Sony's SDK is reimplemented clean, from public docs and emulator observation;
  the SDK disassembly answers narrow questions only, never translated (Ground rules).
- 2026-10-13: Japanese may be translated to English where it helps maintenance ("using
  high-quality translation sources"). Applied to comments and docs only: Japanese inside the
  game's string literals (the kana name-entry grid, leftover VS strings) is data the game
  draws with its Shift-JIS font, so it stays byte-exact.
