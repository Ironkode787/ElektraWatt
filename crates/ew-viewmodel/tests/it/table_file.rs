// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.08 part 3 (D14, ID-1224) on copies of Golden A: `navigator.export_table` writes the Terminal strips table on
//! show byte for byte as `ew export table strips --strip -X1031` writes it (TW-1; the CLI's path, run here through the
//! same `ew-api` calls on a reader session of the saved project), `navigator.import_table` of a file with five changed
//! labels opens the card with "5 changed" and Accept commits one undo step, Escape nothing (TW-1); a cell changed in
//! the project and in the file is one conflict and Accept keeps the project's value (TW-2, ruling 7); the export is a
//! job on the snapshot, and a cancel leaves no file (TW-3); the refusals in words.

use std::path::Path;

use crate::common;
use common::{Tmp, done, refused};
use ew_viewmodel::ew_api;
use ew_viewmodel::ew_api::ew_jobs::Gate;
use ew_viewmodel::ew_api::output::{NavKind, NavRole};
use ew_viewmodel::ew_api::zip::{Method, ZipLimits, read as unzip, write as zip};
use ew_viewmodel::script::card_summary;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

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

#[track_caller]
fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action}: {other:?}"),
    }
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
/// filter shows no row of another strip, so ruling 8's narrowing is not needed); a file with five changed labels opens
/// the card with "5 changed"; Escape commits nothing; Accept commits one `fields.set_many`, one undo step.
#[test]
fn tw1_the_export_is_the_clis_and_an_accepted_import_is_one_undo_step() {
    let tmp = Tmp::new("tw1");
    let mut shell = strips(&tmp);
    // Every row on show sits under the strip -X1031 (ruling 8).
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
    assert_eq!(shell.toast().unwrap().text, format!("Exported x.xlsx: {kib} KiB"));

    // Five labels changed in the file.
    let labels = ["Valve 1", "Valve 2", "Valve 3", "Valve 4", "Valve 5"];
    let edits: Vec<(usize, &str)> = labels.iter().enumerate().map(|(i, l)| (i + 2, *l)).collect();
    let file = tmp.0.join("y.xlsx");
    std::fs::write(&file, edited(&bytes, "Function text", &edits)).unwrap();
    let before = steps(&shell);
    done(&mut shell, "navigator.import_table", json!({"path": text(&file)}));
    assert_eq!(
        card_summary(&shell).unwrap(),
        "Import y.xlsx · 5 changed, 0 kept, 0 conflicts, 0 refused, 0 unknown, 0 missing"
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
        "Imported 5 changed cells from y.xlsx"
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

/// TW-2 as ruling 7 narrows it: a label changed in the project since the export and in the file is "1 conflict", and
/// Accept keeps the project's value (no step).
#[test]
fn tw2_a_conflict_keeps_the_projects_value() {
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
        "Import y.xlsx · 0 changed, 0 kept, 1 conflict: the project's value is kept, 0 refused, 0 unknown, 0 missing"
    );
    let d = shell.describe();
    assert!(d.contains("\"EW 3\" \"File 3\" conflict"), "{d}");
    done(&mut shell, "preview.apply", json!({}));
    assert!(shell.properties().card().is_none());
    assert_eq!(steps(&shell), before, "nothing is committed");
    let now = tmp.0.join("now.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&now)}));
    assert_eq!(
        column_cells(&std::fs::read(&now).unwrap(), "Function text")[2],
        "EW 3"
    );
    shell.shutdown();
}

/// TW-3: an edit while the export job runs applies at once and the file holds the revision the job started from; a
/// cancel leaves no file and no sibling; a second export while one runs is refused.
#[test]
fn tw3_the_export_is_a_job_on_its_snapshot() {
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
        "Import x.csv · 0 changed, 0 kept, 0 conflicts, 0 refused, 0 unknown, 0 missing"
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

/// The import card's describe line reaches the words of `Reach::TableFile` (ruling 9).
#[test]
fn the_import_card_reaches_its_words() {
    let tmp = Tmp::new("tfw");
    let mut shell = strips(&tmp);
    let out = tmp.0.join("x.xlsx");
    done(&mut shell, "navigator.export_table", json!({"path": text(&out)}));
    let file = tmp.0.join("y.xlsx");
    let bytes = std::fs::read(&out).unwrap();
    std::fs::write(&file, edited(&bytes, "Function text", &[(2, "Valve")])).unwrap();
    done(&mut shell, "navigator.import_table", json!({"path": text(&file)}));
    let tokens: Vec<String> = shell
        .describe()
        .lines()
        .flat_map(common::structural_tokens)
        .collect();
    for w in ew_viewmodel::describe::words(ew_viewmodel::describe::Reach::TableFile) {
        assert!(tokens.iter().any(|t| t == w), "{w}");
    }
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

/// ID-1224 ruling 3 (ID-1231): the export is cut to the rows and columns on show: a hidden column is not in the
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
        "Imported 1 changed cell from one.xlsx"
    );
    shell.shutdown();
}

/// Ruling 6 (ID-1231): a project changed after the import's preview refuses its Apply in words, and the card stays.
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

/// ID-1231 (ID-1224 ruling 2's "shown in table mode"): with the canvas focused, the one navigator shown in table mode
/// is the one exported; with two shown and none focused, the refusal says to click one; the focused one decides.
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
        "Several tables are shown; click in the one to export or import."
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
    assert!(other.exists());
    shell.shutdown();
}
