# S0.3: the decomp's GCC build on this Mac

2026-10-13. dw_decomp 09c174c (the `decomp/` submodule, fork `loafykitten/dw_decomp`).

## Steps

```
git submodule update --init decomp
git -C decomp submodule update --init external/psyq_headers     # the only nested one needed
mkdir -p decomp/disks/us
dumpsxiso -x decomp/disks/us -s decomp/disks/us/us.xml "disc/Digimon World (USA).bin"
cd decomp && docker run --rm --platform linux/amd64 -v "$PWD":/dw -v dw_venv:/dw/.venv \
    dw-build sh -c 'make -j8 regenerate && make -j8 TOOLCHAIN=gcc'
```

(`dw-build` is the decomp's own Dockerfile; see `docs/toolchain.md`.) The disc image's SHA-1
is unchanged after extraction. Everything generated (`disks/`, `asm/`, `assets/`, `build/`)
is covered by the decomp's `.gitignore`; `git -C decomp status` stays clean.

## Result

Passes. Under Rosetta it takes a few minutes. `decomp/build/us/psx/gcc/` holds
`SLUS_010.32` (PS-X EXE), `SLUS_010.32.elf` with its `.map`, and the 15 overlay binaries
(BTL, DGET, DOO2, DOOA, EAB, ENDI, EVL, FISH, KAR, MOV, MURD, STD, TRN, TRN2, VS; SHOP is
in main). Nobody runs these: it's a compile/link check only.

837 warning lines. The kinds that matter for a native clang build:

| Count | Warning | Why it matters |
|---:|---|---|
| 113 | control reaches end of non-void function | UB in C if the caller uses the value; clang may optimize on it. Each needs a look. |
| 48 | may be used uninitialized | Same: the PS1 got whatever was in the register. |
| 31 | cast between incompatible function types | The callback signature mismatches from the survey. |
| 49 | signed/unsigned comparison | Usually harmless; check where sizes or `long` widths change. |

The rest are unused variables/parameters, fallthrough and missing braces. There's also one
assembler warning about `VHB_EXT`'s symbol size in generated data.

## What it gives the port

- The extracted executable and overlays that the native build will need data slices from
  (S0.5 report C), in a gitignored place.
- The generated SDK assembly in `decomp/asm/us/` (gitignored): where the SDK `rand()`,
  libgs internals and the GTE commands they use can now be read (survey items left
  unverified).
- The ELF and `.map` give real symbol addresses and sizes for the oracle's state dumps.
- A regression check for `DW1_NATIVE` changes: this build must keep passing with them off.
