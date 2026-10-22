// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The loader target (FT-297, doc 18 §3.12): arbitrary bytes through the `ew-format` reader.
//!
//! The input is a set of files with a mode byte ([`decode`]); the reader's three stages run over it: the
//! manifest ([`Manifest::parse`]), the plan of the files to read ([`plan`]) and the load ([`load`]). Properties:
//!
//! - no stage panics, whatever the bytes, the manifest and the limits (the tiny limits of mode bit 1 put the
//!   limit paths within reach of small inputs);
//! - a manifest that parses writes back to a manifest that parses to the same value;
//! - **a valid input round-trips**: when the load had no finding, the loaded records and library documents
//!   render to files that load again without a finding, with nothing to rewrite and the same records;
//! - in every case where the loaded content renders, the canonical rendering is a fixpoint: loading the
//!   rendering and rendering that gives the same files.

use std::collections::BTreeMap;

use ew_format::{
    EntryKind, FileHash, FilePath, Limits, Loaded, Manifest, ReadPlan, load, plan, render_records, sha256,
};

use crate::golden;

/// Paths a file selector can name beside the golden project's files: the other record-file classes, a library
/// document, an asset, an untrusted file, a stray name (all with IDs that parse).
const EXTRA: [&str; 8] = [
    "layout/0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e.jsonl",
    "imports/0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e.jsonl",
    "exchange/0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e.jsonl",
    "pages/0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e.jsonl",
    "lib/symbols/0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e.json",
    "assets/0000000000000000000000000000000000000000000000000000000000000000.svg",
    "scripts/a.rhai",
    "devices/zz.jsonl",
];

/// The selector of the manifest section.
pub const MANIFEST_SELECTOR: u8 = 0xFF;

/// Mode bit 0: the manifest is made from the files (the hashes match), not read from the manifest section.
pub const MODE_COMPUTED_MANIFEST: u8 = 1;
/// Mode bit 1: tiny limits.
pub const MODE_TINY_LIMITS: u8 = 2;

/// A decoded input.
#[derive(Debug, Default)]
pub struct Case {
    /// The mode byte.
    pub mode: u8,
    /// The files, by path.
    pub files: BTreeMap<FilePath, Vec<u8>>,
    /// The manifest section's bytes.
    pub manifest: Option<Vec<u8>>,
}

/// The path a file selector names.
fn path_of(selector: u8) -> Option<FilePath> {
    let golden = golden::paths();
    let at = usize::from(selector) % (golden.len() + EXTRA.len());
    let name = golden
        .get(at)
        .map_or_else(|| EXTRA[at - golden.len()], String::as_str);
    FilePath::new(name).ok()
}

/// Decodes an input: a mode byte, then sections `[selector][length: u16, little-endian][bytes]`; a section that
/// is cut short takes what is left. A selector indexes the golden project's paths followed by [`EXTRA`] (modulo
/// their number); [`MANIFEST_SELECTOR`] is the manifest.
pub fn decode(input: &[u8]) -> Case {
    let mut case = Case::default();
    let Some((&mode, mut rest)) = input.split_first() else {
        return case;
    };
    case.mode = mode;
    while let [selector, lo, hi, tail @ ..] = rest {
        let len = usize::from(u16::from_le_bytes([*lo, *hi])).min(tail.len());
        let (bytes, after) = tail.split_at(len);
        if *selector == MANIFEST_SELECTOR {
            case.manifest = Some(bytes.to_vec());
        } else if let Some(path) = path_of(*selector) {
            case.files.insert(path, bytes.to_vec());
        }
        rest = after;
    }
    case
}

/// The inverse of [`decode`] for the seed corpus: files by golden path.
pub fn encode(mode: u8, files: &[(&str, &[u8])], manifest: Option<&[u8]>) -> Vec<u8> {
    let mut out = vec![mode];
    let mut section = |selector: u8, bytes: &[u8]| {
        let len = u16::try_from(bytes.len()).expect("a section of at most 65535 bytes");
        out.push(selector);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(bytes);
    };
    for (path, bytes) in files {
        section(golden::index_of(path), bytes);
    }
    if let Some(m) = manifest {
        section(MANIFEST_SELECTOR, m);
    }
    out
}

/// The limits of a run: the defaults, or small ones that a small input can reach.
fn limits_of(mode: u8) -> Limits {
    if mode & MODE_TINY_LIMITS == 0 {
        return Limits::default();
    }
    Limits {
        max_line_bytes: 256,
        max_file_bytes: 2048,
        max_files: 6,
        max_depth: 6,
        max_lines_per_file: 8,
        max_manifest_bytes: 1024,
        max_findings: 4,
    }
}

/// Plans and loads `files` under `manifest` the way the store does: the files the plan names, that are present.
fn read(
    manifest: Option<&Manifest>,
    files: &BTreeMap<FilePath, Vec<u8>>,
    limits: &Limits,
) -> (ReadPlan, Loaded) {
    let present: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|p| (p.as_str().to_owned(), EntryKind::File))
        .collect();
    let reads = plan(manifest, &present, limits);
    let input: BTreeMap<FilePath, Vec<u8>> = reads
        .read
        .iter()
        .filter_map(|p| files.get(p).map(|b| (p.clone(), b.clone())))
        .collect();
    let loaded = load(manifest, &input, limits);
    (reads, loaded)
}

/// The golden project's manifest, the template of the manifests made from files.
fn template() -> Manifest {
    let bytes = &golden::files()["manifest.json"];
    Manifest::parse(bytes, &Limits::default()).expect("the golden manifest parses")
}

/// A manifest listing `files` with their hashes.
fn computed(files: &BTreeMap<FilePath, Vec<u8>>) -> Manifest {
    let mut manifest = template();
    manifest.files = files
        .iter()
        .map(|(p, bytes)| (p.clone(), FileHash::Sha256(sha256(bytes))))
        .collect();
    manifest
}

/// How far an input got (for the replay test; the fuzz target ignores it).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// The records the load returned.
    pub records: usize,
    /// The findings the load returned.
    pub findings: usize,
    /// The loaded content rendered and was loaded again (the round-trip checks ran).
    pub round_tripped: bool,
    /// The input was valid (no finding): the strong round-trip checks ran.
    pub valid: bool,
}

/// Runs the properties over one input; panics (with the failure) when one does not hold.
pub fn check(input: &[u8]) -> Outcome {
    let case = decode(input);
    let limits = limits_of(case.mode);

    // The manifest stage.
    let parsed = case
        .manifest
        .as_deref()
        .and_then(|bytes| Manifest::parse(bytes, &limits).ok());
    if let Some(m) = &parsed {
        let again = Manifest::parse(m.to_text().as_bytes(), &Limits::default());
        assert_eq!(
            again.as_ref(),
            Ok(m),
            "a parsed manifest does not survive its own text form"
        );
    }
    let manifest = if case.mode & MODE_COMPUTED_MANIFEST != 0 {
        Some(computed(&case.files))
    } else {
        parsed
    };

    // The plan and the load.
    let (_, loaded) = read(manifest.as_ref(), &case.files, &limits);
    let mut outcome = Outcome {
        records: loaded.records.len(),
        findings: loaded.findings.len(),
        ..Outcome::default()
    };
    if case.mode & MODE_TINY_LIMITS != 0 {
        return outcome;
    }
    let Some(project) = &loaded.project else {
        return outcome;
    };
    let Ok(files1) = render_records(project, &loaded.records, &loaded.lib) else {
        // A record of an unknown kind that needs more than the record to name its file: not a defect of the reader.
        return outcome;
    };
    outcome.round_tripped = true;

    // Reload the rendering: the manifest is made from it.
    let m1 = computed(&files1);
    let (_, again) = read(Some(&m1), &files1, &limits);
    let valid = loaded.findings.is_empty() && !loaded.read_only;
    outcome.valid = valid;
    if valid {
        assert!(
            again.findings.is_empty(),
            "a valid input renders to files that load with findings: {:?}",
            again.findings
        );
        assert!(
            again.rewrite.is_empty(),
            "a valid input renders to files that need a rewrite: {:?}",
            again.rewrite
        );
        assert_eq!(
            again.project, loaded.project,
            "the project changed over the round trip"
        );
        assert_eq!(
            again.records.len(),
            loaded.records.len(),
            "the records changed over the round trip"
        );
        for record in &loaded.records {
            assert!(
                again.records.contains(record),
                "{record:?} is lost over the round trip"
            );
        }
        assert_eq!(
            again.lib, loaded.lib,
            "the library documents changed over the round trip"
        );
    }
    if let Some(project) = &again.project {
        if let Ok(files2) = render_records(project, &again.records, &again.lib) {
            assert_eq!(
                files1, files2,
                "the canonical rendering is not a fixpoint of load and render"
            );
        }
    }
    outcome
}
