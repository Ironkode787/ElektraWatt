// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 2: the command surface in the window without rendering. CB2 (the tabs and the Draw row's buttons,
//! each dispatching its action), CB3 (the keyboard: Alt, F10, Ctrl+F1, F6, Left and Right), CB4 (the geometry at
//! 1280 by 800 in both densities), CB5 (one HUD with the selection's prompt, no tool strip), CB8 (the app button),
//! FT-514 (a repaint when the window gets the focus back) and IC2 (every icon key of the registry resolves).

use crate::support;

use egui::{Key, Modifiers, Rect, Vec2, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::actions::ACTIONS;
use ew_ui::ew_viewmodel::command_bar::{ButtonKind, CommandSize, Tab, command_bar, command_bar_of};
use ew_ui::ew_viewmodel::shell::{Focus, Mode, Outcome, ShellModel};
use ew_ui::icons::IconId;
use serde_json::json;
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

/// The main window on a copy of Golden A at 1280 by 800 in `density`, folio 101 on show.
fn window(tmp: &Tmp, density: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(
        shell.dispatch("settings.set", &json!({"key": "density", "value": density})),
        Outcome::Done
    );
    let mut h = harness_with(shell, Scripted::default(), Some(dir));
    h.set_size(Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.goto", &json!({"folio": "101"})),
        Outcome::Done
    );
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

/// The rectangle of the only node with `role` and `label`.
fn rect_of(h: &Harness<'_, App>, role: egui::Role, label: &str) -> Rect {
    h.get_by_role_and_label(role, label).rect()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// CB2: the tab list holds the six tabs Project to View in order, Draw selected; every shown button of the Draw row
/// is named by its short label and carries its shortcut; a click on each enabled plain or split button dispatches its
/// action.
#[test]
fn cb2_the_tabs_and_the_draw_row_dispatch_their_actions() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    let list = h.get_by_role_and_label(egui::Role::TabList, "Command tabs");
    let tabs: Vec<(String, bool)> = list
        .get_all_by_role(egui::Role::Tab)
        .map(|n| {
            let n = n.accesskit_node();
            (n.label().unwrap_or_default(), n.is_selected().unwrap_or(false))
        })
        .collect();
    let names: Vec<&str> = tabs.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Project", "Page", "Draw", "Check", "Output", "View"]);
    assert_eq!(
        tabs.iter()
            .filter(|(_, on)| *on)
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["Draw"],
        "Draw is the tab on show"
    );
    let model = command_bar_of(h.state().shell(), Tab::Draw);
    let mut clicked = 0;
    for group in &model.groups {
        for b in &group.buttons {
            let Some(node) = h.query_all_by_label(&b.bar_label).find(|n| {
                n.accesskit_node().role() == egui::Role::Button
                    && support::bar_button(&h, &b.bar_label).accesskit_node().id() == n.accesskit_node().id()
            }) else {
                // A group folded into More at this width.
                continue;
            };
            if !b.shortcut.is_empty() {
                assert!(
                    node.accesskit_node().data().keyboard_shortcut().is_some(),
                    "{} carries its shortcut",
                    b.id
                );
            }
            if !b.enabled || matches!(b.kind, ButtonKind::Menu(_)) {
                continue;
            }
            node.click();
            // ID-1477 (7): the symbol editor takes the place of the page and its docks; Close editor returns to them.
            if b.id == "symbol.new" {
                h.run_ok();
                assert_eq!(h.state().dispatched().last().copied(), Some(b.id));
                let _ = h.state_mut().shell_mut().dispatch("symbol.close", &json!({}));
                settle(&mut h);
                clicked += 1;
                continue;
            }
            settle(&mut h);
            assert_eq!(
                h.state().dispatched().last().copied(),
                Some(b.id),
                "the button {:?} dispatches its action",
                b.bar_label
            );
            clicked += 1;
            // Back to rest: a dialog or a running tool would change the row's state for the next button.
            h.key_press(Key::Escape);
            settle(&mut h);
            let _ = h.state_mut().shell_mut().dispatch("tool.select", &json!({}));
            settle(&mut h);
        }
    }
    assert!(clicked >= 8, "most of the Draw row was clicked: {clicked}");
    h.state_mut().shell_mut().shutdown();
}

/// CB3: Alt pressed and released alone opens the app menu, Escape closes it, F10 opens it; Ctrl+F1 collapses the bar
/// to its tab row and restores it; F6 reaches the command bar, and there Right and Left step through the tabs.
#[test]
fn cb3_the_command_surface_from_the_keyboard() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    assert!(
        support::off_chrome(&h, "File").is_none(),
        "the app menu is closed"
    );
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(Modifiers::ALT));
    h.run_ok();
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.run_ok();
    settle(&mut h);
    assert!(
        support::off_chrome(&h, "File").is_some(),
        "Alt alone opens the app menu"
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(support::off_chrome(&h, "File").is_none(), "Escape closes it");
    h.key_press(Key::F10);
    settle(&mut h);
    assert!(support::off_chrome(&h, "File").is_some(), "F10 opens it");
    h.key_press(Key::Escape);
    settle(&mut h);
    // Ctrl+F1.
    let full = h.state().command_surface_rects().0.unwrap().height();
    h.key_press_modifiers(Modifiers::COMMAND, Key::F1);
    settle(&mut h);
    assert!(h.state().shell().command_bar_collapsed());
    let collapsed = h.state().command_surface_rects().0.unwrap().height();
    assert!(close(collapsed, 33.0), "the tab row and its line: {collapsed}");
    h.key_press_modifiers(Modifiers::COMMAND, Key::F1);
    settle(&mut h);
    assert!(!h.state().shell().command_bar_collapsed());
    assert!(close(h.state().command_surface_rects().0.unwrap().height(), full));
    // F6 round the areas to the command bar.
    for _ in 0..6 {
        if h.state().shell().focus() == Some(Focus::CommandBar) {
            break;
        }
        h.key_press(Key::F6);
        settle(&mut h);
    }
    assert_eq!(h.state().shell().focus(), Some(Focus::CommandBar));
    assert_eq!(h.state().shell().command_tab(), Tab::Draw);
    h.key_press(Key::ArrowRight);
    settle(&mut h);
    assert_eq!(h.state().shell().command_tab(), Tab::Check);
    h.key_press(Key::ArrowLeft);
    h.key_press(Key::ArrowLeft);
    settle(&mut h);
    assert_eq!(h.state().shell().command_tab(), Tab::Page);
    h.state_mut().shell_mut().shutdown();
}

/// Doc 16a §5.5: after a fit on Golden A 101 at 1280 by 800 the sheet ends above the HUD (the fit keeps the HUD's
/// band clear) and keeps off the zoom control; the view is still the fit.
#[test]
fn a_fitted_sheet_ends_above_the_hud() {
    for density in ["comfortable", "compact"] {
        let tmp = Tmp::new();
        let mut h = window(&tmp, density);
        assert!(h.state_mut().shell_mut().dispatch("view.fit_page", &json!({})) == Outcome::Done);
        settle(&mut h);
        wait_for_page(&mut h);
        settle(&mut h);
        let (_, hud, pane) = h.state().command_surface_rects();
        let (hud, pane) = (hud.unwrap(), pane.unwrap());
        let canvas = h.state().shell().canvas_of(0).unwrap();
        assert!(canvas.is_fitted() && canvas.fit_inset() > 0.0, "{density}");
        let (w, ht) = canvas.sheet();
        let host = h.state_mut().canvas_host();
        let (a, b) = (
            host.drawn_screen_of(0, (0, 0)).unwrap(),
            host.drawn_screen_of(0, (w, ht)).unwrap(),
        );
        let sheet = Rect::from_two_pos(a, b);
        assert!(
            !sheet.intersects(hud),
            "{density}: the sheet {sheet:?} under the HUD {hud:?}"
        );
        assert!(sheet.bottom() < hud.top(), "{density}: {sheet:?} above {hud:?}");
        assert!(pane.contains_rect(sheet), "{density}: {sheet:?} in {pane:?}");
    }
}

/// CB4: the geometry of doc 16a §5 at 1280 by 800, Comfortable and Compact: the bar, its tab row and command row,
/// the large and small buttons, the search field, the quick-access buttons, the HUD (36 high, 16 above the page
/// view's bottom, centred on it) and a page view of at least 65 % of the width.
#[test]
fn cb4_the_geometry_at_1280_by_800_in_both_densities() {
    for (density, bar, large_h, large_w, row) in [
        ("comfortable", 85.0, 46.0, 52.0, 52.0),
        ("compact", 69.0, 28.0, 0.0, 36.0),
    ] {
        let tmp = Tmp::new();
        let mut h = window(&tmp, density);
        let (bar_rect, hud, pane) = h.state().command_surface_rects();
        let (bar_rect, hud, pane) = (bar_rect.unwrap(), hud.unwrap(), pane.unwrap());
        assert!(close(bar_rect.height(), bar), "{density}: the bar {bar_rect:?}");
        // The harness puts its 8-px outer margin round the frame (egui_kittest's `Frame::central_panel`): the bar
        // spans the frame's full width at its top.
        let frame = h.ctx.content_rect().shrink(8.0);
        assert!(
            close(bar_rect.top(), frame.top()) && close(bar_rect.width(), frame.width()),
            "{density}: {bar_rect:?} in {frame:?}"
        );
        let tabs = rect_of(&h, egui::Role::TabList, "Command tabs");
        assert!(
            tabs.bottom() <= bar_rect.top() + 32.0 + 0.5,
            "{density}: the tab row {tabs:?}"
        );
        let model = command_bar(h.state().shell());
        let mut seen = (0, 0);
        for b in model.groups.iter().flat_map(|g| &g.buttons) {
            // ID-1497 (10): the bar's own button (a group collapsed at the floor shows none; the HUD's Text is no
            // button of the bar).
            let Some(r) = h
                .query_all_by_label(&b.bar_label)
                .find(|n| n.accesskit_node().role() == egui::Role::Button && bar_rect.contains_rect(n.rect()))
                .map(|n| n.rect())
            else {
                continue;
            };
            assert!(
                r.top() >= bar_rect.top() + 32.0 - 0.5 && r.bottom() <= bar_rect.top() + 32.0 + row + 0.5,
                "{density}: {} sits in the command row: {r:?}",
                b.id
            );
            match b.size {
                CommandSize::Large => {
                    assert!(close(r.height(), large_h), "{density}: {} {r:?}", b.id);
                    assert!(r.width() >= large_w - 0.5, "{density}: {} {r:?}", b.id);
                    seen.0 += 1;
                }
                CommandSize::Small => {
                    assert!(close(r.height(), 24.0), "{density}: {} {r:?}", b.id);
                    seen.1 += 1;
                }
            }
        }
        assert!(
            seen.0 > 0 && seen.1 > 0,
            "{density}: large and small buttons {seen:?}"
        );
        let search = h
            .get_all_by_role(egui::Role::SearchInput)
            .next()
            .expect("the search field")
            .rect();
        assert!(
            close(search.width(), 280.0) && close(search.height(), 24.0),
            "{density}: {search:?}"
        );
        for q in &model.quick {
            let r = support::bar_button(&h, &q.label).rect();
            assert!(
                close(r.width(), 28.0) && close(r.height(), 24.0),
                "{density}: {} {r:?}",
                q.id
            );
        }
        assert!(close(hud.height(), 36.0), "{density}: the HUD {hud:?}");
        assert!(
            close(pane.bottom() - hud.bottom(), 16.0),
            "{density}: {hud:?} in {pane:?}"
        );
        assert!(
            close(hud.center().x, pane.center().x),
            "{density}: {hud:?} in {pane:?}"
        );
        // WP-S2.17 part 3's default workspace (D10): the page view takes at least 65 % of the width.
        assert!(
            pane.width() >= 0.65 * frame.width(),
            "{density}: the page view {pane:?}"
        );
        h.state_mut().shell_mut().shutdown();
    }
}

/// CB5: one HUD, a `Status` node named "Tool" whose value names the tool and the selection's keys; the tool strip
/// is gone.
#[test]
fn cb5_one_hud_with_the_prompt_of_the_selection() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    assert_eq!(
        h.get_all_by_role(egui::Role::Status)
            .filter(|n| n.accesskit_node().label().as_deref() == Some("Tool"))
            .count(),
        1
    );
    // ID-1497 (8): an item drawn on the page (a selection that draws nothing there is not counted).
    let drawn = h.state().shell().canvas().unwrap().page().entities[0].0;
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("select.set", &json!({"ids": [drawn.0.to_string()]})),
        Outcome::Done
    );
    settle(&mut h);
    let value = support::hud_value(&h);
    assert!(value.starts_with("Select"), "{value}");
    assert!(
        value.ends_with("1 selected \u{b7} Del deletes \u{b7} R rotates"),
        "{value}"
    );
    assert!(
        h.query_all_by_label_contains("Select: ").next().is_none(),
        "no tool strip"
    );
    // Arming Place on folio 101: the HUD names the tool, the target (the strip until the pointer predicts the
    // terminal) and the keys; still one Status node.
    assert_eq!(
        h.state_mut().shell_mut().dispatch(
            "place.pick",
            &json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
        ),
        Outcome::Done
    );
    settle(&mut h);
    let value = support::hud_value(&h);
    assert!(value.starts_with("Place"), "{value}");
    assert!(value.contains("-X1031"), "{value}");
    assert!(
        value.ends_with("R rotates \u{b7} V variant \u{b7} Esc done"),
        "{value}"
    );
    assert_eq!(
        h.get_all_by_role(egui::Role::Status)
            .filter(|n| n.accesskit_node().label().as_deref() == Some("Tool"))
            .count(),
        1
    );
    h.state_mut().shell_mut().shutdown();
}

/// CB8: the app button is named "Menu", described "ElektraWatt", carries F10 and opens the ten menus.
#[test]
fn cb8_the_app_button() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    let app = support::bar_button(&h, "Menu");
    assert_eq!(app.accesskit_node().data().description(), Some("ElektraWatt"));
    assert!(app.accesskit_node().data().keyboard_shortcut().is_some());
    app.click();
    settle(&mut h);
    for label in [
        "File", "Edit", "Project", "Page", "Draw", "Check", "Output", "View", "Window", "Help",
    ] {
        assert!(
            support::off_chrome(&h, label).is_some(),
            "the app menu holds {label}"
        );
    }
    h.state_mut().shell_mut().shutdown();
}

/// FT-514: the window repaints when it gets the focus back (a native file dialog closed), so the status hint is
/// never stale.
#[test]
fn ft514_a_repaint_when_the_window_gets_the_focus_back() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    let before = h.state().focus_repaints();
    let focused = |h: &mut Harness<'_, App>, on: bool| {
        h.input_mut()
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .focused = Some(on);
    };
    focused(&mut h, false);
    h.run_ok();
    assert_eq!(
        h.state().focus_repaints(),
        before,
        "losing the focus repaints nothing"
    );
    focused(&mut h, true);
    h.run_ok();
    assert_eq!(h.state().focus_repaints(), before + 1);
    h.run_ok();
    assert_eq!(h.state().focus_repaints(), before + 1, "once per focus change");
    h.state_mut().shell_mut().shutdown();
}

/// IC2: every icon key of the registry names an icon of the set, and every action placed on the bar has one.
#[test]
fn ic2_every_icon_key_resolves() {
    for a in ACTIONS {
        if let Some(key) = a.icon {
            assert!(IconId::from_key(key).is_some(), "{}: {key}", a.id);
        }
        if a.placement.is_some() {
            assert!(a.icon.is_some(), "{} is on the bar without an icon", a.id);
        }
    }
}

/// FT-822 (cycle-39 play-test): the banner lies over the top edge of the canvas below the page tabs; showing and
/// dismissing it moves neither the page view nor the command bar, so a click aimed before it appears still lands.
#[test]
fn ft822_the_banner_overlays_the_canvas_and_moves_nothing() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    let (bar, _, pane) = h.state().command_surface_rects();
    let (bar, pane) = (bar.expect("the command bar"), pane.expect("the page view"));
    h.state_mut().shell_mut().show_banner("Something to read");
    settle(&mut h);
    let (bar2, _, pane2) = h.state().command_surface_rects();
    assert_eq!(bar2, Some(bar), "the command bar stays");
    assert_eq!(pane2, Some(pane), "the page view stays");
    let text = h.get_by_label("Something to read").rect();
    assert!(
        pane.contains(text.center()) && text.top() >= pane.top(),
        "the banner lies over the page view: {text:?} in {pane:?}"
    );
    h.get_by_label("Dismiss").click();
    settle(&mut h);
    assert!(h.state().shell().banner().is_none());
    assert_eq!(
        h.state().command_surface_rects().2,
        Some(pane),
        "dismissing moves nothing"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-1279: in a 1000 × 700 window the View menu's every entry, the Show group's Grid, Frame statistics and the CPU
/// canvas among them, lies inside the window, and Grid is reached and runs from it.
#[test]
fn ft1279_the_view_menu_fits_a_1000_by_700_window() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    h.set_size(Vec2::new(1000.0, 700.0));
    settle(&mut h);
    support::open_menu(&mut h, &["View"]);
    settle(&mut h);
    let screen = h.ctx.content_rect();
    let view = support::menu_node(&h, "View").rect();
    let mut outside = Vec::new();
    let mut seen = Vec::new();
    for n in h.root().children_recursive() {
        let node = n.accesskit_node();
        let role = node.role();
        if !matches!(
            role,
            egui::Role::MenuItem | egui::Role::MenuItemCheckBox | egui::Role::Button
        ) {
            continue;
        }
        let r = n.rect();
        // The View menu's column: in a menu, right of the app menu's column that holds "View".
        let mut in_menu = false;
        let mut at = node.parent();
        while let Some(a) = at {
            in_menu |= a.role() == egui::Role::Menu;
            at = a.parent();
        }
        if !in_menu || r.left() < view.right() - 1.0 {
            continue;
        }
        let label = node.label().unwrap_or_default();
        seen.push(label.clone());
        if r.bottom() > screen.bottom() + 0.5 || r.top() < screen.top() - 0.5 {
            outside.push(format!("{label} at {r:?}"));
        }
    }
    // FT-1367 (ID-1392): Show tips joined the Show group, which has four entries now and so is a submenu (doc 16a
    // §9.2): Grid, Frame statistics and the CPU canvas are reached through it.
    for show in ["Show", "Wire colours"] {
        assert!(seen.iter().any(|s| s == show), "{show} in {seen:?}");
    }
    assert_eq!(outside, Vec::<String>::new());
    support::menu_node(&h, "Show").click();
    settle(&mut h);
    support::menu_node(&h, "Grid").click();
    settle(&mut h);
    assert!(!h.state().shell().grid_on());
    h.state_mut().shell_mut().shutdown();
}

/// FT-1520 (ID-1456, doc 16a §6.13): View ▸ Show's switches draw only their check in the menu's check column, blank
/// when off, never their icons: Grid (on) shows its check, Frame statistics (off) nothing.
#[test]
fn ft1520_view_show_switches_show_only_their_check() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    assert!(h.state().shell().grid_on());
    assert!(!h.state().shell().frame_stats_shown());
    support::open_menu(&mut h, &["View", "Show"]);
    settle(&mut h);
    // The shapes other than text, painted in a row's check column (its left 32 px), smaller than the row's fill.
    fn walk(shape: &egui::Shape, out: &mut Vec<Rect>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(_) | egui::Shape::Noop => {}
            other => out.push(other.visual_bounding_rect()),
        }
    }
    let mut painted = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut painted);
    }
    let in_column = |label: &str| {
        let row = support::menu_node(&h, label).rect();
        let column = Rect::from_min_max(row.left_top(), egui::pos2(row.left() + 32.0, row.bottom()));
        painted
            .iter()
            .filter(|r| r.is_positive() && r.width() < 24.0 && column.contains_rect(**r))
            .count()
    };
    assert!(in_column("Grid") > 0, "Grid's check");
    assert_eq!(
        in_column("Frame statistics"),
        0,
        "no icon beside Frame statistics"
    );
    h.state_mut().shell_mut().shutdown();
}

/// ID-1497 (11): the HUD moves under the tab bar while the pointer is within 48 px of its place at the bottom, and
/// back only once the pointer is more than 96 px away, so it does not flip back and forth between the two.
#[test]
fn the_hud_moves_up_near_the_pointer_and_back_only_far_from_it() {
    let tmp = Tmp::new();
    let mut h = window(&tmp, "comfortable");
    let (_, hud, pane) = h.state().command_surface_rects();
    let (low, pane) = (hud.unwrap(), pane.unwrap());
    let hud_at = |h: &mut Harness<'static, App>, p: egui::Pos2| {
        h.hover_at(p);
        settle(h);
        h.state().command_surface_rects().1.unwrap()
    };
    let above = |d: f32| egui::pos2(low.center().x, low.top() - d);
    assert_eq!(hud_at(&mut h, above(150.0)), low, "far: at the bottom");
    let raised = hud_at(&mut h, above(40.0));
    assert!(
        raised.center().y < pane.center().y,
        "within 48 px: raised {raised:?}"
    );
    assert_eq!(
        hud_at(&mut h, above(70.0)),
        raised,
        "between 48 and 96 px: still raised"
    );
    assert_eq!(
        hud_at(&mut h, above(110.0)),
        low,
        "past 96 px: back at the bottom"
    );
    assert_eq!(
        hud_at(&mut h, above(70.0)),
        low,
        "between 48 and 96 px from below: stays low"
    );
    h.state_mut().shell_mut().shutdown();
}
