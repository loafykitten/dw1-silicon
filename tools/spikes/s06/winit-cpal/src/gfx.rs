// Shared by both spikes (copied verbatim): wgpu setup, clear + one triangle.
use wgpu::*;

const SHADER: &str = r#"
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(0.0, 0.6), vec2(-0.6, -0.6), vec2(0.6, -0.6));
    return vec4(p[i], 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4(1.0, 0.6, 0.1, 1.0); }
"#;

pub struct Gfx {
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pipeline: RenderPipeline,
    pub adapter_info: String,
}

impl Gfx {
    pub fn new(target: SurfaceTargetUnsafe, w: u32, h: u32) -> Gfx {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
        let surface = unsafe { instance.create_surface_unsafe(target) }.expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("adapter");
        let info = adapter.get_info();
        let adapter_info = format!("{} ({:?})", info.name, info.backend);
        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).expect("device");
        let mut config = surface.get_default_config(&adapter, w, h).expect("config");
        config.present_mode = PresentMode::Fifo; // vsync
        surface.configure(&device, &config);
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(config.format.into())],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Gfx { surface, device, queue, config, pipeline, adapter_info }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Returns true if a frame was acquired, drawn and presented.
    pub fn frame(&mut self, t: f64) -> bool {
        let tex = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(t) | CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return false,
        };
        let view = tex.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let g = 0.2 + 0.1 * (t.sin() * 0.5 + 0.5);
            let mut pass = enc.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations { load: LoadOp::Clear(Color { r: 0.05, g, b: 0.3, a: 1.0 }), store: StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
        self.queue.present(tex);
        true
    }
}

/// Present-interval statistics in ms.
pub struct Pacing { last: Option<std::time::Instant>, pub ivs: Vec<f64>, pub skipped: u32 }
impl Pacing {
    pub fn new() -> Self { Pacing { last: None, ivs: vec![], skipped: 0 } }
    pub fn tick(&mut self) {
        let now = std::time::Instant::now();
        if let Some(l) = self.last { self.ivs.push((now - l).as_secs_f64() * 1000.0); }
        self.last = Some(now);
    }
    pub fn report(&self) -> String {
        let n = self.ivs.len().max(1) as f64;
        let mean = self.ivs.iter().sum::<f64>() / n;
        let sd = (self.ivs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
        let mx = self.ivs.iter().cloned().fold(0.0, f64::max);
        let mn = self.ivs.iter().cloned().fold(f64::MAX, f64::min);
        format!("CPU-side present-return interval over {} successful presents ({} skipped acquisitions): mean {:.3} ms ({:.2} Hz), stddev {:.3} ms, min {:.3}, max {:.3}", self.ivs.len(), self.skipped, mean, 1000.0 / mean, sd, mn, mx)
    }
}
