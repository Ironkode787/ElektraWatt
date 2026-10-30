// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 3 in the window without rendering: the rails, the docks and the default workspace (DK1, DK8), the
//! named splitters (FT-338), the page rows' badges (DK2), the property grid's groups (DK3's window half), the status
//! bar (DK4's window half), the history panel from the keyboard (FT-519, DK11's keyboard half) and the left column's
//! collapse to its rail.

mod support;

use egui::{Key, Modifiers, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::layout::Layout;
use ew_ui::ew_viewmodel::shell::{Focus, Mode, Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

/// The main window on a copy of Golden A at 1280 by 800, folio 101 on show.
fn window(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let shell = ShellModel::new(Mode::Gui, tmp.config());
    let mut h = harness_with(shell, Scripted::default(), Some(dir));
    h.set_size(Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
    settle(h);
}

/// The frame inside the harness's 8-px margin (egui_kittest's `Frame::central_panel`).
fn frame(h: &Harness<'_, App>) -> Rect {
    h.ctx.content_rect().shrink(8.0)
}

/// The rail buttons (the left rail's, then the right rail's), by their left edge then their top.
fn rail_buttons(h: &Harness<'_, App>, left: bool) -> Vec<(String, Rect)> {
    let f = frame(h);
    let mut out: Vec<(String, Rect)> = h
        .get_all_by_role(egui::Role::Button)
        .filter(|n| {
            let r = n.rect();
            r.width() > 31.0
                && r.width() < 33.0
                && r.height() > 31.0
                && r.height() < 33.0
                && if left {
                    r.right() <= f.left() + 40.5
                } else {
                    r.left() >= f.right() - 40.5
                }
        })
        .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.rect()))
        .collect();
    out.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    out
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// DK8 (with G5 to G10 of §12.0): the rails, the rail buttons, the left column, the canvas share, the 50 : 50 split of
/// the column, the status bar and the property label column at 1280 by 800; the first five rail entries in the
/// mockup's order with History at the foot; the Project entry's segmented control.
#[test]
fn dk8_the_layout_matches_the_mockup() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    let f = frame(&h);
    // The rails: 40 wide; 32-px buttons centred in them.
    let left = rail_buttons(&h, true);
    let names: Vec<&str> = left.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names[..5],
        [
            "Project",
            "Terminal strips",
            "Cables",
            "Potentials",
            "Structure identifiers"
        ],
        "{names:?}"
    );
    assert_eq!(names.last(), Some(&"History"), "History at the foot");
    for (name, r) in &left {
        assert!(near(r.center().x, f.left() + 20.0, 1.0), "{name} {r:?}");
        assert!(
            near(r.width(), 32.0, 1.0) && near(r.height(), 32.0, 1.0),
            "{name} {r:?}"
        );
    }
    let right = rail_buttons(&h, false);
    let names: Vec<&str> = right.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Symbols", "Parts"]);
    for (name, r) in &right {
        assert!(near(r.center().x, f.right() - 20.0, 1.0), "{name} {r:?}");
    }
    // The status bar: 26 high at the frame's bottom, "Ready" at its left.
    let ready = h.get_by_role_and_label(egui::Role::Label, "Ready").rect();
    assert!(
        near(f.bottom() - ready.center().y, 13.0, 1.0),
        "{ready:?} in {f:?}"
    );
    assert!(near(ready.height(), 22.0, 0.5), "{ready:?}");
    assert!(ready.left() < f.left() + 1.0, "{ready:?}");
    // The left column and the canvas: 280 of the 1200 px between the rails at 1280 (233 ‰); here the harness's
    // margin leaves 1184, so the column is 233 ‰ of that, the page view the rest (G5, G8: 71.9 % of the window
    // within one percentage point).
    let (bar, _, pane) = h.state().command_surface_rects();
    let (bar, pane) = (bar.unwrap(), pane.unwrap());
    let dock = f.width() - 80.0;
    let column = pane.left() - (f.left() + 40.0);
    assert!(near(column, 280.0, 1.5), "the left column {column} of {dock}");
    // The harness's 16 px of margin are the canvas's at a real 1280-px window.
    let share = (pane.width() + 16.0) / (f.width() + 16.0);
    assert!(near(share, 0.719, 0.012), "the canvas share {share}");
    // G9: Pages above Properties, 50 : 50 within three percentage points.
    let props = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| {
            n.accesskit_node().label().as_deref() == Some("Properties") && n.rect().left() < pane.left()
        })
        .expect("the Properties header")
        .rect();
    let (top, bottom) = (bar.bottom(), ready.top() - 2.0);
    let split = (props.top() - top) / (bottom - top);
    assert!(near(split, 0.5, 0.03), "the column splits at {split}");
    // The Project entry's segmented control.
    assert!(h.query_by_label("Pages or devices").is_some());
    assert!(
        h.query_all_by_label("Devices")
            .any(|n| n.accesskit_node().role() == egui::Role::Tab)
    );
    // The default workspace is D10's.
    assert_eq!(h.state().shell().layout(), &Layout::schematic());
    h.state_mut().shell_mut().shutdown();
}

/// DK1, FT-338: every splitter handle is a named `Splitter` node ("Resize Pages and Properties"), the walker passes
/// with no exception while every navigator is open, and each rail button is one Tab stop.
#[test]
fn dk1_named_splitters_and_the_walker_without_a_skip() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    let names: Vec<String> = h
        .get_all_by_role(egui::Role::Splitter)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.contains(" and "))
        .collect();
    assert!(
        names.iter().any(|n| n == "Resize Pages and Properties"),
        "{names:?}"
    );
    assert!(
        names.iter().any(|n| n == "Resize Pages and Page view"),
        "{names:?}"
    );
    assert_eq!(names.len(), h.state().splitter_count(), "{names:?}");
    for k in [
        "devices",
        "connections",
        "strips",
        "cables",
        "plc",
        "potentials",
        "ips",
        "structure",
        "parts",
        "blocks",
    ] {
        dispatch(&mut h, "navigator.show", json!({ "kind": k }));
    }
    support::assert_named(&h);
    // A rail button takes the keyboard focus.
    let node = h.get_by_role_and_label(egui::Role::Button, "Cables");
    assert!(
        node.accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus),
        "the rail button is a Tab stop"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The rail collapses the left column: a click on the shown entry hides the column (the page view takes its width),
/// Ctrl+1 brings it back with Project.
#[test]
fn a_click_on_the_shown_entry_collapses_the_column() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    let before = h.state().command_surface_rects().2.unwrap();
    let f = frame(&h);
    h.get_all_by_role(egui::Role::Button)
        .find(|n| {
            n.accesskit_node().label().as_deref() == Some("Project") && n.rect().right() <= f.left() + 40.5
        })
        .expect("the rail's Project button")
        .click();
    settle(&mut h);
    assert!(h.state().shell().rail_collapsed());
    let after = h.state().command_surface_rects().2.unwrap();
    assert!(after.width() > before.width() + 200.0, "{before:?} → {after:?}");
    assert!(
        h.query_by_label("Pages or devices").is_none(),
        "the column is hidden"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
    settle(&mut h);
    assert!(!h.state().shell().rail_collapsed());
    assert!(h.query_by_label("Pages or devices").is_some());
    h.state_mut().shell_mut().shutdown();
}

/// DK2: the page rows' badges are named nodes after the row's text, never bracketed words.
#[test]
fn dk2_page_rows_carry_badge_nodes_not_bracketed_words() {
    let tmp = Tmp::new();
    let h = window(&tmp);
    let shell = h.state().shell();
    let qet = shell.page_rows().iter().filter(|r| r.qet_grid).count();
    assert!(qet > 0, "Golden A has QET-grid pages");
    for n in h.get_all_by_role(egui::Role::TreeItem) {
        let label = n.accesskit_node().label().unwrap_or_default();
        assert!(!label.contains('['), "{label}");
    }
    assert!(
        h.query_all_by_label("QET grid")
            .any(|n| n.accesskit_node().role() == egui::Role::Label),
        "the badge is a named node"
    );
    let rows = shell.shown_rows(ew_ui::ew_viewmodel::navigators::NavKind::Pages);
    assert!(rows.iter().all(|r| !r.label.contains('[')));
    assert_eq!(
        rows.iter().filter(|r| r.current).count(),
        1,
        "one current page row"
    );
    drop(h);
}

/// DK3 in the window: the Main group first, no edit buttons, one actions button per row with actions, and the
/// Advanced rows behind "Show advanced (n)".
#[test]
fn dk3_properties_main_first_and_advanced_on_demand() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-101F1"]}));
    assert!(h.query_all_by_label("edit").next().is_none(), "no edit buttons");
    let main = h.get_by_role_and_label(egui::Role::Button, "Main").rect();
    let identity = h.get_by_role_and_label(egui::Role::Button, "Identity").rect();
    assert!(main.top() < identity.top(), "Main first");
    let groups = |h: &Harness<'_, App>| h.get_all_by_role(egui::Role::Group).count();
    let shown = groups(&h);
    let link = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Show advanced ("))
        })
        .expect("the Show advanced link");
    link.click_accesskit();
    settle(&mut h);
    assert!(h.state().shell().properties_show_advanced());
    assert!(groups(&h) > shown, "the Advanced rows are on show");
    assert!(h.get_all_by_role(egui::Role::Button).any(|n| {
        n.accesskit_node()
            .label()
            .is_some_and(|l| l.starts_with("Hide advanced ("))
    }),);
    h.state_mut().shell_mut().shutdown();
}

/// DK4 in the window: "Ready" at the left, the zoom, the save state and the renderer in D12's order left to right; no
/// page or language field; the findings field opens Messages.
#[test]
fn dk4_the_status_bar_fields_in_order() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    let fields = ew_ui::ew_viewmodel::status::bar(h.state().shell(), false);
    let mut last = f32::MIN;
    for f in &fields {
        let text = f.text.clone();
        let r = h
            .query_all_by_label(&text)
            .filter(|n| n.rect().bottom() > frame(&h).bottom() - 26.0)
            .map(|n| n.rect())
            .next()
            .unwrap_or_else(|| panic!("the field {text}"));
        assert!(r.left() > last, "{text} after the field before it");
        last = r.left();
    }
    // The page field ("1 (1 of 36)") left the bar.
    for n in h.query_all_by_label_contains(" (1 of ") {
        let r = n.rect();
        assert!(
            r.bottom() < frame(&h).bottom() - 26.0,
            "no page field: {:?} {r:?}",
            n.accesskit_node().label()
        );
    }
    // Messages open: the findings field is one button, a click shows Messages.
    dispatch(&mut h, "messages.show", json!({"on": true}));
    let fields = ew_ui::ew_viewmodel::status::bar(h.state().shell(), false);
    if let Some(f) = fields
        .iter()
        .find(|f| f.kind == ew_ui::ew_viewmodel::status::StatusKind::Findings)
    {
        dispatch(&mut h, "messages.show", json!({"on": false}));
        assert!(
            h.query_by_label(&f.text).is_none(),
            "findings only while Messages is open"
        );
        dispatch(&mut h, "messages.show", json!({"on": true}));
        h.get_by_role_and_label(egui::Role::Button, &f.text).click();
        settle(&mut h);
        assert!(
            h.state()
                .shell()
                .navigator_open(ew_ui::ew_viewmodel::navigators::NavKind::Messages)
        );
    }
    h.state_mut().shell_mut().shutdown();
}

/// FT-519, FT-518: the history panel from the keyboard alone. F6 reaches the panel; Down moves the row cursor to the
/// older entry and Enter runs Undo to here on it; the header names the oldest step kept; the walker passes.
#[test]
fn ft519_the_history_panel_from_the_keyboard() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    for text in ["one", "two", "three"] {
        dispatch(
            &mut h,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
                "path": "function_text", "value": text}}),
        );
    }
    dispatch(&mut h, "history.show", json!({}));
    let cursor = |h: &Harness<'_, App>| h.state().shell().history_pane().view().map(|v| v.cursor);
    assert_eq!(cursor(&h), Some(3));
    // FT-518: the header names the oldest step kept and the memory against its bound.
    assert!(
        h.query_all_by_label_contains("oldest kept: step 1 · ")
            .next()
            .is_some(),
        "the kept line"
    );
    // F6 walks the ring to the panel (the history panel is its last stop while it is shown).
    for _ in 0..12 {
        if h.state().shell().focus() == Some(Focus::History) {
            break;
        }
        h.key_press(Key::F6);
        settle(&mut h);
    }
    assert_eq!(h.state().shell().focus(), Some(Focus::History));
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    let row = h.state().shell().history_pane().row_cursor();
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    let older = h.state().shell().history_pane().row_cursor();
    assert!(
        older.is_some() && older < row.or(Some(usize::MAX)),
        "Down moves toward older entries: {row:?} → {older:?}"
    );
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(cursor(&h), older.map(|i| i + 1), "Enter undid to the cursor row");
    support::assert_named(&h);
    h.state_mut().shell_mut().shutdown();
}

/// FX38d (cycle 38, play-test rv-vis-2 t0035–t0039): after Check project the status bar's findings field is there
/// with the Messages pane closed and its click opens the pane; the pane's own close button (in its header, the pane
/// alone in the dock) closes it; the canvas keeps its zoom and centre through the open and the close.
#[test]
fn fx38d_the_badge_opens_messages_and_its_close_button_closes_it() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    let messages = ew_ui::ew_viewmodel::navigators::NavKind::Messages;
    let view = |h: &Harness<'_, App>| {
        let c = h.state().shell().canvas().expect("a canvas");
        (c.zoom(), c.centre())
    };
    let before = view(&h);
    dispatch(&mut h, "messages.check", json!({}));
    assert!(!h.state().shell().navigator_open(messages));
    let field = ew_ui::ew_viewmodel::status::bar(h.state().shell(), false)
        .into_iter()
        .find(|f| f.kind == ew_ui::ew_viewmodel::status::StatusKind::Findings)
        .expect("the findings field after the check");
    h.get_by_role_and_label(egui::Role::Button, &field.text).click();
    settle(&mut h);
    assert!(
        h.state().shell().navigator_open(messages),
        "the badge opens Messages"
    );
    // FT-769: the untouched fit refits the shorter pane (the whole page stays in view).
    assert!(h.state().shell().canvas().unwrap().is_fitted());
    assert!(view(&h).0 < before.0, "the fit refits the shorter pane");
    let close = ew_ui::ew_viewmodel::strings::text("dock-close");
    h.get_by_role_and_label(egui::Role::Button, close).click();
    settle(&mut h);
    assert!(
        !h.state().shell().navigator_open(messages),
        "the close button closes the dock"
    );
    assert_eq!(view(&h), before, "closing the dock refits the first fit");
    h.state_mut().shell_mut().shutdown();
}
