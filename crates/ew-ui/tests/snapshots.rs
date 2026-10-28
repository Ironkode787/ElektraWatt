// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chrome snapshots of WP-S2.17 (§11 item 3, DL6, DK7): the main window on Golden A folio 101 at 1280 × 800 in
//! every theme and density, and the component gallery at ppp 1 and 2, rendered offscreen (`offscreen`) and written
//! as PNG files under `target/ew-ui-snapshots/` (or `EW_SNAPSHOT_DIR`) for the reviewer, who compares them with the
//! mockup side by side.
//!
//! The pixel gate (D7 as amended: no `egui_kittest` render feature): the main window in light and dark at both
//! densities and the gallery in the four themes at ppp 1 are compared with the blessed PNG files under
//! `tests/expected/snapshots/`. A pixel differs when a channel differs by more than
//! [`offscreen::CHANNEL_TOLERANCE`]; an image fails with more than [`offscreen::MAX_DIFFERING_PIXELS`] differing
//! pixels, and a diff image (differences in red) is written next to the output. `EW_BLESS=1` writes the blessed
//! files. Behind the `snapshots` feature and on Windows only (DX12 WARP: one platform keeps the pixels stable); no
//! adapter there is a failure.

#![cfg(all(feature = "snapshots", windows))]

mod gallery;
mod offscreen;
mod support;

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::Harness;
use ew_ui::app::{App, setup};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;
use support::{Scripted, Tmp};

/// Where the images go.
// Program boundary of the test binary: the reviewer's output directory; no image depends on it.
#[allow(clippy::disallowed_methods)]
fn out_dir() -> PathBuf {
    std::env::var_os("EW_SNAPSHOT_DIR").map_or_else(
        || {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../target/ew-ui-snapshots"
            ))
        },
        PathBuf::from,
    )
}

/// Whether this run writes the blessed images.
// Program boundary of the test binary: the bless switch of the repository's tests (`EW_BLESS=1`).
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

fn device() -> ew_render2d::gpu::HeadlessDevice {
    offscreen::device().expect("no adapter: on Windows DX12 WARP at least must be found")
}

/// Writes `image` for review as `name` and, when `gated`, compares it with the blessed file (or blesses it). Returns
/// the failure, if any.
fn record(name: &str, image: &ew_render_cpu::RgbaImage, gated: bool) -> Option<String> {
    let out = out_dir();
    offscreen::write_png(image, &out.join(format!("{name}.png")));
    if !gated {
        return None;
    }
    let blessed = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/snapshots"))
        .join(format!("{name}.png"));
    if bless() {
        offscreen::write_png(image, &blessed);
        return None;
    }
    let Some(old) = offscreen::read_png(&blessed) else {
        return Some(format!("{name}: no blessed image (run with EW_BLESS=1)"));
    };
    match offscreen::compare(&old, image) {
        None => Some(format!(
            "{name}: {}x{} against the blessed {}x{}",
            image.width, image.height, old.width, old.height
        )),
        Some((count, diff)) if count > offscreen::MAX_DIFFERING_PIXELS => {
            let path = out.join(format!("{name}.diff.png"));
            offscreen::write_png(&diff, &path);
            Some(format!(
                "{name}: {count} pixels differ (at most {} may); see {}",
                offscreen::MAX_DIFFERING_PIXELS,
                path.display()
            ))
        }
        Some(_) => None,
    }
}

/// The main window on a copy of Golden A in `theme` and `density`, with the temporary directory that holds the copy:
/// the caller drops it after the harness, so no copy is left in the temporary directory.
fn main_window(theme: &str, density: &str) -> (Harness<'static, App>, offscreen::Recorder, Tmp) {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(
        shell.dispatch("settings.set", &json!({"key": "theme", "value": theme})),
        Outcome::Done
    );
    // A base without the density setting refuses the key; the image is then Comfortable.
    let _ = shell.dispatch("settings.set", &json!({"key": "density", "value": density}));
    let recorder = offscreen::Recorder::default();
    let app = App::new(shell, Box::new(Scripted::default()), Some(dir));
    let mut h = Harness::builder()
        .with_size(Vec2::new(1280.0, 800.0))
        .with_pixels_per_point(1.0)
        .with_context_setup(setup)
        .renderer(recorder.clone())
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    h.run_ok();
    support::wait_for_page(&mut h);
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.goto", &json!({"folio": "101"})),
        Outcome::Done
    );
    h.run_ok();
    support::wait_for_page(&mut h);
    h.run_ok();
    (h, recorder, tmp)
}

#[test]
fn the_main_window_in_every_theme_and_density() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark", "hc-dark", "hc-light"] {
        for density in ["comfortable", "compact"] {
            let (h, recorder, tmp) = main_window(theme, density);
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            assert_eq!((image.width, image.height), (1280, 800));
            let gated = matches!(theme, "light" | "dark");
            failures.extend(record(&format!("main-{theme}-{density}"), &image, gated));
            // The project closes before its directory is removed.
            drop(h);
            drop(tmp);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// For review only (not gated): the Devices navigator in table mode and the history panel, light and dark.
#[test]
fn the_table_and_the_history_panel_for_review() {
    let dev = device();
    for theme in ["light", "dark"] {
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        let shell = h.state_mut().shell_mut();
        for (action, params) in [
            ("navigator.show", json!({"kind": "devices"})),
            ("navigator.table", json!({"kind": "devices"})),
            ("history.show", json!({})),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        assert_eq!(record(&format!("table-history-{theme}"), &image, false), None);
        drop(h);
        drop(tmp);
    }
}

#[test]
fn the_component_gallery_in_every_theme_at_ppp_1_and_2() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark", "hc-dark", "hc-light"] {
        for (ppp, gated) in [(1.0_f32, true), (2.0, false)] {
            let recorder = offscreen::Recorder::default();
            let mut h = Harness::builder()
                .with_size(Vec2::new(640.0, 560.0))
                .with_pixels_per_point(ppp)
                .with_context_setup(setup)
                .renderer(recorder.clone())
                .build_ui_state(move |ui, _: &mut ()| gallery::show(ui, theme), ());
            h.run_ok();
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            failures.extend(record(&format!("gallery-{theme}-ppp{ppp}"), &image, gated));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
