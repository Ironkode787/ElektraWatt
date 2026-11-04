// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fixes of window check W49.2 in the form dialogs and the strip editor (cycle 49, ID-1278):
//!
//! - F1 (doc 16a §6.12): every form dialog the shell opens, listed from `ew-viewmodel`'s source, takes its first focus
//!   on its first text field, else on its Primary, which shows its focus ring; Enter on a fresh form submits it or is
//!   refused with its reason in the form, and never toggles a check box.
//! - F2 (ID-1278 ruling 7): the strip editor opens a strip without a tag yet by its ID, never by a filter built from
//!   its placeholder tag, and no text of the window holds that tag.

use crate::support;

use std::collections::BTreeSet;
use std::path::Path;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use ew_ui::ew_viewmodel::strings;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

const VIEWMODEL_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ew-viewmodel/src");
const RELEASE_MIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/fixtures/release-min");
/// The date the window passes to a revision action.
const DATE: &str = "2026-10-30";

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// Every `.rs` file under `dir`, recursively, with its text.
fn sources(dir: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(Result::unwrap).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let path = e.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(std::fs::read_to_string(&path).unwrap());
        }
    }
}

/// The string the `const NAME: &str = "…";` nearest above line `at` of `lines` gives `name` (a function's own
/// `const ACTION` shadows another function's), else one anywhere in the file.
fn const_value(lines: &[&str], at: usize, name: &str) -> Option<String> {
    let value = |l: &&str| -> Option<String> {
        let rest = l.split_once(&format!("const {name}: &str = \""))?.1;
        Some(rest.split_once('"')?.0.to_owned())
    };
    lines[..at]
        .iter()
        .rev()
        .find_map(value)
        .or_else(|| lines.iter().find_map(value))
}

/// The action an `action` expression at line `at` names: a string literal or a `const` of the same file.
fn action_of(lines: &[&str], at: usize, expr: &str) -> Option<String> {
    let expr = expr.trim().trim_end_matches([',', ')']).trim();
    if let Some(lit) = expr.strip_prefix('"') {
        return Some(lit.trim_end_matches('"').to_owned());
    }
    (!expr.is_empty() && expr.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .then(|| const_value(lines, at, expr))
        .flatten()
}

/// The actions of every form dialog `ew-viewmodel` opens, read from its source: each `FormDialog { action: … }`, each
/// creation `Spec { action: … }` (whose `dialog` builds the form) and each `open_revision_form(<action>, …)`.
pub(crate) fn form_actions() -> BTreeSet<String> {
    let mut texts = Vec::new();
    sources(Path::new(VIEWMODEL_SRC), &mut texts);
    let mut out = BTreeSet::new();
    for text in &texts {
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (i, line) in lines.iter().enumerate() {
            let builds =
                (line.ends_with("FormDialog {") && !line.contains("struct ") && !line.contains("fn "))
                    || line.ends_with("= Spec {");
            if builds {
                // `action` is the struct's first field; `action: self.action` and `action,` are the shared builders,
                // whose actions the specs and the callers give.
                if let Some(expr) = lines.get(i + 1).and_then(|l| l.strip_prefix("action: "))
                    && let Some(a) = action_of(&lines, i, expr)
                {
                    out.insert(a);
                }
            }
            if let Some(rest) = line.strip_prefix("self.open_revision_form(") {
                let expr = if rest.is_empty() { lines[i + 1] } else { rest };
                if let Some(a) = action_of(&lines, i, expr) {
                    out.insert(a);
                }
            }
        }
    }
    out
}

/// The window on a writable copy of Golden A at `folio`.
fn on_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    goto(&mut h, folio);
    h
}

fn goto(h: &mut Harness<'_, App>, folio: &str) {
    dispatch(h, "page.goto", json!({ "folio": folio }));
    settle(h);
    wait_for_page(h);
    settle(h);
}

/// A copy of the release fixture at `dir`.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().map(Result::unwrap) {
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// The window's shell on its own: opens `dir`, runs `actions`, shuts down.
fn person(tmp: &Tmp, dir: &Path, actions: &[(&str, Value)]) {
    let mut s = ShellModel::new(Mode::Gui, tmp.config());
    for (action, params) in
        std::iter::once(&("file.open", json!({ "dir": dir.to_string_lossy() }))).chain(actions)
    {
        assert_eq!(s.dispatch(action, params), Outcome::Done, "{action} {params}");
    }
    s.shutdown();
}

/// Opens the form of `action` with `params` and checks it (W49.2-F1): the first focus is the first text field, else
/// the Primary; Enter submits it or is refused with its reason in the form, and toggles no check box. Closes what it
/// left open (the form, a card). Returns the action, for the tally.
#[track_caller]
fn check_form(h: &mut Harness<'_, App>, action: &str, params: Value) -> String {
    dispatch(h, action, params);
    settle(h);
    let Some(Dialog::Form(form)) = h.state().shell().dialog().cloned() else {
        panic!("{action}: no form: {:?}", h.state().shell().dialog());
    };
    assert_eq!(form.action, action);
    let primary = ew_ui::ew_viewmodel::dialogs::spec(&Dialog::Form(form.clone()), h.state().shell())
        .primary
        .label;
    match form.fields.iter().find(|f| f.is_text()) {
        Some(first) => {
            let label = strings::text(first.label_key);
            assert!(h.ctx.text_edit_focused(), "{action}: a text field has the focus");
            let focused = h
                .get_all_by_role(egui::Role::TextInput)
                .find(|n| n.accesskit_node().is_focused())
                .unwrap_or_else(|| panic!("{action}: no focused text field"));
            assert_eq!(
                focused.accesskit_node().label().as_deref(),
                Some(label),
                "{action}: the first text field has the first focus"
            );
        }
        None => {
            let node = support::off_chrome(h, &primary).unwrap_or_else(|| panic!("{action}: {primary}"));
            assert!(
                node.accesskit_node().is_focused(),
                "{action}: the Primary {primary} has the first focus"
            );
        }
    }
    let toggles = |f: &ew_ui::ew_viewmodel::shell::FormDialog| -> Vec<String> {
        f.fields
            .iter()
            .filter(|x| x.is_toggle())
            .map(|x| x.value.clone())
            .collect()
    };
    h.key_press(Key::Enter);
    settle(h);
    if let Some(Dialog::Form(after)) = h.state().shell().dialog()
        && after.action == action
    {
        assert!(
            after.error.as_ref().is_some_and(|e| !e.is_empty()),
            "{action}: Enter neither submitted the form nor said why"
        );
        assert_eq!(toggles(after), toggles(&form), "{action}: no check box toggled");
        // The refusal's line above the footer leaves the focus where it was: the Primary pressed keeps it.
        if !form.fields.iter().any(|f| f.is_text()) {
            assert!(
                support::off_chrome(h, &primary).is_some_and(|n| n.accesskit_node().is_focused()),
                "{action}: the Primary {primary} keeps the focus after the refusal"
            );
        }
    }
    let shell = h.state_mut().shell_mut();
    while shell.dialog().is_some() {
        assert_eq!(shell.dispatch("ui.dismiss", &Value::Null), Outcome::Done);
    }
    if shell.properties().card().is_some() {
        let _ = shell.dispatch("preview.cancel", &json!({}));
    }
    settle(h);
    action.to_owned()
}

/// W49.2-F1: every form dialog of the shell, listed from the source, takes its first focus on its first text field,
/// else on its Primary (doc 16a §6.12), and Enter on the fresh form submits it or is refused with its reason (ID-1276
/// ruling 7), toggling no check box. Renumber devices… (no text field, two check boxes) opened on "Include frozen
/// tags" before: the first Enter toggled it, silently.
#[test]
fn w49_2_f1_every_form_dialog_focuses_its_first_text_field_or_its_primary() {
    let listed = form_actions();
    for known in ["devices.renumber", "page.new", "revision.approve", "macros.save"] {
        assert!(
            listed.contains(known),
            "the source scan finds {known}: {listed:?}"
        );
    }
    let mut checked = BTreeSet::new();
    let tmp = Tmp::new();

    // Golden A: the creation, renumber, macro and waiver forms, then the layout set-up forms on the panel page.
    let mut h = on_golden(&tmp, "102");
    // Save as macro… of what is selected: the wires of folio 102.
    let tables = &h.state().shell().canvas().expect("folio 102").page().tables;
    let ids: Vec<String> = tables.connections.iter().map(|c| c.id.to_string()).collect();
    assert!(!ids.is_empty(), "{} reps, {} wires", tables.reps.len(), ids.len());
    dispatch(&mut h, "select.set", json!({ "ids": ids }));
    checked.insert(check_form(&mut h, "macros.save", json!({})));
    dispatch(&mut h, "select.set", json!({ "ids": [] }));
    dispatch(&mut h, "messages.show", json!({ "on": true }));
    settle(&mut h);
    let finding = h
        .state()
        .shell()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .and_then(|t| t.nodes.iter().find(|n| n.role == NavRole::Finding).cloned())
        .expect("a finding of Golden A");
    dispatch(
        &mut h,
        "navigator.row",
        json!({ "kind": "messages", "key": finding.key }),
    );
    checked.insert(check_form(&mut h, "messages.waive", json!({})));
    dispatch(&mut h, "messages.show", json!({ "on": false }));
    for (action, params) in [
        ("page.new", json!({})),
        ("structure.new", json!({})),
        ("external_ref.new", json!({})),
        ("strip.new", json!({ "dialog": true })),
        ("pages.renumber", json!({})),
        ("devices.renumber", json!({})),
    ] {
        checked.insert(check_form(&mut h, action, params));
    }
    // FT-1371 and FT-1372: New cable… and Report block…; Connect cores… with a wire of the page selected.
    for action in ["cable.new", "report_block.new"] {
        checked.insert(check_form(&mut h, action, json!({})));
    }
    goto(&mut h, "102");
    let wire = h.state().shell().canvas().expect("folio 102").page().tables.connections[0]
        .id
        .to_string();
    dispatch(&mut h, "select.set", json!({ "ids": [wire] }));
    checked.insert(check_form(&mut h, "cable.connect_cores", json!({})));
    goto(&mut h, "902");
    for action in ["layout.enclosure_new", "layout.auto_fill_rail"] {
        checked.insert(check_form(&mut h, action, json!({})));
    }
    h.state_mut().shell_mut().shutdown();
    drop(h);

    // The release fixture (one open revision V1.0): Approve, Release (blocked), Set author; then, released by the
    // person at the window, Withdraw.
    let dir = tmp.0.join("release-min");
    copy_dir(Path::new(RELEASE_MIN), &dir);
    let mut h = harness(&tmp, Scripted::default(), Some(dir.clone()));
    settle(&mut h);
    for action in ["revision.approve", "revision.release", "revision.set_author"] {
        checked.insert(check_form(&mut h, action, json!({})));
    }
    dispatch(
        &mut h,
        "revision.approve",
        json!({ "role": "checker", "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    dispatch(
        &mut h,
        "revision.release",
        json!({ "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    settle(&mut h);
    checked.insert(check_form(&mut h, "revision.withdraw", json!({})));
    h.state_mut().shell_mut().shutdown();
    drop(h);

    // An incomplete release beside a fresh copy: the archive of a released copy, offered when the project opens.
    let fresh = tmp.0.join("fresh").join("release-min");
    let other = tmp.0.join("other").join("release-min");
    copy_dir(Path::new(RELEASE_MIN), &fresh);
    let approve = json!({ "role": "checker", "typed": "V1.0", "date": DATE, "confirm": true });
    person(&tmp, &fresh, &[("revision.approve", approve)]);
    copy_dir(&fresh, &other);
    let release = json!({ "typed": "V1.0", "date": DATE, "confirm": true });
    person(&tmp, &other, &[("revision.release", release)]);
    let archive = |d: &Path| {
        d.parent()
            .unwrap()
            .join("release-min.ew-archive")
            .join("V1.0.ewarchive")
    };
    std::fs::create_dir_all(archive(&fresh).parent().unwrap()).unwrap();
    std::fs::copy(archive(&other), archive(&fresh)).unwrap();
    let mut h = harness(&tmp, Scripted::default(), Some(fresh));
    settle(&mut h);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "revision.incomplete"),
        "the incomplete release is offered at open"
    );
    // Offered at open: the check opens it again through its action.
    h.state_mut().shell_mut().dispatch("ui.dismiss", &Value::Null);
    settle(&mut h);
    checked.insert(check_form(&mut h, "revision.incomplete", json!({})));
    h.state_mut().shell_mut().shutdown();

    // WP-S2.11 D3: this is the census of the creation forms (§5 U2).
    assert_eq!(
        crate::census::form_problems(&checked, &listed),
        Vec::<String>::new(),
        "every form dialog of the source is checked"
    );
}

/// Every AccessKit label and value of the window's last frame.
fn texts(h: &Harness<'_, App>) -> Vec<String> {
    h.root()
        .children_recursive()
        .flat_map(|n| [n.accesskit_node().label(), n.accesskit_node().value()])
        .flatten()
        .collect()
}

/// W49.2-F2 (ID-1278 ruling 7): Edit ▸ Strip editor on a new strip without a terminal opens the table on that strip
/// by its ID: no filter text, its own rows (none, so the empty state "No terminals yet"), while Golden A's other strips
/// have rows. No text of the window and no line of `ui.describe` holds the placeholder tag, in English or in Dutch; the
/// describe header names the strip in words. Before, the filter `strip:-?x?` showed with "Nothing matches".
#[test]
fn w49_2_f2_the_strip_editor_opens_a_strip_without_a_tag_by_its_id() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    dispatch(&mut h, "strip.new", json!({}));
    dispatch(&mut h, "navigator.show", json!({ "kind": "strips" }));
    settle(&mut h);
    let key = h
        .state()
        .shell()
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Strip && n.label.contains('?'))
                .map(|n| n.key.clone())
        })
        .expect("the new strip's row");
    dispatch(&mut h, "navigator.row", json!({ "kind": "strips", "key": key }));
    dispatch(&mut h, "strip.edit", json!({}));
    settle(&mut h);
    for (language, header, empty, chip) in [
        (
            "en",
            "navigator \"Terminal strips\" kind strips view tree table sort \"Order\" ascending filter \"\" strip \"New \
             terminal strip \u{b7} no tag yet\" rows 0 showing 0 to 0",
            "No terminals yet",
            "Strip: New terminal strip \u{b7} no tag yet",
        ),
        (
            "nl",
            "navigator \"Klemmenstroken\" kind strips view tree table sort \"Order\" ascending filter \"\" strip \
             \"Nieuwe klemmenstrook \u{b7} nog geen kenmerk\" rows 0 showing 0 to 0",
            "Nog geen klemmen",
            "Strook: Nieuwe klemmenstrook \u{b7} nog geen kenmerk",
        ),
    ] {
        dispatch(&mut h, "view.language", json!({ "value": language }));
        settle(&mut h);
        settle(&mut h);
        let shell = h.state().shell();
        let nav = shell.nav(NavKind::Strips).expect("the strip editor");
        assert_eq!(nav.filter, "", "{language}: no filter text");
        assert!(nav.visible().is_empty(), "{language}: the strip's own rows, none");
        let describe = shell.describe();
        let line = describe
            .lines()
            .find(|l| l.contains(" kind strips "))
            .expect("the strip editor's header");
        assert_eq!(line, header, "{language}");
        let placeholder = |t: &str| {
            let t = t.to_lowercase();
            t.contains("?x?") || t.contains("x??")
        };
        assert!(!placeholder(&describe), "{language}: {describe}");
        // C49-p8b#2: the scope shows in words in the filter row.
        assert!(
            h.query_all_by_label(chip).next().is_some(),
            "{language}: the filter row says {chip}"
        );
        assert!(
            h.query_all_by_label(empty).next().is_some(),
            "{language}: the empty state says {empty}"
        );
        let tagged: Vec<String> = texts(&h).into_iter().filter(|t| placeholder(t)).collect();
        assert!(tagged.is_empty(), "{language}: {tagged:?}");
    }
    // C49-p8b#2: Golden A's strips have rows, the editor showed the new strip alone. The scope's Clear ends it, and
    // so does Escape on the empty filter.
    let scoped = |h: &Harness<'_, App>| {
        let nav = h.state().shell().nav(NavKind::Strips).expect("the strip editor");
        (nav.scope_label().is_some(), nav.visible().is_empty())
    };
    dispatch(&mut h, "view.language", json!({ "value": "en" }));
    settle(&mut h);
    assert_eq!(scoped(&h), (true, true));
    support::off_chrome(&h, "Show every strip")
        .expect("the scope's Clear")
        .click();
    settle(&mut h);
    assert_eq!(scoped(&h), (false, false), "Clear: every strip's rows again");
    let chip = "Strip: New terminal strip \u{b7} no tag yet";
    assert!(h.query_all_by_label(chip).next().is_none());
    // Back to the tree, where the strip's row is, and the editor on it again.
    dispatch(
        &mut h,
        "navigator.table",
        json!({ "kind": "strips", "on": false }),
    );
    dispatch(&mut h, "navigator.row", json!({ "kind": "strips", "key": key }));
    dispatch(&mut h, "strip.edit", json!({}));
    dispatch(
        &mut h,
        "ui.focus",
        json!({ "area": "navigator", "kind": "strips" }),
    );
    settle(&mut h);
    assert_eq!(scoped(&h), (true, true));
    h.key_press(Key::Escape);
    settle(&mut h);
    assert_eq!(scoped(&h), (false, false), "Escape on the empty filter");
    h.state_mut().shell_mut().shutdown();
}

/// W49.2-F1: Enter on a check box a form's Tab reached neither submits the form nor toggles the box, and the box
/// keeps the focus; Space still toggles it.
#[test]
fn w49_2_f1_enter_on_a_forms_check_box_does_nothing() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    dispatch(&mut h, "devices.renumber", json!({}));
    settle(&mut h);
    let value = |h: &Harness<'_, App>| match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => {
            assert_eq!(f.action, "devices.renumber");
            assert_eq!(f.error, None, "nothing submitted");
            f.fields
                .iter()
                .find(|x| x.name == "include_frozen")
                .map(|x| x.value.clone())
                .expect("Include frozen tags")
        }
        other => panic!("the form stays open: {other:?}"),
    };
    let label = strings::text(ew_ui::ew_viewmodel::key!("form-include-frozen"));
    let focused = |h: &Harness<'_, App>| {
        h.get_all_by_role(egui::Role::CheckBox)
            .any(|n| n.accesskit_node().is_focused() && n.accesskit_node().label().as_deref() == Some(label))
    };
    let mut tabs = 0;
    while !focused(&h) {
        tabs += 1;
        assert!(tabs <= 12, "Tab reaches Include frozen tags");
        h.key_press(Key::Tab);
        settle(&mut h);
    }
    let before = value(&h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(value(&h), before, "Enter toggles nothing");
    assert!(focused(&h), "the check box keeps the focus");
    h.key_press(Key::Space);
    settle(&mut h);
    assert_ne!(value(&h), before, "Space toggles it");
    h.state_mut().shell_mut().shutdown();
}
