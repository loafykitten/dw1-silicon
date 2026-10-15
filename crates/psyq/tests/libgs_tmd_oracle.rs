//! `GsSortObject4J` and the TMD primitive handlers the game installs (`initializeGsTMDMap`)
//! against the real SDK's results on synthetic objects, recorded by `oracle/call-sdk.lua` (with
//! `DW1_RESTORE=1`) in PCSX-Redux from the requests `tools/oracle/tmd_requests.py` writes:
//! - `--set fast`: every primitive kind the disc's TMDs use (lit flat F3/F4, lit flat textured
//!   FT3/FT4, lit Gouraud G4, lit Gouraud textured GT3/GT4, unlit flat textured FT3/FT4, unlit
//!   Gouraud textured GT3/GT4, with and without the semi-transparency bit) mixed in random objects
//!   under random matrices, lighting, positions (behind the eye, off screen, far), ordering-table
//!   sizes, shifts and `GsOT` offsets;
//! - `--set div`: attribute 0x200 (`GsDIV1`, the arenas' floors) on the unlit textured kinds;
//! - `--set attr`: display off (bit 31) with other bits;
//! - `--set bits`: each kind with and without the semi-transparency bit.
//!
//! The files are Sony's code's outputs, so they stay local: `req.txt` and `results.txt` under
//! `captures/oracle-runs/p9-render-s4j-{fast,div,attr,bits}` (gitignored; `DW1_LIBGS_TMD_CAPTURES`
//! names another root). Without them the tests show as ignored.
//!
//! Each request writes an object (its `GsDOBJ2`, record, vertices, normals and linked
//! primitives), a `GsOT` and its entries (cleared as `GsClearOt` does), the work base over a
//! filled packet area and, for some, the scratchpad into the emulator's memory, presets all 64 GTE
//! registers and calls `GsSortObject4J(obj, ot, shift, 0x1F800000)`. The test puts the same bytes
//! into the native PS1 RAM stand-in at the same offsets, with the pointer-shaped fields in their
//! native forms (the record's three addresses as 32-bit encoded pointers, the `GsDOBJ2` and
//! `GsOT` as native structs, the table's links and the work base), loads the GTE, installs the
//! handlers the game installs, calls the Rust function and compares, links translated:
//! - the work base (`GsOUT_PACKET_P`);
//! - the whole packet area read back, word for word (what was written and what wasn't);
//! - every ordering-table entry and the guard area after the table;
//! - the scratchpad, when the request read it back;
//! - the GTE's control registers but FLAG (the data registers are the handlers' working storage).
//!
//! A request whose call wrote past the end of the ordering table (an OTZ beyond it: the SDK
//! doesn't check) is not compared: natively that stops the program (`tests/sort4j_traps.rs`); such requests are
//! counted and reported. Every other request must match, and must not fault on the PS1.
//!
//! Debugging: `DW1_TMD_SHOW=N` prints the first N differing requests in full, `DW1_TMD_TRACE=1`
//! each request's number before it runs (to find one that stops the program).

use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    match std::env::var_os("DW1_LIBGS_TMD_CAPTURES") {
        Some(d) => PathBuf::from(d).join(name),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../captures/oracle-runs")
            .join(name),
    }
}

#[cfg(not(have_libgs_tmd_fast_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/p9-render-s4j-fast; local only"]
fn tmd_handlers_match_the_real_sdk() {}

#[cfg(have_libgs_tmd_fast_capture)]
#[test]
fn tmd_handlers_match_the_real_sdk() {
    oracle::run(&dir("p9-render-s4j-fast"));
}

#[cfg(not(have_libgs_tmd_div_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/p9-render-s4j-div; local only"]
fn divided_tmd_handlers_match_the_real_sdk() {}

#[cfg(have_libgs_tmd_div_capture)]
#[test]
fn divided_tmd_handlers_match_the_real_sdk() {
    oracle::run(&dir("p9-render-s4j-div"));
}

#[cfg(not(have_libgs_tmd_attr_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/p9-render-s4j-attr; local only"]
fn hidden_objects_match_the_real_sdk() {}

#[cfg(have_libgs_tmd_attr_capture)]
#[test]
fn hidden_objects_match_the_real_sdk() {
    oracle::run(&dir("p9-render-s4j-attr"));
}

#[cfg(not(have_libgs_tmd_bits_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/p9-render-s4j-bits; local only"]
fn tmd_mode_bits_match_the_real_sdk() {}

#[cfg(have_libgs_tmd_bits_capture)]
#[test]
fn tmd_mode_bits_match_the_real_sdk() {
    oracle::run(&dir("p9-render-s4j-bits"));
}

#[cfg(not(have_libgs_tmd_bounds_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/p9-render-s4j-bounds; local only"]
fn divide_screen_and_near_tests_match_the_real_sdk() {}

#[cfg(have_libgs_tmd_bounds_capture)]
#[test]
fn divide_screen_and_near_tests_match_the_real_sdk() {
    oracle::run(&dir("p9-render-s4j-bounds"));
}

#[cfg(any(
    have_libgs_tmd_bounds_capture,
    have_libgs_tmd_fast_capture,
    have_libgs_tmd_div_capture,
    have_libgs_tmd_attr_capture,
    have_libgs_tmd_bits_capture
))]
mod oracle {
    use psyq::globals::GsFCALL4;
    use psyq::libgs::{
        GsDobj2, GsGetWorkBase, GsOt, GsSetWorkBase, GsSortObject4J, GsTMDdivTNF3, GsTMDdivTNF4,
        GsTMDdivTNG3, GsTMDdivTNG4, GsTMDfastF3L, GsTMDfastF4L, GsTMDfastG4L, GsTMDfastTF3L,
        GsTMDfastTF4L, GsTMDfastTG3L, GsTMDfastTG4L, GsTMDfastTNF3, GsTMDfastTNF4, GsTMDfastTNG3,
        GsTMDfastTNG4, TmdHandler,
    };
    use psyq::memory::{LINK_END, dw1_ps1_ram, dw1_scratchpad, link_of, ps1_ram_ptr, ptr32_of};
    use std::collections::BTreeMap;
    use std::path::Path;

    const BUF: u32 = 0x801E_0000;
    const OBJ: u32 = BUF;
    const OT: u32 = BUF + 0x20;
    const REC: u32 = BUF + 0x40;
    const ORG: u32 = BUF + 0x2000;
    const PACKETS: u32 = BUF + 0x3000;
    const SCRATCHPAD: u32 = 0x1F80_0000;
    const OUT_PACKET_P: u32 = 0x8013_6FE4;
    const LIBGS_HWD0: u32 = 0x8013_6F10;

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

    fn ram() -> &'static mut [u8; 2 << 20] {
        unsafe { &mut *(&raw mut dw1_ps1_ram).cast::<[u8; 2 << 20]>() }
    }

    fn native(a: u32) -> *mut u8 {
        ps1_ram_ptr(a)
    }

    fn rd(a: u32) -> u32 {
        let o = a as usize & 0x1F_FFFF;
        le32(&ram()[o..o + 4], 0)
    }

    fn wr(a: u32, v: u32) {
        let o = a as usize & 0x1F_FFFF;
        ram()[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// A 24-bit link as the PS1 wrote it, in the native encoding.
    fn translate_link(w: u32) -> u32 {
        let link = w & LINK_END;
        if link == LINK_END {
            return w;
        }
        (w & 0xFF00_0000) | link_of(native(0x8000_0000 | link))
    }

    /// The handlers `initializeGsTMDMap` installs that have a native implementation, in their
    /// `GsFCALL4` slots (decomp/src/main/graphics.c; the slot numbers are the real table's as the
    /// game filled it, read back in `p9-render-explore`). The others stay empty: the native sort
    /// stops on them, as the game's trapping stubs would.
    fn install_handlers() {
        let slots: [(usize, TmdHandler); 15] = [
            (0, GsTMDfastF3L),
            (16, GsTMDfastTF3L),
            (22, GsTMDfastTNF3),
            (23, GsTMDdivTNF3),
            (24, GsTMDfastTG3L),
            (30, GsTMDfastTNG3),
            (31, GsTMDdivTNG3),
            (32, GsTMDfastF4L),
            (40, GsTMDfastG4L),
            (48, GsTMDfastTF4L),
            (54, GsTMDfastTNF4),
            (55, GsTMDdivTNF4),
            (56, GsTMDfastTG4L),
            (62, GsTMDfastTNG4),
            (63, GsTMDdivTNG4),
        ];
        unsafe {
            (&raw mut GsFCALL4).write([None; psyq::globals::GS_FCALL_SLOTS]);
            for (slot, f) in slots {
                GsFCALL4[slot] = Some(std::mem::transmute::<TmdHandler, unsafe extern "C" fn()>(f));
            }
        }
    }

    struct Req {
        ins: Vec<(u32, Vec<u8>)>,
        outs: Vec<(u32, usize)>,
        shift: i32,
        gte: Vec<Option<u32>>,
    }

    fn parse(line: &str) -> Req {
        let mut w = line.split_whitespace();
        assert_eq!(w.next(), Some("GsSortObject4J"));
        let args: Vec<u32> = (0..4).map(|_| addr(w.next().unwrap())).collect();
        assert_eq!((args[0], args[1], args[3]), (OBJ, OT, SCRATCHPAD));
        let mut r = Req {
            ins: Vec::new(),
            outs: Vec::new(),
            shift: args[2] as i32,
            gte: Vec::new(),
        };
        for f in w {
            if let Some(v) = f.strip_prefix("in=") {
                let (a, h) = v.split_once(':').unwrap();
                r.ins.push((addr(a), unhex(h)));
            } else if let Some(v) = f.strip_prefix("out=") {
                let (a, n) = v.split_once(':').unwrap();
                r.outs.push((addr(a), word(n) as usize));
            } else if let Some(v) = f.strip_prefix("gtein=") {
                r.gte = v.split(',').map(|x| (x != "x").then(|| word(x))).collect();
            } else {
                assert_eq!(f, "gteout");
            }
        }
        r
    }

    /// The result line: (read-backs by address, GTE registers d0..d31 c0..c31), or None for a
    /// call that faulted.
    /// A result's read-backs by address and its 64 GTE registers.
    type Readback = (BTreeMap<u32, Vec<u8>>, Vec<u32>);

    fn parse_result(line: &str) -> Option<Readback> {
        let mut w = line.split_whitespace();
        w.next();
        let v0 = w.next().unwrap();
        if v0.starts_with("v0=EXC") {
            return None;
        }
        let mut outs = BTreeMap::new();
        let mut gte = Vec::new();
        for f in w {
            let (k, v) = f.split_once('=').unwrap();
            if k == "gte" {
                gte = v.split(',').map(word).collect();
            } else {
                outs.insert(word(k), unhex(v));
            }
        }
        Some((outs, gte))
    }

    /// Runs one request natively; returns the differences.
    fn check(req: &Req, outs: &BTreeMap<u32, Vec<u8>>, gte_out: &[u32]) -> Vec<String> {
        // Fresh buffer: nothing left from the previous request.
        let base = (BUF & 0x1F_FFFF) as usize;
        ram()[base..base + 0x1_0000].fill(0);
        let mut scratch_in = None;
        // The screen size the dividing handlers use: libgs's (GsInitGraph; DW1's is 320 x 240),
        // which a request may change (`in=` at HWD0, VWD0).
        let (mut width, mut height) = (320u16, 240u16);
        for (a, bytes) in &req.ins {
            if *a == LIBGS_HWD0 {
                (width, height) = (le32(bytes, 0) as u16, le32(bytes, 1) as u16);
            }
        }
        psyq::libgs::GsInitGraph(width, height, 4, 0, 0);
        for (a, bytes) in &req.ins {
            match *a {
                OUT_PACKET_P => {}
                SCRATCHPAD => scratch_in = Some(bytes.clone()),
                a => {
                    let o = a as usize & 0x1F_FFFF;
                    ram()[o..o + bytes.len()].copy_from_slice(bytes);
                }
            }
        }
        let ot_len = rd(OT);
        let n_ot = 1u32 << ot_len;
        for i in 0..n_ot {
            wr(ORG + 4 * i, translate_link(rd(ORG + 4 * i)));
        }
        for field in [0, 2, 4] {
            let a = REC + 4 * field;
            wr(a, ptr32_of(native(rd(a))));
        }
        let mut obj = GsDobj2 {
            attribute: rd(OBJ),
            coord2: std::ptr::null_mut(),
            tmd: native(rd(OBJ + 8)).cast(),
            id: rd(OBJ + 12),
        };
        let mut ot = GsOt {
            length: ot_len,
            org: native(rd(OT + 4)).cast(),
            offset: rd(OT + 8),
            point: rd(OT + 12),
            tag: native(rd(OT + 16)).cast(),
        };
        let scratch =
            unsafe { std::slice::from_raw_parts_mut((&raw mut dw1_scratchpad).cast::<u8>(), 1024) };
        match &scratch_in {
            Some(s) => scratch.copy_from_slice(s),
            None => scratch.fill(0),
        }
        GsSetWorkBase(native(PACKETS));
        psyq::gte::with(|g| {
            *g = backend::gte::Gte::new();
            for (i, v) in req.gte.iter().enumerate() {
                if let Some(v) = *v {
                    if i < 32 {
                        g.mtc2(i, v);
                    } else {
                        g.ctc2(i - 32, v);
                    }
                }
            }
        });
        install_handlers();
        unsafe {
            GsSortObject4J(
                &mut obj,
                &mut ot,
                req.shift,
                (&raw mut dw1_scratchpad).cast(),
            )
        };

        let mut bad = Vec::new();
        let wb = GsGetWorkBase() as usize - ps1_ram_ptr(0x8000_0000) as usize;
        let want_wb = le32(&outs[&OUT_PACKET_P], 0) & 0x1F_FFFF;
        if wb != want_wb as usize {
            bad.push(format!(
                "work base +{:#x}, SDK +{:#x}",
                wb as u32 - (PACKETS & 0x1F_FFFF),
                want_wb - (PACKETS & 0x1F_FFFF)
            ));
        }
        // Packets: tags are links; the rest raw.
        let pk = &outs[&PACKETS];
        let mut a = 0usize;
        let mut tags = std::collections::BTreeSet::new();
        while a < (want_wb - (PACKETS & 0x1F_FFFF)) as usize && a + 4 <= pk.len() {
            tags.insert(a);
            let len = pk[a + 3] as usize;
            if std::env::var_os("DW1_DIAG").is_some() {
                let ws: Vec<String> = (0..=len)
                    .map(|k| format!("{:08x}", le32(pk, a / 4 + k)))
                    .collect();
                eprintln!("SDKPK {}", ws.join(" "));
            }
            if len == 0 {
                bad.push(format!("SDK packet at +{a:#x} has length 0"));
                break;
            }
            a += 4 + 4 * len;
        }
        for i in 0..pk.len() / 4 {
            let want = le32(pk, i);
            let want = if tags.contains(&(i * 4)) {
                translate_link(want)
            } else {
                want
            };
            let got = rd(PACKETS + 4 * i as u32);
            if got != want {
                bad.push(format!(
                    "packet word +{:#x}: SDK {want:08x}, native {got:08x}",
                    i * 4
                ));
            }
        }
        // The table and its guard.
        let org = &outs[&ORG];
        for i in 0..org.len() / 4 {
            let want = le32(org, i);
            let want = if (i as u32) < n_ot {
                translate_link(want)
            } else {
                want
            };
            let got = rd(ORG + 4 * i as u32);
            if got != want {
                bad.push(format!("OT word {i}: SDK {want:08x}, native {got:08x}"));
            }
        }
        if let Some(s) = outs.get(&SCRATCHPAD) {
            for i in 0..256 {
                let (want, got) = (le32(s, i), le32(scratch, i));
                if want != got {
                    bad.push(format!(
                        "scratchpad +{:#x}: SDK {want:08x}, native {got:08x}",
                        i * 4
                    ));
                }
            }
        }
        psyq::gte::with(|g| {
            for r in 0..31 {
                // The emulator's register file holds the values `gtein` put there as given; the
                // GTE reads some back sign-extended (16-bit registers): compare as the GTE
                // would read the SDK's.
                let mut canon = backend::gte::Gte::new();
                canon.ctc2(r, gte_out[32 + r]);
                if g.cfc2(r) != canon.cfc2(r) {
                    bad.push(format!(
                        "GTE control {r}: SDK {:08x}, native {:08x}",
                        gte_out[32 + r],
                        g.cfc2(r)
                    ));
                }
            }
        });
        bad
    }

    /// Whether the SDK wrote outside the table: into the guard after it, or further (then some
    /// packet hangs off that entry instead of off one of the table's: it can't be reached from
    /// the table).
    fn wrote_past_table(req: &Req, outs: &BTreeMap<u32, Vec<u8>>) -> bool {
        let ot_in = &req.ins.iter().find(|(a, _)| *a == ORG).unwrap().1;
        let n = 4usize << le32(&req.ins.iter().find(|(a, _)| *a == OT).unwrap().1, 0);
        if outs[&ORG][n..] != ot_in[n..] {
            return true;
        }
        let wb = le32(&outs[&OUT_PACKET_P], 0);
        let pk = &outs[&PACKETS];
        let in_packets = |a: u32| (PACKETS..wb).contains(&a) && ((a - PACKETS) as usize) < pk.len();
        // Every packet reachable from the table's entries.
        let mut reached = std::collections::BTreeSet::new();
        for i in 0..n / 4 {
            let mut a = 0x8000_0000 | (le32(&outs[&ORG], i) & LINK_END);
            while in_packets(a) && reached.insert(a) {
                a = 0x8000_0000 | (le32(pk, ((a - PACKETS) / 4) as usize) & LINK_END);
            }
        }
        // Every packet written, in memory order.
        let mut a = PACKETS;
        while a < wb && ((a - PACKETS) as usize) + 4 <= pk.len() {
            if !reached.contains(&a) {
                return true;
            }
            let len = pk[(a - PACKETS) as usize + 3] as u32;
            if len == 0 {
                return false;
            }
            a += 4 + 4 * len;
        }
        false
    }

    /// The tests share the native PS1 RAM stand-in, the GTE and GsFCALL4, so they take turns.
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub fn run(dir: &Path) {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());

        let req = std::fs::read_to_string(dir.join("req.txt")).unwrap();
        let res = std::fs::read_to_string(dir.join("results.txt")).unwrap();
        let reqs: Vec<&str> = req.lines().filter(|l| !l.trim().is_empty()).collect();
        let ress: Vec<&str> = res.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(reqs.len(), ress.len(), "requests vs results");
        assert!(!reqs.is_empty());
        let show: usize = std::env::var("DW1_TMD_SHOW")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let (mut ok, mut past, mut words) = (0, 0, 0);
        let mut failures = Vec::new();
        for (n, (q, r)) in reqs.iter().zip(&ress).enumerate() {
            let index: usize = r.split_whitespace().next().unwrap().parse().unwrap();
            assert_eq!(index, n + 1, "result order");
            if std::env::var_os("DW1_TMD_TRACE").is_some() {
                eprintln!("request {}", n + 1);
            }
            let req = parse(q);
            let Some((outs, gte_out)) = parse_result(r) else {
                panic!("request {}: the SDK faulted", n + 1);
            };
            if wrote_past_table(&req, &outs) {
                past += 1;
                continue;
            }
            if std::env::var_os("DW1_DIAG").is_some() {
                eprintln!("REQ {}", n + 1);
            }
            let bad = check(&req, &outs, &gte_out);
            if bad.is_empty() {
                ok += 1;
                words += (le32(&outs[&OUT_PACKET_P], 0) - PACKETS) / 4;
            } else {
                if failures.len() < show {
                    eprintln!("request {}: {} differences", n + 1, bad.len());
                    for b in bad.iter().take(40) {
                        eprintln!("  {b}");
                    }
                }
                failures.push(format!("request {}: {}", n + 1, bad[0]));
            }
        }
        println!(
            "{}: {ok} of {} requests match ({words} packet words); {past} wrote past the table (not compared)",
            dir.display(),
            reqs.len()
        );
        assert!(
            failures.is_empty(),
            "{} requests differ:\n{}",
            failures.len(),
            failures[..failures.len().min(
                std::env::var("DW1_TMD_LIST")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10)
            )]
                .join("\n")
        );
    }
}
