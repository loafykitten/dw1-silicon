//! The native `GsSortObject4J` (and the `GsTMDfastTG3L`/`TG4L` handlers it calls through
//! `GsFCALL4`) against real calls captured in PCSX-Redux: the S0.7 spike's 48 (`libgs-pin`,
//! `oracle/capture-libgs.lua`) and the Phase 2 runs (`s4j-*`, `oracle/capture-calls.lua` with
//! `DW1_CAPTURE_STATES=1`), all under `DW1_SORT_CAPTURES` (default `captures/oracle-runs`,
//! gitignored). Each `callNN_in.bin` is the whole machine at the function's entry,
//! `callNN_out.bin` at its return (`DW1STATE`).
//!
//! The entry state's PS1 RAM goes into the native PS1 RAM stand-in (`psyq::memory`) at the same
//! offsets, with the pointer-shaped fields translated to the native forms: the ordering table's
//! 24-bit links (`link_of`), the TMD object record's addresses (32-bit encoded pointers), the
//! `GsDOBJ2` and `GsOT` rebuilt as native structs, the work base as a native pointer. The call
//! is then made through the real entry point. Compared with the capture's return state, links
//! translated the same way:
//! - the work base (`GsOUT_PACKET_P`);
//! - every packet written, word for word;
//! - every ordering-table entry;
//! - the scratchpad, and the GTE's 32 control registers (the data registers are working storage that
//!   the real handlers leave in pipeline-dependent states; not compared);
//! - nothing else in RAM changed (the SDK's own stack frame aside).
//!
//! Strict: every call in `calls.txt` must have both state files, and every call is compared.

// Byte-to-word reads of dump files read better as chunks_exact than as_chunks.
#![allow(clippy::chunks_exact_to_as_chunks)]

#[cfg(not(have_libgs_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/{libgs-pin,s4j-*} (or DW1_SORT_CAPTURES); local only"]
fn native_sort_object4j_matches_the_real_sdk() {}

#[cfg(have_libgs_capture)]
mod common;

#[cfg(have_libgs_capture)]
mod capture {
    use super::common::{
        State, load_gte, native, ram, ram_word, root, set_ram_word, translate_link,
    };
    use psyq::globals::GsFCALL4;
    use psyq::libgs::{
        GsDobj2, GsGetWorkBase, GsOt, GsSetWorkBase, GsSortObject4J, GsTMDfastTG3L, GsTMDfastTG4L,
        GsTMDfastTNF4, TmdHandler,
    };
    use psyq::memory::{PS1_RAM_SIZE, dw1_scratchpad, ps1_ram_ptr, ptr32_of};
    use std::path::{Path, PathBuf};

    const GS_OUT_PACKET_P: u32 = 0x8013_6FE4;
    const SCRATCH_BASE: u32 = 0x1F80_0000;

    struct Call {
        index: usize,
        objp: u32,
        otp: u32,
        shift: i32,
        scratch: u32,
    }

    /// A capture directory's calls. `libgs-pin`'s `calls.txt` is `call vblank objp otp shift
    /// scratch ra` (decimal shift, two-digit file names); `capture-calls.lua`'s is `n vblank
    /// name a0 a1 a2 a3 sp ra v0` (four digits).
    fn calls(dir: &Path) -> (Vec<Call>, usize) {
        let text = std::fs::read_to_string(dir.join("calls.txt")).unwrap();
        let new_format = text.contains(" name ");
        let hex = |s: &str| u32::from_str_radix(s, 16).unwrap();
        let calls = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                if new_format {
                    assert_eq!(f[2], "GsSortObject4J", "{l}");
                    Call {
                        index: f[0].parse().unwrap(),
                        objp: hex(f[3]),
                        otp: hex(f[4]),
                        shift: hex(f[5]) as i32,
                        scratch: hex(f[6]),
                    }
                } else {
                    Call {
                        index: f[0].parse().unwrap(),
                        objp: hex(f[2]),
                        otp: hex(f[3]),
                        shift: f[4].parse().unwrap(),
                        scratch: hex(f[5]),
                    }
                }
            })
            .collect();
        (calls, if new_format { 4 } else { 2 })
    }

    /// The region (RAM offsets) of the ordering table's entries.
    fn ot_region(input: &State, otp: u32) -> (u32, u32) {
        let org = input.r32(otp + 4);
        (org, 4u32 << input.r32(otp))
    }

    /// Runs the native `GsSortObject4J` on call `c`'s entry state. Returns the native RAM before
    /// (translated) and after the call.
    fn run(c: &Call, input: &State) -> (Vec<u8>, Vec<u8>) {
        let ram = ram();
        ram.copy_from_slice(&input.ram);
        // The ordering table's links.
        let (org, size) = ot_region(input, c.otp);
        for i in 0..size / 4 {
            let a = org + 4 * i;
            set_ram_word(ram, a, translate_link(input.r32(a)));
        }
        // The object's TMD record: vertex, normal and primitive addresses become encoded
        // pointers (GsMapModelingData's native form).
        let tmd = input.r32(c.objp + 8);
        for field in [0, 2, 4] {
            let a = tmd + 4 * field;
            set_ram_word(ram, a, ptr32_of(native(input.r32(a))));
        }
        let before = ram.to_vec();

        let mut obj = GsDobj2 {
            attribute: input.r32(c.objp),
            coord2: std::ptr::null_mut(),
            tmd: native(tmd).cast(),
            id: input.r32(c.objp + 12),
        };
        let mut ot = GsOt {
            length: input.r32(c.otp),
            org: native(org).cast(),
            offset: input.r32(c.otp + 8),
            point: input.r32(c.otp + 12),
            tag: native(input.r32(c.otp + 16)).cast(),
        };
        // The game installs these in initializeGsTMDMap: tg3[NDIV][NORMAL], tg4[NDIV][NORMAL] and
        // ntf4[NDIV].
        unsafe {
            GsFCALL4[54] = Some(std::mem::transmute::<TmdHandler, unsafe extern "C" fn()>(
                GsTMDfastTNF4,
            ));
            GsFCALL4[24] = Some(std::mem::transmute::<TmdHandler, unsafe extern "C" fn()>(
                GsTMDfastTG3L,
            ));
            GsFCALL4[56] = Some(std::mem::transmute::<TmdHandler, unsafe extern "C" fn()>(
                GsTMDfastTG4L,
            ));
        }
        // The scratchpad, and the work base (GsOUT_PACKET_P).
        unsafe {
            (&raw mut dw1_scratchpad)
                .cast::<u8>()
                .copy_from(input.scratch.as_ptr(), 1024);
        }
        let scratch: *mut u32 = if c.scratch == SCRATCH_BASE {
            (&raw mut dw1_scratchpad).cast()
        } else {
            native(c.scratch).cast()
        };
        GsSetWorkBase(native(input.r32(GS_OUT_PACKET_P)));
        psyq::gte::with(|g| *g = load_gte(input));

        unsafe { GsSortObject4J(&mut obj, &mut ot, c.shift, scratch) };
        (before, self::ram().to_vec())
    }

    /// Compares one call; returns (packet words checked, noise words ignored) or a description.
    fn check(dir: &Path, width: usize, c: &Call) -> Result<(usize, usize), String> {
        let n = c.index;
        let input = State::load(&dir.join(format!("call{n:0width$}_in.bin")));
        let output = State::load(&dir.join(format!("call{n:0width$}_out.bin")));
        let (before, after) = run(c, &input);

        let p_in = input.r32(GS_OUT_PACKET_P) & 0x1F_FFFF;
        let p_out = output.r32(GS_OUT_PACKET_P) & 0x1F_FFFF;
        let (org, size) = ot_region(&input, c.otp);
        let org = org & 0x1F_FFFF;
        let sp = input.gpr[29] & 0x1F_FFFF;
        let mut bad: Vec<String> = Vec::new();

        // The work base.
        let base = GsGetWorkBase() as usize - ps1_ram_ptr(0x8000_0000) as usize;
        if base != p_out as usize {
            bad.push(format!("work base {base:#x}, capture {p_out:#x}"));
        }

        // Packets: each is a tag (length, link) and `length` words.
        let mut a = p_in;
        let mut packet_words = 0;
        while a < p_out {
            let tag = output.r32(a);
            let len = (tag >> 24) as usize;
            if len == 0 || len > 16 {
                bad.push(format!("capture packet at {a:#x} has tag {tag:08x}"));
                break;
            }
            for k in 0..=len as u32 {
                let want = if k == 0 {
                    translate_link(tag)
                } else {
                    output.r32(a + 4 * k)
                };
                let got = ram_word(&after, a + 4 * k);
                packet_words += 1;
                if got != want {
                    bad.push(format!(
                        "packet {a:#x} word {k}: capture {want:08x}, native {got:08x}"
                    ));
                }
            }
            a += 4 + 4 * len as u32;
        }

        // The ordering table, and nothing else.
        let mut noise = 0;
        for i in 0..PS1_RAM_SIZE as u32 / 4 {
            let ad = i * 4;
            let got = ram_word(&after, ad);
            let was = ram_word(&before, ad);
            if (org..org + size).contains(&ad) {
                let want = translate_link(output.r32(ad));
                if got != want {
                    bad.push(format!(
                        "OT {ad:#x}: capture {want:08x}, native {got:08x} (was {was:08x})"
                    ));
                }
            } else if (p_in..p_out).contains(&ad) {
                // Compared above.
            } else if got != was {
                bad.push(format!(
                    "stray write at {ad:#x}: {was:08x} -> {got:08x} (capture {:08x})",
                    output.r32(ad)
                ));
            } else if output.r32(ad) != input.r32(ad) && !(sp - 0x1000..sp).contains(&ad) {
                // The capture changed something that is not libgs's output (a VBlank interrupt
                // ran inside the call: BIOS kernel words, frame counters); native left it alone.
                noise += 1;
            }
        }

        // The scratchpad and the GTE.
        let scratch =
            unsafe { std::slice::from_raw_parts((&raw const dw1_scratchpad).cast::<u8>(), 1024) };
        if scratch != output.scratch.as_slice() {
            bad.push("scratchpad differs".into());
        }
        // Only the control registers are the SDK's contract, FLAG (31) aside, which is the last command's (the game sets them before the call
        // and expects them back). The data registers are working storage: what the real
        // handlers leave there depends on how they pipeline the primitives (the last lit mode
        // in RGBC, SZ/IR scraps), which no later code reads, so they aren't compared.
        psyq::gte::with(|g| {
            for r in 0..32 {
                if r != 31 && g.cfc2(r) != output.cp2c[r] {
                    bad.push(format!(
                        "GTE control {r}: capture {:08x}, native {:08x}",
                        output.cp2c[r],
                        g.cfc2(r)
                    ));
                }
            }
        });

        if bad.is_empty() {
            return Ok((packet_words, noise));
        }
        let mut msg = format!("{} call {n}: {} differences", dir.display(), bad.len());
        for b in bad.iter().take(8) {
            msg += &format!("\n  {b}");
        }
        Err(msg)
    }

    #[test]
    fn native_sort_object4j_matches_the_real_sdk() {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().unwrap().to_string_lossy();
                name == "libgs-pin" || name.starts_with("s4j-")
            })
            .collect();
        dirs.sort();
        assert!(!dirs.is_empty());
        // A run without its calls.txt was interrupted (capture-calls.lua writes it last): fail
        // rather than skip it.
        for dir in &dirs {
            assert!(
                dir.join("calls.txt").exists(),
                "{}: no calls.txt (an interrupted capture?)",
                dir.display()
            );
        }
        let mut failures = Vec::new();
        let (mut total, mut words_ok, mut noise_total) = (0, 0, 0);
        for dir in &dirs {
            let (all, width) = calls(dir);
            assert!(!all.is_empty(), "{}: no calls", dir.display());
            // Strict: a state file pair for every call, and nothing recorded that calls.txt
            // doesn't list.
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
            for c in &all {
                total += 1;
                match check(dir, width, c) {
                    Ok((w, n)) => {
                        words_ok += w;
                        noise_total += n;
                    }
                    Err(m) => failures.push(m),
                }
            }
        }
        println!(
            "{} of {total} calls match in {} captures; {words_ok} packet words; {noise_total} \
             interrupt-side words ignored",
            total - failures.len(),
            dirs.len()
        );
        assert!(
            failures.is_empty(),
            "{} calls differ:\n{}",
            failures.len(),
            failures[..failures.len().min(6)].join("\n")
        );
    }
}
