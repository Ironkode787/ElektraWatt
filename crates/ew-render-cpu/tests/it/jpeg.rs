// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JPEG decoding of image items (FT-406) over the JPEG variants a logo may come in: greyscale, YCbCr 4:4:4 and 4:2:0,
//! CMYK (Adobe), baseline and progressive, built in the test from fixed coefficients (`common::jpeg`), and the
//! cross-platform determinism of the decoded pixels.

use crate::common;

use common::jpeg::{Layout, baseline, progressive};
use ew_dl::ImageFormat;
use ew_render_cpu::{decode_image, jpeg_is_cmyk};
use sha2::{Digest, Sha256};

#[test]
fn every_layout_decodes_to_its_size() {
    for layout in [Layout::Grey, Layout::Ycbcr444, Layout::Ycbcr420, Layout::Cmyk] {
        let img = decode_image(ImageFormat::Jpeg, &baseline(layout, 32, 16))
            .unwrap_or_else(|| panic!("{layout:?} decodes"));
        assert_eq!(
            (img.width, img.height, img.rgba.len()),
            (32, 16, 32 * 16 * 4),
            "{layout:?}"
        );
        assert!(
            img.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
            "{layout:?} is opaque"
        );
        // The coefficients vary per block, so the image is not flat.
        assert!(
            img.rgba.as_chunks::<4>().0.iter().any(|p| p[..] != img.rgba[..4]),
            "{layout:?} has texture"
        );
    }
    let grey = decode_image(ImageFormat::Jpeg, &baseline(Layout::Grey, 32, 16)).unwrap();
    assert!(
        grey.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] == p[1] && p[1] == p[2]),
        "grey stays grey"
    );
}

#[test]
fn a_progressive_jpeg_decodes_to_the_pixels_of_its_baseline_twin() {
    for layout in [Layout::Grey, Layout::Ycbcr420, Layout::Cmyk] {
        let b = decode_image(ImageFormat::Jpeg, &baseline(layout, 32, 32)).unwrap();
        let p = decode_image(ImageFormat::Jpeg, &progressive(layout, 32, 32))
            .unwrap_or_else(|| panic!("progressive {layout:?} decodes"));
        assert_eq!(b, p, "{layout:?}");
    }
}

/// Review item 5: the CMYK probe uses the decode path's options, so a JPEG wider than zune's default 16384 px is still
/// recognised as CMYK (and embedded as decoded pixels by the PDF writer).
#[test]
fn the_cmyk_probe_sees_the_colour_space_at_any_width() {
    assert!(jpeg_is_cmyk(&baseline(Layout::Cmyk, 32, 16)));
    assert!(jpeg_is_cmyk(&baseline(Layout::Cmyk, 20_000, 16)));
    for layout in [Layout::Grey, Layout::Ycbcr444, Layout::Ycbcr420] {
        assert!(!jpeg_is_cmyk(&baseline(layout, 32, 16)), "{layout:?}");
    }
    assert!(!jpeg_is_cmyk(b"not a jpeg"));
    let wide = decode_image(ImageFormat::Jpeg, &baseline(Layout::Cmyk, 20_000, 16)).unwrap();
    assert_eq!((wide.width, wide.height), (20_000, 16));
}

/// The decoded pixels of a 4:2:0 JPEG with AC data are the same on every CI platform: zune-jpeg picks its IDCT,
/// upsampling and colour conversion (scalar, AVX2 or NEON) at run time, and `set_use_unsafe(false)` does not turn
/// that off in zune-core 0.5.3, so this hash pins the result across the three CI operating systems (x86-64 Linux and
/// Windows, AArch64 macOS). A mismatch on one of them means the raster pixels of the CPU path differ by platform.
#[test]
fn decoded_jpeg_pixels_are_the_same_on_every_platform() {
    let img = decode_image(ImageFormat::Jpeg, &baseline(Layout::Ycbcr420, 48, 32)).unwrap();
    let hash: String = Sha256::digest(&img.rgba)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let simd = if std::arch::is_x86_feature_detected!("avx2") {
        "AVX2 detected"
    } else {
        "no AVX2"
    };
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    let simd = "not x86";
    assert_eq!(
        hash,
        "db1b8a8b89a985e795dba87445653ae3ae1099ed0f27bbfe4c362a43d175a8b0",
        "decoded JPEG pixels on {} ({simd})",
        std::env::consts::ARCH
    );
}

/// A JPEG cut anywhere in its last bytes is refused. zune-jpeg 0.5.15 checks for a premature end of the entropy-coded
/// data only at the start of an MCU row, even in strict mode, so a cut inside the last row decoded "successfully"
/// with zero bits; `decode_image` therefore requires the EOI marker after the last scan.
#[test]
fn a_jpeg_cut_in_its_last_mcu_row_is_refused() {
    for layout in [Layout::Grey, Layout::Ycbcr420, Layout::Cmyk] {
        for (w, h) in [(16u16, 16u16), (48, 32)] {
            let whole = baseline(layout, w, h);
            assert!(decode_image(ImageFormat::Jpeg, &whole).is_some());
            let decoded: Vec<usize> = (1..60)
                .filter(|cut| decode_image(ImageFormat::Jpeg, &whole[..whole.len() - cut]).is_some())
                .collect();
            assert!(
                decoded.is_empty(),
                "{layout:?} {w}x{h} decodes when cut by {decoded:?} bytes"
            );
            // Bytes after the EOI marker (some writers append them) do not matter.
            let mut trailing = whole.clone();
            trailing.extend_from_slice(b"\0\0trailer");
            assert!(
                decode_image(ImageFormat::Jpeg, &trailing).is_some(),
                "{layout:?} with trailing bytes"
            );
        }
    }
}
