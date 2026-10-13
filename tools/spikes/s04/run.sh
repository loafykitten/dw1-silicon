#!/bin/sh
# S0.4 memory spikes: can arm64 macOS map memory below 4 GB, and does a fixed
# window at 0x1_8000_0000 map next to the executable? See docs/memory.md.
set -eu
out=${TMPDIR:-/tmp}/dw1-s04
mkdir -p "$out"
here=$(cd "$(dirname "$0")" && pwd)
clang -O1 -o "$out/lowmem_default" "$here/lowmem.c"
clang -O1 -Wl,-pagezero_size,0x100000 -o "$out/lowmem_pz" "$here/lowmem.c"
clang -O1 -o "$out/window" "$here/window.c"
echo "== default page zero"; "$out/lowmem_default"
echo "== 1 MB page zero (expect SIGKILL at exec)"; "$out/lowmem_pz" || echo "exit $?"
echo "== fixed window, 5 runs"; for _ in 1 2 3 4 5; do "$out/window"; done
