// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the connectivity kinds (`wire_list`, `potential_overview`, `terminal_diagram`,
//! `cable_overview_diagrams`) share: the readiness gate, the project's physical wires with their effective
//! properties, wire ends as cells, terminal bridges and the cell formatting of wire properties.
//!
//! All reads go through `Q` (the Derivation II queries `potential_partition`, `physical_wires_of`,
//! `wire_props_of` and the tracked tables); nothing here reads the model directly.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{
    BridgeKind, ClampKey, ColourSpec, Connection, ConnectionKind, Csa, Device, Endpoint, Function,
    FunctionKind, Page, Unit,
};
use ew_derive::{
    CpRef, DeviceTag, DrawnPageOrder, EffectiveProps, EndKind, FunctionCps, PageRef, Partition, PhysicalWire,
    PhysicalWiresOf, PotentialId, PotentialPartition, PropSource, Q, RepAnchorCell, WireEnd, WirePropsOf,
};

use crate::rowset::{Cell, ColumnDef, NotDerived, QtyUnit, RowSet};

/// The readiness gate of the connectivity kinds.
///
/// The connectivity queries these kinds read (`potential_partition`, `physical_wires_of`, `wire_props_of`)
/// read the full page order, and a `Q` with the report engine runs the pagination inside the page order: a
/// read from there would be a query cycle, which `Q::get` turns into a panic. So a kind is **not derived in
/// a `Q` that carries the engine**, whatever has been evaluated before (one placeholder page naming what it
/// awaits, no notes), and derives in a `Q` without it. The gate depends on that configuration alone, never on
/// the order in which the page order and the rows are read.
///
/// Remove this gate and [`NotDerived::AwaitsPageOrder`] once those queries read `DrawnPageOrder`, as the doc of
/// that query prescribes: the kinds then derive inside pagination too.
///
/// # Errors
///
/// The not-derived row set to return.
pub(super) fn ready(q: &Q, columns: &[ColumnDef]) -> Result<(), RowSet> {
    if crate::queries::engine_installed(q) {
        Err(RowSet::not_derived(columns.to_vec(), NotDerived::AwaitsPageOrder))
    } else {
        Ok(())
    }
}

/// Sets the cell of column `key`. A key the kind's columns do not have is a programming error and stops the
/// build of the rows loudly, so a column change cannot shift the other cells silently.
///
/// # Panics
///
/// When `key` is not a column.
pub(super) fn put(columns: &[ColumnDef], cells: &mut [Cell], key: &str, cell: Cell) {
    let Some(i) = columns.iter().position(|c| c.key == key) else {
        panic!("the kind has no column `{key}`");
    };
    cells[i] = cell;
}

/// One physical wire of the project with its effective properties.
#[derive(Clone, Debug)]
pub(super) struct WireRec {
    /// The wire.
    pub wire: PhysicalWire,
    /// Its effective properties.
    pub props: EffectiveProps,
}

/// Every physical wire of the project, deduplicated (a bond wire belongs to two potentials), in wire-ID
/// order, with the potential partition they came from.
pub(super) struct Wires {
    /// The partition.
    pub partition: Arc<Partition>,
    /// The wires.
    pub wires: Vec<WireRec>,
}

impl Wires {
    /// Reads the partition, the physical wires of every potential and their effective properties.
    pub fn of(q: &Q) -> Wires {
        let partition = q.get::<PotentialPartition>(());
        let mut by_id: BTreeMap<Id<Connection>, WireRec> = BTreeMap::new();
        for p in &partition.potentials {
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
        Wires {
            partition,
            wires: by_id.into_values().collect(),
        }
    }

    /// The name of a potential, `""` when unknown.
    pub fn potential_name(&self, id: PotentialId) -> &str {
        self.partition.get(id).map_or("", |p| p.name.text.as_str())
    }

    /// The potential text of a wire: the name, or `A ⇄ B` for a bond wire (the bond's side order).
    pub fn potential_text(&self, w: &PhysicalWire) -> String {
        if let Some(bond) = w
            .bond
            .and_then(|b| self.partition.bonds.iter().find(|l| l.bond == b))
            && let (Some(a), Some(b)) = (bond.a, bond.b)
        {
            return format!("{} \u{21C4} {}", self.potential_name(a), self.potential_name(b));
        }
        w.potentials
            .iter()
            .map(|p| self.potential_name(*p))
            .collect::<Vec<_>>()
            .join(" \u{21C4} ")
    }
}

/// Puts the From end of a wire first (doc 08 §3.8 "Orientation (From/To)", FR-CON-040): on different pages
/// the end on the earlier page in the page order; otherwise the end with the smaller anchor cell (row, then
/// column), then the smaller CP ID. The page of an end is the page of the constituent that ends there.
///
/// The flow-distance step of the rule (hops over wires, bridges and terminal shorts from the points where
/// the potential enters) is not applied: it needs a per-CP flow distance that `ew-derive` does not expose.
/// A wire that rule decides differently on one page prints its ends the other way round.
fn orient(q: &Q, w: &mut PhysicalWire) {
    let page_of = |c: Option<&Id<Connection>>| c.and_then(|c| q.connections().get(*c)).and_then(|c| c.page);
    let (pa, pb) = (page_of(w.constituents.first()), page_of(w.constituents.last()));
    let swap = match (pa, pb) {
        (Some(a), Some(b)) if a != b => page_position(q, b) < page_position(q, a),
        _ => {
            let cell = |e: &WireEnd, page: Option<Id<Page>>| -> (u32, u32) {
                let Some(page) = page else {
                    return (u32::MAX, u32::MAX);
                };
                let mut reps: Vec<_> = q
                    .idx()
                    .reps_of_function(e.cp.function)
                    .filter(|r| q.reps().get(*r).is_some_and(|r| r.page == page))
                    .collect();
                reps.sort();
                reps.first()
                    .and_then(|r| q.get::<RepAnchorCell>(*r).as_ref().clone())
                    .map_or((u32::MAX, u32::MAX), |c| (c.row, c.column))
            };
            let page = pa.or(pb);
            (cell(&w.ends[1], page), w.ends[1].cp) < (cell(&w.ends[0], page), w.ends[0].cp)
        }
    };
    if swap {
        w.ends.swap(0, 1);
    }
}

/// The device of a CP's function; `None` for an interruption point.
pub(super) fn device_of(q: &Q, f: Id<Function>) -> Option<Id<Device>> {
    q.functions().get(f).and_then(|f| f.device)
}

/// The printed designation of a CP (`""` when the function has no such CP).
pub(super) fn cp_designation(q: &Q, cp: CpRef) -> String {
    q.get::<FunctionCps>(cp.function)
        .iter()
        .find(|c| c.idx == cp.idx)
        .map(|c| c.designation.clone())
        .unwrap_or_default()
}

/// The name of an interruption point function.
pub(super) fn ip_name(q: &Q, f: Id<Function>) -> String {
    match q.functions().get(f).map(|f| f.kind.clone()) {
        Some(FunctionKind::Ip(ip)) => ip.name.as_str().to_owned(),
        _ => String::new(),
    }
}

/// A wire end as the tag cell and the CP cell of a table: a device end is its tag and the CP's
/// designation; an external end `ext. <name>` with no CP; an open end `open <name>`.
pub(super) fn end_cells(q: &Q, end: &WireEnd) -> (Cell, Cell) {
    match end.kind {
        EndKind::Device => match device_of(q, end.cp.function) {
            Some(d) => {
                let cp = cp_designation(q, end.cp);
                (
                    Cell::Tag(d),
                    if cp.is_empty() {
                        Cell::Empty
                    } else {
                        Cell::Ident(cp)
                    },
                )
            }
            None => (Cell::Empty, Cell::Empty),
        },
        EndKind::External => (
            Cell::Ident(format!("ext. {}", ip_name(q, end.cp.function))),
            Cell::Empty,
        ),
        EndKind::Open => (
            Cell::Ident(format!("open {}", ip_name(q, end.cp.function))),
            Cell::Empty,
        ),
    }
}

/// A wire end as one text: `-101F1:2`, a terminal `-X1031:1/b`, `ext. J-E-206-L`.
pub(super) fn end_text(q: &Q, end: &WireEnd) -> String {
    match end.kind {
        EndKind::Device => {
            let Some(d) = device_of(q, end.cp.function) else {
                return String::new();
            };
            let short = tag_short(q, d);
            let cp = cp_designation(q, end.cp);
            let is_terminal = q.devices().get(d).is_some_and(|d| d.aspects.terminal.is_some());
            if is_terminal {
                let clamp = q
                    .get::<FunctionCps>(end.cp.function)
                    .iter()
                    .find(|c| c.idx == end.cp.idx)
                    .and_then(|c| match &c.clamp {
                        Some(ClampKey::Clamp(ch)) => Some(ch.to_string()),
                        _ => None,
                    });
                match clamp {
                    Some(c) => format!("{short}/{c}"),
                    None => short,
                }
            } else if cp.is_empty() {
                short
            } else {
                format!("{short}:{cp}")
            }
        }
        EndKind::External => format!("ext. {}", ip_name(q, end.cp.function)),
        EndKind::Open => format!("open {}", ip_name(q, end.cp.function)),
    }
}

/// The tag of a device without its structure (`-X1031:1`), `""` for a device without one.
pub(super) fn tag_short(q: &Q, d: Id<Device>) -> String {
    q.get::<DeviceTag>(d)
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .unwrap_or_default()
}

/// The natural-order key of a device's full tag, for sorting.
pub(super) fn tag_key(q: &Q, d: Id<Device>) -> NaturalKey {
    natural_key(
        q.get::<DeviceTag>(d)
            .as_ref()
            .as_ref()
            .map(|t| t.full.as_str())
            .unwrap_or(""),
    )
}

/// The position of a drawn page in the drawn page order (`usize::MAX` for an unknown page).
pub(super) fn page_position(q: &Q, page: Id<Page>) -> usize {
    q.get::<DrawnPageOrder>(())
        .position(PageRef::Drawn(page))
        .unwrap_or(usize::MAX)
}

/// The pages of a wire as a folio cell, in page order.
pub(super) fn pages_cell(pages: &[Id<Page>]) -> Cell {
    if pages.is_empty() {
        Cell::Empty
    } else {
        Cell::Folios(pages.iter().map(|p| PageRef::Drawn(*p)).collect())
    }
}

/// The colour of a property as a cell; `Empty` without one.
pub(super) fn colour_cell(c: Option<&ColourSpec>) -> Cell {
    c.map_or(Cell::Empty, |c| Cell::Colour(c.clone()))
}

/// The cross-section as a cell: mm² as a quantity, AWG and kcmil as identifiers, `Empty` without one.
pub(super) fn csa_cell(c: Option<&Csa>) -> Cell {
    match c {
        Some(Csa::Mm2(d)) => Cell::Qty(*d, QtyUnit::Unit(Unit::Mm2)),
        Some(Csa::Awg(n)) => Cell::Ident(format!("AWG{n}")),
        Some(Csa::Kcmil(d)) => Cell::Ident(format!("{d} kcmil")),
        Some(Csa::Unknown(_)) | None => Cell::Empty,
    }
}

/// Where a wire's effective cross-section and colour come from, in the words of doc 20 §8.1: `cdp` (an
/// override on the wire), `core` (the cable's conductor), `rule` (a row of the colour or size profile),
/// `bond` (the bond rule), `potential` (a potential definition), `project` and `built-in` for the project
/// default and the built-in defaults. The two values are joined, size first, when they differ
/// (`cdp/rule`).
pub(super) fn source_text(props: &EffectiveProps) -> String {
    let name = |s: &PropSource| match s {
        PropSource::Override(_) => "cdp",
        PropSource::Conductor(_) => "core",
        PropSource::SizeRow {
            rule: ew_derive::SizeRule::Bond,
            ..
        } => "bond",
        PropSource::ColourRow { .. } | PropSource::SizeRow { .. } => "rule",
        PropSource::PotentialDef(_) => "potential",
        PropSource::ProjectDefault => "project",
        PropSource::BuiltIn { .. } => "built-in",
    };
    let size = props.csa.as_ref().map(|p| name(&p.source));
    let colour = props.colour.as_ref().map(|p| name(&p.source));
    match (size, colour) {
        (Some(s), Some(c)) if s == c => s.to_owned(),
        (Some(s), Some(c)) => format!("{s}/{c}"),
        (Some(s), None) | (None, Some(s)) => s.to_owned(),
        (None, None) => String::new(),
    }
}

/// A neutral text cell.
pub(super) fn text_cell(s: impl AsRef<str>) -> Cell {
    let s = s.as_ref();
    if s.is_empty() {
        Cell::Empty
    } else {
        Cell::Text(MlText::neutral(s))
    }
}

/// One terminal bridge: the bridge connections of one group on one strip (doc 08 §6.5).
#[derive(Clone, Debug)]
pub(super) struct Bridge {
    /// The strip.
    pub strip: Id<Device>,
    /// The group number on the strip.
    pub group: u16,
    /// The bridge kind's name (`plug-in`, `saddle`, `wire`, `inter-level`).
    pub kind: &'static str,
    /// The terminals it joins in `(seq, ID)` order.
    pub terminals: Vec<Id<Device>>,
    /// The page of its first drawn connection, if any is drawn.
    pub page: Option<Id<Page>>,
    /// The label `BR<n>`, numbered over the strips in natural tag order, then group number.
    pub label: String,
}

/// Every bridge of the project: the `Bridge` connections grouped by `(strip, group)`, labelled `BR1`, `BR2` …
/// in natural order of the strip tag, then group. Bridges are never wire rows (doc 08 §6.5).
pub(super) fn bridges(q: &Q) -> Vec<Bridge> {
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

/// The designation of a terminal device (`""` for a device that is not a terminal).
pub(super) fn terminal_designation(q: &Q, t: Id<Device>) -> String {
    q.devices()
        .get(t)
        .and_then(|d| {
            d.aspects
                .terminal
                .as_ref()
                .map(|t| t.designation.as_str().to_owned())
        })
        .unwrap_or_default()
}

/// The first and last designation of a bridge's terminals: `1–4` (en dash), a single terminal as is.
pub(super) fn bridge_span(q: &Q, b: &Bridge) -> String {
    match (b.terminals.first(), b.terminals.last()) {
        (Some(a), Some(z)) if a != z => {
            format!(
                "{}\u{2013}{}",
                terminal_designation(q, *a),
                terminal_designation(q, *z)
            )
        }
        (Some(a), _) => terminal_designation(q, *a),
        _ => String::new(),
    }
}
