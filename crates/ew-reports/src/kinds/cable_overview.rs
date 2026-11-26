// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cable_overview_diagrams`: the cable overview and the per-cable core lists (FR-REP-012, FR-REP-013;
//! folio 906 of the standard layout).

use std::collections::BTreeMap;

use ew_derive::{DevicePartLines, Q, short_tag};
use ew_ids::Id;
use ew_mltext::{MlText, NaturalKey, natural_key};
use ew_schema::{Conductor, Device, Function, LengthSource, ReportBlock, ReportKind};

use super::conn::{WireRec, Wires, colour_cell, csa_cell, end_text, text_cell, words_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, MissingReason, Row, RowId, RowKey, RowKind, RowSet};

/// The cable overview and the core lists.
///
/// A heading "Cable overview", one row per cable in natural order of its tag (type, ends A and B, length),
/// then for each cable a heading with its tag and part and one row per core.
///
/// The ends of a cable are the devices at the From and To ends of its cores (doc 08 §7.7, a terminal
/// standing for its strip), and of a bus cable the devices of its bus line's ports; a core row
/// prints the end of its physical wire (`-X1031:12/b`, `-201U1:1`); a core of a bus cable, over which no wire runs,
/// the pins the signal maps of its bus line's ports give it (`ew_derive::bus_core_pins`, FR-BUS-023), else the port
/// itself (`-101A1:DP`): the line uses every core of its cable; a core of no wire and no line prints nothing.
/// The length is the manual length in metres, `from part` for a length taken from the part
/// (the part carries no value yet), and "length missing" for a cable without one (EW-CORE-033). Cores come
/// in natural order of their names, the shield last: a pin order is not modelled. A core's
/// colour and cross-section are the wire's effective values, else the conductor's.
#[derive(Clone, Copy, Debug)]
pub struct CableOverview;

impl ReportKindDef for CableOverview {
    fn kind(&self) -> ReportKind {
        ReportKind::CableOverviewDiagrams
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("cable", CellType::Tag, text!("column-cable")).weighted(10),
            ColumnDef::new("part", CellType::Ident, text!("column-cable-type")).weighted(16),
            ColumnDef::new("core", CellType::Ident, text!("column-core")).weighted(7),
            ColumnDef::new("colour", CellType::Colour, text!("column-colour")).weighted(7),
            ColumnDef::new("cross_section", CellType::Qty, text!("column-mm2")).weighted(7),
            ColumnDef::new("from", CellType::Text, text!("column-from")).weighted(18),
            ColumnDef::new("to", CellType::Text, text!("column-to")).weighted(18),
            ColumnDef::new("length", CellType::Qty, text!("column-length")).weighted(12),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let wires = Wires::of(q);
        let by_core: BTreeMap<Id<Conductor>, &WireRec> = wires
            .wires
            .iter()
            .filter_map(|w| Some((w.wire.core?, w)))
            .collect();

        let mut cables: Vec<(NaturalKey, Id<Device>)> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.aspects.cable.is_some())
            .map(|(id, _)| (natural_key(&short_tag(q, id)), id))
            .collect();
        cables.sort();

        let mut set = RowSet::new(columns);
        if cables.is_empty() {
            return set;
        }
        set.rows.push(heading(
            set.columns.len(),
            "overview",
            crate::words::in_languages(&text!("cell-cable-overview")),
        ));
        for (_, cable) in &cables {
            set.rows.push(cable_row(q, *cable, &by_core));
        }
        for (_, cable) in &cables {
            let part = part_number(q, *cable);
            let title = if part.is_empty() {
                short_tag(q, *cable)
            } else {
                format!("{} {part}", short_tag(q, *cable))
            };
            set.rows.push(heading(
                set.columns.len(),
                &format!("cable/{}", cable.as_uuid()),
                MlText::neutral(&title),
            ));
            let mut cores: Vec<(bool, Option<u8>, NaturalKey, Id<Conductor>)> = q
                .idx()
                .conductors_of_cable(*cable)
                .filter_map(|c| {
                    let c = q.conductors().get(c)?;
                    Some((c.shield, c.pair, natural_key(c.core.as_str()), c.id))
                })
                .collect();
            cores.sort();
            for (_, _, _, id) in cores {
                if let Some(row) = core_row(q, *cable, id, &by_core) {
                    set.rows.push(row);
                }
            }
        }
        set
    }
}

/// A heading row of `width` cells, `text` in the first.
fn heading(width: usize, key: &str, text: MlText) -> Row {
    let mut cells = vec![Cell::Empty; width];
    cells[0] = Cell::Text(text);
    Row {
        id: RowId(vec![
            RowKey::Name("heading".to_owned()),
            RowKey::Name(key.to_owned()),
        ]),
        kind: RowKind::Heading,
        cells,
        links: Vec::new(),
        sources: Vec::new(),
    }
}

fn part_number(q: &Q, cable: Id<Device>) -> String {
    q.get::<DevicePartLines>(cable)
        .iter()
        .find(|l| !l.is_accessory())
        .and_then(|l| q.parts().get(l.part))
        .map(|c| c.item.part_number.as_str().to_owned())
        .unwrap_or_default()
}

/// A cable's length as every list prints it: metres to the millimetre, "from part" for a length the part gives, "length
/// missing" for a cable without one (EW-CORE-033), else empty.
pub(super) fn length_cell(q: &Q, cable: Id<Device>) -> Cell {
    q.devices()
        .get(cable)
        .and_then(|d| d.aspects.cable.as_ref())
        .map_or(Cell::Empty, |c| length_source_cell(&c.length))
}

/// A cable's length source as every list prints it ([`length_cell`]), and Properties with it: metres to the
/// millimetre, "from part", "length missing"; empty for a source of a newer format.
pub fn length_source_cell(source: &LengthSource) -> Cell {
    match source {
        LengthSource::Manual(nm) => Cell::metres(*nm),
        LengthSource::FromPart => words_cell(&text!("cell-from-part")),
        LengthSource::Missing => Cell::Missing(MissingReason::Length),
        LengthSource::Unknown(_) => Cell::Empty,
    }
}

/// The length source whose words in a shipped language `text` is (any case, the spaces at its ends aside): a length
/// taken from the part, or none ([`length_cell`]'s words); `None` for any other text.
pub fn cable_length_words(text: &str) -> Option<LengthSource> {
    let typed = text.trim().to_lowercase();
    let said = |t: &ew_l10n::Text| {
        let words = crate::words::in_languages(t);
        words
            .languages()
            .filter_map(|l| words.get(l))
            .any(|w| w.to_lowercase() == typed)
    };
    if said(&text!("cell-from-part")) {
        Some(LengthSource::FromPart)
    } else if said(&text!("cell-length-missing")) {
        Some(LengthSource::Missing)
    } else {
        None
    }
}

/// The text of a set of devices: their short tags in natural order, without repeats.
fn device_set(q: &Q, devices: impl IntoIterator<Item = Id<Device>>) -> String {
    let mut tags: Vec<(NaturalKey, String)> = devices
        .into_iter()
        .map(|d| short_tag(q, d))
        .filter(|t| !t.is_empty())
        .map(|t| (natural_key(&t), t))
        .collect();
    tags.sort();
    tags.dedup();
    tags.into_iter().map(|(_, t)| t).collect::<Vec<_>>().join(", ")
}

/// The devices of a cable's end, then the external point its bus line ends at.
fn with_point(devices: String, point: Option<String>) -> String {
    match point {
        Some(p) if devices.is_empty() => p,
        Some(p) => format!("{devices}, {p}"),
        None => devices,
    }
}

/// The functions at the two ends of the bus line a cable carries (`ew_derive::cable_bus_line`), end a first.
fn bus_line_ports(q: &Q, cable: Id<Device>) -> [Option<Id<Function>>; 2] {
    ew_derive::cable_bus_line(q, cable).map_or([None, None], |l| [l.a.function(), l.b.function()])
}

fn cable_row(q: &Q, cable: Id<Device>, by_core: &BTreeMap<Id<Conductor>, &WireRec>) -> Row {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    // A bus cable's ends are the devices of its line's ports.
    let [pa, pb] = bus_line_ports(q, cable);
    a.extend(pa.and_then(|f| super::conn::device_of(q, f)));
    b.extend(pb.and_then(|f| super::conn::device_of(q, f)));
    // An end of no device (an external point) is named as the core rows name it.
    let point = |f: Option<Id<Function>>| {
        f.filter(|f| super::conn::device_of(q, *f).is_none())
            .map(|f| ew_derive::bus_end_label(q, f))
            .filter(|t| !t.is_empty())
    };
    let (point_a, point_b) = (point(pa), point(pb));
    for c in q.idx().conductors_of_cable(cable) {
        if let Some(w) = by_core.get(&c) {
            // A terminal end is its strip: end A of -106W1 is `-X1031, -X1032`, not the single terminals.
            let dev = |i: usize| {
                super::conn::device_of(q, w.wire.ends[i].cp.function).map(|d| {
                    q.devices()
                        .get(d)
                        .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip))
                        .unwrap_or(d)
                })
            };
            a.extend(dev(0));
            b.extend(dev(1));
        }
    }
    let part = part_number(q, cable);
    let mut r = Row::data(
        RowId::entity(cable),
        vec![
            Cell::Tag(cable),
            if part.is_empty() {
                Cell::Empty
            } else {
                Cell::Ident(part)
            },
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            text_cell(with_point(device_set(q, a), point_a)),
            text_cell(with_point(device_set(q, b), point_b)),
            length_cell(q, cable),
        ],
    );
    r.sources = vec![cable.cast()];
    r
}

fn core_row(
    q: &Q,
    cable: Id<Device>,
    core: Id<Conductor>,
    by_core: &BTreeMap<Id<Conductor>, &WireRec>,
) -> Option<Row> {
    let c = q.conductors().get(core)?;
    let wire = by_core.get(&core);
    let colour = wire
        .and_then(|w| w.props.colour.as_ref().map(|p| p.value.clone()))
        .or_else(|| c.colour.clone());
    let csa = wire
        .and_then(|w| w.props.csa.as_ref().map(|p| p.value.clone()))
        .or_else(|| c.csa.clone());
    // a core no wire runs over: the pins a bus line's signal maps give it (FR-BUS-023), else the line's port: the bus
    // line uses every core of its cable
    let bus = if wire.is_none() {
        ew_derive::bus_core_ends(q, cable, c.core.as_str(), c.shield)
    } else {
        [None, None]
    };
    let end = |i: usize| match wire {
        Some(w) => end_text(q, &w.wire.ends[i]),
        None => bus[i].clone().unwrap_or_default(),
    };
    let mut r = Row::data(
        RowId::entity(cable).with(RowKey::Entity(core.as_uuid())),
        vec![
            Cell::Tag(cable),
            Cell::Empty,
            Cell::Ident(c.core.as_str().to_owned()),
            if c.shield {
                Cell::Empty
            } else {
                colour_cell(colour.as_ref())
            },
            if c.shield {
                Cell::Empty
            } else {
                csa_cell(csa.as_ref())
            },
            text_cell(end(0)),
            text_cell(end(1)),
            Cell::Empty,
        ],
    );
    r.sources = vec![cable.cast()];
    Some(r)
}
