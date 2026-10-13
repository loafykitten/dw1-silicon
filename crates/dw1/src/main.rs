//! Digimon World as a native Apple Silicon app.
//!
//! For now (Phase 2) the whole game C is linked and its `main()` runs until the first SDK call
//! with no Rust implementation, which stops the program with that call's name.

use std::path::PathBuf;

/// The disc's `.cue`: `DW1_DISC`, else `<workspace>/disc/Digimon World (USA).cue`.
fn disc_path() -> PathBuf {
    std::env::var_os("DW1_DISC")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../disc/Digimon World (USA).cue")
        })
}

fn open_disc() -> Result<pack::Disc, String> {
    let path = disc_path();
    if !path.exists() {
        return Err(format!(
            "no disc at {}: set DW1_DISC to the .cue of your Digimon World (USA) image",
            path.display()
        ));
    }
    let disc = pack::Disc::open(&path).map_err(|e| e.to_string())?;
    // Reads the whole image once (about a second), so a wrong or damaged dump is caught here
    // and not as garbage in the game.
    disc.verify()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(disc)
}

fn main() {
    match open_disc() {
        Ok(disc) => psyq::libcd::insert_disc(disc),
        Err(e) => {
            eprintln!("dw1: {e}");
            std::process::exit(1);
        }
    }
    // The game's main() never returns on the PS1.
    let status = unsafe { decomp_c::dw1_game_main() };
    eprintln!("dw1: the game's main() returned {status}");
}
