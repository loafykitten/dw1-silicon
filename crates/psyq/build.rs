//! Sets `cfg(have_libgs_capture)` when the `GsSortObject4J` captures (gitignored, local only)
//! exist (see `sort_capture`), and `cfg(have_disc)` (with `DW1_TEST_DISC` and `DW1_TEST_FILES`) when Ember's disc and
//! the files dumpsxiso extracted from it exist, for `tests/libcd_disc.rs`; see
//! `crates/pack/build.rs`.

use std::path::{Path, PathBuf};

fn main() {
    disc();
    gte_capture();
    libgs_light_capture();
    sort_capture();
    println!("cargo::rerun-if-changed=build.rs");
}

/// The capture runs of `GsSortObject4J` under `DW1_SORT_CAPTURES` (default
/// `<workspace>/captures/oracle-runs`): `libgs-pin` (S0.7, `oracle/capture-libgs.lua`) and every
/// `s4j-*` (`oracle/capture-calls.lua` with `DW1_CAPTURE_STATES=1`). Sets
/// `cfg(have_libgs_capture)` when at least one exists (finished or not); without any,
/// `tests/libgs_capture.rs` shows as an ignored placeholder.
fn sort_capture() {
    println!("cargo::rustc-check-cfg=cfg(have_libgs_capture)");
    println!("cargo::rerun-if-env-changed=DW1_SORT_CAPTURES");
    let root = match std::env::var_os("DW1_SORT_CAPTURES") {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs"),
    };
    // Watched even when missing (Cargo then reruns this script on every build), so captures
    // added later are picked up.
    println!("cargo::rerun-if-changed={}", root.display());
    let mut found = 0;
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            // Any run counts, finished or not: the test fails on an interrupted one rather than
            // being ignored.
            if name == "libgs-pin" || name.starts_with("s4j-") {
                found += 1;
            }
        }
    }
    if found > 0 {
        println!("cargo::rustc-cfg=have_libgs_capture");
    } else {
        println!(
            "cargo::warning=GsSortObject4J captures not found under {}: tests/libgs_capture.rs will be ignored",
            root.display()
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
