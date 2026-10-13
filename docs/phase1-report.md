# Phase 1: the game C compiles and links natively

2026-10-13. Phase 1 asked for all of main's C plus the Rust `psyq` crate with trapping stubs to
build and link for arm64, every overlay's C to compile, and the decomp's GCC build to keep
building with `DW1_NATIVE` off.

**Result: done, and a little further.** All 119 game C files of the US build (main, SHOP and
all 15 overlays; everything in `decomp/mk/version/us.mk` except CodeWarrior's startup) compile
with Apple clang and link into one program, `dw1`. Nothing is left unresolved: every SDK call
the game makes lands in `psyq`, implemented or trapping. Running `dw1` starts the game's own
`main()`, which gets as far as its first SDK call, `InitHeap3`, and stops there with that name.
That's where Phase 2 starts.

## How it builds

`crates/decomp-c/build.rs` does everything; there's no second build system.

- **File lists** come from the decomp's own `us.mk`, so the native build follows upstream.
- **Headers:** `crates/decomp-c/include/` comes first on the include path and holds the native
  pieces; then the decomp's `include/`, then Psy-Q's headers (from the decomp's nested
  submodule, never copied here), then the generated text headers.
- **Text:** the decomp keeps game text in `text/**/*.yaml`; its generator (`tools/gen_text.py`,
  run with `.venv/bin/python` or `DW1_PYTHON`) writes the headers into Cargo's `OUT_DIR`.
- **Shift-JIS literals:** the PS1 build compiles with `-fexec-charset=CP932`, which clang
  doesn't support, and clang would silently store UTF-8 instead. Nine files have Japanese or
  full-width characters inside string literals; the build copies them into `OUT_DIR` with each
  such character written as its CP932 bytes (`#line` keeps errors pointing at the original).
  The bytes come from Python's `cp932` codec, the same one the decomp's text generator uses
  (Rust's Shift-JIS tables follow WHATWG, which differs from Microsoft's CP932 for about 2,300
  characters). The lexer (`crates/decomp-c/src/literals.rs`, unit-tested) handles comments,
  escapes, `\u` names and backslash-newline splices. Check: all 58 affected strings in main's
  data are byte-identical to the original executable (`SLUS_010.32`) at the addresses in their
  symbol names.
- **Linking:** the whole archive is linked (not just what `main()` reaches), so any missing
  symbol fails the build. A build check also fails if the game C would call a macOS libc
  function: those have different semantics (see "libc" below), and nothing else would catch
  a silent binding.

## The native header (`dw1_native.h`)

Force-included before every file. It keeps the PS1 meaning of the code:

- **`long` is 32-bit** (`#define long int`). On the PS1 every `long` is 32 bits: `u_long`,
  `VECTOR`, `MATRIX.t`, GPU packet words, SDK prototypes, and the game's own `long *` reads of
  file data. Making it 32-bit everywhere fixes all 521 lines S0.5 counted at once, and the
  SDK structs keep their PS1 layouts (asserted: `VECTOR` 16 bytes, `MATRIX` 32 with `t` at 20,
  `_GsFCALL` 76 pointers). The host's `<stdint.h>` is read first, so `intptr_t` and friends
  stay 64-bit for the pointer-widening work. Costs: `long long` and `long int` can't be
  spelled, which needed two small edits in the decomp (below); `L`-suffixed literals stay
  64-bit, which only widens a few comparisons, all sign-correct.
- **`NULL` is `((void *)0)`.** Psy-Q defines it as a plain `0`, a 32-bit int, which arm64
  doesn't widen when it's passed through `...` or to an unprototyped function.
- **libc renamed:** `memcpy`, `strcpy`, `sprintf` and the rest become `dw1_*`, implemented in
  `crates/psyq/src/libc.rs`. On the PS1 all but `sprintf` are BIOS calls, so they follow
  psx-spx's BIOS descriptions, NULL refusals included; `sprintf` covers the conversions the
  game uses (and the standard flags), tested with every format string in the game.
- **`main` → `dw1_game_main`**, so the Rust app owns the process entry point.

Native replacements for three PS1-only headers sit next to it:
- `inline_n.h`: the 11 GTE macros the game uses (`gte_rtps`, `gte_ldv0`, `gte_stsxy3`...),
  each doing what its MIPS COP2 sequence does, through one shared Rust GTE
  (`crates/psyq/src/gte.rs` over `backend::gte`). An unmodelled GTE command stops the program.
- `setjmp.h`: the host's `_setjmp`/`_longjmp` and buffer (Psy-Q's holds 12 MIPS registers).
- `__rts_info_t__.h`: the linker symbols `initializeHeap()` reads, without ELF sections.

## The `psyq` crate now

- `stubs.rs`: 187 trapping stubs, grouped by library (libapi, libetc, libgpu, libgte, libgs
  with its 27 TMD handlers, libcd with streaming, libpress, libspu, libsnd, libmcrd). Each
  prints its name and aborts. Implementing one means deleting its line.
- `globals.rs`: the SDK data the game links against (`GsIDMATRIX`, `GsWSMATRIX`,
  `GsLIGHTWSMATRIX`, `GsFCALL4`, `StCdIntrFlag`) and the three linker symbols. Layouts are
  asserted against the C headers through bindgen.
- `gte.rs`, `libc.rs`: as above.

The overlay load addresses (`btl_START`...) are empty arrays in `crates/decomp-c` until the
overlay-lifetime work (Phase 2/5); the file layer that would write through them traps first.

## Decomp edits (fork `loafykitten/dw_decomp`, branch `dw1-native`)

Both keep the decomp's own builds unchanged; its GCC build passes with them (container, as in
S0.3).
- `src/main/utils.c`: one `long long` local becomes `int64_t` under `DW1_NATIVE`.
- `src/std/std_bss_*.c`, `src/vs/vs_bss_*.c`: `(long unsigned int *)` casts respelled
  `(unsigned long *)`, the same type.

## Warnings: the triage

`DW1_WARNINGS=1 cargo build -p decomp-c` prints them. About 3,570 in total; by kind:

| Kind | Count | What it means here |
|---|---|---|
| pointer ↔ int casts (`-Wint-to-pointer-cast`, `-Wpointer-to-int-cast` and the `void *` variants) | 1,734 | **The real 64-bit work.** 1,527 are in the three copies of the battle-effect interpreter (`battle_effect.c`, `std_effect.c`, `vs_effect.c`, 509 each), the `EFE_PUSH` pattern S0.4/S0.5 planned for. The other ~200: `evl.c` 49, `doo2.c` 29, `script_interp.c` 21, `dooa.c` 17, `model.c` 12, `effect_common.c` 9, `sound_async.c` 8, and a tail of 1-6 per file. Each gets fixed when its scene runs (Phase 2+), with the memory design in `docs/memory.md`. |
| unprototyped declarations and calls (`-Wdeprecated-non-prototype`) | 1,221 | 233 K&R definitions (harmless) and 958 calls through `()` declarations to 32 functions (`startAnimation` 224, `drawString` 208, `removeObject` 184, `renderString` 72...). On arm64 such a call is only wrong if an argument doesn't match the definition's type (an int where a pointer is expected, or a value out of range for a `char`/`short` parameter). S0.5 found about 15 real mismatches; they need an audit before the scenes that use them (Phase 2). |
| non-void function without a return (`-Wreturn-type`) | 113 | The PS1 returned whatever was in `v0`; natively it's whatever is in `x0`. Harmless unless a caller uses the value; audit with the prototype work. |
| function-pointer casts with a different signature (`-Wcast-function-type-mismatch`) | 38 | Tick/render callbacks taking `short`/`char` called through `void (*)(int)`, and `item.c`'s item table. Same argument rules as above: fine for in-range values. |
| maybe-uninitialized, unsequenced, overlong string initializer, char subscripts, always-true comparisons | 33 | Real but small: undefined behaviour the original compiler happened to tolerate. Listed by `DW1_WARNINGS=1`; fixed when their code runs, or when a replay diverges there. |
| unused variables/labels/functions, sign compares, missing braces, self-assign | ~430 | Style noise from matching-decomp code. Ignored. |

## Review

gpt-6.1-sol (high) reviewed it three times. Fixed from its findings: `%.Ns` reading past an
unterminated source; build-script environment tracking lost when taking over cc's link line;
the literal encoder missing line splices and `\u` escapes, living in a directory `.gitignore`
hid, and using WHATWG Shift-JIS instead of CP932; the Psy-Q headers not watched for rebuilds.
Left as test gaps, covered once Phase 2 runs the game against the emulator: C-driven tests of
the GTE macros, the variadic libc calls and `setjmp`/`longjmp` through the native headers.

## What Phase 2 starts with

Running `dw1` stops at `InitHeap3`. Next in order: the heap (`InitHeap3`/`malloc3`/`free3` on a
native arena, and `initializeHeap()` under `DW1_NATIVE`), then the calls `main()` makes on the
way to the landing screen: callbacks and VSync, the GPU and libgs setup, CD and the pack, pad
input, and the intro movie. The unprototyped-call audit (958 calls, 32 functions) goes first,
since a wrong pointer argument there would look like a much stranger bug later.
