//! The native `OpenTIM`, `ReadTIM` and `LoadClut` against real calls captured in PCSX-Redux
//! (`g2d-tim` and `g2d-clut` under `DW1_SORT_CAPTURES`, default `captures/oracle-runs`,
//! gitignored; made with `oracle/capture-calls.lua`, `DW1_CAPTURE_STATES=1`, File City's map
//! load at Redux VBlanks 12484 and 12745).
//!
//! - `OpenTIM`/`ReadTIM`: the entry state's PS1 RAM goes into the native PS1 RAM stand-in. For a
//!   `ReadTIM` call the library's cursor is first set from the SDK's own static (the word the
//!   preceding `OpenTIM`, or `ReadTIM`, left), then `ReadTIM` runs on a native `TIM_IMAGE`. The
//!   five fields must equal the captured ones (PS1 addresses translated to native pointers), the
//!   return value must be the argument, and the cursor must end where the SDK's static ended.
//!   `OpenTIM` must return 0 and set the cursor to its argument.
//! - `LoadClut`: the VRAM dump at the call's VBlank is loaded into the native GPU, the calls of
//!   that VBlank run in order on their entry states' RAM, and the VRAM rows they touch (and
//!   their neighbours) must equal the dump of the next VBlank. Return values (the CLUT ids) must
//!   match.
//!
//! Strict: every call in `calls.txt` has both state files and is compared.

#![allow(clippy::chunks_exact_to_as_chunks)]

#[cfg(not(have_libgpu_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/g2d-{tim,clut} (or DW1_SORT_CAPTURES); local only"]
fn native_tim_and_clut_calls_match_the_real_sdk() {}

#[cfg(have_libgpu_capture)]
mod common;

#[cfg(have_libgpu_capture)]
mod capture {
    use super::common::{State, native, ram, root};
    use psyq::libgpu::{LoadClut, OpenTIM, ReadTIM, TimImage, tim_cursor};
    use std::path::Path;

    /// libgpu's cursor static in the SDK's data (`ReadTIM`'s next TIM).
    const SDK_TIM_CURSOR: u32 = 0x8013_6DB4;

    struct Call {
        index: usize,
        vblank: u32,
        name: String,
        a0: u32,
        a1: u32,
        a2: u32,
        v0: u32,
    }

    fn calls(dir: &Path) -> Vec<Call> {
        let text = std::fs::read_to_string(dir.join("calls.txt")).unwrap();
        let hex = |s: &str| u32::from_str_radix(s, 16).unwrap();
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                assert!(
                    f.len() >= 10 && f[9] != "-",
                    "calls.txt: call without a return: {l}"
                );
                Call {
                    index: f[0].parse().unwrap(),
                    vblank: f[1].parse().unwrap(),
                    name: f[2].to_string(),
                    a0: hex(f[3]),
                    a1: hex(f[4]),
                    a2: hex(f[5]),
                    v0: hex(f[9]),
                }
            })
            .collect()
    }

    fn states(dir: &Path, n: usize) -> (State, State) {
        (
            State::load(&dir.join(format!("call{n:04}_in.bin"))),
            State::load(&dir.join(format!("call{n:04}_out.bin"))),
        )
    }

    fn check_tim(dir: &Path, c: &Call) -> Result<(), String> {
        let (input, output) = states(dir, c.index);
        ram().copy_from_slice(&input.ram);
        let addr_of = |a: u32| native(a) as usize;
        match c.name.as_str() {
            "OpenTIM" => {
                let r = unsafe { OpenTIM(native(c.a0).cast()) };
                if r != c.v0 as i32 || tim_cursor() != addr_of(c.a0) {
                    return Err(format!("OpenTIM {}: returned {r}, cursor wrong", c.index));
                }
            }
            "ReadTIM" => {
                // The SDK's static was set by the earlier OpenTIM or ReadTIM.
                let before = input.r32(SDK_TIM_CURSOR);
                unsafe { OpenTIM(native(before).cast()) };
                let mut img = TimImage {
                    mode: 0,
                    crect: std::ptr::null_mut(),
                    caddr: std::ptr::null_mut(),
                    prect: std::ptr::null_mut(),
                    paddr: std::ptr::null_mut(),
                };
                let r = unsafe { ReadTIM(&mut img) };
                let want = [
                    output.r32(c.a0),
                    output.r32(c.a0 + 4),
                    output.r32(c.a0 + 8),
                    output.r32(c.a0 + 12),
                    output.r32(c.a0 + 16),
                ];
                let ptrs = [
                    img.crect as usize,
                    img.caddr as usize,
                    img.prect as usize,
                    img.paddr as usize,
                ];
                let wptrs = [
                    addr_of(want[1]),
                    addr_of(want[2]),
                    addr_of(want[3]),
                    addr_of(want[4]),
                ];
                if img.mode != want[0] || ptrs != wptrs {
                    return Err(format!(
                        "ReadTIM {}: mode {:#x}/{:#x}, pointers {ptrs:x?} vs {wptrs:x?}",
                        c.index, img.mode, want[0]
                    ));
                }
                if !std::ptr::eq(r, &img) {
                    return Err(format!("ReadTIM {}: wrong return value", c.index));
                }
                if tim_cursor() != addr_of(output.r32(SDK_TIM_CURSOR)) {
                    return Err(format!("ReadTIM {}: cursor ended elsewhere", c.index));
                }
            }
            other => panic!("g2d-tim: unexpected function {other}"),
        }
        Ok(())
    }

    fn vram_dump(dir: &Path, vblank: u32) -> Vec<u16> {
        let b = std::fs::read(dir.join(format!("vram_{vblank:05}.bin"))).unwrap();
        b.chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    fn check_clut(dir: &Path, all: &[Call]) -> Result<usize, String> {
        let mut done = 0;
        let mut i = 0;
        while i < all.len() {
            let v = all[i].vblank;
            let group: Vec<&Call> = all[i..].iter().take_while(|c| c.vblank == v).collect();
            i += group.len();
            let before = vram_dump(dir, v);
            let after = vram_dump(dir, v + 1);
            psyq::gpu::with(|g| g.vram_mut().copy_from_slice(&before));
            let mut rows = std::collections::BTreeSet::new();
            for c in &group {
                if c.name != "LoadClut" {
                    panic!("g2d-clut: unexpected function {}", c.name);
                }
                let (input, _) = states(dir, c.index);
                ram().copy_from_slice(&input.ram);
                let id = unsafe { LoadClut(native(c.a0).cast(), c.a1 as i32, c.a2 as i32) };
                if u32::from(id) != c.v0 {
                    return Err(format!(
                        "LoadClut {}: id {id:#x}, capture {:#x}",
                        c.index, c.v0
                    ));
                }
                rows.insert(c.a2 as usize);
                done += 1;
            }
            let now = psyq::gpu::with(|g| g.vram().to_vec());
            for &y in &rows {
                for yy in y.saturating_sub(2)..y + 3 {
                    let r = yy * 1024..(yy + 1) * 1024;
                    if now[r.clone()] != after[r] {
                        return Err(format!("LoadClut group at {v}: VRAM row {yy:#x} differs"));
                    }
                }
            }
        }
        Ok(done)
    }

    #[test]
    fn native_tim_and_clut_calls_match_the_real_sdk() {
        let tim = root().join("g2d-tim");
        let clut = root().join("g2d-clut");
        let mut failures = Vec::new();
        let tim_calls = calls(&tim);
        let clut_calls = calls(&clut);
        assert!(!tim_calls.is_empty() && !clut_calls.is_empty());
        for (dir, all) in [(&tim, &tim_calls), (&clut, &clut_calls)] {
            let files = std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("call") && n.ends_with("_in.bin")
                })
                .count();
            assert_eq!(
                files,
                all.len(),
                "{}: state files vs calls.txt",
                dir.display()
            );
        }
        for c in &tim_calls {
            if let Err(m) = check_tim(&tim, c) {
                failures.push(m);
            }
        }
        match check_clut(&clut, &clut_calls) {
            Ok(n) => assert_eq!(n, clut_calls.len()),
            Err(m) => failures.push(m),
        }
        println!(
            "{} TIM calls and {} LoadClut calls; {} failures",
            tim_calls.len(),
            clut_calls.len(),
            failures.len()
        );
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
