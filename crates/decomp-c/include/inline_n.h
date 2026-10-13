/*
 * Native replacement for the GTE inline-assembly macros (<inline_n.h>, MIPS COP2 code in the
 * decomp's include/gcc/).
 *
 * Each macro does what its COP2 instruction sequence does, through crates/psyq's GTE: `lwc2`
 * and `swc2` become a 32-bit load or store plus mtc2/mfc2, `ctc2`/`cfc2` move control
 * registers, and `cop2 <word>` runs the command word on the model. Register numbers are the
 * hardware's (psx-spx, "Geometry Transformation Engine (GTE)").
 *
 * Only the macros the game C uses are defined; any other is an implicit-declaration error, so
 * a new use can't slip through unmodelled.
 */
#ifndef DW1_NATIVE_INLINE_N_H
#define DW1_NATIVE_INLINE_N_H

#include <stdint.h>

/* crates/psyq/src/gte.rs */
void dw1_gte_mtc2(uint32_t reg, uint32_t value);
uint32_t dw1_gte_mfc2(uint32_t reg);
void dw1_gte_ctc2(uint32_t reg, uint32_t value);
uint32_t dw1_gte_cfc2(uint32_t reg);
void dw1_gte_cop2(uint32_t command);

/* Word at byte offset `off` from `p`, as lw/lwc2 read it. */
#define DW1_GTE_WORD(p, off) (*(const uint32_t *)((const char *)(p) + (off)))
#define DW1_GTE_STORE(p, value) (*(uint32_t *)(p) = (value))

/* Vertex loads: VXY0/VZ0 (data registers 0 and 1) and the next two pairs. */
#define gte_ldv0(r0)                                  \
	do {                                          \
		dw1_gte_mtc2(0, DW1_GTE_WORD(r0, 0)); \
		dw1_gte_mtc2(1, DW1_GTE_WORD(r0, 4)); \
	} while (0)

#define gte_ldv3(r0, r1, r2)                          \
	do {                                          \
		dw1_gte_mtc2(0, DW1_GTE_WORD(r0, 0)); \
		dw1_gte_mtc2(1, DW1_GTE_WORD(r0, 4)); \
		dw1_gte_mtc2(2, DW1_GTE_WORD(r1, 0)); \
		dw1_gte_mtc2(3, DW1_GTE_WORD(r1, 4)); \
		dw1_gte_mtc2(4, DW1_GTE_WORD(r2, 0)); \
		dw1_gte_mtc2(5, DW1_GTE_WORD(r2, 4)); \
	} while (0)

/* Leading-zero count: LZCS (data register 30) in, LZCR (31) out. */
#define gte_ldlzc(r0) dw1_gte_mtc2(30, (uint32_t)(r0))
#define gte_stlzc(r0) DW1_GTE_STORE(r0, dw1_gte_mfc2(31))

/* Commands, with the same COP2 instruction words as the PS1 macros. */
#define gte_rtps() dw1_gte_cop2(0x0180001) /* RTPS, sf=1 */
#define gte_rtpt() dw1_gte_cop2(0x0280030) /* RTPT, sf=1 */

/* Screen XY stores: SXY0..SXY2 are data registers 12..14. */
#define gte_stsxy(r0) DW1_GTE_STORE(r0, dw1_gte_mfc2(14))

#define gte_stsxy3(r0, r1, r2)                      \
	do {                                        \
		DW1_GTE_STORE(r0, dw1_gte_mfc2(12)); \
		DW1_GTE_STORE(r1, dw1_gte_mfc2(13)); \
		DW1_GTE_STORE(r2, dw1_gte_mfc2(14)); \
	} while (0)

/* SZ3 (data register 19) shifted right by two, as an arithmetic word. */
#define gte_stszotz(r0) (*(int32_t *)(r0) = (int32_t)dw1_gte_mfc2(19) >> 2)

/*
 * A MATRIX's rotation part is five words (control registers 0..4, the last holding m[2][2]
 * and the padding) and its translation three (5..7) at byte 20. With `long` at 32 bits
 * (dw1_native.h) the native MATRIX has the PS1 layout.
 */
#define gte_SetRotMatrix(r0)                           \
	do {                                           \
		dw1_gte_ctc2(0, DW1_GTE_WORD(r0, 0));  \
		dw1_gte_ctc2(1, DW1_GTE_WORD(r0, 4));  \
		dw1_gte_ctc2(2, DW1_GTE_WORD(r0, 8));  \
		dw1_gte_ctc2(3, DW1_GTE_WORD(r0, 12)); \
		dw1_gte_ctc2(4, DW1_GTE_WORD(r0, 16)); \
	} while (0)

#define gte_SetTransMatrix(r0)                         \
	do {                                           \
		dw1_gte_ctc2(5, DW1_GTE_WORD(r0, 20)); \
		dw1_gte_ctc2(6, DW1_GTE_WORD(r0, 24)); \
		dw1_gte_ctc2(7, DW1_GTE_WORD(r0, 28)); \
	} while (0)

#endif
