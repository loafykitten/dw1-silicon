//! Trapping stubs for SDK calls that aren't implemented yet.
//!
//! The game C links against every SDK function it calls from the start (Phase 1); the ones
//! nobody has written yet stop the program with their name instead of doing something
//! plausible. A stub takes no arguments: on arm64 the caller's arguments sit unread in
//! registers, and nothing returns, so the C prototype doesn't matter.

/// Stops the program at an SDK call with no implementation yet.
#[cold]
pub fn unimplemented(name: &str) -> ! {
    eprintln!(
        "psyq: unimplemented SDK call `{name}` (trapping stub; see crates/psyq/src/stubs.rs)"
    );
    // Which game code asked: walk the frame pointers (arm64 macOS always keeps them; Rust's
    // own unwinder stops early in this panic=abort build) and print the callers' symbols.
    unsafe extern "C" {
        fn backtrace(buffer: *mut *mut std::ffi::c_void, size: i32) -> i32;
        fn backtrace_symbols_fd(buffer: *const *mut std::ffi::c_void, size: i32, fd: i32);
    }
    let mut frames = [std::ptr::null_mut(); 12];
    unsafe {
        let n = backtrace(frames.as_mut_ptr(), frames.len() as i32);
        // Skip this function and the stub.
        if n > 2 {
            backtrace_symbols_fd(frames.as_ptr().add(2), n - 2, 2);
        }
    }
    std::process::abort();
}

/// Defines each named function as an `extern "C"` trapping stub with the C name.
macro_rules! stubs {
    ($($name:ident)*) => {
        $(
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "C" fn $name() {
                $crate::trap::unimplemented(stringify!($name));
            }
        )*
    };
}

pub(crate) use stubs;
