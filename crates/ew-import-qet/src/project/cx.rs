// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the project mappers share: record IDs from source keys ([`Ids`]), the capped findings collector
//! ([`Findings`]) and the registry of placed elements that the conductor mapper resolves ends against
//! ([`Placed`]).

use std::collections::BTreeMap;

use ew_ids::{Id, Uuid, v5};
use ew_interop_api::{Code, ReportEntry, Severity, SourceLocation, SourceRef};
use ew_schema::{Function, ImportLineage, Junction, Page};
use ew_units::Pt2;

use super::model::QetDiagram;

/// The most findings one project import lists; further findings are counted per code in one EW-IMP-001
/// summary entry (doc 14 §11 "Report volume").
pub const MAX_PROJECT_ENTRIES: usize = 10_000;

/// Record IDs and provenance of one import: `v5(namespace, source key)` (doc 14 §2.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ids {
    /// The lineage namespace.
    pub namespace: Uuid,
    /// The lineage record.
    pub lineage: Id<ImportLineage>,
}

impl Ids {
    /// The record ID of source key `key`.
    pub fn id<T>(&self, key: &str) -> Id<T> {
        v5::imported_id(self.namespace, key)
    }

    /// The persisted provenance `{lineage, key}` of source key `key`.
    pub fn source_ref(&self, key: &str) -> SourceRef {
        SourceRef {
            lineage: self.lineage,
            key: key.to_owned(),
            extra: BTreeMap::new(),
        }
    }
}

/// The findings of one project import, capped at `cap` listed entries.
#[derive(Clone, Debug)]
pub struct Findings {
    file: String,
    cap: usize,
    entries: Vec<ReportEntry>,
    /// Entries not listed because the cap was reached, per code.
    suppressed: BTreeMap<Code, u32>,
}

impl Findings {
    /// A collector for source file `file` (a file name, never a path) listing at most `cap` entries.
    pub fn new(file: &str, cap: usize) -> Findings {
        Findings {
            file: file.to_owned(),
            cap,
            entries: Vec::new(),
            suppressed: BTreeMap::new(),
        }
    }

    /// A location in the source file: folio label, item and line, each when known.
    pub fn loc(&self, folio: Option<&str>, item: Option<String>, line: Option<u32>) -> SourceLocation {
        SourceLocation {
            file: self.file.clone(),
            member: None,
            folio: folio.map(str::to_owned),
            cell: None,
            item,
            line,
        }
    }

    /// Records a finding at `loc`.
    pub fn push(&mut self, code: Code, severity: Severity, loc: SourceLocation, message: impl Into<String>) {
        self.entry(ReportEntry::new(code, severity, message).at(loc));
    }

    /// Records a finding of the whole file (no item location).
    pub fn file_level(&mut self, code: Code, severity: Severity, message: impl Into<String>) {
        let loc = self.loc(None, None, None);
        self.push(code, severity, loc, message);
    }

    /// Records a ready entry (from an element definition, for example).
    pub fn entry(&mut self, e: ReportEntry) {
        if self.entries.len() < self.cap {
            self.entries.push(e);
        } else {
            *self.suppressed.entry(e.code).or_default() += 1;
        }
    }

    /// The entries listed so far.
    #[cfg(test)]
    pub fn entries(&self) -> &[ReportEntry] {
        &self.entries
    }

    /// The listed entries, followed by one EW-IMP-001 summary entry (error, as the catalogue defines the
    /// code) when the cap was reached, and the suppressed count per code (for `ImportStats::suppressed`).
    pub fn finish(mut self) -> (Vec<ReportEntry>, BTreeMap<Code, u32>) {
        if !self.suppressed.is_empty() {
            let total: u32 = self.suppressed.values().sum();
            let per_code = self
                .suppressed
                .iter()
                .map(|(c, n)| format!("{c}: {n}"))
                .collect::<Vec<_>>()
                .join(", ");
            let loc = self.loc(None, None, None);
            self.entries.push(
                ReportEntry::new(
                    Code::LIMIT,
                    Severity::Error,
                    format!(
                        "{total} further findings not listed (at most {} per project): {per_code}",
                        self.cap
                    ),
                )
                .at(loc),
            );
        }
        (self.entries, self.suppressed)
    }
}

/// The key of a placed element in the [`Registry`]: its diagram index and its element key (the normalised
/// UUID, `<uuid>#<n>` for the n-th repeat of a UUID on one folio, or `#<n>` for the n-th element of the
/// diagram when it has no UUID).
pub type ElementKey = (usize, String);

/// The placed elements by key, as the element mapper made them, for the conductor mapper.
pub type Registry = BTreeMap<ElementKey, Placed>;

/// What the element mapper made of one placed element.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Placed {
    /// The diagram index.
    pub diagram: usize,
    /// The page.
    pub page: Id<Page>,
    /// The folio label, for report locations.
    pub folio: String,
    /// The project-wide element key that record and wire source keys use (doc 14 §2.3): the UUID, or
    /// `<uuid>#<folio key>#<n>` for a UUID repeated by copy-paste (EW-IMP-120), or `<folio key>#<n>` for an
    /// element without UUID.
    pub ek: String,
    /// Where wires attached to the element end.
    pub target: Target,
    /// The definition's terminals in definition order.
    pub terminals: Vec<PlacedTerminal>,
}

/// One definition terminal of a placed element.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlacedTerminal {
    /// The definition terminal's UUID, normalised (`model::norm_uuid`), if it has one.
    pub uuid: Option<String>,
    /// Its position in definition coordinates, nm.
    pub local: Pt2,
    /// Its position on the page, nm: element position + orientation applied to `local`; `None` on overflow.
    pub page_pos: Option<Pt2>,
}

/// Where the wires of a placed element end.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Target {
    /// At CP `cps[i]` of `function` for definition terminal `i` (`None`: that terminal maps to no CP, and a
    /// wire ending there is lost and reported).
    Function {
        /// The function.
        function: Id<Function>,
        /// The function CP per definition terminal.
        cps: Vec<Option<u16>>,
    },
    /// At the junction `id`, placed at `at`: the conductor mapper emits the `Junction` record with the
    /// connections ending there in document order (doc 14 §4.7, EW-IMP-118).
    Junction {
        /// The junction record ID.
        id: Id<Junction>,
        /// The source key of the junction (for its `source_ref`).
        key: String,
        /// Its position on the page.
        at: Pt2,
    },
    /// Not mapped (already reported); wires ending here are lost (EW-IMP-003).
    Dropped,
}

/// The element keys of the elements of `diagram`, by element index: the normalised UUID; `<uuid>#<n>` for
/// the n-th occurrence (n ≥ 2) of a UUID on this folio (copy-paste, QET #1408); `#<n>` for an element
/// without a UUID at source position `n` ([`QetElement::n`](super::model::QetElement::n)). These keys are
/// folio-local; record source keys use [`Placed::ek`]. The conductor mapper and the element mapper both key elements this way.
pub fn element_keys(diagram: &QetDiagram) -> Vec<String> {
    let mut seen: BTreeMap<&str, u32> = BTreeMap::new();
    diagram
        .elements
        .iter()
        .map(|e| match e.uuid.as_deref() {
            Some(u) => {
                let n = seen.entry(u).or_insert(0);
                *n += 1;
                if *n == 1 { u.to_owned() } else { format!("{u}#{n}") }
            }
            None => format!("#{}", e.n),
        })
        .collect()
}

/// The per-folio integer terminal IDs of `diagram`: `<terminal id>` (trimmed) → (element key, index of the
/// terminal in the element's `<terminals>` list). `keys` are the [`element_keys`] of the diagram. Of two
/// terminals with one ID the first in document order is kept.
pub fn terminal_ids(diagram: &QetDiagram, keys: &[String]) -> BTreeMap<String, (String, usize)> {
    let mut map = BTreeMap::new();
    for (e, key) in diagram.elements.iter().zip(keys) {
        for (i, t) in e.terminals.iter().enumerate() {
            if let Some(id) = &t.id {
                map.entry(id.trim().to_owned())
                    .or_insert_with(|| (key.clone(), i));
            }
        }
    }
    map
}
