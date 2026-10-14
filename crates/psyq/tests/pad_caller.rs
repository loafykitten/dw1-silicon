//! `PadRead` hands the host the return address into the code that called it (replays name
//! the caller from it). Its own test binary: the libetc clock is global.

use std::sync::atomic::{AtomicUsize, Ordering};

static CALLER: AtomicUsize = AtomicUsize::new(0);

struct Recorder;

impl psyq::libetc::Host for Recorder {
    fn vblank(&mut self, _vblank: u32) {}
    fn pad(&mut self, caller: usize) -> u32 {
        CALLER.store(caller, Ordering::SeqCst);
        0
    }
}

/// Stands in for a game function that polls the pad.
#[inline(never)]
extern "C" fn game_poll() -> u32 {
    std::hint::black_box(psyq::libetc::PadRead(0))
}

#[test]
fn pad_read_reports_its_caller() {
    psyq::libetc::set_host(Box::new(Recorder));
    game_poll();
    let start = game_poll as *const () as usize;
    let caller = CALLER.load(Ordering::SeqCst);
    assert!(
        (start..start + 0x100).contains(&caller),
        "caller {caller:#x} is not in game_poll at {start:#x}"
    );
}
