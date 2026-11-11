// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.22 part 2 through the `ew` binary: the page and row outputs (OU-1 to OU-4; OU-5 is the container job of
//! CI-PR-05, OU-6's `--json` rules are in `tests/conventions.rs`).
//!
//! The golden facts of the specification are checked against Golden A's committed files: folio 103's PNG bytes (and
//! every folio's), the 56 channel rows of folio 908, the 44 wires and 6 bridges of the wire list, the three strips of
//! the terminal diagrams.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Output;

use crate::support::{EwTmp, GOLDEN_A, Tmp, args, stderr, stdout};

const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

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
/// `--out` the PNG goes to `103.png` in the working directory and nothing to stdout but the summary; `--dpi 300`
/// doubles both pixel dimensions.
#[test]
fn render_writes_the_golden_png_of_folio_103() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
    // `--dpi 300` doubles both pixel dimensions of the 150 dpi default.
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
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
    // A usage error: clap's rendering of the value parser's refusal.
    assert_eq!(
        stderr(&o),
        "error: invalid value '601' for '--dpi <n>': `--dpi` needs a number from 72 to 600, got `601`\n\nFor more \
         information, try '--help'.\n"
    );
}

/// A copy of Golden A whose A3 frame forms declare a sheet of `width` × `height` millimetres.
fn golden_with_sheet(t: &Tmp, width_mm: i64, height_mm: i64) -> PathBuf {
    let p = t.golden("J-E-2416.ew");
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
    let t = Tmp::new("ew-outputs");
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

    let t = Tmp::new("ew-outputs");
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
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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

/// FT-1278 (GT-4's PNG half): the wire-colour view is a window state and never reaches an export. The view strokes
/// folio 103's W29 in DBU and W27 green and yellow; `ew export png` of folio 103 holds no coloured pixel.
#[test]
fn export_png_holds_no_wire_colour_view() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    let dir = t.0.join("png-103");
    let o = t.ew(&args([&"export", &"png", &p, &"--out", &dir, &"--pages", &"103"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let bytes = std::fs::read(dir.join("103.png")).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0u8; reader.output_buffer_size().expect("a size")];
    let info = reader.next_frame(&mut buf).expect("a frame");
    buf.truncate(info.buffer_size());
    // Folio 103 is drawn in grey: a coloured pixel could only be the view's. A grey PNG holds none.
    let coloured = match info.color_type {
        png::ColorType::Rgba => buf
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] != p[1] || p[1] != p[2])
            .count(),
        _ => 0,
    };
    assert_eq!(coloured, 0, "coloured pixels, the wire-colour view's");
}

/// OU-2: a directory holding a file that is not a PNG is refused (exit 2) and left as it was; a PNG-only directory is
/// replaced as a whole.
#[test]
fn export_png_refuses_a_directory_with_another_file() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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

/// OU-3: `--format xlsx` without `--out` exits 2 asking for the file; an unknown block exits 2 naming it.
#[test]
fn report_refuses_xlsx_without_a_file_and_an_unknown_block() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
        stderr(&o),
        "error: `--format xlsx` writes a file: give `--out <file.xlsx>`\n\nUsage: ew export report [OPTIONS] <--block \
         <block>|--kind <kind>> [project-dir]\n\nFor more information, try '--help'.\n"
    );
    let o = t.ew(&args([&"export", &"report", &p, &"--block", &"777"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: no report block `777` (name a block by its ID or its first folio)\n"
    );
}

// ---------------------------------------------------------------------------------------------------------
// `ew export report --kind <kind>` (D4), the revision overview's changed pages (D11), the block
// parameters on save (D9)

/// The kinds with a golden CSV written by the export by kind: the registered kinds with a file
/// `expected/csv/<kind>.csv`. `crates/ew-api/tests/it/report_kinds.rs` blesses them and fails when a kind the export
/// derives has no file and is not pending, so a deleted file fails there rather than shrinking this set unseen.
fn kind_csvs() -> Vec<&'static str> {
    let dir = Path::new(EXPECTED).join("csv");
    let kinds: Vec<&'static str> = ew_api::output::report_kinds()
        .into_iter()
        .filter(|k| dir.join(format!("{k}.csv")).is_file())
        .collect();
    assert!(!kinds.is_empty(), "no registered kind has a golden CSV");
    kinds
}

/// RK-1, RK-2, RK-7 and the orchestrator's ruling 10: `ew export report --kind <kind> --format csv` writes
/// `expected/csv/<kind>.csv` byte for byte, twice the same.
#[test]
fn report_kind_writes_the_golden_kind_csvs() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    for kind in kind_csvs() {
        let o = t.ew(&args([
            &"export",
            &"report",
            &p,
            &"--kind",
            &kind,
            &"--format",
            &"csv",
        ]));
        assert_eq!(o.status.code(), Some(0), "{kind}: {}", stderr(&o));
        assert!(
            o.stdout == expected(&format!("csv/{kind}.csv")),
            "{kind}: {}",
            stdout(&o)
        );
        let again = t.ew(&args([&"export", &"report", &p, &"--kind", &kind]));
        assert!(again.stdout == o.stdout, "{kind}: twice-run equal");
    }
}

/// RK-3 (the orchestrator's rulings 4 and 5): the refusals of the export by kind, each whole on stderr with
/// exit 2; the unknown-kind refusal lists the registered kinds, each of which but `terminal_diagram` exports.
#[test]
fn report_kind_refusals_and_every_listed_kind_exports() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    let o = t.ew(&args([
        &"export",
        &"report",
        &p,
        &"--kind",
        &"device_list",
        &"--block",
        &"910",
    ]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: give `--block` or `--kind`, not both\n\nUsage: ew export report [OPTIONS] <--block <block>|--kind \
         <kind>> [project-dir]\n\nFor more information, try '--help'.\n"
    );
    let pdf = t.0.join("k.pdf");
    let o = t.ew(&args([
        &"export",
        &"report",
        &p,
        &"--kind",
        &"device_list",
        &"--format",
        &"pdf",
        &"--out",
        &pdf,
    ]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: `--kind` exports rows: a kind has pages only through a block, use `--block <block> --format pdf`\n\n\
         Usage: ew export report [OPTIONS] <--block <block>|--kind <kind>> [project-dir]\n\nFor more information, try \
         '--help'.\n"
    );
    let o = t.ew(&args([&"export", &"report", &p]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: the following required arguments were not provided:\n  <--block <block>|--kind <kind>>\n\nUsage: ew \
         export report <--block <block>|--kind <kind>> <project-dir>\n\nFor more information, try '--help'.\n"
    );
    let o = t.ew(&args([&"export", &"report", &p, &"--kind", &"terminal_diagram"]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: `terminal_diagram` lists one strip: use `ew export terminals`\n"
    );
    let o = t.ew(&args([&"export", &"report", &p, &"--kind", &"device_lists"]));
    assert_eq!(o.status.code(), Some(2));
    let text = stderr(&o);
    // The list is the registry's (a growing set, not pinned here): the text is the fixed sentence, then the names in
    // order, each once, and each of them is a kind of this build (it exports, or is `terminal_diagram`).
    let names = text
        .strip_prefix("error: unknown report kind `device_lists`; the kinds of this build are: ")
        .and_then(|rest| rest.strip_suffix('\n'))
        .unwrap_or_else(|| panic!("{text}"));
    let names: Vec<&str> = names.split(", ").collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(names, sorted);
    for want in ["device_list", "terminal_overview", "terminal_diagram", "io_list"] {
        assert!(names.contains(&want), "{want} is listed: {names:?}");
    }
    for name in names.iter().filter(|n| **n != "terminal_diagram") {
        let o = t.ew(&args([&"export", &"report", &p, &"--kind", name]));
        assert_eq!(o.status.code(), Some(0), "{name}: {}", stderr(&o));
        assert!(stdout(&o).starts_with("row,"), "{name}: {}", stdout(&o));
    }
}

/// RK-6 (the orchestrator's ruling 7): `--block 911 --format csv` prints the changed pages as folio 911
/// does (V1.0 empty, V2.0 "101, 106"); `--kind revision_overview` writes the same bytes.
#[test]
fn report_csv_of_folio_911_prints_the_changed_pages() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    let o = t.ew(&args([
        &"export",
        &"report",
        &p,
        &"--block",
        &"911",
        &"--format",
        &"csv",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let csv = stdout(&o);
    let last = |label: &str| -> String {
        let line = csv
            .lines()
            .find(|l| l.starts_with(&format!("data,{label},")))
            .unwrap_or_else(|| panic!("{label}: {csv}"));
        match line.rsplit_once(",\"") {
            Some((_, quoted)) => quoted.trim_end_matches('"').to_owned(),
            None => line.rsplit(',').next().unwrap_or_default().to_owned(),
        }
    };
    assert_eq!(last("V1.0"), "");
    assert_eq!(last("V2.0"), "101, 106");
    let by_kind = t.ew(&args([&"export", &"report", &p, &"--kind", &"revision_overview"]));
    assert_eq!(stdout(&by_kind), csv);
}

/// RK-5 (D9): a block's `params` survive the save protocol byte for byte: `ew fmt` keeps the member verbatim and a
/// second check finds nothing to rewrite; `"Assembly"` (a value outside the kind) exports the component rows.
#[test]
fn block_params_survive_a_save_and_a_wrong_value_gives_the_default() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    let file = p.join("report-blocks.jsonl");
    let text = std::fs::read_to_string(&file).unwrap();
    let id = block_id(901);
    let edited: String = text
        .lines()
        .map(|l| {
            if l.contains(&format!("\"id\":\"{id}\"")) {
                let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
                v["params"] = serde_json::json!({"mode": "Assembly"});
                serde_json::to_string(&v).unwrap()
            } else {
                l.to_owned()
            }
        })
        .map(|l| l + "\n")
        .collect();
    assert_ne!(edited, text);
    std::fs::write(&file, &edited).unwrap();
    let o = t.ew(&args([&"fmt", &p]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let saved = std::fs::read(&file).unwrap();
    let line = String::from_utf8_lossy(&saved)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["id"] == id.as_str())
        .expect("the block's line");
    assert_eq!(
        line["params"],
        serde_json::json!({"mode": "Assembly"}),
        "the member is kept"
    );
    let o = t.ew(&args([&"fmt", &p, &"--check"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let o = t.ew(&args([&"export", &"report", &p, &"--block", &"901"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(o.stdout == expected("csv/bom_parts.csv"), "{}", stdout(&o));
    assert!(std::fs::read(&file).unwrap() == saved, "an export writes nothing");
}

// ---------------------------------------------------------------------------------------------------------
// OU-4: `ew export wires|terminals`, `--block`

/// OU-4: `ew export wires` equals `expected/csv/wires.csv` (44 wires and 6 bridges).
#[test]
fn export_wires_equals_the_golden_wire_list() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
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

// ---------------------------------------------------------------------------------------------------------
// `ew print` (PR-1, PR-2, D9)

/// The `--json` result of a run, parsed.
fn json(o: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(o)).unwrap_or_else(|e| panic!("{e}: {}", stdout(o)))
}

/// PR-1 and D8: `ew print golden/J-E-2416.ew --sections 1 --paper A5 --fit --out <file>` writes the print set whose
/// SHA-256 is `expected/print/sha256.txt` (written by `ew-api`'s page-output test), prints nothing, leaves no
/// temporary file; `--json` names the five folios at 70 %; `--paper A4 --fit` is 100 %; `--pages 101-106` prints
/// the four folios.
#[test]
fn print_of_golden_a_section_1_on_a5_is_the_committed_print_set() {
    use sha2::{Digest, Sha256};
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    let out = t.0.join("p.pdf");
    let o = t.ew(&args([
        &"print",
        &p,
        &"--sections",
        &"1",
        &"--paper",
        &"A5",
        &"--fit",
        &"--out",
        &out,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!((stdout(&o).as_str(), stderr(&o).as_str()), ("", ""));
    let digest = format!("{:x}\n", Sha256::digest(std::fs::read(&out).unwrap()));
    assert_eq!(digest, expected_text("print/sha256.txt"));
    assert!(!t.0.join("p.pdf.tmp").exists());
    let o = t.ew(&args([
        &"print",
        &p,
        &"--sections",
        &"1",
        &"--paper",
        &"a5",
        &"--fit",
        &"--out",
        &out,
        &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        json(&o),
        serde_json::json!({
            "$schema": "ew.print-result/1",
            "adapter": "file",
            "printer": "",
            "pages": ["100", "101", "102", "103", "106"],
            "paper": "A5",
            "scale": 70,
            "notices": [],
            "out": out.display().to_string(),
        })
    );
    let o = t.ew(&args([
        &"print",
        &p,
        &"--sections",
        &"1",
        &"--paper",
        &"A4",
        &"--fit",
        &"--out",
        &out,
        &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        (json(&o)["scale"].clone(), json(&o)["paper"].clone()),
        (100.into(), "A4".into())
    );
    let o = t.ew(&args([
        &"print", &p, &"--pages", &"101-106", &"--out", &out, &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(json(&o)["pages"], serde_json::json!(["101", "102", "103", "106"]));
}

/// PR-2: a page on an A3 sheet printed on A4 with the fit reads 71 %; without it the page is clipped and the notice
/// names it, on stderr and in the result.
#[test]
fn print_of_an_a3_page_on_a4_is_scaled_or_clipped_with_a_notice() {
    let t = Tmp::new("ew-outputs");
    let p = golden_with_sheet(&t, 420, 297);
    let out = t.0.join("a3.pdf");
    let o = t.ew(&args([
        &"print", &p, &"--pages", &"103", &"--paper", &"A4", &"--fit", &"--out", &out, &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(json(&o)["scale"], 71);
    assert_eq!(json(&o)["notices"], serde_json::json!([]));
    let o = t.ew(&args([
        &"print", &p, &"--pages", &"103", &"--paper", &"A4", &"--out", &out, &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let notice = "page 103 is larger than A4: it is printed at 100 % and clipped from its top-left corner";
    assert_eq!(json(&o)["scale"], 100);
    assert_eq!(json(&o)["notices"], serde_json::json!([notice]));
    assert_eq!(stderr(&o), format!("notice: {notice}\n"));
}

/// D6 and D9: every refusal of `ew print`, whole, with exit 2 and nothing written; `--sections` on `export pdf`
/// refuses `--pages` beside it the same way.
#[test]
fn print_refusals_are_whole_texts() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    let out = t.0.join("r.pdf");
    let usage = |message: &str, usage: &str| {
        format!("error: {message}\n\nUsage: {usage}\n\nFor more information, try '--help'.\n")
    };
    let cases: Vec<(Vec<OsString>, String)> = vec![
        (
            args([&"print", &p, &"--pages", &"101", &"--sections", &"1", &"--out", &out]),
            usage(
                "give `--pages` or `--sections`, not both",
                "ew print [OPTIONS] [project-dir]",
            ),
        ),
        (
            args([&"export", &"pdf", &p, &"--pages", &"101", &"--sections", &"1", &"--out", &out]),
            usage(
                "give `--pages` or `--sections`, not both",
                "ew export pdf [OPTIONS] --out <path> [project-dir]",
            ),
        ),
        (
            args([&"print", &p, &"--paper", &"A7", &"--out", &out]),
            "error: invalid value 'A7' for '--paper <size>': unknown paper `A7` (sheet, A5, A4, A3, A2, A1, A0, \
             Letter, Legal, Tabloid)\n\nFor more information, try '--help'.\n"
                .to_owned(),
        ),
        (
            args([&"print", &p, &"--sections", &"7", &"--out", &out]),
            "error: page selection: no section `7` (the sections are 1, 2, 9)\n".to_owned(),
        ),
        (
            args([&"print", &p, &"--via", &"viewer", &"--printer", &"office"]),
            usage(
                "`--printer` needs `--via lp` or `--via windows`; with the viewer, choose the printer in its print \
                 dialog",
                "ew print [OPTIONS] [project-dir]",
            ),
        ),
    ];
    // Ruling 4 of ID-1218: refused in words on other systems; on Windows it would open the print dialog, so the
    // adapter's own tests cover it there.
    #[cfg(not(windows))]
    let cases = {
        let mut cases = cases;
        cases.push((
            args([&"print", &p, &"--via", &"windows"]),
            usage(
                "`--via windows` prints on Windows only",
                "ew print [OPTIONS] [project-dir]",
            ),
        ));
        cases
    };
    for (line, want) in cases {
        let o = t.ew(&line);
        assert_eq!(o.status.code(), Some(2), "{line:?}");
        assert_eq!(stderr(&o), want, "{line:?}");
        assert_eq!(stdout(&o), "", "{line:?}");
        assert!(!out.exists(), "{line:?}");
    }
}

/// `--sections` on `export pdf`: the section's pages with its divider, the same PDF as the same folios listed.
#[test]
fn export_pdf_takes_sections() {
    let t = Tmp::new("ew-outputs");
    let p = t.golden("J-E-2416.ew");
    let a = t.0.join("a.pdf");
    let b = t.0.join("b.pdf");
    let o = t.ew(&args([&"export", &"pdf", &p, &"--sections", &"1", &"--out", &a]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let o = t.ew(&args([
        &"export",
        &"pdf",
        &p,
        &"--pages",
        &"100,101-106",
        &"--out",
        &b,
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(std::fs::read(&a).unwrap() == std::fs::read(&b).unwrap());
}
