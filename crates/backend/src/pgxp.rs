//! Precise vertices for the HD surfaces' "no wobble" option (Phase 8.3).
//!
//! Translated from DW2-PC (github.com/Wyrelade/DW2-PC, CC0), `backend/pgxp.c` at commit 2d491b5,
//! DW2-PC's own work; its smooth-frames parts (vertex sources, instances, the camera) are left
//! out. DW2 halves its projections into a 320-wide draw area; DW1 projects into its 320x240
//! buffers directly, so nothing here halves.
//!
//! The GTE (`gte.rs`) reports every RTPS result here when this is on: the integer SXY2 the game
//! sees and a float projection of the same point. The game copies the integers into GPU packets
//! by value, so entries are keyed by the integer position as a packet will hold it. A key often
//! has several different points in one frame (distinct vertices less than a pixel apart): each
//! keeps up to `CANDIDATES` of them, and a polygon picks one per corner so its depths agree
//! (`lookup_poly`). The integer is the float rounded down, so the float is stored half a pixel
//! lower: the precise picture then sits where the integer one does on average, and 2D parts
//! drawn at integer positions line up with it. Entries live from the projection to the end of
//! the next `DrawOTag` (`end_frame`).
//!
//! Nothing here changes what the game sees: the GTE's registers are written before the report,
//! and only the HD surfaces read the table.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The GTE saturated IR1/IR2 or SX2/SY2 for this point: the integer isn't the float rounded,
/// and the float is the point's true position (a vertex far off screen).
pub const CLAMPED: u32 = 1;
/// H >= 2 * z (the divide saturates): the float isn't the point; nothing is recorded.
pub const NEAR: u32 = 2;

const BITS: u32 = 15;
const SIZE: usize = 1 << BITS;
const PROBE: usize = 16;
const CANDIDATES: usize = 8;

#[derive(Clone, Copy, Default)]
struct Cand {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy, Default)]
struct Entry {
    key: u32,
    epoch: u32,
    /// Candidates held; `CANDIDATES + 1` = too many, no precise value for this key.
    n: usize,
    c: [Cand; CANDIDATES],
}

struct Table {
    tab: Vec<Entry>,
    epoch: u32,
}

static ON: AtomicBool = AtomicBool::new(false);
static TABLE: Mutex<Option<Table>> = Mutex::new(None);

fn table<R>(f: impl FnOnce(&mut Table) -> R) -> R {
    let mut t = TABLE.lock().unwrap();
    f(t.get_or_insert_with(|| Table {
        tab: vec![Entry::default(); SIZE],
        epoch: 1,
    }))
}

/// Turns recording on or off (off also forgets everything recorded).
pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed);
    if !on {
        end_frame();
    }
}

pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

fn make_key(x: i32, y: i32) -> u32 {
    ((y as u16 as u32) << 16) | x as u16 as u32
}

fn slot(key: u32) -> usize {
    (key.wrapping_mul(2_654_435_761) >> (32 - BITS)) as usize
}

/// One RTPS result: `sx`, `sy` as SXY2 holds them, the float position `x`, `y` (GTE screen
/// space, the same units) and the view depth `z`; `flags` = `CLAMPED`, `NEAR` or 0.
pub fn record(sx: i32, sy: i32, x: f64, y: f64, z: f64, flags: u32) {
    if flags & NEAR != 0 {
        return;
    }
    // The float must describe the same point as the integer result; when the GTE saturated
    // the integer, the float is the true position of that packet vertex.
    if flags & CLAMPED == 0
        && !(x > sx as f64 - 2.0
            && x < sx as f64 + 2.0
            && y > sy as f64 - 2.0
            && y < sy as f64 + 2.0)
    {
        return;
    }
    let (fx, fy) = ((x - 0.5) as f32, (y - 0.5) as f32);
    let key = make_key(sx, sy);
    let k = slot(key);
    table(|t| {
        for i in 0..PROBE {
            let e = &mut t.tab[(k + i) & (SIZE - 1)];
            if e.epoch != t.epoch {
                e.epoch = t.epoch;
                e.key = key;
                e.n = 1;
                e.c[0] = Cand {
                    x: fx,
                    y: fy,
                    z: z as f32,
                };
                return;
            }
            if e.key == key {
                if e.n > CANDIDATES {
                    return;
                }
                // The same point again (a shared vertex projected twice).
                if e.c[..e.n].iter().any(|c| {
                    let (dx, dy) = (c.x - fx, c.y - fy);
                    dx * dx + dy * dy <= 1.0 / 4096.0
                }) {
                    return;
                }
                if e.n < CANDIDATES {
                    e.c[e.n] = Cand {
                        x: fx,
                        y: fy,
                        z: z as f32,
                    };
                    e.n += 1;
                } else {
                    e.n = CANDIDATES + 1;
                }
                return;
            }
        }
    });
}

fn find(t: &Table, x: i32, y: i32) -> Option<&Entry> {
    let key = make_key(x, y);
    let k = slot(key);
    for i in 0..PROBE {
        let e = &t.tab[(k + i) & (SIZE - 1)];
        if e.epoch != t.epoch {
            return None;
        }
        if e.key == key {
            return (e.n <= CANDIDATES).then_some(e);
        }
    }
    None
}

/// Relative depth distance.
fn zdist(a: f32, b: f32) -> f32 {
    (a - b).abs() / a.min(b)
}

/// The candidate of `e` whose depth is closest to `z`.
fn nearest(e: &Entry, z: f32) -> usize {
    (1..e.n).fold(0, |best, j| {
        if zdist(e.c[j].z, z) < zdist(e.c[best].z, z) {
            j
        } else {
            best
        }
    })
}

/// A precise corner: position in the packet's units (draw offset not applied) and view depth.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Precise {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// The precise positions of a polygon's packet vertices `xy` (3 or 4, draw offset not applied),
/// per corner (`None` for a corner nobody projected this frame). A vertex shared by two polygons
/// normally gets the same answer in both (the same point, or the same depth picks it), so
/// edges stay closed.
pub fn lookup_poly(xy: &[(i32, i32)]) -> [Option<Precise>; 4] {
    let n = xy.len();
    assert!(n == 3 || n == 4);
    if !enabled() {
        return [None; 4];
    }
    table(|t| {
        let mut e: [Option<&Entry>; 4] = [None; 4];
        for (i, &(x, y)) in xy.iter().enumerate() {
            e[i] = find(t, x, y);
        }
        let mut pick = [0usize; 4];
        let singles: Vec<f32> = e[..n]
            .iter()
            .flatten()
            .filter(|e| e.n == 1)
            .map(|e| e.c[0].z)
            .collect();
        if let Some(m) = (0..n).find(|&i| e[i].is_some_and(|e| e.n > 1)) {
            if !singles.is_empty() {
                // Each corner with several points takes the one nearest the others' mean depth.
                let zref = singles.iter().sum::<f32>() / singles.len() as f32;
                for i in 0..n {
                    if let Some(ei) = e[i].filter(|e| e.n > 1) {
                        pick[i] = nearest(ei, zref);
                    }
                }
            } else {
                // No single-point corner: try each point of the first such corner as the depth,
                // the others nearest to it, and keep the set with the smallest depth spread.
                let em = e[m].unwrap();
                let mut best: Option<(f32, [usize; 4])> = None;
                for j in 0..em.n {
                    let z = em.c[j].z;
                    let mut p = [0usize; 4];
                    p[m] = j;
                    let (mut zmin, mut zmax) = (z, z);
                    for i in (0..n).filter(|&i| i != m) {
                        if let Some(ei) = e[i] {
                            p[i] = nearest(ei, z);
                            let zi = ei.c[p[i]].z;
                            zmin = zmin.min(zi);
                            zmax = zmax.max(zi);
                        }
                    }
                    let cost = zdist(zmin, zmax);
                    if best.is_none_or(|(c, _)| cost < c) {
                        best = Some((cost, p));
                    }
                }
                pick = best.unwrap().1;
            }
        }
        let mut out = [None; 4];
        for i in 0..n {
            out[i] = e[i].map(|e| {
                let c = e.c[pick[i]];
                Precise {
                    x: c.x,
                    y: c.y,
                    z: c.z,
                }
            });
        }
        out
    })
}

/// After each `DrawOTag`: the projections recorded so far are forgotten.
pub fn end_frame() {
    table(|t| {
        t.epoch = t.epoch.wrapping_add(1);
        if t.epoch == 0 {
            t.tab.fill(Entry::default());
            t.epoch = 1;
        }
    });
}

/// Tests that record take this first (the table is global); recording is off again when the
/// guard goes.
#[cfg(test)]
pub(crate) fn serial() -> impl Drop {
    struct Off(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
    impl Drop for Off {
        fn drop(&mut self) {
            set_enabled(false);
        }
    }
    static GATE: Mutex<()> = Mutex::new(());
    Off(GATE.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_polygon_gets_its_corners_back_half_a_pixel_up() {
        let _g = serial();
        set_enabled(true);
        end_frame();
        record(10, 20, 10.25, 20.75, 100.0, 0);
        record(30, 20, 30.5, 20.5, 110.0, 0);
        record(10, 40, 10.9, 40.1, 120.0, 0);
        let p = lookup_poly(&[(10, 20), (30, 20), (10, 40)]);
        assert_eq!(
            p[0],
            Some(Precise {
                x: 9.75,
                y: 20.25,
                z: 100.0
            })
        );
        assert_eq!(p[2].unwrap().z, 120.0);
        // A corner nobody projected has none; the others still do.
        let p = lookup_poly(&[(10, 20), (30, 20), (11, 40)]);
        assert!(p[0].is_some() && p[1].is_some() && p[2].is_none());
        // Gone after the frame.
        end_frame();
        assert_eq!(lookup_poly(&[(10, 20), (30, 20), (10, 40)]), [None; 4]);
    }

    #[test]
    fn far_or_near_or_mismatched_points_are_not_recorded() {
        let _g = serial();
        end_frame();
        record(5, 5, 5.5, 5.5, 50.0, NEAR);
        record(6, 6, 9.0, 6.5, 50.0, 0); // the float isn't this integer's point
        record(7, 7, 900.0, 7.0, 50.0, CLAMPED); // saturated: kept as the true position
        assert!(find_xy(5, 5).is_none());
        assert!(find_xy(6, 6).is_none());
        assert_eq!(find_xy(7, 7).unwrap(), 899.5);
    }

    fn find_xy(x: i32, y: i32) -> Option<f32> {
        table(|t| find(t, x, y).map(|e| e.c[0].x))
    }

    #[test]
    fn corners_with_several_points_agree_in_depth() {
        let _g = serial();
        set_enabled(true);
        end_frame();
        // Corner (0,0) has two points: one at depth 100 (this model), one at 500 (another).
        record(0, 0, 0.2, 0.2, 500.0, 0);
        record(0, 0, 0.7, 0.7, 100.0, 0);
        record(10, 0, 10.5, 0.5, 105.0, 0);
        record(0, 10, 0.5, 10.5, 95.0, 0);
        let p = lookup_poly(&[(0, 0), (10, 0), (0, 10)]);
        assert_eq!(p[0].unwrap().z, 100.0);
        // Every corner ambiguous: the set with the least depth spread.
        end_frame();
        for (k, (x, y)) in [(0, 0), (10, 0), (0, 10)].into_iter().enumerate() {
            let o = k as f64 * 0.1;
            record(x, y, x as f64 + 0.1 + o, y as f64 + 0.1, 500.0, 0);
            record(
                x,
                y,
                x as f64 + 0.6 + o,
                y as f64 + 0.6,
                100.0 + k as f64,
                0,
            );
        }
        let p = lookup_poly(&[(0, 0), (10, 0), (0, 10)]);
        let zs: Vec<f32> = p[..3].iter().map(|c| c.unwrap().z).collect();
        assert!(zs == [500.0; 3] || zs == [100.0, 101.0, 102.0], "{zs:?}");
    }
}

#[cfg(test)]
mod gte_tests {
    use super::*;
    use crate::gte::Gte;

    /// A GTE looking down z with H = 256, the screen centre at (160.5, 120.25).
    fn gte() -> Gte {
        let mut g = Gte::new();
        g.ctc2(0, 0x1000); // RT11 = 1.0, RT12 = 0
        g.ctc2(2, 0x1000); // RT22 = 1.0
        g.ctc2(4, 0x1000); // RT33 = 1.0
        g.ctc2(24, (160 << 16) + 0x8000); // OFX
        g.ctc2(25, (120 << 16) + 0x4000); // OFY
        g.ctc2(26, 256); // H
        g
    }

    fn rtps(g: &mut Gte, x: i16, y: i16, z: i16) -> ([u32; 32], i32, i32) {
        g.mtc2(0, (y as u16 as u32) << 16 | x as u16 as u32);
        g.mtc2(1, z as u16 as u32);
        g.execute(0x0018_0001); // RTPS, sf
        let regs: [u32; 32] = std::array::from_fn(|r| g.mfc2(r));
        let sxy = regs[14];
        (regs, sxy as i16 as i32, (sxy >> 16) as i16 as i32)
    }

    #[test]
    fn rtps_reports_the_float_behind_its_integer_and_changes_nothing() {
        let _g = serial();
        set_enabled(false);
        let off = rtps(&mut gte(), 37, -21, 1000).0;
        set_enabled(true);
        end_frame();
        let (on, sx, sy) = rtps(&mut gte(), 37, -21, 1000);
        set_enabled(false);
        assert_eq!(off, on, "the registers don't depend on the option");
        // 37 * 256 / 1000 + 160.5 = 169.972; -21 * 256 / 1000 + 120.25 = 114.874.
        assert_eq!((sx, sy), (169, 114));
        set_enabled(true);
        end_frame();
        rtps(&mut gte(), 37, -21, 1000);
        let x = table(|t| find(t, sx, sy).map(|e| (e.c[0].x, e.c[0].y, e.c[0].z)));
        set_enabled(false);
        let (x, y, z) = x.unwrap();
        assert!(
            (x - 169.472).abs() < 0.01 && (y - 114.374).abs() < 0.01,
            "{x} {y}"
        );
        assert_eq!(z, 1000.0);
    }
}
