// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU canvas on a headless device (WP-S2.02 part 2, B4 and the renderer's own rules): a hairline keeps one
//! physical pixel, later paint wins over earlier whatever the batch order, translucency blends last, clips cut,
//! and an idle frame submits nothing. On Windows an adapter must be found (DX12 WARP at least); elsewhere a host
//! without one prints `skipped: no adapter` and passes (§8 item 4).

use std::sync::Arc;

use ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_dl::{Brush, Color, DisplayList, FontKey, Item, Path, Stroke, Transform};
use ew_render_cpu::{FontBytes, FontResolver};
use ew_render2d::gpu::{GpuCanvas, GpuView, HeadlessDevice, MeshCache, prepare};

struct NoFonts;

impl FontResolver for NoFonts {
    fn font(&self, _: FontKey) -> Option<FontBytes> {
        None
    }
}

fn p(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x), Nm(y))
}

fn mm(v: f64) -> i64 {
    (v * 1e6) as i64
}

fn square(x: f64, y: f64, side: f64) -> Path {
    Path::rect(Rect::from_points(p(mm(x), mm(y)), p(mm(x + side), mm(y + side))))
}

/// The headless device, or `None` (and a note) where a host has none; on Windows one must be found.
fn device() -> Option<HeadlessDevice> {
    let d = HeadlessDevice::new();
    if d.is_none() {
        if cfg!(windows) {
            panic!("on Windows an adapter must be found (DX12 WARP at least)");
        }
        println!("skipped: no adapter");
    }
    d
}

/// 96 dpi (100 %), the sheet's origin at the target's corner.
fn view(w: u32, h: u32) -> GpuView {
    GpuView {
        target_px: (w, h),
        origin_px: (0.0, 0.0),
        px_per_nm: 96.0 / 25_400_000.0,
    }
}

fn canvas_with(d: &HeadlessDevice, list: &DisplayList) -> GpuCanvas {
    let mut canvas = d.canvas().expect("the canvas initialises");
    let mut cache = MeshCache::default();
    canvas.set_scene(Arc::new(prepare(1, list, &NoFonts, &mut cache)));
    canvas
}

fn pixel(image: &ew_render_cpu::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.width + x) * 4) as usize;
    [
        image.pixels[i],
        image.pixels[i + 1],
        image.pixels[i + 2],
        image.pixels[i + 3],
    ]
}

/// A pixel some channel of which is at least 128 away from white (§8 item 3).
fn is_ink(px: [u8; 4]) -> bool {
    px[..3].iter().any(|&c| c <= 127)
}

#[test]
fn b4_a_line_of_one_hundredth_of_a_millimetre_leaves_ink() {
    let Some(d) = device() else { return };
    let mut list = DisplayList::new();
    // 10 µm wide: 0.04 pixels at 96 dpi; the vertex shader widens it to one physical pixel.
    list.push(Item::path(
        Path::line(p(mm(10.0), mm(10.0)), p(mm(50.0), mm(10.0))),
        Some(Stroke::solid(Nm(10_000))),
        None,
    ));
    let mut canvas = canvas_with(&d, &list);
    let out = canvas.render(&view(400, 100)).expect("a render");
    assert!(out.submitted && out.resized);
    let image = canvas.read_back().expect("the readback");
    assert_eq!((image.width, image.height), (400, 100));
    let ink = (0..image.width)
        .filter(|&x| (30..46).any(|y| is_ink(pixel(&image, x, y))))
        .count();
    // 40 mm at 96 dpi is about 151 pixels.
    assert!((140..=160).contains(&ink), "{ink} columns with ink");
}

#[test]
fn later_paint_wins_over_earlier_whatever_the_batch_order() {
    let Some(d) = device() else { return };
    let mut list = DisplayList::new();
    // Painted first: a red square in the root's static mesh. Painted second: a black square in a group mesh, which
    // the opaque batch draws before the root mesh. The depth test keeps the paint order.
    list.push(Item::path(
        square(0.0, 0.0, 20.0),
        None,
        Some(Brush::Solid(Color::from_rgb8(255, 0, 0))),
    ));
    list.push(Item::group(
        Some(Transform::from_translation(p(mm(5.0), mm(5.0)))),
        None,
        vec![Item::path(square(0.0, 0.0, 10.0), None, Some(Brush::BLACK))],
    ));
    let mut canvas = canvas_with(&d, &list);
    canvas.render(&view(100, 100)).expect("a render");
    let image = canvas.read_back().expect("the readback");
    // 10 mm is 37.8 pixels: (40, 40) is inside both squares, (70, 70) inside the red one only.
    assert_eq!(pixel(&image, 40, 40), [0, 0, 0, 255]);
    assert_eq!(pixel(&image, 70, 70), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 95, 95), [255, 255, 255, 255], "the paper");
}

#[test]
fn translucent_paint_blends_last_and_clips_cut() {
    let Some(d) = device() else { return };
    let mut list = DisplayList::new();
    list.push(Item::path(
        square(0.0, 0.0, 20.0),
        None,
        Some(Brush::Solid(Color::from_rgba8(0, 0, 255, 128))),
    ));
    // A black square clipped to its left half.
    list.push(Item::group(
        Some(Transform::from_translation(p(mm(10.0), mm(0.0)))),
        Some(Rect::from_points(p(0, 0), p(mm(5.0), mm(10.0)))),
        vec![Item::path(square(0.0, 0.0, 10.0), None, Some(Brush::BLACK))],
    ));
    let mut canvas = canvas_with(&d, &list);
    canvas.render(&view(100, 100)).expect("a render");
    let image = canvas.read_back().expect("the readback");
    let blended = pixel(&image, 10, 60);
    for (got, want) in blended.iter().zip([127u8, 127, 255, 255]) {
        assert!(got.abs_diff(want) <= 2, "{blended:?}");
    }
    // Inside the clip the black square, painted after the translucent blue, covers it (the translucent pass is drawn
    // last but keeps the paint order through the depth test).
    assert_eq!(pixel(&image, 45, 15), [0, 0, 0, 255]);
    // Right of the clip (x > 15 mm = 56.7 px) only the blue.
    let cut = pixel(&image, 65, 15);
    assert!(cut[0].abs_diff(127) <= 2 && cut[2] == 255, "{cut:?}");
}

#[test]
fn an_idle_frame_submits_nothing() {
    let Some(d) = device() else { return };
    let mut list = DisplayList::new();
    list.push(Item::path(square(1.0, 1.0, 5.0), None, Some(Brush::BLACK)));
    let mut canvas = canvas_with(&d, &list);
    let v = view(64, 64);
    assert!(canvas.render(&v).unwrap().submitted);
    for _ in 0..5 {
        let again = canvas.render(&v).unwrap();
        assert!(!again.submitted && !again.resized);
    }
    assert_eq!(canvas.counters().submits, 1);
    // A pan is a new render; a new size a new target.
    let moved = GpuView {
        origin_px: (3.0, 0.0),
        ..v
    };
    assert!(canvas.render(&moved).unwrap().submitted);
    let bigger = GpuView {
        target_px: (80, 64),
        ..moved
    };
    let out = canvas.render(&bigger).unwrap();
    assert!(out.submitted && out.resized);
    assert_eq!(canvas.counters().submits, 3);
    assert_eq!(canvas.counters().meshes_uploaded, 1);
}

#[test]
fn a_target_beyond_the_device_limit_is_an_error_not_a_panic() {
    let Some(d) = device() else { return };
    let mut canvas = canvas_with(&d, &DisplayList::new());
    let max = d.device.limits().max_texture_dimension_2d;
    assert!(canvas.render(&view(max + 1, 8)).is_err());
    assert!(canvas.render(&view(8, 8)).is_ok());
}
