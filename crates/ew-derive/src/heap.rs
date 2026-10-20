// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bytes a derived value occupies, for [`Query::bytes`](crate::Query::bytes): the memory gate counts
//! values and not only dependencies (ADR-0002 amendment 4 item (d), G-SP-04, NFR-PERF-13).
//!
//! The count is an estimate, deliberately on the conservative side: container capacity is counted as
//! `capacity * size_of::<element>()`, the allocator's headers and the nodes of a `BTreeMap` or `BTreeSet`
//! are ignored, and a shared `Arc` pointee is counted in full by each owner.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_model::EndpointKey;
use ew_schema::{
    AcDc, BondKind, CircuitClass, ColourCode, ColourSpec, Csa, IecColour, IpDirection, PartRef,
    PotentialEarthing, PotentialEnv, PotentialType, WireFlags, WireOverrides,
};
use ew_units::{Dec3, Nm};
use serde_json::Value;

use crate::nets::{CpRef, LocalNet, NetId, NetMember};
use crate::note::{DeriveNote, SchemeIssue};
use crate::potentials::{
    Attr, AttrSource, BondLink, NameSource, Potential, PotentialId, PotentialName, Signal,
};
use crate::stitch::{EdgeKind, IpGroup, IpInfo, Level, PairMode, ShortSource, StitchEdge, StitchNode};
use crate::wire_props::{EffectiveProps, PropSource, Prov, SizeRule, WirePropsMap};
use crate::wires::{EndKind, OverrideSources, PhysicalWire, PhysicalWires, WireEnd};

/// The heap bytes a value owns, not counting `size_of::<Self>()` itself.
pub(crate) trait HeapBytes {
    /// The bytes this value owns on the heap.
    fn heap_bytes(&self) -> usize;
}

/// The bytes of a value: its inline size and the heap bytes it owns.
pub(crate) fn value_bytes<T: HeapBytes>(value: &T) -> usize {
    std::mem::size_of::<T>() + value.heap_bytes()
}

/// Implements [`HeapBytes`] as 0 for types that own no heap memory.
macro_rules! no_heap {
    ($($t:ty),* $(,)?) => {
        $(impl HeapBytes for $t {
            fn heap_bytes(&self) -> usize {
                0
            }
        })*
    };
}

no_heap!(
    u8,
    u16,
    u32,
    u64,
    usize,
    i8,
    i16,
    i32,
    i64,
    bool,
    Dec3,
    Nm,
    EndpointKey,
    CpRef,
    NetId,
    PotentialId,
    NameSource,
    AttrSource,
    Level,
    ShortSource,
    EdgeKind,
    PairMode,
    EndKind,
    WireEnd,
    OverrideSources,
    SizeRule,
    IecColour,
    SchemeIssue,
);

impl<T> HeapBytes for Id<T> {
    fn heap_bytes(&self) -> usize {
        0
    }
}

impl HeapBytes for String {
    fn heap_bytes(&self) -> usize {
        self.capacity()
    }
}

impl HeapBytes for Ident {
    fn heap_bytes(&self) -> usize {
        self.as_str().len()
    }
}

impl<T: HeapBytes> HeapBytes for Vec<T> {
    fn heap_bytes(&self) -> usize {
        self.capacity() * std::mem::size_of::<T>() + self.iter().map(HeapBytes::heap_bytes).sum::<usize>()
    }
}

impl<T: HeapBytes> HeapBytes for Option<T> {
    fn heap_bytes(&self) -> usize {
        self.as_ref().map_or(0, HeapBytes::heap_bytes)
    }
}

impl<T: HeapBytes> HeapBytes for Box<T> {
    fn heap_bytes(&self) -> usize {
        std::mem::size_of::<T>() + (**self).heap_bytes()
    }
}

/// A shared value is counted in full by each owner: conservative, since the owners together hold it once.
impl<T: HeapBytes> HeapBytes for Arc<T> {
    fn heap_bytes(&self) -> usize {
        std::mem::size_of::<T>() + (**self).heap_bytes()
    }
}

/// Entries by `len`; the nodes' own overhead is ignored.
impl<K: HeapBytes, V: HeapBytes> HeapBytes for BTreeMap<K, V> {
    fn heap_bytes(&self) -> usize {
        self.len() * (std::mem::size_of::<K>() + std::mem::size_of::<V>())
            + self
                .iter()
                .map(|(k, v)| k.heap_bytes() + v.heap_bytes())
                .sum::<usize>()
    }
}

/// Entries by `len`; the nodes' own overhead is ignored.
impl<T: HeapBytes> HeapBytes for BTreeSet<T> {
    fn heap_bytes(&self) -> usize {
        self.len() * std::mem::size_of::<T>() + self.iter().map(HeapBytes::heap_bytes).sum::<usize>()
    }
}

impl<A: HeapBytes, B: HeapBytes> HeapBytes for (A, B) {
    fn heap_bytes(&self) -> usize {
        self.0.heap_bytes() + self.1.heap_bytes()
    }
}

impl<A: HeapBytes, B: HeapBytes, C: HeapBytes> HeapBytes for (A, B, C) {
    fn heap_bytes(&self) -> usize {
        self.0.heap_bytes() + self.1.heap_bytes() + self.2.heap_bytes()
    }
}

impl<T: HeapBytes, const N: usize> HeapBytes for [T; N] {
    fn heap_bytes(&self) -> usize {
        self.iter().map(HeapBytes::heap_bytes).sum()
    }
}

/// A JSON value: strings by capacity, arrays by capacity, objects by entry count (keys by length).
impl HeapBytes for Value {
    fn heap_bytes(&self) -> usize {
        match self {
            Value::Null | Value::Bool(_) | Value::Number(_) => 0,
            Value::String(s) => s.capacity(),
            Value::Array(a) => {
                a.capacity() * std::mem::size_of::<Value>() + a.iter().map(Value::heap_bytes).sum::<usize>()
            }
            Value::Object(o) => o
                .iter()
                .map(|(k, v)| {
                    k.len() + std::mem::size_of::<String>() + std::mem::size_of::<Value>() + v.heap_bytes()
                })
                .sum(),
        }
    }
}

/// Implements [`HeapBytes`] for an enum whose only heap data is the payload of its `Unknown(Value)` variant.
macro_rules! unknown_heap {
    ($($t:ty),* $(,)?) => {
        $(impl HeapBytes for $t {
            fn heap_bytes(&self) -> usize {
                if let Self::Unknown(v) = self { v.heap_bytes() } else { 0 }
            }
        })*
    };
}

unknown_heap!(
    PotentialType,
    CircuitClass,
    BondKind,
    IpDirection,
    AcDc,
    PotentialEarthing,
    Csa
);

impl HeapBytes for ColourCode {
    fn heap_bytes(&self) -> usize {
        match self {
            ColourCode::Iec(_) => 0,
            ColourCode::Company(i) | ColourCode::Foreign(i) => i.heap_bytes(),
            ColourCode::Unknown(v) => v.heap_bytes(),
        }
    }
}

/// The codes by length (the backing vector's capacity is not exposed).
impl HeapBytes for ColourSpec {
    fn heap_bytes(&self) -> usize {
        std::mem::size_of_val(self.codes()) + self.codes().iter().map(HeapBytes::heap_bytes).sum::<usize>()
    }
}

impl HeapBytes for PartRef {
    fn heap_bytes(&self) -> usize {
        self.variant.heap_bytes() + self.extra.heap_bytes()
    }
}

impl HeapBytes for WireFlags {
    fn heap_bytes(&self) -> usize {
        self.extra.heap_bytes()
    }
}

impl HeapBytes for PotentialEnv {
    fn heap_bytes(&self) -> usize {
        self.ac_dc.heap_bytes()
            + self.frequency.heap_bytes()
            + self.earthing.heap_bytes()
            + self.extra.heap_bytes()
    }
}

impl HeapBytes for WireOverrides {
    fn heap_bytes(&self) -> usize {
        self.csa.heap_bytes()
            + self.colour.heap_bytes()
            + self.wire_part.heap_bytes()
            + self.insulation.heap_bytes()
            + self.function.heap_bytes()
            + self.flags.heap_bytes()
            + self.length.heap_bytes()
            + self.extra.heap_bytes()
    }
}

impl HeapBytes for DeriveNote {
    fn heap_bytes(&self) -> usize {
        match self {
            DeriveNote::UnknownProfile { requested, used } => requested.heap_bytes() + used.heap_bytes(),
            DeriveNote::SchemeNotApplied { scheme, issue } => scheme.heap_bytes() + issue.heap_bytes(),
            DeriveNote::DefaultFrame | DeriveNote::DefaultWireProfile { .. } => 0,
            DeriveNote::MalformedShim { member, detail } => member.heap_bytes() + detail.heap_bytes(),
            DeriveNote::ProfileOverlap { shim, rows, detail } => {
                shim.heap_bytes() + rows.heap_bytes() + detail.heap_bytes()
            }
            DeriveNote::QueryCycle { key, .. } => key.heap_bytes(),
            DeriveNote::Connectivity { subject, detail, .. } | DeriveNote::Xref { subject, detail, .. } => {
                subject.heap_bytes() + detail.heap_bytes()
            }
        }
    }
}

// --- nets ---------------------------------------------------------------------------------------------

impl HeapBytes for NetMember {
    fn heap_bytes(&self) -> usize {
        self.at.heap_bytes() + self.via.heap_bytes()
    }
}

impl HeapBytes for LocalNet {
    fn heap_bytes(&self) -> usize {
        self.id.heap_bytes() + self.members.heap_bytes() + self.connections.heap_bytes()
    }
}

// --- potentials ---------------------------------------------------------------------------------------

impl HeapBytes for PotentialName {
    fn heap_bytes(&self) -> usize {
        self.text.heap_bytes() + self.source.heap_bytes()
    }
}

impl<T: HeapBytes> HeapBytes for Attr<T> {
    fn heap_bytes(&self) -> usize {
        self.value.heap_bytes() + self.source.heap_bytes()
    }
}

impl HeapBytes for Potential {
    fn heap_bytes(&self) -> usize {
        self.id.heap_bytes()
            + self.name.heap_bytes()
            + self.aliases.heap_bytes()
            + self.ip_names.heap_bytes()
            + self.defs.heap_bytes()
            + self.cps.heap_bytes()
            + self.junctions.heap_bytes()
            + self.nodes.heap_bytes()
            + self.signals.heap_bytes()
            + self.nets.heap_bytes()
            + self.ips.heap_bytes()
            + self.ptype.heap_bytes()
            + self.voltage.heap_bytes()
            + self.env.heap_bytes()
            + self.circuit_class.heap_bytes()
    }
}

impl HeapBytes for Signal {
    fn heap_bytes(&self) -> usize {
        self.id.heap_bytes() + self.name.heap_bytes() + self.nets.heap_bytes()
    }
}

impl HeapBytes for BondLink {
    fn heap_bytes(&self) -> usize {
        self.bond.heap_bytes()
            + self.kind.heap_bytes()
            + self.a.heap_bytes()
            + self.b.heap_bytes()
            + self.via.heap_bytes()
            + self.path_ips.heap_bytes()
            + self.names.heap_bytes()
    }
}

// --- stitch -------------------------------------------------------------------------------------------

impl HeapBytes for StitchEdge {
    fn heap_bytes(&self) -> usize {
        self.level.heap_bytes() + self.kind.heap_bytes()
    }
}

impl HeapBytes for StitchNode {
    fn heap_bytes(&self) -> usize {
        self.net.heap_bytes() + self.page.heap_bytes() + self.members.heap_bytes()
    }
}

impl HeapBytes for IpInfo {
    fn heap_bytes(&self) -> usize {
        self.name.heap_bytes() + self.direction.heap_bytes()
    }
}

impl HeapBytes for IpGroup {
    fn heap_bytes(&self) -> usize {
        self.name.heap_bytes()
            + self.members.heap_bytes()
            + self.pairs.heap_bytes()
            + self.unpaired.heap_bytes()
    }
}

// --- wire properties ----------------------------------------------------------------------------------

impl HeapBytes for PropSource {
    fn heap_bytes(&self) -> usize {
        match self {
            PropSource::ColourRow { profile, .. }
            | PropSource::SizeRow { profile, .. }
            | PropSource::BuiltIn { profile } => profile.heap_bytes(),
            PropSource::Override(_)
            | PropSource::Conductor(_)
            | PropSource::PotentialDef(_)
            | PropSource::ProjectDefault => 0,
        }
    }
}

impl<T: HeapBytes> HeapBytes for Prov<T> {
    fn heap_bytes(&self) -> usize {
        self.value.heap_bytes() + self.source.heap_bytes()
    }
}

impl HeapBytes for EffectiveProps {
    fn heap_bytes(&self) -> usize {
        self.csa.heap_bytes()
            + self.colour.heap_bytes()
            + self.wire_part.heap_bytes()
            + self.insulation.heap_bytes()
            + self.function.heap_bytes()
            + self.shielded.heap_bytes()
            + self.twisted.heap_bytes()
            + self.class.heap_bytes()
    }
}

impl HeapBytes for WirePropsMap {
    fn heap_bytes(&self) -> usize {
        self.wires.heap_bytes() + self.by_connection.heap_bytes() + self.notes.heap_bytes()
    }
}

// --- wires --------------------------------------------------------------------------------------------

impl HeapBytes for PhysicalWire {
    fn heap_bytes(&self) -> usize {
        self.ends.heap_bytes()
            + self.constituents.heap_bytes()
            + self.ips.heap_bytes()
            + self.junctions.heap_bytes()
            + self.potentials.heap_bytes()
            + self.pages.heap_bytes()
            + self.overrides.heap_bytes()
            + self.override_sources.heap_bytes()
    }
}

impl HeapBytes for PhysicalWires {
    fn heap_bytes(&self) -> usize {
        self.wires.heap_bytes() + self.notes.heap_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_counts_capacity() {
        let s = String::with_capacity(100);
        assert_eq!(s.heap_bytes(), s.capacity());
        assert!(s.heap_bytes() >= 100);
        assert_eq!(value_bytes(&s), std::mem::size_of::<String>() + s.capacity());
    }

    #[test]
    fn vec_counts_capacity_and_elements() {
        let mut v: Vec<String> = Vec::with_capacity(4);
        v.push(String::from("abc"));
        v.push(String::with_capacity(50));
        let inner: usize = v.iter().map(String::capacity).sum();
        assert_eq!(v.heap_bytes(), 4 * std::mem::size_of::<String>() + inner);
        let ints: Vec<u32> = Vec::with_capacity(8);
        assert_eq!(ints.heap_bytes(), 8 * 4);
    }

    #[test]
    fn btree_map_counts_entries_and_payloads() {
        let mut m: BTreeMap<u32, String> = BTreeMap::new();
        m.insert(1, String::with_capacity(10));
        m.insert(2, String::with_capacity(20));
        let expect =
            2 * (4 + std::mem::size_of::<String>()) + m.values().map(String::capacity).sum::<usize>();
        assert_eq!(m.heap_bytes(), expect);
        let s: BTreeSet<u64> = [1, 2, 3].into_iter().collect();
        assert_eq!(s.heap_bytes(), 24);
    }

    #[test]
    fn option_box_and_tuple() {
        assert_eq!(None::<String>.heap_bytes(), 0);
        let s = String::with_capacity(16);
        let cap = s.capacity();
        assert_eq!(Some(s).heap_bytes(), cap);
        let b = Box::new(String::with_capacity(8));
        assert_eq!(b.heap_bytes(), std::mem::size_of::<String>() + b.capacity());
        let t = (String::with_capacity(5), 7u8);
        assert_eq!(t.heap_bytes(), t.0.capacity());
    }

    #[test]
    fn arc_counts_the_pointee_in_full() {
        let a = Arc::new(String::with_capacity(32));
        let b = Arc::clone(&a);
        let each = std::mem::size_of::<String>() + a.capacity();
        assert_eq!(a.heap_bytes(), each);
        assert_eq!(b.heap_bytes(), each);
    }

    #[test]
    fn json_value_counts_nested_heap() {
        let v = serde_json::json!({"k": ["abc", 1]});
        assert!(v.heap_bytes() > 0);
        assert_eq!(Value::Null.heap_bytes(), 0);
    }
}
