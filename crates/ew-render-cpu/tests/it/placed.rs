// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `render_placed`: the content at the target resolution times the placement's
//! scale at its offset, then the overlay at the paper's resolution; an identity placement renders as `render`.
//! `render_placed_band` (D12): its rows in bands, as a printer is sent them.

use crate::common;

use std::sync::Arc;

use common::{a4, bundled_fonts, is_dark, mm, pixel, text_run};
use ew_dl::{Brush, Color, DisplayList, Item, Path, Pen, Stroke};
use ew_render_cpu::{
    PagePlacement, RenderError, RenderOptions, RgbaImage, SheetSize, pixel_size, render, render_placed,
    render_placed_band,
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

/// A placement on a `width × height` paper at `scale`, without offset or overlay.
fn placement(width: Nm, height: Nm, scale: (u64, u64)) -> PagePlacement {
    PagePlacement {
        width,
        height,
        scale,
        offset: Pt2::ORIGIN,
        overlay: DisplayList::default(),
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
    let identity = placement(sheet_size.width, sheet_size.height, (1, 1));
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
    let half = placement(a4().width, a4().height, (1, 2));
    let (scaled, _) = render_placed(&text, &half, &options(dpi), &fonts, None).unwrap();
    let (a, b) = ink_rows(&full).expect("ink");
    let (c, d) = ink_rows(&scaled).expect("ink");
    let (full_h, half_h) = (i64::from(b - a + 1), i64::from(d - c + 1));
    assert!((2 * half_h - full_h).abs() <= 2, "{half_h} rows against {full_h}");
}

#[test]
fn a_paper_without_extent_is_a_size_error() {
    let flat = placement(Nm(0), Nm::from_mm(148), (1, 1));
    let e = render_placed(&sheet(), &flat, &options(100), &bundled_fonts(), None).unwrap_err();
    assert_eq!(
        e,
        RenderError::Size {
            width: 0,
            height: 583
        }
    );
}

/// The rows a printer band holds.
const BAND: u32 = 256;

/// WN-2: an A4 sheet fitted to A5 (148/210, centred) with an overlay, at 300 dpi, rendered in
/// bands of 256 rows: each band is the paper's width and at most 256 rows, and the bands stacked equal
/// `render_placed`'s image within one level per channel. Text, a curve, a diagonal and a filled block cross the band
/// edges. The count of pixels that differ at all is printed.
#[test]
fn the_bands_of_a_placed_page_stack_to_its_image() {
    let mut content = sheet();
    content.items.extend([
        Item::path(
            Path::line(mm(5, 5), mm(290, 205)),
            Some(Stroke::pen(Pen::P018)),
            None,
        ),
        Item::path(
            Path::circle(mm(150, 95), Nm::from_mm(60)),
            Some(Stroke::pen(Pen::P130)),
            Some(Brush::Solid(Color::from_rgba8(200, 40, 30, 255))),
        ),
        // Baselines near the band edges at 30.8 and 61.6 mm of the sheet (256 rows at 300 dpi, scaled 148/210).
        Item::glyphs(Arc::new(text_run("Band edge 1", mm(200, 32)))),
        Item::glyphs(Arc::new(text_run("Band edge 2", mm(200, 62)))),
    ]);
    let paper = SheetSize {
        width: Nm::from_mm(210),
        height: Nm::from_mm(148),
    };
    let placed = PagePlacement {
        width: paper.width,
        height: paper.height,
        scale: (148, 210),
        // (210 mm − 297 mm × 148 / 210) / 2, centred across; the height fills the paper.
        offset: Pt2::new(Nm(342_857), Nm(0)),
        overlay: DisplayList {
            items: vec![
                Item::glyphs(Arc::new(text_run("scaled 70 %", mm(180, 144)))),
                Item::path(
                    Path::rect(Rect::from_points(mm(150, 130), mm(205, 146))),
                    Some(Stroke::pen(Pen::P026)),
                    None,
                ),
            ],
        },
    };
    let fonts = bundled_fonts();
    let options = options(300);
    let (whole, _) = render_placed(&content, &placed, &options, &fonts, None).unwrap();
    let mut stacked: Vec<u8> = Vec::with_capacity(whole.pixels.len());
    let mut first_row = 0;
    while first_row < whole.height {
        let (band, _) =
            render_placed_band(&content, &placed, &options, first_row, BAND, &fonts, None).unwrap();
        let rows = BAND.min(whole.height - first_row);
        assert_eq!(
            (band.width, band.height),
            (whole.width, rows),
            "the band at row {first_row}"
        );
        assert_eq!(band.pixels.len(), (band.width * rows * 4) as usize);
        stacked.extend_from_slice(&band.pixels);
        first_row += rows;
    }
    assert_eq!(stacked.len(), whole.pixels.len());
    let mut differing = 0usize;
    for (i, (a, b)) in stacked
        .as_chunks::<4>()
        .0
        .iter()
        .zip(whole.pixels.as_chunks::<4>().0)
        .enumerate()
    {
        let off = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        assert!(
            off <= 1,
            "pixel ({}, {}): {a:?} in the bands, {b:?} in the image",
            i as u32 % whole.width,
            i as u32 / whole.width
        );
        differing += usize::from(off > 0);
    }
    println!(
        "bands of {BAND} rows at 300 dpi: {differing} of {} pixels differ, by one level at most",
        whole.pixels.len() / 4
    );
}

/// A band with no row of the paper is a size error naming the paper's width; a band past the last row ends with it.
#[test]
fn a_band_outside_the_paper_is_a_size_error() {
    let identity = placement(a4().width, a4().height, (1, 1));
    let fonts = bundled_fonts();
    let (w, h) = pixel_size(a4(), 100);
    let band = |first_row: u32, rows: u32| {
        render_placed_band(&sheet(), &identity, &options(100), first_row, rows, &fonts, None)
    };
    let none = RenderError::Size { width: w, height: 0 };
    assert_eq!(band(0, 0).unwrap_err(), none);
    assert_eq!(band(h as u32, BAND).unwrap_err(), none);
    let (last, _) = band(h as u32 - 10, BAND).unwrap();
    assert_eq!((u64::from(last.width), last.height), (w, 10));
}
