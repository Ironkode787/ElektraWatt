// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties explains itself (FT-1648): every row the grid shows for the sample's records has its explanation in
//! both shipped languages.

use std::collections::BTreeSet;

use crate::common::{Tmp, done};
use ew_viewmodel::properties::{LabelPath as _, row_label};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

/// The `(kind, label path, label)` of every row of the sheet on show.
fn rows_on_show(s: &ShellModel, out: &mut BTreeSet<(String, String, String)>) {
    let Some(sheet) = s.properties().sheet() else {
        return;
    };
    for section in &sheet.sections {
        for row in &section.rows {
            let generic: String = {
                // One entry per list item kind, not per index.
                let mut g = String::new();
                let mut skip = false;
                for c in row.path_for_label().chars() {
                    match c {
                        '[' => {
                            skip = true;
                            g.push_str("[]");
                        }
                        ']' => skip = false,
                        _ if skip => {}
                        _ => g.push(c),
                    }
                }
                g
            };
            out.insert((section.kind.clone(), generic, row_label(section, row)));
        }
    }
}

/// FT-1648: the project's sheet, every page's, and the sheet of everything a person can select on the sample's
/// drawn pages (its symbols, wires, junctions, texts and boxes) have an explanation for every row, in en-GB and in
/// nl-NL.
#[test]
fn every_row_of_the_sample_explains_itself_in_both_languages() {
    let tmp = Tmp::new("props-explain");
    let mut s = tmp.open(false);
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.show_advanced", json!({"on": true}));
    let mut rows = BTreeSet::new();
    done(&mut s, "props.project", json!({}));
    rows_on_show(&s, &mut rows);
    let folios: Vec<(String, bool)> = s
        .page_rows()
        .iter()
        .map(|r| (r.folio.clone(), r.generated))
        .collect();
    for (folio, generated) in folios {
        done(&mut s, "page.goto", json!({ "folio": folio }));
        done(&mut s, "select.set", json!({"ids": []}));
        rows_on_show(&s, &mut rows);
        if generated {
            continue;
        }
        let Some(tables) = s.canvas().map(|c| std::sync::Arc::clone(&c.page().tables)) else {
            continue;
        };
        let ids = tables
            .reps
            .iter()
            .map(|r| r.id)
            .chain(tables.connections.iter().map(|c| c.id))
            .chain(tables.junctions.iter().map(|j| j.id))
            .chain(tables.items.iter().map(|i| i.id))
            .chain(tables.structure_boxes.iter().map(|b| b.id));
        for id in ids {
            done(&mut s, "select.set", json!({"ids": [id.to_string()]}));
            rows_on_show(&s, &mut rows);
        }
    }
    let missing: Vec<String> = rows
        .iter()
        .filter(|(kind, path, _)| strings::field_helps(kind, path).len() != 2)
        .map(|(kind, path, label)| format!("{kind} {path} ({label})"))
        .collect();
    assert!(
        missing.is_empty(),
        "rows without an explanation in both languages:\n{}",
        missing.join("\n")
    );
}
