# Bugs found in play (Phase 9)

Ember's playthrough (Phase 9.5, `docs/phase9-plan.md`). One entry per bug, newest first. When
something looks wrong, press **F12** in the game: a bug snapshot goes to
`~/Library/Application Support/dw1-silicon/reports/<time>/` (the screen, the game's state, the
last minute of input, the settings, copies of the save slots and the log). Say which snapshot,
and roughly what happened; the snapshot folders stay on the Mac (they hold game data and are
never committed).

For each bug:

- **What happened** and what should have happened (on the PS1 / DuckStation, if known).
- **Where:** place, day, what you were doing.
- **Snapshot:** `reports/<time>`; the save slot just before it, if there is one.
- **Status:** open / reproduced / fixed in `<commit>` / the original game does it too.

## Open

### Misty Trees 1: collision-span stop after a win (2026-11-10)

- **Report:** after winning against a yellow Ogremon-like enemy in Misty Trees 1, the player
  sees `MAP_COLLISION_DATA index outside the PS1 span it shares with the tamer model`.
  The stack reaches `getTileTrigger` <- `renderDropShadow` <- `renderDigimon` <-
  `renderObjects` <- `BTL_battleTickFrame` <- `BTL_deinitializeCombat + 388`.
  This is a native boundary guard, despite the dialog's generic "unimplemented SDK" wording.
- **Evidence:** the original live map is 115 / MIST01. The original live index was NOT
  captured before the app closed. A private COPY of a save was relocated near Fugamon and
  its partner stats boosted solely to reach victory. That fixture reproduces the same
  map/stack/guard, not the exact unsaved battle. A retreating enemy retains its on-screen
  flag at (-1416, 0, -5376), animation 0x24; tile (35, 103), index `0x285f`. This would read
  PS1 `STDVS_CAMERA + 0x5f`, outside the represented span. Native camera pointer layout is
  not interchangeable with PS1 bytes. Off-grid reads within the preceding tamer-model span
  remain meaningful (`docs/phase9-sanitizers.md`); do not globally clamp them to dummy ground.
- **Change:** keep represented collision reads in their original order. For unrepresented
  SHADOW queries only, first project the four actual ground vertices. Skip the read/draw only
  if a common separating screen edge proves the quad invisible, with a four-pixel HD margin
  and conservative MAC/IR/depth-saturation/viewport checks. Crossing/enclosing quads remain
  potentially visible even with every corner outside. Such quads and uncertain cases still
  reach the original fatal guard. No ordering-table link/work-base advancement occurs for
  a culled quad. The strict collision interval `[-0xe000, 0x2800)` is unchanged. Non-native
  source behavior is unchanged. No simulation, input, save or asset changes are included.
- **Measured reproducer projection:** (-6, 1023), (125, 958), (-46, 830), (73, 773), drawing
  offset (320, 44), flags `0x80003000`: wholly below the expanded visible bottom (199).
- **Clean contribution checks:** `python3 tools/tests/drop_shadow_bounds.py` compiles the
  real shadow routine and tile-coordinate helper with recording GPU calls, no assets/saves.
  169 native scenarios plus 8 unchanged-guard boundary checks pass, including expected fatal
  cases; 169 retained non-native scenarios plus 8 guard checks pass. Covers every edge,
  crossing/enclosing/partial/margin quads, camera offsets, saturation, represented negative
  and unsupported indices, trigger rejection, packet/link/work-base invariants. The pre-fix
  source fails the first native offscreen case. Preprocessed non-native translation-unit
  tokens are identical (native ABI test headers, not a retail matching build).
- **Clean native validation:** the unmodified phase0/decomp baseline reproduces the guard
  in the private fixture. The isolated correction completes through VBlank 18000 and returns
  to field state on map 115 at 1x and 6x (precise vertices/full colour). All 85 pre-stop
  canonical checkpoints match the baseline; all 118 scale-1/6 checkpoints match. The existing
  new-game recording completes with 19/19 native before/after checkpoints identical.
  Workspace: 227 passed, 0 failed, 27 fixture-dependent ignored. Release build passes.
  These are native-to-native regressions, NOT new original-PS1/oracle certification.
- **Manual scope:** the player subsequently reports "it works" after the installed-build
  restart/battle retest request (2026-11-10); exact route, enemy and repetition count were
  not supplied. That app also contained separate local keyboard, Techsheet, pause, map-row
  and textbox corrections. None is included here; clean checks above were rerun without them.
  Manual testing used keyboard only; no physical controller was available. Original saves
  and preferences were unchanged during installation. No full-playthrough claim is made.
- **Publication (player authorised, 2026-11-11):** independent drafts
  https://github.com/loafykitten/dw1-silicon/pull/7 and
  https://github.com/loafykitten/dw_decomp/pull/3. The upstream submodule URL is unchanged;
  a fresh recursive public clone with no local object references passes the regression.
- **Draft gates/provenance:** original-PS1 pixels/behavior, the full oracle fixture suite
  and independent Sol/Astra review remain outstanding; no review convergence is claimed.
  The decomp companion must be accepted before final integration. No disc contents, assets,
  saves, screenshots, dumps, generated headers, SDK captures, apps or binaries are uploaded.
  No SDK implementation was translated. This does not certify all post-battle crash causes.
  Generated by Codex (GPT-6.1 sol).

(none yet)

## Fixed

(none yet)
