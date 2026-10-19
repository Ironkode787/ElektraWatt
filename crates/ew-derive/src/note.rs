// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Derivation notes: what a derivation could not do as the project asks and what it did instead
//! (ADR-0003 "profiles are never silently replaced", doc 03 §5.1 defaults).
//!
//! Queries return values, not findings. Where a value rests on a fallback the user should know about, the
//! query pushes a [`DeriveNote`] onto its [`Q`](crate::Q): an unknown pinned profile, a numbering scheme the
//! command layer does not implement, the built-in default frame, a malformed provisional settings member, a
//! dependency cycle in a model that violates the device inheritance invariant. The notes are request-scoped
//! like the cache and kept as a set, so their order is the canonical order of [`DeriveNote`] whatever the
//! evaluation order was. A caller reads them through [`Q::diagnostics`](crate::Q::diagnostics), which needs
//! `&mut Q` and is therefore out of reach of a query body (a query must not depend on what other queries
//! happened to evaluate).
//!
//! **Memo engine.** A note is a side output of the evaluation that pushed it. While every family is cold
//! (phase 1) a fresh `Q` re-evaluates and re-notes. Before `ew-memo` memoises a family that pushes notes,
//! the notes must become part of the memoised value (or the family stays cold).

use std::fmt;

/// How serious a note is.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NoteSeverity {
    /// A documented default applied.
    Info,
    /// The project asks for something this version does not implement; a default applied instead.
    Warning,
    /// The printed output may be wrong (ADR-0003: an unknown profile is never silent).
    Error,
}

/// Why a project numbering scheme was not applied.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SchemeIssue {
    /// Its counter scope is not `"page"`, the only scope the command layer numbers (ID-48).
    CounterScope,
    /// Its format (or a rule's) lacks `{page}`, so counters unique per page would not give unique tags.
    FormatWithoutPage,
    /// Its format (or a rule's) lacks `{letter}` or `{counter}`, the other member of the counter scope
    /// (page, letter) and the counter itself.
    FormatWithoutCounter,
    /// A device names a scheme key (`TagScheme::Named`) that the project's settings do not define.
    UnknownKey,
}

/// One derivation note.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum DeriveNote {
    /// The project pins a derivation profile this build does not ship (ADR-0003). Display uses the newest
    /// shipped profile; profile-dependent commands are refused (WP-S1.11). Integrity code
    /// [`EW_CORE_018`].
    UnknownProfile {
        /// The pinned key.
        requested: String,
        /// The profile used for display.
        used: String,
    },
    /// A numbering scheme was not applied; the pinned profile's default scheme formats the tag instead
    /// (decision log ID-99, until the numbering engine of WP-S1.11 numbers every scheme).
    SchemeNotApplied {
        /// The scheme key.
        scheme: String,
        /// Why.
        issue: SchemeIssue,
    },
    /// No frame is configured for a page: the built-in default geometry of the QET-compatible A4 frame
    /// (doc 03 §5.1, doc 20 §17.1) gives its cells (one note however many pages used it).
    DefaultFrame,
    /// A provisional settings member (`ProjectSettings.extra[...]`, `Project.extra[...]`) is malformed;
    /// the default applies.
    MalformedShim {
        /// The member, such as `"settings.frames"`.
        member: String,
        /// What is wrong, in English.
        detail: String,
    },
    /// Two colour rows of the project's colour profile have the same specificity, can match a common
    /// potential and give different colours (doc 08 §4.2). The profile is used; the first row wins.
    ProfileOverlap {
        /// The shim member, `"settings.colour_profile"`.
        shim: String,
        /// The indexes of the two rows, in profile order.
        rows: (usize, usize),
        /// What is wrong, in English.
        detail: String,
    },
    /// A query needed its own value: the model violates EW-INV-015 (device inheritance graph). The
    /// query fell back to an empty structure or a "?" designation.
    QueryCycle {
        /// The family.
        family: &'static str,
        /// The key, as `Debug` text.
        key: String,
    },
    /// The project has no colour or size profile (`ProjectSettings.extra["colour_profile"]` or
    /// `["size_profile"]`, doc 03 §5.1): the profile rule of the wire-property chain (level 2) uses the
    /// built-in defaults of the derivation profile, with provenance `BuiltIn` (one note per member).
    DefaultWireProfile {
        /// `"colour_profile"` or `"size_profile"`.
        member: &'static str,
    },
    /// A connectivity problem met while deriving nets, potentials, physical wires or wire properties
    /// (doc 08 §3, §8.1): an unpaired interruption point, conflicting potential definitions, an ambiguous
    /// junction feed, a ring, a missing colour or size profile. The derivation chose a documented fallback.
    /// Severity: `Error` for `EW-POT-001`, `EW-POT-014` and `EW-CORE-051`, `Warning` otherwise.
    Connectivity {
        /// The finding code of the catalogue (doc 08 §8.1, doc 06 §6), such as `"EW-POT-017"`.
        code: &'static str,
        /// The subject: the record IDs or names concerned.
        subject: String,
        /// What happened and which fallback applied, in English.
        detail: String,
    },
    /// A cross-reference could not be derived as the model asks (WP-S1.08): a target without an anchor
    /// cell (`EW-CORE-053`), an auxiliary whose main function is unplaced (`EW-CORE-053`), a device with two
    /// main functions (`EW-INV-002`). The text uses the documented fallback. Severity: `Error` for
    /// `EW-INV-002`, `Warning` otherwise.
    Xref {
        /// The finding or integrity code.
        code: &'static str,
        /// The subject: the record IDs concerned.
        subject: String,
        /// What happened and which fallback applied, in English.
        detail: String,
    },
}

/// The integrity code of an unknown pinned derivation profile (doc 05 §6.3, doc 15 §4; decision log ID-72).
pub const EW_CORE_018: &str = "EW-CORE-018";

impl DeriveNote {
    /// The finding code a front-end reports the note under, where the catalogue assigns one: `EW-CORE-018`
    /// for an unknown profile. The other notes have no catalogue code yet.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            DeriveNote::UnknownProfile { .. } => Some(EW_CORE_018),
            DeriveNote::Connectivity { code, .. } | DeriveNote::Xref { code, .. } => Some(code),
            _ => None,
        }
    }

    /// The severity. A [`Connectivity`](DeriveNote::Connectivity) note is an `Error` for the codes whose
    /// fallback may change the printed output or the electrical meaning (`EW-POT-001`, `EW-POT-014`,
    /// `EW-CORE-051`) and a `Warning` for every other code.
    pub fn severity(&self) -> NoteSeverity {
        match self {
            DeriveNote::UnknownProfile { .. } | DeriveNote::QueryCycle { .. } => NoteSeverity::Error,
            DeriveNote::Connectivity { code, .. } => {
                if matches!(*code, "EW-POT-001" | "EW-POT-014" | "EW-CORE-051") {
                    NoteSeverity::Error
                } else {
                    NoteSeverity::Warning
                }
            }
            DeriveNote::Xref { code, .. } => {
                if *code == "EW-INV-002" {
                    NoteSeverity::Error
                } else {
                    NoteSeverity::Warning
                }
            }
            DeriveNote::SchemeNotApplied { .. }
            | DeriveNote::MalformedShim { .. }
            | DeriveNote::ProfileOverlap { .. } => NoteSeverity::Warning,
            DeriveNote::DefaultFrame | DeriveNote::DefaultWireProfile { .. } => NoteSeverity::Info,
        }
    }
}

impl fmt::Display for DeriveNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeriveNote::UnknownProfile { requested, used } => write!(
                f,
                "the project pins derivation profile {requested}, which this version does not ship; \
                 pages are displayed with profile {used} and may differ from the released output"
            ),
            DeriveNote::SchemeNotApplied { scheme, issue } => {
                let why = match issue {
                    SchemeIssue::CounterScope => "its counter scope is not \"page\"",
                    SchemeIssue::FormatWithoutPage => "its format has no {page} token",
                    SchemeIssue::FormatWithoutCounter => "its format lacks {letter} or {counter}",
                    SchemeIssue::UnknownKey => "the project's settings define no scheme of this key",
                };
                write!(
                    f,
                    "numbering scheme {scheme} is not applied ({why}); tags use the profile's default scheme"
                )
            }
            DeriveNote::DefaultFrame => {
                f.write_str("no frame is configured; cells use the default A4 frame geometry")
            }
            DeriveNote::MalformedShim { member, detail } => {
                write!(f, "{member} is malformed ({detail}); the default applies")
            }
            DeriveNote::ProfileOverlap { shim, rows, detail } => write!(
                f,
                "{shim}: rows {} and {} {detail}; the first row wins",
                rows.0, rows.1
            ),
            DeriveNote::QueryCycle { family, key } => write!(
                f,
                "{family} needs its own value for {key}: the device inheritance graph has a cycle"
            ),
            DeriveNote::DefaultWireProfile { member } => write!(
                f,
                "the project has no {member}; wire properties use the built-in defaults of the derivation profile"
            ),
            DeriveNote::Connectivity {
                code,
                subject,
                detail,
            }
            | DeriveNote::Xref {
                code,
                subject,
                detail,
            } => write!(f, "{code} {subject}: {detail}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connectivity(code: &'static str) -> DeriveNote {
        DeriveNote::Connectivity {
            code,
            subject: "x".into(),
            detail: "y".into(),
        }
    }

    #[test]
    fn connectivity_error_codes_are_errors() {
        for code in ["EW-POT-001", "EW-POT-014", "EW-CORE-051"] {
            assert_eq!(connectivity(code).severity(), NoteSeverity::Error, "{code}");
        }
        assert_eq!(connectivity("EW-POT-017").severity(), NoteSeverity::Warning);
    }
}
