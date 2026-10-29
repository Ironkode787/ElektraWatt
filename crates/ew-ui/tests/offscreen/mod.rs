// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An offscreen renderer of a test harness's last frame (WP-S2.17 part 1): the harness hands every texture change to
//! a [`Recorder`]; [`render`] replays them with `ew_ui::offscreen` (moved there by WP-S6.13 part 4), which paints
//! the last frame's shapes on the headless device of `ew-render2d` and reads them back as an RGBA image.

#![allow(dead_code, missing_docs)]

use std::cell::RefCell;
use std::rc::Rc;

use egui::TexturesDelta;
use egui_kittest::TestRenderer;
use ew_render_cpu::RgbaImage;
use ew_render2d::gpu::HeadlessDevice;
pub use ew_ui::offscreen::{TextureOp, device};

/// Collects the texture changes the harness reports, in order.
#[derive(Clone, Default)]
pub struct Recorder(pub Rc<RefCell<Vec<TextureOp>>>);

impl TestRenderer for Recorder {
    fn handle_delta(&mut self, delta: &mut TexturesDelta) {
        TextureOp::drain(delta, &mut self.0.borrow_mut());
    }
}

/// Renders the last frame of `ctx` (`output`'s shapes) with every texture change `recorder` saw, at the context's
/// pixels per point (the renderer core is `ew_ui::offscreen`).
pub fn render(
    dev: &HeadlessDevice,
    ctx: &egui::Context,
    output: &egui::FullOutput,
    recorder: &Recorder,
) -> RgbaImage {
    ew_ui::offscreen::render(dev, ctx, output, &recorder.0.borrow())
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
