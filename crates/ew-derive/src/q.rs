// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tracked-read context [`Q`] (ADR-0002 decision 1, ARCH §3 rule 1, §6.1).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use ew_ids::Id;
use ew_model::{EndpointKey, Entity, IndexName, Model, Rev, RevIndex, RowId, Table};
use ew_schema::{
    Bond, Conductor, Connection, Device, ExternalRef, FnDef, FormDef, Function, Junction, Kind, LibCopy,
    Manufacturer, Page, PageItem, Part, PotentialDef, Project, Record, ReportBlock, Representation, Section,
    StructureBox, StructureNode, SymbolDef,
};

use crate::memo_keys::{self, MemoInput, PartKey};
use crate::memo_store::{self, MemoOf};
use crate::note::{DeriveNote, NoteFrame, NoteSet};
use crate::profile::{self, DerivationProfile, ProfileStatus};
use crate::query::{Cache, Lookup, Policy, Query};
use crate::tracker::{ColdTracker, DepKey, IxKey, Tracker};

/// The memo store a `Q` from a [`MemoStore`](crate::MemoStore) reads through (ADR-0002 amendment 4).
pub(crate) type DeriveStore = ew_memo::Store<Q>;

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
/// # Configuration: the report engine
///
/// Besides the snapshot, a `Q` carries engine-level configuration that is not a tracked input: the
/// derivation profile (resolved from the project row, which is tracked) and the
/// [`ReportPagesProvider`](crate::ReportPagesProvider) behind generated pages ([`Q::with_report_pages`]).
/// The session installs the report engine on every `Q` it hands out; `Q::new` without a provider is the
/// "no reports" configuration for tests. The provider is fixed before the first query (installing it
/// clears the cache), and no query body can observe whether one is installed ([`Q::has_report_pages`]
/// takes `&mut self`). A memoising `Q` takes its provider from its [`MemoStore`](crate::MemoStore): one
/// provider per store, never switched between evaluations that share memo tables.
///
/// # Memoisation
///
/// A `Q` from [`MemoStore::query`](crate::MemoStore::query) reads the `Memo` and `MemoLru` families through
/// the store's `ew-memo` engine (ADR-0002 amendment 4): a read of such a family inside another records one
/// memo dependency, not the nested family's reads; cold families run inside the execution that reads them
/// (each memo execution has its own request cache, so their reads are recorded where they are used); the
/// notes a memoised family pushed are kept with its value and replayed into every `Q` that reads it. A
/// query that caught a cycle ([`Q::try_get`]) makes its execution volatile: its value is not kept.
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
    /// The request-scoped caches: the request's, then one per memo execution in progress.
    cache: RefCell<Vec<Cache>>,
    /// The notes: the request's, then the notes of each memo execution in progress (own notes, and the
    /// shared note sets of the memoised values read).
    notes: RefCell<Vec<NoteFrame>>,
    profile: &'static DerivationProfile,
    profile_status: ProfileStatus,
    /// The report engine behind generated pages (`Q::with_report_pages`, WP-S3.03); none by default.
    /// Configuration, not a tracked input: see the type documentation.
    pub(crate) report_pages: crate::pages::ProviderSlot,
    /// The memo store of a memoising `Q`, which holds this `Q`'s snapshot at its current revision.
    memo: Option<Rc<DeriveStore>>,
    /// Per memo execution in progress (innermost last): whether its body declared changed parts
    /// ([`Q::changed_parts`]; `memo_store` then also declares the notes part when the notes changed), and
    /// whether its family is a producer ([`Query::PRODUCER`], checked in debug builds).
    parts_declared: RefCell<Vec<PartsFrame>>,
}

/// Prints nothing a query could use to tell snapshots apart: no revision, no cache state.
impl fmt::Debug for Q {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Q").finish_non_exhaustive()
    }
}

/// Undoes `start` and `enter` when a query panics: the slot is removed (so a later `get` evaluates again
/// instead of reporting a false cycle), the note frame of a capturing evaluation is popped and the tracker
/// stack is balanced. Disarmed on the normal path.
struct UnwindGuard<'q, Qy: Query> {
    q: &'q Q,
    key: Option<Qy::Key>,
    /// The evaluation pushed a note frame to capture its notes (inside a detached scope).
    capture: bool,
}

impl<Qy: Query> Drop for UnwindGuard<'_, Qy> {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            // `try_borrow_mut`: a panic raised while the cache was borrowed must not double-panic.
            if let Ok(mut cache) = self.q.cache.try_borrow_mut()
                && let Some(top) = cache.last_mut()
            {
                top.abort::<Qy>(&key);
            }
            if self.capture
                && let Ok(mut notes) = self.q.notes.try_borrow_mut()
            {
                notes.pop();
            }
            self.q.tracker.exit(Qy::FAMILY);
        }
    }
}

/// The parts state of one memo execution in progress.
#[derive(Clone, Copy)]
struct PartsFrame {
    /// The body declared changed parts.
    declared: bool,
    /// The family is a producer of parts.
    producer: bool,
    /// The family.
    family: &'static str,
}

/// What one memo execution of a family produced ([`Q::memo_frame`]).
pub(crate) struct MemoFrameOut<V> {
    /// The value.
    pub(crate) value: Arc<V>,
    /// The execution's note set (`None`: no note).
    pub(crate) notes: Option<Arc<NoteSet>>,
    /// Whether the note set is a new allocation (the value owns its bytes).
    pub(crate) owned: bool,
    /// Whether the body declared the parts it changed.
    pub(crate) declared: bool,
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
    /// The number of keys of `family` evaluated (or evaluating) so far in this `Q`'s request cache. A
    /// memoising `Q` keeps its memoised families in the store, not here.
    pub fn cached(&self, family: &str) -> usize {
        self.q.cache.borrow().first().map_or(0, |c| c.len_of(family))
    }

    /// The notes so far, in the canonical order of [`DeriveNote`] (independent of evaluation order); for a
    /// memoising `Q` also the notes of the memoised values it read.
    pub fn notes(&self) -> Vec<DeriveNote> {
        self.q
            .notes
            .borrow()
            .first()
            .map(|n| n.notes().into_iter().collect())
            .unwrap_or_default()
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
            cache: RefCell::new(vec![Cache::default()]),
            notes: RefCell::new(vec![NoteFrame::with(notes)]),
            profile,
            profile_status,
            report_pages: crate::pages::ProviderSlot::default(),
            memo: None,
            parts_declared: RefCell::new(Vec::new()),
        }
    }

    /// A memoising context over `model`, the snapshot `store` was last advanced to, with the store's report
    /// engine (see [`MemoStore::query`](crate::MemoStore::query)).
    pub(crate) fn with_memo(
        model: &Model,
        store: Rc<DeriveStore>,
        provider: Option<Arc<dyn crate::ReportPagesProvider>>,
    ) -> Q {
        let mut q = Q::new(model);
        q.report_pages = crate::pages::ProviderSlot(provider);
        q.memo = Some(store);
        q
    }

    /// A cold context over the same snapshot with the same report engine: the reference of oracle mode.
    pub(crate) fn cold_twin(&self) -> Q {
        let mut q = Q::new(&self.model);
        q.report_pages = crate::pages::ProviderSlot(self.report_pages.0.clone());
        q
    }

    /// The memo store, when this `Q` memoises and its snapshot is the store's current one.
    fn memo_store(&self) -> Option<&DeriveStore> {
        self.memo
            .as_deref()
            .filter(|s| s.current().0 == self.model.rev().0)
    }

    /// Runs one memo execution of `Qy` (the body of [`MemoOf`]): its own request cache and note frame, the
    /// tracker bracket; returns the value and the execution's note set (its own notes and, by reference,
    /// those of the memoised values it read; `None` without notes) with whether that set is a new allocation,
    /// and whether the body declared changed parts ([`Q::changed_parts`]). A panic in `body` unwinds the
    /// frames.
    pub(crate) fn memo_frame<Qy: Query>(
        &self,
        key: &Qy::Key,
        body: impl FnOnce() -> Arc<Qy::Value>,
    ) -> MemoFrameOut<Qy::Value> {
        struct Frames<'q> {
            q: &'q Q,
            family: &'static str,
            armed: bool,
        }
        impl Drop for Frames<'_> {
            fn drop(&mut self) {
                if self.armed {
                    if let Ok(mut c) = self.q.cache.try_borrow_mut() {
                        c.pop();
                    }
                    if let Ok(mut n) = self.q.notes.try_borrow_mut() {
                        n.pop();
                    }
                    if let Ok(mut d) = self.q.parts_declared.try_borrow_mut() {
                        d.pop();
                    }
                    self.q.tracker.exit(self.family);
                }
            }
        }
        self.cache.borrow_mut().push(Cache::default());
        self.notes.borrow_mut().push(NoteFrame::default());
        self.parts_declared.borrow_mut().push(PartsFrame {
            declared: false,
            producer: Qy::PRODUCER,
            family: Qy::FAMILY,
        });
        self.tracker.enter(Qy::FAMILY, key);
        let mut frames = Frames {
            q: self,
            family: Qy::FAMILY,
            armed: true,
        };
        let value = body();
        frames.armed = false;
        self.tracker.exit(Qy::FAMILY);
        self.cache.borrow_mut().pop();
        let declared = self.parts_declared.borrow_mut().pop().is_some_and(|f| f.declared);
        let frame = self.notes.borrow_mut().pop().unwrap_or_default();
        let (notes, owned) = frame.into_set();
        MemoFrameOut {
            value,
            notes,
            owned,
            declared,
        }
    }

    /// Adds the notes of a memoised value to the innermost note frame (a read of the value): one shared
    /// reference, not a copy of the notes.
    pub(crate) fn replay(&self, notes: Option<&Arc<NoteSet>>) {
        let Some(notes) = notes else { return };
        if let Some(top) = self.notes.borrow_mut().last_mut() {
            top.replay(notes);
        }
    }

    /// A query caught a cycle: the memo execution it runs in depends on the evaluation context.
    fn caught_cycle(&self) {
        if let Some(store) = self.memo_store() {
            store.set_volatile();
        }
    }

    /// The key of family `F` that a memo or part entry of [`Changes::via`](crate::Changes::via) names (an
    /// incremental hook maps the dependencies it was marked through); `None` for an input, another family,
    /// or a `Q` that does not memoise.
    pub fn via_key<F: Query>(&self, dep: &ew_memo::DepKey<MemoInput>) -> Option<F::Key> {
        self.memo_store()?.key_of::<MemoOf<F>>(dep)
    }

    /// In an incremental execution, drops the dependency on `F(key)` carried from the previous execution: the
    /// new value no longer relies on it (a page or function the hook removed from its value). A later read in
    /// the same execution records it again. Cold, or in a non-incremental execution, it does nothing (review
    /// cycle 10, finding 3: without it an incremental family kept every dependency it ever read).
    pub(crate) fn forget<F: Query>(&self, key: &F::Key) {
        if let Some(store) = self.memo_store()
            && let Some(dep) = store.memo_key::<MemoOf<F>>(key)
        {
            store.forget(&dep);
        }
    }

    /// In an incremental execution, drops every dependency carried from the previous execution: the execution
    /// depends on what it reads from now on. For a hook that re-reads everything its value relies on (an index
    /// family re-reading its table's stamp) or falls back to the full computation. Cold, or in a
    /// non-incremental execution, it does nothing.
    pub(crate) fn drop_carried(&self) {
        if let Some(store) = self.memo_store() {
            store.drop_carried();
        }
    }

    /// The value of `Qy` for `key`, depending on its part `part` only (ADR-0002 amendment 4): in a memoising
    /// `Q` the reading execution is invalidated when `Qy`'s execution declares `part` changed (or its notes,
    /// [`PartKey::Notes`]), not on every change of `Qy`. The caller must read only what `part` names. Cold, it
    /// is [`Q::get`].
    pub(crate) fn get_part<Qy: Query>(&self, key: Qy::Key, part: PartKey) -> Arc<Qy::Value> {
        if Qy::POLICY != Policy::Cold
            && let Some(store) = self.memo_store()
        {
            self.tracker.query_read(Qy::FAMILY, &key);
            return match memo_store::get_part::<Qy>(self, store, &key, part) {
                Ok(v) => v,
                Err(cycle) => panic!("{cycle}"),
            };
        }
        self.get::<Qy>(key)
    }

    /// The value of `Qy` for `key`, read as a cut-off point ([`PartKey::Value`]): a commit that marks
    /// `Qy(key)` does not mark the reading execution (no transitive marking through it); the reader is
    /// invalidated only when a re-execution of `Qy(key)` changes its value. For high fan-out values with
    /// early cut-off: settings members, profile rows. `Qy` must not declare parts (it is no
    /// [`Query::PRODUCER`]; debug builds check it). Cold, it is [`Q::get`].
    pub(crate) fn get_value_part<Qy: Query>(&self, key: Qy::Key) -> Arc<Qy::Value> {
        debug_assert!(
            !Qy::PRODUCER,
            "ew-derive: the producer `{}` is read as a value part",
            Qy::FAMILY
        );
        self.get_part::<Qy>(key, PartKey::Value)
    }

    /// Declares, in an incremental execution of a producer, that these parts of its value changed (the
    /// readers of other parts stay valid). A producer that declares parts must declare every changed part
    /// whenever it has a previous value; `memo_store` adds [`PartKey::Notes`] when its notes changed. Only a
    /// [`Query::PRODUCER`] declares parts (debug builds check it). Cold, it does nothing.
    pub(crate) fn changed_parts(&self, parts: impl IntoIterator<Item = PartKey>) {
        if let Some(store) = self.memo_store() {
            if let Some(top) = self.parts_declared.borrow_mut().last_mut() {
                debug_assert!(
                    top.producer,
                    "ew-derive: `{}` declares parts but is no producer (Query::PRODUCER)",
                    top.family
                );
                top.declared = true;
            }
            store.changed_parts(parts.into_iter().map(MemoInput::Part));
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
        if Qy::POLICY != Policy::Cold
            && let Some(store) = self.memo_store()
        {
            let got = memo_store::get::<Qy>(self, store, &key);
            if got.is_err() {
                self.caught_cycle();
            }
            return got;
        }
        let lookup = self.top_cache(|c| c.lookup::<Qy>(&key));
        match lookup {
            Lookup::Hit(v, notes) => {
                self.replay(notes.as_ref());
                Ok(v)
            }
            Lookup::Running => {
                self.caught_cycle();
                Err(Cycle {
                    family: Qy::FAMILY,
                    key: format!("{key:?}"),
                })
            }
            Lookup::Miss => {
                // Inside a detached scope the evaluation captures its notes and keeps them with its value: the
                // scope drops them, a later read outside the scope replays them.
                let capture = self.top_cache(|c| {
                    c.start::<Qy>(key.clone());
                    c.detached > 0
                });
                if capture {
                    self.notes.borrow_mut().push(NoteFrame::default());
                }
                self.tracker.enter(Qy::FAMILY, &key);
                let mut guard = UnwindGuard::<Qy> {
                    q: self,
                    key: Some(key.clone()),
                    capture,
                };
                let value = Qy::run(self, &key);
                guard.key = None;
                self.tracker.exit(Qy::FAMILY);
                let notes = if capture {
                    let frame = self.notes.borrow_mut().pop().unwrap_or_default();
                    frame.into_set().0
                } else {
                    None
                };
                self.top_cache(|c| c.finish::<Qy>(key, Arc::clone(&value), notes.clone()));
                self.replay(notes.as_ref());
                Ok(value)
            }
        }
    }

    /// Runs `f` in a detached scope: the notes pushed inside are dropped afterwards. The reads inside are
    /// tracked as usual (they are dependencies of the running query), and the scope shares the request cache of
    /// the frame it runs in (the request's, or the memo execution's): a value computed before the scope is a
    /// hit inside it, and a value computed inside it is a hit afterwards. A cold family evaluated inside the
    /// scope keeps the notes it pushed with its cached value, so a read outside the scope replays them as a
    /// fresh evaluation would push them (review cycle 10, finding 1: a scope with a cache of its own evaluated
    /// every family it read again, once per scope).
    ///
    /// For the note-free connectivity families (ARCH §6.4): an incremental family whose hook re-reads only what
    /// changed cannot reproduce the notes of what it did not re-read, so it carries no notes; helper queries
    /// whose notes belong to their own readers (the default frame of an anchor cell, a structure cycle) run
    /// detached, and the family records what it must report in its value. The scope behaves the same in a
    /// cold and in a memoising `Q`.
    pub(crate) fn detached<R>(&self, f: impl FnOnce() -> R) -> R {
        struct Scope<'q> {
            q: &'q Q,
        }
        impl Drop for Scope<'_> {
            fn drop(&mut self) {
                // A memo execution started inside the scope has popped its own cache by now (on a panic too), so
                // the top cache is the one the scope opened over.
                if let Ok(mut c) = self.q.cache.try_borrow_mut()
                    && let Some(top) = c.last_mut()
                {
                    top.detached = top.detached.saturating_sub(1);
                }
                if let Ok(mut n) = self.q.notes.try_borrow_mut() {
                    n.pop();
                }
            }
        }
        self.top_cache(|c| c.detached += 1);
        self.notes.borrow_mut().push(NoteFrame::default());
        let _scope = Scope { q: self };
        f()
    }

    /// Runs `f` on the innermost request cache.
    fn top_cache<R>(&self, f: impl FnOnce(&mut Cache) -> R) -> R {
        f(self.cache.borrow_mut().last_mut().expect("the request cache"))
    }

    /// This context with the report engine installed: [`PageOrder`](crate::PageOrder) then includes the
    /// generated report pages (WP-S3.03). The provider is configuration, not a tracked input (see the type
    /// documentation), so installing it discards every value this `Q` has already cached and every note
    /// its queries pushed: nothing computed under the previous configuration survives.
    #[must_use]
    pub fn with_report_pages(mut self, provider: Arc<dyn crate::ReportPagesProvider>) -> Q {
        self.report_pages = crate::pages::ProviderSlot(Some(provider));
        *self.cache.get_mut() = vec![Cache::default()];
        // Only the snapshot's own note (an unknown pinned profile) is independent of query evaluation.
        let notes = self.notes.get_mut();
        notes.truncate(1);
        if let Some(n) = notes.first_mut() {
            n.retain_own(|n| matches!(n, DeriveNote::UnknownProfile { .. }));
        }
        // A provider switched on a memoising `Q` would mix configurations in one store: memoise no more.
        self.memo = None;
        self
    }

    /// Whether a report engine is installed. It takes `&mut self`, so a query body (which holds `&Q`)
    /// cannot branch on this untracked configuration:
    ///
    /// ```compile_fail
    /// fn run(q: &ew_derive::Q) -> bool {
    ///     q.has_report_pages()
    /// }
    /// ```
    pub fn has_report_pages(&mut self) -> bool {
        self.report_pages.0.is_some()
    }

    /// Records a derivation note (see [`DeriveNote`]). Notes form a set: pushing one twice keeps one.
    pub fn note(&self, note: DeriveNote) {
        if let Some(top) = self.notes.borrow_mut().last_mut() {
            top.insert(note);
        }
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

    /// The reference of the held asset with this hash (format, intrinsic size): the company logo of the earlier
    /// bare-hash form, which names no format (FT-337), and the asset of an image page item. Recorded as a read of the
    /// project row (`project.company.set` changes the logo the answer is asked for) and of the asset store
    /// ([`DepKey::Assets`]): assets are only ever added, and one arrives in any transaction that carries it, e.g.
    /// an image item placed on another page with the file as `data`, which changes no row this reader read.
    pub fn logo_asset(&self, sha256: &ew_schema::Sha256) -> Option<ew_schema::AssetRef> {
        self.read(DepKey::Project, self.model.project_row().changed_at());
        self.read(DepKey::Assets, self.model.assets_stamp());
        self.model.asset(sha256).map(|a| *a.reference())
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

    /// Page items (graphics, free texts, wire markers and the variants kept verbatim).
    pub fn page_items(&self) -> TableView<'_, PageItem> {
        self.table()
    }

    /// Structure boxes.
    pub fn structure_boxes(&self) -> TableView<'_, StructureBox> {
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

    /// Embedded manufacturer copies (WP-S1.19 D10).
    pub fn manufacturers(&self) -> TableView<'_, LibCopy<Manufacturer>> {
        self.table()
    }

    /// The records of a reserved kind (one without a typed table in this format version, such as
    /// `waiver`), in the writer's line order. They carry no revision stamps, so the read is recorded as
    /// the coarse [`DepKey::Reserved`] at `model.rev()`. Push invalidation touches it only when the reserved
    /// records themselves differ (a stamp change with equal content need not be pushed, `ew_memo::Db`), so a
    /// memo that reads them is not re-marked by every commit; a pull verification still sees the newer stamp
    /// and re-executes. Revisions are typed records with their own table and stamps since FT-165
    /// ([`Q::table`]).
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
        if let Some(store) = self.memo_store() {
            let input = MemoInput::Read(dep);
            let durability = memo_keys::durability(&input);
            store.read_at(input, ew_memo::Rev(observed.0), durability);
        }
    }
}

/// The memo engine's view of a `Q`'s snapshot (ADR-0002 amendment 4): the stamps of the tracked reads, their
/// durability and their page-level registration keys (`memo_keys`).
impl ew_memo::Db for Q {
    type Input = MemoInput;

    fn changed_at(&self, input: &MemoInput) -> ew_memo::Rev {
        match input {
            MemoInput::Read(dep) => ew_memo::Rev(memo_keys::stamp(&self.model, dep).0),
            // Registration keys and parts are never recorded as input dependencies; conservative.
            _ => ew_memo::Rev(self.model.rev().0),
        }
    }

    fn durability(&self, input: &MemoInput) -> ew_memo::Durability {
        memo_keys::durability(input)
    }

    fn coarse(&self, input: &MemoInput) -> MemoInput {
        memo_keys::coarse(&self.model, input)
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

    /// The page items of a page.
    pub fn page_items_by_page(&self, page: Id<Page>) -> impl Iterator<Item = Id<PageItem>> + 'q {
        self.by_id(
            IndexName::PageItemsByPage,
            self.indexes().page_items_by_page(),
            page,
        )
    }

    /// The structure boxes of a page.
    pub fn structure_boxes_by_page(&self, page: Id<Page>) -> impl Iterator<Item = Id<StructureBox>> + 'q {
        self.by_id(
            IndexName::StructureBoxesByPage,
            self.indexes().structure_boxes_by_page(),
            page,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Arc;

    use super::*;
    use crate::memo_keys::test_common::Fx;
    use crate::query::{Policy, Query};

    thread_local! {
        /// The evaluations of [`Inner`] on this thread.
        static INNER_RUNS: Cell<usize> = const { Cell::new(0) };
    }

    /// A cold family that pushes a note and counts its evaluations; key 99 panics.
    struct Inner;

    impl Query for Inner {
        type Key = u8;
        type Value = u8;
        const FAMILY: &'static str = "test_detached_inner";
        const POLICY: Policy = Policy::Cold;

        fn run(q: &Q, key: &u8) -> Arc<u8> {
            INNER_RUNS.with(|c| c.set(c.get() + 1));
            assert!(*key != 99, "inner panics on 99");
            q.note(DeriveNote::DefaultFrame);
            Arc::new(*key)
        }
    }

    /// A cold family that reads `Inner(0)` (or `Inner(99)` for key 99) inside a detached scope, as the
    /// note-free connectivity families read their helpers.
    struct Outer;

    impl Query for Outer {
        type Key = u8;
        type Value = u8;
        const FAMILY: &'static str = "test_detached_outer";
        const POLICY: Policy = Policy::Cold;

        fn run(q: &Q, key: &u8) -> Arc<u8> {
            let inner = if *key == 99 { 99 } else { 0 };
            Arc::new(q.detached(|| *q.get::<Inner>(inner)).wrapping_add(*key))
        }
    }

    fn runs() -> usize {
        INNER_RUNS.with(Cell::get)
    }

    fn reset() {
        INNER_RUNS.with(|c| c.set(0));
    }

    /// Review cycle 10, finding 1: every detached scope had a cache of its own, so a family read by many
    /// scopes was evaluated once per scope. Now it is evaluated once per request, its notes stay out of the
    /// detached scopes, and a read outside the scopes replays them.
    #[test]
    fn detached_scopes_share_the_request_cache_and_keep_notes_out() {
        let model = Fx::new().build();
        reset();
        let mut q = Q::new(&model);
        for k in 1..=20 {
            assert_eq!(*q.get::<Outer>(k), k);
        }
        assert_eq!(runs(), 1, "Inner(0) is evaluated once for twenty detached scopes");
        assert!(
            q.diagnostics().notes().is_empty(),
            "the notes of a detached read are dropped"
        );
        assert_eq!(*q.get::<Inner>(0), 0);
        assert_eq!(runs(), 1, "the read outside the scopes is a hit");
        assert_eq!(
            q.diagnostics().notes(),
            vec![DeriveNote::DefaultFrame],
            "the hit replays the notes the detached evaluation captured"
        );
    }

    /// A value computed before a detached scope is a hit inside it, and its notes are pushed once.
    #[test]
    fn a_value_computed_outside_is_a_hit_inside_a_detached_scope() {
        let model = Fx::new().build();
        reset();
        let mut q = Q::new(&model);
        assert_eq!(*q.get::<Inner>(0), 0);
        assert_eq!(*q.get::<Outer>(5), 5);
        assert_eq!(runs(), 1);
        assert_eq!(q.diagnostics().notes(), vec![DeriveNote::DefaultFrame]);
    }

    /// A panic inside a capturing evaluation unwinds the note frames and the detached counter: the `Q` stays
    /// usable and later reads behave as in a fresh `Q`.
    #[test]
    fn a_panic_inside_a_detached_scope_unwinds_its_frames() {
        let model = Fx::new().build();
        reset();
        let mut q = Q::new(&model);
        let caught = catch_unwind(AssertUnwindSafe(|| q.get::<Outer>(99)));
        assert!(caught.is_err());
        assert_eq!(
            q.notes.borrow().len(),
            1,
            "the capture and scope frames are popped"
        );
        assert_eq!(q.cache.borrow().len(), 1);
        assert_eq!(q.cache.borrow()[0].detached, 0, "the scope is closed");
        assert_eq!(*q.get::<Inner>(0), 0);
        assert_eq!(q.diagnostics().notes(), vec![DeriveNote::DefaultFrame]);
    }

    /// A memoised family whose execution reads `Outer` for ten keys.
    struct Memoised;

    impl Query for Memoised {
        type Key = ();
        type Value = u32;
        const FAMILY: &'static str = "test_detached_memoised";
        const POLICY: Policy = Policy::Memo;

        fn run(q: &Q, _: &()) -> Arc<u32> {
            let mut sum = 0u32;
            for k in 1..=10u8 {
                sum += u32::from(*q.get::<Outer>(k));
            }
            Arc::new(sum)
        }
    }

    /// In a memoising `Q` a cold family read inside a detached scope of a memo execution is cached in that
    /// execution's request cache: the scopes of one execution share it.
    #[test]
    fn detached_scopes_inside_a_memo_execution_share_its_cache() {
        let model = Fx::new().build();
        reset();
        let store = crate::MemoStore::new(&model);
        let mut q = store.query();
        assert_eq!(*q.get::<Memoised>(()), 55);
        assert_eq!(runs(), 1, "one evaluation of Inner(0) in the memo execution");
        assert!(
            q.diagnostics().notes().is_empty(),
            "the memoised value carries no detached note"
        );
    }
}
