// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Import front door for the CLI (`ew import qet`, doc 14 §2.1, §2.5): reads the starter content, runs the
//! QElectroTech project importer and opens the batch as a [`Session`], so that front-ends see the import,
//! its migration report and the derived pages without naming the record types.
//!
//! This is the spike form of `ApplyImport` (doc 14 §3.1): the batch becomes the whole model through
//! [`Session::from_records`] (the model's structural check, no lenient validators, no import passes).

use std::path::Path;

use ew_import_qet::QetProjectImporter;
use ew_import_qet::ew_interop_api::{ImportInput, Importer};
use ew_schema::{LibDocument, Record};

use crate::{Session, SessionOptions};

pub use ew_import_qet::ew_interop_api::{
    Code, ImportBatch, ImportError, ImportReport, ReportEntry, Severity, SourceLocation,
};
pub use ew_import_qet::migration;

/// The built-in starter documents (symbols, FnDefs, forms) an import may reference.
#[derive(Clone, Debug, Default)]
pub struct StarterLibrary {
    docs: Vec<LibDocument>,
}

impl StarterLibrary {
    /// No documents.
    pub fn empty() -> StarterLibrary {
        StarterLibrary::default()
    }

    /// The documents of a starter content directory: every `*.json` of `fndefs`, `forms` and
    /// `symbols/qet10` below `dir`, in file-name order. Missing sub-directories are skipped.
    ///
    /// # Errors
    ///
    /// An I/O error, or `InvalidData` for a file that is not a library document.
    pub fn from_dir(dir: &Path) -> std::io::Result<StarterLibrary> {
        let mut docs = Vec::new();
        for sub in ["fndefs", "forms", "symbols/qet10"] {
            let d = dir.join(sub);
            if !d.is_dir() {
                continue;
            }
            let mut paths: Vec<_> = std::fs::read_dir(&d)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            paths.sort();
            for p in paths {
                let text = std::fs::read_to_string(&p)?;
                let doc = LibDocument::from_json(&text).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: {e}", p.display()))
                })?;
                docs.push(doc);
            }
        }
        Ok(StarterLibrary { docs })
    }

    /// The number of documents.
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    /// Whether there is no document.
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }
}

/// Record counts of an import batch.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ImportCounts {
    /// Pages.
    pub pages: usize,
    /// Devices (strips and terminals included).
    pub devices: usize,
    /// Terminal strips (devices with the strip aspect).
    pub strips: usize,
    /// Terminals (devices with the terminal aspect).
    pub terminals: usize,
    /// Functions (interruption points included).
    pub functions: usize,
    /// Representations.
    pub representations: usize,
    /// Connections.
    pub connections: usize,
    /// Junctions.
    pub junctions: usize,
    /// Library documents.
    pub library: usize,
}

/// The result of one import: the batch and the session over it.
#[derive(Debug)]
pub struct Imported {
    /// The batch, with its report.
    pub batch: ImportBatch,
    /// The session over the batch, or why the model's structural check refused it.
    pub session: Result<Session, String>,
}

impl Imported {
    /// The record counts of the batch.
    pub fn counts(&self) -> ImportCounts {
        let mut c = ImportCounts {
            library: self.batch.lib.len(),
            ..ImportCounts::default()
        };
        for r in &self.batch.records {
            match r {
                Record::Page(_) => c.pages += 1,
                Record::Device(d) => {
                    c.devices += 1;
                    c.strips += usize::from(d.aspects.strip.is_some());
                    c.terminals += usize::from(d.aspects.terminal.is_some());
                }
                Record::Function(_) => c.functions += 1,
                Record::Representation(_) => c.representations += 1,
                Record::Connection(_) => c.connections += 1,
                Record::Junction(_) => c.junctions += 1,
                _ => {}
            }
        }
        c
    }

    /// The migration report of the import (findings grouped by code, doc 14 §2.5).
    pub fn migration_report(&self) -> migration::MigrationReport {
        migration::MigrationReport::from_report(&self.batch.report)
    }
}

/// Opens `batch` as a session: its project record becomes the project, every other record and document the
/// model.
///
/// # Errors
///
/// The text of the model error when the batch has no project record or fails the structural check.
pub fn session_from_batch(batch: &ImportBatch, options: SessionOptions) -> Result<Session, String> {
    let mut project = None;
    let mut records = Vec::with_capacity(batch.records.len());
    for r in &batch.records {
        match r {
            Record::Project(p) if project.is_none() => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let project = project.ok_or_else(|| "the batch holds no project record".to_owned())?;
    Session::from_records(project, records, batch.lib.clone(), options).map_err(|e| e.to_string())
}

/// Imports the QElectroTech project `bytes` named `name` (a file name, never a path) into a new project,
/// with the built-in documents of `library`, and opens it as a session.
///
/// # Errors
///
/// [`ImportError`] when the file as a whole cannot be read (a limit, malformed XML, not a project, a name
/// that is a path). Item-level problems are report entries.
pub fn import_qet(name: &str, bytes: Vec<u8>, library: &StarterLibrary) -> Result<Imported, ImportError> {
    let input = ImportInput::new(name, bytes).map_err(|e| ImportError::Unsupported {
        message: e.to_string(),
    })?;
    let importer = QetProjectImporter::with_library(library.docs.iter().cloned());
    let mut ctx = ew_import_qet::project::new_project_context();
    let batch = importer.import(&input, &mut ctx)?;
    let session = session_from_batch(&batch, SessionOptions::default());
    Ok(Imported { batch, session })
}
