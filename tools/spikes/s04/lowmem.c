#include <stdio.h>
#include <stdint.h>
#include <sys/mman.h>
#include <mach-o/getsect.h>
#include <mach-o/ldsyms.h>
#include <dlfcn.h>

static void try(const char *what, void *hint, int flags) {
    void *p = mmap(hint, 2u << 20, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON | flags, -1, 0);
    if (p == MAP_FAILED) { printf("%-34s hint=%p -> FAILED\n", what, hint); return; }
    *(volatile uint32_t *)p = 0xdeadbeef;
    printf("%-34s hint=%p -> %p %s\n", what, hint, p,
           (uintptr_t)p < 0x80000000u ? "(<2GB)" : (uintptr_t)p < 0x100000000ull ? "(<4GB)" : "(high)");
}

int main(void) {
    Dl_info di; dladdr((void *)main, &di);
    printf("main image base %p\n", di.dli_fbase);
    try("hint, no fixed", (void *)0x10000000, 0);
    try("MAP_FIXED 0x10000000", (void *)0x10000000, MAP_FIXED);
    try("MAP_FIXED 0x80000000", (void *)0x80000000, MAP_FIXED);
    try("MAP_FIXED 0x00100000", (void *)0x00100000, MAP_FIXED);
    return 0;
}
