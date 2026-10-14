//! Digimon World as a native Apple Silicon app.
//!
//! For now (Phase 2) the whole game C is linked and its `main()` runs until the first SDK call
//! with no Rust implementation, which stops the program with that call's name.

use std::path::PathBuf;

mod canon;
mod headless;
mod replay;
mod window;

/// The disc's `.cue`: `DW1_DISC`, else `<workspace>/disc/Digimon World (USA).cue`.
fn disc_path() -> PathBuf {
    std::env::var_os("DW1_DISC")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../disc/Digimon World (USA).cue")
        })
}

fn open_disc() -> Result<pack::Disc, String> {
    let path = disc_path();
    if !path.exists() {
        return Err(format!(
            "no disc at {}: set DW1_DISC to the .cue of your Digimon World (USA) image",
            path.display()
        ));
    }
    let disc = pack::Disc::open(&path).map_err(|e| e.to_string())?;
    // Reads the whole image once (about a second), so a wrong or damaged dump is caught here
    // and not as garbage in the game.
    disc.verify()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(disc)
}

/// `DW1_CRASH_TRACE=1`: on SIGSEGV/SIGBUS print the call stack (symbols) to stderr. lldb can't
/// attach in the agent sandbox, and the game's crashes are otherwise silent (exit 139).
///
/// The handler only uses calls that are safe in a signal handler on macOS: `write`,
/// `backtrace` (primed here, so it doesn't load anything when it runs) and
/// `backtrace_symbols_fd` (writes to the descriptor without allocating), then `_exit`. It runs
/// on its own stack, so a stack overflow is reported too, and resets to the default action on
/// entry, so a fault inside it ends the process instead of looping.
fn install_crash_trace() {
    use std::ffi::c_void;
    #[repr(C)]
    struct SigAction {
        handler: extern "C" fn(i32),
        mask: u32,
        flags: i32,
    }
    #[repr(C)]
    struct StackT {
        sp: *mut c_void,
        size: usize,
        flags: i32,
    }
    // <sys/signal.h>
    const SA_ONSTACK: i32 = 0x0001;
    const SA_RESETHAND: i32 = 0x0004;
    const SIGBUS: i32 = 10;
    const SIGSEGV: i32 = 11;
    unsafe extern "C" {
        fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
        fn sigaltstack(ss: *const StackT, old: *mut StackT) -> i32;
        fn backtrace(buffer: *mut *mut c_void, size: i32) -> i32;
        fn backtrace_symbols_fd(buffer: *const *mut c_void, size: i32, fd: i32);
        fn write(fd: i32, buf: *const c_void, n: usize) -> isize;
        fn _exit(code: i32) -> !;
    }
    extern "C" fn on_crash(_sig: i32) {
        const MSG: &[u8] = b"dw1: fatal signal (SIGSEGV or SIGBUS); call stack:\n";
        let mut frames = [std::ptr::null_mut(); 32];
        unsafe {
            write(2, MSG.as_ptr().cast(), MSG.len());
            let n = backtrace(frames.as_mut_ptr(), frames.len() as i32);
            backtrace_symbols_fd(frames.as_ptr(), n, 2);
            _exit(139);
        }
    }
    unsafe {
        let mut prime = [std::ptr::null_mut(); 1];
        backtrace(prime.as_mut_ptr(), 1);
        // 256 KB for the handler, kept for the life of the process.
        let stack = Box::leak(vec![0u8; 256 * 1024].into_boxed_slice());
        let ss = StackT {
            sp: stack.as_mut_ptr().cast(),
            size: stack.len(),
            flags: 0,
        };
        let act = SigAction {
            handler: on_crash,
            mask: 0,
            flags: SA_ONSTACK | SA_RESETHAND,
        };
        if sigaltstack(&ss, std::ptr::null_mut()) != 0
            || sigaction(SIGSEGV, &act, std::ptr::null_mut()) != 0
            || sigaction(SIGBUS, &act, std::ptr::null_mut()) != 0
        {
            eprintln!("dw1: DW1_CRASH_TRACE: could not install the handler");
        }
    }
}

fn main() {
    if std::env::var_os("DW1_CRASH_TRACE").is_some() {
        install_crash_trace();
    }
    match open_disc() {
        Ok(disc) => psyq::libcd::insert_disc(disc),
        Err(e) => {
            eprintln!("dw1: {e}");
            std::process::exit(1);
        }
    }
    // Memory cards: an image file per slot if given, else a fresh formatted card (the default).
    for (slot, var) in ["DW1_CARD1", "DW1_CARD2"].into_iter().enumerate() {
        if let Some(path) = std::env::var_os(var) {
            let card = std::fs::read(&path)
                .ok()
                .and_then(|b| psyq::libmcrd::Card::from_bytes(&b));
            match card {
                Some(card) => psyq::libmcrd::insert_card(slot, Some(card)),
                None => {
                    eprintln!(
                        "dw1: {var}: {} is not a 128 KB card image",
                        path.to_string_lossy()
                    );
                    std::process::exit(1);
                }
            }
        }
    }
    // Headless for the emulator-comparison runs (and `DW1_HEADLESS=1`); the window otherwise.
    let headless = [
        "DW1_HEADLESS",
        "DW1_REPLAY",
        "DW1_SHOTS",
        "DW1_STOP_AT",
        "DW1_START_AT",
        "DW1_MOVIE",
    ]
    .iter()
    .any(|v| std::env::var_os(v).is_some_and(|x| !x.is_empty() && x != "0"));
    if headless {
        psyq::libetc::set_host(Box::new(headless::Headless::from_env()));
    } else {
        match window::Window::new() {
            Ok(w) => psyq::libetc::set_host(Box::new(w)),
            Err(e) => {
                eprintln!("dw1: cannot open the window: {e}");
                std::process::exit(1);
            }
        }
    }
    // DW1_MOVIE=<id> plays one movie on its own (0 = the intro) and stops: the game's movie
    // player, without the boot that precedes it in main().
    let movie = std::env::var("DW1_MOVIE")
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| {
            // MOV_playMovie indexes its four-entry table without a check.
            match v.parse::<i32>() {
                Ok(id @ 0..=3) => id,
                _ => {
                    eprintln!("dw1: DW1_MOVIE must be a movie number from 0 to 3, not {v:?}");
                    std::process::exit(2);
                }
            }
        });
    if let Some(id) = movie {
        psyq::libetc::ResetCallback();
        psyq::libgpu::ResetGraph(0);
        psyq::libcd::CdInit();
        let completed = unsafe { decomp_c::MOV_playMovie(id) };
        eprintln!(
            "dw1: movie {id} {} at vblank {}",
            if completed == 1 {
                "played to its end"
            } else {
                "was skipped"
            },
            psyq::libetc::vblank_count()
        );
        return;
    }
    // The game's main() never returns on the PS1.
    let status = unsafe { decomp_c::dw1_game_main() };
    eprintln!("dw1: the game's main() returned {status}");
}
