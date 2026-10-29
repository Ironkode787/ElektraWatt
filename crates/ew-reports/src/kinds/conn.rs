// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the connectivity kinds (`wire_list`, `potential_overview`, `terminal_diagram`,
//! `cable_overview_diagrams`) share: the project's physical wires with their effective properties, wire ends
//! as cells, terminal bridges and the cell formatting of wire properties.
//!
//! All reads go through `Q` (the Derivation II queries `potential_partition`, `physical_wires_of`,
//! `wire_props_of` and the tracked tables); nothing here reads the model directly. Those queries read the
//! drawn page order (`DrawnPageOrder`, which has no report pages), not the full page order that the report
//! engine's pagination is part of, so the kinds derive their rows inside the pagination like any other kind.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{
    BridgeKind, ClampKey, ColourSpec, Connection, Csa, Device, Function, FunctionKind, Page, Unit,
};
use ew_derive::{
    BridgeGroup, CpRef, DeviceTag, EffectiveProps, EndKind, FunctionCps, PageRef, Partition, PhysicalWire,
    PhysicalWiresOf, PotentialId, PotentialNameOf, PotentialNames, PotentialPartition, PropSource, Q,
    StripBridges, WireEnd, WirePropsOf, orient_wire,
};

use crate::rowset::{Cell, ColumnDef, QtyUnit};

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
    /// The names of every potential (`potential_name`).
    pub names: BTreeMap<PotentialId, Arc<PotentialNames>>,
    /// The wires.
    pub wires: Vec<WireRec>,
}

impl Wires {
    /// Reads the partition, the physical wires of every potential and their effective properties.
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
                    orient_wire(q, &mut wire);
                    WireRec {
                        wire,
                        props: props.wires.get(&w.id).cloned().unwrap_or_default(),
                    }
                });
            }
        }
        Wires {
            partition,
            names,
            wires: by_id.into_values().collect(),
        }
    }

    /// The name of a potential, `""` when unknown.
    pub fn potential_name(&self, id: PotentialId) -> &str {
        self.names.get(&id).map_or("", |n| n.name.text.as_str())
    }

    /// The potential text of a wire: the name, or `A ⇄ B` for a bond wire (the bond's side order).
    pub fn potential_text(&self, w: &PhysicalWire) -> String {
        if let Some(bond) = w
            .bond
            .and_then(|b| self.partition.bonds().iter().find(|l| l.bond == b))
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

/// How a wire is joined (doc 08 §3.2, FR-CON-005, FR-CON-006; D-20), as the Via column prints it: the
/// junction it passes by its place and wiring, `"<folio>-<cell> star at <CP>"` or `"… chain on <CP>[, <CP>]"`
/// (the wiring without CPs when none decides), then its daisy chains, `"chain on <CP>[, <CP>]"`; each `None`
/// when absent. The junction's style (dot, bevel) is drawing notation and stays on the page.
pub(super) fn join_texts(q: &Q, w: &PhysicalWire) -> (Option<String>, Option<String>) {
    let join = ew_derive::wire_join(q, w);
    // `#n` where printed designations repeat, so the star centres of J5 and J6 stay apart.
    let cp_text = |cp: &CpRef| ew_derive::cp_label(q, *cp);
    let junction = join.junction.map(|j| {
        let folio = q
            .get::<ew_derive::DrawnPageOrder>(())
            .entry(PageRef::Drawn(j.page))
            .map(|e| e.folio.clone())
            .unwrap_or_default();
        let place = match &j.cell {
            Some(c) => format!("{folio}-{c}"),
            None => folio,
        };
        let cps: Vec<String> = j.cps.iter().map(cp_text).collect();
        let mut text = format!("{place} {}", j.wiring.name());
        if !cps.is_empty() {
            let word = match j.wiring {
                ew_derive::JoinWiring::Star => "at",
                ew_derive::JoinWiring::Chain => "on",
            };
            text.push_str(&format!(" {word} {}", cps.join(", ")));
        }
        text
    });
    let daisy = (!join.daisy.is_empty()).then(|| {
        let cps: Vec<String> = join.daisy.iter().map(cp_text).collect();
        format!("chain on {}", cps.join(", "))
    });
    (junction, daisy)
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

/// Every bridge of the project: the strips' [`StripBridges`] groups, labelled `BR1`, `BR2` … in natural order of
/// the strip tag, then group (then strip ID). The strips are those that some terminal names. Bridges are never wire
/// rows (doc 08 §6.5).
pub(super) fn bridges(q: &Q) -> Vec<Bridge> {
    let connections = q.connections();
    bridge_groups(q)
        .into_iter()
        .map(|(label, strip, g)| {
            let kind = match &g.kind {
                BridgeKind::PlugIn => "plug-in",
                BridgeKind::Saddle => "saddle",
                BridgeKind::Wire => "wire",
                BridgeKind::InterLevel => "inter-level",
                BridgeKind::Unknown(_) => "bridge",
            };
            // The page of the group's first drawn connection in ID order.
            let mut ids = g.connections.clone();
            ids.sort_unstable();
            let page = ids.iter().find_map(|c| connections.get(*c).and_then(|c| c.page));
            Bridge {
                strip,
                group: g.group,
                kind,
                terminals: g.terminals,
                page,
                label,
            }
        })
        .collect()
}

/// Every bridge group of the project with its label `BR<n>` and its strip (WP-S2.07 §4.2): the
/// [`StripBridges`] of every strip that a terminal names, numbered over the strips in natural order of the strip
/// tag, then group number, then strip ID.
pub fn bridge_groups(q: &Q) -> Vec<(String, Id<Device>, BridgeGroup)> {
    let strips: BTreeSet<Id<Device>> = q
        .devices()
        .iter()
        .filter_map(|(_, d)| d.aspects.terminal.as_ref().map(|t| t.strip))
        .collect();
    let mut ordered: Vec<(NaturalKey, u16, Id<Device>, BridgeGroup)> = Vec::new();
    for strip in strips {
        let groups = q.get::<StripBridges>(strip);
        if groups.is_empty() {
            continue;
        }
        let key = tag_key(q, strip);
        for g in groups.iter() {
            ordered.push((key.clone(), g.group, strip, g.clone()));
        }
    }
    ordered.sort_by(|a, b| (&a.0, a.1, a.2).cmp(&(&b.0, b.1, b.2)));
    ordered
        .into_iter()
        .enumerate()
        .map(|(i, (_, _, strip, g))| (format!("BR{}", i + 1), strip, g))
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
