//! Compiles DW2-PC's `backend/psxgpu.c` (gitignored under `ref/`) with stubs for the HD and
//! PGXP hooks it calls, and sets `cfg(have_dw2_c)`. Without the source, nothing is built and
//! the tests skip.

use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(have_dw2_c)");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=c/stubs.c");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ref/DW2-PC");
    let src = root.join("backend/psxgpu.c");
    println!("cargo::rerun-if-changed={}", src.display());
    if !src.exists() {
        println!(
            "cargo::warning=ref/DW2-PC/backend/psxgpu.c not found: GPU differential tests will skip"
        );
        return;
    }
    cc::Build::new()
        .file(&src)
        .file("c/stubs.c")
        .include(&root)
        .warnings(false)
        .compile("dw2_psxgpu");
    println!("cargo::rustc-cfg=have_dw2_c");
}
