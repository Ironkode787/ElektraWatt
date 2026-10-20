// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writing a whole project from records (doc 05 §2, §3, §4): the path of an importer's batch, of `ew import
//! csv` and of the golden fixture (doc 20 §3.2 path (c)). `ew-store` writes saves file by file with the same
//! functions.

use std::collections::{BTreeMap, BTreeSet};

use ew_schema::{AnyId, Kind, LibDocument, Project, Record};

use crate::error::FormatError;
use crate::layout::{RecordAnchors, lib_file, lib_key, record_file};
use crate::line::{jsonl_file, lib_document_text, record_line, tagged_line};
use crate::manifest::{FileHash, Manifest, sha256};
use crate::order::LineKey;
use crate::path::{FilePath, MANIFEST};

/// Every JSON Lines file and library document of a project made of `project`, `records` and `lib` (doc 05
/// §2, §3), by path, without the manifest. The result does not depend on the order of `records` and `lib`.
///
/// # Errors
///
/// - [`FormatError::Duplicate`] for two records or documents with one `($k, id)`, or a `project` record
///   among `records`;
/// - [`FormatError::NoFile`] for a record of a reserved or unknown kind whose file needs more than the
///   record ([`unknown_record_file`](crate::unknown_record_file)) and for a library document of an unknown
///   kind;
/// - [`FormatError::Serialize`] for a record without a JSON form.
pub fn render_records(
    project: &Project,
    records: &[Record],
    lib: &[LibDocument],
) -> Result<BTreeMap<FilePath, Vec<u8>>, FormatError> {
    let anchors = RecordAnchors::new(records);
    let mut seen: BTreeSet<(String, AnyId)> = BTreeSet::new();
    let mut lines: BTreeMap<FilePath, Vec<(LineKey, String)>> = BTreeMap::new();
    let project_id: AnyId = project.id.cast();
    seen.insert((Kind::Project.name().to_owned(), project_id));
    lines.entry(FilePath::project()).or_default().push((
        LineKey::new(Kind::Project.name(), Some(project_id)),
        tagged_line(Kind::Project, project)?,
    ));
    for record in records {
        let kind = record.kind_name().unwrap_or("").to_owned();
        let Some(id) = record.id().map(AnyId::from_uuid) else {
            return Err(FormatError::NoFile { kind, id: None });
        };
        if !seen.insert((kind.clone(), id)) {
            return Err(FormatError::Duplicate { kind, id });
        }
        let file = record_file(record, &anchors)?;
        let key = LineKey::new(&kind, Some(id));
        lines.entry(file).or_default().push((key, record_line(record)?));
    }
    let mut out: BTreeMap<FilePath, Vec<u8>> = lines
        .into_iter()
        .map(|(path, lines)| (path, jsonl_file(lines)))
        .collect();
    for doc in lib {
        let (Some(path), Some((kind, id))) = (lib_file(doc), lib_key(doc)) else {
            return Err(FormatError::NoFile {
                kind: "library document of an unknown kind".into(),
                id: None,
            });
        };
        if out.contains_key(&path) {
            return Err(FormatError::Duplicate {
                kind: kind.name().to_owned(),
                id,
            });
        }
        out.insert(path, lib_document_text(doc)?.into_bytes());
    }
    Ok(out)
}

/// The manifest of a set of files: `format` and `min_reader` this crate's [`FORMAT`](ew_schema::FORMAT)
/// (no member of a newer minor exists yet, so `min_reader` equals `format`), the given generator and
/// generation, and the SHA-256 of every file.
pub fn manifest_of(
    files: &BTreeMap<FilePath, Vec<u8>>,
    project: AnyId,
    generation: u64,
    generator: &str,
) -> Manifest {
    let mut manifest = Manifest::new(ew_schema::FORMAT, generator, project, generation);
    manifest.files = files
        .iter()
        .map(|(path, bytes)| (path.clone(), FileHash::Sha256(sha256(bytes))))
        .collect();
    manifest
}

/// A complete project directory as files: [`render_records`] plus `manifest.json` ([`manifest_of`]) of
/// generation `generation`.
///
/// # Errors
///
/// As [`render_records`].
pub fn project_files(
    project: &Project,
    records: &[Record],
    lib: &[LibDocument],
    generation: u64,
    generator: &str,
) -> Result<BTreeMap<FilePath, Vec<u8>>, FormatError> {
    let mut files = render_records(project, records, lib)?;
    let manifest = manifest_of(&files, project.id.cast(), generation, generator);
    let path = FilePath::new(MANIFEST).map_err(|_| FormatError::NoProject)?;
    files.insert(path, manifest.to_text().into_bytes());
    Ok(files)
}
