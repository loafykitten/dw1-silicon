/*
 * Native wrapper for Psy-Q's <libgpu.h>: primitive links.
 *
 * A primitive's tag and an ordering-table entry hold a 24-bit address on the PS1. Natively
 * it's a 24-bit offset into the app's data segment (crates/psyq/src/memory.rs), so the
 * macros that store or follow a link go through psyq. setaddr() stores a pointer as a link
 * and an integer (another tag's raw link, or termPrim's 0xffffffff) as it is.
 */
#ifndef DW1_NATIVE_LIBGPU_H
#define DW1_NATIVE_LIBGPU_H

#include_next <libgpu.h>

uint32_t dw1_gpu_link(const void *p);
void *dw1_gpu_unlink(uint32_t link);

/* __builtin_classify_type: 5 is the pointer class (arrays decay to it). */
#define DW1_GPU_ADDR(a) \
	(__builtin_classify_type(a) == 5 ? dw1_gpu_link((const void *)(uintptr_t)(a)) \
					 : (uint32_t)(uintptr_t)(a))

#undef setaddr
#define setaddr(p, _addr) (((P_TAG *)(p))->addr = DW1_GPU_ADDR(_addr))
#undef nextPrim
#define nextPrim(p) dw1_gpu_unlink(((P_TAG *)(p))->addr)

#endif
