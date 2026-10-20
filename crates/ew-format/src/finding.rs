// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integrity findings of the format layer (doc 05 §6.2, §6.3; the EW-CORE rules of doc 15 §4.2).

use std::fmt;

use crate::path::FilePath;

/// The severity of a finding.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Severity {
    /// Information; nothing blocks.
    Info,
    /// A warning.
    Warning,
    /// An error; it blocks release (an **R** rule of doc 05 §6.3).
    Error,
}

/// What a finding is about. The variants with a catalogue ID map to the EW-CORE rules of doc 05 §6.3
/// (and EW-CORE-019 for the reader limits of §3.6); the others are the classifications of doc 05 §6.2,
/// which have no rule ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FindingKind {
    /// EW-CORE-001: manifest missing, unparseable or with conflict markers (warning).
    ManifestUnreadable,
    /// EW-CORE-002: a mixed generation that cannot roll forward (error).
    MixedGeneration,
    /// EW-CORE-003: a listed file is missing, or is not a regular file (links are not followed,
    /// NFR-SEC-16) (error; read-only).
    MissingFile,
    /// EW-CORE-004: an orphan or sync-conflict copy (warning).
    Orphan,
    /// EW-CORE-005: a JSON syntax error at file:line (error; read-only).
    JsonSyntax,
    /// EW-CORE-006: git conflict markers in a JSONL file (error; read-only until conflict records are
    /// loaded, FR-COL-025).
    ConflictMarkers,
    /// EW-CORE-007: a schema violation; the line is quarantined verbatim (error).
    SchemaViolation,
    /// EW-CORE-009: a duplicate `($k, id)` within or across files; the first occurrence is kept, the others
    /// are quarantined verbatim (error).
    DuplicateRecord,
    /// EW-CORE-010: a record in the wrong file; moved at the next save (info).
    WrongFile,
    /// EW-CORE-014: a library copy whose content does not match its `sha256` (warning).
    LibraryHash,
    /// EW-CORE-015: an asset not matching its hash name (error).
    AssetMismatch,
    /// EW-CORE-016: unknown kinds, members, variants or files of a newer format (info).
    NewerData,
    /// EW-CORE-017: a malformed value of a known variant, kept verbatim (error).
    MalformedVariant,
    /// Doc 05 §6.2: the hash matches after CRLF → LF; rewritten at the next save (info).
    LineEndings,
    /// Doc 05 §6.2: a listed file differs from its manifest hash: changed outside ElektraWatt (info).
    ForeignEdit,
    /// Doc 05 §3.1: a file that is valid but not in canonical form; rewritten at the next save (info,
    /// `ew fmt --check`).
    NonCanonical,
    /// EW-CORE-019 (doc 05 §3.6): a hostile-input limit was exceeded; the file (or line) is not read
    /// (error; read-only). The next free number of the integrity block of doc 15 §4.2, taken in review
    /// cycle 8 for the design pass to record.
    LimitExceeded,
    /// More findings than [`Limits::max_findings`](crate::Limits::max_findings); the rest are counted only
    /// (info).
    Truncated,
}

impl FindingKind {
    /// The catalogue ID (`EW-CORE-005`), where one is assigned.
    pub fn code(self) -> Option<&'static str> {
        Some(match self {
            FindingKind::ManifestUnreadable => "EW-CORE-001",
            FindingKind::MixedGeneration => "EW-CORE-002",
            FindingKind::MissingFile => "EW-CORE-003",
            FindingKind::Orphan => "EW-CORE-004",
            FindingKind::JsonSyntax => "EW-CORE-005",
            FindingKind::ConflictMarkers => "EW-CORE-006",
            FindingKind::SchemaViolation => "EW-CORE-007",
            FindingKind::DuplicateRecord => "EW-CORE-009",
            FindingKind::WrongFile => "EW-CORE-010",
            FindingKind::LibraryHash => "EW-CORE-014",
            FindingKind::AssetMismatch => "EW-CORE-015",
            FindingKind::NewerData => "EW-CORE-016",
            FindingKind::MalformedVariant => "EW-CORE-017",
            FindingKind::LimitExceeded => "EW-CORE-019",
            FindingKind::LineEndings
            | FindingKind::ForeignEdit
            | FindingKind::NonCanonical
            | FindingKind::Truncated => return None,
        })
    }

    /// The severity of doc 05 §6.3.
    pub fn severity(self) -> Severity {
        match self {
            FindingKind::ManifestUnreadable | FindingKind::Orphan | FindingKind::LibraryHash => {
                Severity::Warning
            }
            FindingKind::MixedGeneration
            | FindingKind::MissingFile
            | FindingKind::JsonSyntax
            | FindingKind::ConflictMarkers
            | FindingKind::SchemaViolation
            | FindingKind::DuplicateRecord
            | FindingKind::AssetMismatch
            | FindingKind::MalformedVariant
            | FindingKind::LimitExceeded => Severity::Error,
            FindingKind::WrongFile
            | FindingKind::NewerData
            | FindingKind::LineEndings
            | FindingKind::ForeignEdit
            | FindingKind::NonCanonical
            | FindingKind::Truncated => Severity::Info,
        }
    }

    /// Whether the finding forces a read-only session: a save could otherwise drop or overwrite data the
    /// reader did not see (doc 05 §6.2 EW-CORE-003, §6.3 EW-CORE-005; EW-CORE-002 and -006 and the limits
    /// for the same reason).
    pub fn forces_read_only(self) -> bool {
        matches!(
            self,
            FindingKind::MixedGeneration
                | FindingKind::MissingFile
                | FindingKind::JsonSyntax
                | FindingKind::ConflictMarkers
                | FindingKind::LimitExceeded
        )
    }
}

/// One finding of the format reader.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Finding {
    /// What it is about.
    pub kind: FindingKind,
    /// The file, if the finding concerns one.
    pub file: Option<FilePath>,
    /// The 1-based line, if the finding concerns one.
    pub line: Option<u32>,
    /// What happened, in English.
    pub message: String,
}

impl Finding {
    /// A finding.
    pub fn new(
        kind: FindingKind,
        file: Option<FilePath>,
        line: Option<u32>,
        message: impl Into<String>,
    ) -> Finding {
        Finding {
            kind,
            file,
            line,
            message: message.into(),
        }
    }

    /// The catalogue ID, where one is assigned.
    pub fn code(&self) -> Option<&'static str> {
        self.kind.code()
    }

    /// The severity.
    pub fn severity(&self) -> Severity {
        self.kind.severity()
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = self.code() {
            write!(f, "{code} ")?;
        }
        match (&self.file, self.line) {
            (Some(file), Some(line)) => write!(f, "{file}:{line}: ")?,
            (Some(file), None) => write!(f, "{file}: ")?,
            _ => {}
        }
        f.write_str(&self.message)
    }
}

/// A capped list of findings: at most `cap` are kept, the rest are counted, and one
/// [`FindingKind::Truncated`] finding says how many were dropped ([`Findings::into_vec`]).
#[derive(Clone, Debug)]
pub struct Findings {
    items: Vec<Finding>,
    cap: usize,
    dropped: usize,
    read_only: bool,
    worst: Option<Severity>,
}

impl Findings {
    /// An empty list keeping at most `cap` findings.
    pub fn new(cap: usize) -> Findings {
        Findings {
            items: Vec::new(),
            cap,
            dropped: 0,
            read_only: false,
            worst: None,
        }
    }

    /// Adds a finding, or counts it beyond the cap.
    pub fn push(&mut self, finding: Finding) {
        self.read_only |= finding.kind.forces_read_only();
        self.worst = self.worst.max(Some(finding.severity()));
        if self.items.len() < self.cap {
            self.items.push(finding);
        } else {
            self.dropped += 1;
        }
    }

    /// Whether any finding pushed (kept or dropped) forces a read-only session.
    pub fn forces_read_only(&self) -> bool {
        self.read_only
    }

    /// The highest severity pushed (kept or dropped).
    pub fn worst(&self) -> Option<Severity> {
        self.worst
    }

    /// Appends every finding of `other`, keeping this list's cap.
    pub fn extend(&mut self, other: impl IntoIterator<Item = Finding>) {
        other.into_iter().for_each(|f| self.push(f));
    }

    /// The kept findings.
    pub fn as_slice(&self) -> &[Finding] {
        &self.items
    }

    /// The findings, sorted, plus one [`FindingKind::Truncated`] finding when some were dropped.
    pub fn into_vec(mut self) -> Vec<Finding> {
        self.items.sort();
        if self.dropped > 0 {
            self.items.push(Finding::new(
                FindingKind::Truncated,
                None,
                None,
                format!("{} more findings not listed", self.dropped),
            ));
        }
        self.items
    }
}
