// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The migration chain (doc 05 §10.2; ADR-0011 decision 3; WP-S1.17 D10, decision log ID-1221): pure steps on
//! `serde_json::Value`, run between a line's parse and its typed reading, only when a project of an older format opens
//! ([`Access::Migrate`](crate::Access::Migrate)).
//!
//! - **Kind steps** ([`KindStep`]) take a record or library document of one kind from `$v` n to n + 1; the chain sets
//!   `$v` itself after each step (omitted when 1).
//! - **Format steps** ([`FormatStep`]) go from one format to the next: one step on the manifest's `Value`, one on each
//!   record's and library document's.
//!
//! A value of a file of format F is migrated towards this reader's format ([`Migrations::towards`]) by the record step
//! of every format step from F on, in order, and then by the kind steps of its `$k` from its `$v` up to its kind's.
//! Steps are never deleted, and the shipped chain ([`MIGRATIONS`]) is empty while 0.1 is the first format; a test
//! passes its own chain. A step's error carries the JSON Pointer of the value at fault; the loader adds the file and
//! the line ([`MigrationError`]). A record whose `$v` is below its kind's with no step, and a format with no step
//! towards the reader's, are errors too: the open is refused, never a line quarantined.
//!
//! [`steward_problems`] is the steward's check of D12 (decision log ID-1221 ruling 8): a released list, the format a
//! build writes, its chain and its fixture corpus.

use std::fmt;
use std::path::Path;

use ew_schema::Kind;
use serde_json::Value;

use crate::limits::Limits;
use crate::manifest::Manifest;
use crate::path::{FilePath, MANIFEST};

/// One migration step: a value of the older form to the newer one, or the error at its JSON Pointer
/// ([`MigrationError::at`]).
pub type Step = fn(Value) -> Result<Value, MigrationError>;

/// A step of one kind from `$v` `from` to `from + 1`.
#[derive(Clone, Copy, Debug)]
pub struct KindStep {
    /// The kind (`$k`) of the records or library documents it takes.
    pub kind: Kind,
    /// The `$v` it takes (an absent `$v` is 1).
    pub from: u16,
    /// The step.
    pub step: Step,
}

/// A step from one format to the next.
#[derive(Clone, Copy, Debug)]
pub struct FormatStep {
    /// The format it takes (`"0.0"`).
    pub from: &'static str,
    /// The format it gives (`"0.1"`).
    pub to: &'static str,
    /// The step on the manifest's `Value`.
    pub manifest: Step,
    /// The step on each record's and library document's `Value`.
    pub record: Step,
}

/// A migration chain: every kind step and format step ever shipped (steps are never deleted, ADR-0011 decision 3).
#[derive(Clone, Copy, Debug)]
pub struct Migrations {
    /// The kind steps.
    pub kinds: &'static [KindStep],
    /// The format steps.
    pub formats: &'static [FormatStep],
}

/// The shipped chain: empty, since 0.1 is the first format and no file can need a step yet (WP-S1.17 D10).
pub static MIGRATIONS: Migrations = Migrations {
    kinds: &[],
    formats: &[],
};

/// Why a value or a project was not migrated: a step failed, or the chain has no step where one is needed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MigrationError {
    /// The file: `manifest.json` for the manifest and the format; `None` where no file is known (a fragment, or a
    /// step's own error before the loader names its file).
    pub file: Option<FilePath>,
    /// The 1-based line of a JSON Lines file.
    pub line: Option<u32>,
    /// The JSON Pointer of the value at fault within the line's record, the document or the manifest (`""`: the whole
    /// value).
    pub pointer: String,
    /// What is wrong.
    pub message: String,
}

impl MigrationError {
    /// A step's error at `pointer` (a JSON Pointer into the value it was given).
    pub fn at(pointer: impl Into<String>, message: impl Into<String>) -> MigrationError {
        MigrationError {
            file: None,
            line: None,
            pointer: pointer.into(),
            message: message.into(),
        }
    }

    /// The error with its file and line, where the step left them unset.
    #[must_use]
    pub fn in_file(mut self, file: &FilePath, line: Option<u32>) -> MigrationError {
        if self.file.is_none() {
            self.file = Some(file.clone());
            self.line = line;
        }
        self
    }
}

impl fmt::Display for MigrationError {
    /// `<file> line <n> at <pointer>: <message>`, each part present only when known.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut place = Vec::new();
        if let Some(file) = &self.file {
            place.push(file.to_string());
        }
        if let Some(line) = self.line {
            place.push(format!("line {line}"));
        }
        if !self.pointer.is_empty() {
            place.push(format!("at {}", self.pointer));
        }
        if place.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", place.join(" "), self.message)
        }
    }
}

impl std::error::Error for MigrationError {}

/// The way from one format to another through a chain ([`Migrations::towards`]).
#[derive(Clone, Debug)]
pub struct Migration<'a> {
    chain: &'a Migrations,
    from: String,
    steps: Vec<&'a FormatStep>,
}

impl Migrations {
    /// The way from format `from` to format `to`: the format steps from `from` on, each taking the format the one
    /// before gave, up to `to` (none when they are equal).
    ///
    /// # Errors
    ///
    /// [`MigrationError`] at `manifest.json`'s `/format` when the chain has no such way.
    pub fn towards(&self, from: &str, to: &str) -> Result<Migration<'_>, MigrationError> {
        let mut steps = Vec::new();
        let mut at = from;
        while at != to {
            // Each format is left once at most: a chain that loops never reaches `to`.
            let next = self
                .formats
                .iter()
                .find(|s| s.from == at)
                .filter(|_| steps.len() < self.formats.len());
            let Some(step) = next else {
                return Err(MigrationError {
                    file: FilePath::new(MANIFEST).ok(),
                    line: None,
                    pointer: "/format".to_owned(),
                    message: format!("this version has no migration step from format {from} towards {to}"),
                });
            };
            steps.push(step);
            at = step.to;
        }
        Ok(Migration {
            chain: self,
            from: from.to_owned(),
            steps,
        })
    }
}

impl Migration<'_> {
    /// The format migrated from.
    pub fn from(&self) -> &str {
        &self.from
    }

    /// A record or library document migrated (the module documentation): the record step of every format step in
    /// order, then the kind steps of its `$k` from its `$v` up to its kind's. A value without a known `$k`, or whose
    /// `$v` is not an integer, takes no kind step (its typed reading judges it).
    ///
    /// # Errors
    ///
    /// A step's [`MigrationError`]; one at `/$v` when a kind step is missing, and at `""` when a step does not give an
    /// object.
    pub fn record(&self, value: Value) -> Result<Value, MigrationError> {
        let mut value = value;
        for step in &self.steps {
            value = (step.record)(value)?;
        }
        let Some(kind) = value.get("$k").and_then(Value::as_str).and_then(Kind::from_name) else {
            return Ok(value);
        };
        let target = kind.schema_version();
        loop {
            let v = match value.get("$v") {
                None => 1,
                Some(v) => match v.as_u64().and_then(|v| u16::try_from(v).ok()) {
                    Some(v) => v,
                    None => return Ok(value),
                },
            };
            if v >= target {
                return Ok(value);
            }
            let Some(step) = self.chain.kinds.iter().find(|s| s.kind == kind && s.from == v) else {
                return Err(MigrationError::at(
                    "/$v",
                    format!(
                        "this version has no migration step for a {} at $v {v}",
                        kind.name()
                    ),
                ));
            };
            value = (step.step)(value)?;
            let Value::Object(members) = &mut value else {
                return Err(MigrationError::at(
                    "",
                    format!("the migration step of a {} at $v {v} gave no object", kind.name()),
                ));
            };
            if v + 1 == 1 {
                members.remove("$v");
            } else {
                members.insert("$v".to_owned(), Value::from(v + 1));
            }
        }
    }

    /// The manifest migrated: the manifest step of every format step in order on its `Value` ([`Manifest::to_text`]),
    /// read back as a manifest under `limits`.
    ///
    /// # Errors
    ///
    /// A step's [`MigrationError`], or one naming the member the migrated manifest cannot be read at; each at
    /// `manifest.json`.
    pub fn manifest(&self, manifest: &Manifest, limits: &Limits) -> Result<Manifest, MigrationError> {
        let file = FilePath::new(MANIFEST).ok();
        let at_manifest = |e: MigrationError| match &file {
            Some(file) => e.in_file(file, None),
            None => e,
        };
        let mut value = ew_schema::parse_value(&manifest.to_text())
            .map_err(|e| at_manifest(MigrationError::at("", e.to_string())))?;
        for step in &self.steps {
            value = (step.manifest)(value).map_err(at_manifest)?;
        }
        Manifest::parse(value.to_string().as_bytes(), limits)
            .map_err(|e| at_manifest(MigrationError::at("", format!("the migrated manifest: {e}"))))
    }
}

/// A problem the steward's check finds (WP-S1.17 D12, decision log ID-1221 ruling 8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StewardProblem {
    /// The format a build writes is not released, and the chain has no way to it from a released format (decision
    /// log ID-1230 ruling 7: from each, not from the last one only).
    NoStep {
        /// The released format.
        from: String,
        /// The format the build writes.
        to: String,
    },
    /// A released format has no folder in the fixture corpus (`fixtures/format/<format>/`).
    NoCorpus {
        /// The released format.
        format: String,
    },
}

impl fmt::Display for StewardProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StewardProblem::NoStep { from, to } => write!(
                f,
                "format {to} is not released and the migration chain has no step from the released format {from} \
                 towards it: add the steps with the format change"
            ),
            StewardProblem::NoCorpus { format } => write!(
                f,
                "the released format {format} has no fixture folder fixtures/format/{format}/: add its fixtures"
            ),
        }
    }
}

/// The steward's check (WP-S1.17 D12 as ruling 8 of decision log ID-1221 amends it): for the formats `released` so far
/// (oldest first), the format `format` a build writes, its migration `chain` and the fixture corpus at `corpus`
/// (`fixtures/format/`): every released format without its folder `<corpus>/<format>/`, and, when `format` is not
/// released, the missing way from each released format to `format` (decision log ID-1230 ruling 7). Empty while
/// nothing is released.
///
/// ```
/// use std::path::Path;
///
/// use ew_format::{MIGRATIONS, StewardProblem, steward_problems};
///
/// let corpus = Path::new("no such corpus");
/// assert_eq!(steward_problems(&[], "0.1", &MIGRATIONS, corpus), []);
/// assert_eq!(
///     steward_problems(&["0.1"], "0.2", &MIGRATIONS, corpus),
///     [
///         StewardProblem::NoCorpus { format: "0.1".into() },
///         StewardProblem::NoStep { from: "0.1".into(), to: "0.2".into() },
///     ]
/// );
/// ```
pub fn steward_problems(
    released: &[&str],
    format: &str,
    chain: &Migrations,
    corpus: &Path,
) -> Vec<StewardProblem> {
    let mut problems: Vec<StewardProblem> = released
        .iter()
        .filter(|f| !corpus.join(f).is_dir())
        .map(|f| StewardProblem::NoCorpus {
            format: (*f).to_owned(),
        })
        .collect();
    if !released.contains(&format) {
        problems.extend(
            released
                .iter()
                .filter(|from| chain.towards(from, format).is_err())
                .map(|from| StewardProblem::NoStep {
                    from: (*from).to_owned(),
                    to: format.to_owned(),
                }),
        );
    }
    problems
}
