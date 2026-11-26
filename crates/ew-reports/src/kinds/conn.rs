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

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_derive::{
    CpRef, DeviceTag, EndKind, FunctionCps, PageRef, Partition, PhysicalWire, PotentialId, PotentialNameOf,
    PotentialNames, PotentialPartition, PropSource, Q, WireEnd, WireRecord, WireRecordSet, bridge_groups,
    cp_label, cp_pin, cp_tag_from,
};
use ew_ids::Id;
use ew_l10n::Text;
use ew_mltext::{MlText, NaturalKey, natural_key};
use ew_schema::{
    BridgeKind, ColourSpec, Connection, Csa, Device, DeviceState, Function, FunctionKind, Page, Unit,
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

/// Whether a device has a row of its own in the device lists (`structure_overview`'s device rows,
/// `device_list`): every device that is not a terminal (the terminal reports list terminals).
pub(super) fn is_listed_device(dev: &Device) -> bool {
    dev.aspects.terminal.is_none()
}

/// The state of a terminal as the terminal reports print it (doc 08 §6.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TerminalState {
    /// Spare, as authored.
    Spare,
    /// Reserved, as authored.
    Reserved,
    /// Not fitted, as authored.
    NotFitted,
    /// Wired or bridged.
    Used,
    /// Neither wired nor bridged.
    Unused,
}

impl TerminalState {
    /// The state in words (`used`, in Dutch `gebruikt`; doc 09 §4.7).
    pub(super) fn words(self) -> Text {
        match self {
            TerminalState::Spare => text!("cell-spare"),
            TerminalState::Reserved => text!("cell-reserved"),
            TerminalState::NotFitted => text!("cell-not-fitted"),
            TerminalState::Used => text!("cell-used"),
            TerminalState::Unused => text!("cell-unused"),
        }
    }
}

/// The state of a terminal: spare, reserved or not fitted as authored, else used when the terminal has a wire (`wired`)
/// or belongs to a bridge (`bridged`), and unused otherwise. `terminal_diagram`'s State column and
/// `terminal_overview`'s counts use this one rule.
pub(super) fn terminal_state(state: &DeviceState, wired: bool, bridged: bool) -> TerminalState {
    match state {
        DeviceState::Spare => TerminalState::Spare,
        DeviceState::Reserved => TerminalState::Reserved,
        DeviceState::NotFitted => TerminalState::NotFitted,
        DeviceState::Used | DeviceState::Unknown(_) => {
            if wired || bridged {
                TerminalState::Used
            } else {
                TerminalState::Unused
            }
        }
    }
}

/// The physical wires by the CPs they end at, each CP's in wire-ID order.
pub(super) fn wires_by_cp(wires: &Wires) -> BTreeMap<CpRef, Vec<&WireRecord>> {
    let mut map: BTreeMap<CpRef, Vec<&WireRecord>> = BTreeMap::new();
    for rec in wires.wires.iter() {
        for end in &rec.wire.ends {
            map.entry(end.cp).or_default().push(rec);
        }
    }
    for recs in map.values_mut() {
        recs.sort_by_key(|r| r.wire.id);
    }
    map
}

/// Whether a wire ends at any CP of the terminal `t` (the `wired` of [`terminal_state`]).
pub(super) fn terminal_wired(q: &Q, t: Id<Device>, by_cp: &BTreeMap<CpRef, Vec<&WireRecord>>) -> bool {
    q.idx().functions_of_device(t).any(|f| {
        q.get::<FunctionCps>(f)
            .iter()
            .any(|cp| by_cp.contains_key(&CpRef::new(f, cp.idx)))
    })
}

/// Every physical wire of the project (`ew_derive::WireRecordSet`, in the numbering order) with the potential
/// partition they came from.
pub(super) struct Wires {
    /// The partition.
    pub partition: Arc<Partition>,
    /// The names of every potential (`potential_name`).
    pub names: BTreeMap<PotentialId, Arc<PotentialNames>>,
    /// The wires.
    pub wires: Arc<Vec<WireRecord>>,
    /// The index of each wire in `wires`, by its ID.
    pub by_id: BTreeMap<Id<Connection>, usize>,
}

impl Wires {
    /// Reads the partition, the names of its potentials and the project's wire records.
    pub fn of(q: &Q) -> Wires {
        let partition = q.get::<PotentialPartition>(());
        let names = partition
            .potentials()
            .map(|p| (p.id, q.get::<PotentialNameOf>(p.id)))
            .collect();
        let wires = q.get::<WireRecordSet>(());
        let by_id = wires.iter().enumerate().map(|(i, r)| (r.wire.id, i)).collect();
        Wires {
            partition,
            names,
            wires,
            by_id,
        }
    }

    /// The wire with the ID `id`.
    pub fn get(&self, id: Id<Connection>) -> Option<&WireRecord> {
        self.by_id.get(&id).and_then(|i| self.wires.get(*i))
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

/// The name of an interruption point function.
pub(super) fn ip_name(q: &Q, f: Id<Function>) -> String {
    match q.functions().get(f).map(|f| f.kind.clone()) {
        Some(FunctionKind::Ip(ip)) => ip.name.as_str().to_owned(),
        _ => String::new(),
    }
}

/// A wire end as the tag cell and the CP cell of the wire list (doc 13 §3.2), read against the wire's other end
/// `other`: a device end is its tag without the structure the two ends share (`ew_derive::cp_tag_from`) and its pin
/// (`ew_derive::cp_pin`: a repeated pin with `#n`, a terminal's clamp letter); an external end `ext. <name>` with no
/// CP; an open end `open <name>`.
pub(super) fn end_cells(q: &Q, end: &WireEnd, other: &WireEnd) -> (Cell, Cell) {
    match end.kind {
        EndKind::Device => (
            Cell::ident(&cp_tag_from(q, end.cp, Some(other.cp))),
            Cell::ident(&cp_pin(q, end.cp)),
        ),
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

/// A wire end as one text: `-101F1:2`, a repeated pin `-101G1:+#1`, a terminal `-X1031:1/b` (`ew_derive::cp_label`),
/// `ext. J-E-206-L`.
pub(super) fn end_text(q: &Q, end: &WireEnd) -> String {
    match end.kind {
        EndKind::Device => cp_label(q, end.cp),
        EndKind::External => format!("ext. {}", ip_name(q, end.cp.function)),
        EndKind::Open => format!("open {}", ip_name(q, end.cp.function)),
    }
}

/// How a wire is joined (doc 08 §3.2, FR-CON-005, FR-CON-006; D-20), as the Via column prints it: the junction it
/// passes by its place and wiring, `"<folio>-<cell> star at <CP>"` or `"… chain on <CP>[, <CP>]"` (the wiring without
/// CPs when none decides), then its daisy chains, `"chain on <CP>[, <CP>]"`; each `None` when absent. The junction's
/// style (dot, bevel) is drawing notation and stays on the page.
pub(super) fn join_texts(q: &Q, w: &PhysicalWire) -> (Option<Text>, Option<Text>) {
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
        match (j.wiring, cps.is_empty()) {
            (ew_derive::JoinWiring::Star, true) => text!("cell-join-star", place),
            (ew_derive::JoinWiring::Star, false) => text!("cell-join-star-at", place, cps.join(", ")),
            (ew_derive::JoinWiring::Chain, true) => text!("cell-join-chain", place),
            (ew_derive::JoinWiring::Chain, false) => text!("cell-join-chain-on", place, cps.join(", ")),
        }
    });
    let daisy = (!join.daisy.is_empty()).then(|| {
        let cps = join.daisy.iter().map(cp_text).collect::<Vec<String>>().join(", ");
        text!("cell-daisy-chain", cps)
    });
    (junction, daisy)
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
        Some(c @ (Csa::Awg(_) | Csa::Kcmil(_))) => ew_derive::csa_text(c).map_or(Cell::Empty, Cell::Ident),
        Some(Csa::Unknown(_)) | None => Cell::Empty,
    }
}

/// Where a wire's effective cross-section and colour come from: `cdp` (an
/// override on the wire), `core` (the cable's conductor), `rule` (a row of the colour or size profile),
/// `bond` (the bond rule), `potential` (a potential definition), `project` and `built-in` for the project
/// default and the built-in defaults. The two values are joined, size first, when they differ
/// (`cdp/rule`). In Dutch: `cdp`, `ader`, `regel`, `potentiaalverbinding`, `potentiaal`, `project`, `ingebouwd`.
pub(super) fn source_text(rec: &WireRecord) -> Cell {
    let name = |s: &PropSource| match s {
        PropSource::Override(_) => text!("cell-source-cdp"),
        PropSource::Conductor(_) => text!("cell-source-core"),
        PropSource::SizeRow {
            rule: ew_derive::SizeRule::Bond,
            ..
        } => text!("cell-source-bond"),
        PropSource::ColourRow { .. } | PropSource::SizeRow { .. } => text!("cell-source-rule"),
        PropSource::PotentialDef(_) => text!("cell-source-potential"),
        PropSource::ProjectDefault => text!("cell-source-project"),
        PropSource::BuiltIn { .. } => text!("cell-source-built-in"),
    };
    let size = rec.csa.as_ref().map(|p| name(&p.source));
    let colour = rec.colour.as_ref().map(|p| name(&p.source));
    let words = match (size, colour) {
        (Some(s), Some(c)) if s == c => s,
        (Some(s), Some(c)) => text!("cell-source-pair", s, c),
        (Some(s), None) | (None, Some(s)) => s,
        (None, None) => return Cell::Empty,
    };
    words_cell(&words)
}

/// A wire's size and colour as the workshop lists print them (`0.75 mm² BK`, with a comma where the list writes one).
pub(super) fn wire_type_cell(q: &Q, csa: Option<&Csa>, colour: Option<&ColourSpec>) -> Cell {
    let text = [
        csa.and_then(ew_derive::csa_with_unit).unwrap_or_default(),
        colour.map(ew_derive::colour_text).unwrap_or_default(),
    ]
    .into_iter()
    .filter(|t| !t.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    measured_cell(q, &Text::bare(text))
}

/// A cell of the program's own words that hold a measured value: its decimals written as the list writes a quantity
/// ([`crate::words::in_languages_measured`]).
pub(super) fn measured_cell(q: &Q, words: &Text) -> Cell {
    Cell::Text(crate::words::in_languages_measured(q, words))
}

/// A cell of the program's own words: it prints in the list's language (doc 09 §4.7).
pub(super) fn words_cell(words: &Text) -> Cell {
    Cell::Text(crate::words::in_languages(words))
}

/// A neutral text cell; an empty text is an empty cell.
pub(super) fn text_cell(s: impl AsRef<str>) -> Cell {
    ml_cell(MlText::neutral(s.as_ref()))
}

/// A cell of a text a person wrote in several languages; an empty text is an empty cell.
pub(super) fn ml_cell(t: MlText) -> Cell {
    if t.is_empty() { Cell::Empty } else { Cell::Text(t) }
}

/// One terminal bridge: the bridge connections of one group on one strip (doc 08 §6.5).
#[derive(Clone, Debug)]
pub(super) struct Bridge {
    /// The strip.
    pub strip: Id<Device>,
    /// The group number on the strip.
    pub group: u16,
    /// The bridge kind's name (`plug-in`, `saddle`, `wire`, `inter-level`; in Dutch the bridge's name, `insteekbrug`,
    /// `zadelbrug`, `draadbrug`, `brug tussen niveaus`).
    pub kind: Text,
    /// The terminals it joins in `(seq, ID)` order.
    pub terminals: Vec<Id<Device>>,
    /// Its connections, as the group orders them (a level of a block is bridged where they end, doc 08 §6.3).
    pub connections: Vec<Id<Connection>>,
    /// The page of its first drawn connection, if any is drawn.
    pub page: Option<Id<Page>>,
    /// The label `BR<n>`, numbered over the strips in natural tag order, then group number.
    pub label: String,
    /// Its part, the carrier connection's (`BridgeGroup::part`); `None` without one.
    pub part: Option<ew_schema::PartRef>,
}

/// Every bridge of the project: the strips' [`StripBridges`](ew_derive::StripBridges) groups, labelled `BR1`, `BR2` …
/// in natural order of the strip tag, then group (then strip ID). The strips are those that some terminal names. Bridges
/// are never wire rows (doc 08 §6.5).
pub(super) fn bridges(q: &Q) -> Vec<Bridge> {
    let connections = q.connections();
    bridge_groups(q)
        .into_iter()
        .map(|(label, strip, g)| {
            let kind = match &g.kind {
                BridgeKind::PlugIn => text!("cell-bridge-plug-in"),
                BridgeKind::Saddle => text!("cell-bridge-saddle"),
                BridgeKind::Wire => text!("cell-bridge-wire"),
                BridgeKind::InterLevel => text!("cell-bridge-inter-level"),
                BridgeKind::Unknown(_) => text!("cell-bridge-other"),
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
                connections: g.connections,
                page,
                label,
                part: g.part,
            }
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

/// The terminals a bridge joins in words, as the assembly list prints them: "1 to 4", a single terminal as is.
pub(super) fn bridge_span_from_to(q: &Q, b: &Bridge) -> Text {
    match (b.terminals.first(), b.terminals.last()) {
        (Some(a), Some(z)) if a != z => text!(
            "cell-terminals-from-to",
            terminal_designation(q, *a),
            terminal_designation(q, *z)
        ),
        (Some(a), _) => Text::bare(terminal_designation(q, *a)),
        _ => Text::bare(String::new()),
    }
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
