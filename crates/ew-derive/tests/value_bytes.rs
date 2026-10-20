// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! Real value bytes of the five connectivity families (ADR-0002 amendment 4 item (d)): `Query::bytes` counts
//! the heap the value owns, not only `size_of::<Value>()`, so the memory gate sees what a family holds.
//!
//! - `connectivity_values_count_their_heap`: over the golden project, each family's bytes exceed the inline
//!   size when the value is non-empty and cover its elements.

mod common;
#[path = "common/golden.rs"]
mod golden;

use std::mem::size_of;

use common::{Fx, id, uid};
use ew_derive::{
    EffectiveProps, LocalNet, PageLocalNets, PhysicalWire, PhysicalWiresOf, Potential, PotentialPartition, Q,
    Query, StitchGraphQ, StitchNode, WirePropsOf,
};
use golden::golden;
use serde_json::json;

/// Asserts `bytes` is at least `floor`, and above the inline size when `non_empty`.
fn check(family: &str, bytes: usize, inline: usize, floor: usize, non_empty: bool) {
    println!("{family}: {bytes} bytes (inline {inline}, floor {floor})");
    assert!(
        bytes >= inline.max(floor),
        "{family}: {bytes} < {}",
        inline.max(floor)
    );
    if non_empty {
        assert!(bytes > inline, "{family}: non-empty value counted as inline only");
    }
}

#[test]
fn connectivity_values_count_their_heap() {
    let m = golden().build();
    let q = Q::new(&m);

    let partition = q.get::<PotentialPartition>(());
    let b = PotentialPartition::bytes(&partition);
    let floor = partition.potentials.len() * size_of::<Potential>();
    check(
        "potential_partition",
        b,
        size_of::<<PotentialPartition as Query>::Value>(),
        floor,
        !partition.potentials.is_empty(),
    );

    let graph = q.get::<StitchGraphQ>(());
    let b = StitchGraphQ::bytes(&graph);
    let floor = graph.nodes.len() * size_of::<StitchNode>();
    check(
        "stitch_graph",
        b,
        size_of::<<StitchGraphQ as Query>::Value>(),
        floor,
        !graph.nodes.is_empty() || !graph.ips.is_empty(),
    );

    let nets_inline = size_of::<<PageLocalNets as Query>::Value>();
    let (mut nets_heap, mut nets_floor) = (0, 0);
    for page in q.pages().ids() {
        let nets = q.get::<PageLocalNets>(page);
        let b = PageLocalNets::bytes(&nets);
        assert!(b >= nets_inline);
        nets_floor += nets.nets.len() * size_of::<LocalNet>();
        nets_heap += b - nets_inline;
    }
    check(
        "page_local_nets (all pages)",
        nets_heap + nets_inline,
        nets_inline,
        nets_floor,
        nets_floor > 0,
    );

    let wires_inline = size_of::<<PhysicalWiresOf as Query>::Value>();
    let props_inline = size_of::<<WirePropsOf as Query>::Value>();
    let (mut wires_heap, mut wires_floor, mut props_heap, mut props_floor) = (0, 0, 0, 0);
    for p in &partition.potentials {
        let w = q.get::<PhysicalWiresOf>(p.id);
        let b = PhysicalWiresOf::bytes(&w);
        assert!(b >= wires_inline);
        wires_heap += b - wires_inline;
        wires_floor += w.wires.len() * size_of::<PhysicalWire>();
        let props = q.get::<WirePropsOf>(p.id);
        let b = WirePropsOf::bytes(&props);
        assert!(b >= props_inline);
        props_heap += b - props_inline;
        props_floor += props.wires.len() * size_of::<EffectiveProps>();
    }
    check(
        "physical_wires_of (all potentials)",
        wires_heap + wires_inline,
        wires_inline,
        wires_floor,
        wires_floor > 0,
    );
    check(
        "wire_props_of (all potentials)",
        props_heap + props_inline,
        props_inline,
        props_floor,
        props_floor > 0,
    );
}

const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;
const P1: u128 = 0x100;

/// Three plain functions on one page, two wires between them and a named potential: every family is
/// non-empty.
fn wired() -> ew_model::Model {
    let mut fx = Fx::new();
    fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
    fx.page(P1, 1, None, "multi_line", &[], "one");
    for n in [0x1001_u128, 0x1002, 0x1003] {
        fx.device(n + 0x20_000, "K", Some(n as u32));
        fx.function(n, Some(n + 0x20_000), FNDEF, true);
        fx.rep(n + 0x10_000, P1, SYM, &[n], 0, n as i64 * 1_000_000);
    }
    for (c, a, b) in [(0x2001_u128, 0x1001_u128, 0x1002_u128), (0x2002, 0x1002, 0x1003)] {
        fx.push(json!({
            "$k": "connection", "id": uid(c), "page": uid(P1),
            "a": {"cp": [uid(a), 0]}, "b": {"cp": [uid(b), 0]},
        }));
    }
    fx.push(json!({
        "$k": "potential_def", "id": uid(0x5001), "at": {"cp": [uid(0x1001), 0]}, "name": "A-potential-name",
    }));
    fx.build()
}

#[test]
fn wired_values_count_their_elements() {
    let m = wired();
    let q = Q::new(&m);

    let partition = q.get::<PotentialPartition>(());
    assert!(!partition.potentials.is_empty());
    let b = PotentialPartition::bytes(&partition);
    let inline = size_of::<<PotentialPartition as Query>::Value>();
    assert!(b > inline);
    assert!(b >= partition.potentials.len() * size_of::<Potential>());
    let name_heap: usize = partition.potentials.iter().map(|p| p.name.text.len()).sum();
    assert!(b >= inline + name_heap);

    let graph = q.get::<StitchGraphQ>(());
    assert!(!graph.nodes.is_empty());
    let b = StitchGraphQ::bytes(&graph);
    assert!(b >= size_of::<<StitchGraphQ as Query>::Value>() + graph.nodes.len() * size_of::<StitchNode>());

    let nets = q.get::<PageLocalNets>(id(P1));
    assert!(!nets.nets.is_empty());
    let b = PageLocalNets::bytes(&nets);
    assert!(b >= size_of::<<PageLocalNets as Query>::Value>() + nets.nets.len() * size_of::<LocalNet>());

    let mut wires_seen = 0;
    for p in &partition.potentials {
        let w = q.get::<PhysicalWiresOf>(p.id);
        if !w.wires.is_empty() {
            wires_seen += w.wires.len();
            let b = PhysicalWiresOf::bytes(&w);
            assert!(
                b >= size_of::<<PhysicalWiresOf as Query>::Value>()
                    + w.wires.len() * size_of::<PhysicalWire>()
            );
        }
        let props = q.get::<WirePropsOf>(p.id);
        if !props.wires.is_empty() {
            let b = WirePropsOf::bytes(&props);
            assert!(
                b >= size_of::<<WirePropsOf as Query>::Value>()
                    + props.wires.len() * size_of::<EffectiveProps>()
            );
        }
    }
    assert!(wires_seen > 0, "the fixture has physical wires");
}
