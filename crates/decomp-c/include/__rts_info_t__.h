/*
 * Native replacement for CodeWarrior's <__rts_info_t__.h>, the linker-generated layout
 * symbols. The native build has no PS1 linker script; only the symbols the game C reads are
 * declared, and crates/psyq defines them.
 *
 * main.c's initializeHeap() turns them into the PS1 heap's address and size. Natively that
 * heap is the psyq allocator's business (Phase 2); until then InitHeap3 traps.
 */
#ifndef DW1_NATIVE_RTS_INFO_H
#define DW1_NATIVE_RTS_INFO_H

extern unsigned long _end;        /* first address after .bss */
extern unsigned long _stack_addr; /* initial stack pointer */
extern unsigned long _stack_size; /* stack size in KB */

#endif
