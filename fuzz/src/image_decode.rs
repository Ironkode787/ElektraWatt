// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The image decoder target (WP-S6.04 D9; doc 18 §11 TM-19): one selector byte picks PNG (even) or JPEG (odd), the
//! rest goes through `ew-render-cpu`'s `decode_image`, the one decoder of the asset paths. Property: whatever the
//! bytes, nothing panics, and a decoded image is at least 1 × 1, at most `MAX_IMAGE_PIXELS`, with four bytes per
//! pixel.

use ew_dl::ImageFormat;
use ew_export::ew_render_cpu::{MAX_IMAGE_PIXELS, decode_image};

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// Whether the decoder accepted the bytes.
    pub decoded: bool,
}

/// Runs one input.
///
/// # Panics
///
/// When a decoded image is empty, larger than `MAX_IMAGE_PIXELS` or not four bytes per pixel.
pub fn check(input: &[u8]) -> Outcome {
    let (selector, bytes) = input.split_first().map_or((0, input), |(s, r)| (*s, r));
    let format = if selector % 2 == 0 {
        ImageFormat::Png
    } else {
        ImageFormat::Jpeg
    };
    let Some(image) = decode_image(format, bytes) else {
        return Outcome { decoded: false };
    };
    let pixels = u64::from(image.width) * u64::from(image.height);
    assert!(
        (1..=MAX_IMAGE_PIXELS).contains(&pixels),
        "a decoded image of {} × {}",
        image.width,
        image.height
    );
    assert_eq!(image.rgba.len() as u64, pixels * 4, "four bytes per pixel");
    Outcome { decoded: true }
}
