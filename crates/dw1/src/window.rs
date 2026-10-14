//! The window host: SDL3's 2D renderer shows the game's display, paced to the PS1's field
//! rate, with the keyboard and gamepads as the pad. (The wgpu renderer is Phase 8.)
//!
//! Keys (physical positions, so other layouts work): arrows = D-pad, Z = Cross, X = Circle,
//! A = Square, S = Triangle, Enter = Start, right Shift = Select, Q/W = L1/R1, 1/2 = L2/R2.
//! Gamepads use SDL's standard layout (south = Cross, east = Circle, west = Square,
//! north = Triangle), the D-pad or left stick for directions, the triggers for L2/R2.
//! Closing the window or Cmd-Q exits 0. `DW1_WINDOW_LOG=1` logs frames presented and the
//! display size (first frame, on size changes, then every 60 frames).
//!
//! Before the game starts, `Window::splash` shows a short text screen (about 2.8 s; any key,
//! button or click skips it) saying this is the native Mac build. `DW1_NO_SPLASH=1` skips it;
//! `DW1_SPLASH_SHOT=<file.png>` saves it as a picture. Headless and replay runs never show it.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use sdl3::EventPump;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::keyboard::Scancode;
use sdl3::pixels::PixelFormat;
use sdl3::render::{FRect, ScaleMode, Texture, TextureCreator, WindowCanvas};
use sdl3::video::WindowContext;

mod splash;

/// The PS1's NTSC non-interlaced field rate: 53.693175 MHz / (3413 x 263) = 59.817 Hz
/// [doc: psx-spx GPU timings; PLAN.md's "Timing is measured", docs/s08-oracle.md]. Redux, the
/// oracle, counts at 60.009 Hz, which is a quirk of its counter and not what to match.
/// In thousandths of a hertz, so the arithmetic stays integer.
pub const FIELD_MILLIHZ: u64 = 59_817;

/// The rate the SPU's samples are played at: what libspu renders in a VBlank (the emulator's
/// 564,398 CPU cycles at 768 a sample, 734.89 samples) times the field rate, so the sound keeps
/// pace with the picture. That's 0.3% under the SPU's 44.1 kHz (the oracle's VBlank is a little
/// shorter than the PS1's); SDL resamples to the device, 5 cents flat.
pub const AUDIO_HZ: i32 = ((psyq::libspu::CYCLES_PER_VBLANK * FIELD_MILLIHZ)
    / (psyq::libspu::CYCLES_PER_SAMPLE * 1000)) as i32;
/// Most audio queued before the queue is dropped (a quarter second): after a stall the sound
/// catches up instead of lagging behind the picture for good.
const AUDIO_MAX_QUEUED_BYTES: i32 = AUDIO_HZ * 4 / 4;

/// How many fields behind the clock may fall before it resyncs instead of racing to catch up.
pub const MAX_LAG_FIELDS: u64 = 4;

/// Fixed-step field clock. Field `n` is due at `base + n / FIELD_HZ` (computed from the count,
/// not accumulated, so there is no drift). Times are nanoseconds on any monotonic origin.
pub struct Pacer {
    base_ns: u64,
    fields: u64,
}

impl Pacer {
    pub fn new(now_ns: u64) -> Pacer {
        Pacer {
            base_ns: now_ns,
            fields: 0,
        }
    }

    fn due_ns(&self, fields: u64) -> u64 {
        self.base_ns + (fields as u128 * 1_000_000_000_000 / FIELD_MILLIHZ as u128) as u64
    }

    /// A field is finished at `now_ns`: how long to wait until the next one is due. If the
    /// host is more than `MAX_LAG_FIELDS` late, restarts the clock from now (no wait).
    pub fn next(&mut self, now_ns: u64) -> u64 {
        self.fields += 1;
        let due = self.due_ns(self.fields);
        let late_by = now_ns.saturating_sub(due);
        if late_by > self.due_ns(MAX_LAG_FIELDS) - self.due_ns(0) {
            *self = Pacer::new(now_ns);
            return 0;
        }
        due.saturating_sub(now_ns)
    }
}

// libetc's PadRead bits (libetc.h).
const L2: u32 = 1 << 0;
const R2: u32 = 1 << 1;
const L1: u32 = 1 << 2;
const R1: u32 = 1 << 3;
const TRIANGLE: u32 = 1 << 4;
const CIRCLE: u32 = 1 << 5;
const CROSS: u32 = 1 << 6;
const SQUARE: u32 = 1 << 7;
const SELECT: u32 = 1 << 8;
const START: u32 = 1 << 11;
const UP: u32 = 1 << 12;
const RIGHT: u32 = 1 << 13;
const DOWN: u32 = 1 << 14;
const LEFT: u32 = 1 << 15;

pub fn key_bit(key: Scancode) -> u32 {
    match key {
        Scancode::Up => UP,
        Scancode::Down => DOWN,
        Scancode::Left => LEFT,
        Scancode::Right => RIGHT,
        Scancode::Z => CROSS,
        Scancode::X => CIRCLE,
        Scancode::A => SQUARE,
        Scancode::S => TRIANGLE,
        Scancode::Return | Scancode::KpEnter => START,
        Scancode::RShift => SELECT,
        Scancode::Q => L1,
        Scancode::W => R1,
        Scancode::_1 => L2,
        Scancode::_2 => R2,
        _ => 0,
    }
}

pub fn button_bit(button: Button) -> u32 {
    match button {
        Button::DPadUp => UP,
        Button::DPadDown => DOWN,
        Button::DPadLeft => LEFT,
        Button::DPadRight => RIGHT,
        Button::South => CROSS,
        Button::East => CIRCLE,
        Button::West => SQUARE,
        Button::North => TRIANGLE,
        Button::Start => START,
        Button::Back => SELECT,
        Button::LeftShoulder => L1,
        Button::RightShoulder => R1,
        _ => 0,
    }
}

const AXIS_THRESHOLD: i16 = 16_000;

/// Bits an axis contributes at `value` (both directions of a stick axis are reported, so the
/// result is the whole set for that axis).
pub fn axis_bits(axis: Axis, value: i16) -> u32 {
    let hi = value > AXIS_THRESHOLD;
    let lo = value < -AXIS_THRESHOLD;
    match axis {
        Axis::LeftX => (if lo { LEFT } else { 0 }) | (if hi { RIGHT } else { 0 }),
        Axis::LeftY => (if lo { UP } else { 0 }) | (if hi { DOWN } else { 0 }),
        Axis::TriggerLeft if hi => L2,
        Axis::TriggerRight if hi => R2,
        _ => 0,
    }
}

/// The destination rectangle `(x, y, w, h)` for a `src_w x src_h` picture in a
/// `win_w x win_h` window: the largest whole-number scale (at least 1 when it fits; when the
/// window is smaller than the picture, the fit scale, so it is never cropped), centred.
pub fn letterbox(src_w: u32, src_h: u32, win_w: u32, win_h: u32) -> (f32, f32, f32, f32) {
    let (sw, sh) = (src_w.max(1) as f32, src_h.max(1) as f32);
    let (ww, wh) = (win_w as f32, win_h as f32);
    let fit = (ww / sw).min(wh / sh);
    let scale = if fit >= 1.0 { fit.floor() } else { fit };
    let (w, h) = (sw * scale, sh * scale);
    (((ww - w) / 2.0).floor(), ((wh - h) / 2.0).floor(), w, h)
}

/// One gamepad's held inputs: buttons and each axis kept apart, so a stick returning to centre
/// doesn't release a D-pad direction (or the other way round).
#[derive(Default)]
pub struct PadState {
    buttons: u32,
    axes: BTreeMap<i32, u32>,
}

impl PadState {
    pub fn button(&mut self, button: Button, down: bool) {
        if down {
            self.buttons |= button_bit(button);
        } else {
            self.buttons &= !button_bit(button);
        }
    }

    pub fn axis(&mut self, axis: Axis, value: i16) {
        self.axes.insert(axis as i32, axis_bits(axis, value));
    }

    pub fn bits(&self) -> u32 {
        self.axes.values().fold(self.buttons, |a, b| a | b)
    }
}

pub struct Window {
    canvas: WindowCanvas,
    creator: TextureCreator<WindowContext>,
    texture: Option<(Texture, usize, usize)>,
    pump: EventPump,
    gamepad_sys: sdl3::GamepadSubsystem,
    gamepads: Vec<Gamepad>,
    keys: u32,
    /// Per gamepad, by joystick id.
    pads: BTreeMap<sdl3::joystick::JoystickId, PadState>,
    buf: Vec<u32>,
    pacer: Option<Pacer>,
    t0: Instant,
    frames: u64,
    last_size: Option<(usize, usize)>,
    log: bool,
    /// Pad bits still held from before the game took over (the splash's skip press): not
    /// reported until released.
    held_over: u32,
    /// The sound output (kept open; libspu pushes into it). None if there's no audio device.
    _audio: Option<AudioStreamOwner>,
    /// The title says a save failed.
    save_failed_shown: bool,
}

/// The audio stream as libspu's sink holds it. SDL's stream calls are thread-safe [doc: SDL3
/// SDL_PutAudioStreamData], and the sink only runs on the game's (main) thread anyway.
struct SinkStream(*mut sdl3::sys::audio::SDL_AudioStream);
// SAFETY: see above.
unsafe impl Send for SinkStream {}

/// Opens the default playback device and points libspu's samples at it. DW1_MUTE=1 skips it.
fn open_audio(sdl: &sdl3::Sdl) -> Option<AudioStreamOwner> {
    if std::env::var("DW1_MUTE").is_ok_and(|v| !v.is_empty() && v != "0") {
        return None;
    }
    let spec = AudioSpec {
        freq: Some(AUDIO_HZ),
        channels: Some(2),
        format: Some(AudioFormat::s16_sys()),
    };
    let stream = match sdl
        .audio()
        .and_then(|a| a.default_playback_device().open_device_stream(Some(&spec)))
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("dw1: no sound: {e}");
            return None;
        }
    };
    if let Err(e) = stream.resume() {
        eprintln!("dw1: no sound: {e}");
        return None;
    }
    let sink = SinkStream(stream.raw());
    psyq::libspu::set_audio_sink(Box::new(move |samples: &[i16]| {
        let sink = &sink;
        // SAFETY: the window owns the stream and takes this sink away before closing it
        // (`Drop for Window`).
        unsafe {
            // Behind by more than the limit (a stall): drop what's queued and start again
            // from now, rather than play stale sound late for good.
            if sdl3::sys::audio::SDL_GetAudioStreamQueued(sink.0) > AUDIO_MAX_QUEUED_BYTES {
                sdl3::sys::audio::SDL_ClearAudioStream(sink.0);
            }
            sdl3::sys::audio::SDL_PutAudioStreamData(
                sink.0,
                samples.as_ptr().cast(),
                (samples.len() * 2) as i32,
            );
        }
    }));
    Some(stream)
}

impl Drop for Window {
    /// The audio stream goes with the window: take libspu's sink (which holds its pointer)
    /// away first.
    fn drop(&mut self) {
        psyq::libspu::clear_audio_sink();
    }
}

// SAFETY: SDL objects must stay on the main thread, and the host is only ever called from the
// game's thread, which is main (`dw1_game_main` is called from `main`). `Host: Send` is
// required only because the clock lives in a static.
unsafe impl Send for Window {}

impl Window {
    pub fn new() -> Result<Window, String> {
        let sdl = sdl3::init().map_err(|e| e.to_string())?;
        let video = sdl.video().map_err(|e| e.to_string())?;
        let window = video
            .window("Digimon World (dw1-silicon)", 960, 720)
            .resizable()
            .high_pixel_density()
            .build()
            .map_err(|e| e.to_string())?;
        let canvas = window.into_canvas();
        let creator = canvas.texture_creator();
        let gamepad_sys = sdl.gamepad().map_err(|e| e.to_string())?;
        let pump = sdl.event_pump().map_err(|e| e.to_string())?;
        let audio = open_audio(&sdl);
        let mut w = Window {
            canvas,
            creator,
            texture: None,
            pump,
            gamepad_sys,
            gamepads: Vec::new(),
            keys: 0,
            pads: BTreeMap::new(),
            buf: vec![0; 640 * 480],
            pacer: None,
            t0: Instant::now(),
            frames: 0,
            last_size: None,
            log: std::env::var_os("DW1_WINDOW_LOG").is_some(),
            held_over: 0,
            _audio: audio,
            save_failed_shown: false,
        };
        for id in w.gamepad_sys.gamepads().unwrap_or_default() {
            w.open_gamepad(id);
        }
        Ok(w)
    }

    fn open_gamepad(&mut self, id: sdl3::joystick::JoystickId) {
        if let Ok(g) = self.gamepad_sys.open(id) {
            self.gamepads.push(g);
        }
    }

    fn events(&mut self) {
        let events: Vec<Event> = self.pump.poll_iter().collect();
        for ev in events {
            self.handle(ev);
        }
    }

    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Quit { .. } => {
                if self.log {
                    eprintln!("dw1: window closed after {} frames", self.frames);
                }
                std::process::exit(0);
            }
            Event::KeyDown {
                scancode: Some(k),
                repeat: false,
                ..
            } => self.keys |= key_bit(k),
            Event::KeyUp {
                scancode: Some(k), ..
            } => self.keys &= !key_bit(k),
            Event::GamepadAdded { which, .. } => self.open_gamepad(which),
            Event::GamepadRemoved { which, .. } => {
                self.gamepads.retain(|g| g.id().ok() != Some(which));
                self.pads.remove(&which);
            }
            Event::GamepadButtonDown { which, button, .. } => {
                self.pads.entry(which).or_default().button(button, true)
            }
            Event::GamepadButtonUp { which, button, .. } => {
                self.pads.entry(which).or_default().button(button, false)
            }
            Event::GamepadAxisMotion {
                which, axis, value, ..
            } => self.pads.entry(which).or_default().axis(axis, value),
            _ => {}
        }
    }

    fn held_pad(&self) -> u32 {
        self.pads.values().fold(self.keys, |a, p| a | p.bits())
    }

    fn texture_for(&mut self, w: usize, h: usize) -> &mut Texture {
        if !matches!(self.texture, Some((_, tw, th)) if tw == w && th == h) {
            if let Some((old, ..)) = self.texture.take() {
                // SAFETY: `unsafe_textures` frees nothing on drop; this one is no longer used.
                unsafe { old.destroy() };
            }
            let mut t = self
                .creator
                .create_texture_streaming(PixelFormat::ARGB8888, w as u32, h as u32)
                .expect("create texture");
            t.set_scale_mode(ScaleMode::Nearest);
            self.texture = Some((t, w, h));
        }
        &mut self.texture.as_mut().unwrap().0
    }

    fn present(&mut self) {
        let shown = psyq::gpu::with(|g| g.read_display(&mut self.buf));
        self.canvas.set_draw_color((0, 0, 0));
        self.canvas.clear();
        if let Some((w, h)) = shown {
            let pixels: Vec<u8> = self.buf[..w * h]
                .iter()
                .flat_map(|p| (p | 0xff00_0000).to_le_bytes())
                .collect();
            self.texture_for(w, h)
                .update(None, &pixels, w * 4)
                .expect("texture update");
            let (ow, oh) = self.canvas.output_size().unwrap_or((960, 720));
            let (x, y, dw, dh) = letterbox(w as u32, h as u32, ow, oh);
            let tex = &self.texture.as_ref().unwrap().0;
            let _ = self.canvas.copy(tex, None, FRect::new(x, y, dw, dh));
        }
        self.canvas.present();
        self.frames += 1;
        if self.log
            && (self.frames == 1 || self.last_size != shown || self.frames.is_multiple_of(60))
        {
            match shown {
                Some((w, h)) => eprintln!(
                    "dw1: window: {} frames presented, display {w}x{h}",
                    self.frames
                ),
                None => eprintln!("dw1: window: {} frames presented, display off", self.frames),
            }
        }
        self.last_size = shown;
    }

    /// A save the game thinks it made but that didn't reach the disk shows in the window's
    /// title until a later save succeeds.
    fn show_save_state(&mut self) {
        let failed = psyq::libmcrd::save_failed();
        if failed != self.save_failed_shown {
            self.save_failed_shown = failed;
            let title = if failed {
                "Digimon World (dw1-silicon) - SAVE NOT WRITTEN TO DISK (see the terminal)"
            } else {
                "Digimon World (dw1-silicon)"
            };
            let _ = self.canvas.window_mut().set_title(title);
        }
    }

    fn wait_for_field(&mut self) {
        let now = self.t0.elapsed().as_nanos() as u64;
        let pacer = self.pacer.get_or_insert_with(|| Pacer::new(now));
        let wait = pacer.next(now);
        if wait == 0 {
            return;
        }
        // Sleep most of it, spin the last millisecond: the OS sleep overshoots.
        let end = self.t0 + Duration::from_nanos(now + wait);
        let spin = Duration::from_millis(1);
        if let Some(coarse) = end
            .checked_duration_since(Instant::now())
            .and_then(|d| d.checked_sub(spin))
        {
            std::thread::sleep(coarse);
        }
        while Instant::now() < end {
            std::hint::spin_loop();
        }
    }
}

impl psyq::libetc::Host for Window {
    fn vblank(&mut self, _vblank: u32) {
        self.events();
        self.wait_for_field();
        self.present();
        self.show_save_state();
    }

    fn pad(&mut self, _caller: usize) -> u32 {
        let held = self.held_pad();
        self.held_over &= held;
        held & !self.held_over
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_libetc_bits() {
        let all = [
            (Scancode::Return, 0x800), // PADstart, as main.c masks it
            (Scancode::Up, 1 << 12),
            (Scancode::Right, 1 << 13),
            (Scancode::Down, 1 << 14),
            (Scancode::Left, 1 << 15),
            (Scancode::Z, 0x40),
            (Scancode::X, 0x20),
            (Scancode::A, 0x80),
            (Scancode::S, 0x10),
            (Scancode::RShift, 0x100),
            (Scancode::Q, 0x4),
            (Scancode::W, 0x8),
            (Scancode::_1, 0x1),
            (Scancode::_2, 0x2),
        ];
        for (k, bit) in all {
            assert_eq!(key_bit(k), bit, "{k:?}");
        }
        assert_eq!(key_bit(Scancode::P), 0);
        // No two keys share a bit.
        let or = all.iter().fold(0, |m, (_, b)| m | b);
        assert_eq!(or.count_ones() as usize, all.len());
    }

    #[test]
    fn gamepad_maps() {
        assert_eq!(button_bit(Button::South), 0x40);
        assert_eq!(button_bit(Button::Start), 0x800);
        assert_eq!(axis_bits(Axis::LeftX, -20000), LEFT);
        assert_eq!(axis_bits(Axis::LeftY, 20000), DOWN);
        assert_eq!(axis_bits(Axis::LeftX, 100), 0);
        assert_eq!(axis_bits(Axis::TriggerRight, 30000), R2);

        // A centred stick doesn't release a held D-pad direction, nor the other way round.
        let mut p = PadState::default();
        p.button(Button::DPadLeft, true);
        p.axis(Axis::LeftX, 100);
        assert_eq!(p.bits(), LEFT);
        p.axis(Axis::LeftX, -20000);
        p.button(Button::DPadLeft, false);
        assert_eq!(p.bits(), LEFT);
        p.axis(Axis::LeftX, 0);
        assert_eq!(p.bits(), 0);
    }

    #[test]
    fn pacer_holds_the_field_rate_without_drift() {
        let mut p = Pacer::new(0);
        let mut now = 0;
        for _ in 0..5982 {
            now += p.next(now);
        }
        let expect = 5982u128 * 1_000_000_000_000 / FIELD_MILLIHZ as u128;
        assert_eq!(now as u128, expect);
        assert!((100_000_000_000..100_030_000_000).contains(&now));
    }

    #[test]
    fn pacer_resyncs_after_a_long_stall_and_tolerates_small_ones() {
        let mut p = Pacer::new(0);
        let field = p.next(0); // ~16.7 ms
        assert!((16_700_000..16_730_000).contains(&field));
        // 2 fields late: no resync, no wait (the next deadline is still on the old grid).
        assert_eq!(p.next(field + 2 * field), 0);
        // A 20-field stall: resync; the following wait is a whole field again.
        let now = 25 * field;
        assert_eq!(p.next(now), 0);
        let w = p.next(now);
        assert!((16_700_000..16_730_000).contains(&w));
    }

    #[test]
    fn letterbox_scales_by_whole_numbers_and_centres() {
        assert_eq!(letterbox(320, 240, 960, 720), (0.0, 0.0, 960.0, 720.0));
        assert_eq!(letterbox(320, 240, 1000, 720), (20.0, 0.0, 960.0, 720.0));
        assert_eq!(
            letterbox(320, 240, 1920, 1080),
            (320.0, 60.0, 1280.0, 960.0)
        );
        assert_eq!(letterbox(480, 240, 960, 720), (0.0, 120.0, 960.0, 480.0));
        let (_, _, w, h) = letterbox(320, 240, 160, 200);
        assert_eq!((w, h), (160.0, 120.0));
    }
}
