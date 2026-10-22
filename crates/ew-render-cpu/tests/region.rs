// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `render_region` and `ItemBounds` (WP-S2.02 §4.2, acceptance check A3): the full rectangle is `render` byte for
//! byte, stitched regions match the full raster within the doc 20 tolerance, the surround is outside the sheet, and
//! culling never changes a pixel.

mod common;

use std::sync::Arc;

use common::{Lcg, a4, bundled_fonts, mm, pixel, text_run};
use ew_dl::{Brush, Color, DashPattern, DisplayList, Item, Path, Pen, Stroke, Transform};
use ew_render_cpu::{
    ItemBounds, RegionOptions, RenderError, RenderOptions, RgbaImage, SheetSize, pixel_size, render,
    render_region,
};
use ew_units::{Nm, Orient2, Pt2, Rect};

const SURROUND: Color = Color {
    r: 0x2B,
    g: 0x2B,
    b: 0x2B,
    a: 0xFF,
};

/// A page with every kind of item, items near every edge, a rotated group, a clip and items partly off the sheet.
fn page() -> DisplayList {
    let mut items = vec![
        Item::path(
            Path::line(mm(10, 20), mm(280, 20)),
            Some(Stroke::pen(Pen::P026)),
            None,
        ),
        Item::path(
            Path::line(mm(10, 30), mm(280, 60)),
            Some(Stroke::pen(Pen::P052)),
            None,
        ),
        Item::path(
            Path::line(mm(10, 40), mm(280, 40)),
            Some(Stroke::pen(Pen::P042).dashed(DashPattern::Dash42)),
            None,
        ),
        Item::path(
            Path::rect(Rect::from_points(mm(20, 70), mm(80, 110))),
            None,
            Some(Brush::Solid(Color::from_hex("#C0C0C0").unwrap())),
        ),
        Item::path(
            Path::circle(mm(150, 100), Nm::from_mm(20)),
            Some(Stroke::pen(Pen::P018)),
            Some(Brush::BLACK),
        ),
        Item::glyphs(Arc::new(text_run("Page 101", mm(20, 150)))),
        // Partly off the sheet: the region must not show what lies outside it.
        Item::path(
            Path::rect(Rect::from_points(mm(290, 200), mm(320, 230))),
            None,
            Some(Brush::Solid(Color::from_rgb8(10, 120, 200))),
        ),
        // A thick miter corner close to an edge.
        Item::path(
            Path::polygon(&[mm(100, 180), mm(130, 182), mm(100, 184)]),
            Some(Stroke::solid(Nm::from_mm(2))),
            None,
        ),
    ];
    items.push(Item::group(
        Some(Transform::from_translation(mm(200, 120))),
        Some(Rect::from_points(mm(0, 0), mm(30, 30))),
        vec![Item::path(
            Path::circle(mm(15, 15), Nm::from_mm(25)),
            None,
            Some(Brush::Solid(Color::from_rgb8(200, 30, 30))),
        )],
    ));
    items.push(Item::group(
        Some(Transform::new(mm(240, 160), Orient2::R90, ew_units::Angle2::ZERO)),
        None,
        vec![
            Item::glyphs(Arc::new(text_run("-K1", mm(0, 0)))),
            Item::path(
                Path::line(mm(0, 5), mm(25, 5)),
                Some(Stroke::pen(Pen::P042)),
                None,
            ),
        ],
    ));
    DisplayList { items }
}

fn region(dpi: u32, x: i64, y: i64, width: u32, height: u32) -> RegionOptions {
    RegionOptions {
        dpi,
        x,
        y,
        width,
        height,
        background: Color::WHITE,
        surround: SURROUND,
    }
}

fn full(list: &DisplayList, dpi: u32) -> RgbaImage {
    let options = RenderOptions {
        dpi,
        ..RenderOptions::default()
    };
    render(list, a4(), &options, &bundled_fonts(), None).unwrap().0
}

fn cut(list: &DisplayList, bounds: Option<&ItemBounds>, o: &RegionOptions) -> RgbaImage {
    render_region(list, bounds, a4(), o, &bundled_fonts(), None)
        .unwrap()
        .0
}

#[test]
fn the_full_rectangle_is_render_byte_for_byte() {
    let list = page();
    let bounds = ItemBounds::of(&list, None);
    for dpi in [72, 100, 150, 192] {
        let (w, h) = pixel_size(a4(), dpi);
        let whole = full(&list, dpi);
        let o = region(dpi, 0, 0, w as u32, h as u32);
        let (with_bounds, report) =
            render_region(&list, Some(&bounds), a4(), &o, &bundled_fonts(), None).unwrap();
        assert_eq!(with_bounds, whole, "with bounds at {dpi} dpi");
        assert_eq!(cut(&list, None, &o), whole, "without bounds at {dpi} dpi");
        assert!(report.missing_fonts.is_empty());
    }
}

/// The share of pixels (per cent) where a channel differs by more than 16, and the largest channel difference.
fn compare(a: &[u8], b: &[u8]) -> (f64, u8) {
    assert_eq!(a.len(), b.len());
    let mut bad = 0usize;
    let mut largest = 0u8;
    for (p, q) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        let d = p.iter().zip(q).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        largest = largest.max(d);
        bad += usize::from(d > 16);
    }
    (100.0 * bad as f64 / (a.len() / 4) as f64, largest)
}

#[test]
fn a_three_by_three_stitch_matches_the_full_raster_within_the_doc_20_tolerance() {
    let list = page();
    let bounds = ItemBounds::of(&list, None);
    let dpi = 192;
    let whole = full(&list, dpi);
    // Nine 256-pixel tiles around the circle and the rectangle, stitched into one image.
    let (x0, y0) = (512i64, 512i64);
    let side = 3 * 256u32;
    let mut stitched = vec![0u8; (side * side * 4) as usize];
    for ty in 0..3u32 {
        for tx in 0..3u32 {
            let o = region(dpi, x0 + i64::from(tx * 256), y0 + i64::from(ty * 256), 256, 256);
            let tile = cut(&list, Some(&bounds), &o);
            for row in 0..256u32 {
                let from = (row * 256 * 4) as usize;
                let to = (((ty * 256 + row) * side + tx * 256) * 4) as usize;
                stitched[to..to + 1024].copy_from_slice(&tile.pixels[from..from + 1024]);
            }
        }
    }
    let mut crop = Vec::with_capacity(stitched.len());
    for row in 0..side {
        let from = (((y0 as u32 + row) * whole.width + x0 as u32) * 4) as usize;
        crop.extend_from_slice(&whole.pixels[from..from + (side * 4) as usize]);
    }
    let (percent, largest) = compare(&stitched, &crop);
    println!(
        "3 × 3 stitch at {dpi} dpi: {percent:.4} % of the pixels differ by more than 16, largest difference {largest}"
    );
    assert!(percent <= 0.5, "{percent:.4} % differ");
}

#[test]
fn pixels_outside_the_sheet_are_the_surround() {
    let list = page();
    let dpi = 100;
    let (w, h) = pixel_size(a4(), dpi);
    let whole = full(&list, dpi);
    // A region hanging over the bottom-right corner: the blue rectangle runs off the sheet there.
    let o = region(dpi, w as i64 - 100, h as i64 - 60, 300, 200);
    let img = cut(&list, None, &o);
    let surround = [SURROUND.r, SURROUND.g, SURROUND.b, SURROUND.a];
    for j in 0..200u32 {
        for i in 0..300u32 {
            let (sx, sy) = (w as i64 - 100 + i64::from(i), h as i64 - 60 + i64::from(j));
            let got = pixel(&img, i, j);
            if sx < w as i64 && sy < h as i64 {
                // Inside: the full raster's pixel (here byte-identical: the shift is a whole number of pixels
                // and the geometry is axis-aligned).
                let want = pixel(&whole, sx as u32, sy as u32);
                assert!(
                    got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 16),
                    "({i}, {j}): {got:?} vs {want:?}"
                );
            } else {
                assert_eq!(got, surround, "({i}, {j}) is outside the sheet");
            }
        }
    }
    // A region left of and above the sheet.
    let img = cut(&list, None, &region(dpi, -40, -30, 100, 100));
    assert_eq!(pixel(&img, 0, 0), surround);
    assert_eq!(pixel(&img, 39, 50), surround);
    assert_eq!(pixel(&img, 50, 29), surround);
    assert_eq!(
        pixel(&img, 40, 30),
        [255, 255, 255, 255],
        "the sheet's first pixel"
    );
}

#[test]
fn culling_never_changes_a_pixel() {
    let list = page();
    let bounds = ItemBounds::of(&list, None);
    let mut rng = Lcg(0x5EED);
    for dpi in [36, 150, 600, 3072] {
        let (w, h) = pixel_size(a4(), dpi);
        for _ in 0..6 {
            let x = rng.below(w + 200) as i64 - 100;
            let y = rng.below(h + 200) as i64 - 100;
            let o = region(dpi, x, y, 256, 256);
            assert_eq!(
                cut(&list, Some(&bounds), &o),
                cut(&list, None, &o),
                "{dpi} dpi at ({x}, {y})"
            );
        }
    }
    // Bounds of another list are ignored, not misapplied.
    let other = ItemBounds::of(&DisplayList::new(), None);
    let o = region(150, 0, 0, 512, 512);
    assert_eq!(cut(&list, Some(&other), &o), cut(&list, None, &o));
}

#[test]
fn a_region_of_a_sheet_beyond_the_full_raster_limit_renders() {
    // At 24 576 dpi the A4 sheet is 287 362 pixels wide, beyond `render`'s u16 limit; a region still renders.
    let list = page();
    let dpi = 24_576;
    let (w, _) = pixel_size(a4(), dpi);
    assert!(w > u64::from(u16::MAX));
    let bounds = ItemBounds::of(&list, None);
    // The 0.26 mm line at y = 20 mm, x = 150 mm: row 20 mm × 24 576 / 25.4 = 19 351.
    let row = (20.0 * f64::from(dpi) / 25.4).round() as i64;
    let col = (150.0 * f64::from(dpi) / 25.4).round() as i64;
    let img = cut(&list, Some(&bounds), &region(dpi, col, row - 128, 256, 256));
    assert!(common::is_dark(pixel(&img, 128, 128)), "the line's middle is ink");
    assert_eq!(pixel(&img, 128, 0), [255, 255, 255, 255], "paper above the line");
}

#[test]
fn region_size_errors() {
    let list = page();
    let fonts = bundled_fonts();
    for (w, h) in [(0, 10), (10, 0), (4097, 10), (10, 4097)] {
        assert_eq!(
            render_region(&list, None, a4(), &region(150, 0, 0, w, h), &fonts, None),
            Err(RenderError::Size {
                width: u64::from(w),
                height: u64::from(h)
            })
        );
    }
    assert!(render_region(&list, None, a4(), &region(150, 0, 0, 4096, 4096), &fonts, None).is_ok());
    assert_eq!(
        render_region(&list, None, a4(), &region(0, 0, 0, 10, 10), &fonts, None),
        Err(RenderError::Size { width: 0, height: 0 })
    );
    let flat = SheetSize {
        width: Nm(0),
        height: Nm::from_mm(10),
    };
    assert!(matches!(
        render_region(&list, None, flat, &region(150, 0, 0, 10, 10), &fonts, None),
        Err(RenderError::Size { width: 0, .. })
    ));
}

#[test]
fn item_bounds_cover_what_each_item_draws() {
    // One item per region of the page; a region that misses every inflated box renders as plain paper even
    // without culling, so the boxes are not too small where it matters: compare against the unculled render.
    let p = |x: i64, y: i64| Pt2::new(Nm::from_mm(x), Nm::from_mm(y));
    let list = DisplayList {
        items: vec![
            Item::path(
                Path::line(p(10, 10), p(10, 10)),
                Some(Stroke::pen(Pen::P052)),
                None,
            ),
            Item::path(Path::new(), Some(Stroke::pen(Pen::P052)), None),
            Item::glyphs(Arc::new(text_run("Wg", p(50, 50)))),
        ],
    };
    let bounds = ItemBounds::of(&list, None);
    for dpi in [96, 1200] {
        let (w, h) = pixel_size(a4(), dpi);
        let mut y = 0i64;
        while y < h as i64 {
            let mut x = 0i64;
            while x < w as i64 {
                let o = region(dpi, x, y, 512, 512);
                assert_eq!(
                    cut(&list, Some(&bounds), &o),
                    cut(&list, None, &o),
                    "{dpi} dpi at ({x}, {y})"
                );
                x += 512;
            }
            y += 512;
            if dpi > 100 && y > 4096 {
                break;
            }
        }
    }
}
