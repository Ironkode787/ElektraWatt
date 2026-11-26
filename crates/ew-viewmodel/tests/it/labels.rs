// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Labels… and Workshop settings on a copy of the sample (doc 13 §5.4, §12): the dialog lists one
//! label file per label set and marker part with its tick and count; Label file columns… changes the files' columns
//! and comes back to the dialog; Export writes the ticked files into the folder, each whole, and the toast names the
//! folder; Workshop settings shows the project's Properties at its Workshop rows.

use crate::common;

use common::{Tmp, done, refused};
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
    assert_eq!(d.columns, ["Text 1", "Text 2", "Text 3", "Copies", "For"]);
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
    // Label file columns…: Text 1 alone, headed "Line", then the dialog again with that column.
    done(&mut s, "labels.columns", json!({}));
    let form = common::form(&s);
    let table = form.table.expect("the columns' table");
    let rows: Vec<serde_json::Value> = table
        .rows
        .iter()
        .map(|r| {
            let on = r.key == "Text 1";
            json!({"values": {
                table.columns[0].0.clone(): if on { ew_viewmodel::shell::TICKED } else { "" },
                table.columns[1].0.clone(): if on { "Line" } else { "" },
                table.columns[2].0.clone(): "",
            }})
        })
        .collect();
    done(
        &mut s,
        "labels.columns",
        json!({ "columns": rows, "delimiter": "semicolon" }),
    );
    assert_eq!(labels(&s).columns, ["Line"]);
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
    // Workshop settings: the project's Properties at its Workshop rows.
    done(&mut s, "workshop.settings", json!({}));
    let d = s.describe();
    assert!(d.contains("Ferrules"), "{d}");
}
