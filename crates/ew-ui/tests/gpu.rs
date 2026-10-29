// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU canvas against the CPU raster, render to texture on a headless device (WP-S2.02 part 2, B2 to B4): the
//! parity rule of section 8 item 3 on folio 103 at `D = 150` and on the dense A3 page, a filled #0072B2 rectangle
//! reads back its bytes, a second preparation tessellates nothing and repeated glyphs are one mesh each, and below
//! 2 pixels of em a glyph run is a bar. On Windows an adapter must be found (DX12 WARP at least), and so wherever
//! `EW_REQUIRE_GPU_ADAPTER=1` is set (the CI test jobs); elsewhere a host without one prints `skipped: no adapter`
//! and passes (section 8 item 4).

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_render_cpu::{
    FontBytes, ItemBounds, RegionOptions, RenderOptions, RgbaImage, SheetSize, pixel_size, render,
    render_region,
};
use ew_render2d::gpu::{GpuView, HeadlessDevice, MeshCache, prepare};
use ew_ui::ew_viewmodel::canvas::CanvasPage;
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::{
    Brush, Color, DisplayList, FontKey, Item, ItemKind, Path, Transform,
};
use ew_ui::ew_viewmodel::ew_api::output::font_bytes;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;
use support::Tmp;

/// The bundled fonts.
fn fonts() -> &'static BTreeMap<FontKey, FontBytes> {
    font_bytes()
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

/// Folio 103 of a copy of Golden A, as the canvas shows it.
fn folio_103(tmp: &Tmp) -> Arc<CanvasPage> {
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    let opened = shell.dispatch(
        "file.open",
        &json!({"dir": dir.to_string_lossy(), "read_only": true}),
    );
    assert_eq!(opened, Outcome::Done);
    assert_eq!(
        shell.dispatch("page.goto", &json!({"folio": "103"})),
        Outcome::Done
    );
    let page = Arc::clone(shell.canvas().expect("folio 103 has a canvas").page());
    shell.shutdown();
    page
}

/// The paths and glyph runs `items` draw, groups followed.
fn drawn_items(items: &[Item]) -> usize {
    items
        .iter()
        .map(|item| match &item.kind {
            ItemKind::Path(_) | ItemKind::Glyphs(_) => 1,
            ItemKind::Group(g) => drawn_items(&g.items),
            ItemKind::Image(_) | ItemKind::Instance(_) => 0,
        })
        .sum()
}

/// The dense A3 page of the bench (spec section 9): translated copies of folio 103 on an A3 landscape sheet until
/// at least 5,000 paths and glyph runs are drawn.
fn dense_a3(base: &CanvasPage) -> (DisplayList, (i64, i64)) {
    const A3: (i64, i64) = (420_000_000, 297_000_000);
    let per_copy = drawn_items(&base.list.items);
    let copies = 5_000usize.div_ceil(per_copy).max(1);
    let cols = (1..=copies).find(|c| c * c >= copies).unwrap_or(1);
    let rows = copies.div_ceil(cols);
    let slack = ((A3.0 - base.sheet.0).max(0), (A3.1 - base.sheet.1).max(0));
    let spread = |slack: i64, i: usize, n: usize| slack * i as i64 / (n.max(2) - 1) as i64;
    let mut list = DisplayList::new();
    for i in 0..copies {
        let at = Pt2::new(
            Nm(spread(slack.0, i % cols, cols)),
            Nm(spread(slack.1, i / cols, rows)),
        );
        list.push(Item::group(
            Some(Transform::from_translation(at)),
            None,
            base.list.items.clone(),
        ));
    }
    assert!(drawn_items(&list.items) >= 5_000);
    (list, A3)
}

/// The GPU image of `list` on `sheet` at `dpi`, the sheet's origin at the target's corner, the target the size of
/// the CPU raster.
fn gpu_image(d: &HeadlessDevice, list: &DisplayList, sheet: (i64, i64), dpi: u32) -> RgbaImage {
    let size = sheet_size(sheet);
    let (w, h) = pixel_size(size, dpi);
    let mut canvas = d.canvas().expect("the canvas initialises");
    let mut cache = MeshCache::default();
    canvas.set_scene(Arc::new(prepare(1, list, fonts(), &mut cache)));
    let view = GpuView {
        target_px: (w as u32, h as u32),
        origin_px: (0.0, 0.0),
        px_per_nm: f64::from(dpi) / 25_400_000.0,
    };
    assert!(canvas.render(&view).expect("a render").submitted);
    canvas.read_back().expect("the readback")
}

fn sheet_size(sheet: (i64, i64)) -> SheetSize {
    SheetSize {
        width: Nm(sheet.0),
        height: Nm(sheet.1),
    }
}

/// A pixel some channel of which is at least 128 away from white (section 8 item 3).
fn is_ink(p: &[u8]) -> bool {
    p[..3].iter().any(|&c| c <= 127)
}

/// The clearly-ink pixels of `a` and how many of them have no pixel in `b`'s 3 × 3 neighbourhood with every channel
/// within 128 (section 8 item 3).
fn unmatched(a: &RgbaImage, b: &RgbaImage) -> (usize, usize) {
    let (w, h) = (a.width as i64, a.height as i64);
    let px = |img: &RgbaImage, x: i64, y: i64| {
        let i = ((y * w + x) * 4) as usize;
        [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2]]
    };
    let mut ink = 0;
    let mut missed = 0;
    for y in 0..h {
        for x in 0..w {
            let p = px(a, x, y);
            if !is_ink(&p) {
                continue;
            }
            ink += 1;
            let found = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..w).contains(&nx)
                        && (0..h).contains(&ny)
                        && px(b, nx, ny).iter().zip(p).all(|(q, c)| q.abs_diff(c) <= 128)
                })
            });
            missed += usize::from(!found);
        }
    }
    (missed, ink)
}

/// The bounds of the mean channel difference between the GPU and the CPU image at 150 dpi (see `assert_parity`):
/// measured 0.60 on folio 103 and 6.36 on the dense A3 page (its copies overlap, so the widened hairlines add up),
/// on a hardware DX12 adapter and on WARP alike (cycle 22); the bounds leave a quarter for other adapters' MSAA
/// patterns. A drawing shifted by one pixel or a colour off by 64 on the ink raises the mean several times over.
const MEAN_BOUND_FOLIO_103: f64 = 0.75;
const MEAN_BOUND_DENSE_A3: f64 = 8.0;

/// The mean absolute difference of the red, green and blue channels of two images of one size.
fn mean_difference(a: &RgbaImage, b: &RgbaImage) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;
    for (p, q) in a
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.pixels.as_chunks::<4>().0)
    {
        for c in 0..3 {
            sum += u64::from(p[c].abs_diff(q[c]));
        }
        n += 3;
    }
    sum as f64 / n.max(1) as f64
}

/// The parity rule both ways: at most 1 % of the clearly-ink pixels of each image unmatched in the other.
fn assert_parity(label: &str, gpu: &RgbaImage, cpu: &RgbaImage, mean_bound: f64) {
    assert_eq!((gpu.width, gpu.height), (cpu.width, cpu.height));
    // Beyond section 8's rule (the gate): the mean channel difference over the whole page. The GPU widens hairlines
    // to one physical pixel and its MSAA coverage differs from the CPU's analytic coverage at every edge, so the
    // mean is not 0; a shifted, missing or miscoloured drawing raises it well above the bound.
    let mean = mean_difference(gpu, cpu);
    println!("{label}: mean channel difference {mean:.4}");
    assert!(
        mean <= mean_bound,
        "{label}: mean channel difference {mean:.4} above {mean_bound}"
    );
    for (name, a, b) in [("GPU", gpu, cpu), ("CPU", cpu, gpu)] {
        let (missed, ink) = unmatched(a, b);
        let share = 100.0 * missed as f64 / ink.max(1) as f64;
        println!("{label}: {missed} of {ink} clearly-ink {name} pixels unmatched ({share:.3} %)");
        assert!(ink > 1_000, "{label}: the {name} image has ink");
        assert!(
            share <= 1.0,
            "{label}: {share:.3} % of the {name} ink is unmatched"
        );
    }
}

#[test]
fn b2_folio_103_at_150_dpi_matches_the_cpu_raster() {
    let devices = devices();
    if devices.is_empty() {
        return;
    }
    let tmp = Tmp::new();
    let page = folio_103(&tmp);
    let options = RenderOptions {
        dpi: 150,
        ..RenderOptions::default()
    };
    let (cpu, _) = render(&page.list, sheet_size(page.sheet), &options, fonts(), None).unwrap();
    for d in &devices {
        let gpu = gpu_image(d, &page.list, page.sheet, 150);
        assert_parity(
            &format!("folio 103 at 150 dpi on {:?}", d.summary.kind),
            &gpu,
            &cpu,
            MEAN_BOUND_FOLIO_103,
        );
    }
}

/// The stress variant of the parity on the dense A3 page: in the deep nightly (`deep-nightly.yml`, job
/// `gpu-parity`), not per pull request; `b2_folio_103_at_150_dpi_matches_the_cpu_raster` asserts the same parity on
/// the real page per gate (test audit, 2026-10-29).
#[test]
#[ignore = "nightly: the deep nightly's gpu-parity job runs it (--run-ignored only)"]
fn b2_the_dense_a3_page_matches_the_cpu_raster() {
    let devices = devices();
    if devices.is_empty() {
        return;
    }
    let tmp = Tmp::new();
    let page = folio_103(&tmp);
    let (list, sheet) = dense_a3(&page);
    let options = RenderOptions {
        dpi: 150,
        ..RenderOptions::default()
    };
    let (cpu, _) = render(&list, sheet_size(sheet), &options, fonts(), None).unwrap();
    for d in &devices {
        let gpu = gpu_image(d, &list, sheet, 150);
        assert_parity(
            &format!("dense A3 at 150 dpi on {:?}", d.summary.kind),
            &gpu,
            &cpu,
            MEAN_BOUND_DENSE_A3,
        );
    }
}

#[test]
fn b2_a_filled_0072b2_rectangle_reads_back_its_bytes() {
    for d in devices() {
        rectangle_on(&d);
    }
}

fn rectangle_on(d: &HeadlessDevice) {
    let mut list = DisplayList::new();
    let blue = Color::from_hex("#0072B2").unwrap();
    list.push(Item::path(
        Path::rect(Rect::from_points(
            Pt2::new(Nm(2_000_000), Nm(2_000_000)),
            Pt2::new(Nm(20_000_000), Nm(12_000_000)),
        )),
        None,
        Some(Brush::Solid(blue)),
    ));
    let image = gpu_image(d, &list, (25_000_000, 15_000_000), 96);
    // The texture egui-wgpu samples (Rgba8Unorm, nearest: one texel per physical pixel, tint white): its bytes
    // are what the window shows.
    let at = |x: u32, y: u32| {
        let i = ((y * image.width + x) * 4) as usize;
        [
            image.pixels[i],
            image.pixels[i + 1],
            image.pixels[i + 2],
            image.pixels[i + 3],
        ]
    };
    // The interior, and the first pixel inside each edge: 2 mm and 12 mm are 7.56 and 45.35 pixels at 96 dpi, so
    // columns 8 and 74 and rows 8 and 44 are fully covered.
    for (x, y) in [(20, 20), (40, 30), (70, 40), (8, 20), (74, 20), (20, 8), (20, 44)] {
        let got = at(x, y);
        for (g, want) in got.iter().zip([0x00u8, 0x72, 0xb2, 0xff]) {
            assert!(g.abs_diff(want) <= 1, "({x}, {y}): {got:?}");
        }
    }
    assert_eq!(at(2, 2), [255, 255, 255, 255], "the paper");
}

/// The first column (in row `y`) and the first row (in column `x`) whose pixel is clearly ink.
fn first_ink(image: &RgbaImage, x: u32, y: u32) -> (Option<u32>, Option<u32>) {
    let at = |x: u32, y: u32| {
        let i = ((y * image.width + x) * 4) as usize;
        &image.pixels[i..i + 3]
    };
    (
        (0..image.width).find(|&c| is_ink(at(c, y))),
        (0..image.height).find(|&r| is_ink(at(x, r))),
    )
}

#[test]
fn b2_the_edges_of_a_rectangle_land_on_the_cpu_rasters_pixels() {
    // A filled black rectangle whose left and top edges lie inside pixels at 150 dpi (60.2 and 40.3 pixels), so
    // that both renderers cover the edge pixel by more than half and the pixel before it not at all.
    let px = |v: f64| (v * 25_400_000.0 / 150.0).round() as i64;
    let mut list = DisplayList::new();
    list.push(Item::path(
        Path::rect(Rect::from_points(
            Pt2::new(Nm(px(60.2)), Nm(px(40.3))),
            Pt2::new(Nm(px(160.0)), Nm(px(120.0))),
        )),
        None,
        Some(Brush::BLACK),
    ));
    let sheet = (px(200.0), px(150.0));
    let options = RenderOptions {
        dpi: 150,
        ..RenderOptions::default()
    };
    let (cpu, _) = render(&list, sheet_size(sheet), &options, fonts(), None).unwrap();
    let want = first_ink(&cpu, 100, 80);
    assert_eq!(want, (Some(60), Some(40)), "the CPU raster's edges");
    for d in devices() {
        let gpu = gpu_image(&d, &list, sheet, 150);
        let got = first_ink(&gpu, 100, 80);
        println!(
            "edges on {:?}: GPU column {:?} row {:?}, CPU column {:?} row {:?}",
            d.summary.kind, got.0, got.1, want.0, want.1
        );
        assert_eq!(got, want, "the left and top edges on the same pixels (±0)");
    }
}

#[test]
fn b3_a_second_preparation_of_folio_103_tessellates_nothing_and_glyphs_are_one_mesh_each() {
    let tmp = Tmp::new();
    let page = folio_103(&tmp);
    let mut cache = MeshCache::default();
    let first = prepare(1, &page.list, fonts(), &mut cache);
    assert!(first.stats.tessellated > 0);
    assert_eq!(first.stats.missing_fonts, 0, "the bundled fonts cover folio 103");
    let second = prepare(2, &page.list, fonts(), &mut cache);
    assert_eq!(second.stats.tessellated, 0, "{:?}", second.stats);
    assert_eq!(second.meshes.len(), first.meshes.len());
    // Repeated glyphs: one mesh per (font, glyph) with an outline, however often it is placed.
    let glyph_meshes: BTreeSet<u32> = first.glyphs.iter().map(|g| g.mesh).collect();
    assert!(
        first.glyphs.len() > glyph_meshes.len(),
        "folio 103 repeats glyphs"
    );
    let mut distinct = BTreeSet::new();
    collect_glyphs(&page.list.items, &mut distinct);
    assert!(glyph_meshes.len() <= distinct.len());
    println!(
        "folio 103: {} meshes, {} glyph instances of {} glyph meshes ({} distinct glyphs), {} runs",
        first.meshes.len(),
        first.glyphs.len(),
        glyph_meshes.len(),
        distinct.len(),
        first.runs.len()
    );
}

fn collect_glyphs(items: &[Item], out: &mut BTreeSet<(FontKey, u32)>) {
    for item in items {
        match &item.kind {
            ItemKind::Glyphs(run) => {
                for g in &run.glyphs {
                    out.insert((run.font, g.id));
                }
            }
            ItemKind::Group(g) => collect_glyphs(&g.items, out),
            _ => {}
        }
    }
}

#[test]
fn b4_at_10_per_cent_the_bars_are_the_runs_below_2_pixels() {
    let tmp = Tmp::new();
    let page = folio_103(&tmp);
    let mut cache = MeshCache::default();
    let scene = Arc::new(prepare(1, &page.list, fonts(), &mut cache));
    // 10 % at 1 pixel per point: 9.6 dpi.
    let px_per_nm = 0.1 * 96.0 / 25_400_000.0;
    let below = scene.runs.iter().filter(|r| r.em_nm * px_per_nm < 2.0).count();
    assert_eq!(scene.bar_count(px_per_nm), below);
    assert!(below > 0, "folio 103 has text below 2 pixels at 10 %");
    // At 400 % no run is a bar.
    assert_eq!(scene.bar_count(4.0 * 96.0 / 25_400_000.0), 0);
    for d in devices() {
        let mut canvas = d.canvas().expect("the canvas initialises");
        canvas.set_scene(Arc::clone(&scene));
        let out = canvas
            .render(&GpuView {
                target_px: (200, 150),
                origin_px: (0.0, 0.0),
                px_per_nm,
            })
            .expect("a render");
        assert_eq!((out.bars, out.glyph_runs), (below, scene.runs.len() - below));
    }
}

/// A canvas page of `list` as the shell would show it in session `session`.
fn page_of(session: u64, rev: u64, folio: &str, list: &Arc<DisplayList>) -> Arc<CanvasPage> {
    let mut page = CanvasPage::new(
        rev,
        folio,
        "test",
        (297_000_000, 210_000_000),
        Arc::clone(list),
        None,
    );
    page.session = session;
    Arc::new(page)
}

/// Asks `host` for `page` until its scene is ready, blocking on the worker between the asks (at most a minute per
/// scene): the last answer of `sync_scene`.
fn until_ready(
    host: &mut ew_ui::gpu::GpuHost,
    ctx: &egui::Context,
    page: &Arc<CanvasPage>,
    fonts: &ew_render2d::pool::SharedFonts,
) -> (bool, bool) {
    let mut got = host.sync_scene(ctx, page, fonts);
    for _ in 0..8 {
        if got == (true, true) {
            break;
        }
        host.await_scene_for_tests(page, std::time::Duration::from_secs(60));
        got = host.sync_scene(ctx, page, fonts);
    }
    got
}

/// The answer of `sync_scene` for `page` while the worker is held, so that its scene cannot have arrived (asked
/// twice: the answer does not change while the scene prepares); the worker is released afterwards.
fn while_preparing(
    host: &mut ew_ui::gpu::GpuHost,
    ctx: &egui::Context,
    page: &Arc<CanvasPage>,
    fonts: &ew_render2d::pool::SharedFonts,
) -> (bool, bool) {
    let hold = host.preparation_hold(ctx);
    hold.set(true);
    let first = host.sync_scene(ctx, page, fonts);
    let second = host.sync_scene(ctx, page, fonts);
    hold.set(false);
    assert_eq!(first, second, "the answer holds while the scene prepares");
    first
}

/// The GPU scene is keyed by (session, revision, folio): a new revision of the page on show stands in until it is
/// ready, another folio or another project's session never does, and a late, older scene is dropped (the fix
/// round of WP-S2.02 part 2).
#[test]
fn the_gpu_scene_is_keyed_by_session_revision_and_folio() {
    let tmp = Tmp::new();
    let folio = folio_103(&tmp);
    let fonts: ew_render2d::pool::SharedFonts = Arc::new(fonts().clone());
    let small = Arc::new({
        let mut l = DisplayList::new();
        l.push(Item::path(
            Path::rect(Rect::from_points(
                Pt2::new(Nm(1_000_000), Nm(1_000_000)),
                Pt2::new(Nm(9_000_000), Nm(9_000_000)),
            )),
            None,
            Some(Brush::BLACK),
        ));
        l
    });
    for d in devices() {
        let ctx = egui::Context::default();
        let mut host = ew_ui::gpu::GpuHost::new();
        host.set_canvas_for_tests(d.canvas().expect("the canvas initialises"));
        // Session 1, revision 1, folio 103.
        let p1 = page_of(1, 1, "103", &small);
        assert_eq!(until_ready(&mut host, &ctx, &p1, &fonts), (true, true));
        // A new revision of the same folio: while it prepares (the worker held) the old scene stands in, not ready;
        // then the new one is.
        let p2 = page_of(1, 2, "103", &folio.list);
        assert_eq!(while_preparing(&mut host, &ctx, &p2, &fonts), (true, false));
        assert_eq!(until_ready(&mut host, &ctx, &p2, &fonts), (true, true));
        // Another folio: nothing stands in.
        let p3 = page_of(1, 2, "104", &small);
        assert_eq!(while_preparing(&mut host, &ctx, &p3, &fonts), (false, false));
        assert_eq!(until_ready(&mut host, &ctx, &p3, &fonts), (true, true));
        // Another project at the same revision and folio: a new scene, the old one never stands in.
        let p4 = page_of(2, 2, "104", &folio.list);
        assert_eq!(while_preparing(&mut host, &ctx, &p4, &fonts), (false, false));
        assert_eq!(until_ready(&mut host, &ctx, &p4, &fonts), (true, true));
        let shown = host.canvas().unwrap().scene().unwrap().id;
        assert_eq!(Some(shown), host.requested_scene());
        // A late, older scene is dropped: the newest request wins.
        let p5 = page_of(2, 3, "104", &small);
        let _ = host.sync_scene(&ctx, &p5, &fonts);
        let newest = host.requested_scene().unwrap();
        let mut cache = MeshCache::default();
        let older = Arc::new(prepare(newest - 1, &small, fonts.as_ref(), &mut cache));
        host.deliver_for_tests(Arc::clone(&older), &p5);
        assert!(
            !Arc::ptr_eq(host.canvas().unwrap().scene().unwrap(), &older),
            "the late scene is not shown"
        );
        assert_eq!(until_ready(&mut host, &ctx, &p5, &fonts), (true, true));
        assert_eq!(host.canvas().unwrap().scene().unwrap().id, newest);
        // Releasing (the project closed) forgets the scene: a page of an equal key is asked for again.
        host.release();
        assert!(host.canvas().unwrap().scene().is_none());
        assert_eq!(while_preparing(&mut host, &ctx, &p5, &fonts), (false, false));
        assert!(host.requested_scene() > Some(newest));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Raster images: a page with a PNG or JPEG item is drawn by the GPU path's textured-quad pipeline (FT-480)

/// A PNG of `w × h` opaque red pixels (synthetic).
fn red_png(w: u32, h: u32) -> Vec<u8> {
    ew_render_cpu::encode_png(
        &RgbaImage {
            width: w,
            height: h,
            pixels: [255u8, 0, 0, 255].repeat((w * h) as usize),
        },
        None,
    )
}

/// The pixels of `image` that are clearly red.
fn red_pixels(image: &RgbaImage) -> usize {
    image
        .pixels
        .chunks(4)
        .filter(|p| p[0] > 200 && p[1] < 60 && p[2] < 60)
        .count()
}

fn vulkan() -> ew_ui::gpu::AdapterSummary {
    ew_ui::gpu::AdapterSummary {
        kind: ew_ui::gpu::DeviceKind::Discrete,
        api: ew_ui::gpu::GpuApi::Vulkan,
        name: "test GPU".into(),
    }
}

/// A host on which the switch rule chose the GPU and whose canvas is `d`'s, and a shell to label.
fn gpu_host(d: &HeadlessDevice, tmp: &Tmp) -> (ew_ui::gpu::GpuHost, ShellModel) {
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    let mut host = ew_ui::gpu::GpuHost::new();
    host.simulate(vulkan(), Ok(()));
    host.update(&mut shell);
    host.set_canvas_for_tests(d.canvas().expect("the canvas initialises"));
    assert!(host.drawing(), "the rule chose the GPU");
    (host, shell)
}

/// The canvas's CPU path: the whole sheet as one `render_region` with the list's `ItemBounds` (what the tile pool
/// runs, culling included), and its report.
fn tiles(list: &DisplayList, sheet: (i64, i64), dpi: u32) -> (RgbaImage, ew_render_cpu::RenderReport) {
    let (w, h) = pixel_size(sheet_size(sheet), dpi);
    let bounds = ItemBounds::of(list, None);
    let options = RegionOptions {
        dpi,
        x: 0,
        y: 0,
        width: w as u32,
        height: h as u32,
        background: Color::WHITE,
        surround: Color::WHITE,
    };
    render_region(list, Some(&bounds), sheet_size(sheet), &options, fonts(), None).unwrap()
}

/// The page drawn by `host`'s own path (its preparation worker, the scene keyed by the page, the canvas render) at
/// 96 dpi, the sheet's origin at the target's corner, read back.
fn host_image(
    host: &mut ew_ui::gpu::GpuHost,
    ctx: &egui::Context,
    page: &Arc<CanvasPage>,
    fonts: &ew_render2d::pool::SharedFonts,
) -> RgbaImage {
    assert_eq!(until_ready(host, ctx, page, fonts), (true, true));
    let (w, h) = pixel_size(sheet_size(page.sheet), 96);
    let view = GpuView {
        target_px: (w as u32, h as u32),
        origin_px: (0.0, 0.0),
        px_per_nm: 96.0 / 25_400_000.0,
    };
    // Without eframe's render state the host registers no texture, but the canvas renders.
    let _ = host.render(&view);
    let canvas = host.canvas().expect("the GPU canvas");
    assert!(canvas.fault().is_none());
    canvas.read_back().expect("the readback")
}

/// The red pixels of `image` (96 dpi) outside the rectangle `x_mm × y_mm` (widened by a third of a pixel for
/// anti-aliasing).
fn red_outside(image: &RgbaImage, x_mm: (f64, f64), y_mm: (f64, f64)) -> usize {
    let k = 96.0 / 25.4;
    let (w, h) = (image.width as usize, image.height as usize);
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let i = (y * w + x) * 4;
            image.pixels[i] > 200 && image.pixels[i + 1] < 60 && image.pixels[i + 2] < 60
        })
        .filter(|&(x, y)| {
            let (xm, ym) = (x as f64 / k, y as f64 / k);
            let slack = 0.1;
            !((x_mm.0 - slack..x_mm.1 + slack).contains(&xm)
                && (y_mm.0 - slack..y_mm.1 + slack).contains(&ym))
        })
        .count()
}

/// A page with a PNG item is drawn by the GPU path (FT-480; cycle 29 routed such a page to the CPU tiles, ID-679):
/// the host keeps the GPU for it and the status bar names the GPU; the host's own path (worker, scene, render) draws
/// the PNG's pixels inside the item's rectangle, also with the image inside a group, within the parity rule of
/// WP-S2.02 §8 item 3 against the CPU tiles.
#[test]
fn a_page_with_a_png_item_is_drawn_by_the_gpu_path_with_its_pixels() {
    let devices = devices();
    let tmp = Tmp::new();
    let fonts: ew_render2d::pool::SharedFonts = Arc::new(fonts().clone());
    let at = |x: i64, y: i64| Pt2::new(Nm(x), Nm(y));
    let rect = Rect::from_points(at(20_000_000, 20_000_000), at(60_000_000, 30_000_000));
    let png = Item::image(
        Arc::new(ew_ui::ew_viewmodel::ew_api::output::ew_dl::ImageData {
            sha256: [9; 32],
            format: ew_ui::ew_viewmodel::ew_api::output::ew_dl::ImageFormat::Png,
            bytes: Arc::from(red_png(8, 2)),
        }),
        rect,
    );
    let mut with = DisplayList::new();
    with.push(png.clone());
    // The image also inside a group, as the scene builds it under a translation.
    let mut grouped = DisplayList::new();
    grouped.push(Item::group(
        Some(Transform::from_translation(at(0, 0))),
        None,
        vec![png],
    ));
    let (with, grouped) = (Arc::new(with), Arc::new(grouped));
    for d in &devices {
        let (mut host, mut shell) = gpu_host(d, &tmp);
        let ctx = egui::Context::default();
        for (folio, list) in [("1", &with), ("2", &grouped)] {
            let page = page_of(1, 1, folio, list);
            host.update(&mut shell);
            assert!(host.drawing(), "a page with a PNG item: the GPU path");
            assert_eq!(
                shell.canvas_backend().label(),
                "GPU Vulkan",
                "the status bar names the GPU"
            );
            let gpu = host_image(&mut host, &ctx, &page, &fonts);
            let stats = host.canvas().unwrap().scene().unwrap().stats;
            assert_eq!((stats.images, stats.images_skipped), (1, 0));
            // 40 × 10 mm at 96 dpi is about 151 × 38 pixels of the PNG's red, none outside the rectangle.
            let red = red_pixels(&gpu);
            assert!(red >= 5_000, "{red} red pixels on the GPU path");
            assert_eq!(red_outside(&gpu, (20.0, 60.0), (20.0, 30.0)), 0);
            let (cpu, report) = tiles(list, page.sheet, 96);
            assert_eq!(report.skipped_images, 0);
            assert_parity(
                &format!("PNG page {folio} on {:?}", d.summary.kind),
                &gpu,
                &cpu,
                MEAN_BOUND_FOLIO_103,
            );
        }
    }
}

/// The company logo as a PNG is a raster image in the title block's list: that page keeps the GPU path too (FT-480)
/// and the host's GPU image shows the logo as the CPU tiles do.
#[test]
fn a_title_block_page_with_a_png_logo_is_drawn_by_the_gpu_path() {
    let devices = devices();
    let Some(d) = devices.first() else {
        return;
    };
    let tmp = Tmp::new();
    let fonts: ew_render2d::pool::SharedFonts = Arc::new(fonts().clone());
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    assert_eq!(
        shell.dispatch(
            "file.open",
            &json!({"dir": dir.to_string_lossy(), "read_only": false})
        ),
        Outcome::Done
    );
    assert_eq!(
        shell.dispatch("page.goto", &json!({"folio": "101"})),
        Outcome::Done
    );
    let (mut host, _) = gpu_host(d, &tmp);
    let ctx = egui::Context::default();
    let plain = Arc::clone(shell.canvas().expect("a canvas").page());
    let plain_gpu = host_image(&mut host, &ctx, &plain, &fonts);
    assert_eq!(red_pixels(&plain_gpu), 0, "the golden page has no red");
    // The page's own GPU/CPU difference at 96 dpi (text and widened hairlines), which the logo adds to.
    let plain_mean = mean_difference(&plain_gpu, &tiles(&plain.list, plain.sheet, 96).0);
    let logo = ew_ui::ew_viewmodel::ew_api::base64_encode(&red_png(64, 32));
    assert_eq!(
        shell.dispatch(
            "command.run",
            &json!({"cmd": "project.company.set", "params": {"logo": logo}})
        ),
        Outcome::Done
    );
    assert_eq!(
        shell.dispatch("page.goto", &json!({"folio": "101"})),
        Outcome::Done
    );
    let page = Arc::clone(shell.canvas().expect("a canvas").page());
    assert!(host.drawing(), "a PNG logo in the title block: the GPU path");
    let gpu = host_image(&mut host, &ctx, &page, &fonts);
    assert_eq!(host.canvas().unwrap().scene().unwrap().stats.images, 1);
    let (cpu, report) = tiles(&page.list, page.sheet, 96);
    assert_eq!(report.skipped_images, 0);
    let (on_gpu, on_cpu) = (red_pixels(&gpu), red_pixels(&cpu));
    assert!(on_gpu >= 100, "{on_gpu} red pixels: the logo on the GPU path");
    assert!(
        on_gpu.abs_diff(on_cpu) * 10 <= on_cpu,
        "{on_gpu} red pixels on the GPU path against {on_cpu} on the CPU path"
    );
    // The parity rule, and the logo adds at most 0.05 to the page's mean channel difference.
    println!("folio 101 without the logo: mean channel difference {plain_mean:.4}");
    assert_parity("folio 101 with a PNG logo", &gpu, &cpu, plain_mean + 0.05);
    shell.shutdown();
}
