//! libgte (and `GsSetRefView2`) against the real SDK's results, recorded by `oracle/call-sdk.lua`
//! in PCSX-Redux from the requests `tools/oracle/gte_requests.py` writes.
//!
//! The files are Sony's code's outputs, so they stay local: `req.txt` and `results.txt` under
//! `DW1_GTE_CAPTURE` (default `captures/oracle-runs/gte-final`, gitignored). Without them the
//! test shows as ignored. Each request line is replayed against the Rust functions, in order
//! (the matrix stack and any GTE state a request doesn't set carry over from one request to the
//! next, as they do in the emulator); the return value, the bytes the request reads back and,
//! when the request records them, the GTE registers must be identical.
//!
//! Debugging a failure: `DW1_GTE_SHOW=N` prints the first N mismatches per function with the
//! request, `DW1_GTE_ONLY=name,name` runs just those functions.

#[cfg(not(have_gte_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/gte-final (or DW1_GTE_CAPTURE); local only"]
fn libgte_matches_the_real_sdk() {}

#[cfg(have_gte_capture)]
mod oracle {
    use backend::gte::Gte;
    use psyq::globals::{GsWSMATRIX, GsWSMATRIX_ORG, Matrix};
    use psyq::libgs::{GsRview2, GsSetRefView2};
    use psyq::libgte::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    /// The scratch buffer's address in the emulator (`B+x` is this plus x).
    const BUF: u32 = 0x801E_0000;
    const BUF_SIZE: usize = 0x1_0000 + 0x100;
    const WSMATRIX: u32 = 0x8013_6F84;
    const WSMATRIX_ORG: u32 = 0x8013_6FA4;

    fn dir() -> PathBuf {
        match std::env::var_os("DW1_GTE_CAPTURE") {
            Some(d) => PathBuf::from(d),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../captures/oracle-runs/gte-final"),
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
        stack: Vec<u32>,
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
            stack: vec![],
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
            } else if let Some(v) = w.strip_prefix("stack=") {
                req.stack = v.split(',').map(addr).collect();
            } else if let Some(v) = w.strip_prefix("gtein=") {
                req.gtein = Some(v.split(',').map(str::to_string).collect());
            } else {
                panic!("bad request field {w}");
            }
        }
        req
    }

    struct Result {
        /// The request's number (1-based) as the oracle wrote it.
        number: usize,
        /// The real function raised a CPU exception (a trapping `add` overflowed): an input the
        /// game must never give it, with no defined result to compare.
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

    /// 64-byte aligned scratch memory standing in for the emulator's buffer.
    struct Scratch(Box<[u64]>);

    impl Scratch {
        fn new() -> Self {
            Scratch(vec![0u64; BUF_SIZE / 8].into_boxed_slice())
        }
        fn base(&mut self) -> *mut u8 {
            self.0.as_mut_ptr().cast()
        }
        fn ptr(&mut self, a: u32) -> *mut u8 {
            let off = a.wrapping_sub(BUF) as usize;
            assert!(
                off < BUF_SIZE,
                "address {a:#x} is outside the scratch buffer"
            );
            unsafe { self.base().add(off) }
        }
    }

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

    /// Names of registers, for mismatch reports.
    fn reg_name(n: usize) -> String {
        if n < 32 {
            format!("d{n}")
        } else {
            format!("c{}", n - 32)
        }
    }

    /// Whether register slot `n` (0..32 data, 32..64 control) should be compared.
    fn compared(n: usize, want: &[u32]) -> bool {
        // SXYP, IRGB and ORGB are views of other registers; the emulator's raw slots for them
        // aren't the hardware view.
        let _ = want;
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

    struct Ctx {
        scratch: Scratch,
    }

    /// Calls the named function on the request's arguments, returning its result in the
    /// emulator's terms (a pointer becomes the emulator's address of that buffer offset).
    unsafe fn call(cx: &mut Ctx, rq: &Request) -> u32 {
        let s = &mut cx.scratch;
        let base = s.base() as usize;
        // A pointer result is reported as the emulator address of the same buffer offset.
        let ret_ptr = |p: usize| -> u32 { BUF + (p - base) as u32 };
        let a = rq.args;
        macro_rules! p {
            ($t:ty, $x:expr) => {
                s.ptr($x).cast::<$t>()
            };
        }
        let st = |k: usize| rq.stack[k];
        unsafe {
            match rq.name.as_str() {
                "rsin" => rsin(a[0] as i32) as u32,
                "rcos" => rcos(a[0] as i32) as u32,
                "ratan2" => ratan2(a[0] as i32, a[1] as i32) as u32,
                "SquareRoot0" => SquareRoot0(a[0] as i32) as u32,
                "SetRotMatrix" => {
                    SetRotMatrix(p!(Matrix, a[0]));
                    0
                }
                "SetTransMatrix" => {
                    SetTransMatrix(p!(Matrix, a[0]));
                    0
                }
                "SetLightMatrix" => {
                    SetLightMatrix(p!(Matrix, a[0]));
                    0
                }
                "PushMatrix" => {
                    PushMatrix();
                    0
                }
                "PopMatrix" => {
                    PopMatrix();
                    0
                }
                "TransMatrix" => ret_ptr(TransMatrix(p!(Matrix, a[0]), p!(Vector, a[1])) as usize),
                "ScaleMatrix" => ret_ptr(ScaleMatrix(p!(Matrix, a[0]), p!(Vector, a[1])) as usize),
                "TransposeMatrix" => {
                    ret_ptr(TransposeMatrix(p!(Matrix, a[0]), p!(Matrix, a[1])) as usize)
                }
                "RotMatrix" => ret_ptr(RotMatrix(p!(Svector, a[0]), p!(Matrix, a[1])) as usize),
                "RotMatrixYXZ" => {
                    ret_ptr(RotMatrixYXZ(p!(Svector, a[0]), p!(Matrix, a[1])) as usize)
                }
                "RotMatrixZYX" => {
                    ret_ptr(RotMatrixZYX(p!(Svector, a[0]), p!(Matrix, a[1])) as usize)
                }
                "MulMatrix" => ret_ptr(MulMatrix(p!(Matrix, a[0]), p!(Matrix, a[1])) as usize),
                "MulMatrix0" => {
                    ret_ptr(
                        MulMatrix0(p!(Matrix, a[0]), p!(Matrix, a[1]), p!(Matrix, a[2])) as usize,
                    )
                }
                "CompMatrix" => {
                    ret_ptr(
                        CompMatrix(p!(Matrix, a[0]), p!(Matrix, a[1]), p!(Matrix, a[2])) as usize,
                    )
                }
                "ApplyMatrix" => {
                    ret_ptr(
                        ApplyMatrix(p!(Matrix, a[0]), p!(Svector, a[1]), p!(Vector, a[2])) as usize,
                    )
                }
                "ApplyMatrixSV" => ret_ptr(ApplyMatrixSV(
                    p!(Matrix, a[0]),
                    p!(Svector, a[1]),
                    p!(Svector, a[2]),
                ) as usize),
                "ApplyMatrixLV" => ret_ptr(ApplyMatrixLV(
                    p!(Matrix, a[0]),
                    p!(Vector, a[1]),
                    p!(Vector, a[2]),
                ) as usize),
                "ApplyTransposeMatrixLV" => ret_ptr(ApplyTransposeMatrixLV(
                    p!(Matrix, a[0]),
                    p!(Vector, a[1]),
                    p!(Vector, a[2]),
                ) as usize),
                "RotTransPers" => RotTransPers(
                    p!(Svector, a[0]),
                    p!(i32, a[1]),
                    p!(i32, a[2]),
                    p!(i32, a[3]),
                ) as u32,
                "RotTransPers3" => RotTransPers3(
                    p!(Svector, a[0]),
                    p!(Svector, a[1]),
                    p!(Svector, a[2]),
                    p!(i32, a[3]),
                    p!(i32, st(0)),
                    p!(i32, st(1)),
                    p!(i32, st(2)),
                    p!(i32, st(3)),
                ) as u32,
                "RotTransPers4" => RotTransPers4(
                    p!(Svector, a[0]),
                    p!(Svector, a[1]),
                    p!(Svector, a[2]),
                    p!(Svector, a[3]),
                    p!(i32, st(0)),
                    p!(i32, st(1)),
                    p!(i32, st(2)),
                    p!(i32, st(3)),
                    p!(i32, st(4)),
                    p!(i32, st(5)),
                ) as u32,
                "RotNclip3" => RotNclip3(
                    p!(Svector, a[0]),
                    p!(Svector, a[1]),
                    p!(Svector, a[2]),
                    p!(i32, a[3]),
                    p!(i32, st(0)),
                    p!(i32, st(1)),
                    p!(i32, st(2)),
                    p!(i32, st(3)),
                    p!(i32, st(4)),
                ) as u32,
                "RotNclip4" => RotNclip4(
                    p!(Svector, a[0]),
                    p!(Svector, a[1]),
                    p!(Svector, a[2]),
                    p!(Svector, a[3]),
                    p!(i32, st(0)),
                    p!(i32, st(1)),
                    p!(i32, st(2)),
                    p!(i32, st(3)),
                    p!(i32, st(4)),
                    p!(i32, st(5)),
                    p!(i32, st(6)),
                ) as u32,
                "NormalColorCol" => {
                    NormalColorCol(p!(Svector, a[0]), p!(Cvector, a[1]), p!(Cvector, a[2]));
                    0
                }
                "NormalColorCol3" => {
                    NormalColorCol3(
                        p!(Svector, a[0]),
                        p!(Svector, a[1]),
                        p!(Svector, a[2]),
                        p!(Cvector, a[3]),
                        p!(Cvector, st(0)),
                        p!(Cvector, st(1)),
                        p!(Cvector, st(2)),
                    );
                    0
                }
                "GsSetRefView2" => GsSetRefView2(p!(GsRview2, a[0])) as u32,
                other => panic!("the test doesn't know {other}"),
            }
        }
    }

    /// Functions whose return value isn't meaningful (void in C).
    fn returns_nothing(name: &str) -> bool {
        matches!(
            name,
            "SetRotMatrix"
                | "SetTransMatrix"
                | "SetLightMatrix"
                | "PushMatrix"
                | "PopMatrix"
                | "NormalColorCol"
                | "NormalColorCol3"
        )
    }

    #[test]
    fn libgte_matches_the_real_sdk() {
        let d = dir();
        let reqs = std::fs::read_to_string(d.join("req.txt")).unwrap();
        let results = std::fs::read_to_string(d.join("results.txt")).unwrap();
        let show: usize = std::env::var("DW1_GTE_SHOW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        let only: Option<Vec<String>> = std::env::var("DW1_GTE_ONLY")
            .ok()
            .map(|v| v.split(',').map(str::to_string).collect());
        let mut cx = Ctx {
            scratch: Scratch::new(),
        };
        let mut stats: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut shown: BTreeMap<String, usize> = BTreeMap::new();
        // A run that stopped early (a hang) or an empty file must not pass: unless the partial
        // diagnostic mode is asked for, every request needs its result line.
        let partial = std::env::var_os("DW1_GTE_PARTIAL").is_some();
        let n_reqs = reqs.lines().filter(|l| !l.trim().is_empty()).count();
        let n_results = results.lines().filter(|l| !l.trim().is_empty()).count();
        assert!(n_reqs > 0, "no requests in {}", d.display());
        if !partial {
            assert_eq!(
                n_reqs, n_results,
                "req.txt has {n_reqs} requests but results.txt {n_results} lines (a run that hung \
                 or was cut short? DW1_GTE_PARTIAL=1 inspects what there is)"
            );
        }
        let child: Option<usize> = std::env::var("DW1_GTE_CHILD")
            .ok()
            .and_then(|v| v.parse().ok());
        let mut result_lines = results.lines();
        let mut index = 0usize;
        let mut compared_regs = 0usize;
        let mut skipped = 0usize;
        let mut org_before: Option<Vec<u8>> = None;
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
                    for (a, bytes) in &rq.ins {
                        let p = cx.scratch.ptr(*a);
                        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len()) };
                    }
                    if let Some(words) = &rq.gtein {
                        psyq::gte::with(|g| set_gte(g, words));
                    }
                    unsafe { call(&mut cx, &rq) };
                    return;
                }
                continue;
            }
            // Every output the request asked for must be in the result.
            if !want.exception {
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
            // SquareRoot0 of a negative number reads past the real function's table: not modelled.
            let negative_sqrt = rq.name == "SquareRoot0" && (rq.args[0] as i32) < 0;
            if only.as_ref().is_some_and(|o| !o.contains(&rq.name))
                || want.exception
                || negative_sqrt
            {
                if want.exception {
                    skipped += 1;
                    // The real SDK faulted; the port must stop the program here, not carry on.
                    if only.is_none() {
                        assert_eq!(rq.name, "CompMatrix", "request {index}: unexpected fault");
                        let out = std::process::Command::new(std::env::current_exe().unwrap())
                            .args([
                                "--exact",
                                "oracle::libgte_matches_the_real_sdk",
                                "--nocapture",
                            ])
                            .env("DW1_GTE_CHILD", index.to_string())
                            .output()
                            .unwrap();
                        assert!(
                            !out.status.success()
                                && String::from_utf8_lossy(&out.stderr)
                                    .contains("unimplemented SDK call"),
                            "request {index} faults on the PS1 but the port went on"
                        );
                    }
                }
                continue;
            }
            // Set up memory and GTE state as the emulator had it.
            for (a, bytes) in &rq.ins {
                let p = cx.scratch.ptr(*a);
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len()) };
            }
            // Stack arguments are pointers; the call helper maps them.
            if let Some(words) = &rq.gtein {
                psyq::gte::with(|g| set_gte(g, words));
            }
            if rq.name == "GsSetRefView2" {
                // Poison the result matrix so a call that leaves it alone shows. The copy
                // GsWSMATRIX_ORG is only written by successful calls: it starts each call as the
                // previous call left it in the emulator (or, for the first, as this call leaves it).
                // A GsRVIEW2's super pointer is 4 bytes on the PS1 and 8 here, after the same 28
                // bytes of fields: the request has only the PS1's, so the rest must read as NULL.
                let org_want = want
                    .mem
                    .iter()
                    .find(|(a, _)| *a == WSMATRIX_ORG)
                    .map(|(_, b)| b.clone());
                unsafe {
                    GsWSMATRIX = Matrix::default();
                    let org = org_before.clone().or(org_want).unwrap_or_default();
                    let p = (&raw mut GsWSMATRIX_ORG).cast::<u8>();
                    std::ptr::copy_nonoverlapping(org.as_ptr(), p, org.len().min(32));
                    let p = cx.scratch.ptr(rq.args[0] + 32);
                    std::ptr::write_bytes(p, 0, 8);
                }
            }
            let got_v0 = unsafe { call(&mut cx, &rq) };
            let mut problems: Vec<String> = vec![];
            if !returns_nothing(&rq.name) && got_v0 != want.v0 {
                problems.push(format!("v0 {:#x}, want {:#x}", got_v0, want.v0));
            }
            for (a, expect) in &want.mem {
                let got: Vec<u8> = if *a == WSMATRIX || *a == WSMATRIX_ORG {
                    // The bytes in place: copying the struct would read its padding as uninit.
                    let p: *const u8 = if *a == WSMATRIX {
                        (&raw const GsWSMATRIX).cast()
                    } else {
                        (&raw const GsWSMATRIX_ORG).cast()
                    };
                    unsafe { std::slice::from_raw_parts(p, expect.len()) }.to_vec()
                } else {
                    let p = cx.scratch.ptr(*a);
                    unsafe { std::slice::from_raw_parts(p, expect.len()) }.to_vec()
                };
                if got != *expect {
                    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
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
                    if compared(n, w) {
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
            if rq.name == "GsSetRefView2" {
                org_before = want
                    .mem
                    .iter()
                    .find(|(a, _)| *a == WSMATRIX_ORG)
                    .map(|(_, b)| b.clone());
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
                    eprintln!("MISMATCH {short}");
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
            "{compared_regs} registers compared, {skipped} requests skipped (the real SDK faulted)"
        );
        if child.is_some() {
            return;
        }
        assert_eq!(bad, 0, "{bad} requests differ from the real SDK");
        assert!(
            !partial,
            "DW1_GTE_PARTIAL inspects an incomplete run: it can't count as a pass"
        );
    }
}
