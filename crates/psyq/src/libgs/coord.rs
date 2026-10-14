//! libgs coordinate systems: `GsGetLw`, `GsGetLs`, `GsGetLws`, `GsMulCoord0`, `GsMulCoord3`,
//! `GsSetView2`.
//!
//! Clean-room: meaning from the Psy-Q reference (`[doc]`), arithmetic and side effects from the
//! real functions run in PCSX-Redux (`[obs]`, `tools/oracle/libgs_requests.py`,
//! `tests/libgs_oracle.rs`).

use super::model::GsCoordinate2;
use crate::globals::{GsWSMATRIX, GsWSMATRIX_ORG, Matrix, PSDCNT};
use crate::libgte::{ApplyMatrixLV, MulMatrix, MulMatrix0, TransposeMatrix, Vector};

/// libgs `GsVIEW2`.
#[repr(C)]
#[derive(Debug)]
pub struct GsView2 {
    pub view: Matrix,
    pub sup: *mut GsCoordinate2,
}

/// The chain a `super` list can have: the real code keeps the pointers in a 32-entry array of
/// its own, and a longer chain runs over the neighbouring globals.
const MAX_DEPTH: usize = 32;

fn word_add(dst: *mut i32, src: *const i32) {
    unsafe { dst.write(dst.read().wrapping_add(src.read())) };
}

/// Copies a whole 32-byte MATRIX, padding included (the real code moves it as words).
unsafe fn copy_matrix(dst: *mut Matrix, src: *const Matrix) {
    unsafe { std::ptr::copy(src.cast::<u8>(), dst.cast::<u8>(), 32) };
}

/// `GsMulCoord0(m1, m2, m3)`: `m3 = m1 * m2` for matrices with a translation: the rotations
/// multiplied (`MulMatrix0`) and the translation `m1.rot * m2.t + m1.t` (a long-vector product,
/// 32-bit wrapping sum) [doc; obs].
///
/// The translation goes into `m3.t` first and `m1.t` is added to it from memory afterwards, so
/// `m3 == m1` adds the new translation to itself, which the real function does too [obs].
///
/// # Safety
/// The matrices must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsMulCoord0(m1: *const Matrix, m2: *const Matrix, m3: *mut Matrix) {
    unsafe {
        let t3 = (&raw mut (*m3).t).cast::<Vector>();
        ApplyMatrixLV(m1, (&raw const (*m2).t).cast::<Vector>(), t3);
        MulMatrix0(m1, m2, m3);
        for i in 0..3 {
            word_add(
                (&raw mut (*m3).t).cast::<i32>().add(i),
                (&raw const (*m1).t).cast::<i32>().add(i),
            );
        }
    }
}

/// `GsMulCoord3(m1, m2)`: `m1 = m1 * m2` the same way [doc; obs: unlike `GsMulCoord0(m1, m2,
/// m1)`, the translation uses the old `m1.t`, also when `m2 == m1`].
///
/// # Safety
/// The matrices must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsMulCoord3(m1: *mut Matrix, m2: *const Matrix) {
    unsafe {
        let mut v = Vector::default();
        ApplyMatrixLV(m1, (&raw const (*m2).t).cast::<Vector>(), &mut v);
        let old = (&raw const (*m1).t).read();
        MulMatrix(m1, m2);
        (*m1).t = [
            v.vx.wrapping_add(old[0]),
            v.vy.wrapping_add(old[1]),
            v.vz.wrapping_add(old[2]),
        ];
    }
}

/// The chain from `m` up to its root: `[m, parent, ..., root]`. A `super` that is neither null
/// nor a coordinate system (`SCREEN`, 1, which the real code dereferences and faults on), or a
/// chain deeper than the real code's own stack, stops the program. The ascent stops at the
/// first non-root node whose `flg` is the current stamp (the real code never looks above it,
/// so what its `super` holds doesn't matter); the flag says whether it did.
unsafe fn chain(m: *mut GsCoordinate2, stamp: u32) -> (Vec<*mut GsCoordinate2>, bool) {
    let mut nodes = vec![];
    let mut p = m;
    while !p.is_null() {
        if (p as usize) < 0x1000 {
            crate::trap::unimplemented("a GsCOORDINATE2 super of SCREEN (faults on the PS1)");
        }
        if nodes.len() == MAX_DEPTH {
            crate::trap::unimplemented("a GsCOORDINATE2 chain deeper than 32");
        }
        nodes.push(p);
        let (sup, flg) = unsafe { ((&raw const (*p).sup).read(), (&raw const (*p).flg).read()) };
        if !sup.is_null() && flg == stamp {
            return (nodes, true);
        }
        p = sup;
    }
    (nodes, false)
}

/// Brings `workm` (the local-to-world matrix) of `m` and its parents up to date and returns a
/// pointer to `m`'s [obs: exhaustive over `flg` in {0, 1, stamp, other} for chains of 1 to 4].
///
/// `flg` is a stamp. The stamp `S` is libgs's swap counter `PSDCNT`; a recomputed coordinate
/// system gets `flg = S`, and the game sets `flg = 0` when it changes `coord`. Walking up from
/// `m` (the leaf):
/// - The first coordinate system that is not the root and has `flg == S` is the base: its
///   `workm` is taken as it is and everything below it is recomputed (so `m` itself having
///   `flg == S` recomputes nothing, however stale its parents are).
/// - With no such base, a root with `flg` 0 or `S` is recomputed, and then everything below it.
/// - Otherwise the topmost non-root coordinate system with `flg == 0` is recomputed, and
///   everything below it; if there is none, nothing is.
///
/// A recomputed root's `workm` is a copy of its `coord`; any other is `GsMulCoord0(parent
/// workm, coord)`.
unsafe fn update_workm(m: *mut GsCoordinate2) -> *const Matrix {
    unsafe {
        let stamp = PSDCNT;
        let (nodes, cut) = chain(m, stamp);
        let n = nodes.len();
        let flg = |i: usize| (&raw const (*nodes[i]).flg).read();
        // Nodes 0..first are recomputed (leaf first).
        let first = if cut {
            n - 1
        } else if let Some(base) = (0..n - 1).find(|&i| flg(i) == stamp) {
            base
        } else if flg(n - 1) == 0 || flg(n - 1) == stamp {
            n
        } else {
            (0..n - 1).rev().find(|&i| flg(i) == 0).map_or(0, |i| i + 1)
        };
        for depth in (0..first).rev() {
            let node = nodes[depth];
            if depth + 1 == n {
                copy_matrix(&raw mut (*node).workm, &raw const (*node).coord);
            } else {
                let parent = nodes[depth + 1];
                GsMulCoord0(
                    &raw const (*parent).workm,
                    &raw const (*node).coord,
                    &raw mut (*node).workm,
                );
            }
            (&raw mut (*node).flg).write(stamp);
        }
        &raw const (*m).workm
    }
}

/// `GsGetLw(m, out)`: the local-to-world matrix of `m` [doc; obs: `workm`, kept up to date as in
/// [`update_workm`]].
///
/// # Safety
/// `m` and its chain, and `out`, must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsGetLw(m: *mut GsCoordinate2, out: *mut Matrix) {
    unsafe {
        let lw = update_workm(m);
        copy_matrix(out, lw);
    }
}

/// `GsGetLs(m, out)`: the local-to-screen matrix, `GsMulCoord0(&GsWSMATRIX, &Lw, out)` [doc; obs].
///
/// # Safety
/// `m` and its chain, and `out`, must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsGetLs(m: *mut GsCoordinate2, out: *mut Matrix) {
    unsafe {
        let lw = update_workm(m);
        GsMulCoord0(&raw const GsWSMATRIX, lw, out);
    }
}

/// `GsGetLws(m, outw, outs)`: both, the world matrix first [doc; obs].
///
/// # Safety
/// `m` and its chain, and the outputs, must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsGetLws(m: *mut GsCoordinate2, outw: *mut Matrix, outs: *mut Matrix) {
    unsafe {
        let lw = update_workm(m);
        copy_matrix(outw, lw);
        GsMulCoord0(&raw const GsWSMATRIX, lw, outs);
    }
}

/// `GsSetView2(pv)`: the world-to-screen matrix `GsWSMATRIX` (and its copy `GsWSMATRIX_ORG`)
/// from a viewpoint matrix [doc]. Returns 0.
///
/// With no parent (`super` NULL) the matrix is copied as it is. With one, the matrix is taken
/// relative to the parent's world matrix `Lw`: `GsWSMATRIX = view * Lw^-1`, the inverse built
/// as the transposed rotation with translation `-(Lw^T * Lw.t)` [obs].
///
/// # Safety
/// `pv` and its chain must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn GsSetView2(pv: *const GsView2) -> i32 {
    unsafe {
        let sup = (&raw const (*pv).sup).read();
        if sup.is_null() {
            copy_matrix(&raw mut GsWSMATRIX, &raw const (*pv).view);
        } else {
            let lw = update_workm(sup);
            let mut inv = Matrix::default();
            TransposeMatrix(lw, &mut inv);
            let lw_t = Vector {
                vx: (*lw).t[0],
                vy: (*lw).t[1],
                vz: (*lw).t[2],
                pad: 0,
            };
            let mut t = Vector::default();
            ApplyMatrixLV(&inv, &lw_t, &mut t);
            inv.t = [
                t.vx.wrapping_neg(),
                t.vy.wrapping_neg(),
                t.vz.wrapping_neg(),
            ];
            GsMulCoord0(&raw const (*pv).view, &inv, &raw mut GsWSMATRIX);
        }
        copy_matrix(&raw mut GsWSMATRIX_ORG, &raw const GsWSMATRIX);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(flg: u32, tx: i32, sup: *mut GsCoordinate2) -> GsCoordinate2 {
        let mut n: GsCoordinate2 = unsafe { std::mem::zeroed() };
        n.flg = flg;
        n.coord = Matrix::IDENTITY;
        n.coord.t = [tx, 0, 0];
        n.sup = sup;
        n
    }

    #[test]
    fn a_chain_multiplies_up_and_the_stamp_caches_it() {
        let _gate = crate::gte::serial();
        unsafe { PSDCNT = 7 };
        let mut root = node(0, 100, std::ptr::null_mut());
        let root_ptr: *mut GsCoordinate2 = &mut root;
        let mut leaf = node(0, 20, root_ptr);
        let mut out = Matrix::default();
        unsafe { GsGetLw(&mut leaf, &mut out) };
        assert_eq!(out.t, [120, 0, 0]);
        assert_eq!(out.m, Matrix::IDENTITY.m);
        assert_eq!((leaf.flg, unsafe { (*root_ptr).flg }), (7, 7));
        // The root moves but its flag says nothing changed: the leaf's stamp is current, so the
        // cached matrix stays.
        unsafe { (*root_ptr).coord.t = [500, 0, 0] };
        unsafe { GsGetLw(&mut leaf, &mut out) };
        assert_eq!(out.t, [120, 0, 0]);
        // The game clears the flag of what it changed: the leaf follows its parent again.
        leaf.flg = 0;
        unsafe { (*root_ptr).flg = 0 };
        unsafe { GsGetLw(&mut leaf, &mut out) };
        assert_eq!(out.t, [520, 0, 0]);
    }

    #[test]
    fn mul_coord_adds_the_first_translation_once() {
        let _gate = crate::gte::serial();
        let mut a = Matrix::IDENTITY;
        a.t = [1000, -2000, 3000];
        let mut b = Matrix::IDENTITY;
        b.t = [1, 2, 3];
        let mut c = Matrix::default();
        unsafe { GsMulCoord0(&a, &b, &mut c) };
        assert_eq!(c.t, [1001, -1998, 3003]);
        unsafe { GsMulCoord3(&mut a, &b) };
        assert_eq!(a.t, [1001, -1998, 3003]);
    }
}
