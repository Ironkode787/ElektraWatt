// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.08 part 2 through the `ew` binary: `ew export table` and `ew import table`, the round trip of a navigator's
//! table and of the I/O list (RT-1 to RT-6, D8, D11), over copies of Golden A.
//!
//! A workbook "saved by Excel" is the export with every inline string of every sheet moved into a shared-strings table
//! (`t="s"` cells, as Excel writes them), the hidden sheets kept, and the test's cells replaced.

use std::collections::BTreeMap;
use std::path::Path;

use ew_api::catalog::{SourceFormat, table_limits};
use ew_api::output::sheet::{read_rows, sheet_names};
use ew_api::zip::{Method, ZipLimits, read as unzip, write as zip};
use sha2::{Digest, Sha256};

use crate::tables::{Tmp, args, golden_file, sheet, stderr, stdout};

/// The columns of the Terminal strips table, as `ew export table strips` heads them.
const STRIP_HEADERS: [&str; 13] = [
    "id",
    "Strip",
    "Pos",
    "Terminal",
    "Function text",
    "Group",
    "Part",
    "Potential",
    "Side 1 (clamp a)",
    "Side 2 (clamp b)",
    "Bridge",
    "Bridge part",
    "State",
];

/// A cell an Excel-style save writes in the first sheet.
enum Put {
    /// A shared string.
    Text(&'static str),
    /// A number cell.
    Number(&'static str),
}

/// The column letter of column `c` (0 is `A`); the tables here have fewer than 27 columns.
fn letter(c: usize) -> char {
    assert!(c < 26, "column {c}");
    char::from(b'A' + u8::try_from(c).unwrap())
}

/// The cell reference of a header's column on sheet row `row` (1 is the header row).
fn cell_ref(headers: &[String], header: &str, row: usize) -> String {
    let c = headers
        .iter()
        .position(|h| h == header)
        .unwrap_or_else(|| panic!("no column {header}"));
    format!("{}{row}", letter(c))
}

/// The text between `start` and the next `end` after it.
fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let i = text.find(start).unwrap() + start.len();
    let j = text[i..].find(end).unwrap();
    &text[i..i + j]
}

/// The XML text of `s`.
fn escaped(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The digits of a cell reference.
fn row_of(reference: &str) -> &str {
    reference.trim_start_matches(|c: char| c.is_ascii_uppercase())
}

/// The column number of a cell reference (`A` 1).
fn column_of(reference: &str) -> u32 {
    reference
        .chars()
        .take_while(char::is_ascii_uppercase)
        .fold(0, |n, c| n * 26 + (u32::from(c) - u32::from('A') + 1))
}

/// One worksheet with every inline string moved into `strings` and `edits` (cell reference, value) put in.
fn shared_cells(xml: &str, edits: &[(String, Put)], strings: &mut Vec<String>) -> String {
    let mut shared = |reference: &str, style: &str, text: String| {
        strings.push(text);
        format!(
            "<c r=\"{reference}\"{style} t=\"s\"><v>{}</v></c>",
            strings.len() - 1
        )
    };
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<row ") {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let open = rest.find('>').unwrap() + 1;
        let end = rest.find("</row>").unwrap();
        let (head, mut body) = (&rest[..open], &rest[open..end]);
        let row = between(head, "r=\"", "\"").to_owned();
        let mut cells: Vec<(String, String)> = Vec::new();
        while let Some(c) = body.find("<c ") {
            let tag_end = body[c..].find('>').unwrap() + c;
            let stop = if body.as_bytes()[tag_end - 1] == b'/' {
                tag_end + 1
            } else {
                body[c..].find("</c>").unwrap() + c + 4
            };
            let cell = &body[c..stop];
            cells.push((between(cell, "r=\"", "\"").to_owned(), cell.to_owned()));
            body = &body[stop..];
        }
        for (reference, _) in edits.iter().filter(|(r, _)| row_of(r) == row) {
            if !cells.iter().any(|(r, _)| r == reference) {
                cells.push((reference.clone(), format!("<c r=\"{reference}\"/>")));
            }
        }
        cells.sort_by_key(|(r, _)| column_of(r));
        out.push_str(head);
        for (reference, cell) in cells {
            let open_tag = &cell[..cell.find('>').unwrap()];
            let style = if open_tag.contains(" s=\"") {
                format!(" s=\"{}\"", between(open_tag, " s=\"", "\""))
            } else {
                String::new()
            };
            let put = edits.iter().find(|(r, _)| *r == reference).map(|(_, p)| p);
            let new = match put {
                Some(Put::Number(n)) => format!("<c r=\"{reference}\"{style}><v>{n}</v></c>"),
                Some(Put::Text(t)) => shared(&reference, &style, escaped(t)),
                None if cell.contains("t=\"inlineStr\"") => {
                    let inner = between(&cell, "<is>", "</is>");
                    let text = if inner.starts_with("<t/>") {
                        String::new()
                    } else {
                        let t = &inner[inner.find('>').unwrap() + 1..];
                        t[..t.find("</t>").unwrap()].to_owned()
                    };
                    shared(&reference, &style, text)
                }
                None => cell,
            };
            out.push_str(&new);
        }
        out.push_str("</row>");
        rest = &rest[end + "</row>".len()..];
    }
    out.push_str(rest);
    out
}

/// The workbook as Excel saves it: every sheet's inline strings in `xl/sharedStrings.xml`, and `edits` in the first
/// sheet.
fn excel_save(bytes: &[u8], edits: &[(String, Put)]) -> Vec<u8> {
    let mut strings = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for e in unzip(bytes, &ZipLimits::default()).unwrap() {
        let text = String::from_utf8(e.data).unwrap();
        let data = if e.name.starts_with("xl/worksheets/") {
            let first = e.name == "xl/worksheets/sheet1.xml";
            shared_cells(&text, if first { edits } else { &[] }, &mut strings)
        } else if e.name == "xl/_rels/workbook.xml.rels" {
            text.replace(
                "</Relationships>",
                "<Relationship Id=\"rId99\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/\
                 relationships/sharedStrings\" Target=\"sharedStrings.xml\"/></Relationships>",
            )
        } else if e.name == "[Content_Types].xml" {
            text.replace(
                "</Types>",
                "<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/\
                 vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/></Types>",
            )
        } else {
            text
        };
        files.push((e.name, data.into_bytes()));
    }
    let items: String = strings
        .iter()
        .map(|s| format!("<si><t xml:space=\"preserve\">{s}</t></si>"))
        .collect();
    files.push((
        "xl/sharedStrings.xml".to_owned(),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<sst \
             xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{n}\" \
             uniqueCount=\"{n}\">{items}</sst>",
            n = strings.len()
        )
        .into_bytes(),
    ));
    let entries: Vec<(&str, Method, &[u8])> = files
        .iter()
        .map(|(n, d)| (n.as_str(), Method::Deflated, d.as_slice()))
        .collect();
    zip(&entries).unwrap()
}

/// The workbook with the first `from` of its part `name` replaced by `to`.
fn with_part(bytes: &[u8], name: &str, from: &str, to: &str) -> Vec<u8> {
    let files: Vec<(String, Vec<u8>)> = unzip(bytes, &ZipLimits::default())
        .unwrap()
        .into_iter()
        .map(|e| {
            if e.name == name {
                let text = String::from_utf8(e.data).unwrap();
                assert!(text.contains(from), "{name} holds no {from}");
                (e.name, text.replacen(from, to, 1).into_bytes())
            } else {
                (e.name, e.data)
            }
        })
        .collect();
    let entries: Vec<(&str, Method, &[u8])> = files
        .iter()
        .map(|(n, d)| (n.as_str(), Method::Deflated, d.as_slice()))
        .collect();
    zip(&entries).unwrap()
}

/// Every record of the project in `dir` with an `id`, by its ID (the `.jsonl` files outside hidden folders).
fn records(dir: &Path) -> BTreeMap<String, serde_json::Value> {
    fn walk(dir: &Path, out: &mut BTreeMap<String, serde_json::Value>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let path = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|x| x == "jsonl") {
                for line in std::fs::read_to_string(&path).unwrap().lines() {
                    let v: serde_json::Value = serde_json::from_str(line).unwrap();
                    if let Some(id) = v["id"].as_str() {
                        out.insert(id.to_owned(), v);
                    }
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, &mut out);
    out
}

/// What changed between two record sets: (ID, member, value after), a removed record as the member `$removed`.
fn changes(
    before: &BTreeMap<String, serde_json::Value>,
    after: &BTreeMap<String, serde_json::Value>,
) -> Vec<(String, String, serde_json::Value)> {
    let mut out = Vec::new();
    for (id, b) in before {
        let Some(a) = after.get(id) else {
            out.push((id.clone(), "$removed".to_owned(), serde_json::Value::Null));
            continue;
        };
        let (Some(bo), Some(ao)) = (b.as_object(), a.as_object()) else {
            continue;
        };
        for key in bo.keys().chain(ao.keys().filter(|k| !bo.contains_key(*k))) {
            if bo.get(key) != ao.get(key) {
                out.push((id.clone(), key.clone(), ao.get(key).cloned().unwrap_or_default()));
            }
        }
    }
    for id in after.keys().filter(|id| !before.contains_key(*id)) {
        out.push((id.clone(), "$added".to_owned(), serde_json::Value::Null));
    }
    out
}

/// The terminal ID of a row ID `strip:<uuid>/terminal:<uuid>`.
fn terminal_of(row: &str) -> &str {
    row.rsplit_once("terminal:").unwrap().1
}

/// `ew export table strips <p> --strip -X1031 --format xlsx`, written to `file`.
fn x1031(t: &Tmp, p: &Path, file: &str) -> Vec<u8> {
    t.written(
        &args([
            &"export",
            &"table",
            &"strips",
            &p,
            &"--strip",
            &"-X1031",
            &"--format",
            &"xlsx",
        ]),
        file,
    )
}

/// Writes `bytes` as `name` in the test's directory and returns its path.
fn put(t: &Tmp, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = t.0.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// `ew import table <file> <p> <extra…>`: exit 0, and its stdout.
fn import(t: &Tmp, file: &Path, p: &Path, extra: &[&str]) -> String {
    let mut run = args([&"import", &"table", &file, &p]);
    run.extend(extra.iter().map(Into::into));
    let o = t.ew(&run);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stderr(&o), "");
    stdout(&o)
}

/// RT-1: the strip's workbook writes -X1031's 20 terminal rows in the navigator's order with every column of the
/// Terminal strips table; `_ew` names `nav:strips/<uuid>` and marks `terminal`, `label` and `state` editable and
/// `group`, `bridge_part`, `potential` not; `_ew_base` holds the 20 IDs and the three editable columns with the
/// SHA-256 of its rows.
#[test]
fn rt1_the_strip_workbook_names_its_table_and_holds_its_base() {
    let t = Tmp::new();
    let p = t.golden();
    let bytes = x1031(&t, &p, "x.xlsx");
    let limits = table_limits();
    assert_eq!(
        sheet_names(&bytes, &limits).unwrap(),
        ["-X1031", "_ew", "_ew_base"]
    );
    let main = sheet(&bytes, None);
    assert_eq!(main.headers, STRIP_HEADERS);
    assert_eq!(main.rows.len(), 20);
    let pos: Vec<String> = main.rows.iter().map(|r| r[2].clone()).collect();
    let want: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
    assert_eq!(pos, want, "the navigator's order");
    let strip = between(&main.rows[0][0], "strip:", "/").to_owned();
    assert!(
        main.rows
            .iter()
            .all(|r| r[0].starts_with(&format!("strip:{strip}/terminal:")))
    );
    let ew = read_rows(&bytes, SourceFormat::Xlsx, "_ew", &limits).unwrap();
    let table = ew.iter().find(|r| r[0] == "table").map(|r| r[1].clone());
    assert_eq!(table, Some(format!("nav:strips/{strip}")));
    let editable: BTreeMap<&str, &str> = ew
        .iter()
        .filter(|r| r[0] == "column")
        .map(|r| (r[1].as_str(), r[4].as_str()))
        .collect();
    assert_eq!(editable.len(), STRIP_HEADERS.len() - 1, "every column");
    let marked: Vec<&str> = ew
        .iter()
        .filter(|r| r[0] == "column" && r[4] == "true")
        .map(|r| r[1].as_str())
        .collect();
    assert_eq!(marked, ["terminal", "label", "state"]);
    for key in ["group", "bridge_part", "potential"] {
        assert_eq!(editable.get(key), Some(&"false"), "{key}");
    }
    let base = read_rows(&bytes, SourceFormat::Xlsx, "_ew_base", &limits).unwrap();
    assert_eq!(base.len(), 22);
    assert_eq!(base[0], ["id", "terminal", "label", "state"]);
    for (b, m) in base[1..21].iter().zip(&main.rows) {
        assert_eq!(*b, [m[0].clone(), m[3].clone(), m[4].clone(), m[12].clone()]);
    }
    let text: String = base[..21].iter().map(|r| format!("{}\n", r.join("\t"))).collect();
    assert_eq!(
        base[21],
        [
            "sha256".to_owned(),
            format!("{:x}", Sha256::digest(text.as_bytes()))
        ]
    );
}

/// RT-2 (FR-EXP-004): five Function text cells changed in a workbook saved by Excel preview as five applied cells
/// and change nothing; `--apply` changes exactly those five `function_text` fields and saves; `ew apply` of the
/// preview's command, as a command log, makes the same change in another copy.
#[test]
fn rt2_five_labels_saved_by_excel_apply_in_one_step() {
    let t = Tmp::new();
    let p = t.golden();
    let bytes = x1031(&t, &p, "x.xlsx");
    let main = sheet(&bytes, None);
    let labels = ["Valve 1", "Valve 2", "Valve 3", "Valve 4", "Valve 5"];
    let edits: Vec<(String, Put)> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| (cell_ref(&main.headers, "Function text", i + 2), Put::Text(l)))
        .collect();
    let file = put(&t, "y.xlsx", &excel_save(&bytes, &edits));
    let before = records(&p);
    let lines: String = main.rows[..5]
        .iter()
        .zip(labels)
        .map(|(r, l)| {
            format!(
                "-X1031:{}, Function text: exported \"{}\", project \"{}\", file \"{l}\": applied\n",
                r[3], r[4], r[4]
            )
        })
        .collect();
    assert_eq!(
        import(&t, &file, &p, &[]),
        format!(
            "{lines}5 changed, 0 kept, 0 conflicts, 0 refused, 0 unknown, 0 missing; a preview: nothing changed \
             (give `--apply` to apply it)\n"
        )
    );
    assert_eq!(records(&p), before, "a preview changes nothing");
    let json: serde_json::Value = serde_json::from_str(&import(&t, &file, &p, &["--json"])).unwrap();
    assert_eq!(json["$schema"], "ew.import-table-result/1");
    assert_eq!(
        (&json["three_way"], &json["changed"], &json["applied"]),
        (
            &serde_json::json!(true),
            &serde_json::json!(5),
            &serde_json::json!(false)
        )
    );
    assert_eq!(json["command"]["cmd"], "fields.set_many");
    assert_eq!(
        import(&t, &file, &p, &["--apply"]),
        format!(
            "{lines}5 changed, 0 kept, 0 conflicts, 0 refused, 0 unknown, 0 missing; applied in one step; saved\n"
        )
    );
    let after = records(&p);
    let mut want: Vec<(String, String, serde_json::Value)> = main.rows[..5]
        .iter()
        .zip(labels)
        .map(|(r, l)| {
            (
                terminal_of(&r[0]).to_owned(),
                "function_text".to_owned(),
                serde_json::json!(l),
            )
        })
        .collect();
    want.sort_by_key(|(id, _, _)| id.clone());
    let mut got = changes(&before, &after);
    got.retain(|(id, _, _)| before.get(id).is_some_and(|v| v["$k"] == "device"));
    assert_eq!(got, want, "exactly the five labels");
    let other: Vec<_> = changes(&before, &after)
        .into_iter()
        .filter(|(id, _, _)| !before.get(id).is_some_and(|v| v["$k"] == "device"))
        .collect();
    assert_eq!(other, [], "no other record changes");

    // The command, replayed by `ew apply` as a command log.
    let q = t.golden_as("q.ew");
    let log = format!(
        "{}\n{}\n",
        serde_json::json!({"$schema": ew_api::CMDLOG_SCHEMA, "label": "round trip"}),
        json["command"]
    );
    let log_file = put(&t, "rt2.ewcmd", log.as_bytes());
    let o = t.ew(&args([&"apply", &q, &log_file]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), "applied 1 of 1 lines; saved\n");
    assert_eq!(changes(&before, &records(&q)), changes(&before, &after));
}

/// RT-3: after the export, terminal 20 is deleted in the project and its row edited in the file (EW-IMP-303, naming
/// its ID); terminal 2's label changed in the project only is kept (EW-IMP-302); terminal 3's changed in both keeps
/// the project's value (EW-IMP-304), and with `--take-file` takes the file's.
#[test]
fn rt3_deleted_rows_and_project_changes_are_kept() {
    let t = Tmp::new();
    let p = t.golden();
    let bytes = x1031(&t, &p, "x.xlsx");
    let main = sheet(&bytes, None);
    let label = |row: usize| cell_ref(&main.headers, "Function text", row + 1);
    // The project's changes since the export: two labels (through a round trip of their own) and terminal 20 gone.
    let ew_side = put(
        &t,
        "ew.xlsx",
        &excel_save(
            &bytes,
            &[(label(2), Put::Text("EW 2")), (label(3), Put::Text("EW 3"))],
        ),
    );
    import(&t, &ew_side, &p, &["--apply"]);
    let twenty = terminal_of(&main.rows[19][0]).to_owned();
    let delete = format!(
        "{}\n{}\n",
        serde_json::json!({"$schema": ew_api::CMDLOG_SCHEMA, "label": "delete"}),
        serde_json::json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "device", "id": twenty}]}})
    );
    let log = put(&t, "delete.ewcmd", delete.as_bytes());
    let o = t.ew(&args([&"apply", &p, &log]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let file = put(
        &t,
        "y.xlsx",
        &excel_save(
            &bytes,
            &[(label(3), Put::Text("File 3")), (label(20), Put::Text("File 20"))],
        ),
    );
    let before = records(&p);
    let (two, three) = (&main.rows[1], &main.rows[2]);
    let head = format!(
        "-X1031:{}, Function text: exported \"{}\", project \"EW 2\", file \"{}\": kept\n-X1031:{}, Function text: \
         exported \"{}\", project \"EW 3\", file \"File 3\": ",
        two[3], two[4], two[4], three[3], three[4]
    );
    let findings = format!(
        "EW-IMP-302: Function text of -X1031:{} was changed in the project since the export: the project's value is \
         kept\nEW-IMP-304: Function text of -X1031:{} was changed in the project and in the file: ",
        two[3], three[3]
    );
    let unknown = format!(
        "EW-IMP-303: the row {} of the file is not in the project's table (deleted since the export?): nothing of it \
         is imported\n",
        main.rows[19][0]
    );
    assert_eq!(
        import(&t, &file, &p, &["--apply"]),
        format!(
            "{head}conflict\n{findings}the project's value is kept (`--take-file` takes the file's)\n{unknown}0 \
             changed, 1 kept, 1 conflicts, 0 refused, 1 unknown, 0 missing; nothing to apply\n"
        )
    );
    assert_eq!(records(&p), before, "nothing changes");
    assert_eq!(
        import(&t, &file, &p, &["--apply", "--take-file"]),
        format!(
            "{head}taken\n{findings}the file's value is taken\n{unknown}1 changed, 1 kept, 1 conflicts, 0 refused, 1 \
             unknown, 0 missing; applied in one step; saved\n"
        )
    );
    assert_eq!(
        changes(&before, &records(&p)),
        [(
            terminal_of(&three[0]).to_owned(),
            "function_text".to_owned(),
            serde_json::json!("File 3")
        )]
    );
}

/// RT-4: a Potential cell edited is EW-IMP-301 "derived", a Group cell "edit in the window"; a Terminal "1.10" that
/// Excel stored as the number 1.1 is EW-IMP-305 and imports "1.1"; an edited `_ew_base` falls back to two-way with one
/// notice; a workbook of another project and a CSV without `--table` are refused in D11's words.
#[test]
fn rt4_read_only_cells_numbers_and_refusals() {
    let t = Tmp::new();
    let p = t.golden();
    let bytes = x1031(&t, &p, "x.xlsx");
    let main = sheet(&bytes, None);
    let at = |header: &str| cell_ref(&main.headers, header, 2);
    let first = &main.rows[0];
    // Terminal 1 is named "1.10" in the project, and exported again.
    let renamed = put(
        &t,
        "r.xlsx",
        &excel_save(&bytes, &[(at("Terminal"), Put::Text("1.10"))]),
    );
    import(&t, &renamed, &p, &["--apply"]);
    let bytes = x1031(&t, &p, "x2.xlsx");
    let file = put(
        &t,
        "y.xlsx",
        &excel_save(
            &bytes,
            &[
                (at("Terminal"), Put::Number("1.1")),
                (at("Group"), Put::Text("Spare")),
                (at("Potential"), Put::Text("24VDC-9")),
            ],
        ),
    );
    let before = records(&p);
    assert_eq!(
        import(&t, &file, &p, &["--apply"]),
        format!(
            "-X1031:1.10, Terminal: exported \"1.10\", project \"1.10\", file \"1.1\": applied\n-X1031:1.10, Group: \
             project \"{group}\", file \"Spare\": read-only (edit in the window)\n-X1031:1.10, Potential: project \
             \"{potential}\", file \"24VDC-9\": read-only (derived)\nEW-IMP-305: Terminal of -X1031:1.10 reads \
             \"1.1\" where the export wrote \"1.10\": a spreadsheet stored the text as a number; \"1.1\" is \
             imported\nEW-IMP-301: Group of -X1031:1.10 is read-only in a round trip (edit in the window): the \
             file's value is not imported\nEW-IMP-301: Potential of -X1031:1.10 is read-only in a round trip \
             (derived): the file's value is not imported\n1 changed, 0 kept, 0 conflicts, 2 refused, 0 unknown, 0 \
             missing; applied in one step; saved\n",
            group = first[5],
            potential = first[7]
        )
    );
    let terminal = terminal_of(&first[0]).to_owned();
    let got = changes(&before, &records(&p));
    assert!(
        !got.is_empty() && got.iter().all(|(id, _, _)| *id == terminal),
        "only terminal 1 changes: {got:?}"
    );

    // An edited `_ew_base`: two-way against the project's values, with one notice.
    let bytes = x1031(&t, &p, "x3.xlsx");
    let edited = with_part(&bytes, "xl/worksheets/sheet3.xml", "<t>used</t>", "<t>spare</t>");
    let file = put(&t, "b.xlsx", &edited);
    assert_eq!(
        import(&t, &file, &p, &[]),
        "the `_ew_base` sheet of the workbook was changed, so each cell is compared with the project's value only \
         (two-way)\n0 changed, 0 kept, 0 conflicts, 0 refused, 0 unknown, 0 missing; a preview: nothing to apply\n"
    );

    // A workbook of another project.
    let ew = read_rows(&bytes, SourceFormat::Xlsx, "_ew", &table_limits()).unwrap();
    let project = ew.iter().find(|r| r[0] == "project").unwrap()[1].clone();
    let other = "00000000-0000-4000-8000-000000000001";
    let foreign = put(
        &t,
        "o.xlsx",
        &with_part(&bytes, "xl/worksheets/sheet2.xml", &project, other),
    );
    let o = t.ew(&args([&"import", &"table", &foreign, &p]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        format!("error: the file was exported from another project ({other})\n")
    );

    // A CSV without `--table`.
    let csv = t.0.join("x.csv");
    let o = t.ew(&args([
        &"export",
        &"table",
        &"strips",
        &p,
        &"--strip",
        &"-X1031",
        &"--preset",
        &"excel-en",
        &"--out",
        &csv,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let o = t.ew(&args([&"import", &"table", &csv, &p]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(stderr(&o), "error: a CSV names no table: give `--table <key>`\n");
}

/// The I/O list's CSV with the Comment of its first three channels set to `comments`, and its `id` column kept or
/// dropped.
fn commented(csv: &str, comments: [&str; 3], keep_id: bool) -> String {
    let lines: Vec<&str> = csv.trim_start_matches('\u{feff}').split("\r\n").collect();
    let comment = lines[0].split(';').position(|h| h == "Comment").unwrap();
    let mut out = String::from("\u{feff}");
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let mut cells: Vec<String> = line.split(';').map(str::to_owned).collect();
        if (1..=3).contains(&i) {
            cells[comment] = comments[i - 1].to_owned();
        }
        if !keep_id {
            cells.remove(0);
        }
        out.push_str(&cells.join(";"));
        out.push_str("\r\n");
    }
    out
}

/// RT-5 (FR-PLC-038): `ew export report --kind io_list --preset excel-nl` is the committed preset (a byte-order mark,
/// `;`, the `id` column, 28 rows); three Comment cells changed in Dutch and imported with `--table kind:io_list
/// --apply` change exactly three `comment` fields; without the `id` column the rows match by station, slot and
/// channel with the same result; the I/O list's workbook round-trips the same way.
#[test]
fn rt5_the_io_list_round_trips_by_id_and_by_point() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([
        &"export",
        &"report",
        &p,
        &"--kind",
        &"io_list",
        &"--preset",
        &"excel-nl",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stderr(&o), "28 rows\n");
    let csv = stdout(&o);
    assert!(
        csv.starts_with("\u{feff}id;Configuration project;CPU;Station;"),
        "{csv}"
    );
    assert!(!csv.contains('"'), "no quoted field");
    assert_eq!(csv.matches("\r\n").count(), 29, "the header and 28 rows");
    golden_file(
        "csv/presets/io_list.excel-nl.csv",
        csv.replace("\r\n", "\n").as_bytes(),
    );
    let comments = ["Klep open", "Pomp aan", "Koeling geïnstalleerd"];
    let rows: Vec<Vec<&str>> = csv
        .trim_start_matches('\u{feff}')
        .split("\r\n")
        .skip(1)
        .take(3)
        .map(|l| l.split(';').collect())
        .collect();
    let channel = |r: &[&str]| {
        r[0].trim_start_matches("e:")
            .split('/')
            .next()
            .unwrap()
            .to_owned()
    };
    let want: Vec<(String, String, serde_json::Value)> = rows
        .iter()
        .zip(comments)
        .map(|(r, c)| (channel(r), "comment".to_owned(), serde_json::json!(c)))
        .collect();
    let lines: String = rows
        .iter()
        .zip(comments)
        .map(|(r, c)| {
            format!(
                "{} channel {}, Comment: project \"\", file \"{c}\": applied\n",
                r[6].trim_start_matches('\''),
                r[8]
            )
        })
        .collect();
    let text = format!(
        "{lines}3 changed, 0 kept, 0 conflicts, 0 refused, 0 unknown, 0 missing; applied in one step; saved\n"
    );
    let before = records(&p);
    for (name, keep_id) in [("by-id", true), ("by-point", false)] {
        let copy = t.golden_as(&format!("{name}.ew"));
        let file = put(
            &t,
            &format!("{name}.csv"),
            commented(&csv, comments, keep_id).as_bytes(),
        );
        assert_eq!(
            import(&t, &file, &copy, &["--table", "kind:io_list", "--apply"]),
            text,
            "{name}"
        );
        let mut got = changes(&before, &records(&copy));
        got.sort_by_key(|(id, _, _)| id.clone());
        let mut want = want.clone();
        want.sort_by_key(|(id, _, _)| id.clone());
        assert_eq!(got, want, "{name}");
    }

    // The workbook: the same three cells, saved by Excel.
    let copy = t.golden_as("xlsx.ew");
    let bytes = t.written(
        &args([
            &"export",
            &"report",
            &copy,
            &"--kind",
            &"io_list",
            &"--format",
            &"xlsx",
        ]),
        "io.xlsx",
    );
    let main = sheet(&bytes, None);
    let edits: Vec<(String, Put)> = comments
        .iter()
        .enumerate()
        .map(|(i, c)| (cell_ref(&main.headers, "Comment", i + 2), Put::Text(c)))
        .collect();
    let file = put(&t, "io2.xlsx", &excel_save(&bytes, &edits));
    let lines: String = lines.replace("project \"\"", "exported \"\", project \"\"");
    assert_eq!(
        import(&t, &file, &copy, &["--apply"]),
        format!(
            "{lines}3 changed, 0 kept, 0 conflicts, 0 refused, 0 unknown, 0 missing; applied in one step; saved\n"
        )
    );
    let mut got = changes(&before, &records(&copy));
    got.sort_by_key(|(id, _, _)| id.clone());
    let mut want = want;
    want.sort_by_key(|(id, _, _)| id.clone());
    assert_eq!(got, want, "the workbook");
}

/// D8 and RT-6: every table navigator exports (its `_ew` names `nav:<word>`), twice to the same bytes; another word,
/// `--strip` on another navigator and an unknown strip are refused in D8's words.
#[test]
fn every_table_navigator_exports_and_the_others_are_refused() {
    let t = Tmp::new();
    let p = t.golden();
    for kind in ew_api::output::TABLE_NAVIGATORS {
        let word = kind.word();
        let run = args([&"export", &"table", &word, &p, &"--format", &"xlsx"]);
        let file = format!("{word}.xlsx");
        let bytes = t.written(&run, &file);
        assert!(
            t.written(&run, &file) == bytes,
            "{word}: a second run gives other bytes"
        );
        let ew = read_rows(&bytes, SourceFormat::Xlsx, "_ew", &table_limits()).unwrap();
        let table = ew.iter().find(|r| r[0] == "table").map(|r| r[1].clone());
        assert_eq!(table, Some(format!("nav:{word}")), "{word}");
    }
    let usage = "\n\nUsage: ew export table [OPTIONS] <navigator> [project-dir]\n\nFor more information, try '--help'.\n";
    let o = t.ew(&args([&"export", &"table", &"macros", &p]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        format!(
            "error: unknown navigator `macros` (the table navigators are: pages, devices, connections, strips, \
             cables, plc, potentials, ips, structure, parts, blocks, unplaced){usage}"
        )
    );
    let o = t.ew(&args([
        &"export", &"table", &"devices", &p, &"--strip", &"-X1031",
    ]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        format!("error: `--strip` applies to `export table strips`{usage}")
    );
    let o = t.ew(&args([&"export", &"table", &"strips", &p, &"--strip", &"-X9"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: no terminal strip `-X9` (the strips are: -X1031, -X1032, 1X1)\n"
    );
    let o = t.ew(&args([&"export", &"table", &"strips", &p, &"--format", &"xlsx"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: `--format xlsx` writes a file: give `--out <file.xlsx>`\n\nUsage: ew export table [OPTIONS] \
         <navigator> [project-dir]\n\nFor more information, try '--help'.\n"
    );
}
