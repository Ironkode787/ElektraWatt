// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The switch rule in the window, without rendering (WP-S2.02 part 2, B5, B9, FT-349, FT-356): kittest has no wgpu
//! render state, so the canvas is the CPU canvas; a test stands in for a device's adapter and initialisation, and
//! for a lost device, through `App::gpu_host_mut`.

mod support;

use egui_kittest::Harness;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::layout::PaneKind;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::gpu::{AdapterSummary, DeviceKind, GpuApi, GpuError};
use serde_json::json;
use support::{Scripted, Tmp, harness, settle, wait_for_page};

/// A harness with a copy of Golden A open on folio 103, its canvas drawn.
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

fn vulkan() -> AdapterSummary {
    AdapterSummary {
        kind: DeviceKind::Discrete,
        api: GpuApi::Vulkan,
        name: "test GPU".into(),
    }
}

fn backend(h: &Harness<'_, App>) -> String {
    h.state().shell().canvas_backend().label()
}

fn banner(h: &Harness<'_, App>) -> Option<String> {
    h.state().shell().banner().map(str::to_owned)
}

#[test]
fn b9_kittest_draws_on_the_cpu_canvas() {
    let tmp = Tmp::new();
    let h = on_103(&tmp);
    assert_eq!(backend(&h), "CPU");
    assert!(h.state().page_view_ready());
    assert_eq!(h.state().canvas_counters().gpu_frames, 0);
    assert!(h.state().shell().status_fields(false).contains(&"CPU".to_owned()));
}

#[test]
fn b5_an_injected_initialisation_failure_is_the_cpu_canvas_with_one_banner() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    h.state_mut()
        .gpu_host_mut()
        .simulate(vulkan(), Err(GpuError::Init("pipeline creation failed".into())));
    settle(&mut h);
    assert_eq!(backend(&h), "CPU");
    let text = banner(&h).expect("a banner");
    assert!(
        text.contains("could not start") && text.contains("pipeline creation failed"),
        "{text}"
    );
    // Told once: dismissed, it does not come back.
    h.state_mut().shell_mut().dismiss_banner();
    for _ in 0..3 {
        settle(&mut h);
    }
    assert_eq!(banner(&h), None);
    assert_eq!(backend(&h), "CPU");
    assert!(h.state().page_view_ready(), "the CPU canvas draws the page");
}

#[test]
fn b5_an_injected_device_loss_moves_the_canvas_to_the_cpu_for_the_session() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    h.state_mut().gpu_host_mut().simulate(vulkan(), Ok(()));
    settle(&mut h);
    assert_eq!(backend(&h), "GPU Vulkan");
    assert_eq!(banner(&h), None);
    // The setting is effective at once, and back.
    assert_eq!(
        h.state_mut().shell_mut().dispatch("view.cpu_canvas", &json!({})),
        Outcome::Done
    );
    settle(&mut h);
    assert_eq!(backend(&h), "CPU");
    assert_eq!(
        h.state_mut().shell_mut().dispatch("view.cpu_canvas", &json!({})),
        Outcome::Done
    );
    settle(&mut h);
    assert_eq!(backend(&h), "GPU Vulkan");
    // The device is lost.
    h.state_mut()
        .gpu_host_mut()
        .inject_fault(GpuError::DeviceLost("reset".into()));
    settle(&mut h);
    assert_eq!(backend(&h), "CPU");
    let text = banner(&h).expect("a banner");
    assert!(
        text.contains("rest of this session") && text.contains("reset"),
        "{text}"
    );
    assert!(h.state().shell().status_fields(false).contains(&"CPU".to_owned()));
    h.state_mut().shell_mut().dismiss_banner();
    for _ in 0..3 {
        settle(&mut h);
    }
    assert_eq!(banner(&h), None, "told once");
    assert_eq!(backend(&h), "CPU", "for the rest of the session");
}

#[test]
fn b5_warp_is_labelled_warp_and_other_software_adapters_draw_on_the_cpu() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let warp = AdapterSummary {
        kind: DeviceKind::Cpu,
        api: GpuApi::Dx12,
        name: "Microsoft Basic Render Driver".into(),
    };
    h.state_mut().gpu_host_mut().simulate(warp, Ok(()));
    settle(&mut h);
    assert_eq!(backend(&h), "WARP");
    let tmp2 = Tmp::new();
    let mut h2 = on_103(&tmp2);
    let lavapipe = AdapterSummary {
        kind: DeviceKind::Cpu,
        api: GpuApi::Vulkan,
        name: "llvmpipe".into(),
    };
    h2.state_mut().gpu_host_mut().simulate(lavapipe, Ok(()));
    settle(&mut h2);
    assert_eq!(backend(&h2), "CPU");
    assert_eq!(banner(&h2), None, "a software adapter is no failure");
}

#[test]
fn ft_349_at_ui_scale_150_per_cent_the_page_keeps_its_paper_size() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("view.ui_scale", &json!({"scale": 1.5})),
        Outcome::Done
    );
    settle(&mut h);
    settle(&mut h);
    assert!((h.ctx.pixels_per_point() - 1.5).abs() < 1e-6);
    assert_eq!(
        h.state_mut().shell_mut().dispatch("view.zoom_actual", &json!({})),
        Outcome::Done
    );
    settle(&mut h);
    let c = h.state().shell().canvas().unwrap();
    assert_eq!(c.ui_scale(), 1.5);
    assert!((c.ppp() - 1.5).abs() < 1e-6);
    // 100 % is 96 dpi of paper on a monitor of scale 1, whatever the UI scale: the page's width in physical
    // pixels is its width in inches times 96.
    assert!((c.display_dpi() - 96.0).abs() < 1e-9, "{}", c.display_dpi());
    let page_px = c.sheet().0 as f64 / 25_400_000.0 * c.display_dpi();
    let page_pt = c.sheet().0 as f64 * c.points_per_nm();
    assert!((page_pt * c.ppp() - page_px).abs() < 1e-6);
    // The zoom field is unchanged: paper zoom.
    assert!(
        h.state()
            .shell()
            .status_fields(false)
            .contains(&"100 %".to_owned())
    );
}

#[test]
fn ft_356_a_page_pane_behind_another_tab_keeps_its_tiles_within_a_share() {
    let tmp = Tmp::new();
    let mut h = on_103(&tmp);
    let shown = h.state().canvas_cached_tiles();
    assert!(shown > 0);
    // The page list joins the page view's tab group and is in front: the canvas is hidden.
    h.state_mut()
        .dock_tab_together(PaneKind::Pages, PaneKind::PageView);
    h.step();
    let frames = h.state().canvas_stats().len();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(
        h.state().canvas_stats().len(),
        frames,
        "the canvas is not on screen"
    );
    let hidden = h.state().canvas_cached_tiles();
    assert!(hidden > 0, "the hidden pane keeps tiles (FT-356)");
    assert!(hidden <= shown);
    // Closing the project still lets go of every tile.
    assert_eq!(
        h.state_mut().shell_mut().dispatch("file.close", &json!({})),
        Outcome::Done
    );
    settle(&mut h);
    assert_eq!(h.state().canvas_cached_tiles(), 0);
}
