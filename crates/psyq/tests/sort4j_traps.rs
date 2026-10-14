//! `GsSortObject4J` and its handlers stop the program on what the captures never showed (other
//! mode or flag bits in a handler's slot, indices past an object's lists) instead of drawing
//! something made up. Each case runs in a child process (this test binary again, with
//! `DW1_TRAP_CASE` set) because the stop is an abort.

use psyq::libgs::{GsDobj2, GsOt, GsSortObject4J, GsTMDfastTG3L, Vert};
use psyq::memory::ptr32_of;

/// The object's data: in the program's data segment, like the game's (32-bit pointers to it
/// encode as offsets in that window).
static mut VERTS: [Vert; 2] = [Vert {
    vx: 0,
    vy: 0,
    vz: 0,
    pad: 0,
}; 2];
static mut PRIM: [u32; 7] = [0; 7];
static mut RECORD: [u32; 7] = [0; 7];
static mut PACKETS: [u32; 64] = [0; 64];
static mut OT_ENTRIES: [u32; 4] = [0; 4];

/// A lit Gouraud textured triangle header (mode, flag, 6 input words, 9 output words) with
/// (normal, vertex) index pairs `pairs`.
fn triangle(mode: u32, flag: u32, pairs: [(u16, u16); 3]) -> [u32; 7] {
    let mut p = [0u32; 7];
    p[0] = mode << 24 | flag << 16 | 6 << 8 | 9;
    for (k, (normal, vertex)) in pairs.into_iter().enumerate() {
        p[4 + k] = u32::from(normal) | u32::from(vertex) << 16;
    }
    p
}

fn run_case(name: &str) {
    unsafe {
        let vp = (&raw const VERTS).cast::<Vert>();
        let mut ot = GsOt {
            length: 2,
            org: (&raw mut OT_ENTRIES).cast(),
            offset: 0,
            point: 0,
            tag: std::ptr::null_mut(),
        };
        let pk = (&raw mut PACKETS).cast::<u32>();
        match name {
            "semi_mode" | "two_sided" => {
                // The handler's slot is shared with semi-transparent (mode 0x36) and two-sided
                // (flag 2) triangles; neither was ever captured.
                let (mode, flag) = if name == "semi_mode" {
                    (0x36, 0)
                } else {
                    (0x34, 2)
                };
                PRIM = triangle(mode, flag, [(0, 0), (0, 1), (1, 0)]);
                GsTMDfastTG3L((&raw const PRIM).cast(), vp, vp, pk, 1, 0, &mut ot, pk);
            }
            "bad_index" => {
                // Two vertices, two normals; the triangle uses vertex 2.
                PRIM = triangle(0x34, 0, [(0, 0), (1, 1), (0, 2)]);
                RECORD = [
                    ptr32_of(vp.cast()),
                    2,
                    ptr32_of(vp.cast()),
                    2,
                    ptr32_of((&raw const PRIM).cast()),
                    1,
                    0,
                ];
                // Lit Gouraud textured triangles' slot (div 0, normal light).
                psyq::globals::GsFCALL4[24] = Some(std::mem::transmute::<
                    unsafe extern "C" fn(
                        *const u32,
                        *const Vert,
                        *const Vert,
                        *mut u32,
                        i32,
                        i32,
                        *mut GsOt,
                        *mut u32,
                    ) -> *mut u32,
                    unsafe extern "C" fn(),
                >(GsTMDfastTG3L));
                psyq::libgs::GsSetWorkBase(pk.cast());
                let mut obj = GsDobj2 {
                    attribute: 0,
                    coord2: std::ptr::null_mut(),
                    tmd: (&raw mut RECORD).cast(),
                    id: 0,
                };
                GsSortObject4J(&mut obj, &mut ot, 0, pk);
            }
            other => panic!("unknown case {other}"),
        }
    }
}

#[test]
fn unobserved_primitives_stop_the_program() {
    if let Ok(case) = std::env::var("DW1_TRAP_CASE") {
        run_case(&case);
        return;
    }
    for case in ["semi_mode", "two_sided", "bad_index"] {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "unobserved_primitives_stop_the_program",
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
