# Toolchain (S0.1)

Recorded 2026-10-13 on the main Mac (macOS 27.0, Apple Silicon).

## Native

| Tool | Version | Source |
|------|---------|--------|
| Xcode / Apple clang | 27.0 / 21.0.0 (clang-2100.3.20.102) | Xcode |
| CMake | 4.4.4 | Homebrew `cmake` |
| Ninja | 1.13.2 | Homebrew `ninja` |
| SDL3 | 3.4.18 | Homebrew `sdl3` |
| GNU make | 4.4.1, as `gmake` | Homebrew `make` (the decomp needs 4.3+ for grouped targets; macOS `make` is 3.81) |
| MIPS binutils | GNU Binutils 2.47 (`mipsel-linux-gnu-*`) | Homebrew `mipsel-linux-gnu-binutils` |
| Python | 3.14.8, venv at `.venv/` (gitignored) with dw_decomp's `requirements.txt` (splat64 0.50.0, spimdisasm 1.42.4, rabbitizer 1.16.2). Since Phase 1 the native build needs it too: `crates/decomp-c/build.rs` runs the decomp's text generator (PyYAML) and Python's `cp932` codec with `.venv/bin/python`, or `DW1_PYTHON` | Homebrew `python@3.14` |
| dumpsxiso / mkpsxiso | 2.20 | built from source (Lameguy64/mkpsxiso tag v2.20), installed to `~/.local/bin` |
| PCSX-Redux | commit `26bfd693` (2026-10-12; not newer, see `docs/s08-oracle.md`) | built from source in `ref/pcsx-redux` (`make`); deps from its `install-brew-dependencies.sh`: capstone 5.0.9, curl 8.22.0, ffmpeg 9.0.2, freetype 2.14.3, libuv 1.53.0, pkgconf 3.0.8, zlib 1.3.2 |
| PCSX-Redux on booklab | the same 26bfd693 binary, copied | `~/Developer/dw1-oracle` on booklab (M1); its Homebrew capstone 5.0.9, freetype 2.14.3, ffmpeg 9.0.2, sdl3 3.4.18 (installed 2026-10-13), libuv, llhttp |
| Rust | rustc / cargo 1.99.0, rustup 1.29.1 | rustup (already installed) |
| Codex CLI | 0.162.1; `-m gpt-6.1-sol` answers | Homebrew |

Homebrew has no MIPS GCC, so `mipsel-linux-gnu-gcc` (needed by the decomp's
`make TOOLCHAIN=gcc`) comes from the container below. PCSX-Redux's own tap has a bare-metal
`mipsel-none-elf-gcc` that could be tried later as a native alternative.

## Decomp dev container

dw_decomp's `Dockerfile` (Ubuntu 24.04), built locally with OrbStack:

```
docker build --platform linux/amd64 --tag dw-build:latest .     # from the dw_decomp checkout
docker run --rm --platform linux/amd64 -v "$PWD":/dw -v dw_venv:/dw/.venv dw-build <cmd>
```

`linux/amd64` (Rosetta) because `pygfxd` has no arm64 Linux wheel and the image has no
native C compiler to build one. Inside: `mipsel-linux-gnu-gcc` 12.4.0, GNU Make 4.3,
Python 3.12.3, splat64 0.50.0.
