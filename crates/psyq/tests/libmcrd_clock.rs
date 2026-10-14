//! libmcrd through its exported calls and the virtual clock: the main-menu card flow as
//! PCSX-Redux ran it (`oracle/capture-calls.lua` in captures/oracle-runs/mcrd-timing and
//! mcrd-idle; VBlank numbers below are that recording's), then each operation timed in
//! isolation (`oracle/call-sdk.lua`, mcrd-lat). Its own test binary: the clock and the card
//! library are global, and the test runs in one order.

use std::ffi::{CString, c_char};

use psyq::libetc::{VSync, vblank_count};
use psyq::libmcrd::*;

/// The recording's first card command (the boot's exist) is at 2088.
const FIRST: i64 = 2088;

struct Game {
    base: u32,
    chan: i32,
    /// Recording-numbered VBlank and kind of every exist the loop issued.
    exists: Vec<i64>,
    /// First VBlank each exist was seen done by a poll.
    done: Vec<i64>,
    pending: Option<i64>,
}

impl Game {
    fn abs(&self) -> i64 {
        (vblank_count() - self.base) as i64 + FIRST
    }

    /// `updateMemoryCardState`: poll `MemCardSync(1)`, restart the exist when idle.
    fn card_tick(&mut self) {
        let (mut cmd, mut res) = (0u32, 0u32);
        match unsafe { MemCardSync(1, &mut cmd, &mut res) } {
            -1 => {
                assert_eq!(MemCardExist(self.chan), 1);
                self.exists.push(self.abs());
                self.pending = Some(self.abs());
                self.chan ^= 0x10;
            }
            1 => {
                assert_eq!(cmd, MC_FUNC_EXIST);
                self.done.push(self.abs());
                self.pending = None;
            }
            _ => {}
        }
    }

    /// The tick's end: the next VBlank.
    fn end_tick(&mut self) {
        VSync(0);
    }

    fn sync0(&mut self) -> (i32, u32, u32) {
        let (mut cmd, mut res) = (0u32, 0u32);
        let r = unsafe { MemCardSync(0, &mut cmd, &mut res) };
        (r, cmd, res)
    }

    fn run_to(&mut self, until: i64) {
        while self.abs() < until {
            self.card_tick();
            self.end_tick();
        }
    }
}

fn name() -> CString {
    CString::new("BASLUS-01032DMR0").unwrap()
}

#[test]
fn card_flow_and_operations_on_the_clock() {
    MemCardInit(1);
    MemCardStart();
    let mut g = Game {
        base: vblank_count(),
        chan: 0x10,
        exists: Vec::new(),
        done: Vec::new(),
        pending: None,
    };

    // The boot's exist at 2088 is done at 2091; the loop restarts at 2092 and every 5 VBlanks.
    g.run_to(2089);
    assert_eq!(g.exists, [2088]);
    g.run_to(2602);
    assert_eq!(g.done[..2], [2091, 2096]);
    assert_eq!(g.exists[..3], [2088, 2092, 2097]);

    // Tick 2602 issued an exist; tick 2603 (the player pressed Cross): the menu waits for it
    // (done 2606), accepts (blocks until 2684), and the tick ends at 2685 [obs: accept call at
    // 2606, the next poll at 2685].
    assert_eq!(g.abs(), 2602);
    g.card_tick();
    g.end_tick();
    assert_eq!(g.abs(), 2603);
    g.card_tick();
    assert_eq!(g.sync0().0, 1);
    assert_eq!(g.abs(), 2606);
    assert_eq!(MemCardAccept(0), 1);
    let (r, cmd, res) = g.sync0();
    assert_eq!((r, cmd, res), (1, MC_FUNC_ACCEPT, 0));
    assert_eq!(g.abs(), 2684);
    g.end_tick();
    assert_eq!(g.abs(), 2685);

    // Tick 2685: the loop's exist (done 3 later, right after the accept), blocked on at 2688,
    // then two directory reads of 80 VBlanks each (2688, 2768, 2848), and the tick ends 2849.
    g.card_tick();
    assert_eq!(*g.exists.last().unwrap(), 2685);
    assert_eq!(g.sync0().0, 1);
    assert_eq!(g.abs(), 2688);
    let pat = CString::new("*").unwrap();
    let mut dir = [DirEntry {
        name: [0; 20],
        attr: 0,
        size: 0,
        next: std::ptr::null_mut(),
        head: 0,
        system: [0; 4],
    }; 15];
    let mut files = 0i32;
    for end in [2768, 2848] {
        let r = unsafe {
            MemCardGetDirentry(
                0,
                pat.as_ptr() as *const c_char,
                dir.as_mut_ptr(),
                &mut files,
                0,
                15,
            )
        };
        assert_eq!((r, files), (0, 0));
        assert_eq!(g.abs(), end);
    }
    g.end_tick();
    assert_eq!(g.abs(), 2849);

    // The loop resumes: an exist at 2849 done at 2852 (3), then every 5 VBlanks.
    g.run_to(2860);
    assert!(g.exists.ends_with(&[2849, 2853, 2858]), "{:?}", g.exists);
    assert!(g.done.ends_with(&[2852, 2857]), "{:?}", g.done);

    // The periodic re-check: the exist at 3108 is done at 3204 (96 VBlanks), the loop's next
    // exist is 3205 [obs: calls 1888 and the sync at 3204].
    g.run_to(3205);
    assert!(
        g.exists.contains(&3103) && g.exists.contains(&3108),
        "{:?}",
        g.exists
    );
    assert!(g.exists.iter().all(|&e| !(3109..3205).contains(&e)));
    assert!(g.done.contains(&3204));
    g.card_tick();
    assert_eq!(*g.exists.last().unwrap(), 3205);
    g.end_tick();
    g.run_to(3209);

    // Tick 3209 (menu 0x10): the exist is done, the menu's own poll sees idle (-1), and
    // CreateFile returns 10 VBlanks later; the tick ends at 3220 [obs: 3209, next poll 3220].
    assert_eq!(g.abs(), 3209);
    g.card_tick();
    assert_eq!(*g.done.last().unwrap(), 3209);
    let (mut c2, mut r2) = (0u32, 0u32);
    assert_eq!(unsafe { MemCardSync(1, &mut c2, &mut r2) }, -1);
    assert_eq!(unsafe { MemCardCreateFile(0, name().as_ptr(), 1) }, 0);
    assert_eq!(g.abs(), 3219);
    g.end_tick();
    assert_eq!(g.abs(), 3220);

    // Tick 3220 (menu 0x11): the loop's exist, waited on (3224), then the save is written:
    // 136 VBlanks, and the tick ends at 3361 [obs: write at 3224, the next poll at 3361].
    g.card_tick();
    assert_eq!(*g.exists.last().unwrap(), 3220);
    assert_eq!(g.sync0().0, 1);
    assert_eq!(g.abs(), 3224);
    let save = vec![0x5Au8; 0x2000];
    let w = unsafe { MemCardWriteFile(0, name().as_ptr(), save.as_ptr(), 0, 0x2000) };
    assert_eq!(w, 1);
    assert_eq!(g.sync0(), (1, MC_FUNC_WRITE_FILE, 0));
    assert_eq!(g.abs(), 3360);
    g.end_tick();
    assert_eq!(g.abs(), 3361);

    // The loop resumes: exist at 3361, done at 3364 (3 VBlanks, right after the write).
    g.card_tick();
    assert_eq!(*g.exists.last().unwrap(), 3361);
    g.end_tick();
    g.run_to(3366);
    assert_eq!(g.done.last().copied(), Some(3364));
    // And the next two re-checks: exists at 4830 and 6552 done at 4926 and 6646, the loop's
    // exists at 4927 and 6647 [obs: mcrd-idle].
    g.run_to(6660);
    for (start, done, next) in [(4830, 4926, 4927), (6552, 6646, 6647)] {
        assert!(g.exists.contains(&start), "exist at {start}");
        assert!(g.exists.contains(&next), "exist at {next}");
        assert!(g.exists.iter().all(|&e| !(start + 1..next).contains(&e)));
        assert!(g.done.contains(&done), "done at {done}");
        assert!(g.done.iter().all(|&d| !(start + 4..done).contains(&d)));
    }

    // Each operation in isolation, as the call-sdk measured it, through blocking syncs.
    // Drain the loop's exist, then idle.
    g.sync0();
    let mut buf = vec![0u8; 0x2000];
    let op = |g: &mut Game, f: &mut dyn FnMut() -> i32, took: i64, status: i32, result: u32| {
        let t = g.abs();
        let r = f();
        if status >= 0 {
            // A synchronous command: the status is its return value.
            assert_eq!(r, status);
        } else {
            assert_eq!(r, 1);
            let (s, _, res) = g.sync0();
            assert_eq!((s, res), (1, result));
        }
        assert_eq!(g.abs() - t, took, "operation at {t}");
    };
    let n = name();
    let missing = CString::new("BASLUS-01032DMR1").unwrap();
    let p = buf.as_mut_ptr();
    // Reads: 128, 8192 bytes (8, 134); a missing file (6).
    op(
        &mut g,
        &mut || unsafe { MemCardReadFile(0, n.as_ptr(), p, 0, 128) },
        8,
        -1,
        0,
    );
    op(
        &mut g,
        &mut || unsafe { MemCardReadFile(0, n.as_ptr(), p, 0, 0x2000) },
        134,
        -1,
        0,
    );
    op(
        &mut g,
        &mut || unsafe { MemCardReadFile(0, missing.as_ptr(), p, 0, 0x2000) },
        6,
        -1,
        5,
    );
    // Writes: 128 bytes (10); a missing file (6).
    op(
        &mut g,
        &mut || unsafe { MemCardWriteFile(0, n.as_ptr(), p, 0, 128) },
        10,
        -1,
        0,
    );
    op(
        &mut g,
        &mut || unsafe { MemCardWriteFile(0, missing.as_ptr(), p, 0, 128) },
        6,
        -1,
        5,
    );
    // Create of an existing name: result 6 after 2.
    op(
        &mut g,
        &mut || unsafe { MemCardCreateFile(0, n.as_ptr(), 1) },
        2,
        6,
        0,
    );
    // Delete: a missing file 81, an existing one-block file 7.
    op(
        &mut g,
        &mut || unsafe { MemCardDeleteFile(0, missing.as_ptr()) },
        81,
        5,
        0,
    );
    op(
        &mut g,
        &mut || unsafe { MemCardDeleteFile(0, n.as_ptr()) },
        7,
        0,
        0,
    );
    // Format a formatted card: 72.
    op(&mut g, &mut || MemCardFormat(0), 72, 0, 0);
    // A blocking exist and accept.
    op(&mut g, &mut || MemCardExist(0), 4, -1, 0);
    op(&mut g, &mut || MemCardAccept(0), 78, -1, 0);
}
