// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-SP memo prototype (WP-S1.05): `ew_memo::Db` over a model snapshot, and memo families that wrap one
//! `ew-derive` query family each, without any change to `ew-derive` or `ew-memo`.
//!
//! # The `Db` mapping
//!
//! - **Inputs** are `ew-derive`'s [`DepKey`]s, exactly what `Q`'s tracked accessors report: the project row,
//!   a row (present or absent), a whole table, one index bucket, a whole index, the reserved records.
//! - **Stamps.** [`ModelDb::changed_at`] performs the same tracked read through a probe `Q` over the snapshot
//!   whose tracker echoes the observed stamp back ([`Echo`]). The stamps are therefore by construction the
//!   ones `ew-derive`'s verifier contract names (`crates/ew-derive/src/tracker.rs`): no raw-read access, no
//!   second definition of "the stamp of a dependency". A whole-index read of an index other than the
//!   endpoint index (which no query takes today) answers the model's revision: conservative, never stale.
//! - **Durability.** The project row (settings, the pinned profile) is `Settings`; rows and tables of the
//!   embedded library copies are `Library`; everything else is `Drawing`. A commit advances the store with the
//!   most durable level among its touched keys ([`durability_of_keys`]).
//! - **Revisions.** The store revision is the model revision (`ew_model::Rev(n)` is `ew_memo::Rev(n)`); a
//!   measurement sample works on an O(1) fork of a store warmed on the baseline, so every fork sees one linear
//!   history (baseline, then the sample's commit).
//!
//! # The memo families ([`Memo`])
//!
//! `Memo<F>` memoises the `ew-derive` family `F` **as one opaque unit**: its `execute` evaluates `F` in a
//! fresh `Q` whose tracker records every read of the whole evaluation, nested queries included ([`Rec`]),
//! and reports the reads to `ew-memo` as the memo's dependencies. The dependency list is minimised first
//! ([`minimal_deps`]): duplicates go, and a row read of a table that was also read whole is dropped (a
//! table's stamp moves with every row put, so the whole-table read subsumes it). The value is the `Arc`
//! `ew-derive` returned; equivalence (early cut-off and the oracle) is `Eq` on the value.
//!
//! What this cannot express without a hook in `Q` (the report's "integration" section): nested families are
//! not separate memos, so a memoised `potential_partition` cannot reuse memoised `page_local_nets`; `Q::get`
//! would have to consult the store for families whose policy is `Memo` and report a memo dependency
//! (`DepKey::Memo`) instead of the nested reads.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use ew_derive::{DepKey, IxKey, Q, Query as DeriveQuery, Tracker};
use ew_memo::{Cx, Cycle, Db, Durability, Equivalence, Query, Rev, Store};
use ew_model::{AnyKey, Entity, IndexName, Model};
use ew_schema::{
    AnyId, Bond, Conductor, Connection, Device, ExternalRef, FnDef, FormDef, Function, ImportLineage,
    Junction, Kind, LibCopy, Page, Part, PotentialDef, ReportBlock, Representation, Section, StructureNode,
    SymbolDef,
};

// ---------------------------------------------------------------------------------------------------------
// Trackers

/// A tracker that remembers the stamp of the last read (the probe of [`ModelDb::changed_at`]).
#[derive(Debug, Default)]
pub struct Echo(Cell<Option<ew_model::Rev>>);

impl Tracker for Echo {
    fn read(&self, _dep: DepKey, observed: ew_model::Rev) {
        self.0.set(Some(observed));
    }
}

/// A lean recorder of every read of one evaluation (no query stack, no key formatting).
#[derive(Debug, Default)]
pub struct Rec(RefCell<Vec<(DepKey, ew_model::Rev)>>);

impl Rec {
    /// The reads so far, in read order.
    pub fn take(&self) -> Vec<(DepKey, ew_model::Rev)> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

impl Tracker for Rec {
    fn read(&self, dep: DepKey, observed: ew_model::Rev) {
        self.0.borrow_mut().push((dep, observed));
    }
}

/// The deduplicated dependencies of a read list, in `DepKey` order, without the row reads of tables that
/// were also read whole (see the module documentation).
pub fn minimal_deps(reads: &[(DepKey, ew_model::Rev)]) -> Vec<DepKey> {
    let mut set: BTreeMap<DepKey, ()> = BTreeMap::new();
    for (d, _) in reads {
        set.insert(*d, ());
    }
    let whole: Vec<Kind> = set
        .keys()
        .filter_map(|d| match d {
            DepKey::Table(k) => Some(*k),
            _ => None,
        })
        .collect();
    set.into_keys()
        .filter(|d| match d {
            DepKey::Row((k, _)) => !whole.contains(k),
            _ => true,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// The Db

/// `ew_memo::Db` over one model snapshot (see the module documentation).
pub struct ModelDb {
    model: Model,
    probe: Q,
    echo: Rc<Echo>,
}

impl fmt::Debug for ModelDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelDb")
            .field("rev", &self.model.rev())
            .finish_non_exhaustive()
    }
}

impl ModelDb {
    /// The database over `model` (an O(1) clone).
    pub fn new(model: &Model) -> ModelDb {
        let echo = Rc::new(Echo::default());
        let probe = Q::with_tracker(model, Box::new(Rc::clone(&echo)));
        ModelDb {
            model: model.clone(),
            probe,
            echo,
        }
    }

    /// The snapshot.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// The store revision of the snapshot.
    pub fn rev(&self) -> Rev {
        Rev(self.model.rev().0)
    }

    /// Performs the tracked read of `dep` on the probe `Q`; the echo tracker catches the stamp.
    fn probe(&self, dep: &DepKey) {
        let q = &self.probe;
        match *dep {
            DepKey::Project => {
                let _ = q.project();
            }
            DepKey::Row((kind, id)) => row_read(q, kind, id),
            DepKey::Table(kind) => table_read(q, kind),
            DepKey::IndexKey(name, key) => index_key_read(q, name, key),
            DepKey::Index(_) | DepKey::Reserved => {
                // Conservative: the model's revision (the stamp `Q` gives the reserved records).
                let _ = q.reserved(Kind::Revision).next();
            }
        }
    }
}

fn row<T: Entity>(q: &Q, id: AnyId) {
    let _ = q.table::<T>().get(id.cast());
}

fn whole<T: Entity>(q: &Q) {
    let _ = q.table::<T>().len();
}

macro_rules! by_kind {
    ($kind:expr, $f:ident, $q:expr $(, $arg:expr)*) => {
        match $kind {
            Kind::ImportLineage => $f::<ImportLineage>($q $(, $arg)*),
            Kind::StructureNode => $f::<StructureNode>($q $(, $arg)*),
            Kind::Section => $f::<Section>($q $(, $arg)*),
            Kind::ExternalRef => $f::<ExternalRef>($q $(, $arg)*),
            Kind::Page => $f::<Page>($q $(, $arg)*),
            Kind::Device => $f::<Device>($q $(, $arg)*),
            Kind::Function => $f::<Function>($q $(, $arg)*),
            Kind::Representation => $f::<Representation>($q $(, $arg)*),
            Kind::Connection => $f::<Connection>($q $(, $arg)*),
            Kind::Junction => $f::<Junction>($q $(, $arg)*),
            Kind::Conductor => $f::<Conductor>($q $(, $arg)*),
            Kind::PotentialDef => $f::<PotentialDef>($q $(, $arg)*),
            Kind::Bond => $f::<Bond>($q $(, $arg)*),
            Kind::ReportBlock => $f::<ReportBlock>($q $(, $arg)*),
            Kind::Part => $f::<LibCopy<Part>>($q $(, $arg)*),
            Kind::Symbol => $f::<LibCopy<SymbolDef>>($q $(, $arg)*),
            Kind::Fndef => $f::<LibCopy<FnDef>>($q $(, $arg)*),
            Kind::Form => $f::<LibCopy<FormDef>>($q $(, $arg)*),
            // A kind without a table: `Q` has no read of it; conservative.
            _ => {
                let _ = $q.reserved(Kind::Revision).next();
            }
        }
    };
}

fn row_read(q: &Q, kind: Kind, id: AnyId) {
    by_kind!(kind, row, q, id);
}

fn table_read(q: &Q, kind: Kind) {
    by_kind!(kind, whole, q);
}

fn index_key_read(q: &Q, name: IndexName, key: IxKey) {
    let ix = q.idx();
    match (name, key) {
        (IndexName::FunctionsByDevice, IxKey::Id(id)) => drop(ix.functions_of_device(id.cast())),
        (IndexName::RepsByPage, IxKey::Id(id)) => drop(ix.reps_by_page(id.cast())),
        (IndexName::RepsByFunction, IxKey::Id(id)) => drop(ix.reps_of_function(id.cast())),
        (IndexName::ConnectionsByPage, IxKey::Id(id)) => drop(ix.connections_by_page(id.cast())),
        (IndexName::ConnectionsByEndpoint, IxKey::Endpoint(at)) => drop(ix.connections_at(at)),
        (IndexName::JunctionsByPage, IxKey::Id(id)) => drop(ix.junctions_by_page(id.cast())),
        (IndexName::TerminalsByStrip, IxKey::Id(id)) => drop(ix.terminals_of_strip(id.cast())),
        (IndexName::ConductorsByCable, IxKey::Id(id)) => drop(ix.conductors_of_cable(id.cast())),
        (IndexName::StructureChildren, IxKey::Id(id)) => drop(ix.structure_children(id.cast())),
        (IndexName::DeviceChildren, IxKey::Id(id)) => drop(ix.device_children(id.cast())),
        (IndexName::PagesBySection, IxKey::Id(id)) => drop(ix.pages_of_section(id.cast())),
        (IndexName::PotentialDefsByEndpoint, IxKey::Endpoint(at)) => drop(ix.potential_defs_at(at)),
        (IndexName::BondsByEndpoint, IxKey::Endpoint(at)) => drop(ix.bonds_at(at)),
        // A key shape no accessor produces: conservative.
        _ => {
            let _ = q.reserved(Kind::Revision).next();
        }
    }
}

impl Db for ModelDb {
    type Input = DepKey;

    fn changed_at(&self, input: &DepKey) -> Rev {
        self.echo.0.set(None);
        self.probe(input);
        Rev(self.echo.0.get().expect("every probe read reports a stamp").0)
    }

    fn durability(&self, input: &DepKey) -> Durability {
        durability_of(input)
    }
}

/// Whether a kind is an embedded library copy.
fn is_library(kind: Kind) -> bool {
    matches!(kind, Kind::Part | Kind::Symbol | Kind::Fndef | Kind::Form)
}

/// The durability of an input (see the module documentation).
pub fn durability_of(input: &DepKey) -> Durability {
    match input {
        DepKey::Project => Durability::Settings,
        DepKey::Row((k, _)) | DepKey::Table(k) if is_library(*k) => Durability::Library,
        _ => Durability::Drawing,
    }
}

/// The durability to advance with after a commit that put `keys`: the most durable level among them
/// (`Drawing` for an empty list).
pub fn durability_of_keys(keys: &[AnyKey]) -> Durability {
    keys.iter()
        .map(|(k, _)| match k {
            Kind::Project => Durability::Settings,
            k if is_library(*k) => Durability::Library,
            _ => Durability::Drawing,
        })
        .max()
        .unwrap_or(Durability::Drawing)
}

// ---------------------------------------------------------------------------------------------------------
// Memo families

/// The memo family over the `ew-derive` family `F` (see the module documentation).
pub struct Memo<F>(PhantomData<F>);

impl<F> fmt::Debug for Memo<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Memo")
    }
}

/// Evaluates `F(key)` in a fresh recording `Q` over `model`: the value and every read.
pub fn evaluate<F: DeriveQuery>(
    model: &Model,
    key: &F::Key,
) -> (Arc<F::Value>, Vec<(DepKey, ew_model::Rev)>) {
    let rec = Rc::new(Rec::default());
    let q = Q::with_tracker(model, Box::new(Rc::clone(&rec)));
    let value = q.get::<F>(key.clone());
    drop(q);
    (value, rec.take())
}

impl<F> Query<ModelDb> for Memo<F>
where
    F: DeriveQuery,
    F::Key: Send + Sync,
    F::Value: Eq,
{
    type Key = F::Key;
    type Value = Arc<F::Value>;
    const NAME: &'static str = F::FAMILY;

    fn equivalence() -> Equivalence<Arc<F::Value>> {
        Equivalence::Eq(|a, b| Arc::ptr_eq(a, b) || a == b)
    }

    fn execute(cx: &mut Cx<'_, ModelDb>, key: &F::Key) -> Result<Arc<F::Value>, Cycle> {
        let (value, reads) = evaluate::<F>(cx.db().model(), key);
        for dep in minimal_deps(&reads) {
            cx.read(&dep);
        }
        Ok(value)
    }
}

/// A store over [`ModelDb`] at the revision of `db`'s snapshot (advanced once when it is past zero).
pub fn store_at(db: &ModelDb) -> Store<ModelDb> {
    let store = Store::new();
    if db.rev() > Rev::ZERO {
        store.advance(db.rev(), Durability::Library);
    }
    store
}
