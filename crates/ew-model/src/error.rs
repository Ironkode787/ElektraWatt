// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The error of loading, applying and planning.

use std::fmt;

use crate::entity::AnyKey;
use crate::rev::Rev;

/// Why a batch, a load or a delete plan was rejected. A rejected batch leaves the model unchanged (the
/// candidate snapshot is dropped).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ModelError {
    /// The batch's revision is not greater than the model's.
    RevNotIncreasing {
        /// The model's revision.
        current: Rev,
        /// The revision given to `apply`.
        given: Rev,
    },
    /// An op is malformed: its rows do not match its key, it names a kind without a table, or it would
    /// remove or re-key the project row.
    Malformed {
        /// The op's key.
        key: AnyKey,
        /// What is wrong.
        reason: &'static str,
    },
    /// An op's `before` is not the current row: the batch was prepared on another snapshot. This is the
    /// optimistic-rebase signal of ADR-0008: re-run the command on the current snapshot.
    StaleBefore {
        /// The op's key.
        key: AnyKey,
    },
    /// A record was removed while a referrer still names it through a link whose delete policy is
    /// `forbid` (doc 03 §4.2).
    Referenced {
        /// The link, as `kind.field`.
        link: String,
        /// The removed record.
        target: AnyKey,
        /// The record that still names it.
        referrer: AnyKey,
    },
    /// A structural invariant (ADR-0009 decision 2) does not hold on the candidate snapshot.
    Invariant {
        /// The invariant's stable ID (`ew-model/invariants.md`).
        id: &'static str,
        /// What is violated, naming the records.
        message: String,
        /// The records involved, the offending one first.
        keys: Vec<AnyKey>,
    },
    /// A prevent rule of the plugged-in hook rejected the batch (ADR-0009 decision 3, strict mode).
    Prevented {
        /// The rule ID, such as `EW-TERM-002`.
        rule: String,
        /// The finding's message.
        message: String,
        /// The records involved.
        keys: Vec<AnyKey>,
    },
    /// Two records of one kind share an ID (EW-CORE-009), or the record set holds a second project row.
    DuplicateRecord {
        /// The duplicated key (the smallest one if there are several).
        key: AnyKey,
    },
    /// The key names no record.
    NotFound {
        /// The key.
        key: AnyKey,
    },
}

/// Formats a key as `kind:id`.
pub(crate) struct KeyFmt<'a>(pub(crate) &'a AnyKey);

impl fmt::Display for KeyFmt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.0.0, self.0.1)
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::RevNotIncreasing { current, given } => {
                write!(
                    f,
                    "revision {given} is not greater than the model's revision {current}"
                )
            }
            ModelError::Malformed { key, reason } => write!(f, "malformed op on {}: {reason}", KeyFmt(key)),
            ModelError::StaleBefore { key } => {
                write!(
                    f,
                    "stale op on {}: its `before` is not the current row",
                    KeyFmt(key)
                )
            }
            ModelError::Referenced {
                link,
                target,
                referrer,
            } => write!(
                f,
                "{} cannot be removed: {} still names it through {link} (delete policy forbid)",
                KeyFmt(target),
                KeyFmt(referrer)
            ),
            ModelError::Invariant { id, message, .. } => write!(f, "{id}: {message}"),
            ModelError::Prevented { rule, message, .. } => write!(f, "{rule}: {message}"),
            ModelError::DuplicateRecord { key } => write!(f, "duplicate record {}", KeyFmt(key)),
            ModelError::NotFound { key } => write!(f, "no record {}", KeyFmt(key)),
        }
    }
}

impl std::error::Error for ModelError {}
