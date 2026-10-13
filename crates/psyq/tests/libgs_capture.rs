//! `libgs::sort_object4j` against 48 real `GsSortObject4J` calls captured in PCSX-Redux
//! (`oracle/capture-libgs.lua`; files under `DW1_LIBGS_CAPTURE`, default
//! `captures/oracle-runs/libgs-pin`, gitignored). Each `callNN_in.bin` is the whole machine at
//! the function's entry, `callNN_out.bin` at its return.

// Byte-to-word reads of dump files read better as chunks_exact than as_chunks.
#![allow(clippy::chunks_exact_to_as_chunks)]

#[cfg(not(have_libgs_capture))]
#[test]
#[ignore = "needs captures/oracle-runs/libgs-pin (or DW1_LIBGS_CAPTURE); local only"]
fn libgs_matches_capture() {}

#[cfg(have_libgs_capture)]
mod capture {
    use backend::gte::Gte;
    use psyq::libgs::{GS_OUT_PACKET_P, Memory, Ps1Memory, RAM_SIZE, sort_object4j};
    use std::path::{Path, PathBuf};

    /// One machine snapshot (`DW1STATE`: GPR[34], CP0[32], CP2D[32], CP2C[32], pc, 1 KiB
    /// scratchpad, 2 MiB RAM; all little-endian u32).
    struct State {
        gpr: [u32; 34],
        cp2d: [u32; 32],
        cp2c: [u32; 32],
        mem: Ps1Memory,
    }

    fn word(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
    }

    fn load(path: &Path) -> State {
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(&b[..8], b"DW1STATE", "{}", path.display());
        let regs = &b[8..8 + 131 * 4];
        let mut s = State {
            gpr: [0; 34],
            cp2d: [0; 32],
            cp2c: [0; 32],
            mem: Ps1Memory::new(),
        };
        for i in 0..34 {
            s.gpr[i] = word(regs, i);
        }
        for i in 0..32 {
            s.cp2d[i] = word(regs, 66 + i);
            s.cp2c[i] = word(regs, 98 + i);
        }
        let at = 8 + 131 * 4;
        s.mem.scratch.copy_from_slice(&b[at..at + 1024]);
        s.mem
            .ram
            .copy_from_slice(&b[at + 1024..at + 1024 + RAM_SIZE]);
        s
    }

    /// The GTE as the game left it. Skip the data registers whose writes have side effects or
    /// are read-only (SXYP moves the FIFO, IRGB rewrites IR1-3, ORGB and LZCR are derived).
    fn load_gte(s: &State) -> Gte {
        let mut g = Gte::new();
        for r in 0..32 {
            if !matches!(r, 15 | 28 | 29 | 31) {
                g.mtc2(r, s.cp2d[r]);
            }
        }
        for r in 0..32 {
            g.ctc2(r, s.cp2c[r]);
        }
        g
    }

    fn dir() -> PathBuf {
        match std::env::var_os("DW1_LIBGS_CAPTURE") {
            Some(d) => PathBuf::from(d),
            None => {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/oracle-runs/libgs-pin")
            }
        }
    }

    struct Call {
        index: usize,
        objp: u32,
        otp: u32,
        shift: u32,
        scratch: u32,
    }

    fn calls() -> Vec<Call> {
        let text = std::fs::read_to_string(dir().join("calls.txt")).unwrap();
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                let hex = |i: usize| u32::from_str_radix(f[i], 16).unwrap();
                Call {
                    index: f[0].parse().unwrap(),
                    objp: hex(2),
                    otp: hex(3),
                    shift: f[4].parse().unwrap(),
                    scratch: hex(5),
                }
            })
            .collect()
    }

    fn words(m: &Ps1Memory) -> impl Iterator<Item = u32> + '_ {
        m.ram
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
    }

    /// A GP0 polygon packet (tag, then colour/XY/texture words per vertex) in words.
    fn decode(m: &Ps1Memory, addr: u32) -> String {
        let tag = m.read32(addr);
        let len = (tag >> 24) as usize;
        let mut s = format!(
            "@{addr:08x} tag={tag:08x} (len {len}, link {:06x})",
            tag & 0xFF_FFFF
        );
        if len == 0 || len > 16 {
            return s;
        }
        let w: Vec<u32> = (0..len)
            .map(|i| m.read32(addr + 4 + 4 * i as u32))
            .collect();
        s += &format!("\n    cmd={:02x}", w[0] >> 24);
        for (k, v) in w.chunks(3).enumerate() {
            s += &format!(
                "\n    v{k}: rgb={:06x} xy=({}, {}) uv=({:02x}, {:02x}) aux={:04x}",
                v[0] & 0xFF_FFFF,
                v[1] as i16,
                (v[1] >> 16) as i16,
                v[2] & 0xFF,
                (v[2] >> 8) & 0xFF,
                v[2] >> 16
            );
        }
        s
    }

    /// Loads call `c`'s entry state and runs the Rust `sort_object4j` on it. Returns the entry
    /// and return states of the capture and the memory after the Rust call.
    fn run(c: &Call) -> (State, State, Ps1Memory) {
        let n = c.index;
        let input = load(&dir().join(format!("call{n:02}_in.bin")));
        let output = load(&dir().join(format!("call{n:02}_out.bin")));
        let mut mem = Ps1Memory::new();
        mem.ram.copy_from_slice(&input.mem.ram);
        mem.scratch = input.mem.scratch;
        let mut gte = load_gte(&input);
        sort_object4j(&mut mem, &mut gte, c.objp, c.otp, c.shift, c.scratch)
            .unwrap_or_else(|e| panic!("call {n}: {e:?}"));
        (input, output, mem)
    }

    /// Compares one call; returns a failure description.
    fn check(c: &Call) -> Result<(usize, usize), String> {
        let n = c.index;
        let (input, output, mem) = run(c);
        let p_in = input.mem.read32(GS_OUT_PACKET_P);
        let p_out = output.mem.read32(GS_OUT_PACKET_P);
        let sp = input.gpr[29];

        // OT: GsOT.org, 2^length entries.
        let ot_org = input.mem.read32(c.otp + 4) & 0x1F_FFFF;
        let ot_len = 4u32 << input.mem.read32(c.otp);
        let (pk0, pk1) = (p_in & 0x1F_FFFF, p_out & 0x1F_FFFF);
        let in_stack = |a: u32| a >= (sp & 0x1F_FFFF) - 0x1000 && a < (sp & 0x1F_FFFF);
        let mut bad = Vec::new();
        let mut noise = 0;
        for (i, ((a, o), r)) in words(&input.mem)
            .zip(words(&output.mem))
            .zip(words(&mem))
            .enumerate()
        {
            let ad = i as u32 * 4;
            // Rust never touches the emulated stack: the call's own frame there may change in the
            // capture, but Rust must leave every stack word as it was.
            if in_stack(ad) {
                if r != a {
                    bad.push((ad, o, r));
                }
                continue;
            }
            if a == o && a == r {
                continue;
            }
            let ours = (ad >= pk0 && ad < pk1)
                || (ad >= ot_org && ad < ot_org + ot_len)
                || ad == GS_OUT_PACKET_P & 0x1F_FFFF;
            if r == o {
                continue;
            }
            if !ours && r == a {
                // The capture changed something that is not libgs's output (a vblank interrupt
                // ran inside the call: BIOS kernel words, frame counters); Rust left it alone.
                noise += 1;
                continue;
            }
            bad.push((ad, o, r));
        }
        if bad.is_empty() && mem.scratch == output.mem.scratch {
            return Ok(((pk1 - pk0) as usize / 4, noise));
        }
        let mut msg = format!("call {n}: {} words differ", bad.len());
        for (ad, o, r) in bad.iter().take(6) {
            msg += &format!(
                "\n  {:08x}: capture {o:08x}, rust {r:08x}",
                ad | 0x8000_0000
            );
        }
        // First differing packet, decoded.
        let mut a = pk0;
        while a < pk1 {
            let len = output.mem.read32(a | 0x8000_0000) >> 24;
            let size = 4 + 4 * len;
            if (0..size / 4).any(|k| {
                output.mem.read32((a + 4 * k) | 0x8000_0000)
                    != mem.read32((a + 4 * k) | 0x8000_0000)
            }) {
                msg += &format!(
                    "\n  first differing packet\n  capture: {}\n  rust:    {}",
                    decode(&output.mem, a | 0x8000_0000),
                    decode(&mem, a | 0x8000_0000)
                );
                break;
            }
            if len == 0 {
                break;
            }
            a += size;
        }
        if mem.scratch != output.mem.scratch {
            msg += "\n  scratchpad differs";
        }
        Err(msg)
    }

    /// Draws an ordering table's chain through the software GPU: from `tag` (the OT's last
    /// entry) following the links until a terminator, each node's body as GP0 words.
    fn render(m: &Ps1Memory, tag: u32, vram_first: &[u8]) -> Vec<u16> {
        let mut gpu = backend::gpu::Gpu::new();
        for (px, b) in gpu.vram_mut().iter_mut().zip(vram_first.chunks_exact(2)) {
            *px = u16::from_le_bytes([b[0], b[1]]);
        }
        let mut node = tag;
        for _ in 0..1_000_000 {
            let t = m.read32(node);
            let len = t >> 24;
            if len > 0 {
                let body: Vec<u32> = (0..len).map(|i| m.read32(node + 4 + 4 * i)).collect();
                gpu.gp0(&body);
            }
            let link = t & 0xFF_FFFF;
            if link == 0xFF_FFFF || link == 0 {
                break;
            }
            node = link | 0x8000_0000;
        }
        gpu.vram().to_vec()
    }

    /// For a few calls: the ordering table after the Rust call and after the capture's return,
    /// each drawn through `backend::gpu` on the frame's first VRAM. The packets were compared
    /// word for word above; this checks that they draw, to identical VRAM. (The table holds every
    /// packet of the frame so far, the game's other primitives included.)
    #[test]
    fn libgs_packets_render_like_capture() {
        let vram_first = std::fs::read(dir().join("vram_first.bin")).unwrap();
        for c in calls()
            .iter()
            .filter(|c| [0, 15, 30, 47].contains(&c.index))
        {
            let (_, output, mem) = run(c);
            let tag = output.mem.read32(c.otp + 16);
            let a = render(&mem, tag, &vram_first);
            let b = render(&output.mem, tag, &vram_first);
            let drawn = a
                .iter()
                .zip(vram_first.chunks_exact(2))
                .filter(|(p, o)| **p != u16::from_le_bytes([o[0], o[1]]))
                .count();
            println!(
                "call {}: {drawn} pixels differ from the frame's first VRAM",
                c.index
            );
            assert!(drawn > 0, "call {}: nothing drawn", c.index);
            assert!(a == b, "call {}: VRAM after drawing the OT", c.index);
        }
    }

    #[test]
    fn libgs_matches_capture() {
        let mut failures = Vec::new();
        let (mut words_ok, mut noise_total) = (0, 0);
        let all = calls();
        assert_eq!(all.len(), 48, "calls.txt");
        for c in &all {
            match check(c) {
                Ok((w, n)) => {
                    words_ok += w;
                    noise_total += n;
                }
                Err(m) => failures.push(m),
            }
        }
        println!(
            "{} of {} calls match; {words_ok} packet words; {noise_total} interrupt-side words ignored",
            all.len() - failures.len(),
            all.len()
        );
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
