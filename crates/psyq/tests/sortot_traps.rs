//! `GsSortOt` stops the program when the entry index `point - offset` falls outside the
//! destination table (the SDK writes outside it, which the native function can't do safely)
//! instead of writing there. Each case runs in a child process (this test binary again, with
//! `DW1_TRAP_CASE` set) because the stop is an abort.

use psyq::libgs::{GsOt, GsSortOt};

static mut SRC_ENTRIES: [u32; 2] = [0x00FF_FFFF; 2];
static mut DST_ENTRIES: [u32; 4] = [0x00FF_FFFF; 4];

fn run_case(name: &str) {
    unsafe {
        let mut src = GsOt {
            length: 1,
            org: (&raw mut SRC_ENTRIES).cast(),
            offset: 0,
            point: 0,
            tag: (&raw mut SRC_ENTRIES).cast::<u32>().add(1),
        };
        let mut dst = GsOt {
            length: 2,
            org: (&raw mut DST_ENTRIES).cast(),
            offset: 0,
            point: 0,
            tag: (&raw mut DST_ENTRIES).cast::<u32>().add(3),
        };
        match name {
            // The first entry past the table.
            "past_the_end" => src.point = 4,
            // `point` below `offset`: an entry before the table.
            "before_the_start" => {
                src.point = 0;
                dst.offset = 1;
            }
            // Both inside: not a trap.
            "inside" => src.point = 3,
            other => panic!("unknown case {other}"),
        }
        GsSortOt(&mut src, &mut dst);
        if name == "inside" {
            // Entry 3 now links to the source's top entry, and the source's entry 0 took the
            // old word.
            assert_eq!(
                (&raw const DST_ENTRIES).cast::<u32>().add(3).read() & 0xFF00_0000,
                0
            );
            assert_eq!((&raw const SRC_ENTRIES).cast::<u32>().read(), 0x00FF_FFFF);
        }
    }
}

#[test]
fn an_index_outside_the_destination_stops_the_program() {
    if let Ok(case) = std::env::var("DW1_TRAP_CASE") {
        run_case(&case);
        return;
    }
    let child = |case: &str| {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "an_index_outside_the_destination_stops_the_program",
                "--nocapture",
            ])
            .env("DW1_TRAP_CASE", case)
            .output()
            .unwrap()
    };
    for case in ["past_the_end", "before_the_start"] {
        let out = child(case);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && err.contains("unimplemented SDK call `GsSortOt"),
            "{case} did not stop the program: {err}"
        );
    }
    let out = child("inside");
    assert!(
        out.status.success(),
        "an index inside the table must not stop: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
