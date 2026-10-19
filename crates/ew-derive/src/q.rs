// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tracked-read context [`Q`] (ADR-0002 decision 1, ARCH §3 rule 1, §6.1).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use ew_ids::Id;
use ew_model::{EndpointKey, Entity, IndexName, Model, Rev, RevIndex, RowId, Table};
use ew_schema::{
    Bond, Conductor, Connection, Device, ExternalRef, FnDef, FormDef, Function, Junction, Kind, LibCopy,
    Page, Part, PotentialDef, Project, Record, ReportBlock, Representation, Section, StructureNode,
    SymbolDef,
};

use crate::note::DeriveNote;
use crate::profile::{self, DerivationProfile, ProfileStatus};
use crate::query::{Cache, Lookup, Query};
use crate::tracker::{ColdTracker, DepKey, IxKey, Tracker};

/// The tracked-read context: one model snapshot, a tracker and the request-scoped cache.
///
/// A query receives `&Q` and nothing else. `Q` owns its snapshot (a `Model` clone is O(1)) and never
/// hands it out: no method returns the `Model`, a raw `Table` or a raw index, and the fields are private.
/// Every accessor reports its read to the [`Tracker`] with the revision stamp it observed, so a memo
/// engine can verify the value later (ADR-0002 decisions 1 and 4).
///
/// `Q` is request-scoped: build one per top-level evaluation (a frame, a command preview, an export
/// pass). It is not `Sync`; the cache lives in a `RefCell`.
///
/// ```compile_fail
/// // A query cannot reach the model behind `Q`: there is no accessor and the field is private.
/// fn sneak(q: &ew_derive::Q) -> &ew_model::Model {
///     &q.model
/// }
/// ```
pub struct Q {
    model: Model,
    tracker: Box<dyn Tracker>,
    cache: RefCell<Cache>,
    notes: RefCell<BTreeSet<DeriveNote>>,
    profile: &'static DerivationProfile,
    profile_status: ProfileStatus,
    /// The report engine behind generated pages (`Q::with_report_pages`, WP-S3.03); none by default.
    pub(crate) report_pages: crate::pages::ProviderSlot,
}

/// Prints nothing a query could use to tell snapshots apart: no revision, no cache state.
impl fmt::Debug for Q {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Q").finish_non_exhaustive()
    }
}

/// Undoes `start` and `enter` when a query panics: the slot is removed (so a later `get` evaluates again
/// instead of reporting a false cycle) and the tracker stack is balanced. Disarmed on the normal path.
struct UnwindGuard<'q, Qy: Query> {
    q: &'q Q,
    key: Option<Qy::Key>,
}

impl<Qy: Query> Drop for UnwindGuard<'_, Qy> {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            // `try_borrow_mut`: a panic raised while the cache was borrowed must not double-panic.
            if let Ok(mut cache) = self.q.cache.try_borrow_mut() {
                cache.abort::<Qy>(&key);
            }
            self.q.tracker.exit(Qy::FAMILY);
        }
    }
}

/// A query needed its own value (ADR-0002 decision 4), as returned by [`Q::try_get`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cycle {
    /// The family whose evaluation is already running for the key.
    pub family: &'static str,
    /// The key, as `Debug` text.
    pub key: String,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "query cycle: `{}` needs its own value for {}",
            self.family, self.key
        )
    }
}

impl std::error::Error for Cycle {}

/// The diagnostics of a [`Q`]: what it evaluated and the [`DeriveNote`]s its queries pushed. Obtained
/// through [`Q::diagnostics`], which takes `&mut Q`, so a query body (which holds `&Q` only) cannot read
/// this untracked state.
#[derive(Debug)]
pub struct Diagnostics<'q> {
    q: &'q Q,
}

impl Diagnostics<'_> {
    /// The number of keys of `family` evaluated (or evaluating) so far.
    pub fn cached(&self, family: &str) -> usize {
        self.q.cache.borrow().len_of(family)
    }

    /// The notes so far, in the canonical order of [`DeriveNote`] (independent of evaluation order).
    pub fn notes(&self) -> Vec<DeriveNote> {
        self.q.notes.borrow().iter().cloned().collect()
    }
}

impl Q {
    /// A cold context over `model` (phase 1: the [`ColdTracker`]).
    pub fn new(model: &Model) -> Q {
        Q::with_tracker(model, Box::new(ColdTracker))
    }

    /// A context whose reads go to `tracker`.
    pub fn with_tracker(model: &Model, tracker: Box<dyn Tracker>) -> Q {
        let model = model.clone();
        let (profile, profile_status) = profile::resolve(model.project().derivation_profile.as_str());
        let mut notes = BTreeSet::new();
        // ADR-0003: an unknown pinned profile is never silent. It is a property of the snapshot, so every
        // `Q` over it carries the note from the start.
        if let ProfileStatus::Unknown { requested } = &profile_status {
            notes.insert(DeriveNote::UnknownProfile {
                requested: requested.clone(),
                used: profile.version.clone(),
            });
        }
        Q {
            model,
            tracker,
            cache: RefCell::new(Cache::default()),
            notes: RefCell::new(notes),
            profile,
            profile_status,
            report_pages: crate::pages::ProviderSlot::default(),
        }
    }

    /// The derived value of `Qy` for `key`, computed at most once per `Q`.
    ///
    /// # Panics
    ///
    /// On a dependency cycle (a query that needs its own value; ADR-0002 decision 4), and when two query
    /// types share one family name. A query edge that follows model content (a device's parent, a
    /// terminal's strip) uses [`Q::try_get`] instead, so that no model content can make a query panic.
    pub fn get<Qy: Query>(&self, key: Qy::Key) -> Arc<Qy::Value> {
        match self.try_get::<Qy>(key) {
            Ok(v) => v,
            Err(cycle) => panic!("{cycle}"),
        }
    }

    /// [`Q::get`], returning a [`Cycle`] instead of panicking when `Qy` is already evaluating `key`.
    ///
    /// The caller falls back to a neutral value and records the cycle with [`Q::note_cycle`]. On a model
    /// that breaks the invariant ruling the cycle out (EW-INV-015, which the integrity check enforces on
    /// load), the fallback values depend on which key was asked first; that is acceptable for a rejected
    /// model, a panic is not.
    ///
    /// # Panics
    ///
    /// When two query types share one family name.
    pub fn try_get<Qy: Query>(&self, key: Qy::Key) -> Result<Arc<Qy::Value>, Cycle> {
        self.tracker.query_read(Qy::FAMILY, &key);
        let lookup = self.cache.borrow_mut().lookup::<Qy>(&key);
        match lookup {
            Lookup::Hit(v) => Ok(v),
            Lookup::Running => Err(Cycle {
                family: Qy::FAMILY,
                key: format!("{key:?}"),
            }),
            Lookup::Miss => {
                self.cache.borrow_mut().start::<Qy>(key.clone());
                self.tracker.enter(Qy::FAMILY, &key);
                let mut guard = UnwindGuard::<Qy> {
                    q: self,
                    key: Some(key.clone()),
                };
                let value = Qy::run(self, &key);
                guard.key = None;
                self.tracker.exit(Qy::FAMILY);
                self.cache.borrow_mut().finish::<Qy>(key, Arc::clone(&value));
                Ok(value)
            }
        }
    }

    /// Records a derivation note (see [`DeriveNote`]). Notes form a set: pushing one twice keeps one.
    pub fn note(&self, note: DeriveNote) {
        self.notes.borrow_mut().insert(note);
    }

    /// Records a cycle returned by [`Q::try_get`] as a [`DeriveNote::QueryCycle`].
    pub fn note_cycle(&self, cycle: Cycle) {
        self.note(DeriveNote::QueryCycle {
            family: cycle.family,
            key: cycle.key,
        });
    }

    /// The diagnostics handle: evaluated keys and notes. It takes `&mut self`, so only the owner of the
    /// `Q` reaches it, never a query body:
    ///
    /// ```compile_fail
    /// // A query receives `&Q`; the untracked diagnostics need `&mut Q`.
    /// fn run(q: &ew_derive::Q) -> usize {
    ///     q.diagnostics().cached("device_tag")
    /// }
    /// ```
    pub fn diagnostics(&mut self) -> Diagnostics<'_> {
        Diagnostics { q: self }
    }

    // -----------------------------------------------------------------------------------------------------
    // Project and profile

    /// The project row.
    pub fn project(&self) -> &Arc<Project> {
        self.read(DepKey::Project, self.model.project_row().changed_at());
        self.model.project()
    }

    /// The derivation profile the project pins (ADR-0003, ARCH §6.6). A key this build does not ship
    /// falls back to the newest shipped profile; [`Q::profile_status`] says so.
    pub fn profile(&self) -> &'static DerivationProfile {
        self.read(DepKey::Project, self.model.project_row().changed_at());
        self.profile
    }

    /// Whether the pinned profile was found.
    pub fn profile_status(&self) -> &ProfileStatus {
        self.read(DepKey::Project, self.model.project_row().changed_at());
        &self.profile_status
    }

    // -----------------------------------------------------------------------------------------------------
    // Tables

    /// The tracked view of one table.
    pub fn table<T: Entity>(&self) -> TableView<'_, T> {
        TableView {
            table: self.model.table::<T>(),
            q: self,
        }
    }

    /// Structure nodes.
    pub fn structure_nodes(&self) -> TableView<'_, StructureNode> {
        self.table()
    }

    /// Sections.
    pub fn sections(&self) -> TableView<'_, Section> {
        self.table()
    }

    /// External references.
    pub fn external_refs(&self) -> TableView<'_, ExternalRef> {
        self.table()
    }

    /// Pages.
    pub fn pages(&self) -> TableView<'_, Page> {
        self.table()
    }

    /// Devices.
    pub fn devices(&self) -> TableView<'_, Device> {
        self.table()
    }

    /// Functions.
    pub fn functions(&self) -> TableView<'_, Function> {
        self.table()
    }

    /// Representations.
    pub fn reps(&self) -> TableView<'_, Representation> {
        self.table()
    }

    /// Connections.
    pub fn connections(&self) -> TableView<'_, Connection> {
        self.table()
    }

    /// Junctions.
    pub fn junctions(&self) -> TableView<'_, Junction> {
        self.table()
    }

    /// Conductors.
    pub fn conductors(&self) -> TableView<'_, Conductor> {
        self.table()
    }

    /// Potential definitions.
    pub fn potential_defs(&self) -> TableView<'_, PotentialDef> {
        self.table()
    }

    /// Bonds.
    pub fn bonds(&self) -> TableView<'_, Bond> {
        self.table()
    }

    /// Report blocks.
    pub fn report_blocks(&self) -> TableView<'_, ReportBlock> {
        self.table()
    }

    /// Embedded part copies.
    pub fn parts(&self) -> TableView<'_, LibCopy<Part>> {
        self.table()
    }

    /// Embedded symbol copies.
    pub fn symbols(&self) -> TableView<'_, LibCopy<SymbolDef>> {
        self.table()
    }

    /// Embedded function-definition copies.
    pub fn fndefs(&self) -> TableView<'_, LibCopy<FnDef>> {
        self.table()
    }

    /// Embedded form copies.
    pub fn forms(&self) -> TableView<'_, LibCopy<FormDef>> {
        self.table()
    }

    /// The records of a reserved kind (one without a typed table in this format version, such as
    /// `revision`), in the writer's line order. They carry no revision stamps, so the read is recorded as
    /// the coarse [`DepKey::Reserved`] at `model.rev()`: any change to the model invalidates every reader.
    /// `title_block`, which reads revisions through it, is therefore memo-hostile until revisions are typed
    /// records (doc 03 §18) with their own table and stamps.
    pub fn reserved(&self, kind: Kind) -> impl Iterator<Item = &serde_json::Value> + '_ {
        self.read(DepKey::Reserved, self.model.rev());
        let name = kind.name();
        self.model.unknown_records().iter().filter_map(move |r| match r {
            Record::Unknown(v) if v.get("$k").and_then(serde_json::Value::as_str) == Some(name) => Some(v),
            _ => None,
        })
    }

    // -----------------------------------------------------------------------------------------------------
    // Indexes

    /// The tracked views of the base indexes.
    pub fn idx(&self) -> IdxView<'_> {
        IdxView { q: self }
    }

    fn read(&self, dep: DepKey, observed: Rev) {
        self.tracker.read(dep, observed);
    }
}

/// The tracked view of one table of `Q`'s snapshot.
pub struct TableView<'q, T: Entity> {
    table: &'q Table<T>,
    q: &'q Q,
}

impl<T: Entity> Clone for TableView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Entity> Copy for TableView<'_, T> {}

impl<T: Entity> fmt::Debug for TableView<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableView").field("kind", &T::KIND).finish()
    }
}

impl<'q, T: Entity> TableView<'q, T> {
    /// The record with this ID; records `DepKey::Row` at the row's `changed_at`, or, for an absent row, at
    /// the table's.
    pub fn get(&self, id: RowId<T>) -> Option<&'q Arc<T>> {
        match self.table.row(&id) {
            Some(row) => {
                self.q.read(DepKey::Row(T::key(id)), row.changed_at());
                Some(row.value())
            }
            None => {
                self.q.read(DepKey::Row(T::key(id)), self.table.changed_at());
                None
            }
        }
    }

    /// Whether a row with this ID exists.
    pub fn contains(&self, id: RowId<T>) -> bool {
        self.get(id).is_some()
    }

    /// Every record in ID order; records a whole-table read.
    pub fn iter(&self) -> impl Iterator<Item = (RowId<T>, &'q Arc<T>)> + 'q {
        self.q.read(DepKey::Table(T::KIND), self.table.changed_at());
        self.table.iter().map(|(id, v)| (*id, v))
    }

    /// Every ID in order; records a whole-table read.
    pub fn ids(&self) -> impl Iterator<Item = RowId<T>> + 'q {
        self.q.read(DepKey::Table(T::KIND), self.table.changed_at());
        self.table.ids()
    }

    /// The number of rows; records a whole-table read.
    pub fn len(&self) -> usize {
        self.q.read(DepKey::Table(T::KIND), self.table.changed_at());
        self.table.len()
    }

    /// Whether the table is empty; records a whole-table read.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The tracked views of the base reverse indexes (ARCH §4.1). Each lookup records
/// `DepKey::IndexKey(name, key)`, observed at the key's `changed_at`; for an absent key the observed
/// stamp is the whole index's `changed_at`. It returns the values in order.
#[derive(Clone, Copy, Debug)]
pub struct IdxView<'q> {
    q: &'q Q,
}

impl<'q> IdxView<'q> {
    fn bucket<K: Ord + Clone, V: Ord + Clone + Copy>(
        &self,
        name: IndexName,
        ix: &'q RevIndex<K, V>,
        key: &K,
        dep: IxKey,
    ) -> std::vec::IntoIter<V> {
        let observed = ix.key_changed_at(key).unwrap_or(ix.changed_at());
        self.q.read(DepKey::IndexKey(name, dep), observed);
        ix.get(key).copied().collect::<Vec<V>>().into_iter()
    }

    fn by_id<K, V: Ord + Clone + Copy>(
        &self,
        name: IndexName,
        ix: &'q RevIndex<Id<K>, V>,
        key: Id<K>,
    ) -> std::vec::IntoIter<V> {
        self.bucket(name, ix, &key, IxKey::Id(key.cast()))
    }

    fn indexes(&self) -> &'q ew_model::BaseIndexes {
        self.q.model.indexes()
    }

    /// The functions of a device (`Function.device`).
    pub fn functions_of_device(&self, device: Id<Device>) -> impl Iterator<Item = Id<Function>> + 'q {
        self.by_id(
            IndexName::FunctionsByDevice,
            self.indexes().functions_by_device(),
            device,
        )
    }

    /// The representations on a page.
    pub fn reps_by_page(&self, page: Id<Page>) -> impl Iterator<Item = Id<Representation>> + 'q {
        self.by_id(IndexName::RepsByPage, self.indexes().reps_by_page(), page)
    }

    /// The representations that draw a function (`functions` or `pin_map[].function`).
    pub fn reps_of_function(&self, f: Id<Function>) -> impl Iterator<Item = Id<Representation>> + 'q {
        self.by_id(IndexName::RepsByFunction, self.indexes().reps_by_function(), f)
    }

    /// The connections drawn on a page.
    pub fn connections_by_page(&self, page: Id<Page>) -> impl Iterator<Item = Id<Connection>> + 'q {
        self.by_id(
            IndexName::ConnectionsByPage,
            self.indexes().connections_by_page(),
            page,
        )
    }

    /// The connections that end at an endpoint.
    pub fn connections_at(&self, at: EndpointKey) -> impl Iterator<Item = Id<Connection>> + 'q {
        self.bucket(
            IndexName::ConnectionsByEndpoint,
            self.indexes().connections_by_endpoint(),
            &at,
            IxKey::Endpoint(at),
        )
    }

    /// The connections at CP `idx` of function `f`.
    pub fn connections_at_cp(&self, f: Id<Function>, idx: u16) -> impl Iterator<Item = Id<Connection>> + 'q {
        self.connections_at(EndpointKey::Cp(f, idx))
    }

    /// Every CP of `f` that has a connection, with its connections, in CP order. A range read: records the
    /// whole endpoint index.
    pub fn connected_cps_of_function(&self, f: Id<Function>) -> Vec<(u16, Vec<Id<Connection>>)> {
        let ix = self.indexes().connections_by_endpoint();
        self.q
            .read(DepKey::Index(IndexName::ConnectionsByEndpoint), ix.changed_at());
        ix.range(EndpointKey::cps_of(f))
            .filter_map(|(k, b)| match k {
                EndpointKey::Cp(_, idx) => Some((*idx, b.values().copied().collect())),
                EndpointKey::Junction(_) => None,
            })
            .collect()
    }

    /// The junctions on a page.
    pub fn junctions_by_page(&self, page: Id<Page>) -> impl Iterator<Item = Id<Junction>> + 'q {
        self.by_id(
            IndexName::JunctionsByPage,
            self.indexes().junctions_by_page(),
            page,
        )
    }

    /// The terminal devices of a strip device.
    pub fn terminals_of_strip(&self, strip: Id<Device>) -> impl Iterator<Item = Id<Device>> + 'q {
        self.by_id(
            IndexName::TerminalsByStrip,
            self.indexes().terminals_by_strip(),
            strip,
        )
    }

    /// The conductors of a cable device.
    pub fn conductors_of_cable(&self, cable: Id<Device>) -> impl Iterator<Item = Id<Conductor>> + 'q {
        self.by_id(
            IndexName::ConductorsByCable,
            self.indexes().conductors_by_cable(),
            cable,
        )
    }

    /// The child nodes of a structure node.
    pub fn structure_children(
        &self,
        node: Id<StructureNode>,
    ) -> impl Iterator<Item = Id<StructureNode>> + 'q {
        self.by_id(
            IndexName::StructureChildren,
            self.indexes().structure_children(),
            node,
        )
    }

    /// The child devices of a device (assemblies, `Device.parent`).
    pub fn device_children(&self, device: Id<Device>) -> impl Iterator<Item = Id<Device>> + 'q {
        self.by_id(
            IndexName::DeviceChildren,
            self.indexes().device_children(),
            device,
        )
    }

    /// The pages of a section (`Folio.section`).
    pub fn pages_of_section(&self, section: Id<Section>) -> impl Iterator<Item = Id<Page>> + 'q {
        self.by_id(
            IndexName::PagesBySection,
            self.indexes().pages_by_section(),
            section,
        )
    }

    /// The potential definitions placed at an endpoint.
    pub fn potential_defs_at(&self, at: EndpointKey) -> impl Iterator<Item = Id<PotentialDef>> + 'q {
        self.bucket(
            IndexName::PotentialDefsByEndpoint,
            self.indexes().potential_defs_by_endpoint(),
            &at,
            IxKey::Endpoint(at),
        )
    }

    /// The bonds at an endpoint.
    pub fn bonds_at(&self, at: EndpointKey) -> impl Iterator<Item = Id<Bond>> + 'q {
        self.bucket(
            IndexName::BondsByEndpoint,
            self.indexes().bonds_by_endpoint(),
            &at,
            IxKey::Endpoint(at),
        )
    }
}
