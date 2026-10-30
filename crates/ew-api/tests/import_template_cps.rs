// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-655: the tables importer and `ew-derive`'s `function_cps` agree on the CPs of a template-origin function. The
//! golden tables plus a contactor whose template pairs by position (coil `A1`/`A2` at pattern 0/1, aux contact
//! `13`/`14` at pattern 4/5, each FnDef with CPs 0 and 1): every connection end the importer writes is a CP the
//! derivation gives, with the designation the table names.

mod golden_common;

use ew_derive::FunctionCps;
use ew_import_tables::ew_interop_api::ImportInput;
use ew_import_tables::{TablesImporter, bootstrap_context};
use ew_schema::{Endpoint, Record};

/// Rows appended to the golden tables: the positional contactor `-102K9` and two wires on it.
const EXTRA: &[(&str, &str)] = &[
    (
        "parts.csv",
        "KPOS,Siemens,KPOS,,,Positional test contactor,piece,,",
    ),
    ("part_cps.csv", "KPOS:0,KPOS,0,A1,,,,"),
    ("part_cps.csv", "KPOS:1,KPOS,1,A2,,,,"),
    ("part_cps.csv", "KPOS:2,KPOS,2,1,,,,"),
    ("part_cps.csv", "KPOS:3,KPOS,3,2,,,,"),
    ("part_cps.csv", "KPOS:4,KPOS,4,13,,,,"),
    ("part_cps.csv", "KPOS:5,KPOS,5,14,,,,"),
    ("part_templates.csv", "KPOS:0,KPOS,0,ew.fn.coil,true,0;1"),
    ("part_templates.csv", "KPOS:1,KPOS,1,ew.fn.contact-no,false,4;5"),
    ("devices.csv", "-102K9,K,9,,,,,,=P01,+J-E-2416,,KPOS,,,,,,,,,,,,,"),
    ("functions.csv", "-102K9,-102K9,ew.fn.coil,,,,,,,,"),
    ("functions.csv", "-102K9#aux,-102K9,ew.fn.contact-no,aux,,,,,,,"),
    ("connections.csv", "WK91,WK91,,-102K9:13,-102K9:A1,,,,,,,,,"),
    ("connections.csv", "WK92,WK92,,-102K9:14,-102K9:A2,,,,,,,,,"),
];

fn inputs() -> Vec<ImportInput> {
    let dir = golden_common::repo().join("golden/J-E-2416/tables");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let mut text = std::fs::read_to_string(p).unwrap();
            for (_, row) in EXTRA.iter().filter(|(f, _)| *f == name) {
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(row);
                text.push('\n');
            }
            ImportInput::new(&name, text.into_bytes()).unwrap()
        })
        .collect()
}

#[test]
fn imported_connection_ends_are_derived_cps_of_a_positional_template() {
    let batch = TablesImporter::new(golden_common::library())
        .import(&inputs(), &bootstrap_context())
        .expect("the extended tables import");
    // The contactor's functions record their template entries, so the derivation follows the importer's choice.
    let k9: Vec<&ew_schema::Function> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Function(f) => Some(f),
            _ => None,
        })
        .filter(|f| {
            f.source_ref
                .as_ref()
                .is_some_and(|s| format!("{s:?}").contains("-102K9"))
        })
        .collect();
    let mut entries: Vec<Option<u16>> = k9.iter().map(|f| f.template_entry).collect();
    entries.sort_unstable();
    assert_eq!(entries, [Some(0), Some(1)]);
    let s = golden_common::session(&batch);
    let q = s.query();
    // Positional pairing: both functions have CPs 0 and 1, drawn as their pattern entries.
    for f in &k9 {
        let cps = q.get::<FunctionCps>(f.id);
        let got: Vec<(u16, &str)> = cps.iter().map(|c| (c.idx, c.designation.as_str())).collect();
        let want: &[(u16, &str)] = if f.template_entry == Some(1) {
            &[(0, "13"), (1, "14")]
        } else {
            &[(0, "A1"), (1, "A2")]
        };
        assert_eq!(got, want);
    }
    // Agreement over the whole batch: every CP end of every connection is a derived CP of its function.
    let mut checked = 0;
    for r in &batch.records {
        let Record::Connection(c) = r else { continue };
        for end in [&c.a, &c.b] {
            if let Endpoint::Cp(f, i) = end {
                let cps = q.get::<FunctionCps>(*f);
                assert!(
                    cps.iter().any(|cp| cp.idx == *i),
                    "connection {:?}: CP {i} of function {f} is not derived ({:?})",
                    c.source_ref,
                    cps.iter().map(|cp| cp.idx).collect::<Vec<_>>()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 4, "the batch has CP ends besides the contactor's");
}
