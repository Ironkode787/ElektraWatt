// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew query` and `ew queries` (WP-S1.22 part 4, D11 to D13) on a copy of Golden A: the text and the `--json` result,
//! the failure paths, the registry's snapshot, determinism and `--project`.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_api::cli_results as results;
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const QUERIES_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/ew-api/api/queries.json"
);

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory with its own user directories, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-query-{}-{n}", std::process::id()));
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

/// Every file under `dir` with its bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                walk(root, &e.path(), out);
            } else {
                let rel = e
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(e.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
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

/// `schema` compiled by the independent validator under the name `url`.
fn compile(url: &str, schema: Value) -> (boon::Schemas, boon::SchemaIndex) {
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.add_resource(url, schema).unwrap();
    let index = compiler
        .compile(url, &mut schemas)
        .unwrap_or_else(|e| panic!("{url}: {e}"));
    (schemas, index)
}

/// The committed result schema `name` (`bin/ew/schemas/`), compiled.
fn result_schema(name: &str) -> (boon::Schemas, boon::SchemaIndex) {
    let file = results::file_name(name);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas").join(&file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{file} is not committed under bin/ew/schemas/"));
    compile(&format!("urn:ew:{file}"), serde_json::from_str(&text).unwrap())
}

/// CL-2 for `ew.query-result/1`: `$schema` first, valid against the committed schema, typed with
/// `deny_unknown_fields`, byte-equal after the round trip.
fn assert_query_result(out: &str) -> results::QueryResult {
    assert!(
        out.starts_with(&format!("{{\n  \"$schema\": \"{}\"", results::QUERY)),
        "{out}"
    );
    let value: Value = serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"));
    let (schemas, index) = result_schema(results::QUERY);
    if let Err(e) = schemas.validate(&value, index) {
        panic!("the output does not validate against its schema: {e:#}\n{out}");
    }
    let typed: results::QueryResult = serde_json::from_str(out).unwrap();
    assert_eq!(results::to_text(&typed), out, "the round trip");
    typed
}

/// QU-1: `ew query device_tag tag:-101F1` prints the full designation and the displayed tag; with `--json` the
/// `ew.query-result/1` object, whose `output` holds both and whose `explain` is null without `--explain`.
#[test]
fn device_tag_prints_its_text_and_its_json_result() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([&"query", &p, &"device_tag", &"tag:-101F1"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), "full       =P01+J-E-2416-101F1\ndisplayed  -101F1\n");
    assert_eq!(stderr(&o), "");

    let o = t.ew(&args([&"query", &p, &"device_tag", &"tag:-101F1", &"--json"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let r = assert_query_result(&stdout(&o));
    assert_eq!(
        (r.query.as_str(), r.key.as_str(), r.rev),
        ("device_tag", "tag:-101F1", 0)
    );
    assert_eq!(
        r.output,
        json!({"full": "=P01+J-E-2416-101F1", "displayed": "-101F1"})
    );
    assert_eq!(r.explain, None);
    assert!(stdout(&o).contains("\"explain\": null"), "{}", stdout(&o));

    // QU-2's explanation in the result: the policy, the dependencies and the provenance (a fresh process: a miss).
    let o = t.ew(&args([
        &"query",
        &p,
        &"device_tag",
        &"tag:-101F1",
        &"--explain",
        &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let e = assert_query_result(&stdout(&o)).explain.expect("--explain");
    assert_eq!(e.family, "device_tag");
    assert_eq!(
        e.policy,
        "Memo · device_tag · verified at rev 0 (miss) · changed at rev 0"
    );
    assert_eq!(e.provenance.as_deref(), Some("numbering scheme 'page-based'"));
    assert!(
        e.depends_on
            .contains(&"main representation of -101F1 on folio 101 (rev 0)".to_owned()),
        "{:?}",
        e.depends_on
    );
}

/// QU-3 and QU-5 through the program: every entry of `ew queries --json` answers its example with exit 0, and the
/// `output` of its `--json` result validates against the entry's own output schema.
#[test]
fn every_example_validates_against_its_output_schema() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&args([&"queries", &"--json"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let list: Value = serde_json::from_str(&stdout(&o)).unwrap();
    for q in list["queries"].as_array().unwrap() {
        let name = q["name"].as_str().unwrap();
        let example = q["example"].as_str().unwrap();
        let o = t.ew(&args([&"query", &p, &name, &example, &"--json"]));
        assert_eq!(o.status.code(), Some(0), "{name}: {}", stderr(&o));
        let r = assert_query_result(&stdout(&o));
        assert_eq!((r.query.as_str(), r.key.as_str()), (name, example));
        let (schemas, index) = compile(&format!("urn:ew:query:{name}"), q["output"].clone());
        if let Err(e) = schemas.validate(&r.output, index) {
            panic!(
                "{name}: the output does not validate against its schema: {e:#}\n{}",
                r.output
            );
        }
    }
}

/// The failure paths exit 2 with `error: ` and the message on stderr, nothing on stdout. An unknown query is refused
/// before the project is opened; a key that names no record prints the replay's message (QU-3); a record of another
/// kind is named in words ("Every adapter checks the resolved record's kind", ruling 3 of cycle 41).
#[test]
fn failures_exit_2_with_their_message() {
    let t = Tmp::new();
    let p = t.golden();
    let missing = t.0.join("no-such-project");
    for (cmd, message) in [
        (
            args([&"query", &missing, &"no_such", &"-"]),
            "error: unknown query `no_such`; `ew queries` lists the queries\n",
        ),
        (
            args([&"query", &p, &"device_tag", &"tag:-999Z9"]),
            "error: `tag:-999Z9`: no device prints this tag\n",
        ),
        (
            args([&"query", &p, &"device_tag", &"folio:101"]),
            "error: `folio:101` names page 101 Power Supply 1 - Main Current; `device_tag` takes a device \
             (`tag:-101F1` or `id:<uuid>`)\n",
        ),
        (
            args([&"query", &p, &"channel_addresses", &"tag:-101F1"]),
            "error: `tag:-101F1` names device -101F1, which is not a PLC module; `channel_addresses` takes a PLC \
             module (`tag:-201U5`)\n",
        ),
        (
            args([&"query", &p, &"page_order", &"x"]),
            "error: `page_order` takes no key: give `-`\n",
        ),
        (
            args([&"query", &p, &"wire_numbers", &"folio:901"]),
            "error: `folio:901`: no drawn page has this folio label\n",
        ),
        (
            args([&"query", &p, &"device_tag", &"id:not-a-uuid", &"--explain"]),
            "error: `id:not-a-uuid`: not a UUID\n",
        ),
    ] {
        let o = t.ew(&cmd);
        assert_eq!(o.status.code(), Some(2), "{cmd:?}: {}", stderr(&o));
        assert_eq!(stderr(&o), message, "{cmd:?}");
        assert_eq!(stdout(&o), "", "{cmd:?}");
    }
    // Usage errors print the message and the usage.
    for (cmd, first) in [
        (
            args([&"query", &"device_tag"]),
            "error: `query` needs a query name and a key (`ew query device_tag tag:-101F1`; `ew queries` lists the \
             queries)",
        ),
        (
            args([&"query", &p, &"device_tag", &"tag:-101F1", &"--bogus"]),
            "error: unknown option `--bogus`",
        ),
        (
            args([&"query", &p, &"device_tag", &"tag:-101F1", &"extra"]),
            "error: unexpected argument `extra`",
        ),
        (args([&"queries", &"--bogus"]), "error: unknown option `--bogus`"),
    ] {
        let o = t.ew(&cmd);
        assert_eq!(o.status.code(), Some(2), "{cmd:?}");
        let err = stderr(&o);
        assert_eq!(err.lines().next(), Some(first), "{cmd:?}: {err}");
        assert_eq!(err.lines().nth(1), Some("usage:"), "{cmd:?}: {err}");
        assert_eq!(stdout(&o), "", "{cmd:?}");
    }
}

/// QU-5 (D13): `ew queries --json` prints `crates/ew-api/api/queries.json` byte for byte; the text form is one line
/// per entry, name, key and description separated by tabs.
#[test]
fn queries_prints_the_committed_registry() {
    let t = Tmp::new();
    let o = t.ew(&args([&"queries", &"--json"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), std::fs::read_to_string(QUERIES_JSON).unwrap());
    let value: Value = serde_json::from_str(&stdout(&o)).unwrap();
    let (schemas, index) = result_schema(results::QUERIES);
    schemas.validate(&value, index).unwrap();
    let typed: results::QueriesResult = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(results::to_text(&typed), stdout(&o));

    let o = t.ew(&args([&"queries"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let lines: Vec<String> = typed
        .queries
        .iter()
        .map(|q| format!("{}\t{}\t{}", q.name, q.key, q.description))
        .collect();
    assert_eq!(stdout(&o), format!("{}\n", lines.join("\n")));
    assert!(stdout(&o).starts_with("bom_rows\t-\t"), "{}", stdout(&o));
}

/// QU-4: two runs print the same text, with and without `--explain`; a query opens the project as a reader and
/// writes nothing into it.
#[test]
fn two_runs_print_the_same_text_and_write_nothing() {
    let t = Tmp::new();
    let p = t.golden();
    let before = tree(&p);
    for cmd in [
        args([&"query", &p, &"device_tag", &"tag:-101F1", &"--explain"]),
        args([&"query", &p, &"page_order", &"-"]),
        args([
            &"query",
            &p,
            &"xref_text_of",
            &"tag:-201U1",
            &"--explain",
            &"--json",
        ]),
    ] {
        let first = t.ew(&cmd);
        let second = t.ew(&cmd);
        assert_eq!(first.status.code(), Some(0), "{cmd:?}: {}", stderr(&first));
        assert_eq!(stdout(&first), stdout(&second), "{cmd:?}");
    }
    assert!(tree(&p) == before, "the project is unchanged");
}

/// CL-1 for `ew query`: the positional directory and `--project <dir>` print the same; both together exit 2 with
/// D3's message.
#[test]
fn project_flag_equals_the_positional_directory() {
    let t = Tmp::new();
    let p = t.golden();
    let positional = t.ew(&args([&"query", &p, &"cell_of", &"tag:-101F1", &"--explain"]));
    let flag = t.ew(&args([
        &"query",
        &"cell_of",
        &"tag:-101F1",
        &"--project",
        &p,
        &"--explain",
    ]));
    assert_eq!(positional.status.code(), Some(0), "{}", stderr(&positional));
    assert_eq!(flag.status.code(), Some(0), "{}", stderr(&flag));
    assert_eq!(stdout(&positional), stdout(&flag));
    assert_eq!(
        stdout(&positional),
        "cell_of tag:-101F1 @ rev 0\n  folio      101\n  cell       B8\n  policy     Cold · rep_anchor_cell is \
         computed on demand and has no memo\n  provenance main representation\n"
    );
    let both = t.ew(&args([&"query", &p, &"cell_of", &"tag:-101F1", &"--project", &p]));
    assert_eq!(both.status.code(), Some(2));
    assert_eq!(
        stderr(&both).lines().next(),
        Some("error: `--project` cannot be combined with a positional project directory")
    );
}
