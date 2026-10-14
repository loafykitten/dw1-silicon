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
    use super::common::{State, native, ram, ram_word, root, set_ram_word};
    use psyq::libgs::{GsOt, GsSortOt};
    use psyq::memory::LINK_END;

    /// A captured link in the native encoding; the SDK's own empty packets, where an emptied
    /// table's entry 0 points on the PS1, are the end of the list natively.
    fn translate_link(w: u32) -> u32 {
        if w & LINK_END == 0x116BF8 {
            (w & 0xFF00_0000) | LINK_END
        } else {
            super::common::translate_link(w)
        }
    }

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
    /// Where an emptied table's chain ends on the PS1 (the SDK's own empty packets).
    const SENTINEL: u32 = 0x116BF8;

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

    /// A link word in the native encoding; the sentinel and the end marker both end the list.
    fn translate(w: u32) -> u32 {
        let link = w & LINK_END;
        if link == LINK_END || link == SENTINEL {
            (w & 0xFF00_0000) | LINK_END
        } else {
            (w & 0xFF00_0000) | link_of(ps1_ram_ptr(0x8000_0000 | link))
        }
    }

    fn ram() -> &'static mut [u8; 2 << 20] {
        unsafe { &mut *(&raw mut dw1_ps1_ram).cast::<[u8; 2 << 20]>() }
    }

    fn word_at(ram: &[u8; 2 << 20], a: u32) -> u32 {
        le32(&ram[(a as usize & 0x1F_FFFF)..], 0)
    }

    /// In a region a request reads back, every word of a table is a link; a packet region is
    /// four-word packets whose first word is the link.
    fn is_link_word(at: u32, i: usize) -> bool {
        at < BUF + 0x2000 || i.is_multiple_of(4)
    }

    #[test]
    fn sortot_matches_the_requests() {
        let _ram = super::RAM.lock().unwrap_or_else(|e| e.into_inner());
        let req = std::fs::read_to_string(dir().join("req.txt")).unwrap();
        let res = std::fs::read_to_string(dir().join("results.txt")).unwrap();
        let reqs: Vec<&str> = req.lines().filter(|l| !l.trim().is_empty()).collect();
        let ress: Vec<&str> = res.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(reqs.len(), ress.len(), "requests vs results");
        assert!(reqs.len() >= 300);
        let (mut compared, mut skipped, mut end_marked, mut with_packets) = (0, 0, 0, 0);
        for (n, (q, r)) in reqs.iter().zip(&ress).enumerate() {
            let mut words = q.split_whitespace();
            assert_eq!(words.next(), Some("GsSortOt"));
            let args: Vec<u32> = (0..4).map(|_| addr(words.next().unwrap())).collect();
            let ram = ram();
            // The inputs from the tables on: (address, number of words).
            let mut regions = Vec::new();
            for w in words {
                if let Some(v) = w.strip_prefix("in=") {
                    let (a, hex) = v.split_once(':').unwrap();
                    let (a, bytes) = (addr(a), unhex(hex));
                    let o = a as usize & 0x1F_FFFF;
                    ram[o..o + bytes.len()].copy_from_slice(&bytes);
                    if a >= BUF + 0x200 && a != BUF + 0x1000 {
                        regions.push((a, bytes.len() / 4));
                    }
                }
            }
            let struct_of = |at: u32| {
                let b = &ram[(at as usize & 0x1F_FFFF)..];
                (le32(b, 0), le32(b, 1), le32(b, 2), le32(b, 3), le32(b, 4))
            };
            let (src_s, dst_s) = (struct_of(args[0]), struct_of(args[1]));
            let index = i64::from(src_s.3) - i64::from(dst_s.2);
            if !(0..1 << dst_s.0).contains(&index) {
                skipped += 1; // out of range: the native function stops instead
                continue;
            }
            // Follow the source's chain from entry 0 as the request left it.
            let (mut at, mut steps) = (src_s.1, 0);
            let end = loop {
                let link = word_at(ram, at) & LINK_END;
                if link == LINK_END || link == SENTINEL {
                    break link;
                }
                at = 0x8000_0000 | link;
                steps += 1;
                assert!(steps < 64, "request {n}: the chain doesn't end");
            };
            if end == LINK_END {
                // The SDK overwrites entry 0 itself when the chain ends in the 0xFFFFFF marker,
                // whatever hangs off it; the game never has that, and natively it isn't copied.
                end_marked += 1;
                continue;
            }
            if steps > 0 {
                with_packets += 1;
            }
            // Everything the request wrote goes native: link words translated.
            for &(a, count) in &regions {
                for i in 0..count {
                    if is_link_word(a, i) {
                        let o = (a as usize & 0x1F_FFFF) + 4 * i;
                        let w = translate(le32(&ram[o..], 0));
                        ram[o..o + 4].copy_from_slice(&w.to_le_bytes());
                    }
                }
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
            let mut got_regions = 0;
            for w in r.split_whitespace().skip(1) {
                if w.starts_with("v0=") {
                    continue;
                }
                let (a, hex) = w.split_once('=').unwrap();
                let (a, want) = (addr(a), unhex(hex));
                if a < BUF + 0x200 || a == BUF + 0x1000 {
                    continue; // the two structs are unchanged
                }
                for i in 0..want.len() / 4 {
                    let got = word_at(ram, a + 4 * i as u32);
                    let expect = if is_link_word(a, i) {
                        translate(le32(&want, i))
                    } else {
                        le32(&want, i)
                    };
                    assert_eq!(got, expect, "request {n}: word {i} of {a:#x}");
                }
                got_regions += 1;
            }
            assert!(got_regions >= 2, "request {n}: tables read back");
            compared += 1;
        }
        assert!(
            compared >= 140 && with_packets >= 40,
            "{compared} compared, {with_packets} with packets in bucket 0 ({skipped} out of range, {end_marked} end-marked)"
        );
    }
}
