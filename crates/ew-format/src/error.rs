// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The writer's error type.

use std::fmt;

use ew_schema::AnyId;

/// A record or document the writer cannot put into a file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FormatError {
    /// The record has no JSON form (ADR-0005: a coordinate beyond ±(2⁵³ − 1)).
    Serialize {
        /// `kind:id` of the record.
        record: String,
        /// The serialiser's message.
        message: String,
    },
    /// A record of a reserved or unknown kind that was not read from a file and whose ownership rule needs
    /// more than the record itself (doc 05 §10.3: such records stay in the file they came from).
    NoFile {
        /// The `$k`.
        kind: String,
        /// The ID, if any.
        id: Option<AnyId>,
    },
    /// Two records with one `($k, id)` (EW-CORE-009) in the input of a writer.
    Duplicate {
        /// The `$k`.
        kind: String,
        /// The ID.
        id: AnyId,
    },
    /// A project without its `project` record.
    NoProject,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Serialize { record, message } => {
                write!(f, "the record {record} has no JSON form: {message}")
            }
            FormatError::NoFile { kind, id } => match id {
                Some(id) => write!(
                    f,
                    "no file for the record {kind}:{id} of a kind without a writable rule"
                ),
                None => write!(f, "no file for a record of kind \"{kind}\" without an ID"),
            },
            FormatError::Duplicate { kind, id } => write!(f, "two records {kind}:{id} (EW-CORE-009)"),
            FormatError::NoProject => f.write_str("the records hold no project record"),
        }
    }
}

impl std::error::Error for FormatError {}
