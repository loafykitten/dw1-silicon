# dw1-silicon

A native arm64 macOS port of Digimon World (PS1, USA SLUS-01032), built from the
jype0/dw_decomp matching decompilation with the PS1 libraries replaced by a Mac layer
written in Rust (DW2-PC is the reference). **Read `PLAN.md` before doing anything**:
it's the handoff, with the goal, ground rules, sources, architecture, oracle, phases
and open questions.

- **Current status (2026-10-13, end of session 2):** Phase 0 done. The go/no-go report
  (`docs/phase0-report.md`) recommends go; confirm Ember's go before starting Phase 1.
  Public repo `loafykitten/dw1-silicon` (branch `phase0`). Direction: hybrid (decomp C game
  + Rust for everything new), full Rust as the endgame. Done: plan review, S0.1-S0.8
  (Rust GTE/GPU/decoders; clean Rust `GsSortObject4J` matches 48 captured calls; SDL3
  provisionally for window/input/audio; PCSX-Redux oracle pinned to 26bfd693 because of
  upstream bug #2322, with pad replays and state dumps certified byte-identical on this Mac
  and booklab). Sony's SDK is reimplemented clean (never translated from disassembly).
  PLAN.md's "Where things stand" is the handoff, including how to work with the oracle;
  keep it and this line current at the end of every session.
- **Never commit disc data** (image, extracted files, assets, dumps, captures). They live
  under the gitignored `disc/`, `dumps/`, `build/`, `captures/`, `ref/`. Check
  `git status` before every commit.
- **Git:** work branch, never `main` directly; commit and push at your discretion (Ember,
  2026-10-13).
- **Model routing:** Claude implements (subagents per PLAN.md); gpt-6.1-sol (high) reviews
  every implementation, GPT 6 Astra (high) for touchy work; bulk investigation and
  computer use go to gpt-6.1-sol through Codex. Never Haiku.
