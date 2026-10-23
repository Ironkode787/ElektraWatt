// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Raster image items decoded for drawing (FT-384, FT-406; decision log ID-576, ID-605): PNG with `png`, JPEG with
//! `zune-jpeg`, both bounded by [`MAX_IMAGE_PIXELS`] before any pixel buffer is allocated, so that a small file cannot
//! decompress into an unbounded buffer. The PDF writer of `ew-export` decodes through [`decode_image`] too before it
//! hands the original bytes to krilla, so an image the CPU renderer cannot draw is not embedded either.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ew_dl::ImageFormat;

/// The most pixels a decoded image may have (16 Mi pixels, 64 MiB as RGBA8): a larger PNG or JPEG is not decoded.
pub const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;

/// A decoded raster image: straight-alpha (NOT premultiplied) RGBA8, row-major, top row first.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DecodedImage {
    /// Width in pixels (at least 1).
    pub width: u16,
    /// Height in pixels (at least 1).
    pub height: u16,
    /// `width × height × 4` bytes.
    pub rgba: Vec<u8>,
}

/// The pixels of PNG or JPEG `bytes`, or `None` when the format is SVG (drawn as paths, never decoded here), the bytes
/// do not decode completely (a truncated JPEG is refused, not padded: it must end its last scan with the EOI marker,
/// see `has_eoi_after_last_scan`), a side is zero or exceeds `u16::MAX`, or the
/// image has more than [`MAX_IMAGE_PIXELS`]. A 16-bit PNG is reduced to 8 bits; a greyscale or CMYK JPEG is converted
/// to RGB by the decoder. The decoders run inside `catch_unwind` (it works with `panic = "unwind"`, the workspace
/// profiles' setting), so hostile bytes give `None`, never a panic.
///
/// Deliberate limitations (ruled): EXIF orientation is ignored here and in the PDF writer, so a JPEG prints as its
/// stored pixel grid on both paths; an embedded ICC profile is ignored here, while krilla embeds it in the PDF (a
/// recorded difference in v0).
pub fn decode_image(format: ImageFormat, bytes: &[u8]) -> Option<DecodedImage> {
    let decode = || match format {
        ImageFormat::Png => decode_png(bytes),
        ImageFormat::Jpeg => decode_jpeg(bytes),
        ImageFormat::Svg => None,
    };
    catch_unwind(AssertUnwindSafe(decode)).ok().flatten()
}

/// Whether JPEG `bytes` hold CMYK or YCCK samples (`false` when the header does not parse). The PDF writer embeds such a
/// JPEG as its decoded RGB pixels rather than unchanged: a CMYK image in a PDF/A needs a CMYK output intent, which the
/// writer does not have.
pub fn jpeg_is_cmyk(bytes: &[u8]) -> bool {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;

    let probe = || {
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), jpeg_options());
        decoder.decode_headers().ok()?;
        decoder.input_colorspace()
    };
    matches!(
        catch_unwind(AssertUnwindSafe(probe)).ok().flatten(),
        Some(ColorSpace::CMYK | ColorSpace::YCCK)
    )
}

/// The zune-jpeg options of every JPEG read here: sides up to `u16::MAX` (the pixel bound is checked separately), strict
/// mode (a premature end of the entropy-coded data is an error, not a grey fill), RGB output (expanded to RGBA here:
/// zune-jpeg 0.5.15 writes a CMYK image into an RGBA buffer three bytes per pixel, so its RGBA output of CMYK is wrong).
fn jpeg_options() -> zune_jpeg::zune_core::options::DecoderOptions {
    zune_jpeg::zune_core::options::DecoderOptions::default()
        .set_max_width(usize::from(u16::MAX))
        .set_max_height(usize::from(u16::MAX))
        .set_strict_mode(true)
        .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGB)
}

/// The checked side lengths of a `w × h` image: both in `1..=u16::MAX`, at most [`MAX_IMAGE_PIXELS`] in all.
fn checked_size(w: u64, h: u64) -> Option<(u16, u16)> {
    if w.checked_mul(h)? > MAX_IMAGE_PIXELS {
        return None;
    }
    let (w, h) = (u16::try_from(w).ok()?, u16::try_from(h).ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

fn decode_png(bytes: &[u8]) -> Option<DecodedImage> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(MAX_IMAGE_PIXELS * 4).unwrap_or(usize::MAX),
    });
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = checked_size(u64::from(reader.info().width), u64::from(reader.info().height))?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    reader.next_frame(&mut buf).ok()?;
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return None;
    }
    let rgba: Vec<u8> = match color {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        _ => return None,
    };
    finish(width, height, rgba)
}

/// Whether JPEG `bytes` hold an EOI marker (`FF D9`) after the start of their last scan (`FF DA`). Neither marker can
/// occur inside entropy-coded data (an `FF` there is stuffed as `FF 00`; restart markers are `FF D0`–`FF D7`), so a
/// file cut anywhere after its last scan header lacks it. zune-jpeg 0.5.15 checks for a premature end of the data only
/// at the start of an MCU row, even in strict mode, and decodes a cut inside the last row with zero bits; this check
/// refuses such a file. Bytes after the EOI marker are allowed.
fn has_eoi_after_last_scan(bytes: &[u8]) -> bool {
    let last_scan = bytes.windows(2).rposition(|w| w == [0xff, 0xda]);
    last_scan.is_some_and(|i| bytes[i..].windows(2).any(|w| w == [0xff, 0xd9]))
}

fn decode_jpeg(bytes: &[u8]) -> Option<DecodedImage> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;

    if !has_eoi_after_last_scan(bytes) {
        return None;
    }
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), jpeg_options());
    decoder.decode_headers().ok()?;
    let (w, h) = decoder.dimensions()?;
    let (width, height) = checked_size(u64::try_from(w).ok()?, u64::try_from(h).ok()?)?;
    let out = decoder.output_colorspace()?;
    let mut buf = vec![0u8; decoder.output_buffer_size()?];
    decoder.decode_into(&mut buf).ok()?;
    let rgba: Vec<u8> = match out {
        ColorSpace::RGBA => buf,
        ColorSpace::RGB => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        ColorSpace::Luma => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        ColorSpace::LumaA => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        _ => return None,
    };
    finish(width, height, rgba)
}

fn finish(width: u16, height: u16, rgba: Vec<u8>) -> Option<DecodedImage> {
    (rgba.len() == usize::from(width) * usize::from(height) * 4).then_some(DecodedImage {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG of `w × h` pixels of one RGBA colour (synthetic).
    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(&rgba.repeat((w * h) as usize)).unwrap();
        writer.finish().unwrap();
        out
    }

    #[test]
    fn a_png_decodes_to_straight_rgba() {
        let img = decode_image(ImageFormat::Png, &png(3, 2, [10, 20, 30, 128])).unwrap();
        assert_eq!((img.width, img.height), (3, 2));
        assert_eq!(img.rgba, [10, 20, 30, 128].repeat(6));
    }

    /// A 16 × 8 baseline JPEG (4:4:4, all quantisers 1, DC-only blocks, written by hand): the left 8 × 8 block red,
    /// the right one blue.
    fn red_blue_jpeg() -> Vec<u8> {
        let mut b = vec![0xff, 0xd8, 0xff, 0xdb, 0x00, 0x84, 0x00];
        b.extend([1u8; 64]);
        b.push(0x01);
        b.extend([1u8; 64]);
        b.extend_from_slice(&[
            0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x08, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01,
            0x03, 0x11, 0x01, 0xff, 0xc4, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01,
            0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x3f, 0x00, 0x92, 0xfa, 0x54, 0xea, 0xfe, 0x12, 0x87, 0x5d,
            0x50, 0x5b, 0x5f, 0x7f, 0xff, 0xd9,
        ]);
        b
    }

    #[test]
    fn a_jpeg_decodes_to_rgba() {
        let img = decode_image(ImageFormat::Jpeg, &red_blue_jpeg()).unwrap();
        assert_eq!((img.width, img.height), (16, 8));
        let px = |x: usize, y: usize| &img.rgba[(y * 16 + x) * 4..][..4];
        for y in 0..8 {
            let (l, r) = (px(1, y), px(14, y));
            assert!(
                l[0] > 240 && l[1] < 16 && l[2] < 16 && l[3] == 255,
                "red at row {y}: {l:?}"
            );
            assert!(
                r[0] < 16 && r[1] < 16 && r[2] > 240 && r[3] == 255,
                "blue at row {y}: {r:?}"
            );
        }
    }

    /// Hostile input is refused, never a panic: a JPEG cut inside its entropy-coded data (and one cut before it), a JPEG
    /// header declaring 8192 × 8192 pixels (over the bound) with no data, and bytes that are no JPEG or PNG at all.
    #[test]
    fn hostile_rasters_are_refused() {
        let jpeg = red_blue_jpeg();
        let scan = jpeg.len() - 16;
        assert_eq!(
            decode_image(ImageFormat::Jpeg, &jpeg[..scan + 4]),
            None,
            "cut inside the scan"
        );
        assert_eq!(
            decode_image(ImageFormat::Jpeg, &jpeg[..scan - 20]),
            None,
            "cut in the headers"
        );
        let mut huge = jpeg.clone();
        let sof = huge.windows(2).position(|w| w == [0xff, 0xc0]).unwrap();
        huge[sof + 5..sof + 9].copy_from_slice(&[0x20, 0x00, 0x20, 0x00]);
        assert_eq!(
            decode_image(ImageFormat::Jpeg, &huge),
            None,
            "8192 × 8192 declared"
        );
        let mut wide = png(1, 1, [0, 0, 0, 255]);
        wide[16..24].copy_from_slice(&[0, 0x01, 0x86, 0xa0, 0, 0x01, 0x86, 0xa0]);
        assert_eq!(
            decode_image(ImageFormat::Png, &wide),
            None,
            "100000 × 100000 declared"
        );
        assert_eq!(
            decode_image(ImageFormat::Jpeg, &png(2, 2, [1, 2, 3, 4])),
            None,
            "a PNG called JPEG"
        );
        assert_eq!(decode_image(ImageFormat::Png, &jpeg), None, "a JPEG called PNG");
        assert_eq!(
            decode_image(ImageFormat::Jpeg, b"GIF89a\x01\x00\x01\x00\x00\x00\x00;"),
            None,
            "a GIF"
        );
        assert_eq!(
            decode_image(ImageFormat::Png, b"RIFF\x1a\x00\x00\x00WEBPVP8L"),
            None,
            "a WebP"
        );
    }

    #[test]
    fn svg_and_empty_bytes_are_not_decoded() {
        assert_eq!(decode_image(ImageFormat::Svg, b"<svg/>"), None);
        assert_eq!(decode_image(ImageFormat::Png, &[]), None);
        assert_eq!(decode_image(ImageFormat::Jpeg, &[]), None);
    }

    #[test]
    fn the_size_bound_holds_before_allocation() {
        assert_eq!(checked_size(4096, 4096), Some((4096, 4096)));
        assert_eq!(checked_size(4096, 4097), None);
        assert_eq!(checked_size(65_536, 1), None);
        assert_eq!(checked_size(0, 5), None);
        assert_eq!(checked_size(u64::MAX, u64::MAX), None);
    }
}
