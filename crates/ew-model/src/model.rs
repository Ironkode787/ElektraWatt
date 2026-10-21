// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`Model`] snapshot (ARCH §4.1): one table per entity kind, the project row, the embedded library
//! copies, unknown records and the base indexes.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_schema::{
    Bond, Channel, Conductor, Connection, Device, DeviceLink, ExternalRef, Function, FunctionKind,
    ImportLineage, Junction, Kind, KindRegistry, LibDocument, Page, PlcConfigProject, PotentialDef, Project,
    Record, ReportBlock, Representation, Section, StructureNode,
};

use crate::entity::{AnyKey, AnyRow, Entity, LibraryCopies, get_row, put_row, same_tables};
use crate::error::ModelError;
use crate::index::BaseIndexes;
use crate::invariants::{self, Scope};
use crate::links::{LINKS, LinkDecl, LinkId, Policy, links_to};
use crate::op::Op;
use crate::prevent::Strictness;
use crate::rev::{Rev, Row, Table};

/// One immutable snapshot of a project in memory. `Clone` is O(1): every table and index is a persistent
/// map, rows are `Arc`s.
///
/// The model changes only through [`Model::apply`]. Reading is public; the fields are crate-private (ARCH
/// §3 rule 1).
#[derive(Clone, Debug)]
pub struct Model {
    pub(crate) rev: Rev,
    pub(crate) project: Row<Project>,
    pub(crate) plc_config_projects: Table<PlcConfigProject>,
    pub(crate) lineages: Table<ImportLineage>,
    pub(crate) structure: Table<StructureNode>,
    pub(crate) sections: Table<Section>,
    pub(crate) ext_refs: Table<ExternalRef>,
    pub(crate) pages: Table<Page>,
    pub(crate) devices: Table<Device>,
    pub(crate) functions: Table<Function>,
    pub(crate) reps: Table<Representation>,
    pub(crate) connections: Table<Connection>,
    pub(crate) junctions: Table<Junction>,
    pub(crate) conductors: Table<Conductor>,
    pub(crate) channels: Table<Channel>,
    pub(crate) potential_defs: Table<PotentialDef>,
    pub(crate) bonds: Table<Bond>,
    pub(crate) device_links: Table<DeviceLink>,
    pub(crate) report_blocks: Table<ReportBlock>,
    pub(crate) lib: LibraryCopies,
    /// Records of unknown or reserved kinds, kept for the writer in the line order of doc 05 §3.3; they
    /// take part in nothing else.
    pub(crate) unknown: Arc<Vec<Record>>,
    /// Library documents of unknown kinds, likewise (by `$k`, then ID).
    pub(crate) unknown_lib: Arc<Vec<LibDocument>>,
    pub(crate) idx: BaseIndexes,
}

impl Model {
    /// An empty model holding only the project row, at [`Rev::ZERO`].
    pub fn new(project: Project) -> Model {
        Model {
            rev: Rev::ZERO,
            project: Row::new(Arc::new(project), Rev::ZERO),
            plc_config_projects: Table::default(),
            lineages: Table::default(),
            structure: Table::default(),
            sections: Table::default(),
            ext_refs: Table::default(),
            pages: Table::default(),
            devices: Table::default(),
            functions: Table::default(),
            reps: Table::default(),
            connections: Table::default(),
            junctions: Table::default(),
            conductors: Table::default(),
            channels: Table::default(),
            potential_defs: Table::default(),
            bonds: Table::default(),
            device_links: Table::default(),
            report_blocks: Table::default(),
            lib: LibraryCopies::default(),
            unknown: Arc::new(Vec::new()),
            unknown_lib: Arc::new(Vec::new()),
            idx: BaseIndexes::default(),
        }
    }

    /// Builds a model from loaded records at [`Rev::ZERO`]: fills the tables, rebuilds the indexes and
    /// runs the whole invariant catalogue (the `ew check --integrity` semantics of ADR-0009 decision 5).
    ///
    /// The result does not depend on the order of `records` and `lib` (NFR-REL-07): tables and indexes
    /// are ordered maps, and records and documents of kinds without a table are kept in the line order of
    /// doc 05 §3.3 (kind rank, then the ID's bytes 8–15 big-endian, then the full ID; kinds without a rank
    /// after ranked ones, by `$k` text).
    ///
    /// Two records of one kind with one ID, or a project record among `records`, are
    /// [`ModelError::DuplicateRecord`] (the smallest such key). Two records or documents of a kind without a
    /// table with one `($k, id)` are `DuplicateRecord` too when the registry knows the kind (a reserved
    /// kind), else [`ModelError::DuplicateUnknownRecord`] (EW-CORE-009).
    pub fn from_records(
        project: Project,
        records: impl IntoIterator<Item = Record>,
        lib: impl IntoIterator<Item = LibDocument>,
    ) -> Result<Model, ModelError> {
        let mut m = Model::new(project);
        let mut duplicates = BTreeSet::new();
        let mut unknown = Vec::new();
        let mut unknown_lib = Vec::new();
        let rows = records
            .into_iter()
            .filter_map(|r| match r {
                Record::Project(p) => Some(AnyRow::Project(Arc::new(p))),
                Record::PlcConfigProject(r) => Some(AnyRow::of(r)),
                Record::ImportLineage(r) => Some(AnyRow::of(r)),
                Record::StructureNode(r) => Some(AnyRow::of(r)),
                Record::Section(r) => Some(AnyRow::of(r)),
                Record::ExternalRef(r) => Some(AnyRow::of(r)),
                Record::Page(r) => Some(AnyRow::of(r)),
                Record::Device(r) => Some(AnyRow::of(r)),
                Record::Function(r) => Some(AnyRow::of(r)),
                Record::Representation(r) => Some(AnyRow::of(r)),
                Record::Connection(r) => Some(AnyRow::of(r)),
                Record::Junction(r) => Some(AnyRow::of(r)),
                Record::Conductor(r) => Some(AnyRow::of(r)),
                Record::Channel(r) => Some(AnyRow::of(r)),
                Record::PotentialDef(r) => Some(AnyRow::of(r)),
                Record::Bond(r) => Some(AnyRow::of(r)),
                Record::DeviceLink(r) => Some(AnyRow::of(r)),
                Record::ReportBlock(r) => Some(AnyRow::of(r)),
                other => {
                    unknown.push(other);
                    None
                }
            })
            .collect::<Vec<_>>();
        let lib_rows = lib
            .into_iter()
            .filter_map(|d| match d {
                LibDocument::Part(d) => Some(AnyRow::of(d)),
                LibDocument::Symbol(d) => Some(AnyRow::of(d)),
                LibDocument::Fndef(d) => Some(AnyRow::of(d)),
                LibDocument::Form(d) => Some(AnyRow::of(d)),
                other => {
                    unknown_lib.push(other);
                    None
                }
            })
            .collect::<Vec<_>>();
        for row in rows.iter().chain(lib_rows.iter()) {
            let key = row.key();
            if key.0 == Kind::Project || get_row(&m, &key).is_some() {
                duplicates.insert(key);
                continue;
            }
            put_row(&mut m, &key, Some(row), Rev::ZERO);
        }
        if let Some(key) = duplicates.first() {
            return Err(ModelError::DuplicateRecord { key: *key });
        }
        sort_unknown(&mut unknown, |r| match r {
            Record::Unknown(v) => v,
            _ => unreachable!("only records of unknown kinds are kept aside"),
        })?;
        sort_unknown(&mut unknown_lib, |d| match d {
            LibDocument::Unknown(v) => v,
            _ => unreachable!("only documents of unknown kinds are kept aside"),
        })?;
        m.unknown = Arc::new(unknown);
        m.unknown_lib = Arc::new(unknown_lib);
        m.idx = BaseIndexes::rebuild(&m);
        m.integrity_check()?;
        Ok(m)
    }

    /// Evaluates every structural invariant on every record; the first violation, in catalogue order and
    /// then key order, is the error.
    pub fn integrity_check(&self) -> Result<(), ModelError> {
        invariants::check(self, &Scope::full(self))
    }

    /// The model's revision.
    pub fn rev(&self) -> Rev {
        self.rev
    }

    raw_read! {
        /// The project record.
        fn project(&self) -> &Arc<Project> {
            &self.project.value
        }
    }

    raw_read! {
        /// The project row with its `changed_at`.
        fn project_row(&self) -> &Row<Project> {
            &self.project
        }
    }

    raw_read! {
        /// The table of entity kind `T`.
        fn table<T: Entity>(&self) -> &Table<T> {
            T::table(self)
        }
    }

    raw_read! {
        /// The embedded library copies.
        fn lib(&self) -> &LibraryCopies {
            &self.lib
        }
    }

    raw_read! {
        /// The base indexes.
        fn indexes(&self) -> &BaseIndexes {
            &self.idx
        }
    }

    raw_read! {
        /// Records of unknown or reserved kinds, in the line order of doc 05 §3.3.
        fn unknown_records(&self) -> &[Record] {
            &self.unknown
        }
    }

    raw_read! {
        /// Library documents of unknown kinds, by `$k`, then ID (as records of kinds without a rank).
        fn unknown_lib_documents(&self) -> &[LibDocument] {
            &self.unknown_lib
        }
    }

    raw_read! {
        /// The current row of a key, kind-erased (what a transaction puts into an op's `before`).
        fn row(&self, key: &AnyKey) -> Option<AnyRow> {
            get_row(self, key)
        }
    }

    raw_read! {
        /// Whether a row exists under the key.
        fn contains(&self, key: &AnyKey) -> bool {
            get_row(self, key).is_some()
        }
    }

    /// The project's strictness for prevent rules ([`Strictness::of`]).
    pub fn strictness(&self) -> Strictness {
        Strictness::of(&self.project.value)
    }

    /// Whether both models hold the same records, unknown records and index entries (`rev` and
    /// `changed_at` stamps are ignored).
    pub fn content_eq(&self, other: &Model) -> bool {
        (Arc::ptr_eq(&self.project.value, &other.project.value) || self.project.value == other.project.value)
            && same_tables(self, other)
            && self.unknown == other.unknown
            && self.unknown_lib == other.unknown_lib
            && self.idx.same_entries(&other.idx)
    }

    raw_read! {
        /// The records that name `target`, with the link they use, in link order then key order.
        fn referrers(&self, target: &AnyKey) -> Vec<(&'static LinkDecl, AnyKey)> {
            links_to(target.0)
                .flat_map(|(lid, link)| {
                    self.referrers_via(lid, target)
                        .into_iter()
                        .map(move |id| (link, (link.from, id)))
                })
                .collect()
        }
    }

    /// The referrers of `target` through one link, verified against their rows.
    pub(crate) fn referrers_via(&self, link_id: LinkId, target: &AnyKey) -> Vec<ew_schema::AnyId> {
        let link = &LINKS[link_id];
        self.idx
            .candidates(link_id, target)
            .into_iter()
            .filter(|src| {
                get_row(self, &(link.from, *src))
                    .is_some_and(|row| link.targets_of(&row).iter().any(|t| t.id() == target.1))
            })
            .collect()
    }

    raw_read! {
        /// The ops a transaction must contain to remove `key` under the delete policies (doc 03 §4.2,
        /// FR-DEV-030): the removal itself, every `cascade` referrer (transitively), every `unlink` referrer
        /// with its reference cleared (or dropped, for a mandatory field), modifications first, then removals,
        /// each in key order.
        ///
        /// Besides the link policies, the plan applies one ownership rule (doc 05 §2.2 row 32, EW-INV-013): an
        /// interruption-point function is owned by its representation, so removing its last representation
        /// removes the function as well, and with it the connections at its CPs (`connection.a`/`b` cascade)
        /// and the definitions and bonds there (unlink, dropped).
        ///
        /// A referrer through a `forbid` link that the plan does not remove itself is
        /// [`ModelError::Referenced`]. The plan covers link policies only: other invariants (one main function
        /// per device, for instance) are the transaction's business and are checked by [`Model::apply`].
        ///
        /// Gated by `raw-read`, like the other untracked readers: the plan carries the current rows of the
        /// removed and modified records in its ops (`before`), so it reads the model. A transaction builder
        /// that holds the feature (`TxBuilder` in `ew-commands`) calls it; [`Model::apply`] consumes the ops.
        fn delete_plan(&self, key: &AnyKey) -> Result<Vec<Op>, ModelError> {
            self.delete_plan_many(std::slice::from_ref(key))
        }
    }

    raw_read! {
        /// The plan of [`Model::delete_plan`] for several roots at once: one plan that removes every key
        /// in `keys` with everything the delete policies take along. The result does not depend on the
        /// order of `keys`, and a root that another root's cascade removes is planned once. Every root
        /// must exist ([`ModelError::NotFound`] for the first missing one, in the order given).
        fn delete_plan_many(&self, keys: &[AnyKey]) -> Result<Vec<Op>, ModelError> {
            for key in keys {
                if key.0 == Kind::Project {
                    return Err(ModelError::Malformed {
                        key: *key,
                        reason: "the project row cannot be removed",
                    });
                }
                if get_row(self, key).is_none() {
                    return Err(ModelError::NotFound { key: *key });
                }
            }
            let mut deleted: BTreeSet<AnyKey> = keys.iter().copied().collect();
            let mut work: Vec<AnyKey> = deleted.iter().copied().collect();
            let mut blockers: BTreeSet<(AnyKey, LinkId, AnyKey)> = BTreeSet::new();
            let mut unlinks: BTreeMap<AnyKey, BTreeSet<LinkId>> = BTreeMap::new();
            loop {
                while let Some(target) = work.pop() {
                    for (lid, link) in links_to(target.0) {
                        for src in self.referrers_via(lid, &target) {
                            let referrer = (link.from, src);
                            let drop = match (link.policy, link.unlink) {
                                (Policy::Cascade, _) => true,
                                (Policy::Forbid, _) => {
                                    blockers.insert((referrer, lid, target));
                                    false
                                }
                                (Policy::Unlink, Some(unlink)) => {
                                    let row = get_row(self, &referrer).expect("verified referrer");
                                    let dropped = unlink(&row, &deleted).is_none();
                                    if !dropped {
                                        unlinks.entry(referrer).or_default().insert(lid);
                                    }
                                    dropped
                                }
                                (Policy::Unlink, None) => {
                                    unreachable!("unlink link {} without unlink fn", link.path())
                                }
                            };
                            if drop && deleted.insert(referrer) {
                                work.push(referrer);
                            }
                        }
                    }
                }
                let orphans = self.orphaned_ip_functions(&deleted);
                if orphans.is_empty() {
                    break;
                }
                for f in orphans {
                    deleted.insert(f);
                    work.push(f);
                }
            }
            if let Some((referrer, lid, target)) = blockers.iter().find(|(r, _, _)| !deleted.contains(r)) {
                return Err(ModelError::Referenced {
                    link: LINKS[*lid].path(),
                    target: *target,
                    referrer: *referrer,
                });
            }
            let mut ops = Vec::new();
            for (referrer, lids) in &unlinks {
                if deleted.contains(referrer) {
                    continue;
                }
                let before = get_row(self, referrer).expect("verified referrer");
                let mut after = before.clone();
                for lid in lids {
                    let unlink = LINKS[*lid].unlink.expect("unlink link");
                    after = unlink(&after, &deleted).expect("a modifying unlink keeps its record");
                }
                ops.push(Op::update(before, after));
            }
            for k in &deleted {
                ops.push(Op::delete(get_row(self, k).expect("planned row exists")));
            }
            Ok(ops)
        }
    }

    /// The interruption-point functions that a planned removal leaves without a representation: named by a
    /// removed representation, not removed themselves, and every representation of theirs removed.
    fn orphaned_ip_functions(&self, deleted: &BTreeSet<AnyKey>) -> BTreeSet<AnyKey> {
        let mut out = BTreeSet::new();
        let reps = deleted
            .range((Kind::Representation, ew_ids::Id::from_uuid(ew_ids::Uuid::nil()))..)
            .take_while(|k| k.0 == Kind::Representation);
        for key in reps {
            let Some(rep) = self.reps.get(&key.1.cast()) else {
                continue;
            };
            for f in rep
                .functions
                .iter()
                .chain(rep.pin_map.iter().map(|p| &p.function))
            {
                let fkey = (Kind::Function, f.cast());
                if deleted.contains(&fkey) || out.contains(&fkey) {
                    continue;
                }
                let is_ip = self
                    .functions
                    .get(f)
                    .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_)));
                let all_reps_removed = self
                    .idx
                    .reps_by_function
                    .get(f)
                    .all(|r| deleted.contains(&(Kind::Representation, r.cast())));
                if is_ip && all_reps_removed {
                    out.insert(fkey);
                }
            }
        }
        out
    }
}

/// The sort key of a record or library document of a kind without a table: doc 05 §3.3's line order.
///
/// 1. Kinds with a rank in the [`KindRegistry`] (reserved record kinds) come first, by rank; kinds without
///    one (newer kinds, library kinds) follow, by their `$k` text.
/// 2. Then the ID's bytes 8–15 read as a big-endian `u64` (its random bits), then the full ID; a record
///    without a UUID `id` sorts after those with one.
/// 3. Then the canonical compact JSON, so that the order is total and independent of the input order.
type UnknownOrder = ((u8, u8, String), (bool, u64, [u8; 16]), String);

fn unknown_kind_and_id(v: &serde_json::Value) -> (Option<&str>, Option<ew_ids::Uuid>) {
    let k = v.get("$k").and_then(serde_json::Value::as_str);
    let id = v
        .get("id")
        .and_then(serde_json::Value::as_str)
        .and_then(|s| s.parse::<ew_ids::Id<()>>().ok())
        .map(|id| id.as_uuid());
    (k, id)
}

fn unknown_order(v: &serde_json::Value) -> UnknownOrder {
    let (k, id) = unknown_kind_and_id(v);
    let kind = match k.and_then(Kind::from_name).and_then(KindRegistry::rank) {
        Some(rank) => (0, rank, String::new()),
        None => (1, 0, k.unwrap_or_default().to_owned()),
    };
    let id = match id {
        Some(u) => {
            let b = *u.as_bytes();
            let random = u64::from_be_bytes(b[8..16].try_into().expect("eight bytes"));
            (false, random, b)
        }
        None => (true, 0, [0; 16]),
    };
    (kind, id, serde_json::to_string(v).unwrap_or_default())
}

/// Sorts records or documents of unknown kinds into line order and rejects a duplicate `($k, id)`.
fn sort_unknown<T>(items: &mut [T], value: impl Fn(&T) -> &serde_json::Value) -> Result<(), ModelError> {
    items.sort_by_cached_key(|item| unknown_order(value(item)));
    let mut seen: BTreeSet<(&str, ew_ids::Uuid)> = BTreeSet::new();
    let mut duplicates: BTreeSet<(&str, ew_ids::Uuid)> = BTreeSet::new();
    for item in items.iter() {
        if let (Some(k), Some(id)) = unknown_kind_and_id(value(item)) {
            if !seen.insert((k, id)) {
                duplicates.insert((k, id));
            }
        }
    }
    match duplicates.first() {
        None => Ok(()),
        Some((k, id)) => Err(match Kind::from_name(k) {
            Some(kind) => ModelError::DuplicateRecord {
                key: (kind, ew_ids::Id::from_uuid(*id)),
            },
            None => ModelError::DuplicateUnknownRecord {
                kind: (*k).to_owned(),
                id: ew_ids::Id::from_uuid(*id),
            },
        }),
    }
}
