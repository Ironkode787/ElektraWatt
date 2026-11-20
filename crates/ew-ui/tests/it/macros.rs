// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the window: the Macros navigator's detail of the control macro (name, version, "No
//! preview", its devices); inserting the control macro on folio 106 from the navigator (the supply block's card lists
//! EW-CORE-052's refusal of its second sources; the core macro's card has no conflicts and Apply is one undo
//! step) as a kittest flow and its `ew.uiscript/1` twin, and the accessible-name walker of every settle.
//!
//! The control macro is saved first through Save as macro… into a fixture user library on another copy of the sample (no
//! macro ships in `content/`); a headless script cannot save one (a library write is a front-end operation).

use crate::support;

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_viewmodel::ew_api::library::{LibraryTiers, Registry};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::script::{self, NoExtension};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, done, harness_with, settle};

/// The fixture's tiers: the compiled-in libraries and a user directory.
fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// A shell in `mode` with the tiers `t` on the copy `name` of the sample, showing folio 102.
fn shell_on(tmp: &Tmp, name: &str, mode: Mode, t: &LibraryTiers) -> ShellModel {
    let dir = tmp.sample(name);
    let config = ShellConfig::in_dir(&tmp.0.join(format!("config-{name}")), "0.0.0-test")
        .with_libraries(Some(t.clone()));
    let mut shell = ShellModel::new(mode, config);
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    shell
}

/// The fixture: a user library "Mine" holding "Control 24 V DC" (folio 102's devices with their wires) and
/// "Supply block" (folio 101 whole, its interruption points included), saved through Save as macro… in the window. Returns
/// the tiers and the two macros' row keys.
fn fixture(tmp: &Tmp) -> (LibraryTiers, String, String) {
    let t = tiers(&tmp.0);
    Registry::open(&t)
        .unwrap()
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    let mut shell = shell_on(tmp, "save.ew", Mode::Gui, &t);
    for (name, folio, ips) in [("Control 24 V DC", "102", false), ("Supply block", "101", true)] {
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        let tables = &shell.canvas().expect("the page").page().tables;
        let ids: Vec<String> = tables
            .reps
            .iter()
            .filter(|r| ips || r.device.is_some())
            .map(|r| r.id)
            .chain(tables.connections.iter().map(|c| c.id))
            .chain(tables.junctions.iter().map(|j| j.id))
            .map(|id| id.to_string())
            .collect();
        done(&mut shell, "select.set", json!({ "ids": ids }));
        done(
            &mut shell,
            "macros.save",
            json!({"name": name, "library": "Mine", "confirm": true}),
        );
    }
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));
    let key = |label: &str| {
        let t = shell.nav(NavKind::Macros).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::Macro && n.label == label)
            .map(|n| n.key.clone())
            .unwrap()
    };
    let (core, block) = (key("Control 24 V DC"), key("Supply block"));
    shell.shutdown();
    (t, core, block)
}

/// Whether a node shows `text` as its name or its value.
fn has_text(h: &Harness<'_, App>, text: &str) -> bool {
    h.root().children_recursive().any(|n| {
        let n = n.accesskit_node();
        n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text)
    })
}

/// Whether a node's name or value contains `text`.
fn has_text_part(h: &Harness<'_, App>, text: &str) -> bool {
    h.root().children_recursive().any(|n| {
        let n = n.accesskit_node();
        n.label().is_some_and(|l| l.contains(text)) || n.value().is_some_and(|v| v.contains(text))
    })
}

fn group_named(h: &Harness<'_, App>, name: &str) -> bool {
    h.get_all_by_role(egui::Role::Group)
        .any(|n| n.accesskit_node().label().as_deref() == Some(name))
}

/// A click on the detail's Insert macro…, scrolled into view first: a macro of many devices lists them above it, below
/// the pane's fold.
fn insert_macro(h: &mut Harness<'_, App>) {
    h.get_by_label("Insert macro\u{2026}").scroll_to_me();
    settle(h);
    h.get_by_label("Insert macro\u{2026}").click();
    settle(h);
}

/// The number of representations of the page on show.
fn reps(h: &Harness<'_, App>) -> usize {
    h.state()
        .shell()
        .canvas()
        .map_or(0, |c| c.page().tables.reps.len())
}

/// UI-1 and UI-2 in the window, each settle followed by the accessible-name walker (UI-6).
#[test]
fn insert_the_control_macro_on_folio_106_from_the_macros_navigator() {
    let tmp = Tmp::new();
    let (t, core, block) = fixture(&tmp);
    let shell = shell_on(&tmp, "s.ew", Mode::Gui, &t);
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    let dispatch = |h: &mut Harness<'_, App>, action: &str, params: Value| {
        assert_eq!(
            h.state_mut().shell_mut().dispatch(action, &params),
            Outcome::Done,
            "{action} {params}"
        );
    };
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "106", "type": "multi_line", "title": "Control 2"}),
    );
    dispatch(&mut h, "page.goto", json!({"folio": "106"}));
    dispatch(&mut h, "navigator.show", json!({"kind": "macros"}));
    settle(&mut h);
    // The navigator's header and the rail's entry.
    assert!(
        h.query_all_by_label("Macros").count() >= 1,
        "the Macros navigator"
    );
    // UI-1: the detail of the macro under the cursor.
    dispatch(&mut h, "navigator.row", json!({"kind": "macros", "key": core}));
    settle(&mut h);
    assert!(group_named(&h, "Control 24 V DC"), "the detail block");
    // The thumbnail, an image named after the macro, in place of "No preview".
    assert!(
        h.get_all_by_role(egui::Role::Image)
            .any(|n| n.accesskit_node().label().as_deref() == Some("Preview of Control 24 V DC")),
        "the thumbnail"
    );
    assert!(!has_text(&h, "No preview"));
    // Folio 102 as the sample's script draws it: a breaker, a contactor, two push buttons, a lamp, and a strip with eight
    // terminals.
    assert!(has_text(&h, "Devices (14)"));
    assert!(has_text_part(&h, "Version 1.0.0 · Mine"));

    // UI-2, the supply block: each of folio 101's sources (1L1, 1L2, 1L3, PE, 24V, 0V) would be a second source of its
    // name, so the card lists the rule's refusal of each (the pairings a card lists: `ew-viewmodel`'s
    // `macros_libraries`).
    dispatch(&mut h, "navigator.row", json!({"kind": "macros", "key": block}));
    settle(&mut h);
    insert_macro(&mut h);
    assert_eq!(h.state().shell().tool().name(), "macro");
    dispatch(&mut h, "canvas.press", json!({"x_mm": 100.0, "y_mm": 100.0}));
    dispatch(&mut h, "canvas.release", json!({"x_mm": 100.0, "y_mm": 100.0}));
    settle(&mut h);
    assert!(group_named(&h, "Preview"), "the Preview tab");
    assert!(h.query_by_label("Conflicts (3)").is_some() || has_text_part(&h, "Conflicts"));
    assert!(
        has_text_part(&h, "interruption point 24V: 2 source points"),
        "the refusal of 24V"
    );
    dispatch(&mut h, "preview.cancel", json!({}));
    settle(&mut h);

    // UI-2, the core macro: no conflicts; Apply is one undo step.
    dispatch(&mut h, "navigator.row", json!({"kind": "macros", "key": core}));
    settle(&mut h);
    insert_macro(&mut h);
    dispatch(&mut h, "canvas.press", json!({"x_mm": 100.0, "y_mm": 100.0}));
    dispatch(&mut h, "canvas.release", json!({"x_mm": 100.0, "y_mm": 100.0}));
    settle(&mut h);
    assert!(has_text(&h, "No conflicts"));
    let before = reps(&h);
    let rev = h.state().shell().revision().unwrap();
    h.get_by_label("Apply").click();
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    assert_eq!(h.state().shell().revision(), Some(rev + 1), "one transaction");
    assert!(reps(&h) > before);
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    assert_eq!(reps(&h), before, "one undo step");
    h.state_mut().shell_mut().shutdown();
}

/// UI-2's `ew.uiscript/1` twin, headless: the same steps as script lines; the block's Apply fails with the rule's
/// text, the core macro's Apply is one command-log line and one undo step.
#[test]
fn the_uiscript_twin() {
    let tmp = Tmp::new();
    let (t, core, block) = fixture(&tmp);
    let mut shell = shell_on(&tmp, "s.ew", Mode::Headless, &t);
    let step = |action: &str, params: Value| json!({"action": action, "params": params}).to_string();
    let lines = [
        r#"{"$schema":"ew.uiscript/1","label":"UI-2"}"#.to_owned(),
        step(
            "page.new",
            json!({"folio": "106", "type": "multi_line", "title": "Control 2"}),
        ),
        step("page.goto", json!({"folio": "106"})),
        step("navigator.show", json!({"kind": "macros"})),
        step("navigator.open", json!({"kind": "macros", "key": block})),
        step("canvas.press", json!({"x_mm": 100.0, "y_mm": 100.0})),
        step("canvas.release", json!({"x_mm": 100.0, "y_mm": 100.0})),
        step("preview.cancel", json!({})),
        step("navigator.open", json!({"kind": "macros", "key": core})),
        step("canvas.press", json!({"x_mm": 100.0, "y_mm": 100.0})),
        step("canvas.release", json!({"x_mm": 100.0, "y_mm": 100.0})),
        step("preview.apply", json!({})),
        step("edit.undo", json!({})),
    ];
    let steps = script::parse(&lines.join("\n")).unwrap();
    let mut out = Vec::new();
    let end = script::run(&mut shell, &mut NoExtension, &steps, false, &mut |l| {
        out.push(l.to_owned());
        Ok(())
    })
    .unwrap();
    assert_eq!(end.exit_code(), 0, "{out:#?}");
    let line = |n: usize| -> Value { serde_json::from_str(&out[n]).unwrap() };
    // Header, then one line per step: after the block's click (step 6) its card is pending with the rule's refusal
    // of its second sources; after the core macro's click (step 10) with no conflicts.
    let pending = line(6)["pending"].as_str().unwrap_or_default().to_owned();
    assert!(pending.contains("EW-CORE-052"), "{pending}");
    assert!(
        pending.contains("interruption point 24V: 2 source points")
            && pending.contains("interruption point 0V: 2 source points"),
        "{pending}"
    );
    let pending = line(10)["pending"].as_str().unwrap_or_default().to_owned();
    assert!(
        pending.contains("Insert macro") && pending.contains("No conflicts"),
        "{pending}"
    );
    let applied = line(11);
    assert_eq!(applied["outcome"], "done");
    assert_eq!(applied["state"]["can_undo"], true);
    let undone = line(12);
    assert_eq!(undone["outcome"], "done");
    shell.shutdown();
}
