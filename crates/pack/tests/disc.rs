//! Reads the real disc (when it exists; see build.rs) and compares its files with the ones
//! dumpsxiso extracted from it.

#[cfg_attr(not(have_disc), ignore = "disc not found")]
#[test]
fn files_match_the_extracted_dump() {
    #[cfg(have_disc)]
    {
        use std::path::Path;
        let disc = pack::Disc::open(env!("DW1_TEST_DISC")).unwrap();
        let root = Path::new(env!("DW1_TEST_FILES"));
        let (mut compared, mut skipped) = (0, 0);
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let rel = path.strip_prefix(root).unwrap().to_str().unwrap();
                let disc_path = format!("\\{};1", rel.replace('/', "\\"));
                // dumpsxiso extras: the licence area (sectors 0-15) and its layout XML.
                if rel == "license_data.dat" || rel.ends_with(".xml") {
                    continue;
                }
                let found = disc
                    .find(&disc_path)
                    .unwrap()
                    .unwrap_or_else(|| panic!("{disc_path} not on the disc"));
                let expected = std::fs::read(&path).unwrap();
                // Form 2 files (movies, XA) are extracted as raw sectors, so sizes differ.
                if expected.len() != found.size as usize || disc.is_form2(found.lba).unwrap() {
                    skipped += 1;
                    continue;
                }
                assert_eq!(
                    disc.read_file(&disc_path).unwrap().unwrap(),
                    expected,
                    "{disc_path}"
                );
                compared += 1;
            }
        }
        eprintln!("compared {compared} files, skipped {skipped}");
        assert!(compared > 100, "only {compared} files compared");
    }
}

#[cfg_attr(not(have_disc), ignore = "disc not found")]
#[test]
fn reads_the_volume() {
    #[cfg(have_disc)]
    {
        let disc = pack::Disc::open(env!("DW1_TEST_DISC")).unwrap();
        assert!(disc.sector_count() > 100_000);
        let boot = disc.find("\\SLUS_010.32;1").unwrap().unwrap();
        assert!(boot.size > 0);
        // The system.cnf pointing at it is in the root too.
        assert!(disc.find("\\system.cnf").unwrap().is_some());
    }
}

#[cfg_attr(not(have_disc), ignore = "disc not found")]
#[test]
fn the_disc_is_the_known_redump() {
    #[cfg(have_disc)]
    pack::Disc::open(env!("DW1_TEST_DISC"))
        .unwrap()
        .verify()
        .unwrap();
}
