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
/// The handler runs on its own stack (so a stack overflow is reported too) and resets to the
/// default action on entry (so a fault inside it ends the process). It walks the frame-pointer
/// chain of the interrupted code from the registers the kernel saved (arm64 macOS keeps frame
/// pointers), so it doesn't depend on the system's stack walker, which finds nothing from an
/// alternate stack on older macOS. Output uses only `write`, `backtrace_symbols_fd` (writes to
/// the descriptor without allocating) and `_exit`.
fn install_crash_trace() {
    use std::ffi::c_void;
    #[repr(C)]
    struct SigAction {
        handler: extern "C" fn(i32, *mut c_void, *mut c_void),
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
    const SA_SIGINFO: i32 = 0x0040;
    const SIGBUS: i32 = 10;
    const SIGSEGV: i32 = 11;
    // <sys/_types/_ucontext.h>, <mach/arm/_structs.h> (arm64): `uc_mcontext` is the pointer at
    // byte 48 of the ucontext; in the mcontext, the 16-byte exception state is followed by the
    // thread state x0-x28, then fp, lr, sp, pc.
    const UC_MCONTEXT: usize = 48;
    const MC_FP: usize = 16 + 29 * 8;
    const MC_LR: usize = MC_FP + 8;
    const MC_PC: usize = MC_FP + 24;
    unsafe extern "C" {
        fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
        fn sigaltstack(ss: *const StackT, old: *mut StackT) -> i32;
        fn backtrace_symbols_fd(buffer: *const *mut c_void, size: i32, fd: i32);
        fn write(fd: i32, buf: *const c_void, n: usize) -> isize;
        fn _exit(code: i32) -> !;
    }
    extern "C" fn on_crash(_sig: i32, _info: *mut c_void, uc: *mut c_void) {
        const MSG: &[u8] = b"dw1: fatal signal (SIGSEGV or SIGBUS); call stack:\n";
        let mut frames = [std::ptr::null_mut::<c_void>(); 32];
        unsafe {
            write(2, MSG.as_ptr().cast(), MSG.len());
            let mc = uc.cast::<u8>().add(UC_MCONTEXT).cast::<*const u8>().read();
            let reg = |off: usize| mc.add(off).cast::<usize>().read();
            frames[0] = reg(MC_PC) as *mut c_void;
            frames[1] = reg(MC_LR) as *mut c_void;
            let mut n = 2;
            // Each frame record is (caller's fp, return address). Stop at anything that doesn't
            // look like a record further up the same stack.
            let mut fp = reg(MC_FP);
            while n < frames.len() && fp != 0 && fp.is_multiple_of(16) {
                let next = (fp as *const usize).read();
                let ret = (fp as *const usize).add(1).read();
                if ret == 0 {
                    break;
                }
                frames[n] = ret as *mut c_void;
                n += 1;
                if next <= fp || next - fp > 8 << 20 {
                    break;
                }
                fp = next;
            }
            backtrace_symbols_fd(frames.as_ptr(), n as i32, 2);
            _exit(139);
        }
    }
    unsafe {
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
            flags: SA_ONSTACK | SA_RESETHAND | SA_SIGINFO,
        };
        if sigaltstack(&ss, std::ptr::null_mut()) != 0
            || sigaction(SIGSEGV, &act, std::ptr::null_mut()) != 0
            || sigaction(SIGBUS, &act, std::ptr::null_mut()) != 0
        {
            eprintln!("dw1: DW1_CRASH_TRACE: could not install the handler");
        }
    }
}

/// Where the window build keeps the memory cards: `DW1_SAVE_DIR`, else
/// `~/Library/Application Support/dw1-silicon`; an empty `DW1_SAVE_DIR` keeps nothing.
fn save_dir() -> Option<std::path::PathBuf> {
    match std::env::var_os("DW1_SAVE_DIR") {
        Some(d) if d.is_empty() => None,
        Some(d) => Some(d.into()),
        None => std::env::var_os("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Application Support/dw1-silicon")),
    }
}

/// The window build's cards persist (Phase 7): slot n is `card<n>.mcd` in the save directory.
/// A card file that exists is the card (whatever `DW1_CARD<n>` says: that only seeds a card
/// that isn't there yet, so the launcher's fixture doesn't undo the player's saves); the game's
/// changes are written back as they happen (libmcrd: atomic, with a `.bak` of the old file).
fn keep_cards() {
    let Some(dir) = save_dir() else { return };
    for slot in 0..2 {
        let path = dir.join(format!("card{}.mcd", slot + 1));
        match std::fs::read(&path) {
            Ok(bytes) => match psyq::libmcrd::Card::from_bytes(&bytes) {
                Some(card) => psyq::libmcrd::insert_card(slot, Some(card)),
                None => {
                    eprintln!(
                        "dw1: {} is not a 128 KB card image; not using it",
                        path.display()
                    );
                    continue;
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!("dw1: {}: {e}; not using it", path.display());
                continue;
            }
        }
        eprintln!("dw1: memory card {} kept in {}", slot + 1, path.display());
        psyq::libmcrd::set_card_file(slot, Some(path));
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
        // Headless runs keep cards only when asked to (`DW1_SAVE_DIR` set), so replays
        // don't write anything by default.
        if std::env::var_os("DW1_SAVE_DIR").is_some_and(|d| !d.is_empty()) {
            keep_cards();
        }
        psyq::libetc::set_host(Box::new(headless::Headless::from_env()));
    } else {
        keep_cards();
        match window::Window::new() {
            Ok(mut w) => {
                if !std::env::var_os("DW1_NO_SPLASH").is_some_and(|x| !x.is_empty() && x != "0") {
                    w.splash();
                }
                psyq::libetc::set_host(Box::new(w));
            }
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
    // The overlays' initialised data as linked, for their reloads (decomp_c::overlays).
    decomp_c::overlays::snapshot();
    // The game's main() never returns on the PS1.
    let status = unsafe { decomp_c::dw1_game_main() };
    eprintln!("dw1: the game's main() returned {status}");
}

/// Overlay data lifetimes (decomp_c::overlays), in this binary because it links every overlay
/// object the way the app does.
#[cfg(test)]
mod overlay_tests {
    #[test]
    fn every_overlay_has_a_section_and_reset_restores_it() {
        use decomp_c::overlays::{dw1_overlay_reset, section_info, sections, snapshot};
        snapshot();
        let found = sections();
        // Every overlay unit but SHOP (resident) has initialised data.
        let ids: Vec<u32> = found.iter().map(|s| s.0).collect();
        for id in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15, 16] {
            assert!(
                ids.contains(&id),
                "overlay {id} has no data section: {found:?}"
            );
        }
        // BTL's resident initialised variables (its section's size depends on what the linker
        // keeps: the test binary dead-strips the game code that uses most of the data).
        let btl = found.iter().find(|s| s.0 == 1).unwrap();
        assert!(btl.1 > 0 && btl.2 >= 20, "BTL section {btl:?}");
        // A reset puts the linked bytes back but keeps the resident variables' values.
        let (start, len, keeps) = section_info(1).unwrap();
        let kept_byte = |p: *mut u8| {
            keeps
                .iter()
                .any(|&(a, n)| p >= a && (p as usize) < a as usize + n)
        };
        unsafe {
            let (kept, _) = keeps[0];
            let old_kept = kept.read();
            kept.write(old_kept ^ 0xA5);
            let probe = (0..len).map(|i| start.add(i)).find(|&p| !kept_byte(p));
            let old_probe = probe.map(|p| p.read());
            if let Some(p) = probe {
                p.write(old_probe.unwrap() ^ 0x5A);
            }
            dw1_overlay_reset(1);
            if let Some(p) = probe {
                assert_eq!(
                    p.read(),
                    old_probe.unwrap(),
                    "reset didn't restore the section"
                );
            }
            assert_eq!(
                kept.read(),
                old_kept ^ 0xA5,
                "reset didn't keep the resident variable"
            );
            kept.write(old_kept);
        }
    }
}
