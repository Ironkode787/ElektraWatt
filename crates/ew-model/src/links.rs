// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The link table: every ID-valued field of every record that names another record, with its target
//! kind, its delete policy (doc 03 §4.2) and the base index that reverse-maps it (ARCH §4.1, ADR-0001
//! decision 5).
//!
//! [`LINKS`] is the single source of truth for
//! - the base indexes ([`crate::BaseIndexes`]): a link with an [`IndexName`] feeds that named index, every
//!   other link feeds the generic referrer index, so every link can be followed backwards;
//! - the structural invariant "references resolve" (EW-INV-001): after every batch each touched record's
//!   links name live records, and a removed record has no referrer left;
//! - the delete policies: [`Policy::Forbid`] makes [`Model::apply`](crate::Model::apply) reject the
//!   removal ([`ModelError::Referenced`](crate::ModelError::Referenced)); [`Policy::Cascade`] and
//!   [`Policy::Unlink`] are computed by [`Model::delete_plan`](crate::Model::delete_plan), which a
//!   transaction includes (WP-S1.04), and `apply` only verifies the result.
//!
//! [`NOT_LINKS`] lists every other ID-valued member of the persisted schema with the reason it is not a
//! link. The field-mask test checks `LINKS ∪ NOT_LINKS ∪ {every kind's own id}` against the JSON Schemas
//! in `schemas/0.1`, so adding an ID field to a record without declaring it fails CI.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{AnyId, ConnectionKind, Endpoint, FunctionKind, Kind, Slot, SourceRef};

use crate::entity::{AnyKey, AnyRow};

/// What happens to a referrer when the record it links to is removed (doc 03 §4.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Policy {
    /// The referrer is removed as well.
    Cascade,
    /// The reference is cleared: an optional field becomes `None`, a list loses the entry, a cable core
    /// reverts to a plain wire. A mandatory field that cannot be cleared drops its record ("definition
    /// dropped", doc 03 §4.2).
    Unlink,
    /// The removal is rejected while a referrer exists.
    Forbid,
}

/// The named base indexes (ARCH §4.1). Links without one are served by the generic referrer index.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum IndexName {
    /// `Function.device` → functions.
    FunctionsByDevice,
    /// `Representation.page` → representations.
    RepsByPage,
    /// `Representation.functions` and `pin_map[].function` → representations.
    RepsByFunction,
    /// `Connection.page` → connections.
    ConnectionsByPage,
    /// `Connection.a`/`b` → connections, keyed by CP `(function, idx)` or junction.
    ConnectionsByEndpoint,
    /// `Junction.page` → junctions.
    JunctionsByPage,
    /// `Terminal.strip` → terminal devices.
    TerminalsByStrip,
    /// `Conductor.cable` → conductors.
    ConductorsByCable,
    /// `StructureNode.parent` → child nodes (`children_by_parent`, structure tree).
    StructureChildren,
    /// `Device.parent` → child devices (`children_by_parent`, assembly graph).
    DeviceChildren,
    /// `Page.folio.section` → pages.
    PagesBySection,
    /// `PotentialDef.at` → potential definitions, keyed by endpoint.
    PotentialDefsByEndpoint,
    /// `Bond.a`/`b` → bonds, keyed by endpoint.
    BondsByEndpoint,
}

/// One link occurrence in a record: the target ID, and for a CP endpoint the CP index.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Target {
    /// A record ID of the link's target kind.
    Id(AnyId),
    /// A connection point `(function, idx)`; the target record is the function.
    Cp(AnyId, u16),
}

impl Target {
    /// The ID of the target record.
    pub(crate) fn id(self) -> AnyId {
        match self {
            Target::Id(id) | Target::Cp(id, _) => id,
        }
    }
}

/// Extracts the link occurrences of one link from a row of the link's kind (other kinds yield nothing).
pub(crate) type TargetsFn = fn(&AnyRow, &mut Vec<Target>);

/// Clears every reference of one link that names a removed key; `None` drops the referrer.
pub(crate) type UnlinkFn = fn(&AnyRow, &BTreeSet<AnyKey>) -> Option<AnyRow>;

/// One declared link field.
#[derive(Clone, Copy)]
pub struct LinkDecl {
    /// The kind holding the field.
    pub from: Kind,
    /// The field path in the kind's JSON Schema (`terminal.strip`, `parts[].part.copy`, `a.cp[0]`).
    pub field: &'static str,
    /// The kind it names.
    pub to: Kind,
    /// The delete policy.
    pub policy: Policy,
    /// The named base index fed by this link, if any.
    pub index: Option<IndexName>,
    pub(crate) targets: TargetsFn,
    pub(crate) unlink: Option<UnlinkFn>,
}

impl LinkDecl {
    /// Whether a named base index serves this link (every link is reverse-mapped; the others by the
    /// generic referrer index).
    pub fn indexed(&self) -> bool {
        self.index.is_some()
    }

    /// `kind.field`, the path used in errors and in the field-mask test.
    pub fn path(&self) -> String {
        format!("{}.{}", self.from.name(), self.field)
    }

    pub(crate) fn targets_of(&self, row: &AnyRow) -> Vec<Target> {
        let mut out = Vec::new();
        (self.targets)(row, &mut out);
        out
    }
}

impl std::fmt::Debug for LinkDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkDecl")
            .field("from", &self.from)
            .field("field", &self.field)
            .field("to", &self.to)
            .field("policy", &self.policy)
            .field("index", &self.index)
            .finish()
    }
}

/// The position of a link in [`LINKS`].
pub type LinkId = usize;

// ---------------------------------------------------------------------------------------------------------
// Extraction helpers

fn id<T>(id: Id<T>, out: &mut Vec<Target>) {
    out.push(Target::Id(id.cast()));
}

fn opt<T>(v: Option<Id<T>>, out: &mut Vec<Target>) {
    if let Some(v) = v {
        id(v, out);
    }
}

fn slot(s: &Option<Slot>, out: &mut Vec<Target>) {
    if let Some(Slot::Node(n)) = s {
        id(*n, out);
    }
}

fn lineage(s: &Option<SourceRef>, out: &mut Vec<Target>) {
    if let Some(s) = s {
        id(s.lineage, out);
    }
}

fn cp(e: &Endpoint, out: &mut Vec<Target>) {
    if let Endpoint::Cp(f, idx) = e {
        out.push(Target::Cp(f.cast(), *idx));
    }
}

fn junction(e: &Endpoint, out: &mut Vec<Target>) {
    if let Endpoint::Junction(j) = e {
        id(*j, out);
    }
}

fn gone<T>(del: &BTreeSet<AnyKey>, kind: Kind, id: Id<T>) -> bool {
    del.contains(&(kind, id.cast()))
}

/// Declares one link. `$v` binds the typed record of kind `$from`.
macro_rules! link {
    ($from:ident . $field:literal -> $to:ident, $policy:ident, $index:expr, |$v:ident, $out:ident| $body:expr) => {
        link!($from . $field -> $to, $policy, $index, |$v, $out| $body, None)
    };
    ($from:ident . $field:literal -> $to:ident, $policy:ident, $index:expr, |$v:ident, $out:ident| $body:expr, $unlink:expr) => {
        LinkDecl {
            from: Kind::$from,
            field: $field,
            to: Kind::$to,
            policy: Policy::$policy,
            index: $index,
            targets: |row, $out| {
                if let AnyRow::$from($v) = row {
                    $body
                }
            },
            unlink: $unlink,
        }
    };
}

/// The `source_ref.lineage` link of a kind: forbid (a lineage anchors the identity of its records).
macro_rules! lineage_link {
    ($from:ident) => {
        link!($from."source_ref.lineage" -> ImportLineage, Forbid, None, |v, out| lineage(&v.source_ref, out))
    };
}

/// The seven structure slots of a `StructureAssignment` member: forbid (doc 03 §4.2: a structure node
/// referenced by pages or devices is merged, never deleted under them).
macro_rules! slot_links {
    ($from:ident) => {
        [
            link!($from."structure.functional_assignment" -> StructureNode, Forbid, None,
                |v, out| slot(&v.structure.functional_assignment, out)),
            link!($from."structure.function" -> StructureNode, Forbid, None, |v, out| slot(&v.structure.function, out)),
            link!($from."structure.installation_site" -> StructureNode, Forbid, None,
                |v, out| slot(&v.structure.installation_site, out)),
            link!($from."structure.location" -> StructureNode, Forbid, None, |v, out| slot(&v.structure.location, out)),
            link!($from."structure.document_kind" -> StructureNode, Forbid, None,
                |v, out| slot(&v.structure.document_kind, out)),
            link!($from."structure.user_defined" -> StructureNode, Forbid, None,
                |v, out| slot(&v.structure.user_defined, out)),
            link!($from."structure.type" -> StructureNode, Forbid, None, |v, out| slot(&v.structure.type_, out)),
        ]
    };
}

const PAGE_SLOTS: [LinkDecl; 7] = slot_links!(Page);
const DEVICE_SLOTS: [LinkDecl; 7] = slot_links!(Device);

/// Drops the referrer (a definition or bond at a removed endpoint).
const DROP: Option<UnlinkFn> = Some(|_, _| None);

use IndexName as I;

/// Every link field of the persisted v0 schema. See the module documentation; `invariants.md` prints the
/// same table.
pub static LINKS: &[LinkDecl] = &[
    // project
    lineage_link!(Project),
    link!(Project."supply.origin.source" -> ExternalRef, Unlink, None,
    |v, out| opt(v.supply.as_ref().and_then(|s| s.origin.as_ref()).and_then(|o| o.source), out),
    Some(|row, del| {
        let AnyRow::Project(p) = row else { return Some(row.clone()) };
        let mut p = (**p).clone();
        if let Some(o) = p.supply.as_mut().and_then(|s| s.origin.as_mut()) {
            if o.source.is_some_and(|s| gone(del, Kind::ExternalRef, s)) {
                o.source = None;
            }
        }
        Some(AnyRow::Project(Arc::new(p)))
    })),
    // import_lineage
    link!(ImportLineage."anchor.copy.of" -> ImportLineage, Forbid, None, |v, out| {
        if let ew_schema::LineageAnchor::Copy { of, .. } = &v.anchor {
            id(*of, out);
        }
    }),
    // structure_node
    link!(StructureNode."parent" -> StructureNode, Forbid, Some(I::StructureChildren), |v, out| opt(v.parent, out)),
    lineage_link!(StructureNode),
    // section
    lineage_link!(Section),
    // external_ref
    link!(ExternalRef."location" -> StructureNode, Forbid, None, |v, out| id(v.location, out)),
    lineage_link!(ExternalRef),
    // page
    link!(Page."folio.section" -> Section, Forbid, Some(I::PagesBySection), |v, out| opt(v.folio.section, out)),
    link!(Page."form" -> Form, Forbid, None, |v, out| opt(v.form, out)),
    PAGE_SLOTS[0],
    PAGE_SLOTS[1],
    PAGE_SLOTS[2],
    PAGE_SLOTS[3],
    PAGE_SLOTS[4],
    PAGE_SLOTS[5],
    PAGE_SLOTS[6],
    lineage_link!(Page),
    // device
    link!(Device."parent" -> Device, Forbid, Some(I::DeviceChildren), |v, out| opt(v.parent, out)),
    link!(Device."parts[].part.copy" -> Part, Forbid, None, |v, out| {
        for p in &v.parts {
            id(p.part.copy, out);
        }
    }),
    link!(Device."strip.accessories[].part.copy" -> Part, Forbid, None, |v, out| {
        if let Some(s) = &v.aspects.strip {
            for a in &s.accessories {
                id(a.part.copy, out);
            }
        }
    }),
    DEVICE_SLOTS[0],
    DEVICE_SLOTS[1],
    DEVICE_SLOTS[2],
    DEVICE_SLOTS[3],
    DEVICE_SLOTS[4],
    DEVICE_SLOTS[5],
    DEVICE_SLOTS[6],
    link!(Device."terminal.strip" -> Device, Forbid, Some(I::TerminalsByStrip),
        |v, out| opt(v.aspects.terminal.as_ref().map(|t| t.strip), out)),
    lineage_link!(Device),
    // function
    link!(Function."device" -> Device, Cascade, Some(I::FunctionsByDevice), |v, out| opt(v.device, out)),
    link!(Function."def" -> Fndef, Forbid, None, |v, out| id(v.def, out)),
    link!(Function."kind.ip.external" -> ExternalRef, Unlink, None,
    |v, out| if let FunctionKind::Ip(ip) = &v.kind { opt(ip.external, out) },
    Some(|row, del| {
        let AnyRow::Function(f) = row else { return Some(row.clone()) };
        let mut f = (**f).clone();
        if let FunctionKind::Ip(ip) = &mut f.kind {
            if ip.external.is_some_and(|e| gone(del, Kind::ExternalRef, e)) {
                ip.external = None;
            }
        }
        Some(AnyRow::Function(Arc::new(f)))
    })),
    lineage_link!(Function),
    // representation
    link!(Representation."functions[]" -> Function, Cascade, Some(I::RepsByFunction), |v, out| {
        for f in &v.functions {
            id(*f, out);
        }
    }),
    link!(Representation."pin_map[].function" -> Function, Cascade, Some(I::RepsByFunction), |v, out| {
        for p in &v.pin_map {
            id(p.function, out);
        }
    }),
    link!(Representation."page" -> Page, Cascade, Some(I::RepsByPage), |v, out| id(v.page, out)),
    link!(Representation."symbol" -> Symbol, Forbid, None, |v, out| id(v.symbol, out)),
    lineage_link!(Representation),
    // connection
    link!(Connection."a.cp[0]" -> Function, Cascade, Some(I::ConnectionsByEndpoint), |v, out| cp(&v.a, out)),
    link!(Connection."a.junction" -> Junction, Cascade, Some(I::ConnectionsByEndpoint), |v, out| junction(&v.a, out)),
    link!(Connection."b.cp[0]" -> Function, Cascade, Some(I::ConnectionsByEndpoint), |v, out| cp(&v.b, out)),
    link!(Connection."b.junction" -> Junction, Cascade, Some(I::ConnectionsByEndpoint), |v, out| junction(&v.b, out)),
    link!(Connection."page" -> Page, Cascade, Some(I::ConnectionsByPage), |v, out| opt(v.page, out)),
    link!(Connection."kind.core" -> Conductor, Unlink, None,
    |v, out| if let ConnectionKind::Core(c) = &v.kind { id(*c, out) },
    Some(|row, del| {
        let AnyRow::Connection(c) = row else { return Some(row.clone()) };
        let mut c = (**c).clone();
        if matches!(c.kind, ConnectionKind::Core(core) if gone(del, Kind::Conductor, core)) {
            c.kind = ConnectionKind::Wire;
        }
        Some(AnyRow::Connection(Arc::new(c)))
    })),
    link!(Connection."kind.bus.cable" -> Device, Unlink, None,
    |v, out| if let ConnectionKind::Bus(b) = &v.kind { opt(b.cable, out) },
    Some(|row, del| {
        let AnyRow::Connection(c) = row else { return Some(row.clone()) };
        let mut c = (**c).clone();
        if let ConnectionKind::Bus(b) = &mut c.kind {
            if b.cable.is_some_and(|d| gone(del, Kind::Device, d)) {
                b.cable = None;
            }
        }
        Some(AnyRow::Connection(Arc::new(c)))
    })),
    link!(Connection."overrides.wire_part.copy" -> Part, Forbid, None,
        |v, out| opt(v.overrides.wire_part.as_ref().map(|p| p.copy), out)),
    lineage_link!(Connection),
    // junction
    link!(Junction."page" -> Page, Cascade, Some(I::JunctionsByPage), |v, out| id(v.page, out)),
    link!(Junction."sequence[]" -> Connection, Unlink, None,
    |v, out| {
        for c in &v.sequence {
            id(*c, out);
        }
    },
    Some(|row, del| {
        let AnyRow::Junction(j) = row else { return Some(row.clone()) };
        let mut j = (**j).clone();
        j.sequence.retain(|c| !gone(del, Kind::Connection, *c));
        Some(AnyRow::Junction(Arc::new(j)))
    })),
    lineage_link!(Junction),
    // conductor
    link!(Conductor."cable" -> Device, Cascade, Some(I::ConductorsByCable), |v, out| id(v.cable, out)),
    lineage_link!(Conductor),
    // potential_def
    link!(PotentialDef."at.cp[0]" -> Function, Unlink, Some(I::PotentialDefsByEndpoint), |v, out| cp(&v.at, out), DROP),
    link!(PotentialDef."at.junction" -> Junction, Unlink, Some(I::PotentialDefsByEndpoint),
        |v, out| junction(&v.at, out), DROP),
    link!(PotentialDef."props.wire_part.copy" -> Part, Forbid, None,
        |v, out| opt(v.props.wire_part.as_ref().map(|p| p.copy), out)),
    lineage_link!(PotentialDef),
    // bond
    link!(Bond."a.cp[0]" -> Function, Unlink, Some(I::BondsByEndpoint), |v, out| cp(&v.a, out), DROP),
    link!(Bond."a.junction" -> Junction, Unlink, Some(I::BondsByEndpoint), |v, out| junction(&v.a, out), DROP),
    link!(Bond."b.cp[0]" -> Function, Unlink, Some(I::BondsByEndpoint), |v, out| cp(&v.b, out), DROP),
    link!(Bond."b.junction" -> Junction, Unlink, Some(I::BondsByEndpoint), |v, out| junction(&v.b, out), DROP),
    link!(Bond."via" -> Connection, Unlink, None, |v, out| opt(v.via, out),
    Some(|row, del| {
        let AnyRow::Bond(b) = row else { return Some(row.clone()) };
        let mut b = (**b).clone();
        if b.via.is_some_and(|c| gone(del, Kind::Connection, c)) {
            b.via = None;
        }
        Some(AnyRow::Bond(Arc::new(b)))
    })),
    lineage_link!(Bond),
    // report_block
    link!(ReportBlock."form" -> Form, Forbid, None, |v, out| id(v.form, out)),
    link!(ReportBlock."section" -> Section, Forbid, None, |v, out| opt(v.section, out)),
    link!(ReportBlock."embedded.page" -> Page, Cascade, None, |v, out| opt(v.embedded.as_ref().map(|e| e.page), out)),
    lineage_link!(ReportBlock),
    // library copies
    link!(Form."title_block" -> Form, Forbid, None, |v, out| opt(v.item.title_block, out)),
    link!(Form."continuation" -> Form, Forbid, None, |v, out| opt(v.item.continuation, out)),
];

/// Every ID-valued schema member that is **not** a link, as `(kind name, field path, reason)`. Each
/// kind's own `id` is identity and not listed.
pub static NOT_LINKS: &[(&str, &str, &str)] = &[
    (
        "import_lineage",
        "namespace",
        "the computed v5 namespace of the lineage (doc 05 §3.2a), not a record",
    ),
    (
        "import_lineage",
        "anchor.qet_project_uuid",
        "a foreign project UUID",
    ),
    (
        "import_lineage",
        "last_run",
        "import_run is a reserved kind without a table in v0",
    ),
    (
        "report_block",
        "manual.include[]",
        "a kind-erased selection; a stale ID selects nothing",
    ),
    (
        "report_block",
        "manual.exclude[]",
        "a kind-erased selection; a stale ID selects nothing",
    ),
    (
        "report_block",
        "source.sql_query.file",
        "the asset ID of a query file, not a record",
    ),
    (
        "part",
        "library_id",
        "library identity of the copy (external library)",
    ),
    (
        "part",
        "item_id",
        "library identity of the copy (external library)",
    ),
    (
        "part",
        "meta.status.deprecated.successor.library",
        "LibRef into an external library",
    ),
    (
        "part",
        "meta.status.deprecated.successor.item",
        "LibRef into an external library",
    ),
    ("part", "manufacturer.library", "LibRef into an external library"),
    ("part", "manufacturer.item", "LibRef into an external library"),
    (
        "part",
        "function_template[].fndef.library",
        "LibRef into an external library",
    ),
    (
        "part",
        "function_template[].fndef.item",
        "LibRef into an external library",
    ),
    (
        "symbol",
        "library_id",
        "library identity of the copy (external library)",
    ),
    (
        "symbol",
        "item_id",
        "library identity of the copy (external library)",
    ),
    (
        "symbol",
        "meta.status.deprecated.successor.library",
        "LibRef into an external library",
    ),
    (
        "symbol",
        "meta.status.deprecated.successor.item",
        "LibRef into an external library",
    ),
    ("symbol", "fndef.library", "LibRef into an external library"),
    ("symbol", "fndef.item", "LibRef into an external library"),
    (
        "fndef",
        "library_id",
        "library identity of the copy (external library)",
    ),
    (
        "fndef",
        "item_id",
        "library identity of the copy (external library)",
    ),
    (
        "fndef",
        "meta.status.deprecated.successor.library",
        "LibRef into an external library",
    ),
    (
        "fndef",
        "meta.status.deprecated.successor.item",
        "LibRef into an external library",
    ),
    (
        "form",
        "library_id",
        "library identity of the copy (external library)",
    ),
    (
        "form",
        "item_id",
        "library identity of the copy (external library)",
    ),
    (
        "form",
        "meta.status.deprecated.successor.library",
        "LibRef into an external library",
    ),
    (
        "form",
        "meta.status.deprecated.successor.item",
        "LibRef into an external library",
    ),
];

/// The links whose target is `kind`, with their positions.
pub(crate) fn links_to(kind: Kind) -> impl Iterator<Item = (LinkId, &'static LinkDecl)> {
    LINKS.iter().enumerate().filter(move |(_, l)| l.to == kind)
}

/// The links held by records of `kind`, with their positions.
pub(crate) fn links_from(kind: Kind) -> impl Iterator<Item = (LinkId, &'static LinkDecl)> {
    LINKS.iter().enumerate().filter(move |(_, l)| l.from == kind)
}
