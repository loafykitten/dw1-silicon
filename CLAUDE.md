# dw1-silicon

A native arm64 macOS port of Digimon World (PS1, USA SLUS-01032), built from the
jype0/dw_decomp matching decompilation with the PS1 libraries replaced by a Mac layer
written in Rust (DW2-PC is the reference). **Read `PLAN.md` before doing anything**:
it's the handoff, with the goal, ground rules, sources, architecture, oracle, phases
and open questions.

- **Current status (2026-10-14, end of session 6):** Phases 1-4, 6 and 7 done; Phase 5
  (overlays) has battles, training and the Item Keeper certified (`tools/replay-check.sh`, ten
  replays), the rest waits on fixture saves from Ember. Phase 8 (renderer) mostly built: wgpu
  window (`crates/render`), HD surfaces at 2-8x (`backend::hd`) and no wobble (`backend::pgxp`)
  beside the exact 1x VRAM, settings with hotkeys, "full colour" (F5, session 6); every replay
  identical with them all on (`docs/phase8-plan.md`). Full colour on by default (Ember).
  Next: close Phase 8, then Phase 9.
  Public repo `loafykitten/dw1-silicon` (branch `phase0`); decomp fork branch `dw1-native`
  pushed. Direction: hybrid (decomp C game + Rust for everything new), full Rust as the
  endgame (not Bevy: asked again 2026-10-14, kept). Sony's SDK is reimplemented clean (never
  translated from disassembly). PLAN.md's "Where things stand" is the handoff, including how to
  work with the oracle; keep it and this line current at the end of every session.
- **Never commit disc data** (image, extracted files, assets, dumps, captures). They live
  under the gitignored `disc/`, `dumps/`, `build/`, `captures/`, `ref/`. Check
  `git status` before every commit.
- **Git:** work branch, never `main` directly; commit and push at your discretion (Ember,
  2026-10-13).
- **Model routing:** Claude implements (subagents per PLAN.md); gpt-6.1-sol (high) reviews
  every implementation, GPT 6 Astra (high) for touchy work; bulk investigation and
  computer use go to gpt-6.1-sol through Codex. Never Haiku.
