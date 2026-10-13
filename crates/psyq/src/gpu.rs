//! The game's GPU: one `backend::gpu::Gpu` (VRAM and the software rasterizer), driven by
//! libgpu and libgs. Drawing is synchronous: a packet runs when it's submitted, so `DrawSync`
//! never waits and VRAM is always up to date.

use backend::gpu::Gpu;
use std::sync::{LazyLock, Mutex};

use crate::memory::{LINK_END, ptr_of_link};

static GPU: LazyLock<Mutex<Gpu>> = LazyLock::new(|| Mutex::new(Gpu::new()));

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
                return;
            }
            p = ptr_of_link(tag);
        }
        eprintln!("psyq: GPU packet list at {head:p} doesn't end (a loop?)");
        std::process::abort();
    });
}
