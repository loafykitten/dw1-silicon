# Phase 2 plan: boot to the main menu

2026-10-13. PLAN.md's gate for Phase 2: the landing screen matches the emulator's at 1x, and
a replay reaches the main menu with no stub trapped. This file breaks it into slices, each
implemented, reviewed by gpt-6.1-sol (GPT 6 Astra for the GPU layer) and checked before the
next. Keep it current: mark slices done with the date and the commit.

## What main() does on the way

`src/main/main.c` `main()`: `initializeHeap`, `SsInit`, `ResetCallback`, `ResetGraph(0)`,
`SetGraphDebug(0)`, `initializeFramebuffer` (libgs display setup), `PadInit`, `CdInit`,
`MemCardInit`/`MemCardStart`, then the game's own initializers (models, world objects, the
TMD map, the file queue, fonts, scripts, UI, clock...), `loadTIMFile` twice, then the loop:
`playMovie(0, 1)` (the intro, MOV overlay), `initializeMusic`, `runLandingScreen` until Start,
`runMainMenu`. So Phase 2 touches every SDK library except the card's reads (Phase 3).

## Design decisions

- **The heap** (`InitHeap3`/`malloc3`/`free3`): a first-fit allocator in Rust over a static
  arena. The PS1 heap is 0x30390 bytes (`_end` 0x801BFB64 rounded up to 0x801BFB70, stack top
  0x801FFF00 minus 64 KB; read from `SLUS_010.32`). Native structs can be bigger, so the arena
  is larger, and the allocator reports when use passes the PS1 size (a sign the PS1 would have
  run out where the port didn't). `initializeHeap()` keeps calling `InitHeap3`; under
  `DW1_NATIVE` it passes nothing meaningful and the Rust heap ignores it.
- **GPU packet links:** a PS1 primitive's tag holds a 24-bit address of the next one;
  ordering-table entries hold the same. Natively a link is a 24-bit **offset from the start of
  the app's data segment** (`__DATA`, 1.3 MB today). Every ordering table and packet buffer is
  game data (globals, overlay "bss" files) or the heap arena, all inside it. 0xFFFFFF stays the
  end marker. Linking anything outside the window (a stack local, a host allocation) traps.
  The libgpu macros that do links (`setaddr`, `getaddr`, `addPrim`, `nextPrim`...) are
  redefined by a native `libgpu.h` that includes Psy-Q's and overrides them; `AddPrim` and
  libgs's sort calls use the same encoding.
- **The disc:** a `pack` crate reads Ember's `.cue`/`.bin` (MODE2/2352), checks its SHA-1
  against the known redump, and parses ISO9660 for `CdSearchFile`. libcd reads sectors from it.
  Copying the needed sectors into a separate pack file (DW2-PC's design) is a packaging
  question for Phase 9; until then the verified image is the pack.
- **Time:** VBlanks come from a clock the host advances. Headless runs (tests, replays) advance
  it exactly once per `VSync` wait; the window paces it in real time. CD and transfer
  completions are scheduled on that clock, never on wall time.
- **The picture:** `backend::gpu` executes the packets into VRAM; headless runs write the
  display area as PNG, the window shows it (SDL3, 1x for now; the `render` crate is Phase 8).
- **Fixed PS1 addresses** (`GENERAL_BUFFER` at 0x80010000, assets inside overlay images, the
  overlays' load addresses): the decomp names them through `DW1_PS1_ADDR(type, addr)`
  (`dw/types.h`; a plain cast on the PS1), which natively points into a 2 MB static stand-in
  for PS1 RAM (`psyq::memory`) at the same offset. Overlay files load there; their code is
  linked into the program. The scratchpad is a 1 KB static (native `libetc.h`).
- **32-bit pointers inside loaded data** (GsMapModelingData rewrites a TMD's offsets as
  addresses): `0x80000000 | offset` into the same data-segment window as the GPU links
  (`psyq::memory::ptr32_of`), so they fit and read like KSEG0 addresses. The game C reads
  such fields in about 35 places (TMD primitives in effects); those get `DW1_NATIVE` decodes
  with their scenes (Phase 4/5), together with a native `TMD_STRUCT` whose address fields are
  32-bit (Sony's declares pointers, which are 8 bytes natively).
- **Pointer-to-integer casts:** every cast clang flags as truncating a pointer is spelled
  `intptr_t`/`uintptr_t` in the fork (558 sites; the same types on the PS1, and the MIPS build
  is byte-identical). Integers that hold addresses (variables, parameters) are widened the same
  way when their code runs. The battle-effect interpreter's `int32_t` stack (`EFE_PUSH`, about
  1,500 sites over three copies) is its own design task with the battles (Phase 5).
- **The GTE at boot** starts from the registers the BIOS leaves when the game's `main()` runs
  (observed), since the game inherits them.
- **libgte** and libgs's camera are fitted against the real SDK functions run inside the
  emulator on chosen inputs (`oracle/call-sdk.lua`), e.g. `SquareRoot0(4000000)` is 1999,
  which is why the boot camera's matrix has 0x1002 on its diagonal.
- **Sound:** silent but stateful: VAB/SEQ handles, transfer completions, voice key state, so
  the game's waits finish. Real audio is Phase 6.

## Slices

1. **Prototype audit (done 2026-10-13, `docs/abi-audit-calls.md`; the open items are listed
   there).** 958 calls go through `()` declarations to 32 functions (Phase 1
   report). A gpt-6.1-sol read-only pass lists, per call, any argument whose type doesn't match
   the definition (int for a pointer, values out of range for `char`/`short` parameters,
   different struct pointers); Claude checks samples and fixes the real ones in the fork under
   `DW1_NATIVE`, or with prototypes where they're compatible. Same pass for the 113
   missing-return functions whose callers use the value.
2. **Heap and the first init calls (done 2026-10-13):** the allocator; `ResetCallback`, `SetGraphDebug`,
   `PadInit`, `SsInit` and other calls that only set state. Check: `dw1` gets past them.
3. **Disc (done 2026-10-13):** `pack` crate; libcd (`CdInit`, `CdSearchFile`, `CdRead`,
   `CdReadSync`,
   `CdControl`/`CdControlB`, `CdIntToPos`/`CdPosToInt`) with completions on the clock. Check:
   files the game reads match the extracted files byte for byte.
4. **GPU and libgs 2D (in progress: transfers, environments, setters, AddPrim, buffers,
   ordering tables and GsSortClear done; the 2D sorts wait for captures):** VRAM transfers (`LoadImage`, `StoreImage`, `MoveImage`,
   `ClearImage`, `DrawSync`), environments (`PutDrawEnv`, `PutDispEnv`, `SetDefDispEnv`,
   `SetDrawOffset`, `SetDispMask`), primitive setters and `AddPrim`, TIM parsing, and libgs's
   display buffers, ordering tables, sprites, box fills and clears. Check: packets against
   Redux captures of the landing screen (S0.7's method), then pixels.
5. **Clock, pad, frames (in progress: virtual VSync clock, replay host with caller checks,
   headless PNG output done; the window isn't):** `VSync`, `PadRead` from a replay file (S0.8's format) or the
   keyboard/gamepad, headless PNG output and the window.
6. **Silent sound (done 2026-10-13):** libsnd/libspu state.
6b. **Memory cards (done 2026-10-13):** libmcrd over 128 KB images, two fresh formatted cards
   by default (byte-identical to the emulator's), the async `MemCardSync` protocol.
6c. **libgte (in progress):** all 29 functions, fitted against `oracle/call-sdk.lua`.
7. **The intro movie:** MOV's streaming reads (`St*`), the MDEC calls (`DecDCT*`), XA
   skipped silently. Check: it plays to the end and skips on Start.
8. **The gate:** the landing screen at 1x against Redux's frame; the `new-game` replay from
   boot to the main menu with no trap and the S0.8 checkpoints on the way equal. Re-entry: back
   to the landing screen and through again.
