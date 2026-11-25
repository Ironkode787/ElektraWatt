// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Derivation notes: what a derivation could not do as the project asks and what it did instead
//! (ADR-0003 "profiles are never silently replaced", doc 03 §5.1 defaults).
//!
//! Queries return values, not findings. Where a value rests on a fallback a rule reports, the value holds its
//! notes (`PhysicalWires::notes`, `RailTerminal::notes`, `IpPairing::notes` …), each a [`DeriveNote`] in the
//! canonical order of the type whatever the evaluation order was; the checks read them from the value.

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

/// What a [`Connectivity`](DeriveNote::Connectivity) note found, as the facts a rule reads.
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

/// One derivation note: what a value rests on that a rule reports.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum DeriveNote {
    /// A connectivity problem met while deriving nets, potentials or physical wires (doc 08 §3, §8.1): an unpaired
    /// interruption point, an ambiguous junction feed, a ring, a rail-bonded terminal that joins no location. The
    /// derivation chose a documented fallback.
    Connectivity {
        /// The finding code of the catalogue (doc 08 §8.1, doc 06 §6), such as `"EW-POT-017"`.
        code: &'static str,
        /// What was found, as the facts a rule reads.
        fact: NoteFact,
        /// The record the note is about: a connection, a junction, a function or a device.
        subject: ew_ids::Id<()>,
    },
}
