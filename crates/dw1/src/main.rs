//! Digimon World as a native Apple Silicon app.
//!
//! For now (Phase 2) the whole game C is linked and its `main()` runs until the first SDK call
//! with no Rust implementation, which stops the program with that call's name.

use std::path::PathBuf;

mod headless;

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
    // Memory cards: an image file per slot if given, else a fresh formatted card (the default).
    for (slot, var) in ["DW1_CARD1", "DW1_CARD2"].into_iter().enumerate() {
        if let Some(path) = std::env::var_os(var) {
            let card = std::fs::read(&path)
                .ok()
                .and_then(|b| psyq::libmcrd::Card::from_bytes(&b));
            match card {
                Some(card) => psyq::libmcrd::insert_card(slot, Some(card)),
                None => {
                    eprintln!(
                        "dw1: {var}: {} is not a 128 KB card image",
                        path.to_string_lossy()
                    );
                    std::process::exit(1);
                }
            }
        }
    }
    psyq::libetc::set_host(Box::new(headless::Headless::from_env()));
    // DW1_MOVIE=<id> plays one movie on its own (0 = the intro) and stops: the game's movie
    // player, without the boot that precedes it in main().
    if let Some(id) = std::env::var("DW1_MOVIE").ok().and_then(|v| v.parse().ok()) {
        psyq::libetc::ResetCallback();
        psyq::libgpu::ResetGraph(0);
        psyq::libcd::CdInit();
        let completed = unsafe { decomp_c::MOV_playMovie(id) };
        eprintln!(
            "dw1: movie {id} {} at vblank {}",
            if completed == 1 {
                "played to its end"
            } else {
                "was skipped"
            },
            psyq::libetc::vblank_count()
        );
        return;
    }
    // The game's main() never returns on the PS1.
    let status = unsafe { decomp_c::dw1_game_main() };
    eprintln!("dw1: the game's main() returned {status}");
}
