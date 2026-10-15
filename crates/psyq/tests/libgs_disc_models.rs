//! Every model on the disc sorts natively: each TMD (the `.TMD` files and the TMDs inside other
//! files: character models, effect banks, overlays) is mapped and linked as the game does and every
//! object goes through `GsSortObject4J` with the handlers the game installs, from several views
//! (attribute 0; `GsDIV1` too for the arenas' floors, STDDAT\*_TOGI.TMD object 0, as STD and VS
//! draw them). None may reach a kind without a native handler or any other stop: this is the
//! check that the handlers cover every primitive kind DW1's models use (82,411 primitives, the
//! survey in `docs/phase9-render.md`).
//!
//! Needs the disc image (`disc/Digimon World (USA).cue`, or `DW1_DISC`); without it the test is
//! ignored.

#[cfg_attr(not(have_disc_image), ignore = "disc image not found")]
#[test]
fn every_disc_model_sorts_natively() {
    #[cfg(have_disc_image)]
    sort::run();
}

#[cfg(have_disc_image)]
mod sort {
    use backend::gte::Gte;
    use psyq::globals::{GS_FCALL_SLOTS, GsFCALL4};
    use psyq::libgs::*;
    use psyq::memory::dw1_scratchpad;

    /// The TMD being sorted, the ordering table and the packet area: statics, in the link window
    /// like the game's.
    static mut TMD: [u32; 1 << 18] = [0; 1 << 18];
    static mut OT: [u32; 1 << 14] = [0; 1 << 14];
    static mut PACKETS: [u32; 1 << 20] = [0; 1 << 20];

    /// `initializeGsTMDMap`'s table (decomp/src/main/graphics.c), with the native handlers; the
    /// slots whose handlers are still trapping stubs stay empty (the sort stops on them too).
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
            (&raw mut GsFCALL4).write([None; GS_FCALL_SLOTS]);
            for (slot, f) in slots {
                GsFCALL4[slot] = Some(std::mem::transmute::<TmdHandler, unsafe extern "C" fn()>(f));
            }
        }
    }

    /// Whether `data` holds a TMD at `off` whose primitives all have known kinds and lengths;
    /// returns its size in bytes (up to the end of its last list).
    fn tmd_at(data: &[u8], off: usize) -> Option<usize> {
        let w = |o: usize| -> Option<u32> {
            data.get(o..o + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        if w(off)? != 0x41 || w(off + 4)? != 0 {
            return None;
        }
        let n = w(off + 8)? as usize;
        if n == 0 || n >= 200 {
            return None;
        }
        let table = off + 12;
        let mut end = table + 28 * n;
        for i in 0..n {
            let o = table + 28 * i;
            let (vt, nv, nt, nn, pt, np) = (
                w(o)?,
                w(o + 4)?,
                w(o + 8)?,
                w(o + 12)?,
                w(o + 16)?,
                w(o + 20)?,
            );
            end = end.max(table + vt as usize + 8 * nv as usize);
            end = end.max(table + nt as usize + 8 * nn as usize);
            let mut p = table + pt as usize;
            for _ in 0..np {
                let h = w(p)?;
                let (ilen, flag, mode) = ((h >> 8) as u8, (h >> 16) as u8, (h >> 24) as u8);
                let words = primitive_words(flag, mode)?;
                if words != usize::from(ilen) {
                    return None;
                }
                p += 4 + 4 * words;
            }
            end = end.max(p);
        }
        (end <= data.len()).then_some(end - off)
    }

    fn view(g: &mut Gte, seed: u32, distance: i32) {
        // A rotation about two axes by angles from the seed, the object `distance` ahead.
        let a = (seed as f64) * 0.7;
        let b = (seed as f64) * 1.3;
        let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
        let m = [cb, 0.0, sb, sa * sb, ca, -sa * cb, -ca * sb, sa, ca * cb];
        let q: Vec<i32> = m.iter().map(|v| (v * 4096.0).round() as i32).collect();
        let pair = |x: i32, y: i32| (x as u32 & 0xFFFF) | (y as u32) << 16;
        let c = [
            pair(q[0], q[1]),
            pair(q[2], q[3]),
            pair(q[4], q[5]),
            pair(q[6], q[7]),
            q[8] as u32 & 0xFFFF,
            0,
            0,
            distance as u32,
            pair(-2000, 1000),
            pair(3000, 0),
            pair(-1000, 2000),
            pair(0, 0),
            0,
            0x800,
            0x800,
            0x800,
            pair(0xC00, 0),
            pair(0, 0xC00),
            pair(0, 0),
            pair(0xC00, 0),
            0,
            0,
            0,
            0,
            160 << 16,
            120 << 16,
            0x3E8,
            0xFFFF_EF9E,
            0x0140_0000,
            0x155,
            0x100,
        ];
        for (r, v) in c.iter().enumerate() {
            g.ctc2(r, *v);
        }
    }

    pub fn run() {
        let disc = pack::Disc::open(env!("DW1_TEST_DISC_IMAGE")).unwrap();
        GsInitGraph(320, 240, 4, 0, 0);
        install_handlers();
        let (mut models, mut objects, mut prims, mut words) = (0, 0, 0usize, 0usize);
        for (path, _) in disc.files().unwrap() {
            let data = disc.read_file(&path).unwrap().unwrap();
            let offsets: Vec<usize> = if path.to_ascii_uppercase().ends_with(".TMD") {
                vec![0]
            } else {
                (0..data.len().saturating_sub(12)).step_by(4).collect()
            };
            for off in offsets {
                let Some(size) = tmd_at(&data, off) else {
                    continue;
                };
                if size > 4 << 18 {
                    panic!("{path}+{off:#x}: TMD of {size} bytes is bigger than the buffer");
                }
                models += 1;
                let floor = path.contains("_TOGI.TMD");
                unsafe {
                    let tmd = (&raw mut TMD).cast::<u32>();
                    let bytes = std::slice::from_raw_parts_mut(tmd.cast::<u8>(), size);
                    bytes.copy_from_slice(&data[off..off + size]);
                    GsMapModelingData(tmd.add(1));
                    let n = tmd.add(2).read() as usize;
                    for i in 0..n {
                        let mut obj = GsDobj2 {
                            attribute: 0,
                            coord2: std::ptr::null_mut(),
                            tmd: std::ptr::null_mut(),
                            id: 0,
                        };
                        dw1_GsLinkObject4(tmd.add(3) as usize, &mut obj, i as i32);
                        objects += 1;
                        prims += obj.tmd.add(5).read() as usize;
                        let attributes: &[u32] = if floor && i == 0 { &[0, 0x200] } else { &[0] };
                        for &attribute in attributes {
                            obj.attribute = attribute;
                            for (k, distance) in [(1, 1500), (2, 600), (3, 4000), (4, 300)] {
                                let mut ot = GsOt {
                                    length: 14,
                                    org: (&raw mut OT).cast(),
                                    offset: 0,
                                    point: 0,
                                    tag: std::ptr::null_mut(),
                                };
                                GsClearOt(0, 0, &mut ot);
                                let base = (&raw mut PACKETS).cast::<u32>();
                                GsSetWorkBase(base.cast());
                                psyq::gte::with(|g| view(g, k * 7 + i as u32, distance));
                                GsSortObject4J(
                                    &mut obj,
                                    &mut ot,
                                    2,
                                    (&raw mut dw1_scratchpad).cast(),
                                );
                                words += (GsGetWorkBase() as usize - base as usize) / 4;
                            }
                        }
                    }
                }
            }
        }
        println!(
            "{models} TMDs, {objects} objects, {prims} primitives sorted from 4+ views ({words} packet words)"
        );
        assert!(prims > 80_000, "found only {prims} primitives");
    }
}
