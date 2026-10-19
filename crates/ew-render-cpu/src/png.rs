// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic PNG encoding of a rendered image (FR-OUT-014).

use crate::render::RgbaImage;

/// Lossless PNG of `image` with fixed encoder settings, so equal pixels give equal bytes.
///
/// The image is written as 8-bit grayscale when every pixel is opaque and has `r == g == b`, else as
/// 8-bit RGBA. The only ancillary chunk is `pHYs` (pixels per metre, `round(dpi / 0.0254)`), written
/// when `dpi` is `Some`. Compression is [`png::Compression::High`] with the adaptive filter
/// ([`png::Filter::Adaptive`], a fixed per-row heuristic, not randomised).
///
/// A pixel buffer shorter than `width × height × 4` is read as transparent black beyond its end;
/// extra bytes are ignored. An image with a zero dimension yields an empty vector (PNG has no empty
/// image).
pub fn encode_png(image: &RgbaImage, dpi: Option<u32>) -> Vec<u8> {
    encode(image, dpi).unwrap_or_default()
}

fn encode(image: &RgbaImage, dpi: Option<u32>) -> Option<Vec<u8>> {
    let len = (image.width as usize)
        .checked_mul(image.height as usize)?
        .checked_mul(4)?;
    let mut rgba = image.pixels.get(..len).unwrap_or(&image.pixels).to_vec();
    rgba.resize(len, 0);

    let gray = rgba
        .chunks_exact(4)
        .all(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2]);
    let (color, data) = if gray {
        (
            png::ColorType::Grayscale,
            rgba.chunks_exact(4).map(|p| p[0]).collect::<Vec<u8>>(),
        )
    } else {
        (png::ColorType::Rgba, rgba)
    };

    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    encoder.set_filter(png::Filter::Adaptive);
    if let Some(dpi) = dpi {
        let ppm = (f64::from(dpi) / 0.0254).round() as u32;
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));
    }
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&data).ok()?;
    writer.finish().ok()?;
    Some(out)
}
