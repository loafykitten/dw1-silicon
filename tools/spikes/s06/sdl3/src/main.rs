mod gfx;
use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream};
use sdl3::event::Event;
use sdl3::gamepad::Button;
use std::time::{Duration, Instant};

struct Tone { phase: f32, calls: u32 }
impl AudioCallback<i16> for Tone {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let n = (requested as usize) / 2; // `requested` counts i16 samples; 2 per stereo frame
        self.calls += 1;
        if self.calls <= 5 { println!("audio callback #{}: requested {} samples = {} frames = {:.2} ms", self.calls, requested, n, n as f64 / 44.1); }
        let mut buf = Vec::with_capacity(n * 2);
        for _ in 0..n {
            let s = (self.phase * std::f32::consts::TAU).sin() * 0.25 * i16::MAX as f32;
            self.phase = (self.phase + 440.0 / 44100.0).fract();
            buf.push(s as i16);
            buf.push(s as i16);
        }
        let _ = stream.put_data_i16(&buf);
    }
}

fn main() {
    let t0 = Instant::now();
    let sdl = sdl3::init().expect("sdl init");
    println!("SDL version: {}", sdl3::version::version());
    println!("video driver: {}", sdl.video().unwrap().current_video_driver());
    let video = sdl.video().unwrap();
    let mut pos = None;
    for d in video.displays().unwrap_or_default() {
        let (b, m) = (d.get_bounds().unwrap(), d.get_mode().unwrap());
        println!("display {:?}: {}x{} at ({}, {}), {:.2} Hz", d.get_name(), b.w, b.h, b.x, b.y, m.refresh_rate);
        if std::env::var("S06_HZ").ok().and_then(|v| v.parse::<f32>().ok()).is_some_and(|h| (h - m.refresh_rate).abs() < 0.5) {
            pos = Some((b.x + 100, b.y + 100));
        }
    }
    let mut wb = video.window("s06 sdl3", 960, 720);
    if let Some((x, y)) = pos { wb.position(x, y); }
    let window = wb
        .high_pixel_density()
        .resizable()
        .metal_view()
        .build()
        .expect("window");
    let (pw, ph) = window.size_in_pixels();
    println!("window 960x720 points, {}x{} pixels, display scale {:.2}, pixel density {:.2}", pw, ph, window.display_scale(), window.pixel_density());
    if let Ok(d) = window.get_display() {
        if let Ok(m) = d.get_mode() {
            println!("display refresh rate: {:.3} Hz ({}x{})", m.refresh_rate, m.w, m.h);
        }
    }

    use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(window.display_handle().unwrap().as_raw()),
        raw_window_handle: window.window_handle().unwrap().as_raw(),
    };
    let mut gfx = gfx::Gfx::new(target, pw, ph);
    println!("wgpu adapter: {}; format {:?}", gfx.adapter_info, gfx.config.format);

    // gamepads
    let gp = sdl.gamepad().unwrap();
    match gp.mappings() {
        Ok(m) => {
            let mut by: std::collections::BTreeMap<String, usize> = Default::default();
            for x in &m { *by.entry(x.rsplit("platform:").next().unwrap_or("?").trim_end_matches(',').to_string()).or_default() += 1; }
            println!("SDL mapping list for this platform: {} total; by platform tag (last 'platform:' field): {:?}", m.len(), by);
        }
        Err(e) => println!("mapping list error: {e}"),
    }
    let ids = match gp.gamepads() {
        Ok(v) => { println!("gamepads found at startup: {}", v.len()); v }
        Err(e) => { println!("gamepad enumeration ERROR: {e}"); vec![] }
    };
    let mut open = Vec::new();
    for id in ids {
        if let Ok(g) = gp.open(id) {
            let rumble = unsafe {
                let props = sdl3::sys::gamepad::SDL_GetGamepadProperties(g.raw());
                sdl3::sys::properties::SDL_GetBooleanProperty(props, sdl3::sys::gamepad::SDL_PROP_GAMEPAD_CAP_RUMBLE_BOOLEAN, false)
            };
            println!("gamepad: {:?} type {:?} rumble-capable: {}", g.name(), g.r#type(), rumble);
            open.push(g);
        }
    }

    // audio
    let audio = sdl.audio().unwrap();
    let spec = AudioSpec { freq: Some(44100), channels: Some(2), format: Some(AudioFormat::S16LE) };
    let stream = audio
        .open_playback_stream(&spec, Tone { phase: 0.0, calls: 0 })
        .expect("audio stream");
    stream.resume().unwrap();
    println!("audio stream opened: 44100 Hz stereo s16 (callback/pull model)");
    if let Some(id) = stream.device_id() {
        unsafe {
            let mut spec = std::mem::zeroed();
            let mut frames = 0;
            let ok = sdl3::sys::audio::SDL_GetAudioDeviceFormat(id.id(), &mut spec, &mut frames);
            println!("audio DEVICE format (ok={ok}): {} Hz, {} ch, format {:?}, device buffer {} frames = {:.2} ms", spec.freq, spec.channels, spec.format, frames, frames as f64 * 1000.0 / spec.freq.max(1) as f64);
        }
    }
    let audio_start = Instant::now();

    let mut pump = sdl.event_pump().unwrap();
    let mut pacing = gfx::Pacing::new();
    let mut frames = 0u32;
    let mut reported = false;
    let mut audio_closed = false;
    'main: loop {
        for ev in pump.poll_iter() {
            match ev {
                Event::Quit { .. } => break 'main,
                Event::KeyDown { keycode, scancode, repeat, .. } => println!("key down {:?} {:?} repeat={}", keycode, scancode, repeat),
                Event::KeyUp { keycode, .. } => println!("key up {:?}", keycode),
                Event::GamepadAdded { which, .. } => {
                    println!("gamepad added id {}", which);
                    if let Ok(g) = gp.open(which) { println!("  opened {:?}", g.name()); open.push(g); }
                }
                Event::GamepadRemoved { which, .. } => println!("gamepad removed id {}", which),
                Event::GamepadButtonDown { button, .. } => println!("pad button down {:?}", button),
                Event::GamepadButtonUp { button, .. } => println!("pad button up {:?}", button),
                Event::Window { win_event: sdl3::event::WindowEvent::PixelSizeChanged(w, h), .. } => gfx.resize(w as u32, h as u32),
                _ => {}
            }
        }
        if gfx.frame(t0.elapsed().as_secs_f64()) { pacing.tick(); } else { pacing.skipped += 1; }
        frames += 1;
        if frames == 20 { pacing.ivs.clear(); } // drop warm-up
        if pacing.ivs.len() >= 300 && !reported {
            println!("{}", pacing.report());
            reported = true;
        }
        if audio_start.elapsed() > Duration::from_millis(1200) && !audio_closed {
            drop(stream_pause(&stream));
            audio_closed = true;
            println!("tone finished (played ~1 s)");
        }
        if reported && audio_closed { break; }
        if t0.elapsed() > Duration::from_secs(15) { println!("timeout"); break; }
    }
    let _ = Button::South;
    println!("exit ok after {:.1} s, {} frames", t0.elapsed().as_secs_f64(), frames);
}

fn stream_pause<C: AudioCallback<i16>>(s: &sdl3::audio::AudioStreamWithCallback<C>) { let _ = s.pause(); }
