//! `SetDrawLoad` against the real SDK's results, recorded by `oracle/call-sdk.lua` in PCSX-Redux
//! from the requests `tools/oracle/drawload_requests.py` writes (160 rectangles: the game's 16 x 1
//! CLUT rows, other sizes up to and past a `DR_LOAD`'s 13 data words, zero and negative sizes).
//!
//! The files are Sony's code's outputs, so they stay local: `req.txt` and `results.txt` under
//! `DW1_LIBGPU_DRAWLOAD_CAPTURE` (default `captures/oracle-runs/p9-render-drawload`, gitignored).
//! Without them the test shows as ignored.
//!
//! Each request fills a `DR_LOAD` and the words after it with 0xA5, writes a `RECT` and calls
//! `SetDrawLoad`; the test does the same natively and compares all 33 words and the `RECT`. The
//! rectangles of more than 12 words, which the SDK answers with a one-word packet and the game
//! never uses, stop the program natively; they are counted, not compared (each is checked to be
//! that one-word packet).

// Byte-to-word reads read better as chunks_exact than as_chunks.
#![allow(clippy::chunks_exact_to_as_chunks)]

#[cfg(not(have_libgpu_drawload_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/p9-render-drawload (or DW1_LIBGPU_DRAWLOAD_CAPTURE); local only"]
fn set_draw_load_matches_the_real_sdk() {}

#[cfg(have_libgpu_drawload_capture)]
#[test]
fn set_draw_load_matches_the_real_sdk() {
    use psyq::libgpu::{DrLoad, Rect, SetDrawLoad};
    use std::path::PathBuf;

    let dir = match std::env::var_os("DW1_LIBGPU_DRAWLOAD_CAPTURE") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../captures/oracle-runs/p9-render-drawload"),
    };
    let unhex = |s: &str| -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    };
    let req = std::fs::read_to_string(dir.join("req.txt")).unwrap();
    let res = std::fs::read_to_string(dir.join("results.txt")).unwrap();
    let reqs: Vec<&str> = req.lines().filter(|l| !l.trim().is_empty()).collect();
    let ress: Vec<&str> = res.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(reqs.len(), ress.len());
    let (mut ok, mut big) = (0, 0);
    for (n, (q, r)) in reqs.iter().zip(&ress).enumerate() {
        let rect_in = unhex(
            q.split_whitespace()
                .nth(5)
                .unwrap()
                .strip_prefix("in=B+40:")
                .unwrap(),
        );
        let mut f = r.split_whitespace();
        assert_eq!(f.next().unwrap(), (n + 1).to_string());
        assert!(
            !f.next().unwrap().contains("EXC"),
            "request {}: the SDK faulted",
            n + 1
        );
        let words = unhex(f.next().unwrap().strip_prefix("801e0100=").unwrap());
        let rect_out = unhex(f.next().unwrap().strip_prefix("801e0040=").unwrap());
        assert_eq!(rect_in, rect_out, "request {}: RECT changed", n + 1);
        let sdk: Vec<u32> = words
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let s16 = |i: usize| i16::from_le_bytes([rect_in[2 * i], rect_in[2 * i + 1]]);
        let rect = Rect {
            x: s16(0),
            y: s16(1),
            w: s16(2),
            h: s16(3),
        };
        if (i32::from(rect.w) * i32::from(rect.h) + 1) >> 1 > 12
            || (i32::from(rect.w) * i32::from(rect.h) + 1) >> 1 < 0
        {
            // The one-word packet (natively a stop).
            assert_eq!(sdk[0], 0x0100_0000, "request {}", n + 1);
            assert!(
                sdk[4..].iter().all(|&w| w == 0xA5A5_A5A5),
                "request {}",
                n + 1
            );
            big += 1;
            continue;
        }
        let mut native = [0xA5A5_A5A5u32; 33];
        unsafe { SetDrawLoad(native.as_mut_ptr().cast::<DrLoad>(), &rect) };
        assert_eq!(
            native.as_slice(),
            sdk.as_slice(),
            "request {}: {rect:?}",
            n + 1
        );
        ok += 1;
    }
    println!(
        "{ok} rectangles match; {big} over 12 words (the SDK's one-word packet; natively a stop)"
    );
}
