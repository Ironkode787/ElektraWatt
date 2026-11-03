// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 39 (window check W39c2; doc 16a §2, ID-908: the UI is not fitted to the floor): the Choose part dialog, the
//! Device-from-part picker and the command palette grow with the window. Each is measured at the floor (1280 by 800)
//! and at the typical size (1920 by 1080): wider at the typical size, a share of the window there, and never narrower
//! than at the floor.

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, size: egui::Vec2) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    h.set_size(size);
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// The width of the text field with the placeholder `hint`.
fn field_width(h: &Harness<'_, App>, hint: &str) -> f32 {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some(hint))
        .unwrap_or_else(|| panic!("the field {hint}"))
        .rect()
        .width()
}

/// Measures `measure` after `open` at the floor and at the typical size.
fn at_both_sizes(
    open: impl Fn(&mut Harness<'_, App>),
    measure: impl Fn(&Harness<'_, App>) -> f32,
) -> (f32, f32) {
    let mut out = Vec::new();
    for size in [egui::Vec2::new(1280.0, 800.0), egui::Vec2::new(1920.0, 1080.0)] {
        let tmp = Tmp::new();
        let mut h = on_golden(&tmp, size);
        open(&mut h);
        settle(&mut h);
        settle(&mut h);
        settle(&mut h);
        out.push(measure(&h));
        h.state_mut().shell_mut().shutdown();
    }
    (out[0], out[1])
}

fn assert_grows(what: &str, floor: f32, typical: f32) {
    assert!(
        typical > floor,
        "{what}: {typical} at 1920 against {floor} at 1280"
    );
    assert!(
        typical >= 0.3 * 1920.0,
        "{what}: {typical} is no share of the 1920-wide window"
    );
}

#[test]
fn the_choose_part_dialog_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| {
            dispatch(h, "select.set", json!({"refs": ["tag:F55"]}));
            dispatch(h, "part.choose", json!({"use": 0}));
        },
        |h| {
            h.get_by_role_and_label(egui::Role::ListBox, "Parts")
                .rect()
                .width()
        },
    );
    assert_grows("the part list", floor, typical);
}

#[test]
fn the_part_picker_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| {
            dispatch(h, "page.goto", json!({"folio": "101"}));
            dispatch(h, "tool.place_part", json!({}));
        },
        |h| field_width(h, "Filter by key, part number or function"),
    );
    assert_grows("the picker's filter", floor, typical);
}

/// FT-1143 (ID-1225) at the floor: a part row's function text that does not fit its column is cut with an ellipsis
/// inside the row, never cut hard at the popup's edge, and the row's tooltip ends with the whole text.
#[test]
fn ft1143_a_part_rows_function_is_cut_with_an_ellipsis_and_whole_in_its_tooltip() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(&mut h, "tool.place_part", json!({}));
    // The rows with the longest function texts ("1 of 1 NO position switch 13-14 main").
    dispatch(&mut h, "picker.filter", json!({"text": "switch"}));
    settle(&mut h);
    settle(&mut h);
    settle(&mut h);
    // Each painted text, its rectangle and whether it is cut with an ellipsis.
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect, bool)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                t.visual_bounding_rect(),
                t.galley.elided,
            )),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut texts);
    }
    // The rows of part functions: their tooltip (the description) ends with "<n> of <m> <function>".
    let rows: Vec<(egui::Rect, String)> = h
        .get_all_by_role(egui::Role::Button)
        .filter_map(|n| {
            let tip = n.accesskit_node().description()?.to_owned();
            let meta = tip.lines().last()?.to_owned();
            let mut words = meta.split(' ');
            let numbered = words.next()?.parse::<usize>().is_ok() && words.next()? == "of";
            numbered.then(|| (n.rect(), meta))
        })
        .collect();
    assert!(!rows.is_empty(), "part rows");
    let mut cut = 0;
    for (row, meta) in &rows {
        // The row's function text as painted.
        for (text, at, elided) in texts
            .iter()
            .filter(|(text, at, _)| row.contains(at.left_center()) && text == meta)
        {
            assert!(
                at.right() <= row.right() + 0.5,
                "{text:?} at {at:?} inside the row {row:?}"
            );
            if *elided {
                cut += 1;
            }
        }
    }
    assert!(
        cut > 0,
        "a function text cut with an ellipsis at the floor: {rows:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_command_palette_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| dispatch(h, "tools.palette", json!({})),
        |h| field_width(h, "Type a command…"),
    );
    assert_grows("the palette's field", floor, typical);
}

/// C40: wider at 1920 than at the floor, never narrower than `base` (the floor width) and within the window.
fn assert_grows_from(what: &str, base: f32, floor: f32, typical: f32) {
    assert!(
        typical > floor,
        "{what}: {typical} at 1920 against {floor} at 1280"
    );
    assert!(
        floor >= base - 0.5 && typical >= base - 0.5,
        "{what}: {floor} and {typical} keep the floor width {base}"
    );
    assert!(typical <= 1920.0, "{what}: {typical} within the window");
}

fn tokens() -> ew_ui::theme::Tokens {
    ew_ui::theme::tokens(ew_ui::theme::Theme::Light, ew_ui::theme::Density::Comfortable)
}

/// C40 (FT-798): the command search grows with the window from `size.search.width`, staying at the tab row's right
/// end.
#[test]
fn the_command_search_grows_with_the_window() {
    let t = tokens();
    let measure = |h: &Harness<'_, App>| {
        let r = h.get_by_role(egui::Role::SearchInput).rect();
        let window = h.ctx.content_rect().width();
        assert!(
            r.right() <= window && window - r.right() <= t.space_5 + 0.5,
            "the search stays at the right end: {r:?}"
        );
        r.width()
    };
    let (floor, typical) = at_both_sizes(|_| {}, measure);
    assert!((floor - t.size_search_width).abs() < 0.5, "{floor}");
    assert!(typical <= t.size_search_max_width + 0.5, "{typical}");
    assert_grows_from("the command search", t.size_search_width, floor, typical);
}

/// The parts import's mapping card's width (its area).
fn mapping_card(h: &Harness<'_, App>) -> f32 {
    h.ctx
        .memory(|m| m.area_rect(egui::Id::unique("parts-map-card")))
        .expect("the mapping card")
        .width()
}

/// Opens the parts import's mapping step on the table at `path`.
fn open_mapping(path: &std::path::Path) -> impl Fn(&mut Harness<'_, App>) + '_ {
    move |h| {
        let path = path.to_string_lossy().into_owned();
        dispatch(h, "parts.import", json!({ "path": path }));
        assert!(h.state().shell().parts_map().is_some(), "the mapping step");
    }
}

/// C40 (FT-816 a): the parts import's column mapping card is a form that grows with the window when its samples need
/// the room (a table with a long description): wider at 1920 than at the floor. Golden A's own table fits the form
/// width at both sizes, so it is not stretched.
#[test]
fn the_parts_mapping_card_grows_with_the_window() {
    let csv_dir = Tmp::new();
    let csv = csv_dir.0.join("long.csv");
    let long = "Miniature circuit breaker 1P+N characteristic C 10 A 6 kA with auxiliary contact and a long \
                catalogue text that no form at the floor can show whole";
    let head = "key,manufacturer,part_number,kind,type_designation,description,unit,test_data,tech_data";
    std::fs::write(&csv, format!("{head}\nA1,Acme,A1,,,{long},piece,,\n")).unwrap();
    let (floor, typical) = at_both_sizes(open_mapping(&csv), mapping_card);
    assert_grows_from("the mapping card", tokens().size_dialog_form, floor, typical);
    let golden =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv");
    let (floor, typical) = at_both_sizes(open_mapping(&golden), mapping_card);
    assert!(
        (floor - typical).abs() < 0.5 && floor < tokens().size_dialog_form + 0.5,
        "Golden A's table keeps the form width: {floor} and {typical}"
    );
}

/// WP-S3.07 part 3 (ruling 4 of ID-1180): the print pre-dialog is `size.dialog.preview` (1040) wide at the floor and
/// 0.65 of the window at 1920 (1248).
#[test]
fn the_print_dialog_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| dispatch(h, "file.print", json!({"range": "current"})),
        |h| {
            h.ctx
                .memory(|m| m.area_rect(egui::Id::unique("ew-dialog")))
                .expect("the print dialog")
                .width()
        },
    );
    assert!(
        (floor - tokens().size_dialog_preview).abs() < 1.0,
        "{floor} at the floor"
    );
    assert!((typical - 0.65 * 1920.0).abs() < 1.0, "{typical} at 1920");
    assert_grows_from("the print dialog", tokens().size_dialog_preview, floor, typical);
}

/// A page title long enough that its change line needs more than `size.dialog.small`.
const LONG_TITLE: &str =
    "Power Supply 1 - Main Current with a deliberately long title that no small dialog can hold on one line";

/// C40 (FT-816 a): the history's Show changes popup grows with the window when its lines need the room (a short
/// list keeps the small width; past the grown width a line elides).
#[test]
fn the_show_changes_popup_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| {
            dispatch(
                h,
                "command.run",
                json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [{"ref": "folio:101"}],
                    "path": "title", "value": LONG_TITLE}}),
            );
            dispatch(h, "history.show", json!({}));
            dispatch(h, "history.changes", json!({"entry": 1}));
        },
        |h| {
            h.ctx
                .memory(|m| m.area_rect(egui::Id::unique("history-changes")))
                .expect("the Show changes popup")
                .width()
        },
    );
    // The window's area is its content: the dialog width less the `space.5` padding on either side.
    let t = tokens();
    assert_grows_from(
        "Show changes",
        t.size_dialog_small - 2.0 * t.space_5,
        floor,
        typical,
    );
}

/// C40 (FT-798): at 1920 × 1080 no row of Golden A's Pages tree on show is cut (each button is as wide as its whole
/// title), and the left column is wider than its share of the floor.
#[test]
fn no_pages_row_is_cut_at_1920() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, egui::Vec2::new(1920.0, 1080.0));
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    settle(&mut h);
    settle(&mut h);
    let labels: Vec<(String, bool)> = h
        .state()
        .shell()
        .shown_rows(ew_ui::ew_viewmodel::navigators::NavKind::Pages)
        .into_iter()
        .map(|r| (r.label, r.current))
        .collect();
    let tree = h.get_by_label("Page navigator");
    assert!(
        tree.rect().width() >= tokens().size_panel_left + 100.0,
        "the pane grew: {:?}",
        tree.rect()
    );
    let rows: Vec<(String, f32)> = tree
        .get_all_by_role(egui::Role::TreeItem)
        .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.rect().width()))
        .collect();
    let font = egui::TextStyle::Body.resolve(&h.ctx.global_style());
    let strong = ew_ui::theme::font(&tokens().type_body_strong);
    let mut checked = 0;
    for (label, current) in &labels {
        let Some((_, width)) = rows.iter().find(|(n, _)| n.starts_with(label.as_str())) else {
            continue;
        };
        // The current page is in `type.body.strong`.
        let f = if *current { strong.clone() } else { font.clone() };
        let full = h.ctx.fonts_mut(|x| {
            x.layout_no_wrap(label.clone(), f, egui::Color32::default())
                .size()
                .x
        });
        assert!(*width >= full - 0.5, "row {label} is cut: {width} < {full}");
        checked += 1;
    }
    assert!(
        labels.iter().any(|(l, _)| l.starts_with("100 ")) && checked >= 5,
        "rows checked: {checked} of {labels:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// C40 (FT-816 b): the Pages table's Title is re-filled when its pane grows: after a resize from the floor to 1920
/// it is wider.
#[test]
fn the_pages_table_title_refills_on_a_resize() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "navigator.table", json!({"kind": "pages", "on": true}));
    settle(&mut h);
    settle(&mut h);
    let title = |h: &Harness<'_, App>| h.get_by_label_contains("Sort by Title").rect().width();
    let floor = title(&h);
    h.set_size(egui::Vec2::new(1920.0, 1080.0));
    for _ in 0..4 {
        settle(&mut h);
    }
    let typical = title(&h);
    assert!(
        typical > floor + 1.0,
        "Title {typical} at 1920 against {floor} at 1280"
    );
    h.state_mut().shell_mut().shutdown();
}
