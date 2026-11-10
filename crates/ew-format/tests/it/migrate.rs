// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The migration chain: its steps on `Value`, the loader's
//! migrated reading ([`load_migrated`]) and the steward's check. The test chain goes from the test format `"0.0"` to
//! `"0.1"`; every shipped kind is at `$v` 1, so its kind steps go from `$v` 0.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use ew_format::ew_schema::{Kind, LibDocument, Sha256};
use ew_format::{
    FilePath, FormatStep, KindStep, Limits, MIGRATIONS, Manifest, MigrationError, Migrations, StewardProblem,
    lib_document_text, lib_file, load_migrated, sha256, steward_problems,
};
use serde_json::{Value, json};

use crate::{PROJECT, project};

/// [`PROJECT`] as the test format `"0.0"` wrote it: `$v` 0 and the member's old name.
const PROJECT_V0: &str =
    r#"{"$k":"project","$v":0,"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","profile":"2027.1"}"#;
/// A kind this reader does not know, with a member the format step renames.
const UNKNOWN_OLD: &str = r#"{"$k":"frobnicator","id":"3b9c1d72-5e48-7a06-8f13-2d6c0e9a4b51","old":1}"#;
const UNKNOWN_NEW: &str = r#"{"$k":"frobnicator","id":"3b9c1d72-5e48-7a06-8f13-2d6c0e9a4b51","new":1}"#;

type Files = BTreeMap<FilePath, Vec<u8>>;

fn p(s: &str) -> FilePath {
    FilePath::new(s).unwrap()
}

/// `value` with its top-level member `from` named `to`.
fn renamed(mut value: Value, from: &str, to: &str) -> Value {
    if let Some(members) = value.as_object_mut() {
        if let Some(v) = members.remove(from) {
            members.insert(to.to_owned(), v);
        }
    }
    value
}

fn project_v0(value: Value) -> Result<Value, MigrationError> {
    Ok(renamed(value, "profile", "derivation_profile"))
}

fn fndef_v0(value: Value) -> Result<Value, MigrationError> {
    Ok(renamed(value, "revision", "version"))
}

fn format_record(value: Value) -> Result<Value, MigrationError> {
    Ok(renamed(value, "old", "new"))
}

fn format_manifest(value: Value) -> Result<Value, MigrationError> {
    Ok(renamed(value, "x_old", "x_new"))
}

fn fails(_: Value) -> Result<Value, MigrationError> {
    Err(MigrationError::at("/profile", "the profile cannot be read"))
}

fn no_object(_: Value) -> Result<Value, MigrationError> {
    Ok(Value::Null)
}

/// `step` of a record traced in its member `trace`.
fn traced(mut value: Value, step: &str) -> Value {
    if let Some(trace) = value.get_mut("trace").and_then(Value::as_array_mut) {
        trace.push(step.into());
    }
    value
}

fn trace_format(value: Value) -> Result<Value, MigrationError> {
    Ok(traced(value, "format"))
}

fn trace_kind(value: Value) -> Result<Value, MigrationError> {
    Ok(traced(value, "kind"))
}

const FORMAT_STEP: FormatStep = FormatStep {
    from: "0.0",
    to: "0.1",
    manifest: format_manifest,
    record: format_record,
};

/// The test chain of MG-1.
static CHAIN: Migrations = Migrations {
    kinds: &[
        KindStep {
            kind: Kind::Project,
            from: 0,
            step: project_v0,
        },
        KindStep {
            kind: Kind::Fndef,
            from: 0,
            step: fndef_v0,
        },
    ],
    formats: &[FORMAT_STEP],
};

/// The test chain without its kind steps.
static FORMATS_ONLY: Migrations = Migrations {
    kinds: &[],
    formats: &[FORMAT_STEP],
};

/// A test chain whose project step fails.
static FAILING: Migrations = Migrations {
    kinds: &[
        KindStep {
            kind: Kind::Project,
            from: 0,
            step: fails,
        },
        KindStep {
            kind: Kind::Fndef,
            from: 0,
            step: fndef_v0,
        },
    ],
    formats: &[FORMAT_STEP],
};

/// A test chain whose project step gives no object.
static NO_OBJECT: Migrations = Migrations {
    kinds: &[KindStep {
        kind: Kind::Project,
        from: 0,
        step: no_object,
    }],
    formats: &[],
};

/// A test chain that traces the order of its steps.
static TRACING: Migrations = Migrations {
    kinds: &[KindStep {
        kind: Kind::Project,
        from: 0,
        step: trace_kind,
    }],
    formats: &[FormatStep {
        from: "0.0",
        to: "0.1",
        manifest: format_manifest,
        record: trace_format,
    }],
};

/// The fndef of the repository's library with a correct copy hash.
fn fndef() -> LibDocument {
    let path = format!(
        "{}/../../content/fndefs/ew.fn.coil.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut doc = LibDocument::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    let LibDocument::Fndef(copy) = &mut doc else {
        panic!("not a fndef");
    };
    copy.sha256 = Sha256(sha256(serde_json::to_string(&copy.item).unwrap().as_bytes()));
    doc
}

/// [`fndef`]'s text as the test format `"0.0"` wrote it: `$v` 0 and its `version` named `revision`.
fn fndef_v0_text() -> String {
    let mut value: Value = serde_json::from_str(&lib_document_text(&fndef()).unwrap()).unwrap();
    value["$v"] = json!(0);
    serde_json::to_string_pretty(&renamed(value, "version", "revision")).unwrap()
}

/// A `"0.0"` directory without its manifest: the project at `$v` 0, a frobnicator line the format step renames and
/// the fndef at `$v` 0.
fn files_v0() -> Files {
    let mut files = Files::new();
    files.insert(p("project.jsonl"), format!("{PROJECT_V0}\n").into_bytes());
    files.insert(p("structure.jsonl"), format!("{UNKNOWN_OLD}\n").into_bytes());
    files.insert(lib_file(&fndef()).unwrap(), fndef_v0_text().into_bytes());
    files
}

#[test]
fn the_shipped_chain_is_empty_and_reaches_no_older_format() {
    let e = MIGRATIONS.towards("0.0", "0.1").unwrap_err();
    assert_eq!(
        e,
        MigrationError {
            file: Some(p("manifest.json")),
            line: None,
            pointer: "/format".to_owned(),
            message: "this version has no migration step from format 0.0 towards 0.1".to_owned(),
        }
    );
    assert_eq!(
        e.to_string(),
        "manifest.json at /format: this version has no migration step from format 0.0 towards 0.1"
    );
    assert_eq!(MIGRATIONS.towards("0.1", "0.1").unwrap().from(), "0.1");
}

#[test]
fn a_record_takes_the_format_steps_first_and_then_its_kind_steps() {
    let way = TRACING.towards("0.0", "0.1").unwrap();
    let record = json!({"$k": "project", "$v": 0, "id": "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37", "trace": []});
    assert_eq!(
        way.record(record).unwrap(),
        json!({"$k": "project", "id": "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37", "trace": ["format", "kind"]})
    );
    // A record at its kind's `$v` takes no kind step.
    let current = json!({"$k": "project", "id": "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37", "trace": []});
    assert_eq!(
        way.record(current).unwrap(),
        json!({"$k": "project", "id": "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37", "trace": ["format"]})
    );
}

#[test]
fn a_missing_kind_step_and_a_step_that_gives_no_object_are_errors() {
    let record: Value = serde_json::from_str(PROJECT_V0).unwrap();
    let way = FORMATS_ONLY.towards("0.0", "0.1").unwrap();
    assert_eq!(
        way.record(record.clone()).unwrap_err(),
        MigrationError::at("/$v", "this version has no migration step for a project at $v 0")
    );
    let way = NO_OBJECT.towards("0.1", "0.1").unwrap();
    assert_eq!(
        way.record(record).unwrap_err(),
        MigrationError::at("", "the migration step of a project at $v 0 gave no object")
    );
}

#[test]
fn the_manifest_takes_the_manifest_steps() {
    let text = r#"{
  "format": "0.0",
  "min_reader": "0.0",
  "generator": "test",
  "project": "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37",
  "generation": 1,
  "files": {},
  "x_old": "kept"
}
"#;
    let limits = Limits::default();
    let manifest = Manifest::parse(text.as_bytes(), &limits).unwrap();
    let migrated = CHAIN
        .towards("0.0", "0.1")
        .unwrap()
        .manifest(&manifest, &limits)
        .unwrap();
    assert_eq!(migrated.extra.get("x_new"), Some(&json!("kept")));
    assert_eq!(migrated.extra.get("x_old"), None);
    assert_eq!(migrated.format, "0.0", "the save writes the format, not the step");
}

/// MG-1 at the loader: the migrated records and documents are read typed, their files are named apart from the
/// rewrite set and get no `NonCanonical` finding, and a line kept verbatim keeps its migrated text.
#[test]
fn load_migrated_reads_the_older_files_and_names_those_a_step_changed() {
    let way = CHAIN.towards("0.0", "0.1").unwrap();
    let loaded = load_migrated(None, &files_v0(), &Limits::default(), &way).unwrap();
    assert_eq!(loaded.project, Some(project()));
    assert_eq!(loaded.lib, [fndef()]);
    let lib = lib_file(&fndef()).unwrap();
    let migrated: BTreeSet<&str> = loaded.migrated.iter().map(FilePath::as_str).collect();
    assert_eq!(
        migrated,
        BTreeSet::from([lib.as_str(), "project.jsonl", "structure.jsonl"])
    );
    assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
    let codes: Vec<Option<&str>> = loaded.findings.iter().map(|f| f.code()).collect();
    assert_eq!(codes, [Some("EW-CORE-016")], "{:?}", loaded.findings);
    assert_eq!(loaded.kept.len(), 1);
    assert_eq!(loaded.kept[0].text, UNKNOWN_NEW.as_bytes());
}

/// An unchanged file is not migrated: a `"0.0"` file that no step changes stays out of the set.
#[test]
fn a_file_no_step_changes_is_not_migrated() {
    let way = CHAIN.towards("0.0", "0.1").unwrap();
    let mut files = Files::new();
    files.insert(p("project.jsonl"), format!("{PROJECT}\n").into_bytes());
    let loaded = load_migrated(None, &files, &Limits::default(), &way).unwrap();
    assert!(loaded.migrated.is_empty(), "{:?}", loaded.migrated);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert_eq!(loaded.project, Some(project()));
}

/// MG-2 at the loader: a failing step and a missing step refuse the load with the file, the line and the pointer.
#[test]
fn a_failing_or_missing_step_refuses_the_load_with_its_file_line_and_pointer() {
    let way = FAILING.towards("0.0", "0.1").unwrap();
    let e = load_migrated(None, &files_v0(), &Limits::default(), &way).unwrap_err();
    assert_eq!(
        e,
        MigrationError {
            file: Some(p("project.jsonl")),
            line: Some(1),
            pointer: "/profile".to_owned(),
            message: "the profile cannot be read".to_owned(),
        }
    );
    assert_eq!(
        e.to_string(),
        "project.jsonl line 1 at /profile: the profile cannot be read"
    );
    // The files are read in path order: the library document first.
    let way = FORMATS_ONLY.towards("0.0", "0.1").unwrap();
    let e = load_migrated(None, &files_v0(), &Limits::default(), &way).unwrap_err();
    assert_eq!(
        e.to_string(),
        format!(
            "{} at /$v: this version has no migration step for a fndef at $v 0",
            lib_file(&fndef()).unwrap()
        )
    );
}

/// The `"0.0"` fndef with its item's `contact_kind` named `kind`, hashed as the old format wrote it: over its old item.
fn fndef_v0_item_text() -> String {
    let mut value: Value = serde_json::from_str(&fndef_v0_text()).unwrap();
    value = renamed(value, "contact_kind", "kind");
    let mut item = value.as_object().unwrap().clone();
    for envelope in ["$v", "id", "library_id", "item_id", "version", "sha256"] {
        item.remove(envelope);
    }
    let old = Sha256(sha256(serde_json::to_string(&item).unwrap().as_bytes()));
    value["sha256"] = json!(old.to_string());
    serde_json::to_string_pretty(&value).unwrap()
}

fn fndef_item_v0(value: Value) -> Result<Value, MigrationError> {
    Ok(renamed(fndef_v0(value)?, "kind", "contact_kind"))
}

/// A test chain whose fndef step changes the item.
static ITEM_STEP: Migrations = Migrations {
    kinds: &[KindStep {
        kind: Kind::Fndef,
        from: 0,
        step: fndef_item_v0,
    }],
    formats: &[],
};

/// Decision log ID-1230 ruling 3: a step that changes a library copy's item is not an edit; the loader hashes the
/// migrated item, so no EW-CORE-014 follows.
#[test]
fn a_step_that_changes_a_copys_item_rehashes_it() {
    let lib = lib_file(&fndef()).unwrap();
    let files = Files::from([(lib.clone(), fndef_v0_item_text().into_bytes())]);
    let way = ITEM_STEP.towards("0.1", "0.1").unwrap();
    let loaded = load_migrated(None, &files, &Limits::default(), &way).unwrap();
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert_eq!(loaded.lib, [fndef()]);
    assert_eq!(loaded.migrated, BTreeSet::from([lib]));
}

fn fndef_same(value: Value) -> Result<Value, MigrationError> {
    Ok(value)
}

/// Decision log ID-1250: a step that changes a library copy's `$v` only leaves its item and its hash alone, so a
/// stale hash on the copy still raises EW-CORE-014.
#[test]
fn a_stale_hash_on_an_item_no_step_changed_still_raises_ew_core_014() {
    static ENVELOPE_STEP: Migrations = Migrations {
        kinds: &[KindStep {
            kind: Kind::Fndef,
            from: 0,
            step: fndef_same,
        }],
        formats: &[],
    };
    let mut value: Value = serde_json::from_str(&lib_document_text(&fndef()).unwrap()).unwrap();
    value["$v"] = json!(0);
    value["sha256"] = json!(Sha256([0; 32]).to_string());
    let lib = lib_file(&fndef()).unwrap();
    let files = Files::from([(
        lib.clone(),
        serde_json::to_string_pretty(&value).unwrap().into_bytes(),
    )]);
    let way = ENVELOPE_STEP.towards("0.1", "0.1").unwrap();
    let loaded = load_migrated(None, &files, &Limits::default(), &way).unwrap();
    assert_eq!(loaded.migrated, BTreeSet::from([lib]));
    let codes: Vec<Option<&str>> = loaded.findings.iter().map(|f| f.code()).collect();
    assert_eq!(codes, [Some("EW-CORE-014")], "{:?}", loaded.findings);
}

fn fndef_unknown(value: Value) -> Result<Value, MigrationError> {
    let mut value = fndef_v0(value)?;
    value["$k"] = json!("frobnicator");
    Ok(value)
}

fn fndef_untyped(value: Value) -> Result<Value, MigrationError> {
    let mut value = fndef_v0(value)?;
    value.as_object_mut().unwrap().remove("item_id");
    Ok(value)
}

/// Decision log ID-1230 ruling 2: a library document a step changed that would end opaque (of an unknown kind, with
/// no typed reading, or a duplicate) refuses the load, since its old bytes would stay under the new format.
#[test]
fn a_migrated_library_document_that_ends_opaque_refuses_the_load() {
    static UNKNOWN: Migrations = Migrations {
        kinds: &[KindStep {
            kind: Kind::Fndef,
            from: 0,
            step: fndef_unknown,
        }],
        formats: &[],
    };
    static UNTYPED: Migrations = Migrations {
        kinds: &[KindStep {
            kind: Kind::Fndef,
            from: 0,
            step: fndef_untyped,
        }],
        formats: &[],
    };
    let lib = lib_file(&fndef()).unwrap();
    let files = Files::from([(lib.clone(), fndef_v0_text().into_bytes())]);
    let refusal = |chain: &Migrations, files: &Files| {
        let way = chain.towards("0.1", "0.1").unwrap();
        load_migrated(None, files, &Limits::default(), &way)
            .unwrap_err()
            .to_string()
    };
    assert_eq!(
        refusal(&UNKNOWN, &files),
        format!("{lib}: a migration step changed a library document of a kind this version keeps unchanged")
    );
    assert_eq!(
        refusal(&UNTYPED, &files),
        format!(
            "{lib}: a migration step gave a library document this version cannot read: missing field `item_id` at \"\""
        )
    );
    // The same document under a second name: the second in path order is the duplicate.
    let second = p("lib/fndefs/ffffffff-ffff-7fff-bfff-ffffffffffff.json");
    let mut twice = files.clone();
    twice.insert(second.clone(), fndef_v0_text().into_bytes());
    assert_eq!(
        refusal(&CHAIN, &twice),
        format!(
            "{second}: once migrated, this file holds a function definition (version 0.1.0), which another library \
             file holds too; remove one of the two files and open the project again"
        )
    );
}

/// A test chain from 0.1 to 0.2.
static TOWARDS_0_2: Migrations = Migrations {
    kinds: &[],
    formats: &[FormatStep {
        from: "0.1",
        to: "0.2",
        manifest: format_manifest,
        record: format_record,
    }],
};

/// A corpus folder of its own under the system's temporary folder, removed when dropped.
struct Corpus(PathBuf);

impl Corpus {
    fn with(formats: &[&str]) -> Corpus {
        let root = std::env::temp_dir().join(format!("ew-format-steward-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in formats {
            std::fs::create_dir_all(root.join(f)).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        Corpus(root)
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// MG-6 as ruling 8 amends it: the steward's check on injected lists; with two released formats, a way from each.
#[test]
fn the_steward_check_bites_at_the_first_format_change() {
    let corpus = Corpus::with(&["0.0", "0.1"]);
    let root = corpus.0.as_path();
    // Nothing released: nothing to check.
    assert_eq!(steward_problems(&[], "0.1", &MIGRATIONS, root), []);
    // The released format is the one written, its folder present.
    assert_eq!(steward_problems(&["0.1"], "0.1", &MIGRATIONS, root), []);
    // The format changed without a step.
    let problems = steward_problems(&["0.1"], "0.2", &MIGRATIONS, root);
    assert_eq!(
        problems,
        [StewardProblem::NoStep {
            from: "0.1".into(),
            to: "0.2".into()
        }]
    );
    assert_eq!(
        problems[0].to_string(),
        "format 0.2 is not released and the migration chain has no step from the released format 0.1 towards it: \
         add the steps with the format change"
    );
    // With the step.
    assert_eq!(steward_problems(&["0.1"], "0.2", &TOWARDS_0_2, root), []);
    // A released format without its fixtures.
    let problems = steward_problems(&["0.1", "0.2"], "0.2", &TOWARDS_0_2, root);
    assert_eq!(problems, [StewardProblem::NoCorpus { format: "0.2".into() }]);
    assert_eq!(
        problems[0].to_string(),
        "the released format 0.2 has no fixture folder fixtures/format/0.2/: add its fixtures"
    );
    // Two released formats: the last one has a way, the first one none.
    assert_eq!(
        steward_problems(&["0.0", "0.1"], "0.2", &TOWARDS_0_2, root),
        [StewardProblem::NoStep {
            from: "0.0".into(),
            to: "0.2".into()
        }]
    );
    assert_eq!(steward_problems(&["0.0", "0.1"], "0.1", &TOWARDS_0_2, root), []);
}
