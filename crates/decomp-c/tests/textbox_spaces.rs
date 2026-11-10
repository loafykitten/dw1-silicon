//! The real US textbox routine must not derive leading spaces from an uninitialised value.
//! Retain its previous-glyph padding once assigned (including numeric field alignment).
//! Synthetic strings only; no disc, saves or captures.

use decomp_c as _;
use psyq::gpu;

unsafe extern "C" {
    fn initializeFontCLUT();
    fn clearTextArea();
    fn clearTextSubArea(rect: *mut psyq::libgpu::Rect);
    fn drawString2(text: *const u8, x: i16, y: i16, flag: i32) -> i32;
    fn drawGlyph(codepoint: i32, x: i32, y: i32) -> i32;
    fn convertAsciiToJis(ch: i32) -> i32;
    fn swapShortBytes(code: i32) -> i32;
    static mut DRAW_STRING2_IS_FIXED_WIDTH: i8;
}

fn text_area() -> Vec<u16> {
    gpu::with(|g| {
        (256..512)
            .flat_map(|y| g.vram()[y * 1024 + 704..y * 1024 + 768].iter().copied())
            .collect()
    })
}

#[test]
fn leading_spaces_are_neutral_or_fixed_cells_without_changing_later_padding() {
    unsafe { initializeFontCLUT() };
    let cases: &[&[u8]] = &[
        b"  A\0",                 // first character(s) are spaces: no preceding advance exists
        b"  Wi iW\0",             // retain padding after both wide and narrow glyphs
        b"  A   B\0",             // repeated internal spaces
        b"   \0",                 // an entirely blank line
        b"AB\0",                  // no spaces: unchanged
        b"\x81\x40A\0",           // explicit Shift-JIS space
        b" \x81\x40A\x81\x40B\0", // mixed ASCII/JIS with an internal space
    ];
    for fixed in [false, true] {
        for &(x, y) in &[(0i16, 0i16), (16, 48)] {
            for &text in cases {
                unsafe {
                    clearTextArea();
                    DRAW_STRING2_IS_FIXED_WIDTH = i8::from(fixed);
                    if !fixed {
                        // Dropping only initial padding must be equivalent: after the first
                        // glyph, use the unchanged real routine as the reference. This keeps
                        // its established numeric padding rather than substituting a new font
                        // space width, which moves digits into a different source rectangle.
                        let mut at = 0;
                        loop {
                            match &text[at..] {
                                [b' ', ..] => at += 1,
                                [0x81, 0x40, ..] => at += 2,
                                _ => break,
                            }
                        }
                        assert_eq!(drawString2(text[at..].as_ptr(), x, y, 1), 1);
                    } else {
                        let mut at = 0;
                        let mut col = 0;
                        while text[at] != 0 {
                            let glyph = if text[at] < 0x80 {
                                let code = convertAsciiToJis(text[at].into());
                                at += 1;
                                swapShortBytes(code)
                            } else {
                                let code = i32::from(text[at]) | i32::from(text[at + 1]) << 8;
                                at += 2;
                                code
                            };
                            drawGlyph(glyph, i32::from(x) + col, y.into());
                            col += 12;
                        }
                    }
                    let expected = text_area();
                    clearTextArea();
                    DRAW_STRING2_IS_FIXED_WIDTH = i8::from(fixed);
                    assert_eq!(drawString2(text.as_ptr(), x, y, 1), 1);
                    let actual = text_area();
                    let differences = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
                    assert_eq!(differences, 0, "{text:?}, fixed={fixed}, origin=({x},{y})");
                }
            }

            // Numeric fields depend on a space reusing the preceding glyph's advance,
            // not the blank glyph's own width. Check explicit positions after both a
            // wide and a narrow prefix, with ASCII and Shift-JIS padding. This reference
            // draws individual glyphs/cells and does not call drawString2.
            for prefix in [b'A', b'i'] {
                for gap in [1, 3] {
                    for sjis in [false, true] {
                        let mut text = vec![prefix];
                        for _ in 0..gap {
                            text.extend_from_slice(if sjis { &[0x81, 0x40] } else { b" " });
                        }
                        text.extend_from_slice(b"1\0");
                        unsafe {
                            clearTextArea();
                            DRAW_STRING2_IS_FIXED_WIDTH = i8::from(fixed);
                            let prefix_code = swapShortBytes(convertAsciiToJis(prefix.into()));
                            let natural = drawGlyph(prefix_code, x.into(), y.into());
                            let advance = if fixed { 12 } else { natural };
                            let mut col = advance;
                            for _ in 0..gap {
                                let mut rect = psyq::libgpu::Rect {
                                    x: x + col as i16,
                                    y,
                                    w: 12,
                                    h: 12,
                                };
                                clearTextSubArea(&mut rect);
                                col += advance;
                            }
                            drawGlyph(
                                swapShortBytes(convertAsciiToJis(b'1'.into())),
                                i32::from(x) + col,
                                y.into(),
                            );
                            let expected = text_area();
                            clearTextArea();
                            assert_eq!(drawString2(text.as_ptr(), x, y, 1), 1);
                            let actual = text_area();
                            let differences =
                                actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
                            assert_eq!(
                                differences, 0,
                                "numeric {text:?}, fixed={fixed}, origin=({x},{y})"
                            );
                        }
                    }
                }
            }
        }
    }
    unsafe { DRAW_STRING2_IS_FIXED_WIDTH = 0 };
}
