// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`TxBuilder`]: the scratch snapshot with read-your-writes that a command and the normalisers write
//! through (ADR-0008, ARCH §5.2), and the [`ReadSet`] it records for the optimistic rebase of previews.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::{EntropyError, Id, IdSource, UnixTime, Uuid, V7Generator};
use ew_model::{AnyKey, AnyRow, EndpointKey, Entity, Model, Op, Rev, RowId};
use ew_schema::{
    AnyId, Bond, Conductor, Connection, Device, ExternalRef, FnDef, FormDef, Function, ImportLineage,
    Junction, Kind, LibCopy, Page, Part, PotentialDef, Project, ReportBlock, Representation, Section,
    StructureNode, SymbolDef,
};

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
        Kind::ReportBlock => at::<ReportBlock>(model, id),
        Kind::Revision => at::<ew_schema::Revision>(model, id),
        Kind::Part => at::<LibCopy<Part>>(model, id),
        Kind::Symbol => at::<LibCopy<SymbolDef>>(model, id),
        Kind::Fndef => at::<LibCopy<FnDef>>(model, id),
        Kind::Form => at::<LibCopy<FormDef>>(model, id),
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
        Kind::ReportBlock => at::<ReportBlock>(model),
        Kind::Revision => at::<ew_schema::Revision>(model),
        Kind::Part => at::<LibCopy<Part>>(model),
        Kind::Symbol => at::<LibCopy<SymbolDef>>(model),
        Kind::Fndef => at::<LibCopy<FnDef>>(model),
        Kind::Form => at::<LibCopy<FormDef>>(model),
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
    /// delete-time rule EW-INV-021 for a library copy named through a `LibRef`).
    pub fn remove_row(&mut self, key: AnyKey) {
        if !self.pending.contains_key(&key) {
            let present = self.base.row(&key).is_some();
            self.note_row(key, present);
        }
        self.stale.insert(key);
        self.pending.insert(key, None);
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

    /// Ends the transaction: its ops, its read set and the keys it inserted.
    pub fn finish(self) -> (Vec<Op>, ReadSet, Vec<AnyKey>) {
        let ops = self.ops();
        let inserted = self.inserted();
        (ops, self.reads.into_inner(), inserted)
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
