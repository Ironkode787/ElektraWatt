// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the window: a click on a greyed command says why.

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::json;
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

/// The texts of the visible toasts.
fn toasts(h: &Harness<'_, App>) -> Vec<String> {
    h.state()
        .shell()
        .notifications()
        .visible()
        .iter()
        .map(|n| n.text.clone())
        .collect()
}

/// In the window: the Output tab's Parts list button, clicked on 102 zoomed in, shows 901 fitted in
/// the canvas with its page row marked in the page tree.
#[test]
fn the_parts_list_button_shows_the_bom_fitted_with_its_row_marked() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    for (action, params) in [
        ("page.goto", json!({ "folio": "102" })),
        ("view.command_tab", json!({ "tab": "output" })),
        ("view.zoom_in", json!({})),
    ] {
        assert_eq!(h.state_mut().shell_mut().dispatch(action, &params), Outcome::Done);
    }
    settle(&mut h);
    h.get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Parts list"))
        .expect("the Parts list button")
        .click();
    settle(&mut h);
    settle(&mut h);
    let s = h.state().shell();
    assert_eq!(s.current_folio(), Some("901"));
    let c = s.canvas().expect("a canvas");
    assert_eq!(c.zoom(), c.fit_zoom(), "fitted");
    let row = s
        .page_rows()
        .iter()
        .position(|r| r.folio == "901")
        .map(|i| s.navigator().key_of(s.page_rows(), i))
        .expect("the row of 901");
    assert!(s.navigator().is_marked(&row), "the row of 901 is marked");
    assert_eq!(s.navigator().marked().len(), 1);
    h.state_mut().shell_mut().shutdown();
}

/// "a click on a disabled command-bar button shows its reason as a toast, as well as in its
/// tooltip." On a section divider the Symbol button is greyed; a click on it runs nothing and the toast says why.
#[test]
fn a_click_on_a_greyed_command_shows_its_reason_as_a_toast() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    let divider = h
        .state()
        .shell()
        .page_rows()
        .iter()
        .find(|r| r.kind == "section_divider")
        .map(|r| r.folio.clone())
        .expect("Golden A has a section divider");
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.goto", &json!({ "folio": divider })),
        Outcome::Done
    );
    settle(&mut h);
    assert_eq!(toasts(&h), Vec::<String>::new());
    let symbol = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Symbol"))
        .expect("the Symbol button");
    assert!(symbol.accesskit_node().is_disabled(), "greyed on a divider");
    symbol.click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select", "nothing runs");
    assert_eq!(
        toasts(&h),
        ["A section divider takes no symbols or wires: draw on a schematic page"]
    );
    h.state_mut().shell_mut().shutdown();
}

/// Doc 16a P3: on a section divider, with the Select tool idle and nothing selected, the canvas HUD
/// says why the drawing tools are greyed (the page type's reason, whole), where the user looks; on a schematic page
/// it says the tool's hint again.
#[test]
fn the_hud_gives_the_page_types_reason_while_no_tool_is_armed() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    let divider = h
        .state()
        .shell()
        .page_rows()
        .iter()
        .find(|r| r.kind == "section_divider")
        .map(|r| r.folio.clone())
        .expect("Golden A has a section divider");
    let prompt = |h: &Harness<'_, App>| ew_ui::ew_viewmodel::command_bar::hud(h.state().shell()).prompt;
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.goto", &json!({ "folio": divider })),
        Outcome::Done
    );
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert_eq!(
        prompt(&h),
        "A section divider takes no symbols or wires: draw on a schematic page"
    );
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.goto", &json!({ "folio": "101" })),
        Outcome::Done
    );
    settle(&mut h);
    assert_eq!(prompt(&h), "Click to select; drag for a rubber band");
    h.state_mut().shell_mut().shutdown();
}
