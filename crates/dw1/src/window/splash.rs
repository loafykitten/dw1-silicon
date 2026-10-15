//! The start-up splash: a few lines of text on a dark screen that say this is the native Mac
//! build, so it can't be taken for the PS1 original. Drawn with SDL's debug font on a software
//! canvas the window's size, shown through the renderer before the game starts; it touches no
//! GPU, clock, pad or game state. `DW1_SPLASH_SHOT=<file.png>` saves it at full brightness
//! (for documentation).

use std::time::{Duration, Instant};

use render::{Fit, Picture};
use sdl3::event::Event;
use sdl3::pixels::PixelFormat;
use sdl3::render::Canvas;
use sdl3::surface::Surface;

use super::{Window, letterbox};

/// Fade in, hold, fade out.
const FADE: Duration = Duration::from_millis(500);
const HOLD: Duration = Duration::from_millis(1800);

/// The picture is laid out on a 320x240 grid (the game's own width) and scaled like the game.
const GRID: (u32, u32) = (320, 240);

pub(super) type Rgb = (u8, u8, u8);

pub(super) const BACKGROUND: Rgb = (14, 16, 24);

/// (text, size in debug-font cells of 8 px, y on the grid, colour).
const LINES: [(&str, f32, f32, Rgb); 5] = [
    ("Digimon World", 2.0, 78.0, (235, 238, 245)),
    (
        "Native Mac build, written in Rust",
        1.0,
        114.0,
        (170, 205, 255),
    ),
    ("loafykitten/dw1-silicon", 1.0, 130.0, (140, 148, 168)),
    ("Unofficial fan project", 1.0, 146.0, (100, 108, 126)),
    ("Press any button", 1.0, 204.0, (90, 96, 112)),
];

/// Brightness of the text at `t` into the splash, 0 to 1.
fn brightness(t: Duration) -> f32 {
    let end = FADE + HOLD + FADE;
    let t = t.min(end);
    if t < FADE {
        t.as_secs_f32() / FADE.as_secs_f32()
    } else if t < FADE + HOLD {
        1.0
    } else {
        (end - t).as_secs_f32() / FADE.as_secs_f32()
    }
}

fn mix(from: Rgb, to: Rgb, k: f32) -> Rgb {
    let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
    (f(from.0, to.0), f(from.1, to.1), f(from.2, to.2))
}

/// Where a line of `chars` characters at `size` goes, for a grid drawn at `(x, y)` with `unit`
/// output pixels per grid unit: the glyph scale and the top-left in output pixels, centred.
fn place(chars: usize, size: f32, gy: f32, x: f32, y: f32, unit: f32) -> (f32, (f32, f32)) {
    let cells = chars as f32 * 8.0 * size;
    (
        unit * size,
        (x + (GRID.0 as f32 - cells) / 2.0 * unit, y + gy * unit),
    )
}

impl Window {
    /// Shows the splash for about 2.8 s; any key, gamepad button or click ends it early.
    /// Closing the window exits 0, as in the game. Input is handled as in the game, so a
    /// controller plugged in meanwhile works, and a button still held at the end isn't a game
    /// press until released.
    pub fn splash(&mut self) {
        let shot = std::env::var_os("DW1_SPLASH_SHOT");
        let start = Instant::now();
        let mut shot_pending = shot.is_some();
        let mut pixels = Vec::new();
        loop {
            let t = start.elapsed();
            let (w, h) = self.window.size_in_pixels();
            let (w, h) = self.renderer.drawable_size(w, h);
            // A screenshot request holds at full brightness for the frame it captures. A window
            // with no area (in a transition) shows nothing this time.
            let level = if shot_pending { 1.0 } else { brightness(t) };
            match (w > 0 && h > 0).then(|| draw_splash(w, h, level, &mut pixels)) {
                None => {}
                Some(Ok(())) => {
                    if let (true, Some(path)) = (shot_pending, &shot) {
                        save_png(path.as_ref(), &pixels, w, h);
                        shot_pending = false;
                    }
                    let picture = Picture {
                        pixels: &pixels,
                        w,
                        h,
                    };
                    Self::show(
                        &mut self.renderer,
                        &self.window,
                        Some(picture),
                        Fit::Exact,
                        None,
                    );
                }
                Some(Err(e)) => {
                    eprintln!("dw1: splash: {e}");
                    return;
                }
            }
            let events: Vec<Event> = self.pump.poll_iter().collect();
            let skip = events.iter().any(|ev| {
                matches!(
                    ev,
                    Event::KeyDown { repeat: false, .. }
                        | Event::MouseButtonDown { .. }
                        | Event::GamepadButtonDown { .. }
                )
            });
            for ev in events {
                self.handle(ev);
            }
            if skip || t >= FADE + HOLD + FADE {
                self.held_over = self.held_pad();
                return;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }
}

/// Draws the splash at `brightness` on a `w x h` software canvas into `out` (0x00RRGGBB).
fn draw_splash(w: u32, h: u32, brightness: f32, out: &mut Vec<u32>) -> Result<(), String> {
    draw_lines(w, h, &LINES, brightness, out)
}

/// Draws centred lines of text (text, size in 8 px font cells, y on the 320x240 grid, colour)
/// at `brightness` on a `w x h` software canvas into `out` (0x00RRGGBB).
pub(super) fn draw_lines(
    w: u32,
    h: u32,
    lines: &[(&str, f32, f32, Rgb)],
    brightness: f32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let surface = Surface::new(w, h, PixelFormat::XRGB8888).map_err(|e| e.to_string())?;
    let mut canvas: Canvas<Surface> = surface.into_canvas().map_err(|e| e.to_string())?;
    canvas.set_draw_color(BACKGROUND);
    canvas.clear();
    let (x, y, gw, _) = letterbox(GRID.0, GRID.1, w, h);
    // Output pixels per grid unit.
    let unit = gw / GRID.0 as f32;
    for &(text, size, gy, colour) in lines {
        let (glyph, (px, py)) = place(text.chars().count(), size, gy, x, y, unit);
        canvas.set_draw_color(mix(BACKGROUND, colour, brightness));
        canvas.set_scale(glyph, glyph).ok();
        let _ = canvas.draw_debug_text(text, (px / glyph, py / glyph));
    }
    canvas.present();
    let surface = canvas.into_surface();
    let pitch = surface.pitch() as usize;
    out.clear();
    surface.with_lock(|px| {
        for row in 0..h as usize {
            out.extend(
                px[row * pitch..][..w as usize * 4]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|&b| u32::from_ne_bytes(b) & 0x00ff_ffff),
            );
        }
    });
    Ok(())
}

/// Writes `w x h` pixels of 0x00RRGGBB to a PNG; a failure is reported and otherwise ignored.
fn save_png(path: &std::path::Path, pixels: &[u32], w: u32, h: u32) {
    let result = (|| -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
            .collect();
        writer.write_image_data(&rgb).map_err(|e| e.to_string())
    })();
    if let Err(e) = result {
        eprintln!("dw1: DW1_SPLASH_SHOT: {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fades_in_holds_and_fades_out() {
        let ms = Duration::from_millis;
        assert_eq!(brightness(ms(0)), 0.0);
        assert_eq!(brightness(ms(250)), 0.5);
        assert_eq!(brightness(FADE), 1.0);
        assert_eq!(brightness(FADE + HOLD / 2), 1.0);
        assert_eq!(brightness(FADE + HOLD + FADE / 2), 0.5);
        assert_eq!(brightness(ms(10_000)), 0.0);
    }

    #[test]
    fn every_line_fits_at_any_window_size() {
        for (ow, oh) in [(960, 720), (1920, 1080), (320, 240), (200, 150), (100, 300)] {
            let (x, y, w, h) = letterbox(GRID.0, GRID.1, ow, oh);
            let unit = w / GRID.0 as f32;
            for (text, size, gy, _) in LINES {
                let (glyph, (px, py)) = place(text.len(), size, gy, x, y, unit);
                let right = px + text.len() as f32 * 8.0 * glyph;
                assert!(px >= x && right <= x + w + 0.01, "{text} at {ow}x{oh}");
                assert!(py >= y && py + 8.0 * glyph <= y + h, "{text} at {ow}x{oh}");
            }
        }
    }
}
