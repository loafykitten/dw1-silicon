//! The native `GsSortBoxFill`, `GsSortPoly` and `GsSortSprite` against real calls captured in PCSX-Redux
//! (`g2d-box-*` and `g2d-spr` under `DW1_SORT_CAPTURES`, default `captures/oracle-runs`, gitignored; made with
//! `oracle/capture-calls.lua` and `DW1_CAPTURE_STATES=1`, see `docs/phase2-plan.md`, slice 6f).
//! Each `callNNNN_in.bin` is the whole machine at the function's entry, `callNNNN_out.bin` at its
//! return.
//!
//! The entry state's PS1 RAM goes into the native PS1 RAM stand-in (`psyq::memory`) with the
//! ordering table's links translated to the native 24-bit offsets; the `GsOT` is rebuilt as a
//! native struct and the work base set from `GsOUT_PACKET_P`. The call is made through the real
//! entry point with the argument structs read in place (they hold no pointers). Compared with the
//! return state, links translated the same way:
//! - the work base;
//! - every packet written, word for word;
//! - every ordering-table entry;
//! - the scratchpad and the GTE's control registers;
//! - nothing else in RAM changed (the SDK's own stack frame aside; native writes none; changes
//!   the capture shows elsewhere are a VBlank interrupt's, counted and ignored).
//!
//! The return value is not compared: all three functions return `void` (`v0` in `calls.txt` is
//! whatever the SDK's last instruction left there).
//!
//! Strict: every call in `calls.txt` has both state files and is compared, only these
//! functions occur, and each occurs.

#![allow(clippy::chunks_exact_to_as_chunks)]

#[cfg(not(have_libgs_2d_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/g2d-box-* (or DW1_SORT_CAPTURES); local only"]
fn native_2d_sorts_match_the_real_sdk() {}

#[cfg(have_libgs_2d_capture)]
mod common;

#[cfg(have_libgs_2d_capture)]
mod capture {
    use super::common::{
        State, load_gte, native, ram, ram_word, root, set_ram_word, translate_link,
    };
    use psyq::libgs::{
        GsBoxf, GsGetWorkBase, GsOt, GsSetWorkBase, GsSortBoxFill, GsSortPoly, GsSortSprite,
        GsSprite,
    };
    use psyq::memory::{PS1_RAM_SIZE, dw1_scratchpad, ps1_ram_ptr};
    use std::path::{Path, PathBuf};

    const GS_OUT_PACKET_P: u32 = 0x8013_6FE4;

    struct Call {
        index: usize,
        name: String,
        a0: u32,
        otp: u32,
        pri: u16,
        sp: u32,
    }

    /// `capture-calls.lua`'s `calls.txt`: `n vblank name a0 a1 a2 a3 sp ra v0`.
    fn calls(dir: &Path) -> Vec<Call> {
        let text = std::fs::read_to_string(dir.join("calls.txt")).unwrap();
        let hex = |s: &str| u32::from_str_radix(s, 16).unwrap();
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                assert!(
                    f.len() == 10 && f[9] != "-",
                    "calls.txt: call without a return: {l}"
                );
                Call {
                    index: f[0].parse().unwrap(),
                    name: f[2].to_string(),
                    a0: hex(f[3]),
                    otp: hex(f[4]),
                    pri: hex(f[5]) as u16,
                    sp: hex(f[7]),
                }
            })
            .collect()
    }

    /// Runs the native function on the call's entry state; returns the native RAM before
    /// (translated) and after.
    fn run(c: &Call, input: &State) -> (Vec<u8>, Vec<u8>) {
        let ram = ram();
        ram.copy_from_slice(&input.ram);
        let org = input.r32(c.otp + 4);
        for i in 0..(4u32 << input.r32(c.otp)) / 4 {
            let a = org + 4 * i;
            set_ram_word(ram, a, translate_link(input.r32(a)));
        }
        let before = ram.to_vec();
        let mut ot = GsOt {
            length: input.r32(c.otp),
            org: native(org).cast(),
            offset: input.r32(c.otp + 8),
            point: input.r32(c.otp + 12),
            tag: native(input.r32(c.otp + 16)).cast(),
        };
        unsafe {
            (&raw mut dw1_scratchpad)
                .cast::<u8>()
                .copy_from(input.scratch.as_ptr(), 1024);
        }
        GsSetWorkBase(native(input.r32(GS_OUT_PACKET_P)));
        psyq::gte::with(|g| *g = load_gte(input));
        unsafe {
            match c.name.as_str() {
                "GsSortBoxFill" => {
                    GsSortBoxFill(native(c.a0).cast::<GsBoxf>(), &mut ot, c.pri);
                }
                "GsSortPoly" => GsSortPoly(native(c.a0).cast::<u32>(), &mut ot, c.pri),
                "GsSortSprite" => GsSortSprite(native(c.a0).cast::<GsSprite>(), &mut ot, c.pri),
                other => panic!("call {}: unexpected function {other}", c.index),
            }
        }
        (before, self::ram().to_vec())
    }

    fn check(dir: &Path, c: &Call) -> Result<(usize, usize), String> {
        let n = c.index;
        let input = State::load(&dir.join(format!("call{n:04}_in.bin")));
        let output = State::load(&dir.join(format!("call{n:04}_out.bin")));
        let (before, after) = run(c, &input);

        let p_in = input.r32(GS_OUT_PACKET_P) & 0x1F_FFFF;
        let p_out = output.r32(GS_OUT_PACKET_P) & 0x1F_FFFF;
        let org = input.r32(c.otp + 4) & 0x1F_FFFF;
        let size = 4u32 << input.r32(c.otp);
        let sp = c.sp & 0x1F_FFFF;
        let mut bad: Vec<String> = Vec::new();

        let base = GsGetWorkBase() as usize - ps1_ram_ptr(0x8000_0000) as usize;
        if base != p_out as usize {
            bad.push(format!("work base {base:#x}, capture {p_out:#x}"));
        }
        if p_out <= p_in {
            bad.push("the capture wrote no packet".into());
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
                let mut got = ram_word(&after, a + 4 * k);
                let mut want = want;
                // The POLY_FT4 a game builds with `SetPolyFT4` leaves its two pad halfwords (the
                // high halves of words 7 and 9) unset; the GPU ignores them and `GsSortPoly`
                // zeroes them, so they are masked out of the comparison for that call only.
                if c.name == "GsSortPoly" && len == 9 && (k == 7 || k == 9) {
                    got &= 0xFFFF;
                    want &= 0xFFFF;
                }
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
            } else if output.r32(ad) != input.r32(ad)
                && !(sp - 0x1000..sp).contains(&ad)
                && ad != GS_OUT_PACKET_P & 0x1F_FFFF
            {
                // The capture changed something that is not libgs's output: a VBlank interrupt
                // ran inside the call (BIOS kernel words, frame counters); native left it alone.
                noise += 1;
            }
        }

        let scratch =
            unsafe { std::slice::from_raw_parts((&raw const dw1_scratchpad).cast::<u8>(), 1024) };
        if scratch != output.scratch.as_slice() {
            bad.push("scratchpad differs".into());
        }
        psyq::gte::with(|g| {
            for r in 0..32 {
                if g.cfc2(r) != output.cp2c[r] {
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
    fn native_2d_sorts_match_the_real_sdk() {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("g2d-box-")
                    || p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("g2d-spr")
                        && p.join("calls.txt").exists()
            })
            .collect();
        dirs.sort();
        assert!(!dirs.is_empty());
        let mut failures = Vec::new();
        let (mut total, mut words_ok, mut noise_total) = (0, 0, 0);
        let (mut boxes, mut polys, mut sprites) = (0, 0, 0);
        for dir in &dirs {
            let all = calls(dir);
            assert!(!all.is_empty(), "{}: no calls", dir.display());
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
                match c.name.as_str() {
                    "GsSortBoxFill" => boxes += 1,
                    "GsSortPoly" => polys += 1,
                    "GsSortSprite" => sprites += 1,
                    other => panic!("{}: unexpected function {other}", dir.display()),
                }
                match check(dir, c) {
                    Ok((w, n)) => {
                        words_ok += w;
                        noise_total += n;
                    }
                    Err(m) => failures.push(m),
                }
            }
        }
        assert!(boxes > 0 && polys > 0 && sprites > 0);
        println!(
            "{} of {total} calls match in {} captures ({boxes} GsSortBoxFill, {polys} \
             GsSortPoly, {sprites} GsSortSprite); {words_ok} packet words; {noise_total} interrupt-side words ignored",
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
