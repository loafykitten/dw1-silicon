//! The movie libraries against Ember's disc (when it exists; see build.rs): `\MOVIE\OP1.STR;1`
//! streamed through `CdRead2` and the `St*` calls the way MOV does it, every frame
//! variable-length decoded, some decoded to pixels and written as PNG under `captures/`.

#[cfg_attr(not(have_disc), ignore = "disc not found")]
#[test]
fn the_intro_streams_to_its_last_frame() {
    #[cfg(have_disc)]
    {
        use psyq::libcd::*;
        use psyq::libetc::{ResetCallback, VSync, vblank_count};
        use psyq::libpress::*;

        // MOV_MOVIES[0] (the US build): the frame count its completion test uses.
        const FRAMES: u32 = 3637;
        const RUN_LEVEL_WORDS: usize = 0x28000 / 4;
        const SLICE_WORDS: usize = 24 * 240 / 2;

        insert_disc(pack::Disc::open(env!("DW1_TEST_DISC")).unwrap());
        assert_eq!(CdInit(), 1);
        ResetCallback();
        let mut file = CdlFile {
            pos: CdlLoc::default(),
            size: 0,
            name: [0; 16],
        };
        assert!(!unsafe { CdSearchFile(&mut file, c"\\MOVIE\\OP1.STR;1".as_ptr()) }.is_null());

        // MOV_startStream, MOV_startRead.
        DecDCTReset(0);
        unsafe { StSetRing(std::ptr::null_mut(), 24) };
        StSetStream(1, 1, u32::MAX, None, None);
        let mut mode = 0x80u8;
        let mut pos = file.pos;
        assert_eq!(
            unsafe { CdControl(CDL_SETLOC, (&raw mut pos).cast(), std::ptr::null_mut()) },
            1
        );
        assert_eq!(
            unsafe { CdControl(CDL_SETMODE, &raw mut mode, std::ptr::null_mut()) },
            1
        );
        VSync(3);
        let read_at = vblank_count();
        assert_eq!(CdRead2(0x1e0), 1);

        let mut codes = vec![0u32; RUN_LEVEL_WORDS];
        let mut slice = vec![0u32; SLICE_WORDS];
        let mut frames = 0;
        let mut lit_frames = 0;
        let mut last = 0;
        let mut first_arrival = 0;
        let mut width_height = (0, 0);
        let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../captures/movie");
        while frames < FRAMES {
            let (mut bs, mut header): (*const u8, *const u8) = (std::ptr::null(), std::ptr::null());
            assert_eq!(
                unsafe { StGetNext(&mut bs, &mut header) },
                0,
                "after {frames} frames"
            );
            let h = unsafe { std::slice::from_raw_parts(header, 32) };
            let number = u32::from_le_bytes(h[8..12].try_into().unwrap());
            let (w, ht) = (
                u16::from_le_bytes([h[16], h[17]]),
                u16::from_le_bytes([h[18], h[19]]),
            );
            if frames == 0 {
                first_arrival = vblank_count();
                width_height = (w, ht);
            }
            assert_eq!((w, ht), width_height);
            assert_eq!(number, last + 1, "frames in order");
            last = number;
            assert_eq!(
                unsafe { DecDCTvlc2(bs, codes.as_mut_ptr(), std::ptr::null()) },
                0,
                "frame {number}"
            );
            // Pixels for a few frames.
            if number == 1 || number % 600 == 0 || number == FRAMES {
                let (w, ht) = (usize::from(w), usize::from(ht));
                let mut rgb = vec![0u8; w * ht * 3];
                unsafe { DecDCTin(codes.as_ptr(), 3) };
                for x in (0..w).step_by(16) {
                    unsafe { DecDCTout(slice.as_mut_ptr(), (24 * ht / 2) as i32) };
                    let bytes: Vec<u8> = slice.iter().flat_map(|w| w.to_le_bytes()).collect();
                    for y in 0..ht {
                        let row = &bytes[y * 48..y * 48 + 48];
                        let cols = 16.min(w - x);
                        rgb[(y * w + x) * 3..(y * w + x + cols) * 3]
                            .copy_from_slice(&row[..cols * 3]);
                    }
                }
                // A picture, not a flat fill: lit, many colours, and varying across the frame.
                let lit = rgb.iter().filter(|&&b| b > 16).count() > rgb.len() / 20;
                let colours: std::collections::HashSet<&[u8]> = rgb.chunks(3).collect();
                let left_right_differ = (0..ht).any(|y| {
                    rgb[y * w * 3..y * w * 3 + 3] != rgb[(y * w + w - 1) * 3..(y * w + w) * 3]
                });
                let top_bottom_differ = rgb[..w * 3] != rgb[(ht - 1) * w * 3..ht * w * 3];
                lit_frames += usize::from(
                    lit && colours.len() >= 256 && (left_right_differ || top_bottom_differ),
                );
                std::fs::create_dir_all(&out_dir).unwrap();
                let file =
                    std::fs::File::create(out_dir.join(format!("op1_{number:04}.png"))).unwrap();
                let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, ht as u32);
                enc.set_color(png::ColorType::Rgb);
                enc.set_depth(png::BitDepth::Eight);
                enc.write_header().unwrap().write_image_data(&rgb).unwrap();
            }
            unsafe { StFreeRing(bs) };
            frames += 1;
        }
        eprintln!(
            "OP1: {frames} frames, {}x{}, first frame {} VBlanks after CdRead2, last at VBlank {}",
            width_height.0,
            width_height.1,
            first_arrival - read_at,
            vblank_count()
        );
        assert!(
            lit_frames >= 3,
            "only {lit_frames} sampled frames have any picture"
        );
        StUnSetRing();
        assert_eq!(
            unsafe { CdControlB(CDL_PAUSE, std::ptr::null(), std::ptr::null_mut()) },
            1
        );
    }
}
