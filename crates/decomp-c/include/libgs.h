/*
 * Native wrapper for Psy-Q's <libgs.h>.
 *
 * GsLinkObject4 takes the TMD's address as an `unsigned long`, 32 bits here; natively it
 * goes to psyq at pointer width (the decomp's call sites cast to uintptr_t).
 */
#ifndef DW1_NATIVE_LIBGS_H
#define DW1_NATIVE_LIBGS_H

#include_next <libgs.h>

void dw1_GsLinkObject4(uintptr_t tmd_base, GsDOBJ2 *objp, int n);
#define GsLinkObject4(tmd_base, objp, n) dw1_GsLinkObject4((uintptr_t)(tmd_base), (objp), (n))

#endif
