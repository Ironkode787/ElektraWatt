// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A QElectroTech element collection into a user library, on demand (doc 07 §3.6, FR-SYM-029): the `.elmt` files of a
//! directory (the user's QET installation) become symbols and function definitions of a writable library, each on the
//! QET 10 px grid with its source path, under the licence `LicenseRef-QET-elements` and the flag `foreign_terms`. QET's
//! terms allow the elements in diagrams but not in EW's packs; the flag is what keeps them out of any pack
//! (NFR-LIC-06). The collection is never bundled, and `ELEMENTS.LICENSE`, when the directory has it, is named in every
//! item's sources with its SHA-256.

use std::collections::BTreeSet;
use std::path::Path;

use ew_derive::ew_ids::v5::{self, LineageAnchor};
use ew_derive::ew_ids::{Id, Uuid};
use ew_import_qet::{ElementSource, read_element};
use ew_schema::{ImportLineage, LibDocument, Project};
use sha2::Digest as _;

use crate::catalog::{CatalogError, ImportReport, Limits, ReportEntry, Severity};
use crate::library;

/// The licence of every item of a collection.
pub const LICENCE: &str = "LicenseRef-QET-elements";

/// The file of QET's element licence, beside the elements.
pub const LICENCE_FILE: &str = "ELEMENTS.LICENSE";

/// The deepest folder read, and the most files.
const MAX_DEPTH: usize = 16;

/// One `.elmt` of a collection: its path inside the collection (`/` separated) and its bytes.
pub type ElementFile = (String, Vec<u8>);

/// The files of a collection read from disk: the elements, and the licence file's SHA-256 when it exists.
#[derive(Debug)]
pub struct Collection {
    /// The `.elmt` files by path, in path order.
    pub elements: Vec<ElementFile>,
    /// The SHA-256 of `ELEMENTS.LICENSE` at the top of the directory, when there is one.
    pub licence: Option<[u8; 32]>,
    /// The files too large to be an element definition, which are not read (path, why).
    pub skipped: Vec<(String, String)>,
}

/// Reads the `.elmt` files below `dir` (links are not followed; at most 16 folders deep, 100,000 files, each at most
/// 16 MiB, 512 MiB together).
///
/// # Errors
///
/// [`CatalogError::Io`] when the directory cannot be read, [`CatalogError::Import`] over a limit.
pub fn read_collection(dir: &Path) -> Result<Collection, CatalogError> {
    let limits = Limits::default();
    let io = |p: &Path, e: std::io::Error| CatalogError::Io(p.display().to_string(), e);
    let too_many = |what: &str, limit: u64| {
        CatalogError::Import(crate::catalog::ImportError::LimitExceeded {
            what: what.to_owned(),
            limit,
        })
    };
    let mut found: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut todo = vec![(dir.to_path_buf(), String::new(), 0usize)];
    while let Some((folder, prefix, depth)) = todo.pop() {
        for entry in std::fs::read_dir(&folder).map_err(|e| io(&folder, e))? {
            let entry = entry.map_err(|e| io(&folder, e))?;
            let meta = std::fs::symlink_metadata(entry.path()).map_err(|e| io(&entry.path(), e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if meta.is_dir() && depth < MAX_DEPTH {
                todo.push((entry.path(), path, depth + 1));
            } else if meta.is_file() && name.to_ascii_lowercase().ends_with(".elmt") {
                if found.len() as u64 >= u64::from(limits.max_archive_entries) {
                    return Err(too_many(
                        "collection files",
                        u64::from(limits.max_archive_entries),
                    ));
                }
                found.push((path, entry.path()));
            }
        }
    }
    found.sort();
    let max = ew_import_qet::ew_interop_api::Limits::element_definition().max_input_bytes;
    let (mut total, mut elements, mut skipped) = (0u64, Vec::new(), Vec::new());
    for (path, file) in found {
        let len = std::fs::metadata(&file).map_err(|e| io(&file, e))?.len();
        if len > max {
            skipped.push((path, format!("larger than {max} bytes")));
            continue;
        }
        total += len;
        if total > limits.max_input_bytes {
            return Err(too_many("collection size", limits.max_input_bytes));
        }
        elements.push((path, std::fs::read(&file).map_err(|e| io(&file, e))?));
    }
    let licence = std::fs::read(dir.join(LICENCE_FILE))
        .ok()
        .filter(|b| b.len() <= 1024 * 1024)
        .map(|b| sha2::Sha256::digest(&b).into());
    Ok(Collection {
        elements,
        licence,
        skipped,
    })
}

/// The symbols and function definitions of a collection, and what was said about the files.
#[derive(Debug)]
pub struct Converted {
    /// The documents for the library: function definitions and symbols, deduplicated, in path order of their files.
    pub documents: Vec<LibDocument>,
    /// The symbols read.
    pub symbols: usize,
    /// The files that could not be read, with the reason (path, why).
    pub refused: Vec<(String, String)>,
    /// The findings of the readings and a summary of what repeats.
    pub report: ImportReport,
}

/// Converts the `files` of a collection into documents of the library `library`. The item IDs are minted in the
/// lineage `qet` anchored on the library, so converting the same collection again gives the same items. A file the
/// reader refuses is listed and the rest go on.
pub fn convert(library: Uuid, collection: &Collection) -> Converted {
    let namespace = v5::lineage_namespace("qet", &LineageAnchor::Fixed(&library.hyphenated().to_string()))
        .unwrap_or_else(|_| unreachable!("\"qet\" is a valid importer name"));
    let lineage = ew_import_qet::ew_interop_api::LineageInfo {
        id: v5::imported_id::<ImportLineage>(namespace, "qet:collection"),
        namespace,
    };
    let project: Id<Project> = Id::from_uuid(library);
    let limits = ew_import_qet::ew_interop_api::Limits::element_definition();
    let mut report = ImportReport::new(
        "qet",
        ew_import_qet::IMPORTER_VERSION,
        ew_import_catalog::ew_interop_api::SourceInfo {
            name: "QElectroTech element collection".to_owned(),
            format: "qet-elements".to_owned(),
            ..Default::default()
        },
    );
    let (mut documents, mut seen, mut refused, mut symbols) =
        (Vec::new(), BTreeSet::new(), Vec::new(), 0usize);
    let licence_source = collection.licence.map(|sha| {
        let hex: String = sha.iter().map(|b| format!("{b:02x}")).collect();
        format!("qet.licence:{LICENCE_FILE}@sha256:{hex}")
    });
    for (path, why) in &collection.skipped {
        report.push(ReportEntry::new(
            crate::import::Code::LIMIT,
            Severity::Error,
            format!("{path}: not converted: {why}"),
        ));
        refused.push((path.clone(), why.clone()));
    }
    for (path, bytes) in &collection.elements {
        let file = path.rsplit('/').next().unwrap_or(path);
        let result = std::str::from_utf8(bytes)
            .map_err(|_| "not UTF-8 text".to_owned())
            .and_then(|xml| {
                let src = ElementSource::new(file, Some(path)).map_err(|e| e.to_string())?;
                read_element(xml, &src, &lineage, project, &limits).map_err(|e| e.to_string())
            });
        let mut imported = match result {
            Ok(i) => i,
            Err(why) => {
                report.push(ReportEntry::new(
                    crate::import::Code::MALFORMED_ITEM,
                    Severity::Error,
                    format!("{path}: not converted: {why}"),
                ));
                refused.push((path.clone(), why));
                continue;
            }
        };
        for e in imported.entries.drain(..) {
            if report.entries.len() < 1_000 {
                report.push(e);
            }
        }
        let stamp = |meta: &mut ew_schema::ItemMeta| {
            meta.licence = Some(LICENCE.to_owned());
            meta.foreign_terms = true;
            if let Some(s) = &licence_source
                && !meta.sources.contains(s)
            {
                meta.sources.push(s.clone());
            }
        };
        stamp(&mut imported.symbol.item.meta);
        if !seen.insert(imported.symbol.id) {
            report.push(ReportEntry::new(
                crate::import::Code::DROPPED_BY_DESIGN,
                Severity::Info,
                format!("{path}: the same element as an earlier file: kept once"),
            ));
            continue;
        }
        symbols += 1;
        if let Some(mut f) = imported.fndef.take() {
            stamp(&mut f.item.meta);
            documents.push(LibDocument::Fndef(f));
        }
        documents.push(LibDocument::Symbol(imported.symbol));
    }
    report
        .stats
        .per_kind
        .insert("symbol".into(), u32::try_from(symbols).unwrap_or(u32::MAX));
    Converted {
        documents,
        symbols,
        refused,
        report,
    }
}

/// Converts `collection` and writes it into the writable library `library` (a user library; a front-end operation like
/// every library write). A second run of the same collection writes nothing.
///
/// # Errors
///
/// [`CatalogError::Library`] for a library that is not installed, read-only, locked or stale-locked, or a write that
/// fails.
pub fn import_into_library(
    reg: &mut library::Registry,
    library: Uuid,
    collection: &Collection,
    take_over: bool,
) -> Result<(Converted, Option<library::WriteReport>), CatalogError> {
    let info = reg
        .libraries()
        .into_iter()
        .find(|l| l.id == library)
        .ok_or(CatalogError::Library(library::LibraryError::NotFound))?;
    if !info.writable {
        return Err(CatalogError::Library(library::LibraryError::ReadOnly));
    }
    let converted = convert(library, collection);
    if converted.documents.is_empty() {
        return Ok((converted, None));
    }
    if take_over {
        reg.take_over_stale(library).map_err(CatalogError::Library)?;
    }
    let write = reg
        .write(library, &converted.documents)
        .map_err(CatalogError::Library)?;
    Ok((converted, Some(write)))
}
