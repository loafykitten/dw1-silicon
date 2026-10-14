//! libgs lighting and the screen matrices: `GsSetFlatLight`, `GsSetAmbient`, `GsSetLightMode`,
//! `GsSetLightMatrix`, `GsSetLsMatrix`.
//!
//! Clean-room like the rest of libgs: what each call is for comes from the Psy-Q reference
//! (`[doc]`); the arithmetic and every side effect from running the real functions in
//! PCSX-Redux on chosen inputs (`[obs]`, `tools/oracle/libgs_requests.py`, replayed by
//! `tests/libgs_oracle.rs`).

use crate::globals::{_LC, GsLIGHT_MODE, GsLIGHTWSMATRIX, Matrix};
use crate::libgte::{
    MulMatrix0, PopMatrix, PushMatrix, SetLightMatrix, SetRotMatrix, SetTransMatrix, SquareRoot0,
};
use backend::gte::creg;

/// libgs `GsF_LIGHT`: a light's direction (any length) and colour.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GsFLight {
    pub vx: i32,
    pub vy: i32,
    pub vz: i32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// `GsSetFlatLight(id, lt)`: light `id` (0..=2) shines along `lt`'s direction with `lt`'s
/// colour [doc]. Returns 0, or -1 when the direction has no length.
///
/// What the real function does, as measured:
/// - `r = SquareRoot0(x*x + y*y + z*z)`, the coarse libgte root of the (32-bit wrapping) sum of
///   squares. A sum whose root is 0 returns -1 and touches nothing else (a sum that wraps
///   negative makes `SquareRoot0` stop the program, as there).
/// - Row `id` of `GsLIGHTWSMATRIX` becomes the reversed unit direction `(-x, -y, -z) * 4096 / r`
///   (C division, truncating; the product wraps in 32 bits), the direction the light's rays
///   come from in 4.12. Column `id` of the light colour matrix (`_LC`) becomes the colour
///   scaled to 4.12, `c * 4096 / 255` (truncating). Only those six shorts are written.
/// - An `id` outside 0..=2 stores nothing and returns 0.
/// - Either way the whole colour matrix goes to the GTE's LCM registers afterwards.
///
/// # Safety
/// `lt` must point to a valid light (its three vector words and three colour bytes are read).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSetFlatLight(id: i32, lt: *const GsFLight) -> i32 {
    let (v, c) = unsafe {
        (
            [
                (&raw const (*lt).vx).read(),
                (&raw const (*lt).vy).read(),
                (&raw const (*lt).vz).read(),
            ],
            [
                (&raw const (*lt).r).read(),
                (&raw const (*lt).g).read(),
                (&raw const (*lt).b).read(),
            ],
        )
    };
    let sq = |a: i32| a.wrapping_mul(a);
    let r = SquareRoot0(sq(v[0]).wrapping_add(sq(v[1])).wrapping_add(sq(v[2])));
    if r == 0 {
        return -1;
    }
    unsafe {
        if (0..=2).contains(&id) {
            let id = id as usize;
            for k in 0..3 {
                (&raw mut GsLIGHTWSMATRIX.m[id][k])
                    .write((v[k].wrapping_neg().wrapping_mul(4096) / r) as i16);
                (&raw mut _LC.m[k][id]).write((i32::from(c[k]) * 4096 / 255) as i16);
            }
        }
        let lc = (&raw const _LC.m).read();
        crate::gte::with(|g| {
            crate::libgte::load_matrix(g, creg::LCM0, &Matrix { m: lc, t: [0; 3] });
        });
    }
    0
}

/// `GsSetAmbient(r, g, b)`: the ambient light, 0x1000 being full white [doc]: the GTE's
/// background colour registers get the values with their low 4 bits cleared [obs].
#[unsafe(no_mangle)]
pub extern "C" fn GsSetAmbient(r: i32, g: i32, b: i32) {
    crate::gte::with(|gte| {
        for (reg, v) in [creg::RBK, creg::GBK, creg::BBK].into_iter().zip([r, g, b]) {
            gte.ctc2(reg, (v & !15) as u32);
        }
    });
}

/// `GsSetLightMode(mode)`: how polygons are lit (0 normal, 1 and up the fog and off variants) [doc].
/// Modes 0..=3 are stored; any other value leaves the mode as it was [obs].
#[unsafe(no_mangle)]
pub extern "C" fn GsSetLightMode(mode: i32) {
    if (0..=3).contains(&mode) {
        unsafe { GsLIGHT_MODE = mode };
    }
}

/// `GsSetLightMatrix(mp)`: the GTE's light matrix LLM becomes `GsLIGHTWSMATRIX * mp`, the
/// lights' directions seen from the object whose world matrix is `mp` [doc]. It borrows RT
/// through the matrix stack (push, `MulMatrix0`, `SetLightMatrix`, pop), so RT and TR end as
/// they were while the rest of the GTE keeps what the multiplication left [obs: all 64
/// registers].
///
/// # Safety
/// `mp` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSetLightMatrix(mp: *const Matrix) {
    let mut tmp = Matrix::default();
    PushMatrix();
    unsafe {
        MulMatrix0(&raw const GsLIGHTWSMATRIX, mp, &mut tmp);
        SetLightMatrix(&tmp);
    }
    PopMatrix();
}

/// `GsSetLsMatrix(mp)`: `mp` becomes the GTE's rotation matrix RT and translation TR [doc; obs:
/// nothing else changes].
///
/// # Safety
/// `mp` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSetLsMatrix(mp: *const Matrix) {
    unsafe {
        SetRotMatrix(mp);
        SetTransMatrix(mp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backend::gte::creg;

    #[test]
    fn the_games_lights_and_ambient() {
        let _gate = crate::gte::serial();
        // new_game.c's first light [obs: the real function gives rows of -1137, -3792, -1137].
        let l = GsFLight {
            vx: 0x1E,
            vy: 0x64,
            vz: 0x1E,
            r: 0x40,
            g: 0x40,
            b: 0x40,
        };
        assert_eq!(unsafe { GsSetFlatLight(0, &l) }, 0);
        let (row, col) = unsafe {
            (
                GsLIGHTWSMATRIX.m[0],
                [_LC.m[0][0], _LC.m[1][0], _LC.m[2][0]],
            )
        };
        assert_eq!(row, [-1137, -3792, -1137]);
        assert_eq!(col, [0x404; 3]);
        assert_eq!(crate::gte::with(|g| g.cfc2(creg::LCM0)), 0x0000_0404);
        GsSetAmbient(0x800, 0x801, 0x1FFF);
        let bk = crate::gte::with(|g| [g.cfc2(creg::RBK), g.cfc2(creg::GBK), g.cfc2(creg::BBK)]);
        assert_eq!(bk, [0x800, 0x800, 0x1FF0]);
        // No length, no light; a light that doesn't exist stores nothing.
        let zero = GsFLight::default();
        assert_eq!(unsafe { GsSetFlatLight(1, &zero) }, -1);
        assert_eq!(unsafe { GsSetFlatLight(3, &l) }, 0);
        GsSetLightMode(2);
        GsSetLightMode(4);
        assert_eq!(unsafe { GsLIGHT_MODE }, 2);
    }
}
