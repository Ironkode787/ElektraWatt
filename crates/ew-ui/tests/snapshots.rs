// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chrome snapshots of WP-S2.17 (§11 item 3, DL6, DK7): the main window on Golden A folio 101 at 1280 × 800 in
//! every theme and density, and the component gallery at ppp 1 and 2, rendered offscreen (`offscreen`) and written
//! as PNG files under `target/ew-ui-snapshots/` (or `EW_SNAPSHOT_DIR`) for the reviewer, who compares them with the
//! mockup side by side. Behind the `snapshots` feature; a host without a graphics adapter prints the reason and
//! passes, except on Windows, where WARP must be there.
//!
//! The pixel comparison against blessed images (D7: `egui_kittest`'s `snapshot` feature and `kittest.toml`) waits
//! for the D7 supply-chain ruling: these images are evidence, not yet a gate.

#![cfg(feature = "snapshots")]

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

fn device() -> Option<ew_render2d::gpu::HeadlessDevice> {
    let dev = offscreen::device();
    if dev.is_none() {
        // On Windows an adapter must be found (DX12 WARP at least).
        #[cfg(windows)]
        panic!("no adapter: on Windows DX12 WARP at least must be found");
        #[cfg(not(windows))]
        println!("skipped: no adapter");
    }
    dev
}

fn main_window(theme: &str, density: &str) -> (Harness<'static, App>, offscreen::Recorder) {
    let tmp = Box::leak(Box::new(Tmp::new()));
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
    (h, recorder)
}

#[test]
fn the_main_window_in_every_theme_and_density() {
    let Some(dev) = device() else { return };
    for theme in ["light", "dark", "hc-dark", "hc-light"] {
        for density in ["comfortable", "compact"] {
            let (h, recorder) = main_window(theme, density);
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            assert_eq!((image.width, image.height), (1280, 800));
            offscreen::write_png(&image, &out_dir().join(format!("main-{theme}-{density}.png")));
        }
    }
}

#[test]
fn the_component_gallery_in_every_theme_at_ppp_1_and_2() {
    let Some(dev) = device() else { return };
    for theme in ["light", "dark", "hc-dark", "hc-light"] {
        for ppp in [1.0_f32, 2.0] {
            let recorder = offscreen::Recorder::default();
            let mut h = Harness::builder()
                .with_size(Vec2::new(640.0, 560.0))
                .with_pixels_per_point(ppp)
                .with_context_setup(setup)
                .renderer(recorder.clone())
                .build_ui_state(move |ui, _: &mut ()| gallery::show(ui, theme), ());
            h.run_ok();
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            offscreen::write_png(&image, &out_dir().join(format!("gallery-{theme}-ppp{ppp}.png")));
        }
    }
}
