// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-410: File, Import, Parts table… as the headless action `parts.import`: the golden parts table into a new
//! project through the inline preview card (29 created, none skipped; a second import skips all 29), the picker
//! listing the parts in part-number order, a transcript that is byte-identical across runs and has no UUID, the
//! disabled menu item without a project, a refused table with its pointer and no card, and the recorded command.

use crate::common;

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
    done(&mut shell, "parts.map_import", json!({}));
    transcript.push(shell.describe());
    done(&mut shell, "preview.apply", json!({}));
    transcript.push(shell.describe());
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    done(&mut shell, "parts.map_import", json!({}));
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
    done(&mut shell, "parts.map_import", json!({}));
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
    // FT-759: the preview says what it imports in words, not "29 records".
    let summary = shell.preview_summary().expect("a summary");
    assert_eq!(summary, "29 new parts \u{b7} 0 already in the project");
    // The card is the import's own: Apply and Cancel need neither the grid nor the find pane.
    done(&mut shell, "preview.apply", json!({}));
    assert!(shell.can_undo(), "one undo step");
    // FT-719: the applied import is a toast, not a banner over the tab title.
    assert_eq!(shell.banner(), None);
    assert_eq!(
        common::state(&shell)["toast"]["text"],
        "Imported 29 parts from parts.csv (0 skipped)."
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
    done(&mut shell, "parts.map_import", json!({}));
    done(&mut shell, "preview.apply", json!({}));
    done(
        &mut shell,
        "parts.import",
        json!({"path": golden_parts().to_string_lossy()}),
    );
    done(&mut shell, "parts.map_import", json!({}));
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
    done(&mut shell, "parts.map_import", json!({}));
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
    // FT-649: a table without a header row cannot be mapped.
    std::fs::write(table.join("parts.csv"), "\n\n").unwrap();
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
    assert!(message.contains("header row"), "{message}");
    assert!(shell.properties().card().is_none(), "no card");
    assert!(shell.parts_map().is_none(), "no mapping step");
    let text = shell.describe();
    let banner = line_with(&text, "banner:");
    assert!(
        banner.contains("Parts table refused:") && banner.contains("header row"),
        "{banner}"
    );
    assert!(!text.lines().any(|l| l.starts_with("card ")), "{text}");
    assert!(shell.recorded_commands().is_empty());
    shell.shutdown();
}

/// A supplier's list with headers of its own (FT-649's fixture).
const SUPPLIER: &str = "Hersteller;Artikelnummer;Bezeichnung;Nennstrom\n\
                        Schneider Electric;A9F74210;Leitungsschutzschalter C10;10 A\n\
                        Phoenix Contact;3044102;Durchgangsklemme;\n";

/// The `card mapping` lines of the describe text.
fn mapping_lines(shell: &ShellModel) -> Vec<String> {
    let text = shell.describe();
    let mut out = Vec::new();
    let mut inside = false;
    for l in text.lines() {
        if l.starts_with("card mapping ") {
            inside = true;
        } else if inside && !l.starts_with("  ") {
            break;
        }
        if inside {
            out.push(l.to_owned());
        }
    }
    out
}

/// FT-649: a table with other headers opens the mapping step (no member pre-filled, Import refused with the
/// importer's message); mapped, it imports through the card and the parts are in the project; the map is saved for
/// the source key and fills the next import of the same source.
#[test]
fn a_supplier_list_imports_after_mapping_and_its_map_is_saved() {
    let tmp = Tmp::new("parts-map");
    let file = tmp.0.join("Supplier List.csv");
    std::fs::write(&file, SUPPLIER).unwrap();
    let mut shell = headless(&tmp);
    new_project(&mut shell, &tmp.0.join("proj.ew"));
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    let lines = mapping_lines(&shell);
    assert_eq!(
        lines[..3],
        [
            "card mapping \"Supplier List.csv\" source \"supplier-list\"",
            "  column \"Hersteller\" sample \"Schneider Electric\" member none",
            "  column \"Artikelnummer\" sample \"A9F74210\" member none",
        ],
        "{lines:?}"
    );
    assert_eq!(
        lines.last().unwrap(),
        "  error \"the column map needs a column for `manufacturer`\""
    );
    assert!(!shell.parts_map().unwrap().can_import());
    let out = shell.dispatch("parts.map_import", &json!({}));
    assert_eq!(
        out,
        Outcome::Failed {
            message: "the column map needs a column for `manufacturer`".into()
        }
    );
    assert!(shell.properties().card().is_none(), "no import card");
    // Cycle 37 (window check W37): the card shows the refusal and the members in words; the describe line and the
    // outcome above keep the member names.
    assert_eq!(
        shell.parts_map().unwrap().error_text().as_deref(),
        Some("Map a column to Manufacturer")
    );
    let words = |m: &str, h: &str| ew_viewmodel::shell::PartsMap::member_text(m, h);
    assert_eq!(words("manufacturer", "Hersteller"), "Manufacturer");
    assert_eq!(words("part_number", "Artikelnummer"), "Part number");
    assert_eq!(words("description.de", "Bezeichnung"), "Description (de)");
    assert_eq!(
        words("tech_data.nennstrom", "Nennstrom"),
        "Technical data: Nennstrom"
    );
    assert_eq!(words("free_props.farbe", "Farbe"), "Free property: Farbe");
    // The member choices of a column: the fixed members, then its own technical-data and free-property members.
    let choices = ew_viewmodel::shell::PartsMap::choices("Nennstrom");
    assert_eq!(choices[..2], ["manufacturer", "part_number"]);
    assert!(choices.contains(&"tech_data.nennstrom".to_owned()), "{choices:?}");
    for (column, member) in [
        ("Hersteller", "manufacturer"),
        ("Artikelnummer", "part_number"),
        ("Bezeichnung", "description.de"),
        ("Nennstrom", "tech_data.nennstrom"),
    ] {
        done(
            &mut shell,
            "parts.map",
            json!({"column": column, "member": member}),
        );
    }
    assert!(shell.parts_map().unwrap().can_import());
    assert!(!mapping_lines(&shell).iter().any(|l| l.starts_with("  error")));
    assert_eq!(shell.parts_map().unwrap().error_text(), None);
    // A member chosen twice, in words on the card.
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Artikelnummer", "member": "manufacturer"}),
    );
    assert_eq!(
        shell.parts_map().unwrap().error_text().as_deref(),
        Some("Manufacturer is chosen for two columns")
    );
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Artikelnummer", "member": "part_number"}),
    );
    // A member that is no part member is refused with the importer's message.
    let out = shell.dispatch("parts.map", &json!({"column": "Nennstrom", "member": "rating"}));
    assert!(
        matches!(&out, Outcome::Failed { message } if message.starts_with("`rating` is not a part member")),
        "{out:?}"
    );
    done(&mut shell, "parts.map_import", json!({}));
    assert!(shell.parts_map().is_none(), "the mapping step closed");
    let text = shell.describe();
    assert!(
        text.contains("card ready \"parts.import\" created 2 skipped 0 errors 0"),
        "{text}"
    );
    // Cycle 37 (window check W37): the Preview tab names the added parts by their numbers.
    let parts: Vec<String> = shell
        .preview_pane()
        .entities()
        .iter()
        .filter(|e| e.kind == "part")
        .map(|e| e.label.clone())
        .collect();
    assert_eq!(parts, ["3044102", "A9F74210"]);
    done(&mut shell, "preview.apply", json!({}));
    assert!(shell.can_undo(), "one undo step");
    // The parts are in the project: saved and read back.
    done(&mut shell, "file.save", json!({}));
    let (reopened, _) = Session::open_read_only(&tmp.0.join("proj.ew"), VERSION).unwrap();
    let mut numbers: Vec<String> = reopened
        .query()
        .parts()
        .iter()
        .map(|(_, p)| p.item.part_number.to_string())
        .collect();
    numbers.sort();
    assert_eq!(numbers, ["3044102", "A9F74210"]);
    drop(reopened);
    // The map is saved for the source key.
    let saved = std::fs::read_to_string(tmp.0.join("config/imports/supplier-list.toml")).unwrap();
    assert!(saved.contains("part_number = \"Artikelnummer\""), "{saved}");
    assert!(
        saved.contains("\"tech_data.nennstrom\" = \"Nennstrom\""),
        "{saved}"
    );
    // The next import of the same source is pre-filled from it.
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    let lines = mapping_lines(&shell);
    assert_eq!(
        lines[..3],
        [
            "card mapping \"Supplier List.csv\" source \"supplier-list\" saved",
            "  column \"Hersteller\" sample \"Schneider Electric\" member \"manufacturer\"",
            "  column \"Artikelnummer\" sample \"A9F74210\" member \"part_number\"",
        ],
        "{lines:?}"
    );
    // FT-719: the same table again holds no new parts: one toast says so, with no banner and no card.
    done(&mut shell, "parts.map_import", json!({}));
    assert_eq!(shell.banner(), None);
    assert!(shell.properties().card().is_none());
    assert_eq!(
        common::state(&shell)["toast"]["text"],
        "No new parts in Supplier List.csv: 2 already in the project, 0 rejected."
    );
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    // Another source key without a saved map keeps the members; Escape closes the step.
    done(&mut shell, "parts.map", json!({"source": "other-list"}));
    let map = shell.parts_map().unwrap();
    assert!(!map.saved && map.can_import(), "{map:?}");
    let out = shell.dispatch("parts.map", &json!({"source": "Bad Key"}));
    assert!(
        matches!(&out, Outcome::Failed { message } if message.starts_with("`Bad Key` is not a source key")),
        "{out:?}"
    );
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.parts_map().is_none());
    shell.shutdown();
}

/// FT-649: a required member left unmapped refuses the import with the importer's message, and nothing is imported.
#[test]
fn an_unmapped_part_number_refuses_with_the_importers_message() {
    let tmp = Tmp::new("parts-map-required");
    let file = tmp.0.join("list.csv");
    std::fs::write(&file, SUPPLIER).unwrap();
    let mut shell = headless(&tmp);
    new_project(&mut shell, &tmp.0.join("proj.ew"));
    done(
        &mut shell,
        "parts.import",
        json!({"path": file.to_string_lossy()}),
    );
    done(
        &mut shell,
        "parts.map",
        json!({"column": "Hersteller", "member": "manufacturer"}),
    );
    let message = "the column map needs a column for `part_number`";
    assert_eq!(shell.parts_map().unwrap().error.as_deref(), Some(message));
    assert_eq!(
        shell.parts_map().unwrap().error_text().as_deref(),
        Some("Map a column to Part number")
    );
    assert_eq!(
        shell.dispatch("parts.map_import", &json!({})),
        Outcome::Failed {
            message: message.into()
        }
    );
    assert!(shell.parts_map().is_some(), "the step stays open");
    assert!(shell.properties().card().is_none());
    assert!(shell.recorded_commands().is_empty());
    assert!(
        !tmp.0.join("config/imports/list.toml").exists(),
        "a refused map is not saved"
    );
    shell.shutdown();
}

#[test]
fn the_five_mapping_words_are_in_the_vocabulary_once() {
    use ew_viewmodel::describe::{MAP_WORDS, VOCABULARY};
    for w in MAP_WORDS {
        assert_eq!(VOCABULARY.iter().filter(|v| *v == w).count(), 1, "{w}");
    }
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
    done(&mut shell, "parts.map_import", json!({}));
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
    done(&mut shell, "parts.map_import", json!({}));
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
