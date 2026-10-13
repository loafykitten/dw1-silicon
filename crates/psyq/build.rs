//! Sets `cfg(have_libgs_capture)` when the S0.7 libgs captures (gitignored, local only) exist:
//! `DW1_LIBGS_CAPTURE`, default `<workspace>/captures/oracle-runs/libgs-pin`. Without them
//! `tests/libgs_capture.rs` shows as an ignored placeholder.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rustc-check-cfg=cfg(have_libgs_capture)");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=DW1_LIBGS_CAPTURE");
    let dir = match std::env::var_os("DW1_LIBGS_CAPTURE") {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs/libgs-pin"),
    };
    // Watched even when missing (Cargo then reruns this script on every build), so captures
    // added later are picked up.
    println!(
        "cargo::rerun-if-changed={}",
        dir.join("calls.txt").display()
    );
    if dir.join("calls.txt").exists() {
        println!("cargo::rustc-cfg=have_libgs_capture");
    } else {
        println!(
            "cargo::warning=libgs captures not found ({}): tests/libgs_capture.rs will be ignored",
            dir.display()
        );
    }
}
