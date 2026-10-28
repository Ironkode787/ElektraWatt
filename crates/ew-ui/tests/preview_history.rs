// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 3 in the window (§10.3): Renumber devices... opens its dialog, OK opens the `bulk` card in the Preview
//! tab with Golden B1's tag changes and Apply commits it (PV2); the property grid's card is a one-line chip while the
//! tab shows it (PV3); the history panel lists the steps with Undo to here and Show changes (PV5); each followed by the
//! accessible-name walker.

mod support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// Whether a node shows `text` as its name or (a label) as its value.
fn has_text(h: &Harness<'_, App>, text: &str) -> bool {
    h.root().children_recursive().any(|n| {
        let n = n.accesskit_node();
        n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text)
    })
}

fn group_named(h: &Harness<'_, App>, name: &str) -> bool {
    h.get_all_by_role(egui::Role::Group)
        .any(|n| n.accesskit_node().label().as_deref() == Some(name))
}

#[test]
fn pv2_renumber_devices_through_the_dialog_and_the_preview_tab() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": {"default": "page-based",
            "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}", "rules": [], "child_separator": "-"}]}}}),
    );
    dispatch(&mut h, "devices.renumber", json!({}));
    settle(&mut h);
    assert!(
        h.query_by_label("Renumber devices").is_some(),
        "the dialog's title"
    );
    h.state_mut().shell_mut().set_form_field("include_frozen", "true");
    settle(&mut h);
    support::off_chrome(&h, "Renumber")
        .expect("the dialog's Renumber")
        .click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    assert!(group_named(&h, "Preview"), "the Preview tab");
    assert!(
        has_text(&h, "Preview: numbering.renumber · as of rev 1"),
        "the header"
    );
    assert!(has_text(
        &h,
        "8 records · 11 tags · 0 cross-reference texts · 1 pages"
    ));
    assert!(
        has_text(&h, "-XPE → -101X2"),
        "the first tag change, virtualised rows"
    );
    assert!(h.query_by_label("Tags (11)").is_some());
    let before = h.state().shell().can_undo();
    h.get_by_label("Apply").click();
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    assert!(h.state().shell().can_undo() || before);
    assert!(has_text(&h, "No preview is open"));
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn pv3_the_grids_card_is_a_chip_while_the_tab_shows_it() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    settle(&mut h);
    assert!(group_named(&h, "Preview"));
    let chip = h
        .root()
        .children_recursive()
        .filter_map(|n| n.accesskit_node().value())
        .find(|l| l.starts_with("Preview open: "));
    assert!(chip.is_some(), "the chip");
    assert!(h.query_by_label("Show").is_some(), "the chip's Show");
    // Cancel from the chip's own button: the first Cancel is the chip's or the tab's, either ends the card.
    h.get_all_by_label("Cancel").next().unwrap().click();
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn pv5_the_history_panel_lists_the_steps_with_undo_to_here_and_show_changes() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    for text in ["one", "two"] {
        dispatch(
            &mut h,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
                "path": "function_text", "value": text}}),
        );
    }
    dispatch(&mut h, "history.show", json!({}));
    settle(&mut h);
    assert!(group_named(&h, "History"), "the panel");
    assert!(has_text(&h, "2 steps · cursor at 2"));
    assert_eq!(h.get_all_by_label("Undo to here").count(), 2);
    // Show changes of the newest entry (the first row): a window with the record.
    h.get_all_by_label("Show changes").next().unwrap().click();
    settle(&mut h);
    assert!(h.query_by_label("Changes of step 2").is_some());
    assert!(has_text(&h, "changed device -101F1 (function_text)"));
    h.get_by_label("Close").click();
    settle(&mut h);
    assert!(h.state().shell().history_pane().changes().is_none());
    // Undo to here on the oldest entry (the last row): one step undone.
    h.get_all_by_label("Undo to here").last().unwrap().click();
    settle(&mut h);
    assert_eq!(h.state().shell().history_pane().view().unwrap().cursor, 1);
    assert!(has_text(&h, "2 steps · cursor at 1"));
    assert!(!matches!(
        h.state().shell().dialog(),
        Some(Dialog::ConfirmPurge { .. })
    ));
    h.state_mut().shell_mut().shutdown();
}

/// W31-2: at 1280×800 the renumber card's first tag change is on show in the Preview tab without a click: Tags is the
/// list that carries the change, first and open, below Apply and Cancel.
#[test]
fn w31_2_the_first_tag_change_is_on_show_at_1280_by_800() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": {"default": "page-based",
            "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}", "rules": [], "child_separator": "-"}]}}}),
    );
    dispatch(&mut h, "devices.renumber", json!({}));
    settle(&mut h);
    h.state_mut().shell_mut().set_form_field("include_frozen", "true");
    settle(&mut h);
    support::off_chrome(&h, "Renumber")
        .expect("the dialog's Renumber")
        .click();
    settle(&mut h);
    settle(&mut h);
    let pane = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Preview"))
        .expect("the Preview tab")
        .rect();
    let first = h.get_by_label("-XPE → -101X2").rect();
    assert!(
        pane.expand(1.0).contains_rect(first) && first.bottom() <= 800.0,
        "the first tag change {first:?} inside the tab {pane:?}"
    );
    let apply = h.get_by_label("Apply").rect();
    assert!(apply.bottom() <= first.top(), "Apply {apply:?} above the lists");
    assert!(h.query_by_label("Tags (11)").is_some());
    h.state_mut().shell_mut().shutdown();
}

/// W31-7: an undone entry offers Redo to here (`history.undo_to` redoes up to it, §8.4), not Undo to here; the applied
/// entries keep Undo to here. Its row is named `…, undone`.
#[test]
fn w31_7_an_undone_entry_offers_redo_to_here() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    for text in ["one", "two", "three"] {
        dispatch(
            &mut h,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
                "path": "function_text", "value": text}}),
        );
    }
    dispatch(&mut h, "history.show", json!({}));
    settle(&mut h);
    assert_eq!(h.get_all_by_label("Undo to here").count(), 3);
    assert_eq!(h.query_all_by_label("Redo to here").count(), 0);
    dispatch(&mut h, "history.undo_to", json!({"entry": 1}));
    settle(&mut h);
    assert!(has_text(&h, "3 steps · cursor at 1"));
    // Entries 3 and 2 are undone (the first two rows), entry 1 is applied.
    assert_eq!(h.get_all_by_label("Redo to here").count(), 2);
    assert_eq!(h.get_all_by_label("Undo to here").count(), 1);
    let undone = h
        .get_all_by_role(egui::Role::Group)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.ends_with(", undone"))
        .count();
    assert_eq!(undone, 2, "the undone rows are named so");
    // Redo to here on entry 3 (the first row) redoes both.
    h.get_all_by_label("Redo to here").next().unwrap().click();
    settle(&mut h);
    assert_eq!(h.state().shell().history_pane().view().unwrap().cursor, 3);
    assert_eq!(h.query_all_by_label("Redo to here").count(), 0);
    h.state_mut().shell_mut().shutdown();
}
