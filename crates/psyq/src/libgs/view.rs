//! libgs projection and view: `GsSetProjection`, `GsSetNearClip`, `GsSetRefView2`.

use std::sync::Mutex;

use crate::globals::{GsWSMATRIX, GsWSMATRIX_ORG, Matrix};
use crate::libgte::{SquareRoot0, Vector, load_matrix, load_v0, mul_long, rcos, rsin};
use backend::gte::creg;

/// libgs `GsRVIEW2`.
#[repr(C)]
#[derive(Debug)]
pub struct GsRview2 {
    pub vpx: i32,
    pub vpy: i32,
    pub vpz: i32,
    pub vrx: i32,
    pub vry: i32,
    pub vrz: i32,
    pub rz: i32,
    pub sup: *mut super::model::GsCoordinate2,
}

struct View {
    projection: i32,
    near_clip: i32,
}

static VIEW: Mutex<View> = Mutex::new(View {
    projection: 1000,
    near_clip: 10,
});

/// The near clip distance `GsSetNearClip` set (for the object sorters).
pub fn near_clip() -> i32 {
    VIEW.lock().unwrap().near_clip
}

/// `GsSetProjection(h)`: the projection distance, GTE register H [doc; obs: only H changed].
#[unsafe(no_mangle)]
pub extern "C" fn GsSetProjection(h: i32) {
    VIEW.lock().unwrap().projection = h;
    crate::gte::with(|g| g.ctc2(26, h as u32));
}

/// `GsSetNearClip(near)`: the near clip distance [doc; obs: stored, nothing else].
#[unsafe(no_mangle)]
pub extern "C" fn GsSetNearClip(near: i32) -> i32 {
    VIEW.lock().unwrap().near_clip = near;
    0
}

/// `GsSetRefView2(pv)`: the world-to-screen matrix `GsWSMATRIX` (and its copy `GsWSMATRIX_ORG`)
/// for a camera at `vp` looking at `vr`, twisted by `rz` [doc; obs: bit exact against the real
/// function, see `tests/libgte_oracle.rs`]. Returns 0, or 1 when `vp` and `vr` coincide.
///
/// What the real function does, as measured:
/// - `vp` and `vr` are shifted right (arithmetic) together until every component is below 2^15
///   in magnitude, and the difference `d = vr - vp` of the shifted values is the view direction.
/// - With `lxz = SquareRoot0(dx^2 + dz^2)` and `r = SquareRoot0(dx^2 + dy^2 + dz^2)` (both the
///   coarse libgte root, so the rows come out up to 0.3% too long: 4098, 4105...), the heading is
///   `c = dz * 4096 / lxz`, `s = -dx * 4096 / lxz` (C division; straight up or down, `lxz == 0`,
///   gives c = 4096, s = 0) and the pitch `sp = dy * 4096 / r`, `cp = lxz * 4096 / r`.
/// - The rows are pitch times heading, with the twist in front: the twist angle is
///   `-rz / 360` (C division, so `rz` is in odd units) turns of 1/4096 through `rsin`/`rcos`,
///   rotating the first two rows. Every product is floored when shifted by 12.
/// - The translation is `-(R * vp)` for the unshifted `vp` through the long-vector product of
///   `ApplyMatrixLV`. The GTE is left with RT = R, V0 = (s, 0, c) (straight up or down:
///   (0, -sp, 0)), and the registers of that product; LZCS is the last square root's argument.
/// - `vp == vr` (after shifting) fails: `GsWSMATRIX` becomes the identity, `GsWSMATRIX_ORG` is
///   untouched, and the result is 1.
///
/// Not modelled: a parent coordinate system (`super`, which the game always sets to NULL) stops
/// the program, and view directions whose squared lengths overflow 32 bits (only reachable with
/// a `vp` and `vr` on opposite sides of the origin and beyond 2^14) make the real function take
/// the square root of a negative number, which has no model (see `SquareRoot0`).
///
/// # Safety
/// `pv` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSetRefView2(pv: *mut GsRview2) -> i32 {
    let v = unsafe { &*pv };
    if !v.sup.is_null() {
        crate::trap::unimplemented("GsSetRefView2 with a parent coordinate system");
    }
    let vp = [v.vpx, v.vpy, v.vpz];
    let vr = [v.vrx, v.vry, v.vrz];
    let mut k = 0;
    // |INT_MIN| is INT_MIN, so a component of -2^31 never counts as large [obs].
    while vp
        .iter()
        .chain(&vr)
        .any(|x| x.wrapping_abs() >> k >= 1 << 15)
    {
        k += 1;
    }
    let d = [0, 1, 2].map(|i| (vr[i] >> k).wrapping_sub(vp[i] >> k));
    let sqr = |a: i32| a.wrapping_mul(a);
    let r_arg = sqr(d[0]).wrapping_add(sqr(d[1])).wrapping_add(sqr(d[2]));
    let lxz_arg = sqr(d[0]).wrapping_add(sqr(d[2]));
    let r = SquareRoot0(r_arg);
    let lxz = SquareRoot0(lxz_arg);
    if r == 0 {
        unsafe { store_matrix(&raw mut GsWSMATRIX, &Matrix::IDENTITY) };
        return 1;
    }
    let (c, s) = if lxz == 0 {
        (4096, 0)
    } else {
        (
            d[2].wrapping_mul(4096) / lxz,
            d[0].wrapping_neg().wrapping_mul(4096) / lxz,
        )
    };
    let sp = d[1].wrapping_mul(4096) / r;
    let cp = lxz.wrapping_mul(4096) / r;
    // The twist: theta = -rz / 360 (computed as (-rz) / 360), rotating the first two rows.
    let theta = rz_angle(v.rz);
    let (sz, cz) = (rsin(theta), rcos(theta));
    let sh = |x: i32| x >> 12;
    let tp = [
        [cz, sh(-sz * cp), sh(sz * sp)],
        [sz, sh(cz * cp), sh(-cz * sp)],
        [0, sp, cp],
    ];
    let mut m = Matrix::default();
    for (i, row) in tp.iter().enumerate() {
        m.m[i] = [
            sh(row[0] * c - row[2] * s) as i16,
            row[1] as i16,
            sh(row[0] * s + row[2] * c) as i16,
        ];
    }
    let t = crate::gte::with(|g| {
        if lxz == 0 {
            load_v0(g, 0, (-sp) as i16, 0);
        } else {
            load_v0(g, s as i16, 0, c as i16);
        }
        load_matrix(g, creg::RT0, &m);
        let neg = Vector {
            vx: vp[0].wrapping_neg(),
            vy: vp[1].wrapping_neg(),
            vz: vp[2].wrapping_neg(),
            pad: 0,
        };
        mul_long(g, &neg)
    });
    m.t = t;
    unsafe {
        store_matrix(&raw mut GsWSMATRIX, &m);
        store_matrix(&raw mut GsWSMATRIX_ORG, &m);
    }
    0
}

/// Stores `m`; the last rotation element goes out as a whole word, so the 2 padding bytes after
/// it take its sign (0xFFFF when it is negative) [obs].
unsafe fn store_matrix(dst: *mut Matrix, m: &Matrix) {
    unsafe {
        (*dst).m = m.m;
        (*dst).t = m.t;
        dst.cast::<u8>()
            .add(16)
            .cast::<i32>()
            .write_unaligned(i32::from(m.m[2][2]));
    }
}

/// The twist angle in 1/4096 turns for a `GsRVIEW2.rz` [obs]: `-rz / 360`, negated before the C
/// division (so it differs from `-(rz / 360)` only for `i32::MIN`).
fn rz_angle(rz: i32) -> i32 {
    rz.wrapping_neg() / 360
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boot_view_is_the_ps1s() {
        let _gate = crate::gte::serial();
        let mut v = GsRview2 {
            vpx: 0,
            vpy: 0,
            vpz: -2000,
            vrx: 0,
            vry: 0,
            vrz: 0,
            rz: 0,
            sup: std::ptr::null_mut(),
        };
        assert_eq!(unsafe { GsSetRefView2(&mut v) }, 0);
        let m = unsafe { GsWSMATRIX };
        // The boot view [obs: the real call at the game's start]: the coarse square root of
        // 2000^2 is 1999, so the unit axes come out 2000 / 1999 long.
        assert_eq!(m.m, [[0x1002, 0, 0], [0, 0x1000, 0], [0, 0, 0x1002]]);
        assert_eq!(m.t, [0, 0, 2000]);
        assert_eq!(unsafe { GsWSMATRIX_ORG }, m);
    }

    #[test]
    fn a_zero_length_view_fails_and_resets_the_matrix() {
        let _gate = crate::gte::serial();
        let mut v = GsRview2 {
            vpx: 5,
            vpy: -7,
            vpz: 11,
            vrx: 5,
            vry: -7,
            vrz: 11,
            rz: 0,
            sup: std::ptr::null_mut(),
        };
        unsafe { GsWSMATRIX = Matrix::default() };
        assert_eq!(unsafe { GsSetRefView2(&mut v) }, 1);
        assert_eq!(unsafe { GsWSMATRIX }, Matrix::IDENTITY);
    }
}
