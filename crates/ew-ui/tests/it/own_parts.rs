// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The person's own parts, made in the window from nothing (FT-1766, FT-1767): New part… in the device browser opens
//! the part editor on a new part, whose bus interfaces are a form; Save puts the part into the project and into My
//! parts. The window's own path: clicks and keys on a new project at 1920 × 1080.

use crate::persona_round::{click, key, typed};
use crate::support::Tmp;

use ew_ui::ew_viewmodel::ew_api::library::LibraryTiers;
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use ew_ui::visual::Driver;
use serde_json::{Value, json};

/// A driver over a new project from the built-in template, with the person's own libraries under the test's folder
/// (My parts is made there).
pub(crate) fn new_project(tmp: &Tmp) -> Driver {
    let tiers = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut d = Driver::in_folder(tmp.config().with_libraries(Some(tiers)), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// Clicks the last widget named `name` (a dialog is painted over the window, so its widgets come last: the dialog's
/// Save, not the quick-access one; its Description, not the page's) and settles.
pub(crate) fn press(d: &mut Driver, name: &str) {
    let at = d
        .labels()
        .into_iter()
        .rfind(|(l, _)| l == name)
        .unwrap_or_else(|| panic!("no widget named {name:?}"))
        .1;
    d.input(&click(at));
    d.settle();
}

/// Clicks the field named `name` and types `text` into it.
pub(crate) fn fill(d: &mut Driver, name: &str, text: &str) {
    press(d, name);
    d.input(&typed(text));
    d.settle();
}

/// The open part editor's draft.
fn draft(d: &mut Driver) -> ew_ui::ew_viewmodel::ew_api::PartDraft {
    match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => e.draft.clone(),
        other => panic!("the part editor is open: {other:?}"),
    }
}

/// The open project's records as JSON, one per record.
pub(crate) fn records(d: &mut Driver) -> Vec<Value> {
    let world = d.shell_mut().world_for_tests().expect("a project");
    ew_commands::rows::model_lines(&world.model)
        .expect("every record has a JSON form")
        .into_iter()
        .map(|(_, line)| serde_json::from_str::<Value>(&line).unwrap_or(Value::Null))
        .collect()
}

/// Every file of a library folder, one after the other.
fn library_text(dir: &std::path::Path) -> String {
    let mut out = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for e in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
            }
        }
    }
    out
}

/// An unmanaged switch with five ports, typed from its data sheet: New part… in the device browser, the identity, one
/// bus interface whose role is passive and whose port count is 5 (a wrong count is refused in words at the field),
/// Save. The project holds the part with its bus section, as My parts' part, and My parts holds it for the next
/// project.
#[test]
fn a_new_part_with_a_bus_interface_is_made_in_the_window_and_kept_in_my_parts() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&key("Shift+I"));
    d.settle();
    press(&mut d, "New part…");
    assert!(draft(&mut d).new, "New part… opens the editor on a new part");
    fill(&mut d, "Manufacturer", "Acme");
    fill(&mut d, "Part number", "SW-5");
    fill(&mut d, "Description", "Ethernet switch, unmanaged, 5 ports");
    press(&mut d, "Add interface");
    assert_eq!(draft(&mut d).bus.len(), 1, "one interface row");
    press(&mut d, "Role 1");
    press(&mut d, "Passive");
    fill(&mut d, "Ports 1", "0");
    let said = d
        .texts()
        .into_iter()
        .any(|t| t.starts_with("Ports: \u{201c}0\u{201d} is no number of ports"));
    assert!(said, "a wrong port count is refused in words at its field");
    d.input(&key("Backspace"));
    d.input(&typed("5"));
    d.settle();
    let row = draft(&mut d).bus[0].clone();
    assert_eq!(
        (row.interface.as_str(), row.bus.as_str(), row.role.as_str(), row.ports.as_str()),
        ("X1", "profinet", "passive", "5")
    );
    press(&mut d, "Save");
    let left = match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => Some((e.error.clone(), e.draft.messages.clone())),
        _ => None,
    };
    assert_eq!(left, None, "Save closes the editor");
    let all = records(&mut d);
    let part = all
        .iter()
        .find(|v| v["part_number"] == "SW-5")
        .expect("the project holds the new part");
    assert!(
        part["description"].to_string().contains("Ethernet switch, unmanaged, 5 ports"),
        "{}",
        part["description"]
    );
    let port = &part["sections"]["bus"]["ports"][0];
    assert_eq!((port["role"].as_str(), port["ports"].as_u64()), (Some("passive"), Some(5)));
    let toast = d.shell_mut().toast().map(|t| t.text.clone());
    assert_eq!(toast.as_deref(), Some("Part SW-5 is in the project and in My parts."));
    let mine = tmp.0.join("data").join("libraries").join("user-parts.ewlib");
    assert!(
        library_text(&mine).contains("\"part_number\":\"SW-5\""),
        "My parts holds it for the next project"
    );
}
