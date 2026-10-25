// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PNG-to-PDF target (FT-436; doc 18 §11 TM-19): a PNG that `ew_render_cpu::decode_image` accepts is drawn by the
//! PDF writer, which hands its bytes to krilla. Property: whatever the bytes, nothing panics; and when the decoder
//! accepts them, the PDF/A-2b of one page holding the image is written (`Ok`) with the image drawn
//! (`report.images == 1`, `report.skipped_images == 0`). A failure of that property is a panic with the writer's
//! error, so the fuzzer reports it as a crash.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_dl::{Destination, DisplayList, FontKey, ImageData, ImageFormat, Item};
use ew_export::ew_render_cpu::FontBytes;
use ew_export::{PdfDate, PdfOptions, PdfPage, pdf};
use uuid::Uuid;

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// Whether `decode_image` accepted the bytes as a PNG (then the PDF was written and checked).
    pub decoded: bool,
}

/// A point in millimetres on the page.
fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm::from_mm(x), Nm::from_mm(y))
}

/// Runs one input.
///
/// # Panics
///
/// When `decode_image` accepts the bytes and the PDF writer refuses the page or does not draw the image.
pub fn check(data: &[u8]) -> Outcome {
    let Some(_image) = ew_export::ew_render_cpu::decode_image(ew_dl::ImageFormat::Png, data) else {
        return Outcome { decoded: false };
    };
    let page = PdfPage {
        destination: Destination::Page(Uuid::from_u128(1)),
        label: "1".to_owned(),
        title: "1 Page 1".to_owned(),
        section: None,
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
        content: DisplayList {
            items: vec![Item::image(
                Arc::new(ImageData {
                    sha256: [1; 32],
                    format: ImageFormat::Png,
                    bytes: Arc::from(data),
                }),
                Rect::from_points(mm(10, 10), mm(110, 80)),
            )],
        },
    };
    let options = PdfOptions {
        document_id: "fuzz".into(),
        ..PdfOptions::new(PdfDate::ymd(2026, 1, 1))
    };
    let fonts: BTreeMap<FontKey, FontBytes> = BTreeMap::new();
    let result = pdf(&[page], &options, &fonts, None);
    let out = result
        .unwrap_or_else(|e| panic!("a PNG that decode_image accepts did not serialise through krilla: {e}"));
    assert_eq!(
        (out.report.images, out.report.skipped_images),
        (1, 0),
        "a PNG that decode_image accepts was not drawn"
    );
    Outcome { decoded: true }
}
