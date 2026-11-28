// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Labels… and Workshop settings on a copy of the sample (doc 13 §5.4, §12): the dialog lists one
//! label file per label set and marker part with its tick and count, and the label file's profile in a line; Label
//! file… changes the files' columns and form and comes back to the dialog; Export writes the ticked files into the folder, each whole, and the toast names the
//! folder; Workshop settings shows the project's Properties at its Workshop rows.

use crate::common;

use common::{Tmp, copy_dir, done, refused};
use ew_viewmodel::shell::Dialog;
use ew_viewmodel::shell::labels::LabelsDialog;
use serde_json::json;

fn labels(s: &ew_viewmodel::shell::ShellModel) -> &LabelsDialog {
    match s.dialog() {
        Some(Dialog::Labels(d)) => d,
        other => panic!("no Labels… dialog: {other:?}"),
    }
}

#[test]
fn labels_lists_the_files_changes_their_columns_and_writes_the_ticked_ones() {
    let tmp = Tmp::new("labels");
    let mut s = tmp.sample_in(ew_viewmodel::shell::Mode::Gui, true, "101");
    done(&mut s, "labels.show", json!({}));
    let d = labels(&s);
    let files = d.files.clone().expect("the label files");
    let wires = files
        .iter()
        .find(|f| f.set == "Wire markers")
        .expect("the wire markers' file");
    assert_eq!(wires.marker_part, "EW-GEN-MRK-1");
    assert!(wires.count > 0 && wires.on);
    assert_eq!(d.profile, "Excel · Text 1, Text 2, Text 3, Copies, For");
    // Show lists the labels that do not fit, each with its set and marker part; Hide counts them only.
    assert!(!d.misfits.is_empty() && !d.show_misfits);
    done(&mut s, "labels.misfits", json!({}));
    let d = labels(&s);
    assert!(d.show_misfits);
    assert!(
        d.misfits.iter().all(|m| m.contains(" on ") && m.contains(": ")),
        "{:?}",
        d.misfits
    );
    // A file left unticked is not written.
    let cables = files
        .iter()
        .find(|f| f.set == "Cable markers")
        .expect("the cable markers' file")
        .name
        .clone();
    done(&mut s, "labels.tick", json!({ "file": cables, "on": false }));
    // Label file…: Text 1 alone, headed "Line", as a CSV, then the dialog again with that profile. The Delimiter shows
    // for a CSV only; two columns with one heading are refused in words.
    done(&mut s, "labels.columns", json!({}));
    let form = common::form(&s);
    assert!(!form.shows("delimiter"), "Excel has no delimiter");
    let table = form.table.expect("the columns' table");
    let rows = |headed: &[(&str, &str)]| -> Vec<serde_json::Value> {
        table
            .rows
            .iter()
            .map(|r| {
                let heading = headed.iter().find(|(k, _)| *k == r.key).map(|(_, h)| *h);
                json!({"values": {
                    table.columns[0].0.clone(): if heading.is_some() { ew_viewmodel::shell::TICKED } else { "" },
                    table.columns[1].0.clone(): heading.unwrap_or(""),
                    table.columns[2].0.clone(): "",
                }})
            })
            .collect()
    };
    let twice = rows(&[("Text 1", "Line"), ("Text 2", "Line")]);
    let refusal = common::failed(
        &mut s,
        "labels.columns",
        json!({ "columns": twice, "file": "csv" }),
    );
    assert!(refusal.contains("Line"), "{refusal}");
    done(
        &mut s,
        "labels.columns",
        json!({ "columns": rows(&[("Text 1", "Line")]), "file": "csv", "delimiter": "semicolon" }),
    );
    assert_eq!(labels(&s).profile, "CSV with semicolons · Line");
    let csv = |set: &str| {
        labels(&s)
            .files
            .clone()
            .expect("the files")
            .into_iter()
            .find(|f| f.set == set)
            .expect("the file")
    };
    let (wires, cables) = (csv("Wire markers"), csv("Cable markers"));
    assert!(wires.name.ends_with(".csv") && wires.on, "{}", wires.name);
    assert!(!cables.on, "an unticked file stays unticked in another form");
    let cables = cables.name;
    let dir = tmp.0.join("labels");
    done(&mut s, "labels.write", json!({ "dir": dir.to_string_lossy() }));
    let written: Vec<String> = std::fs::read_dir(&dir)
        .expect("the folder")
        .map(|e| e.expect("an entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert!(written.contains(&wires.name), "{written:?}");
    assert!(!written.contains(&cables), "{written:?}");
    let text = std::fs::read_to_string(dir.join(&wires.name)).expect("the wire markers' file");
    assert!(text.starts_with("\u{feff}Line\r\n-101Q1:1\r\n"), "{text}");
    // Nothing ticked: Export is refused in words.
    for f in labels(&s).files.clone().expect("the files") {
        done(&mut s, "labels.tick", json!({ "file": f.name, "on": false }));
    }
    assert_eq!(
        refused(&mut s, "labels.write", json!({ "dir": "x" })),
        "reason-labels-none"
    );
    done(&mut s, "ui.dismiss", json!({}));
    // Workshop settings: the project's Properties at its Workshop rows, the label file's among them in a line.
    done(&mut s, "workshop.settings", json!({}));
    let d = s.describe();
    assert!(d.contains("Ferrules"), "{d}");
    assert!(d.contains("CSV with semicolons · Line"), "{d}");
    // Generic columns fills the form with the generic profile; Apply gives the project the generic one back.
    done(&mut s, "labels.columns", json!({}));
    done(&mut s, "labels.columns", json!({ "generic": true }));
    let form = common::form(&s);
    let file = form.fields.iter().find(|f| f.name == "file").expect("File");
    assert_eq!(file.value, "xlsx");
    let table = form.table.clone().expect("the columns' table");
    let rows: Vec<serde_json::Value> = table
        .rows
        .iter()
        .map(|r| {
            let values: serde_json::Map<String, serde_json::Value> = table
                .columns
                .iter()
                .zip(&r.cells)
                .map(|((c, _), v)| (c.clone(), json!(v)))
                .collect();
            json!({ "values": values })
        })
        .collect();
    done(
        &mut s,
        "labels.columns",
        json!({ "columns": rows, "file": "xlsx", "delimiter": "comma" }),
    );
    done(&mut s, "workshop.settings", json!({}));
    let d = s.describe();
    assert!(d.contains("Excel · Text 1, Text 2, Text 3, Copies, For"), "{d}");
}

/// Label file… keeps what this build does not know of the export profile (doc 13 §5.4): a delimiter and a form of a
/// newer format, which a project saved by a newer build holds, stay as they are when the person leaves them, and so do
/// the profile's unknown members and those of a column it keeps.
#[test]
fn label_file_columns_keep_what_this_build_does_not_know() {
    let tmp = Tmp::new("labels-newer");
    // The copy of the sample as a newer build saved it: its profile with a delimiter and members this build does not know.
    let copy = tmp.sample_copy();
    common::copy_dir(std::path::Path::new(common::SAMPLE), &copy);
    let file = copy.join("project.jsonl");
    let lines: Vec<String> = std::fs::read_to_string(&file)
        .expect("the project file")
        .lines()
        .map(|l| {
            let mut v: serde_json::Value = serde_json::from_str(l).expect("a record");
            if v["$k"] == "project" {
                v["settings"]["workshop"]["label_profile"] = json!({
                    "columns": [{"key": "text_1", "heading": "Line", "width": 12}],
                    "delimiter": "tab",
                    "file": "ods",
                    "printer": "label-2",
                });
            }
            v.to_string()
        })
        .collect();
    std::fs::write(
        &file,
        lines.join(
            "
",
        ) + "
",
    )
    .expect("written");
    let mut s = tmp.sample_in(ew_viewmodel::shell::Mode::Gui, true, "101");
    done(&mut s, "labels.columns", json!({}));
    let form = common::form(&s);
    let value = |name: &str| {
        form.fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.clone())
            .expect("the field")
    };
    let (file, delimiter) = (value("file"), value("delimiter"));
    let table = form.table.expect("the columns' table");
    let rows: Vec<serde_json::Value> = table
        .rows
        .iter()
        .map(|r| {
            let (tick, heading, order) = match r.key.as_str() {
                "Text 1" => (ew_viewmodel::shell::TICKED, "Line", "1"),
                "For" => (ew_viewmodel::shell::TICKED, "For", "2"),
                _ => ("", "", ""),
            };
            json!({"values": {
                table.columns[0].0.clone(): tick,
                table.columns[1].0.clone(): heading,
                table.columns[2].0.clone(): order,
            }})
        })
        .collect();
    done(
        &mut s,
        "labels.columns",
        json!({ "columns": rows, "file": file, "delimiter": delimiter }),
    );
    done(&mut s, "file.save", json!({}));
    let (session, _) =
        ew_viewmodel::ew_api::Session::open_read_only(&tmp.sample_copy(), "test").expect("the saved copy");
    let stored =
        serde_json::to_value(&session.query().project().settings.workshop.label_profile).expect("JSON");
    assert_eq!(
        stored,
        json!({
            "columns": [{"key": "text_1", "heading": "Line", "width": 12}, {"key": "for", "heading": "For"}],
            "delimiter": "tab",
            "file": "ods",
            "printer": "label-2",
        })
    );
}

/// Where "Changed since a release" cannot be chosen, its place says why: the sample has no released revision; a
/// project released whose archive is not beside it says that, as the rework list does.
#[test]
fn changed_since_a_release_says_why_it_cannot_be_chosen() {
    let tmp = Tmp::new("labels-no-release");
    let mut s = tmp.sample_in(ew_viewmodel::shell::Mode::Gui, true, "101");
    done(&mut s, "labels.show", json!({}));
    assert_eq!(
        labels(&s).since_note.as_deref(),
        Some("Changed since a release: none yet, the project has no released revision")
    );
    assert!(
        s.describe()
            .contains("  note \"Changed since a release: none yet, the project has no released revision\""),
        "the dialog's text says it"
    );
    s.shutdown();

    let dir = tmp.0.join("release-min");
    copy_dir(std::path::Path::new(ew_testdir::RELEASE_MIN), &dir);
    let mut s = tmp.empty_shell(ew_viewmodel::shell::Mode::Gui);
    done(&mut s, "file.open", json!({ "dir": dir.to_string_lossy() }));
    for (action, step) in [
        ("revision.approve", json!({ "typed": "V1.0" })),
        (
            "revision.approve",
            json!({ "confirm": true, "date": "2026-10-30" }),
        ),
        ("revision.release", json!({ "typed": "V1.0" })),
        (
            "revision.release",
            json!({ "confirm": true, "date": "2026-10-30" }),
        ),
    ] {
        done(&mut s, action, step);
    }
    std::fs::remove_dir_all(tmp.0.join("release-min.ew-archive")).expect("the archive moved away");
    done(&mut s, "labels.show", json!({}));
    let note = labels(&s).since_note.clone().unwrap_or_default();
    assert!(
        note.starts_with("Changed since a release: V1.0 is released, but its archive, "),
        "{note}"
    );
    s.shutdown();
}
