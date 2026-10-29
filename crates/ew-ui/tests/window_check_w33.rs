// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fixes of window check W33 (cycle 34, FT-558 to FT-567) that a node or a painted shape can show without the
//! pixel gate: the Columns button's inset, the grid's scroll bar inside the focus ring, the form dialogs' words and
//! check boxes, the label column, and the dock's borderless "All tabs" button.

mod support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The window on a writable copy of Golden A with the Terminal strips navigator in table mode, Properties hidden.
fn strips(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let shell = ShellModel::new(Mode::Gui, tmp.config());
    let mut h: Harness<'static, App> = harness_with(shell, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1000.0, 900.0));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "props.show", json!({"on": false}));
    done(shell, "navigator.show", json!({"kind": "strips"}));
    done(shell, "navigator.table", json!({"kind": "strips"}));
    done(shell, "ui.focus", json!({"area": "navigator", "kind": "strips"}));
    settle(&mut h);
    h
}

/// FT-567: the tool line keeps `space.4` on both sides: the Columns button ends `space.4` inside the grid's pane, as
/// the Table toggle starts `space.4` inside it.
#[test]
fn ft567_the_columns_button_keeps_space_4_from_the_pane_edge() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    let grid = h.get_by_label("Terminal strips table").rect();
    let columns = h.get_by_label("Columns").rect();
    let toggle = h.get_by_label("Table").rect();
    assert!(
        (toggle.left() - grid.left() - t.space_4).abs() < 0.5,
        "the toggle at {toggle:?} in {grid:?}"
    );
    assert!(
        (grid.right() - columns.right() - t.space_4).abs() < 0.5,
        "Columns at {columns:?} in {grid:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The main window on a writable copy of Golden A, folio 101 shown.
fn window(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let shell = ShellModel::new(Mode::Gui, tmp.config());
    let mut h: Harness<'static, App> = harness_with(shell, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    h
}

/// The value of field `name` of the open form dialog.
fn form_value(h: &Harness<'static, App>, name: &str) -> String {
    match h.state().shell().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f
            .fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.clone())
            .expect("the field"),
        other => panic!("no form dialog: {other:?}"),
    }
}

/// The shown value of the open dialog's one combo box.
fn combo_value(h: &Harness<'static, App>) -> String {
    let combo = h.get_by_role(egui::Role::ComboBox);
    let node = combo.accesskit_node();
    node.value()
        .map(|v| v.to_string())
        .or_else(|| node.label().map(|l| l.to_string()))
        .unwrap_or_default()
}

/// FT-558: a choice shows doc 16a §8's words ("Multi-line", "Location"), never the raw parameter value, and the action
/// still gets the raw value.
#[test]
fn ft558_choices_show_display_words() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    done(h.state_mut().shell_mut(), "page.new", json!({}));
    settle(&mut h);
    assert_eq!(combo_value(&h), "Multi-line", "the page type in words");
    assert_eq!(form_value(&h, "type"), "multi_line");
    h.get_by_role(egui::Role::ComboBox).click();
    settle(&mut h);
    assert!(
        h.query_by_label("Table of contents").is_some(),
        "the choices in words"
    );
    assert!(h.query_by_label("table_of_contents").is_none(), "no raw choice");
    h.get_by_label("Single-line").click();
    settle(&mut h);
    assert_eq!(
        form_value(&h, "type"),
        "single_line",
        "the action gets the raw value"
    );
    done(h.state_mut().shell_mut(), "ui.dismiss", Value::Null);
    done(h.state_mut().shell_mut(), "structure.new", json!({}));
    settle(&mut h);
    assert_eq!(combo_value(&h), "Location", "the structure kind in words");
    h.state_mut().shell_mut().shutdown();
}

/// FT-558: a yes-or-no field is a check box named by its label; checking it sets the field to `true`.
#[test]
fn ft558_booleans_are_check_boxes() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    done(h.state_mut().shell_mut(), "pages.renumber", json!({}));
    settle(&mut h);
    assert!(h.query_by_label("false").is_none(), "no raw boolean");
    assert_eq!(form_value(&h, "keep_gaps"), "false");
    h.get_by_role_and_label(egui::Role::CheckBox, "Keep gaps").click();
    settle(&mut h);
    assert_eq!(form_value(&h, "keep_gaps"), "true");
    h.state_mut().shell_mut().shutdown();
}

/// FT-566: the form dialogs share one label column: every field starts at the same x in every form dialog, and the
/// fields fill the body to the same right edge.
#[test]
fn ft566_one_label_column_and_full_width_fields() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    let mut edges = Vec::new();
    for (action, label) in [
        ("page.new", "Folio"),
        ("structure.new", "Designation"),
        ("external_ref.new", "Device"),
        ("pages.renumber", "Start"),
    ] {
        done(h.state_mut().shell_mut(), action, json!({}));
        for _ in 0..4 {
            h.run_ok();
        }
        let field = h.get_by_role_and_label(egui::Role::TextInput, label).rect();
        edges.push((action, field.left(), field.right()));
        done(h.state_mut().shell_mut(), "ui.dismiss", Value::Null);
        settle(&mut h);
    }
    let (_, left, right) = edges[0];
    for (action, l, r) in &edges {
        assert!(
            (l - left).abs() < 0.5 && (r - right).abs() < 0.5,
            "{action}: {l}..{r} against {left}..{right}"
        );
    }
    h.state_mut().shell_mut().shutdown();
}

/// FT-562: the dock's "All tabs" button is a borderless 24-px icon button (doc 16a §6.3) that opens the list of tabs.
#[test]
fn ft562_all_tabs_is_a_subtle_icon_button() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    for action in ["messages.show", "history.show"] {
        done(h.state_mut().shell_mut(), action, json!({}));
    }
    settle(&mut h);
    let t = ew_ui::theme::current(&h.ctx);
    let button = h.get_all_by_label("All tabs").next().expect("All tabs").rect();
    assert_eq!(
        button.size(),
        egui::Vec2::splat(t.size_control_inline),
        "{button:?}"
    );
    let before = h.get_all_by_label("Messages").count();
    h.get_all_by_label("All tabs").next().expect("All tabs").click();
    settle(&mut h);
    assert!(
        h.get_all_by_label("Messages").count() > before,
        "the list of tabs opened"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-561: a dock tab's count chip stands off the chrome in every theme: a fill other than the chrome's, or (high
/// contrast) a `color.border.default` outline; its text keeps the text contrast on that fill.
#[test]
fn ft561_the_count_chip_stands_off_the_chrome() {
    use ew_ui::theme::{Density, Theme, contrast, tokens};
    for theme in [
        Theme::Light,
        Theme::Dark,
        Theme::HighContrastDark,
        Theme::HighContrastLight,
    ] {
        let t = tokens(theme, Density::Comfortable);
        if theme.is_high_contrast() {
            assert!(t.border_raised_width > 0.0, "{theme:?}: outlined");
        } else {
            assert_ne!(
                t.badge_on_chrome, t.surface_chrome,
                "{theme:?}: a fill of its own"
            );
        }
        let floor = if theme.is_high_contrast() { 7.0 } else { 4.5 };
        assert!(
            contrast(t.text_secondary, t.badge_on_chrome) >= floor,
            "{theme:?}"
        );
    }
}

/// FT-581 (ruling: the exemption stands): a disabled control's label stays legible but clearly under an enabled one:
/// `color.text.disabled` is at least 2:1 and under 3:1 on the panel in light and dark, so the disabled Primary (drawn
/// as a Secondary) never reads as an enabled button.
#[test]
fn ft581_disabled_text_is_legible_and_distinct() {
    use ew_ui::theme::{Density, Theme, contrast, tokens};
    for theme in [Theme::Light, Theme::Dark] {
        let t = tokens(theme, Density::Comfortable);
        let ratio = contrast(t.text_disabled, t.surface_panel);
        assert!((2.0..3.0).contains(&ratio), "{theme:?}: {ratio}");
        assert!(
            contrast(t.text_primary, t.surface_panel) > 2.0 * ratio,
            "{theme:?}"
        );
    }
}
