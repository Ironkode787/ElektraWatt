// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 6 (§11), the value-set items: FT-785 (the macro card's ad-hoc values: a typed value is a literal of
//! `macro.insert` that wins over the value set and the default; a value that does not coerce shows the command's
//! refusal under its field) and FT-786 (Save as macro…'s value-set table: add, rename, remove, save; an invalid cell
//! is refused naming the set and the variable), in the shell on copies of Golden A with a fixture user library.

mod common;

use std::path::Path;

use common::{Tmp, done};
use ew_viewmodel::ew_api::library::{LibraryTiers, Registry};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::ew_api::{MacroScope, MacroSpec, Recovery, Session, SessionOptions, StoreOptions};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::properties::{CardOrigin, MacroInsert};
use ew_viewmodel::shell::{Dialog, FormDialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

/// The tiers of a fixture under `root`: no system path (the compiled-in libraries), a user directory.
fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// A fresh user library "Mine" under `root`: the tiers and its ID.
fn user_library(root: &Path) -> (LibraryTiers, Uuid) {
    let t = tiers(root);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    (t, lib)
}

/// A window shell with the tiers `t` on a writable copy `copy` of Golden A, on `folio`.
fn shell_with(tmp: &Tmp, copy: &str, t: &LibraryTiers, folio: &str) -> ShellModel {
    let dir = tmp.0.join(copy);
    if !dir.exists() {
        common::copy_dir(Path::new(common::GOLDEN_A), &dir);
    }
    let config = ShellConfig::in_dir(&tmp.0.join(format!("config-{copy}")), "0.0.0-test")
        .with_libraries(Some(t.clone()));
    let mut shell = ShellModel::new(Mode::Gui, config);
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({ "folio": folio }));
    shell
}

/// Seeds "Mine" with the macro "PSU values" (folio 102's supply and breakers with their wires) through the session:
/// the variable `pt` (the supply's process tag, default `PSU-D`), the variable `n` (the supply's tag counter, no
/// default) and the value set `a` (`pt` = `PSU-A`). The shell's Save as macro… gives no default, so the session
/// writes it.
fn seed_macro(tmp: &Tmp, t: &LibraryTiers, lib: Uuid) {
    let dir = tmp.0.join("seed");
    common::copy_dir(Path::new(common::GOLDEN_A), &dir);
    let options = SessionOptions {
        libraries: Some(t.clone()),
        ..SessionOptions::default()
    };
    let (mut s, _) = Session::open(&dir, StoreOptions::new("0.0.0-test"), options, Recovery::Keep).unwrap();
    let tables = s.page_tables("102").expect("folio 102");
    let scope: Vec<Uuid> = tables
        .reps
        .iter()
        .filter(|r| r.device.is_some())
        .map(|r| r.id)
        .chain(tables.connections.iter().map(|c| c.id))
        .chain(tables.junctions.iter().map(|j| j.id))
        .collect();
    let g1 = s.resolve_reference("tag:-102G1").unwrap().to_string();
    let spec = MacroSpec {
        key: "psu-values".into(),
        name: serde_json::from_value(json!("PSU values")).unwrap(),
        description: serde_json::from_value(json!("")).unwrap(),
        scope: MacroScope::Window(scope),
        origin: None,
        keep_tags: Vec::new(),
        variables: serde_json::from_value(json!([
            {"name": "pt", "ty": "process_tag", "default": "PSU-D",
             "bindings": [{"record": g1, "field": "device.process_tag"}]},
            {"name": "n", "ty": "tag_counter", "bindings": [{"record": g1, "field": "device.tag.counter"}]}
        ]))
        .unwrap(),
        groups: Vec::new(),
        value_sets: serde_json::from_value(json!([{"name": "a", "values": {"pt": "PSU-A"}}])).unwrap(),
    };
    s.save_macro(lib, &spec).unwrap();
}

/// A shell on a new folio 105 with the Macros navigator open: the row key of "PSU values".
fn on_folio_105(shell: &mut ShellModel) -> String {
    done(shell, "page.goto", json!({"folio": "102"}));
    done(
        shell,
        "page.new",
        json!({"folio": "105", "type": "multi_line", "title": "Power Supply 5"}),
    );
    done(shell, "page.goto", json!({"folio": "105"}));
    done(shell, "navigator.show", json!({"kind": "macros"}));
    let t = shell.nav(NavKind::Macros).and_then(|n| n.table()).unwrap();
    t.nodes
        .iter()
        .find(|n| n.role == NavRole::Macro && n.label == "PSU values")
        .map(|n| n.key.clone())
        .expect("the PSU values row")
}

/// The open macro card.
fn card(shell: &ShellModel) -> MacroInsert {
    match &shell.properties().card().expect("the macro card").origin {
        CardOrigin::Macro(m) => (**m).clone(),
        other => panic!("a macro card: {other:?}"),
    }
}

/// The process tag property of the device `tag`, as Properties shows it.
fn process_tag(shell: &mut ShellModel, tag: &str) -> String {
    done(shell, "select.set", json!({"refs": [format!("tag:{tag}")]}));
    done(shell, "props.show", json!({"on": true}));
    let sheet = shell.properties().sheet().expect("a property sheet");
    let (_, row) =
        ew_viewmodel::properties::find_row(sheet, "device/process_tag").expect("the process tag row");
    ew_viewmodel::properties::value_text(row)
}

/// Inserts "PSU values" at 100/100 on folio 105 with the value set `set` and the typed values `typed`, applies it and
/// returns the inserted supply's process tag; the insertion is undone again.
fn insert_and_read(shell: &mut ShellModel, key: &str, set: Option<&str>, typed: &[(&str, &str)]) -> String {
    done(
        shell,
        "macros.place",
        json!({"key": key, "x_mm": 100.0, "y_mm": 100.0}),
    );
    if let Some(set) = set {
        done(shell, "macros.value_set", json!({ "name": set }));
    }
    for (name, value) in typed {
        done(shell, "macros.value", json!({"name": name, "value": value}));
    }
    let reps = |shell: &ShellModel| shell.canvas().map_or(0, |c| c.page().tables.reps.len());
    let (rev, before) = (shell.revision().unwrap(), reps(shell));
    done(shell, "preview.apply", json!({}));
    assert_eq!(shell.revision(), Some(rev + 1), "one transaction");
    let tag = process_tag(shell, "-105G1");
    done(shell, "edit.undo", json!({}));
    assert_eq!(reps(shell), before, "one undo step removes the macro");
    tag
}

/// FT-785: the card has one field per variable typed by it, its placeholder the default (`PSU-D`) or the chosen set's
/// value (`PSU-A`); a typed value is passed in `values` and survives a change of value set; inserted, the literal wins
/// over the value set, the value set over the default, and the default applies without either.
#[test]
fn ft785_a_typed_value_wins_over_the_value_set_and_the_default() {
    let tmp = Tmp::new("macro-values-785");
    let (t, lib) = user_library(&tmp.0);
    seed_macro(&tmp, &t, lib);
    let mut shell = shell_with(&tmp, "a", &t, "102");
    let key = on_folio_105(&mut shell);

    done(
        &mut shell,
        "macros.place",
        json!({"key": key, "x_mm": 100.0, "y_mm": 100.0}),
    );
    let m = card(&shell);
    let pt = m.fields.iter().find(|f| f.name == "pt").expect("the pt field");
    assert_eq!(
        (pt.ty.as_str(), pt.placeholder.as_str(), pt.value.as_str()),
        ("process_tag", "PSU-D", "")
    );
    let n = m.fields.iter().find(|f| f.name == "n").expect("the n field");
    assert_eq!((n.ty.as_str(), n.placeholder.as_str()), ("tag_counter", ""));
    done(&mut shell, "macros.value_set", json!({"name": "a"}));
    assert_eq!(card(&shell).fields[0].placeholder, "PSU-A");
    done(
        &mut shell,
        "macros.value",
        json!({"name": "pt", "value": "PSU-X"}),
    );
    let m = card(&shell);
    assert_eq!(
        (m.fields[0].value.as_str(), m.fields[0].error.as_ref()),
        ("PSU-X", None)
    );
    let c = shell.properties().card().unwrap();
    assert!(c.preview.result.is_ok(), "{:?}", c.preview.result.as_ref().err());
    assert_eq!(c.preview.command["values"]["pt"], "PSU-X");
    assert_eq!(c.preview.command["value_set"], "a");
    // The typed value stays across a change of value set; a blank value clears it.
    done(&mut shell, "macros.value_set", json!({}));
    assert_eq!(card(&shell).fields[0].value, "PSU-X");
    done(&mut shell, "macros.value", json!({"name": "pt", "value": " "}));
    assert_eq!(card(&shell).fields[0].value, "");
    // An unknown variable is refused.
    assert!(matches!(
        shell.dispatch("macros.value", &json!({"name": "zz", "value": "1"})),
        Outcome::Failed { .. }
    ));
    done(&mut shell, "preview.cancel", json!({}));

    // Inserted: the literal over the value set over the default.
    assert!(insert_and_read(&mut shell, &key, Some("a"), &[("pt", "PSU-X")]).contains("PSU-X"));
    assert!(insert_and_read(&mut shell, &key, Some("a"), &[]).contains("PSU-A"));
    assert!(insert_and_read(&mut shell, &key, None, &[]).contains("PSU-D"));
}

/// FT-785: a typed value that does not coerce (a tag counter of letters) shows the command's refusal under its field
/// in words, without the variable's name; Apply fails with it and the card stays; clearing the value previews again.
#[test]
fn ft785_a_value_that_does_not_coerce_is_refused_under_its_field() {
    let tmp = Tmp::new("macro-values-785b");
    let (t, lib) = user_library(&tmp.0);
    seed_macro(&tmp, &t, lib);
    let mut shell = shell_with(&tmp, "a", &t, "102");
    let key = on_folio_105(&mut shell);
    done(
        &mut shell,
        "macros.place",
        json!({"key": key, "x_mm": 100.0, "y_mm": 100.0}),
    );
    done(&mut shell, "macros.value", json!({"name": "n", "value": "abc"}));
    let m = card(&shell);
    let n = m.fields.iter().find(|f| f.name == "n").unwrap();
    let error = n.error.as_deref().expect("the refusal under the field");
    assert!(error.contains("\"abc\" is not a tag counter"), "{error}");
    assert!(!error.contains("variable `n`"), "{error}");
    assert!(m.fields.iter().find(|f| f.name == "pt").unwrap().error.is_none());
    assert!(shell.properties().card().unwrap().preview.result.is_err());
    // The card leads in words naming the variable, not with the command's pointer.
    let lead = shell
        .properties()
        .card()
        .unwrap()
        .failed_lead()
        .expect("the lead");
    assert!(
        lead.text.contains(" n") && !lead.text.contains("/values"),
        "{lead:?}"
    );
    assert!(matches!(
        shell.dispatch("preview.apply", &json!({})),
        Outcome::Failed { .. }
    ));
    assert!(shell.properties().card().is_some(), "the refused card stays");
    // A tag counter typed as digits is taken.
    done(&mut shell, "macros.value", json!({"name": "n", "value": "7"}));
    let c = shell.properties().card().unwrap();
    assert!(c.preview.result.is_ok(), "{:?}", c.preview.result.as_ref().err());
    assert_eq!(c.preview.command["values"]["n"], 7);
    assert!(card(&shell).fields.iter().all(|f| f.error.is_none()));
}

/// The Save as macro… dialog, open.
fn save_form(shell: &ShellModel) -> &FormDialog {
    match shell.dialog() {
        Some(Dialog::Form(f)) if f.action == "macros.save" => f,
        other => panic!("the Save as macro dialog: {other:?}"),
    }
}

/// Folio 102's supply and breakers with their wires selected, after Make variable on -102G1's row `row` as `name`.
fn select_with_variable(shell: &mut ShellModel, row: &str, name: &str) {
    done(shell, "select.set", json!({"refs": ["tag:-102G1"]}));
    done(shell, "props.show", json!({"on": true}));
    done(shell, "props.make_variable", json!({"row": row, "name": name}));
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

/// The detail of the macro `name` of the Macros navigator.
fn detail(shell: &mut ShellModel, name: &str) -> ew_viewmodel::ew_api::output::MacroDetail {
    done(shell, "navigator.show", json!({"kind": "macros"}));
    let key = {
        let t = shell.nav(NavKind::Macros).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::Macro && n.label == name)
            .map(|n| n.key.clone())
            .unwrap_or_else(|| panic!("no macro row {name}"))
    };
    done(shell, "navigator.row", json!({"kind": "macros", "key": key}));
    shell.macro_detail().expect("the detail").clone()
}

/// FT-786: the dialog's table has one column per variable (Make variable's) and starts empty; Add names the rows
/// `Set 1`, `Set 2`, `Set 3`; a rename, a cell edit and a remove change the rows; Save writes the table's sets into the
/// macro (its value sets equal the table). `value_sets` given to `macros.save` prefills the table.
#[test]
fn ft786_the_value_set_table_adds_renames_removes_and_saves() {
    let tmp = Tmp::new("macro-values-786");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", &t, "102");
    select_with_variable(&mut shell, "device/process_tag", "pt");
    done(&mut shell, "macros.save", json!({}));
    let table = save_form(&shell).table.clone().expect("the value-set table");
    assert_eq!(table.columns, vec![("pt".to_owned(), "process_tag".to_owned())]);
    assert!(table.rows.is_empty());
    for _ in 0..3 {
        shell.add_form_table_row();
    }
    let keys: Vec<String> = save_form(&shell)
        .table
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(keys, ["Set 1", "Set 2", "Set 3"]);
    shell.set_form_table_cell(0, Some(0), "PSU-1");
    shell.set_form_table_cell(1, Some(0), "PSU-2");
    shell.set_form_table_cell(2, Some(0), "PSU-3");
    shell.set_form_table_cell(1, None, "spare");
    shell.remove_form_table_row(2);
    // A row or column the table does not have changes nothing.
    shell.set_form_table_cell(9, Some(0), "x");
    shell.set_form_table_cell(0, Some(9), "x");
    shell.remove_form_table_row(9);
    let rows = &save_form(&shell).table.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[1].key.as_str(), rows[1].cells[0].as_str()),
        ("spare", "PSU-2")
    );
    shell.set_form_field("name", "PSU sets");
    assert_eq!(shell.submit_form(), Outcome::Done);
    assert!(shell.dialog().is_none(), "a save closes the dialog");
    let d = detail(&mut shell, "PSU sets");
    assert_eq!(d.value_sets, ["Set 1", "spare"]);
    assert_eq!(
        d.value_set_values,
        vec![
            [("pt".to_owned(), json!("PSU-1"))].into(),
            [("pt".to_owned(), json!("PSU-2"))].into()
        ]
    );
    assert_eq!(d.variable_fields[0].0, "pt");

    // `value_sets` prefills the table.
    select_with_variable(&mut shell, "device/process_tag", "pt");
    done(
        &mut shell,
        "macros.save",
        json!({"value_sets": [{"name": "x", "values": {"pt": "PSU-9"}}, {"name": "y"}]}),
    );
    let rows = &save_form(&shell).table.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].key.as_str(), rows[0].cells[0].as_str()), ("x", "PSU-9"));
    assert_eq!((rows[1].key.as_str(), rows[1].cells[0].as_str()), ("y", ""));
}

/// FT-786: Save with a cell that does not coerce is refused in words naming the set and the variable, and a set
/// without a name by its row number; the dialog stays with the refusal and nothing is written.
#[test]
fn ft786_an_invalid_cell_is_refused_naming_the_set_and_the_variable() {
    let tmp = Tmp::new("macro-values-786b");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", &t, "102");
    select_with_variable(&mut shell, "device/tag.counter", "n");
    done(&mut shell, "macros.save", json!({}));
    assert_eq!(
        save_form(&shell).table.as_ref().unwrap().columns,
        vec![("n".to_owned(), "tag_counter".to_owned())]
    );
    shell.add_form_table_row();
    shell.set_form_table_cell(0, Some(0), "abc");
    shell.set_form_field("name", "PSU bad");
    let Outcome::Failed { message } = shell.submit_form() else {
        panic!("the save is refused")
    };
    assert!(message.contains("\u{201c}Set 1\u{201d}"), "{message}");
    assert!(message.contains("\u{201c}n\u{201d}"), "{message}");
    assert!(message.contains("\"abc\" is not a tag counter"), "{message}");
    assert_eq!(save_form(&shell).error.as_deref(), Some(message.as_str()));
    // A set without a name.
    shell.set_form_table_cell(0, Some(0), "3");
    shell.set_form_table_cell(0, None, " ");
    let Outcome::Failed { message } = shell.submit_form() else {
        panic!("the save is refused")
    };
    assert_eq!(message, "Value set 1 needs a name.");
    // Nothing was written; a valid cell saves.
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));
    let t = shell.nav(NavKind::Macros).and_then(|n| n.table()).unwrap();
    assert!(!t.nodes.iter().any(|n| n.label == "PSU bad"));
}
