// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Derivation notes: what a derivation could not do as the project asks and what it did instead
//! (ADR-0003 "profiles are never silently replaced", doc 03 §5.1 defaults).
//!
//! Queries return values, not findings. Where a value rests on a fallback a rule reports, the value holds its
//! notes (`PhysicalWires::notes`, `RailTerminal::notes`, `IpPairing::notes` …), each a [`DeriveNote`] in the
//! canonical order of the type whatever the evaluation order was; the checks read them from the value.

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
    /// Its counter scope is not `"page"`, the only scope the command layer numbers.
    CounterScope,
    /// Its format (or a rule's) lacks `{page}`, so counters unique per page would not give unique tags.
    FormatWithoutPage,
    /// Its format (or a rule's) lacks `{letter}` or `{counter}`, the other member of the counter scope
    /// (page, letter) and the counter itself.
    FormatWithoutCounter,
    /// A device names a scheme key (`TagScheme::Named`) that the project's settings do not define.
    UnknownKey,
}

impl SchemeIssue {
    /// Why the scheme is not applied, as a clause for messages.
    pub fn describe(&self) -> &'static str {
        match self {
            SchemeIssue::CounterScope => "its counter scope is not \"page\"",
            SchemeIssue::FormatWithoutPage => "its format has no {page} token",
            SchemeIssue::FormatWithoutCounter => "its format lacks {letter} or {counter}",
            SchemeIssue::UnknownKey => "the project's settings define no scheme of this key",
        }
    }
}

/// What a [`Connectivity`](DeriveNote::Connectivity) note found, as the facts a rule reads; the note's `detail` says it
/// in English for a log.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum NoteFact {
    /// A fact no rule reads beyond the note's code and subject.
    Other,
    /// The connection closes a ring in the drawing graph and forms no physical wire (`EW-POT-012`).
    Ring,
    /// A constituent of a physical wire names another conductor than the one that applies (`EW-POT-014`).
    Conductor,
    /// A constituent of a physical wire overrides a value the wire already has; the first value applies
    /// (`EW-POT-014`).
    Override,
    /// A rail-bonded terminal without a location, in a project with `locations` own locations, joins no location PE
    /// node (`EW-POT-008`).
    RailWithoutLocation {
        /// The project's own locations.
        locations: usize,
    },
    /// A rail-bonded terminal without a PE connection point joins no location PE node (`EW-TERM-005`).
    RailWithoutPe,
}

/// One derivation note.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum DeriveNote {
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
        /// What was found, as the facts a rule reads.
        fact: NoteFact,
        /// The subject: the record IDs or names concerned.
        subject: String,
        /// What happened and which fallback applied, in English.
        detail: String,
    },
    /// A wire marker lists a connection that does not exist or is not drawn on the marker's page (a file of
    /// an older build, which kept page items verbatim): a load finding, not a refusal (ADR-0009 amendment
    /// 8). The derivation and the drawing ignore the entry; the next edit of the marker may write a clean list.
    StaleMarkerEntry {
        /// The marker (page item) ID.
        marker: String,
        /// The listed connection ID.
        connection: String,
        /// What is wrong with it, in English (`does not exist`, `is not drawn on the marker's page`).
        detail: &'static str,
    },
}

impl DeriveNote {
    /// The finding code a front-end reports the note under, where the catalogue assigns one: a connectivity
    /// note's. The other notes have no catalogue code yet.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            DeriveNote::Connectivity { code, .. } => Some(code),
            _ => None,
        }
    }

    /// The severity. A [`Connectivity`](DeriveNote::Connectivity) note is an `Error` for the codes whose
    /// fallback may change the printed output or the electrical meaning (`EW-POT-001`, `EW-POT-014`,
    /// `EW-CORE-051`) and a `Warning` for every other code.
    pub fn severity(&self) -> NoteSeverity {
        match self {
            DeriveNote::QueryCycle { .. } => NoteSeverity::Error,
            DeriveNote::Connectivity { code, .. } => {
                if matches!(*code, "EW-POT-001" | "EW-POT-014" | "EW-CORE-051") {
                    NoteSeverity::Error
                } else {
                    NoteSeverity::Warning
                }
            }
            DeriveNote::MalformedShim { .. }
            | DeriveNote::ProfileOverlap { .. }
            | DeriveNote::StaleMarkerEntry { .. } => NoteSeverity::Warning,
            DeriveNote::DefaultWireProfile { .. } => NoteSeverity::Info,
        }
    }
}

impl fmt::Display for DeriveNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            DeriveNote::StaleMarkerEntry {
                marker,
                connection,
                detail,
            } => write!(
                f,
                "wire marker {marker} lists connection {connection}, which {detail}; the entry is ignored until the \
                 marker is edited"
            ),
            DeriveNote::Connectivity {
                code,
                subject,
                detail,
                ..
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
            fact: NoteFact::Other,
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
