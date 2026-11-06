// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The steward's check and the fixture corpus job (WP-S1.17 D12 as decision log ID-1221 ruling 8 amends it): the
//! check runs on the shipped constants and the repository's corpus `fixtures/format/`, and the corpus job opens, saves
//! and compares each `fixtures/format/<format>/<project>/` with `<project>.expected/`. Both pass while nothing is
//! released and the corpus is absent; WP-S6.08 adds `fixtures/format/0.1/` with the first release.

// The corpus is read from the repository directly, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use common::{options, project_files};
use ew_store::ew_format::ew_schema::{FORMAT, RELEASED_FORMATS};
use ew_store::ew_format::{MIGRATIONS, steward_problems};
use ew_store::mem::DirSync;
use ew_store::{Changes, Opened, Store};

/// The repository's fixture corpus.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/format")
}

/// Every file below `dir` by relative path with `/`.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
            let path = e.expect("a directory entry").path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("below the base")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).expect("a readable file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The subfolders of `dir` in name order; none when it is absent.
fn folders(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// MG-6's steward half: the shipped chain, the released formats and the repository's corpus have no problem.
#[test]
fn mg6_the_shipped_chain_and_corpus_pass_the_steward_check() {
    let problems = steward_problems(RELEASED_FORMATS, FORMAT, &MIGRATIONS, &corpus());
    let words: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(words.is_empty(), "{words:#?}");
}

/// FT-96 (WP-S6.08 D5 and D24, RL-4; ADR-0011 amendment 1, doc 05 §10.3): a project of this format whose record lines
/// carry members this reader does not know, appended as a later additive release writes them (after a record's members,
/// two of them out of byte order, and after a nested object's), opens; an edit of another record of the same file
/// saves, and every untouched line of that file is written as it was, byte for byte, the edited record keeping its
/// unknown members.
#[test]
fn ft96_untouched_lines_with_later_members_are_kept_byte_for_byte() {
    use ew_model::{AnyRow, Op};
    use ew_store::ew_format::ew_schema::Device;
    use ew_store::ew_format::{FileHash, FilePath, Limits, MANIFEST, Manifest, sha256};
    use std::sync::Arc;

    let later = |line: &str| -> String {
        let line = match line.strip_suffix("}}") {
            Some(head) if !head.ends_with('{') => format!("{head},\"later_ref\":1}}}}"),
            _ => line.to_owned(),
        };
        format!(
            "{},\"zeta\":\"kept\",\"alpha\":\"kept\"}}",
            &line[..line.len() - 1]
        )
    };
    let mut files = common::small_fixture();
    let (shard, text) = files
        .iter()
        .filter(|(p, _)| p.starts_with("devices/"))
        .map(|(p, b)| (p.clone(), String::from_utf8(b.clone()).expect("UTF-8")))
        .find(|(_, t)| t.lines().count() >= 3 && t.starts_with("{\"$k\":\"device\""))
        .expect("a device shard of three lines or more");
    let lines: Vec<String> = text.lines().map(later).collect();
    assert!(
        lines.iter().any(|l| l.contains("\"later_ref\":1}")),
        "a nested member too"
    );
    let bytes = format!("{}\n", lines.join("\n")).into_bytes();
    let mut manifest = Manifest::parse(&files[MANIFEST], &Limits::default()).unwrap();
    manifest
        .files
        .insert(FilePath::new(&shard).unwrap(), FileHash::Sha256(sha256(&bytes)));
    files.insert(shard.clone(), bytes);
    files.insert(MANIFEST.to_owned(), manifest.to_text().into_bytes());

    let (vfs, root) = common::mem_project_of(DirSync::Explicit, files);
    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, ew_store::Access::Edit);
    let first: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    let id = first["id"].as_str().unwrap().to_owned();
    let row = model
        .table::<Device>()
        .iter()
        .find(|(_, d)| d.id.to_string() == id)
        .map(|(_, d)| Arc::clone(d))
        .expect("the shard's first device");
    let after = common::device_with_text(&row, "edited");
    let mut keys = BTreeSet::new();
    let model = common::apply(
        &model,
        &[Op::update(AnyRow::Device(row), AnyRow::of(after))],
        &mut keys,
    )
    .expect("the edit applies");
    let saved = store.save(&model, Changes::Keys(&keys)).expect("saves");
    store.close();
    assert_eq!(saved.written, [FilePath::new(&shard).unwrap()]);
    let written = String::from_utf8(project_files(&vfs, &root)[&shard].clone()).unwrap();
    let written: Vec<&str> = written.lines().collect();
    assert_eq!(written.len(), lines.len());
    assert!(
        written[0].contains("\"function_text\":\"edited\"")
            && written[0].contains("\"alpha\":\"kept\"")
            && written[0].contains("\"zeta\":\"kept\""),
        "{}",
        written[0]
    );
    assert_eq!(&written[1..], &lines[1..]);
}

/// The corpus job: each project of each released format's folder opens (migrated when older, with the consent),
/// saves in full and equals its expected folder file by file. No project is there while the corpus is absent.
#[test]
fn the_corpus_job_saves_each_fixture_to_its_expected_files() {
    for format in folders(&corpus()) {
        let projects: BTreeSet<PathBuf> = folders(&format)
            .into_iter()
            .filter(|p| !p.extension().is_some_and(|x| x == "expected"))
            .collect();
        for project in projects {
            let mut expected = project.clone().into_os_string();
            expected.push(".expected");
            let (vfs, root) = common::mem_project_of(DirSync::Explicit, tree(&project));
            let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1))
                .unwrap_or_else(|e| panic!("{} opens: {e}", project.display()));
            store.confirm_upgrade();
            store
                .save(&model, Changes::All)
                .unwrap_or_else(|e| panic!("{} saves: {e}", project.display()));
            store.close();
            // `EW_BLESS=1` writes the save as the expected folder (WP-S6.08 D13); the fixture itself never changes.
            if std::env::var_os("EW_BLESS").is_some() {
                let expected = Path::new(&expected);
                let _ = std::fs::remove_dir_all(expected);
                for (rel, bytes) in project_files(&vfs, &root) {
                    let path = expected.join(&rel);
                    std::fs::create_dir_all(path.parent().expect("below the folder")).expect("a folder");
                    std::fs::write(path, bytes).expect("an expected file");
                }
            }
            assert_eq!(
                project_files(&vfs, &root),
                tree(Path::new(&expected)),
                "{}",
                project.display()
            );
        }
    }
}
