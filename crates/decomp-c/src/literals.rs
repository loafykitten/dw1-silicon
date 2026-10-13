//! Re-encodes the non-ASCII characters in C string and character literals as CP932 bytes.
//!
//! The PS1 build compiles with `-fexec-charset=CP932`; clang has no such option and would
//! store UTF-8. `build.rs` uses this module (through `#[path]`) and tests run from `lib.rs`.
//!
//! The lexer works on the source after C's translation phase 2 (backslash-newline splices
//! removed), so a splice can't hide a comment delimiter, a quote or a `\u` escape, but it
//! writes every splice back where it was, keeping line numbers.

use std::fmt::Write as _;

/// One character of the spliced source, with the raw text of any splices just before it.
struct Unit<'a> {
    ch: char,
    splices: &'a str,
}

/// Splits `text` into spliced characters, plus the splices left after the last one.
fn units(text: &str) -> (Vec<Unit<'_>>, &str) {
    let mut units = Vec::with_capacity(text.len());
    let mut rest = text;
    loop {
        let before = rest;
        // Each splice is a backslash, then LF or CRLF.
        while let Some(after) = rest
            .strip_prefix("\\\n")
            .or_else(|| rest.strip_prefix("\\\r\n"))
        {
            rest = after;
        }
        let splices = &before[..before.len() - rest.len()];
        let Some(ch) = rest.chars().next() else {
            return (units, splices);
        };
        rest = &rest[ch.len_utf8()..];
        units.push(Unit { ch, splices });
    }
}

/// Every non-ASCII character inside a string or character literal of `text`, written directly
/// or as a `\u`/`\U` escape: what [`encode_literals`] will ask `encode` for.
pub fn literal_chars(text: &str) -> Vec<char> {
    let mut chars = Vec::new();
    rewrite(text, &mut |c| {
        chars.push(c);
        Some(Vec::new())
    });
    chars
}

/// `text` with every non-ASCII character inside a string or character literal replaced by
/// the octal escapes of the bytes `encode` gives for it, or `None` when there's no such
/// character. Comments and everything else are left alone (clang reads UTF-8 source; only
/// the stored bytes need converting). Panics, naming `name`, when `encode` returns `None`.
pub fn encode_literals(
    text: &str,
    name: &str,
    encode: &mut dyn FnMut(char) -> Option<Vec<u8>>,
) -> Option<String> {
    rewrite(text, &mut |c| {
        Some(encode(c).unwrap_or_else(|| panic!("{name}: no CP932 encoding for {c:?}")))
    })
}

fn rewrite(text: &str, encode: &mut dyn FnMut(char) -> Option<Vec<u8>>) -> Option<String> {
    #[derive(PartialEq)]
    enum State {
        Code,
        LineComment,
        BlockComment,
        Literal(char),
    }
    let (units, tail) = units(text);
    let at = |i: usize| units.get(i).map(|u| u.ch);
    let mut out = String::with_capacity(text.len());
    let emit = |out: &mut String, u: &Unit| {
        out.push_str(u.splices);
        out.push(u.ch);
    };
    let mut state = State::Code;
    let mut changed = false;
    let mut i = 0;
    while i < units.len() {
        let c = units[i].ch;
        match state {
            State::Code => {
                if c == '/' && at(i + 1) == Some('/') {
                    state = State::LineComment;
                } else if c == '/' && at(i + 1) == Some('*') {
                    emit(&mut out, &units[i]);
                    i += 1;
                    state = State::BlockComment;
                } else if c == '"' || c == '\'' {
                    state = State::Literal(c);
                }
            }
            State::LineComment => {
                if c == '\n' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                if c == '*' && at(i + 1) == Some('/') {
                    emit(&mut out, &units[i]);
                    i += 1;
                    state = State::Code;
                }
            }
            State::Literal(quote) => {
                if c == '\\' {
                    let digits = match at(i + 1) {
                        Some('u') => 4,
                        Some('U') => 8,
                        _ => 0,
                    };
                    let hex: String = (i + 2..i + 2 + digits).filter_map(at).collect();
                    let decoded = (digits > 0 && hex.len() == digits)
                        .then(|| u32::from_str_radix(&hex, 16).ok())
                        .flatten()
                        .and_then(char::from_u32)
                        .filter(|ch| !ch.is_ascii());
                    if let Some(ch) = decoded {
                        // The escape's characters go; any splices inside it stay, after the
                        // bytes, so the line count doesn't change.
                        let span = &units[i..i + 2 + digits];
                        out.push_str(span[0].splices);
                        push_octal(&mut out, &encode(ch)?);
                        for u in &span[1..] {
                            out.push_str(u.splices);
                        }
                        changed = true;
                        i += 2 + digits;
                        continue;
                    }
                    // Any other escape: the backslash and the character after it, as written.
                    emit(&mut out, &units[i]);
                    i += 1;
                    if i < units.len() {
                        emit(&mut out, &units[i]);
                        i += 1;
                    }
                    continue;
                } else if c == quote || c == '\n' {
                    state = State::Code;
                } else if !c.is_ascii() {
                    out.push_str(units[i].splices);
                    push_octal(&mut out, &encode(c)?);
                    changed = true;
                    i += 1;
                    continue;
                }
            }
        }
        emit(&mut out, &units[i]);
        i += 1;
    }
    out.push_str(tail);
    changed.then_some(out)
}

/// Appends `bytes` as octal escapes, always three digits so a following digit can't join the
/// escape.
fn push_octal(out: &mut String, bytes: &[u8]) {
    for b in bytes {
        write!(out, "\\{b:03o}").unwrap();
    }
}
