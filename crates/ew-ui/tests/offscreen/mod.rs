// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An offscreen renderer of a test harness's last frame (WP-S2.17 part 1): the harness hands every texture change to
//! a [`Recorder`]; [`render`] replays them into an `egui_wgpu` renderer on the headless device of `ew-render2d`
//! (the software adapter where there is one, DX12 WARP on Windows), paints the last frame's shapes into a texture and
//! reads it back as an RGBA image. It uses only packages already in the graph (no `egui_kittest` render feature).

#![allow(dead_code, missing_docs)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::channel;

use egui::TexturesDelta;
use egui_kittest::TestRenderer;
use egui_wgpu::wgpu;
use ew_render_cpu::RgbaImage;
use ew_render2d::gpu::HeadlessDevice;

/// One texture change.
pub enum TextureOp {
    Set(egui::TextureId, egui::epaint::ImageDelta),
    Free(egui::TextureId),
}

/// Collects the texture changes the harness reports, in order.
#[derive(Clone, Default)]
pub struct Recorder(pub Rc<RefCell<Vec<TextureOp>>>);

impl TestRenderer for Recorder {
    fn handle_delta(&mut self, delta: &mut TexturesDelta) {
        let mut ops = self.0.borrow_mut();
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

/// The headless device of the snapshots: the fallback (software) adapter where there is one, else the default one.
pub fn device() -> Option<HeadlessDevice> {
    HeadlessDevice::all().pop()
}

/// Renders the last frame of `ctx` (`output`'s shapes) with every texture change `recorder` saw, at the context's
/// pixels per point.
pub fn render(
    dev: &HeadlessDevice,
    ctx: &egui::Context,
    output: &egui::FullOutput,
    recorder: &Recorder,
) -> RgbaImage {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = egui_wgpu::Renderer::new(&dev.device, format, egui_wgpu::RendererOptions::PREDICTABLE);
    for op in recorder.0.borrow().iter() {
        match op {
            TextureOp::Set(id, image) => {
                renderer.update_texture(&dev.device, &dev.queue, *id, image);
            }
            TextureOp::Free(id) => renderer.free_texture(id),
        }
    }
    let ppp = ctx.pixels_per_point();
    let size = ctx.content_rect().size() * ppp;
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [size.x.round() as u32, size.y.round() as u32],
        pixels_per_point: ppp,
    };
    let jobs = ctx.tessellate(output.shapes.clone(), ppp);
    let mut encoder = dev
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("offscreen"),
        });
    let user = renderer.update_buffers(&dev.device, &dev.queue, &mut encoder, &jobs, &screen);
    let (w, h) = (screen.size_in_pixels[0], screen.size_in_pixels[1]);
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
        format,
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
        renderer.render(&mut pass, &jobs, &screen);
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
    dev.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("poll");
    rx.recv().expect("map").expect("map");
    let data = slice.get_mapped_range().expect("mapped");
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for line in data.chunks_exact(row as usize) {
        pixels.extend_from_slice(&line[..(w * 4) as usize]);
    }
    RgbaImage {
        width: w,
        height: h,
        pixels,
    }
}

/// Writes `image` as a PNG to `path` (directories created).
pub fn write_png(image: &RgbaImage, path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(path, ew_render_cpu::encode_png(image, None)).unwrap();
}
