# S0.8: oracle bring-up (first slice)

2026-10-13. PCSX-Redux 5811e22 (built from source, `ref/pcsx-redux`), Ember's disc and BIOS.

## Running it

Headless, scripted, no computer use needed:

```
cd captures/oracle-runs/<run>      # gitignored; Redux writes its config and card files here
ref/pcsx-redux/pcsx-redux -cli -portable -interpreter -softgpu \
    -bios scph1001.bin -iso "disc/Digimon World (USA).cue" \
    -dofile oracle/probe-boot.lua -run > run.log 2>&1
```

- `-cli` runs without a window, prints Lua `print` and the game's own debug output to
  stdout; the script ends the run with `PCSX.quit`.
- **`-interpreter`**: the dynarec exits immediately on arm64. The interpreter runs at about
  real time (600 VBlanks in 11 s).
- **Avoid Exec breakpoints:** with one active the interpreter drops to ~3 VBlanks/s. Read
  memory once per VBlank instead (`GPU::Vsync` event + `PCSX.getMemPtr()`).
- Input: `PCSX.SIO0.slots[1].pads[1].setOverride(bit)` / `clearOverride(bit)`, PS1 pad bits
  (Start 3, Cross 14).

## Pinned reference

| | |
|---|---|
| Emulator | PCSX-Redux 5811e223d3c3 (2026-10-13), interpreter, soft GPU, portable config |
| BIOS | Ember's `scph1001.bin`, SHA-1 `649895efd79d14790eabb362e94eb0622093dfb9`. Redux knows its CRC (55847d8c) but has no name for it. Strings: kernel "PS-X Realtime Kernel Ver.2.5", "BOOTSTRAP LOADER Type C Ver 2.1 03-JUL-1994", kernel date 1994-09-22; "System Controller ROM Version 99/02/01 c3" (Redux's emulated CD controller). |
| Disc | Ember's redump (SHA-1 in PLAN.md) |
| Cards | Redux creates empty `memcard1.mcd`/`memcard2.mcd` in the run directory |

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
