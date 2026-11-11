// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.22 part 1 through the `ew` binary: the conventions of doc 17 §3.1 (`--project`, `--json`) and the published
//! result schemas (CL-1 to CL-5, CL-7; CL-6 is in `tests/it/cli.rs`), with part 2's subcommands under the same rules
//! (OU-6; their outputs are checked in `tests/outputs.rs`).
//!
//! The result schemas under `bin/ew/schemas/` are an equality snapshot of `ew_api::cli_results::schema_files()`;
//! `EW_CLI_BLESS=1 cargo nextest run -p ew --test it -E 'test(/^conventions::/)'` rewrites the directory. Every `--json` output is
//! checked three ways: it validates against its committed schema file (an independent JSON Schema validator,
//! `boon`), it deserialises into its type (`deny_unknown_fields`) and re-serialises byte for byte, and a second run
//! prints the same bytes.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use ew_api::cli_results as results;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::support::{EwTmp, GOLDEN_A, GOLDEN_PARTS, GOLDEN_TABLES, Tmp, args, stderr, stdout, tree};

const VARIANTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/variants");
const QET: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors/></diagram></project>"#;

/// A line that is not JSON after the first line of the project's first page file (`ew fmt --repair`'s case, WP-S1.17
/// ID-1185), with the file's hash in the manifest as a save of those bytes writes it (`saved`), or written by hand, the
/// manifest untouched.
fn with_a_broken_line(project: &Path, saved: bool) {
    use sha2::Digest;
    let hash = |b: &[u8]| {
        let hex: String = sha2::Sha256::digest(b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect();
        format!("sha256:{hex}")
    };
    let first = tree(&project.join("pages"))
        .into_keys()
        .next()
        .expect("a page file");
    let file = project.join("pages").join(first);
    let old = std::fs::read(&file).unwrap();
    let at = old.iter().position(|b| *b == b'\n').expect("a first line") + 1;
    let new = [&old[..at], &b"{\"$k\":\"page\",\n"[..], &old[at..]].concat();
    if saved {
        let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
        assert_eq!(manifest.matches(&hash(&old)).count(), 1);
        std::fs::write(
            project.join("manifest.json"),
            manifest.replace(&hash(&old), &hash(&new)),
        )
        .unwrap();
    }
    std::fs::write(&file, new).unwrap();
}

/// The result schemas by file name, as `ew-api` generates them.
fn schema_files() -> BTreeMap<String, String> {
    results::schema_files().into_iter().collect()
}

// ---------------------------------------------------------------------------------------------------------
// The published schemas (D5)

/// The result schemas are written for the user guide where `EW_REFERENCE_DIR` asks for them.
// The variable only chooses whether the generated files are also written out.
#[allow(clippy::disallowed_methods)]
#[test]
fn the_result_schemas_are_written_where_the_reference_is_asked_for() {
    let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(|d| PathBuf::from(d).join("results")) else {
        return;
    };
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in schema_files() {
        std::fs::write(dir.join(name), text).unwrap();
    }
}

/// The schema files and their index: every result of the index has its file, every file is a draft 2020-12
/// document the independent validator compiles, LF with a final newline.
#[test]
fn every_result_schema_compiles() {
    let files = schema_files();
    let index: serde_json::Value = serde_json::from_str(&files["index.json"]).unwrap();
    for r in index["results"].as_array().unwrap() {
        let file = r["schema"].as_str().unwrap();
        let text = files.get(file).unwrap_or_else(|| panic!("{file} missing"));
        assert!(text.ends_with("}\n") && !text.contains('\r'), "{file}");
        let schema: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(schema["title"], r["name"], "{file}");
        let _ = compile(r["name"].as_str().unwrap());
    }
}

/// The schema `name`, compiled by the independent validator.
fn compile(name: &str) -> (boon::Schemas, boon::SchemaIndex) {
    let file = results::file_name(name);
    let text = schema_files()
        .remove(&file)
        .unwrap_or_else(|| panic!("no result schema {file}"));
    let schema: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    let url = format!("urn:ew:{}", file);
    compiler.add_resource(&url, schema).unwrap();
    let index = compiler
        .compile(&url, &mut schemas)
        .unwrap_or_else(|e| panic!("{file}: {e}"));
    (schemas, index)
}

/// CL-2: the output is one JSON object whose first member is `$schema`, naming `name`, a file of the index; it
/// validates against that file, deserialises into `T` with `deny_unknown_fields` and re-serialises byte-equal.
pub(super) fn assert_result<T: Serialize + DeserializeOwned>(out: &str, name: &str) -> T {
    assert!(
        out.starts_with(&format!("{{\n  \"$schema\": \"{name}\"")),
        "the first member names {name}: {out}"
    );
    let index: serde_json::Value = serde_json::from_str(&schema_files()["index.json"]).unwrap();
    assert!(
        index["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["name"] == name),
        "{name} is in index.json"
    );
    let value: serde_json::Value = serde_json::from_str(out).unwrap_or_else(|e| panic!("{name}: {e}: {out}"));
    let (schemas, index) = compile(name);
    if let Err(e) = schemas.validate(&value, index) {
        panic!("{name}: the output does not validate against its schema: {e:#}\n{out}");
    }
    let typed: T = serde_json::from_str(out).unwrap_or_else(|e| panic!("{name}: {e}: {out}"));
    assert_eq!(results::to_text(&typed), out, "{name}: the round trip");
    typed
}

// ---------------------------------------------------------------------------------------------------------
// CL-1: `--project`

/// CL-1: for every subcommand that takes a project, the positional directory and `--project <dir>` give equal stdout
/// and exit codes on Golden A's copy, and both together exit 2 with D3's message. The human-only subcommands are
/// compared off a terminal (both forms exit 2 before the project is opened).
#[test]
fn project_flag_equals_the_positional_directory() {
    let t = Tmp::new("ew-conventions");
    let p = t.golden("J-E-2416.ew");
    let tiny = t.tiny("tiny.ew");
    let sqlite = t.0.join("p.sqlite");
    let pdf = t.0.join("p.pdf");
    let svg = t.0.join("svg");
    let png_dir = t.0.join("png");
    let png = t.0.join("103.png");
    let report_pdf = t.0.join("908.pdf");
    let ewz = t.0.join("p.ewz");
    let parts = t.0.join("parts.csv");
    std::fs::copy(GOLDEN_PARTS, &parts).unwrap();
    // (the words before the project, the words after it, the project)
    let cases: Vec<(Vec<&str>, Vec<OsString>, &Path)> = vec![
        (vec!["check"], args([&"--json"]), &p),
        (
            vec!["sql"],
            args([&"SELECT count(*) AS n FROM api_v1_device"]),
            &p,
        ),
        (vec!["sql", "dump"], args([&sqlite]), &p),
        (vec!["export", "cables"], vec![], &p),
        (vec!["export", "bom"], args([&"--json"]), &p),
        (
            vec!["export", "pdf"],
            args([&"--out", &pdf, &"--pages", &"101"]),
            &p,
        ),
        (
            vec!["export", "svg"],
            args([&"--out", &svg, &"--pages", &"101"]),
            &p,
        ),
        (vec!["export", "wires"], vec![], &p),
        (vec!["export", "terminals"], args([&"--strip", &"-X1031"]), &p),
        (
            vec!["export", "report"],
            args([&"--block", &"908", &"--json"]),
            &p,
        ),
        (
            vec!["export", "report"],
            args([&"--block", &"908", &"--format", &"pdf", &"--out", &report_pdf]),
            &p,
        ),
        (
            vec!["export", "png"],
            args([&"--out", &png_dir, &"--pages", &"103"]),
            &p,
        ),
        (
            vec!["render"],
            args([&"--page", &"103", &"--png", &"--out", &png]),
            &p,
        ),
        (vec!["export", "ewz"], args([&"--out", &ewz, &"--json"]), &p),
        (vec!["verify-derived"], args([&"--json"]), &tiny),
        (vec!["layout"], args([&"--page", &"101", &"--json"]), &p),
        (vec!["fmt"], args([&"--check", &"--json"]), &p),
        (vec!["recover"], args([&"--json"]), &p),
        (vec!["project", "assets", "purge"], args([&"--preview"]), &p),
        (
            vec!["project", "checks", "set"],
            args([&"--active", &"drafting"]),
            &p,
        ),
        (vec!["revision", "approve"], args([&"--as", &"checker"]), &p),
        (vec!["revision", "release"], args([&"A"]), &p),
    ];
    for (words, rest, project) in cases {
        let words: Vec<OsString> = words.iter().map(OsString::from).collect();
        let mut positional = words.clone();
        // `revision release <label> [<project-dir>]`: the label comes first.
        let label_first = words.len() > 1 && words[1] == "release" && words[0] == "revision";
        if label_first {
            positional.extend(rest.iter().cloned());
            positional.push(project.as_os_str().to_owned());
        } else {
            positional.push(project.as_os_str().to_owned());
            positional.extend(rest.iter().cloned());
        }
        let mut flagged = words.clone();
        flagged.extend(rest.iter().cloned());
        flagged.push("--project".into());
        flagged.push(project.as_os_str().to_owned());
        let a = t.ew(&positional);
        let b = t.ew(&flagged);
        assert_eq!(
            (a.status.code(), stdout(&a)),
            (b.status.code(), stdout(&b)),
            "{positional:?} against {flagged:?}: {} / {}",
            stderr(&a),
            stderr(&b)
        );
        assert!(
            matches!(a.status.code(), Some(0..=2)),
            "{positional:?}: {}",
            stderr(&a)
        );
        // Both together: a usage error with D3's rule, in clap's words; the usage line is the subcommand's.
        let mut both = positional.clone();
        both.push("--project".into());
        both.push(project.as_os_str().to_owned());
        let o = t.ew(&both);
        assert_eq!(o.status.code(), Some(2), "{both:?}");
        assert_eq!(
            stderr(&o).lines().next(),
            Some("error: the argument '[project-dir]' cannot be used with '--project <dir>'"),
            "{both:?}: {}",
            stderr(&o)
        );
    }
    // `import parts` names its project only with `--project`.
    let target = t.tiny("target.ew");
    let o = t.ew(&args([&"import", &"parts", &parts, &"--project", &target]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

/// CL-1, FT-249: `ew apply --project <dir> <file.ewcmd>` replays as the positional form does: equal stdout and exit
/// code, and the two copies hold the same files afterwards.
#[test]
fn apply_with_the_project_flag_replays_as_the_positional_form() {
    let t = Tmp::new("ew-conventions");
    let a = t.golden("a.ew");
    let b = t.golden("b.ew");
    let log = Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let positional = t.ew(&args([&"apply", &a, &log]));
    let flagged = t.ew(&args([&"apply", &"--project", &b, &log]));
    assert_eq!(positional.status.code(), Some(0), "{}", stderr(&positional));
    assert_eq!(
        (positional.status.code(), stdout(&positional)),
        (flagged.status.code(), stdout(&flagged))
    );
    assert!(tree(&a) == tree(&b), "the two replays saved the same files");
    assert!(tree(&a) != tree(Path::new(GOLDEN_A)), "the replay is saved");
}

// ---------------------------------------------------------------------------------------------------------
// CL-2 and CL-7: `--json`

/// Runs `args` twice (CL-7: the same bytes both times) and checks the output as [`assert_result`] does.
fn json_twice<T: Serialize + DeserializeOwned>(t: &Tmp, args: &[OsString], name: &str, code: i32) -> T {
    let first = t.ew_with(args, true);
    assert_eq!(first.status.code(), Some(code), "{args:?}: {}", stderr(&first));
    let second = t.ew_with(args, true);
    assert_eq!(
        stdout(&first),
        stdout(&second),
        "{args:?}: two runs print the same bytes"
    );
    assert_result::<T>(&stdout(&first), name)
}

/// CL-2 and CL-7 for the subcommands that read a project or the libraries: each `--json` output validates against
/// its published schema, round-trips through its type and repeats byte for byte.
#[test]
fn every_reading_json_result_validates_round_trips_and_repeats() {
    let t = Tmp::new("ew-conventions");
    let p = t.golden("J-E-2416.ew");
    let tiny = t.tiny("tiny.ew");
    let check: results::CheckResult = json_twice(&t, &args([&"check", &p, &"--json"]), results::CHECK, 1);
    assert_eq!(check.scheme, "pre-delivery");
    let sql: results::SqlResult = json_twice(
        &t,
        &args([
            &"sql",
            &p,
            &"SELECT 1 AS i, 2.5 AS r, 'x' AS t, NULL AS z",
            &"--json",
        ]),
        results::SQL,
        0,
    );
    assert_eq!(sql.columns, ["i", "r", "t", "z"]);
    let sqlite = t.0.join("p.sqlite");
    let dump: results::SqlDumpResult = json_twice(
        &t,
        &args([&"sql", &"dump", &p, &sqlite, &"--json"]),
        results::SQL_DUMP,
        0,
    );
    assert_eq!(dump.out, sqlite.display().to_string());
    let cables: results::ExportRowsResult = json_twice(
        &t,
        &args([&"export", &"cables", &p, &"--json"]),
        results::EXPORT_CABLES,
        0,
    );
    assert!(cables.csv.is_some() && cables.out.is_none());
    let bom_file = t.0.join("bom.csv");
    let bom: results::ExportRowsResult = json_twice(
        &t,
        &args([&"export", &"bom", &p, &"--out", &bom_file, &"--json"]),
        results::EXPORT_BOM,
        0,
    );
    assert!(bom.csv.is_none() && bom.rows > 0);
    assert_eq!(
        std::fs::read_to_string(&bom_file).unwrap().lines().count(),
        bom.rows + 1
    );
    let pdf = t.0.join("p.pdf");
    let r: results::ExportPdfResult = json_twice(
        &t,
        &args([
            &"export", &"pdf", &p, &"--out", &pdf, &"--pages", &"101-102", &"--json",
        ]),
        results::EXPORT_PDF,
        0,
    );
    assert_eq!(r.pages, 2);
    let svg = t.0.join("svg");
    let r: results::ExportSvgResult = json_twice(
        &t,
        &args([
            &"export", &"svg", &p, &"--out", &svg, &"--pages", &"101-102", &"--json",
        ]),
        results::EXPORT_SVG,
        0,
    );
    assert_eq!(r.files, ["101.svg", "102.svg"]);
    // Part 2 (OU-6).
    let wires: results::ExportRowsResult = json_twice(
        &t,
        &args([&"export", &"wires", &p, &"--json"]),
        results::EXPORT_WIRES,
        0,
    );
    assert!(wires.csv.is_some() && wires.rows > 0);
    let terminals: results::ExportTerminalsResult = json_twice(
        &t,
        &args([&"export", &"terminals", &p, &"--json"]),
        results::EXPORT_TERMINALS,
        0,
    );
    assert_eq!(terminals.strips, ["1X1", "-X1031", "-X1032"]);
    let report: results::ExportReportResult = json_twice(
        &t,
        &args([&"export", &"report", &p, &"--block", &"908", &"--json"]),
        results::EXPORT_REPORT,
        0,
    );
    assert_eq!(
        (report.kind.as_str(), report.format.as_str()),
        ("plc_overview", "csv")
    );
    // WP-S3.06 D4 and the orchestrator's ruling 2: `--kind` prints the kind, the rows and `"block": ""`.
    let by_kind: results::ExportReportResult = json_twice(
        &t,
        &args([&"export", &"report", &p, &"--kind", &"device_list", &"--json"]),
        results::EXPORT_REPORT,
        0,
    );
    assert_eq!(
        (
            by_kind.block.as_str(),
            by_kind.kind.as_str(),
            by_kind.format.as_str(),
            by_kind.rows,
            by_kind.folios.is_none(),
        ),
        ("", "device_list", "csv", Some(68), true)
    );
    let report_pdf = t.0.join("908.pdf");
    let report: results::ExportReportResult = json_twice(
        &t,
        &args([
            &"export",
            &"report",
            &p,
            &"--block",
            &"908",
            &"--format",
            &"pdf",
            &"--out",
            &report_pdf,
            &"--json",
        ]),
        results::EXPORT_REPORT,
        0,
    );
    assert_eq!(
        report.folios.as_deref(),
        Some(&["908".to_owned(), "908.1".to_owned(), "908.2".to_owned()][..])
    );
    let png_dir = t.0.join("png");
    let r: results::ExportPngResult = json_twice(
        &t,
        &args([
            &"export", &"png", &p, &"--out", &png_dir, &"--pages", &"101-102", &"--dpi", &"72", &"--json",
        ]),
        results::EXPORT_PNG,
        0,
    );
    assert_eq!(
        (r.files.as_slice(), r.dpi),
        (&["101.png".to_owned(), "102.png".to_owned()][..], 72)
    );
    let png = t.0.join("103.png");
    let r: results::RenderResult = json_twice(
        &t,
        &args([
            &"render", &p, &"--page", &"103", &"--png", &"--out", &png, &"--json",
        ]),
        results::RENDER,
        0,
    );
    assert_eq!((r.page.as_str(), r.dpi), ("103", 150));
    let log = Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let preview: results::ApplyResult = json_twice(
        &t,
        &args([&"apply", &p, &log, &"--preview", &"--json"]),
        results::APPLY,
        0,
    );
    let changes = preview.preview.expect("the preview's changes");
    assert!(
        changes
            .tags
            .iter()
            .any(|c| c.before == "-106W1" && c.after == "-104W1"),
        "{changes:?}"
    );
    assert!(!preview.saved);
    let verify: results::VerifyDerivedResult = json_twice(
        &t,
        &args([&"verify-derived", &tiny, &"--json"]),
        results::VERIFY_DERIVED,
        0,
    );
    assert!(verify.differences.is_empty() && verify.oracle.is_empty());
    let layout: results::LayoutResult = json_twice(
        &t,
        &args([&"layout", &p, &"--page", &"101", &"--json"]),
        results::LAYOUT,
        0,
    );
    assert_eq!(layout.pages[0].folio, "101");
    // The index first: the first open of the tiers builds it, so a later listing reports it fresh every time.
    let index: results::LibIndexResult = json_twice(
        &t,
        &args([&"lib", &"index", &"--rebuild", &"--json"]),
        results::LIB_INDEX,
        0,
    );
    assert!(index.parts > 0);
    let list: results::LibListResult =
        json_twice(&t, &args([&"lib", &"list", &"--json"]), results::LIB_LIST, 0);
    assert!(list.libraries.iter().any(|l| l.key == "ew-starter"));
    let hits: results::LibSearchResult = json_twice(
        &t,
        &args([
            &"lib",
            &"search",
            &"ew.fn.mcb-1pn",
            &"--kind",
            &"fndef",
            &"--json",
        ]),
        results::LIB_SEARCH,
        0,
    );
    assert!(
        matches!(hits.hits.first(), Some(results::LibHit::Item(_))),
        "{hits:?}"
    );
    // A part hit (the compiled-in starter parts) takes the other shape.
    let parts: results::LibSearchResult = json_twice(
        &t,
        &args([&"lib", &"search", &"", &"--limit", &"1", &"--json"]),
        results::LIB_SEARCH,
        0,
    );
    assert!(
        matches!(parts.hits.first(), Some(results::LibHit::Part(_))),
        "{parts:?}"
    );
    let fmt: results::FmtResult = json_twice(&t, &args([&"fmt", &p, &"--check", &"--json"]), results::FMT, 0);
    assert_eq!(
        (fmt.check, fmt.files.len(), fmt.kept.len()),
        (true, 0, 0),
        "{fmt:?}"
    );
    let commands: results::CommandsResult =
        json_twice(&t, &args([&"commands", &"--json"]), results::COMMANDS, 0);
    assert!(!commands.commands.is_empty());
    let qet = t.0.join("t.qet");
    std::fs::write(&qet, QET).unwrap();
    let import: results::ImportQetResult = json_twice(
        &t,
        &args([&"import", &"qet", &qet, &"--json"]),
        results::IMPORT_QET,
        0,
    );
    assert_eq!((import.source.as_str(), import.counts.pages), ("t.qet", 1));
    // Nothing above wrote into the project.
    assert!(
        tree(&p) == tree(Path::new(GOLDEN_A)),
        "a reader wrote into the project"
    );
}

/// CL-2 and CL-7 for the subcommands that write: each `--json` output validates and round-trips; two runs on two
/// equal fresh targets print the same bytes (a library created with a new ID is the exception: its ID is new).
#[test]
fn every_writing_json_result_validates_and_round_trips() {
    let t = Tmp::new("ew-conventions");
    // `apply` on two copies.
    let log = Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let mut outs = Vec::new();
    for name in ["a.ew", "b.ew"] {
        let p = t.golden(name);
        let o = t.ew(&args([&"apply", &p, &log, &"--json"]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::ApplyResult = assert_result(&stdout(&o), results::APPLY);
        assert_eq!(
            (r.lines, r.committed, r.saved, r.preview.is_none()),
            (1, 1, true, true)
        );
        outs.push(stdout(&o));
    }
    assert_eq!(outs[0], outs[1]);
    // `import parts` into two fresh targets.
    let mut outs = Vec::new();
    for name in ["c.ew", "d.ew"] {
        let target = t.tiny(name);
        let o = t.ew(&args([
            &"import",
            &"parts",
            &GOLDEN_PARTS,
            &"--project",
            &target,
            &"--json",
        ]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::ImportPartsResult = assert_result(&stdout(&o), results::IMPORT_PARTS);
        assert!(r.created > 0 && r.rows.is_none(), "{r:?}");
        outs.push(stdout(&o));
    }
    assert_eq!(outs[0], outs[1]);
    // `lib create` and `lib import parts`.
    let lib = t.0.join("data").join("libraries").join("std.ewlib");
    let o = t.ew(&args([
        &"lib",
        &"create",
        &lib,
        &"--key",
        &"std",
        &"--name",
        &"Standard",
        &"--licence",
        &"CC0-1.0",
        &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let r: results::LibCreateResult = assert_result(&stdout(&o), results::LIB_CREATE);
    assert!(r.listed && r.key == "std", "{r:?}");
    let list = t.0.join("supplier.csv");
    let map = t.0.join("supplier.toml");
    std::fs::write(
        &list,
        "Maker,Order code,Text,Category\nPhoenix Contact,3209510,Feed-through terminal,terminal\n",
    )
    .unwrap();
    std::fs::write(
        &map,
        "[columns]\nmanufacturer = \"Maker\"\npart_number = \"Order code\"\ndescription = \"Text\"\nkind = \"Category\"\n",
    )
    .unwrap();
    let import = |extra: &str| {
        t.ew(&args([
            &"lib",
            &"import",
            &"parts",
            &"std",
            &list,
            &"--map",
            &map,
            &"--as-of",
            &"2026-10-29",
            &extra,
        ]))
    };
    let o = import("--json");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let r: results::LibImportPartsResult = assert_result(&stdout(&o), results::LIB_IMPORT_PARTS);
    assert_eq!((r.library.as_str(), r.rows.new), ("std", 1), "{r:?}");
    assert!(r.written.is_some_and(|w| w.added > 0), "{r:?}");
    // `fmt` on two copies with the same two lines swapped.
    let mut outs = Vec::new();
    for name in ["f.ew", "g.ew"] {
        let p = t.golden(name);
        let file = p.join("devices").join("71.jsonl");
        let text = std::fs::read_to_string(&file).unwrap();
        let mut lines: Vec<&str> = text.lines().collect();
        lines.swap(0, 1);
        std::fs::write(&file, format!("{}\n", lines.join("\n"))).unwrap();
        let o = t.ew(&args([&"fmt", &p, &"--json"]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::FmtResult = assert_result(&stdout(&o), results::FMT);
        assert_eq!((r.check, r.files.len(), r.generation), (false, 1, 2), "{r:?}");
        outs.push(stdout(&o));
    }
    assert_eq!(outs[0], outs[1]);
    // `fmt --repair` on two copies with the same broken line: the same bytes but for the backup's
    // path; then again on one, with nothing to repair. On a third copy the line is written by hand: the backup counts
    // the page file as edited outside EW.
    let mut outs = Vec::new();
    for (name, saved) in [("r1.ew", true), ("r2.ew", true), ("r3.ew", false)] {
        let p = t.golden(name);
        with_a_broken_line(&p, saved);
        let o = t.ew(&args([&"fmt", &p, &"--repair", &"--json"]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::FmtRepairResult = assert_result(&stdout(&o), results::FMT_REPAIR);
        assert_eq!(
            (r.discarded.len(), r.files.len(), r.kept.len(), r.generation),
            (1, 1, 0, 2),
            "{r:?}"
        );
        let backup = r.backup.expect("a backup");
        assert_eq!(
            (backup.generation, backup.reused, backup.edited),
            (1, false, usize::from(!saved))
        );
        let path = serde_json::to_string(&backup.path).unwrap();
        outs.push(stdout(&o).replace(&path, "\"<backup>\""));
    }
    assert_eq!(outs[0], outs[1]);
    let o = t.ew(&args([&"fmt", &t.0.join("r1.ew"), &"--repair", &"--json"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let r: results::FmtRepairResult = assert_result(&stdout(&o), results::FMT_REPAIR);
    assert_eq!(
        (r.discarded.len(), r.backup, r.generation),
        (0, None, 2),
        "nothing to repair"
    );
    // `recover` and `recover --replay` of a crashed session's journal on two copies.
    let mut outs = Vec::new();
    for name in ["h.ew", "i.ew"] {
        let p = t.golden(name);
        let mut store = ew_api::StoreOptions::new("test");
        store.state_dir = Some(t.state_dir());
        let (mut crashed, _) = ew_api::Session::open(
            &p,
            store,
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        crashed
            .execute(&ew_api::AnyCommand {
                name: "page.create".into(),
                params: serde_json::json!({"number": 7777, "title": {"en": "T"}}),
            })
            .unwrap();
        drop(crashed);
        let o = t.ew(&args([&"recover", &p, &"--json"]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::RecoverResult = assert_result(&stdout(&o), results::RECOVER);
        assert_eq!(
            (r.action, r.replayed, r.generation),
            (results::RecoverAction::Summary, false, 1),
            "{r:?}"
        );
        let o = t.ew(&args([&"recover", &p, &"--replay", &"--json"]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::RecoverResult = assert_result(&stdout(&o), results::RECOVER);
        assert_eq!(
            r.journal,
            Some(results::JournalSummary {
                transactions: 1,
                labels: vec!["page.create".to_owned()],
                base_matches: true,
                further: 0,
            })
        );
        assert_eq!(
            (r.action, r.replayed, r.generation),
            (results::RecoverAction::Replay, true, 2),
            "{r:?}"
        );
        outs.push(stdout(&o));
    }
    assert_eq!(outs[0], outs[1]);
    // `export ewz` twice to the same file, and `extract` twice into the same fresh target.
    let golden = t.golden("ewz.ew");
    let ewz = t.0.join("p.ewz");
    let r: results::ExportEwzResult = json_twice(
        &t,
        &args([&"export", &"ewz", &golden, &"--out", &ewz, &"--json"]),
        results::EXPORT_EWZ,
        0,
    );
    assert_eq!((r.generation, r.files), (1, 183), "{r:?}");
    let into = t.0.join("extracted.ew");
    let mut outs = Vec::new();
    for _ in 0..2 {
        let _ = std::fs::remove_dir_all(&into);
        let o = t.ew(&args([&"extract", &ewz, &"--into", &into, &"--json"]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::ExtractResult = assert_result(&stdout(&o), results::EXTRACT);
        assert_eq!((r.generation, r.files), (1, 183), "{r:?}");
        outs.push(stdout(&o));
    }
    assert_eq!(outs[0], outs[1]);
    // `import csv` twice into the same fresh target.
    let out = t.0.join("csv.ew");
    let mut outs = Vec::new();
    for _ in 0..2 {
        let _ = std::fs::remove_dir_all(&out);
        let o = t.ew_with(
            &args([&"import", &"csv", &GOLDEN_TABLES, &"--out", &out, &"--json"]),
            true,
        );
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let r: results::ImportCsvResult = assert_result(&stdout(&o), results::IMPORT_CSV);
        assert!(
            r.written && r.records > 0 && r.mode == results::ImportCsvMode::Strict,
            "{r:?}"
        );
        outs.push(stdout(&o));
    }
    assert_eq!(outs[0], outs[1]);
}

// ---------------------------------------------------------------------------------------------------------
// CL-3 to CL-5

/// CL-3: `--json` is `--format json` for `check` (byte for byte) and `sql`; with another `--format` it is a usage
/// error.
#[test]
fn json_is_the_alias_of_format_json() {
    let t = Tmp::new("ew-conventions");
    let p = t.golden("J-E-2416.ew");
    let json = t.ew(&args([&"check", &p, &"--json"]));
    let format = t.ew(&args([&"check", &p, &"--format", &"json"]));
    assert_eq!(json.status.code(), Some(1), "{}", stderr(&json));
    assert_eq!(json.stdout, format.stdout, "`--json` equals `--format json`");
    let both = t.ew(&args([&"check", &p, &"--json", &"--format", &"json"]));
    assert_eq!(both.stdout, format.stdout);
    let o = t.ew(&args([&"check", &p, &"--json", &"--format", &"md"]));
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("`--json` cannot be combined with `--format md`"),
        "{}",
        stderr(&o)
    );
    let q = "SELECT 1 AS one";
    let json = t.ew(&args([&"sql", &p, &q, &"--json"]));
    let format = t.ew(&args([&"sql", &p, &q, &"--format", &"json"]));
    assert_eq!((json.status.code(), &json.stdout), (Some(0), &format.stdout));
    let o = t.ew(&args([&"sql", &p, &q, &"--format", &"csv", &"--json"]));
    assert_eq!(o.status.code(), Some(2));
}

/// CL-5: `ew commands --json` lists the command registry's index in its order; `ew schema` prints a command's
/// parameter schema and a result schema byte for byte as the registries generate them; an unknown name exits 2
/// naming it.
#[test]
fn registry_subcommands_print_the_registries_files() {
    let t = Tmp::new("ew-conventions");
    let commands: BTreeMap<String, String> = ew_commands::CommandRegistry::builtin()
        .schema_files()
        .into_iter()
        .collect();
    let index: serde_json::Value = serde_json::from_str(&commands["index.json"]).unwrap();
    let o = t.ew(&args([&"commands", &"--json"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let listed: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(
        listed["commands"], index["commands"],
        "the entries of index.json in its order"
    );
    let text = t.ew(&args([&"commands"]));
    assert_eq!(
        stdout(&text).lines().count(),
        index["commands"].as_array().unwrap().len()
    );
    let o = t.ew(&args([&"schema", &"fields.set"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), commands["fields.set.schema.json"]);
    let o = t.ew(&args([&"schema", &"ew.check-result/1"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), schema_files()["ew.check-result-1.schema.json"]);
    let o = t.ew(&args([&"schema", &"no.such"]));
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("`no.such`"), "{}", stderr(&o));
    assert!(stdout(&o).is_empty());
}
