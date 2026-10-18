// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`Model`] snapshot (ARCH §4.1): one table per entity kind, the project row, the embedded library
//! copies, unknown records and the base indexes.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_schema::{
    Bond, Conductor, Connection, Device, ExternalRef, Function, ImportLineage, Junction, Kind, LibDocument,
    Page, PotentialDef, Project, Record, ReportBlock, Representation, Section, StructureNode,
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
    pub(crate) potential_defs: Table<PotentialDef>,
    pub(crate) bonds: Table<Bond>,
    pub(crate) report_blocks: Table<ReportBlock>,
    pub(crate) lib: LibraryCopies,
    /// Records of unknown or reserved kinds, kept for the writer and in canonical-line order; they take
    /// part in nothing else.
    pub(crate) unknown: Arc<Vec<Record>>,
    /// Library documents of unknown kinds, likewise.
    pub(crate) unknown_lib: Arc<Vec<LibDocument>>,
    pub(crate) idx: BaseIndexes,
}

impl Model {
    /// An empty model holding only the project row, at [`Rev::ZERO`].
    pub fn new(project: Project) -> Model {
        Model {
            rev: Rev::ZERO,
            project: Row::new(Arc::new(project), Rev::ZERO),
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
            potential_defs: Table::default(),
            bonds: Table::default(),
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
    /// are ordered maps, and unknown records are kept in canonical-line order. Two records of one kind
    /// with one ID, or a project record among `records`, are [`ModelError::DuplicateRecord`] (the
    /// smallest such key).
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
                Record::PotentialDef(r) => Some(AnyRow::of(r)),
                Record::Bond(r) => Some(AnyRow::of(r)),
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
        unknown.sort_by_cached_key(|r| serde_json::to_string(r).unwrap_or_default());
        unknown_lib.sort_by_cached_key(|d| serde_json::to_string(d).unwrap_or_default());
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

    /// The project record.
    pub fn project(&self) -> &Arc<Project> {
        &self.project.value
    }

    /// The project row with its `changed_at`.
    pub fn project_row(&self) -> &Row<Project> {
        &self.project
    }

    /// The table of entity kind `T`.
    pub fn table<T: Entity>(&self) -> &Table<T> {
        T::table(self)
    }

    /// The embedded library copies.
    pub fn lib(&self) -> &LibraryCopies {
        &self.lib
    }

    /// The base indexes.
    pub fn indexes(&self) -> &BaseIndexes {
        &self.idx
    }

    /// Records of unknown or reserved kinds, in canonical-line order.
    pub fn unknown_records(&self) -> &[Record] {
        &self.unknown
    }

    /// Library documents of unknown kinds, in canonical order.
    pub fn unknown_lib_documents(&self) -> &[LibDocument] {
        &self.unknown_lib
    }

    /// The current row of a key, kind-erased (what a transaction puts into an op's `before`).
    pub fn row(&self, key: &AnyKey) -> Option<AnyRow> {
        get_row(self, key)
    }

    /// Whether a row exists under the key.
    pub fn contains(&self, key: &AnyKey) -> bool {
        get_row(self, key).is_some()
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

    /// The records that name `target`, with the link they use, in link order then key order.
    pub fn referrers(&self, target: &AnyKey) -> Vec<(&'static LinkDecl, AnyKey)> {
        links_to(target.0)
            .flat_map(|(lid, link)| {
                self.referrers_via(lid, target)
                    .into_iter()
                    .map(move |id| (link, (link.from, id)))
            })
            .collect()
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

    /// The ops a transaction must contain to remove `key` under the delete policies (doc 03 §4.2,
    /// FR-DEV-030): the removal itself, every `cascade` referrer (transitively), every `unlink` referrer
    /// with its reference cleared (or dropped, for a mandatory field), modifications first, then removals,
    /// each in key order.
    ///
    /// A referrer through a `forbid` link that the plan does not remove itself is
    /// [`ModelError::Referenced`]. The plan covers link policies only: other invariants (one main function
    /// per device, for instance) are the transaction's business and are checked by [`Model::apply`].
    pub fn delete_plan(&self, key: &AnyKey) -> Result<Vec<Op>, ModelError> {
        if key.0 == Kind::Project {
            return Err(ModelError::Malformed {
                key: *key,
                reason: "the project row cannot be removed",
            });
        }
        if get_row(self, key).is_none() {
            return Err(ModelError::NotFound { key: *key });
        }
        let mut deleted: BTreeSet<AnyKey> = BTreeSet::from([*key]);
        let mut work = vec![*key];
        let mut blockers: BTreeSet<(AnyKey, LinkId, AnyKey)> = BTreeSet::new();
        let mut unlinks: BTreeMap<AnyKey, BTreeSet<LinkId>> = BTreeMap::new();
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
