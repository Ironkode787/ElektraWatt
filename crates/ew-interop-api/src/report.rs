// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The import report (doc 14 §2.4): entries with codes, severities, defect classes and source locations,
//! the source description and counts.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// An import finding code `EW-IMP-nnn` (doc 14 §2.5 catalogue), stored as its number `nnn` (1–999).
///
/// ```
/// use ew_interop_api::Code;
///
/// let c = Code::imp(119);
/// assert_eq!(c.to_string(), "EW-IMP-119");
/// assert_eq!("EW-IMP-006".parse::<Code>(), Ok(Code::imp(6)));
/// assert!("EW-IMP-1000".parse::<Code>().is_err());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Code(u16);

impl Code {
    /// Input limit exceeded; item or file refused.
    pub const LIMIT: Code = Code(1);
    /// Malformed item skipped (NaN, overflow, unparsable node).
    pub const MALFORMED_ITEM: Code = Code(2);
    /// Unresolvable reference dropped.
    pub const UNRESOLVED_REFERENCE: Code = Code(3);
    /// Content dropped by design.
    pub const DROPPED_BY_DESIGN: Code = Code(4);
    /// Font substituted.
    pub const FONT_SUBSTITUTED: Code = Code(6);
    /// Value kept as foreign or opaque text.
    pub const FOREIGN_VALUE: Code = Code(8);
    /// Imported text styles not bound to slot kinds.
    pub const STYLES_NOT_BOUND: Code = Code(114);
    /// Element without function semantics; generic black box.
    pub const BLACK_BOX: Code = Code(119);

    /// `EW-IMP-<n>`.
    ///
    /// # Panics
    ///
    /// If `n` is 0 or above 999 (a programming error: codes are constants of the catalogue).
    pub const fn imp(n: u16) -> Code {
        assert!(n >= 1 && n <= 999, "EW-IMP codes are 001 to 999");
        Code(n)
    }

    /// The number `nnn`.
    pub const fn number(self) -> u16 {
        self.0
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EW-IMP-{:03}", self.0)
    }
}

/// Why a string is not a [`Code`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CodeParseError;

impl fmt::Display for CodeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not an import code EW-IMP-nnn")
    }
}

impl std::error::Error for CodeParseError {}

impl FromStr for Code {
    type Err = CodeParseError;

    /// Accepts exactly `EW-IMP-` followed by three ASCII digits, not `000`.
    fn from_str(s: &str) -> Result<Code, CodeParseError> {
        let digits = s.strip_prefix("EW-IMP-").ok_or(CodeParseError)?;
        if digits.len() != 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(CodeParseError);
        }
        let n: u16 = digits.parse().map_err(|_| CodeParseError)?;
        if n == 0 {
            return Err(CodeParseError);
        }
        Ok(Code(n))
    }
}

/// Severity of a report entry (doc 14 §2.5 "Sev." column).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Severity {
    /// For information.
    Info,
    /// Needs review.
    Warning,
    /// Content was lost or refused.
    Error,
}

/// A defect class `D-nn` of the reference project's defect list (doc 14 §2.5 "Defect class" column).
///
/// ```
/// use ew_interop_api::DefectClass;
///
/// assert_eq!(DefectClass(3).to_string(), "D-03");
/// assert_eq!(DefectClass(29).to_string(), "D-29");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DefectClass(pub u8);

impl fmt::Display for DefectClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "D-{:02}", self.0)
    }
}

/// Where in the source a finding was made (doc 14 §2.4 `SourceLoc`). Every member is optional except the
/// file name; none is ever a filesystem path.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct SourceLocation {
    /// The file name.
    pub file: String,
    /// An archive member or an embedded definition's path (`embed://import/…`).
    pub member: Option<String>,
    /// The folio.
    pub folio: Option<String>,
    /// The frame cell, such as `103-A3`.
    pub cell: Option<String>,
    /// The item: a UUID, handle, part number or XML element name with its position.
    pub item: Option<String>,
    /// 1-based line.
    pub line: Option<u32>,
}

/// One finding of an import (doc 14 §2.4).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ReportEntry {
    /// The finding code.
    pub code: Code,
    /// The severity.
    pub severity: Severity,
    /// The defect class, if the finding belongs to one.
    pub defect: Option<DefectClass>,
    /// What happened, in English (Fluent keys and arguments arrive with the migration report UI).
    pub message: String,
    /// Where.
    pub location: Option<SourceLocation>,
}

impl ReportEntry {
    /// An entry without defect class and location.
    pub fn new(code: Code, severity: Severity, message: impl Into<String>) -> ReportEntry {
        ReportEntry {
            code,
            severity,
            defect: None,
            message: message.into(),
            location: None,
        }
    }

    /// This entry at `location`.
    pub fn at(self, location: SourceLocation) -> ReportEntry {
        ReportEntry {
            location: Some(location),
            ..self
        }
    }

    /// This entry with defect class `defect`.
    pub fn with_defect(self, defect: DefectClass) -> ReportEntry {
        ReportEntry {
            defect: Some(defect),
            ..self
        }
    }
}

/// What was imported (doc 14 §2.4 `SourceInfo`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct SourceInfo {
    /// The file name, never a path.
    pub name: String,
    /// SHA-256 of the file.
    pub sha256: [u8; 32],
    /// Size in bytes.
    pub size: u64,
    /// The detected format, such as `"qet-element"`.
    pub format: String,
    /// The detected format version, such as `"0.80"`.
    pub format_version: Option<String>,
    /// Free-form detail about the source that downstream steps need, such as `"link type master"`.
    pub detail: Option<String>,
}

/// Counts of an import (doc 14 §2.4 `ImportStats`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct ImportStats {
    /// Emitted records and library documents per `$k`.
    pub per_kind: BTreeMap<String, u32>,
    /// Items dropped (reported, not emitted).
    pub dropped: u32,
    /// Objects inferred.
    pub inferred: u32,
    /// Connections lost; FR-IMP-007 requires 0.
    pub lost_connections: u32,
    /// Findings not listed in `entries` because the importer's report cap was reached, per code (doc 14
    /// §11 "Report volume"); the report lists one EW-IMP-001 summary entry for them.
    pub suppressed: BTreeMap<Code, u32>,
}

/// The report of one import (doc 14 §2.4).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ImportReport {
    /// The importer ID.
    pub importer: String,
    /// The importer version.
    pub importer_version: String,
    /// The source.
    pub source: SourceInfo,
    /// The findings, in the order they were made.
    pub entries: Vec<ReportEntry>,
    /// Counts.
    pub stats: ImportStats,
}

impl ImportReport {
    /// An empty report of `importer` at `importer_version` about `source`.
    pub fn new(importer: &str, importer_version: &str, source: SourceInfo) -> ImportReport {
        ImportReport {
            importer: importer.to_owned(),
            importer_version: importer_version.to_owned(),
            source,
            entries: Vec::new(),
            stats: ImportStats::default(),
        }
    }

    /// Appends `entry`.
    pub fn push(&mut self, entry: ReportEntry) {
        self.entries.push(entry);
    }

    /// Whether any entry has [`Severity::Error`].
    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|e| e.severity == Severity::Error)
    }

    /// The entries with `code`, in order.
    pub fn with_code(&self, code: Code) -> impl Iterator<Item = &ReportEntry> + '_ {
        self.entries.iter().filter(move |e| e.code == code)
    }
}
