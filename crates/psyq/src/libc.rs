//! The C library calls the game makes, under the `dw1_` names `dw1_native.h` gives them so the
//! game C can't bind to macOS's libc (whose `size_t` is 64-bit where Psy-Q's is 32, and whose
//! NULL handling differs).
//!
//! In DW1's executable, every one of these except `sprintf` is a three-instruction jump into
//! the BIOS (`decomp/asm/us/main/psyq/libc.s`), so they follow the BIOS functions as psx-spx
//! documents them ("Kernel (BIOS)", BIOS Memory/String Functions): most refuse a NULL pointer
//! or a length above 0x7FFFFFFF instead of crashing. `sprintf` is the SDK's own code, written
//! here from the C standard's description of the conversions DW1 uses.

use core::ffi::{VaList, c_char, c_int, c_uchar, c_void};
use std::io::Write;

/// BIOS A(0Eh) `abs`.
#[unsafe(no_mangle)]
pub extern "C" fn dw1_abs(value: c_int) -> c_int {
    value.wrapping_abs()
}

/// BIOS A(2Ah) `memcpy`: copies nothing when `dst` is NULL or `len` is above 0x7FFFFFFF
/// (negative as an int); always returns `dst`.
///
/// # Safety
/// As C `memcpy`, for the bytes it copies.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_memcpy(
    dst: *mut c_void,
    src: *const c_void,
    len: c_int,
) -> *mut c_void {
    if !dst.is_null() && len > 0 {
        // The BIOS copies forwards byte by byte; overlapping callers get that behaviour.
        let (d, s) = (dst.cast::<u8>(), src.cast::<u8>());
        for i in 0..len as usize {
            unsafe { d.add(i).write(s.add(i).read()) };
        }
    }
    dst
}

/// BIOS A(2Bh) `memset`: fills nothing when `dst` is NULL or `len` is above 0x7FFFFFFF;
/// returns `dst`, or NULL when `len` is 0 or above 0x7FFFFFFF.
///
/// # Safety
/// As C `memset`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_memset(dst: *mut c_uchar, fill: c_uchar, len: c_int) -> *mut c_void {
    if len <= 0 {
        return core::ptr::null_mut();
    }
    if !dst.is_null() {
        unsafe { dst.write_bytes(fill, len as usize) };
    }
    dst.cast()
}

/// BIOS A(28h) `bzero`: `memset` with a zero fill.
///
/// # Safety
/// As C `memset`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_bzero(dst: *mut c_uchar, len: c_int) -> *mut c_void {
    unsafe { dw1_memset(dst, 0, len) }
}

/// BIOS A(1Bh) `strlen`: 0 for NULL.
///
/// # Safety
/// `src` is NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_strlen(src: *const c_char) -> c_int {
    if src.is_null() {
        return 0;
    }
    let mut n = 0;
    while unsafe { *src.add(n) } != 0 {
        n += 1;
    }
    n as c_int
}

/// BIOS A(19h) `strcpy`: copies up to and including the NUL; refuses (returning NULL) when
/// either pointer is NULL.
///
/// # Safety
/// As C `strcpy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    if dst.is_null() || src.is_null() {
        return core::ptr::null_mut();
    }
    let mut i = 0;
    loop {
        let c = unsafe { *src.add(i) };
        unsafe { *dst.add(i) = c };
        if c == 0 {
            return dst;
        }
        i += 1;
    }
}

/// BIOS A(1Ah) `strncpy`: copies at most `maxlen` bytes, with no terminator when `src` is
/// that long or longer, and pads a shorter copy with NULs up to `maxlen`. psx-spx doesn't say
/// what it does with NULL; this refuses like `strcpy` (unverified).
///
/// # Safety
/// As C `strncpy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_strncpy(
    dst: *mut c_char,
    src: *const c_char,
    maxlen: c_int,
) -> *mut c_char {
    if dst.is_null() || src.is_null() {
        return core::ptr::null_mut();
    }
    let maxlen = maxlen.max(0) as usize;
    let mut i = 0;
    while i < maxlen {
        let c = unsafe { *src.add(i) };
        if c == 0 {
            break;
        }
        unsafe { *dst.add(i) = c };
        i += 1;
    }
    while i < maxlen {
        unsafe { *dst.add(i) = 0 };
        i += 1;
    }
    dst
}

/// BIOS A(15h) `strcat`: appends `src` to `dst`; refuses (returning NULL) when either is NULL.
///
/// # Safety
/// As C `strcat`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    if dst.is_null() || src.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        dw1_strcpy(dst.add(dw1_strlen(dst) as usize), src);
    }
    dst
}

/// BIOS A(17h) `strcmp`: the difference of the first mismatching bytes, each sign-extended;
/// with NULLs, 0 when both are NULL, -1 when only `a` is, +1 when only `b` is.
///
/// # Safety
/// Each pointer is NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_strcmp(a: *const c_char, b: *const c_char) -> c_int {
    match (a.is_null(), b.is_null()) {
        (true, true) => return 0,
        (true, false) => return -1,
        (false, true) => return 1,
        _ => {}
    }
    let mut i = 0;
    loop {
        // Sign extension as on the PS1, whatever the host's `c_char` is.
        let (x, y) = unsafe { (*a.cast::<i8>().add(i), *b.cast::<i8>().add(i)) };
        if x != y {
            return c_int::from(x) - c_int::from(y);
        }
        if x == 0 {
            return 0;
        }
        i += 1;
    }
}

/// BIOS A(1Fh) `strrchr`: the address of the last `c` in `src` (the terminator's when `c` is
/// 0), or NULL when there's none or `src` is NULL.
///
/// # Safety
/// `src` is NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_strrchr(src: *const c_char, c: c_char) -> *mut c_char {
    if src.is_null() {
        return core::ptr::null_mut();
    }
    let mut found = core::ptr::null();
    let mut p = src;
    loop {
        let x = unsafe { *p };
        if x == c {
            found = p;
        }
        if x == 0 {
            return found.cast_mut();
        }
        p = unsafe { p.add(1) };
    }
}

/// The SDK's `sprintf`: formats into `buf` with a terminating NUL and returns the length.
///
/// # Safety
/// As C `sprintf`: `buf` must hold the output, and the arguments must match the format.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_sprintf(buf: *mut c_char, fmt: *const c_char, mut args: ...) -> c_int {
    let out = unsafe { format(fmt, &mut args) };
    unsafe {
        core::ptr::copy_nonoverlapping(out.as_ptr(), buf.cast::<u8>(), out.len());
        *buf.add(out.len()) = 0;
    }
    out.len() as c_int
}

/// BIOS A(3Fh) `printf`: the PS1's debug TTY, which is the host's stderr here.
///
/// # Safety
/// As C `printf`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dw1_printf(fmt: *const c_char, mut args: ...) -> c_int {
    let out = unsafe { format(fmt, &mut args) };
    // The TTY can't fail on the PS1; a closed stderr just loses the text.
    let _ = std::io::stderr().write_all(&out);
    out.len() as c_int
}

/// One argument as a conversion consumes it.
enum Arg {
    Int(i32),
    Str(*const c_char),
}

/// Formats `fmt` with C-variadic arguments.
///
/// # Safety
/// `fmt` is a NUL-terminated string and `args` holds what it asks for: a 32-bit int for each
/// integer or character conversion (`long` is 32 bits on the PS1), a pointer for `%s`.
unsafe fn format(fmt: *const c_char, args: &mut VaList<'_>) -> Vec<u8> {
    let fmt = unsafe { core::ffi::CStr::from_ptr(fmt) }.to_bytes();
    format_with(fmt, |is_str| {
        if is_str {
            Arg::Str(unsafe { args.next_arg::<*const c_char>() })
        } else {
            Arg::Int(unsafe { args.next_arg::<c_int>() })
        }
    })
}

/// The formatter behind `sprintf`/`printf`, over any argument source (tests use a list).
///
/// Supports what the C standard gives `printf` for 32-bit integers, characters and strings:
/// flags `-+ #0`, width and precision (digits or `*`), the `h` and `l` length modifiers, and
/// the conversions `d i u o x X c s %`. Anything else is copied through as written.
fn format_with(fmt: &[u8], mut next: impl FnMut(bool) -> Arg) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    let int_arg = |next: &mut dyn FnMut(bool) -> Arg| match next(false) {
        Arg::Int(v) => v,
        Arg::Str(_) => unreachable!(),
    };
    while i < fmt.len() {
        let c = fmt[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        let start = i - 1;
        let (mut left, mut plus, mut space, mut alt, mut zero) =
            (false, false, false, false, false);
        while i < fmt.len() {
            match fmt[i] {
                b'-' => left = true,
                b'+' => plus = true,
                b' ' => space = true,
                b'#' => alt = true,
                b'0' => zero = true,
                _ => break,
            }
            i += 1;
        }
        let mut width = 0usize;
        if fmt.get(i) == Some(&b'*') {
            i += 1;
            let w = int_arg(&mut next);
            if w < 0 {
                left = true;
            }
            width = w.unsigned_abs() as usize;
        } else {
            while let Some(d @ b'0'..=b'9') = fmt.get(i) {
                width = width * 10 + usize::from(d - b'0');
                i += 1;
            }
        }
        let mut precision = None;
        if fmt.get(i) == Some(&b'.') {
            i += 1;
            if fmt.get(i) == Some(&b'*') {
                i += 1;
                let p = int_arg(&mut next);
                precision = (p >= 0).then_some(p as usize);
            } else {
                let mut p = 0usize;
                while let Some(d @ b'0'..=b'9') = fmt.get(i) {
                    p = p * 10 + usize::from(d - b'0');
                    i += 1;
                }
                precision = Some(p);
            }
        }
        let mut half = false;
        while let Some(m @ (b'h' | b'l')) = fmt.get(i) {
            half = *m == b'h';
            i += 1;
        }
        let Some(&conv) = fmt.get(i) else {
            out.extend_from_slice(&fmt[start..]);
            break;
        };
        i += 1;

        let (prefix, body): (&[u8], Vec<u8>) = match conv {
            b'%' => {
                out.push(b'%');
                continue;
            }
            b'c' => (b"", vec![int_arg(&mut next) as u8]),
            b's' => {
                let Arg::Str(p) = next(true) else {
                    unreachable!()
                };
                let limit = precision.unwrap_or(usize::MAX);
                let body = if p.is_null() {
                    b"(null)"[..limit.min(6)].to_vec()
                } else {
                    // With a precision the source needn't be terminated: read no further.
                    let mut s = Vec::new();
                    while s.len() < limit {
                        let byte = unsafe { *p.add(s.len()) } as u8;
                        if byte == 0 {
                            break;
                        }
                        s.push(byte);
                    }
                    s
                };
                (b"", body)
            }
            b'd' | b'i' | b'u' | b'o' | b'x' | b'X' => {
                let raw = int_arg(&mut next);
                let raw = if half { i32::from(raw as i16) } else { raw };
                let signed = matches!(conv, b'd' | b'i');
                let (negative, magnitude) = if signed {
                    (raw < 0, u64::from(raw.unsigned_abs()))
                } else {
                    let v = if half {
                        u32::from(raw as u16)
                    } else {
                        raw as u32
                    };
                    (false, u64::from(v))
                };
                let mut digits = match conv {
                    b'o' => format!("{magnitude:o}"),
                    b'x' => format!("{magnitude:x}"),
                    b'X' => format!("{magnitude:X}"),
                    _ => magnitude.to_string(),
                }
                .into_bytes();
                if precision == Some(0) && magnitude == 0 {
                    digits.clear();
                }
                if let Some(p) = precision {
                    while digits.len() < p {
                        digits.insert(0, b'0');
                    }
                }
                if alt && conv == b'o' && digits.first() != Some(&b'0') {
                    digits.insert(0, b'0');
                }
                let prefix: &[u8] = if negative {
                    b"-"
                } else if signed && plus {
                    b"+"
                } else if signed && space {
                    b" "
                } else if alt && magnitude != 0 && conv == b'x' {
                    b"0x"
                } else if alt && magnitude != 0 && conv == b'X' {
                    b"0X"
                } else {
                    b""
                };
                // `0` pads between the sign and the digits, unless `-` or a precision is given.
                if zero && !left && precision.is_none() {
                    while prefix.len() + digits.len() < width {
                        digits.insert(0, b'0');
                    }
                }
                (prefix, digits)
            }
            _ => {
                out.extend_from_slice(&fmt[start..i]);
                continue;
            }
        };
        let len = prefix.len() + body.len();
        let pad = width.saturating_sub(len);
        if !left {
            out.resize(out.len() + pad, b' ');
        }
        out.extend_from_slice(prefix);
        out.extend_from_slice(&body);
        if left {
            out.resize(out.len() + pad, b' ');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn fmt(f: &str, ints: &[i32]) -> String {
        let mut it = ints.iter();
        String::from_utf8(format_with(f.as_bytes(), |_| Arg::Int(*it.next().unwrap()))).unwrap()
    }

    #[test]
    fn the_games_formats() {
        // utils.c NUMBER_FORMATS, main_menu.c FMT_NUMBER, game_menu.c FMT_PLAYTIME, the
        // effect interpreters' "%d\n", fish.yaml's lengths.
        assert_eq!(fmt("%01d", &[7]), "7");
        assert_eq!(fmt("%02d", &[7]), "07");
        assert_eq!(fmt("%03d", &[-7]), "-07");
        assert_eq!(fmt("%06d", &[123]), "000123");
        assert_eq!(fmt("%04d", &[123456]), "123456");
        assert_eq!(fmt("%i", &[-42]), "-42");
        assert_eq!(fmt("%d : %d", &[12, 5]), "12 : 5");
        assert_eq!(fmt("%d\n", &[i32::MIN]), "-2147483648\n");
        assert_eq!(
            fmt("Great! Total length %d m %d cm", &[1, 20]),
            "Great! Total length 1 m 20 cm"
        );
    }

    #[test]
    fn flags_widths_and_conversions() {
        assert_eq!(
            fmt("[%5d|%-5d|%+d|% d]", &[42, 42, 42, 42]),
            "[   42|42   |+42| 42]"
        );
        assert_eq!(
            fmt("%x %X %#x %o %#o %u", &[255, 255, 255, 8, 8, -1]),
            "ff FF 0xff 10 010 4294967295"
        );
        assert_eq!(fmt("%.3d|%5.3d|%05.3d", &[7, 7, 7]), "007|  007|  007");
        assert_eq!(fmt("%*d|%-*d|", &[4, 1, 3, 2]), "   1|2  |");
        assert_eq!(fmt("%hd %hx %ld", &[0x1_FFFF, 0x1_FFFF, -3]), "-1 ffff -3");
        assert_eq!(fmt("%c%c 100%%", &[b'o' as i32, b'k' as i32]), "ok 100%");
        assert_eq!(fmt("%.0d|%q|%", &[0]), "|%q|%");
    }

    #[test]
    fn strings_through_sprintf() {
        let name = CString::new("Agumon").unwrap();
        let mut buf = [0x55 as c_char; 32];
        let n = unsafe {
            dw1_sprintf(
                buf.as_mut_ptr(),
                c"%s!%3s|%.3s|%d".as_ptr(),
                name.as_ptr(),
                c"a".as_ptr(),
                name.as_ptr(),
                9,
            )
        };
        let s = unsafe { core::ffi::CStr::from_ptr(buf.as_ptr()) };
        assert_eq!(s.to_str().unwrap(), "Agumon!  a|Agu|9");
        assert_eq!(n as usize, s.to_bytes().len());

        // `%.Ns` reads at most N bytes, so an unterminated source is fine.
        let unterminated = [b'a' as c_char, b'b' as c_char, b'c' as c_char];
        let n = unsafe {
            dw1_sprintf(
                buf.as_mut_ptr(),
                c"%.3s|%.2s".as_ptr(),
                unterminated.as_ptr(),
                core::ptr::null::<c_char>(),
            )
        };
        let s = unsafe { core::ffi::CStr::from_ptr(buf.as_ptr()) };
        assert_eq!(s.to_str().unwrap(), "abc|(n");
        assert_eq!(n, 6);
    }

    #[test]
    fn bios_string_functions_follow_psx_spx() {
        let null = core::ptr::null::<c_char>();
        let mut buf = [1 as c_char; 8];
        unsafe {
            assert_eq!(dw1_strlen(null), 0);
            assert_eq!(dw1_strlen(c"abc".as_ptr()), 3);
            assert_eq!(dw1_strcmp(null, null), 0);
            assert_eq!(dw1_strcmp(null, c"a".as_ptr()), -1);
            assert_eq!(dw1_strcmp(c"a".as_ptr(), null), 1);
            assert_eq!(dw1_strcmp(c"ab".as_ptr(), c"ab".as_ptr()), 0);
            assert_eq!(dw1_strcmp(c"ab".as_ptr(), c"ac".as_ptr()), -1);
            // Bytes are sign-extended: 0x80 compares below 'a'.
            assert_eq!(dw1_strcmp(c"\x80".as_ptr(), c"a".as_ptr()), -128 - 0x61);
            assert!(dw1_strcpy(buf.as_mut_ptr(), null).is_null());
            assert_eq!(
                dw1_strcpy(buf.as_mut_ptr(), c"hi".as_ptr()),
                buf.as_mut_ptr()
            );
            assert_eq!(
                dw1_strcat(buf.as_mut_ptr(), c"ya".as_ptr()),
                buf.as_mut_ptr()
            );
            assert_eq!(core::ffi::CStr::from_ptr(buf.as_ptr()), c"hiya");
            let last = dw1_strrchr(buf.as_ptr(), b'i' as c_char);
            assert_eq!(last.offset_from(buf.as_ptr()), 1);
            assert_eq!(dw1_strrchr(buf.as_ptr(), 0).offset_from(buf.as_ptr()), 4);
            assert!(dw1_strrchr(buf.as_ptr(), b'z' as c_char).is_null());

            // strncpy: no terminator when the source fills maxlen, NUL padding otherwise.
            let mut dst = [9 as c_char; 6];
            dw1_strncpy(dst.as_mut_ptr(), c"abcdef".as_ptr(), 3);
            assert_eq!(dst.map(|c| c as u8), *b"abc\x09\x09\x09");
            dw1_strncpy(dst.as_mut_ptr(), c"x".as_ptr(), 5);
            assert_eq!(dst.map(|c| c as u8), *b"x\0\0\0\0\x09");
        }
    }

    #[test]
    fn bios_memory_functions_follow_psx_spx() {
        let mut buf = [7u8; 4];
        let p = buf.as_mut_ptr();
        unsafe {
            assert!(dw1_memset(p, 1, 0).is_null());
            assert!(dw1_memset(p, 1, -1).is_null());
            assert_eq!(dw1_memset(p, 1, 2), p.cast());
            assert_eq!(buf, [1, 1, 7, 7]);
            assert_eq!(dw1_bzero(p, 1), p.cast());
            assert_eq!(buf, [0, 1, 7, 7]);
            let src = [9u8, 8];
            assert_eq!(dw1_memcpy(p.cast(), src.as_ptr().cast(), -5), p.cast());
            assert_eq!(buf, [0, 1, 7, 7]);
            assert_eq!(
                dw1_memcpy(p.add(2).cast(), src.as_ptr().cast(), 2),
                p.add(2).cast()
            );
            assert_eq!(buf, [0, 1, 9, 8]);
            assert!(dw1_memcpy(core::ptr::null_mut(), src.as_ptr().cast(), 2).is_null());
        }
        assert_eq!(dw1_abs(-5), 5);
        assert_eq!(dw1_abs(i32::MIN), i32::MIN);
    }
}
