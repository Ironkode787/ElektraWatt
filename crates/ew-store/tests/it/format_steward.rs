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
            assert_eq!(
                project_files(&vfs, &root),
                tree(Path::new(&expected)),
                "{}",
                project.display()
            );
        }
    }
}
