//! libgs projection and view: `GsSetProjection`, `GsSetNearClip`, `GsSetRefView2`.

use std::sync::Mutex;

use crate::globals::{GsWSMATRIX, Matrix};

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

/// Integer square root, rounded down.
fn isqrt(v: u64) -> u64 {
    if v == 0 {
        return 0;
    }
    let mut x = (v as f64).sqrt() as u64;
    while x * x > v {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= v {
        x += 1;
    }
    x
}

/// `GsSetRefView2(pv)`: the world-to-screen matrix `GsWSMATRIX` for a camera at `vp` looking
/// at `vr` with twist `rz` [doc].
///
/// **Provisional (Phase 4 settles it):** the axes come from the normalised view direction with
/// an exact integer square root. On the PS1 the one boot call observed (vp = (0, 0, -2000),
/// vr = 0, rz = 0) gives a diagonal of 0x1002, 0x1000, 0x1002 and translation (0, 0, 2000):
/// libgs's square root returns 1999 for 4,000,000 there, which this doesn't reproduce (it
/// gives 0x1000s). Nothing in Phase 2's scenes draws in 3D; Phase 4 derives the real arithmetic
/// from gameplay captures. Twists and a parent coordinate system (`super`) aren't handled yet.
///
/// # Safety
/// `pv` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSetRefView2(pv: *mut GsRview2) -> i32 {
    let v = unsafe { &*pv };
    if v.rz != 0 || !v.sup.is_null() {
        crate::trap::unimplemented("GsSetRefView2 with a twist or a parent coordinate system");
    }
    let (dx, dy, dz) = (
        i64::from(v.vrx) - i64::from(v.vpx),
        i64::from(v.vry) - i64::from(v.vpy),
        i64::from(v.vrz) - i64::from(v.vpz),
    );
    let lxz = isqrt((dx * dx + dz * dz) as u64) as i64;
    let r = isqrt((dx * dx + dy * dy + dz * dz) as u64) as i64;
    if lxz == 0 || r == 0 {
        crate::trap::unimplemented("GsSetRefView2 looking straight up or down");
    }
    let one = 4096i64;
    let x = [dz * one / lxz, 0, -dx * one / lxz];
    let y = [
        -dx * dy * one / (lxz * r),
        lxz * one / r,
        -dz * dy * one / (lxz * r),
    ];
    let z = [dx * one / r, dy * one / r, dz * one / r];
    let rows = [x, y, z];
    let mut m = Matrix::default();
    for (i, row) in rows.iter().enumerate() {
        for (j, &c) in row.iter().enumerate() {
            m.m[i][j] = c as i16;
        }
    }
    let vp = [i64::from(v.vpx), i64::from(v.vpy), i64::from(v.vpz)];
    for (i, row) in rows.iter().enumerate() {
        let dot: i64 = row.iter().zip(vp).map(|(&a, b)| a * -b).sum();
        m.t[i] = (dot >> 12) as i32;
    }
    unsafe { GsWSMATRIX = m };
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boot_view_is_close_to_the_ps1s() {
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
        unsafe { GsSetRefView2(&mut v) };
        let m = unsafe { GsWSMATRIX };
        // The PS1 gives 0x1002 on m[0][0] and m[2][2] (see the function's docs).
        assert_eq!(m.m, [[0x1000, 0, 0], [0, 0x1000, 0], [0, 0, 0x1000]]);
        assert_eq!(m.t, [0, 0, 2000]);
    }
}
