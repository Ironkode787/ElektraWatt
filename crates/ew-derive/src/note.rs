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
    /// A query needed its own value: the model violates EW-INV-015 (device inheritance graph). The
    /// query fell back to an empty structure or a "?" designation.
    QueryCycle {
        /// The family.
        family: &'static str,
        /// The key, as `Debug` text.
        key: String,
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
            _ => None,
        }
    }

    /// The severity.
    pub fn severity(&self) -> NoteSeverity {
        match self {
            DeriveNote::UnknownProfile { .. } | DeriveNote::QueryCycle { .. } => NoteSeverity::Error,
            DeriveNote::SchemeNotApplied { .. } | DeriveNote::MalformedShim { .. } => NoteSeverity::Warning,
            DeriveNote::DefaultFrame => NoteSeverity::Info,
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
            DeriveNote::QueryCycle { family, key } => write!(
                f,
                "{family} needs its own value for {key}: the device inheritance graph has a cycle"
            ),
        }
    }
}
