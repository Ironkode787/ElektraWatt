// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On copies of Golden A: `navigator.export_table` writes the Terminal strips table on
//! show byte for byte as `ew export table strips --strip -X1031` writes it (TW-1; the CLI's path, run here through the
//! same `ew-api` calls on a reader session of the saved project), `navigator.import_table` of a file with five changed
//! labels opens the card with "5 changed" and Accept commits one undo step, Escape nothing (TW-1); a cell changed in
//! the project and in the file is one conflict and Accept keeps the project's value (TW-2); the export is a
//! job on the snapshot, and a cancel leaves no file (TW-3); the refusals in words.

use std::path::Path;

use crate::common;
use common::{Tmp, done, failed, refused};
use ew_viewmodel::ew_api;
use ew_viewmodel::ew_api::ew_jobs::Gate;
use ew_viewmodel::ew_api::output::{NavKind, NavRole};
use ew_viewmodel::ew_api::zip::{Method, ZipLimits, read as unzip, write as zip};
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::script::card_summary;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A writable copy of Golden A, headless, with the Terminal strips table on show, filtered `-X1031`, in node order.
fn strips(tmp: &Tmp) -> ShellModel {
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "strips", "on": true}),
    );
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "strips", "text": "-X1031"}),
    );
    done(
        &mut shell,
        "navigator.sort",
        json!({"kind": "strips", "by": "order"}),
    );
    shell
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The workbook `ew export table strips --strip -X1031` writes without `SOURCE_DATE_EPOCH`, by the CLI's own calls
/// (`bin/ew/src/cli.rs`, `run_export_nav_table`) on a reader session of the project at `dir`.
fn cli_workbook(dir: &Path) -> Vec<u8> {
    let (session, _) = ew_api::Session::open_reader(dir, "0.0.0-test", "export").unwrap();
    let key = session.navigator_table_key("strips", Some("-X1031")).unwrap();
    let table = session.table_export(&key).unwrap();
    let tables = std::slice::from_ref(&table);
    let meta = ew_api::output::with_base_digest(&session.workbook_meta(None), tables);
    ew_api::output::xlsx(tables, &meta).unwrap()
}

/// The parts of a workbook: (name, text).
fn parts(bytes: &[u8]) -> Vec<(String, String)> {
    unzip(bytes, &ZipLimits::default())
        .unwrap()
        .into_iter()
        .map(|e| (e.name, String::from_utf8(e.data).unwrap()))
        .collect()
}

/// The first part of `a` that differs from `b`'s, with the first differing byte's offset and the text around it.
fn first_difference(a: &[u8], b: &[u8]) -> Option<String> {
    let (a, b) = (parts(a), parts(b));
    for (i, (name, text)) in a.iter().enumerate() {
        let Some((other_name, other)) = b.get(i) else {
            return Some(format!("part {name} has no counterpart"));
        };
        if name != other_name || text != other {
            let at = text
                .bytes()
                .zip(other.bytes())
                .position(|(x, y)| x != y)
                .unwrap_or(text.len().min(other.len()));
            let near = |t: &str| {
                t.get(at.saturating_sub(40)..(at + 80).min(t.len()))
                    .unwrap_or("")
                    .to_owned()
            };
            return Some(format!(
                "part {name} / {other_name} at byte {at}: window {:?}, cli {:?}",
                near(text),
                near(other)
            ));
        }
    }
    (a.len() != b.len()).then(|| "the part counts differ".to_owned())
}

/// The cell reference letters of the first sheet's column headed `header`.
fn column_letter(bytes: &[u8], header: &str) -> char {
    let limits = ew_api::catalog::table_limits();
    let sheet =
        ew_api::output::sheet::read_sheet(bytes, ew_api::catalog::SourceFormat::Xlsx, None, &limits).unwrap();
    let c = sheet
        .headers
        .iter()
        .position(|h| h == header)
        .unwrap_or_else(|| panic!("no column {header}"));
    char::from(b'A' + u8::try_from(c).unwrap())
}

/// The first data rows of the first sheet: the cells of `header`, in sheet order.
fn column_cells(bytes: &[u8], header: &str) -> Vec<String> {
    let limits = ew_api::catalog::table_limits();
    let sheet =
        ew_api::output::sheet::read_sheet(bytes, ew_api::catalog::SourceFormat::Xlsx, None, &limits).unwrap();
    let c = sheet.headers.iter().position(|h| h == header).unwrap();
    sheet
        .rows
        .iter()
        .map(|r| r.get(c).cloned().unwrap_or_default())
        .collect()
}

/// The sheet XML `sheet` with cell `<col><row>` set to the inline string `value` (replaced, or inserted in column
/// order where the writer left the empty cell out), as a spreadsheet program saves an edit.
fn set_cell(sheet: &str, col: char, row: usize, value: &str) -> String {
    let start = sheet.find(&format!("<row r=\"{row}\"")).unwrap();
    let open_end = start + sheet[start..].find('>').unwrap() + 1;
    let end = start + sheet[start..].find("</row>").unwrap();
    let at = format!("{col}{row}");
    let mut cells: Vec<String> = sheet[open_end..end]
        .split("<c r=\"")
        .filter(|s| !s.is_empty())
        .map(|s| format!("<c r=\"{s}"))
        .filter(|c| !c.starts_with(&format!("<c r=\"{at}\"")))
        .collect();
    let new = format!("<c r=\"{at}\" t=\"inlineStr\"><is><t>{value}</t></is></c>");
    let place = cells
        .iter()
        .position(|c| c.as_bytes()[6] > col as u8)
        .unwrap_or(cells.len());
    cells.insert(place, new);
    format!("{}{}{}", &sheet[..open_end], cells.concat(), &sheet[end..])
}

/// The workbook `bytes` with the cells `edits` (sheet row, value) of the column headed `header` changed in its first
/// sheet, every other part as it was (the hidden `_ew` and `_ew_base` sheets kept).
fn edited(bytes: &[u8], header: &str, edits: &[(usize, &str)]) -> Vec<u8> {
    let col = column_letter(bytes, header);
    let files: Vec<(String, Vec<u8>)> = parts(bytes)
        .into_iter()
        .map(|(name, mut text)| {
            if name == "xl/worksheets/sheet1.xml" {
                for (row, value) in edits {
                    text = set_cell(&text, col, *row, value);
                }
            }
            (name, text.into_bytes())
        })
        .collect();
    let entries: Vec<(&str, Method, &[u8])> = files
        .iter()
        .map(|(n, d)| (n.as_str(), Method::Deflated, d.as_slice()))
        .collect();
    zip(&entries).unwrap()
}

/// The `_ew` sheet's value of `key` in workbook `bytes`.
fn ew_value(bytes: &[u8], key: &str) -> String {
    let limits = ew_api::catalog::table_limits();
    ew_api::output::sheet::read_rows(bytes, ew_api::catalog::SourceFormat::Xlsx, "_ew", &limits)
        .unwrap()
        .into_iter()
        .find(|r| r.first().is_some_and(|c| c == key))
        .and_then(|r| r.get(1).cloned())
        .unwrap()
}

fn steps(shell: &ShellModel) -> usize {
    shell.recorded_commands().len()
}

/// TW-1: the table on show filtered to -X1031 in node order is the CLI's `--strip -X1031` workbook byte for byte (the
/// filter shows no row of another strip, so no narrowing is needed); a file with five changed labels opens
/// the card with "5 changed"; Escape commits nothing; Accept commits one `fields.set_many`, one undo step.
#[test]
fn the_export_is_the_clis_and_an_accepted_import_is_one_undo_step() {
    let tmp = Tmp::new("tw1");
    let mut shell = strips(&tmp);
    // Every row on show sits under the strip -X1031.
    let n = shell.nav(NavKind::Strips).unwrap();
    let table = n.table().unwrap();
    let parents: Vec<Option<u32>> = n
        .visible()
        .iter()
        .map(|r| table.nodes[r.node as usize].parent)
        .collect();
    let strip = parents[0].unwrap() as usize;
    assert_eq!(table.nodes[strip].role, NavRole::Strip);
    assert_eq!(table.nodes[strip].label, "-X1031");
    assert!(
        parents.iter().all(|p| *p == Some(strip as u32)),
        "the filter -X1031 shows rows of another strip"
    );
    assert_eq!(parents.len(), 20, "the 20 terminals of -X1031");

    let out = tmp.0.join("x.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    let bytes = std::fs::read(&out).unwrap();
    let cli = cli_workbook(&tmp.golden_copy());
    assert_eq!(first_difference(&bytes, &cli), None);
    assert_eq!(
        bytes, cli,
        "byte-equal to `ew export table strips --strip -X1031`"
    );
    done(&mut shell, "jobs.show", json!({}));
    let kib = bytes.len().div_ceil(1024);
    let rev = shell.revision().unwrap();
    let d = shell.describe();
    assert!(
        d.contains(&format!("  job 1 table \"x.xlsx\" done kib {kib} rev {rev}\n")),
        "the job's end names no pages: {d}"
    );
    assert_eq!(
        shell.toast().unwrap().text,
        format!("Exported x.xlsx: {kib} KiB.")
    );

    // Five labels changed in the file.
    let labels = ["Valve 1", "Valve 2", "Valve 3", "Valve 4", "Valve 5"];
    let edits: Vec<(usize, &str)> = labels.iter().enumerate().map(|(i, l)| (i + 2, *l)).collect();
    let file = tmp.0.join("y.xlsx");
    std::fs::write(&file, edited(&bytes, "Function text", &edits)).unwrap();
    let before = steps(&shell);
    done(&mut shell, "navigator.import_table", json!({"path": text(&file)}));
    assert_eq!(
        card_summary(&shell).unwrap(),
        "Import y.xlsx · 5 cells changed, 0 cells kept, 0 conflicts, 0 cells refused, 0 unknown rows, 0 rows not in \
         the file"
    );
    // A cell line says what Apply will do, never "applied" before it.
    let ew_viewmodel::properties::CardOrigin::TableImport(c) = &shell.properties().card().unwrap().origin
    else {
        panic!("a table import card");
    };
    let first = &c.import.cells[0];
    assert_eq!(
        ew_viewmodel::properties::TableImportCard::cell_line(first),
        format!(
            "-X1031:1 · Function text · \"{}\" → \"Valve 1\" · Apply takes the file's value",
            first.ew
        )
    );
    let d = shell.describe();
    assert!(
        d.contains(
            "card ready \"fields.set_many\" table-import \"y.xlsx\" changed 5 kept 0 conflicts 0 refused 0 unknown 0 \
             missing 0"
        ),
        "{d}"
    );
    assert!(d.contains("  cell \"-X1031:1\" \"Function text\" "), "{d}");
    assert!(d.contains("\"Valve 1\" applied"), "{d}");
    // Escape commits nothing.
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.properties().card().is_none());
    assert_eq!(steps(&shell), before);

    done(&mut shell, "navigator.import_table", json!({"path": text(&file)}));
    done(&mut shell, "preview.apply", json!({}));
    assert!(shell.properties().card().is_none());
    assert_eq!(steps(&shell), before + 1, "one step");
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "fields.set_many");
    assert_eq!(
        shell.toast().unwrap().text,
        "Imported 5 changed cells from y.xlsx."
    );
    let again = tmp.0.join("x2.xlsx");
    done(
        &mut shell,
        "navigator.export_table",
        json!({"path": text(&again)}),
    );
    assert_eq!(
        column_cells(&std::fs::read(&again).unwrap(), "Function text")[..5],
        labels
    );
    // One undo step restores the five.
    let old = column_cells(&bytes, "Function text");
    done(&mut shell, "edit.undo", json!({}));
    let undone = tmp.0.join("x3.xlsx");
    done(
        &mut shell,
        "navigator.export_table",
        json!({"path": text(&undone)}),
    );
    assert_eq!(
        column_cells(&std::fs::read(&undone).unwrap(), "Function text"),
        old
    );
    shell.shutdown();
}

/// TW-2 restored: a label changed in the project since the export and in the file is "1 conflict",
/// which keeps the project's value, so the file changes nothing and Apply is refused; with "Take the file's
/// values" on, the bytes the card keeps are read again and Accept applies the file's value in one step.
#[test]
fn a_conflict_keeps_the_projects_value_until_the_switch_takes_the_files() {
    let tmp = Tmp::new("tw2");
    let mut shell = strips(&tmp);
    let out = tmp.0.join("x.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    let bytes = std::fs::read(&out).unwrap();
    // The project's change since the export: terminal 3's label, through an import of its own.
    let ew = tmp.0.join("ew.xlsx");
    std::fs::write(&ew, edited(&bytes, "Function text", &[(4, "EW 3")])).unwrap();
    done(&mut shell, "navigator.import_table", json!({"path": text(&ew)}));
    done(&mut shell, "preview.apply", json!({}));
    let before = steps(&shell);
    let file = tmp.0.join("y.xlsx");
    std::fs::write(&file, edited(&bytes, "Function text", &[(4, "File 3")])).unwrap();
    done(&mut shell, "navigator.import_table", json!({"path": text(&file)}));
    assert_eq!(
        card_summary(&shell).unwrap(),
        "Import y.xlsx · 0 cells changed, 0 cells kept, 1 conflict: the project's value is kept, 0 cells refused, 0 \
         unknown rows, 0 rows not in the file"
    );
    let d = shell.describe();
    assert!(d.contains("\"EW 3\" \"File 3\" conflict"), "{d}");
    assert_eq!(
        refused(&mut shell, "preview.apply", json!({})),
        "reason-table-import-nothing"
    );
    assert_eq!(
        ew_viewmodel::strings::text(ew_viewmodel::key!("reason-table-import-nothing")),
        "The file changes nothing in the project"
    );
    assert!(shell.properties().card().is_some(), "the card stays");
    assert_eq!(steps(&shell), before, "nothing is committed");
    // The switch: the file's value is taken.
    done(&mut shell, "preview.take_file", json!({"on": true}));
    assert_eq!(
        card_summary(&shell).unwrap(),
        "Import y.xlsx · 1 cell changed, 0 cells kept, 1 conflict: the file's value is taken, 0 cells refused, 0 \
         unknown rows, 0 rows not in the file"
    );
    done(&mut shell, "preview.apply", json!({}));
    assert!(shell.properties().card().is_none());
    assert_eq!(steps(&shell), before + 1, "one step");
    let now = tmp.0.join("now.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&now)}));
    assert_eq!(
        column_cells(&std::fs::read(&now).unwrap(), "Function text")[2],
        "File 3"
    );
    shell.shutdown();
}

/// TW-3: an edit while the export job runs applies at once and the file holds the revision the job started from; a
/// cancel leaves no file and no sibling; a second export while one runs is refused.
#[test]
fn the_export_is_a_job_on_its_snapshot() {
    let tmp = Tmp::new("tw3");
    let mut shell = strips(&tmp);
    let gate = Gate::new();
    gate.hold("rename");
    shell.set_job_gate_for_tests(Some(gate.clone()));
    let out = tmp.0.join("x.xlsx");
    let rev = shell.revision().unwrap();
    done(
        &mut shell,
        "navigator.export_table",
        json!({"path": text(&out), "wait": false}),
    );
    gate.wait_reached("rename");
    assert_eq!(
        refused(
            &mut shell,
            "navigator.export_table",
            json!({"path": text(&tmp.0.join("z.xlsx"))})
        ),
        "reason-export-running"
    );
    done(
        &mut shell,
        "page.new",
        json!({"folio": "999", "type": "multi_line", "title": "Meanwhile"}),
    );
    assert_ne!(shell.revision().unwrap(), rev, "the edit applied at once");
    gate.release();
    done(&mut shell, "jobs.wait", json!({}));
    assert_eq!(ew_value(&std::fs::read(&out).unwrap(), "rev"), rev.to_string());

    let gate = Gate::new();
    gate.hold("rename");
    shell.set_job_gate_for_tests(Some(gate.clone()));
    let second = tmp.0.join("cancelled.csv");
    done(
        &mut shell,
        "navigator.export_table",
        json!({"path": text(&second), "wait": false}),
    );
    gate.wait_reached("rename");
    let sibling = tmp.0.join(".cancelled.csv.ew-part");
    assert!(sibling.exists(), "the bytes are in the hidden sibling");
    done(&mut shell, "jobs.cancel", json!({}));
    gate.release();
    done(&mut shell, "jobs.wait", json!({}));
    assert!(!second.exists(), "no file");
    assert!(!sibling.exists(), "no sibling");
    shell.shutdown();
}

/// The refusals: a tree, the Messages pane, a preset of a workbook and an unknown preset, an existing file without
/// `replace`, an unreadable file, a file of no table; a CSV by its name, in its preset.
#[test]
fn the_refusals_in_words_and_a_csv() {
    let tmp = Tmp::new("tfr");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    let out = tmp.0.join("x.xlsx");
    assert_eq!(
        refused(&mut shell, "navigator.export_table", json!({"path": text(&out)})),
        "reason-table-off"
    );
    assert_eq!(
        refused(&mut shell, "navigator.import_table", json!({"path": text(&out)})),
        "reason-table-off"
    );
    done(&mut shell, "messages.show", json!({"on": true}));
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "messages"}),
    );
    assert_eq!(
        failed(&mut shell, "navigator.export_table", json!({"path": text(&out)})),
        "The messages pane has no table mode; its rows are findings."
    );
    let mut shell = strips(&tmp);
    assert_eq!(
        failed(
            &mut shell,
            "navigator.export_table",
            json!({"path": text(&out), "preset": "excel-nl"})
        ),
        "A preset applies to CSV only; drop it, or choose the format csv."
    );
    assert_eq!(
        failed(
            &mut shell,
            "navigator.export_table",
            json!({"path": text(&out), "format": "csv", "preset": "excel"})
        ),
        "There is no CSV preset excel; the presets are ew, excel-nl, excel-en and rfc4180."
    );
    let csv = tmp.0.join("x.csv");
    done(
        &mut shell,
        "navigator.export_table",
        json!({"path": text(&csv), "preset": "excel-nl"}),
    );
    let written = std::fs::read(&csv).unwrap();
    assert!(written.starts_with(b"\xEF\xBB\xBFid;"), "excel-nl: a BOM and `;`");
    assert_eq!(
        refused(&mut shell, "navigator.export_table", json!({"path": text(&csv)})),
        "reason-file-exists"
    );
    // A CSV re-imports against the view's table, with nothing changed.
    done(&mut shell, "navigator.import_table", json!({"path": text(&csv)}));
    assert_eq!(
        card_summary(&shell).unwrap(),
        "Import x.csv · Nothing to import: the file matches the project."
    );
    done(&mut shell, "preview.cancel", json!({}));
    let missing = tmp.0.join("none.xlsx");
    let message = failed(
        &mut shell,
        "navigator.import_table",
        json!({"path": text(&missing)}),
    );
    assert_eq!(message, "There is no file none.xlsx.");
    let junk = tmp.0.join("junk.xlsx");
    std::fs::write(&junk, b"PK\x03\x04 not a workbook").unwrap();
    let message = failed(&mut shell, "navigator.import_table", json!({"path": text(&junk)}));
    assert_eq!(
        message,
        "The file cannot be read: malformed input: not a readable XLSX workbook: no ZIP central directory."
    );
    shell.shutdown();
}

/// A CSV export with no preset takes `excel-en`, which keeps the `id` column, so the file re-imports with
/// nothing changed and no refusal; the preset `ew`, which writes no `id` column, is refused in words.
#[test]
fn a_csv_export_keeps_its_id_column_and_ew_is_refused() {
    let tmp = Tmp::new("tfe");
    let mut shell = strips(&tmp);
    let csv = tmp.0.join("x.csv");
    done(&mut shell, "navigator.export_table", json!({"path": text(&csv)}));
    let written = std::fs::read(&csv).unwrap();
    assert!(
        written.starts_with(b"\xEF\xBB\xBFid,"),
        "excel-en: a BOM, then `id` and `,`"
    );
    done(&mut shell, "navigator.import_table", json!({"path": text(&csv)}));
    assert_eq!(
        card_summary(&shell).unwrap(),
        "Import x.csv · Nothing to import: the file matches the project."
    );
    done(&mut shell, "preview.cancel", json!({}));
    let other = tmp.0.join("y.csv");
    assert_eq!(
        refused(
            &mut shell,
            "navigator.export_table",
            json!({"path": text(&other), "preset": "ew"})
        ),
        "reason-table-preset-ew"
    );
    assert_eq!(
        ew_viewmodel::strings::text(ew_viewmodel::key!("reason-table-preset-ew")),
        "The preset ew writes no id column, so the file cannot be imported again; choose excel-nl or excel-en"
    );
    assert!(!other.exists(), "no file");
    shell.shutdown();
}

/// The node keys of the Terminal strips rows on show, in the order on show.
fn shown_rows(shell: &ShellModel) -> Vec<String> {
    let n = shell.nav(NavKind::Strips).unwrap();
    let table = n.table().unwrap();
    n.visible()
        .iter()
        .map(|r| table.nodes[r.node as usize].key.clone())
        .collect()
}

/// The first sheet's column headers of workbook `bytes`.
fn headers(bytes: &[u8]) -> Vec<String> {
    let limits = ew_api::catalog::table_limits();
    ew_api::output::sheet::read_sheet(bytes, ew_api::catalog::SourceFormat::Xlsx, None, &limits)
        .unwrap()
        .headers
}

/// The export is cut to the rows and columns on show: a hidden column is not in the
/// header, `id` is kept, only the rows on show are written, in the order on show; the cut file re-imports three-way
/// with no notice, and one changed cell is the toast's "1 changed cell".
#[test]
fn the_export_is_cut_to_the_rows_and_columns_on_show() {
    let tmp = Tmp::new("tfc");
    let mut shell = strips(&tmp);
    let all = shown_rows(&shell);
    // The Pneumatics terminals of -X1031 (1 to 4 and 11 to 14), the last first.
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "strips", "text": "-X1031 Pneumatics"}),
    );
    done(
        &mut shell,
        "navigator.sort",
        json!({"kind": "strips", "by": "order", "descending": true}),
    );
    let bridge = {
        let t = shell.nav(NavKind::Strips).unwrap().table().unwrap();
        t.columns
            .iter()
            .find(|c| c.label == "Bridge")
            .unwrap()
            .key
            .clone()
    };
    done(
        &mut shell,
        "table.columns",
        json!({"kind": "strips", "hide": [bridge]}),
    );
    let rows = shown_rows(&shell);
    assert_eq!(rows.len(), 8, "the filter shows a subset: {rows:?}");
    let mut in_table_order: Vec<String> = all.iter().filter(|k| rows.contains(k)).cloned().collect();
    in_table_order.reverse();
    assert_eq!(rows, in_table_order, "the order on show is the table's, reversed");
    let out = tmp.0.join("cut.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    let bytes = std::fs::read(&out).unwrap();
    let header = headers(&bytes);
    assert!(!header.iter().any(|h| h == "Bridge"), "{header:?}");
    assert!(header.iter().any(|h| h == "Function text"), "{header:?}");
    assert_eq!(header[0], "id");
    assert_eq!(
        column_cells(&bytes, "id"),
        rows,
        "the rows on show, in the order on show"
    );

    let file = tmp.0.join("one.xlsx");
    std::fs::write(&file, edited(&bytes, "Function text", &[(2, "Valve")])).unwrap();
    done(&mut shell, "navigator.import_table", json!({"path": text(&file)}));
    let card = shell.properties().card().unwrap();
    let ew_viewmodel::properties::CardOrigin::TableImport(c) = &card.origin else {
        panic!("a table import card");
    };
    assert!(c.import.three_way, "three-way against the cut's base");
    assert_eq!(c.import.notices, Vec::<String>::new());
    assert_eq!(c.import.counts().changed, 1);
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(
        shell.toast().unwrap().text,
        "Imported 1 changed cell from one.xlsx."
    );
    shell.shutdown();
}

/// Every file below `dir` with its bytes, by relative path.
fn files_of(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// SE-9 at the call site: the window's table read panics inside its guard (the test fault
/// `table-read`); `navigator.import_table` fails with S2 whole, opens no card, records no command and leaves the
/// project's files as they were.
#[test]
fn a_panicking_table_read_fails_with_s2_and_changes_nothing() {
    let tmp = Tmp::new("tfp");
    let mut shell = strips(&tmp);
    let out = tmp.0.join("x.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    let file = tmp.0.join("y.xlsx");
    std::fs::copy(&out, &file).unwrap();
    let before = (steps(&shell), files_of(&tmp.golden_copy()));
    shell.set_job_panic_for_tests(Some("table-read"));
    assert_eq!(
        failed(&mut shell, "navigator.import_table", json!({"path": text(&file)})),
        "The table import stopped on an internal error while reading y.xlsx; nothing was changed."
    );
    assert!(shell.properties().card().is_none(), "no card");
    assert!(
        (steps(&shell), files_of(&tmp.golden_copy())) == before,
        "the import changed the project"
    );
    shell.shutdown();
}

/// Ruling 6: a project changed after the import's preview refuses its Apply in words, and the card stays.
#[test]
fn a_stale_import_is_refused_and_the_card_stays() {
    let tmp = Tmp::new("tfs");
    let mut shell = strips(&tmp);
    let out = tmp.0.join("x.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    let file = tmp.0.join("y.xlsx");
    let bytes = std::fs::read(&out).unwrap();
    std::fs::write(&file, edited(&bytes, "Function text", &[(2, "Valve")])).unwrap();
    done(&mut shell, "navigator.import_table", json!({"path": text(&file)}));
    let before = steps(&shell);
    done(
        &mut shell,
        "page.new",
        json!({"folio": "999", "type": "multi_line", "title": "Meanwhile"}),
    );
    assert_eq!(
        refused(&mut shell, "preview.apply", json!({})),
        "reason-table-import-stale"
    );
    assert!(shell.properties().card().is_some(), "the card stays");
    assert_eq!(steps(&shell), before + 1, "only the new page");
    shell.shutdown();
}

/// With the canvas focused, the one navigator shown in table mode
/// is the one exported; with two shown (Devices beside Pages, Terminal strips moved to the bottom dock) and none
/// focused, the refusal says to click one; the focused one decides.
#[test]
fn the_table_shown_decides_without_the_focus() {
    let tmp = Tmp::new("tfn");
    let mut shell = strips(&tmp);
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    assert_eq!(shell.focused_navigator(), None);
    let out = tmp.0.join("x.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    assert_eq!(
        column_cells(&std::fs::read(&out).unwrap(), "id"),
        shown_rows(&shell),
        "the Terminal strips table"
    );

    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "devices", "on": true}),
    );
    // Terminal strips behind the Devices tab moves to the bottom dock, so both tables are shown.
    let strips_pane = PaneKind::Navigator(NavKind::Strips);
    let moved = shell.layout().without(strips_pane).with_bottom(strips_pane);
    shell.set_layout(moved, 1.0);
    assert_eq!(shell.layout().shown_in_place_of(strips_pane), Some(strips_pane));
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    let other = tmp.0.join("y.xlsx");
    for action in ["navigator.export_table", "navigator.import_table"] {
        assert_eq!(
            refused(&mut shell, action, json!({"path": text(&other)})),
            "reason-table-which"
        );
    }
    assert_eq!(
        ew_viewmodel::strings::text(ew_viewmodel::key!("reason-table-which")),
        "Several tables are shown; click in the one to export or import"
    );
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "strips"}),
    );
    done(
        &mut shell,
        "navigator.export_table",
        json!({"path": text(&other)}),
    );
    // The focused Terminal strips table is the one written.
    assert_eq!(
        column_cells(&std::fs::read(&other).unwrap(), "id"),
        shown_rows(&shell),
        "the focused Terminal strips table"
    );
    shell.shutdown();
}

/// A table-mode navigator behind another tab is not shown and does not count; with the canvas
/// focused, the one table shown is exported.
#[test]
fn a_table_behind_another_tab_does_not_count() {
    let tmp = Tmp::new("tfb");
    let mut shell = strips(&tmp);
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "devices", "on": true}),
    );
    let strips_pane = PaneKind::Navigator(NavKind::Strips);
    assert_eq!(
        shell.layout().shown_in_place_of(strips_pane),
        Some(PaneKind::Navigator(NavKind::Devices)),
        "Terminal strips is behind the Devices tab"
    );
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    let out = tmp.0.join("x.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    let n = shell.nav(NavKind::Devices).unwrap();
    let table = n.table().unwrap();
    let devices: Vec<String> = n
        .visible()
        .iter()
        .map(|r| table.nodes[r.node as usize].key.clone())
        .collect();
    assert_eq!(
        column_cells(&std::fs::read(&out).unwrap(), "id"),
        devices,
        "the Devices table"
    );
    shell.shutdown();
}

/// F6 onto a navigator behind another tab brings its tab to the front of its pane, so the focused
/// table is the shown one, and Export table writes its rows; `ui.focus` does the same.
#[test]
fn focus_brings_a_navigator_behind_another_tab_forward() {
    let tmp = Tmp::new("tff");
    let mut shell = strips(&tmp);
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "devices", "on": true}),
    );
    let strips_pane = PaneKind::Navigator(NavKind::Strips);
    let devices_pane = PaneKind::Navigator(NavKind::Devices);
    assert_eq!(
        shell.layout().shown_in_place_of(strips_pane),
        Some(devices_pane),
        "Terminal strips is behind the Devices tab"
    );
    done(&mut shell, "ui.focus", json!({"area": "commandbar"}));
    // F6 from the command bar reaches every open navigator within one step each, Pages included.
    for _ in 0..=NavKind::ALL.len() {
        if shell.focused_navigator() == Some(NavKind::Strips) {
            break;
        }
        done(&mut shell, "ui.focus_next", json!({}));
    }
    assert_eq!(
        shell.focused_navigator(),
        Some(NavKind::Strips),
        "F6 reaches Terminal strips"
    );
    assert_eq!(
        shell.layout().shown_in_place_of(strips_pane),
        Some(strips_pane),
        "F6 brought Terminal strips forward"
    );
    let out = tmp.0.join("f6.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    assert_eq!(
        column_cells(&std::fs::read(&out).unwrap(), "id"),
        shown_rows(&shell),
        "the Terminal strips table"
    );

    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "devices"}),
    );
    assert_eq!(shell.layout().shown_in_place_of(strips_pane), Some(devices_pane));
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "strips"}),
    );
    assert_eq!(
        shell.layout().shown_in_place_of(strips_pane),
        Some(strips_pane),
        "ui.focus brought Terminal strips forward"
    );
    shell.shutdown();
}

/// The Macros navigator in table mode, focused, refuses Export table with its reason, whole.
#[test]
fn a_table_without_a_file_form_refuses_its_export() {
    let tmp = Tmp::new("tfm");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "macros", "on": true}),
    );
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "macros"}),
    );
    let out = tmp.0.join("m.xlsx");
    let reason = refused(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    assert_eq!(
        ew_viewmodel::strings::text(ew_viewmodel::strings::Key::run_time(reason)),
        "This table cannot be written to a file; copy its cells instead"
    );
    assert!(!out.exists(), "no file");
    shell.shutdown();
}

/// An XLSX exported from Golden A, imported into a new project's Terminal strips table, fails with
/// the other project's error, whole.
#[test]
fn a_file_of_another_project_is_refused() {
    let tmp = Tmp::new("tfo");
    let mut shell = strips(&tmp);
    let out = tmp.0.join("a.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    shell.shutdown();
    let mut other = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config-new"), "0.0.0-test"),
    );
    done(
        &mut other,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(&mut other, "navigator.show", json!({"kind": "strips"}));
    done(
        &mut other,
        "navigator.table",
        json!({"kind": "strips", "on": true}),
    );
    assert_eq!(
        failed(&mut other, "navigator.import_table", json!({"path": text(&out)})),
        "The file was exported from another project; export the table from this project again."
    );
    other.shutdown();
}
