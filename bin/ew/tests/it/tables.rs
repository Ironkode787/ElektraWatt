// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.08 part 1 through the `ew` binary: the table writers behind every row export (TX-1 to TX-5, TX-7) and the
//! golden files of D16's part 1.
//!
//! The golden files (D16): `golden/J-E-2416/expected/xlsx/sha256.txt` (one line `<sha256>  <file>` per workbook) and
//! one cell dump per workbook, `expected/xlsx/<name>.cells.txt` (each sheet read back through the round trip's reader,
//! [`ew_api::output::workbook_dump`]), for `bom_parts.xlsx`, `wires.xlsx` and `block_908.xlsx`; and the CSV presets of
//! the bill of materials, `expected/csv/presets/bom_parts.<preset>.csv`. They are new files beside the eight CSVs,
//! whose bytes do not move. `EW_BLESS=1 cargo nextest run -p ew --all-features --test it -E 'test(/^tables::/)'`
//! rewrites them.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_api::catalog::{SourceFormat, table_limits};
use ew_api::output::sheet::{SheetTable, read_sheet, sheet_names};
use ew_api::zip::{ZipLimits, read as unzip};
use sha2::{Digest, Sha256};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory with its own user directories, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-tables-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    /// `ew` in this directory with its user directories inside it, no starter content, no display and
    /// `SOURCE_DATE_EPOCH` as given (unset by default).
    fn ew_at(&self, args: &[OsString], epoch: Option<&str>) -> Output {
        let mut cmd = Command::new(bin_exe());
        cmd.args(args)
            .current_dir(&self.0)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("SOURCE_DATE_EPOCH")
            .env_remove("EW_CONTENT_DIR")
            .env("EW_DATA_DIR", self.0.join("data"))
            .env("EW_CACHE_DIR", self.0.join("cache"));
        if let Some(e) = epoch {
            cmd.env("SOURCE_DATE_EPOCH", e);
        }
        for d in [
            "HOME",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "XDG_STATE_HOME",
            "XDG_DATA_HOME",
            "XDG_CONFIG_HOME",
        ] {
            let dir = self.0.join("home").join(d.to_ascii_lowercase());
            std::fs::create_dir_all(&dir).unwrap();
            cmd.env(d, &dir);
        }
        cmd.output().unwrap()
    }

    fn ew(&self, args: &[OsString]) -> Output {
        self.ew_at(args, None)
    }

    /// A copy of Golden A.
    fn golden(&self) -> PathBuf {
        let copy = self.0.join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &copy);
        copy
    }

    /// Runs `ew <args> --out <dir>/<file>` and returns the file's bytes, after a successful run.
    fn written(&self, args: &[OsString], file: &str) -> Vec<u8> {
        let out = self.0.join(file);
        let mut all: Vec<OsString> = args.to_vec();
        all.push("--out".into());
        all.push(out.clone().into_os_string());
        let o = self.ew(&all);
        assert_eq!(o.status.code(), Some(0), "{args:?}: {}", stderr(&o));
        assert_eq!(stdout(&o), format!("export written to {}\n", out.display()));
        assert!(
            !self.0.join(format!("{file}.tmp")).exists(),
            "no temporary file is left"
        );
        std::fs::read(&out).unwrap()
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The path of the `ew` binary under test (nextest's run-time path, else the compile-time one).
// The program boundary of a test: the path only chooses which binary runs; no compared result depends on it.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_ew")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ew"))) // bin-exe-fallback
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

fn args<const N: usize>(items: [&dyn AsRef<OsStr>; N]) -> Vec<OsString> {
    items.iter().map(|a| a.as_ref().to_owned()).collect()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

// The program boundary of a test: `EW_BLESS=1` rewrites the golden files instead of comparing them.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// Compares `bytes` with the committed file `rel` below `expected/`, or writes it under `EW_BLESS=1`.
fn golden_file(rel: &str, bytes: &[u8]) {
    let path = Path::new(EXPECTED).join(rel);
    if bless() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        return;
    }
    let committed =
        std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 generates it)", path.display()));
    assert!(
        committed == bytes,
        "{} differs from the export (EW_BLESS=1 rewrites it)",
        path.display()
    );
}

/// The ID of Golden A's report block with this first folio (`report-blocks.jsonl`).
fn block_id(first_folio: u32) -> String {
    let text = std::fs::read_to_string(Path::new(GOLDEN_A).join("report-blocks.jsonl")).unwrap();
    text.lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["first_folio"] == first_folio)
        .and_then(|v| v["id"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("no report block with the first folio {first_folio}"))
}

/// The sheet `name` of a workbook (the first when `None`), read back through the round trip's reader.
fn sheet(bytes: &[u8], name: Option<&str>) -> SheetTable {
    read_sheet(bytes, SourceFormat::Xlsx, name, &table_limits()).unwrap()
}

/// A CSV read through the same reader.
fn csv_table(text: &str) -> SheetTable {
    read_sheet(text.as_bytes(), SourceFormat::Csv, None, &table_limits()).unwrap()
}

/// The text of the entry `name` of a workbook (the strict reader of `ew-zip` refuses another date, version or flag
/// than the writer's, so a successful read also proves every entry's 1980-01-01 00:00).
fn part(bytes: &[u8], name: &str) -> String {
    let entries = unzip(bytes, &ZipLimits::default()).unwrap();
    let entry = entries
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no part {name}"));
    String::from_utf8(entry.data.clone()).unwrap()
}

/// D16: the part-1 workbooks of Golden A (`bom_parts`, `wires`, `block_908`) are the committed digests and cell
/// dumps, and each export twice gives the same bytes (TX-5).
#[test]
fn the_part_1_workbooks_are_the_committed_digests_and_dumps() {
    let t = Tmp::new();
    let p = t.golden();
    let runs: [(&str, Vec<OsString>); 3] = [
        ("bom_parts", args([&"export", &"bom", &p, &"--format", &"xlsx"])),
        ("wires", args([&"export", &"wires", &p, &"--format", &"xlsx"])),
        (
            "block_908",
            args([&"export", &"report", &p, &"--block", &"908", &"--format", &"xlsx"]),
        ),
    ];
    let mut digests = String::new();
    for (name, run) in &runs {
        let file = format!("{name}.xlsx");
        let bytes = t.written(run, &file);
        assert!(
            t.written(run, &file) == bytes,
            "{name}: a second run gives other bytes"
        );
        digests.push_str(&format!("{:x}  {file}\n", Sha256::digest(&bytes)));
        let dump = ew_api::output::workbook_dump(&bytes).unwrap();
        golden_file(&format!("xlsx/{name}.cells.txt"), dump.as_bytes());
    }
    golden_file("xlsx/sha256.txt", digests.as_bytes());
}

/// TX-1 (FR-REP-030): the wire list's workbook, read back, is `wires.csv`: the header `id` and its labels, its 51
/// rows (50 data rows, one heading row) in folio 907's order with the same texts; column A is hidden; `_ew` names
/// the block of folio 907 and every column read-only; there is no `_ew_base`.
#[test]
fn the_wire_list_workbook_reads_back_as_the_golden_wire_list() {
    let t = Tmp::new();
    let p = t.golden();
    let bytes = t.written(&args([&"export", &"wires", &p, &"--format", &"xlsx"]), "w.xlsx");
    let want = csv_table(&std::fs::read_to_string(Path::new(EXPECTED).join("csv/wires.csv")).unwrap());
    let got = sheet(&bytes, None);
    assert_eq!(got.headers[0], "id");
    assert_eq!(got.headers[1..], want.headers[1..]);
    assert_eq!(got.rows.len(), 51);
    assert_eq!(want.rows.len(), 51);
    let data = got.rows.iter().filter(|r| !r[0].is_empty()).count();
    assert_eq!(data, 50);
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

/// TX-2: the bill of materials' Qty is a numeric cell; the block of folio 908 has its 56 rows (FR-EXT-027); the
/// terminal diagrams give one sheet per strip; `ew sql` writes the query's rows (FT-218).
#[test]
fn the_row_exports_and_sql_write_workbooks() {
    let t = Tmp::new();
    let p = t.golden();
    let bom = t.written(&args([&"export", &"bom", &p, &"--format", &"xlsx"]), "b.xlsx");
    let want = csv_table(&std::fs::read_to_string(Path::new(EXPECTED).join("csv/bom_parts.csv")).unwrap());
    let got = sheet(&bom, None);
    assert_eq!(got.rows.len(), want.rows.len());
    for (g, w) in got.rows.iter().zip(&want.rows) {
        assert_eq!(g[1..], w[1..]);
    }
    let xml = part(&bom, "xl/worksheets/sheet1.xml");
    assert!(
        xml.contains("<c r=\"C2\"><v>1</v></c>"),
        "row 2's quantity is a number"
    );
    assert!(xml.contains("<v>1.71</v>") && xml.contains("<v>3.17</v>"));

    let block = t.written(
        &args([&"export", &"report", &p, &"--block", &"908", &"--format", &"xlsx"]),
        "r.xlsx",
    );
    assert_eq!(sheet(&block, None).rows.len(), 56);

    let terminals = t.written(
        &args([&"export", &"terminals", &p, &"--format", &"xlsx"]),
        "t.xlsx",
    );
    assert_eq!(
        sheet_names(&terminals, &table_limits()).unwrap(),
        ["1X1", "-X1031", "-X1032", "_ew"]
    );
    let x1031 = sheet(&terminals, Some("-X1031"));
    let want =
        csv_table(&std::fs::read_to_string(Path::new(EXPECTED).join("csv/terminals_X1031.csv")).unwrap());
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
/// the quantities 1.71 and 3.17 written "1,71" and "3,17" in `excel-nl`); `--bom off --line-end lf` drops the
/// byte-order mark and writes LF; the default is still `bom_parts.csv`. The presets write CRLF, asserted here; the
/// committed files hold LF, as the repository's `eol=lf` stores every text file.
#[test]
fn the_bom_presets_are_the_committed_files() {
    let t = Tmp::new();
    let p = t.golden();
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
        golden_file(
            &format!("csv/presets/bom_parts.{preset}.csv"),
            text.replace("\r\n", "\n").as_bytes(),
        );
        texts.push(text);
    }
    let nl = &texts[0];
    assert!(nl.starts_with("\u{feff}id;Pos;Qty;Unit;Manufacturer;Part number;Description;Tags;Folios\r\n"));
    assert!(nl.contains(";1,71;m;") && nl.contains(";3,17;m;"), "{nl}");
    assert!(texts[1].starts_with("\u{feff}id,Pos,Qty,") && texts[1].contains(",1.71,m,"));
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
    let o = t.ew(&args([&"export", &"bom", &p]));
    assert_eq!(
        stdout(&o),
        std::fs::read_to_string(Path::new(EXPECTED).join("csv/bom_parts.csv")).unwrap()
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
        "error: `--preset` applies to CSV: `--format xlsx` takes none\n\nUsage: ew export bom [OPTIONS] \
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
    let t = Tmp::new();
    let p = t.golden();
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
    let t = Tmp::new();
    let p = t.golden();
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
    let t = Tmp::new();
    let p = t.golden();
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
    assert_eq!(v["strips"], serde_json::json!(["1X1", "-X1031", "-X1032"]));
    assert!(v["csv"].as_str().unwrap().starts_with("id,Terminal,"));
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
