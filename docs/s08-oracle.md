# S0.8: oracle bring-up (first slice)

2026-10-13. PCSX-Redux 26bfd693 (built from source, `ref/pcsx-redux`; see "Pinned reference" for
why not the newer 5811e22), Ember's disc and BIOS.

## Running it

Headless, scripted, no computer use needed:

```
R=/Users/fenn/Developer/dw1-silicon
mkdir -p "$R/captures/oracle-runs/<run>" && cd "$R/captures/oracle-runs/<run>"   # fresh; gitignored
"$R/ref/pcsx-redux/pcsx-redux" -cli -portable -interpreter -softgpu \
    -bios "$R/scph1001.bin" -iso "$R/disc/Digimon World (USA).cue" \
    -dofile "$R/oracle/probe-boot.lua" -run > run.log 2>&1
```

Redux writes its card files into the current directory, so each run gets a fresh one.

- `-cli` runs without a window, prints Lua `print` and the game's own debug output to
  stdout; the script ends the run with `PCSX.quit`.
- **`-interpreter`**: the dynarec exits immediately on arm64. The interpreter runs at about
  real time (600 VBlanks in 11 s).
- **Debugger only when needed:** `-debugger` (needed for breakpoints) slows every instruction,
  to ~3 VBlanks/s. Read memory once per VBlank where possible (`GPU::Vsync` event +
  `PCSX.getMemPtr()`); for breakpoints, switch the debugger on from Lua at the point of
  interest (`PCSX.settings.emulator.Debug.Debug = true`, as `oracle/capture-libgs.lua` does).
- **One emulator at a time.** Two concurrent runs stalled (idle, no output) on the pinned
  build; alone it runs at about real time.
- After switching commits, rebuild clean (`rm -rf objs/Release bins/Release`): an incremental
  build across #2283 hung in the BIOS.
- Input: `PCSX.SIO0.slots[1].pads[1].setOverride(bit)` / `clearOverride(bit)`, PS1 pad bits
  (Start 3, Cross 14).

## Pinned reference

| | |
|---|---|
| Emulator | PCSX-Redux 26bfd693 (main, 2026-10-12, the merge before PR #2283), interpreter, soft GPU, portable config |
| BIOS | Ember's `scph1001.bin`, SHA-1 `649895efd79d14790eabb362e94eb0622093dfb9`. Redux knows its CRC (55847d8c) but has no name for it. Strings: kernel "PS-X Realtime Kernel Ver.2.5", "BOOTSTRAP LOADER Type C Ver 2.1 03-JUL-1994", kernel date 1994-09-22; "System Controller ROM Version 99/02/01 c3" (Redux's emulated CD controller). |
| Disc | Ember's redump (SHA-1 in PLAN.md) |
| Cards | Redux creates empty `memcard1.mcd`/`memcard2.mcd` in the run directory |

**Why 26bfd693 and not 5811e22.** Redux PR #2283 ("Interpreter: delay GTE register writes by two
instructions", merged 2026-10-12 as 54c357f7) makes an MTC2/CTC2/LWC2 value invisible to a GTE
command in either of the next two instructions. Sony's `MulMatrix` (DW1's at 0x8009ADC0, used by
`GsGetLws` through `GsMulCoord3`) writes VZ0 and issues MVMVA one `nop` later, so under #2283 each
column is multiplied with the previous column's Z. In the libgs captures (VBlank 14000), 92 of
106 parent-child bone products were wrong on 5811e22 and all 106 fit "MVMVA used the previous
VZ0" exactly, and characters rendered as flat streaks or not at all. On 26bfd693 all 183 are exact
and the characters render. A standalone test EXE (VZ0 0x100 then 0x200, N nops, MVMVA) gives the
new value at N=1 on 26bfd693 and the old one on 5811e22. The game is correct on hardware, so
#2283's delay is too long for this sequence. Upstream's hardware test (nugget
`tests/gte-store-delay`) does show the old value at N=1 for SQR reading IR1 and OP reading a
control register, so the delay probably depends on when each command reads its operands.
Reported as grumpycoders/pcsx-redux#2322 (2026-10-13); unpin once it's fixed.

## Results

- **The game boots** on the retail BIOS: `SYSTEM.CNF` → `SLUS_010.32` at 0x800A4240, then
  the intro and landing-screen loop (the game's own debug prints: `ResetGraph`, `CD_init`,
  `CdRead: retry...`).
- **The game's `rand()` is the BIOS generator.** The SDK stub at 0x8009127C jumps to BIOS
  A(2Fh) at 0xBFC021F0: `x = x * 0x41C64E6D + 0x3039`, returns `(x >> 16) & 0x7FFF`, state at
  RAM 0x9010 (disassembled from Ember's BIOS). Only A(2Fh)/A(30h) touch 0x9010.
- **The state is 0 at boot**, and with the probe's button schedule (Start/Cross from VBlank
  600) the first calls come at VBlank 1393 (6 calls) and 1423 (200 calls); every change is
  reachable by stepping the generator. `psyq::bios` reproduces both states
  (`rand_matches_the_emulator`).
- **Deterministic:** two fresh emulator processes gave identical probe output (seed, call
  counts, VBlank numbers).

## Replays, canonical state, certification (2026-10-13, session 2)

**Scripts.** `oracle/replay.lua` (modes `record` and `replay`), the committed replay
`oracle/replays/new-game.txt` (7131 polls; pad masks, VBlank numbers and caller addresses, no
game data), `oracle/canonical-state.lua` (schema in `docs/canonical-state.md`) and
`oracle/field-rate.lua`. Same command line as above with `-dofile "$R/oracle/replay.lua"` and the
environment `DW1REPLAY_MODE=record|replay`; `DW1REPLAY_FILE` (replay input, default
`oracle/replays/new-game.txt`), `DW1REPLAY_OUT` (record output, default `./replay.txt`),
`DW1REPLAY_TARGET` (route milestone to stop at, default `gameplay`).

- **Pad hook.** The game reads the pad only through `PadRead` (0x80091BE8). The script breaks on
  its entry (reads `$ra`) and on `$ra` (reads `v0` at return). Record logs
  `seq vblank mask ra`; replay installs no pad overrides and overwrites `v0` with the recorded
  mask. It fails with exit 1 and the sequence number if a caller address differs, if polls run
  past the recording, or if fewer were consumed at the end, and it reports whether the VBlank
  numbers line up. Breakpoints need `PCSX.settings.emulator.Debug.Debug = true`, switched on from
  Lua at start; it did not slow emulation (385 s with breakpoints vs 383 s without, 19400 VBlanks).
- **The 7131 polls:** 3315 from `MOV_playMovie` (0x800108D8, the FMV Start test, 2 VBlanks apart
  from VBlank 1651), 2656 from `pollInputGame` (0x800EFD84), 994 from `pollInputMenu`
  (0x800EFE2C), 166 from `handlePause` (0x800E633C); 453 have a button down. The pad is read
  throughout, including gameplay (the last poll is at VBlank 19398).
- **Pitfall:** every Lua breakpoint costs a LuaJIT ffi callback and a process gets only about
  2000, so a breakpoint per poll died after 2043 polls ("too many callbacks", and Redux logs
  "Lua Breakpoint invoker threw an exception, deleting breakpoint" and carries on unobserved,
  which looks like a clean run that simply stops polling). The return breakpoints are now
  persistent, one per distinct caller (at most 14), and every callback is wrapped to fail the
  run on a Lua error.
- **Checkpoints** (canonical dump, `ckpt_NNNN_<why>.bin/.txt` in the run directory): at each change
  of `MAIN_STATE`, `GAME_STATE` or `CURRENT_SCREEN` (tested at each poll return), every 600 polls,
  and at the end. The gameplay route makes 19: three at the first poll (initial values), 11
  "every 600 polls", screen changes to 204, 238, 218 and 204 again, and `final`. MAIN_STATE and
  GAME_STATE stay 0 for the whole route.

### Certification

All runs in fresh empty directories under `captures/oracle-runs/s08-replay/`, one emulator at a
time, the pinned binary and flags above.

| Run | Wall | Result |
|---|---|---|
| record (route + hooks) | 385 s | 7131 polls, final VBlank 19400, 19 checkpoints |
| replay 1 (fresh process) | 384 s | 7131 of 7131 polls, 0 VBlank mismatches, 19 checkpoints |
| replay 2 (fresh process) | 385 s | same |
| plain `route-new-game.lua` | 383 s | no hooks, for the perturbation check |

- **Replays are certified:** all 19 checkpoint dumps (834 bytes each) are byte-identical across
  record, replay 1 and replay 2, the checkpoint names and poll/VBlank numbers agree, and every
  replayed poll came from the same caller address with the same VBlank number as recorded.
- **The debugger does not perturb emulation:** the record run's BIOS rand state at VBlank 19400
  (`0xb738fdb3`) and all 16 milestone screenshots (`.raw`) are byte-identical to the plain
  route run's.
- **Second machine (booklab, Apple M1, same pinned Redux binary, same BIOS and disc SHA-1s):**
  the replay of `oracle/replays/new-game.txt` exited 0 in 389 s and all 19 checkpoint dumps are
  byte-identical to this Mac's replay 1 (run by Ember's coordinator).
- **Negative test 1:** changing the caller address of poll 20 makes the replay fail with exit 1
  at poll 20 ("divergence at poll 20").
- **Negative test 2:** in a copy of the replay file, the whole CROSS press at polls 474-477
  (VBlanks 2400-2403, the title menu's NEW GAME) was zeroed. The replay exits 1 after 64 s:
  "divergence at poll 1045: caller 0x800efe2c, recorded 0x800efd84 (vblank 3217)" (the game was
  still in a menu loop, `pollInputMenu`, where the recording had reached `pollInputGame`). The
  four checkpoints written before that are identical to the recording's. Zeroing only polls
  474-475 still exited 0 with identical checkpoints, because the press continued at 476-477.
- **Scope.** This certifies replays on the pinned Redux configuration (26bfd693, interpreter, soft
  GPU, this BIOS and disc) on two machines. Keying to polls does not make a replay independent
  of CPU timing: CD completion that crosses a VBlank, and the movie code's CPU-iteration limits
  (`MOV_decodeFrame`, `MOV_waitForFlip`), can change control flow, and a different emulator
  build or core could diverge. The replay fails loudly on such a divergence rather than hiding
  it.
- **Hardening after review:** the run only exits 0 through the real completion path. At the end it
  checks that the debugger is still on, the entry and return breakpoints are enabled and no
  `PadRead` is pending; a Quitting event before completion exits 1; canonical dump writes check
  every write and close. Re-recorded and replayed twice afterwards: the replay file and all 19
  dumps are byte-identical to the first certification.

### Field rate

`oracle/field-rate.lua`: `PCSX.getCPUCycles()` at each `GPU::Vsync`, collecting exactly 1000 real
intervals right after the route reaches gameplay (VBlanks 19400-20400, game idle, no debugger):
mean **564398.0** cycles per VBlank (564397.997), min 564154, max 564649, so
**Hz = 33868800 / 564398.0 = 60.0087**. That is Redux's 2146 x 263 = 564398
(`src/core/psxcounters.cc`); the +-250 cycle spread is the VBlank event being taken at the next
instruction boundary. Hardware NTSC non-interlaced is 53.693175 MHz / (3413 x 263) = 59.817 Hz
(566204 CPU cycles per field), so a Redux field is 0.32% shorter in CPU cycles.

What that means for the oracle: the game counts VBlanks (`VSync(n)`, the clock advances 20 ticks
a minute per game update), so logic keyed to VBlanks sees the same number of frames, but each
Redux frame holds about 0.3% fewer CPU cycles than hardware. Anything that depends on CPU time
inside a frame (movie code's iteration limits, CD/SPU completion) can behave differently from
the console, and this has not been checked against hardware. A port that wants hardware pacing
should present at 59.82 Hz (non-interlaced); the oracle's 60.009 Hz comes from Redux's counter
constants, not from the game.

## Not done yet

- Where the game is at VBlanks 1393/1423 (probably the new-game intro), and getting further
  (the probe stops making progress after that; it needs menu-specific input).
- `VSync(n)` timing against hardware (the field rate itself is measured, above).
- Whether OpenBIOS's `rand` and boot state match (not done; only matters if we ever drop the
  retail BIOS).
