/*
 * Force-included before every decomp C file in the native build (-include dw1_native.h).
 *
 * Renames the PS1 runtime functions whose names clash with the host libc, so the game C
 * binds to the Rust implementations in crates/psyq instead of macOS's.
 */
#ifndef DW1_NATIVE_H
#define DW1_NATIVE_H

#define DW1_NATIVE 1

/* BIOS A(2Fh)/A(30h), reached through the SDK's libc stubs on the PS1. */
#define rand dw1_bios_rand
#define srand dw1_bios_srand

#endif
