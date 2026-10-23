// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rendering tests: sizes, lines, determinism, text, instances, dashes, errors, hostile input.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{Lcg, a4, bundled_fonts, is_dark, mm, pixel, sha256, text_run};
use ew_dl::{
    Brush, Color, DashPattern, DisplayList, FontKey, Item, Path, Pen, ShapedRun, Stroke, SymbolGeomId,
    Transform,
};
use ew_render_cpu::{
    FontBytes, RenderError, RenderOptions, RenderReport, RgbaImage, SheetSize, encode_png, pixel_size, render,
};
use ew_units::{Nm, Orient2, Pt2, Rect};

fn sym(extra: u16) -> SymbolGeomId {
    SymbolGeomId {
        symbol: Default::default(),
        variant: Orient2::R0,
        extra: Some(extra),
    }
}

fn render_a4(list: &DisplayList, dpi: u32) -> (RgbaImage, RenderReport) {
    let options = RenderOptions {
        dpi,
        ..RenderOptions::default()
    };
    render(list, a4(), &options, &bundled_fonts(), None).expect("renders")
}

#[test]
fn pixel_sizes() {
    assert_eq!(pixel_size(a4(), 150), (1754, 1240));
    assert_eq!(pixel_size(a4(), 72), (842, 595));
    let tiny = SheetSize {
        width: Nm(1),
        height: Nm(1),
    };
    assert_eq!(pixel_size(tiny, 1), (1, 1));
    let zero = SheetSize {
        width: Nm(0),
        height: Nm(-5),
    };
    assert_eq!(pixel_size(zero, 300), (1, 1));
    // Half rounds up: 1.5 px is 2, just below is 1, 0.5 px is 1 (the minimum anyway).
    let sheet = |nm| SheetSize {
        width: Nm(nm),
        height: Nm(nm),
    };
    assert_eq!(pixel_size(sheet(38_100_000), 1), (2, 2));
    assert_eq!(pixel_size(sheet(38_099_999), 1), (1, 1));
    assert_eq!(pixel_size(sheet(12_700_000), 1), (1, 1));
}

#[test]
fn horizontal_line() {
    let list = DisplayList {
        items: vec![Item::path(
            Path::line(mm(10, 100), mm(287, 100)),
            Some(Stroke::pen(Pen::P026)),
            None,
        )],
    };
    let (img, report) = render_a4(&list, 150);
    assert_eq!((img.width, img.height), (1754, 1240));
    assert_eq!(img.pixels.len(), 1754 * 1240 * 4);
    assert_eq!(report, RenderReport::default());
    // 100 mm = 590.55 px; the 1.54 px wide line covers row 590 completely.
    let on = pixel(&img, 900, 590);
    assert!(is_dark(on), "{on:?}");
    assert_eq!(on[3], 255);
    assert_eq!(pixel(&img, 900, 200), [255, 255, 255, 255]);
    assert_eq!(pixel(&img, 900, 800), [255, 255, 255, 255]);
    // Left of the line's start (10 mm = 59 px).
    assert_eq!(pixel(&img, 20, 590), [255, 255, 255, 255]);
    // Rows a few pixels away are clean.
    assert_eq!(pixel(&img, 900, 586), [255, 255, 255, 255]);
    assert_eq!(pixel(&img, 900, 595), [255, 255, 255, 255]);
}

#[test]
fn hairline_is_drawn_not_clamped() {
    // 0.13 mm at 72 dpi is 0.37 px: faint but present, and never a full pixel.
    let list = DisplayList {
        items: vec![Item::path(
            Path::line(mm(10, 100), mm(287, 100)),
            Some(Stroke::pen(Pen::P013)),
            None,
        )],
    };
    let (img, _) = render_a4(&list, 72);
    // 100 mm = 283.5 px at 72 dpi.
    let darkest = (280..287).map(|y| pixel(&img, 400, y)[0]).min().expect("rows");
    assert!(darkest < 255, "hairline vanished");
    assert!(darkest > 100, "hairline was widened to a full pixel: {darkest}");
}

/// A page with every kind of item the determinism test needs.
fn busy_page() -> DisplayList {
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
    DisplayList { items }
}

#[test]
fn deterministic_pixels_and_png() {
    let list = busy_page();
    let (a, _) = render_a4(&list, 100);
    let (b, _) = render_a4(&list, 100);
    assert_eq!(sha256(&a.pixels), sha256(&b.pixels));
    assert_eq!(a, b);
    let pa = encode_png(&a, Some(100));
    let pb = encode_png(&b, Some(100));
    assert_eq!(sha256(&pa), sha256(&pb));
    assert!(pa.starts_with(b"\x89PNG"));
    // The page is not blank: it has ink.
    assert!(
        a.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| is_dark([p[0], p[1], p[2], p[3]]))
    );
}

#[test]
fn clip_hides_what_is_outside() {
    let list = DisplayList {
        items: vec![Item::group(
            None,
            Some(Rect::from_points(mm(10, 10), mm(30, 30))),
            vec![Item::path(
                Path::rect(Rect::from_points(mm(0, 0), mm(100, 100))),
                None,
                Some(Brush::BLACK),
            )],
        )],
    };
    let (img, _) = render_a4(&list, 100);
    // 20 mm = 79 px inside; 50 mm = 197 px outside the clip.
    assert!(is_dark(pixel(&img, 79, 79)));
    assert_eq!(pixel(&img, 197, 197), [255, 255, 255, 255]);
    assert_eq!(pixel(&img, 5, 5), [255, 255, 255, 255]);
}

#[test]
fn text_stays_inside_its_layout_quad() {
    let dpi = 300;
    let run = text_run("HHHH", mm(40, 60));
    let quad = run.layout_quad();
    let list = DisplayList {
        items: vec![Item::glyphs(Arc::new(run))],
    };
    let (img, report) = render_a4(&list, dpi);
    assert!(report.missing_fonts.is_empty());

    let to_px = |n: Nm| n.0 as f64 * f64::from(dpi) / 25_400_000.0;
    let (qx0, qx1) = (
        quad.iter().map(|p| to_px(p.x)).fold(f64::MAX, f64::min),
        quad.iter().map(|p| to_px(p.x)).fold(f64::MIN, f64::max),
    );
    let (qy0, qy1) = (
        quad.iter().map(|p| to_px(p.y)).fold(f64::MAX, f64::min),
        quad.iter().map(|p| to_px(p.y)).fold(f64::MIN, f64::max),
    );

    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..img.height {
        for x in 0..img.width {
            if is_dark(pixel(&img, x, y)) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    assert!(x0 <= x1, "no ink");
    let (x0, y0, x1, y1) = (
        f64::from(x0),
        f64::from(y0),
        f64::from(x1) + 1.0,
        f64::from(y1) + 1.0,
    );
    assert!(x0 >= qx0 - 2.0 && x1 <= qx1 + 2.0, "x {x0}..{x1} vs {qx0}..{qx1}");
    assert!(y0 >= qy0 - 2.0 && y1 <= qy1 + 2.0, "y {y0}..{y1} vs {qy0}..{qy1}");
    // The ink fills a good part of the box: four letters wide, cap height tall.
    assert!(x1 - x0 > 0.6 * (qx1 - qx0), "ink too narrow");
    assert!(y1 - y0 > 0.3 * (qy1 - qy0), "ink too short");
}

#[test]
fn rotated_and_coloured_text() {
    let mut run = text_run("HHHH", mm(100, 100));
    run.transform = Transform::from_orient(Orient2::R90);
    run.brush = Brush::Solid(Color::from_rgb8(255, 0, 0));
    let quad = run.layout_quad();
    let list = DisplayList {
        items: vec![Item::glyphs(Arc::new(run))],
    };
    let (img, _) = render_a4(&list, 150);
    let to_px = |n: Nm| n.0 as f64 * 150.0 / 25_400_000.0;
    let mut red = 0;
    for y in 0..img.height {
        for x in 0..img.width {
            let p = pixel(&img, x, y);
            if p[0] > 200 && p[1] < 80 && p[2] < 80 {
                red += 1;
                let (fx, fy) = (f64::from(x), f64::from(y));
                let inside = fx >= quad.iter().map(|q| to_px(q.x)).fold(f64::MAX, f64::min) - 2.0
                    && fx <= quad.iter().map(|q| to_px(q.x)).fold(f64::MIN, f64::max) + 2.0
                    && fy >= quad.iter().map(|q| to_px(q.y)).fold(f64::MAX, f64::min) - 2.0
                    && fy <= quad.iter().map(|q| to_px(q.y)).fold(f64::MIN, f64::max) + 2.0;
                assert!(inside, "red pixel outside the quad at {x},{y}");
            }
        }
    }
    assert!(red > 50, "rotated red text did not render ({red} px)");
}

#[test]
fn missing_font_is_skipped_and_reported_once() {
    let ghost = FontKey {
        sha256: [7; 32],
        face_index: 0,
    };
    let mut a: ShapedRun = text_run("HHHH", mm(40, 60));
    a.font = ghost;
    let mut b = text_run("HH", mm(40, 100));
    b.font = ghost;
    let list = DisplayList {
        items: vec![
            Item::glyphs(Arc::new(a)),
            Item::glyphs(Arc::new(b)),
            Item::glyphs(Arc::new(text_run("HHHH", mm(40, 140)))),
        ],
    };
    let (img, report) = render_a4(&list, 100);
    assert_eq!(report.missing_fonts, vec![ghost]);
    // Only the third run (with a real font) left ink: 140 mm = 551 px baseline, 60 mm = 236 px.
    let ink_rows = |lo: u32, hi: u32| (lo..hi).any(|y| (0..img.width).any(|x| is_dark(pixel(&img, x, y))));
    assert!(!ink_rows(200, 260));
    assert!(!ink_rows(360, 420));
    assert!(ink_rows(500, 570));
}

#[test]
fn garbage_font_bytes_do_not_panic() {
    let key = FontKey {
        sha256: [9; 32],
        face_index: 0,
    };
    let mut fonts: BTreeMap<FontKey, FontBytes> = BTreeMap::new();
    fonts.insert(
        key,
        FontBytes {
            data: Arc::from(vec![0u8; 64]),
            index: 3,
        },
    );
    let mut run = text_run("HHHH", mm(40, 60));
    run.font = key;
    let list = DisplayList {
        items: vec![Item::glyphs(Arc::new(run))],
    };
    let (img, report) = render(&list, a4(), &RenderOptions::default(), &fonts, None).expect("renders");
    assert_eq!(pixel(&img, 10, 10), [255, 255, 255, 255]);
    // A file the glyph machinery cannot open counts as a missing font.
    assert_eq!(report.missing_fonts, vec![key]);
}

#[test]
fn invalid_font_bytes_are_validated_once_per_key_and_reported_missing() {
    let key = FontKey {
        sha256: [0xAB; 32],
        face_index: 0,
    };
    let mut fonts: BTreeMap<FontKey, FontBytes> = BTreeMap::new();
    fonts.insert(
        key,
        FontBytes {
            data: Arc::from(vec![0xABu8; 64]),
            index: 0,
        },
    );
    let mut a = text_run("HHHH", mm(40, 60));
    a.font = key;
    let mut b = text_run("HH", mm(40, 100));
    b.font = key;
    let list = DisplayList {
        items: vec![Item::glyphs(Arc::new(a)), Item::glyphs(Arc::new(b))],
    };
    let (img, report) = render(&list, a4(), &RenderOptions::default(), &fonts, None).expect("renders");
    assert_eq!(
        report.missing_fonts,
        vec![key],
        "one entry for two runs of the same bad key"
    );
    assert!(!(0..img.height).any(|y| (0..img.width).any(|x| is_dark(pixel(&img, x, y)))));
}

#[test]
fn instances_draw_translated_and_unresolved_are_counted() {
    let geom = DisplayList {
        items: vec![Item::path(
            Path::rect(Rect::from_points(mm(-5, -5), mm(5, 5))),
            None,
            Some(Brush::BLACK),
        )],
    };
    let mut lib: BTreeMap<SymbolGeomId, Arc<DisplayList>> = BTreeMap::new();
    lib.insert(sym(1), Arc::new(geom));
    let list = DisplayList {
        items: vec![
            Item::instance(sym(1), Transform::from_translation(mm(100, 100))),
            Item::instance(sym(2), Transform::from_translation(mm(200, 100))),
        ],
    };
    let options = RenderOptions {
        dpi: 100,
        ..RenderOptions::default()
    };
    let (img, report) = render(&list, a4(), &options, &bundled_fonts(), Some(&lib)).unwrap();
    assert_eq!(report.unresolved_instances, 1);
    // 100 mm = 394 px.
    assert!(is_dark(pixel(&img, 394, 394)));
    assert_eq!(pixel(&img, 10, 10), [255, 255, 255, 255]);
    // 200 mm = 787 px: the unresolved instance left nothing.
    assert_eq!(pixel(&img, 787, 394), [255, 255, 255, 255]);

    // Without a resolver both are skipped and counted.
    let (img, report) = render(&list, a4(), &options, &bundled_fonts(), None).unwrap();
    assert_eq!(report.unresolved_instances, 2);
    assert_eq!(pixel(&img, 394, 394), [255, 255, 255, 255]);
}

#[test]
fn cyclic_instances_terminate() {
    let looping = DisplayList {
        items: vec![
            Item::path(
                Path::rect(Rect::from_points(mm(0, 0), mm(2, 2))),
                None,
                Some(Brush::BLACK),
            ),
            Item::instance(sym(9), Transform::from_translation(mm(3, 0))),
        ],
    };
    let mut lib: BTreeMap<SymbolGeomId, Arc<DisplayList>> = BTreeMap::new();
    lib.insert(sym(9), Arc::new(looping));
    let list = DisplayList {
        items: vec![Item::instance(sym(9), Transform::from_translation(mm(10, 10)))],
    };
    let (_, report) = render(
        &list,
        a4(),
        &RenderOptions::default(),
        &bundled_fonts(),
        Some(&lib),
    )
    .unwrap();
    // The recursion stops at the depth limit, the last level is counted as unresolved.
    assert_eq!(report.unresolved_instances, 1);
}

/// A PNG of `w × h` RGBA pixels (synthetic).
fn png_bytes(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
    writer.finish().unwrap();
    out
}

fn image_item(sha: u8, format: ew_dl::ImageFormat, bytes: &[u8]) -> Item {
    Item::image(
        Arc::new(ew_dl::ImageData {
            sha256: [sha; 32],
            format,
            bytes: Arc::from(bytes),
        }),
        Rect::from_points(mm(10, 10), mm(50, 50)),
    )
}

/// FT-384: a PNG image is decoded and stretched over its rectangle (a 2 × 1 image: red left, blue right); a JPEG (no
/// decoder in this renderer's graph), an image without bytes and bytes that do not decode are counted, not drawn.
#[test]
fn png_images_are_drawn_and_the_rest_counted() {
    let png = png_bytes(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]);
    let list = DisplayList {
        items: vec![image_item(1, ew_dl::ImageFormat::Png, &png)],
    };
    let (img, report) = render_a4(&list, 72);
    assert_eq!(report.skipped_images, 0);
    // 10 mm to 50 mm at 72 dpi: pixels 28 to 142; the halves meet at 85.
    assert_eq!(pixel(&img, 50, 80), [255, 0, 0, 255], "the left pixel");
    assert_eq!(pixel(&img, 120, 80), [0, 0, 255, 255], "the right pixel");
    assert_eq!(pixel(&img, 20, 80), [255, 255, 255, 255], "outside the rectangle");
    assert_eq!(pixel(&img, 50, 150), [255, 255, 255, 255], "below the rectangle");
    for (format, bytes) in [
        (ew_dl::ImageFormat::Jpeg, &[0xff, 0xd8, 0xff, 0xe0][..]),
        (ew_dl::ImageFormat::Png, &[][..]),
        (
            ew_dl::ImageFormat::Png,
            &[0x89, 0x50, 0x4e, 0x47, 13, 10, 0x1a, 10, 0x78][..],
        ),
    ] {
        let list = DisplayList {
            items: vec![image_item(2, format, bytes)],
        };
        let (img, report) = render_a4(&list, 72);
        assert_eq!(report.skipped_images, 1, "{format:?}");
        assert_eq!(pixel(&img, 50, 80), [255, 255, 255, 255]);
    }
    // A PNG larger than the pixel bound is skipped (its header alone says so).
    let mut huge = png_bytes(1, 1, &[0, 0, 0, 255]);
    huge[16..20].copy_from_slice(&5_000u32.to_be_bytes());
    huge[20..24].copy_from_slice(&5_000u32.to_be_bytes());
    let list = DisplayList {
        items: vec![image_item(3, ew_dl::ImageFormat::Png, &huge)],
    };
    assert_eq!(render_a4(&list, 72).1.skipped_images, 1);
}

#[test]
fn dashes_alternate_four_to_two() {
    // A 100 mm × 20 mm sheet at 600 dpi; a 1 mm pen dashed 4:2 = 94.49 px on, 47.24 px off.
    let sheet = SheetSize {
        width: Nm::from_mm(100),
        height: Nm::from_mm(20),
    };
    let list = DisplayList {
        items: vec![Item::path(
            Path::line(mm(0, 10), mm(100, 10)),
            Some(Stroke::solid(Nm::from_mm(1)).dashed(DashPattern::Dash42)),
            None,
        )],
    };
    let options = RenderOptions {
        dpi: 600,
        ..RenderOptions::default()
    };
    let (img, _) = render(&list, sheet, &options, &bundled_fonts(), None).unwrap();
    let y = 236; // 10 mm = 236.2 px
    let mut runs: Vec<(bool, u32)> = Vec::new();
    for x in 0..img.width {
        let dark = is_dark(pixel(&img, x, y));
        match runs.last_mut() {
            Some((d, n)) if *d == dark => *n += 1,
            _ => runs.push((dark, 1)),
        }
    }
    assert!(runs.len() >= 8, "{runs:?}");
    // The first run is a dash, the last is cut by the sheet edge; check the ones between.
    for &(dark, n) in &runs[1..runs.len() - 1] {
        let want = if dark { 94.49 } else { 47.24 };
        assert!(
            (f64::from(n) - want).abs() <= 1.0,
            "{} run of {n} px, want {want}: {runs:?}",
            if dark { "dark" } else { "white" }
        );
    }
}

#[test]
fn degenerate_dashes_draw_solid() {
    let mut stroke = Stroke::solid(Nm::from_mm(1));
    stroke.dash = Some(ew_dl::Dash {
        segments: vec![Nm(0), Nm(0)],
        offset: Nm(0),
    });
    let list = DisplayList {
        items: vec![Item::path(
            Path::line(mm(10, 10), mm(100, 10)),
            Some(stroke),
            None,
        )],
    };
    let (img, _) = render_a4(&list, 100);
    assert!(is_dark(pixel(&img, 200, 39)));
}

#[test]
fn background_and_alpha() {
    let options = RenderOptions {
        dpi: 50,
        background: Color::from_rgba8(10, 20, 30, 0),
    };
    let (img, _) = render(&DisplayList::new(), a4(), &options, &bundled_fonts(), None).unwrap();
    assert_eq!(pixel(&img, 3, 3), [0, 0, 0, 0]);

    let options = RenderOptions {
        dpi: 50,
        background: Color::from_rgb8(10, 20, 30),
    };
    let list = DisplayList {
        items: vec![Item::path(
            Path::rect(Rect::from_points(mm(0, 0), mm(100, 100))),
            None,
            Some(Brush::Solid(Color::from_rgba8(255, 0, 0, 128))),
        )],
    };
    let (img, _) = render(&list, a4(), &options, &bundled_fonts(), None).unwrap();
    let p = pixel(&img, 10, 10);
    assert_eq!(p[3], 255);
    assert!((i32::from(p[0]) - 133).abs() <= 2, "{p:?}");
    assert!((i32::from(p[1]) - 10).abs() <= 2, "{p:?}");
    assert_eq!(pixel(&img, 600, 400), [10, 20, 30, 255]);
}

#[test]
fn size_errors() {
    let fonts = bundled_fonts();
    let empty = DisplayList::new();
    let big = SheetSize {
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
    };
    // 297 mm at 10 000 dpi is 116 929 px.
    let options = RenderOptions {
        dpi: 10_000,
        ..RenderOptions::default()
    };
    assert_eq!(
        render(&empty, big, &options, &fonts, None).unwrap_err(),
        RenderError::Size {
            width: 116_929,
            height: 82_677
        }
    );
    // One axis too large is enough.
    let tall = SheetSize {
        width: Nm::from_mm(10),
        height: Nm::from_mm(20_000),
    };
    assert!(matches!(
        render(&empty, tall, &RenderOptions::default(), &fonts, None),
        Err(RenderError::Size { .. })
    ));
    // Zero and negative sheets, and dpi 0.
    for sheet in [
        SheetSize {
            width: Nm(0),
            height: Nm(0),
        },
        SheetSize {
            width: Nm::from_mm(10),
            height: Nm(0),
        },
        SheetSize {
            width: Nm(-1),
            height: Nm::from_mm(10),
        },
    ] {
        let err = render(&empty, sheet, &RenderOptions::default(), &fonts, None).unwrap_err();
        assert!(matches!(err, RenderError::Size { .. }), "{err:?}");
        assert!(!err.to_string().is_empty());
    }
    let zero_dpi = RenderOptions {
        dpi: 0,
        ..RenderOptions::default()
    };
    assert!(render(&empty, a4(), &zero_dpi, &fonts, None).is_err());
}

#[test]
fn hostile_many_items() {
    let mut rng = Lcg(0x5EED);
    let mut items = Vec::new();
    for _ in 0..10_000 {
        // Starts on and around the page, runs up to 40 mm in any direction.
        let c = |rng: &mut Lcg| i64::try_from(rng.below(320_000_000)).unwrap() - 10_000_000;
        let d = |rng: &mut Lcg| i64::try_from(rng.below(80_000_000)).unwrap() - 40_000_000;
        let a = Pt2::new(Nm(c(&mut rng)), Nm(c(&mut rng)));
        let b = Pt2::new(Nm(a.x.0 + d(&mut rng)), Nm(a.y.0 + d(&mut rng)));
        let width = Nm(i64::try_from(rng.below(2_000_000)).unwrap());
        items.push(Item::path(Path::line(a, b), Some(Stroke::solid(width)), None));
    }
    for i in 0..200 {
        let origin = Pt2::new(
            Nm(i64::try_from(rng.below(300_000_000)).unwrap()),
            Nm(i64::try_from(rng.below(210_000_000)).unwrap()),
        );
        let mut run = text_run("Page 101 Hostile", origin);
        if i % 3 == 0 {
            run.transform = Transform::from_orient(Orient2::R90);
        }
        items.push(Item::glyphs(Arc::new(run)));
    }
    let list = DisplayList { items };
    let (img, report) = render_a4(&list, 72);
    assert_eq!((img.width, img.height), (842, 595));
    assert_eq!(report, RenderReport::default());
}

#[test]
fn deep_group_nesting_does_not_overflow() {
    let mut item = Item::path(
        Path::rect(Rect::from_points(mm(10, 10), mm(20, 20))),
        None,
        Some(Brush::BLACK),
    );
    for _ in 0..5_000 {
        item = Item::group(None, None, vec![item]);
    }
    let list = DisplayList { items: vec![item] };
    // Dropping a list this deep recurses too, so run on a roomy stack; the renderer itself gives up
    // on the branch beyond its depth limit.
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let (img, _) = render_a4(&list, 50);
            assert_eq!(img.width, 585);
        })
        .unwrap();
    handle.join().unwrap();
}
