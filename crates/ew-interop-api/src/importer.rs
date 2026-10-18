// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Importer` trait, its context and its result.

use std::fmt;

use ew_ids::{Id, Uuid};
use ew_schema::{Date, ImportLineage, LibDocument, Record};

use crate::{ImportInput, ImportReport};

/// How sure an importer is that it can read a file (doc 14 §2.1 `sniff`): content first, the file name
/// extension last.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Detection {
    /// Not this format.
    No,
    /// Possibly this format, with a score 0–100 (higher is surer); used to rank importers.
    Maybe(u8),
    /// Certainly this format.
    Certain,
}

/// An existing import lineage to import into (doc 14 §2.3): its record ID and its namespace, which the host
/// reads from the target's `ImportLineage` record.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LineageInfo {
    /// The `ImportLineage` record ID.
    pub id: Id<ImportLineage>,
    /// `ImportLineage.namespace`, the namespace of every imported ID.
    pub namespace: Uuid,
}

/// Input limits beyond the file size (doc 14 §11, NFR-SEC-02, FR-IMP-005). A violation refuses the file
/// ([`ImportError::LimitExceeded`]) or skips the item with EW-IMP-001.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Largest accepted input, in bytes.
    pub max_input_bytes: u64,
    /// Deepest accepted XML element nesting.
    pub max_xml_depth: u32,
    /// Most XML nodes (elements, attributes, texts) in one document.
    pub max_xml_nodes: u32,
    /// Longest accepted XML attribute value, in bytes.
    pub max_attribute_bytes: usize,
}

impl Default for Limits {
    /// The values of doc 14 §11: XML depth ≤ 256, attribute value ≤ 16 MiB; inputs up to 512 MiB and
    /// 10 million XML nodes (the DXF file limit and a node count that parses in well under a second).
    fn default() -> Limits {
        Limits {
            max_input_bytes: 512 * 1024 * 1024,
            max_xml_depth: 256,
            max_xml_nodes: 10_000_000,
            max_attribute_bytes: 16 * 1024 * 1024,
        }
    }
}

/// What an import runs against (doc 14 §2.1 `ImportCx`, minimal form): plain data, never the model.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ImportContext {
    /// The lineage to import into; `None` creates a new one, whose `ImportLineage` record the importer
    /// emits in the batch with the importer's natural anchor (doc 14 §2.3).
    pub lineage: Option<LineageInfo>,
    /// The command's `as_of` date; importers never read the wall clock (doc 07 §3.6).
    pub as_of: Option<Date>,
    /// Input limits.
    pub limits: Limits,
}

/// The result of one import (doc 14 §2.1): format-level records with final IDs, embedded library
/// documents, and the report.
#[derive(Clone, PartialEq, Debug)]
pub struct ImportBatch {
    /// Records, sorted by kind rank, then ID.
    pub records: Vec<Record>,
    /// Library documents (symbols, function definitions, forms) for the project's import library, sorted
    /// by kind, then copy ID.
    pub lib: Vec<LibDocument>,
    /// The report.
    pub report: ImportReport,
}

/// Why an importer refused a whole file. Problems with single items are `ReportEntry`s
/// ([`crate::ReportEntry`]), never errors.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImportError {
    /// The input exceeds a limit (EW-IMP-001 for the file).
    LimitExceeded {
        /// What was exceeded, such as `"input size"` or `"XML depth"`.
        what: String,
        /// The limit.
        limit: u64,
    },
    /// The input is not well-formed in its format (malformed XML, wrong root element).
    Malformed {
        /// What is wrong.
        message: String,
        /// 1-based line of the problem, if known.
        line: Option<u32>,
    },
    /// The input is well-formed but of a kind or version this importer does not read.
    Unsupported {
        /// What is not supported.
        message: String,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::LimitExceeded { what, limit } => {
                write!(f, "input exceeds the {what} limit of {limit}")
            }
            ImportError::Malformed {
                message,
                line: Some(n),
            } => write!(f, "malformed input at line {n}: {message}"),
            ImportError::Malformed { message, line: None } => write!(f, "malformed input: {message}"),
            ImportError::Unsupported { message } => write!(f, "unsupported input: {message}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// An importer of one source format (doc 14 §2.1).
///
/// Implementations are pure: `import` reads only its input, never another file, the model, the clock or the
/// network, and is deterministic (NFR-SEC-18). Everything that cannot be mapped becomes a report entry;
/// only a file that cannot be read at all is an [`ImportError`].
pub trait Importer: Send + Sync {
    /// The importer's lower-case ID, also the `ImportLineage.importer`: `"qet"`, `"dxf"`, `"golden-csv"`.
    fn id(&self) -> &'static str;

    /// The importer's version, recorded in the report and the import run; bumped on every mapping change.
    fn version(&self) -> &'static str;

    /// Whether this importer reads a file starting with `head` (the first bytes, at most the whole file)
    /// and named `name` (a file name, never a path). Content decides first; the extension only breaks ties.
    fn detect(&self, head: &[u8], name: &str) -> Detection;

    /// Imports one file.
    ///
    /// # Errors
    ///
    /// [`ImportError`] when the file as a whole cannot be read; item-level problems are report entries.
    fn import(&self, input: &ImportInput, ctx: &mut ImportContext) -> Result<ImportBatch, ImportError>;
}
