//! libgs's lights, view and coordinate-system functions against the real SDK's results, recorded
//! by `oracle/call-sdk.lua` in PCSX-Redux from the requests `tools/oracle/libgs_requests.py`
//! writes: `GsSetFlatLight`, `GsSetAmbient`, `GsSetLightMode`, `GsSetLightMatrix`,
//! `GsSetLsMatrix`, `GsSetView2`, `GsGetLw`, `GsGetLs`, `GsGetLws`, `GsMulCoord0`, `GsMulCoord3`.
//!
//! The files are Sony's code's outputs, so they stay local: `req.txt` and `results.txt` under
//! `DW1_LIBGS_LIGHT_CAPTURE` (default `captures/oracle-runs/libgs-light`, gitignored). Without
//! them the test shows as ignored. The requests run in order against one memory image (the
//! emulator's scratch buffer, PS1 layout) and the libgs globals, which carry over from request
//! to request as they do in the emulator. A GsCOORDINATE2 or GsVIEW2 in the image has 32-bit
//! pointers; the test builds native structs (8-byte pointers) from the same values, calls the
//! Rust function and writes the fields back into the image, so every byte the request reads
//! back (`out=`) is compared as the PS1 had it. Also compared: the return value (when the
//! function has one), all libgs globals the requests read back, and all 64 GTE registers.
//!
//! The fault cases (a `super` of SCREEN) must stop the program, as the real code faults.
//!
//! Debugging a failure: `DW1_LIBGS_SHOW=N` prints the first N mismatches per function with the
//! request, `DW1_LIBGS_ONLY=name,name` runs just those functions (their state then differs from
//! the recording's, so use it with a function that doesn't depend on earlier requests).

#[cfg(not(have_libgs_light_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/libgs-light (or DW1_LIBGS_LIGHT_CAPTURE); local only"]
fn libgs_lights_and_coordinates_match_the_real_sdk() {}

#[cfg(have_libgs_light_capture)]
mod oracle {
    use backend::gte::Gte;
    use psyq::globals::{
        _LC, GsLIGHT_MODE, GsLIGHTWSMATRIX, GsWSMATRIX, GsWSMATRIX_ORG, Matrix, PSDCNT,
    };
    use psyq::libgs::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    /// The scratch buffer's address in the emulator (`B+x` is this plus x).
    const BUF: u32 = 0x801E_0000;
    const BUF_SIZE: usize = 0x1_0000 + 0x100;

    fn dir() -> PathBuf {
        match std::env::var_os("DW1_LIBGS_LIGHT_CAPTURE") {
            Some(d) => PathBuf::from(d),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../captures/oracle-runs/libgs-light"),
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

    /// A request address: a hex number, or `B+offset` into the scratch buffer.
    fn addr(s: &str) -> u32 {
        match s.strip_prefix("B+") {
            Some(off) => BUF + word(off),
            None => word(s),
        }
    }

    struct Request {
        name: String,
        args: [u32; 4],
        ins: Vec<(u32, Vec<u8>)>,
        outs: Vec<(u32, usize)>,
        gtein: Option<Vec<String>>,
        gteout: bool,
    }

    fn parse(line: &str) -> Request {
        let mut words = line.split_whitespace();
        let name = words.next().unwrap().to_string();
        let mut args = [0u32; 4];
        for a in &mut args {
            *a = addr(words.next().unwrap());
        }
        let mut req = Request {
            name,
            args,
            ins: vec![],
            outs: vec![],
            gtein: None,
            gteout: false,
        };
        for w in words {
            if w == "gteout" {
                req.gteout = true;
            } else if let Some(v) = w.strip_prefix("in=") {
                let (a, hex) = v.split_once(':').unwrap();
                req.ins.push((addr(a), unhex(hex)));
            } else if let Some(v) = w.strip_prefix("out=") {
                let (a, n) = v.split_once(':').unwrap();
                req.outs.push((addr(a), word(n) as usize));
            } else if let Some(v) = w.strip_prefix("gtein=") {
                req.gtein = Some(v.split(',').map(str::to_string).collect());
            } else {
                panic!("bad request field {w}");
            }
        }
        req
    }

    struct Result {
        number: usize,
        exception: bool,
        v0: u32,
        mem: Vec<(u32, Vec<u8>)>,
        gte: Option<Vec<u32>>,
    }

    fn parse_result(line: &str) -> Result {
        let mut words = line.split_whitespace();
        let number = words
            .next()
            .and_then(|w| w.parse().ok())
            .expect("result line without a call number");
        let v0 = words.next().unwrap().strip_prefix("v0=").unwrap();
        let mut res = Result {
            number,
            exception: v0.starts_with("EXC"),
            v0: if v0.starts_with("EXC") { 0 } else { word(v0) },
            mem: vec![],
            gte: None,
        };
        for w in words {
            if let Some(g) = w.strip_prefix("gte=") {
                res.gte = Some(g.split(',').map(word).collect());
            } else {
                let (a, hex) = w.split_once('=').unwrap();
                res.mem.push((word(a), unhex(hex)));
            }
        }
        res
    }

    // ---- the memory image: the scratch buffer plus the libgs globals ----

    /// 64-byte aligned scratch memory standing in for the emulator's buffer.
    struct Image(Box<[u64]>);

    impl Image {
        fn new() -> Self {
            Image(vec![0u64; BUF_SIZE / 8].into_boxed_slice())
        }

        /// The byte at emulator address `a`: in a libgs global (which the Rust code keeps in
        /// statics) or in the scratch buffer.
        fn ptr(&mut self, a: u32) -> *mut u8 {
            const GLOBALS: [(u32, usize); 6] = [
                (0x8013_6E08, 4),  // PSDCNT
                (0x8013_6F1C, 4),  // GsLIGHT_MODE
                (0x8013_6F24, 32), // GsLIGHTWSMATRIX
                (0x8013_6F44, 32), // _LC
                (0x8013_6F84, 32), // GsWSMATRIX
                (0x8013_6FA4, 32), // GsWSMATRIX_ORG
            ];
            for (i, &(start, len)) in GLOBALS.iter().enumerate() {
                if a >= start && ((a - start) as usize) < len {
                    let off = (a - start) as usize;
                    let base: *mut u8 = match i {
                        0 => (&raw mut PSDCNT).cast(),
                        1 => (&raw mut GsLIGHT_MODE).cast(),
                        2 => (&raw mut GsLIGHTWSMATRIX).cast(),
                        3 => (&raw mut _LC).cast(),
                        4 => (&raw mut GsWSMATRIX).cast(),
                        _ => (&raw mut GsWSMATRIX_ORG).cast(),
                    };
                    return unsafe { base.add(off) };
                }
            }
            let off = a.wrapping_sub(BUF) as usize;
            assert!(
                off < BUF_SIZE,
                "address {a:#x} is outside the scratch buffer"
            );
            unsafe { self.0.as_mut_ptr().cast::<u8>().add(off) }
        }

        fn write(&mut self, a: u32, bytes: &[u8]) {
            for (i, &b) in bytes.iter().enumerate() {
                unsafe { self.ptr(a + i as u32).write(b) };
            }
        }

        fn read(&mut self, a: u32, n: usize) -> Vec<u8> {
            (0..n)
                .map(|i| unsafe { self.ptr(a + i as u32).read() })
                .collect()
        }

        fn read_u32(&mut self, a: u32) -> u32 {
            u32::from_le_bytes(self.read(a, 4).try_into().unwrap())
        }
    }

    // ---- GsCOORDINATE2 and GsVIEW2: PS1 image <-> native structs ----

    /// A PS1 `GsCOORDINATE2` is 0x50 bytes: flg, coord (32), workm (32), param, super, sub.
    /// The native `param` and `sub` hold these markers; the function must not change them.
    const PARAM_MARK: usize = 0xAAAA_0000;
    const SUB_MARK: usize = 0xBBBB_0000;

    struct Chain {
        /// (image address, native struct), leaf first.
        nodes: Vec<(u32, Box<GsCoordinate2>)>,
    }

    impl Chain {
        /// Builds the native chain starting at image address `start` (0 for none).
        unsafe fn build(img: &mut Image, start: u32) -> Chain {
            let mut nodes: Vec<(u32, Box<GsCoordinate2>)> = vec![];
            let mut a = start;
            while a != 0 && a != 1 && nodes.len() < 40 {
                let mut n: Box<GsCoordinate2> = unsafe { Box::new_zeroed().assume_init() };
                n.flg = img.read_u32(a);
                let coord = img.read(a + 4, 32);
                let workm = img.read(a + 36, 32);
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        coord.as_ptr(),
                        (&raw mut n.coord).cast::<u8>(),
                        32,
                    );
                    std::ptr::copy_nonoverlapping(
                        workm.as_ptr(),
                        (&raw mut n.workm).cast::<u8>(),
                        32,
                    );
                }
                n.param = (PARAM_MARK + a as usize) as *mut u8;
                n.sub = (SUB_MARK + a as usize) as *mut GsCoordinate2;
                let sup = img.read_u32(a + 72);
                nodes.push((a, n));
                a = sup;
            }
            // Link each node to the next one; the last one's super is NULL, or SCREEN (1).
            let last_sup = if a == 1 {
                std::ptr::without_provenance_mut::<GsCoordinate2>(1)
            } else {
                std::ptr::null_mut()
            };
            let n = nodes.len();
            for i in 0..n {
                let next: *mut GsCoordinate2 = if i + 1 < n {
                    &mut *nodes[i + 1].1
                } else {
                    last_sup
                };
                nodes[i].1.sup = next;
            }
            Chain { nodes }
        }

        fn leaf(&mut self) -> *mut GsCoordinate2 {
            self.nodes
                .first_mut()
                .map_or(std::ptr::null_mut(), |n| &mut *n.1)
        }

        /// Writes flg, coord and workm back into the image.
        unsafe fn store(&mut self, img: &mut Image) {
            for (a, n) in &mut self.nodes {
                assert_eq!(n.param as usize, PARAM_MARK + *a as usize, "param changed");
                assert_eq!(n.sub as usize, SUB_MARK + *a as usize, "sub changed");
                img.write(*a, &n.flg.to_le_bytes());
                let coord =
                    unsafe { std::slice::from_raw_parts((&raw const n.coord).cast::<u8>(), 32) };
                img.write(*a + 4, coord);
                let workm =
                    unsafe { std::slice::from_raw_parts((&raw const n.workm).cast::<u8>(), 32) };
                img.write(*a + 36, workm);
            }
        }
    }

    // ---- GTE state ----

    /// Registers compared by their low 16 bits: the emulator keeps these as 16-bit fields in a
    /// 32-bit slot and leaves the upper half as it was.
    const D_LOW16: [usize; 10] = [1, 3, 5, 7, 8, 9, 10, 11, 16, 17];
    const D_LOW16_B: [usize; 2] = [18, 19];
    const C_LOW16: [usize; 7] = [4, 12, 20, 26, 27, 29, 30];

    fn set_gte(g: &mut Gte, words: &[String]) {
        *g = Gte::new();
        for (n, w) in words.iter().enumerate() {
            if w == "x" {
                continue;
            }
            let v = word(w);
            if n < 32 {
                if n != 31 {
                    g.mtc2(n, v);
                }
            } else {
                g.ctc2(n - 32, v);
            }
        }
    }

    fn gte_state(g: &Gte) -> Vec<u32> {
        let mut out = vec![];
        for n in 0..32 {
            out.push(g.mfc2(n));
        }
        for n in 0..32 {
            out.push(g.cfc2(n));
        }
        out
    }

    fn reg_name(n: usize) -> String {
        if n < 32 {
            format!("d{n}")
        } else {
            format!("c{}", n - 32)
        }
    }

    /// SXYP, IRGB and ORGB are views of other registers; the emulator's raw slots for them
    /// aren't the hardware view.
    fn compared(n: usize) -> bool {
        !matches!(n, 15 | 28 | 29)
    }

    fn same_reg(n: usize, a: u32, b: u32) -> bool {
        let low = D_LOW16.contains(&n)
            || D_LOW16_B.contains(&n)
            || (n >= 32 && C_LOW16.contains(&(n - 32)));
        if low {
            a & 0xFFFF == b & 0xFFFF
        } else {
            a == b
        }
    }

    // ---- calls ----

    /// Calls the named function on the request's arguments; returns its result in the
    /// emulator's terms.
    unsafe fn call(img: &mut Image, rq: &Request) -> u32 {
        let a = rq.args;
        macro_rules! p {
            ($t:ty, $x:expr) => {
                img.ptr($x).cast::<$t>()
            };
        }
        unsafe {
            match rq.name.as_str() {
                "GsSetFlatLight" => GsSetFlatLight(a[0] as i32, p!(GsFLight, a[1])) as u32,
                "GsSetAmbient" => {
                    GsSetAmbient(a[0] as i32, a[1] as i32, a[2] as i32);
                    0
                }
                "GsSetLightMode" => {
                    GsSetLightMode(a[0] as i32);
                    0
                }
                "GsSetLightMatrix" => {
                    GsSetLightMatrix(p!(Matrix, a[0]));
                    0
                }
                "GsSetLsMatrix" => {
                    GsSetLsMatrix(p!(Matrix, a[0]));
                    0
                }
                "GsMulCoord0" => {
                    GsMulCoord0(p!(Matrix, a[0]), p!(Matrix, a[1]), p!(Matrix, a[2]));
                    0
                }
                "GsMulCoord3" => {
                    GsMulCoord3(p!(Matrix, a[0]), p!(Matrix, a[1]));
                    0
                }
                "GsGetLw" | "GsGetLs" | "GsGetLws" => {
                    let mut chain = Chain::build(img, a[0]);
                    let leaf = chain.leaf();
                    match rq.name.as_str() {
                        "GsGetLw" => GsGetLw(leaf, p!(Matrix, a[1])),
                        "GsGetLs" => GsGetLs(leaf, p!(Matrix, a[1])),
                        _ => GsGetLws(leaf, p!(Matrix, a[1]), p!(Matrix, a[2])),
                    }
                    chain.store(img);
                    0
                }
                "GsSetView2" => {
                    let mut view: Box<GsView2> = Box::new_zeroed().assume_init();
                    let bytes = img.read(a[0], 32);
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        (&raw mut view.view).cast::<u8>(),
                        32,
                    );
                    let sup = img.read_u32(a[0] + 32);
                    let mut chain = Chain::build(img, sup);
                    view.sup = if sup == 1 {
                        std::ptr::without_provenance_mut::<GsCoordinate2>(1)
                    } else {
                        chain.leaf()
                    };
                    let r = GsSetView2(&*view);
                    chain.store(img);
                    r as u32
                }
                other => panic!("the test doesn't know {other}"),
            }
        }
    }

    /// Functions with a return value worth comparing (the rest are void in C).
    fn returns_value(name: &str) -> bool {
        matches!(name, "GsSetFlatLight" | "GsSetView2")
    }

    fn apply(img: &mut Image, rq: &Request) {
        for (a, bytes) in &rq.ins {
            img.write(*a, bytes);
        }
        if let Some(words) = &rq.gtein {
            psyq::gte::with(|g| set_gte(g, words));
        }
    }

    #[test]
    fn libgs_lights_and_coordinates_match_the_real_sdk() {
        let d = dir();
        let reqs = std::fs::read_to_string(d.join("req.txt")).unwrap();
        let results = std::fs::read_to_string(d.join("results.txt")).unwrap();
        let show: usize = std::env::var("DW1_LIBGS_SHOW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        let only: Option<Vec<String>> = std::env::var("DW1_LIBGS_ONLY")
            .ok()
            .map(|v| v.split(',').map(str::to_string).collect());
        let child: Option<usize> = std::env::var("DW1_LIBGS_CHILD")
            .ok()
            .and_then(|v| v.parse().ok());
        let partial = std::env::var_os("DW1_LIBGS_PARTIAL").is_some();
        let trace = std::env::var_os("DW1_LIBGS_TRACE").is_some();
        let n_reqs = reqs.lines().filter(|l| !l.trim().is_empty()).count();
        let n_results = results.lines().filter(|l| !l.trim().is_empty()).count();
        assert!(n_reqs > 0, "no requests in {}", d.display());
        if !partial {
            assert_eq!(
                n_reqs, n_results,
                "req.txt has {n_reqs} requests but results.txt {n_results} lines (a run that hung \
                 or was cut short? DW1_LIBGS_PARTIAL=1 inspects what there is)"
            );
        }
        let mut img = Image::new();
        let mut stats: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut shown: BTreeMap<String, usize> = BTreeMap::new();
        let mut result_lines = results.lines().filter(|l| !l.trim().is_empty());
        let mut index = 0usize;
        let mut compared_regs = 0usize;
        let mut compared_bytes = 0usize;
        let mut faults = 0usize;
        for line in reqs.lines().filter(|l| !l.trim().is_empty()) {
            index += 1;
            let Some(res_line) = result_lines.next() else {
                break; // only reached in the partial diagnostic mode
            };
            let rq = parse(line);
            let want = parse_result(res_line);
            assert_eq!(
                want.number, index,
                "result lines are out of order at request {index}"
            );
            if let Some(c) = child {
                // Child mode: run only request `c` (one the real SDK faulted on) and return.
                if c == index {
                    apply(&mut img, &rq);
                    unsafe { call(&mut img, &rq) };
                    return;
                }
                continue;
            }
            if !want.exception {
                // Every output the request asked for must be in the result.
                for (a, n) in &rq.outs {
                    assert!(
                        want.mem.iter().any(|(wa, b)| wa == a && b.len() == *n),
                        "request {index} ({}): result lacks the {n} bytes at {a:#x}",
                        rq.name
                    );
                }
                assert_eq!(
                    rq.gteout,
                    want.gte.is_some(),
                    "request {index} ({}): GTE registers requested/recorded mismatch",
                    rq.name
                );
            }
            if want.exception {
                faults += 1;
                // The real SDK faulted; the port must stop the program here, not carry on.
                if only.is_none() {
                    let out = std::process::Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--exact",
                            "oracle::libgs_lights_and_coordinates_match_the_real_sdk",
                            "--nocapture",
                        ])
                        .env("DW1_LIBGS_CHILD", index.to_string())
                        .output()
                        .unwrap();
                    assert!(
                        !out.status.success()
                            && String::from_utf8_lossy(&out.stderr)
                                .contains("unimplemented SDK call"),
                        "request {index} ({}) faults on the PS1 but the port went on",
                        rq.name
                    );
                }
                continue;
            }
            if only.as_ref().is_some_and(|o| !o.contains(&rq.name)) {
                continue;
            }
            apply(&mut img, &rq);
            if trace {
                eprintln!("request {index}: {line}");
            }
            let got_v0 = unsafe { call(&mut img, &rq) };
            let mut problems: Vec<String> = vec![];
            if returns_value(&rq.name) && got_v0 != want.v0 {
                problems.push(format!("v0 {:#x}, want {:#x}", got_v0, want.v0));
            }
            let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
            for (a, expect) in &want.mem {
                let got = img.read(*a, expect.len());
                compared_bytes += got.len();
                if got != *expect {
                    problems.push(format!(
                        "memory {:#x}: {}, want {}",
                        a,
                        hex(&got),
                        hex(expect)
                    ));
                }
            }
            if let Some(w) = &want.gte {
                let got = psyq::gte::with(|g| gte_state(g));
                for n in 0..64 {
                    if compared(n) {
                        compared_regs += 1;
                        if !same_reg(n, got[n], w[n]) {
                            problems.push(format!(
                                "{} = {:#010x}, want {:#010x}",
                                reg_name(n),
                                got[n],
                                w[n]
                            ));
                        }
                    }
                }
            }
            let e = stats.entry(rq.name.clone()).or_default();
            e.0 += 1;
            if !problems.is_empty() {
                e.1 += 1;
                let n = shown.entry(rq.name.clone()).or_default();
                if *n < show {
                    *n += 1;
                    let short: String = line
                        .split_whitespace()
                        .filter(|w| !w.starts_with("gtein="))
                        .collect::<Vec<_>>()
                        .join(" ");
                    eprintln!("MISMATCH #{index} {short}");
                    for p in problems.iter().take(12) {
                        eprintln!("    {p}");
                    }
                }
            }
        }
        let mut bad = 0;
        for (name, (n, m)) in &stats {
            eprintln!("{name:24} {n:6} requests, {m:6} mismatches");
            bad += m;
        }
        eprintln!(
            "{compared_regs} registers and {compared_bytes} bytes compared, {faults} requests \
             faulted on the PS1 (each stopped the port too)"
        );
        if child.is_some() {
            return;
        }
        assert_eq!(bad, 0, "{bad} requests differ from the real SDK");
        assert!(
            !partial,
            "DW1_LIBGS_PARTIAL inspects an incomplete run: it can't count as a pass"
        );
    }
}
