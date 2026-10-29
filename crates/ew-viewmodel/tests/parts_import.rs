// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-410: File, Import, Parts table… as the headless action `parts.import`: the golden parts table into a new
//! project through the inline preview card (29 created, none skipped; a second import skips all 29), the picker
//! listing the parts in part-number order, a transcript that is byte-identical across runs and has no UUID, the
//! disabled menu item without a project, a refused table with its pointer and no card, and the recorded command.

mod common;

use std::path::{Path, PathBuf};

use common::{Tmp, done, refused};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::ew_api::import::read_parts_tables;
use ew_viewmodel::shell::{Mode, Outcome, Prompt, ShellConfig, ShellModel};
use ew_viewmodel::tools::PickerRow;
use serde_json::{Value, json};

const VERSION: &str = "0.0.0-test";

fn golden_parts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv")
}

fn headless(tmp: &Tmp) -> ShellModel {
    ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), VERSION),
    )
}

fn new_project(shell: &mut ShellModel, dir: &Path) {
    done(
        shell,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
}

fn line_with(text: &str, prefix: &str) -> String {
    text.lines()
        .map(str::trim_start)
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line {prefix}\n{text}"))
        .to_owned()
}

/// One run of the flow in a fresh temporary directory: the describe text after every step, joined.
fn run_flow(tag: &str) -> String {
    let tmp = Tmp::new(tag);
    let dir = tmp.0.join("proj.ew");
    let mut shell = headless(&tmp);
    new_project(&mut shell, &dir);
    let mut transcript = Vec::new();
    transcript.push(shell.describe());
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    transcript.push(shell.describe());
    done(&mut shell, "preview.apply", json!({}));
    transcript.push(shell.describe());
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    transcript.push(shell.describe());
    done(&mut shell, "preview.cancel", json!({}));
    transcript.push(shell.describe());
    shell.shutdown();
    transcript.join("\n----\n")
}

#[test]
fn the_golden_parts_table_imports_through_the_card_once() {
    let tmp = Tmp::new("parts-import");
    let dir = tmp.0.join("proj.ew");
    let mut shell = headless(&tmp);
    new_project(&mut shell, &dir);
    let calls = shell.session_calls();
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    assert_eq!(
        shell.session_calls().preview_with,
        calls.preview_with + 1,
        "a Bulk command is previewed through the host"
    );
    assert!(!shell.can_undo(), "the card waits for Apply");
    let text = shell.describe();
    assert!(
        text.contains("card ready \"parts.import\" created 29 skipped 0 errors 0"),
        "{text}"
    );
    let card = &common::state(&shell)["card"];
    assert_eq!(card["command"], "parts.import");
    assert_eq!(card["origin"], "import");
    assert_eq!(card["state"], "ready");
    // The card is the import's own: Apply and Cancel need neither the grid nor the find pane.
    done(&mut shell, "preview.apply", json!({}));
    assert!(shell.can_undo(), "one undo step");
    assert_eq!(
        shell.banner(),
        Some("Imported 29 parts from parts.csv (0 skipped).")
    );
    assert!(shell.properties().card().is_none());
    // Saved, closed and read back: 29 project-local part copies.
    done(&mut shell, "file.save", json!({}));
    done(&mut shell, "file.close", json!({}));
    let (reopened, _) = Session::open_read_only(&dir, VERSION).unwrap();
    assert_eq!(reopened.query().parts().iter().count(), 29);
    // A second import skips every key.
    new_project(&mut shell, &tmp.0.join("again.ew"));
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    done(&mut shell, "preview.apply", json!({}));
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    let text = shell.describe();
    assert!(
        text.contains("card ready \"parts.import\" created 0 skipped 29 errors 0"),
        "{text}"
    );
    done(&mut shell, "preview.cancel", json!({}));
    assert!(shell.properties().card().is_none());
    assert_eq!(refused(&mut shell, "preview.cancel", json!({})), "reason-no-card");
    shell.shutdown();
}

#[test]
fn the_picker_lists_the_imported_parts_under_their_manufacturer_in_part_number_order() {
    let tmp = Tmp::new("parts-picker");
    let mut shell = headless(&tmp);
    new_project(&mut shell, &tmp.0.join("proj.ew"));
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    done(&mut shell, "preview.apply", json!({}));
    done(&mut shell, "tool.place", json!({}));
    let picker = shell.picker().expect("the picker is open");
    // FT-425 (WP-S2.17 part 4, CP5): grouped by manufacturer, in part-number order within one (FT-439).
    let rows: Vec<(usize, &str, &str)> = picker
        .rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r {
            PickerRow::PartFn(p, _) => {
                let part = &picker.catalogue.parts[*p];
                Some((i, part.manufacturer.as_str(), part.part_number.as_str()))
            }
            _ => None,
        })
        .collect();
    assert!(!rows.is_empty(), "part rows are listed");
    let keys: Vec<(&str, &str)> = rows.iter().map(|(_, m, n)| (*m, *n)).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "manufacturer, then part-number order");
    // A header stands above the first part row of each manufacturer and above no other row.
    for (k, (i, m, _)) in rows.iter().enumerate() {
        let first = k == 0 || rows[k - 1].1 != *m;
        assert_eq!(picker.header_before(*i), first.then_some(*m), "row {i}");
    }
    assert!(
        (0..picker.rows.len())
            .filter(|i| !matches!(picker.rows[*i], PickerRow::PartFn(..)))
            .all(|i| picker.header_before(i).is_none()),
        "no header above a symbol row"
    );
    // The filter matches the manufacturer.
    let maker = rows[0].1.to_owned();
    done(&mut shell, "picker.filter", json!({"text": maker}));
    let picker = shell.picker().expect("the picker is open");
    assert!(picker.rows.iter().any(|r| matches!(r, PickerRow::PartFn(..))));
    done(&mut shell, "picker.filter", json!({"text": ""}));
    let text = shell.describe();
    assert!(text.contains("  row part \""), "{text}");
    shell.shutdown();
}

#[test]
fn the_transcript_is_byte_identical_across_runs_and_has_no_uuid() {
    let a = run_flow("parts-det-a");
    let b = run_flow("parts-det-b");
    assert_eq!(a, b);
    assert_eq!(ew_viewmodel::alias::find_uuid(&a), None, "{a}");
    assert!(a.contains("card ready \"parts.import\" created 29 skipped 0 errors 0"));
    assert!(a.contains("card ready \"parts.import\" created 0 skipped 29 errors 0"));
}

#[test]
fn without_a_project_the_menu_item_is_disabled_with_its_reason() {
    let tmp = Tmp::new("parts-no-project");
    let mut shell = headless(&tmp);
    let text = shell.describe();
    let line = line_with(&text, "item parts.import");
    assert_eq!(
        line, "item parts.import \"Parts table…\" disabled: No project is open",
        "{text}"
    );
    assert_eq!(
        refused(
            &mut shell,
            "parts.import",
            json!({"path": golden_parts().to_string_lossy()})
        ),
        "reason-no-project"
    );
    shell.shutdown();
}

#[test]
fn a_read_only_project_refuses_the_import() {
    let tmp = Tmp::new("parts-read-only");
    let mut shell = tmp.shell("101");
    assert_eq!(
        refused(
            &mut shell,
            "parts.import",
            json!({"path": golden_parts().to_string_lossy()})
        ),
        "reason-read-only"
    );
    let text = shell.describe();
    assert!(
        line_with(&text, "item parts.import").contains("disabled: The project is open read-only"),
        "{text}"
    );
    shell.shutdown();
}

#[test]
fn a_bad_table_is_refused_with_its_pointer_and_opens_no_card() {
    let tmp = Tmp::new("parts-bad");
    let table = tmp.0.join("tables");
    std::fs::create_dir_all(&table).unwrap();
    std::fs::write(table.join("parts.csv"), "key,part_number\nA,A1\n").unwrap();
    let mut shell = headless(&tmp);
    new_project(&mut shell, &tmp.0.join("proj.ew"));
    let out = shell.dispatch(
        "parts.import",
        &json!({"path": table.join("parts.csv").to_string_lossy()}),
    );
    let Outcome::Failed { message } = out else {
        panic!("{out:?}");
    };
    assert!(message.starts_with("Parts table refused:"), "{message}");
    assert!(message.contains("manufacturer"), "{message}");
    assert!(shell.properties().card().is_none(), "no card");
    let text = shell.describe();
    let banner = line_with(&text, "banner:");
    assert!(
        banner.contains("Parts table refused:") && banner.contains("manufacturer"),
        "{banner}"
    );
    assert!(!text.lines().any(|l| l.starts_with("card ")), "{text}");
    assert!(shell.recorded_commands().is_empty());
    shell.shutdown();
}

#[test]
fn the_recorded_command_carries_the_parameters_of_the_read() {
    let tmp = Tmp::new("parts-record");
    let mut shell = headless(&tmp);
    new_project(&mut shell, &tmp.0.join("proj.ew"));
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    assert!(shell.recorded_commands().is_empty(), "the card records nothing");
    done(&mut shell, "preview.apply", json!({}));
    let steps: Vec<(String, Value)> = shell
        .recorded_commands()
        .iter()
        .map(|c| (c.cmd.clone(), c.params.clone()))
        .collect();
    let imports: Vec<&(String, Value)> = steps.iter().filter(|(c, _)| c == "parts.import").collect();
    assert_eq!(imports.len(), 1, "{steps:?}");
    // The project ID of the new project is in the copies; the recorded line equals a read for that project.
    let dir = tmp.0.join("proj.ew");
    done(&mut shell, "file.save", json!({}));
    let (reopened, _) = Session::open_read_only(&dir, VERSION).unwrap();
    let id = reopened.query().project().id.as_uuid();
    let read = read_parts_tables(&golden_parts(), id).unwrap();
    assert_eq!(imports[0].1, read.params());
    shell.shutdown();
}

#[test]
fn the_window_asks_for_the_table_and_headless_needs_the_parameter() {
    let tmp = Tmp::new("parts-prompt");
    let mut headless_shell = headless(&tmp);
    new_project(&mut headless_shell, &tmp.0.join("proj.ew"));
    let out = headless_shell.dispatch("parts.import", &json!({}));
    assert!(matches!(out, Outcome::Failed { .. }), "{out:?}");
    headless_shell.shutdown();

    let tmp = Tmp::new("parts-prompt-gui");
    let mut gui = ShellModel::new(Mode::Gui, ShellConfig::in_dir(&tmp.0.join("config"), VERSION));
    new_project(&mut gui, &tmp.0.join("proj.ew"));
    done(&mut gui, "parts.import", json!({}));
    let prompt = gui.take_prompt().expect("a prompt for the window");
    assert_eq!(prompt.action, "parts.import");
    assert_eq!(prompt.prompt, Prompt::OpenPartsTable);
    assert_eq!(prompt.param, "path");
    gui.shutdown();
}

#[test]
fn the_card_survives_selection_changes_and_goes_with_the_project() {
    let tmp = Tmp::new("parts-survive");
    let mut shell = headless(&tmp);
    new_project(&mut shell, &tmp.0.join("proj.ew"));
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    done(&mut shell, "props.show", json!({}));
    done(&mut shell, "find.open", json!({}));
    done(&mut shell, "find.close", json!({}));
    assert!(
        shell.import_card_open(),
        "the grid and the find pane do not end it"
    );
    done(&mut shell, "file.close", json!({"discard": true}));
    assert!(shell.properties().card().is_none());
    shell.shutdown();
}

#[test]
fn the_three_words_are_in_the_vocabulary_once() {
    use ew_viewmodel::describe::{IMPORT_WORDS, VOCABULARY};
    for w in IMPORT_WORDS {
        assert_eq!(VOCABULARY.iter().filter(|v| *v == w).count(), 1, "{w}");
    }
}
