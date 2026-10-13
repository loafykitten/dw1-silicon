/*
 * Native wrapper for Psy-Q's <libetc.h>: the scratchpad (1 KB of fast RAM at 0x1F800000 on
 * the PS1) is a native buffer, crates/psyq/src/memory.rs.
 */
#ifndef DW1_NATIVE_LIBETC_H
#define DW1_NATIVE_LIBETC_H

#include_next <libetc.h>

extern uint8_t dw1_scratchpad[];

#undef getScratchAddr
#define getScratchAddr(offset) ((u_long *)(dw1_scratchpad + (offset) * 4))

#endif
