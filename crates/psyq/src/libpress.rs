//! libpress: the MDEC calls the movie player uses (`DecDCTReset`, `DecDCTvlcBuild`,
//! `DecDCTvlc2`, `DecDCTin`, `DecDCTout`, `DecDCToutCallback`), on `backend::mdec`.
//!
//! How MOV drives it (src/mov/mov.c): `MOV_startStream` resets the MDEC and installs
//! `MOV_onSliceDecoded` with `DecDCToutCallback`; `MOV_decodeFrame` turns a frame's bitstream
//! into run-length codes with `DecDCTvlc2` into one of two buffers; `MOV_playMovie` then calls
//! `DecDCTin(codes, 3)` and `DecDCTout(slice, 24 * height / 2)` for the first 16-pixel-wide slice
//! of the frame. On the PS1 the MDEC's output DMA interrupt then calls the callback, which
//! starts the next slice's `DecDCTout` and uploads the finished one with `LoadImage`; the last
//! slice's callback sets `flipPending`, which `MOV_waitForFlip` waits for while the main loop
//! decodes the next frame's codes.
//!
//! **Native model:** decoding is synchronous. `DecDCTout` decodes its slice, then runs the
//! callback; a `DecDCTout` the callback issues is queued and runs when the callback returns
//! (the interrupt that would start it on the PS1), not recursively. By the time the outer
//! `DecDCTout` returns, the whole frame has been decoded and uploaded, which is the state
//! `MOV_waitForFlip` finds on the PS1 when it stops waiting. A game that polled
//! `flipPending` before the outer call returned would see the end state at once; MOV doesn't.
//!
//! Rules are tagged `[doc]` (the Psy-Q library reference and `libpress.h`, psx-spx's MDEC
//! chapter) or `[obs]` (what MOV's calls show).

use std::collections::VecDeque;
use std::sync::Mutex;

use backend::mdec::{Mdec, vlc};

/// Words of run-length codes a frame can produce: the game's buffers are 0x28000 bytes
/// (`MOV_RUN_LEVEL_BUFFER_*`) and `DecDCTvlc2` is not told a size. [obs: dw/mov.h]
const VLC_BUFFER_WORDS: usize = 0x28000 / 4;

type Callback = unsafe extern "C" fn();

struct State {
    mdec: Mdec,
    callback: Option<Callback>,
    in_callback: bool,
    /// `DecDCTout` calls made from inside the callback: (buffer address, words).
    queued: VecDeque<(usize, usize)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap();
    f(guard.get_or_insert_with(|| State {
        mdec: Mdec::new(),
        callback: None,
        in_callback: false,
        queued: VecDeque::new(),
    }))
}

/// `DecDCTReset(mode)`: resets the MDEC (0: everything, 1: just the data path), restoring the
/// default quantisation and scale tables the decoder uses. The callback set by
/// `DecDCToutCallback` stays [guess: the SDK's reset leaves its callback table alone; MOV sets
/// the callback right after]. [doc]
#[unsafe(no_mangle)]
pub extern "C" fn DecDCTReset(_mode: i32) {
    with_state(|s| s.mdec = Mdec::new());
}

/// `DecDCToutCallback(func)`: the function called when `DecDCTout` has filled its buffer;
/// NULL removes it. Returns 0 [doc: the previous callback or 0; MOV ignores it].
#[unsafe(no_mangle)]
pub extern "C" fn DecDCToutCallback(func: Option<Callback>) -> i32 {
    with_state(|s| s.callback = func);
    0
}

/// `DecDCTvlcBuild(table)`: builds the decoding table `DecDCTvlc2` takes (34,816 halfwords).
/// The decoder here has its own tables, so `table` is left as the game made it. [doc]
///
/// # Safety
/// `table` is not dereferenced.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DecDCTvlcBuild(_table: *mut u16) {}

/// Run-length codes for one frame's bitstream: `bs` is the frame (an 8-byte header, then the
/// bitstream), `buf` the code buffer. Returns 0, or -1 when the bitstream is bad (`buf[0]` is
/// then an empty stream, so the MDEC draws black). [doc: libpress]
pub fn vlc2(bs: &[u8], buf: &mut [u32]) -> i32 {
    match vlc(bs, buf) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("psyq: DecDCTvlc2: bad bitstream ({e:?})");
            -1
        }
    }
}

/// `DecDCTvlc2(bs, buf, table)`: variable-length decodes a frame into MDEC run-length codes.
/// Returns 0 on success, -1 on a bad frame. The frame's length comes from the stream
/// (`StGetNext` handed out `bs`), since the call is not given one. [doc]
///
/// # Safety
/// `bs` must be a pointer `StGetNext` returned (its frame not yet freed or still readable), and
/// `buf` must have room for 0x28000 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DecDCTvlc2(bs: *const u8, buf: *mut u32, _table: *const u16) -> i32 {
    let Some(len) = crate::libcd::stream_frame_len(bs) else {
        eprintln!("psyq: DecDCTvlc2: {bs:p} isn't a frame StGetNext handed out");
        std::process::abort();
    };
    let bs = unsafe { std::slice::from_raw_parts(bs, len) };
    let buf = unsafe { std::slice::from_raw_parts_mut(buf, VLC_BUFFER_WORDS) };
    vlc2(bs, buf)
}

/// `DecDCTin(buf, mode)`: gives the MDEC run-length codes (`buf`: a `0x3800nnnn` word, then
/// `nnnn` words) and the output format: `mode` bit 0 selects 24-bit output (else 15-bit), bit 1
/// sets the 15-bit mask bit. [doc; obs: MOV passes 3 and displays the result as 24-bit, 480
/// halfwords for 320 pixels]
///
/// # Safety
/// `buf` must hold the header word and the words it announces.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DecDCTin(buf: *const u32, mode: i32) {
    let words = (unsafe { buf.read() } & 0xFFFF) as usize + 1;
    let input = unsafe { std::slice::from_raw_parts(buf, words) };
    with_state(|s| s.mdec.start(input, mode & 1 != 0, mode & 2 != 0));
}

/// `DecDCTout(buf, size)`: decodes `size` words of pixels into `buf` (whole 16x16 macroblocks,
/// 24-bit: 192 words each), then runs the `DecDCToutCallback` function.
///
/// Called from inside that function, it only queues the request: the callback has returned
/// by the time the slice is decoded, as the DMA interrupt would have it on the PS1. See the
/// module notes.
///
/// # Safety
/// `buf` must have room for `size` words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DecDCTout(buf: *mut u32, size: i32) {
    let size = usize::try_from(size).unwrap_or(0);
    // A request from inside the callback is queued for the loop below.
    let queued = with_state(|s| {
        s.in_callback && {
            s.queued.push_back((buf as usize, size));
            true
        }
    });
    if queued {
        return;
    }
    let mut next = Some((buf as usize, size));
    while let Some((addr, words)) = next.take() {
        let callback = with_state(|s| {
            let out = unsafe { std::slice::from_raw_parts_mut(addr as *mut u32, words) };
            s.mdec.out(out);
            s.callback
        });
        let Some(callback) = callback else {
            return; // no interrupt handler: the buffer is just filled
        };
        with_state(|s| s.in_callback = true);
        unsafe { callback() };
        next = with_state(|s| {
            s.in_callback = false;
            s.queued.pop_front()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// One test owns the global MDEC state.
    static SLICES: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
    static CALLS: AtomicU32 = AtomicU32::new(0);
    static DEPTH: AtomicU32 = AtomicU32::new(0);
    static MAX_DEPTH: AtomicU32 = AtomicU32::new(0);
    /// The next slice buffers the callback asks for, most recent last.
    static NEXT: Mutex<Vec<(usize, i32)>> = Mutex::new(Vec::new());

    unsafe extern "C" fn callback() {
        let depth = DEPTH.fetch_add(1, Ordering::SeqCst) + 1;
        MAX_DEPTH.fetch_max(depth, Ordering::SeqCst);
        CALLS.fetch_add(1, Ordering::SeqCst);
        // Like MOV_onSliceDecoded: start the next slice, if there is one.
        let next = NEXT.lock().unwrap().pop();
        if let Some((addr, words)) = next {
            SLICES.lock().unwrap().push((addr, words as usize));
            unsafe { DecDCTout(addr as *mut u32, words) };
        }
        DEPTH.fetch_sub(1, Ordering::SeqCst);
    }

    /// Two macroblocks, flat grey then a darker flat grey: six blocks each, a DC only.
    fn codes() -> Vec<u32> {
        let mut halves: Vec<u16> = Vec::new();
        for dc in [0x20u16, 0x10] {
            for block_dc in [0, 0, dc, dc, dc, dc] {
                halves.push(block_dc & 0x3FF); // q 0: DC only, scaled by 2
                halves.push(0xFE00);
            }
        }
        while !halves.len().is_multiple_of(64) {
            halves.push(0xFE00);
        }
        let mut v = vec![0x3800_0000 | (halves.len() / 2) as u32];
        v.extend(
            halves
                .chunks(2)
                .map(|h| u32::from(h[0]) | u32::from(h[1]) << 16),
        );
        v
    }

    #[test]
    fn slices_decode_in_order_and_the_callback_chain_does_not_recurse() {
        let codes = codes();
        let mut a = vec![0u32; 192];
        let mut b = vec![0u32; 192];
        DecDCTReset(0);
        assert_eq!(DecDCToutCallback(Some(callback)), 0);
        unsafe { DecDCTin(codes.as_ptr(), 3) };
        // Two slices of one macroblock: the callback after the first asks for the second (into
        // `b`), the callback after that asks for nothing.
        NEXT.lock().unwrap().push((b.as_mut_ptr() as usize, 192));
        unsafe { DecDCTout(a.as_mut_ptr(), 192) };
        // The whole chain ran before DecDCTout returned, one callback at a time.
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(
            MAX_DEPTH.load(Ordering::SeqCst),
            1,
            "callback ran inside callback"
        );
        assert_eq!(
            SLICES.lock().unwrap().len(),
            1,
            "one request came from the callback"
        );
        let grey = |s: &[u32]| s.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>();
        let (ga, gb) = (grey(&a), grey(&b));
        assert!(ga.iter().all(|&x| x == ga[0]) && gb.iter().all(|&x| x == gb[0]));
        assert!(
            ga[0] > gb[0],
            "first macroblock is the lighter one: {} {}",
            ga[0],
            gb[0]
        );

        // Without a callback the slice is just filled.
        DecDCToutCallback(None);
        let mut c = vec![0u32; 192];
        unsafe { DecDCTin(codes.as_ptr(), 3) };
        unsafe { DecDCTout(c.as_mut_ptr(), 192) };
        assert_eq!(c, a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn vlc2_reports_bad_frames() {
        let mut buf = vec![7u32; 8];
        assert_eq!(vlc2(&[0; 16], &mut buf), -1);
        assert_eq!(buf[0], 0x3800_0000);
    }
}
