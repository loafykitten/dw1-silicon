//! The disc at start-up (Phase 9.1): the image `crate::disc::at_startup` found is checked with
//! "Checking the disc image" on screen; when none is known yet, or the one known doesn't work,
//! the window says what it needs and asks for the image with the macOS file dialog (or a `.cue`
//! dropped on the window), checks it and keeps its path for the next runs. Checking reads the
//! whole image, so it runs on its own thread and the window keeps answering. Drawn like the
//! splash, before the game starts; it touches no game state.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use render::{Fit, Picture};
use sdl3::dialog::{DialogError, DialogFileFilter};
use sdl3::event::Event;
use sdl3::gamepad::Button;
use sdl3::keyboard::Scancode;

use super::Window;
use super::splash::{Rgb, draw_lines};
use crate::disc::{Origin, Start};

const TITLE: Rgb = (235, 238, 245);
const ASK: Rgb = (170, 205, 255);
const PLAIN: Rgb = (140, 148, 168);
const PROBLEM: Rgb = (255, 170, 110);
const HINT: Rgb = (100, 108, 126);

/// Characters a line of size-1 text holds on the 320-wide grid, with a margin.
const WRAP: usize = 38;
/// Most lines of a problem shown.
const PROBLEM_LINES: usize = 5;

/// `text` broken at spaces into lines of at most `width` characters (a longer word gets a line
/// of its own, cut at `width`).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let word: String = word.chars().take(width).collect();
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The screen's lines: what to choose, then the problem with the last image (if any), then
/// how to choose, or "Checking" while an image is read. `asked`: the player is choosing (else
/// the image found at start-up is being checked: the title and "Checking" only).
fn screen(problem: Option<&str>, asked: bool, checking: bool) -> Vec<(String, f32, f32, Rgb)> {
    if !asked {
        return vec![
            ("Digimon World".to_string(), 2.0, 78.0, TITLE),
            ("Checking the disc image...".to_string(), 1.0, 130.0, PLAIN),
        ];
    }
    let mut lines = vec![
        ("Digimon World".to_string(), 2.0, 36.0, TITLE),
        ("Choose your disc image".to_string(), 1.0, 74.0, ASK),
        (
            "Digimon World (USA), SLUS-01032,".to_string(),
            1.0,
            92.0,
            PLAIN,
        ),
        ("as a .cue/.bin pair".to_string(), 1.0, 104.0, PLAIN),
    ];
    if let Some(p) = problem {
        let mut wrapped = wrap(p, WRAP);
        if wrapped.len() > PROBLEM_LINES {
            wrapped.truncate(PROBLEM_LINES);
            if let Some(last) = wrapped.last_mut() {
                *last = last.chars().take(WRAP - 3).collect::<String>() + "...";
            }
        }
        for (i, l) in wrapped.into_iter().enumerate() {
            lines.push((l, 1.0, 128.0 + 11.0 * i as f32, PROBLEM));
        }
    }
    if checking {
        lines.push(("Checking the image...".to_string(), 1.0, 204.0, ASK));
    } else {
        lines.push((
            "Enter or click: choose the .cue".to_string(),
            1.0,
            198.0,
            HINT,
        ));
        lines.push(("or drop it on this window".to_string(), 1.0, 210.0, HINT));
    }
    lines
}

type Checked = mpsc::Receiver<Result<pack::Disc, String>>;

/// Starts checking the image at `path` on its own thread.
fn check(path: PathBuf, origin: Origin) -> (PathBuf, Origin, Checked) {
    let (done, result) = mpsc::channel();
    let p = path.clone();
    std::thread::spawn(move || {
        let _ = done.send(crate::disc::open_checked(&p));
    });
    (path, origin, result)
}

impl Window {
    /// The disc to play: checks the image `start` names (a wrong `DW1_DISC` stops the program;
    /// any other wrong image goes to the first-run screen with the reason), or asks until one
    /// checks out. An image chosen here is kept for the next runs. Closing the window exits 0.
    pub fn find_disc(&mut self, start: Start) -> pack::Disc {
        let (tx, rx) = mpsc::channel::<Option<PathBuf>>();
        let mut dialog_open = false;
        let mut pixels = Vec::new();
        let (mut asked, mut problem) = (false, None);
        // The image being checked, where it came from, and where its result comes back.
        let mut checking = None;
        match start {
            Start::Check(path, origin) => checking = Some(check(path, origin)),
            Start::Ask(p) => (asked, problem) = (true, p),
        }
        loop {
            let (w, h) = self.window.size_in_pixels();
            let (w, h) = self.renderer.drawable_size(w, h);
            if w > 0 && h > 0 {
                let lines = screen(problem.as_deref(), asked, checking.is_some());
                let refs: Vec<(&str, f32, f32, Rgb)> = lines
                    .iter()
                    .map(|(t, s, y, c)| (t.as_str(), *s, *y, *c))
                    .collect();
                match draw_lines(w, h, &refs, 1.0, &mut pixels) {
                    Ok(()) => {
                        Self::show(
                            &mut self.renderer,
                            &self.window,
                            Some(Picture {
                                pixels: &pixels,
                                w,
                                h,
                            }),
                            Fit::Exact,
                            None,
                        );
                    }
                    Err(e) => eprintln!("dw1: disc screen: {e}"),
                }
            }
            if let Some((path, origin, result)) = &checking {
                let outcome = match result.try_recv() {
                    Ok(r) => Some(r),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("The image couldn't be checked.".to_string()))
                    }
                };
                match outcome {
                    Some(Ok(disc)) => {
                        if *origin == Origin::Chosen {
                            crate::disc::remember(path);
                        }
                        // what is still queued (the press that chose, say) is seen before the
                        // game's held-button mask is taken
                        for ev in self.pump.poll_iter().collect::<Vec<_>>() {
                            self.handle(ev);
                        }
                        self.held_over = self.held_pad();
                        return disc;
                    }
                    Some(Err(e)) => {
                        eprintln!("dw1: {}: {e}", path.display());
                        if *origin == Origin::Env {
                            eprintln!("dw1: DW1_DISC names an image that doesn't work");
                            std::process::exit(1);
                        }
                        problem = Some(crate::disc::problem(*origin, path, &e));
                        asked = true;
                        checking = None;
                    }
                    None => {}
                }
            }
            let mut chosen: Option<PathBuf> = None;
            let mut ask = false;
            for ev in self.pump.poll_iter().collect::<Vec<_>>() {
                match ev {
                    Event::DropFile { ref filename, .. } => {
                        chosen = Some(PathBuf::from(filename));
                    }
                    Event::KeyDown {
                        scancode: Some(Scancode::Return | Scancode::KpEnter | Scancode::Space),
                        repeat: false,
                        ..
                    }
                    | Event::MouseButtonDown { .. }
                    | Event::GamepadButtonDown {
                        button: Button::South,
                        ..
                    } => ask = true,
                    _ => {}
                }
                self.handle(ev);
            }
            if ask && asked && !dialog_open && checking.is_none() {
                dialog_open = self.open_dialog(tx.clone());
            }
            if let Ok(picked) = rx.try_recv() {
                dialog_open = false;
                chosen = chosen.or(picked);
            }
            if let Some(path) = chosen.filter(|_| asked && checking.is_none()) {
                checking = Some(check(path, Origin::Chosen));
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    /// Opens the macOS file dialog for a `.cue` or `.bin`; the choice (None if cancelled) comes
    /// back on `tx` when the dialog closes. False if it couldn't open.
    fn open_dialog(&self, tx: mpsc::Sender<Option<PathBuf>>) -> bool {
        let filters = [DialogFileFilter {
            name: "Disc image (.cue, .bin)",
            pattern: "cue;bin",
        }];
        let result = sdl3::dialog::show_open_file_dialog(
            &filters,
            None::<PathBuf>,
            false,
            Some(&self.window),
            Box::new(move |chosen, _| {
                let path = match chosen {
                    Ok(paths) => paths.into_iter().next(),
                    Err(DialogError::Canceled) => None,
                    Err(e) => {
                        eprintln!("dw1: file dialog: {e}");
                        None
                    }
                };
                let _ = tx.send(path);
            }),
        );
        match result {
            Ok(()) => true,
            Err(e) => {
                eprintln!("dw1: file dialog: {e}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_messages_wrap_at_spaces() {
        assert_eq!(
            wrap("This is a European disc (SLES_028.14).", 20),
            ["This is a European", "disc (SLES_028.14)."]
        );
        assert_eq!(wrap("", 20), Vec::<String>::new());
        assert_eq!(wrap("abcdefghijklmnopqrstuvwxyz", 10), ["abcdefghij"]);
    }

    #[test]
    fn every_line_fits_the_grid() {
        let long = "word ".repeat(80);
        for (asked, checking) in [(true, false), (true, true), (false, true)] {
            for (text, size, y, _) in screen(Some(&long), asked, checking) {
                assert!(text.chars().count() as f32 * 8.0 * size <= 320.0, "{text}");
                assert!(y + 8.0 * size <= 240.0, "{text} at {y}");
            }
        }
        assert_eq!(
            screen(Some(&long), true, false)
                .iter()
                .filter(|l| l.3 == PROBLEM)
                .count(),
            PROBLEM_LINES
        );
    }
}
