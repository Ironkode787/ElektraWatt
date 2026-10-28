// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU canvas on a headless device (WP-S2.02 part 2, B4 and the renderer's own rules): a hairline keeps one
//! physical pixel, later paint wins over earlier whatever the batch order, translucency blends last, clips cut,
//! and an idle frame submits nothing. Raster images (FT-480) are drawn by the textured-quad pipeline in paint order,
//! clipped, cached and downscaled, within the parity rule of §8 item 3 against the CPU raster. On Windows an adapter
//! must be found (DX12 WARP at least); elsewhere a host without one prints `skipped: no adapter` and passes (§8 item
//! 4).

use std::sync::Arc;

#[path = "../../ew-render-cpu/tests/common/jpeg.rs"]
mod jpeg;

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

/// Whether the test run requires an adapter (`EW_REQUIRE_GPU_ADAPTER=1`, set by the CI test jobs): then a missing
/// adapter fails the GPU tests, which otherwise skip with a note nextest does not show for a passing test.
// Program boundary of the test binary: CI's switch for the GPU tests, read once per test; no output depends on it.
#[allow(clippy::disallowed_methods)]
fn adapter_required() -> bool {
    std::env::var_os("EW_REQUIRE_GPU_ADAPTER").is_some_and(|v| v == "1")
}

/// The headless devices (the default adapter and the fallback adapter where they differ), with a note where a host
/// has none; on Windows one must be found.
fn devices() -> Vec<HeadlessDevice> {
    let all = HeadlessDevice::all();
    if all.is_empty() {
        if cfg!(windows) {
            panic!("on Windows an adapter must be found (DX12 WARP at least)");
        }
        if adapter_required() {
            panic!("EW_REQUIRE_GPU_ADAPTER=1 and no graphics adapter was found");
        }
        println!("skipped: no adapter");
    }
    for d in &all {
        println!("adapter: {:?} {:?}", d.summary.kind, d.summary.api);
    }
    all
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
    for d in devices() {
        let d = &d;
        let mut list = DisplayList::new();
        // 10 µm wide: 0.04 pixels at 96 dpi; the vertex shader widens it to one physical pixel.
        list.push(Item::path(
            Path::line(p(mm(10.0), mm(10.0)), p(mm(50.0), mm(10.0))),
            Some(Stroke::solid(Nm(10_000))),
            None,
        ));
        let mut canvas = canvas_with(d, &list);
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
}

#[test]
fn later_paint_wins_over_earlier_whatever_the_batch_order() {
    for d in devices() {
        let d = &d;
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
        let mut canvas = canvas_with(d, &list);
        canvas.render(&view(100, 100)).expect("a render");
        let image = canvas.read_back().expect("the readback");
        // 10 mm is 37.8 pixels: (40, 40) is inside both squares, (70, 70) inside the red one only.
        assert_eq!(pixel(&image, 40, 40), [0, 0, 0, 255]);
        assert_eq!(pixel(&image, 70, 70), [255, 0, 0, 255]);
        assert_eq!(pixel(&image, 95, 95), [255, 255, 255, 255], "the paper");
    }
}

#[test]
fn translucent_paint_blends_last_and_clips_cut() {
    for d in devices() {
        let d = &d;
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
        let mut canvas = canvas_with(d, &list);
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
}

#[test]
fn an_idle_frame_submits_nothing() {
    for d in devices() {
        let d = &d;
        let mut list = DisplayList::new();
        list.push(Item::path(square(1.0, 1.0, 5.0), None, Some(Brush::BLACK)));
        let mut canvas = canvas_with(d, &list);
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
}

#[test]
fn a_target_beyond_the_device_limit_is_an_error_not_a_panic() {
    for d in devices() {
        let d = &d;
        let mut canvas = canvas_with(d, &DisplayList::new());
        let max = d.device.limits().max_texture_dimension_2d;
        assert!(canvas.render(&view(max + 1, 8)).is_err());
        assert!(canvas.render(&view(8, 8)).is_ok());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Raster images (FT-480): the textured-quad pipeline, checked against the CPU raster

/// A PNG of `w × h` pixels, pixel `(x, y)` given by `f` (synthetic).
fn png_of(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let f = &f;
    let pixels = (0..h).flat_map(|y| (0..w).flat_map(move |x| f(x, y))).collect();
    ew_render_cpu::encode_png(
        &ew_render_cpu::RgbaImage {
            width: w,
            height: h,
            pixels,
        },
        None,
    )
}

/// An image item of `bytes` with the (test-only) hash `[sha; 32]`; distinct images get distinct hashes.
fn image_item(sha: u8, format: ew_dl::ImageFormat, bytes: Vec<u8>, rect: Rect) -> Item {
    Item::image(
        Arc::new(ew_dl::ImageData {
            sha256: [sha; 32],
            format,
            bytes: Arc::from(bytes),
        }),
        rect,
    )
}

fn mm_rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
    Rect::from_points(p(mm(x0), mm(y0)), p(mm(x1), mm(y1)))
}

/// The pixel at (`x_mm`, `y_mm`) of an image at 96 dpi with the sheet's origin at its corner.
fn at_mm(image: &ew_render_cpu::RgbaImage, x_mm: f64, y_mm: f64) -> [u8; 4] {
    let k = 96.0 / 25.4;
    pixel(image, (x_mm * k) as u32, (y_mm * k) as u32)
}

fn near(got: [u8; 4], want: [u8; 4], tolerance: u8) -> bool {
    got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= tolerance)
}

/// The CPU raster of `list` at 96 dpi on a `w × h` pixel sheet (the reference of the parity checks).
fn cpu_image(list: &DisplayList, w: u32, h: u32) -> ew_render_cpu::RgbaImage {
    let k = 25_400_000.0 / 96.0;
    let sheet = ew_render_cpu::SheetSize {
        width: Nm((f64::from(w) * k).round() as i64),
        height: Nm((f64::from(h) * k).round() as i64),
    };
    let options = ew_render_cpu::RenderOptions {
        dpi: 96,
        ..ew_render_cpu::RenderOptions::default()
    };
    let (image, report) = ew_render_cpu::render(list, sheet, &options, &NoFonts, None).unwrap();
    assert_eq!((image.width, image.height), (w, h));
    assert_eq!(report.skipped_images, 0, "the CPU path draws every image");
    image
}

/// The GPU raster of `list` at 96 dpi on a `w × h` target.
fn gpu_image(d: &HeadlessDevice, list: &DisplayList, w: u32, h: u32) -> ew_render_cpu::RgbaImage {
    let mut canvas = canvas_with(d, list);
    canvas.render(&view(w, h)).expect("a render");
    assert_eq!(canvas.counters().textures_refused, 0);
    canvas.read_back().expect("the readback")
}

/// The clearly-ink pixels of `a` and how many of them have no pixel in `b`'s 3 × 3 neighbourhood with every channel
/// within 128: the parity rule of WP-S2.02 §8 item 3, as `ew-ui`'s B2 tests apply it.
fn unmatched(a: &ew_render_cpu::RgbaImage, b: &ew_render_cpu::RgbaImage) -> (usize, usize) {
    let (w, h) = (i64::from(a.width), i64::from(a.height));
    let px = |img: &ew_render_cpu::RgbaImage, x: i64, y: i64| pixel(img, x as u32, y as u32);
    let mut ink = 0;
    let mut missed = 0;
    for y in 0..h {
        for x in 0..w {
            let c = px(a, x, y);
            if !is_ink(c) {
                continue;
            }
            ink += 1;
            let found = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..w).contains(&nx) && (0..h).contains(&ny) && near(px(b, nx, ny), c, 128)
                })
            });
            missed += usize::from(!found);
        }
    }
    (missed, ink)
}

/// The mean absolute difference of the red, green and blue channels.
fn mean_difference(a: &ew_render_cpu::RgbaImage, b: &ew_render_cpu::RgbaImage) -> f64 {
    let sum: u64 = a
        .pixels
        .chunks(4)
        .zip(b.pixels.chunks(4))
        .map(|(p, q)| (0..3).map(|c| u64::from(p[c].abs_diff(q[c]))).sum::<u64>())
        .sum();
    sum as f64 / (a.pixels.len() / 4 * 3).max(1) as f64
}

/// The bound of the mean channel difference between the GPU and the CPU raster of the image pages: measured 0.27 on
/// the hardware DX12 adapter and on WARP alike (cycle 30), both pages; the bound leaves about a quarter for other
/// adapters' MSAA patterns. A missing, shifted or nearest-sampled image raises it several times over.
const MEAN_BOUND_IMAGES: f64 = 0.35;

/// The parity rule both ways (at most 1 % of the clearly-ink pixels unmatched) and the mean channel difference
/// within `mean_bound`.
fn assert_parity(
    label: &str,
    gpu: &ew_render_cpu::RgbaImage,
    cpu: &ew_render_cpu::RgbaImage,
    mean_bound: f64,
) {
    assert_eq!((gpu.width, gpu.height), (cpu.width, cpu.height));
    let mean = mean_difference(gpu, cpu);
    println!("{label}: mean channel difference {mean:.4}");
    assert!(mean <= mean_bound, "{label}: mean {mean:.4} above {mean_bound}");
    for (name, a, b) in [("GPU", gpu, cpu), ("CPU", cpu, gpu)] {
        let (missed, ink) = unmatched(a, b);
        let share = 100.0 * missed as f64 / ink.max(1) as f64;
        println!("{label}: {missed} of {ink} clearly-ink {name} pixels unmatched ({share:.3} %)");
        assert!(ink > 500, "{label}: the {name} image has ink");
        assert!(
            share <= 1.0,
            "{label}: {share:.3} % of the {name} ink is unmatched"
        );
    }
}

#[test]
fn a_png_item_is_drawn_inside_its_rectangle() {
    let red = png_of(8, 2, |_, _| [255, 0, 0, 255]);
    let mut list = DisplayList::new();
    list.push(image_item(
        3,
        ew_dl::ImageFormat::Png,
        red,
        mm_rect(10.0, 10.0, 50.0, 20.0),
    ));
    let mut cache = MeshCache::default();
    let scene = prepare(1, &list, &NoFonts, &mut cache);
    assert_eq!((scene.stats.images, scene.stats.images_skipped), (1, 0));
    for d in devices() {
        let image = gpu_image(&d, &list, 300, 100);
        let k = 96.0 / 25.4;
        let mut red_pixels = 0;
        for y in 0..image.height {
            for x in 0..image.width {
                let px = pixel(&image, x, y);
                if px[0] > 200 && px[1] < 60 && px[2] < 60 {
                    red_pixels += 1;
                    let (xm, ym) = (f64::from(x) / k, f64::from(y) / k);
                    assert!(
                        (9.7..50.3).contains(&xm) && (9.7..20.3).contains(&ym),
                        "red at ({x}, {y}) outside the rectangle"
                    );
                }
            }
        }
        // 40 × 10 mm at 96 dpi: about 151 × 38 pixels.
        assert!((5_400..=6_000).contains(&red_pixels), "{red_pixels} red pixels");
        assert_eq!(at_mm(&image, 30.0, 15.0), [255, 0, 0, 255]);
        assert_eq!(at_mm(&image, 70.0, 15.0), [255, 255, 255, 255], "the paper");
    }
}

#[test]
fn a_jpeg_item_matches_the_cpu_raster() {
    let jpeg = jpeg::baseline(jpeg::Layout::Ycbcr420, 64, 32);
    let mut list = DisplayList::new();
    list.push(image_item(
        4,
        ew_dl::ImageFormat::Jpeg,
        jpeg,
        mm_rect(5.0, 5.0, 45.0, 25.0),
    ));
    let cpu = cpu_image(&list, 200, 120);
    for d in devices() {
        let gpu = gpu_image(&d, &list, 200, 120);
        // The JPEG's pixels inside the rectangle: the centre matches the CPU raster closely.
        let (g, c) = (at_mm(&gpu, 25.0, 15.0), at_mm(&cpu, 25.0, 15.0));
        assert!(near(g, c, 3), "{g:?} against the CPU's {c:?}");
        assert_ne!(g, [255, 255, 255, 255], "the image is drawn");
        assert_parity(
            &format!("JPEG on {:?}", d.summary.kind),
            &gpu,
            &cpu,
            MEAN_BOUND_IMAGES,
        );
    }
}

#[test]
fn an_image_with_alpha_over_a_filled_path_blends_in_paint_order() {
    let half_red = png_of(4, 4, |_, _| [255, 0, 0, 128]);
    let mut list = DisplayList::new();
    list.push(Item::path(
        square(0.0, 0.0, 20.0),
        None,
        Some(Brush::Solid(Color::from_rgb8(0, 0, 255))),
    ));
    list.push(image_item(
        5,
        ew_dl::ImageFormat::Png,
        half_red,
        mm_rect(10.0, 0.0, 30.0, 20.0),
    ));
    let cpu = cpu_image(&list, 120, 80);
    let over = at_mm(&cpu, 15.0, 10.0);
    assert!(near(over, [128, 0, 127, 255], 2), "the CPU blend {over:?}");
    for d in devices() {
        let gpu = gpu_image(&d, &list, 120, 80);
        let got = at_mm(&gpu, 15.0, 10.0);
        assert!(
            near(got, over, 2),
            "over the path: {got:?} against the CPU's {over:?}"
        );
        let alone = at_mm(&gpu, 25.0, 10.0);
        assert!(
            near(alone, at_mm(&cpu, 25.0, 10.0), 2),
            "over the paper: {alone:?}"
        );
        assert_eq!(at_mm(&gpu, 5.0, 10.0), [0, 0, 255, 255], "the path alone");
    }
}

#[test]
fn a_path_painted_after_an_image_covers_it() {
    let green = png_of(4, 4, |_, _| [0, 200, 0, 255]);
    let half_green = png_of(4, 4, |_, _| [0, 200, 0, 100]);
    for (sha, bytes) in [(6, green), (7, half_green)] {
        let mut list = DisplayList::new();
        // The image inside a group (its own context), the black square after it in the root's static mesh, which
        // the opaque pass may draw first: the depth test keeps the paint order.
        list.push(Item::group(
            Some(Transform::from_translation(p(mm(2.0), mm(2.0)))),
            None,
            vec![image_item(
                sha,
                ew_dl::ImageFormat::Png,
                bytes,
                mm_rect(0.0, 0.0, 20.0, 20.0),
            )],
        ));
        list.push(Item::path(square(10.0, 10.0, 15.0), None, Some(Brush::BLACK)));
        let cpu = cpu_image(&list, 120, 120);
        let image_px = at_mm(&cpu, 5.0, 5.0);
        assert!(image_px[0] <= 160, "image {sha} on the CPU path: {image_px:?}");
        for d in devices() {
            let gpu = gpu_image(&d, &list, 120, 120);
            assert_eq!(
                at_mm(&gpu, 15.0, 15.0),
                [0, 0, 0, 255],
                "image {sha}: the later path covers it"
            );
            let got = at_mm(&gpu, 5.0, 5.0);
            assert!(
                near(got, image_px, 2),
                "image {sha} elsewhere: {got:?} against the CPU's {image_px:?}"
            );
        }
    }
}

#[test]
fn a_clipped_image_is_clipped() {
    let blue = png_of(4, 4, |_, _| [0, 0, 255, 255]);
    let mut list = DisplayList::new();
    // The image's left half only.
    list.push(Item::group(
        Some(Transform::from_translation(p(mm(10.0), mm(0.0)))),
        Some(mm_rect(0.0, 0.0, 10.0, 20.0)),
        vec![image_item(
            8,
            ew_dl::ImageFormat::Png,
            blue,
            mm_rect(0.0, 0.0, 20.0, 20.0),
        )],
    ));
    let cpu = cpu_image(&list, 140, 80);
    for d in devices() {
        let gpu = gpu_image(&d, &list, 140, 80);
        assert_eq!(at_mm(&gpu, 15.0, 10.0), [0, 0, 255, 255], "inside the clip");
        assert_eq!(at_mm(&gpu, 25.0, 10.0), [255, 255, 255, 255], "outside the clip");
        assert_eq!(
            at_mm(&cpu, 25.0, 10.0),
            [255, 255, 255, 255],
            "as on the CPU path"
        );
    }
}

/// Images at several scales, turned and mirrored, with alpha over paths and under a later path, against the CPU
/// raster: the parity rule of §8 item 3 and a mean channel difference bound.
#[test]
fn images_match_the_cpu_raster_within_the_parity_rule() {
    // A pattern with smooth and hard edges and an alpha ramp.
    let pattern = png_of(16, 12, |x, y| {
        let a = if y < 6 { 255 } else { (64 + x * 12) as u8 };
        [
            (x * 16) as u8,
            (y * 20) as u8,
            if (x / 4 + y / 4) % 2 == 0 { 40 } else { 220 },
            a,
        ]
    });
    let jpeg = jpeg::baseline(jpeg::Layout::Ycbcr444, 32, 32);
    let mut list = DisplayList::new();
    list.push(Item::path(
        square(5.0, 5.0, 30.0),
        None,
        Some(Brush::Solid(Color::from_rgb8(0, 114, 178))),
    ));
    list.push(image_item(
        9,
        ew_dl::ImageFormat::Png,
        pattern.clone(),
        mm_rect(10.0, 10.0, 42.0, 34.0),
    ));
    // Turned a quarter, and mirrored.
    list.push(Item::group(
        Some(Transform::new(
            p(mm(80.0), mm(10.0)),
            ew_dl::ew_units::Orient2::R90,
            ew_dl::ew_units::Angle2::ZERO,
        )),
        None,
        vec![image_item(
            9,
            ew_dl::ImageFormat::Png,
            pattern,
            mm_rect(0.0, 0.0, 30.0, 20.0),
        )],
    ));
    list.push(Item::group(
        Some(Transform::new(
            p(mm(130.0), mm(40.0)),
            ew_dl::ew_units::Orient2::M0,
            ew_dl::ew_units::Angle2::ZERO,
        )),
        None,
        vec![image_item(
            10,
            ew_dl::ImageFormat::Jpeg,
            jpeg,
            mm_rect(0.0, 0.0, 25.0, 25.0),
        )],
    ));
    // A later path over the PNG.
    list.push(Item::path(
        Path::line(p(mm(5.0), mm(30.0)), p(mm(140.0), mm(30.0))),
        Some(Stroke::solid(Nm(400_000))),
        None,
    ));
    let cpu = cpu_image(&list, 560, 300);
    for d in devices() {
        let gpu = gpu_image(&d, &list, 560, 300);
        assert_parity(
            &format!("images on {:?}", d.summary.kind),
            &gpu,
            &cpu,
            MEAN_BOUND_IMAGES,
        );
    }
}

#[test]
fn the_texture_cache_evicts_past_its_budget_and_uploads_again_on_demand() {
    let one = |sha: u8, rgb: [u8; 3]| {
        let mut list = DisplayList::new();
        let bytes = png_of(16, 16, |_, _| [rgb[0], rgb[1], rgb[2], 255]);
        list.push(image_item(
            sha,
            ew_dl::ImageFormat::Png,
            bytes,
            mm_rect(2.0, 2.0, 12.0, 12.0),
        ));
        list
    };
    let (a, b) = (one(11, [255, 0, 0]), one(12, [0, 0, 255]));
    for d in devices() {
        let mut canvas = d.canvas().expect("the canvas initialises");
        // Room for one 16 × 16 texture (1 KiB) beyond the current scene's.
        canvas.set_texture_budget(1_024);
        let mut cache = MeshCache::default();
        let v = view(64, 64);
        canvas.set_scene(Arc::new(prepare(1, &a, &NoFonts, &mut cache)));
        canvas.render(&v).unwrap();
        assert_eq!((canvas.texture_count(), canvas.texture_bytes()), (1, 1_024));
        canvas.set_scene(Arc::new(prepare(2, &b, &NoFonts, &mut cache)));
        canvas.render(&v).unwrap();
        assert_eq!(
            canvas.texture_count(),
            1,
            "the older texture is evicted past the budget"
        );
        assert_eq!(canvas.counters().textures_uploaded, 2);
        assert_eq!(at_mm(&canvas.read_back().unwrap(), 7.0, 7.0), [0, 0, 255, 255]);
        // Back to the first scene: its texture is uploaded again and drawn.
        canvas.set_scene(Arc::new(prepare(3, &a, &NoFonts, &mut cache)));
        canvas.render(&v).unwrap();
        assert_eq!(canvas.counters().textures_uploaded, 3);
        assert_eq!(at_mm(&canvas.read_back().unwrap(), 7.0, 7.0), [255, 0, 0, 255]);
        // Within the default budget nothing is evicted and a known image is not uploaded again.
        canvas.set_texture_budget(ew_render2d::gpu::GPU_TEXTURE_BYTES);
        canvas.set_scene(Arc::new(prepare(4, &b, &NoFonts, &mut cache)));
        canvas.set_scene(Arc::new(prepare(5, &a, &NoFonts, &mut cache)));
        assert_eq!(canvas.texture_count(), 2);
        assert_eq!(
            canvas.counters().textures_uploaded,
            4,
            "b uploaded once more, a kept"
        );
    }
}

#[test]
fn an_oversize_image_is_downscaled_on_the_worker_and_one_beyond_the_device_is_refused() {
    // 64 × 8 texels, left half red and right half blue, within a maximum side of 16: 16 × 2 texels.
    let wide = png_of(64, 8, |x, _| {
        if x < 32 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        }
    });
    let mut list = DisplayList::new();
    list.push(image_item(
        13,
        ew_dl::ImageFormat::Png,
        wide,
        mm_rect(0.0, 0.0, 40.0, 5.0),
    ));
    let mut cache = MeshCache::default();
    cache.set_max_image_side(16);
    let scene = prepare(1, &list, &NoFonts, &mut cache);
    let texture = &scene.images[0];
    assert_eq!((texture.width, texture.height, texture.downscaled), (16, 2, true));
    for d in devices() {
        let mut canvas = d.canvas().expect("the canvas initialises");
        canvas.set_scene(Arc::new(prepare(2, &list, &NoFonts, &mut cache)));
        canvas.render(&view(200, 40)).unwrap();
        let image = canvas.read_back().unwrap();
        assert_eq!(at_mm(&image, 5.0, 2.5), [255, 0, 0, 255]);
        assert_eq!(at_mm(&image, 35.0, 2.5), [0, 0, 255, 255]);
        // An image one texel wider than the device allows (a preparation told a larger maximum) is refused and not
        // drawn: no fault, the rest of the page is drawn.
        let max = canvas.max_image_side();
        let Ok(side) = u16::try_from(max + 1) else {
            println!("skipped: the device allows {max} texels, beyond a PNG's decodable side");
            continue;
        };
        let too_wide = png_of(u32::from(side), 1, |_, _| [255, 0, 0, 255]);
        let mut big = DisplayList::new();
        big.push(image_item(
            14,
            ew_dl::ImageFormat::Png,
            too_wide,
            mm_rect(0.0, 0.0, 40.0, 5.0),
        ));
        big.push(Item::path(square(45.0, 0.0, 5.0), None, Some(Brush::BLACK)));
        let mut unbounded = MeshCache::default();
        unbounded.set_max_image_side(u32::from(u16::MAX));
        canvas.set_scene(Arc::new(prepare(3, &big, &NoFonts, &mut unbounded)));
        canvas.render(&view(200, 40)).expect("no fault");
        assert_eq!(canvas.counters().textures_refused, 1);
        assert!(canvas.fault().is_none());
        let image = canvas.read_back().unwrap();
        assert_eq!(
            at_mm(&image, 5.0, 2.5),
            [255, 255, 255, 255],
            "the refused image is not drawn"
        );
        assert_eq!(at_mm(&image, 47.0, 2.5), [0, 0, 0, 255], "the path is");
    }
}
