//! The native `GsSortOt` against the real SDK, two ways (both local only, gitignored, under
//! `DW1_SORT_CAPTURES`, default `captures/oracle-runs`):
//!
//! - `g2d-sortot`: 13 calls captured in the care-fixture replay's DIGIMON status screen
//!   (`oracle/capture-calls.lua`, `DW1_CAPTURE_STATES=1`; the digivice's partner model goes into
//!   the frame's table). Each `callNNNN_in.bin` is the whole machine at entry, `_out.bin` at
//!   return. The entry state's table links are translated to the native encoding, the call is
//!   made on native `GsOt`s built from the two tables, and every word of the two tables must
//!   equal the return state's (translated the same way); the return value is the destination.
//! - `libgs-sortot`: 262 `call-sdk.lua` requests from `tools/oracle/sortot_requests.py` (tables
//!   of every size, points, offsets, entries going through packets). Requests whose index
//!   `point - offset` falls outside the destination are skipped (the SDK writes outside it, and
//!   so does the native function, but the table read-back can't show that).

#[cfg(not(have_libgs_sortot_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/g2d-sortot and libgs-sortot (or DW1_SORT_CAPTURES); local only"]
fn sortot_matches_the_real_sdk() {}

#[cfg(have_libgs_sortot_capture)]
mod common;

/// Both tests use the one native PS1 RAM stand-in.
#[cfg(have_libgs_sortot_capture)]
static RAM: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(have_libgs_sortot_capture)]
mod capture {
    use super::common::{State, native, ram, ram_word, root, set_ram_word, translate_link};
    use psyq::libgs::{GsOt, GsSortOt};

    fn ot_at(s: &State, a: u32) -> GsOt {
        GsOt {
            length: s.r32(a),
            org: native(s.r32(a + 4)).cast(),
            offset: s.r32(a + 8),
            point: s.r32(a + 12),
            tag: native(s.r32(a + 16)).cast(),
        }
    }

    #[test]
    fn sortot_matches_captured_calls() {
        let _ram = super::RAM.lock().unwrap_or_else(|e| e.into_inner());
        let dir = root().join("g2d-sortot");
        let text = std::fs::read_to_string(dir.join("calls.txt")).unwrap();
        let hex = |s: &str| u32::from_str_radix(s, 16).unwrap();
        let mut n_calls = 0;
        for l in text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        {
            let f: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(f[2], "GsSortOt");
            let (n, src_at, dst_at) = (f[0], hex(f[3]), hex(f[4]));
            let input = State::load(&dir.join(format!("call{n}_in.bin")));
            let output = State::load(&dir.join(format!("call{n}_out.bin")));
            let ram = ram();
            ram.copy_from_slice(&input.ram);
            let tables = [
                (src_at, input.r32(src_at + 4), input.r32(src_at)),
                (dst_at, input.r32(dst_at + 4), input.r32(dst_at)),
            ];
            for &(_, org, length) in &tables {
                for i in 0..1u32 << length {
                    let a = org + 4 * i;
                    set_ram_word(ram, a, translate_link(input.r32(a)));
                }
            }
            let (mut src, mut dst) = (ot_at(&input, src_at), ot_at(&input, dst_at));
            let ret = unsafe { GsSortOt(&mut src, &mut dst) };
            assert!(
                std::ptr::eq(ret, &dst),
                "call {n}: the return value is the destination"
            );
            let after = self::ram().to_vec();
            for &(_, org, length) in &tables {
                for i in 0..1u32 << length {
                    let a = org + 4 * i;
                    assert_eq!(
                        ram_word(&after, a),
                        translate_link(output.r32(a)),
                        "call {n}: entry at {a:#x}"
                    );
                }
            }
            n_calls += 1;
        }
        assert!(n_calls >= 10, "only {n_calls} calls");
    }
}

#[cfg(have_libgs_sortot_capture)]
mod oracle {
    use psyq::libgs::{GsOt, GsSortOt};
    use psyq::memory::{LINK_END, dw1_ps1_ram, link_of, ps1_ram_ptr};
    use std::path::PathBuf;

    const BUF: u32 = 0x801E_0000;

    fn dir() -> PathBuf {
        match std::env::var_os("DW1_SORT_CAPTURES") {
            Some(d) => PathBuf::from(d).join("libgs-sortot"),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../captures/oracle-runs/libgs-sortot"),
        }
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    fn addr(s: &str) -> u32 {
        match s.strip_prefix("B+") {
            Some(off) => BUF + u32::from_str_radix(off, 16).unwrap(),
            None => u32::from_str_radix(s, 16).unwrap(),
        }
    }

    fn le32(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
    }

    fn translate(w: u32) -> u32 {
        if w & LINK_END == LINK_END {
            w
        } else {
            (w & 0xFF00_0000) | link_of(ps1_ram_ptr(0x8000_0000 | (w & LINK_END)))
        }
    }

    fn write_words(ram: &mut [u8], at: u32, words: impl Iterator<Item = u32>) {
        for (i, w) in words.enumerate() {
            let o = (at as usize & 0x1F_FFFF) + 4 * i;
            ram[o..o + 4].copy_from_slice(&w.to_le_bytes());
        }
    }

    #[test]
    fn sortot_matches_the_requests() {
        let _ram = super::RAM.lock().unwrap_or_else(|e| e.into_inner());
        let req = std::fs::read_to_string(dir().join("req.txt")).unwrap();
        let res = std::fs::read_to_string(dir().join("results.txt")).unwrap();
        let reqs: Vec<&str> = req.lines().filter(|l| !l.trim().is_empty()).collect();
        let ress: Vec<&str> = res.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(reqs.len(), ress.len(), "requests vs results");
        assert!(reqs.len() >= 200);
        let (mut compared, mut skipped) = (0, 0);
        for (n, (q, r)) in reqs.iter().zip(&ress).enumerate() {
            let mut words = q.split_whitespace();
            assert_eq!(words.next(), Some("GsSortOt"));
            let args: Vec<u32> = (0..4).map(|_| addr(words.next().unwrap())).collect();
            let ram = unsafe { &mut *(&raw mut dw1_ps1_ram).cast::<[u8; 2 << 20]>() };
            let mut inputs = Vec::new();
            for w in words {
                if let Some(v) = w.strip_prefix("in=") {
                    let (a, hex) = v.split_once(':').unwrap();
                    inputs.push((addr(a), unhex(hex)));
                }
            }
            // The tables first (struct and entries as the request made them).
            for (a, bytes) in &inputs {
                let o = *a as usize & 0x1F_FFFF;
                ram[o..o + bytes.len()].copy_from_slice(bytes);
            }
            let struct_of = |at: u32| {
                let b = &ram[(at as usize & 0x1F_FFFF)..];
                (le32(b, 0), le32(b, 1), le32(b, 2), le32(b, 3), le32(b, 4))
            };
            let (src_s, dst_s) = (struct_of(args[0]), struct_of(args[1]));
            let index = i64::from(src_s.3) - i64::from(dst_s.2);
            if !(0..1 << dst_s.0).contains(&index) {
                skipped += 1;
                continue;
            }
            // Entries of both tables go native (links translated; chained packets stay as the
            // request left them, in native memory they are only ever linked to).
            for &(len, org) in &[(src_s.0, src_s.1), (dst_s.0, dst_s.1)] {
                let words: Vec<u32> = (0..1usize << len)
                    .map(|i| translate(le32(&ram[(org as usize & 0x1F_FFFF)..], i)))
                    .collect();
                write_words(ram, org, words.into_iter());
            }
            let mut src = GsOt {
                length: src_s.0,
                org: ps1_ram_ptr(src_s.1).cast(),
                offset: src_s.2,
                point: src_s.3,
                tag: ps1_ram_ptr(src_s.4).cast(),
            };
            let mut dst = GsOt {
                length: dst_s.0,
                org: ps1_ram_ptr(dst_s.1).cast(),
                offset: dst_s.2,
                point: dst_s.3,
                tag: ps1_ram_ptr(dst_s.4).cast(),
            };
            let ret = unsafe { GsSortOt(&mut src, &mut dst) };
            assert!(std::ptr::eq(ret, &dst), "request {n}: return value");
            let mut got_tables = 0;
            for w in r.split_whitespace().skip(1) {
                if w.starts_with("v0=") {
                    continue;
                }
                let (a, hex) = w.split_once('=').unwrap();
                let (a, want) = (addr(a), unhex(hex));
                if a == BUF + 0x200 || a == BUF + 0x1100 {
                    for i in 0..want.len() / 4 {
                        let got = le32(&ram[(a as usize & 0x1F_FFFF)..], i);
                        assert_eq!(
                            got,
                            translate(le32(&want, i)),
                            "request {n}: entry {i} of {a:#x}"
                        );
                    }
                    got_tables += 1;
                }
            }
            assert_eq!(got_tables, 2, "request {n}: both tables read back");
            compared += 1;
        }
        assert!(
            compared >= 140,
            "only {compared} compared ({skipped} skipped)"
        );
    }
}
