//! Sets `cfg(have_disc)` (and `DW1_TEST_DISC` / `DW1_TEST_FILES`, the paths the integration test
//! reads) when Ember's disc and the files dumpsxiso extracted from it exist:
//! `DW1_DISC` (default `<workspace>/disc/Digimon World (USA).cue`) and `DW1_DISC_FILES`
//! (default `<workspace>/decomp/disks/us`). Without them `tests/disc.rs` shows as ignored.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rustc-check-cfg=cfg(have_disc)");
    println!("cargo::rerun-if-changed=build.rs");
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
        println!(
            "cargo::warning=disc or extracted files not found ({}, {}): tests/disc.rs will be ignored",
            cue.display(),
            files.display()
        );
    }
}
