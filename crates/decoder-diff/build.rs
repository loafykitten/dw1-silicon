//! Compiles DW2-PC's `backend/xadec.c`, `mdec.c` and `psxspu.c` (gitignored under `ref/`) and
//! sets `cfg(have_dw2_c)`. Without the sources nothing is built and the tests skip.

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
        build.file(p);
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
