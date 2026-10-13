//! libsnd against the disc's sound banks (when the extracted files exist; see build.rs): the
//! way `readVHBFile` (decomp/src/main/sound.c) splits a `.VHB` and opens it must succeed for
//! every bank, with the body landing in sound RAM.

#[cfg_attr(not(have_disc), ignore = "disc not found")]
#[test]
fn every_bank_on_the_disc_opens() {
    #[cfg(have_disc)]
    {
        use psyq::libsnd::SndState;
        use psyq::libspu::SpuState;

        let dir = std::path::Path::new(env!("DW1_TEST_FILES")).join("SOUND/VHB");
        let mut opened = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let file = std::fs::read(&path).unwrap();
            // VhbFileHeader: the VH and VB offsets.
            let word = |i: usize| u32::from_le_bytes(file[i * 4..i * 4 + 4].try_into().unwrap());
            let (vh, vb) = (word(0) as usize & !3, word(1) as usize);
            let mut snd = SndState::new();
            let mut spu = SpuState::new();
            snd.init(&mut spu);
            let id = snd.open_head_sticky(&file[vh..vb], 0, 0x1010);
            assert_eq!(id, 0, "{}", path.display());
            assert_eq!(
                snd.trans_body(&mut spu, &file[vb..], id),
                id,
                "{}",
                path.display()
            );
            opened += 1;
        }
        assert!(opened >= 2, "expected the SS and SL banks at least");
    }
}
