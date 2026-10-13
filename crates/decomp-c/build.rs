//! Compiles game C from the `decomp/` submodule for the host and generates Rust bindings for
//! the decomp structs Rust needs to read.
//!
//! Only files that build natively without SDK headers are listed so far; the list grows as
//! `psyq` gains the libraries they call.

use std::env;
use std::path::{Path, PathBuf};

/// Game C files built natively, relative to `decomp/`.
const GAME_C: &[&str] = &[
    "src/main/rng.c",
    "src/main/math.c",
    // Owns SIN_LOOKUP_TABLE and ATAN_LOOKUP_TABLE (math.c's trig) and the file table.
    "src/main/file_table_us.c",
];

/// Headers bindgen reads, relative to `decomp/include/`.
const BOUND_HEADERS: &[&str] = &["dw/params.h"];

/// Preprocessor setup shared by the C compiler and bindgen, so the bindings describe the C
/// exactly as it is compiled (including everything `DW1_NATIVE` changes).
fn preprocessor_args(native_include: &Path, decomp_include: &Path) -> Vec<String> {
    vec![
        format!("-I{}", native_include.display()),
        format!("-I{}", decomp_include.display()),
        "-DVERSION_US".into(),
        "-DLANGUAGE_C".into(),
        "-include".into(),
        native_include.join("dw1_native.h").display().to_string(),
    ]
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let decomp = manifest
        .join("../../decomp")
        .canonicalize()
        .expect("decomp/ submodule missing: run `git submodule update --init decomp`");
    let decomp_include = decomp.join("include");
    let native_include = manifest.join("include");
    let cpp_args = preprocessor_args(&native_include, &decomp_include);

    let mut build = cc::Build::new();
    build.std("gnu99").warnings(true);
    for arg in &cpp_args {
        build.flag(arg);
    }
    // From the decomp's own GCC flags (decomp/mk/toolchain/gcc.mk): the matching C relies on
    // wrapping and loose aliasing, and must not have libc calls swapped for builtins.
    build
        .flag("-fwrapv")
        .flag("-fno-strict-aliasing")
        .flag("-fno-common")
        .flag("-fno-builtin")
        // Plain char is signed for MIPS GCC and for Apple arm64; pinned so it can't drift.
        // The retail CodeWarrior default is unconfirmed (docs/survey.md, item 8).
        .flag("-fsigned-char");
    for file in GAME_C {
        build.file(decomp.join(file));
    }
    build.compile("decomp");

    let mut bindings = bindgen::Builder::default()
        .clang_args(["-std=gnu99", "-fsigned-char"])
        .clang_args(&cpp_args)
        .allowlist_type("DigimonPara")
        .allowlist_type("PartnerPara")
        .use_core()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    for header in BOUND_HEADERS {
        bindings = bindings.header(decomp_include.join(header).to_str().unwrap());
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .generate()
        .expect("bindgen failed on the decomp headers")
        .write_to_file(out.join("bindings.rs"))
        .unwrap();

    // Directories are watched recursively, so any header or C edit rebuilds.
    for dir in [&decomp.join("src"), &decomp_include, &native_include] {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
}
