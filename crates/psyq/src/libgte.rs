//! libgte: matrix and vector helpers over the GTE, the projection calls, and the integer math
//! (`rsin`, `rcos`, `ratan2`, `SquareRoot0`).
//!
//! Clean-room like the rest of the SDK layer: the API meaning comes from the Psy-Q headers and
//! library reference, the GTE commands from psx-spx, and every rounding, overflow and register
//! side effect from running the real functions on chosen inputs in PCSX-Redux
//! (`oracle/call-sdk.lua`, requests from `tools/oracle/gte_requests.py`, compared in
//! `tests/libgte_oracle.rs`). Rules are tagged `[doc]` (documented), `[hdr]` (the headers) and
//! `[obs]` (observed on the real SDK).
//!
//! `long` is 32-bit here (`crates/decomp-c/include/dw1_native.h`), as on the PS1.

use crate::globals::Matrix;
use backend::gte::{Gte, cmd, creg, dreg};
use std::sync::Mutex;

/// libgte `SVECTOR`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Svector {
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    pub pad: i16,
}

/// libgte `VECTOR`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vector {
    pub vx: i32,
    pub vy: i32,
    pub vz: i32,
    pub pad: i32,
}

/// libgte `CVECTOR`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cvector {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub cd: u8,
}

// ---- integer math ----

/// `rsin(a)`: 4096 * sin of `a` in 1/4096 turns, rounded to the nearest integer; only the low 12
/// bits of `a` count [obs: matches round(4096 sin) for every angle].
#[unsafe(no_mangle)]
pub extern "C" fn rsin(a: i32) -> i32 {
    let a = (a & 0xFFF) as f64;
    (4096.0 * (a * std::f64::consts::TAU / 4096.0).sin()).round() as i32
}

/// `rcos(a)`: the cosine, the sine a quarter turn on [obs].
#[unsafe(no_mangle)]
pub extern "C" fn rcos(a: i32) -> i32 {
    rsin(a.wrapping_add(0x400))
}

/// The angle (1/4096 turns, 0..=512) of a slope `i / 1024`: the table `ratan2` indexes [obs:
/// atan((i + 3/8) / 1024) rounded to the nearest 1/4096 turn, for all 1025 cells].
fn atan_cell(i: i32) -> i32 {
    ((i as f64 + 0.375) / 1024.0)
        .atan()
        .mul_add(2048.0 / std::f64::consts::PI, 0.5)
        .floor() as i32
}

/// The table cell of the slope `n / d` (0 <= n <= d), 0..=1024: `n * 1024 / d` in the real
/// function's 32-bit arithmetic [obs: for n >= 2^21, where `n << 10` would overflow, it divides
/// `n` by `d >> 10` instead, so the cell is slightly off for large vectors].
fn slope_cell(n: i32, d: i32) -> i32 {
    if n >= 1 << 21 {
        n / (d >> 10)
    } else {
        n.wrapping_shl(10) / d
    }
}

/// `ratan2(y, x)`: the angle of the point (x, y) in 1/4096 turns, -2048..=2048 [obs].
///
/// The octant comes from comparing |y| with |x| as signed 32-bit numbers (so |INT_MIN| is
/// negative, with the odd results that gives, all reproduced), then the angle from the table cell
/// of the smaller over the larger, and the signs of x and y. `ratan2(0, INT_MIN)` and
/// `ratan2(INT_MIN, 0)` never return on the PS1 (a division by zero); here they stop the program.
#[unsafe(no_mangle)]
pub extern "C" fn ratan2(y: i32, x: i32) -> i32 {
    let (ay, ax) = (y.wrapping_abs(), x.wrapping_abs());
    if ay == 0 && ax == 0 {
        return 0;
    }
    let a = if ay < ax {
        if ax == 0 {
            crate::trap::unimplemented("ratan2(INT_MIN, 0): hangs on the PS1");
        }
        atan_cell(slope_cell(ay, ax))
    } else {
        if ay == 0 {
            crate::trap::unimplemented("ratan2(0, INT_MIN): hangs on the PS1");
        }
        1024 - atan_cell(slope_cell(ax, ay))
    };
    let a = if x < 0 { 2048 - a } else { a };
    if y < 0 { -a } else { a }
}

/// `SquareRoot0(a)`: the square root of `a`, coarse above 256 [obs].
///
/// Exact (rounded down) for `a < 256`. Above that, with `4^m <= a < 4^(m+1)`, only the top 7-8
/// bits of `a` count (the radicand is cut to a multiple of `4^(m-3)`, a table index in 64..=255
/// in effect) and the root of that keeps only its top 13 bits (its low `m-12` bits are zero):
/// `SquareRoot0(4_000_000)` is 1999, not 2000. The argument is also left in the GTE's LZCS
/// (the real function counts leading zeros through it).
///
/// Negative arguments make the real function read before its table (the answers have no formula);
/// this returns 0 for them and for 0 [not modelled: the game never passes one].
#[unsafe(no_mangle)]
pub extern "C" fn SquareRoot0(a: i32) -> i32 {
    crate::gte::with(|g| g.mtc2(dreg::LZCS, a as u32));
    if a <= 0 {
        return 0;
    }
    let a = a as u32;
    let m = (31 - a.leading_zeros()) / 2;
    let sh = 2 * m.saturating_sub(3);
    let root = (((a >> sh) << sh) as f64).sqrt() as u32;
    let granule = 1u32 << m.saturating_sub(12);
    (root / granule * granule) as i32
}

// ---- GTE helpers ----

fn pack(lo: i16, hi: i16) -> u32 {
    (lo as u16 as u32) | ((hi as u16 as u32) << 16)
}

/// `ctc2` the five words of a 3x3 matrix at control register `base`.
pub(crate) fn load_matrix(g: &mut Gte, base: usize, m: &Matrix) {
    g.ctc2(base, pack(m.m[0][0], m.m[0][1]));
    g.ctc2(base + 1, pack(m.m[0][2], m.m[1][0]));
    g.ctc2(base + 2, pack(m.m[1][1], m.m[1][2]));
    g.ctc2(base + 3, pack(m.m[2][0], m.m[2][1]));
    g.ctc2(base + 4, m.m[2][2] as u32);
}

pub(crate) fn load_v0(g: &mut Gte, x: i16, y: i16, z: i16) {
    g.mtc2(dreg::VXY0, pack(x, y));
    g.mtc2(dreg::VZ0, z as u16 as u32);
}

fn mac(g: &Gte, i: usize) -> i32 {
    g.mfc2(dreg::MAC0 + i) as i32
}

fn ir(g: &Gte, i: usize) -> i32 {
    g.mfc2(dreg::IR0 + i) as i16 as i32
}

// ---- matrix registers ----

/// `SetRotMatrix(m)`: the rotation part of `m` into the GTE's RT [doc].
///
/// # Safety
/// `m` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SetRotMatrix(m: *const Matrix) {
    let m = unsafe { &*m };
    crate::gte::with(|g| load_matrix(g, creg::RT0, m));
}

/// `SetLightMatrix(m)`: `m` into the GTE's light matrix LLM [doc].
///
/// # Safety
/// `m` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SetLightMatrix(m: *const Matrix) {
    let m = unsafe { &*m };
    crate::gte::with(|g| load_matrix(g, creg::LLM0, m));
}

/// `SetTransMatrix(m)`: the translation of `m` into the GTE's TR [doc].
///
/// # Safety
/// `m` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SetTransMatrix(m: *const Matrix) {
    let m = unsafe { &*m };
    crate::gte::with(|g| {
        for i in 0..3 {
            g.ctc2(creg::TRX + i, m.t[i] as u32);
        }
    });
}

/// The matrix stack: RT and TR, eight control words per entry.
static STACK: Mutex<Vec<[u32; 8]>> = Mutex::new(Vec::new());

/// How many entries the stack holds [obs: 20; a push on a full stack does nothing, and neither
/// does a pop on an empty one].
const STACK_DEPTH: usize = 20;

/// `PushMatrix()`: saves the GTE's RT and TR on a stack [doc].
#[unsafe(no_mangle)]
pub extern "C" fn PushMatrix() {
    let mut entry = [0u32; 8];
    crate::gte::with(|g| {
        for (i, e) in entry.iter_mut().enumerate() {
            *e = g.cfc2(i);
        }
    });
    let mut stack = STACK.lock().unwrap();
    if stack.len() < STACK_DEPTH {
        stack.push(entry);
    }
}

/// `PopMatrix()`: restores the last pushed RT and TR [doc].
#[unsafe(no_mangle)]
pub extern "C" fn PopMatrix() {
    let Some(entry) = STACK.lock().unwrap().pop() else {
        return;
    };
    crate::gte::with(|g| {
        for (i, &e) in entry.iter().enumerate() {
            g.ctc2(i, e);
        }
    });
}

// ---- software matrices ----

/// `TransMatrix(m, v)`: sets the translation of `m` to `v` [doc].
///
/// # Safety
/// `m` and `v` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn TransMatrix(m: *mut Matrix, v: *const Vector) -> *mut Matrix {
    unsafe {
        let v = *v;
        (*m).t = [v.vx, v.vy, v.vz];
    }
    m
}

/// `ScaleMatrix(m, v)`: scales column j of `m` by `v[j]` / 4096 [doc].
///
/// The last element is stored as a whole word (the 32-bit product shifted), so the 2 padding
/// bytes after it hold the upper half of that word [obs].
///
/// # Safety
/// `m` and `v` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ScaleMatrix(m: *mut Matrix, v: *const Vector) -> *mut Matrix {
    unsafe {
        let v = *v;
        let s = [v.vx, v.vy, v.vz];
        let m22 = (*m).m[2][2];
        for row in &mut (*m).m {
            for (e, s) in row.iter_mut().zip(s) {
                *e = (i32::from(*e).wrapping_mul(s) >> 12) as i16;
            }
        }
        let last = i32::from(m22).wrapping_mul(s[2]) >> 12;
        m.cast::<u8>().add(16).cast::<i32>().write_unaligned(last);
    }
    m
}

/// `TransposeMatrix(m0, m1)`: the rotation of `m0` transposed into `m1` [doc].
///
/// # Safety
/// `m0` and `m1` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn TransposeMatrix(m0: *const Matrix, m1: *mut Matrix) -> *mut Matrix {
    unsafe {
        let a = (*m0).m;
        (*m1).m = std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]));
    }
    m1
}

/// `x * y / 4096` in the 4.12 fixed point of the rotation code: the product shifted right with
/// the sign kept (a floor), as every step of the angle matrices does it.
fn mul12(x: i32, y: i32) -> i32 {
    x.wrapping_mul(y) >> 12
}

/// The sines and cosines of the three angles of `r`.
fn trig(r: &Svector) -> [(i32, i32); 3] {
    [r.vx, r.vy, r.vz].map(|a| (rsin(i32::from(a)), rcos(i32::from(a))))
}

/// Stores a rotation computed in `i32` into `m` (the entries are shorts).
unsafe fn store_rot(m: *mut Matrix, rot: [[i32; 3]; 3]) {
    for (i, row) in rot.iter().enumerate() {
        for (j, &v) in row.iter().enumerate() {
            unsafe { (*m).m[i][j] = v as i16 };
        }
    }
}

/// `RotMatrix(r, m)`: the rotation matrix Rx(r.vx) * Ry(r.vy) * Rz(r.vz), angles in 1/4096
/// turns, 4.12 fixed point; the translation is left alone [doc: the Psy-Q reference; obs: the
/// rounding].
///
/// The matrix is Rx times (Ry times Rz), and each product is shifted (floor) as soon as it is
/// formed, the three-factor entries through a rounded two-factor intermediate [obs: bit exact
/// over 1200 random angle triples]. Negative intermediates are floored, not negated after.
///
/// # Safety
/// `r` and `m` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotMatrix(r: *const Svector, m: *mut Matrix) -> *mut Matrix {
    let [(sx, cx), (sy, cy), (sz, cz)] = trig(unsafe { &*r });
    let a = mul12(-sy, cz);
    let n = mul12(-sy, sz);
    unsafe {
        store_rot(
            m,
            [
                [mul12(cy, cz), mul12(-cy, sz), sy],
                [
                    mul12(cx, sz) - mul12(sx, a),
                    mul12(cx, cz) + mul12(sx, n),
                    mul12(-sx, cy),
                ],
                [
                    mul12(sx, sz) + mul12(cx, a),
                    mul12(sx, cz) - mul12(cx, n),
                    mul12(cx, cy),
                ],
            ],
        );
    }
    m
}

/// `RotMatrixYXZ(r, m)`: Ry(r.vy) * Rx(r.vx) * Rz(r.vz) [doc; obs].
///
/// # Safety
/// `r` and `m` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotMatrixYXZ(r: *const Svector, m: *mut Matrix) -> *mut Matrix {
    let [(sx, cx), (sy, cy), (sz, cz)] = trig(unsafe { &*r });
    let p = mul12(sx, sy);
    let q = mul12(cy, sx);
    unsafe {
        store_rot(
            m,
            [
                [
                    mul12(cy, cz) + mul12(sz, p),
                    -mul12(cy, sz) + mul12(cz, p),
                    mul12(cx, sy),
                ],
                [mul12(cx, sz), mul12(cx, cz), -sx],
                [
                    -mul12(cz, sy) + mul12(sz, q),
                    mul12(sy, sz) + mul12(cz, q),
                    mul12(cx, cy),
                ],
            ],
        );
    }
    m
}

/// `RotMatrixZYX(r, m)`: Rz(r.vz) * Ry(r.vy) * Rx(r.vx) [doc; obs].
///
/// # Safety
/// `r` and `m` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotMatrixZYX(r: *const Svector, m: *mut Matrix) -> *mut Matrix {
    let [(sx, cx), (sy, cy), (sz, cz)] = trig(unsafe { &*r });
    let p = mul12(sx, sy);
    let q = mul12(cx, sy);
    unsafe {
        store_rot(
            m,
            [
                [
                    mul12(cy, cz),
                    -mul12(cx, sz) + mul12(cz, p),
                    mul12(sx, sz) + mul12(cz, q),
                ],
                [
                    mul12(cy, sz),
                    mul12(cx, cz) + mul12(sz, p),
                    -mul12(cz, sx) + mul12(sz, q),
                ],
                [-sy, mul12(cy, sx), mul12(cx, cy)],
            ],
        );
    }
    m
}

// ---- matrix arithmetic through the GTE ----

/// The three columns of `m1` through `m0` as the GTE does it: RT = `m0`, then one MVMVA per
/// column; returns the result matrix.
fn mul_columns(g: &mut Gte, m0: &Matrix, m1: &Matrix) -> [[i16; 3]; 3] {
    load_matrix(g, creg::RT0, m0);
    let mut out = [[0i16; 3]; 3];
    for c in 0..3 {
        load_v0(g, m1.m[0][c], m1.m[1][c], m1.m[2][c]);
        g.execute(cmd::mvmva(1, 0, 0, 3, 0));
        for (r, row) in out.iter_mut().enumerate() {
            row[c] = ir(g, r + 1) as i16;
        }
    }
    out
}

/// Stores a rotation the GTE computed: the last element goes out as a whole word (IR3, sign
/// extended), so the padding bytes after it hold its sign [obs].
unsafe fn store_gte_rot(m: *mut Matrix, rot: [[i16; 3]; 3]) {
    unsafe {
        (*m).m = rot;
        m.cast::<u8>()
            .add(16)
            .cast::<i32>()
            .write_unaligned(i32::from(rot[2][2]));
    }
}

/// `MulMatrix0(m0, m1, m2)`: the rotation product `m0 * m1` into `m2` [doc].
///
/// # Safety
/// The matrices must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MulMatrix0(
    m0: *const Matrix,
    m1: *const Matrix,
    m2: *mut Matrix,
) -> *mut Matrix {
    let (a, b) = unsafe { (*m0, *m1) };
    let r = crate::gte::with(|g| mul_columns(g, &a, &b));
    unsafe { store_gte_rot(m2, r) };
    m2
}

/// `MulMatrix(m0, m1)`: `m0 = m0 * m1` (rotation) [doc].
///
/// # Safety
/// The matrices must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn MulMatrix(m0: *mut Matrix, m1: *const Matrix) -> *mut Matrix {
    unsafe { MulMatrix0(m0, m1, m0) }
}

/// `CompMatrix(m0, m1, m2)`: the composition `m2 = m0 * m1`: the rotations multiplied, and the
/// translation `m0 * m1.t + m0.t` [doc; obs].
///
/// The translation is `m0` times the low 16 bits of each part of `m1.t` (so a translation beyond
/// 16 bits is cut, not split as `ApplyMatrixLV` does), and `m0.t` is added with a trapping `add`:
/// the real function faults when the sum overflows, which is not reproduced (the result wraps).
///
/// # Safety
/// The matrices must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn CompMatrix(
    m0: *const Matrix,
    m1: *const Matrix,
    m2: *mut Matrix,
) -> *mut Matrix {
    let (a, b) = unsafe { (*m0, *m1) };
    let (r, t) = crate::gte::with(|g| {
        let r = mul_columns(g, &a, &b);
        // The translation goes in as the low 16 bits of each part (`gte_ldlv0`), not split.
        load_v0(g, b.t[0] as i16, b.t[1] as i16, b.t[2] as i16);
        g.execute(cmd::mvmva(1, 0, 0, 3, 0));
        (r, [mac(g, 1), mac(g, 2), mac(g, 3)])
    });
    unsafe {
        store_gte_rot(m2, r);
        for ((dst, t), t0) in (*m2).t.iter_mut().zip(t).zip(a.t) {
            *dst = t.wrapping_add(t0);
        }
    }
    m2
}

/// `ApplyMatrix(m, v0, v1)`: `v1 = m * v0` [doc].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ApplyMatrix(
    m: *const Matrix,
    v0: *const Svector,
    v1: *mut Vector,
) -> *mut Vector {
    let (m, v) = unsafe { (*m, *v0) };
    crate::gte::with(|g| {
        load_matrix(g, creg::RT0, &m);
        load_v0(g, v.vx, v.vy, v.vz);
        g.execute(cmd::mvmva(1, 0, 0, 3, 0));
        unsafe {
            (*v1).vx = mac(g, 1);
            (*v1).vy = mac(g, 2);
            (*v1).vz = mac(g, 3);
        }
    });
    v1
}

/// `ApplyMatrixSV(m, v0, v1)`: `v1 = m * v0`, short vectors [doc].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ApplyMatrixSV(
    m: *const Matrix,
    v0: *const Svector,
    v1: *mut Svector,
) -> *mut Svector {
    let (m, v) = unsafe { (*m, *v0) };
    crate::gte::with(|g| {
        load_matrix(g, creg::RT0, &m);
        load_v0(g, v.vx, v.vy, v.vz);
        g.execute(cmd::mvmva(1, 0, 0, 3, 0));
        unsafe {
            (*v1).vx = ir(g, 1) as i16;
            (*v1).vy = ir(g, 2) as i16;
            (*v1).vz = ir(g, 3) as i16;
        }
    });
    v1
}

/// `m * v` for a long vector, on the GTE (RT must hold `m`): the vector is split into 15-bit
/// pieces, `v = hi * 32768 + lo` (C division, so `lo` takes the sign of `v`), since the GTE's
/// vector registers are 16 bits wide. `m * hi` is taken unshifted and moved up 3 (32768 / 4096),
/// `m * lo` shifted down 12, and the two added [obs: bit exact, with the registers it leaves].
pub(crate) fn mul_long(g: &mut Gte, v: &Vector) -> [i32; 3] {
    let src = [v.vx, v.vy, v.vz];
    for (i, s) in src.iter().enumerate() {
        g.mtc2(dreg::IR1 + i, (s / 32768) as u32);
    }
    g.execute(cmd::mvmva(0, 0, 3, 3, 0));
    let hi = [mac(g, 1), mac(g, 2), mac(g, 3)];
    for (i, s) in src.iter().enumerate() {
        g.mtc2(dreg::IR1 + i, (s % 32768) as u32);
    }
    g.execute(cmd::mvmva(1, 0, 3, 3, 0));
    [
        (hi[0] << 3).wrapping_add(mac(g, 1)),
        (hi[1] << 3).wrapping_add(mac(g, 2)),
        (hi[2] << 3).wrapping_add(mac(g, 3)),
    ]
}

/// `ApplyMatrixLV(m, v0, v1)`: `v1 = m * v0` with long vectors; leaves RT = `m` and the last
/// (low) partial product in IR/MAC [doc; obs].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ApplyMatrixLV(
    m: *const Matrix,
    v0: *const Vector,
    v1: *mut Vector,
) -> *mut Vector {
    let (m, v) = unsafe { (*m, *v0) };
    let r = crate::gte::with(|g| {
        load_matrix(g, creg::RT0, &m);
        mul_long(g, &v)
    });
    unsafe {
        (*v1).vx = r[0];
        (*v1).vy = r[1];
        (*v1).vz = r[2];
    }
    v1
}

/// `ApplyTransposeMatrixLV(m, v0, v1)`: `v1 = transpose(m) * v0` with long vectors; RT is left
/// holding the transpose, and V0 the low parts `v % 32768` (loaded first) [doc; obs].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ApplyTransposeMatrixLV(
    m: *const Matrix,
    v0: *const Vector,
    v1: *mut Vector,
) -> *mut Vector {
    let (m, v) = unsafe { (*m, *v0) };
    let mut t = Matrix::default();
    for i in 0..3 {
        for j in 0..3 {
            t.m[i][j] = m.m[j][i];
        }
    }
    let r = crate::gte::with(|g| {
        load_v0(
            g,
            (v.vx % 32768) as i16,
            (v.vy % 32768) as i16,
            (v.vz % 32768) as i16,
        );
        load_matrix(g, creg::RT0, &t);
        mul_long(g, &v)
    });
    unsafe {
        (*v1).vx = r[0];
        (*v1).vy = r[1];
        (*v1).vz = r[2];
    }
    v1
}

// ---- projection and lighting ----

/// `RotTransPers(v0, sxy, p, flag)`: projects `v0`; returns OTZ [doc].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotTransPers(
    v0: *const Svector,
    sxy: *mut i32,
    p: *mut i32,
    flag: *mut i32,
) -> i32 {
    let v = unsafe { *v0 };
    crate::gte::with(|g| {
        load_v0(g, v.vx, v.vy, v.vz);
        g.execute(cmd::RTPS);
        unsafe {
            *sxy = g.mfc2(dreg::SXY2) as i32;
            *p = g.mfc2(dreg::IR0) as i32;
            *flag = g.cfc2(creg::FLAG) as i32;
        }
        (g.mfc2(dreg::SZ3) as i32) >> 2
    })
}

/// `RotTransPers3`: three vertices; returns OTZ [doc].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotTransPers3(
    v0: *const Svector,
    v1: *const Svector,
    v2: *const Svector,
    sxy0: *mut i32,
    sxy1: *mut i32,
    sxy2: *mut i32,
    p: *mut i32,
    flag: *mut i32,
) -> i32 {
    let v = unsafe { [*v0, *v1, *v2] };
    crate::gte::with(|g| {
        let w = [dreg::VXY0, dreg::VXY1, dreg::VXY2];
        for (k, vk) in v.iter().enumerate() {
            g.mtc2(w[k], pack(vk.vx, vk.vy));
            g.mtc2(w[k] + 1, vk.vz as u16 as u32);
        }
        g.execute(cmd::RTPT);
        unsafe {
            *sxy0 = g.mfc2(dreg::SXY0) as i32;
            *sxy1 = g.mfc2(dreg::SXY1) as i32;
            *sxy2 = g.mfc2(dreg::SXY2) as i32;
            *p = g.mfc2(dreg::IR0) as i32;
            *flag = g.cfc2(creg::FLAG) as i32;
        }
        (g.mfc2(dreg::SZ3) as i32) >> 2
    })
}

/// `RotTransPers4`: four vertices; returns OTZ [doc].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotTransPers4(
    v0: *const Svector,
    v1: *const Svector,
    v2: *const Svector,
    v3: *const Svector,
    sxy0: *mut i32,
    sxy1: *mut i32,
    sxy2: *mut i32,
    sxy3: *mut i32,
    p: *mut i32,
    flag: *mut i32,
) -> i32 {
    let v = unsafe { [*v0, *v1, *v2, *v3] };
    crate::gte::with(|g| {
        let w = [dreg::VXY0, dreg::VXY1, dreg::VXY2];
        for (k, vk) in v.iter().take(3).enumerate() {
            g.mtc2(w[k], pack(vk.vx, vk.vy));
            g.mtc2(w[k] + 1, vk.vz as u16 as u32);
        }
        g.execute(cmd::RTPT);
        let flag_rtpt = g.cfc2(creg::FLAG);
        unsafe {
            *sxy0 = g.mfc2(dreg::SXY0) as i32;
            *sxy1 = g.mfc2(dreg::SXY1) as i32;
        }
        load_v0(g, v[3].vx, v[3].vy, v[3].vz);
        g.execute(cmd::RTPS);
        unsafe {
            *sxy2 = g.mfc2(dreg::SXY1) as i32;
            *sxy3 = g.mfc2(dreg::SXY2) as i32;
            *p = g.mfc2(dreg::IR0) as i32;
            *flag = (flag_rtpt | g.cfc2(creg::FLAG)) as i32;
        }
        (g.mfc2(dreg::SZ3) as i32) >> 2
    })
}

/// `RotNclip3`: three vertices and the winding; returns MAC0 (the NCLIP result). Only `flag` is
/// stored when it is not positive (the triangle faces away) [obs].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotNclip3(
    v0: *const Svector,
    v1: *const Svector,
    v2: *const Svector,
    sxy0: *mut i32,
    sxy1: *mut i32,
    sxy2: *mut i32,
    p: *mut i32,
    otz: *mut i32,
    flag: *mut i32,
) -> i32 {
    let v = unsafe { [*v0, *v1, *v2] };
    crate::gte::with(|g| {
        let w = [dreg::VXY0, dreg::VXY1, dreg::VXY2];
        for (k, vk) in v.iter().enumerate() {
            g.mtc2(w[k], pack(vk.vx, vk.vy));
            g.mtc2(w[k] + 1, vk.vz as u16 as u32);
        }
        g.execute(cmd::RTPT);
        unsafe { *flag = g.cfc2(creg::FLAG) as i32 };
        g.execute(cmd::NCLIP);
        let clip = mac(g, 0);
        if clip > 0 {
            unsafe {
                *sxy0 = g.mfc2(dreg::SXY0) as i32;
                *sxy1 = g.mfc2(dreg::SXY1) as i32;
                *sxy2 = g.mfc2(dreg::SXY2) as i32;
                *p = g.mfc2(dreg::IR0) as i32;
                *otz = (g.mfc2(dreg::SZ3) as i32) >> 2;
            }
        }
        clip
    })
}

/// `RotNclip4`: four vertices and the winding of the first three. When the first three face away
/// (MAC0 of NCLIP not positive) it returns that number and stores only `flag` (from the RTPT);
/// otherwise it projects the fourth, stores everything, and returns the combined flags [obs].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RotNclip4(
    v0: *const Svector,
    v1: *const Svector,
    v2: *const Svector,
    v3: *const Svector,
    sxy0: *mut i32,
    sxy1: *mut i32,
    sxy2: *mut i32,
    sxy3: *mut i32,
    p: *mut i32,
    otz: *mut i32,
    flag: *mut i32,
) -> i32 {
    let v = unsafe { [*v0, *v1, *v2, *v3] };
    crate::gte::with(|g| {
        let w = [dreg::VXY0, dreg::VXY1, dreg::VXY2];
        for (k, vk) in v.iter().take(3).enumerate() {
            g.mtc2(w[k], pack(vk.vx, vk.vy));
            g.mtc2(w[k] + 1, vk.vz as u16 as u32);
        }
        g.execute(cmd::RTPT);
        let flag_rtpt = g.cfc2(creg::FLAG);
        unsafe { *flag = flag_rtpt as i32 };
        g.execute(cmd::NCLIP);
        let clip = mac(g, 0);
        if clip > 0 {
            unsafe {
                *sxy0 = g.mfc2(dreg::SXY0) as i32;
                *sxy1 = g.mfc2(dreg::SXY1) as i32;
            }
            load_v0(g, v[3].vx, v[3].vy, v[3].vz);
            g.execute(cmd::RTPS);
            unsafe {
                *sxy2 = g.mfc2(dreg::SXY1) as i32;
                *sxy3 = g.mfc2(dreg::SXY2) as i32;
                *p = g.mfc2(dreg::IR0) as i32;
                *flag = (flag_rtpt | g.cfc2(creg::FLAG)) as i32;
                *otz = (g.mfc2(dreg::SZ3) as i32) >> 2;
                // The drawn path falls out of the function with the flags still in the result
                // register [obs]: the game's `0 < RotNclip4(...)` sees 0 for a quad with no flag
                // set, and a negative number when the error bit is.
                return *flag;
            }
        }
        clip
    })
}

/// `NormalColorCol(v0, in, out)`: light the normal `v0` with the light and colour matrices,
/// modulating the colour `in`; the result (code byte kept) goes to `out` [doc].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn NormalColorCol(
    v0: *const Svector,
    inc: *const Cvector,
    out: *mut Cvector,
) {
    let (v, c) = unsafe { (*v0, *inc) };
    crate::gte::with(|g| {
        load_v0(g, v.vx, v.vy, v.vz);
        g.mtc2(dreg::RGBC, u32::from_le_bytes([c.r, c.g, c.b, c.cd]));
        g.execute(cmd::NCCS);
        let w = g.mfc2(dreg::RGB2).to_le_bytes();
        unsafe {
            *out = Cvector {
                r: w[0],
                g: w[1],
                b: w[2],
                cd: w[3],
            }
        };
    });
}

/// `NormalColorCol3`: three normals lit with one colour [doc].
///
/// # Safety
/// The pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn NormalColorCol3(
    v0: *const Svector,
    v1: *const Svector,
    v2: *const Svector,
    inc: *const Cvector,
    out0: *mut Cvector,
    out1: *mut Cvector,
    out2: *mut Cvector,
) {
    let v = unsafe { [*v0, *v1, *v2] };
    let c = unsafe { *inc };
    crate::gte::with(|g| {
        let w = [dreg::VXY0, dreg::VXY1, dreg::VXY2];
        for (k, vk) in v.iter().enumerate() {
            g.mtc2(w[k], pack(vk.vx, vk.vy));
            g.mtc2(w[k] + 1, vk.vz as u16 as u32);
        }
        g.mtc2(dreg::RGBC, u32::from_le_bytes([c.r, c.g, c.b, c.cd]));
        g.execute(cmd::NCCT);
        let rgb = |g: &Gte, r| g.mfc2(r).to_le_bytes();
        let (a, b, d) = (rgb(g, dreg::RGB0), rgb(g, dreg::RGB1), rgb(g, dreg::RGB2));
        unsafe {
            *out0 = Cvector {
                r: a[0],
                g: a[1],
                b: a[2],
                cd: a[3],
            };
            *out1 = Cvector {
                r: b[0],
                g: b[1],
                b: b[2],
                cd: b[3],
            };
            *out2 = Cvector {
                r: d[0],
                g: d[1],
                b: d[2],
                cd: d[3],
            };
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gte::serial;

    // The expected values below were each measured on the real SDK (PCSX-Redux,
    // `oracle/call-sdk.lua`); the full comparison is `tests/libgte_oracle.rs`.

    #[test]
    fn the_trig_table_is_rounded_sines() {
        assert_eq!(rsin(0x400), 0x1000);
        assert_eq!(rsin(-0x400), -0x1000);
        assert_eq!(rsin(12345), 358); // only the low 12 bits count: 12345 = 3 * 4096 + 57
        assert_eq!(rsin(0x7FFF_FFFF), -6);
        assert_eq!(rcos(0), 0x1000);
        assert_eq!(rcos(0x800), -0x1000);
        assert_eq!(rcos(0x400), 0);
    }

    #[test]
    fn ratan2_is_an_angle_in_4096ths_of_a_turn() {
        assert_eq!(ratan2(100, 100), 0x200);
        assert_eq!(ratan2(1, 0), 0x400);
        assert_eq!(ratan2(0, -3), 0x800);
        assert_eq!(ratan2(-5, -3), -1376);
        assert_eq!(ratan2(0, 0), 0);
        // Above 2^21 the slope is divided out in a coarser way: exact math would say 0x70.
        assert_eq!(ratan2(0x2A_05A8, 0xF0_6689), 0x71);
        // |INT_MIN| is negative to the real function, which takes the other octant.
        assert_eq!(ratan2(0x7FFF_FFFF, i32::MIN), 0x400);
        assert_eq!(ratan2(i32::MIN, i32::MAX), 0);
        assert_eq!(ratan2(i32::MIN, -1), -0x800);
    }

    #[test]
    fn square_root_is_coarse_above_256() {
        let _gate = serial();
        assert_eq!(SquareRoot0(0), 0);
        assert_eq!(SquareRoot0(255), 15);
        assert_eq!(SquareRoot0(289), 16); // not 17: the low two bits are dropped
        assert_eq!(SquareRoot0(292), 17);
        assert_eq!(SquareRoot0(4_000_000), 1999); // not 2000
        assert_eq!(SquareRoot0(0x7FFF_FFFF), 46152);
        // It leaves its argument in LZCS.
        assert_eq!(crate::gte::dw1_gte_mfc2(dreg::LZCS as u32), 0x7FFF_FFFF);
    }

    fn matrix_m1() -> Matrix {
        Matrix {
            m: [
                [0x1000, 0x200, -0x300],
                [0x400, 0xF00, 0x500],
                [-0x600, 0x700, 0xE00],
            ],
            t: [1000, -2000, 3000],
        }
    }

    fn matrix_m2() -> Matrix {
        Matrix {
            m: [
                [0xC00, -0x200, 0x300],
                [0x400, 0xC00, -0x500],
                [0x600, 0x700, 0xC00],
            ],
            t: [10, -20, 30],
        }
    }

    #[test]
    fn angle_matrices_are_rounded_products() {
        let _gate = serial();
        let r = Svector {
            vx: 0x100,
            vy: 0x200,
            vz: 0x300,
            pad: 0,
        };
        let mut m = matrix_m1();
        unsafe { RotMatrixZYX(&r, &mut m) };
        assert_eq!(
            m.m,
            [[1107, -3072, 2470], [2675, 2469, 1872], [-2896, 1107, 2675]]
        );
        assert_eq!(m.t, [1000, -2000, 3000]); // the translation is left alone
        unsafe { RotMatrix(&r, &mut m) };
        assert_eq!(i32::from(m.m[0][2]), rsin(0x200)); // Rx * Ry * Rz: the top right is sin(ry)
        unsafe { RotMatrixYXZ(&r, &mut m) };
        assert_eq!(i32::from(m.m[1][2]), -rsin(0x100)); // Ry * Rx * Rz: the middle right is -sin(rx)
    }

    #[test]
    fn matrix_products_use_the_gte() {
        let _gate = serial();
        let (a, b) = (matrix_m1(), matrix_m2());
        let mut out = Matrix::default();
        unsafe { CompMatrix(&a, &b, &mut out) };
        assert_eq!(
            out.m,
            [[2912, -464, 32], [2208, 3312, -48], [640, 3104, 1840]]
        );
        assert_eq!(out.t, [1001, -2007, 3013]);
        let mut prod = Matrix::default();
        unsafe { MulMatrix0(&a, &b, &mut prod) };
        assert_eq!(prod.m, out.m);
        // The last element is stored as a word: the 2 bytes after it hold its sign.
        let bytes = unsafe { std::slice::from_raw_parts((&raw const prod).cast::<u8>(), 20) };
        assert_eq!(&bytes[16..20], &[0x30, 0x07, 0, 0]);
    }

    #[test]
    fn long_vectors_are_split_in_two() {
        let _gate = serial();
        let m = matrix_m1();
        let v = Vector {
            vx: 100_000,
            vy: -200_000,
            vz: 300,
            pad: 0,
        };
        let mut out = Vector::default();
        unsafe { ApplyMatrixLV(&m, &v, &mut out) };
        assert_eq!((out.vx, out.vy, out.vz), (74_943, -162_407, -124_738));
    }

    #[test]
    fn the_matrix_stack_holds_twenty() {
        let _gate = serial();
        let make = |n: i16| Matrix {
            m: [[n, 0, 0], [0, 0, 0], [0, 0, 0]],
            t: [i32::from(n); 3],
        };
        let rt = || crate::gte::dw1_gte_cfc2(creg::RT0 as u32);
        for n in 1..=25 {
            unsafe {
                SetRotMatrix(&make(n));
                SetTransMatrix(&make(n));
            }
            PushMatrix();
        }
        // Pushes beyond 20 did nothing, so the first pop brings back the 20th.
        PopMatrix();
        assert_eq!(rt(), 20);
        for _ in 0..19 {
            PopMatrix();
        }
        assert_eq!(rt(), 1);
        // An empty stack ignores a pop.
        unsafe { SetRotMatrix(&make(99)) };
        PopMatrix();
        assert_eq!(rt(), 99);
    }
}
