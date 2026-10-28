// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU canvas (WP-S2.02 §7.1): a prepared scene drawn instanced into the canvas's own multisampled colour and
//! depth attachments, resolved into a texture of the target's physical size.
//!
//! **Paint order.** Opaque meshes and opaque images are drawn first, batched by mesh (and texture) in any order,
//! with the depth test and depth writes (later in paint order is nearer); translucent meshes, LOD bars, translucent
//! glyphs and images with alpha follow in paint order with the depth test and without depth writes.
//!
//! **Raster images** (FT-480). A second pair of pipelines over the same vertex and instance layout, view uniform,
//! MSAA sample count and depth format draws an image instance as the unit square with its texture (`fs_image` in
//! the one WGSL file): bind group 1 holds the texture and the sampler. Textures are premultiplied `Rgba8Unorm`
//! (gamma-space bytes, as the CPU path's u8 pipeline interpolates them) and blended as premultiplied colour. The
//! sampler is bilinear with clamp-to-edge and no mipmaps: the CPU path's `ImageSampler::default()` (peniko's
//! `ImageQuality::Medium`, `Extend::Pad`), which vello_cpu narrows to nearest only for a 1:1 integer translation,
//! where bilinear sampling at texel centres gives the same texels. The textures are cached by SHA-256 within
//! [`GPU_TEXTURE_BYTES`] (least recently used first, never those of the current scene), one bind group each; a scene
//! whose texture was evicted uploads it again from its prepared image. An image wider or taller than the device's
//! `max_texture_dimension_2d` (the worker downscales to it, [`super::prepare::fit_side`]) is refused and counted,
//! never a fault.
//!
//! **Target.** 4× MSAA where the adapter supports it for both formats (else 1), resolved into an `Rgba8Unorm`
//! texture. That is the format egui-wgpu's native textures must have; its shader samples it as gamma-space colour,
//! and the canvas writes the authored sRGB bytes unchanged, so a filled rectangle reads back its own bytes (B2) and
//! blending happens in gamma space as in the CPU renderer's u8 pipeline. The resolve is cleared to the paper's
//! white: the host shows it over the sheet's rectangle and paints the surround and the overlays itself.
//!
//! **Frames.** [`GpuCanvas::render`] encodes and submits only when the scene, the view, the target size or the LOD
//! cut changed since the last submission; an idle frame submits nothing (doc 16 §1 rule 3).
//!
//! **Buffers.** Uploads use `Queue::write_buffer` and `Queue::write_texture` only. The one mapped buffer is the readback of
//! [`GpuCanvas::read_back`] (tests and the bench): its mapped view lives inside the block that awaited the map,
//! is copied out there and dropped before `unmap` (the mapped-view rule of §7.1; wgpu 30.0.1 releases a buffer on
//! `destroy` or device loss without checking for live views, upstream gfx-rs/wgpu#10256).

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, PoisonError};

use ew_render_cpu::RgbaImage;

use super::prepare::{ImageTexture, Instance, MeshKey, PreparedScene};
use super::switch::GpuError;
use super::tess::VERTEX_BYTES;
use crate::view::DrawTransform;

/// The colour format of the canvas's target (see the module documentation).
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The depth format.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The bytes of one instance record.
const INSTANCE_BYTES: usize = 48;

/// The bytes of the view uniform.
const UNIFORM_BYTES: u64 = 32;

/// The GPU mesh cache's budget beyond the meshes of the current scene.
pub const GPU_MESH_BYTES: u64 = 128 * 1024 * 1024;

/// The GPU texture cache's budget beyond the textures of the current scene (raster images, 4 bytes per texel).
pub const GPU_TEXTURE_BYTES: u64 = 256 * 1024 * 1024;

/// An image's texture key: the SHA-256 of its encoded bytes.
type TextureKey = [u8; 32];

/// What the canvas shows: the target size and the sheet-to-pixel transform.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GpuView {
    /// The target in physical pixels.
    pub target_px: (u32, u32),
    /// Where the sheet's origin lands, in physical pixels of the target.
    pub origin_px: (f64, f64),
    /// Physical pixels per sheet nanometre.
    pub px_per_nm: f64,
}

impl GpuView {
    /// The view of `t` on a target of `target_px` pixels.
    pub fn of(t: &DrawTransform, target_px: (u32, u32)) -> GpuView {
        GpuView {
            target_px,
            origin_px: t.origin,
            px_per_nm: t.px_per_nm(),
        }
    }
}

/// What one call of [`GpuCanvas::render`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RenderOutcome {
    /// A render was encoded and submitted.
    pub submitted: bool,
    /// The target was (re)created at a new size (the host registers the new texture).
    pub resized: bool,
    /// Glyph runs drawn as bars.
    pub bars: usize,
    /// Glyph runs drawn as glyphs.
    pub glyph_runs: usize,
}

/// Exact counts of the canvas's work.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GpuCounters {
    /// Renders submitted.
    pub submits: u64,
    /// Meshes uploaded.
    pub meshes_uploaded: u64,
    /// Instance buffers written.
    pub instance_writes: u64,
    /// Scenes set.
    pub scenes: u64,
    /// Image textures uploaded.
    pub textures_uploaded: u64,
    /// Image textures refused (a side beyond the device's `max_texture_dimension_2d`, or texels that do not fill
    /// the stated size); their images are not drawn.
    pub textures_refused: u64,
}

struct Targets {
    size: (u32, u32),
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    bytes: u64,
    used: u64,
}

/// One cached image texture with its bind group.
struct GpuTexture {
    bind_group: wgpu::BindGroup,
    bytes: u64,
    used: u64,
}

/// One instanced draw.
#[derive(Clone, PartialEq, Debug)]
struct Draw {
    mesh: u32,
    instances: Range<u32>,
    translucent: bool,
    /// The image of an image draw, as an index into the scene's images.
    image: Option<u32>,
}

struct SceneState {
    prepared: Arc<PreparedScene>,
    keys: Vec<MeshKey>,
    /// The texture key of each of the scene's images.
    texture_keys: Vec<TextureKey>,
    instances: Option<wgpu::Buffer>,
    capacity: u64,
    draws: Vec<Draw>,
    cut: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct RenderKey {
    scene: u64,
    size: (u32, u32),
    origin: (u64, u64),
    scale: u64,
    cut: usize,
}

/// The pipelines of the canvas, the image textures' bind group layout and the sampler.
struct Pipelines {
    opaque: wgpu::RenderPipeline,
    translucent: wgpu::RenderPipeline,
    image_opaque: wgpu::RenderPipeline,
    image_translucent: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Pipelines {
    fn of(&self, translucent: bool, image: bool) -> &wgpu::RenderPipeline {
        match (image, translucent) {
            (false, false) => &self.opaque,
            (false, true) => &self.translucent,
            (true, false) => &self.image_opaque,
            (true, true) => &self.image_translucent,
        }
    }
}

/// The wgpu renderer of the canvas over one device.
pub struct GpuCanvas {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sample_count: u32,
    pipelines: Pipelines,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    targets: Option<Targets>,
    meshes: BTreeMap<MeshKey, GpuMesh>,
    mesh_bytes: u64,
    textures: BTreeMap<TextureKey, GpuTexture>,
    texture_bytes: u64,
    texture_budget: u64,
    generation: u64,
    scene: Option<SceneState>,
    last: Option<RenderKey>,
    fault: Arc<Mutex<Option<GpuError>>>,
    counters: GpuCounters,
}

impl std::fmt::Debug for GpuCanvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuCanvas")
            .field("sample_count", &self.sample_count)
            .field("meshes", &self.meshes.len())
            .field("counters", &self.counters)
            .finish_non_exhaustive()
    }
}

/// 4 when the adapter can multisample both of the canvas's formats four times, else 1.
pub fn sample_count_for(adapter: &wgpu::Adapter) -> u32 {
    let four = |f: wgpu::TextureFormat| {
        adapter
            .get_texture_format_features(f)
            .flags
            .sample_count_supported(4)
    };
    if four(TARGET_FORMAT) && four(DEPTH_FORMAT) {
        4
    } else {
        1
    }
}

fn vertex_layouts() -> [Option<wgpu::VertexBufferLayout<'static>>; 2] {
    const VERTEX: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x2, 1 => Float32x2, 2 => Float32, 3 => Unorm8x4, 4 => Uint32
    ];
    const INSTANCE: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        5 => Float32x4, 6 => Float32x2, 7 => Float32x4, 8 => Unorm8x4, 9 => Uint32
    ];
    [
        Some(wgpu::VertexBufferLayout {
            array_stride: VERTEX_BYTES as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: INSTANCE_BYTES as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &INSTANCE,
        }),
    ]
}

fn error_text(e: Option<wgpu::Error>) -> Option<String> {
    e.map(|e| e.to_string())
}

/// `len` rounded up to the copy alignment (4), at least 4.
fn aligned(len: u64) -> u64 {
    len.max(4).div_ceil(wgpu::COPY_BUFFER_ALIGNMENT) * wgpu::COPY_BUFFER_ALIGNMENT
}

fn push_instance(out: &mut Vec<u8>, i: &Instance) {
    for v in i.transform.iter().take(4) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &i.transform[4..6] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &i.clip {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&i.tint);
    out.extend_from_slice(&i.depth.to_le_bytes());
}

impl GpuCanvas {
    /// The canvas on `device`, multisampled `sample_count` times (4 or 1, see [`sample_count_for`]). The
    /// pipelines are made inside validation and memory error scopes; a lost device or an error that no scope
    /// catches later is recorded for [`GpuCanvas::fault`] instead of wgpu's default panic.
    ///
    /// # Errors
    ///
    /// [`GpuError::Init`] with the text of the first error a scope caught.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, sample_count: u32) -> Result<GpuCanvas, GpuError> {
        let sample_count = if sample_count == 4 { 4 } else { 1 };
        let fault = Arc::new(Mutex::new(None));
        {
            let slot = Arc::clone(&fault);
            // The handlers are set on the window's shared device and outlive the canvas, so every error is logged as
            // well as recorded: egui-wgpu's own errors are never swallowed silently.
            device.set_device_lost_callback(move |reason, message| {
                tracing::error!(target: "ew_render2d::gpu", "graphics device lost ({reason:?}): {message}");
                let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
                slot.get_or_insert(GpuError::DeviceLost(format!("{reason:?}: {message}")));
            });
            let slot = Arc::clone(&fault);
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                let message = e.to_string();
                tracing::error!(target: "ew_render2d::gpu", "uncaptured graphics error: {message}");
                let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
                slot.get_or_insert(GpuError::Uncaptured(message));
            }));
        }
        let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let built = Self::build(&device, sample_count);
        let validation = error_text(pollster::block_on(validation.pop()));
        let memory = error_text(pollster::block_on(memory.pop()));
        if let Some(e) = validation.or(memory) {
            return Err(GpuError::Init(e));
        }
        let (pipelines, uniform, bind_group) = built;
        Ok(GpuCanvas {
            device,
            queue,
            sample_count,
            pipelines,
            uniform,
            bind_group,
            targets: None,
            meshes: BTreeMap::new(),
            mesh_bytes: 0,
            textures: BTreeMap::new(),
            texture_bytes: 0,
            texture_budget: GPU_TEXTURE_BYTES,
            generation: 0,
            scene: None,
            last: None,
            fault,
            counters: GpuCounters::default(),
        })
    }

    fn build(device: &wgpu::Device, sample_count: u32) -> (Pipelines, wgpu::Buffer, wgpu::BindGroup) {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ew-canvas"),
            source: wgpu::ShaderSource::Wgsl(include_str!("canvas.wgsl").into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ew-canvas-view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ew-canvas"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        // Group 1 of the image pipelines: the image's texture and the sampler.
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ew-canvas-image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let image_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ew-canvas-image"),
            bind_group_layouts: &[Some(&bind_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        // The CPU path's sampler (module documentation): bilinear, clamp to edge, no mipmaps.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ew-canvas-image"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..wgpu::SamplerDescriptor::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ew-canvas-view"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ew-canvas-view"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let buffers = vertex_layouts();
        // Straight-alpha colour for the meshes; premultiplied texels for the images.
        let straight = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = |translucent: bool, image: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(match (image, translucent) {
                    (false, false) => "ew-canvas-opaque",
                    (false, true) => "ew-canvas-translucent",
                    (true, false) => "ew-canvas-image-opaque",
                    (true, true) => "ew-canvas-image-translucent",
                }),
                layout: Some(if image { &image_layout } else { &layout }),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // Mirrored instances flip the winding: no culling.
                    cull_mode: None,
                    ..wgpu::PrimitiveState::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!translucent),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(if image { "fs_image" } else { "fs_main" }),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: TARGET_FORMAT,
                        blend: Some(wgpu::BlendState {
                            color: if image { premultiplied } else { straight },
                            alpha: premultiplied,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipelines = Pipelines {
            opaque: pipeline(false, false),
            translucent: pipeline(true, false),
            image_opaque: pipeline(false, true),
            image_translucent: pipeline(true, true),
            texture_layout,
            sampler,
        };
        (pipelines, uniform, bind_group)
    }

    /// The device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The sample count of the colour and depth attachments.
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// A lost device or an uncaptured error recorded since the canvas was made (WP-S2.02 §7.2 row 8).
    pub fn fault(&self) -> Option<GpuError> {
        self.fault.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The slot [`GpuCanvas::fault`] reads (a host test injects a device loss through it).
    pub fn fault_slot(&self) -> Arc<Mutex<Option<GpuError>>> {
        Arc::clone(&self.fault)
    }

    /// The counters.
    pub fn counters(&self) -> GpuCounters {
        self.counters
    }

    /// The longest image side the device takes (`max_texture_dimension_2d`): the preparation's maximum side
    /// ([`super::prepare::MeshCache::set_max_image_side`], [`super::prepare::Preparer::new`]).
    pub fn max_image_side(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// The number of cached image textures.
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// The bytes of the cached image textures.
    pub fn texture_bytes(&self) -> u64 {
        self.texture_bytes
    }

    /// Sets the texture cache's budget ([`GPU_TEXTURE_BYTES`] by default; tests set a smaller one). It applies from
    /// the next [`GpuCanvas::set_scene`].
    pub fn set_texture_budget(&mut self, bytes: u64) {
        self.texture_budget = bytes;
    }

    /// The scene on show.
    pub fn scene(&self) -> Option<&Arc<PreparedScene>> {
        self.scene.as_ref().map(|s| &s.prepared)
    }

    /// The resolved texture of the last render (the host registers it with egui-wgpu once per size).
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|t| &t.resolve_view)
    }

    /// The target size in physical pixels.
    pub fn target_size(&self) -> Option<(u32, u32)> {
        self.targets.as_ref().map(|t| t.size)
    }

    /// Uploads `image` as a texture with its bind group (`Queue::write_texture`), or refuses it (counted) when a side
    /// is 0 or beyond the device's limit or its texels do not fill its size.
    fn upload_texture(&mut self, image: &ImageTexture, generation: u64) {
        let (w, h) = (image.width, image.height);
        let max = self.max_image_side();
        let fits = w > 0 && h > 0 && w <= max && h <= max;
        if !fits || image.rgba.len() as u64 != u64::from(w) * u64::from(h) * 4 {
            self.counters.textures_refused += 1;
            return;
        }
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ew-canvas-image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ew-canvas-image"),
            layout: &self.pipelines.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.pipelines.sampler),
                },
            ],
        });
        let bytes = image.bytes();
        self.texture_bytes += bytes;
        self.textures.insert(
            image.sha256,
            GpuTexture {
                bind_group,
                bytes,
                used: generation,
            },
        );
        self.counters.textures_uploaded += 1;
    }

    /// Shows `prepared`: uploads the meshes and image textures the GPU does not have yet (`Queue::write_buffer`,
    /// `Queue::write_texture`), and drops cached meshes and textures of older scenes while over [`GPU_MESH_BYTES`]
    /// and the texture budget ([`GPU_TEXTURE_BYTES`] unless [`GpuCanvas::set_texture_budget`] set another),
    /// least recently used first.
    pub fn set_scene(&mut self, prepared: Arc<PreparedScene>) {
        self.generation += 1;
        let generation = self.generation;
        let mut keys = Vec::with_capacity(prepared.meshes.len());
        for (key, mesh) in &prepared.meshes {
            keys.push(*key);
            if let Some(m) = self.meshes.get_mut(key) {
                m.used = generation;
                continue;
            }
            let vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ew-canvas-mesh-vertices"),
                size: aligned(mesh.vertices.len() as u64),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let indices = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ew-canvas-mesh-indices"),
                size: aligned(mesh.indices.len() as u64 * 4),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if !mesh.vertices.is_empty() {
                self.queue.write_buffer(&vertices, 0, &mesh.vertices);
            }
            let index_bytes: Vec<u8> = mesh.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
            if !index_bytes.is_empty() {
                self.queue.write_buffer(&indices, 0, &index_bytes);
            }
            let bytes = mesh.bytes();
            self.mesh_bytes += bytes;
            self.meshes.insert(
                *key,
                GpuMesh {
                    vertices,
                    indices,
                    index_count: mesh.indices.len() as u32,
                    bytes,
                    used: generation,
                },
            );
            self.counters.meshes_uploaded += 1;
        }
        if self.mesh_bytes > GPU_MESH_BYTES {
            let mut old: Vec<(u64, MeshKey)> = self
                .meshes
                .iter()
                .filter(|(_, m)| m.used < generation)
                .map(|(k, m)| (m.used, *k))
                .collect();
            old.sort_unstable();
            for (_, key) in old {
                if self.mesh_bytes <= GPU_MESH_BYTES {
                    break;
                }
                if let Some(m) = self.meshes.remove(&key) {
                    self.mesh_bytes -= m.bytes;
                }
            }
        }
        let mut texture_keys = Vec::with_capacity(prepared.images.len());
        for image in &prepared.images {
            texture_keys.push(image.sha256);
            match self.textures.get_mut(&image.sha256) {
                Some(t) => t.used = generation,
                None => self.upload_texture(image, generation),
            }
        }
        if self.texture_bytes > self.texture_budget {
            let mut old: Vec<(u64, TextureKey)> = self
                .textures
                .iter()
                .filter(|(_, t)| t.used < generation)
                .map(|(k, t)| (t.used, *k))
                .collect();
            old.sort_unstable();
            for (_, key) in old {
                if self.texture_bytes <= self.texture_budget {
                    break;
                }
                if let Some(t) = self.textures.remove(&key) {
                    self.texture_bytes -= t.bytes;
                }
            }
        }
        self.scene = Some(SceneState {
            prepared,
            keys,
            texture_keys,
            instances: None,
            capacity: 0,
            draws: Vec::new(),
            cut: None,
        });
        self.last = None;
        self.counters.scenes += 1;
    }

    /// Drops the scene (and keeps the meshes for the next one).
    pub fn clear_scene(&mut self) {
        self.scene = None;
        self.last = None;
    }

    /// Drops the render targets (the multisampled attachments and the resolve texture): the canvas is not on
    /// screen. The next render makes them again.
    pub fn release_targets(&mut self) {
        self.targets = None;
        self.last = None;
    }

    fn ensure_targets(&mut self, size: (u32, u32)) -> Result<bool, GpuError> {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return Ok(false);
        }
        let max = self.device.limits().max_texture_dimension_2d;
        if size.0 == 0 || size.1 == 0 || size.0 > max || size.1 > max {
            return Err(GpuError::Limits(format!(
                "a {} × {} pixel target (the device allows {max} per side)",
                size.0, size.1
            )));
        }
        let extent = wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };
        let texture = |label, format, samples, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let resolve = texture(
            "ew-canvas-resolve",
            TARGET_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let msaa = (self.sample_count > 1).then(|| {
            texture(
                "ew-canvas-msaa",
                TARGET_FORMAT,
                self.sample_count,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
            .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let depth = texture(
            "ew-canvas-depth",
            DEPTH_FORMAT,
            self.sample_count,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&wgpu::TextureViewDescriptor::default());
        let resolve_view = resolve.create_view(&wgpu::TextureViewDescriptor::default());
        self.targets = Some(Targets {
            size,
            msaa,
            depth,
            resolve,
            resolve_view,
        });
        self.last = None;
        Ok(true)
    }

    /// Builds and writes the instance buffer of the scene for the LOD cut `cut` (the runs below it are bars).
    fn write_instances(&mut self, cut: usize) {
        let Some(scene) = self.scene.as_mut() else {
            return;
        };
        let p = &scene.prepared;
        // Each instance with the image it draws (`None`: a mesh).
        let mut opaque_glyphs: Vec<&Instance> = Vec::new();
        let mut opaque_images: Vec<(&Instance, Option<u32>)> = Vec::new();
        let mut translucent: Vec<(&Instance, Option<u32>)> =
            p.translucent.iter().map(|i| (i, None)).collect();
        for (i, run) in p.runs.iter().enumerate() {
            if i < cut {
                translucent.push((&run.bar, None));
            } else if run.translucent {
                translucent.extend(p.glyphs[run.glyphs.clone()].iter().map(|i| (i, None)));
            } else {
                opaque_glyphs.extend(&p.glyphs[run.glyphs.clone()]);
            }
        }
        for image in &p.image_instances {
            let opaque = p.images.get(image.image as usize).is_some_and(|t| t.opaque);
            let entry = (&image.quad, Some(image.image));
            if opaque {
                opaque_images.push(entry);
            } else {
                translucent.push(entry);
            }
        }
        opaque_glyphs.sort_by_key(|i| i.mesh);
        opaque_images.sort_by_key(|(_, image)| *image);
        translucent.sort_by_key(|(i, _)| i.depth);
        let mut bytes = Vec::with_capacity(
            (p.opaque.len() + opaque_glyphs.len() + opaque_images.len() + translucent.len()) * INSTANCE_BYTES,
        );
        let mut draws: Vec<Draw> = Vec::new();
        let all = p
            .opaque
            .iter()
            .map(|i| (i, None, false))
            .chain(opaque_glyphs.into_iter().map(|i| (i, None, false)))
            .chain(opaque_images.into_iter().map(|(i, image)| (i, image, false)))
            .chain(translucent.into_iter().map(|(i, image)| (i, image, true)));
        for (n, (inst, image, translucent)) in (0u32..).zip(all) {
            push_instance(&mut bytes, inst);
            match draws.last_mut() {
                Some(d)
                    if d.mesh == inst.mesh
                        && d.translucent == translucent
                        && d.image == image
                        && d.instances.end == n =>
                {
                    d.instances.end = n + 1;
                }
                _ => draws.push(Draw {
                    mesh: inst.mesh,
                    instances: n..n + 1,
                    translucent,
                    image,
                }),
            }
        }
        let needed = aligned(bytes.len() as u64);
        if scene.instances.is_none() || scene.capacity < needed {
            let capacity = needed.next_power_of_two();
            scene.instances = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ew-canvas-instances"),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            scene.capacity = capacity;
        }
        if let Some(buffer) = &scene.instances
            && !bytes.is_empty()
        {
            self.queue.write_buffer(buffer, 0, &bytes);
        }
        scene.draws = draws;
        scene.cut = Some(cut);
        self.counters.instance_writes += 1;
    }

    /// Renders the scene at `view` when anything changed since the last submission; an idle call submits nothing.
    ///
    /// # Errors
    ///
    /// [`GpuError::Limits`] when the target exceeds the device's texture size, or the recorded fault of a lost
    /// device or an uncaptured error.
    pub fn render(&mut self, view: &GpuView) -> Result<RenderOutcome, GpuError> {
        if let Some(fault) = self.fault() {
            return Err(fault);
        }
        let resized = self.ensure_targets(view.target_px)?;
        let Some(scene) = &self.scene else {
            return Ok(RenderOutcome {
                resized,
                ..RenderOutcome::default()
            });
        };
        let cut = scene.prepared.bar_count(view.px_per_nm);
        let runs = scene.prepared.runs.len();
        let key = RenderKey {
            scene: scene.prepared.id,
            size: view.target_px,
            origin: (view.origin_px.0.to_bits(), view.origin_px.1.to_bits()),
            scale: view.px_per_nm.to_bits(),
            cut,
        };
        let mut outcome = RenderOutcome {
            submitted: false,
            resized,
            bars: cut,
            glyph_runs: runs - cut,
        };
        if self.last == Some(key) {
            return Ok(outcome);
        }
        if scene.cut != Some(cut) {
            self.write_instances(cut);
        }
        let (Some(scene), Some(targets)) = (&self.scene, &self.targets) else {
            return Ok(outcome);
        };
        let depth_count = scene.prepared.depth_count as f32;
        let mut uniform = Vec::with_capacity(UNIFORM_BYTES as usize);
        for v in [
            view.origin_px.0 as f32,
            view.origin_px.1 as f32,
            (view.px_per_nm * 1000.0) as f32,
            depth_count,
            view.target_px.0 as f32,
            view.target_px.1 as f32,
            0.0,
            0.0,
        ] {
            uniform.extend_from_slice(&v.to_le_bytes());
        }
        self.queue.write_buffer(&self.uniform, 0, &uniform);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ew-canvas"),
            });
        {
            let (view_tex, resolve) = match &targets.msaa {
                Some(msaa) => (msaa, Some(&targets.resolve_view)),
                None => (&targets.resolve_view, None),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ew-canvas"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_tex,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            if let Some(instances) = &scene.instances {
                pass.set_vertex_buffer(1, instances.slice(..));
                // The pipeline set last: (translucent, image).
                let mut current = None;
                for d in &scene.draws {
                    let Some(mesh) = scene.keys.get(d.mesh as usize).and_then(|k| self.meshes.get(k)) else {
                        continue;
                    };
                    if mesh.index_count == 0 {
                        continue;
                    }
                    // An image draw binds its texture; a refused texture is not drawn.
                    let texture = match d.image {
                        None => None,
                        Some(i) => {
                            let cached = scene
                                .texture_keys
                                .get(i as usize)
                                .and_then(|k| self.textures.get(k));
                            let Some(t) = cached else { continue };
                            Some(t)
                        }
                    };
                    let wanted = (d.translucent, texture.is_some());
                    if current != Some(wanted) {
                        pass.set_pipeline(self.pipelines.of(wanted.0, wanted.1));
                        current = Some(wanted);
                    }
                    if let Some(t) = texture {
                        pass.set_bind_group(1, &t.bind_group, &[]);
                    }
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, d.instances.clone());
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        self.last = Some(key);
        self.counters.submits += 1;
        outcome.submitted = true;
        Ok(outcome)
    }

    /// Reads the resolved texture back (tests and the bench). The mapped view lives only inside the block that
    /// awaited the map; it is copied out and dropped there, before `unmap` (§7.1).
    ///
    /// # Errors
    ///
    /// [`GpuError::Uncaptured`] when the map fails or the device cannot be polled; [`GpuError::Limits`] without a
    /// rendered target.
    pub fn read_back(&self) -> Result<RgbaImage, GpuError> {
        let Some(targets) = &self.targets else {
            return Err(GpuError::Limits("nothing has been rendered".to_owned()));
        };
        let (w, h) = targets.size;
        let row = u64::from(w) * 4;
        let padded = row.div_ceil(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
            * u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ew-canvas-readback"),
            size: padded * u64::from(h),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ew-canvas-readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.resolve,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| GpuError::Uncaptured(e.to_string()))?;
        match rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(GpuError::Uncaptured(e.to_string())),
            Err(e) => return Err(GpuError::Uncaptured(e.to_string())),
        }
        let pixels = {
            // The one mapped view of the canvas: used and dropped inside this block.
            let mapped = slice
                .get_mapped_range()
                .map_err(|e| GpuError::Uncaptured(e.to_string()))?;
            let mut out = Vec::with_capacity((row * u64::from(h)) as usize);
            for y in 0..u64::from(h) {
                let at = (y * padded) as usize;
                out.extend_from_slice(&mapped[at..at + row as usize]);
            }
            out
        };
        buffer.unmap();
        Ok(RgbaImage {
            width: w,
            height: h,
            pixels,
        })
    }

    /// Waits until the device has finished the submitted work (the bench's per-frame wait).
    ///
    /// # Errors
    ///
    /// The poll's error as [`GpuError::Uncaptured`].
    pub fn wait(&self) -> Result<(), GpuError> {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map(|_| ())
            .map_err(|e| GpuError::Uncaptured(e.to_string()))
    }
}
