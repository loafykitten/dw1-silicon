/*
 * Native wrapper for Psy-Q's <libgs.h>.
 *
 * - A TMD object record (`struct TMD_STRUCT`) is file data: seven 32-bit words. Psy-Q
 *   declares its address fields as pointers, 8 bytes here, which would move every field and
 *   the object table after the 12-byte TMD header. Natively the record keeps the file's
 *   layout, and once GsMapModelingData has mapped it the address fields hold 32-bit encoded
 *   pointers (crates/psyq/src/memory.rs), read through DW1_TMD_ADDR (dw1_native.h).
 * - GsLinkObject4 takes the TMD's address as an `unsigned long`, 32 bits here; natively it
 *   goes to psyq at pointer width (the decomp's call sites cast to uintptr_t).
 */
#ifndef DW1_NATIVE_LIBGS_H
#define DW1_NATIVE_LIBGS_H

/* Psy-Q's own definition gets out of the way under another tag. */
#define TMD_STRUCT dw1_psyq_TMD_STRUCT
#include_next <libgs.h>
#undef TMD_STRUCT

struct TMD_STRUCT {
	uint32_t vertop; /* encoded pointer once mapped */
	uint32_t vern;
	uint32_t nortop; /* encoded pointer once mapped */
	uint32_t norn;
	uint32_t primtop; /* encoded pointer once mapped */
	uint32_t primn;
	uint32_t scale;
};

void dw1_GsLinkObject4(uintptr_t tmd_base, GsDOBJ2 *objp, int n);
#define GsLinkObject4(tmd_base, objp, n) dw1_GsLinkObject4((uintptr_t)(tmd_base), (objp), (n))

#endif
