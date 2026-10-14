//! The start-up splash: a few lines of text on a dark screen that say this is the native Mac
//! build, so it can't be taken for the PS1 original. Drawn with SDL's debug font straight on
//! the window's canvas before the game starts; it touches no GPU, clock, pad or game state.
//! `DW1_SPLASH_SHOT=<file.png>` saves the canvas at full brightness (for documentation).

use std::time::{Duration, Instant};

use sdl3::event::Event;
use sdl3::pixels::PixelFormat;

use super::{Window, letterbox};

/// Fade in, hold, fade out.
const FADE: Duration = Duration::from_millis(500);
const HOLD: Duration = Duration::from_millis(1800);

/// The picture is laid out on a 320x240 grid (the game's own width) and scaled like the game.
const GRID: (u32, u32) = (320, 240);

type Rgb = (u8, u8, u8);

const BACKGROUND: Rgb = (14, 16, 24);

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
        loop {
            let t = start.elapsed();
            // A screenshot request holds at full brightness for the frame it captures.
            self.draw_splash(if shot_pending { 1.0 } else { brightness(t) });
            if let (true, Some(path)) = (shot_pending, &shot) {
                self.save_canvas(path.as_ref());
                shot_pending = false;
            }
            self.canvas.present();
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

    fn draw_splash(&mut self, brightness: f32) {
        self.canvas.set_scale(1.0, 1.0).ok();
        self.canvas.set_draw_color(BACKGROUND);
        self.canvas.clear();
        let (ow, oh) = self.canvas.output_size().unwrap_or((960, 720));
        let (x, y, w, _) = letterbox(GRID.0, GRID.1, ow, oh);
        // Output pixels per grid unit.
        let unit = w / GRID.0 as f32;
        for (text, size, gy, colour) in LINES {
            let (glyph, (px, py)) = place(text.len(), size, gy, x, y, unit);
            self.canvas
                .set_draw_color(mix(BACKGROUND, colour, brightness));
            self.canvas.set_scale(glyph, glyph).ok();
            let _ = self.canvas.draw_debug_text(text, (px / glyph, py / glyph));
        }
        self.canvas.set_scale(1.0, 1.0).ok();
    }

    /// Writes the canvas to a PNG; a failure is reported and otherwise ignored.
    fn save_canvas(&self, path: &std::path::Path) {
        let result = (|| -> Result<(), String> {
            let surface = self.canvas.read_pixels(None).map_err(|e| e.to_string())?;
            let surface = surface
                .convert_format(PixelFormat::RGB24)
                .map_err(|e| e.to_string())?;
            let (w, h, pitch) = (surface.width(), surface.height(), surface.pitch() as usize);
            let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().map_err(|e| e.to_string())?;
            let rows = surface.with_lock(|px| {
                (0..h as usize)
                    .flat_map(|r| px[r * pitch..r * pitch + w as usize * 3].iter().copied())
                    .collect::<Vec<u8>>()
            });
            writer.write_image_data(&rows).map_err(|e| e.to_string())
        })();
        if let Err(e) = result {
            eprintln!("dw1: DW1_SPLASH_SHOT: {}: {e}", path.display());
        }
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
