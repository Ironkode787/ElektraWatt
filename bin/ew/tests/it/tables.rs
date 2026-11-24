// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Through the `ew` binary, on a copy of the sample: the table writers behind every row export and their expected
//! files (the round trip's tests are `round_trip`, with the helpers here).
//!
//! The expected files (D16), under `bin/ew/tests/expected/`: one cell dump per workbook, `xlsx/<name>.cells.txt`
//! (each sheet read back through the round trip's reader, [`ew_api::output::workbook_dump`]), for `bom_parts.xlsx`,
//! `wires.xlsx`, `block_908.xlsx` (part 1), `strips_X1031.xlsx` and `io_list.xlsx` (part 2); and the CSV presets,
//! `csv/presets/bom_parts.<preset>.csv` and `io_list.excel-nl.csv` (`round_trip`). Their IDs are numbered by order of
//! appearance ([`numbered_ids`]): the sample's IDs are made by its script. The CSVs of the `ew` dialect are pinned by
//! `ew-api`'s tests, not here.
//! `EW_BLESS=1 cargo nextest run -p ew --all-features --test it -E 'test(/^(tables|round_trip)::/)'` rewrites them.

use std::ffi::OsString;
use std::path::Path;

use ew_api::catalog::{SourceFormat, table_limits};
use ew_api::output::sheet::{SheetTable, read_sheet, sheet_names};
use ew_api::zip::{ZipLimits, read as unzip};

use crate::support::{SAMPLE, Tmp, args, stderr, stdout};

/// The expected files of the `ew` tests.
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected");

// The program boundary of a test: `EW_BLESS=1` rewrites the expected files instead of comparing them.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// `text` with every UUID written `<id n>`, numbered by order of first appearance.
fn numbered_ids(text: &str) -> String {
    let uuid = |s: &[u8]| {
        s.len() == 36
            && s.iter().enumerate().all(|(i, c)| {
                if matches!(i, 8 | 13 | 18 | 23) {
                    *c == b'-'
                } else {
                    c.is_ascii_hexdigit()
                }
            })
    };
    let bytes = text.as_bytes();
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::new();
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        if bytes.len() >= i + 36 && uuid(&bytes[i..i + 36]) {
            let id = &text[i..i + 36];
            let n = match seen.iter().position(|s| *s == id) {
                Some(n) => n + 1,
                None => {
                    seen.push(id);
                    seen.len()
                }
            };
            out.push_str(&format!("<id {n}>"));
            i += 36;
        } else {
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// Compares `text`, its IDs numbered ([`numbered_ids`]), with the committed file `rel` below `expected/`, or writes
/// it under `EW_BLESS=1`.
pub(crate) fn expected_file(rel: &str, text: &str) {
    let path = Path::new(EXPECTED).join(rel);
    let text = numbered_ids(text);
    if bless() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 generates it)", path.display()));
    assert!(
        committed == text,
        "{} differs from the export (EW_BLESS=1 rewrites it)",
        path.display()
    );
}

/// The ID of the sample's report block with this first folio (`report-blocks.jsonl`).
fn block_id(first_folio: u32) -> String {
    let text = std::fs::read_to_string(Path::new(SAMPLE).join("report-blocks.jsonl")).unwrap();
    text.lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["first_folio"] == first_folio)
        .and_then(|v| v["id"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("no report block with the first folio {first_folio}"))
}

/// The sheet `name` of a workbook (the first when `None`), read back through the round trip's reader.
pub(crate) fn sheet(bytes: &[u8], name: Option<&str>) -> SheetTable {
    read_sheet(bytes, SourceFormat::Xlsx, name, &table_limits()).unwrap()
}

/// A CSV read through the same reader.
fn csv_table(text: &str) -> SheetTable {
    read_sheet(text.as_bytes(), SourceFormat::Csv, None, &table_limits()).unwrap()
}

/// The standard output of a successful `ew` run.
fn printed(t: &Tmp, run: &[OsString]) -> String {
    let o = t.ew(run);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    stdout(&o)
}

/// The text of the entry `name` of a workbook (the strict reader of `ew-zip` refuses another date, version or flag
/// than the writer's, so a successful read also proves every entry's 1980-01-01 00:00).
pub(crate) fn part(bytes: &[u8], name: &str) -> String {
    let entries = unzip(bytes, &ZipLimits::default()).unwrap();
    let entry = entries
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no part {name}"));
    String::from_utf8(entry.data.clone()).unwrap()
}

/// D16: the workbooks of the sample, part 1's (`bom_parts`, `wires`, `block_908`) and part 2's (`strips_X1031` of `ew
/// export table`, `io_list` of `ew export report --kind`), are the committed cell dumps, and each export
/// twice gives the same bytes (TX-5, RT-6).
#[test]
fn the_workbooks_are_the_committed_dumps() {
    let t = Tmp::new("ew-tables");
    let p = t.sample("sample.ew");
    let runs: [(&str, Vec<OsString>); 5] = [
        ("bom_parts", args([&"export", &"bom", &p, &"--format", &"xlsx"])),
        ("wires", args([&"export", &"wires", &p, &"--format", &"xlsx"])),
        (
            "block_908",
            args([&"export", &"report", &p, &"--block", &"908", &"--format", &"xlsx"]),
        ),
        (
            "strips_X1031",
            args([
                &"export",
                &"table",
                &"strips",
                &p,
                &"--strip",
                &"-X1031",
                &"--format",
                &"xlsx",
            ]),
        ),
        (
            "io_list",
            args([
                &"export",
                &"report",
                &p,
                &"--kind",
                &"io_list",
                &"--format",
                &"xlsx",
            ]),
        ),
    ];
    for (name, run) in &runs {
        let file = format!("{name}.xlsx");
        let bytes = t.written(run, &file);
        assert!(
            t.written(run, &file) == bytes,
            "{name}: a second run gives other bytes"
        );
        // The producer cell names the program's version, which the dump reads as `<version>`.
        let dump = ew_api::output::workbook_dump(&bytes).unwrap().replace(
            concat!("\new\t", env!("CARGO_PKG_VERSION"), "\n"),
            "\new\t<version>\n",
        );
        expected_file(&format!("xlsx/{name}.cells.txt"), &dump);
    }
}

/// TX-1 (FR-REP-030): the wire list's workbook, read back, is the wire list's CSV: the header `id` and its labels, its
/// rows (the data rows and the heading rows) in folio 907's order with the same texts; column A is hidden; `_ew` names
/// the block of folio 907 and every column read-only; there is no `_ew_base`.
#[test]
fn the_wire_list_workbook_reads_back_as_the_wire_list() {
    let t = Tmp::new("ew-tables");
    let p = t.sample("sample.ew");
    let bytes = t.written(&args([&"export", &"wires", &p, &"--format", &"xlsx"]), "w.xlsx");
    let want = csv_table(&printed(&t, &args([&"export", &"wires", &p])));
    let got = sheet(&bytes, None);
    assert_eq!(got.headers[0], "id");
    assert_eq!(got.headers[1..], want.headers[1..]);
    assert_eq!(got.rows.len(), want.rows.len());
    assert!(want.rows.iter().any(|r| r[0] == "heading"));
    for (i, (g, w)) in got.rows.iter().zip(&want.rows).enumerate() {
        assert_eq!(g[1..], w[1..], "row {}", i + 1);
        assert_eq!(
            g[0].is_empty(),
            w[0] == "heading",
            "row {}: the id of a heading row is empty",
            i + 1
        );
    }
    assert_eq!(
        sheet_names(&bytes, &table_limits()).unwrap(),
        ["wire_list", "_ew"]
    );
    let first = part(&bytes, "xl/worksheets/sheet1.xml");
    assert!(first.contains("<col min=\"1\" max=\"1\" width=\"0\" hidden=\"1\" customWidth=\"1\"/>"));
    let ew = sheet(&bytes, Some("_ew"));
    assert_eq!(ew.headers, ["format", "ew-table/1"]);
    let table = ew.rows.iter().find(|r| r[0] == "table").unwrap();
    assert_eq!(table[1], format!("block:{}", block_id(907)));
    // The reader keeps the cells under the first row's two headers, so the column rows are read from the part.
    let ew_xml = part(&bytes, "xl/worksheets/sheet2.xml");
    let columns = want.headers.len() - 1;
    assert_eq!(ew_xml.matches("<t>false</t>").count(), columns);
    assert_eq!(ew_xml.matches("<t>true</t>").count(), 0);
    let workbook = part(&bytes, "xl/workbook.xml");
    assert!(workbook.contains("<sheet name=\"_ew\" sheetId=\"2\" state=\"hidden\" r:id=\"rId2\"/>"));
}

/// TX-2: the bill of materials' Qty is a numeric cell (the sample's 15 feed-through terminals, its rails' 0.94 m and
/// its duct's 0.47 m); the block of folio 908 has the rows of its CSV (FR-EXT-027); the terminal diagrams give one
/// sheet per strip; `ew sql` writes the query's rows.
#[test]
fn the_row_exports_and_sql_write_workbooks() {
    let t = Tmp::new("ew-tables");
    let p = t.sample("sample.ew");
    let bom = t.written(&args([&"export", &"bom", &p, &"--format", &"xlsx"]), "b.xlsx");
    let want = csv_table(&printed(&t, &args([&"export", &"bom", &p])));
    let got = sheet(&bom, None);
    assert_eq!(got.rows.len(), want.rows.len());
    for (g, w) in got.rows.iter().zip(&want.rows) {
        assert_eq!(g[1..], w[1..]);
    }
    let xml = part(&bom, "xl/worksheets/sheet1.xml");
    assert!(
        xml.contains("<c r=\"C2\"><v>15</v></c>"),
        "row 2's quantity is a number"
    );
    assert!(xml.contains("<v>0.94</v>") && xml.contains("<v>0.47</v>"));

    let block = t.written(
        &args([&"export", &"report", &p, &"--block", &"908", &"--format", &"xlsx"]),
        "r.xlsx",
    );
    let block_csv = csv_table(&printed(
        &t,
        &args([&"export", &"report", &p, &"--block", &"908"]),
    ));
    assert_eq!(sheet(&block, None).rows.len(), block_csv.rows.len());

    let terminals = t.written(
        &args([&"export", &"terminals", &p, &"--format", &"xlsx"]),
        "t.xlsx",
    );
    assert_eq!(
        sheet_names(&terminals, &table_limits()).unwrap(),
        ["-X1011", "-X1021", "-X1031", "_ew"]
    );
    let x1031 = sheet(&terminals, Some("-X1031"));
    let want = csv_table(&printed(
        &t,
        &args([&"export", &"terminals", &p, &"--strip", &"-X1031"]),
    ));
    assert_eq!(x1031.rows.len(), want.rows.len());
    for (g, w) in x1031.rows.iter().zip(&want.rows) {
        assert_eq!(g[1..], w[1..]);
    }

    let out = t.0.join("q.xlsx");
    let o = t.ew(&args([
        &"sql",
        &p,
        &"SELECT 'a' AS name, 2 AS n UNION ALL SELECT 'b', 3 ORDER BY 1",
        &"--format",
        &"xlsx",
        &"--out",
        &out,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), format!("result written to {}\n", out.display()));
    let q = std::fs::read(&out).unwrap();
    let got = sheet(&q, Some("sql"));
    assert_eq!(got.headers, ["id", "name", "n"]);
    assert_eq!(got.rows, [["", "a", "2"], ["", "b", "3"]]);
    let ew = sheet(&q, Some("_ew"));
    assert!(ew.rows.iter().any(|r| r[0] == "table" && r[1] == "sql"));

    for run in [
        args([&"export", &"wires", &p, &"--format", &"xlsx"]),
        args([&"sql", &p, &"SELECT 1", &"--format", &"xlsx"]),
    ] {
        let o = t.ew(&run);
        assert_eq!(o.status.code(), Some(2));
        let usage = if run[0] == "sql" {
            "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
             ew sql <COMMAND>"
        } else {
            "ew export wires [OPTIONS] [project-dir]"
        };
        assert_eq!(
            stderr(&o),
            format!(
                "error: `--format xlsx` writes a file: give `--out <file.xlsx>`\n\nUsage: {usage}\n\nFor more \
                 information, try '--help'.\n"
            )
        );
    }
}

/// TX-3 and D16: `--preset excel-nl`, `excel-en` and `rfc4180` on `ew export bom` are the committed files (`id` first,
/// the quantities 0.94 and 0.47 written "0,94" and "0,47" in `excel-nl`); `--bom off --line-end lf` drops the
/// byte-order mark and writes LF; the default is still the `ew` dialect. The presets write CRLF, asserted here; the
/// committed files hold LF, as the repository's `eol=lf` stores every text file.
#[test]
fn the_bom_presets_are_the_committed_files() {
    let t = Tmp::new("ew-tables");
    let p = t.sample("sample.ew");
    let mut texts = Vec::new();
    for preset in ["excel-nl", "excel-en", "rfc4180"] {
        let o = t.ew(&args([&"export", &"bom", &p, &"--preset", &preset]));
        assert_eq!(o.status.code(), Some(0), "{preset}: {}", stderr(&o));
        let text = stdout(&o);
        // Every line ends in CRLF; the committed file keeps LF (the repository's `eol=lf`).
        assert_eq!(
            text.matches('\n').count(),
            text.matches("\r\n").count(),
            "{preset}"
        );
        expected_file(
            &format!("csv/presets/bom_parts.{preset}.csv"),
            &text.replace("\r\n", "\n"),
        );
        texts.push(text);
    }
    let nl = &texts[0];
    assert!(nl.starts_with("\u{feff}id;Pos;Qty;Unit;Manufacturer;Part number;Description;Tags;Folios\r\n"));
    assert!(nl.contains(";0,94;m;") && nl.contains(";0,47;m;"), "{nl}");
    assert!(texts[1].starts_with("\u{feff}id,Pos,Qty,") && texts[1].contains(",0.94,m,"));
    assert!(texts[2].starts_with("id,Pos,Qty,") && texts[2].ends_with("\r\n"));
    let o = t.ew(&args([
        &"export",
        &"bom",
        &p,
        &"--preset",
        &"excel-nl",
        &"--bom",
        &"off",
        &"--line-end",
        &"lf",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let want = nl.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    assert_eq!(stdout(&o), want);
    assert_eq!(
        printed(&t, &args([&"export", &"bom", &p])),
        printed(&t, &args([&"export", &"bom", &p, &"--preset", &"ew"]))
    );
    // The two refusals of D6, whole.
    let o = t.ew(&args([
        &"export",
        &"bom",
        &p,
        &"--format",
        &"xlsx",
        &"--out",
        &"x.xlsx",
        &"--preset",
        &"excel-nl",
    ]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: `--preset` applies to CSV only; drop it, or give `--format csv`\n\nUsage: ew export bom [OPTIONS] \
         [project-dir]\n\nFor more information, try '--help'.\n"
    );
    let o = t.ew(&args([&"export", &"bom", &p, &"--preset", &"excel"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: invalid value 'excel' for '--preset <preset>': unknown preset `excel` (ew, excel-nl, excel-en, \
         rfc4180)\n\nFor more information, try '--help'.\n"
    );
}

/// TX-4 (NFR-SEC-17) end to end: a text that starts like a formula is an inline string in the workbook (no part has
/// an `<f>` element) and is guarded in the Excel presets, written verbatim in `rfc4180`; a negative number is never
/// guarded.
#[test]
fn formula_text_is_never_a_formula() {
    let t = Tmp::new("ew-tables");
    let p = t.sample("sample.ew");
    let query = "SELECT '=HYPERLINK(\"x\")' AS t, '-3X1' AS tag, -1.5 AS n";
    let out = t.0.join("f.xlsx");
    let o = t.ew(&args([&"sql", &p, &query, &"--format", &"xlsx", &"--out", &out]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let bytes = std::fs::read(&out).unwrap();
    let sheet_xml = part(&bytes, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains("<c r=\"B2\" t=\"inlineStr\"><is><t>=HYPERLINK(&quot;x&quot;)</t></is></c>"),
        "{sheet_xml}"
    );
    assert!(sheet_xml.contains("<c r=\"D2\"><v>-1.5</v></c>"), "{sheet_xml}");
    for entry in unzip(&bytes, &ZipLimits::default()).unwrap() {
        let text = String::from_utf8_lossy(&entry.data);
        assert!(!text.contains("<f>") && !text.contains("<f "), "{}", entry.name);
    }
    let csv = |preset: &str| {
        let o = t.ew(&args([&"sql", &p, &query, &"--preset", &preset]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        stdout(&o)
    };
    assert_eq!(
        csv("excel-nl"),
        "\u{feff}id;t;tag;n\r\n;\"'=HYPERLINK(\"\"x\"\")\";'-3X1;-1,5\r\n"
    );
    assert_eq!(
        csv("excel-en"),
        "\u{feff}id,t,tag,n\r\n,\"'=HYPERLINK(\"\"x\"\")\",'-3X1,-1.5\r\n"
    );
    assert_eq!(
        csv("rfc4180"),
        "id,t,tag,n\r\n,\"=HYPERLINK(\"\"x\"\")\",-3X1,-1.5\r\n"
    );
}

/// TX-5: with `SOURCE_DATE_EPOCH` set, `docProps/core.xml` carries that date; without it, the model's date.
#[test]
fn the_workbook_date_is_the_export_date() {
    let t = Tmp::new("ew-tables");
    let p = t.sample("sample.ew");
    let out = t.0.join("d.xlsx");
    let run = args([&"export", &"wires", &p, &"--format", &"xlsx", &"--out", &out]);
    let o = t.ew_at(&run, Some("1767225600"));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let core = part(&std::fs::read(&out).unwrap(), "docProps/core.xml");
    assert!(
        core.contains("<dcterms:created xsi:type=\"dcterms:W3CDTF\">2026-01-01T00:00:00Z</dcterms:created>"),
        "{core}"
    );
    let o = t.ew(&run);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let core = part(&std::fs::read(&out).unwrap(), "docProps/core.xml");
    assert!(!core.contains("2026-01-01T00:00:00Z"), "{core}");
    let app = part(&std::fs::read(&out).unwrap(), "docProps/app.xml");
    assert!(app.contains("<Application>ElektraWatt</Application>"));
}

/// TX-7: every `--json` export result names the format, the preset of a CSV and the revision.
#[test]
fn the_json_results_name_format_preset_and_revision() {
    let t = Tmp::new("ew-tables");
    let p = t.sample("sample.ew");
    let json = |run: Vec<OsString>| -> serde_json::Value {
        let o = t.ew(&run);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        serde_json::from_str(&stdout(&o)).unwrap()
    };
    let out = t.0.join("j.xlsx");
    let v = json(args([
        &"export",
        &"bom",
        &p,
        &"--format",
        &"xlsx",
        &"--out",
        &out,
        &"--json",
    ]));
    assert_eq!(
        (&v["format"], &v["preset"], v["rev"].is_u64(), &v["csv"]),
        (
            &serde_json::json!("xlsx"),
            &serde_json::Value::Null,
            true,
            &serde_json::Value::Null
        )
    );
    let v = json(args([
        &"export",
        &"terminals",
        &p,
        &"--preset",
        &"rfc4180",
        &"--json",
    ]));
    assert_eq!(
        (&v["format"], &v["preset"]),
        (&serde_json::json!("csv"), &serde_json::json!("rfc4180"))
    );
    assert_eq!(v["strips"], serde_json::json!(["-X1011", "-X1021", "-X1031"]));
    assert!(v["csv"].as_str().unwrap().starts_with("id,Strip,Terminal,"));
    let v = json(args([&"export", &"wires", &p, &"--json"]));
    assert_eq!(
        (&v["format"], &v["preset"]),
        (&serde_json::json!("csv"), &serde_json::json!("ew"))
    );
    let v = json(args([
        &"export",
        &"report",
        &p,
        &"--kind",
        &"io_list",
        &"--format",
        &"xlsx",
        &"--out",
        &out,
        &"--json",
    ]));
    assert_eq!(
        (&v["format"], &v["kind"], &v["block"]),
        (
            &serde_json::json!("xlsx"),
            &serde_json::json!("io_list"),
            &serde_json::json!("")
        )
    );
    assert!(v["rev"].is_u64());
}
