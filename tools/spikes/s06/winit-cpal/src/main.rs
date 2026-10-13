mod gfx;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

static CALLS: AtomicU32 = AtomicU32::new(0);

fn start_audio() -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    println!("audio device: {:?}", device.id());
    if let Ok(c) = device.default_output_config() {
        println!("audio default config: {:?}", c);
    }
    if let Ok(cfgs) = device.supported_output_configs() {
        for c in cfgs { println!("audio supported: {:?}", c); }
    }
    let rate: u32 = std::env::var("S06_RATE").ok().and_then(|v| v.parse().ok()).unwrap_or(44100);
    let config = cpal::StreamConfig { channels: 2, sample_rate: rate, buffer_size: cpal::BufferSize::Default };
    let mut phase = 0f32;
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [i16], _| {
                let n = CALLS.fetch_add(1, Ordering::Relaxed);
                if n < 5 { println!("audio callback #{}: {} frames = {:.2} ms", n + 1, data.len() / 2, (data.len() / 2) as f64 * 1000.0 / rate as f64); }
                for f in data.chunks_mut(2) {
                    let s = (phase * std::f32::consts::TAU).sin() * 0.25 * i16::MAX as f32;
                    phase = (phase + 440.0 / rate as f32).fract();
                    f[0] = s as i16;
                    f[1] = s as i16;
                }
            },
            |e| eprintln!("audio error: {e}"),
            None,
        )
        .map_err(|e| println!("build_output_stream failed: {e}"))
        .ok()?;
    stream.play().ok()?;
    if let Ok(c) = device.default_output_config() {
        println!("device config AFTER opening our {} Hz stream: {} Hz, buffer range {:?}", rate, c.sample_rate(), c.buffer_size());
    }
    println!("audio stream opened: {} Hz stereo s16 (callback/pull model, rate requested directly)", rate);
    Some(stream)
}

struct App {
    t0: Instant,
    // Field order matters: `gfx` holds a surface made from raw handles, so it must drop
    // before `window`.
    gfx: Option<gfx::Gfx>,
    window: Option<Arc<Window>>,
    pacing: gfx::Pacing,
    frames: u32,
    reported: bool,
    audio: Option<cpal::Stream>,
    audio_start: Instant,
    audio_done: bool,
    gilrs: Option<gilrs::Gilrs>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let mut attrs = Window::default_attributes().with_title("s06 winit").with_inner_size(LogicalSize::new(960, 720));
        for m in el.available_monitors() {
            println!("monitor {:?}: {:?} at {:?}, {:?} mHz", m.name(), m.size(), m.position(), m.refresh_rate_millihertz());
            if let Ok(want) = std::env::var("S06_HZ") {
                if m.refresh_rate_millihertz().map(|r| r / 1000).map(|r| r.to_string()) == Some(want) {
                    attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(m.position().x + 100, m.position().y + 100));
                }
            }
        }
        let w = Arc::new(el.create_window(attrs).expect("window"));
        let size = w.inner_size();
        println!("window 960x720 points, {}x{} pixels, scale factor {:.2}", size.width, size.height, w.scale_factor());
        if let Some(m) = w.current_monitor() {
            println!("display refresh rate: {:?} mHz ({:?})", m.refresh_rate_millihertz(), m.size());
        }
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(w.display_handle().unwrap().as_raw()),
            raw_window_handle: w.window_handle().unwrap().as_raw(),
        };
        let g = gfx::Gfx::new(target, size.width, size.height);
        println!("wgpu adapter: {}; format {:?}", g.adapter_info, g.config.format);
        self.gfx = Some(g);
        self.window = Some(w);
        self.audio = start_audio();
        self.audio_start = Instant::now();
        if let Some(gl) = &self.gilrs {
            let n = gl.gamepads().count();
            println!("gamepads found at startup: {}", n);
            for (_, g) in gl.gamepads() { println!("gamepad {:?}: ff supported {}", g.name(), g.is_ff_supported()); }
        }
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::KeyboardInput { event, .. } => println!(
                "key {} {:?} {:?} repeat={}",
                if event.state == ElementState::Pressed { "down" } else { "up" },
                event.logical_key, event.physical_key, event.repeat
            ),
            WindowEvent::Resized(s) => { if let Some(g) = &mut self.gfx { g.resize(s.width, s.height); } }
            WindowEvent::RedrawRequested => {
                if let Some(gl) = &mut self.gilrs {
                    while let Some(e) = gl.next_event() { println!("gamepad event: {:?}", e); }
                }
                let ok = self.gfx.as_mut().map(|g| g.frame(self.t0.elapsed().as_secs_f64())).unwrap_or(false);
                if ok { self.pacing.tick(); } else { self.pacing.skipped += 1; }
                self.frames += 1;
                if self.frames == 20 { self.pacing.ivs.clear(); }
                if self.pacing.ivs.len() >= 300 && !self.reported {
                    println!("{}", self.pacing.report());
                    self.reported = true;
                }
                if self.audio_start.elapsed() > Duration::from_millis(1200) && !self.audio_done && self.audio.is_some() {
                    if let Some(a) = &self.audio { let _ = a.pause(); }
                    self.audio_done = true;
                    println!("tone finished (played ~1 s)");
                }
                if (self.reported && self.audio_done) || self.t0.elapsed() > Duration::from_secs(15) {
                    println!("exit ok after {:.1} s, {} frames", self.t0.elapsed().as_secs_f64(), self.frames);
                    el.exit();
                    return;
                }
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    let gilrs = match gilrs::Gilrs::new() {
        Ok(g) => Some(g),
        Err(e) => { println!("gilrs init failed: {e}"); None }
    };
    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App { t0: Instant::now(), window: None, gfx: None, pacing: gfx::Pacing::new(), frames: 0, reported: false, audio: None, audio_start: Instant::now(), audio_done: false, gilrs };
    el.run_app(&mut app).unwrap();
}
