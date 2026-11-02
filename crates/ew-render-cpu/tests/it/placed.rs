// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `render_placed` (WP-S3.07 D7, ruling 1 of ID-1180): the content at the target resolution times the placement's
//! scale at its offset, then the overlay at the paper's resolution; an identity placement renders as `render`.

use crate::common;

use std::sync::Arc;

use common::{a4, bundled_fonts, is_dark, mm, pixel, text_run};
use ew_dl::{Brush, DisplayList, Item, Path, Pen, Stroke};
use ew_render_cpu::{
    PagePlacement, RenderError, RenderOptions, RgbaImage, SheetSize, pixel_size, render, render_placed,
};
use ew_units::{Nm, Pt2, Rect};

/// A sheet with a filled block, a line and a text.
fn sheet() -> DisplayList {
    DisplayList {
        items: vec![
            Item::path(
                Path::rect(Rect::from_points(mm(20, 70), mm(80, 110))),
                None,
                Some(Brush::BLACK),
            ),
            Item::path(
                Path::line(mm(10, 20), mm(280, 20)),
                Some(Stroke::pen(Pen::P052)),
                None,
            ),
            Item::glyphs(Arc::new(text_run("Page 101", mm(20, 150)))),
        ],
    }
}

fn options(dpi: u32) -> RenderOptions {
    RenderOptions {
        dpi,
        ..RenderOptions::default()
    }
}

/// The pixel of the paper point `p` (mm) at `dpi`.
fn at(img: &RgbaImage, p: (f64, f64), dpi: u32) -> [u8; 4] {
    let px = |v: f64| (v / 25.4 * f64::from(dpi)).floor() as u32;
    pixel(img, px(p.0), px(p.1))
}

/// The rows of `img` with ink, first and last.
fn ink_rows(img: &RgbaImage) -> Option<(u32, u32)> {
    let rows: Vec<u32> = (0..img.height)
        .filter(|&y| (0..img.width).any(|x| is_dark(pixel(img, x, y))))
        .collect();
    Some((*rows.first()?, *rows.last()?))
}

#[test]
fn an_identity_placement_renders_exactly_as_render() {
    let list = sheet();
    let sheet_size = a4();
    let identity = PagePlacement {
        width: sheet_size.width,
        height: sheet_size.height,
        scale: (1, 1),
        offset: Pt2::ORIGIN,
        overlay: DisplayList::default(),
    };
    let fonts = bundled_fonts();
    let (plain, plain_report) = render(&list, sheet_size, &options(100), &fonts, None).unwrap();
    let (placed, placed_report) = render_placed(&list, &identity, &options(100), &fonts, None).unwrap();
    assert_eq!(placed, plain);
    assert_eq!(placed_report, plain_report);
}

/// The block (20..80 × 70..110 mm on the sheet) at ½ from (50, 20) mm lies at 60..90 × 55..75 mm on the paper; the
/// sheet's own place of it stays white; the image is the paper's size; the overlay is drawn at the paper's scale.
#[test]
fn the_content_is_scaled_at_the_offset_and_the_overlay_is_not() {
    let paper = SheetSize {
        width: Nm::from_mm(210),
        height: Nm::from_mm(148),
    };
    let half = PagePlacement {
        width: paper.width,
        height: paper.height,
        scale: (1, 2),
        offset: mm(50, 20),
        overlay: DisplayList {
            items: vec![Item::path(
                Path::rect(Rect::from_points(mm(150, 120), mm(200, 140))),
                None,
                Some(Brush::BLACK),
            )],
        },
    };
    let dpi = 100;
    let (img, _) = render_placed(&sheet(), &half, &options(dpi), &bundled_fonts(), None).unwrap();
    assert_eq!(
        (u64::from(img.width), u64::from(img.height)),
        pixel_size(paper, dpi)
    );
    assert_eq!(half.place(mm(20, 70)), mm(60, 55));
    assert_eq!(half.place(mm(80, 110)), mm(90, 75));
    assert!(is_dark(at(&img, (75.0, 65.0), dpi)), "the block, placed");
    assert!(
        !is_dark(at(&img, (30.0, 100.0), dpi)),
        "the block's sheet place stays white"
    );
    assert!(!is_dark(at(&img, (95.0, 65.0), dpi)), "beyond the placed block");
    // The overlay at 150..200 × 120..140 mm of the paper, unscaled and unshifted.
    assert!(is_dark(at(&img, (155.0, 125.0), dpi)), "the overlay");
    assert!(is_dark(at(&img, (195.0, 138.0), dpi)), "the overlay's far corner");
    assert!(!is_dark(at(&img, (205.0, 138.0), dpi)), "beyond the overlay");
}

/// Glyphs are sized by the composed scale: a text placed at ½ has half the ink height of the text on its sheet.
#[test]
fn glyphs_are_sized_by_the_composed_scale() {
    let text = DisplayList {
        items: vec![Item::glyphs(Arc::new(text_run("HHHH", mm(20, 50))))],
    };
    let fonts = bundled_fonts();
    let dpi = 300;
    let (full, _) = render(&text, a4(), &options(dpi), &fonts, None).unwrap();
    let half = PagePlacement {
        width: a4().width,
        height: a4().height,
        scale: (1, 2),
        offset: Pt2::ORIGIN,
        overlay: DisplayList::default(),
    };
    let (scaled, _) = render_placed(&text, &half, &options(dpi), &fonts, None).unwrap();
    let (a, b) = ink_rows(&full).expect("ink");
    let (c, d) = ink_rows(&scaled).expect("ink");
    let (full_h, half_h) = (i64::from(b - a + 1), i64::from(d - c + 1));
    assert!((2 * half_h - full_h).abs() <= 2, "{half_h} rows against {full_h}");
}

#[test]
fn a_paper_without_extent_is_a_size_error() {
    let flat = PagePlacement {
        width: Nm(0),
        height: Nm::from_mm(148),
        scale: (1, 1),
        offset: Pt2::ORIGIN,
        overlay: DisplayList::default(),
    };
    let e = render_placed(&sheet(), &flat, &options(100), &bundled_fonts(), None).unwrap_err();
    assert_eq!(
        e,
        RenderError::Size {
            width: 0,
            height: 583
        }
    );
}
