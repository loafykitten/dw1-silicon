//! The game's GPU: one `backend::gpu::Gpu` (VRAM and the software rasterizer), driven by
//! libgpu and libgs. Drawing is synchronous: a packet runs when it's submitted, so `DrawSync`
//! never waits and VRAM is always up to date.

use backend::gpu::Gpu;
use std::sync::{LazyLock, Mutex};

use crate::memory::{LINK_END, ptr_of_link};

/// The game's GPU. `DW1_SCALE=<1..8>` turns the HD surfaces on at that render scale (1, the
/// default, is off: nothing but the 1x picture); `DW1_HD_THREADS=<n>` sets the drawing threads
/// (default one per core, at most 8); `DW1_PGXP=1` draws 3D polygons on the HD surfaces at
/// their precise projected positions ("no wobble"; nothing changes at scale 1).
static GPU: LazyLock<Mutex<Gpu>> = LazyLock::new(|| {
    let mut gpu = Gpu::new();
    let env = |name: &str| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
    };
    if let Some(n) = env("DW1_HD_THREADS") {
        gpu.set_hd_threads(n as usize);
    }
    if let Some(s) = env("DW1_SCALE") {
        gpu.set_scale(s);
    }
    backend::pgxp::set_enabled(gpu.scale() > 1 && env("DW1_PGXP") == Some(1));
    Mutex::new(gpu)
});

/// Runs `f` on the GPU. The game is single-threaded; the lock only keeps this sound.
pub fn with<R>(f: impl FnOnce(&mut Gpu) -> R) -> R {
    f(&mut GPU.lock().unwrap())
}

/// Longest chain `draw_chain` follows before deciding the list loops.
const MAX_PACKETS: usize = 1 << 20;

/// Runs a linked list of packets starting at `head`, as the GPU's DMA does for an ordering
/// table [doc: psx-spx "GPU DMA, linked list"]: each packet is a tag word (bits 0-23 the link
/// to the next packet, bits 24-31 the number of command words after the tag) followed by
/// its GP0 words. The list ends at the link 0xFFFFFF.
///
/// # Safety
/// `head` and every packet it links to must be valid for their tag and command words.
pub unsafe fn draw_chain(head: *const u32) {
    let mut p = head;
    let mut words = Vec::with_capacity(16);
    with(|gpu| {
        for _ in 0..MAX_PACKETS {
            if p.is_null() {
                return;
            }
            let tag = unsafe { p.read() };
            let len = (tag >> 24) as usize;
            if len > 0 {
                words.clear();
                words.extend((1..=len).map(|i| unsafe { p.add(i).read() }));
                gpu.gp0(&words);
            }
            if tag & LINK_END == LINK_END {
                // The frame is drawn: its projections are done with.
                backend::pgxp::end_frame();
                return;
            }
            p = ptr_of_link(tag);
        }
        eprintln!("psyq: GPU packet list at {head:p} doesn't end (a loop?)");
        std::process::abort();
    });
}
