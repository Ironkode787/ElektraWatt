// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The connectivity facts the wire, terminal, cable, conductor, potential and bond tables share: the
//! project's physical wires with their effective properties, oriented From/To, in reading order; the
//! terminal bridges with their labels.
//!
//! These are the same rules the report kinds of `ew-reports` apply to the wire list, the terminal diagrams,
//! the cable overview and the potential overview (doc 08 §3.8, §6.5; doc 20 §7.3), read through the same
//! `ew-derive` queries; the acceptance SQL over the views and the report rows are compared with the same
//! expected CSVs, so the two implementations are held equal by the golden tests. Moving them into shared
//! `ew-derive` queries is follow-up work (the dependency rules let `ew-reports` use `ew-sqlview`, not the
//! reverse).

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::{
    BridgeKind, Connection, ConnectionKind, Device, Endpoint, Function, FunctionKind, Page,
};
use ew_derive::{
    CpRef, DeviceTag, DrawnPageOrder, EffectiveProps, FunctionCps, PageRef, Partition, PhysicalWire,
    PhysicalWiresOf, PotentialId, PotentialNameOf, PotentialNames, PotentialPartition, Q, RepAnchorCell,
    WireEnd, WirePropsOf,
};

/// One physical wire with its effective properties, oriented (`ends[0]` is the From end).
#[derive(Clone, Debug)]
pub(crate) struct WireRec {
    /// The wire.
    pub wire: PhysicalWire,
    /// Its effective properties.
    pub props: EffectiveProps,
}

/// Every physical wire of the project in reading order, with the partition and the potential names.
pub(crate) struct Wires {
    /// The partition.
    pub partition: Arc<Partition>,
    /// The names of every potential.
    pub names: BTreeMap<PotentialId, Arc<PotentialNames>>,
    /// The wires in reading order (doc 08 §3.8): see [`Wires::of`].
    pub wires: Vec<WireRec>,
}

impl Wires {
    /// Reads the partition, the physical wires of every potential (deduplicated: a bond wire belongs to two)
    /// and their effective properties; orients each wire and sorts them in reading order: the page of the
    /// wire's first constituent, the frame cell (column, row) of the From end and of the To end on that page,
    /// the tags and CP designations of the ends, the wire ID.
    pub fn of(q: &Q) -> Wires {
        let partition = q.get::<PotentialPartition>(());
        let mut by_id: BTreeMap<Id<Connection>, WireRec> = BTreeMap::new();
        let mut names = BTreeMap::new();
        for p in partition.potentials() {
            names.insert(p.id, q.get::<PotentialNameOf>(p.id));
            let physical = q.get::<PhysicalWiresOf>(p.id);
            let props = q.get::<WirePropsOf>(p.id);
            for w in &physical.wires {
                by_id.entry(w.id).or_insert_with(|| {
                    let mut wire = w.clone();
                    orient(q, &mut wire);
                    WireRec {
                        wire,
                        props: props.wires.get(&w.id).cloned().unwrap_or_default(),
                    }
                });
            }
        }
        let mut keyed: Vec<(WireKey, WireRec)> = by_id.into_values().map(|r| (wire_key(q, &r), r)).collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        Wires {
            partition,
            names,
            wires: keyed.into_iter().map(|(_, r)| r).collect(),
        }
    }

    /// The name of a potential, `""` when unknown.
    pub fn potential_name(&self, id: PotentialId) -> &str {
        self.names.get(&id).map_or("", |n| n.name.text.as_str())
    }

    /// The potential text of a wire: the name, or `A ⇄ B` for a bond wire (the bond's side order).
    pub fn potential_text(&self, w: &PhysicalWire) -> String {
        let bond = w
            .bond
            .and_then(|b| self.partition.bonds().iter().find(|l| l.bond == b));
        if let Some(bond) = bond {
            if let (Some(a), Some(b)) = (bond.a, bond.b) {
                return format!("{} \u{21C4} {}", self.potential_name(a), self.potential_name(b));
            }
        }
        w.potentials
            .iter()
            .map(|p| self.potential_name(*p))
            .collect::<Vec<_>>()
            .join(" \u{21C4} ")
    }
}

/// Puts the From end of a wire first (doc 08 §3.8, FR-CON-040): on different pages the end on the earlier
/// page; otherwise the end with the smaller anchor cell (row, then column), then the smaller CP ID. The
/// flow-distance step of the rule is not applied (no per-CP flow distance is derived yet).
fn orient(q: &Q, w: &mut PhysicalWire) {
    let page_of = |c: Option<&Id<Connection>>| c.and_then(|c| q.connections().get(*c)).and_then(|c| c.page);
    let (pa, pb) = (page_of(w.constituents.first()), page_of(w.constituents.last()));
    let swap = match (pa, pb) {
        (Some(a), Some(b)) if a != b => page_position(q, b) < page_position(q, a),
        _ => {
            let page = pa.or(pb);
            let cell = |e: &WireEnd| {
                anchor_cell(q, e.cp.function, page)
                    .map_or((u32::MAX, u32::MAX), |(column, row)| (row, column))
            };
            (cell(&w.ends[1]), w.ends[1].cp) < (cell(&w.ends[0]), w.ends[0].cp)
        }
    };
    if swap {
        w.ends.swap(0, 1);
    }
}

/// The (column, row) of the frame cell of the first representation (by ID) that draws `function` on `page`.
pub(crate) fn anchor_cell(q: &Q, function: Id<Function>, page: Option<Id<Page>>) -> Option<(u32, u32)> {
    let page = page?;
    let mut reps: Vec<_> = q
        .idx()
        .reps_of_function(function)
        .filter(|r| q.reps().get(*r).is_some_and(|r| r.page == page))
        .collect();
    reps.sort();
    let cell = q.get::<RepAnchorCell>(*reps.first()?);
    cell.as_ref().as_ref().map(|c| (c.column, c.row))
}

type WireKey = (
    usize,
    (u32, u32),
    (u32, u32),
    NaturalKey,
    String,
    NaturalKey,
    String,
    Id<Connection>,
);

fn wire_key(q: &Q, rec: &WireRec) -> WireKey {
    let w = &rec.wire;
    let page = w.pages.first().copied();
    let pos = page.map_or(usize::MAX, |p| page_position(q, p));
    let [a, b] = &w.ends;
    let cell = |e: &WireEnd| anchor_cell(q, e.cp.function, page).unwrap_or((u32::MAX, u32::MAX));
    let tag = |e: &WireEnd| match device_of(q, e.cp.function) {
        Some(d) => tag_key(q, d),
        None => natural_key(&ip_name(q, e.cp.function)),
    };
    (
        pos,
        cell(a),
        cell(b),
        tag(a),
        cp_designation(q, a.cp),
        tag(b),
        cp_designation(q, b.cp),
        w.id,
    )
}

/// The device of a CP's function; `None` for an interruption point.
pub(crate) fn device_of(q: &Q, f: Id<Function>) -> Option<Id<Device>> {
    q.functions().get(f).and_then(|f| f.device)
}

/// The CP of a function by index.
pub(crate) fn cp_of(q: &Q, cp: CpRef) -> Option<ew_derive::Cp> {
    q.get::<FunctionCps>(cp.function)
        .iter()
        .find(|c| c.idx == cp.idx)
        .cloned()
}

/// The printed designation of a CP (`""` when the function has no such CP).
pub(crate) fn cp_designation(q: &Q, cp: CpRef) -> String {
    cp_of(q, cp).map(|c| c.designation).unwrap_or_default()
}

/// The name of an interruption point function (`""` for any other function).
pub(crate) fn ip_name(q: &Q, f: Id<Function>) -> String {
    match q.functions().get(f).map(|f| f.kind.clone()) {
        Some(FunctionKind::Ip(ip)) => ip.name.as_str().to_owned(),
        _ => String::new(),
    }
}

/// The list form of a device's tag (`-X1031:1`), `None` for a device without one.
pub(crate) fn tag_short(q: &Q, d: Id<Device>) -> Option<String> {
    q.get::<DeviceTag>(d).as_ref().as_ref().map(|t| t.short())
}

/// The natural-order key of a device's full tag.
pub(crate) fn tag_key(q: &Q, d: Id<Device>) -> NaturalKey {
    natural_key(
        q.get::<DeviceTag>(d)
            .as_ref()
            .as_ref()
            .map(|t| t.full.as_str())
            .unwrap_or(""),
    )
}

/// The position of a drawn page in the drawn page order (`usize::MAX` for an unknown page).
pub(crate) fn page_position(q: &Q, page: Id<Page>) -> usize {
    q.get::<DrawnPageOrder>(())
        .position(PageRef::Drawn(page))
        .unwrap_or(usize::MAX)
}

/// One terminal bridge: the bridge connections of one group on one strip (doc 08 §6.5).
#[derive(Clone, Debug)]
pub(crate) struct Bridge {
    /// The strip.
    pub strip: Id<Device>,
    /// The group number on the strip.
    pub group: u16,
    /// The bridge kind (`plug-in`, `saddle`, `wire`, `inter-level`).
    pub kind: &'static str,
    /// The terminals it joins in `(seq, ID)` order.
    pub terminals: Vec<Id<Device>>,
    /// The page of its first drawn connection, if any is drawn.
    pub page: Option<Id<Page>>,
    /// The label `BR<n>`, numbered over the strips in natural tag order, then group.
    pub label: String,
}

/// Every bridge of the project, labelled `BR1`, `BR2` … in natural order of the strip tag, then group.
pub(crate) fn bridges(q: &Q) -> Vec<Bridge> {
    let seq_of = |t: Id<Device>| {
        q.devices()
            .get(t)
            .and_then(|d| d.aspects.terminal.as_ref())
            .map(|t| t.seq)
    };
    let mut groups: BTreeMap<(Id<Device>, u16), Bridge> = BTreeMap::new();
    for (_, c) in q.connections().iter() {
        let ConnectionKind::Bridge(spec) = &c.kind else {
            continue;
        };
        let terminals: Vec<Id<Device>> = [&c.a, &c.b]
            .into_iter()
            .filter_map(|e| match e {
                Endpoint::Cp(f, _) => device_of(q, *f),
                _ => None,
            })
            .collect();
        let Some(strip) = terminals.iter().find_map(|t| {
            q.devices()
                .get(*t)
                .and_then(|d| d.aspects.terminal.as_ref())
                .map(|t| t.strip)
        }) else {
            continue;
        };
        let kind = match &spec.kind {
            BridgeKind::PlugIn => "plug-in",
            BridgeKind::Saddle => "saddle",
            BridgeKind::Wire => "wire",
            BridgeKind::InterLevel => "inter-level",
            BridgeKind::Unknown(_) => "bridge",
        };
        let bridge = groups.entry((strip, spec.group)).or_insert_with(|| Bridge {
            strip,
            group: spec.group,
            kind,
            terminals: Vec::new(),
            page: None,
            label: String::new(),
        });
        for t in terminals {
            if !bridge.terminals.contains(&t) {
                bridge.terminals.push(t);
            }
        }
        bridge.page = bridge.page.or(c.page);
    }
    let mut ordered: Vec<(NaturalKey, Bridge)> = groups
        .into_values()
        .map(|mut b| {
            b.terminals.sort_by_key(|t| (seq_of(*t), *t));
            (tag_key(q, b.strip), b)
        })
        .collect();
    ordered.sort_by(|a, b| (&a.0, a.1.group, a.1.strip).cmp(&(&b.0, b.1.group, b.1.strip)));
    ordered
        .into_iter()
        .enumerate()
        .map(|(i, (_, mut b))| {
            b.label = format!("BR{}", i + 1);
            b
        })
        .collect()
}
