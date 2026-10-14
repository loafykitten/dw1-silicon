//! Sets `cfg(have_libgs_capture)` when the S0.7 libgs captures (gitignored, local only) exist:
//! `DW1_LIBGS_CAPTURE`, default `<workspace>/captures/oracle-runs/libgs-pin`. Without them
//! `tests/libgs_capture.rs` shows as an ignored placeholder.
//!
//! Also sets `cfg(have_disc)` (with `DW1_TEST_DISC` and `DW1_TEST_FILES`) when Ember's disc and
//! the files dumpsxiso extracted from it exist, for `tests/libcd_disc.rs`; see
//! `crates/pack/build.rs`.

use std::path::{Path, PathBuf};

fn main() {
    disc();
    gte_capture();
    libgs_light_capture();
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

fn disc() {
    println!("cargo::rustc-check-cfg=cfg(have_disc)");
    println!("cargo::rerun-if-env-changed=DW1_DISC");
    println!("cargo::rerun-if-env-changed=DW1_DISC_FILES");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cue = std::env::var_os("DW1_DISC")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("disc/Digimon World (USA).cue"));
    let files = std::env::var_os("DW1_DISC_FILES")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("decomp/disks/us"));
    println!("cargo::rerun-if-changed={}", cue.display());
    println!("cargo::rerun-if-changed={}", files.display());
    if cue.exists() && files.is_dir() {
        println!("cargo::rustc-cfg=have_disc");
        println!("cargo::rustc-env=DW1_TEST_DISC={}", cue.display());
        println!("cargo::rustc-env=DW1_TEST_FILES={}", files.display());
    } else {
        println!("cargo::warning=disc not found: tests/libcd_disc.rs will be ignored");
    }
}

/// Sets `cfg(have_gte_capture)` when the libgte oracle run (`tools/oracle/gte_requests.py`'s
/// requests, `req.txt` and `results.txt`) exists: `DW1_GTE_CAPTURE`, default
/// `<workspace>/captures/oracle-runs/gte-final`. Without it `tests/libgte_oracle.rs` is ignored.
fn gte_capture() {
    println!("cargo::rustc-check-cfg=cfg(have_gte_capture)");
    println!("cargo::rerun-if-env-changed=DW1_GTE_CAPTURE");
    let dir = match std::env::var_os("DW1_GTE_CAPTURE") {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs/gte-final"),
    };
    println!(
        "cargo::rerun-if-changed={}",
        dir.join("results.txt").display()
    );
    if dir.join("req.txt").exists() && dir.join("results.txt").exists() {
        println!("cargo::rustc-cfg=have_gte_capture");
    } else {
        println!(
            "cargo::warning=libgte oracle run not found ({}): tests/libgte_oracle.rs will be ignored",
            dir.display()
        );
    }
}

/// Sets `cfg(have_libgs_light_capture)` when the libgs light/view/coordinate oracle run
/// (`tools/oracle/libgs_requests.py`'s requests, `req.txt` and `results.txt`) exists:
/// `DW1_LIBGS_LIGHT_CAPTURE`, default `<workspace>/captures/oracle-runs/libgs-light`. Without it
/// `tests/libgs_oracle.rs` is ignored.
fn libgs_light_capture() {
    println!("cargo::rustc-check-cfg=cfg(have_libgs_light_capture)");
    println!("cargo::rerun-if-env-changed=DW1_LIBGS_LIGHT_CAPTURE");
    let dir = match std::env::var_os("DW1_LIBGS_LIGHT_CAPTURE") {
        Some(d) => PathBuf::from(d),
        None => {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs/libgs-light")
        }
    };
    println!(
        "cargo::rerun-if-changed={}",
        dir.join("results.txt").display()
    );
    if dir.join("req.txt").exists() && dir.join("results.txt").exists() {
        println!("cargo::rustc-cfg=have_libgs_light_capture");
    } else {
        println!(
            "cargo::warning=libgs light oracle run not found ({}): tests/libgs_oracle.rs will be ignored",
            dir.display()
        );
    }
}
