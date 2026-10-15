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
