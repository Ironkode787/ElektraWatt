// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Monochrome mapping and PNG encoding tests.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{a4, bundled_fonts, is_dark, mm, pixel, text_run};
use ew_dl::{
    Brush, Color, DisplayList, EntityRef, Item, ItemKind, Path, Pen, Stroke, SymbolGeomId, Transform,
};
use ew_render_cpu::{RenderOptions, RgbaImage, encode_png, monochrome, render, render_monochrome};
use ew_units::{Orient2, Rect};

fn solid(c: Color) -> Brush {
    Brush::Solid(c)
}

fn rect_path() -> Path {
    Path::rect(Rect::from_points(mm(0, 0), mm(10, 10)))
}

fn path_of(item: &Item) -> &ew_dl::PathItem {
    match &item.kind {
        ItemKind::Path(p) => p,
        other => panic!("not a path: {other:?}"),
    }
}

fn group_of(item: &Item) -> &ew_dl::GroupItem {
    match &item.kind {
        ItemKind::Group(g) => g,
        other => panic!("not a group: {other:?}"),
    }
}

#[test]
fn monochrome_maps_strokes_fills_and_glyphs() {
    let red = Color::from_rgb8(255, 0, 0);
    let mut glyph_run = text_run("HH", mm(10, 10));
    glyph_run.brush = solid(red);
    let nested = Item::group(
        Some(Transform::from_translation(mm(5, 5))),
        Some(Rect::from_points(mm(0, 0), mm(50, 50))),
        vec![
            Item::path(
                rect_path(),
                Some(Stroke::pen(Pen::P026).with_brush(solid(Color::from_rgba8(0, 0, 255, 100)))),
                Some(solid(Color::from_rgba8(255, 128, 0, 77))),
            ),
            Item::group(
                None,
                None,
                vec![Item::path(
                    rect_path(),
                    Some(Stroke::pen(Pen::P013).with_brush(solid(red))),
                    Some(solid(Color::from_hex("#C0C0C0").unwrap())),
                )],
            ),
        ],
    );
    let entity = EntityRef(Default::default());
    let list = DisplayList {
        items: vec![
            Item::path(
                rect_path(),
                Some(Stroke::pen(Pen::P026).with_brush(solid(red))),
                Some(solid(Color::from_hex("#C0C0C0").unwrap())),
            )
            .with_entity(entity),
            Item::path(rect_path(), None, Some(solid(Color::WHITE))),
            Item::path(
                rect_path(),
                None,
                Some(solid(Color::from_rgba8(255, 255, 255, 40))),
            ),
            Item::glyphs(Arc::new(glyph_run.clone())),
            nested,
        ],
    };
    let mono = monochrome(&list);
    assert_eq!(mono.items.len(), list.items.len());

    let p0 = path_of(&mono.items[0]);
    assert_eq!(p0.stroke.as_ref().unwrap().brush, solid(Color::BLACK));
    // The width and the rest of the stroke survive.
    assert_eq!(p0.stroke.as_ref().unwrap().width, Pen::P026.width());
    assert_eq!(p0.fill, Some(solid(Color::from_rgb8(192, 192, 192))));
    assert_eq!(mono.items[0].entity, Some(entity));
    assert_eq!(p0.path, rect_path());

    // White stays white, with its alpha.
    assert_eq!(path_of(&mono.items[1]).fill, Some(solid(Color::WHITE)));
    assert_eq!(
        path_of(&mono.items[2]).fill,
        Some(solid(Color::from_rgba8(255, 255, 255, 40)))
    );

    match &mono.items[3].kind {
        ItemKind::Glyphs(run) => {
            assert_eq!(run.brush, solid(Color::BLACK));
            assert_eq!(run.glyphs, glyph_run.glyphs);
            assert_eq!(run.origin, glyph_run.origin);
        }
        other => panic!("{other:?}"),
    }

    let g = group_of(&mono.items[4]);
    assert_eq!(g.transform, Some(Transform::from_translation(mm(5, 5))));
    assert_eq!(g.clip, Some(Rect::from_points(mm(0, 0), mm(50, 50))));
    let inner = path_of(&g.items[0]);
    // Alpha is kept: stroke black with alpha 100; orange fill grey (luminance) with alpha 77.
    assert_eq!(
        inner.stroke.as_ref().unwrap().brush,
        solid(Color::from_rgba8(0, 0, 0, 100))
    );
    // (2126 * 255 + 7152 * 128 + 5000) / 10000 = 146.
    assert_eq!(inner.fill, Some(solid(Color::from_rgba8(146, 146, 146, 77))));
    let deep = path_of(&group_of(&g.items[1]).items[0]);
    assert_eq!(deep.stroke.as_ref().unwrap().brush, solid(Color::BLACK));
    assert_eq!(deep.fill, Some(solid(Color::from_rgb8(192, 192, 192))));

    // The original list is untouched and the mapping is idempotent.
    assert_eq!(path_of(&list.items[0]).stroke.as_ref().unwrap().brush, solid(red));
    assert_eq!(monochrome(&mono), mono);
}

#[test]
fn monochrome_luminance_extremes() {
    let list = DisplayList {
        items: vec![
            Item::path(rect_path(), None, Some(solid(Color::BLACK))),
            Item::path(rect_path(), None, Some(solid(Color::from_rgb8(254, 255, 255)))),
            Item::path(rect_path(), None, Some(solid(Color::from_rgb8(0, 255, 0)))),
        ],
    };
    let mono = monochrome(&list);
    assert_eq!(path_of(&mono.items[0]).fill, Some(solid(Color::BLACK)));
    // Not exactly white: the luminance 254.99 rounds to 255 grey, which paints as white anyway.
    assert_eq!(
        path_of(&mono.items[1]).fill,
        Some(solid(Color::from_rgb8(255, 255, 255)))
    );
    assert_eq!(
        path_of(&mono.items[2]).fill,
        Some(solid(Color::from_rgb8(182, 182, 182)))
    );
}

#[test]
fn render_monochrome_inlines_instances() {
    let id = SymbolGeomId {
        symbol: Default::default(),
        variant: Orient2::R0,
        extra: Some(1),
    };
    let geom = DisplayList {
        items: vec![Item::path(
            Path::rect(Rect::from_points(mm(-5, -5), mm(5, 5))),
            None,
            Some(solid(Color::from_rgb8(255, 0, 0))),
        )],
    };
    let mut lib: BTreeMap<SymbolGeomId, Arc<DisplayList>> = BTreeMap::new();
    lib.insert(id, Arc::new(geom));
    let other = SymbolGeomId { extra: Some(2), ..id };
    let list = DisplayList {
        items: vec![
            Item::instance(id, Transform::from_translation(mm(100, 100))),
            Item::instance(other, Transform::from_translation(mm(200, 100))),
        ],
    };
    let options = RenderOptions {
        dpi: 100,
        ..RenderOptions::default()
    };
    let fonts = bundled_fonts();

    // In colour the instance is red; monochrome turns it into the luminance grey of red (54).
    let (colour, _) = render(&list, a4(), &options, &fonts, Some(&lib)).unwrap();
    assert_eq!(pixel(&colour, 394, 394), [255, 0, 0, 255]);
    let (mono, report) = render_monochrome(&list, a4(), &options, &fonts, Some(&lib)).unwrap();
    assert_eq!(report.unresolved_instances, 1);
    assert_eq!(pixel(&mono, 394, 394), [54, 54, 54, 255]);
    assert_eq!(pixel(&mono, 787, 394), [255, 255, 255, 255]);

    // Without a resolver nothing is drawn and both are counted.
    let (_, report) = render_monochrome(&list, a4(), &options, &fonts, None).unwrap();
    assert_eq!(report.unresolved_instances, 2);
}

#[test]
fn render_monochrome_text_is_black() {
    let mut run = text_run("HHHH", mm(40, 60));
    run.brush = solid(Color::from_rgb8(255, 0, 0));
    let list = DisplayList {
        items: vec![Item::glyphs(Arc::new(run))],
    };
    let options = RenderOptions {
        dpi: 200,
        ..RenderOptions::default()
    };
    let (img, _) = render_monochrome(&list, a4(), &options, &bundled_fonts(), None).unwrap();
    let mut ink = 0;
    for p in img.pixels.as_chunks::<4>().0.iter() {
        assert!(p[0] == p[1] && p[1] == p[2], "colour leaked: {p:?}");
        if is_dark([p[0], p[1], p[2], p[3]]) {
            ink += 1;
        }
    }
    assert!(ink > 100);
}

fn decode(bytes: &[u8]) -> (png::OutputInfo, Vec<u8>, Option<png::PixelDimensions>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("valid png");
    let dims = reader.info().pixel_dims;
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf.truncate(info.buffer_size());
    (info, buf, dims)
}

fn image(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&f(x, y));
        }
    }
    RgbaImage {
        width,
        height,
        pixels,
    }
}

#[test]
fn png_grayscale_for_grey_images() {
    let img = image(17, 9, |x, y| {
        let v = match (x + y) % 3 {
            0 => 0,
            1 => 255,
            _ => 192,
        };
        [v, v, v, 255]
    });
    let bytes = encode_png(&img, Some(150));
    let (info, data, dims) = decode(&bytes);
    assert_eq!((info.width, info.height), (17, 9));
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    let want: Vec<u8> = img.pixels.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
    assert_eq!(data, want);
    // 150 dpi = 5906 pixels per metre.
    let dims = dims.expect("pHYs");
    assert_eq!((dims.xppu, dims.yppu, dims.unit), (5906, 5906, png::Unit::Meter));
    // Byte-identical encodings.
    assert_eq!(bytes, encode_png(&img, Some(150)));
}

#[test]
fn png_rgba_when_colour_or_alpha_present() {
    let mut img = image(8, 8, |_, _| [255, 255, 255, 255]);
    img.pixels[4 * 10..4 * 10 + 4].copy_from_slice(&[255, 0, 0, 255]);
    let bytes = encode_png(&img, None);
    let (info, data, dims) = decode(&bytes);
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(data, img.pixels);
    assert!(dims.is_none(), "no pHYs without a dpi");
    assert_eq!(bytes, encode_png(&img, None));

    // A grey pixel with alpha < 255 is RGBA too.
    let mut img = image(4, 4, |_, _| [9, 9, 9, 255]);
    img.pixels[3] = 100;
    let (info, data, _) = decode(&encode_png(&img, None));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(data, img.pixels);
}

#[test]
fn png_of_a_rendered_page_round_trips() {
    let list = DisplayList {
        items: vec![
            Item::path(
                ew_dl::Path::line(mm(10, 10), mm(100, 60)),
                Some(Stroke::pen(Pen::P042)),
                None,
            ),
            Item::glyphs(Arc::new(text_run("Page 101", mm(20, 100)))),
        ],
    };
    let options = RenderOptions {
        dpi: 100,
        ..RenderOptions::default()
    };
    let (img, _) = render(&list, a4(), &options, &bundled_fonts(), None).unwrap();
    let (info, data, _) = decode(&encode_png(&img, Some(100)));
    // Black text on white with grey anti-aliasing: grayscale.
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    let want: Vec<u8> = img.pixels.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
    assert_eq!(data, want);
}

#[test]
fn png_of_malformed_images_does_not_panic() {
    // Short buffer: read as transparent black beyond its end, hence RGBA.
    let short = RgbaImage {
        width: 4,
        height: 4,
        pixels: vec![255; 10],
    };
    let (info, data, _) = decode(&encode_png(&short, None));
    assert_eq!((info.width, info.height), (4, 4));
    assert_eq!(data.len(), 4 * 4 * 4);
    // Zero dimension: nothing to encode.
    let empty = RgbaImage {
        width: 0,
        height: 5,
        pixels: Vec::new(),
    };
    assert!(encode_png(&empty, None).is_empty());
}
