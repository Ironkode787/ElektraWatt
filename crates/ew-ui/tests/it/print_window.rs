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

/// Every shape of the last frame with its clip rectangle.
fn shapes(h: &Harness<'_, App>) -> Vec<(egui::Shape, egui::Rect)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(egui::Shape, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            s => out.push((s.clone(), clip)),
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, c.clip_rect, &mut out);
    }
    out
}

/// FT-1124 (ID-1225): a Pages text the parser refuses shows one sentence in words, once, under the field, in doc 16a
/// §6.1's Error state (the field's boundary in `color.error`, the error icon before the sentence); the preview shows
/// the empty state.
#[test]
fn ft1124_a_refused_pages_text_shows_once_under_the_field_in_the_error_state() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(
        &mut h,
        "file.print",
        json!({"range": "pages", "pages": "101,,102"}),
    );
    settle(&mut h);
    let sentence = "The pages could not be read. Type folio labels or ranges, separated by commas.";
    let lines: Vec<egui::Rect> = h.get_all_by_label(sentence).map(|n| n.rect()).collect();
    assert_eq!(lines.len(), 1, "the sentence once");
    let line = lines[0];
    assert!(
        h.get_all_by_label("No pages to print").next().is_some(),
        "the empty state"
    );
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Pages"))
        .expect("the Pages field")
        .rect();
    assert!(line.top() >= field.bottom(), "{line:?} under {field:?}");
    let t = ew_ui::theme::current(&h.ctx);
    let error = t.error.color32();
    let all = shapes(&h);
    assert!(
        all.iter().any(|(s, _)| matches!(s, egui::Shape::Rect(r)
            if r.stroke.color == error && r.rect == field)),
        "the field's error boundary"
    );
    assert!(
        all.iter().any(|(s, _)| !matches!(s, egui::Shape::Text(_))
            && s.visual_bounding_rect().right() <= line.left()
            && s.visual_bounding_rect().left() >= field.left() - 1.0
            && s.visual_bounding_rect().center().y >= line.top()
            && s.visual_bounding_rect().center().y <= line.bottom()
            && match s {
                egui::Shape::Path(p) => p.stroke.color == egui::epaint::ColorMode::Solid(error),
                egui::Shape::LineSegment { stroke, .. } => stroke.color == error,
                egui::Shape::Circle(c) => c.stroke.color == error || c.fill == error,
                _ => false,
            }),
        "the error icon before the sentence"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-1125 (ID-1225; doc 16a §6.12): the pre-dialog's first focus is its Primary, Print…, once the set is built.
#[test]
fn ft1125_the_first_focus_is_print() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(&mut h, "file.print", json!({"range": "current"}));
    run_until_built(&mut h);
    settle(&mut h);
    let focused: Vec<String> = h
        .get_all_by(|n| n.is_focused())
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect();
    assert_eq!(focused, ["Print…"]);
    h.state_mut().shell_mut().shutdown();
}

/// FT-1126 (ID-1225): with paper A3 and no fit the preview draws the paper as a sheet: its whole edge (no clip cuts
/// it) in `color.border.control`, which meets the non-text contrast on the dialog's surface (the theme's pairs).
#[test]
fn ft1126_the_preview_draws_the_papers_whole_edge() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), Some(tmp.golden("g.ew")));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(
        &mut h,
        "file.print",
        json!({"range": "current", "paper": "A3", "fit": false}),
    );
    run_until_built(&mut h);
    let t = ew_ui::theme::current(&h.ctx);
    let edge = egui::Stroke::new(t.border_width, t.border_control.color32());
    let sheets: Vec<(egui::Rect, egui::Rect)> = shapes(&h)
        .into_iter()
        .filter_map(|(s, clip)| match s {
            egui::Shape::Rect(r) if r.stroke == edge && r.stroke_kind == egui::StrokeKind::Outside => {
                Some((r.rect, clip))
            }
            _ => None,
        })
        .collect();
    assert_eq!(sheets.len(), 1, "one paper edge: {sheets:?}");
    let (sheet, clip) = sheets[0];
    assert!(
        clip.contains_rect(sheet.expand(t.border_width)),
        "the edge {sheet:?} whole in the clip {clip:?}"
    );
    assert!(dialog(&h).contains_rect(sheet.expand(t.border_width)));
    assert!(ew_ui::theme::contrast(t.border_control, t.surface_raised) >= 3.0);
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
        // FT-1126 (ID-1225): A3 with no fit, the paper's edge whole.
        act(&mut d, "print.set", json!({"paper": "A3", "fit": false}));
        frame(&mut d, "a3");
        // FT-1124 (ID-1225): a refused Pages text, once, under the field.
        act(
            &mut d,
            "print.set",
            json!({"range": "pages", "pages": "101,,102"}),
        );
        frame(&mut d, "pages-error");
        act(&mut d, "print.set", json!({"range": "sections"}));
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
