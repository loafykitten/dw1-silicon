//! Digimon World as a native Apple Silicon app.
//!
//! For now (Phase 1) the whole game C is linked and its `main()` runs until the first SDK call
//! with no Rust implementation, which stops the program with that call's name.

fn main() {
    // The game's main() never returns on the PS1.
    let status = unsafe { decomp_c::dw1_game_main() };
    eprintln!("dw1: the game's main() returned {status}");
}
