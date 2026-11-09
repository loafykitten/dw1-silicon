# dw1-silicon

A native arm64 macOS port of Digimon World (PS1, USA SLUS-01032), built from the
jype0/dw_decomp matching decompilation with the PS1 libraries replaced by a Mac layer
written in Rust (DW2-PC is the reference). **Read `PLAN.md` before doing anything**:
it's the handoff, with the goal, ground rules, sources, architecture, oracle, phases
and open questions.

- **Draft native pause-loop contribution (2026-11-09):** a live combat freeze was traced to
  cached PadRead input in the no-VSync pause wait. The pinned decomp proposes native-only
  field waits in shared and VS pause loops, with 28 synthetic branch/scenario checks in
  `tools/tests/pause_loop.py`. Live keyboard combat pause/resume was verified twice on the
  local combined build; physical gamepads, live VS and original-PS1 pause timing are not
  certified. This is an alternative to upstream dw1-silicon PR #1, not a new issue; no
  Techsheet, keyboard-mapping or map-row changes are included. Clean workspace tests:
  227 passed, 0 failed, 27 fixture-dependent ignored; release builds. Native code is in
  loafykitten/dw_decomp draft #1. The draft gitmodules URL temporarily uses the authorized
  contributor fork for reproducible checkout; restore upstream/repin after dependency merge.
  See `docs/bugs.md`.

- **Current status (2026-10-15, session 6):** Phases 1-4, 6 and 7 done; Phase 5
  (overlays) has battles, training and the Item Keeper certified; Phase 9's streams reached the
  rest without Ember's saves (debug room, downloaded saves, moved saves): forty-three replays
  in `tools/replay-check.sh`, all identical. Phase 8 (renderer) done: wgpu
  window (`crates/render`), HD surfaces at 2-8x (`backend::hd`) and no wobble (`backend::pgxp`)
  beside the exact 1x VRAM, settings with hotkeys, "full colour" (F5, session 6); every replay
  identical with them all on (`docs/phase8-plan.md`). Full colour on by default (Ember).
  No wobble no longer cracks models. Phase 9 under way (`docs/phase9-plan.md`): 9.1 first-run
  disc screen, 9.2 the `.app` (`tools/make-app.sh`), 9.3 F12 bug snapshots and 9.4 the guide
  (`docs/guide.html`) done; next 9.5, Ember's playthrough (bugs in `docs/bugs.md`).
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
