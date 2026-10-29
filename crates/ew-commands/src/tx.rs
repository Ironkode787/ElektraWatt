// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`TxBuilder`]: the scratch snapshot with read-your-writes that a command and the normalisers write
//! through (ADR-0008, ARCH §5.2), and the [`ReadSet`] it records for the optimistic rebase of previews.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::{EntropyError, Id, IdSource, UnixTime, Uuid, V7Generator};
use ew_model::{AnyKey, AnyRow, Asset, EndpointKey, Entity, Model, Op, Rev, RowId};
use ew_schema::{
    AnyId, AssetError, AssetRef, Bond, Channel, Conductor, Connection, Device, Endpoint, ExternalRef, FnDef,
    FormDef, Function, ImportLineage, Junction, Kind, LibCopy, MacroInstance, Page, Part, PotentialDef,
    Project, ReportBlock, Representation, Section, Sha256, StructureNode, SymbolDef,
};
use sha2::Digest;

use crate::error::CmdError;

/// An index read of a transaction: which base index was asked for which key. The read set stores the
/// IDs the base index answered, and the rebase check asks again.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum IndexRead {
    /// `Function.device` → functions.
    FunctionsByDevice(Id<Device>),
    /// `Representation.functions` / `pin_map[].function` → representations.
    RepsByFunction(Id<Function>),
    /// `Representation.page` → representations.
    RepsByPage(Id<Page>),
    /// `Connection.a`/`b` at one endpoint → connections.
    ConnectionsAt(EndpointKey),
    /// `Connection.a`/`b` at any CP of a function → connections.
    ConnectionsOfFunction(Id<Function>),
    /// `Connection.page` → connections.
    ConnectionsByPage(Id<Page>),
    /// `Junction.page` → junctions.
    JunctionsByPage(Id<Page>),
    /// `Terminal.strip` → terminal devices.
    TerminalsByStrip(Id<Device>),
    /// `Conductor.cable` → conductors.
    ConductorsByCable(Id<Device>),
    /// `Page.folio.section` → pages.
    PagesBySection(Id<Section>),
    /// `PotentialDef.at` → potential definitions.
    PotentialDefsAt(EndpointKey),
    /// `Bond.a`/`b` → bonds.
    BondsAt(EndpointKey),
    /// `Channel.module` → channels (the generic referrer index of the link).
    ChannelsOfModule(Id<Device>),
    /// `Channel.binding` at any CP of a function → channels (the generic referrer index of the link).
    ChannelsAtFunction(Id<Function>),
    /// `PageItem.page` → page items.
    PageItemsByPage(Id<Page>),
    /// `StructureBox.page` → structure boxes.
    StructureBoxesByPage(Id<Page>),
    /// `PageItem.kind.wire_marker.connections[]` → the wire markers listing a connection (the generic referrer
    /// index of the link).
    MarkersOfConnection(Id<Connection>),
    /// `MacroInstance.members[][1]` → the macro instances listing a record as a member (the generic referrer index
    /// of the kind-erased link, keyed under its nominal kind `Page`).
    MacroInstancesOfMember(AnyId),
}

/// The referrers of `target` through the link `from.field`, in ID order (the generic referrer index serves
/// the links that have no named index of their own).
fn referrers_through(idx: &ew_model::BaseIndexes, from: Kind, field: &str, target: AnyKey) -> Vec<AnyId> {
    let link = ew_model::LINKS
        .iter()
        .position(|l| l.from == from && l.field == field);
    // a misspelt field name would answer nothing, silently: the tests below name every link this file reads
    debug_assert!(
        link.is_some(),
        "the link {from:?}.{field} is not declared in ew_model::LINKS"
    );
    let Some(link) = link else {
        return Vec::new();
    };
    idx.referrers()
        .get(&target)
        .filter(|(l, _)| *l == link)
        .map(|(_, src)| *src)
        .collect()
}

impl IndexRead {
    /// What the base index of `model` answers, in ID order.
    fn base_ids(self, model: &Model) -> Vec<AnyId> {
        let idx = model.indexes();
        let ids: Vec<AnyId> = match self {
            IndexRead::FunctionsByDevice(d) => idx.functions_by_device().get(&d).map(|v| v.cast()).collect(),
            IndexRead::RepsByFunction(f) => idx.reps_by_function().get(&f).map(|v| v.cast()).collect(),
            IndexRead::RepsByPage(p) => idx.reps_by_page().get(&p).map(|v| v.cast()).collect(),
            IndexRead::ConnectionsAt(e) => idx.connections_by_endpoint().get(&e).map(|v| v.cast()).collect(),
            IndexRead::ConnectionsOfFunction(f) => {
                let set: BTreeSet<AnyId> = idx
                    .connections_by_endpoint()
                    .range(EndpointKey::cps_of(f))
                    .flat_map(|(_, b)| b.values().map(|v| v.cast()))
                    .collect();
                set.into_iter().collect()
            }
            IndexRead::ConnectionsByPage(p) => idx.connections_by_page().get(&p).map(|v| v.cast()).collect(),
            IndexRead::JunctionsByPage(p) => idx.junctions_by_page().get(&p).map(|v| v.cast()).collect(),
            IndexRead::TerminalsByStrip(d) => idx.terminals_by_strip().get(&d).map(|v| v.cast()).collect(),
            IndexRead::ConductorsByCable(d) => idx.conductors_by_cable().get(&d).map(|v| v.cast()).collect(),
            IndexRead::PagesBySection(s) => idx.pages_by_section().get(&s).map(|v| v.cast()).collect(),
            IndexRead::PotentialDefsAt(e) => idx
                .potential_defs_by_endpoint()
                .get(&e)
                .map(|v| v.cast())
                .collect(),
            IndexRead::BondsAt(e) => idx.bonds_by_endpoint().get(&e).map(|v| v.cast()).collect(),
            IndexRead::ChannelsOfModule(m) => {
                referrers_through(idx, Kind::Channel, "module", (Kind::Device, m.cast()))
            }
            IndexRead::ChannelsAtFunction(f) => {
                referrers_through(idx, Kind::Channel, "binding.cp[0]", (Kind::Function, f.cast()))
            }
            IndexRead::PageItemsByPage(p) => idx.page_items_by_page().get(&p).map(|v| v.cast()).collect(),
            IndexRead::StructureBoxesByPage(p) => {
                idx.structure_boxes_by_page().get(&p).map(|v| v.cast()).collect()
            }
            IndexRead::MarkersOfConnection(c) => referrers_through(
                idx,
                Kind::PageItem,
                "kind.wire_marker.connections[]",
                (Kind::Connection, c.cast()),
            ),
            IndexRead::MacroInstancesOfMember(m) => {
                referrers_through(idx, Kind::MacroInstance, "members[][1]", (Kind::Page, m))
            }
        };
        ids
    }
}

/// The base reads of a transaction (ADR-0008 "jobs return prepared transactions with their read set"):
/// rows, index lookups and whole-table scans. A prepared transaction may be applied to a later snapshot
/// only if none of them changed in between ([`ReadSet::unchanged_since`]).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ReadSet {
    /// Row keys, with whether the row existed in the base.
    rows: BTreeMap<AnyKey, bool>,
    /// Index lookups with the IDs the base index answered.
    indexes: BTreeMap<IndexRead, Vec<AnyId>>,
    /// Kinds whose whole table was scanned.
    tables: BTreeSet<Kind>,
}

/// `changed_at` of a row, kind-erased; `None` when the row does not exist.
fn row_changed_at(model: &Model, key: &AnyKey) -> Option<Rev> {
    fn at<T: Entity>(model: &Model, id: AnyId) -> Option<Rev> {
        model.table::<T>().row(&id.cast()).map(|r| r.changed_at())
    }
    let (kind, id) = *key;
    match kind {
        Kind::Project => {
            let row = model.project_row();
            (row.value().id.cast::<()>() == id).then(|| row.changed_at())
        }
        Kind::PlcConfigProject => at::<ew_schema::PlcConfigProject>(model, id),
        Kind::ImportLineage => at::<ImportLineage>(model, id),
        Kind::StructureNode => at::<StructureNode>(model, id),
        Kind::Section => at::<Section>(model, id),
        Kind::ExternalRef => at::<ExternalRef>(model, id),
        Kind::Page => at::<Page>(model, id),
        Kind::Device => at::<Device>(model, id),
        Kind::Function => at::<Function>(model, id),
        Kind::Representation => at::<Representation>(model, id),
        Kind::Connection => at::<Connection>(model, id),
        Kind::Junction => at::<Junction>(model, id),
        Kind::Conductor => at::<Conductor>(model, id),
        Kind::Channel => at::<ew_schema::Channel>(model, id),
        Kind::PotentialDef => at::<PotentialDef>(model, id),
        Kind::Bond => at::<Bond>(model, id),
        Kind::DeviceLink => at::<ew_schema::DeviceLink>(model, id),
        Kind::PageItem => at::<ew_schema::PageItem>(model, id),
        Kind::StructureBox => at::<ew_schema::StructureBox>(model, id),
        Kind::ReportBlock => at::<ReportBlock>(model, id),
        Kind::MacroInstance => at::<ew_schema::MacroInstance>(model, id),
        Kind::Revision => at::<ew_schema::Revision>(model, id),
        Kind::Part => at::<LibCopy<Part>>(model, id),
        Kind::Symbol => at::<LibCopy<SymbolDef>>(model, id),
        Kind::Fndef => at::<LibCopy<FnDef>>(model, id),
        Kind::Form => at::<LibCopy<FormDef>>(model, id),
        Kind::Macro => at::<LibCopy<ew_schema::MacroDef>>(model, id),
        Kind::Manufacturer => at::<LibCopy<ew_schema::Manufacturer>>(model, id),
        _ => None,
    }
}

/// `changed_at` of a whole table; kinds without a table never change.
fn table_changed_at(model: &Model, kind: Kind) -> Rev {
    fn at<T: Entity>(model: &Model) -> Rev {
        model.table::<T>().changed_at()
    }
    match kind {
        Kind::Project => model.project_row().changed_at(),
        Kind::PlcConfigProject => at::<ew_schema::PlcConfigProject>(model),
        Kind::ImportLineage => at::<ImportLineage>(model),
        Kind::StructureNode => at::<StructureNode>(model),
        Kind::Section => at::<Section>(model),
        Kind::ExternalRef => at::<ExternalRef>(model),
        Kind::Page => at::<Page>(model),
        Kind::Device => at::<Device>(model),
        Kind::Function => at::<Function>(model),
        Kind::Representation => at::<Representation>(model),
        Kind::Connection => at::<Connection>(model),
        Kind::Junction => at::<Junction>(model),
        Kind::Conductor => at::<Conductor>(model),
        Kind::Channel => at::<ew_schema::Channel>(model),
        Kind::PotentialDef => at::<PotentialDef>(model),
        Kind::Bond => at::<Bond>(model),
        Kind::DeviceLink => at::<ew_schema::DeviceLink>(model),
        Kind::PageItem => at::<ew_schema::PageItem>(model),
        Kind::StructureBox => at::<ew_schema::StructureBox>(model),
        Kind::ReportBlock => at::<ReportBlock>(model),
        Kind::MacroInstance => at::<ew_schema::MacroInstance>(model),
        Kind::Revision => at::<ew_schema::Revision>(model),
        Kind::Part => at::<LibCopy<Part>>(model),
        Kind::Symbol => at::<LibCopy<SymbolDef>>(model),
        Kind::Fndef => at::<LibCopy<FnDef>>(model),
        Kind::Form => at::<LibCopy<FormDef>>(model),
        Kind::Macro => at::<LibCopy<ew_schema::MacroDef>>(model),
        Kind::Manufacturer => at::<LibCopy<ew_schema::Manufacturer>>(model),
        _ => Rev::ZERO,
    }
}

impl ReadSet {
    /// Whether nothing was read.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.indexes.is_empty() && self.tables.is_empty()
    }

    /// The number of recorded reads.
    pub fn len(&self) -> usize {
        self.rows.len() + self.indexes.len() + self.tables.len()
    }

    /// The row keys read, in key order.
    pub fn rows(&self) -> impl Iterator<Item = &AnyKey> + '_ {
        self.rows.keys()
    }

    /// Whether every read gives the same answer on `model` as on the snapshot of revision `since` the
    /// reads were made on: no read row was inserted, changed or removed after `since`, every index lookup
    /// answers the same IDs, and no scanned table changed after `since`.
    pub fn unchanged_since(&self, model: &Model, since: Rev) -> bool {
        self.rows
            .iter()
            .all(|(key, present)| match row_changed_at(model, key) {
                Some(rev) => *present && rev <= since,
                None => !*present,
            })
            && self
                .indexes
                .iter()
                .all(|(read, ids)| read.base_ids(model) == *ids)
            && self
                .tables
                .iter()
                .all(|kind| table_changed_at(model, *kind) <= since)
    }
}

/// Lends a `&mut dyn IdSource` to a [`V7Generator`].
struct ByRef<'s>(&'s mut dyn IdSource);

impl IdSource for ByRef<'_> {
    fn now(&mut self) -> UnixTime {
        self.0.now()
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        self.0.fill(buf)
    }
}

/// The first key of a kind in key order.
fn first_key(kind: Kind) -> AnyKey {
    (kind, Id::from_uuid(Uuid::nil()))
}

/// The scratch snapshot of one transaction: a base [`Model`] plus pending row puts, with read-your-writes.
///
/// - Reads (`get`, `iter`, the index lookups) see the pending puts and record what they read from the base
///   in the [`ReadSet`].
/// - Writes (`put`, `insert`, `update`, `delete`) go to the pending map; `delete` includes the cascade and
///   unlink ops of the delete policies (`Model::delete_plan`).
/// - New IDs come from [`TxBuilder::mint`], which draws on the session's injected [`IdSource`], so a
///   seeded source makes a command sequence reproducible (ADR-0008).
/// - [`TxBuilder::ops`] turns the pending puts into one `Op` per changed key, in key order, with `before`
///   taken from the base.
///
/// Only the session's commit pipeline constructs one ([`crate::Engine`]).
pub struct TxBuilder<'a> {
    base: &'a Model,
    pending: BTreeMap<AnyKey, Option<AnyRow>>,
    inserted: Vec<AnyKey>,
    ids: &'a mut dyn IdSource,
    reads: RefCell<ReadSet>,
    source: &'static str,
    /// The planning snapshot of [`TxBuilder::delete_many`]: the base with the pending puts applied
    /// without invariants (`Model::apply_unchecked`), kept up to date incrementally.
    scratch: Option<Model>,
    /// The keys written since `scratch` was last brought up to date.
    stale: BTreeSet<AnyKey>,
    /// The notices of the normalisers ([`TxBuilder::notice`]), in first-emission order, without repeats.
    notices: Vec<String>,
    /// The assets this transaction brings ([`TxBuilder::put_asset`]), by hash.
    assets: BTreeMap<Sha256, Asset>,
}

impl std::fmt::Debug for TxBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxBuilder")
            .field("base_rev", &self.base.rev())
            .field("pending", &self.pending.len())
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<'a> TxBuilder<'a> {
    /// A builder over `base` drawing new IDs from `ids`.
    pub(crate) fn new(base: &'a Model, ids: &'a mut dyn IdSource) -> TxBuilder<'a> {
        TxBuilder {
            base,
            pending: BTreeMap::new(),
            inserted: Vec::new(),
            ids,
            reads: RefCell::new(ReadSet::default()),
            source: "",
            scratch: None,
            stale: BTreeSet::new(),
            notices: Vec::new(),
            assets: BTreeMap::new(),
        }
    }

    /// Records a notice for the command's outcome (`Outcome::notices`): what a normaliser left undone and
    /// why (a coil that stays unbound, surplus channels kept). A notice equal to one already recorded is
    /// dropped, so a normaliser that runs in several passes reports a condition once.
    pub fn notice(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !self.notices.contains(&text) {
            self.notices.push(text);
        }
    }

    /// Takes the recorded notices.
    pub(crate) fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    /// Names the command or normaliser now writing, for [`TxBuilder::reject`].
    pub(crate) fn set_source(&mut self, source: &'static str) {
        self.source = source;
    }

    /// The command or normaliser now writing.
    pub fn source(&self) -> &'static str {
        self.source
    }

    /// The base snapshot's revision.
    pub fn base_rev(&self) -> Rev {
        self.base.rev()
    }

    /// The base snapshot itself, for comparisons with the state before the transaction (online numbering
    /// compares base and candidate collisions). Reads through it are **not** recorded in the read set:
    /// use it only for facts that the transaction's recorded reads also depend on.
    pub(crate) fn base_model(&self) -> &'a Model {
        self.base
    }

    // -----------------------------------------------------------------------------------------------------
    // Reads

    fn note_row(&self, key: AnyKey, present: bool) {
        self.reads.borrow_mut().rows.entry(key).or_insert(present);
    }

    /// The current row of a key (pending put, else base), kind-erased.
    pub fn row(&self, key: &AnyKey) -> Option<AnyRow> {
        if let Some(p) = self.pending.get(key) {
            return p.clone();
        }
        let row = self.base.row(key);
        self.note_row(*key, row.is_some());
        row
    }

    /// The base row of a key, ignoring pending puts (normalisers compare old and new endpoints).
    pub fn base_row(&self, key: &AnyKey) -> Option<AnyRow> {
        let row = self.base.row(key);
        self.note_row(*key, row.is_some());
        row
    }

    /// The current record of kind `T` with this ID.
    pub fn get<T: Entity>(&self, id: RowId<T>) -> Option<Arc<T>> {
        self.row(&T::key(id)).and_then(|r| r.get::<T>().cloned())
    }

    /// The current record, or [`CmdError::NotFound`].
    pub fn require<T: Entity>(&self, id: RowId<T>) -> Result<Arc<T>, CmdError> {
        self.get(id).ok_or(CmdError::NotFound { key: T::key(id) })
    }

    /// Whether a record of kind `T` with this ID exists.
    pub fn exists<T: Entity>(&self, id: RowId<T>) -> bool {
        self.row(&T::key(id)).is_some()
    }

    /// The project record, with read-your-writes over [`TxBuilder::set_project`].
    pub fn project(&self) -> Arc<Project> {
        let key = (Kind::Project, self.base.project().id.cast::<()>());
        if let Some(Some(AnyRow::Project(p))) = self.pending.get(&key) {
            return Arc::clone(p);
        }
        self.note_row(key, true);
        Arc::clone(self.base.project())
    }

    /// Replaces the project record (an update of the project row: its ID never changes, it is never
    /// inserted or removed). A record equal to the current one puts nothing.
    pub fn set_project(&mut self, project: Project) -> Result<(), CmdError> {
        let current = self.project();
        if project.id != current.id {
            return Err(CmdError::internal(format!(
                "{}: the project record cannot be re-keyed",
                self.source
            )));
        }
        if project != *current {
            self.put_row(AnyRow::Project(Arc::new(project)));
        }
        Ok(())
    }

    /// Every current record of kind `T`, in ID order. Records a whole-table read.
    pub fn iter<T: Entity>(&self) -> Vec<Arc<T>> {
        self.reads.borrow_mut().tables.insert(T::KIND);
        let mut out: BTreeMap<RowId<T>, Arc<T>> = self
            .base
            .table::<T>()
            .iter()
            .filter(|(id, _)| !self.pending.contains_key(&T::key(**id)))
            .map(|(id, row)| (*id, Arc::clone(row)))
            .collect();
        for (key, row) in self.pending_of(T::KIND) {
            if let Some(r) = row.as_ref().and_then(|r| r.get::<T>()) {
                out.insert(key.1.cast(), Arc::clone(r));
            }
        }
        out.into_values().collect()
    }

    fn pending_of(&self, kind: Kind) -> impl Iterator<Item = (&AnyKey, &Option<AnyRow>)> + '_ {
        self.pending
            .range(first_key(kind)..)
            .take_while(move |(k, _)| k.0 == kind)
    }

    /// An index lookup with read-your-writes: the base index's answer for base rows without a pending put,
    /// plus every pending row of kind `T` that satisfies `pred`.
    fn indexed<T: Entity>(&self, read: IndexRead, pred: impl Fn(&T) -> bool) -> Vec<RowId<T>> {
        let base = read.base_ids(self.base);
        let mut out: BTreeSet<RowId<T>> = base
            .iter()
            .filter(|id| !self.pending.contains_key(&(T::KIND, **id)))
            .map(|id| id.cast())
            .collect();
        self.reads.borrow_mut().indexes.entry(read).or_insert(base);
        for (key, row) in self.pending_of(T::KIND) {
            if let Some(r) = row.as_ref().and_then(|r| r.get::<T>()) {
                if pred(r) {
                    out.insert(key.1.cast());
                }
            }
        }
        out.into_iter().collect()
    }

    /// The functions of a device.
    pub fn functions_of_device(&self, device: Id<Device>) -> Vec<Id<Function>> {
        self.indexed::<Function>(IndexRead::FunctionsByDevice(device), |f| f.device == Some(device))
    }

    /// The representations that draw a function.
    pub fn reps_of_function(&self, function: Id<Function>) -> Vec<Id<Representation>> {
        self.indexed::<Representation>(IndexRead::RepsByFunction(function), |r| {
            r.functions.contains(&function) || r.pin_map.as_slice().iter().any(|p| p.function == function)
        })
    }

    /// The representations on a page.
    pub fn reps_on_page(&self, page: Id<Page>) -> Vec<Id<Representation>> {
        self.indexed::<Representation>(IndexRead::RepsByPage(page), |r| r.page == page)
    }

    /// The connections ending at an endpoint, in ID order.
    pub fn connections_at(&self, at: EndpointKey) -> Vec<Id<Connection>> {
        self.indexed::<Connection>(IndexRead::ConnectionsAt(at), |c| {
            EndpointKey::of(&c.a) == Some(at) || EndpointKey::of(&c.b) == Some(at)
        })
    }

    /// The connections ending at any CP of a function.
    pub fn connections_of_function(&self, function: Id<Function>) -> Vec<Id<Connection>> {
        self.indexed::<Connection>(IndexRead::ConnectionsOfFunction(function), |c| {
            c.a.function() == Some(function) || c.b.function() == Some(function)
        })
    }

    /// The connections drawn on a page.
    pub fn connections_on_page(&self, page: Id<Page>) -> Vec<Id<Connection>> {
        self.indexed::<Connection>(IndexRead::ConnectionsByPage(page), |c| c.page == Some(page))
    }

    /// The junctions on a page.
    pub fn junctions_on_page(&self, page: Id<Page>) -> Vec<Id<Junction>> {
        self.indexed::<Junction>(IndexRead::JunctionsByPage(page), |j| j.page == page)
    }

    /// The terminal devices of a strip.
    pub fn terminals_of_strip(&self, strip: Id<Device>) -> Vec<Id<Device>> {
        self.indexed::<Device>(IndexRead::TerminalsByStrip(strip), |d| {
            d.aspects.terminal.as_ref().is_some_and(|t| t.strip == strip)
        })
    }

    /// The conductors of a cable.
    pub fn conductors_of_cable(&self, cable: Id<Device>) -> Vec<Id<Conductor>> {
        self.indexed::<Conductor>(IndexRead::ConductorsByCable(cable), |c| c.cable == cable)
    }

    /// The pages of a section.
    pub fn pages_in_section(&self, section: Id<Section>) -> Vec<Id<Page>> {
        self.indexed::<Page>(IndexRead::PagesBySection(section), |p| {
            p.folio.section == Some(section)
        })
    }

    /// The potential definitions at an endpoint.
    pub fn potential_defs_at(&self, at: EndpointKey) -> Vec<Id<PotentialDef>> {
        self.indexed::<PotentialDef>(IndexRead::PotentialDefsAt(at), |d| {
            EndpointKey::of(&d.at) == Some(at)
        })
    }

    /// The bonds at an endpoint.
    pub fn bonds_at(&self, at: EndpointKey) -> Vec<Id<Bond>> {
        self.indexed::<Bond>(IndexRead::BondsAt(at), |b| {
            EndpointKey::of(&b.a) == Some(at) || EndpointKey::of(&b.b) == Some(at)
        })
    }

    /// The channels of a module, in ID order (FT-319: an index lookup, not a scan of the channel table).
    pub fn channels_of_module(&self, module: Id<Device>) -> Vec<Id<Channel>> {
        self.indexed::<Channel>(IndexRead::ChannelsOfModule(module), |c| c.module == module)
    }

    /// The channels bound to CP `idx` of `function`, in ID order (FT-319: an index lookup, not a scan).
    pub fn channels_at_cp(&self, function: Id<Function>, idx: u16) -> Vec<Id<Channel>> {
        self.indexed::<Channel>(
            IndexRead::ChannelsAtFunction(function),
            |c| matches!(c.binding, Some(Endpoint::Cp(f, _)) if f == function),
        )
        .into_iter()
        .filter(|id| {
            self.get::<Channel>(*id)
                .is_some_and(|c| c.binding == Some(Endpoint::Cp(function, idx)))
        })
        .collect()
    }

    /// The page items of a page, in ID order.
    pub fn page_items_on_page(&self, page: Id<Page>) -> Vec<Id<ew_schema::PageItem>> {
        self.indexed::<ew_schema::PageItem>(IndexRead::PageItemsByPage(page), |i| i.page == page)
    }

    /// The structure boxes of a page, in ID order.
    pub fn structure_boxes_on_page(&self, page: Id<Page>) -> Vec<Id<ew_schema::StructureBox>> {
        self.indexed::<ew_schema::StructureBox>(IndexRead::StructureBoxesByPage(page), |b| b.page == page)
    }

    /// The wire markers that list a connection, in ID order.
    pub fn markers_of_connection(&self, connection: Id<Connection>) -> Vec<Id<ew_schema::PageItem>> {
        self.indexed::<ew_schema::PageItem>(IndexRead::MarkersOfConnection(connection), |i| {
            i.kind
                .wire_marker()
                .is_some_and(|m| m.connections.contains(&connection))
        })
    }

    /// The macro instances that list `member` (a record of a kind of [`ew_model::MACRO_MEMBER_KINDS`]) among their
    /// members, in ID order.
    pub fn macro_instances_of_member(&self, member: AnyId) -> Vec<Id<MacroInstance>> {
        self.indexed::<MacroInstance>(IndexRead::MacroInstancesOfMember(member), |mi| {
            mi.members.iter().any(|(_, created)| *created == member)
        })
    }

    /// The next free counter for `letter` in `scope` (FR-NUM-007); see [`crate::numbering`].
    pub fn next_free_counter(&self, letter: &ew_mltext::Ident, scope: crate::numbering::CounterScope) -> u32 {
        crate::numbering::next_free_counter(self, letter, scope, None)
    }

    // -----------------------------------------------------------------------------------------------------
    // Writes

    /// A new UUIDv7 from the session's ID source.
    pub fn mint<T>(&mut self) -> Id<T> {
        V7Generator::new(ByRef(&mut *self.ids)).next()
    }

    /// Puts a kind-erased row under its key (insert or replace).
    pub fn put_row(&mut self, row: AnyRow) {
        let key = row.key();
        if !self.pending.contains_key(&key) {
            let present = self.base.row(&key).is_some();
            self.note_row(key, present);
        }
        self.stale.insert(key);
        self.pending.insert(key, Some(row));
    }

    /// Puts a record under its ID (insert or replace).
    pub fn put<T: Entity>(&mut self, record: T) {
        self.put_row(T::wrap(Arc::new(record)));
    }

    /// Inserts a new record; its ID must be free.
    pub fn insert<T: Entity>(&mut self, record: T) -> Result<RowId<T>, CmdError> {
        let id = record.id();
        let key = T::key(id);
        if self.row(&key).is_some() {
            return Err(CmdError::internal(format!(
                "{}: insert of an existing record {}:{}",
                self.source, key.0, key.1
            )));
        }
        self.put(record);
        self.inserted.push(key);
        Ok(id)
    }

    /// Changes a record in place.
    pub fn update<T: Entity>(&mut self, id: RowId<T>, f: impl FnOnce(&mut T)) -> Result<(), CmdError> {
        let current = self.require::<T>(id)?;
        let mut next = (*current).clone();
        f(&mut next);
        if next != *current {
            self.put(next);
        }
        Ok(())
    }

    /// Removes a row without consulting the delete policies. The caller guarantees that nothing still
    /// names it at the end of the transaction (the structural invariants check it anyway, and so does the
    /// delete-time rule EW-INV-021 for a library copy named through a `LibRef`), with one exception it takes care
    /// of itself: a record of a kind of [`ew_model::MACRO_MEMBER_KINDS`] leaves every macro instance that lists it
    /// (the unlink policy of `macro_instance.members[][1]`, WP-S1.19 §6.1), so the normalisers' direct removals
    /// (a dissolved junction, a merged connection, an emptied wire marker) do not each need to know the link.
    pub fn remove_row(&mut self, key: AnyKey) {
        if !self.pending.contains_key(&key) {
            let present = self.base.row(&key).is_some();
            self.note_row(key, present);
        }
        self.stale.insert(key);
        self.pending.insert(key, None);
        if ew_model::MACRO_MEMBER_KINDS.contains(&key.0) {
            self.unlink_macro_member(key.1);
        }
    }

    /// Takes `member` out of every macro instance that lists it; the instance stays, even when left empty.
    fn unlink_macro_member(&mut self, member: AnyId) {
        for mi in self.macro_instances_of_member(member) {
            let Some(row) = self.get::<MacroInstance>(mi) else {
                continue;
            };
            let mut next = (*row).clone();
            next.members.retain(|(_, created)| *created != member);
            if next != *row {
                self.put(next);
            }
        }
    }

    /// Takes `connection` out of the wire markers that list it, except those on `keep_on` (the page it lies on
    /// now). With `carrier` (the connection that carries the wire on, e.g. a dissolved junction's kept member), a
    /// marker on the carrier's page lists the carrier in its place unless it lists it already. A marker left empty
    /// is removed: the unlink policy of the marker list (EW-INV-023).
    pub(crate) fn unlink_markers(
        &mut self,
        connection: Id<Connection>,
        carrier: Option<Id<Connection>>,
        keep_on: Option<Id<Page>>,
    ) {
        for m in self.markers_of_connection(connection) {
            let Some(item) = self.get::<ew_schema::PageItem>(m) else {
                continue;
            };
            if keep_on == Some(item.page) {
                continue;
            }
            let carrier = carrier.filter(|k| {
                self.get::<Connection>(*k)
                    .is_some_and(|k| k.page == Some(item.page))
            });
            let mut next = (*item).clone();
            if let ew_schema::PageItemKind::WireMarker(w) = &mut next.kind {
                let at = w.connections.iter().position(|c| *c == connection);
                w.connections.retain(|c| *c != connection);
                if let (Some(k), Some(at)) = (carrier, at) {
                    if !w.connections.contains(&k) {
                        w.connections.insert(at, k);
                    }
                }
                if w.connections.is_empty() {
                    self.remove_row((Kind::PageItem, m.cast()));
                    continue;
                }
            }
            self.put(next);
        }
    }

    /// Removes a record with everything the delete policies take along (doc 03 §4.2): the ops of
    /// `Model::delete_plan`, computed on the current state of the transaction. See
    /// [`TxBuilder::delete_many`].
    pub fn delete(&mut self, key: AnyKey) -> Result<(), CmdError> {
        self.delete_many(&[key])
    }

    /// Removes several records in **one** delete plan (`Model::delete_plan_many`), so the result does not
    /// depend on their order and a record that another one's cascade removes is planned once.
    ///
    /// The plan is computed on the current state of the transaction: the base with the pending puts
    /// applied **without** the structural invariants (`Model::apply_unchecked`), because transient states
    /// inside a transaction are legal (ADR-0009 decision 1). The candidate is validated once, at the end
    /// of the transaction, by `Model::apply_with`. The planning snapshot is kept and brought up to date
    /// with only the keys written since the previous plan. Every key must exist now
    /// ([`CmdError::NotFound`]).
    pub fn delete_many(&mut self, keys: &[AnyKey]) -> Result<(), CmdError> {
        for key in keys {
            if self.row(key).is_none() {
                return Err(CmdError::NotFound { key: *key });
            }
        }
        if keys.is_empty() {
            return Ok(());
        }
        let plan = self.planning_snapshot()?.delete_plan_many(keys)?;
        for op in plan {
            let k = op.key();
            if !self.pending.contains_key(&k) {
                let present = self.base.row(&k).is_some();
                self.note_row(k, present);
            }
            self.stale.insert(k);
            self.pending.insert(k, op.after().cloned());
        }
        Ok(())
    }

    /// The base with every pending put applied, without invariants; brought up to date incrementally.
    fn planning_snapshot(&mut self) -> Result<&Model, CmdError> {
        let scratch = match self.scratch.take() {
            None if self.pending.is_empty() => self.base.clone(),
            None => {
                let ops = self.ops();
                self.base
                    .apply_unchecked(&ops, self.base.rev().next())
                    .map_err(|e| self.planning_error(&e))?
            }
            Some(scratch) => {
                let ops: Vec<Op> = self
                    .stale
                    .iter()
                    .filter_map(|key| {
                        let before = scratch.row(key);
                        let after = self.pending.get(key).cloned().flatten();
                        (!same_opt(before.as_ref(), after.as_ref())).then_some(Op::Put {
                            key: *key,
                            before,
                            after,
                        })
                    })
                    .collect();
                if ops.is_empty() {
                    scratch
                } else {
                    scratch
                        .apply_unchecked(&ops, scratch.rev().next())
                        .map_err(|e| self.planning_error(&e))?
                }
            }
        };
        self.stale.clear();
        Ok(self.scratch.insert(scratch))
    }

    fn planning_error(&self, e: &ew_model::ModelError) -> CmdError {
        CmdError::internal(format!(
            "{}: the planning snapshot of the transaction cannot be built ({e})",
            self.source
        ))
    }

    /// Removes a record if it still exists ([`TxBuilder::delete`]); a record already removed in this
    /// transaction is skipped.
    pub fn delete_if_exists(&mut self, key: AnyKey) -> Result<(), CmdError> {
        if self.row(&key).is_none() {
            return Ok(());
        }
        self.delete(key)
    }

    /// A refusal by the current command or normaliser: `return tx.reject("cause; remedy")`.
    pub fn reject<T>(&self, reason: impl Into<String>) -> Result<T, CmdError> {
        Err(CmdError::rejected(self.source, reason))
    }

    // -----------------------------------------------------------------------------------------------------
    // State

    /// The keys whose current row differs from the base, in key order.
    pub fn changed_keys(&self) -> Vec<AnyKey> {
        self.pending
            .iter()
            .filter(|(key, after)| {
                let before = self.base.row(key);
                !same_opt(before.as_ref(), after.as_ref())
            })
            .map(|(key, _)| *key)
            .collect()
    }

    /// The changed keys of one kind.
    pub fn changed_of(&self, kind: Kind) -> Vec<AnyKey> {
        self.pending_of(kind)
            .filter(|(key, after)| !same_opt(self.base.row(key).as_ref(), after.as_ref()))
            .map(|(key, _)| *key)
            .collect()
    }

    /// Whether the key names a row that this transaction created (absent in the base, present now).
    pub fn is_new(&self, key: &AnyKey) -> bool {
        matches!(self.pending.get(key), Some(Some(_))) && self.base.row(key).is_none()
    }

    /// The keys inserted with [`TxBuilder::insert`], in insertion order, that still exist.
    pub fn inserted(&self) -> Vec<AnyKey> {
        self.inserted
            .iter()
            .filter(|k| matches!(self.pending.get(*k), Some(Some(_))))
            .copied()
            .collect()
    }

    /// The pending puts as ops: one per changed key, in key order, `before` from the base.
    pub fn ops(&self) -> Vec<Op> {
        self.pending
            .iter()
            .filter_map(|(key, after)| {
                let before = self.base.row(key);
                if same_opt(before.as_ref(), after.as_ref()) {
                    return None;
                }
                Some(Op::Put {
                    key: *key,
                    before,
                    after: after.clone(),
                })
            })
            .collect()
    }

    // -----------------------------------------------------------------------------------------------------
    // Assets

    /// Brings image bytes into the project's asset store (doc 05 §13.3): sniffs their format and intrinsic
    /// size, applies the size limits ([`ew_schema::sniff`]) and hashes them. The reference it returns is what a
    /// record stores; the bytes join the model with this transaction's ops (an asset is not an op: undo keeps
    /// it, and the save writes the assets that records name).
    ///
    /// # Errors
    ///
    /// The [`AssetError`] of bytes that are no SVG, PNG or JPEG file, or exceed a limit.
    pub fn put_asset(&mut self, bytes: Vec<u8>) -> Result<AssetRef, AssetError> {
        let (format, size) = ew_schema::sniff(&bytes)?;
        let reference = AssetRef {
            sha256: Sha256(sha2::Sha256::digest(&bytes).into()),
            format,
            size,
        };
        self.assets
            .entry(reference.sha256)
            .or_insert_with(|| Asset::new(reference, bytes));
        Ok(reference)
    }

    /// The asset with this hash, brought by this transaction or held by the base (a structural lookup: asset
    /// bytes never change under their hash, so it is not recorded in the read set).
    pub fn asset(&self, sha256: &Sha256) -> Option<AssetRef> {
        self.assets
            .get(sha256)
            .or_else(|| self.base.asset(sha256))
            .map(|a| *a.reference())
    }

    /// Ends the transaction: its ops, its read set, the keys it inserted and the assets it brings.
    pub fn finish(self) -> (Vec<Op>, ReadSet, Vec<AnyKey>, Vec<Asset>) {
        let ops = self.ops();
        let inserted = self.inserted();
        (
            ops,
            self.reads.into_inner(),
            inserted,
            self.assets.into_values().collect(),
        )
    }
}

/// Whether two optional rows are the same record (both absent, or pointer- or value-equal).
fn same_opt(a: Option<&AnyRow>, b: Option<&AnyRow>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same(b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The links that `IndexRead` reads through the generic referrer index are declared, with the target kind the
    /// lookup keys on (a misspelt name would make the index answer nothing).
    #[test]
    fn the_referrer_links_of_the_channel_reads_exist() {
        for (field, to) in [("module", Kind::Device), ("binding.cp[0]", Kind::Function)] {
            let link = ew_model::LINKS
                .iter()
                .find(|l| l.from == Kind::Channel && l.field == field)
                .unwrap_or_else(|| panic!("Channel.{field} is not a declared link"));
            assert_eq!(link.to, to, "Channel.{field}");
            assert!(
                !link.indexed(),
                "Channel.{field} is served by the generic referrer index"
            );
        }
    }

    /// The macro-member read keys the generic referrer index under the kind-erased link's nominal `to`.
    #[test]
    fn the_referrer_link_of_the_macro_member_read_exists() {
        let link = ew_model::LINKS
            .iter()
            .find(|l| l.from == Kind::MacroInstance && l.field == "members[][1]")
            .expect("MacroInstance.members[][1] is a declared link");
        assert_eq!(link.to, Kind::Page);
        assert_eq!(link.kind_erased, ew_model::MACRO_MEMBER_KINDS);
        assert!(!link.indexed());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "is not declared in ew_model::LINKS")]
    fn a_link_name_that_does_not_exist_is_caught_in_debug_builds() {
        let nowhere: AnyKey = (Kind::Device, Id::<()>::from_uuid(Uuid::nil()));
        let _ = referrers_through(&ew_model::BaseIndexes::default(), Kind::Channel, "modul", nowhere);
    }
}
