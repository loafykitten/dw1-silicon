//! Digimon World as a native Apple Silicon app. For now (S0.6) it only proves the game C and
//! the Rust PS1 layer link into one program.

fn main() {
    let roll = unsafe { decomp_c::randomLimit(100) };
    let custom = unsafe { decomp_c::customRandom(1, 6) };
    println!("dw1: game C linked; randomLimit(100) = {roll}, customRandom(1, 6) = {custom}");
}
