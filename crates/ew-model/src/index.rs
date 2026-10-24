// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Base reverse indexes (ARCH §4.1, ADR-0001 decision 5): pure reverse-FK maps, each key with its own
//! `changed_at`, maintained by delta by [`Model::apply_with`](crate::Model::apply_with) and
//! [`Model::apply_history`](crate::Model::apply_history), rebuilt on load, never saved.
//!
//! Every link of [`LINKS`] feeds exactly one index: its named index, or the generic referrer index. So
//! every link can be followed backwards in `O(log n)`, which the delete policies and the invariant
//! "references resolve" rely on.

use std::collections::BTreeSet;
use std::ops::{RangeBounds, RangeInclusive};

use ew_ids::Id;
use ew_schema::{
    AnyId, Bond, Conductor, Connection, Device, Endpoint, Function, Junction, Kind, Page, PageItem,
    PotentialDef, Representation, Section, StructureBox, StructureNode,
};
use imbl::{OrdMap, OrdSet};

use crate::entity::{AnyKey, AnyRow};
use crate::links::{IndexName, LINKS, LinkId, Target, links_from};
use crate::rev::Rev;

/// The values of one index key and the revision that last changed them.
#[derive(Clone, Debug)]
pub struct Bucket<V: Ord + Clone> {
    set: OrdSet<V>,
    changed_at: Rev,
}

impl<V: Ord + Clone> Bucket<V> {
    /// The values in order.
    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.set.iter()
    }

    /// Whether the bucket holds `v`.
    pub fn contains(&self, v: &V) -> bool {
        self.set.contains(v)
    }

    /// The number of values (never zero: empty buckets are removed).
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Always `false`: empty buckets are removed from their index.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// The revision that last added or removed a value of this key.
    pub fn changed_at(&self) -> Rev {
        self.changed_at
    }
}

/// One reverse index: key → ordered set of referrer IDs, with a `changed_at` per key and one for the
/// whole index.
///
/// A key whose last value is removed disappears; the index-level [`RevIndex::changed_at`] covers readers
/// that observed its absence.
#[derive(Clone, Debug)]
pub struct RevIndex<K: Ord + Clone, V: Ord + Clone> {
    map: OrdMap<K, Bucket<V>>,
    changed_at: Rev,
}

impl<K: Ord + Clone, V: Ord + Clone> Default for RevIndex<K, V> {
    fn default() -> Self {
        RevIndex {
            map: OrdMap::new(),
            changed_at: Rev::ZERO,
        }
    }
}

impl<K: Ord + Clone, V: Ord + Clone> RevIndex<K, V> {
    /// The values of a key, in order (empty for an absent key).
    pub fn get(&self, key: &K) -> impl Iterator<Item = &V> + '_ {
        self.map.get(key).into_iter().flat_map(|b| b.set.iter())
    }

    /// The bucket of a key.
    pub fn bucket(&self, key: &K) -> Option<&Bucket<V>> {
        self.map.get(key)
    }

    /// Whether `key` maps to `v`.
    pub fn contains(&self, key: &K, v: &V) -> bool {
        self.map.get(key).is_some_and(|b| b.set.contains(v))
    }

    /// The revision that last changed a key's values.
    pub fn key_changed_at(&self, key: &K) -> Option<Rev> {
        self.map.get(key).map(|b| b.changed_at)
    }

    /// The revision that last changed any key.
    pub fn changed_at(&self) -> Rev {
        self.changed_at
    }

    /// Every key with its bucket, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &Bucket<V>)> + '_ {
        self.map.iter()
    }

    /// The keys in a range, with their buckets.
    pub fn range<R: RangeBounds<K>>(&self, range: R) -> impl Iterator<Item = (&K, &Bucket<V>)> + '_ {
        self.map.range(range)
    }

    /// The number of keys.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the index has no key.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Whether both indexes hold the same keys and values (`changed_at` stamps are ignored).
    pub fn same_entries(&self, other: &Self) -> bool {
        self.map.len() == other.map.len()
            && self
                .map
                .iter()
                .zip(other.map.iter())
                .all(|((ka, ba), (kb, bb))| ka == kb && ba.set == bb.set)
    }

    pub(crate) fn insert(&mut self, key: K, v: V, rev: Rev) {
        let bucket = self.map.entry(key).or_insert_with(|| Bucket {
            set: OrdSet::new(),
            changed_at: rev,
        });
        bucket.set.insert(v);
        bucket.changed_at = rev;
        self.changed_at = rev;
    }

    pub(crate) fn remove(&mut self, key: &K, v: &V, rev: Rev) {
        if let Some(bucket) = self.map.get_mut(key) {
            bucket.set.remove(v);
            bucket.changed_at = rev;
            if bucket.set.is_empty() {
                self.map.remove(key);
            }
        }
        self.changed_at = rev;
    }
}

/// An index key for an endpoint: a connection point `(function, idx)` or a junction. CPs sort before
/// junctions, and a function's CPs are contiguous ([`EndpointKey::cps_of`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EndpointKey {
    /// Connection point `idx` of a function.
    Cp(Id<Function>, u16),
    /// A junction.
    Junction(Id<Junction>),
}

impl EndpointKey {
    /// The key of an endpoint; `None` for an endpoint of a newer format.
    pub fn of(e: &Endpoint) -> Option<EndpointKey> {
        match e {
            Endpoint::Cp(f, idx) => Some(EndpointKey::Cp(*f, *idx)),
            Endpoint::Junction(j) => Some(EndpointKey::Junction(*j)),
            _ => None,
        }
    }

    /// The key range covering every CP of a function.
    pub fn cps_of(f: Id<Function>) -> RangeInclusive<EndpointKey> {
        EndpointKey::Cp(f, 0)..=EndpointKey::Cp(f, u16::MAX)
    }

    /// The record the endpoint names.
    pub fn target(self) -> AnyKey {
        match self {
            EndpointKey::Cp(f, _) => (Kind::Function, f.cast()),
            EndpointKey::Junction(j) => (Kind::Junction, j.cast()),
        }
    }
}

/// The base indexes of a model.
#[derive(Clone, Debug, Default)]
pub struct BaseIndexes {
    pub(crate) functions_by_device: RevIndex<Id<Device>, Id<Function>>,
    pub(crate) reps_by_page: RevIndex<Id<Page>, Id<Representation>>,
    pub(crate) reps_by_function: RevIndex<Id<Function>, Id<Representation>>,
    pub(crate) connections_by_page: RevIndex<Id<Page>, Id<Connection>>,
    pub(crate) connections_by_endpoint: RevIndex<EndpointKey, Id<Connection>>,
    pub(crate) junctions_by_page: RevIndex<Id<Page>, Id<Junction>>,
    pub(crate) terminals_by_strip: RevIndex<Id<Device>, Id<Device>>,
    pub(crate) conductors_by_cable: RevIndex<Id<Device>, Id<Conductor>>,
    pub(crate) structure_children: RevIndex<Id<StructureNode>, Id<StructureNode>>,
    pub(crate) device_children: RevIndex<Id<Device>, Id<Device>>,
    pub(crate) pages_by_section: RevIndex<Id<Section>, Id<Page>>,
    pub(crate) potential_defs_by_endpoint: RevIndex<EndpointKey, Id<PotentialDef>>,
    pub(crate) bonds_by_endpoint: RevIndex<EndpointKey, Id<Bond>>,
    pub(crate) page_items_by_page: RevIndex<Id<Page>, Id<PageItem>>,
    pub(crate) structure_boxes_by_page: RevIndex<Id<Page>, Id<StructureBox>>,
    pub(crate) referrers: RevIndex<AnyKey, (LinkId, AnyId)>,
}

/// The key part of one index entry.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum IxKey {
    Id(AnyId),
    Ep(EndpointKey),
}

impl IxKey {
    fn id(self) -> AnyId {
        match self {
            IxKey::Id(id) => id,
            IxKey::Ep(e) => e.target().1,
        }
    }

    fn ep(self) -> EndpointKey {
        match self {
            IxKey::Ep(e) => e,
            IxKey::Id(_) => unreachable!("endpoint index fed by a non-endpoint link"),
        }
    }
}

/// One entry a row contributes to the indexes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum IxEntry {
    Named(IndexName, IxKey, AnyId),
    Referrer(AnyKey, LinkId, AnyId),
}

fn endpoint_keyed(name: IndexName) -> bool {
    matches!(
        name,
        IndexName::ConnectionsByEndpoint | IndexName::PotentialDefsByEndpoint | IndexName::BondsByEndpoint
    )
}

/// The index entries of one row (a set: duplicates within one record collapse).
fn row_entries(row: &AnyRow) -> BTreeSet<IxEntry> {
    let source = row.id();
    let mut out = BTreeSet::new();
    let mut targets = Vec::new();
    for (link_id, link) in links_from(row.kind()) {
        targets.clear();
        (link.targets)(row, &mut targets);
        for t in &targets {
            out.insert(match link.index {
                Some(name) if endpoint_keyed(name) => {
                    let key = match *t {
                        Target::Cp(f, idx) => EndpointKey::Cp(f.cast(), idx),
                        Target::Id(j) => EndpointKey::Junction(j.cast()),
                    };
                    IxEntry::Named(name, IxKey::Ep(key), source)
                }
                Some(name) => IxEntry::Named(name, IxKey::Id(t.id()), source),
                None => IxEntry::Referrer((link.to, t.id()), link_id, source),
            });
        }
    }
    out
}

impl BaseIndexes {
    raw_read! {
        /// `Function.device` → functions.
        fn functions_by_device(&self) -> &RevIndex<Id<Device>, Id<Function>> {
            &self.functions_by_device
        }
    }

    raw_read! {
        /// `Representation.page` → representations.
        fn reps_by_page(&self) -> &RevIndex<Id<Page>, Id<Representation>> {
            &self.reps_by_page
        }
    }

    raw_read! {
        /// `Representation.functions` and `pin_map[].function` → representations.
        fn reps_by_function(&self) -> &RevIndex<Id<Function>, Id<Representation>> {
            &self.reps_by_function
        }
    }

    raw_read! {
        /// `Connection.page` → connections.
        fn connections_by_page(&self) -> &RevIndex<Id<Page>, Id<Connection>> {
            &self.connections_by_page
        }
    }

    raw_read! {
        /// `Connection.a`/`b` → connections, by CP `(function, idx)` or junction.
        fn connections_by_endpoint(&self) -> &RevIndex<EndpointKey, Id<Connection>> {
            &self.connections_by_endpoint
        }
    }

    raw_read! {
        /// `Junction.page` → junctions.
        fn junctions_by_page(&self) -> &RevIndex<Id<Page>, Id<Junction>> {
            &self.junctions_by_page
        }
    }

    raw_read! {
        /// Strip device → terminal devices whose `terminal.strip` names it.
        fn terminals_by_strip(&self) -> &RevIndex<Id<Device>, Id<Device>> {
            &self.terminals_by_strip
        }
    }

    raw_read! {
        /// Cable device → conductors.
        fn conductors_by_cable(&self) -> &RevIndex<Id<Device>, Id<Conductor>> {
            &self.conductors_by_cable
        }
    }

    raw_read! {
        /// Structure node → child nodes (`children_by_parent` of the structure trees).
        fn structure_children(&self) -> &RevIndex<Id<StructureNode>, Id<StructureNode>> {
            &self.structure_children
        }
    }

    raw_read! {
        /// Device → child devices (`children_by_parent` of the assembly graph).
        fn device_children(&self) -> &RevIndex<Id<Device>, Id<Device>> {
            &self.device_children
        }
    }

    raw_read! {
        /// Section → pages.
        fn pages_by_section(&self) -> &RevIndex<Id<Section>, Id<Page>> {
            &self.pages_by_section
        }
    }

    raw_read! {
        /// `PotentialDef.at` → potential definitions, by endpoint.
        fn potential_defs_by_endpoint(&self) -> &RevIndex<EndpointKey, Id<PotentialDef>> {
            &self.potential_defs_by_endpoint
        }
    }

    raw_read! {
        /// `Bond.a`/`b` → bonds, by endpoint.
        fn bonds_by_endpoint(&self) -> &RevIndex<EndpointKey, Id<Bond>> {
            &self.bonds_by_endpoint
        }
    }

    raw_read! {
        /// `PageItem.page` → page items.
        fn page_items_by_page(&self) -> &RevIndex<Id<Page>, Id<PageItem>> {
            &self.page_items_by_page
        }
    }

    raw_read! {
        /// `StructureBox.page` → structure boxes.
        fn structure_boxes_by_page(&self) -> &RevIndex<Id<Page>, Id<StructureBox>> {
            &self.structure_boxes_by_page
        }
    }

    raw_read! {
        /// The generic referrer index of every link without a named index: target key →
        /// `(link position in LINKS, referrer ID)`.
        fn referrers(&self) -> &RevIndex<AnyKey, (LinkId, AnyId)> {
            &self.referrers
        }
    }

    raw_read! {
        /// Builds the indexes from scratch, stamping every key with the model's revision.
        fn rebuild(model: &crate::Model) -> BaseIndexes {
            let mut idx = BaseIndexes::default();
            let rev = model.rev();
            let project = AnyRow::Project(std::sync::Arc::clone(model.project()));
            idx.update(None, Some(&project), rev);
            for key in crate::entity::table_keys(model) {
                let row = crate::entity::get_row(model, &key);
                idx.update(None, row.as_ref(), rev);
            }
            idx
        }
    }

    raw_read! {
        /// Whether both index sets hold the same entries (`changed_at` stamps are ignored).
        fn same_entries(&self, other: &BaseIndexes) -> bool {
            self.functions_by_device.same_entries(&other.functions_by_device)
                && self.reps_by_page.same_entries(&other.reps_by_page)
                && self.reps_by_function.same_entries(&other.reps_by_function)
                && self.connections_by_page.same_entries(&other.connections_by_page)
                && self
                    .connections_by_endpoint
                    .same_entries(&other.connections_by_endpoint)
                && self.junctions_by_page.same_entries(&other.junctions_by_page)
                && self.terminals_by_strip.same_entries(&other.terminals_by_strip)
                && self.conductors_by_cable.same_entries(&other.conductors_by_cable)
                && self.structure_children.same_entries(&other.structure_children)
                && self.device_children.same_entries(&other.device_children)
                && self.pages_by_section.same_entries(&other.pages_by_section)
                && self
                    .potential_defs_by_endpoint
                    .same_entries(&other.potential_defs_by_endpoint)
                && self.bonds_by_endpoint.same_entries(&other.bonds_by_endpoint)
                && self.page_items_by_page.same_entries(&other.page_items_by_page)
                && self
                    .structure_boxes_by_page
                    .same_entries(&other.structure_boxes_by_page)
                && self.referrers.same_entries(&other.referrers)
        }
    }

    /// Applies the delta of one row change: removes the entries of `before` that `after` lacks and adds
    /// those of `after` that `before` lacks, stamping touched keys with `rev`.
    pub(crate) fn update(&mut self, before: Option<&AnyRow>, after: Option<&AnyRow>, rev: Rev) {
        let old = before.map(row_entries).unwrap_or_default();
        let new = after.map(row_entries).unwrap_or_default();
        for e in old.difference(&new) {
            self.edit(e, false, rev);
        }
        for e in new.difference(&old) {
            self.edit(e, true, rev);
        }
    }

    fn edit(&mut self, e: &IxEntry, insert: bool, rev: Rev) {
        fn put<K: Ord + Clone, V: Ord + Clone>(ix: &mut RevIndex<K, V>, k: K, v: V, insert: bool, rev: Rev) {
            if insert {
                ix.insert(k, v, rev);
            } else {
                ix.remove(&k, &v, rev);
            }
        }
        match *e {
            IxEntry::Named(name, key, src) => match name {
                IndexName::FunctionsByDevice => {
                    put(
                        &mut self.functions_by_device,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::RepsByPage => {
                    put(&mut self.reps_by_page, key.id().cast(), src.cast(), insert, rev)
                }
                IndexName::RepsByFunction => {
                    put(
                        &mut self.reps_by_function,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::ConnectionsByPage => {
                    put(
                        &mut self.connections_by_page,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::ConnectionsByEndpoint => {
                    put(
                        &mut self.connections_by_endpoint,
                        key.ep(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::JunctionsByPage => {
                    put(
                        &mut self.junctions_by_page,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::TerminalsByStrip => {
                    put(
                        &mut self.terminals_by_strip,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::ConductorsByCable => {
                    put(
                        &mut self.conductors_by_cable,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::StructureChildren => {
                    put(
                        &mut self.structure_children,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::DeviceChildren => {
                    put(
                        &mut self.device_children,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::PagesBySection => {
                    put(
                        &mut self.pages_by_section,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::PotentialDefsByEndpoint => {
                    put(
                        &mut self.potential_defs_by_endpoint,
                        key.ep(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::BondsByEndpoint => {
                    put(&mut self.bonds_by_endpoint, key.ep(), src.cast(), insert, rev)
                }
                IndexName::PageItemsByPage => {
                    put(
                        &mut self.page_items_by_page,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
                IndexName::StructureBoxesByPage => {
                    put(
                        &mut self.structure_boxes_by_page,
                        key.id().cast(),
                        src.cast(),
                        insert,
                        rev,
                    );
                }
            },
            IxEntry::Referrer(target, link, src) => {
                put(&mut self.referrers, target, (link, src), insert, rev)
            }
        }
    }

    /// Candidate referrer IDs of `target` through link `link_id`: exact for links with their own index
    /// key, a superset when several links share a named index (the caller re-checks the referrer row).
    pub(crate) fn candidates(&self, link_id: LinkId, target: &AnyKey) -> Vec<AnyId> {
        fn ids<K: Ord + Clone, V>(ix: &RevIndex<K, Id<V>>, k: &K) -> Vec<AnyId> {
            ix.get(k).map(|v| v.cast()).collect()
        }
        fn eps<V>(ix: &RevIndex<EndpointKey, Id<V>>, target: &AnyKey) -> Vec<AnyId> {
            match target.0 {
                Kind::Function => ix
                    .range(EndpointKey::cps_of(target.1.cast()))
                    .flat_map(|(_, b)| b.values().map(|v| v.cast()))
                    .collect::<BTreeSet<AnyId>>()
                    .into_iter()
                    .collect(),
                Kind::Junction => ix
                    .get(&EndpointKey::Junction(target.1.cast()))
                    .map(|v| v.cast())
                    .collect(),
                _ => Vec::new(),
            }
        }
        let link = &LINKS[link_id];
        let t = target.1;
        match link.index {
            None => self
                .referrers
                .get(target)
                .filter(|(l, _)| *l == link_id)
                .map(|(_, src)| *src)
                .collect(),
            Some(IndexName::FunctionsByDevice) => ids(&self.functions_by_device, &t.cast()),
            Some(IndexName::RepsByPage) => ids(&self.reps_by_page, &t.cast()),
            Some(IndexName::RepsByFunction) => ids(&self.reps_by_function, &t.cast()),
            Some(IndexName::ConnectionsByPage) => ids(&self.connections_by_page, &t.cast()),
            Some(IndexName::ConnectionsByEndpoint) => eps(&self.connections_by_endpoint, target),
            Some(IndexName::JunctionsByPage) => ids(&self.junctions_by_page, &t.cast()),
            Some(IndexName::TerminalsByStrip) => ids(&self.terminals_by_strip, &t.cast()),
            Some(IndexName::ConductorsByCable) => ids(&self.conductors_by_cable, &t.cast()),
            Some(IndexName::StructureChildren) => ids(&self.structure_children, &t.cast()),
            Some(IndexName::DeviceChildren) => ids(&self.device_children, &t.cast()),
            Some(IndexName::PagesBySection) => ids(&self.pages_by_section, &t.cast()),
            Some(IndexName::PotentialDefsByEndpoint) => eps(&self.potential_defs_by_endpoint, target),
            Some(IndexName::BondsByEndpoint) => eps(&self.bonds_by_endpoint, target),
            Some(IndexName::PageItemsByPage) => ids(&self.page_items_by_page, &t.cast()),
            Some(IndexName::StructureBoxesByPage) => ids(&self.structure_boxes_by_page, &t.cast()),
        }
    }
}
