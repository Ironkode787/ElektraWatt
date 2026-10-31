// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.22 part 2 through the `ew` binary: the page and row outputs (OU-1 to OU-4; OU-5 is the container job of
//! CI-PR-05, OU-6's `--json` rules are in `tests/conventions.rs`).
//!
//! The golden facts of the specification are checked against Golden A's committed files: folio 103's PNG bytes (and
//! every folio's), the 56 channel rows of folio 908, the 44 wires and 6 bridges of the wire list, the three strips of
//! the terminal diagrams.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory with its own user directories, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-outputs-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    /// `ew` in this directory with its user directories inside it, no starter content, no display and no
    /// `SOURCE_DATE_EPOCH`.
    fn ew(&self, args: &[OsString]) -> Output {
        let mut cmd = Command::new(bin_exe());
        cmd.args(args)
            .current_dir(&self.0)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("SOURCE_DATE_EPOCH")
            .env_remove("EW_CONTENT_DIR")
            .env("EW_DATA_DIR", self.0.join("data"))
            .env("EW_CACHE_DIR", self.0.join("cache"));
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

    /// A copy of Golden A.
    fn golden(&self) -> PathBuf {
        let copy = self.0.join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &copy);
        copy
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

/// The arguments: words and paths.
fn args<const N: usize>(items: [&dyn AsRef<OsStr>; N]) -> Vec<OsString> {
    items.iter().map(|a| a.as_ref().to_owned()).collect()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// A committed expected file.
fn expected(rel: &str) -> Vec<u8> {
    let path = Path::new(EXPECTED).join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn expected_text(rel: &str) -> String {
    String::from_utf8(expected(rel)).unwrap()
}

/// The pixel size of a PNG (its IHDR chunk).
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "a PNG signature");
    assert_eq!(&bytes[12..16], b"IHDR");
    let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

/// Every file of a directory with its bytes, by name.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect()
}

/// The data rows of a golden-dialect CSV.
fn data_rows(csv: &str) -> usize {
    csv.lines().filter(|l| l.starts_with("data,")).count()
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

// ---------------------------------------------------------------------------------------------------------
// OU-1: `ew render`

/// OU-1 (FR-EXT-004): `ew render --page 103 --png --out 103.png` writes the bytes of `expected/png/103.png`; without
/// `--out` the PNG goes to `103.png` in the working directory and nothing to stdout but the summary.
#[test]
fn render_writes_the_golden_png_of_folio_103() {
    let t = Tmp::new();
    let p = t.golden();
    let out = t.0.join("out.png");
    let o = t.ew(&args([
        &"render", &p, &"--page", &"103", &"--png", &"--out", &out,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let want = expected("png/103.png");
    assert!(
        std::fs::read(&out).unwrap() == want,
        "103.png differs from the golden PNG"
    );
    let o = t.ew(&args([&"render", &"--project", &p, &"--page", &"103", &"--png"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        std::fs::read(t.0.join("103.png")).unwrap() == want,
        "the default file is <folio>.png in the working directory"
    );
    assert!(!stdout(&o).contains("PNG\r\n"), "no PNG bytes on stdout");
}

/// OU-1: `--dpi 300` doubles both pixel dimensions of the 150 dpi default.
#[test]
fn render_at_300_dpi_doubles_both_dimensions() {
    let t = Tmp::new();
    let p = t.golden();
    let out = t.0.join("103-300.png");
    let o = t.ew(&args([
        &"render", &p, &"--page", &"103", &"--png", &"--dpi", &"300", &"--out", &out,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let (w, h) = png_size(&expected("png/103.png"));
    assert_eq!(png_size(&std::fs::read(&out).unwrap()), (2 * w, 2 * h));
}

/// OU-1: an unknown folio exits 2 naming it and writes nothing; a `--dpi` outside 72 to 600 is a usage error.
#[test]
fn render_refuses_an_unknown_folio_and_a_bad_resolution() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([&"render", &p, &"--page", &"999", &"--png"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: page selection: the page order has no folio `999`\n"
    );
    assert!(!t.0.join("999.png").exists());
    let o = t.ew(&args([
        &"render", &p, &"--page", &"103", &"--png", &"--dpi", &"601",
    ]));
    assert_eq!(o.status.code(), Some(2));
    // A usage error: the line, then the usage text.
    assert_eq!(
        stderr(&o).lines().next(),
        Some("error: `--dpi` needs a number from 72 to 600, got `601`")
    );
}

/// A copy of Golden A whose A3 frame forms declare a sheet of `width` × `height` millimetres.
fn golden_with_sheet(t: &Tmp, width_mm: i64, height_mm: i64) -> PathBuf {
    let p = t.golden();
    let a3 = r#""sheet": {"width": 297000000, "height": 210000000}"#;
    let wide = format!(
        r#""sheet": {{"width": {}, "height": {}}}"#,
        width_mm * 1_000_000,
        height_mm * 1_000_000
    );
    for e in std::fs::read_dir(p.join("lib").join("forms")).unwrap() {
        let path = e.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        if text.contains(a3) {
            std::fs::write(&path, text.replace(a3, &wide)).unwrap();
        }
    }
    p
}

/// Cycle-40 review, part 3, finding 1: a page whose raster would exceed 150 million pixels is refused before anything
/// is allocated (exit 2, the whole text), never an aborted allocation. A sheet tens of metres wide is too large at
/// every resolution; a 4 m sheet names the highest whole dpi that fits. `export png` refuses it the same way and
/// writes nothing.
#[test]
fn a_page_too_large_to_rasterise_is_refused_with_words() {
    let t = Tmp::new();
    let p = golden_with_sheet(&t, 4_000, 2_000);
    let o = t.ew(&args([&"render", &p, &"--page", &"103", &"--png"]));
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert_eq!(
        stderr(&o),
        "error: PNG: Folio 103 would be 23622 × 11811 pixels at 150 dpi; the limit is 150 million pixels. \
         Choose 109 dpi or less.\n"
    );
    assert!(!t.0.join("103.png").exists());
    let dir = t.0.join("png");
    let o = t.ew(&args([&"export", &"png", &p, &"--out", &dir, &"--pages", &"103"]));
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert_eq!(
        stderr(&o),
        "error: PNG: Folio 103 would be 23622 × 11811 pixels at 150 dpi; the limit is 150 million pixels. \
         Choose 109 dpi or less.\n"
    );
    assert!(!dir.exists());

    let t = Tmp::new();
    let p = golden_with_sheet(&t, 20_000, 20_000);
    let o = t.ew(&args([
        &"render", &p, &"--page", &"103", &"--png", &"--dpi", &"72",
    ]));
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert_eq!(
        stderr(&o),
        "error: PNG: Folio 103 would be 56693 × 56693 pixels at 72 dpi; the limit is 150 million pixels. \
         The page is too large at every resolution from 72 dpi.\n"
    );
}

// ---------------------------------------------------------------------------------------------------------
// OU-2: `ew export png`

/// OU-2 (FT-254): `ew export png --out <dir>` writes one PNG per folio of Golden A, each equal to its committed PNG,
/// on one thread and on four (`--threads` changes the speed only).
#[test]
fn export_png_writes_every_golden_png_in_one_and_four_threads() {
    let t = Tmp::new();
    let p = t.golden();
    let committed = files(&Path::new(EXPECTED).join("png"));
    for threads in ["1", "4"] {
        let dir = t.0.join(format!("png-{threads}"));
        let o = t.ew(&args([
            &"export",
            &"png",
            &p,
            &"--out",
            &dir,
            &"--threads",
            &threads,
        ]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let written = files(&dir);
        assert_eq!(
            written.keys().collect::<Vec<_>>(),
            committed.keys().collect::<Vec<_>>(),
            "one PNG per folio"
        );
        let differing: Vec<&String> = written
            .iter()
            .filter(|(name, bytes)| committed.get(*name) != Some(bytes))
            .map(|(name, _)| name)
            .collect();
        assert!(differing.is_empty(), "{threads} threads: {differing:?} differ");
    }
}

/// OU-2: a directory holding a file that is not a PNG is refused (exit 2) and left as it was; a PNG-only directory is
/// replaced as a whole.
#[test]
fn export_png_refuses_a_directory_with_another_file() {
    let t = Tmp::new();
    let p = t.golden();
    let dir = t.0.join("mixed");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "keep me").unwrap();
    let o = t.ew(&args([&"export", &"png", &p, &"--out", &dir, &"--pages", &"103"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        format!(
            "error: {}: the directory holds notes.txt, which is not a PNG file; choose an empty or PNG-only directory\n",
            dir.display()
        )
    );
    assert_eq!(files(&dir).keys().collect::<Vec<_>>(), ["notes.txt"]);
    let clean = t.0.join("clean");
    std::fs::create_dir_all(&clean).unwrap();
    std::fs::write(clean.join("stale.png"), "old").unwrap();
    let o = t.ew(&args([
        &"export", &"png", &p, &"--out", &clean, &"--pages", &"103",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(files(&clean).keys().collect::<Vec<_>>(), ["103.png"]);
}

// ---------------------------------------------------------------------------------------------------------
// OU-3: `ew export report`

/// OU-3 (FR-EXT-027, doc 20 §11.3): `ew export report --block 908 --format csv` writes the 56 channel rows of folios
/// 908 to 908.2; by the block's ID it writes the same.
#[test]
fn report_csv_writes_the_56_channel_rows_of_folio_908() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([
        &"export",
        &"report",
        &p,
        &"--block",
        &"908",
        &"--format",
        &"csv",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let csv = stdout(&o);
    assert_eq!(data_rows(&csv), 56, "{csv}");
    assert!(csv.starts_with("row,Module,"), "{csv}");
    let by_id = t.ew(&args([&"export", &"report", &p, &"--block", &block_id(908)]));
    assert_eq!(stdout(&by_id), csv);
}

/// OU-3: `--format pdf` writes the PDF of folios 908, 908.1 and 908.2 only: the bytes of `ew export pdf --pages
/// 908,908.1,908.2`.
#[test]
fn report_pdf_writes_the_block_folios_only() {
    let t = Tmp::new();
    let p = t.golden();
    let report = t.0.join("report.pdf");
    let o = t.ew(&args([
        &"export",
        &"report",
        &p,
        &"--block",
        &"908",
        &"--format",
        &"pdf",
        &"--out",
        &report,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        stdout(&o).contains("3 pages (908, 908.1, 908.2)"),
        "{}",
        stdout(&o)
    );
    let pages = t.0.join("pages.pdf");
    let o = t.ew(&args([
        &"export",
        &"pdf",
        &p,
        &"--out",
        &pages,
        &"--pages",
        &"908,908.1,908.2",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(std::fs::read(&report).unwrap() == std::fs::read(&pages).unwrap());
}

/// OU-3: `--format xlsx` exits 2 naming FT-61; an unknown block exits 2 naming it.
#[test]
fn report_refuses_xlsx_and_an_unknown_block() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([
        &"export",
        &"report",
        &p,
        &"--block",
        &"908",
        &"--format",
        &"xlsx",
    ]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o).lines().next(),
        Some("error: `--format xlsx` is not built yet (csv, pdf; XLSX waits for the writer of FT-61)")
    );
    let o = t.ew(&args([&"export", &"report", &p, &"--block", &"777"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: no report block `777` (name a block by its ID or its first folio)\n"
    );
}

// ---------------------------------------------------------------------------------------------------------
// OU-4: `ew export wires|terminals`, `--block`

/// OU-4: `ew export wires` equals `expected/csv/wires.csv` (44 wires and 6 bridges).
#[test]
fn export_wires_equals_the_golden_wire_list() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([&"export", &"wires", &p]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let want = expected_text("csv/wires.csv");
    assert_eq!(stdout(&o), want);
    assert_eq!(data_rows(&want), 44 + 6);
    // By its folio, to a file.
    let out = t.0.join("wires.csv");
    let o = t.ew(&args([
        &"export", &"wires", &p, &"--block", &"907", &"--out", &out,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), want);
}

/// OU-4: `ew export terminals --strip -X1031` equals `expected/csv/terminals_X1031.csv`; without `--strip` the three
/// strips 1X1, -X1031 and -X1032 follow each other, each after a heading row, with the rows of its committed file.
#[test]
fn export_terminals_writes_one_strip_or_every_strip() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([&"export", &"terminals", &p, &"--strip", &"-X1031"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), expected_text("csv/terminals_X1031.csv"));
    let o = t.ew(&args([&"export", &"terminals", &p]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let all = stdout(&o);
    let mut want = String::new();
    for (tag, file) in [("1X1", "1X1"), ("-X1031", "X1031"), ("-X1032", "X1032")] {
        let text = expected_text(&format!("csv/terminals_{file}.csv"));
        let (header, body) = text.split_once('\n').unwrap();
        if want.is_empty() {
            want.push_str(header);
            want.push('\n');
        }
        let columns = header.split(',').count();
        want.push_str(&format!("heading,{tag}{}\n", ",".repeat(columns - 2)));
        want.push_str(body);
    }
    assert_eq!(all, want);
    let counts: Vec<usize> = all.split("\nheading,").skip(1).map(data_rows).collect();
    assert_eq!(counts, [3, 20, 5]);
}

/// OU-4: a strip that is not there exits 2 naming it; a block of another kind exits 2 naming its kind.
#[test]
fn row_exports_refuse_an_unknown_strip_and_a_block_of_another_kind() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([&"export", &"terminals", &p, &"--strip", &"-X9"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: no terminal strip `-X9` in the terminal diagrams (they list 1X1, -X1031, -X1032)\n"
    );
    let o = t.ew(&args([&"export", &"wires", &p, &"--block", &"908"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: the report block `908` is of kind `plc_overview`, not `wire_list`\n"
    );
    let o = t.ew(&args([
        &"export",
        &"terminals",
        &p,
        &"--block",
        &"904",
        &"--strip",
        &"1X1",
    ]));
    assert_eq!(o.status.code(), Some(2), "1X1 is not on folio 904's block");
    assert_eq!(
        stderr(&o),
        "error: no terminal strip `1X1` in the terminal diagrams (they list -X1031)\n"
    );
}

/// OU-4: `ew export bom --block <id of 901>` equals the default bill of materials, in both modes; `cables --block 906`
/// equals the default cable list.
#[test]
fn a_named_block_equals_the_default_one() {
    let t = Tmp::new();
    let p = t.golden();
    let id = block_id(901);
    for mode in ["parts", "assembly"] {
        let default = t.ew(&args([&"export", &"bom", &p, &"--mode", &mode]));
        let named = t.ew(&args([&"export", &"bom", &p, &"--mode", &mode, &"--block", &id]));
        assert_eq!(named.status.code(), Some(0), "{}", stderr(&named));
        assert_eq!(stdout(&named), stdout(&default), "{mode}");
    }
    assert_eq!(
        stdout(&t.ew(&args([&"export", &"bom", &p]))),
        expected_text("csv/bom_parts.csv")
    );
    let default = t.ew(&args([&"export", &"cables", &p]));
    let named = t.ew(&args([&"export", &"cables", &p, &"--block", &"906"]));
    assert_eq!(named.status.code(), Some(0), "{}", stderr(&named));
    assert_eq!(stdout(&named), stdout(&default));
}
