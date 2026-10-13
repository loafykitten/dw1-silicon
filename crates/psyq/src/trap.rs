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
