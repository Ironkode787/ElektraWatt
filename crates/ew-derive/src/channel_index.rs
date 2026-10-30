// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The channel indexes (FT-319; doc 10 §2.4, §3.2 step 4, §3.3): a channels-by-module index and a
//! connection-point → channel index, as memoised derived families.
//!
//! The PLC queries used to find a module's channels by their v5 IDs (`v5(module, "channel", n)`), while
//! normaliser N1 keys a channel on (module, number); and derivation step 4 built the set of bound CPs from the
//! whole channel table for every module, so one edit of any channel re-ran every module's states. Both reads
//! now go through two **index families** of the pattern of [`crate::index`] (ARCH §6.3 rule 5):
//!
//! - [`ChannelsByModule`] `channels_by_module()`: every channel by (module, number, ID), incremental over the
//!   channel table. An edit that changes neither a channel's module nor its number (a designation, a symbol, a
//!   binding) changes no entry, so the family cuts off and no reader is re-run; a channel that is added, removed,
//!   renumbered or moved to another module changes the entries of its modules.
//! - [`ModuleChannelIds`] `module_channel_ids(module)`: one module's slice of that index, the channels by number
//!   with the lowest ID where two channels share a number (as N1 keys them), read as a cut-off point. Readers
//!   of one module are invalidated only when that module's slice changes, and read the channel rows by ID, so
//!   an edit of one channel re-runs its own module's families only (ARCH §6.3: the finest key is a module).
//! - [`ChannelsByCp`] `channels_by_cp()`: every channel bound to a CP by (function, CP index, ID), incremental
//!   likewise. Step 4 asks it whether a coil is bound (a channel of any module may bind it). A **binding** edit
//!   changes an entry of this index, so it is not cut off: step 4 reads the whole value (`q.get`, not a slice per
//!   CP), and a binding edit of any channel re-runs `channel_states` of every module whose position map claims a
//!   channel (one test pins the counts, `plc_positions.rs`); an edit that leaves the CP unchanged cuts it off. A
//!   slice per CP would cut that fan-out and is not built.
//!
//! The same lookups in a transaction (N1, N2, N3, `plc.bind_channel`, `plc.set_module_part`) are
//! `TxBuilder::channels_of_module` and `channels_at_cp`, which read the model's base indexes with
//! read-your-writes: a transaction has no memoising `Q` over its pending state.

use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Channel, Device, Endpoint, Function};

use crate::Q;
use crate::heap::HeapBytes;
use crate::index::{Index, touched_rows};
use crate::query::{Changes, Policy, Query};

/// A reverse map over the channel table: each channel that has a key, and the channels of each key in
/// (key, ID) order. The value of the two index families.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChannelKeyed<K: Ord + Clone> {
    /// The key of each channel that has one.
    of: Index<Id<Channel>, K>,
    /// `(key, channel)` pairs.
    by_key: Index<(K, Id<Channel>), ()>,
}

impl<K: Ord + Clone> Default for ChannelKeyed<K> {
    fn default() -> Self {
        ChannelKeyed {
            of: Index::default(),
            by_key: Index::default(),
        }
    }
}

impl<K: Ord + Clone> ChannelKeyed<K> {
    /// Sets the key of a channel (`None`: it has none); an equal key changes nothing.
    fn set(&mut self, id: Id<Channel>, key: Option<K>)
    where
        K: HeapBytes,
    {
        if self.of.get(&id) == key.as_ref() {
            return;
        }
        if let Some(old) = self.of.get(&id).cloned() {
            self.by_key.set((old, id), None);
        }
        if let Some(new) = &key {
            self.by_key.set((new.clone(), id), Some(()));
        }
        self.of.set(id, key);
    }

    /// The number of channels that have a key.
    pub fn len(&self) -> usize {
        self.of.len()
    }

    /// Whether no channel has a key.
    pub fn is_empty(&self) -> bool {
        self.of.is_empty()
    }
}

impl<K: Ord + Clone + HeapBytes> HeapBytes for ChannelKeyed<K> {
    fn heap_bytes(&self) -> usize {
        self.of.heap_bytes() + self.by_key.heap_bytes()
    }
}

/// The smallest channel ID, the start of a key's range.
fn first_id() -> Id<Channel> {
    Id::from_uuid(Default::default())
}

impl ChannelKeyed<(Id<Device>, u16)> {
    /// The channels of `module` by number, in number order, each number once: of two channels with one number (an
    /// import that bypassed the v5 rule), the one whose ID is `v5(module, "channel", number)`, else the lowest ID
    /// (WP-S1.21 D10, FT-350: the channel N1 keeps; EW-CORE-008 `duplicate_number` reports the others).
    pub fn of_module(&self, module: Id<Device>) -> Vec<(u16, Id<Channel>)> {
        let mut out: Vec<(u16, Id<Channel>)> = Vec::new();
        for (((m, number), id), ()) in self.by_key.range(((module, 0), first_id())..) {
            if *m != module {
                break;
            }
            match out.last_mut() {
                Some((n, kept)) if n == number => {
                    if *id == ew_ids::v5::plc_channel_id::<Device, Channel>(module, *number) {
                        *kept = *id;
                    }
                }
                _ => out.push((*number, *id)),
            }
        }
        out
    }

    /// Every channel of `module` by number in (number, ID) order, duplicates of a number included (EW-CORE-008
    /// `duplicate_number`, FT-350).
    pub fn all_of_module(&self, module: Id<Device>) -> Vec<(u16, Id<Channel>)> {
        self.by_key
            .range(((module, 0), first_id())..)
            .take_while(|(((m, _), _), ())| *m == module)
            .map(|(((_, number), id), ())| (*number, *id))
            .collect()
    }

    /// The modules that hold two or more channels of one number, in module order (EW-CORE-008 `duplicate_number`).
    pub fn modules_with_duplicates(&self) -> Vec<Id<Device>> {
        let mut out: Vec<Id<Device>> = Vec::new();
        let mut last: Option<(Id<Device>, u16)> = None;
        for (((m, number), _), ()) in self.by_key.iter() {
            if last == Some((*m, *number)) && out.last() != Some(m) {
                out.push(*m);
            }
            last = Some((*m, *number));
        }
        out
    }
}

impl ChannelKeyed<(Id<Function>, u16)> {
    /// The channels bound to CP `idx` of `function`, in ID order.
    pub fn at_cp(&self, function: Id<Function>, idx: u16) -> impl Iterator<Item = Id<Channel>> + '_ {
        self.by_key
            .range(((function, idx), first_id())..)
            .take_while(move |(((f, i), _), _)| *f == function && *i == idx)
            .map(|(((_, _), id), ())| *id)
    }

    /// Whether some channel is bound to CP `idx` of `function`.
    pub fn is_bound(&self, function: Id<Function>, idx: u16) -> bool {
        self.at_cp(function, idx).next().is_some()
    }
}

/// An incremental index over the channel table: the key of a channel is `select(channel)`.
///
/// Cold (no previous value, or no touched list) it reads the table whole. Incremental it re-selects the touched
/// channels from `previous` and re-reads the table's stamp, the dependency the cold run recorded; the
/// dependencies carried from the previous execution are dropped first (as `index::table_index` does).
fn keyed_index<K: Ord + Clone + HeapBytes>(
    q: &Q,
    previous: Option<(&ChannelKeyed<K>, Changes<'_>)>,
    select: impl Fn(&Channel) -> Option<K>,
) -> ChannelKeyed<K> {
    q.drop_carried();
    let channels = q.table::<Channel>();
    let Some((prev, touched)) = previous.and_then(|(p, c)| c.touched().map(|t| (p, t))) else {
        let mut v = ChannelKeyed::default();
        for (id, c) in channels.iter() {
            v.set(id, select(c));
        }
        return v;
    };
    let _ = channels.len();
    let mut v = prev.clone();
    for id in touched_rows::<Channel>(touched) {
        v.set(id, channels.get(id).and_then(|c| select(c)));
    }
    v
}

/// `channels_by_module()`: every channel by (module, number, ID); see the module documentation.
#[derive(Debug)]
pub struct ChannelsByModule;

impl Query for ChannelsByModule {
    type Key = ();
    type Value = ChannelKeyed<(Id<Device>, u16)>;
    const FAMILY: &'static str = "channels_by_module";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        Arc::new(keyed_index(q, previous.map(|(p, c)| (p.as_ref(), c)), |c| {
            Some((c.module, c.number))
        }))
    }
}

/// `module_channel_ids(module)`: the channels of one module by number, each number once with its lowest ID (the
/// slice of [`ChannelsByModule`] that the module's readers depend on).
#[derive(Debug)]
pub struct ModuleChannelIds;

impl Query for ModuleChannelIds {
    type Key = Id<Device>;
    type Value = Vec<(u16, Id<Channel>)>;
    const FAMILY: &'static str = "module_channel_ids";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, key: &Id<Device>) -> Arc<Self::Value> {
        Arc::new(q.get_value_part::<ChannelsByModule>(()).of_module(*key))
    }
}

/// `channels_by_cp()`: every channel bound to a CP, by (function, CP index, ID); see the module documentation.
#[derive(Debug)]
pub struct ChannelsByCp;

impl Query for ChannelsByCp {
    type Key = ();
    type Value = ChannelKeyed<(Id<Function>, u16)>;
    const FAMILY: &'static str = "channels_by_cp";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        Arc::new(keyed_index(
            q,
            previous.map(|(p, c)| (p.as_ref(), c)),
            |c| match &c.binding {
                Some(Endpoint::Cp(f, idx)) => Some((*f, *idx)),
                _ => None,
            },
        ))
    }
}
