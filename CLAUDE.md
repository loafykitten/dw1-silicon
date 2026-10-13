# dw1-silicon

A native arm64 macOS port of Digimon World (PS1, USA SLUS-01032), built from the
jype0/dw_decomp matching decompilation with the PS1 libraries replaced by a Mac layer
written in Rust (DW2-PC is the reference). **Read `PLAN.md` before doing anything**: it's the handoff,
with the goal, ground rules, sources, architecture, oracle, phases and open questions.

- **Current status (2026-10-13, session 2):** plan reviewed by Astra and revised
  (`docs/plan-review-1.md`); direction set: hybrid (decomp C game + Rust for everything
  new), full Rust as the endgame. S0.1 done (tools in `docs/toolchain.md`; repo on branch
  `phase0`, nothing committed yet); S0.5 survey done (`docs/survey.md`); arm64 can't map
  below 4 GB (measured). dw_decomp comes in as a submodule of Fenn's fork (waiting on the
  fork). Upstream clones in gitignored `ref/`. PLAN.md's "Where things stand" is the
  handoff; keep it and this line current at the end of every session.
- **Never commit disc data** (image, extracted files, assets, dumps, captures). They live
  under the gitignored `disc/`, `dumps/`, `build/`, `captures/`, `ref/`. Check `git status` before
  every commit.
- **Git:** work branch, never `main` directly; commit and push at your discretion (Fenn,
  2026-10-13).
- **Model routing:** Claude implements (subagents per PLAN.md); gpt-6.1-sol (high) reviews
  every implementation, GPT 6 Astra (high) for touchy work; bulk investigation and
  computer use go to gpt-6.1-sol through Codex. Never Haiku.
