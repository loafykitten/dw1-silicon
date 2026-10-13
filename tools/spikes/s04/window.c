#include <stdio.h>
#include <stdint.h>
#include <sys/mman.h>
#include <dlfcn.h>
static int g_data[16];
int main(void) {
    Dl_info di; dladdr((void *)main, &di);
    void *want = (void *)0x180000000ull;           /* same 4 GB-aligned region as the image */
    void *p = mmap(want, 64u << 20, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANON|MAP_FIXED, -1, 0);
    printf("image %p data %p fixed64MB@%p -> %p hi32 image=%llx data=%llx\n", di.dli_fbase, (void*)g_data, want, p,
           (unsigned long long)((uintptr_t)di.dli_fbase >> 32), (unsigned long long)((uintptr_t)g_data >> 32));
    return 0;
}
