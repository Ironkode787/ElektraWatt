// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU canvas against the CPU raster, render to texture on a headless device (WP-S2.02 part 2, B2 to B4): the
//! parity rule of section 8 item 3 on folio 103 at `D = 150` and on the dense A3 page, a filled #0072B2 rectangle
//! reads back its bytes, a second preparation tessellates nothing and repeated glyphs are one mesh each, and below
//! 2 pixels of em a glyph run is a bar. On Windows an adapter must be found (DX12 WARP at least); elsewhere a host
//! without one prints `skipped: no adapter` and passes (section 8 item 4).

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_render_cpu::{FontBytes, RenderOptions, RgbaImage, SheetSize, pixel_size, render};
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

/// The headless devices (the default adapter and the fallback adapter where they differ), with a note where a host
/// has none; on Windows one must be found.
fn devices() -> Vec<HeadlessDevice> {
    let all = HeadlessDevice::all();
    if all.is_empty() {
        if cfg!(windows) {
            panic!("on Windows an adapter must be found (DX12 WARP at least)");
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

/// The parity rule both ways: at most 1 % of the clearly-ink pixels of each image unmatched in the other.
fn assert_parity(label: &str, gpu: &RgbaImage, cpu: &RgbaImage) {
    assert_eq!((gpu.width, gpu.height), (cpu.width, cpu.height));
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
        );
    }
}

#[test]
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
    for (x, y) in [(20, 20), (40, 30), (70, 40)] {
        let got = at(x, y);
        for (g, want) in got.iter().zip([0x00u8, 0x72, 0xb2, 0xff]) {
            assert!(g.abs_diff(want) <= 1, "({x}, {y}): {got:?}");
        }
    }
    assert_eq!(at(2, 2), [255, 255, 255, 255], "the paper");
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
