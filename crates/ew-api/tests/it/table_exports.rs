// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over the sample in `ew-api`: the table writers behind the row exports (`row_output`), where the model can be
//! changed or reloaded: -101F1's function text set to a formula, the records shuffled, and the `ew` dialect through
//! the table model.

use ew_api::output::sheet::read_sheet;
use ew_api::output::{
    CsvOptions, PdfDate, Preset, RowExport, RowsFormat, RowsOutput, Table, TableCell, row_export, row_output,
};

/// The seed of the record shuffle.
const SHUFFLE_SEED: u64 = 0x5eed_ac14;

/// The workbook of `what` over `session` with a fixed date.
fn workbook(session: &ew_api::Session, what: &RowExport) -> Vec<u8> {
    let meta = session.workbook_meta(Some(PdfDate::ymd(2026, 1, 1)));
    let file = row_output(&session.query(), what, None, &RowsFormat::Xlsx(meta)).unwrap();
    let RowsOutput::Xlsx(bytes) = file.output else {
        panic!("not a workbook");
    };
    bytes
}

/// The CSV of `what` over `session` in `preset`.
fn csv(session: &ew_api::Session, what: &RowExport, preset: Preset) -> String {
    let format = RowsFormat::Csv(CsvOptions::preset(preset));
    let file = row_output(&session.query(), what, None, &format).unwrap();
    let RowsOutput::Csv(text) = file.output else {
        panic!("not a CSV");
    };
    text
}

/// The workbooks of a model loaded with its records shuffled equal the file-order ones, byte for byte, for a
/// block's rows, the bill of materials and every strip of the terminal diagrams.
#[test]
fn the_workbooks_do_not_depend_on_record_order() {
    let plain = crate::view::sample_session(None);
    let shuffled = crate::view::sample_session(Some(SHUFFLE_SEED));
    for what in [
        RowExport::Wires,
        RowExport::Bom(ew_api::BomMode::Component),
        RowExport::Terminals { strip: None },
        RowExport::Kind("device_list".to_owned()),
    ] {
        let a = workbook(&plain, &what);
        assert!(
            a == workbook(&shuffled, &what),
            "{what:?}: the shuffled model's workbook differs"
        );
        assert!(a == workbook(&plain, &what), "{what:?}: a second export differs");
    }
}

/// -101F1's function text is `=HYPERLINK("x")`. The workbook holds it as an inline string and no
/// part has an `<f>` element; `excel-nl` writes it guarded and quoted as RFC 4180 asks, `rfc4180` verbatim; a tag
/// that starts with `+` is guarded in the Excel presets only.
#[test]
fn a_formula_function_text_stays_text() {
    let mut session = crate::common::sample();
    let f1 = session.resolve_reference("tag:-101F1").expect("the breaker");
    session
        .execute(&ew_api::AnyCommand {
            name: "fields.set".into(),
            params: serde_json::json!({
                "kind": "device",
                "targets": [f1.to_string()],
                "path": "function_text",
                "value": {"en": "=HYPERLINK(\"x\")"},
            }),
        })
        .expect("the function text is set");
    let what = RowExport::Kind("device_list".to_owned());
    let bytes = workbook(&session, &what);
    let entries = ew_api::zip::read(&bytes, &ew_api::zip::ZipLimits::default()).unwrap();
    for e in &entries {
        let text = String::from_utf8_lossy(&e.data);
        assert!(!text.contains("<f>") && !text.contains("<f "), "{}", e.name);
    }
    let sheet = entries
        .iter()
        .find(|e| e.name == "xl/worksheets/sheet1.xml")
        .map(|e| String::from_utf8_lossy(&e.data).into_owned())
        .unwrap();
    assert!(
        sheet.contains("t=\"inlineStr\"><is><t>=HYPERLINK(&quot;x&quot;)</t></is></c>"),
        "{sheet}"
    );
    let table = read_sheet(
        &bytes,
        ew_api::catalog::SourceFormat::Xlsx,
        None,
        &ew_api::catalog::table_limits(),
    )
    .unwrap();
    let row = table
        .rows
        .iter()
        .find(|r| r[1] == "+CAB-101F1")
        .expect("the -101F1 row");
    assert_eq!(row[2], "=HYPERLINK(\"x\")");

    let line = |text: &str| -> String {
        text.lines()
            .find(|l| l.contains("101F1"))
            .expect("the -101F1 line")
            .to_owned()
    };
    let nl = line(&csv(&session, &what, Preset::ExcelNl));
    assert!(
        nl.contains(";'+CAB-101F1;\"'=HYPERLINK(\"\"x\"\")\";EW-GEN-MCB-4;"),
        "{nl}"
    );
    let en = line(&csv(&session, &what, Preset::ExcelEn));
    assert!(
        en.contains(",'+CAB-101F1,\"'=HYPERLINK(\"\"x\"\")\",EW-GEN-MCB-4,"),
        "{en}"
    );
    let rfc = line(&csv(&session, &what, Preset::Rfc4180));
    assert!(
        rfc.contains(",+CAB-101F1,\"=HYPERLINK(\"\"x\"\")\",EW-GEN-MCB-4,"),
        "{rfc}"
    );
}

/// The default preset `ew` is the dialect of `row_export` byte for byte, and the table model written in `ew` is
/// the same text where the project writes a decimal point (the sample).
#[test]
fn the_ew_preset_is_the_row_export_dialect() {
    let session = crate::common::sample();
    let q = session.query();
    for what in [RowExport::Wires, RowExport::Bom(ew_api::BomMode::Component)] {
        let golden = row_export(&q, &what, None).unwrap().csv;
        assert_eq!(csv(&session, &what, Preset::Ew), golden);
        let RowsOutput::Xlsx(bytes) =
            row_output(&q, &what, None, &RowsFormat::Xlsx(session.workbook_meta(None)))
                .unwrap()
                .output
        else {
            panic!("not a workbook");
        };
        assert!(!bytes.is_empty());
    }
    let blocks = q.report_blocks();
    let (_, block) = blocks
        .iter()
        .find(|(_, b)| ew_api::output::block_kind(b) == "parts_summary")
        .expect("the sample's parts list");
    let rows = ew_reports::kinds::PartsSummary.rows_in_mode(&q, ew_api::BomMode::Component);
    let set = ew_reports::arranged(&q, block, rows);
    let table = Table::from_rowset(&q, &set, "bom", "kind:parts_summary/parts", None);
    assert_eq!(
        ew_api::output::table_csv(&table, &CsvOptions::default()),
        ew_reports::csv::rows_csv(&q, &set)
    );
    // The quantities are numbers in their neutral form, the positions too.
    let qty = set.column("qty").unwrap();
    assert!(
        table
            .rows
            .iter()
            .all(|r| matches!(r.cells[qty], TableCell::Number(_)))
    );
}

/// A Dutch project's list in Excel is in Dutch, as its page (owner, 2026-11-27: people read the Excel file): its
/// headings and the program's words in its cells, its sizes and lengths numbers without a decimal sign; the same
/// list's CSV keeps the English headings and words, which programs read; an English project's workbook stays English.
/// The Dutch workbook of the I/O list reads back against its own words: unchanged, it applies nothing.
#[test]
fn a_dutch_projects_workbook_is_dutch_and_its_csv_english() {
    let english = crate::common::sample();
    let mut dutch = crate::common::sample();
    crate::common::run_cmd(
        &mut dutch,
        "project.settings.set",
        serde_json::json!({"member": "languages", "value": {"source": "nl-NL"}}),
    );
    let wires = RowExport::Kind("wire_list".to_owned());
    let sheet = |bytes: &[u8]| {
        read_sheet(
            bytes,
            ew_api::catalog::SourceFormat::Xlsx,
            None,
            &ew_api::catalog::table_limits(),
        )
        .unwrap()
    };
    let nl = sheet(&workbook(&dutch, &wires));
    for heading in ["Nr.", "Kleur", "Lengte mm"] {
        assert!(
            nl.headers.iter().any(|h| h == heading),
            "{heading}: {:?}",
            nl.headers
        );
    }
    assert!(!nl.headers.iter().any(|h| h == "Colour"), "{:?}", nl.headers);
    let cells: Vec<&String> = nl.rows.iter().flatten().collect();
    assert!(
        cells.iter().any(|c| c.starts_with("huls ")),
        "the program's words in Dutch"
    );
    assert!(
        cells.iter().any(|c| *c == "2.5") && !cells.iter().any(|c| *c == "2,5"),
        "numbers stay numbers"
    );
    let en = sheet(&workbook(&english, &wires));
    assert!(en.headers.iter().any(|h| h == "Colour"), "{:?}", en.headers);
    let first = |text: String| text.lines().next().unwrap_or_default().to_owned();
    let csv_line = first(csv(&dutch, &wires, Preset::ExcelEn));
    assert!(
        csv_line.contains("Colour") && !csv_line.contains("Kleur"),
        "{csv_line}"
    );
    assert!(
        !csv(&dutch, &wires, Preset::ExcelEn).contains("huls "),
        "the CSV's words stay English"
    );
    let io = RowExport::Kind("io_list".to_owned());
    let import = dutch
        .table_import(
            &workbook(&dutch, &io),
            &ew_api::output::ImportTableOptions::default(),
        )
        .expect("the Dutch workbook reads back");
    assert!(import.command.is_none(), "{:?}", import.cells);
}
