//! libcd against Ember's disc (when it exists; see build.rs): the way the game's file layer
//! reads (`CdSearchFile`, or `CdIntToPos` from its own table, then Setloc, `CdRead`,
//! `CdReadSync`) must give the bytes dumpsxiso extracted from the disc.

#[cfg_attr(not(have_disc), ignore = "disc not found")]
#[test]
fn the_game_read_sequence_returns_the_extracted_files() {
    #[cfg(have_disc)]
    {
        use psyq::libcd::*;
        use std::ffi::CString;

        insert_disc(pack::Disc::open(env!("DW1_TEST_DISC")).unwrap());
        assert_eq!(CdInit(), 1);
        let root = std::path::Path::new(env!("DW1_TEST_FILES"));
        // Files of several sizes and directories. The read length is rounded up to whole
        // sectors, as the game's is.
        let names = [
            "SLUS_010.32",
            "MMM.DAT",
            "MOV_REL.BIN",
            "CHDAT/ALLTIM.TIM",
            "BTL_REL.BIN",
        ];
        let mut checked = 0;
        for rel in names {
            let Ok(expected) = std::fs::read(root.join(rel)) else {
                continue;
            };
            let name = CString::new(format!("\\{};1", rel.replace('/', "\\"))).unwrap();
            let mut file = CdlFile {
                pos: CdlLoc::default(),
                size: 0,
                name: [0; 16],
            };
            assert!(!unsafe { CdSearchFile(&mut file, name.as_ptr()) }.is_null());
            assert_eq!(file.size as usize, expected.len(), "{rel}");
            let sectors = (file.size as usize).div_ceil(2048);
            let mut buf = vec![0u8; sectors * 2048];
            // Both ways of getting a position: the search result, and CdIntToPos of its sector.
            let lba = unsafe { CdPosToInt(&file.pos) };
            let mut pos = CdlLoc::default();
            unsafe { CdIntToPos(lba, &mut pos) };
            assert_eq!(pos, file.pos);
            let issued =
                unsafe { CdControl(CDL_SETLOC, (&raw const pos).cast(), std::ptr::null_mut()) };
            assert_eq!(issued, 1);
            assert_eq!(unsafe { CdRead(sectors as i32, buf.as_mut_ptr(), 0x80) }, 1);
            assert_eq!(unsafe { CdReadSync(0, std::ptr::null_mut()) }, 0);
            assert_eq!(&buf[..expected.len()], &expected[..], "{rel}");
            checked += 1;
        }
        assert!(
            checked >= 3,
            "only {checked} files found in the extracted dump"
        );
    }
}
