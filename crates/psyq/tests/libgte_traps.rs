//! Inputs the real libgte faults or reads garbage on stop the program (`trap::unimplemented`)
//! instead of returning something made up. Each case runs in a child process (this test binary
//! again, with `DW1_TRAP_CASE` set) because the stop is an abort.

use psyq::globals::Matrix;
use psyq::libgs::{GsRview2, GsSetRefView2};
use psyq::libgte::{CompMatrix, SquareRoot0};

fn run_case(name: &str) {
    match name {
        "sqrt_negative" => {
            SquareRoot0(-1);
        }
        "view_overflow" => {
            // Squared lengths past 2^31: the real function takes SquareRoot0 of a negative.
            let mut v = GsRview2 {
                vpx: 0,
                vpy: 0,
                vpz: 0,
                vrx: 32767,
                vry: 32767,
                vrz: 32767,
                rz: 0,
                sup: std::ptr::null_mut(),
            };
            unsafe { GsSetRefView2(&mut v) };
        }
        "comp_overflow" => {
            let a = Matrix {
                m: [[0x1000, 0, 0], [0, 0x1000, 0], [0, 0, 0x1000]],
                t: [i32::MAX, 0, 0],
            };
            let mut b = a;
            b.t = [100, 0, 0];
            let mut out = Matrix::default();
            unsafe { CompMatrix(&a, &b, &mut out) };
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
    for case in ["sqrt_negative", "view_overflow", "comp_overflow"] {
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
    // And the neighbour that is fine still returns.
    assert_eq!(SquareRoot0(0), 0);
}
