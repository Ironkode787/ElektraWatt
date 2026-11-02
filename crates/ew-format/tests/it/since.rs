// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The computed `min_reader` on Golden A (WP-S1.17 D3, FT-95, SG-1): `"0.1"` with the shipped tables, and the greatest
//! format of an injected table's rows that Golden A's records use. Golden A is only read.

use std::collections::BTreeMap;
use std::path::Path;

use ew_format::ew_schema::{FORMAT, Kind, MemberSince, VariantSince};
use ew_format::{
    FilePath, Limits, MANIFEST, Manifest, SINCE, SinceTables, file_since, manifest_of, min_reader,
};

/// Every file of `golden/J-E-2416.ew`, read only, by project-relative path.
fn golden() -> BTreeMap<FilePath, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<FilePath, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("below the base")
                    .to_string_lossy()
                    .replace('\\', "/");
                let rel = FilePath::new(rel).expect("a project path");
                out.insert(rel, std::fs::read(&path).expect("a readable file"));
            }
        }
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    let mut out = BTreeMap::new();
    walk(&dir, &dir, &mut out);
    assert!(!out.is_empty(), "the golden fixture {} is missing", dir.display());
    out
}

/// `min_reader` of `files` for a manifest of `format` under `tables`, the manifest left out.
fn computed(format: &str, files: &BTreeMap<FilePath, Vec<u8>>, tables: &SinceTables<'_>) -> String {
    min_reader(
        format,
        files
            .iter()
            .filter(|(path, _)| path.as_str() != MANIFEST)
            .filter_map(|(path, bytes)| file_since(format, path, bytes, tables)),
    )
}

#[test]
fn golden_a_needs_a_reader_of_its_own_format() {
    let files = golden();
    let manifest = Manifest::parse(
        &files[&FilePath::new(MANIFEST).expect("a path")],
        &Limits::default(),
    )
    .expect("Golden A's manifest");
    assert_eq!(computed(FORMAT, &files, &SINCE), "0.1");
    assert_eq!(manifest.min_reader, "0.1", "the stamp on disk agrees");
    let mut listed = files.clone();
    listed.retain(|path, _| path.as_str() != MANIFEST);
    let rewritten = manifest_of(&listed, manifest.project, 1, "ElektraWatt 0.0.0");
    assert_eq!(
        rewritten.min_reader, "0.1",
        "the importer's path computes the same stamp"
    );
}

#[test]
fn an_injected_newer_member_or_variant_that_golden_a_uses_raises_min_reader() {
    // SG-1, first clause, on Golden A's records: before 1.0 a format's floor is the format itself (doc 05 §10.1), so
    // the tables name 1.x formats and the manifest is a 1.3 one. Golden A has devices with a function text and junctions
    // drawn as a bevel; it has no `frobnicate` member.
    const MEMBERS: &[MemberSince] = &[
        MemberSince {
            kind: Kind::Device,
            path: "function_text",
            since: "1.1",
        },
        MemberSince {
            kind: Kind::Device,
            path: "frobnicate",
            since: "1.3",
        },
    ];
    const VARIANTS: &[VariantSince] = &[VariantSince {
        kind: Kind::Junction,
        path: "style",
        variant: "bevel",
        since: "1.2",
    }];
    let files = golden();
    let members = SinceTables {
        kinds: &[],
        members: MEMBERS,
        variants: &[],
    };
    assert_eq!(computed("1.3", &files, &members), "1.1");
    let both = SinceTables {
        kinds: &[],
        members: MEMBERS,
        variants: VARIANTS,
    };
    assert_eq!(computed("1.3", &files, &both), "1.2");
    // Only the files that use them carry a value.
    let pages = files
        .iter()
        .filter(|(path, bytes)| file_since("1.3", path, bytes, &both) == Some("1.2"))
        .count();
    assert!(pages > 0, "a page with a bevelled junction");
    assert!(
        files
            .iter()
            .filter(|(path, _)| path.as_str() == "project.jsonl")
            .all(|(path, bytes)| file_since("1.3", path, bytes, &both).is_none()),
        "the project file uses neither"
    );
    // Never above the manifest's format: a 1.1 manifest is 1.1.
    assert_eq!(computed("1.1", &files, &both), "1.1");
}
