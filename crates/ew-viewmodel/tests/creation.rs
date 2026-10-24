// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-408 and FT-409: `page.new`, `structure.new` and `external_ref.new` at the view-model level, on a new project (the
//! builtin template) in both modes: the pages 101 to 103 and the node and reference of folio 101's external points, the
//! refusals, the creation dialog of the window, the menu entries and the vocabulary of the dialog's describe lines.

mod common;

use common::{Tmp, at, done, refused};
use ew_viewmodel::actions::{MenuEntry, menu_model};
use ew_viewmodel::describe::{FREE_TEXT_MARKERS, VOCABULARY, describe};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::tools::PreviewStatus;
use serde_json::{Value, json};

fn shell_in(tmp: &Tmp, mode: Mode) -> ShellModel {
    ShellModel::new(mode, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"))
}

/// A shell in `mode` with a new project (`new.ew` of `tmp`, the builtin template) open.
fn new_project(tmp: &Tmp, mode: Mode) -> ShellModel {
    let mut shell = shell_in(tmp, mode);
    done(
        &mut shell,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    shell
}

fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

#[test]
fn page_new_creates_folios_101_to_103_in_a_new_project() {
    let tmp = Tmp::new("pagenew");
    let mut shell = new_project(&tmp, Mode::Headless);
    let before = shell.page_labels().len();
    done(
        &mut shell,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power Supply 1 - Main Current"}),
    );
    // A number is a folio too, as a script written by hand may give it.
    done(&mut shell, "page.new", json!({"folio": 102}));
    done(
        &mut shell,
        "page.new",
        json!({"folio": "103", "type": "single_line", "title": null}),
    );
    let labels = shell.page_labels().to_vec();
    assert_eq!(labels.len(), before + 3, "{labels:?}");
    for folio in ["101", "102", "103"] {
        assert!(labels.iter().any(|l| l == folio), "{folio} in {labels:?}");
    }
    let rows = shell.page_rows().to_vec();
    let kind = |folio: &str| rows.iter().find(|r| r.folio == folio).unwrap().kind.clone();
    assert_eq!(kind("101"), "multi_line");
    assert_eq!(kind("102"), "multi_line", "the default type");
    assert_eq!(kind("103"), "single_line");
    assert_eq!(
        rows.iter().find(|r| r.folio == "101").unwrap().title,
        "Power Supply 1 - Main Current"
    );
    // The new page is shown; three creations are three undo steps.
    assert_eq!(shell.current_folio(), Some("103"));
    assert_eq!(shell.recorded_commands().len(), 3);
    assert!(shell.recorded_commands().iter().all(|c| c.cmd == "page.create"));
    for _ in 0..3 {
        done(&mut shell, "edit.undo", json!({}));
    }
    assert_eq!(shell.page_labels().len(), before);
    shell.shutdown();
}

#[test]
fn a_label_a_section_and_the_refusals_of_page_new() {
    let tmp = Tmp::new("pagenew-refused");
    let mut shell = new_project(&tmp, Mode::Headless);
    done(&mut shell, "page.new", json!({"folio": "101"}));
    // `101a` is the number 101 with the label 101a.
    done(&mut shell, "page.new", json!({"folio": "101a"}));
    assert!(shell.page_labels().iter().any(|l| l == "101a"));
    // A folio of the section's own range is not needed here: an unknown section is refused by its number.
    let m = failed(&mut shell, "page.new", json!({"folio": "104", "section": "9"}));
    assert!(m.contains("no section 9"), "{m}");
    let m = failed(&mut shell, "page.new", json!({"folio": "101"}));
    assert!(m.contains("already exists"), "{m}");
    let m = failed(&mut shell, "page.new", json!({"folio": "A1"}));
    assert_eq!(m, "A folio starts with its number: \"A1\" does not.");
    let m = failed(&mut shell, "page.new", json!({"folio": "105", "type": "report"}));
    assert!(
        m.starts_with("\"report\" is not a page type; use one of: title_page"),
        "{m}"
    );
    assert_eq!(
        failed(&mut shell, "page.new", json!({})),
        "Missing parameter `folio`."
    );
    assert_eq!(
        failed(&mut shell, "page.new", json!({"folio": 105, "title": true})),
        "Parameter `title` is not valid."
    );
    // Nothing of the refusals was committed.
    assert_eq!(shell.recorded_commands().len(), 2);
    shell.shutdown();
}

#[test]
fn the_new_actions_are_refused_without_a_project_and_read_only() {
    let tmp = Tmp::new("create-refused");
    let mut shell = shell_in(&tmp, Mode::Headless);
    for (action, params) in [
        ("page.new", json!({"folio": "101"})),
        (
            "structure.new",
            json!({"kind": "location", "designation": "J-E-206"}),
        ),
        (
            "external_ref.new",
            json!({"location": "J-E-206", "device": "JE-N1000-03"}),
        ),
    ] {
        assert_eq!(
            refused(&mut shell, action, params.clone()),
            "reason-no-project",
            "{action}"
        );
    }
    let tmp = Tmp::new("create-ro");
    let mut shell = tmp.shell("101");
    for (action, params) in [
        ("page.new", json!({"folio": "901"})),
        (
            "structure.new",
            json!({"kind": "location", "designation": "J-E-206"}),
        ),
        ("external_ref.new", json!({"location": "J-E-206", "device": "x"})),
    ] {
        assert_eq!(
            refused(&mut shell, action, params),
            "reason-read-only",
            "{action}"
        );
    }
}

/// The nodes and external references of the project `new.ew` of `tmp` as saved: (aspect, code, parent code) and
/// (location code, device, connection, project).
#[allow(clippy::type_complexity)]
fn saved_structure(
    tmp: &Tmp,
) -> (
    Vec<(String, String, Option<String>)>,
    Vec<(String, String, Option<String>, Option<String>)>,
) {
    let (session, _) = Session::open_read_only(&tmp.0.join("new.ew"), "0.0.0-test").unwrap();
    let q = session.query();
    let nodes = q.structure_nodes();
    let code_of = |id| nodes.get(id).map(|n| n.code.as_str().to_owned());
    let mut a: Vec<_> = nodes
        .iter()
        .map(|(_, n)| {
            (
                serde_json::to_value(&n.aspect)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
                n.code.as_str().to_owned(),
                n.parent.and_then(code_of),
            )
        })
        .collect();
    a.sort();
    let mut b: Vec<_> = q
        .external_refs()
        .iter()
        .map(|(_, e)| {
            (
                code_of(e.location).unwrap(),
                e.device.as_str().to_owned(),
                e.connection.as_ref().map(|c| c.as_str().to_owned()),
                e.project.as_ref().map(|c| c.as_str().to_owned()),
            )
        })
        .collect();
    b.sort();
    (a, b)
}

#[test]
fn a_structure_node_and_an_external_reference_are_created_by_their_designations() {
    let tmp = Tmp::new("structure");
    let mut shell = new_project(&tmp, Mode::Headless);
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "J-E-2416"}),
    );
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "+J-E-206", "parent": "+J-E-2416"}),
    );
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "function", "designation": "=P01"}),
    );
    done(
        &mut shell,
        "external_ref.new",
        json!({"location": "+J-E-206", "device": "JE-N1000-03", "connection": "L"}),
    );
    done(&mut shell, "file.save", json!({}));
    let (nodes, refs) = saved_structure(&tmp);
    let found = |aspect: &str, code: &str| nodes.iter().any(|n| n.0 == aspect && n.1 == code);
    assert!(
        found("location", "J-E-206") && found("function", "P01"),
        "{nodes:?}"
    );
    assert!(
        nodes.contains(&("location".into(), "J-E-206".into(), Some("J-E-2416".into()))),
        "{nodes:?}"
    );
    assert_eq!(
        refs,
        [("J-E-206".into(), "JE-N1000-03".into(), Some("L".into()), None)]
    );
    // One undo step each; the node list is not touched by a refusal.
    assert_eq!(shell.recorded_commands().len(), 4);
    shell.shutdown();
}

#[test]
fn structure_designations_are_refused_when_unknown_wrong_or_ambiguous() {
    let tmp = Tmp::new("structure-refused");
    let mut shell = new_project(&tmp, Mode::Headless);
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "A"}),
    );
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "B"}),
    );
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "X", "parent": "A"}),
    );
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "X", "parent": "B"}),
    );
    let m = failed(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "Y", "parent": "X"}),
    );
    assert_eq!(
        m,
        "2 nodes share the designation \"X\": @1 (under A), @2 (under B).          Give the alias of one of them instead."
    );
    // The aliases the refusal printed are the nodes: the next step names one as the parent.
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "Y", "parent": "@2"}),
    );
    // Another aspect's sign is refused naming both, in the designation, the parent and the location.
    let wrong = "\"=P01\" starts with =, the sign of the function aspect, but this node is in the location aspect.";
    for (action, params) in [
        ("structure.new", json!({"kind": "location", "designation": "=P01"})),
        (
            "structure.new",
            json!({"kind": "location", "designation": "Z", "parent": "=P01"}),
        ),
        ("external_ref.new", json!({"location": "=P01", "device": "d"})),
    ] {
        assert_eq!(failed(&mut shell, action, params), wrong, "{action}");
    }
    let m = failed(
        &mut shell,
        "structure.new",
        json!({"kind": "function", "designation": "==P01"}),
    );
    assert_eq!(
        m,
        "\"==P01\" starts with ==, the sign of the functional assignment aspect, but this node is in the function aspect."
    );
    assert_eq!(
        failed(&mut shell, "structure.new", json!({"kind": "location", "designation": "+"})),
        "Missing parameter `designation`."
    );
    let m = failed(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "Y", "parent": "Q"}),
    );
    assert_eq!(m, "The project has no location node \"Q\".");
    // A node of another aspect is no parent, and a location is the only place of an external reference.
    done(
        &mut shell,
        "structure.new",
        json!({"kind": "function", "designation": "P01"}),
    );
    let m = failed(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "Y", "parent": "P01"}),
    );
    assert_eq!(m, "The project has no location node \"P01\".");
    let m = failed(
        &mut shell,
        "external_ref.new",
        json!({"location": "P01", "device": "d"}),
    );
    assert_eq!(m, "The project has no location node \"P01\".");
    let m = failed(
        &mut shell,
        "structure.new",
        json!({"kind": "product", "designation": "Y"}),
    );
    assert!(
        m.starts_with("\"product\" is not a structure kind; use one of: functional_assignment"),
        "{m}"
    );
    // The command's own refusals reach the step: a repeated sibling code.
    let m = failed(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "A"}),
    );
    assert!(m.contains("already exists"), "{m}");
    let m = failed(
        &mut shell,
        "structure.new",
        json!({"kind": "location", "designation": "Z", "parent": "@7"}),
    );
    assert!(
        m.contains("@7"),
        "an alias the run never printed is refused naming it: {m}"
    );
    assert_eq!(
        failed(&mut shell, "external_ref.new", json!({"location": "A"})),
        "Missing parameter `device`."
    );
    shell.shutdown();
}

#[test]
fn a_page_change_ends_a_modal_tool() {
    let tmp = Tmp::new("tool-ends");
    let mut shell = new_project(&tmp, Mode::Headless);
    done(&mut shell, "page.new", json!({"folio": "101"}));
    done(&mut shell, "tool.wire", json!({}));
    assert_eq!(shell.tool().name(), "wire");
    done(&mut shell, "page.new", json!({"folio": "102"}));
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(shell.tool().name(), "select");
    shell.shutdown();
}

#[test]
fn the_window_asks_for_the_fields_and_keeps_a_refusal_in_the_dialog() {
    let tmp = Tmp::new("form");
    let mut shell = new_project(&tmp, Mode::Gui);
    let before = shell.page_labels().len();
    done(&mut shell, "page.new", json!({"title": "Typed first"}));
    let Some(Dialog::Form(form)) = shell.dialog().cloned() else {
        panic!("{:?}", shell.dialog());
    };
    assert_eq!(form.action, "page.new");
    let values: Vec<(&str, &str)> = form.fields.iter().map(|f| (f.name, f.value.as_str())).collect();
    assert_eq!(
        values,
        [
            ("folio", ""),
            ("type", "multi_line"),
            ("title", "Typed first"),
            ("section", "")
        ]
    );
    assert!(form.fields[1].choices.contains(&"single_line") && !form.fields[1].choices.contains(&"report"));
    // Enter with no folio says what is missing and stays open.
    assert!(matches!(shell.submit_form(), Outcome::Failed { .. }));
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!()
    };
    assert_eq!(form.error.as_deref(), Some("Missing parameter `folio`."));
    shell.set_form_field("folio", "101");
    shell.set_form_field("type", "single_line");
    assert!(matches!(shell.dialog(), Some(Dialog::Form(f)) if f.error.is_none()));
    assert_eq!(shell.submit_form(), Outcome::Done);
    assert!(shell.dialog().is_none());
    assert_eq!(shell.page_labels().len(), before + 1);
    assert_eq!(shell.current_folio(), Some("101"));
    // The same folio again: the command's refusal stays under the fields, nothing is committed.
    done(&mut shell, "page.new", json!({}));
    shell.set_form_field("folio", "101");
    let Outcome::Failed { message } = shell.submit_form() else {
        panic!()
    };
    assert!(message.contains("already exists"));
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!()
    };
    assert_eq!(form.error.as_deref(), Some(message.as_str()));
    assert_eq!(shell.page_labels().len(), before + 1);
    // Escape cancels.
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.dialog().is_none());
    assert_eq!(shell.page_labels().len(), before + 1);
    // The other two dialogs.
    done(&mut shell, "structure.new", json!({}));
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!()
    };
    assert_eq!(form.fields[0].value, "location");
    shell.set_form_field("designation", "J-E-206");
    assert_eq!(shell.submit_form(), Outcome::Done);
    done(&mut shell, "external_ref.new", json!({"device": "JE-N1000-03"}));
    shell.set_form_field("location", "J-E-206");
    assert_eq!(shell.submit_form(), Outcome::Done);
    assert!(shell.dialog().is_none());
    done(&mut shell, "file.save", json!({}));
    let (nodes, refs) = saved_structure(&tmp);
    assert!(nodes.iter().any(|n| n.1 == "J-E-206"));
    assert_eq!(refs.len(), 1);
    // A dialog left open when the project closes is dropped by its own Enter, which has nothing to create in.
    done(&mut shell, "page.new", json!({}));
    done(&mut shell, "file.close", json!({}));
    shell.set_form_field("folio", "500");
    assert!(matches!(
        shell.submit_form(),
        Outcome::Refused {
            reason_key: "reason-no-project"
        }
    ));
    assert!(shell.dialog().is_none());
    shell.shutdown();
}

#[test]
fn the_dialog_describes_itself_in_the_vocabulary() {
    let tmp = Tmp::new("form-describe");
    let mut shell = new_project(&tmp, Mode::Gui);
    done(&mut shell, "page.new", json!({}));
    shell.set_form_field("folio", "A1");
    let _ = shell.submit_form();
    let text = describe(&shell);
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == "dialog \"New page\"")
        .expect("the dialog line");
    assert_eq!(
        &lines[at + 1..],
        [
            "  field \"Folio\": \"A1\"",
            "  field \"Type\": \"multi_line\"",
            "  field \"Title\": \"\"",
            "  field \"Section number\": \"\"",
            "  error: A folio starts with its number: \"A1\" does not.",
            "  button \"Create\"",
            "  button \"Cancel\"",
        ]
    );
    for line in &lines[at..] {
        let mut outside = String::new();
        let mut quoted = false;
        for c in line.chars() {
            if c == '"' {
                quoted = !quoted;
                outside.push(' ');
            } else if !quoted {
                outside.push(c);
            }
        }
        // `error:` leads user text to the end of the line.
        let rest = outside.split("error:").next().unwrap_or_default();
        for token in rest.split_whitespace() {
            assert!(
                token.chars().all(|c| !c.is_alphanumeric())
                    || VOCABULARY.contains(&token)
                    || FREE_TEXT_MARKERS.contains(&token),
                "`{token}` of `{line}`"
            );
        }
    }
    shell.shutdown();
}

#[test]
fn the_menu_entries_are_disabled_with_their_reason() {
    let tmp = Tmp::new("form-menu");
    let mut shell = shell_in(&tmp, Mode::Gui);
    let items = |shell: &ShellModel| {
        let mut out = Vec::new();
        for m in menu_model(shell) {
            for e in m.entries {
                if let MenuEntry::Item(i) = e
                    && ["page.new", "structure.new", "external_ref.new"].contains(&i.id)
                {
                    out.push((m.label.clone(), i.label, i.enabled, i.reason, i.shortcut));
                }
            }
        }
        out
    };
    let no_project = items(&shell);
    assert_eq!(
        no_project,
        [
            (
                "Insert".to_owned(),
                "Structure node...".to_owned(),
                false,
                Some("No project is open".to_owned()),
                String::new()
            ),
            (
                "Insert".to_owned(),
                "External reference...".to_owned(),
                false,
                Some("No project is open".to_owned()),
                String::new()
            ),
            (
                "Page".to_owned(),
                "New page...".to_owned(),
                false,
                Some("No project is open".to_owned()),
                String::new()
            ),
        ]
    );
    done(
        &mut shell,
        "file.new",
        json!({"dir": tmp.0.join("m.ew").to_string_lossy(), "template": "builtin"}),
    );
    assert!(items(&shell).iter().all(|i| i.2 && i.3.is_none()));
    shell.shutdown();
}

#[test]
fn a_structure_node_leaves_an_armed_place_tool_and_its_preview_alone() {
    let tmp = Tmp::new("tool-keeps");
    let mut s = tmp.shell_in(Mode::Gui, true, "101");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    let p = {
        let snapper = &s.canvas().unwrap().page().snapper;
        (snapper.page_of_index(60), snapper.page_of_index(30))
    };
    let sheet = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(&mut s, "canvas.pointer", at(sheet, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(s.tool().name(), "place");
    assert_eq!(s.preview_status(), Some(PreviewStatus::Ready), "the ghost's preview");
    // Escape closes the dialog and nothing else: the tool stays armed.
    done(&mut s, "structure.new", json!({}));
    assert!(matches!(s.dialog(), Some(Dialog::Form(_))));
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.dialog().is_none());
    assert_eq!(s.tool().name(), "place");
    // A node is created with the tool armed: one commit of the action's own preview, the tool's preview put back and
    // stale.
    let rev = s.revision();
    let calls = s.session_calls();
    done(
        &mut s,
        "structure.new",
        json!({"kind": "location", "designation": "J-E-206"}),
    );
    assert_ne!(s.revision(), rev);
    assert_eq!(s.tool().name(), "place");
    assert_eq!(s.preview_status(), Some(PreviewStatus::Stale));
    let after = s.session_calls();
    assert_eq!(after.preview_with, calls.preview_with + 1);
    assert_eq!(after.commit, calls.commit + 1);
    assert_eq!(after.execute_with, calls.execute_with);
    // The tool's next commit is not the stale preview's: it goes through `execute_with` and adds one representation.
    let before = s.canvas().unwrap().page().tables.reps.len();
    done(&mut s, "canvas.press", at(sheet, json!({})));
    done(&mut s, "canvas.release", at(sheet, json!({})));
    assert_eq!(s.session_calls().execute_with, after.execute_with + 1);
    assert_eq!(s.canvas().unwrap().page().tables.reps.len(), before + 1);
    s.shutdown();
}
