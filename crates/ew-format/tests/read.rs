// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `plan` and `load` (doc 05 §3, §6.2, §6.3): the read plan and one test per finding of the loader.

use std::collections::BTreeMap;

use ew_format::ew_schema::{LibDocument, Project, Record, Sha256};
use ew_format::{
    EntryKind, FilePath, Finding, FindingKind, Kept, Limits, Loaded, Manifest, hash_text, lib_document_text,
    lib_file, load, plan, project_files, record_line, sha256,
};

const PAGE: &str = r#"{"$k":"page","id":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","folio":{"section":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","number":101},"kind":"multi_line","structure":{"function":"7d2c95e0-1f4a-5b38-a6e7-0c93b5d1f284","location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"},"title":{"en-GB":"Power Supply 1 - Main Current"},"grid":{"num":7937500,"den":3}}"#;
const DEVICE: &str = r#"{"$k":"device","id":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","tag":{"letter":"G","counter":1},"kind":"psu","parts":[{"part":{"copy":"e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025"},"qty":"1"}],"function_text":{"en-GB":"Power supply 1 AC / 24 V DC"},"tech_data":[{"key":"i_out","value":{"value":"20","unit":"A"}}]}"#;
const FUNCTION: &str = r#"{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","device":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":"black_box"}"#;
const PROJECT: &str =
    r#"{"$k":"project","id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

const PAGE_FILE: &str = "pages/8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a.jsonl";
const SHARD_FILE: &str = "devices/c2.jsonl";
const FUNCTION_ID: &str = "1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19";

type Files = BTreeMap<FilePath, Vec<u8>>;

fn p(s: &str) -> FilePath {
    FilePath::new(s).unwrap()
}

fn record(line: &str) -> Record {
    Record::from_json(line).unwrap()
}

fn project() -> Project {
    match record(PROJECT) {
        Record::Project(project) => project,
        other => panic!("not a project: {other:?}"),
    }
}

/// The canonical directory of the three records and the project, with its manifest.
fn dir() -> Files {
    project_files(
        &project(),
        &[record(PAGE), record(DEVICE), record(FUNCTION)],
        &[],
        1,
        "test",
    )
    .unwrap()
}

fn manifest_of(files: &Files) -> Manifest {
    Manifest::parse(&files[&p("manifest.json")], &Limits::default()).unwrap()
}

/// The layout files of a directory without its manifest, as `load` receives them.
fn layout(mut files: Files) -> Files {
    files.remove(&p("manifest.json"));
    files
}

fn load_plain(files: &Files) -> Loaded {
    load(None, files, &Limits::default())
}

fn put(files: &mut Files, path: &str, text: &str) {
    files.insert(p(path), text.as_bytes().to_vec());
}

fn text_of(files: &Files, path: &str) -> String {
    String::from_utf8(files[&p(path)].clone()).unwrap()
}

fn of_kind(loaded: &Loaded, kind: FindingKind) -> Vec<&Finding> {
    loaded.findings.iter().filter(|f| f.kind == kind).collect()
}

fn kinds(loaded: &Loaded) -> Vec<FindingKind> {
    loaded.findings.iter().map(|f| f.kind).collect()
}

fn present(entries: &[(&str, EntryKind)]) -> BTreeMap<String, EntryKind> {
    entries.iter().map(|(n, k)| ((*n).to_owned(), *k)).collect()
}

fn names(paths: &[FilePath]) -> Vec<&str> {
    paths.iter().map(FilePath::as_str).collect()
}

/// A record file `path` holding `lines` (LF-terminated), loaded without a manifest.
fn load_lines(path: &str, lines: &[&str]) -> Loaded {
    let mut files = Files::new();
    put(&mut files, path, &format!("{}\n", lines.join("\n")));
    load_plain(&files)
}

#[test]
fn fixtures_are_canonical() {
    for line in [PAGE, DEVICE, FUNCTION, PROJECT] {
        assert_eq!(record_line(&record(line)).unwrap(), line);
    }
}

// ---- plan ---------------------------------------------------------------------------------------------------

#[test]
fn plan_reads_listed_files_that_are_present() {
    let files = dir();
    let manifest = manifest_of(&files);
    let entries: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect();
    let plan = plan(Some(&manifest), &entries, &Limits::default());
    assert_eq!(names(&plan.read), [SHARD_FILE, PAGE_FILE, "project.jsonl"]);
    assert!(plan.findings.is_empty(), "{:?}", plan.findings);
}

#[test]
fn plan_listed_but_missing_is_ew_core_003() {
    let files = dir();
    let manifest = manifest_of(&files);
    let mut entries: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect();
    entries.remove(SHARD_FILE);
    let plan = plan(Some(&manifest), &entries, &Limits::default());
    assert_eq!(names(&plan.read), [PAGE_FILE, "project.jsonl"]);
    assert_eq!(plan.findings.len(), 1);
    assert_eq!(plan.findings[0].kind, FindingKind::MissingFile);
    assert_eq!(plan.findings[0].code(), Some("EW-CORE-003"));
    assert_eq!(plan.findings[0].file, Some(p(SHARD_FILE)));
}

#[test]
fn plan_listed_link_is_ew_core_003_and_not_read() {
    let files = dir();
    let manifest = manifest_of(&files);
    let mut entries: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect();
    entries.insert(SHARD_FILE.to_owned(), EntryKind::Link);
    entries.insert(PAGE_FILE.to_owned(), EntryKind::Other);
    entries.insert("project.jsonl".to_owned(), EntryKind::Dir);
    let plan = plan(Some(&manifest), &entries, &Limits::default());
    assert!(plan.read.is_empty());
    assert_eq!(plan.findings.len(), 3);
    for finding in &plan.findings {
        assert_eq!(finding.kind, FindingKind::MissingFile);
        assert!(
            finding.message.contains("not a regular file"),
            "{}",
            finding.message
        );
        assert!(
            finding.message.contains("links are not followed"),
            "{}",
            finding.message
        );
    }
}

#[test]
fn plan_unlisted_files_are_orphans_and_not_read() {
    let files = dir();
    let manifest = manifest_of(&files);
    let mut entries: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect();
    let unlisted = "pages/00000000-0000-7000-8000-000000000001.jsonl";
    let copy = "devices/c2 (conflicted copy).jsonl";
    entries.insert(unlisted.to_owned(), EntryKind::File);
    entries.insert(copy.to_owned(), EntryKind::File);
    // Neither a directory nor a user's own file is an orphan.
    entries.insert("pages".to_owned(), EntryKind::Dir);
    entries.insert("README.txt".to_owned(), EntryKind::File);
    let plan = plan(Some(&manifest), &entries, &Limits::default());
    assert_eq!(names(&plan.read), [SHARD_FILE, PAGE_FILE, "project.jsonl"]);
    let orphans: Vec<_> = plan.findings.iter().map(|f| (f.kind, f.file.clone())).collect();
    assert_eq!(plan.findings.len(), 2, "{:?}", plan.findings);
    assert!(orphans.contains(&(FindingKind::Orphan, Some(p(unlisted)))));
    assert!(orphans.contains(&(FindingKind::Orphan, Some(p(copy)))));
    assert_eq!(plan.findings[0].code(), Some("EW-CORE-004"));
}

#[test]
fn plan_without_a_manifest_reads_the_layout() {
    let entries = present(&[
        ("project.jsonl", EntryKind::File),
        (PAGE_FILE, EntryKind::File),
        (SHARD_FILE, EntryKind::File),
        ("README.txt", EntryKind::File),
        ("manifest.json", EntryKind::File),
        (".~ew.lock", EntryKind::File),
        (".ew/u-x/view.json", EntryKind::File),
        ("devices/c2.jsonl.tmp-3", EntryKind::File),
        ("manifest.json.tmp-4", EntryKind::File),
        ("pages", EntryKind::Dir),
        (
            "lib/fndefs/5d1ab4de-706a-5b85-b463-8b15311c4f39.json",
            EntryKind::File,
        ),
        (
            "assets/956304c8a5d96ff4f4f32b9a3519669bd88e0e3caac2c0341a7d66a3b24e5a3d.svg",
            EntryKind::File,
        ),
        ("devices/zz.jsonl", EntryKind::File),
        ("devices/00.jsonl", EntryKind::Link),
    ]);
    let plan = plan(None, &entries, &Limits::default());
    assert_eq!(
        names(&plan.read),
        [
            "assets/956304c8a5d96ff4f4f32b9a3519669bd88e0e3caac2c0341a7d66a3b24e5a3d.svg",
            SHARD_FILE,
            "lib/fndefs/5d1ab4de-706a-5b85-b463-8b15311c4f39.json",
            PAGE_FILE,
            "project.jsonl",
        ]
    );
    let by_kind: Vec<_> = plan.findings.iter().map(|f| (f.kind, f.file.clone())).collect();
    assert_eq!(by_kind.len(), 2, "{by_kind:?}");
    assert!(by_kind.contains(&(FindingKind::Orphan, Some(p("devices/zz.jsonl")))));
    assert!(by_kind.contains(&(FindingKind::MissingFile, Some(p("devices/00.jsonl")))));
}

#[test]
fn plan_too_many_entries_is_limit_exceeded() {
    let entries = present(&[
        ("project.jsonl", EntryKind::File),
        (PAGE_FILE, EntryKind::File),
        (SHARD_FILE, EntryKind::File),
    ]);
    let limits = Limits {
        max_files: 2,
        ..Limits::default()
    };
    let plan = plan(None, &entries, &limits);
    assert!(plan.read.is_empty());
    assert_eq!(plan.findings.len(), 1);
    assert_eq!(plan.findings[0].kind, FindingKind::LimitExceeded);
    assert_eq!(plan.findings[0].file, None);
    let files = dir();
    let manifest = manifest_of(&files);
    let plan = ew_format::plan(Some(&manifest), &entries, &limits);
    assert!(plan.read.is_empty());
    assert_eq!(plan.findings[0].kind, FindingKind::LimitExceeded);
}

// ---- load: the canonical directory ----------------------------------------------------------------------------

#[test]
fn canonical_directory_loads_clean() {
    let files = dir();
    let manifest = manifest_of(&files);
    let entries: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect();
    let reads = plan(Some(&manifest), &entries, &Limits::default());
    let input: Files = reads
        .read
        .iter()
        .map(|path| (path.clone(), files[path].clone()))
        .collect();
    let loaded = load(Some(&manifest), &input, &Limits::default());
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty());
    assert!(!loaded.read_only);
    assert_eq!(loaded.project, Some(project()));
    // File order: devices/c2.jsonl (device, function), then the page.
    assert_eq!(loaded.records, [record(DEVICE), record(FUNCTION), record(PAGE)]);
    assert!(loaded.kept.is_empty() && loaded.kept_docs.is_empty() && loaded.opaque.is_empty());
    assert_eq!(loaded.hashes.len(), 3);
    for (path, bytes) in &input {
        assert_eq!(loaded.hashes[path], sha256(bytes));
    }
}

// ---- load: line findings ------------------------------------------------------------------------------------

fn quarantined(loaded: &Loaded, line: u32) -> &ew_format::KeptLine {
    loaded
        .kept
        .iter()
        .find(|k| k.line == line)
        .unwrap_or_else(|| panic!("no kept line {line}: {:?}", loaded.kept))
}

#[test]
fn json_syntax_error_is_ew_core_005_and_read_only() {
    let bad = r#"{"$k":"page","id":"#;
    let loaded = load_lines(SHARD_FILE, &[DEVICE, FUNCTION, bad]);
    let found = of_kind(&loaded, FindingKind::JsonSyntax);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].file, Some(p(SHARD_FILE)));
    assert_eq!(found[0].line, Some(3));
    assert_eq!(found[0].code(), Some("EW-CORE-005"));
    assert!(loaded.read_only);
    let kept = quarantined(&loaded, 3);
    assert_eq!(kept.kept, Kept::Quarantined);
    assert_eq!(kept.text, bad.as_bytes());
    assert_eq!(kept.key, ew_format::LineKey::keyless().with_seq(3));
    assert_eq!(loaded.records, [record(DEVICE), record(FUNCTION)]);
    // Nothing is lost: the quarantined line stays in its file (keyless lines sort last).
    assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
}

#[test]
fn conflict_markers_are_ew_core_006_and_read_only() {
    for marker in ["<<<<<<< HEAD", "=======", ">>>>>>> branch", "||||||| base"] {
        let loaded = load_lines(SHARD_FILE, &[DEVICE, marker, FUNCTION]);
        let found = of_kind(&loaded, FindingKind::ConflictMarkers);
        assert_eq!(found.len(), 1, "{marker}");
        assert_eq!(found[0].line, Some(2));
        assert_eq!(found[0].code(), Some("EW-CORE-006"));
        assert!(loaded.read_only, "{marker}");
        assert_eq!(quarantined(&loaded, 2).text, marker.as_bytes());
        assert_eq!(quarantined(&loaded, 2).kept, Kept::Quarantined);
        assert_eq!(loaded.records.len(), 2);
    }
}

#[test]
fn duplicate_member_is_ew_core_007() {
    let line = r#"{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","kind":"black_box","kind":"black_box"}"#;
    let loaded = load_lines(SHARD_FILE, &[DEVICE, line]);
    let found = of_kind(&loaded, FindingKind::SchemaViolation);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, Some(2));
    assert_eq!(found[0].code(), Some("EW-CORE-007"));
    assert!(
        found[0].message.contains("duplicate member"),
        "{}",
        found[0].message
    );
    assert!(!loaded.read_only);
    assert_eq!(quarantined(&loaded, 2).text, line.as_bytes());
    assert_eq!(loaded.records, [record(DEVICE)]);
}

#[test]
fn float_is_ew_core_007() {
    let line = PAGE.replace("\"number\":101", "\"number\":1.5");
    let loaded = load_lines(PAGE_FILE, &[&line]);
    let found = of_kind(&loaded, FindingKind::SchemaViolation);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].line, Some(1));
    assert_eq!(found[0].file, Some(p(PAGE_FILE)));
    assert_eq!(quarantined(&loaded, 1).text, line.as_bytes());
    assert_eq!(
        quarantined(&loaded, 1).key,
        ew_format::LineKey::of(&record(PAGE)).unwrap().with_seq(1)
    );
    assert!(loaded.records.is_empty());
}

#[test]
fn same_id_twice_in_one_file_is_ew_core_009() {
    let second = DEVICE.replace("\"letter\":\"G\"", "\"letter\":\"K\"");
    let loaded = load_lines(SHARD_FILE, &[DEVICE, &second]);
    let found = of_kind(&loaded, FindingKind::DuplicateRecord);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, Some(2));
    assert_eq!(found[0].code(), Some("EW-CORE-009"));
    assert!(
        found[0].message.contains("devices/c2.jsonl:1"),
        "{}",
        found[0].message
    );
    assert_eq!(loaded.records, [record(DEVICE)]);
    let kept = quarantined(&loaded, 2);
    assert_eq!(kept.kept, Kept::Quarantined);
    assert_eq!(kept.text, second.as_bytes());
    assert_eq!(
        kept.key,
        ew_format::LineKey::of(&record(DEVICE)).unwrap().with_seq(2)
    );
    assert!(!loaded.read_only);
}

#[test]
fn same_id_in_two_files_is_ew_core_009() {
    let mut files = Files::new();
    put(&mut files, SHARD_FILE, &format!("{DEVICE}\n"));
    put(&mut files, "devices/ff.jsonl", &format!("{DEVICE}\n"));
    let loaded = load_plain(&files);
    let found = of_kind(&loaded, FindingKind::DuplicateRecord);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].file, Some(p("devices/ff.jsonl")));
    assert_eq!(found[0].line, Some(1));
    assert!(found[0].message.contains(SHARD_FILE), "{}", found[0].message);
    assert_eq!(loaded.records, [record(DEVICE)]);
    assert_eq!(quarantined(&loaded, 1).file, p("devices/ff.jsonl"));
}

#[test]
fn a_second_project_record_is_a_duplicate() {
    let other = PROJECT.replace(
        "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37",
        "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b38",
    );
    let loaded = load_lines("project.jsonl", &[PROJECT, &other]);
    assert_eq!(of_kind(&loaded, FindingKind::DuplicateRecord).len(), 1);
    assert_eq!(loaded.project, Some(project()));
    assert_eq!(quarantined(&loaded, 2).text, other.as_bytes());
}

const UNKNOWN_A: &str =
    r#"{"$k":"frobnicator","id":"3b9c1d72-5e48-7a06-8f13-2d6c0e9a4b51","x":{"b":1,"a":2}}"#;
const UNKNOWN_B: &str =
    r#"{"$k":"frobnicator","id":"4c0d2e83-6f59-7b17-9a24-3e7d1fab5c62","x":{"b":3,"a":4}}"#;

#[test]
fn unknown_kind_is_kept_verbatim_with_one_ew_core_016_per_kind_and_file() {
    // An unknown kind is not canonical-checked against its (re-sorted) member order: the original line is
    // what the writer keeps, so the file stays canonical.
    let loaded = load_lines("structure.jsonl", &[UNKNOWN_A, UNKNOWN_B]);
    let found = of_kind(&loaded, FindingKind::NewerData);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].code(), Some("EW-CORE-016"));
    assert_eq!(found[0].file, Some(p("structure.jsonl")));
    assert_eq!(loaded.records.len(), 2);
    assert!(matches!(loaded.records[0], Record::Unknown(_)));
    assert_eq!(loaded.kept.len(), 2);
    for (kept, text) in loaded.kept.iter().zip([UNKNOWN_A, UNKNOWN_B]) {
        assert_eq!(kept.kept, Kept::UnknownKind);
        assert_eq!(kept.text, text.as_bytes());
        assert!(kept.key.id().is_some());
    }
    assert_eq!(loaded.kept[0].line, 1);
    assert_eq!(loaded.kept[1].line, 2);
    assert!(!loaded.read_only);
    assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
    // A second file with the same kind reports it again.
    let mut files = Files::new();
    put(&mut files, "structure.jsonl", &format!("{UNKNOWN_A}\n"));
    put(&mut files, "report-blocks.jsonl", &format!("{UNKNOWN_B}\n"));
    let loaded = load_plain(&files);
    assert_eq!(of_kind(&loaded, FindingKind::NewerData).len(), 2);
}

#[test]
fn reserved_kind_is_kept_without_a_finding() {
    let line = r#"{"$k":"waiver","id":"3b9c1d72-5e48-7a06-8f13-2d6c0e9a4b51","note":"x"}"#;
    let loaded = load_lines("waivers.jsonl", &[line]);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.kept.len(), 1);
    assert_eq!(loaded.kept[0].kept, Kept::UnknownKind);
    assert_eq!(loaded.kept[0].text, line.as_bytes());
    assert!(loaded.rewrite.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// Migration of the revision kind (FT-165): typed within format 0.1 (ADR-0011 amendment 4, ID-334)

/// The two revision lines of the golden fixture as the cycle-16 build wrote them: `revision` was a reserved
/// kind, kept as an unknown record, so its members were written in byte-wise key order.
const OLD_REVISIONS: [&str; 2] = [
    r#"{"$k":"revision","date":"2025-03-03","description":"Initial issue","id":"4a41f813-2160-5abe-89f2-a3f019bf4268","label":"V1.0","source_ref":{"key":"revisions|V1.0","lineage":"763dd2f1-b9b8-558e-8d55-97d7304a4929"},"state":"released"}"#,
    r#"{"$k":"revision","author":"engineer","date":"2025-04-14","description":"Changes on 101 and 106","id":"31a45c26-4b99-565c-bbac-68077de2577c","label":"V2.0","page_notes":[{"page":"71260e59-ea02-5e16-8678-401d4044d9d7"},{"page":"b57668be-b65f-5802-a9dc-3ba4c8ed9866"}],"source_ref":{"key":"revisions|V2.0","lineage":"763dd2f1-b9b8-558e-8d55-97d7304a4929"},"state":"open"}"#,
];

/// The same records in the canonical form of the typed kind: `$k`, `id`, the members in declaration order.
const NEW_REVISIONS: [&str; 2] = [
    r#"{"$k":"revision","id":"4a41f813-2160-5abe-89f2-a3f019bf4268","label":"V1.0","date":"2025-03-03","description":"Initial issue","state":"released","source_ref":{"lineage":"763dd2f1-b9b8-558e-8d55-97d7304a4929","key":"revisions|V1.0"}}"#,
    r#"{"$k":"revision","id":"31a45c26-4b99-565c-bbac-68077de2577c","label":"V2.0","date":"2025-04-14","author":"engineer","description":"Changes on 101 and 106","state":"open","page_notes":[{"page":"71260e59-ea02-5e16-8678-401d4044d9d7"},{"page":"b57668be-b65f-5802-a9dc-3ba4c8ed9866"}],"source_ref":{"lineage":"763dd2f1-b9b8-558e-8d55-97d7304a4929","key":"revisions|V2.0"}}"#,
];

/// Upgrade on read: an old `revisions.jsonl` loads into typed records with every member, the only finding
/// is the info `NonCanonical` (nothing is quarantined or kept verbatim), the file is scheduled for a rewrite,
/// and the writer's lines are the canonical ones of the typed kind.
#[test]
fn an_old_revisions_file_loads_typed_and_is_rewritten_canonically() {
    let loaded = load_lines("revisions.jsonl", &OLD_REVISIONS);
    assert_eq!(
        kinds(&loaded),
        [FindingKind::NonCanonical],
        "{:?}",
        loaded.findings
    );
    assert!(loaded.kept.is_empty(), "{:?}", loaded.kept);
    assert!(loaded.rewrite.contains(&p("revisions.jsonl")));
    assert!(!loaded.read_only);
    let lines: Vec<String> = loaded
        .records
        .iter()
        .map(|r| {
            assert!(matches!(r, Record::Revision(_)), "{r:?}");
            record_line(r).unwrap()
        })
        .collect();
    assert_eq!(lines, NEW_REVISIONS);
    // The rewritten file reads back without a finding: the migration is one step.
    let again = load_lines("revisions.jsonl", &NEW_REVISIONS);
    assert!(again.findings.is_empty(), "{:?}", again.findings);
    assert!(again.rewrite.is_empty());
}

/// Members a newer build writes (the release record, the sign-off) survive in `extra`; a revision line that
/// does not parse (a page note as a bare page ID, which no writer produced) is quarantined verbatim with
/// EW-CORE-007, never dropped.
#[test]
fn revision_members_of_a_newer_build_survive_and_a_broken_line_is_quarantined() {
    let newer = r#"{"$k":"revision","id":"4a41f813-2160-5abe-89f2-a3f019bf4268","label":"V1.0","state":"released","release":{"changed_pages":[]},"sign_off":{"checker":"checker"}}"#;
    let loaded = load_lines("revisions.jsonl", &[newer]);
    assert!(
        loaded.findings.iter().all(|f| f.kind == FindingKind::NewerData),
        "{:?}",
        loaded.findings
    );
    let [Record::Revision(r)] = loaded.records.as_slice() else {
        panic!("one revision: {:?}", loaded.records);
    };
    assert_eq!(
        r.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["release", "sign_off"]
    );
    assert_eq!(record_line(&loaded.records[0]).unwrap(), newer);

    let broken = r#"{"$k":"revision","id":"4a41f813-2160-5abe-89f2-a3f019bf4268","label":"V1.0","page_notes":["71260e59-ea02-5e16-8678-401d4044d9d7"],"state":"open"}"#;
    let loaded = load_lines("revisions.jsonl", &[broken]);
    let found = of_kind(&loaded, FindingKind::SchemaViolation);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].code(), Some("EW-CORE-007"));
    assert!(loaded.records.is_empty());
    assert_eq!(loaded.kept.len(), 1);
    assert_eq!(loaded.kept[0].text, broken.as_bytes());
}

const MALFORMED: &str = r#"{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":{"ip":{"direction":"source","name":" bad"}}}"#;

#[test]
fn malformed_variant_is_ew_core_017_and_untouched() {
    let loaded = load_lines("devices/ff.jsonl", &[MALFORMED]);
    let found = of_kind(&loaded, FindingKind::MalformedVariant);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].line, Some(1));
    assert_eq!(found[0].code(), Some("EW-CORE-017"));
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.kept.len(), 1);
    let Kept::Untouched { original } = &loaded.kept[0].kept else {
        panic!("{:?}", loaded.kept[0]);
    };
    assert_eq!(**original, loaded.records[0]);
    assert_eq!(loaded.kept[0].text, MALFORMED.as_bytes());
    assert_eq!(
        loaded.kept[0].key.id().map(|i| i.to_string()).as_deref(),
        Some(FUNCTION_ID)
    );
}

#[test]
fn non_nfc_text_is_normalised_and_the_file_is_rewritten() {
    let line = PAGE.replace("Power Supply 1", "Re\u{301}sume\u{301}");
    assert!(line.contains('\u{301}'));
    let loaded = load_lines(PAGE_FILE, &[&line]);
    assert!(loaded.rewrite.contains(&p(PAGE_FILE)));
    let found = of_kind(&loaded, FindingKind::NonCanonical);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].file, Some(p(PAGE_FILE)));
    let Record::Page(page) = &loaded.records[0] else {
        panic!("{:?}", loaded.records[0]);
    };
    let text = serde_json::to_string(page).unwrap();
    assert!(text.contains("R\u{e9}sum\u{e9} - Main Current"), "{text}");
    assert!(!text.contains('\u{301}'));
}

#[test]
fn crlf_that_matches_the_manifest_after_conversion_is_line_endings() {
    let files = dir();
    let manifest = manifest_of(&files);
    let mut input = layout(files);
    let crlf = text_of(&input, SHARD_FILE).replace('\n', "\r\n");
    put(&mut input, SHARD_FILE, &crlf);
    let loaded = load(Some(&manifest), &input, &Limits::default());
    let found = of_kind(&loaded, FindingKind::LineEndings);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].file, Some(p(SHARD_FILE)));
    assert!(of_kind(&loaded, FindingKind::ForeignEdit).is_empty());
    assert!(loaded.rewrite.contains(&p(SHARD_FILE)));
    assert_eq!(loaded.rewrite.len(), 1);
    assert!(!loaded.read_only);
    // The records are all there, the hash is the one found on disk.
    assert_eq!(loaded.records.len(), 3);
    assert_eq!(loaded.hashes[&p(SHARD_FILE)], sha256(crlf.as_bytes()));
}

#[test]
fn foreign_edit_and_pending_hash_are_foreign_edit() {
    let files = dir();
    let manifest = manifest_of(&files);
    let mut input = layout(files);
    let edited = text_of(&input, SHARD_FILE).replace("\"counter\":1", "\"counter\":2");
    put(&mut input, SHARD_FILE, &edited);
    let loaded = load(Some(&manifest), &input, &Limits::default());
    let found = of_kind(&loaded, FindingKind::ForeignEdit);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].file, Some(p(SHARD_FILE)));
    // A valid edit in canonical form needs no rewrite.
    assert!(loaded.rewrite.is_empty());
    assert_eq!(loaded.records.len(), 3);

    let files = dir();
    let mut merged = manifest_of(&files);
    merged.merged = true;
    merged.files.insert(p(SHARD_FILE), ew_format::FileHash::Pending);
    let loaded = load(Some(&merged), &layout(files), &Limits::default());
    let found = of_kind(&loaded, FindingKind::ForeignEdit);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("pending"), "{}", found[0].message);
}

fn assert_rewritten_only(loaded: &Loaded, path: &str) {
    assert_eq!(loaded.rewrite.iter().collect::<Vec<_>>(), [&p(path)]);
    let found = of_kind(loaded, FindingKind::NonCanonical);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].file, Some(p(path)));
    assert_eq!(found[0].code(), None);
    assert!(!loaded.read_only);
    assert_eq!(kinds(loaded), [FindingKind::NonCanonical]);
    assert_eq!(loaded.records.len(), 3, "{:?}", loaded.records);
}

/// A shard in a non-canonical shape is read and rewritten at the next save, one case per shape: a BOM, a missing
/// final LF, a blank line and lines out of order are rewritten with no other effect (`assert_rewritten_only`); an
/// empty file is rewritten with one non-canonical finding and one record left in the layout.
#[test]
fn rewritten() {
    type Shape = fn(&str) -> String;
    let cases: [(&str, Shape, bool); 5] = [
        ("bom_is_rewritten", |t| format!("\u{feff}{t}"), true),
        (
            "missing_final_lf_is_rewritten",
            |t| t.trim_end_matches('\n').to_owned(),
            true,
        ),
        ("blank_line_is_rewritten", |t| t.replacen('\n', "\n\n", 1), true),
        (
            "lines_out_of_order_are_rewritten",
            |_| format!("{FUNCTION}\n{DEVICE}\n"),
            true,
        ),
        ("empty_file_is_rewritten", |_| String::new(), false),
    ];
    for (name, shape, only) in cases {
        let mut files = layout(dir());
        let text = shape(&text_of(&files, SHARD_FILE));
        put(&mut files, SHARD_FILE, &text);
        let loaded = load_plain(&files);
        if only {
            in_case(name, || assert_rewritten_only(&loaded, SHARD_FILE));
        } else {
            assert!(loaded.rewrite.contains(&p(SHARD_FILE)), "case {name}");
            assert_eq!(
                of_kind(&loaded, FindingKind::NonCanonical).len(),
                1,
                "case {name}"
            );
            assert_eq!(loaded.records.len(), 1, "case {name}");
        }
    }
}

/// Runs `check`, naming `case` in its failure message.
fn in_case(case: &str, check: impl FnOnce()) {
    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)) {
        let msg = e
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| e.downcast_ref::<&str>().copied())
            .unwrap_or("a panic without a message");
        panic!("case {case}: {msg}");
    }
}
#[test]
fn misplaced_record_is_ew_core_010() {
    let mut files = Files::new();
    put(&mut files, "devices/00.jsonl", &format!("{DEVICE}\n"));
    put(&mut files, SHARD_FILE, &format!("{FUNCTION}\n"));
    let loaded = load_plain(&files);
    let found = of_kind(&loaded, FindingKind::WrongFile);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].file, Some(p("devices/00.jsonl")));
    assert_eq!(found[0].line, Some(1));
    assert_eq!(found[0].code(), Some("EW-CORE-010"));
    assert!(found[0].message.contains(SHARD_FILE), "{}", found[0].message);
    assert_eq!(
        loaded.rewrite.iter().map(FilePath::as_str).collect::<Vec<_>>(),
        ["devices/00.jsonl", SHARD_FILE]
    );
    assert!(!loaded.read_only);
    assert_eq!(loaded.records.len(), 2);
}

// ---- load: library documents and assets -----------------------------------------------------------------------

/// The fndef of the repository's library with a correct copy hash.
fn fndef() -> LibDocument {
    let path = format!(
        "{}/../../content/fndefs/ew.fn.coil.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).unwrap();
    let mut doc = LibDocument::from_json(&text).unwrap();
    let LibDocument::Fndef(copy) = &mut doc else {
        panic!("not a fndef");
    };
    copy.sha256 = Sha256(sha256(serde_json::to_string(&copy.item).unwrap().as_bytes()));
    doc
}

fn doc_files(doc: &LibDocument, at: &str) -> Files {
    let mut files = Files::new();
    put(&mut files, at, &lib_document_text(doc).unwrap());
    files
}

#[test]
fn canonical_library_document_loads_clean() {
    let doc = fndef();
    let path = lib_file(&doc).unwrap();
    let files = doc_files(&doc, path.as_str());
    let loaded = load_plain(&files);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty());
    assert_eq!(loaded.lib, [doc]);
    assert!(loaded.opaque.is_empty() && loaded.kept_docs.is_empty());
}

#[test]
fn library_document_in_the_wrong_directory_is_ew_core_010() {
    let doc = fndef();
    let expected = lib_file(&doc).unwrap();
    let wrong = expected.as_str().replace("lib/fndefs/", "lib/parts/");
    let loaded = load_plain(&doc_files(&doc, &wrong));
    let found = of_kind(&loaded, FindingKind::WrongFile);
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].file, Some(p(&wrong)));
    assert_eq!(found[0].code(), Some("EW-CORE-010"));
    assert!(loaded.rewrite.contains(&p(&wrong)));
    assert!(loaded.rewrite.contains(&expected));
    assert_eq!(loaded.lib.len(), 1);
}

#[test]
fn library_copy_hash_mismatch_is_ew_core_014() {
    let mut doc = fndef();
    let LibDocument::Fndef(copy) = &mut doc else {
        panic!("not a fndef");
    };
    copy.sha256 = Sha256([7; 32]);
    let path = lib_file(&doc).unwrap();
    let loaded = load_plain(&doc_files(&doc, path.as_str()));
    assert_eq!(kinds(&loaded), [FindingKind::LibraryHash]);
    assert_eq!(loaded.findings[0].code(), Some("EW-CORE-014"));
    assert_eq!(loaded.findings[0].file, Some(path));
    assert!(loaded.rewrite.is_empty());
    assert_eq!(loaded.lib.len(), 1);
}

#[test]
fn non_canonical_library_document_is_rewritten() {
    let doc = fndef();
    let path = lib_file(&doc).unwrap();
    let compact = serde_json::to_string(&doc).unwrap();
    let mut files = Files::new();
    put(&mut files, path.as_str(), &compact);
    let loaded = load_plain(&files);
    assert_eq!(kinds(&loaded), [FindingKind::NonCanonical]);
    assert!(loaded.rewrite.contains(&path));
}

/// A BOM before a library document is a shape, as before a JSON Lines file: the document is read, and rewritten
/// at the next save. The cargo-fuzz target `migration_chain` found it (cycle 22): the document was a syntax error,
/// opaque, and the records naming it refused the whole project (`fuzz/corpus/migration_chain/lib-document-bom`).
#[test]
fn library_document_with_a_bom_is_read_and_rewritten() {
    let doc = fndef();
    let path = lib_file(&doc).unwrap();
    let text = format!("\u{feff}{}", lib_document_text(&doc).unwrap());
    let mut files = Files::new();
    put(&mut files, path.as_str(), &text);
    let loaded = load_plain(&files);
    assert_eq!(
        kinds(&loaded),
        [FindingKind::NonCanonical],
        "{:?}",
        loaded.findings
    );
    assert_eq!(loaded.findings[0].file, Some(path.clone()));
    assert_eq!(loaded.lib, [doc]);
    assert!(loaded.opaque.is_empty() && loaded.kept_docs.is_empty());
    assert_eq!(loaded.rewrite.iter().collect::<Vec<_>>(), [&path]);
    assert!(!loaded.read_only);
}

/// A BOM is stripped once: a second one is inside the JSON text, a syntax error that keeps the document opaque.
#[test]
fn library_document_with_two_boms_is_opaque() {
    let doc = fndef();
    let path = lib_file(&doc).unwrap();
    let text = format!("\u{feff}\u{feff}{}", lib_document_text(&doc).unwrap());
    let mut files = Files::new();
    put(&mut files, path.as_str(), &text);
    let loaded = load_plain(&files);
    assert_eq!(kinds(&loaded), [FindingKind::JsonSyntax], "{:?}", loaded.findings);
    assert!(loaded.lib.is_empty());
    assert_eq!(loaded.opaque[&path], sha256(text.as_bytes()));
    assert!(loaded.rewrite.is_empty());
}

#[test]
fn library_document_of_a_newer_kind_is_opaque_and_in_lib() {
    let id = "5d1ab4de-706a-5b85-b463-8b15311c4f39";
    let unknown = format!("{{\n  \"$k\": \"gadget\",\n  \"id\": \"{id}\",\n  \"z\": 1\n}}\n");
    let reserved = format!("{{\n  \"$k\": \"plc_profile\",\n  \"id\": \"{id}\"\n}}\n");
    let mut files = Files::new();
    put(&mut files, &format!("lib/fndefs/{id}.json"), &unknown);
    put(&mut files, &format!("lib/plc-profiles/{id}.json"), &reserved);
    let loaded = load_plain(&files);
    assert_eq!(kinds(&loaded), [FindingKind::NewerData]);
    assert_eq!(loaded.findings[0].file, Some(p(&format!("lib/fndefs/{id}.json"))));
    assert_eq!(loaded.lib.len(), 2);
    assert_eq!(loaded.opaque.len(), 2);
    assert_eq!(
        loaded.opaque[&p(&format!("lib/fndefs/{id}.json"))],
        sha256(unknown.as_bytes())
    );
    assert!(loaded.rewrite.is_empty());
}

#[test]
fn bad_library_documents_are_opaque() {
    let id = "5d1ab4de-706a-5b85-b463-8b15311c4f39";
    let mut files = Files::new();
    put(&mut files, &format!("lib/fndefs/{id}.json"), "{\"$k\": ");
    put(
        &mut files,
        "lib/parts/5d1ab4de-706a-5b85-b463-8b15311c4f3a.json",
        "{\"$k\": \"part\", \"$k\": \"part\"}",
    );
    put(
        &mut files,
        "lib/forms/5d1ab4de-706a-5b85-b463-8b15311c4f3b.json",
        "{\"$k\": \"form\"}",
    );
    let loaded = load_plain(&files);
    assert_eq!(
        kinds(&loaded),
        [
            FindingKind::JsonSyntax,
            FindingKind::SchemaViolation,
            FindingKind::SchemaViolation
        ]
    );
    assert_eq!(loaded.opaque.len(), 3);
    assert!(loaded.lib.is_empty());
    assert!(loaded.read_only);
}

#[test]
fn asset_with_the_wrong_hash_name_is_ew_core_015() {
    let content = b"<svg/>";
    let good = format!("assets/{}.svg", &hash_text(&sha256(content))["sha256:".len()..]);
    let bad = format!("assets/{}.svg", &hash_text(&sha256(b"other"))["sha256:".len()..]);
    let mut files = Files::new();
    files.insert(p(&good), content.to_vec());
    files.insert(p(&bad), content.to_vec());
    let loaded = load_plain(&files);
    assert_eq!(loaded.findings.len(), 1, "{:?}", loaded.findings);
    assert_eq!(loaded.findings[0].kind, FindingKind::AssetMismatch);
    assert_eq!(loaded.findings[0].code(), Some("EW-CORE-015"));
    assert_eq!(loaded.findings[0].file, Some(p(&bad)));
    assert_eq!(loaded.opaque.len(), 2);
    assert!(!loaded.read_only);
}

#[test]
fn a_listed_file_of_no_known_class_is_kept_with_newer_data() {
    let mut files = dir();
    files.insert(p("notes/extra.txt"), b"keep me".to_vec());
    let manifest = {
        let mut m = manifest_of(&files);
        m.files.insert(
            p("notes/extra.txt"),
            ew_format::FileHash::Sha256(sha256(b"keep me")),
        );
        m
    };
    let loaded = load(Some(&manifest), &layout(files), &Limits::default());
    assert_eq!(kinds(&loaded), [FindingKind::NewerData]);
    assert_eq!(loaded.opaque[&p("notes/extra.txt")], sha256(b"keep me"));
}

// ---- load: caps -----------------------------------------------------------------------------------------------

#[test]
fn findings_are_capped_with_one_truncated() {
    let lines: Vec<String> = (0..10).map(|i| format!("{{\"bad\":{i}")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut files = Files::new();
    put(&mut files, SHARD_FILE, &format!("{}\n", refs.join("\n")));
    let limits = Limits {
        max_findings: 3,
        ..Limits::default()
    };
    let loaded = load(None, &files, &limits);
    assert_eq!(loaded.findings.len(), 4, "{:?}", loaded.findings);
    assert_eq!(
        loaded
            .findings
            .iter()
            .filter(|f| f.kind == FindingKind::JsonSyntax)
            .count(),
        3
    );
    assert_eq!(loaded.findings[3].kind, FindingKind::Truncated);
    assert!(
        loaded.findings[3].message.starts_with("7 "),
        "{}",
        loaded.findings[3].message
    );
    assert!(loaded.read_only);
    // Every line is kept, whether or not its finding was listed.
    assert_eq!(loaded.kept.len(), 10);
}

// ---------------------------------------------------------------------------------------------------------
// Page items and structure boxes (WP-S2.04 part 4a): typed within format 0.1 (ADR-0011 amendment 4, ID-334)

/// A page file with both kinds, every typed page-item variant and an unknown variant, in the canonical form of the
/// typed kinds and in line order (doc 05 §3.3: the page, then the page items, then the structure box).
const ITEM_LINES: [&str; 6] = [
    PAGE,
    r#"{"$k":"page_item","id":"5a0c1e2d-3b4f-5a61-8000-000000000001","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","kind":{"graphic":{"shape":{"polyline":{"points":[[0,0],[1000000,0],[1000000,2000000]]}},"stroke":{"width":350000},"dash":"dash_dot"}}}"#,
    r#"{"$k":"page_item","id":"5a0c1e2d-3b4f-5a61-8000-000000000002","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","kind":{"free_text":{"text":"40x60","at":[50800000,30480000],"align":"right"}}}"#,
    r#"{"$k":"page_item","id":"5a0c1e2d-3b4f-5a61-8000-000000000003","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","kind":{"wire_marker":{"connections":["b29e4f17-8d63-5a0c-9f71-3c6a2e8d05b1"],"at":[90000000,24000000],"fields":["csa","colour"]}}}"#,
    r#"{"$k":"page_item","id":"5a0c1e2d-3b4f-5a61-8000-000000000004","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","kind":{"macro_box":{"key":"m1","outline":[[0,0],[1000000,0],[1000000,1000000]]}}}"#,
    r#"{"$k":"structure_box","id":"5a0c1e2d-3b4f-5a61-8000-000000000005","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","outline":[[20000000,40000000],[80000000,40000000],[80000000,64000000],[20000000,64000000]],"style":"dash_dot","caption":"External Connection","assigns":{"location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"}}"#,
];

/// The migration test of WP-S2.04 §10.1: a page file holding both kinds and an unknown `page_item` variant reads
/// without a finding (typed records, the unknown variant kept as `PageItemKind::Unknown`) and its lines are written
/// back byte for byte: nothing is quarantined, kept aside or scheduled for a rewrite.
#[test]
fn a_page_file_with_page_items_and_a_structure_box_round_trips_byte_for_byte() {
    use ew_format::ew_schema::PageItemKind;
    let loaded = load_lines(PAGE_FILE, &ITEM_LINES);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.kept.is_empty(), "{:?}", loaded.kept);
    assert!(loaded.rewrite.is_empty());
    assert_eq!(loaded.records.len(), ITEM_LINES.len());
    let lines: Vec<String> = loaded.records.iter().map(|r| record_line(r).unwrap()).collect();
    assert_eq!(lines, ITEM_LINES);
    let unknown = loaded
        .records
        .iter()
        .any(|r| matches!(r, Record::PageItem(i) if matches!(i.kind, PageItemKind::Unknown(_))));
    assert!(unknown, "the macro box is kept as an unknown variant");
    assert!(
        loaded
            .records
            .iter()
            .any(|r| matches!(r, Record::StructureBox(_)))
    );
}

/// The same records as the cycle-26 build wrote them: `page_item` and `structure_box` were reserved kinds, kept
/// as unknown records, so every object's members were written in byte-wise key order. This build reads them typed,
/// reports the file as not canonical (info) and schedules its rewrite in the canonical member order of the typed
/// kinds (the revision precedent, FT-165); a newer build's file read by the cycle-26 reader is the reserved-kind
/// path, kept line for line (`reserved_kind_is_kept_without_a_finding`).
#[test]
fn page_items_written_by_an_older_build_load_typed_and_are_rewritten_canonically() {
    let old: Vec<String> = ITEM_LINES
        .iter()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            serde_json::to_string(&v).unwrap()
        })
        .collect();
    assert_ne!(
        old[1], ITEM_LINES[1],
        "the byte-wise order differs from the typed one"
    );
    let old_refs: Vec<&str> = old.iter().map(String::as_str).collect();
    let loaded = load_lines(PAGE_FILE, &old_refs);
    assert_eq!(
        kinds(&loaded),
        [FindingKind::NonCanonical],
        "{:?}",
        loaded.findings
    );
    assert!(loaded.kept.is_empty(), "{:?}", loaded.kept);
    assert!(loaded.rewrite.contains(&p(PAGE_FILE)));
    let lines: Vec<String> = loaded.records.iter().map(|r| record_line(r).unwrap()).collect();
    assert_eq!(lines, ITEM_LINES);
}
