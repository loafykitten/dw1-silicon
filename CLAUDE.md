# dw1-silicon

A native arm64 macOS port of Digimon World (PS1, USA SLUS-01032), built from the
jype0/dw_decomp matching decompilation with the PS1 libraries replaced by a Mac layer
written in Rust (DW2-PC is the reference). **Read `PLAN.md` before doing anything**:
it's the handoff, with the goal, ground rules, sources, architecture, oracle, phases
and open questions.

- **Current status (2026-10-13, session 2):** Phase 0 done, go/no-go report
  (`docs/phase0-report.md`, recommends go) waiting on Ember. Public repo
  `loafykitten/dw1-silicon` (branch `phase0`). Direction: hybrid (decomp C game + Rust for
  everything new), full Rust as the endgame. Done: plan review, S0.1-S0.5, S0.7 (clean
  Rust `GsSortObject4J` matches 48 captured calls), S0.6 (GTE, GPU, decoders in Rust; SDL3
  static for window/input/audio, provisional), S0.8 (headless PCSX-Redux pinned to 26bfd693, new-game route to
  gameplay, pad replays and canonical state dumps byte-identical across fresh processes, field
  rate 60.009 Hz measured; the game's `rand()` is the BIOS generator, seed 0 at boot). Sony's SDK is reimplemented
  clean (never translated from disassembly). PLAN.md's "Where things stand" is the
  handoff; keep it and this line current at the end of every session.
- **Never commit disc data** (image, extracted files, assets, dumps, captures). They live
  under the gitignored `disc/`, `dumps/`, `build/`, `captures/`, `ref/`. Check
  `git status` before every commit.
- **Git:** work branch, never `main` directly; commit and push at your discretion (Ember,
  2026-10-13).
- **Model routing:** Claude implements (subagents per PLAN.md); gpt-6.1-sol (high) reviews
  every implementation, GPT 6 Astra (high) for touchy work; bulk investigation and
  computer use go to gpt-6.1-sol through Codex. Never Haiku.
