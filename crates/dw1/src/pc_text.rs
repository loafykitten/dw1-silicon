//! The game's own lines that talk about memory cards, reworded for the save slots (Phase 7).
//!
//! Jijimon's Battle Registration machine is "a Save Machine for Memory Card Battle Data" in the
//! map scripts (`\SCN\DG.SCN`, which holds every scene's script and its text). With the save slots
//! there is no memory card, so the window build has the drive deliver those lines as "Battle
//! Data" instead (`psyq::libcd::patch_file`). A script's text sits between its instructions,
//! so a line keeps its length: the shorter words are followed by spaces, which show as nothing
//! at the end of a line. Ember asked for it (2026-10-14).

/// The script text's characters, as the US game writes them: full-width Shift-JIS letters,
/// digits and the punctuation used here (font.c's `CHAR_TO_GLYPH_TABLE`).
fn sjis(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for c in text.chars() {
        let code: u16 = match c {
            'A'..='Z' => 0x8260 + (c as u16 - 'A' as u16),
            'a'..='z' => 0x8281 + (c as u16 - 'a' as u16),
            '0'..='9' => 0x824f + (c as u16 - '0' as u16),
            ' ' => 0x8140,
            '.' => 0x8142,
            '?' => 0x8148,
            _ => panic!("no script character for {c:?}"),
        };
        out.extend_from_slice(&code.to_be_bytes());
    }
    out
}

/// A line break in the script text.
const NEWLINE: [u8; 2] = [0x0d, 0x00];
/// Back to the normal colour after a highlighted phrase (the machine's name is pink).
const PLAIN: [u8; 2] = [0x01, 0x01];

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// (what the disc says, what the slots build says): the four places the machine is named, in
/// Jijimon's house before and after the story moves on.
fn rewordings() -> Vec<(Vec<u8>, Vec<u8>)> {
    let battle_data = |end: &str| cat(&[&sjis("Battle Data"), &PLAIN, &sjis(end)]);
    let mut out = vec![
        // "That's a Save Machine / for Memory Card Battle / Data."
        (
            cat(&[
                &sjis("for Memory Card Battle"),
                &NEWLINE,
                &sjis("Data"),
                &PLAIN,
                &sjis("."),
            ]),
            cat(&[&sjis("for "), &battle_data(".")]),
        ),
        // "Do you want to use the / Save Machine for / Memory Card Battle?"
        (
            cat(&[&sjis("Memory Card Battle"), &PLAIN, &sjis("?")]),
            battle_data("?"),
        ),
    ];
    // "That is a / Save Machine for / Memory Card Battle Data." and the question after it.
    for end in [".", "?"] {
        out.push((
            cat(&[&sjis("Memory Card Battle Data"), &PLAIN, &sjis(end)]),
            battle_data(end),
        ));
    }
    for (old, new) in &mut out {
        assert!(new.len() <= old.len() && (old.len() - new.len()) % 2 == 0);
        while new.len() < old.len() {
            new.extend_from_slice(&sjis(" "));
        }
    }
    out
}

/// Every place in `data` one of the rewordings applies, as (offset, new bytes). Returns the
/// edits and how many rewordings were found nowhere.
fn edits(data: &[u8]) -> (Vec<(usize, Vec<u8>)>, usize) {
    let mut edits = Vec::new();
    let mut missing = 0;
    for (old, new) in rewordings() {
        let found: Vec<usize> = data
            .windows(old.len())
            .enumerate()
            .filter(|(_, w)| *w == old.as_slice())
            .map(|(i, _)| i)
            .collect();
        if found.is_empty() {
            missing += 1;
        }
        edits.extend(found.into_iter().map(|at| (at, new.clone())));
    }
    (edits, missing)
}

/// Rewords the lines on the inserted disc. A disc without them (another release) keeps its text,
/// with a warning.
pub fn apply() {
    let mut missing = 0;
    let result = psyq::libcd::patch_file("\\SCN\\DG.SCN", |data| {
        let (edits, m) = edits(data);
        missing = m;
        edits
    });
    match result {
        Ok(_) if missing == 0 => {}
        Ok(_) => eprintln!(
            "dw1: {missing} of the save machine's memory card lines weren't found on this disc; \
             they keep their wording"
        ),
        Err(e) => eprintln!("dw1: the save machine's lines keep their wording: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewordings_keep_lengths_and_end_in_spaces() {
        for (old, new) in rewordings() {
            assert_eq!(old.len(), new.len());
            assert!(new.ends_with(&sjis(" ")));
            assert!(
                !new.windows(2).any(|w| w == NEWLINE),
                "one line fewer, never more"
            );
        }
    }

    #[test]
    fn edits_find_each_line() {
        // A script stretch shaped like the disc's: instructions, then the text.
        let line = cat(&[
            &[0x1a, 0x00],
            &sjis("Do you want to use the"),
            &NEWLINE,
            &[0x01, 0x04],
            &sjis("Save Machine for"),
            &NEWLINE,
            &sjis("Memory Card Battle"),
            &PLAIN,
            &sjis("?"),
            &NEWLINE,
            &[0x00, 0x00],
        ]);
        let (found, missing) = edits(&line);
        assert_eq!(missing, 3);
        assert_eq!(found.len(), 1);
        let (at, new) = &found[0];
        let mut patched = line.clone();
        patched[*at..*at + new.len()].copy_from_slice(new);
        let tail = cat(&[
            &sjis("Battle Data"),
            &PLAIN,
            &sjis("?       "),
            &NEWLINE,
            &[0, 0],
        ]);
        assert!(patched.ends_with(&tail));
        assert_eq!(patched.len(), line.len());
    }
}
