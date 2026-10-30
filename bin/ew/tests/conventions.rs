// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.22 part 1 through the `ew` binary: the conventions of doc 17 §3.1 (`--project`, `--json`) and the published
//! result schemas (CL-1 to CL-5, CL-7; CL-6 is in `tests/cli.rs`).
//!
//! The result schemas under `bin/ew/schemas/` are an equality snapshot of `ew_api::cli_results::schema_files()`;
//! `EW_CLI_BLESS=1 cargo nextest run -p ew --test conventions` rewrites the directory. Every `--json` output is
//! checked three ways: it validates against its committed schema file (an independent JSON Schema validator,
//! `boon`), it deserialises into its type (`deny_unknown_fields`) and re-serialises byte for byte, and a second run
//! prints the same bytes.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_api::cli_results as results;
use serde::Serialize;
use serde::de::DeserializeOwned;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const VARIANTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/variants");
const GOLDEN_PARTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../golden/J-E-2416/tables/parts.csv"
);
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
const QET: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors/></diagram></project>"#;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory with its own user, data and cache directories, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-conventions-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    /// `ew` with the user directories, `EW_DATA_DIR` and `EW_CACHE_DIR` inside this directory, no starter content
    /// unless `content`, no display and no `SOURCE_DATE_EPOCH`.
    fn ew_with(&self, args: &[OsString], content: bool) -> Output {
        let mut cmd = Command::new(bin_exe());
        cmd.args(args)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("SOURCE_DATE_EPOCH")
            .env("EW_DATA_DIR", self.0.join("data"))
            .env("EW_CACHE_DIR", self.0.join("cache"));
        if content {
            cmd.env("EW_CONTENT_DIR", CONTENT);
        } else {
            cmd.env_remove("EW_CONTENT_DIR");
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
        self.ew_with(args, false)
    }

    /// A copy of Golden A named `name`.
    fn golden(&self, name: &str) -> PathBuf {
        let copy = self.0.join(name);
        copy_dir(Path::new(GOLDEN_A), &copy);
        copy
    }

    /// A small saved project: one page, no report blocks.
    fn tiny(&self, name: &str) -> PathBuf {
        let project = serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
        )
        .unwrap();
        let mut session =
            ew_api::Session::from_records(project, [], [], ew_api::SessionOptions::default()).unwrap();
        session
            .execute(&ew_api::AnyCommand {
                name: "page.create".into(),
                params: serde_json::json!({"number": 1, "title": "Cover"}),
            })
            .unwrap();
        let dir = self.0.join(name);
        let mut store = ew_api::StoreOptions::new("test");
        store.state_dir = None;
        session.create(&dir, store).unwrap();
        session.close();
        dir
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

/// Every file of a directory tree with its bytes, by relative path.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn go(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                go(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    go(dir, dir, &mut out);
    out
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

fn schema_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas")
}

fn committed() -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(schema_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            files.insert(name, std::fs::read_to_string(entry.path()).unwrap());
        }
    }
    files
}

/// Whether `EW_CLI_BLESS` asks for the committed schemas to be rewritten.
// The flag only chooses whether the committed files are rewritten; the compared results never depend on it.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_CLI_BLESS").is_some()
}

// ---------------------------------------------------------------------------------------------------------
// The published schemas (D5)

/// D5: the committed result schemas equal the generated ones (an equality snapshot, blessed with `EW_CLI_BLESS=1`).
#[test]
fn committed_result_schemas_match_the_generated_ones() {
    let generated: BTreeMap<String, String> = results::schema_files().into_iter().collect();
    if bless() {
        let dir = schema_dir();
        std::fs::create_dir_all(&dir).unwrap();
        for name in committed().keys() {
            if !generated.contains_key(name) {
                std::fs::remove_file(dir.join(name)).unwrap();
            }
        }
        for (name, text) in &generated {
            std::fs::write(dir.join(name), text).unwrap();
        }
        return;
    }
    let committed = committed();
    let stale: Vec<&String> = committed.keys().filter(|n| !generated.contains_key(*n)).collect();
    let differing: Vec<&String> = generated
        .iter()
        .filter(|(name, text)| committed.get(*name) != Some(text))
        .map(|(name, _)| name)
        .collect();
    assert!(
        stale.is_empty() && differing.is_empty(),
        "bin/ew/schemas/ is out of date (differing or missing: {differing:?}; stale: {stale:?}); run \
         `EW_CLI_BLESS=1 cargo nextest run -p ew --test conventions` and commit the result"
    );
}

/// The committed schema files and their index: every result of the index has its file, every file is a draft
/// 2020-12 document the independent validator compiles, LF with a final newline.
#[test]
fn every_committed_schema_compiles() {
    let files = committed();
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

/// The schema `name` from its committed file, compiled by the independent validator.
fn compile(name: &str) -> (boon::Schemas, boon::SchemaIndex) {
    let file = results::file_name(name);
    let text = committed()
        .remove(&file)
        .unwrap_or_else(|| panic!("{file} is not committed under bin/ew/schemas/"));
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
fn assert_result<T: Serialize + DeserializeOwned>(out: &str, name: &str) -> T {
    assert!(
        out.starts_with(&format!("{{\n  \"$schema\": \"{name}\"")),
        "the first member names {name}: {out}"
    );
    let index: serde_json::Value = serde_json::from_str(&committed()["index.json"]).unwrap();
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
    let t = Tmp::new();
    let p = t.golden("J-E-2416.ew");
    let tiny = t.tiny("tiny.ew");
    let sqlite = t.0.join("p.sqlite");
    let pdf = t.0.join("p.pdf");
    let svg = t.0.join("svg");
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
        (vec!["verify-derived"], args([&"--json"]), &tiny),
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
        // Both together: a usage error with D3's message.
        let mut both = positional.clone();
        both.push("--project".into());
        both.push(project.as_os_str().to_owned());
        let o = t.ew(&both);
        assert_eq!(o.status.code(), Some(2), "{both:?}");
        assert!(
            stderr(&o).contains("`--project` cannot be combined with a positional project directory"),
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
    let t = Tmp::new();
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
    let t = Tmp::new();
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
    let t = Tmp::new();
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
}

// ---------------------------------------------------------------------------------------------------------
// CL-3 to CL-5

/// CL-3: `--json` is `--format json` for `check` (byte for byte) and `sql`; with another `--format` it is a usage
/// error.
#[test]
fn json_is_the_alias_of_format_json() {
    let t = Tmp::new();
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

/// CL-4: the release-note line of the `lib` JSON change (D3 (d)) exists; the shape itself is asserted by
/// `tests/cli.rs` and by the reading results above.
#[test]
fn the_lib_json_change_has_its_release_note_line() {
    let notes = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/release-notes/unreleased.md"),
    )
    .unwrap();
    for item in ["lib list --json", "lib search --json"] {
        assert!(
            notes
                .lines()
                .any(|l| l.contains(&format!("api-break: cli {item}"))),
            "docs/release-notes/unreleased.md names `{item}`"
        );
    }
}

/// CL-5: `ew commands --json` lists the command registry's index in its order; `ew schema` prints a command's
/// parameter schema and a result schema byte for byte as committed; an unknown name exits 2 naming it.
#[test]
fn registry_subcommands_print_the_committed_files() {
    let t = Tmp::new();
    let commands_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ew-commands/schemas");
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(commands_dir.join("index.json")).unwrap()).unwrap();
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
    assert_eq!(
        stdout(&o),
        std::fs::read_to_string(commands_dir.join("fields.set.schema.json")).unwrap()
    );
    let o = t.ew(&args([&"schema", &"ew.check-result/1"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), committed()["ew.check-result-1.schema.json"]);
    let o = t.ew(&args([&"schema", &"no.such"]));
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("`no.such`"), "{}", stderr(&o));
    assert!(stdout(&o).is_empty());
}
