//! The movie calls end to end on a synthetic disc (no game data): two one-sector STR frames
//! with an XA audio sector between them, streamed with `CdRead2` and the `St*` calls, decoded
//! with `DecDCTvlc2`/`DecDCTin`/`DecDCTout` and a callback like MOV's, on the virtual clock.
//! One test: the drive, the clock and the MDEC are global.

use psyq::libcd::*;
use psyq::libetc::{Host, ResetCallback, VSync, set_host, vblank_count};
use psyq::libpress::*;
use psyq::stream::START_LATENCY;
use std::sync::{Arc, Mutex};

const FIRST_LBA: usize = 24;
const RAW: usize = 2352;

/// A bitstream of one flat macroblock (version 2, quantiser 1): the six blocks have DC only,
/// the luma ones all 0x20 (grey), then the end-of-frame code.
fn bitstream(luma_dc: u32) -> Vec<u8> {
    let mut bits = String::new();
    for dc in [0, 0, luma_dc, luma_dc, luma_dc, luma_dc] {
        bits += &format!("{dc:010b}10");
    }
    bits += "0111111111";
    while !bits.len().is_multiple_of(16) {
        bits.push('0');
    }
    let mut bs = vec![32, 0, 0x00, 0x38, 1, 0, 2, 0];
    for ch in bits.as_bytes().chunks(16) {
        let w = ch.iter().fold(0u16, |a, &c| a << 1 | u16::from(c - b'0'));
        bs.extend_from_slice(&w.to_le_bytes());
    }
    bs
}

/// A raw video sector: the 32-byte STR header and the bitstream in a one-sector frame.
fn video_sector(frame: u32, bs: &[u8]) -> Vec<u8> {
    let mut s = vec![0u8; RAW];
    s[18] = 0x08;
    let d = &mut s[24..];
    d[0..2].copy_from_slice(&0x0160u16.to_le_bytes());
    d[2..4].copy_from_slice(&0x8001u16.to_le_bytes());
    d[6..8].copy_from_slice(&1u16.to_le_bytes());
    d[8..12].copy_from_slice(&frame.to_le_bytes());
    d[12..16].copy_from_slice(&(bs.len() as u32).to_le_bytes());
    d[16..18].copy_from_slice(&16u16.to_le_bytes());
    d[18..20].copy_from_slice(&16u16.to_le_bytes());
    d[32..32 + bs.len()].copy_from_slice(bs);
    s
}

struct Recorder(Arc<Mutex<Vec<u32>>>);

impl Host for Recorder {
    fn vblank(&mut self, vblank: u32) {
        self.0.lock().unwrap().push(vblank);
    }
    fn pad(&mut self, _caller: usize) -> u32 {
        0
    }
}

static SLICES: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// MOV_onSliceDecoded's shape: one slice per frame here, so it only counts.
unsafe extern "C" fn on_slice() {
    SLICES.lock().unwrap().push(vblank_count());
}

#[test]
fn two_frames_stream_decode_and_end() {
    let mut image = pack::testing::synthetic_image();
    image.extend(video_sector(1, &bitstream(0x20)));
    let mut audio = vec![0u8; RAW];
    audio[18] = 0x64; // XA audio, form 2
    image.extend(audio);
    image.extend(video_sector(2, &bitstream(0x08)));
    assert_eq!(image.len(), (FIRST_LBA + 3) * RAW);
    insert_disc(pack::Disc::from_memory(image).unwrap());
    assert_eq!(CdInit(), 1);
    let seen = Arc::new(Mutex::new(Vec::new()));
    set_host(Box::new(Recorder(seen.clone())));
    ResetCallback();

    // MOV_startStream, MOV_startRead.
    DecDCTReset(0);
    DecDCToutCallback(Some(on_slice));
    unsafe { StSetRing(std::ptr::null_mut(), 24) };
    StSetStream(1, 1, u32::MAX, None, None);
    let mut loc = int_to_pos(FIRST_LBA as i32);
    let mut mode = 0x80u8;
    unsafe {
        assert_eq!(
            CdControl(CDL_SETLOC, (&raw mut loc).cast(), std::ptr::null_mut()),
            1
        );
        assert_eq!(
            CdControl(CDL_SETMODE, &raw mut mode, std::ptr::null_mut()),
            1
        );
    }
    VSync(3);
    let read_at = vblank_count();
    assert_eq!(read_at, 3);
    assert_eq!(CdRead2(0x1e0), 1);
    assert_eq!(vblank_count(), read_at, "starting the read takes no time");

    // The first poll finds nothing arrived and runs the clock to the frame's sector: the
    // first sector arrives START_LATENCY + 1 VBlanks after the read began.
    let frame = |n: u32, expect_at: u32, grey: u8| {
        let (mut bs, mut header): (*const u8, *const u8) = (std::ptr::null(), std::ptr::null());
        assert_eq!(unsafe { StGetNext(&mut bs, &mut header) }, 0, "frame {n}");
        assert_eq!(vblank_count(), expect_at, "frame {n} arrived");
        let h = unsafe { std::slice::from_raw_parts(header, 32) };
        assert_eq!(u32::from_le_bytes(h[8..12].try_into().unwrap()), n);
        let mut codes = vec![0u32; 0x28000 / 4];
        assert_eq!(
            unsafe { DecDCTvlc2(bs, codes.as_mut_ptr(), std::ptr::null()) },
            0
        );
        unsafe { StFreeRing(bs) };
        let mut px = vec![0u32; 192];
        unsafe { DecDCTin(codes.as_ptr(), 3) };
        unsafe { DecDCTout(px.as_mut_ptr(), 192) };
        let bytes: Vec<u8> = px.iter().flat_map(|w| w.to_le_bytes()).collect();
        assert!(
            bytes.iter().all(|&b| b == grey),
            "frame {n}: {:?}",
            &bytes[..6]
        );
    };
    frame(1, read_at + START_LATENCY + 1, 128 + 8);
    // Frame 2 is the third sector (an audio sector is between): 3 * 225,792 cycles is 1.2
    // VBlanks, so it arrives in the second.
    frame(2, read_at + START_LATENCY + 2, 128 + 2);
    // The callback ran once per DecDCTout, at the clock's time.
    let now = read_at + START_LATENCY + 2;
    assert_eq!(*SLICES.lock().unwrap(), [read_at + START_LATENCY + 1, now]);
    // Every VBlank in between ran the host's callback.
    assert_eq!(*seen.lock().unwrap(), (1..=now).collect::<Vec<_>>());

    // Past the last sector nothing is coming: StGetNext says so, and keeps saying so, without
    // moving the clock; StGetBackloc says where the next frame would have started.
    for _ in 0..3 {
        let (mut bs, mut header): (*const u8, *const u8) = (std::ptr::null(), std::ptr::null());
        assert_eq!(unsafe { StGetNext(&mut bs, &mut header) }, 1);
        assert_eq!(vblank_count(), now);
    }
    let mut back = CdlLoc::default();
    assert_eq!(unsafe { StGetBackloc(&mut back) }, 3);
    assert_eq!(back, int_to_pos(FIRST_LBA as i32 + 3));

    DecDCToutCallback(None);
    StUnSetRing();
    assert_eq!(
        unsafe { CdControlB(CDL_PAUSE, std::ptr::null(), std::ptr::null_mut()) },
        1
    );
}
