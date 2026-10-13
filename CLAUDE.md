# dw1-silicon

A native arm64 macOS port of Digimon World (PS1, USA SLUS-01032), built from the
jype0/dw_decomp matching decompilation with the PS1 libraries replaced by a Mac layer
written in Rust (DW2-PC is the reference). **Read `PLAN.md` before doing anything**:
it's the handoff, with the goal, ground rules, sources, architecture, oracle, phases
and open questions.

- **Current status (2026-10-13, session 2):** Phase 0 under way, public repo
  `loafykitten/dw1-silicon` (branch `phase0`). Direction: hybrid (decomp C game + Rust for
  everything new), full Rust as the endgame. Done: plan review, S0.1 tools, S0.3 decomp
  GCC build (`decomp/` submodule of Ember's fork), S0.4 memory design, S0.5 survey, S0.6
  first slice (Cargo workspace builds decomp C; the game's `rand()` is the BIOS generator).
  Next: S0.2 provenance, rest of S0.6, S0.7, S0.8. PLAN.md's "Where things stand" is the
  handoff; keep it and this line current at the end of every session.
- **Never commit disc data** (image, extracted files, assets, dumps, captures). They live
  under the gitignored `disc/`, `dumps/`, `build/`, `captures/`, `ref/`. Check
  `git status` before every commit.
- **Git:** work branch, never `main` directly; commit and push at your discretion (Ember,
  2026-10-13).
- **Model routing:** Claude implements (subagents per PLAN.md); gpt-6.1-sol (high) reviews
  every implementation, GPT 6 Astra (high) for touchy work; bulk investigation and
  computer use go to gpt-6.1-sol through Codex. Never Haiku.
