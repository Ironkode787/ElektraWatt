// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page canvas without rendering (WP-S2.02 part 1, acceptance checks A9, A10, A11) and the tile parity of the
//! CPU path: keys, wheel, drag and pointer reach the canvas through the registry, the status bar follows, the
//! canvas is one named AccessKit node, view steps re-execute no query, and tiles stitched at one resolution match
//! the whole-page raster.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Event, Key, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, TouchPhase, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_render_cpu::{FontBytes, FontResolver, RenderOptions, SheetSize, pixel_size, render};
use ew_render2d::pool::Scene;
use ew_render2d::tiles::{TILE_SIZE, TileKey};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::canvas::{rect_centre, zoom_in_step};
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::FontKey;
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::ew_units::Nm;
use ew_ui::ew_viewmodel::ew_api::output::font_bytes;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::json;
use support::{Scripted, Tmp, harness, settle, wait_for_page};

/// A harness with a copy of Golden A open on folio 103, its canvas settled and drawn.
fn on_103(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.goto", &json!({"folio": "103"})),
        Outcome::Done
    );
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn zoom(h: &Harness<'_, App>) -> f64 {
    h.state().shell().canvas().unwrap().zoom()
}

fn centre(h: &Harness<'_, App>) -> (i64, i64) {
    h.state().shell().canvas().unwrap().centre()
}

fn status(h: &Harness<'_, App>) -> Vec<String> {
    h.state().shell().status_fields(false)
}

fn canvas_rect(h: &Harness<'_, App>) -> egui::Rect {
    h.get_by_role(egui::Role::Canvas).rect()
}

#[test]
fn a10_ctrl_plus_zooms_the_canvas_and_leaves_the_ui_scale() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let fit = zoom(&h);
    assert!(h.state().shell().canvas().unwrap().is_fitted());
    h.key_press_modifiers(Modifiers::COMMAND, Key::Plus);
    settle(&mut h);
    assert_eq!(zoom(&h), zoom_in_step(fit));
    assert_eq!(h.ctx.zoom_factor(), 1.0, "egui's own keyboard zoom is off");
    let percent = format!("{} %", (zoom(&h) * 100.0).round());
    assert!(status(&h).contains(&percent), "{:?}", status(&h));
    assert!(
        h.query_by_label(&percent).is_some(),
        "the status bar shows {percent}"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Equals);
    settle(&mut h);
    assert_eq!(zoom(&h), zoom_in_step(zoom_in_step(fit)));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Minus);
    settle(&mut h);
    assert_eq!(zoom(&h), zoom_in_step(fit));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num0);
    settle(&mut h);
    assert_eq!(zoom(&h), 1.0);
    assert!(status(&h).contains(&"100 %".to_owned()));
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a10_home_fits_and_ctrl_arrow_right_moves_the_view() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let fit = zoom(&h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Plus);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Plus);
    settle(&mut h);
    assert!(zoom(&h) > fit);
    let (x0, y0) = centre(&h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowRight);
    settle(&mut h);
    let c = h.state().shell().canvas().unwrap();
    let step = (c.pane().0 / 8.0 / c.points_per_nm()).trunc() as i64;
    assert_eq!(centre(&h), (x0 + step, y0), "one eighth of the pane to the right");
    h.key_press(Key::Home);
    settle(&mut h);
    assert_eq!(zoom(&h), fit);
    assert!(h.state().shell().canvas().unwrap().is_fitted());
    assert_eq!(centre(&h), (148_500_000, 105_000_000));
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a10_a_line_unit_wheel_event_zooms_about_the_pointer() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let at = canvas_rect(&h).center() + Vec2::new(120.0, -60.0);
    h.hover_at(at);
    settle(&mut h);
    let before = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .pointer()
        .expect("the pointer is on the canvas");
    let fit = zoom(&h);
    h.event(Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: Vec2::new(0.0, 1.0),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    });
    settle(&mut h);
    assert_eq!(zoom(&h), zoom_in_step(fit), "one ladder step per line");
    // The sheet point under the pointer is unchanged (to within the rounding of a point position).
    h.hover_at(at);
    settle(&mut h);
    let after = h.state().shell().canvas().unwrap().pointer().unwrap();
    assert!(
        (after.0 - before.0).abs() <= 20 && (after.1 - before.1).abs() <= 20,
        "{before:?} -> {after:?}"
    );
    // Half lines accumulate: two halves are one step out.
    for _ in 0..2 {
        h.event(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: Vec2::new(0.0, -0.5),
            phase: TouchPhase::Move,
            source: MouseWheelSource::Wheel,
            modifiers: Modifiers::NONE,
        });
        settle(&mut h);
    }
    assert_eq!(
        zoom(&h),
        ew_ui::ew_viewmodel::canvas::zoom_out_step(zoom_in_step(fit)),
        "the two halves are one ladder step out"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a10_a_middle_button_drag_pans() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num0);
    settle(&mut h);
    let (x0, y0) = centre(&h);
    let a = canvas_rect(&h).center();
    let b = a + Vec2::new(-80.0, 40.0);
    let button = |pos: Pos2, pressed: bool| Event::PointerButton {
        pos,
        button: PointerButton::Middle,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.event(Event::PointerMoved(a));
    h.event(button(a, true));
    h.step();
    for i in 1..=4 {
        h.event(Event::PointerMoved(a + (b - a) * (i as f32 / 4.0)));
        h.step();
    }
    h.event(button(b, false));
    settle(&mut h);
    let k = h.state().shell().canvas().unwrap().points_per_nm();
    let (x1, y1) = centre(&h);
    // The content followed the pointer: the centre moved the other way, by the drag in nm (each step truncated).
    let want = ((80.0 / k) as i64, (-40.0 / k) as i64);
    assert!(
        (x1 - x0 - want.0).abs() <= 8 && (y1 - y0 - want.1).abs() <= 8,
        "{:?}",
        (x1 - x0, y1 - y0)
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a10_view_grid_and_the_cursor_field() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    assert!(status(&h).contains(&"QET 10 px".to_owned()), "{:?}", status(&h));
    h.get_by_label("View").click();
    settle(&mut h);
    h.get_by_label("Grid").click();
    settle(&mut h);
    assert!(!h.state().shell().grid_on());
    assert!(
        h.query_by_label("Grid off").is_some(),
        "the status bar says the grid is off"
    );
    // The cursor field follows the pointer, and goes when the pointer leaves the canvas.
    let rect = canvas_rect(&h);
    h.hover_at(rect.center());
    settle(&mut h);
    let first = status(&h)
        .into_iter()
        .find(|f| f.starts_with("x "))
        .expect("a cursor field");
    assert!(first.ends_with(" mm"), "{first}");
    assert!(h.query_by_label(&first).is_some());
    h.hover_at(rect.center() + Vec2::new(50.0, 30.0));
    settle(&mut h);
    let second = status(&h).into_iter().find(|f| f.starts_with("x ")).unwrap();
    assert_ne!(first, second);
    h.hover_at(Pos2::new(2.0, 2.0));
    settle(&mut h);
    assert!(
        !status(&h).iter().any(|f| f.starts_with("x ")),
        "{:?}",
        status(&h)
    );
    assert_eq!(h.state().shell().canvas().unwrap().pointer(), None);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a10_the_canvas_is_one_named_canvas_node() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let node = h.get_by_role(egui::Role::Canvas);
    let title = h.state().shell().canvas().unwrap().page().title.clone();
    assert!(!title.is_empty() && !title.starts_with("103 "), "{title}");
    assert_eq!(
        node.accesskit_node().label().as_deref(),
        Some(format!("Page 103, {title}").as_str())
    );
    let description = node
        .accesskit_node()
        .description()
        .map(|d| d.to_string())
        .unwrap_or_default();
    assert!(description.starts_with("Read-only view, zoom "), "{description}");
    assert!(description.ends_with(" %"), "{description}");
    // The walker passes (settle runs it) and a click selects nothing.
    node.click();
    settle(&mut h);
    assert!(h.state().shell().canvas().unwrap().hover().is_none() || h.state().shell().dialog().is_none());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a11_the_statistics_hold_one_sample_per_canvas_frame() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let before = h.state().canvas_stats().len();
    for _ in 0..5 {
        h.step();
    }
    let stats = h.state().canvas_stats();
    assert_eq!(stats.len(), before + 5);
    // Summaries exist; their values are reported, never asserted (WP-S2.02 section 9).
    let frame = stats.frame();
    assert!(frame.max >= frame.p99 && frame.p99 >= frame.p50);
    println!(
        "canvas frame statistics over {} frames (debug build, kittest, not a measurement): frame p50 {:.3} ms, p99 \
         {:.3} ms; render p99 {:.3} ms",
        stats.len(),
        frame.p50,
        frame.p99,
        stats.phase(ew_render2d::stats::Phase::Render).p99
    );
    h.get_by_label("View").click();
    settle(&mut h);
    h.get_by_label("Frame statistics").click();
    settle(&mut h);
    assert!(h.state().shell().frame_stats_shown());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a9_after_the_first_frame_view_steps_query_nothing_and_request_no_cached_tile() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let shell = h.state().shell();
    let calls = shell.page_content_calls();
    let (_, re) = shell.memo_totals().unwrap();
    let scenes = h.state().canvas_counters().scenes;
    let entities = shell.canvas().unwrap().page().entities.clone();
    let rect = canvas_rect(&h);
    for i in 0..200usize {
        let shell = h.state_mut().shell_mut();
        let _ = match i % 4 {
            0 => shell.dispatch("view.pan", &json!({"dx": 17, "dy": -9})),
            1 => shell.dispatch(
                "view.zoom_to",
                &json!({"percent": 60 + (i % 9) * 35, "x_mm": 140, "y_mm": 90}),
            ),
            2 => {
                let (x, y) = rect_centre(entities[i % entities.len()].1);
                shell.dispatch(
                    "canvas.pointer",
                    &json!({"x_mm": x as f64 / 1e6, "y_mm": y as f64 / 1e6}),
                )
            }
            _ => {
                h.hover_at(rect.center() + Vec2::new((i % 13) as f32 * 7.0, (i % 5) as f32 * 11.0));
                Outcome::Done
            }
        };
        h.step();
    }
    let shell = h.state().shell();
    assert_eq!(shell.page_content_calls(), calls, "no further page_content call");
    assert_eq!(shell.memo_totals().unwrap().1 - re, 0, "no memo re-execution");
    let counters = h.state().canvas_counters();
    assert_eq!(
        counters.scenes, scenes,
        "no further display list fetched for the canvas"
    );
    assert_eq!(
        counters.requested_while_cached, 0,
        "no tile requested while cached"
    );
    println!("{counters:?}");
    h.state_mut().shell_mut().shutdown();
}

struct Fonts(&'static BTreeMap<FontKey, FontBytes>);

impl FontResolver for Fonts {
    fn font(&self, key: FontKey) -> Option<FontBytes> {
        self.0.font(key)
    }
}

/// The share of pixels (per cent) where a channel differs by more than 16, and the largest channel difference.
fn compare(a: &[u8], b: &[u8]) -> (f64, u8) {
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
fn tile_parity_the_stitched_tiles_of_folio_103_match_the_whole_page_raster() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    settle(&mut h);
    let shell = h.state_mut().shell_mut();
    assert_eq!(
        shell.dispatch("page.goto", &json!({"folio": "103"})),
        Outcome::Done
    );
    let page = Arc::clone(shell.canvas().unwrap().page());
    shell.shutdown();
    let sheet = SheetSize {
        width: Nm(page.sheet.0),
        height: Nm(page.sheet.1),
    };
    let fonts: Arc<dyn FontResolver + Send + Sync> = Arc::new(Fonts(font_bytes()));
    let scene = Scene::new(1, Arc::clone(&page.list), sheet, fonts);
    for dpi in [150, 192] {
        let (w, hgt) = pixel_size(sheet, dpi);
        let (w, hgt) = (w as usize, hgt as usize);
        let mut stitched = vec![0u8; w * hgt * 4];
        for ty in 0..(hgt as u32).div_ceil(TILE_SIZE) {
            for tx in 0..(w as u32).div_ceil(TILE_SIZE) {
                let key = TileKey {
                    scene: 1,
                    dpi,
                    tx,
                    ty,
                };
                let tile = scene.render_tile(key).unwrap();
                let (x0, y0) = (tx as usize * 256, ty as usize * 256);
                for row in 0..tile.height as usize {
                    let from = row * tile.width as usize * 4;
                    let to = ((y0 + row) * w + x0) * 4;
                    stitched[to..to + tile.width as usize * 4]
                        .copy_from_slice(&tile.pixels[from..from + tile.width as usize * 4]);
                }
            }
        }
        let options = RenderOptions {
            dpi,
            ..RenderOptions::default()
        };
        let (whole, _) = render(&page.list, sheet, &options, font_bytes(), None).unwrap();
        let (percent, largest) = compare(&stitched, &whole.pixels);
        println!(
            "folio 103 at {dpi} dpi: {percent:.4} % of the pixels of the stitched tiles differ by more than 16 from the \
             whole-page raster, largest channel difference {largest}"
        );
        assert!(percent <= 0.5, "{percent:.4} % differ at {dpi} dpi");
    }
}
