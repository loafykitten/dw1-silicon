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
column is multiplied with the previous column's Z. Every bone matrix in the game came out wrong:
in a capture at VBlank 14000 all 14 sampled parent-child products fit "MVMVA used the previous
VZ0" exactly, and characters rendered as flat streaks or not at all. On 26bfd693 the same products
are exact and the characters render. The game is correct on hardware, so #2283's delay is too long
for this sequence (its PR notes the timing was fitted to one measured pair, SQR on IR1). Not
reported upstream yet.

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

## Not done yet

- Real replays: a recorded input stream keyed to pad polls (the probe's schedule is a fixed
  stand-in), canonical state dumps at checkpoints, certification on booklab.
- Where the game is at VBlanks 1393/1423 (probably the new-game intro), and getting further
  (the probe stops making progress after that; it needs menu-specific input).
- The field rate (59.826 vs 59.94 Hz) and `VSync(n)` timing.
- Whether OpenBIOS's `rand` and boot state match (only matters if we ever drop the retail
  BIOS).
