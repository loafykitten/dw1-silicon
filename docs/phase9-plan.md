# Phase 9: the long tail, packaging and the guide (plan and progress)

2026-10-14, session 6. PLAN.md's Phase 9: "Ember plays start to finish (with fixture saves along
the way); bugs go to `docs/bugs.md` with save and repro. Then packaging (`.app` that runs on a Mac
without Homebrew, first-run pack build, settings) and the deliverable: a self-contained HTML
guide (`docs/guide.html`) on installing, building and playing. This is the first release: a
faithful hybrid."

## What packaging means here

- **Nobody downloads a built app.** The binary holds the game's code compiled from the decomp
  and its text (generated from the decomp's `text/*.yaml`), so it is as unshareable as the disc.
  Every player builds it on their own Mac from this repo and their own disc image. "Packaging"
  is a script that turns that build into a normal `.app`, and a first run that finds the disc.
- **The app already needs nothing but macOS** (checked 2026-10-14: `otool -L target/release/dw1`
  lists only system frameworks; SDL3 is built from source and linked statically). The build
  needs Xcode's command line tools (clang, `xcrun`, `nm`, python3), Rust (rustup), CMake (SDL3's
  build) and PyYAML (the decomp's `tools/gen_text.py`). The guide says how to get each without
  Homebrew (CMake from cmake.org or `pip3 install --user cmake`, PyYAML with `pip3 install
  --user`); Homebrew works too.
- **No pack is built.** The plan's "first-run pack build" predates the disc reader: the game
  reads the redump `.cue`/`.bin` directly and checks the whole image's SHA-1 at start
  (`pack::Disc::verify`, about a second). The first run only has to find the image.

## Slices

Each slice: implemented (main session or a Sonnet subagent), reviewed by gpt-6.1-sol (high)
until no finding is left, checked as below. 9.1-9.3 in order; 9.4 any time; 9.5 runs alongside.

### 9.1 First run finds the disc

The window build without `DW1_DISC` reads the remembered image path from
`~/Library/Application Support/dw1-silicon/disc.txt`; with none (or the file gone), a small
window says what it needs and opens the macOS file dialog for the `.cue` (a `.bin` alone is
accepted when a `.cue` with the same name sits beside it, or synthesised for a single-track
MODE2/2352 image). The image is verified with a progress note; a wrong image gets a plain
message saying which release it looks like (EU/JP serials, a different hash, a multi-track or
compressed image) and the dialog again. A good image's path is remembered; it is never copied or
changed. Dragging a `.cue` onto the window works too.

- Headless and replay runs keep today's behaviour (`DW1_DISC` or the repo's `disc/`).
- **Checks:** unit tests for the path logic (remembered, missing, `.bin`-only, wrong hash) with
  small synthetic images; a run with a temporary `HOME` and no `DW1_DISC` shows the picker
  (Codex computer use, gpt-6.1-sol), accepts the real image, and starts the game; the second
  run starts straight away.

### 9.2 The `.app`

`tools/make-app.sh`: release build, then `build/dw1-silicon.app` (Info.plist: name, bundle id,
version from Cargo, minimum macOS, high-resolution capable, gamepad support), an original icon
(no game art), ad-hoc signed (`codesign -s -`), optionally copied to `~/Applications`. The
script fails if the binary links anything outside `/System` and `/usr/lib`.

- Launched from Finder the app has no terminal: stderr goes to
  `~/Library/Logs/dw1-silicon/dw1-<date>.log` (the last few kept), with `DW1_CRASH_TRACE` on, so
  a crash leaves a report Ember can send. A stop on something the port doesn't handle shows a
  message box with the log's path.
- First run with no saves seeds the slots from DuckStation's card if one exists (read only), as
  `dw1-mac` does; `dw1-mac` stays as the terminal way to play (messages in the terminal).
- **Checks:** the app starts from Finder and from `open`; with a temporary `HOME` it takes the
  first-run path; the log appears; booklab (an M1 without this repo's toolchain state) runs a
  copy of the app, if reachable, as the "another Mac" test.

### 9.3 Bug reports

`docs/bugs.md` (template: what happened, where, the save slot, the log, steps). F12 in the window
writes a bug snapshot to `~/Library/Application Support/dw1-silicon/reports/<time>/`: the
current save slot's file, a picture of the screen, the log so far, the settings and the last
minute of pad input, so a bug found in play comes with what's needed to repeat it. Nothing
leaves the Mac.

- **Checks:** a snapshot from the window build has every part; a test for the pad-input ring.

### 9.4 The guide and the README

`docs/guide.html`, one self-contained page: what this is, what you need (a Mac with Apple
Silicon, your own disc image of Digimon World (USA), the build tools), building step by step,
making and opening the app, first run, controls (keyboard and gamepad), picture settings (each
labelled original or addition), saves (slots, backups, moving them to and from DuckStation),
reporting a bug, what is and isn't checked against the original yet, credits and license. The
README's status and build section updated to match.

- **Checks:** every command in the guide run as written on this Mac from a fresh clone in a
  temporary directory; Ember reads it.

### 9.5 The playthrough

Ember plays start to finish in the app. Each bug goes to `docs/bugs.md` with its snapshot, then
gets a repro (natively from the save; on the emulator when it might be the original game's own
behaviour), a fix, and where possible an oracle replay that covers it. Saves made just before
the overlays not yet checked (Battle Mode, tournaments, fishing, digivolving, death and rebirth,
curling, recruiting, the other gyms) become fixtures and certified replays (Phase 5's method).

## Done when

The app builds from a fresh clone by following the guide, runs from Finder without a terminal,
finds the disc on first run, and Ember has played through with every bug found fixed or
written down as accepted. That is the first release.

## Progress

- 2026-10-14: plan written.
- **9.1 done** (session 6): `crates/dw1/src/disc.rs` (which image: `DW1_DISC`, `DW1_ASK_DISC=1`
  to ask anyway, `disc.txt`, the repo's `disc/`; plain messages for compressed images, `.iso`s,
  missing files, other releases named from `SYSTEM.CNF`) and `window/disc_picker.rs` (the screen,
  the macOS file dialog, drag and drop). Every image is checked on its own thread with the window
  answering ("Checking the disc image..." for a moment at each start). The disc now goes in before
  the saves in both modes (the save machine's text patch needs it; it asserts so). Sol reviewed
  five times (seven findings: the start-up order regression, three responsiveness, two messages,
  extensionless `DW1_DISC`), converged. Not yet tried by hand: the dialog and drag and drop.
- **9.2 done** (session 6): `tools/make-app.sh [--install]` builds `build/Digimon World.app`
  (refuses a binary linking anything outside macOS; Info.plist from `packaging/Info.plist.in`;
  icon from `packaging/icon-1024.png`, an original pixel egg drawn by `tools/make-icon.py`;
  signed ad hoc; `--install` only replaces our own bundle in `~/Applications`).
  `crates/dw1/src/applog.rs`: an app run logs to `~/Library/Logs/dw1-silicon/` (10 kept, each
  locked by its run, pruning under a folder lock), shows stops (every PS1-layer stop now goes
  through `psyq::trap::stop`), panics and start-up failures in a message box (main thread only),
  and reports a crash at the next start (a marker per log, liveness by the log's lock, removed
  only once reported). DuckStation's card seeds kept slots on their first run. Sol reviewed six
  times (thirteen findings, mostly log-loss races and headless isolation), converged. Checked
  live: the app starts from `open`, logs, and its marker goes on quit. Not yet: booklab
  (unreachable), a crash report and the stop box seen by hand.
- **9.3 done** (session 6): F12 writes a bug snapshot (`window/snapshot.rs`: screen at 1x and as
  shown, the canonical state, the last minute of pad input as `DW1_PAD_SCRIPT` lines, settings,
  save slot copies, the log, `about.txt`) to `Application Support/dw1-silicon/reports/<time>/`;
  `docs/bugs.md` for the playthrough. Sol: two passes (three findings fixed).
- **9.4 written** (session 6): `docs/guide.html` and the README's build section. The tools part
  needs no Homebrew: PyYAML and CMake go in the repo's `.venv` (the build already prefers its
  Python; `make-app.sh` uses its CMake when none is on PATH). Sol found five problems in the first
  draft (Homebrew Python refusing `pip --user`, Battle Mode's player 2 described wrongly, the
  DuckStation card's location, the care claim, the copy buttons), fixed. A fresh clone from
  GitHub built and matched the new-game replay (19/19). Ember read it: "looks great"
  (2026-10-15).
- **Before the playthrough: the unrecorded parts** (session 6, 2026-10-15). Sol's static audit of
  the overlays no recording reaches (`docs/phase9-overlay-audit.md`) found definite native bugs in
  digivolution (EVL), death and rebirth (DOOA/DOO2), tournaments and Battle Mode (STD/VS), the boss
  buildup (EAB) and the ending (ENDI); three Sonnet subagents fixed them in the decomp fork, and
  the debug room (`docs/phase9-routes.md`: a fixture save with four story flags zeroed) gave
  recordings of digivolution and death. Those found three more things the audit couldn't see: an
  out-of-range model index the PS1 reads from the next part of the model file (`sort4j`, bounded
  by the table `GsMapModelingData` saw), empty entity slots in EVL/DOOA/EAB's visibility loops (as
  MURD's), and STATS_LIMIT_ARRAY read one past its end for lives (the PS1 read 1 there). Now
  certified: `debug-evo` (79/79 dense checkpoints) and `debug-death` (116/116); twelve replays in
  `tools/replay-check.sh`. Astra reviewed the batch twice (one finding: the model-header search
  could read outside the model, replaced by the mapped-table lookup), converged. Decomp fork
  9102d79. Still open: the missing `GsTMDfast*` handlers (audit finding 9: arenas, result scenes,
  Seadramon, curling stones) and `SetDrawLoad`; recordings of fishing, curling, tournaments,
  recruiting, the other gyms, Battle Mode (Redux's second card slot never answers), the boss
  buildup and the ending; three judgement calls to check against the emulator when reached (the
  native stops on a finisher's uninitialised `tech`, VS intro's `prev = 0`, ENDI's stale stack
  word).
- **Sanitizer runs** (session 6, 2026-10-15, `docs/phase9-sanitizers.md`): UBSan and ASan builds
  of the game C over the twelve replays found PS1-layout dependencies the replays happened not to
  expose: the item menu box allocated at the PS1's 0x20 bytes (natively 40: six rows overwrote
  the next allocation's header), maps with 4-5 CLUTs storing into CAMERA_TARGET past MAP_CLUTS,
  seven stray buff-trail stores (one of which corrupted the battle command layouts natively), the
  tamer model / scratch / collision grid and the maphead / script offset tables being single PS1
  spans that natively held together only by declaration order, plus out-of-range reads made
  explicit. All fixed (decomp f5decbe), Astra twice. Build-wide: one macOS deployment target
  (11.0) in `.cargo/config.toml` (SDL had been built for the build machine's macOS).
- **Parallel streams** (2026-10-15): three Opus 5.5 subagents in worktrees under
  `/Users/fenn/Developer/dw1-silicon-wt/` (render: the missing GsTMDfast handlers; fishcurl:
  fishing, curling, recruiting, the other gyms; arena: tournaments and Battle Mode). Three
  emulators run concurrently and deterministically (measured).
- **Render stream** (2026-10-15, `docs/phase9-render.md`): audit finding 9 and `SetDrawLoad`
  done. Every primitive kind the disc's 345 TMDs use has a native handler (`libgs/tmd.rs`: F3L,
  F4L, G4L, TF3L, TF4L, TNF3, TNF4 with mode 0x2D, TNG3, TNG4 alongside TG3L/TG4L), the arenas'
  `GsDIV1` floors divide (`libgs/divide.rs`: TNF3/TNF4/TNG3/TNG4, in the scratchpad as the SDK
  does), hidden objects are skipped, `GsOT.offset` is honoured, and `SetDrawLoad` builds the
  EFE's CLUT loads. Proven on the real SDK with synthetic objects through `call-sdk.lua` (new:
  scratchpad access, `DW1_RESTORE`): 5,300 `GsSortObject4J` requests and 160 `SetDrawLoad`
  rectangles match word for word (packets, table, scratchpad, GTE), and every disc model sorts
  natively without a stop. Found on the way: the SDK drops polygons whose projection has a FLAG
  error (the old TG handlers didn't), writes some packet words before its tests, and subtracts
  `GsOT.offset`.
- **Tournaments and Battle Mode certified** (arena stream, 2026-10-15). `battle-mode` (two cards:
  BeelzemontZero's and Bob's saves, `p9-arena-vs-p1`/`-p2`; 1 vs 1, the fight on Auto to "1P WIN"
  and back to the title; 4,375 polls, 15 checkpoints, 95 of 95 at every 50 polls) and `arena-cup`
  (Master99's save, `p9-arena-master99`: the Arena Lobby, today's Grade C cup, the first match won
  with a finisher, the second round's intro; 14,639 polls, 36 checkpoints, 304 of 304 at every 50)
  are certified on Redux (fresh replays byte-identical) and identical natively; fourteen replays in
  `tools/replay-check.sh`. Player 2's dead card prompt was Redux's pad 2 being unplugged by default,
  not the card (`docs/phase9-routes.md`, Battle Mode); two-player profiles plug it in and replays
  record `pad2 connected`. Native fixes: `VS__loadTextures` stored through the title menu's NULL
  `ENTITY_TABLE[0]` (the PS1 wrote a 0 to low RAM that already held 0: measured), and the EFE
  scratch areas from word 34 overlapped the native `EfeScratch` (the radial waves' ring counter
  overwrote its `scale` pointer; BTL/STD/VS share the header). Judgement calls: VS__runIntro's first
  `prev` is the caller's `$s1`, measured as 1 (only bit 0x40 is tested, so 0 and 1 behave alike;
  now 1); the STD/VS camera-curve and opcode-table fixes are confirmed by both replays matching;
  the finisher's uninitialised `tech` (STD_startQueuedMove / VS_startFighterMove) was not reached
  by either recording (no measurement, no native stop; the player's finisher in STD takes another
  path). From the disassembly the PS1's value is the caller's `$s3`: `VS__tickDigimonAttackRanged`'s
  `move` argument or `VS__tickDigimonOther`'s fighter index (STD's callers likewise), to confirm on
  the emulator when an AI finisher is recorded.
