// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The three-way merge target (WP-S1.17 part 4b, D15; decision log ID-1271 ruling 8): three arbitrary sides and a path
//! selector through `ew_format::merge_file`, the core of `ew merge-driver`, under the default limits and the shipped
//! chain. Properties: whatever the bytes, nothing panics; and a merge of two sides that each reload editable reloads
//! editable too, with or without conflicts (D14: "the file stays loadable"; decision log ID-1273). A side reloads when
//! it loads alone at its path through `ew_format::load` without read-only, `manifest.json` when `Manifest::parse`
//! reads it (the loader takes the manifest apart from the files).
//!
//! The input: one selector byte (an index into [`PATHS`], modulo its length), the base and ours each as a 4-byte
//! little-endian length and its bytes (a length past the end takes the rest), then theirs, the rest.

use std::collections::BTreeMap;

use ew_format::{FilePath, Limits, MIGRATIONS, Manifest, MergeSide, load, merge_file};

/// The paths a selector picks: each class of `merge_file` (record files of a page, the root and a device shard, a
/// library document, the manifest, a file of no project class).
pub const PATHS: [&str; 6] = [
    "pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl",
    "project.jsonl",
    "devices/c2.jsonl",
    "lib/parts/07a6c371-1881-5acc-9ac3-0d773f0ddc16.json",
    "manifest.json",
    "README.txt",
];

/// One decoded input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Case<'a> {
    /// The project-relative path, one of [`PATHS`].
    pub path: &'static str,
    /// `%O`.
    pub base: &'a [u8],
    /// `%A`.
    pub ours: &'a [u8],
    /// `%B`.
    pub theirs: &'a [u8],
}

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// The conflicts the merge wrote.
    pub conflicts: usize,
    /// Whether the reload property was checked (both sides reload).
    pub reloaded: bool,
}

/// The next length-prefixed part of `input`, and the rest.
fn part(input: &[u8]) -> (&[u8], &[u8]) {
    let (len, rest) = input.split_at(input.len().min(4));
    let mut le = [0u8; 4];
    le[..len.len()].copy_from_slice(len);
    let n = (u32::from_le_bytes(le) as usize).min(rest.len());
    rest.split_at(n)
}

/// Decodes an input.
pub fn decode(input: &[u8]) -> Case<'_> {
    let (selector, rest) = input.split_first().map_or((0, input), |(s, r)| (*s, r));
    let (base, rest) = part(rest);
    let (ours, theirs) = part(rest);
    Case {
        path: PATHS[usize::from(selector) % PATHS.len()],
        base,
        ours,
        theirs,
    }
}

/// Encodes a case: the selector `path` (an index into [`PATHS`]) and the three sides.
///
/// # Panics
///
/// When the base or ours is 4 GiB or longer.
pub fn encode(path: u8, base: &[u8], ours: &[u8], theirs: &[u8]) -> Vec<u8> {
    let mut out = vec![path];
    for side in [base, ours] {
        out.extend(u32::try_from(side.len()).expect("under 4 GiB").to_le_bytes());
        out.extend(side);
    }
    out.extend(theirs);
    out
}

/// Whether `bytes` at `path` reload editable.
fn reloads(path: &str, bytes: &[u8]) -> bool {
    let limits = Limits::default();
    if path == "manifest.json" {
        return Manifest::parse(bytes, &limits).is_ok();
    }
    let files = BTreeMap::from([(FilePath::new(path).expect("a project path"), bytes.to_vec())]);
    !load(None, &files, &limits).read_only
}

/// Runs one input.
///
/// # Panics
///
/// When a merge of two sides that reload editable does not reload editable.
pub fn check(input: &[u8]) -> Outcome {
    let case = decode(input);
    let merged = merge_file(
        case.path,
        MergeSide::current(case.base),
        MergeSide::current(case.ours),
        MergeSide::current(case.theirs),
        &Limits::default(),
        &MIGRATIONS,
    );
    let reloaded = reloads(case.path, case.ours) && reloads(case.path, case.theirs);
    if reloaded {
        assert!(
            reloads(case.path, &merged.bytes),
            "a merge of {} with {} conflicts does not reload editable:\n{}",
            case.path,
            merged.conflicts,
            String::from_utf8_lossy(&merged.bytes)
        );
    }
    Outcome {
        conflicts: merged.conflicts,
        reloaded,
    }
}
