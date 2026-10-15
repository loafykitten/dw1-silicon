//! Compiles DW2-PC's `backend/xadec.c`, `mdec.c` and `psxspu.c` (gitignored under `ref/`) and
//! sets `cfg(have_dw2_c)`. Without the sources nothing is built and the tests skip.
//!
//! `mdec.c` is compiled with its colour conversion changed to `backend::mdec`'s (products
//! rounded to the nearest, sums cut to 9 bits before saturating: measured against the
//! emulator, docs/phase2-movie.md), so everything else in the MDEC still compares bit for bit.

use std::path::Path;

const FILES: [&str; 3] = ["backend/xadec.c", "backend/mdec.c", "backend/psxspu.c"];

fn main() {
    println!("cargo::rustc-check-cfg=cfg(have_dw2_c)");
    println!("cargo::rerun-if-changed=build.rs");
    // DW2PC_REF_DIR overrides the location (point it at an empty directory to see the skip).
    println!("cargo::rerun-if-env-changed=DW2PC_REF_DIR");
    let root = match std::env::var_os("DW2PC_REF_DIR") {
        Some(d) => Path::new(&d).to_path_buf(),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ref/DW2-PC"),
    };
    let mut build = cc::Build::new();
    let mut all = true;
    for f in FILES {
        let p = root.join(f);
        println!("cargo::rerun-if-changed={}", p.display());
        all &= p.exists();
        if f == "backend/mdec.c" && p.exists() {
            build.file(colour_patched(&p));
        } else {
            build.file(p);
        }
    }
    if !all {
        println!(
            "cargo::warning=ref/DW2-PC/backend/{{xadec,mdec,psxspu}}.c not found: decoder differential tests will skip"
        );
        return;
    }
    // psxspu.c includes psxspu_gauss.inc and every file includes "backend/*.h".
    println!(
        "cargo::rerun-if-changed={}",
        root.join("backend/psxspu_gauss.inc").display()
    );
    build
        .include(&root)
        .warnings(false)
        .flag("-fno-common")
        .compile("dw2_decoders");
    println!("cargo::rustc-cfg=have_dw2_c");
}

/// `mdec.c` with the port's colour conversion, written to OUT_DIR (it includes
/// "backend/mdec.h" through the include path, so it compiles from there).
fn colour_patched(src: &Path) -> std::path::PathBuf {
    let c = std::fs::read_to_string(src).expect("read mdec.c");
    let swaps = [
        (
            "r = clamp(lum + ((cr[c] * 1436) >> 10), -128, 127) + 128;",
            "r = sat9(lum + ((cr[c] * 1436 + 512) >> 10));",
        ),
        (
            "g = clamp(lum - ((cb[c] * 352 + cr[c] * 731) >> 10), -128, 127) + 128;",
            "g = sat9(lum - ((cb[c] * 352 + cr[c] * 731 + 512) >> 10));",
        ),
        (
            "b = clamp(lum + ((cb[c] * 1815) >> 10), -128, 127) + 128;",
            "b = sat9(lum + ((cb[c] * 1815 + 512) >> 10));",
        ),
        (
            "static int clamp(int v, int lo, int hi) {",
            "static int sat9(int v);\nstatic int clamp(int v, int lo, int hi) {",
        ),
    ];
    let mut out = c;
    for (from, to) in swaps {
        assert!(out.contains(from), "mdec.c no longer has `{from}`");
        out = out.replacen(from, to, 1);
    }
    out.push_str(
        "\nstatic int sat9(int v) { v = ((v & 0x1FF) ^ 0x100) - 0x100; \
         return clamp(v, -128, 127) + 128; }\n",
    );
    let dst = Path::new(&std::env::var("OUT_DIR").unwrap()).join("mdec_port_colour.c");
    std::fs::write(&dst, out).expect("write the patched mdec.c");
    dst
}
