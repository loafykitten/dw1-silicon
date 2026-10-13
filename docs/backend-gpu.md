# Backend: the software GPU

`crates/backend/src/gpu.rs` (2026-10-13) is a Rust translation of DW2-PC's 1x software PS1
GPU (`backend/psxgpu.c`, CC0), without its HD and PGXP paths.

## How it's checked

`crates/gpu-diff` compiles DW2-PC's original C from the gitignored `ref/DW2-PC` and runs both
on pseudo-random GP0 streams (general, blend/mask-heavy, textured/window-heavy; 2,500 each,
env `GPU_DIFF_CASES`), comparing all of VRAM, every VRAM read-back and the display read-out
bit for bit: 7,500 cases, 0 mismatches. A mismatch is shrunk to a minimal op list. Without
`ref/DW2-PC` the test shows as ignored. Reviewed by GPT 6 Astra (high).

This proves the translation, not the hardware: DW2-PC's C has known differences from a real
PS1, which we keep for now.

## Differences from hardware to check against PCSX-Redux

Ranked by how likely DW1 hits them (Astra's review, plus the porter's list):

1. **Modulation before semi-transparency** (`gpu.rs` ~270): the foreground is clamped before
   blending; hardware (and Redux's non-dithered textured path) keeps larger modulated values
   until after. Example: texel 31, colour 160, background 0, ABR0 gives 15 here vs 19.
   DW1's fades go through this (`decomp/src/main/fade.c:157`). Highest priority.
2. **Polygon coverage and interpolation** (`gpu.rs` ~302, ~348): exact barycentric division
   with integer truncation vs the hardware's fixed-point edge walkers and gradients; no
   hardware vertex rounding. Check thin triangles, shared quad diagonals, UV boundaries and
   Gouraud gradients on real DW1 models.
3. **Transfers vs mask bits and overlap** (`gpu.rs` ~582, ~609): fill, upload and VRAM copy
   ignore E6 mask settings and the draw area; copies buffer rows. Hardware honours E6 for
   uploads/copies. DW1's animation code issues data-driven VRAM copies
   (`decomp/src/main/anim.c:331`).
4. Dithering: applied to textured modulated polygons only when E1 bit 9 is set, never to
   rectangles, raw textures or flat lines.
5. Size limits: polygons with a bounding box of 1024×512 or more, and lines spanning that,
   are dropped (hardware: 1023×511).
6. Polylines: the terminator test (`& 0xF000F000 == 0x50005000`) also matches in coordinate
   words of Gouraud polylines, and can end a polyline on an unlucky vertex.
7. Smaller ones: GP0 02 fill keeps the top 5 bits per channel; textured polygons rewrite E1
   page bits 0-8 and 11 only; quads take CLUT/page from vertices 0 and 1; lines ignore the
   page's blend mode unless semi-transparent; the mask bit comes from texel bit 15 even when
   not blended; GP0 C0 (VRAM to CPU) is ignored; drawing-to-display protection is ignored;
   24-bit display read wraps within a 2048-byte row.

S0.7 (the libgs/TMD spike) and Phase 2's title-screen comparison are where these get
measured: render the same frame in Redux and in the port and diff VRAM.
