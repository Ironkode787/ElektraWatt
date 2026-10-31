// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 6 (§11) in the window, the value-set items: FT-785 (the macro card's ad-hoc value fields, labelled
//! by variable and type, with the command's refusal of a typed value under its field) and FT-786 (Save as macro…'s
//! value-set table: a header, one named field per cell, a remove button per row and "Add value set").

mod support;

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::library::{LibraryTiers, Registry};
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, settle};

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

#[track_caller]
fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// A window shell with the tiers `t` on the copy `name` of Golden A, showing folio 102.
fn shell_on(tmp: &Tmp, name: &str, t: &LibraryTiers) -> ShellModel {
    let dir = tmp.golden(name);
    let config = ShellConfig::in_dir(&tmp.0.join(format!("config-{name}")), "0.0.0-test")
        .with_libraries(Some(t.clone()));
    let mut shell = ShellModel::new(Mode::Gui, config);
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    shell
}

/// Make variable on -102G1's process tag (`pt`) and tag counter (`n`), then folio 102's supply and breakers with
/// their wires selected.
fn variables_and_selection(shell: &mut ShellModel) {
    done(shell, "page.goto", json!({"folio": "102"}));
    done(shell, "select.set", json!({"refs": ["tag:-102G1"]}));
    done(shell, "props.show", json!({"on": true}));
    for (row, name) in [("device/process_tag", "pt"), ("device/tag.counter", "n")] {
        done(shell, "props.make_variable", json!({"row": row, "name": name}));
    }
    let tables = &shell.canvas().expect("folio 102").page().tables;
    let ids: Vec<String> = tables
        .reps
        .iter()
        .filter(|r| r.device.is_some())
        .map(|r| r.id)
        .chain(tables.connections.iter().map(|c| c.id))
        .chain(tables.junctions.iter().map(|j| j.id))
        .map(|id| id.to_string())
        .collect();
    done(shell, "select.set", json!({ "ids": ids }));
}

/// The three value sets of the fixture's table.
fn three_sets() -> Value {
    json!([
        {"name": "Small", "values": {"pt": "PSU-S", "n": "1"}},
        {"name": "Medium", "values": {"pt": "PSU-M", "n": "2"}},
        {"name": "Large", "values": {"pt": "PSU-L"}}
    ])
}

/// Whether a node's name or value contains `text`.
fn has_text_part(h: &Harness<'_, App>, text: &str) -> bool {
    h.root().children_recursive().any(|n| {
        let n = n.accesskit_node();
        n.label().is_some_and(|l| l.contains(text)) || n.value().is_some_and(|v| v.contains(text))
    })
}

/// A text field named `name` showing `value`.
fn field(h: &Harness<'_, App>, name: &str, value: &str) -> bool {
    h.get_all_by_role(egui::Role::TextInput).any(|n| {
        let n = n.accesskit_node();
        n.label().as_deref() == Some(name) && n.value().as_deref().unwrap_or("") == value
    })
}

/// FT-786 in the window: the Save as macro… dialog shows the value-set table (the header, one field per cell named
/// by column and row, a remove button per row) and Add value set adds a row; FT-785: the saved macro's card shows a
/// field per variable labelled "<name> (<type>)", and a refused typed value shows the reason under it.
#[test]
fn ft785_ft786_the_card_and_the_table_render_their_fields() {
    let tmp = Tmp::new();
    let t = tiers(&tmp.0);
    Registry::open(&t)
        .unwrap()
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    let mut shell = shell_on(&tmp, "g.ew", &t);
    variables_and_selection(&mut shell);
    done(&mut shell, "macros.save", json!({ "value_sets": three_sets() }));
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);

    // The table.
    assert!(has_text_part(&h, "Value sets"));
    assert!(
        field(&h, "Value set, value set 1", "Small"),
        "the first column's field"
    );
    assert!(field(&h, "pt, value set 2", "PSU-M"), "a cell");
    assert!(field(&h, "n, value set 3", ""), "a blank cell");
    assert!(h.query_by_label("Remove value set Large").is_some());
    h.get_by_label("Add value set").click();
    settle(&mut h);
    let rows = match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f.table.as_ref().map_or(0, |t| t.rows.len()),
        _ => 0,
    };
    assert_eq!(rows, 4, "Add value set adds a row");
    h.get_by_label("Remove value set Set 4").click();
    settle(&mut h);

    // Save it, then insert it on folio 105.
    let shell = h.state_mut().shell_mut();
    shell.set_form_field("name", "PSU sizes");
    shell.set_form_field("library", "Mine");
    assert_eq!(shell.submit_form(), Outcome::Done);
    done(
        shell,
        "page.new",
        json!({"folio": "105", "type": "multi_line", "title": "Power Supply 5"}),
    );
    done(shell, "page.goto", json!({"folio": "105"}));
    done(shell, "navigator.show", json!({"kind": "macros"}));
    let key = {
        let t = shell.nav(NavKind::Macros).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::Macro && n.label == "PSU sizes")
            .map(|n| n.key.clone())
            .unwrap()
    };
    done(
        shell,
        "macros.place",
        json!({"key": key, "x_mm": 100.0, "y_mm": 100.0}),
    );
    done(shell, "macros.value_set", json!({"name": "Medium"}));
    settle(&mut h);
    assert!(has_text_part(&h, "pt (process_tag)"), "the pt field's label");
    assert!(has_text_part(&h, "n (tag_counter)"), "the n field's label");
    assert!(field(&h, "pt (process_tag)", ""), "an untyped field");
    done(
        h.state_mut().shell_mut(),
        "macros.value",
        json!({"name": "n", "value": "abc"}),
    );
    settle(&mut h);
    assert!(field(&h, "n (tag_counter)", "abc"), "the typed value");
    assert!(
        has_text_part(&h, "is not a tag counter"),
        "the refusal under the field"
    );
    h.state_mut().shell_mut().shutdown();
}
