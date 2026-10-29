// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The offscreen renderer (WP-S2.17 part 1, moved here by WP-S6.13 part 4 §4.2): egui's shapes painted by an
//! `egui_wgpu` renderer on the headless device of `ew-render2d` (the software adapter where there is one, DX12 WARP
//! on Windows) into a texture that is read back as an RGBA image. The chrome snapshots replay a test harness's
//! texture changes into a fresh renderer ([`render`]); the visual play-test driver keeps one renderer for the run
//! ([`Offscreen`]). It uses only packages already in the graph.

use std::sync::mpsc::channel;

use egui::TexturesDelta;
use egui_wgpu::wgpu;
use ew_render_cpu::RgbaImage;
use ew_render2d::gpu::HeadlessDevice;

/// One texture change.
pub enum TextureOp {
    /// A texture's image, or a part of it, set.
    Set(egui::TextureId, egui::epaint::ImageDelta),
    /// A texture freed.
    Free(egui::TextureId),
}

impl TextureOp {
    /// Moves the changes of `delta` to `ops`: every set (per texture in order), then every free.
    pub fn drain(delta: &mut TexturesDelta, ops: &mut Vec<TextureOp>) {
        // A hash map of the textures' changes: the order between textures does not matter, within one it is kept.
        #[allow(clippy::iter_over_hash_type)]
        for (id, images) in delta.set.drain() {
            for image in images {
                ops.push(TextureOp::Set(id, image));
            }
        }
        #[allow(clippy::iter_over_hash_type)]
        ops.extend(delta.free.drain().map(TextureOp::Free));
    }
}

/// The headless device of the offscreen renderer: the fallback (software) adapter where there is one, else the
/// default one. `None` when the machine has no adapter at all.
pub fn device() -> Option<HeadlessDevice> {
    HeadlessDevice::all().pop()
}

/// The texture format of every offscreen image.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// An `egui_wgpu` renderer that keeps its textures between frames.
pub struct Offscreen {
    renderer: egui_wgpu::Renderer,
}

impl Offscreen {
    /// A renderer on `dev` with no textures.
    pub fn new(dev: &HeadlessDevice) -> Offscreen {
        Offscreen {
            renderer: egui_wgpu::Renderer::new(&dev.device, FORMAT, egui_wgpu::RendererOptions::PREDICTABLE),
        }
    }

    /// Applies `ops` in order.
    pub fn apply(&mut self, dev: &HeadlessDevice, ops: &[TextureOp]) {
        for op in ops {
            match op {
                TextureOp::Set(id, image) => {
                    self.renderer.update_texture(&dev.device, &dev.queue, *id, image);
                }
                TextureOp::Free(id) => self.renderer.free_texture(id),
            }
        }
    }

    /// Applies a frame's texture sets (before its shapes are painted).
    pub fn set_textures(&mut self, dev: &HeadlessDevice, delta: &TexturesDelta) {
        // The sets of one frame name distinct textures or patch one in order: the order between textures does not
        // reach the image.
        #[allow(clippy::iter_over_hash_type)]
        for (id, images) in &delta.set {
            for image in images {
                self.renderer.update_texture(&dev.device, &dev.queue, *id, image);
            }
        }
    }

    /// Applies a frame's texture frees (after its shapes are painted).
    pub fn free_textures(&mut self, delta: &TexturesDelta) {
        // Freeing is independent per texture.
        #[allow(clippy::iter_over_hash_type)]
        for id in &delta.free {
            self.renderer.free_texture(id);
        }
    }

    /// Paints `shapes` of `ctx` at `ppp` into an image of `size_in_pixels` on a black ground and reads it back.
    pub fn paint(
        &mut self,
        dev: &HeadlessDevice,
        ctx: &egui::Context,
        shapes: Vec<egui::epaint::ClippedShape>,
        size_in_pixels: [u32; 2],
        ppp: f32,
    ) -> RgbaImage {
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels,
            pixels_per_point: ppp,
        };
        let jobs = ctx.tessellate(shapes, ppp);
        let mut encoder = dev
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen"),
            });
        let user = self
            .renderer
            .update_buffers(&dev.device, &dev.queue, &mut encoder, &jobs, &screen);
        let (w, h) = (size_in_pixels[0], size_in_pixels[1]);
        let texture = dev.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("offscreen"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &jobs, &screen);
        }
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = dev.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen read-back"),
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        dev.queue
            .submit(user.into_iter().chain(std::iter::once(encoder.finish())));
        let slice = buffer.slice(..);
        let (tx, rx) = channel();
        slice.map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
        let mapped = dev
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .is_ok()
            && matches!(rx.recv(), Ok(Ok(())));
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        match mapped.then(|| slice.get_mapped_range().ok()).flatten() {
            Some(data) => {
                for line in data.chunks_exact(row as usize) {
                    pixels.extend_from_slice(&line[..(w * 4) as usize]);
                }
            }
            // A lost device reads back as a black image of the right size rather than a panic.
            None => pixels.resize((w * h * 4) as usize, 0),
        }
        RgbaImage {
            width: w,
            height: h,
            pixels,
        }
    }
}

/// The size in pixels of `ctx`'s content at its pixels per point.
pub fn size_in_pixels(ctx: &egui::Context) -> [u32; 2] {
    let size = ctx.content_rect().size() * ctx.pixels_per_point();
    [size.x.round() as u32, size.y.round() as u32]
}

/// Renders the last frame of `ctx` (`output`'s shapes) with every texture change in `ops` replayed into a fresh
/// renderer, at the context's pixels per point.
pub fn render(
    dev: &HeadlessDevice,
    ctx: &egui::Context,
    output: &egui::FullOutput,
    ops: &[TextureOp],
) -> RgbaImage {
    let mut offscreen = Offscreen::new(dev);
    offscreen.apply(dev, ops);
    let ppp = ctx.pixels_per_point();
    offscreen.paint(dev, ctx, output.shapes.clone(), size_in_pixels(ctx), ppp)
}
