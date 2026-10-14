//! `GsSortSprite` against the real SDK's results, recorded by `oracle/call-sdk.lua` in PCSX-Redux
//! from the requests `tools/oracle/sprite_requests.py` writes: the clock hand at its 60 angles
//! and several positions, 600 random rotations and 100 unrotated sprites.
//!
//! The files are Sony's code's outputs, so they stay local: `req.txt` and `results.txt` under
//! `DW1_LIBGS_SPRITE_CAPTURE` (default `captures/oracle-runs/libgs-sprite`, gitignored). Without
//! them the test shows as ignored.
//!
//! Each request writes a sprite, a 16-entry ordering table and the work base into the emulator's
//! scratch buffer. The test puts the same bytes into the native PS1 RAM stand-in at the same
//! offsets (the table's links are the list end only, which is the same natively), calls the Rust
//! function and compares what the request reads back: the packet area word for word (the link
//! translated to the native encoding), the table, the work base and the sprite struct (unchanged).
//! The GTE is set to what the emulator's menu leaves (OFX = OFY = 0, H = 1000) first; after a
//! rotated sprite the rotation matrix and translation (control registers 0-7) must equal the
//! SDK's. The data registers are scratch and not compared.
//!
//! Strict: every request has a result with its own index and no exception, and every read-back
//! it asked for (packet area, table, work base, sprite, GTE registers) is present with its
//! length; none is skipped.

#[cfg(not(have_libgs_sprite_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/libgs-sprite (or DW1_LIBGS_SPRITE_CAPTURE); local only"]
fn sprite_sort_matches_the_real_sdk() {}

#[cfg(have_libgs_sprite_capture)]
mod oracle {
    use psyq::libgs::{GsGetWorkBase, GsOt, GsSetWorkBase, GsSortSprite, GsSprite};
    use psyq::memory::{LINK_END, dw1_ps1_ram, link_of, ps1_ram_ptr};
    use std::path::PathBuf;

    const BUF: u32 = 0x801E_0000;

    fn dir() -> PathBuf {
        match std::env::var_os("DW1_LIBGS_SPRITE_CAPTURE") {
            Some(d) => PathBuf::from(d),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../captures/oracle-runs/libgs-sprite"),
        }
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    fn word(s: &str) -> u32 {
        u32::from_str_radix(s, 16).unwrap_or_else(|_| panic!("bad number {s}"))
    }

    fn addr(s: &str) -> u32 {
        match s.strip_prefix("B+") {
            Some(off) => BUF + word(off),
            None => word(s),
        }
    }

    fn le32(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
    }

    #[test]
    fn sprite_sort_matches_the_real_sdk() {
        let req = std::fs::read_to_string(dir().join("req.txt")).unwrap();
        let res = std::fs::read_to_string(dir().join("results.txt")).unwrap();
        let reqs: Vec<&str> = req.lines().filter(|l| !l.trim().is_empty()).collect();
        let ress: Vec<&str> = res.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(reqs.len(), ress.len(), "requests vs results");
        assert!(!reqs.is_empty());
        let (mut rotated, mut flat) = (0, 0);
        let mut failures = Vec::new();
        for (n, (q, r)) in reqs.iter().zip(&ress).enumerate() {
            let mut words = q.split_whitespace();
            assert_eq!(words.next(), Some("GsSortSprite"));
            let args: Vec<u32> = (0..4).map(|_| addr(words.next().unwrap())).collect();
            let (sprite_at, ot_at, pri) = (args[0], args[1], args[2] as u16);
            let mut work_base = 0;
            let ram = unsafe { &mut *(&raw mut dw1_ps1_ram).cast::<[u8; 2 << 20]>() };
            for w in words {
                if let Some(v) = w.strip_prefix("in=") {
                    let (a, hex) = v.split_once(':').unwrap();
                    let (a, bytes) = (addr(a), unhex(hex));
                    if a == 0x8013_6FE4 {
                        work_base = le32(&bytes, 0);
                    } else {
                        let o = a as usize & 0x1F_FFFF;
                        ram[o..o + bytes.len()].copy_from_slice(&bytes);
                    }
                }
            }
            let ot = &ram[(ot_at as usize & 0x1F_FFFF)..];
            let mut gs_ot = GsOt {
                length: le32(ot, 0),
                org: ps1_ram_ptr(le32(ot, 1)).cast(),
                offset: le32(ot, 2),
                point: le32(ot, 3),
                tag: ps1_ram_ptr(le32(ot, 4)).cast(),
            };
            GsSetWorkBase(ps1_ram_ptr(work_base));
            // The GTE as the emulator's menu leaves it: OFX = OFY = 0, H = 1000.
            psyq::gte::with(|g| {
                g.ctc2(24, 0);
                g.ctc2(25, 0);
                g.ctc2(26, 1000);
            });
            let sprite_before = ram[(sprite_at as usize & 0x1F_FFFF)..][..36].to_vec();
            unsafe {
                GsSortSprite(ps1_ram_ptr(sprite_at).cast::<GsSprite>(), &mut gs_ot, pri);
            }
            let end = GsGetWorkBase() as usize - ps1_ram_ptr(0x8000_0000) as usize;

            // The real results.
            // Every requested read-back must be present with its length: the packet area (0x80
            // bytes), the table (64), the work base (4), the sprite (36) and the 64 GTE registers.
            let mut seen = [false; 5];
            let mut bad = Vec::new();
            let mut fields = r.split_whitespace();
            assert_eq!(
                fields.next().and_then(|i| i.parse::<usize>().ok()),
                Some(n + 1),
                "result {n}: wrong or missing index"
            );
            let v0 = fields.next().unwrap_or("");
            assert!(
                v0.starts_with("v0=") && !v0.starts_with("v0=EXC"),
                "result {n}: {v0}"
            );
            for w in fields {
                if let Some(g) = w.strip_prefix("gte=") {
                    // The rotation matrix and translation the rotated form leaves in the GTE
                    // (control registers 0-7); the unrotated form leaves the GTE alone, which
                    // the recording can't show (it didn't preset the registers).
                    let regs: Vec<u32> = g.split(',').map(word).collect();
                    assert_eq!(regs.len(), 64, "request {n}: GTE registers");
                    seen[4] = true;
                    let rotated = ram[(sprite_at as usize & 0x1F_FFFF) + 32..][..4] != [0; 4];
                    if rotated {
                        psyq::gte::with(|gte| {
                            for k in 0..8 {
                                if gte.cfc2(k) != regs[32 + k] {
                                    bad.push(format!(
                                        "GTE control {k}: {:08x}, SDK {:08x}",
                                        gte.cfc2(k),
                                        regs[32 + k]
                                    ));
                                }
                            }
                        });
                    }
                    continue;
                }
                if w.starts_with("diff=") {
                    continue;
                }
                let (a, hex) = w.split_once('=').unwrap();
                let (a, want) = (word(a), unhex(hex));
                let o = a as usize & 0x1F_FFFF;
                let mut expected_len = |len: usize, slot: usize| {
                    assert_eq!(want.len(), len, "request {n}: read-back at {a:#x}");
                    seen[slot] = true;
                };
                if a == 0x8013_6FE4 {
                    expected_len(4, 2);
                    let want_end = (le32(&want, 0) & 0x1F_FFFF) as usize;
                    if want_end != end {
                        bad.push(format!("work base {end:#x}, SDK {want_end:#x}"));
                    }
                } else if a == BUF + 0x300 {
                    expected_len(64, 1);
                    // The ordering table: entry `pri` links to the packet, the rest end the list.
                    for i in 0..16 {
                        let w_sdk = le32(&want, i);
                        let w_native = le32(&ram[o..], i);
                        let expect = if w_sdk & LINK_END == LINK_END {
                            w_sdk
                        } else {
                            (w_sdk & 0xFF00_0000)
                                | link_of(ps1_ram_ptr(0x8000_0000 | (w_sdk & LINK_END)))
                        };
                        if w_native != expect {
                            bad.push(format!("OT[{i}] {w_native:08x}, SDK {w_sdk:08x}"));
                        }
                    }
                } else if a == BUF + 0x400 {
                    expected_len(0x80, 0);
                    // The packet area: the first word's link translated, the rest as is, and
                    // the 0xA5 fill past the packet untouched.
                    for i in 0..want.len() / 4 {
                        let w_sdk = le32(&want, i);
                        let w_native = le32(&ram[o..], i);
                        let expect = if i == 0 && w_sdk & LINK_END != LINK_END {
                            (w_sdk & 0xFF00_0000)
                                | link_of(ps1_ram_ptr(0x8000_0000 | (w_sdk & LINK_END)))
                        } else {
                            w_sdk
                        };
                        if w_native != expect {
                            bad.push(format!("packet word {i}: {w_native:08x}, SDK {w_sdk:08x}"));
                        }
                    }
                    if (le32(&want, 0) >> 24) == 9 {
                        rotated += 1;
                    } else {
                        flat += 1;
                    }
                } else if a == sprite_at {
                    expected_len(36, 3);
                    if want != sprite_before || ram[(o)..][..36] != sprite_before[..] {
                        bad.push("sprite struct changed".into());
                    }
                } else {
                    panic!("request {n}: unexpected read-back at {a:#x}");
                }
            }
            assert!(
                seen.iter().all(|&s| s),
                "request {n}: missing read-backs {seen:?}"
            );
            if !bad.is_empty() {
                failures.push(format!(
                    "request {n}: {} differences, first: {}",
                    bad.len(),
                    bad[0]
                ));
            }
        }
        println!(
            "{} of {} requests match ({rotated} rotated, {flat} unrotated)",
            reqs.len() - failures.len(),
            reqs.len()
        );
        assert!(
            failures.is_empty(),
            "{} requests differ:\n{}",
            failures.len(),
            failures[..failures.len().min(8)].join("\n")
        );
    }
}
