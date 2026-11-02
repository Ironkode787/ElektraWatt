// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.07 part 3 in the window (D10, ruling 4 of ID-1180): the print pre-dialog's texts at the floor and at 1920,
//! each inside the dialog; the progress line while the set builds and Cancel ending it; the empty state with both
//! print buttons disabled; Print… on a platform without printing says so in one toast (ruling 3's default). Every
//! settle runs the accessible-name walker.

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::ew_jobs::Gate;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_ui::visual::{Driver, FrameRequest, Geometry, parse_input};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, harness_with, settle};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// Frames until the print set is built (bounded); its thread wakes the window, a frame takes its end.
fn run_until_built(h: &mut Harness<'_, App>) {
    for _ in 0..3000 {
        h.run_ok();
        if h.state().shell().print_set().is_some() {
            settle(h);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the print set was not built");
}

/// The dialog's area.
fn dialog(h: &Harness<'_, App>) -> egui::Rect {
    h.ctx
        .memory(|m| m.area_rect(egui::Id::unique("ew-dialog")))
        .expect("the print dialog")
}

#[test]
fn the_print_dialog_shows_every_text_inside_it_at_both_sizes() {
    for size in [egui::Vec2::new(1280.0, 800.0), egui::Vec2::new(1920.0, 1080.0)] {
        let tmp = Tmp::new();
        let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
        h.set_size(size);
        settle(&mut h);
        dispatch(
            &mut h,
            "file.print",
            json!({"range": "sections", "sections": "1", "paper": "A5", "fit": true}),
        );
        run_until_built(&mut h);
        let area = dialog(&h);
        // The first section's row, "Section 1: <its title>".
        let section = h.state().shell().print_dialog().unwrap().sections[0].label();
        assert!(section.starts_with("Section 1: "), "{section}");
        assert!(
            area.bottom() <= size.y && area.top() >= 0.0,
            "{area:?} in {size:?}"
        );
        for text in [
            "Print",
            "Range",
            "All",
            "Current page",
            "Sections",
            "Pages",
            section.as_str(),
            "Paper",
            "Scale to fit",
            "Monochrome",
            "1 of 5",
            "Previous",
            "Next",
            "Print…",
            "Open in PDF viewer",
            "Cancel",
        ] {
            let node = h
                .get_all_by_label(text)
                .find(|n| area.contains_rect(n.rect()))
                .unwrap_or_else(|| panic!("{text} inside the dialog at {size:?}"));
            assert!(node.rect().width() > 0.0, "{text}");
        }
        h.get_by_label("Next").click();
        settle(&mut h);
        assert!(h.query_by_label("2 of 5").is_some(), "Next moves the preview");
        h.state_mut().shell_mut().shutdown();
    }
}

#[test]
fn print_on_a_platform_without_printing_says_so_in_one_toast() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(&mut h, "file.print", json!({"range": "current"}));
    run_until_built(&mut h);
    h.get_by_label("Print…").click();
    settle(&mut h);
    settle(&mut h);
    assert!(h.state().shell().print_dialog().is_none(), "the dialog closes");
    assert!(
        h.query_by_label(
            "printing is not available in this window; export a PDF and print it from your PDF viewer"
        )
        .is_some(),
        "the toast"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_progress_line_stands_in_the_previews_place_and_cancel_ends_the_build() {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let gate = Gate::new();
    gate.hold("page");
    shell.set_job_gate_for_tests(Some(gate.clone()));
    let mut h = harness_with(shell, Scripted::default(), Some(tmp.golden("g.ew")));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(
        &mut h,
        "file.print",
        json!({"range": "sections", "sections": "1"}),
    );
    gate.wait_reached("page");
    settle(&mut h);
    assert!(h.query_by_label("Building the print set… 0 %").is_some());
    h.get_by_label("Cancel").click();
    settle(&mut h);
    gate.release();
    assert!(h.state().shell().print_dialog().is_none());
    assert!(
        h.state().shell().print_progress().is_none(),
        "the build is cancelled"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn no_page_chosen_shows_the_empty_state_and_disables_both_print_buttons() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(&mut h, "file.print", json!({"range": "sections"}));
    settle(&mut h);
    for text in ["No pages to print", "Tick a section or type the pages to print"] {
        assert!(h.get_all_by_label(text).next().is_some(), "{text}");
    }
    for name in ["Print…", "Open in PDF viewer"] {
        let button = h.get_by_label(name);
        assert!(button.accesskit_node().is_disabled(), "{name}");
    }
    h.state_mut().shell_mut().shutdown();
}

/// PW-4 (offscreen, the visual driver): the pre-dialog built, empty and building at the 1280 × 800 floor and at
/// 1920 × 1080, painted to `target/ew-ui-snapshots/print/` for the window check (not gated: no expected image). The
/// building frame holds the job at a test gate, which a script cannot (a visual run waits for the build).
#[test]
fn pw4_the_visual_driver_paints_the_three_states_at_both_sizes() {
    let out = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/ew-ui-snapshots/print"
    ));
    for (size, geometry) in [
        ("floor", Geometry::DEFAULT),
        ("1920", Geometry::parse(Some("1920x1080"), Some("1")).unwrap()),
    ] {
        let tmp = Tmp::new();
        let golden = tmp.golden("g.ew");
        let mut d = Driver::with_geometry(tmp.config(), Some(out.clone()), tmp.0.clone(), geometry).unwrap();
        // A dispatch, then a short wait, so the next frame line paints the changed model.
        let wait = parse_input(json!({"input": "wait", "ms": 100}).as_object().unwrap()).unwrap();
        let act = |d: &mut Driver, action: &str, params: Value| {
            assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done, "{action}");
            d.input(&wait);
        };
        let frame = |d: &mut Driver, state: &str| {
            let request = FrameRequest {
                file: format!("print-{state}-{size}.png"),
                settle: true,
                zoom: None,
                paint: true,
            };
            assert!(d.frame_line(&request).unwrap().is_some(), "{state} at {size}");
        };
        act(
            &mut d,
            "file.open",
            json!({"dir": golden.to_string_lossy(), "read_only": true}),
        );
        act(&mut d, "page.goto", json!({"folio": "101"}));
        act(
            &mut d,
            "file.print",
            json!({"range": "sections", "sections": "1", "paper": "A5", "fit": true}),
        );
        frame(&mut d, "built");
        assert!(d.shell_mut().print_set().is_some(), "built at {size}");
        act(&mut d, "print.set", json!({"sections": ""}));
        frame(&mut d, "empty");
        let gate = Gate::new();
        gate.hold("page");
        d.shell_mut().set_job_gate_for_tests(Some(gate.clone()));
        act(&mut d, "print.set", json!({"sections": "1"}));
        gate.wait_reached("page");
        frame(&mut d, "building");
        assert!(d.shell_mut().print_progress().is_some(), "building at {size}");
        act(&mut d, "ui.dismiss", json!({}));
        gate.release();
        d.shell_mut().shutdown();
    }
}
