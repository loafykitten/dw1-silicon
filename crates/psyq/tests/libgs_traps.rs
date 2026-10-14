//! Inputs the real libgs faults or reads garbage on stop the program (`trap::unimplemented`)
//! instead of returning something made up. Each case runs in a child process (this test binary
//! again, with `DW1_TRAP_CASE` set) because the stop is an abort.

use psyq::globals::Matrix;
use psyq::libgs::{GsCoordinate2, GsFLight, GsGetLw, GsSetFlatLight};

fn coord(sup: *mut GsCoordinate2) -> GsCoordinate2 {
    let mut c: GsCoordinate2 = unsafe { std::mem::zeroed() };
    c.sup = sup;
    c
}

fn run_case(name: &str) {
    let mut out = Matrix::default();
    match name {
        "screen_super" => {
            // A `super` of SCREEN (1): the real code dereferences it and faults.
            let mut c = coord(std::ptr::without_provenance_mut(1));
            unsafe { GsGetLw(&mut c, &mut out) };
        }
        "deep_chain" => {
            // 33 deep: the real code's own stack of 32 runs over its neighbouring globals.
            let mut chain: Vec<GsCoordinate2> =
                (0..33).map(|_| coord(std::ptr::null_mut())).collect();
            for i in 0..32 {
                let next: *mut GsCoordinate2 = &mut chain[i + 1];
                chain[i].sup = next;
            }
            unsafe { GsGetLw(&mut chain[0], &mut out) };
        }
        "negative_sum" => {
            // x*x + y*y + z*z wraps negative: the real square root reads before its table.
            let l = GsFLight {
                vx: 32768,
                vy: 32768,
                vz: 0,
                r: 1,
                g: 2,
                b: 3,
            };
            unsafe { GsSetFlatLight(0, &l) };
        }
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn unmodelled_inputs_stop_the_program() {
    if let Ok(case) = std::env::var("DW1_TRAP_CASE") {
        run_case(&case);
        return;
    }
    for case in ["screen_super", "deep_chain", "negative_sum"] {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "unmodelled_inputs_stop_the_program",
                "--nocapture",
            ])
            .env("DW1_TRAP_CASE", case)
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && err.contains("unimplemented SDK call"),
            "{case} did not stop the program: {err}"
        );
    }
}
