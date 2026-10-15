//! The window's picture on wgpu (Metal): the game's display (0x00RRGGBB pixels, from
//! `backend::gpu`) uploaded as a texture each field and drawn into the window.
//!
//! Colours pass through unchanged: the picture texture and the window are both 8-bit unorm,
//! not sRGB, so a PS1 colour reaches the screen as the emulator shows it. The picture is drawn
//! with a "sharp bilinear" filter: each source pixel is first enlarged by the largest whole
//! number that fits, then the result is filtered to the final size. At a whole-number scale
//! that is exactly nearest-neighbour (the 1x picture pixel for pixel, `tests`); at any other
//! size the pixels stay square-edged with only a one-pixel blend between them.
//!
//! Placement (`Fit`): the game's picture is 4:3 (320x240 on a 4:3 TV), shown either at the
//! largest whole multiple of 320x240 that fits (`Integer`, the original look; black borders) or
//! as large as the window allows at 4:3 (`Fill`, an addition). `Exact` puts a picture the size
//! of the window on it one to one (the splash).

use wgpu::*;

/// How the picture is placed in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// The largest whole multiple of the base size (320x240) that fits, centred.
    Integer,
    /// As large as fits at 4:3, centred.
    Fill,
    /// One picture pixel per window pixel, at the top left (the picture is the window's size).
    Exact,
}

/// The base picture the game draws at 1x [doc: `GsInitGraph(320, 240, ...)`, main.c:3577].
pub const BASE: (u32, u32) = (320, 240);

/// Where a `src_w x src_h` picture goes in a `win_w x win_h` window: `(x, y, w, h)` in window
/// pixels. `Integer` and `Fill` keep 4:3 whatever the picture's size (a 640-wide picture is
/// shown at the same size as a 320-wide one, as on a TV); when the window is smaller than the
/// base, `Integer` shrinks to fit rather than crop.
pub fn place(fit: Fit, src_w: u32, src_h: u32, win_w: u32, win_h: u32) -> (u32, u32, u32, u32) {
    let (ww, wh) = (win_w as u64, win_h as u64);
    let (w, h) = match fit {
        Fit::Exact => return (0, 0, src_w.min(win_w), src_h.min(win_h)),
        Fit::Integer if ww >= BASE.0 as u64 && wh >= BASE.1 as u64 => {
            let k = (ww / BASE.0 as u64).min(wh / BASE.1 as u64);
            (BASE.0 as u64 * k, BASE.1 as u64 * k)
        }
        // 4:3, as large as fits.
        _ if ww * 3 <= wh * 4 => (ww, ww * 3 / 4),
        _ => (wh * 4 / 3, wh),
    };
    let _ = (src_w, src_h);
    (
        ((ww - w) / 2) as u32,
        ((wh - h) / 2) as u32,
        w as u32,
        h as u32,
    )
}

const SHADER: &str = r#"
struct Place {
    // Destination rect in target pixels: x, y, w, h.
    dst: vec4<f32>,
    // Target size in pixels.
    size: vec2<f32>,
    // Picture size in texels.
    src: vec2<f32>,
};
@group(0) @binding(0) var<uniform> place: Place;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var pic_sampler: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    // Two triangles covering the destination rect.
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0));
    let c = corners[i];
    let p = place.dst.xy + c * place.dst.zw;
    let ndc = vec2(p.x / place.size.x * 2.0 - 1.0, 1.0 - p.y / place.size.y * 2.0);
    var o: Out;
    o.pos = vec4(ndc, 0.0, 1.0);
    o.uv = c;
    return o;
}

@fragment fn fs(o: Out) -> @location(0) vec4<f32> {
    // Sharp bilinear: within each texel, the middle (scale - 1) / scale of it is flat and only
    // the rim blends into the neighbour; at a whole-number scale every sample lands flat.
    let scale = max(floor(place.dst.zw / place.src), vec2(1.0));
    let texel = o.uv * place.src;
    let base = floor(texel);
    let d = fract(texel) - 0.5;
    let keep = 0.5 - 0.5 / scale;
    let f = (d - clamp(d, -keep, keep)) * scale + 0.5;
    let c = textureSampleLevel(picture, pic_sampler, (base + f) / place.src, 0.0);
    return vec4(c.rgb, 1.0);
}
"#;

/// The picture format: 0x00RRGGBB little-endian is B, G, R, X in memory.
const PICTURE_FORMAT: TextureFormat = TextureFormat::Bgra8Unorm;

/// What draws pictures, with or without a window: the device, the pipeline for one target
/// format, and the picture texture (remade when the picture's size changes).
struct Painter {
    device: Device,
    queue: Queue,
    pipeline: RenderPipeline,
    layout: BindGroupLayout,
    sampler: Sampler,
    uniform: Buffer,
    picture: Option<(Texture, BindGroup, u32, u32)>,
}

impl Painter {
    fn new(device: Device, queue: Queue, format: TextureFormat) -> Painter {
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("picture"),
            source: ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("picture"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("picture"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("picture"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("picture"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&BufferDescriptor {
            label: Some("place"),
            size: 32,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Painter {
            device,
            queue,
            pipeline,
            layout,
            sampler,
            uniform,
            picture: None,
        }
    }

    /// Uploads a `w x h` picture (0x00RRGGBB per pixel, row after row).
    fn upload(&mut self, pixels: &[u32], w: u32, h: u32) {
        if w == 0 || h == 0 || pixels.len() < (w as usize * h as usize) {
            return;
        }
        if !matches!(self.picture, Some((_, _, pw, ph)) if pw == w && ph == h) {
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("picture"),
                size: Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: PICTURE_FORMAT,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bind = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("picture"),
                layout: &self.layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.uniform.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.picture = Some((texture, bind, w, h));
        }
        let (texture, ..) = self.picture.as_ref().unwrap();
        let bytes: &[u8] = bytemuck_u32(&pixels[..(w * h) as usize]);
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytes,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Clears `view` (a `tw x th` target) to black and draws the uploaded picture placed by
    /// `fit` (nothing but black when there's no picture).
    fn draw(&self, view: &TextureView, tw: u32, th: u32, fit: Fit, show: bool) -> CommandBuffer {
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&RenderPassDescriptor {
                label: Some("picture"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let (true, Some((_, bind, pw, ph))) = (show, &self.picture) {
                let (x, y, w, h) = place(fit, *pw, *ph, tw, th);
                if w > 0 && h > 0 {
                    let u: [f32; 8] = [
                        x as f32, y as f32, w as f32, h as f32, tw as f32, th as f32, *pw as f32,
                        *ph as f32,
                    ];
                    self.queue.write_buffer(&self.uniform, 0, bytemuck_f32(&u));
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, bind, &[]);
                    pass.draw(0..6, 0..1);
                }
            }
        }
        enc.finish()
    }
}

fn bytemuck_u32(v: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding and any byte is a valid u8; the length is in bytes.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), std::mem::size_of_val(v)) }
}

fn bytemuck_f32(v: &[f32]) -> &[u8] {
    // SAFETY: as above.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), std::mem::size_of_val(v)) }
}

/// The window's renderer.
pub struct Renderer {
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    /// The largest drawable or picture side the GPU takes.
    max_size: u32,
    painter: Painter,
    /// Frames that couldn't get a window texture (occluded, timed out).
    pub skipped: u64,
}

/// A picture to show: `w x h` pixels of 0x00RRGGBB.
pub struct Picture<'a> {
    pub pixels: &'a [u32],
    pub w: u32,
    pub h: u32,
}

impl Renderer {
    /// A renderer on the window behind `target`, whose drawable is `w x h` pixels.
    ///
    /// # Safety
    /// The window behind `target` must outlive the renderer.
    pub unsafe fn new(target: SurfaceTargetUnsafe, w: u32, h: u32) -> Result<Renderer, String> {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
        // SAFETY: the caller keeps the window alive.
        let surface =
            unsafe { instance.create_surface_unsafe(target) }.map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        // The adapter's own limits: a window across large displays can be wider than the
        // default 8192 pixels.
        let limits = adapter.limits();
        let max_size = limits.max_texture_dimension_2d;
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("dw1"),
            required_limits: limits,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        // Not sRGB: colours go to the screen as they are (see the module comment).
        let format = [TextureFormat::Bgra8Unorm, TextureFormat::Rgba8Unorm]
            .into_iter()
            .find(|f| caps.formats.contains(f))
            .ok_or_else(|| format!("no 8-bit unorm window format in {:?}", caps.formats))?;
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: fit_within(w, h, max_size).0.max(1),
            height: fit_within(w, h, max_size).1.max(1),
            present_mode: present_mode(&caps.present_modes),
            desired_maximum_frame_latency: 1,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
            // sRGB: the 8-bit values written are the colours shown.
            color_space: SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        Ok(Renderer {
            surface,
            config,
            max_size,
            painter: Painter::new(device, queue, format),
            skipped: 0,
        })
    }

    /// A renderer on `window`, whose drawable is `w x h` pixels.
    ///
    /// # Safety
    /// `window` must outlive the renderer.
    pub unsafe fn for_window(
        window: &(impl rwh::HasWindowHandle + rwh::HasDisplayHandle),
        w: u32,
        h: u32,
    ) -> Result<Renderer, String> {
        let target = SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(window.display_handle().map_err(|e| e.to_string())?.as_raw()),
            raw_window_handle: window.window_handle().map_err(|e| e.to_string())?.as_raw(),
        };
        // SAFETY: the caller keeps the window alive.
        unsafe { Renderer::new(target, w, h) }
    }

    /// The window's present mode, for the log.
    pub fn present_mode(&self) -> PresentMode {
        self.config.present_mode
    }

    /// The size a `w x h` drawable is drawn at: at most the GPU's largest texture a side (the
    /// window system stretches a smaller drawable over the window).
    pub fn drawable_size(&self, w: u32, h: u32) -> (u32, u32) {
        fit_within(w, h, self.max_size)
    }

    /// The drawable is now `w x h` pixels (0 while the window has no area: kept as it was).
    pub fn resize(&mut self, w: u32, h: u32) {
        let (w, h) = self.drawable_size(w, h);
        if w > 0 && h > 0 && (w, h) != (self.config.width, self.config.height) {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.painter.device, &self.config);
        }
    }

    /// Shows `picture` placed by `fit` on black (black only if `None`). False if the window had
    /// no texture to draw into this time (hidden, minimised); the frame is then dropped.
    pub fn present(&mut self, picture: Option<Picture>, fit: Fit) -> bool {
        let picture = picture
            .filter(|p| (1..=self.max_size).contains(&p.w) && (1..=self.max_size).contains(&p.h));
        let texture = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(t) | CurrentSurfaceTexture::Suboptimal(t) => t,
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.painter.device, &self.config);
                self.skipped += 1;
                return false;
            }
            other => {
                if self.skipped == 0
                    || std::env::var_os("DW1_WINDOW_LOG").is_some()
                        && self.skipped.is_power_of_two()
                {
                    eprintln!("render: no window texture this frame: {other:?}");
                }
                self.skipped += 1;
                return false;
            }
        };
        if let Some(p) = &picture {
            self.painter.upload(p.pixels, p.w, p.h);
        }
        let view = texture.texture.create_view(&Default::default());
        let cmd = self.painter.draw(
            &view,
            self.config.width,
            self.config.height,
            fit,
            picture.is_some(),
        );
        self.painter.queue.submit([cmd]);
        self.painter.queue.present(texture);
        true
    }
}

/// `w x h` scaled down, keeping its shape, so neither side is over `max`.
fn fit_within(w: u32, h: u32, max: u32) -> (u32, u32) {
    let big = w.max(h);
    if big <= max {
        return (w, h);
    }
    let k = |v: u32| (v as u64 * max as u64 / big as u64) as u32;
    (k(w), k(h))
}

/// Fifo (wait for the display's refresh, no tearing) unless `DW1_PRESENT=immediate`. The game
/// paces itself to the PS1's field rate (59.817 Hz); on a 60 Hz or faster display Fifo never
/// holds a frame back by more than one refresh.
fn present_mode(available: &[PresentMode]) -> PresentMode {
    let want = match std::env::var("DW1_PRESENT").as_deref() {
        Ok("immediate") => PresentMode::Immediate,
        Ok("mailbox") => PresentMode::Mailbox,
        _ => PresentMode::Fifo,
    };
    if available.contains(&want) {
        want
    } else {
        PresentMode::Fifo
    }
}

/// Draws a picture offscreen and reads it back (tests and screenshots of the window's view).
pub struct Offscreen {
    painter: Painter,
}

impl Offscreen {
    /// None when there's no GPU adapter (the tests then skip).
    pub fn new() -> Option<Offscreen> {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&RequestAdapterOptions::default())).ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).ok()?;
        Some(Offscreen {
            painter: Painter::new(device, queue, TextureFormat::Bgra8Unorm),
        })
    }

    /// `picture` placed by `fit` in a `tw x th` target, read back as 0x00RRGGBB.
    pub fn render(&mut self, picture: Picture, fit: Fit, tw: u32, th: u32) -> Vec<u32> {
        let p = &mut self.painter;
        p.upload(picture.pixels, picture.w, picture.h);
        let target = p.device.create_texture(&TextureDescriptor {
            label: Some("offscreen"),
            size: Extent3d {
                width: tw,
                height: th,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let draw = p.draw(&view, tw, th, fit, true);
        // Rows padded to 256 bytes for the copy.
        let row = (tw * 4).div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = p.device.create_buffer(&BufferDescriptor {
            label: Some("readback"),
            size: (row * th) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = p.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(th),
                },
            },
            Extent3d {
                width: tw,
                height: th,
                depth_or_array_layers: 1,
            },
        );
        p.queue.submit([draw, enc.finish()]);
        buffer.map_async(MapMode::Read, .., |r| r.expect("map readback"));
        p.device
            .poll(PollType::wait_indefinitely())
            .expect("GPU wait");
        let data = buffer.get_mapped_range(..).expect("readback range");
        let mut out = Vec::with_capacity((tw * th) as usize);
        for y in 0..th as usize {
            let r = &data[y * row as usize..][..tw as usize * 4];
            out.extend(
                r.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|&[b, g, r, _]| u32::from_le_bytes([b, g, r, 0])),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_drawables_keep_their_shape() {
        assert_eq!(fit_within(1920, 1080, 8192), (1920, 1080));
        assert_eq!(fit_within(20_000, 8_000, 16_384), (16_384, 6_553));
        assert_eq!(fit_within(8_000, 20_000, 16_384), (6_553, 16_384));
    }

    #[test]
    fn placement() {
        // Integer: the largest whole multiple of 320x240, centred.
        assert_eq!(place(Fit::Integer, 320, 240, 960, 720), (0, 0, 960, 720));
        assert_eq!(place(Fit::Integer, 320, 240, 1000, 720), (20, 0, 960, 720));
        assert_eq!(
            place(Fit::Integer, 320, 240, 1920, 1080),
            (320, 60, 1280, 960)
        );
        // An HD picture is placed like the 1x one.
        assert_eq!(
            place(Fit::Integer, 1280, 960, 1920, 1080),
            (320, 60, 1280, 960)
        );
        // Smaller than the base: shrink to fit at 4:3.
        assert_eq!(place(Fit::Integer, 320, 240, 160, 200), (0, 40, 160, 120));
        // Fill: as large as fits at 4:3.
        assert_eq!(place(Fit::Fill, 320, 240, 1920, 1080), (240, 0, 1440, 1080));
        assert_eq!(place(Fit::Fill, 320, 240, 1000, 2000), (0, 625, 1000, 750));
        assert_eq!(place(Fit::Exact, 800, 600, 800, 600), (0, 0, 800, 600));
    }

    /// A picture with every colour channel value and sharp edges everywhere.
    fn test_picture(w: u32, h: u32) -> Vec<u32> {
        (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let r = (x * 7 + y) & 0xff;
                let g = (y * 13 + x * 3) & 0xff;
                let b = (x ^ y) & 0xff;
                (r << 16) | (g << 8) | b
            })
            .collect()
    }

    #[test]
    fn whole_number_scales_are_the_picture_pixel_for_pixel() {
        let Some(mut off) = Offscreen::new() else {
            eprintln!("no GPU adapter: skipped");
            return;
        };
        let (w, h) = BASE;
        let pic = test_picture(w, h);
        for k in 1..=4 {
            let (tw, th) = (w * k, h * k);
            let out = off.render(Picture { pixels: &pic, w, h }, Fit::Integer, tw, th);
            for y in 0..th {
                for x in 0..tw {
                    let want = pic[((y / k) * w + x / k) as usize];
                    let got = out[(y * tw + x) as usize];
                    assert_eq!(got, want, "scale {k}: pixel ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn integer_fit_centres_on_black() {
        let Some(mut off) = Offscreen::new() else {
            return;
        };
        let pic = vec![0x00ff_8040; 320 * 240];
        let out = off.render(
            Picture {
                pixels: &pic,
                w: 320,
                h: 240,
            },
            Fit::Integer,
            700,
            500,
        );
        // 640x480 at (30, 10).
        assert_eq!(out[0], 0);
        assert_eq!(out[10 * 700 + 29], 0);
        assert_eq!(out[10 * 700 + 30], 0x00ff_8040);
        assert_eq!(out[489 * 700 + 669], 0x00ff_8040);
        assert_eq!(out[490 * 700 + 669], 0);
    }

    #[test]
    fn fill_keeps_flat_colour_inside_pixels() {
        let Some(mut off) = Offscreen::new() else {
            return;
        };
        // A 2.5x enlargement: pixel centres stay the picture's colour exactly.
        let pic = test_picture(320, 240);
        let out = off.render(
            Picture {
                pixels: &pic,
                w: 320,
                h: 240,
            },
            Fit::Fill,
            800,
            600,
        );
        for (sx, sy) in [(0u32, 0u32), (10, 7), (319, 239), (160, 120)] {
            // Output pixel nearest the source pixel's centre.
            let (ox, oy) = (
                ((sx as f32 + 0.5) * 2.5) as u32,
                ((sy as f32 + 0.5) * 2.5) as u32,
            );
            assert_eq!(
                out[(oy * 800 + ox) as usize],
                pic[(sy * 320 + sx) as usize],
                "({sx}, {sy})"
            );
        }
    }
}
