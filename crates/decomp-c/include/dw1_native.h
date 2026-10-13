/*
 * Force-included before every decomp C file in the native build (-include dw1_native.h).
 *
 * The decomp C is PS1 code: ILP32, with Psy-Q's headers and libc. This header keeps that
 * meaning on arm64 macOS (LP64) and points the PS1 runtime calls at crates/psyq.
 *
 * The directory holding this file comes first on the include path, so the headers next to it
 * (inline_n.h, setjmp.h, __rts_info_t__.h) replace the PS1-only ones from the decomp and
 * Psy-Q.
 */
#ifndef DW1_NATIVE_H
#define DW1_NATIVE_H

#define DW1_NATIVE 1

/*
 * The host's fixed-width types, read before `long` is redefined below so that intptr_t,
 * uintptr_t, int64_t and friends keep their native widths. dw/types.h includes <stdint.h>
 * again later; its include guard makes that a no-op.
 */
#include <stdint.h>

/*
 * `long` is 32 bits on the PS1 and 64 on arm64 macOS. Every `long` in the game C and in
 * Psy-Q's headers (u_long, VECTOR, MATRIX.t, GPU packet words, SDK prototypes) means 32 bits,
 * so it's made 32 bits here (docs/phase1-report.md, "long"). `long long` isn't expressible
 * after this; the one game use is guarded by DW1_NATIVE in the decomp. Integer literals with
 * an L suffix stay 64-bit, which only widens comparisons (all sign-correct; checked in S0.5).
 */
#define long int

/*
 * Psy-Q and the decomp define NULL as a plain 0 (each behind #ifndef NULL). That's a 32-bit
 * int, which arm64 doesn't widen when it's passed through `...` or to an unprototyped
 * function expecting a pointer; a pointer constant is always the right width.
 */
#define NULL ((void *)0)

/* BIOS A(2Fh)/A(30h), reached through the SDK's libc stubs on the PS1. */
#define rand dw1_bios_rand
#define srand dw1_bios_srand

/*
 * The rest of the PS1 libc the game calls (crates/psyq/src/libc.rs), renamed so it can't bind
 * to macOS's: Psy-Q's prototypes take 32-bit sizes, and the BIOS versions refuse NULLs.
 */
#define abs dw1_abs
#define bzero dw1_bzero
#define exit dw1_exit
#define memcpy dw1_memcpy
#define memset dw1_memset
#define printf dw1_printf
#define sprintf dw1_sprintf
#define strcat dw1_strcat
#define strcmp dw1_strcmp
#define strcpy dw1_strcpy
#define strlen dw1_strlen
#define strncpy dw1_strncpy
#define strrchr dw1_strrchr

/* The game's main() (main.c), which the app calls; Rust's binary owns the C `main`. */
#define main dw1_game_main

#endif
