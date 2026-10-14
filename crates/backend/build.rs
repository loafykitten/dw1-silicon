//! Sets `cfg(have_gpu_capture)` when the GPU oracle run (`tools/oracle/gpu_requests.py`'s
//! `cases.txt` and the emulator's `results.txt`) exists: `DW1_GPU_CAPTURE`, default
//! `<workspace>/captures/oracle-runs/gpu-prims`. Without it `tests/gpu_oracle.rs` is ignored.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rustc-check-cfg=cfg(have_gpu_capture)");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=DW1_GPU_CAPTURE");
    let dir = match std::env::var_os("DW1_GPU_CAPTURE") {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs/gpu-prims"),
    };
    // Watched even when missing, so captures added later are picked up.
    println!(
        "cargo::rerun-if-changed={}",
        dir.join("results.txt").display()
    );
    if dir.join("cases.txt").exists() && dir.join("results.txt").exists() {
        println!("cargo::rustc-cfg=have_gpu_capture");
    } else {
        println!(
            "cargo::warning=GPU oracle run not found ({}): tests/gpu_oracle.rs will be ignored",
            dir.display()
        );
    }
}
