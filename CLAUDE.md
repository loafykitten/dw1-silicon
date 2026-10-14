# dw1-silicon

A native arm64 macOS port of Digimon World (PS1, USA SLUS-01032), built from the
jype0/dw_decomp matching decompilation with the PS1 libraries replaced by a Mac layer
written in Rust (DW2-PC is the reference). **Read `PLAN.md` before doing anything**:
it's the handoff, with the goal, ground rules, sources, architecture, oracle, phases
and open questions.

- **Current status (2026-10-14, end of session 3):** Phases 1-4 done (`docs/phase2-report.md`,
  `docs/phase3-plan.md`, `docs/phase4-plan.md`): the `new-game`, `file-city-walk` and
  `care-fixture` replays match the emulator at every checkpoint, every compared frame is
  pixel-identical, and the build plays in a window (`dw1-mac`). Next is Phase 5 (overlays,
  battles first). Public repo `loafykitten/dw1-silicon` (branch `phase0`); decomp fork branch
  `dw1-native` pushed. Direction: hybrid (decomp C game + Rust for everything new), full Rust
  as the endgame. Sony's SDK is reimplemented clean (never translated from disassembly).
  PLAN.md's "Where things stand" is the handoff, including how to work with the oracle; keep it and this line current at the end of every session.
- **Never commit disc data** (image, extracted files, assets, dumps, captures). They live
  under the gitignored `disc/`, `dumps/`, `build/`, `captures/`, `ref/`. Check
  `git status` before every commit.
- **Git:** work branch, never `main` directly; commit and push at your discretion (Ember,
  2026-10-13).
- **Model routing:** Claude implements (subagents per PLAN.md); gpt-6.1-sol (high) reviews
  every implementation, GPT 6 Astra (high) for touchy work; bulk investigation and
  computer use go to gpt-6.1-sol through Codex. Never Haiku.
