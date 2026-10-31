// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict and lenient colour import (doc 20 §19 D-17, doc 14 §4.12 and §8.4, doc 15 §2.6 and §4.10): a foreign
//! colour code such as `BR` fails its row in a strict import (EW-IMP-002) and is kept and reported with a quick
//! fix in a lenient one (EW-IMP-008).

use crate::common;

use ew_import_tables::TablesImporter;
use ew_import_tables::ids::record_id;
use ew_interop_api::{Code, ImportBatch, ImportInput, ImportMode, QuickFixProposal, ReportEntry, Severity};
use ew_schema::{ColourCode, ColourSpec, Conductor, Connection, IecColour, LibDocument, Record};
use serde_json::json;

fn import(inputs: &[ImportInput], mode: ImportMode) -> ImportBatch {
    let mut ctx = common::context();
    ctx.mode = mode;
    TablesImporter::new(common::library())
        .import(inputs, &ctx)
        .expect("the import succeeds")
}

/// The golden tables with `from` replaced by `to` in table `file` (`from` occurs exactly once).
fn golden_with(file: &str, from: &str, to: &str) -> Vec<ImportInput> {
    common::golden_inputs()
        .into_iter()
        .map(|input| {
            if input.name() != file {
                return input;
            }
            let text = String::from_utf8(input.bytes().to_vec()).expect("UTF-8");
            assert_eq!(text.matches(from).count(), 1, "{file}: {from}");
            ImportInput::new(file, text.replace(from, to).into_bytes()).unwrap()
        })
        .collect()
}

fn foreign(code: &str) -> ColourCode {
    ColourCode::Foreign(code.parse().unwrap())
}

fn iec(code: IecColour) -> ColourCode {
    ColourCode::Iec(code)
}

fn entries(b: &ImportBatch, code: Code) -> Vec<&ReportEntry> {
    b.report.with_code(code).collect()
}

fn errors(b: &ImportBatch) -> Vec<&ReportEntry> {
    b.report
        .entries
        .iter()
        .filter(|e| e.severity == Severity::Error)
        .collect()
}

fn conductor(b: &ImportBatch, key: &str) -> Option<Conductor> {
    let id = record_id::<Conductor>("conductors", key);
    b.records.iter().find_map(|r| match r {
        Record::Conductor(c) if c.id == id => Some(c.clone()),
        _ => None,
    })
}

fn at(e: &ReportEntry, file: &str, item: &str) {
    let loc = e.location.as_ref().expect("a location");
    assert_eq!((loc.file.as_str(), loc.item.as_deref()), (file, Some(item)));
    assert!(loc.line.is_some_and(|l| l >= 2), "{loc:?}");
}

const BR_ON_BN: (&str, &str, &str) = (
    "conductors.csv",
    "-106W1:BN,-106W1,BN,BN,",
    "-106W1:BN,-106W1,BN,BR,",
);

/// Strict: the conductor whose colour is `BR` is refused with EW-IMP-002, named in the message, and no
/// EW-IMP-008 is reported.
#[test]
fn strict_refuses_a_foreign_colour_code() {
    let (file, from, to) = BR_ON_BN;
    let b = import(&golden_with(file, from, to), ImportMode::Strict);
    assert!(conductor(&b, "-106W1:BN").is_none());
    let found: Vec<&ReportEntry> = entries(&b, Code::MALFORMED_ITEM)
        .into_iter()
        .filter(|e| e.location.as_ref().is_some_and(|l| l.file == "conductors.csv"))
        .collect();
    assert_eq!(found.len(), 1, "{:#?}", b.report.entries);
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(
        found[0].message,
        "row dropped: column `colour`: `BR` is a foreign colour code (IEC 60757: BN); a strict import stores \
         no foreign code (EW-CORE-031)"
    );
    at(found[0], "conductors.csv", "-106W1:BN");
    assert!(entries(&b, Code::FOREIGN_VALUE).is_empty());
    assert!(conductor(&b, "-106W1:BK").is_some());
}

/// The default context of the bootstrap is strict.
#[test]
fn the_bootstrap_runs_strict() {
    assert_eq!(ew_import_tables::bootstrap_context().mode, ImportMode::Strict);
    assert_eq!(common::context().mode, ImportMode::Strict);
}

/// Lenient: the conductor keeps `[Foreign("BR")]`, one EW-IMP-008 warning at the row carries the quick fix
/// `fields.set` with the IEC colour, and nothing else is reported.
#[test]
fn lenient_keeps_a_foreign_colour_code_with_a_quick_fix() {
    let (file, from, to) = BR_ON_BN;
    let b = import(&golden_with(file, from, to), ImportMode::Lenient);
    let c = conductor(&b, "-106W1:BN").expect("the conductor is kept");
    assert_eq!(
        c.colour.as_ref().map(ColourSpec::codes),
        Some(&[foreign("BR")][..])
    );
    assert!(errors(&b).is_empty(), "{:#?}", b.report.entries);
    assert_eq!(b.report.entries.len(), 1, "{:#?}", b.report.entries);
    let found = entries(&b, Code::FOREIGN_VALUE);
    assert_eq!(found.len(), 1);
    let e = found[0];
    assert_eq!(e.severity, Severity::Warning);
    assert_eq!(
        e.message,
        "column `colour`: `BR` kept as a foreign colour code (IEC 60757: BN)"
    );
    at(e, "conductors.csv", "-106W1:BN");
    let id = record_id::<Conductor>("conductors", "-106W1:BN").to_string();
    assert_eq!(
        e.fix,
        Some(QuickFixProposal {
            label: "use BN".into(),
            command: "fields.set".into(),
            params: json!({"kind": "conductor", "targets": [id], "path": "colour", "value": ["BN"]}),
        })
    );
    // Without the changed cell the conductor is the golden one.
    let golden = import(&common::golden_inputs(), ImportMode::Lenient);
    assert!(golden.report.entries.is_empty());
    let g = conductor(&golden, "-106W1:BN").unwrap();
    assert_eq!(
        g.colour.as_ref().map(ColourSpec::codes),
        Some(&[iec(IecColour::Bn)][..])
    );
}

// ---------------------------------------------------------------------------------------------------------
// Other cells and tables

const DEVICES: &str = "key,letter,counter,cable\n-1W1,W,1,true\n";

fn conductors(cells: &[&str], mode: ImportMode) -> ImportBatch {
    let mut text = String::from("key,cable,core,colour\n");
    for (i, cell) in cells.iter().enumerate() {
        text.push_str(&format!("-1W1:{i},-1W1,{i},{cell}\n"));
    }
    let files = [
        ("project.csv", common::PROJECT_CSV),
        ("devices.csv", DEVICES),
        ("conductors.csv", text.as_str()),
    ];
    import(&common::inputs(&files), mode)
}

/// A two-chunk cell keeps its IEC chunk; the quick fix maps every alias and keeps the IEC chunk.
#[test]
fn a_two_chunk_cell_is_kept_lenient_and_maps_both_chunks() {
    let b = conductors(&["BRWH", "WSBR"], ImportMode::Lenient);
    let first = conductor(&b, "-1W1:0").expect("kept");
    assert_eq!(
        first.colour.as_ref().map(ColourSpec::codes),
        Some(&[foreign("BR"), iec(IecColour::Wh)][..])
    );
    let found = entries(&b, Code::FOREIGN_VALUE);
    assert_eq!(found.len(), 2, "{:#?}", b.report.entries);
    assert_eq!(
        found[0].message,
        "column `colour`: `BR` kept as a foreign colour code (IEC 60757: BN)"
    );
    let id = record_id::<Conductor>("conductors", "-1W1:0").to_string();
    assert_eq!(
        found[0].fix,
        Some(QuickFixProposal {
            label: "use BN".into(),
            command: "fields.set".into(),
            params: json!({"kind": "conductor", "targets": [id], "path": "colour", "value": ["BN", "WH"]}),
        })
    );
    // Two aliases in one cell: one finding listing both, the label lists both codes.
    assert_eq!(
        found[1].message,
        "column `colour`: `WS` → `WH` and `BR` → `BN` kept as foreign colour codes (IEC 60757)"
    );
    let fix = found[1].fix.as_ref().expect("a fix");
    assert_eq!(fix.label, "use WH, BN");
    assert_eq!(fix.params["value"], json!(["WH", "BN"]));
    assert!(errors(&b).is_empty());
    // Strict drops both rows.
    let b = conductors(&["BRWH", "WSBR"], ImportMode::Strict);
    assert!(conductor(&b, "-1W1:0").is_none() && conductor(&b, "-1W1:1").is_none());
    assert_eq!(errors(&b).len(), 2);
    assert_eq!(
        errors(&b)[1].message,
        "row dropped: column `colour`: `WS` → `WH` and `BR` → `BN` are foreign colour codes (IEC 60757); a strict \
         import stores no foreign code (EW-CORE-031)"
    );
}

/// IEC codes and company codes are no foreign codes, in both modes.
#[test]
fn company_and_iec_codes_are_unaffected() {
    for mode in [ImportMode::Strict, ImportMode::Lenient] {
        let b = conductors(&["DBU", "GNYE", "BK"], mode);
        assert!(b.report.entries.is_empty(), "{mode:?}: {:#?}", b.report.entries);
        let colour = |k: &str| conductor(&b, k).unwrap().colour.unwrap();
        assert!(matches!(colour("-1W1:0").codes(), [ColourCode::Company(_)]));
        assert_eq!(colour("-1W1:1").codes(), [iec(IecColour::Gn), iec(IecColour::Ye)]);
        assert_eq!(colour("-1W1:2").codes(), [iec(IecColour::Bk)]);
    }
}

/// A `connections.csv` colour override: the quick fix edits `overrides.colour` of the connection.
#[test]
fn a_connection_override_gets_a_quick_fix_on_overrides_colour() {
    let (file, from, to) = (
        "connections.csv",
        "W1,W1,101,ip:J-E-206-L@101,1X1:1/field,,,,,,,,,",
        "W1,W1,101,ip:J-E-206-L@101,1X1:1/field,,,,,,,GE,,",
    );
    let inputs = golden_with(file, from, to);
    let b = import(&inputs, ImportMode::Lenient);
    let id = record_id::<Connection>("connections", "W1");
    let c = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Connection(c) if c.id == id => Some(c),
            _ => None,
        })
        .expect("the connection is kept");
    assert_eq!(
        c.overrides.colour.as_ref().map(ColourSpec::codes),
        Some(&[foreign("GE")][..])
    );
    assert_eq!(b.report.entries.len(), 1, "{:#?}", b.report.entries);
    let e = &b.report.entries[0];
    assert_eq!((e.code, e.severity), (Code::FOREIGN_VALUE, Severity::Warning));
    assert_eq!(
        e.message,
        "column `colour`: `GE` kept as a foreign colour code (IEC 60757: YE)"
    );
    at(e, "connections.csv", "W1");
    assert_eq!(
        e.fix,
        Some(QuickFixProposal {
            label: "use YE".into(),
            command: "fields.set".into(),
            params: json!({
                "kind": "connection",
                "targets": [id.to_string()],
                "path": "overrides.colour",
                "value": ["YE"],
            }),
        })
    );
    // Strict: the connection is dropped.
    let b = import(&inputs, ImportMode::Strict);
    assert!(
        !b.records
            .iter()
            .any(|r| matches!(r, Record::Connection(c) if c.id == id))
    );
    assert_eq!(
        entries(&b, Code::MALFORMED_ITEM).len(),
        1,
        "{:#?}",
        b.report.entries
    );
}

/// A cable core of a part copy: the finding has no quick fix (no command edits a part copy yet).
#[test]
fn a_cable_core_finding_has_no_quick_fix() {
    let (file, from, to) = (
        "cable_cores.csv",
        "NEBU-G78W5-K-2-N-LE5:BN,NEBU-G78W5-K-2-N-LE5,4,BN,BN,",
        "NEBU-G78W5-K-2-N-LE5:BN,NEBU-G78W5-K-2-N-LE5,4,BN,BR,",
    );
    let inputs = golden_with(file, from, to);
    let cores = |b: &ImportBatch| {
        b.lib
            .iter()
            .find_map(|d| match d {
                LibDocument::Part(p)
                    if p.item
                        .meta
                        .key
                        .as_ref()
                        .is_some_and(|k| k.as_str() == "NEBU-G78W5-K-2-N-LE5") =>
                {
                    Some(p.item.sections.cable_cores.clone())
                }
                _ => None,
            })
            .expect("the part")
    };
    let b = import(&inputs, ImportMode::Lenient);
    let kept = cores(&b);
    assert!(
        kept.iter()
            .any(|c| c.colour.as_ref().map(ColourSpec::codes) == Some(&[foreign("BR")][..]))
    );
    let found = entries(&b, Code::FOREIGN_VALUE);
    assert_eq!(found.len(), 1, "{:#?}", b.report.entries);
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found[0].fix, None);
    at(found[0], "cable_cores.csv", "NEBU-G78W5-K-2-N-LE5:BN");
    assert!(errors(&b).is_empty());
    let strict = import(&inputs, ImportMode::Strict);
    assert_eq!(cores(&strict).len(), kept.len() - 1);
    assert_eq!(errors(&strict).len(), 1, "{:#?}", strict.report.entries);
    assert!(entries(&strict, Code::FOREIGN_VALUE).is_empty());
}

/// A foreign code in the colour profile is kept lenient (without a quick fix) and refuses the row strict.
#[test]
fn a_colour_profile_row_follows_the_mode() {
    let (file, from, to) = (
        "colour_profile.csv",
        "3,24 VDC,0V,M,White,WH,m,",
        "3,24 VDC,0V,M,White,WS,m,",
    );
    let inputs = golden_with(file, from, to);
    let b = import(&inputs, ImportMode::Lenient);
    let found = entries(&b, Code::FOREIGN_VALUE);
    assert_eq!(found.len(), 1, "{:#?}", b.report.entries);
    assert_eq!(
        found[0].message,
        "column `code`: `WS` kept as a foreign colour code (IEC 60757: WH)"
    );
    assert_eq!(found[0].fix, None);
    assert!(errors(&b).is_empty());
    let strict = import(&inputs, ImportMode::Strict);
    assert_eq!(errors(&strict).len(), 1, "{:#?}", strict.report.entries);
    assert!(entries(&strict, Code::FOREIGN_VALUE).is_empty());
}
