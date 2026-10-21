// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The slot for prevent rules (ADR-0009 decisions 3 and 4): the project's [`Strictness`] and the
//! [`PreventHook`] that `ew-checks` plugs into [`Model::apply_with`](crate::Model::apply_with) (WP-S1.15).
//! `ew-model` implements no prevent rule itself.

use ew_schema::{Date, Project};
use serde_json::Value;

use crate::entity::AnyKey;
use crate::model::Model;

/// The project's strictness for prevent rules (ADR-0009 decision 4, doc 15 §3.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Strictness {
    /// A violation rejects the transaction (the default for new projects).
    #[default]
    Strict,
    /// Set by an import: violations arriving through an import, an external change or a merge are
    /// committed as findings; interactive edits are still validated strictly.
    Lenient {
        /// When the project became lenient, if recorded.
        since: Option<Date>,
    },
}

impl Strictness {
    /// Reads the strictness of a project.
    ///
    /// `ew-schema` v0 has no typed `CheckSettings` yet (doc 15 §2.1; the settings of later work packages
    /// stay in `extra`), so this reads `settings.extra.checks.strictness`: `"strict"`, `"lenient"` or
    /// `{"lenient": {"since": "YYYY-MM-DD"}}`. Anything else, or nothing, is [`Strictness::Strict`]. The
    /// read-through switches to the typed member when WP-S1.15 adds it.
    pub fn of(project: &Project) -> Strictness {
        let Some(value) = project
            .settings
            .extra
            .get("checks")
            .and_then(|c| c.get("strictness"))
        else {
            return Strictness::Strict;
        };
        match value {
            Value::String(s) if s == "lenient" => Strictness::Lenient { since: None },
            Value::Object(map) => match map.get("lenient") {
                Some(inner) => Strictness::Lenient {
                    since: inner
                        .get("since")
                        .and_then(Value::as_str)
                        .and_then(|s| s.parse::<Date>().ok()),
                },
                None => Strictness::Strict,
            },
            _ => Strictness::Strict,
        }
    }
}

/// A prevent-rule violation reported by a [`PreventHook`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PreventViolation {
    /// The rule ID, such as `EW-TERM-002`.
    pub rule: String,
    /// The finding's message, ideally with the proposed alternative.
    pub message: String,
    /// The records involved.
    pub keys: Vec<AnyKey>,
}

/// Evaluates prevent rules on a candidate snapshot after the structural invariants passed.
///
/// The hook reads [`Model::strictness`] and decides itself what a lenient project lets through; an `Err`
/// rejects the batch with [`ModelError::Prevented`](crate::ModelError::Prevented).
///
/// The hook also receives the snapshot the batch was applied to (`base`), so that a rule can tell a
/// violation the batch creates from one the model already held: a lenient import may have committed
/// duplicates, and an edit elsewhere on the same strip must not be refused for them (doc 15 §3.5,
/// FR-CHK-011).
pub trait PreventHook: Send + Sync {
    /// Checks `candidate`, the result of applying a batch to `base`; `touched` lists the keys the batch
    /// put, sorted.
    fn check(&self, base: &Model, candidate: &Model, touched: &[AnyKey]) -> Result<(), PreventViolation>;
}

/// Options of [`Model::apply_with`](crate::Model::apply_with).
#[derive(Clone, Copy, Default)]
pub struct ApplyOptions<'a> {
    /// Prevent rules to evaluate after the structural invariants; `None` evaluates none.
    pub prevent: Option<&'a dyn PreventHook>,
}

impl std::fmt::Debug for ApplyOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplyOptions")
            .field("prevent", &self.prevent.map(|_| "dyn PreventHook"))
            .finish()
    }
}
