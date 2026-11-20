// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An offscreen renderer of a test harness's last frame: the harness hands every texture change to
//! a [`Recorder`]; [`render`] applies the changes since the test's last image to the test's one renderer of
//! `ew_ui::offscreen`, which paints the last frame's shapes on the headless device of `ew-render2d` and reads them
//! back as an RGBA image.

#![allow(missing_docs)]

use std::cell::RefCell;
use std::rc::Rc;

use egui::TexturesDelta;
use egui_kittest::TestRenderer;
use ew_render_cpu::RgbaImage;
use ew_render2d::gpu::HeadlessDevice;
pub use ew_ui::offscreen::{TextureOp, device};
use ew_ui::offscreen::{Offscreen, size_in_pixels};

/// Collects the texture changes the harness reports, in order, and holds the renderer of the test's images with the
/// number of changes it has applied. On the software adapter a renderer's first image took about 0.4 s more than
/// the next (pipelines and uploads, measured by hand), so a test's images share one renderer.
#[derive(Clone, Default)]
pub struct Recorder {
    ops: Rc<RefCell<Vec<TextureOp>>>,
    renderer: Rc<RefCell<Option<(Offscreen, usize)>>>,
}

impl TestRenderer for Recorder {
    fn handle_delta(&mut self, delta: &mut TexturesDelta) {
        TextureOp::drain(delta, &mut self.ops.borrow_mut());
    }
}

/// Renders the last frame of `ctx` (`output`'s shapes) with every texture change `recorder` saw, at the context's
/// pixels per point: the changes since the test's last image go to its renderer (made on `dev` at the first image).
pub fn render(
    dev: &HeadlessDevice,
    ctx: &egui::Context,
    output: &egui::FullOutput,
    recorder: &Recorder,
) -> RgbaImage {
    let mut held = recorder.renderer.borrow_mut();
    let (renderer, applied) = held.get_or_insert_with(|| (Offscreen::new(dev), 0));
    let ops = recorder.ops.borrow();
    renderer.apply(dev, &ops[*applied..]);
    *applied = ops.len();
    renderer.paint(dev, ctx, output.shapes.clone(), size_in_pixels(ctx), ctx.pixels_per_point())
}

/// Writes `image` as a PNG to `path` (directories created).
pub fn write_png(image: &RgbaImage, path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(path, ew_render_cpu::encode_png(image, None)).unwrap();
}

/// The pixel gate's tolerance: a pixel differs when one of its channels differs by more than this (of 255).
pub const CHANNEL_TOLERANCE: u8 = 4;

/// The pixel gate's allowance: at most this many differing pixels per image.
pub const MAX_DIFFERING_PIXELS: usize = 10;

/// Reads a PNG written by [`write_png`] (8-bit RGBA, or 8-bit grey when every pixel is opaque grey).
pub fn read_png(path: &std::path::Path) -> Option<RgbaImage> {
    let bytes = std::fs::read(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        _ => return None,
    };
    Some(RgbaImage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

/// The number of pixels of `new` that differ from `old` beyond [`CHANNEL_TOLERANCE`], and an image of the
/// differences (`new` dimmed to a quarter, each differing pixel opaque red). `None` when the sizes differ.
pub fn compare(old: &RgbaImage, new: &RgbaImage) -> Option<(usize, RgbaImage)> {
    if (old.width, old.height) != (new.width, new.height) {
        return None;
    }
    let mut count = 0;
    let mut pixels = Vec::with_capacity(new.pixels.len());
    for (a, b) in old
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(new.pixels.as_chunks::<4>().0)
    {
        if a.iter().zip(b).any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE) {
            count += 1;
            pixels.extend_from_slice(&[255, 0, 0, 255]);
        } else {
            pixels.extend_from_slice(&[b[0] / 4, b[1] / 4, b[2] / 4, 255]);
        }
    }
    Some((
        count,
        RgbaImage {
            width: new.width,
            height: new.height,
            pixels,
        },
    ))
}
