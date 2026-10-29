// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lane X32b of cycle 32: the pane-side defects of the window check, in the window without rendering (doc 16a v1.2
//! §5.4, §5.8, §6.6, §6.8, §6.21). Every navigator row keeps to its pane with its title ellipsised before right-aligned
//! badges; the expanders are chevron buttons; the Properties value cell takes the rest of its row; a navigator's tool
//! line is one row; the renderer field says why it is amber.

mod support;

use egui::Rect;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigators::NavKind;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// A copy of Golden A in the window at 1280 × 800, on folio 101.
fn on_golden(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(
        shell.dispatch("file.open", &json!({"dir": dir.to_string_lossy()})),
        Outcome::Done
    );
    let mut h = harness_with(shell, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    settle(&mut h);
    h
}

/// Whether `inner` lies inside `outer` horizontally (half a point of rounding).
fn inside_x(outer: Rect, inner: Rect) -> bool {
    inner.left() >= outer.left() - 0.5 && inner.right() <= outer.right() + 0.5
}

/// Item 1: on Golden A at 1280 × 800 every row and every chip of the Pages tree lies inside the pane; the long row 100
/// is cut (its title is wider than its button) and its chip still shows; the expanders are named chevron buttons, not
/// "-" and "+".
#[test]
fn x32b_pages_rows_keep_to_the_pane_and_the_long_row_is_ellipsised() {
    let tmp = Tmp::new();
    let h = on_golden(&tmp);
    let tree = h.get_by_label("Page navigator");
    let pane = tree.rect();
    let rows: Vec<(String, Rect)> = tree
        .get_all_by_role(egui::Role::TreeItem)
        .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.rect()))
        .collect();
    assert!(rows.len() >= 5, "rows on show: {rows:?}");
    for (name, r) in &rows {
        assert!(inside_x(pane, *r), "row {name} {r:?} inside the pane {pane:?}");
    }
    let chips: Vec<(String, Rect)> = tree
        .get_all_by_role(egui::Role::Label)
        .map(|n| {
            let n2 = n.accesskit_node();
            (n2.label().or_else(|| n2.value()).unwrap_or_default(), n.rect())
        })
        .collect();
    assert!(
        chips.iter().any(|(c, _)| c == "QET grid"),
        "the QET grid chips: {chips:?}"
    );
    for (name, r) in &chips {
        assert!(inside_x(pane, *r), "chip {name} {r:?} inside the pane {pane:?}");
    }
    // Row 100: its whole title is wider than the button it got, so egui cut it with an ellipsis.
    let label = h
        .state()
        .shell()
        .shown_rows(NavKind::Pages)
        .into_iter()
        .find(|r| r.label.starts_with("100 "))
        .expect("row 100")
        .label;
    let (_, row) = rows
        .iter()
        .find(|(n, _)| n.starts_with(&label))
        .expect("row 100 on show");
    let font = egui::TextStyle::Body.resolve(&h.ctx.global_style());
    let full = h.ctx.fonts_mut(|f| {
        f.layout_no_wrap(label.clone(), font, egui::Color32::default())
            .size()
            .x
    });
    assert!(
        row.width() < full,
        "row 100 is cut: {} < {full} ({label})",
        row.width()
    );
    // Item 5: the expanders are buttons named by their action with the expanded state.
    let expanders: Vec<_> = tree
        .get_all_by_label("Expand or collapse")
        .map(|n| n.accesskit_node().data().is_expanded())
        .collect();
    assert!(
        expanders.len() >= 2 && expanders.iter().all(|e| *e == Some(true)),
        "the open sections' chevrons: {expanders:?}"
    );
    assert!(tree.query_by_label("-").is_none() && tree.query_by_label("+").is_none());
}

/// Item 2: at 1280 × 800 on folio 101 the value cell of Title spans to within `space.4` of the Properties pane's
/// right edge, and the title is shown whole when it fits the cell.
#[test]
fn x32b_the_property_value_cell_takes_the_rest_of_the_row() {
    let tmp = Tmp::new();
    let h = on_golden(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    let pane = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Properties"))
        .expect("the Properties pane")
        .rect();
    let row = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Title"))
        .expect("the Title row");
    let cell = row
        .get_all_by_label("Power Supply 1 - Main Current")
        .next()
        .expect("the value cell named by the whole value")
        .rect();
    assert!(
        cell.right() >= pane.right() - t.space_4 - 0.5 && cell.right() <= pane.right() + 0.5,
        "the cell {cell:?} ends at the row's padding in {pane:?}"
    );
    assert!(
        cell.left() <= pane.left() + t.space_4 + t.size_property_label + t.space_3 + 0.5,
        "the cell {cell:?} starts after the label column"
    );
}

/// Item 3 and 6: the Devices pane in table mode at the default left column has its tool line on one row inside the
/// pane's `space.4` gutters: the Table toggle, the filter field and the Columns menu share one centre line.
#[test]
fn x32b_the_table_tool_line_is_one_row() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "navigator.show", json!({"kind": "devices"}));
    dispatch(&mut h, "navigator.table", json!({"kind": "devices"}));
    settle(&mut h);
    let t = ew_ui::theme::current(&h.ctx);
    let pane = h.get_by_label("Devices table").rect();
    let toggle = h
        .get_all_by_label("Table")
        .map(|n| n.rect())
        .find(|r| inside_x(pane, *r))
        .expect("the Table toggle");
    let columns = h.get_by_label("Columns").rect();
    let filter = h
        .get_all_by_role(egui::Role::TextInput)
        .map(|n| n.rect())
        .find(|r| r.left() > toggle.right() && r.right() < columns.left())
        .expect("the filter field between them");
    for (what, r) in [("toggle", toggle), ("filter", filter), ("columns", columns)] {
        assert!(
            (r.center().y - toggle.center().y).abs() < 1.0,
            "{what} {r:?} on the toggle's row {toggle:?}"
        );
        assert!(
            r.left() >= pane.left() + t.space_4 - 0.5 && r.right() <= pane.right() - t.space_4 + 0.5,
            "{what} {r:?} inside the gutters of {pane:?}"
        );
    }
    assert!(
        toggle.width() <= t.size_control_inline + 0.5,
        "the toggle is an icon button"
    );
}

/// Item 8: the renderer field names what it means in its tooltip (amber is doc 16a §5.8's CPU fallback warning).
#[test]
fn x32b_the_renderer_field_says_why_in_its_tooltip() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    h.get_by_label("CPU").hover();
    settle(&mut h);
    settle(&mut h);
    assert!(
        h.query_by_label("The canvas draws on the CPU").is_some(),
        "the renderer's tooltip"
    );
}
