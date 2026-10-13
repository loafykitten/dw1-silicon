# S0.4: memory representation

2026-10-13, macOS 27.0, Apple clang 21.0.0, this Mac. Spike sources and a runner are in
`tools/spikes/s04/` (`run.sh`).

## Measured

| Test | Result |
|---|---|
| `mmap` with a hint below 4 GB (0x10000000), no `MAP_FIXED` | hint ignored; mapped at 0x1_0007_C000 |
| `MAP_FIXED` at 0x100000, 0x10000000, 0x80000000 | fails |
| link with `-pagezero_size 0x100000` (with or without `-image_base`, `-no_pie`) | killed at exec (SIGKILL, exit 137) before `main` |
| executable base under ASLR, 200 runs | always 0x1_0009_8000 to 0x1_04FF_4000 (~80 MB slide); globals in the same range |
| `MAP_FIXED` 64 MB at 0x1_8000_0000, 200 runs | succeeds every time |

So DW2-PC's layout (executable, globals, PS1 RAM and heap in a 16 MB window below 2 GB)
is impossible for an ordinary arm64 macOS process: the kernel enforces the 4 GB page zero.
There is no ILP32 mode on macOS either (arm64_32 is watchOS only). Native pointers are 8
bytes, full stop.

## Design, per kind of reference

The survey (`docs/survey.md`, report B) counts the sites. Each kind gets its own handling:

1. **Native objects keep native pointers.** Structs grow. That's fine wherever nothing reads
   them as bytes, sizes them with literals, or stores them in 32 bits. The survey lists the
   exceptions (literal sizes and strides: `battle_effect.c:2133` 0x280 for `EfeSlot`s that
   are 0x500 natively, the 136-byte position stride, `fish.c:3057`, `map.c:4405`...). Each
   becomes `sizeof`/`offsetof`, under `DW1_NATIVE`.
2. **Disk and save data stays in its original byte layout**, read through fixed-width
   types. The SDK's `long`/`u_long` become 32-bit in our headers (DW2-PC's `psyq_u_long`
   idea), and the game's own `long *` reads of file data (MMD offsets `model.c:3415`, TFS
   `map.c:4234`, TIM headers) get `int32_t`/`uint32_t`. The save payload has no pointers;
   it's a fixed record with asserted offsets and size (0xF00, checksums over 0x4E4 and
   0xA00 bytes).
3. **Pointers stored in 32-bit integers are widened, not windowed.** The effect interpreter
   (`EFE_PUSH`, its 32-slot operand and 16-slot call stacks, integer-address variables) and
   the scattered `(int32_t)ptr` arithmetic:
   - stacks and integer-address variables become `intptr_t` (a few header lines cover the
     353 `EFE_PUSH`/`EFE_POP` sites);
   - arithmetic-only casts, `(char *)((int32_t)p + n)`, become `intptr_t` casts
     mechanically (ast-grep), ~380 lines;
   - the ~70 sites that store an address into a 32-bit *field* get the field widened or, if
     the field lives in loaded data, go to item 5.
   Why not the window alone: a plain C cast from `int32_t` back to a pointer can't restore
   the high half, so a window only helps if every round trip is rewritten anyway, and then
   widening is simpler.
4. **Fixed PS1 addresses** (22 macros: `GENERAL_BUFFER` 0x80010000, `TEXTURE_BUFFER`,
   overlay model buffers, ordering tables; `include/dw/main.h:8`) point into a **PS1 RAM
   arena**: one 2 MB block, with a `PS1_ADDR(a)` macro turning 0x80xxxxxx into
   `arena + (a & 0x1FFFFF)` with a bounds check. Overlays that the game loads to fixed
   addresses and buffers it uses by address live there too, which keeps overlap behaviour
   (e.g. STD overwriting DOOA's first 0x2E0 bytes) for free. The scratchpad
   (`getScratchAddr`, 1,117 uses) is a separate 1 KB native block.
5. **In-place pointer writes into loaded buffers.** Two systems write addresses into file
   bytes:
   - **TMD mapping** (`GsMapModelingData`, 35 sites): the Rust libgs keeps a native side
     table per mapped model instead of rewriting the 28-byte object records; the game's
     own reads of `primtop` (`graphics.c:170-189`) go through an accessor.
   - **EFE files** (seven-pointer `EfeSubEffect` records, UV-anim pointers, writes at
     `battle_effect.c:4618-4748`): a native mirror per loaded EFE slot holds the pointer
     fields; the file bytes keep their original layout.
   If either turns out to need real 32-bit values inside the buffer, the **4 GB window** is
   the fallback: allocate those objects in a fixed arena at 0x1_8000_0000 and store
   `(uint32_t)(p - window_base)` with explicit encode/decode macros. Measured to work; not
   needed until proven otherwise.
6. **GPU packet links** (24-bit ordering-table links): the Rust libgpu/libgs own every
   packet buffer and ordering table (they're allocated by the SDK work-base calls, not by
   game structs), so links are offsets from the packet arena's base. The game C only
   writes primitive fields through the `set*` macros.
7. **Callbacks and `setjmp`/`longjmp`:** host `jmp_buf`; the script interpreter's
   `longjmp` before `setjmp` (`script_interp.c:520` vs `:601`) and its re-entry cycle
   get a native review in Phase 2. Function-pointer mismatches (31 casts) get fixed
   signatures.

## Still open

- Whether any 32-bit-in-loaded-data case survives items 3 and 5 (decides whether the
  window is built at all). Known by Phase 2 when EFE and TMD run.
- Whether the fixed mapping holds in the hardened, signed `.app` (only matters if the
  window is needed).
- Exact SDK `u_long` uses inside structs the game shares with the SDK, now readable in the
  generated assembly (`decomp/asm/us/`) and the headers (`decomp/external/psyq_headers`).
