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

// -------------------------------------------------------------------------------------------------------------------
// The front door (WP-S3.09 D3, D4)

pub use ew_import_qet::ew_interop_api::{Limits, LineageInfo};

/// Why a batch breaks the importer contract (WP-S3.09 D3, T2's reasons).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContractBreach {
    /// Two records share `(kind, id)`.
    SharedIdentity,
    /// A record's ID is not `v5(namespace, source_ref.key)`.
    IdentityNotFromKey,
    /// A record's `source_ref` names another lineage than the batch's.
    OtherLineage,
    /// The records are not sorted by kind rank, then ID.
    OutOfOrder,
}

impl ContractBreach {
    /// The reason in T2's words.
    pub fn reason(self) -> &'static str {
        match self {
            ContractBreach::SharedIdentity => "two records share one identity",
            ContractBreach::IdentityNotFromKey => "a record's identity does not follow from its source key",
            ContractBreach::OtherLineage => "a record names another import",
            ContractBreach::OutOfOrder => "the records are out of order",
        }
    }

    /// The text key of the reason (the shell's words for it).
    pub fn key(self) -> &'static str {
        match self {
            ContractBreach::SharedIdentity => "reason-import-shared-identity",
            ContractBreach::IdentityNotFromKey => "reason-import-identity-key",
            ContractBreach::OtherLineage => "reason-import-other-lineage",
            ContractBreach::OutOfOrder => "reason-import-order",
        }
    }
}

/// Why the front door imported nothing (WP-S3.09 D4): the model is untouched in every case.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The importer, or a limit checked before it ran, refused the input (the importer's English text).
    Import(ImportError),
    /// The importer panicked while reading `file` (T1).
    Panicked {
        /// The source's file name.
        file: String,
    },
    /// The batch breaks the importer contract (T2).
    Contract {
        /// The source's file name.
        file: String,
        /// Which rule it breaks.
        breach: ContractBreach,
    },
}

impl From<ImportError> for Refusal {
    fn from(e: ImportError) -> Refusal {
        Refusal::Import(e)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Import(e) => write!(f, "{e}"),
            Refusal::Panicked { file } => write!(
                f,
                "The importer stopped on an internal error while reading {file}; nothing was imported."
            ),
            Refusal::Contract { file, breach } => write!(
                f,
                "The import of {file} was refused because {}; nothing was imported.",
                breach.reason()
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// What an importer returns through the front door: a batch is checked against the contract (D3), other results
/// (library documents of the parts imports) are not batches of project records.
pub trait FrontDoorOutput {
    /// The contract check of the result, against the lineage the import ran in.
    ///
    /// # Errors
    ///
    /// The first [`ContractBreach`].
    fn contract(&self, _lineage: Option<&LineageInfo>) -> Result<(), ContractBreach> {
        Ok(())
    }
}

impl FrontDoorOutput for ImportBatch {
    fn contract(&self, lineage: Option<&LineageInfo>) -> Result<(), ContractBreach> {
        check_contract(self, lineage)
    }
}

impl FrontDoorOutput for ew_import_catalog::PartsImport {}

impl FrontDoorOutput for ew_import_tables::PartsTables {}

/// The one front door of every importer call (WP-S3.09 D4): refuses inputs above `limits.max_input_bytes`, a total
/// above `limits.max_expanded_bytes` and more than `limits.max_input_files` inputs before `run` is called (`sizes` are
/// the inputs' byte counts); runs `run` inside `catch_unwind`, a panic becoming [`Refusal::Panicked`]; and checks a
/// batch against the contract ([`check_contract`], against `lineage` when the batch emits none). `file` names the
/// source in the refusal.
///
/// # Errors
///
/// [`Refusal`]: nothing was imported.
pub fn front_door<T: FrontDoorOutput>(
    file: &str,
    sizes: &[u64],
    limits: &Limits,
    lineage: Option<&LineageInfo>,
    run: impl FnOnce() -> Result<T, ImportError>,
) -> Result<T, Refusal> {
    let limit = |what: &str, limit: u64| Refusal::Import(ImportError::LimitExceeded {
        what: what.to_owned(),
        limit,
    });
    if sizes.len() as u64 > u64::from(limits.max_input_files) {
        return Err(limit("input file count", u64::from(limits.max_input_files)));
    }
    if sizes.iter().any(|&s| s > limits.max_input_bytes) {
        return Err(limit("input size", limits.max_input_bytes));
    }
    if sizes.iter().fold(0u64, |a, &s| a.saturating_add(s)) > limits.max_expanded_bytes {
        return Err(limit("expanded input size", limits.max_expanded_bytes));
    }
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        .map_err(|_| Refusal::Panicked { file: file.to_owned() })??;
    out.contract(lineage).map_err(|breach| Refusal::Contract {
        file: file.to_owned(),
        breach,
    })?;
    Ok(out)
}

/// The importer contract of a batch (WP-S3.09 D3): no two records share `(kind, id)`; every record with a `source_ref`
/// names the batch's lineage (its `import_lineage` record, else `lineage`); a record's ID is
/// `v5(namespace, source_ref.key)`, except the project record, channels and template functions (`v5(device, index)`,
/// ADR-0004 amendment 2); and the records are sorted by kind rank, then ID. It never repairs a batch.
///
/// # Errors
///
/// The first [`ContractBreach`], checked in the order above per record.
pub fn check_contract(batch: &ImportBatch, lineage: Option<&LineageInfo>) -> Result<(), ContractBreach> {
    use ew_derive::ew_ids::v5;
    let own = batch.records.iter().find_map(|r| match r {
        Record::ImportLineage(l) => Some((l.id.as_uuid(), l.namespace.as_uuid())),
        _ => None,
    });
    let lineage = own.or_else(|| lineage.map(|l| (l.id.as_uuid(), l.namespace)));
    let mut seen = std::collections::BTreeSet::new();
    let mut last: Option<(u8, ew_derive::ew_ids::Uuid)> = None;
    for record in &batch.records {
        let (Some(kind), Some(id)) = (record.kind(), record.id()) else {
            continue;
        };
        if !seen.insert((kind, id)) {
            return Err(ContractBreach::SharedIdentity);
        }
        let key = (kind.rank().unwrap_or(u8::MAX), id);
        if last.is_some_and(|l| l > key) {
            return Err(ContractBreach::OutOfOrder);
        }
        last = Some(key);
        let Some((lineage_id, namespace)) = lineage else {
            continue;
        };
        let value = serde_json::to_value(record).unwrap_or_default();
        let Some(source) = value.get("source_ref") else {
            continue;
        };
        let named = source.get("lineage").and_then(serde_json::Value::as_str);
        if named != Some(lineage_id.to_string().as_str()) {
            return Err(ContractBreach::OtherLineage);
        }
        let source_key = source.get("key").and_then(serde_json::Value::as_str).unwrap_or_default();
        let derived: ew_derive::ew_ids::Id<()> = v5::imported_id(namespace, source_key);
        let exempt = || match record {
            Record::Project(_) | Record::Channel(_) => true,
            Record::Function(f) => f.device.is_some_and(|d| {
                (0..1024u32).any(|i| {
                    let t: ew_derive::ew_ids::Id<()> = v5::template_function_id(d, i);
                    t.as_uuid() == id
                })
            }),
            _ => false,
        };
        if derived.as_uuid() != id && !exempt() {
            return Err(ContractBreach::IdentityNotFromKey);
        }
    }
    Ok(())
}

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
    /// Lenient check findings on the imported model (doc 14 §2.5 "lenient check findings"): the open
    /// findings of the `migration` scheme, grouped by rule ([`lenient_findings`]).
    pub live: Vec<migration::LiveGroup>,
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
        migration::MigrationReport::from_report(&self.batch.report).with_live(self.live.clone())
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
/// [`Refusal`] when the file as a whole cannot be read (a limit, malformed XML, not a project, a name that is a path),
/// the importer panicked or its batch breaks the contract ([`front_door`]). Item-level problems are report entries.
pub fn import_qet(name: &str, bytes: Vec<u8>, library: &StarterLibrary) -> Result<Imported, Refusal> {
    let input = ImportInput::new(name, bytes).map_err(|e| ImportError::Unsupported {
        message: e.to_string(),
    })?;
    let importer = QetProjectImporter::with_library(library.docs.iter().cloned());
    let mut ctx = ew_import_qet::project::new_project_context();
    let limits = ctx.limits;
    let sizes = [input.bytes().len() as u64];
    let batch = front_door(name, &sizes, &limits, None, || importer.import(&input, &mut ctx))?;
    let session = session_from_batch(&batch, SessionOptions::default());
    let live = session.as_ref().map(lenient_findings).unwrap_or_default();
    Ok(Imported { batch, session, live })
}

/// The lenient check findings of an imported model (doc 15 §10.2, doc 14 §2.5): the `migration` scheme of
/// `ew-checks` (every rule of `pre-delivery` at its release severity, reporting prevent violations as
/// findings) run over the session, its open findings grouped by rule in rule order, one message per finding.
/// This replaces the hand-written duplicate checks of cycle 11 (decision log ID-276); the import run's own
/// findings stay in the importer's report.
pub fn lenient_findings(session: &Session) -> Vec<migration::LiveGroup> {
    use std::collections::BTreeMap;
    let report = session.check(&crate::checks::MIGRATION, crate::checks::CheckMode::Release);
    let mut groups: BTreeMap<&'static str, migration::LiveGroup> = BTreeMap::new();
    for f in report.findings.iter().filter(|f| f.is_open()) {
        let title = crate::checks::rule(f.rule).map_or(f.rule, |r| r.meta().title);
        let g = groups.entry(f.rule).or_insert_with(|| migration::LiveGroup {
            rule: f.rule.to_owned(),
            title: title.to_owned(),
            count: 0,
            messages: Vec::new(),
        });
        g.count = g.count.saturating_add(1);
        g.messages.push(format!("{}: {}", f.subject, f.message));
    }
    groups.into_values().collect()
}

// -------------------------------------------------------------------------------------------------------------------
// Parts tables (FT-410)

/// The `(created, skipped)` counts of a `parts.import` outcome note.
pub use ew_commands::commands::import_counts as parts_import_counts;
pub use ew_import_tables::SiblingTables;

/// The command a parts table import dispatches.
pub const PARTS_IMPORT: &str = "parts.import";

/// Part copies read from a parts table, ready for `parts.import` (FT-410).
#[derive(Clone, PartialEq, Debug)]
pub struct PartsImport {
    /// The base name of the chosen file (the copies' provenance).
    pub source: String,
    /// The sibling tables that were read with it, by file name, in [`SiblingTables::FILE_NAMES`] order.
    pub siblings: Vec<String>,
    /// The project-local part copies in the `lib/` JSON form, in `parts.csv` row order.
    pub copies: Vec<serde_json::Value>,
    /// The project-local manufacturer copies the parts name (WP-S1.19 D10), in the `lib/` JSON form, in name order.
    pub manufacturers: Vec<serde_json::Value>,
}

impl PartsImport {
    /// The parameters of `parts.import`: `{"copies": [...]}`, the part copies, then the manufacturer copies.
    pub fn params(&self) -> serde_json::Value {
        let copies: Vec<&serde_json::Value> = self.copies.iter().chain(&self.manufacturers).collect();
        serde_json::json!({ "copies": copies })
    }
}

/// Why a parts table could not be read.
#[derive(Debug)]
pub enum PartsReadError {
    /// A file could not be read (its base name, the error).
    Io(String, std::io::Error),
    /// The tables were refused (the first problem, with its pointer).
    Import(ImportError),
    /// The front door refused the import: the importer panicked (WP-S3.09 D4).
    Refused(Refusal),
}

impl From<Refusal> for PartsReadError {
    fn from(r: Refusal) -> PartsReadError {
        match r {
            Refusal::Import(e) => PartsReadError::Import(e),
            other => PartsReadError::Refused(other),
        }
    }
}

impl std::fmt::Display for PartsReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartsReadError::Io(name, e) => write!(f, "{name}: {e}"),
            PartsReadError::Import(e) => write!(f, "{e}"),
            PartsReadError::Refused(r) => write!(f, "{r}"),
        }
    }
}

impl std::error::Error for PartsReadError {}

/// The starter FnDefs (compiled in, `ew_library::starter`) as the built-in library that part templates resolve in.
fn starter_fndefs() -> &'static ew_import_tables::BuiltinLibrary {
    static LIB: std::sync::OnceLock<ew_import_tables::BuiltinLibrary> = std::sync::OnceLock::new();
    LIB.get_or_init(|| {
        let docs = ew_library::starter::items()
            .iter()
            .filter(|i| i.kind == ew_library::starter::StarterKind::Fndef)
            .filter_map(|i| LibDocument::from_json(i.json).ok());
        ew_import_tables::BuiltinLibrary::new(docs).unwrap_or_default()
    })
}

/// Parses a `parts.csv` of the golden dialect (text) and its sibling tables into project-local part copies of the
/// project `project` (`ew_import_tables::parse_parts_tables` with the starter FnDefs). `source` is the chosen file's
/// name; only its base name is kept.
///
/// A table that a spreadsheet saved with a byte-order mark (one leading U+FEFF) is read without it (FT-464): the
/// user's path of `parts.import` and `ew import parts` accepts it, while the golden bootstrap, which calls
/// `ew_import_tables` directly, stays strict.
///
/// # Errors
///
/// The first problem of any table, with its pointer (strict), or the front door's refusal ([`front_door`]).
pub fn parse_parts_tables(
    parts_csv: &str,
    siblings: &SiblingTables<'_>,
    project: ew_derive::ew_ids::Uuid,
    source: &str,
) -> Result<PartsImport, Refusal> {
    fn unmarked(t: &str) -> &str {
        t.strip_prefix('\u{feff}').unwrap_or(t)
    }
    let siblings = &SiblingTables {
        part_cps: siblings.part_cps.map(unmarked),
        part_templates: siblings.part_templates.map(unmarked),
        cable_cores: siblings.cable_cores.map(unmarked),
        part_plc: siblings.part_plc.map(unmarked),
    };
    let sizes: Vec<u64> = [Some(parts_csv), siblings.part_cps, siblings.part_templates, siblings.cable_cores, siblings.part_plc]
        .into_iter()
        .flatten()
        .map(|t| t.len() as u64)
        .collect();
    let file = source.rsplit(['/', '\\']).next().unwrap_or(source);
    let copies = front_door(file, &sizes, &Limits::default(), None, || {
        ew_import_tables::parse_parts_tables(
            unmarked(parts_csv),
            siblings,
            ew_derive::ew_ids::Id::from_uuid(project),
            source,
            starter_fndefs(),
        )
    })?;
    let source = source.rsplit(['/', '\\']).next().unwrap_or(source).to_owned();
    let siblings = SiblingTables::FILE_NAMES
        .iter()
        .zip([
            siblings.part_cps,
            siblings.part_templates,
            siblings.cable_cores,
            siblings.part_plc,
        ])
        .filter(|(_, t)| t.is_some())
        .map(|(n, _)| (*n).to_owned())
        .collect();
    let manufacturers = copies
        .manufacturers
        .into_iter()
        .map(|c| serde_json::to_value(LibDocument::Manufacturer(c)).unwrap_or_default())
        .collect();
    let copies = copies
        .parts
        .into_iter()
        .map(|c| serde_json::to_value(LibDocument::Part(c)).unwrap_or_default())
        .collect();
    Ok(PartsImport {
        source,
        siblings,
        copies,
        manufacturers,
    })
}

/// Reads the parts table `path` and the sibling tables that lie in its folder (`part_cps.csv`, `part_templates.csv`,
/// `cable_cores.csv`, `part_plc.csv`; an absent table is skipped, so the import is parts-only without them) and
/// parses them for project `project` ([`parse_parts_tables`]). The host's file read: the CLI and the shell call it,
/// then dispatch `parts.import` with [`PartsImport::params`].
///
/// # Errors
///
/// [`PartsReadError::Io`] for a file that cannot be read (named by its base name), [`PartsReadError::Import`] for
/// refused tables.
pub fn read_parts_tables(
    path: &Path,
    project: ew_derive::ew_ids::Uuid,
) -> Result<PartsImport, PartsReadError> {
    let name = |p: &Path| {
        p.file_name()
            .map_or_else(|| "parts.csv".to_owned(), |n| n.to_string_lossy().into_owned())
    };
    let read = |p: &Path| {
        let limit = ew_import_tables::TableLimits::default().max_file_bytes;
        let len = std::fs::metadata(p)
            .map_err(|e| PartsReadError::Io(name(p), e))?
            .len();
        if len > limit {
            return Err(PartsReadError::Import(ImportError::LimitExceeded {
                what: format!("file size of {}", name(p)),
                limit,
            }));
        }
        std::fs::read_to_string(p).map_err(|e| PartsReadError::Io(name(p), e))
    };
    let parts = read(path)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut texts: Vec<(&'static str, String)> = Vec::new();
    for file in SiblingTables::FILE_NAMES {
        let p = dir.join(file);
        if p.is_file() {
            texts.push((file, read(&p)?));
        }
    }
    let mut siblings = SiblingTables::default();
    for (file, text) in &texts {
        siblings.set(file, text);
    }
    parse_parts_tables(&parts, &siblings, project, &name(path)).map_err(PartsReadError::from)
}

// -------------------------------------------------------------------------------------------------------------------
// Golden-dialect tables (`ew import csv`, FR-IMP-022, FT-27)

pub use ew_import_tables::LibraryError;
pub use ew_import_tables::csv::CsvError;
pub use ew_import_tables::ew_interop_api::ImportMode;

/// Why `ew import csv` imports nothing.
#[derive(Debug)]
pub enum TablesError {
    /// The tables directory or one of its files cannot be read.
    Read {
        /// The directory or file.
        path: std::path::PathBuf,
        /// The error.
        error: std::io::Error,
    },
    /// The directory holds more CSV files than the importer reads (`Limits::max_input_files`).
    TooManyFiles {
        /// The limit.
        limit: usize,
    },
    /// The CSV files are larger together than the bootstrap's input limit (64 MiB, doc 14 §11).
    TooLarge {
        /// The limit in bytes.
        limit: u64,
    },
    /// A table of the dialect that is not readable CSV, with the reader's error (its line, and its column where the
    /// reader names one).
    Unreadable {
        /// The table's file name.
        file: String,
        /// The reader's error.
        error: CsvError,
    },
    /// The starter content does not make a library the tables can reference.
    Library(LibraryError),
    /// The import as a whole is refused (a limit, no project table, a file name that is a path).
    Import(ImportError),
    /// The front door refused the import: the importer panicked or its batch breaks the contract (WP-S3.09 D4).
    Refused(Refusal),
}

impl std::fmt::Display for TablesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TablesError::Read { path, error } => write!(f, "{}: {error}", path.display()),
            TablesError::TooManyFiles { limit } => {
                write!(
                    f,
                    "the directory holds more than {limit} CSV files; nothing was imported"
                )
            }
            TablesError::TooLarge { limit } => write!(
                f,
                "the CSV files are larger than {limit} bytes together; nothing was imported"
            ),
            TablesError::Unreadable { file, error } => write!(f, "{file}: {error}; nothing was imported"),
            TablesError::Library(e) => write!(f, "starter content: {e}"),
            TablesError::Import(e) => write!(f, "{e}"),
            TablesError::Refused(r) => write!(f, "{r}"),
        }
    }
}

impl std::error::Error for TablesError {}

/// Reads every `*.csv` regular file of `dir` (no links, no sub-directories), in file-name order: at most
/// `Limits::bootstrap_tables().max_input_files` files, their sum within the bootstrap's input limit, both checked before a byte is read.
///
/// # Errors
///
/// [`TablesError`]: an unreadable directory or file, too many files, too many bytes, a file name the importer
/// refuses.
pub fn read_tables(dir: &Path) -> Result<Vec<ImportInput>, TablesError> {
    use std::io::Read as _;
    let read_err = |path: &Path| {
        let path = path.to_path_buf();
        move |error| TablesError::Read { path, error }
    };
    let limit = ew_import_tables::bootstrap_context().limits.max_input_bytes;
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(read_err(dir))? {
        let entry = entry.map_err(read_err(dir))?;
        let is_file = entry.file_type().map_err(read_err(&entry.path()))?.is_file();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_file || !name.ends_with(".csv") {
            continue;
        }
        let max_files = ew_import_tables::ew_interop_api::Limits::bootstrap_tables().max_input_files as usize;
        if files.len() == max_files {
            return Err(TablesError::TooManyFiles { limit: max_files });
        }
        files.push((name, entry.path()));
    }
    files.sort();
    let mut total: u64 = 0;
    let mut inputs = Vec::with_capacity(files.len());
    for (name, path) in files {
        let len = std::fs::metadata(&path).map_err(read_err(&path))?.len();
        if total.saturating_add(len) > limit {
            return Err(TablesError::TooLarge { limit });
        }
        // The file may grow after the size check: read at most what is left of the limit, plus one byte to see it.
        let left = limit - total;
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .and_then(|f| f.take(left.saturating_add(1)).read_to_end(&mut bytes))
            .map_err(read_err(&path))?;
        let read = bytes.len() as u64;
        if read > left {
            return Err(TablesError::TooLarge { limit });
        }
        total += read;
        let input = ImportInput::new(&name, bytes).map_err(|e| {
            TablesError::Import(ImportError::Unsupported {
                message: e.to_string(),
            })
        })?;
        inputs.push(input);
    }
    Ok(inputs)
}

/// Imports golden-dialect tables (`golden/J-E-2416/tables/`, doc 20) with the symbols, FnDefs and forms of `library`
/// in `mode` (strict, the bootstrap's default, drops a row with a foreign colour code with EW-IMP-002; lenient keeps
/// it with EW-IMP-008, decision log ID-375). A table of the dialect that is not readable CSV refuses the import, with
/// the reader's line (the importer itself would only drop the table with a finding).
///
/// # Errors
///
/// [`TablesError::Unreadable`] for an unreadable table, [`TablesError::Library`] for starter content the tables
/// cannot reference, [`TablesError::Import`] when the import as a whole is refused. Row problems are report entries.
pub fn import_tables(
    inputs: &[ImportInput],
    library: &StarterLibrary,
    mode: ImportMode,
) -> Result<ImportBatch, TablesError> {
    let limits = ew_import_tables::TableLimits::default();
    let known: std::collections::BTreeSet<String> = ew_import_tables::table_names().collect();
    for input in inputs.iter().filter(|i| known.contains(i.name())) {
        if let Err(error) = ew_import_tables::csv::parse(input.bytes(), &limits) {
            return Err(TablesError::Unreadable {
                file: input.name().to_owned(),
                error,
            });
        }
    }
    let builtin =
        ew_import_tables::BuiltinLibrary::new(library.docs.iter().cloned()).map_err(TablesError::Library)?;
    let mut ctx = ew_import_tables::bootstrap_context();
    ctx.mode = mode;
    let sizes: Vec<u64> = inputs.iter().map(|i| i.bytes().len() as u64).collect();
    let importer = ew_import_tables::TablesImporter::new(builtin);
    front_door("the tables", &sizes, &ctx.limits, ctx.lineage.as_ref(), || importer.import(inputs, &ctx)).map_err(
        |r| match r {
            Refusal::Import(e) => TablesError::Import(e),
            other => TablesError::Refused(other),
        },
    )
}

/// Why a new project was not written.
#[derive(Debug)]
pub enum CreateError {
    /// The target exists and is not an empty directory.
    NotEmpty(std::path::PathBuf),
    /// The batch does not pass the model's structural check.
    Model(String),
    /// Writing the project failed; nothing is left at the target.
    Write {
        /// The target directory.
        path: std::path::PathBuf,
        /// What failed.
        error: String,
    },
}

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreateError::NotEmpty(p) => write!(
                f,
                "{}: not an empty directory; the new project needs a new or empty directory",
                p.display()
            ),
            CreateError::Model(e) => write!(f, "the imported records do not make a project: {e}"),
            CreateError::Write { path, error } => {
                write!(f, "{}: {error}; nothing was written", path.display())
            }
        }
    }
}

impl std::error::Error for CreateError {}

/// Writes `batch` as a new project (generation 1) into `out`, which must not exist or be an empty directory: the
/// project is written into a new sibling directory and renamed to `out` once complete, so `out` never holds a part of
/// a project. The project is not journalled and its lock is released afterwards.
///
/// # Errors
///
/// [`CreateError`]: `out` holds something, the batch does not make a model, or the write fails (the sibling directory
/// is removed again).
pub fn create_project(
    batch: &ImportBatch,
    out: &Path,
    mut store: crate::StoreOptions,
) -> Result<(), CreateError> {
    let empty_dir = match std::fs::symlink_metadata(out) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Ok(meta) if meta.is_dir() => {
            let mut entries = std::fs::read_dir(out).map_err(|e| CreateError::Write {
                path: out.to_path_buf(),
                error: e.to_string(),
            })?;
            if entries.next().is_some() {
                return Err(CreateError::NotEmpty(out.to_path_buf()));
            }
            true
        }
        Ok(_) => return Err(CreateError::NotEmpty(out.to_path_buf())),
        Err(e) => {
            return Err(CreateError::Write {
                path: out.to_path_buf(),
                error: e.to_string(),
            });
        }
    };
    let Some(name) = out.file_name() else {
        return Err(CreateError::NotEmpty(out.to_path_buf()));
    };
    let mut session = session_from_batch(batch, SessionOptions::default()).map_err(CreateError::Model)?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(name);
    temp_name.push(format!(".import-{}", std::process::id()));
    let temp = parent.join(temp_name);
    let write_err = |error: String| CreateError::Write {
        path: out.to_path_buf(),
        error,
    };
    // A new directory of ours: `create_dir` fails when anything is at that name.
    std::fs::create_dir(&temp).map_err(|e| write_err(format!("{}: {e}", temp.display())))?;
    store.state_dir = None;
    let written = match session.create(&temp, store) {
        Ok(()) => {
            session.close();
            let moved = if empty_dir {
                std::fs::remove_dir(out).and_then(|()| std::fs::rename(&temp, out))
            } else {
                std::fs::rename(&temp, out)
            };
            moved.map_err(|e| write_err(e.to_string()))
        }
        Err(e) => {
            session.close();
            Err(write_err(e.to_string()))
        }
    };
    if written.is_err() {
        // Only the directory created above, which holds only what this call wrote.
        let _ = std::fs::remove_dir_all(&temp);
    } else {
        use ew_store::Vfs as _;
        let _ = ew_store::OsVfs.sync_dir(parent);
    }
    written
}
