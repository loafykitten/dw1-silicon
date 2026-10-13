/* Test-only stand-ins for the HD renderer and PGXP hooks psxgpu.c calls. HD is off, so none of
 * the drawing hooks runs; the ones reached with HD off do nothing. */
#include <stdint.h>

#include "backend/psxgpu_hd.h"
#include "backend/pgxp.h"

int PsxHd_On(void) { return 0; }
int PsxHd_Pgxp(void) { return 0; }
int PsxHd_SmoothActive(void) { return 0; }
void PsxHd_Sync(void) {}
void PsxHd_Register(int x, int y, int w, int h) { (void)x; (void)y; (void)w; (void)h; }
void PsxHd_Fill(int x, int y, int w, int h, uint16_t c) { (void)x; (void)y; (void)w; (void)h; (void)c; }
void PsxHd_Refresh(int x, int y, int w, int h) { (void)x; (void)y; (void)w; (void)h; }
void PsxHd_Triangle(const PsxGpuState *st, const PsxVtx *v0, const PsxVtx *v1, const PsxVtx *v2, int gouraud,
                    const PsxTex *t, int semi, int abr, int precise) {
    (void)st; (void)v0; (void)v1; (void)v2; (void)gouraud; (void)t; (void)semi; (void)abr; (void)precise;
}
void PsxHd_Rect(const PsxGpuState *st, int x0, int y0, int w, int h, const PsxTex *t, int u0, int v0, int r, int g,
                int b, int semi, int abr) {
    (void)st; (void)x0; (void)y0; (void)w; (void)h; (void)t; (void)u0; (void)v0; (void)r; (void)g; (void)b;
    (void)semi; (void)abr;
}
void PsxHd_Line(const PsxGpuState *st, const PsxVtx *a, const PsxVtx *b, int gouraud, int semi, int abr) {
    (void)st; (void)a; (void)b; (void)gouraud; (void)semi; (void)abr;
}
int PsxHd_ReadDisplay(int x, int y, int w, int h, uint32_t *out, int *ow, int *oh) {
    (void)x; (void)y; (void)w; (void)h; (void)out; (void)ow; (void)oh; return 0;
}
int PsxHd_DisplayTexture(int x, int y, int w, int h, void **tex, int *tw, int *th, int *sx, int *sy, int *sw,
                         int *sh) {
    (void)x; (void)y; (void)w; (void)h; (void)tex; (void)tw; (void)th; (void)sx; (void)sy; (void)sw; (void)sh;
    return 0;
}
int Pgxp_LookupPoly(int n, const int *x, const int *y, float *fx, float *fy, float *fz, int *src) {
    (void)n; (void)x; (void)y; (void)fx; (void)fy; (void)fz; (void)src; return 0;
}
