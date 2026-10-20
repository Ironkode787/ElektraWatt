// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `potential_overview`: the potential overview (FR-REP-016; golden 909).

use std::collections::BTreeSet;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::{CircuitClass, Page, PotentialType, ReportBlock};
use ew_derive::{DrawnPageOrder, PageRef, Potential, PotentialId, PropSource, Q};

use super::conn::{Wires, colour_cell, csa_cell, put};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The potential overview.
///
/// One row per potential of the partition in natural order of its name, then one row per bond. Member
/// counts, not member lists, so that rows never wrap (doc 20 §16): `aliases` counts the interruption-point
/// names on the potential besides its own name, `members` its device connection points (a terminal once,
/// whatever its clamps and sides) and its external points. `colour` and
/// `cross_section` are the profile's default for the potential: the effective values of its first wire (by
/// wire ID) whose value comes from the profile or a potential definition (not from a wire override, a core
/// or a bond rule); empty when no wire has one. `pages` are the pages of its wires, in page order.
///
/// A potential the model has no record of (the PROFIBUS `DP` bus line of doc 20 §7.3, which needs the `Bus`
/// connections of the PLC work package) has no row.
#[derive(Clone, Copy, Debug)]
pub struct PotentialOverview;

impl ReportKindDef for PotentialOverview {
    fn name(&self) -> &'static str {
        "potential_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("potential", CellType::Ident, "Potential").weighted(14),
            ColumnDef::new("kind", CellType::Text, "Type").weighted(12),
            ColumnDef::new("voltage", CellType::Ident, "Voltage").weighted(10),
            ColumnDef::new("class", CellType::Text, "Class").weighted(10),
            ColumnDef::new("colour", CellType::Colour, "Colour").weighted(8),
            ColumnDef::new("cross_section", CellType::Qty, "mm\u{00B2}").weighted(8),
            ColumnDef::new("aliases", CellType::Number, "Aliases").weighted(8),
            ColumnDef::new("members", CellType::Number, "Members").weighted(8),
            ColumnDef::new("pages", CellType::Folios, "Pages").weighted(14),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let wires = Wires::of(q);
        let order = q.get::<DrawnPageOrder>(());
        let mut keyed: Vec<((NaturalKey, PotentialId), Row)> = wires
            .partition
            .potentials()
            .map(|p| {
                let pages = potential_pages(&wires, p, &order);
                (
                    (natural_key(wires.potential_name(p.id)), p.id),
                    potential_row(q, &wires, p, pages),
                )
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        let mut set = RowSet::new(columns);
        set.rows = keyed.into_iter().map(|(_, r)| r).collect();

        for (i, bond) in wires.partition.bonds().iter().enumerate() {
            let name = |p: Option<PotentialId>| p.map_or("?", |p| wires.potential_name(p)).to_owned();
            let label = format!("B{} ({} \u{21C4} {})", i + 1, name(bond.a), name(bond.b));
            let pages: Vec<PageRef> = bond
                .via
                .and_then(|v| {
                    wires
                        .wires
                        .iter()
                        .find(|w| w.wire.id == v)
                        .map(|w| w.wire.pages.iter().map(|p| PageRef::Drawn(*p)).collect())
                })
                .unwrap_or_default();
            let mut cells = vec![Cell::Empty; set.columns.len()];
            put(&set.columns, &mut cells, "potential", Cell::Ident(label));
            put(
                &set.columns,
                &mut cells,
                "kind",
                Cell::Text(ew_derive::ew_mltext::MlText::neutral("Bond")),
            );
            if !pages.is_empty() {
                put(&set.columns, &mut cells, "pages", Cell::Folios(pages));
            }
            set.rows.push(Row::data(
                RowId::entity(bond.bond).with(RowKey::Name("bond".to_owned())),
                cells,
            ));
        }
        set
    }
}

fn potential_pages(wires: &Wires, p: &Potential, order: &ew_derive::PageOrderV) -> Vec<PageRef> {
    let mut pages: BTreeSet<(usize, Id<Page>)> = BTreeSet::new();
    for w in &wires.wires {
        if w.wire.potentials.contains(&p.id) {
            for page in &w.wire.pages {
                pages.insert((order.position(PageRef::Drawn(*page)).unwrap_or(usize::MAX), *page));
            }
        }
    }
    pages.into_iter().map(|(_, p)| PageRef::Drawn(p)).collect()
}

/// The members of a potential as doc 20 §7.3 counts them: the connection points of devices, a terminal as one
/// member whatever its clamps and sides, and the external interruption points (the other side of a field
/// wire); the interruption points between pages are not members.
fn member_count(q: &Q, p: &Potential) -> usize {
    use ew_derive::ew_schema::FunctionKind;
    let mut members: BTreeSet<(ew_derive::ew_ids::Uuid, u16)> = BTreeSet::new();
    for cp in &p.cps {
        let Some(f) = q.functions().get(cp.function) else {
            continue;
        };
        match (&f.kind, f.device) {
            (FunctionKind::Ip(ip), _) => {
                if ip.external.is_some() {
                    members.insert((cp.function.as_uuid(), cp.idx));
                }
            }
            (_, Some(d)) if q.devices().get(d).is_some_and(|d| d.aspects.terminal.is_some()) => {
                members.insert((d.as_uuid(), 0));
            }
            _ => {
                members.insert((cp.function.as_uuid(), cp.idx));
            }
        }
    }
    members.len()
}

fn potential_row(q: &Q, wires: &Wires, p: &Potential, pages: Vec<PageRef>) -> Row {
    // The profile default: the first wire by ID whose value comes from a profile row or a definition.
    let default = |from_profile: &dyn Fn(&crate::kinds::conn::WireRec) -> bool| {
        wires
            .wires
            .iter()
            .filter(|w| w.wire.potentials.contains(&p.id) && w.wire.bond.is_none() && from_profile(w))
            .map(|w| &w.props)
            .next()
    };
    let profile = |s: &PropSource| {
        matches!(
            s,
            PropSource::ColourRow { .. } | PropSource::SizeRow { .. } | PropSource::PotentialDef(_)
        )
    };
    let colour = default(&|w| w.props.colour.as_ref().is_some_and(|c| profile(&c.source)))
        .and_then(|e| e.colour.as_ref());
    let csa =
        default(&|w| w.props.csa.as_ref().is_some_and(|c| profile(&c.source))).and_then(|e| e.csa.as_ref());
    let attrs = q.get::<ew_derive::PotentialAttrsOf>(p.id);
    let names = q.get::<ew_derive::PotentialNameOf>(p.id);
    let kind = attrs.ptype.as_ref().map_or(Cell::Empty, |t| {
        Cell::Text(ew_derive::ew_mltext::MlText::neutral(type_name(&t.value)))
    });
    let voltage = attrs.voltage.as_ref().map_or(Cell::Empty, |v| {
        let ac_dc = match attrs.env.as_ref().and_then(|e| e.value.ac_dc.as_ref()) {
            Some(ew_derive::ew_schema::AcDc::Ac) => " V AC",
            Some(ew_derive::ew_schema::AcDc::Dc) => " V DC",
            _ => " V",
        };
        Cell::Ident(format!("{}{ac_dc}", v.value))
    });
    let class = attrs.circuit_class.as_ref().map_or(Cell::Empty, |c| {
        Cell::Text(ew_derive::ew_mltext::MlText::neutral(class_name(&c.value)))
    });
    let mut r = Row::data(
        match p.id {
            PotentialId::Def(d) => RowId::entity(d),
            PotentialId::Cp(c) => RowId::entity(c),
        }
        .with(RowKey::Name("potential".to_owned())),
        vec![
            Cell::Ident(names.name.text.clone()),
            kind,
            voltage,
            class,
            colour_cell(colour.map(|c| &c.value)),
            csa_cell(csa.map(|c| &c.value)),
            Cell::Number(i64::try_from(names.aliases.len()).unwrap_or(i64::MAX)),
            Cell::Number(i64::try_from(member_count(q, p)).unwrap_or(i64::MAX)),
            if pages.is_empty() {
                Cell::Empty
            } else {
                Cell::Folios(pages)
            },
        ],
    );
    r.sources = Vec::new();
    r
}

fn type_name(t: &PotentialType) -> &'static str {
    match t {
        PotentialType::L1 => "L1",
        PotentialType::L2 => "L2",
        PotentialType::L3 => "L3",
        PotentialType::L => "L",
        PotentialType::N => "N",
        PotentialType::Pe => "PE",
        PotentialType::Pen => "PEN",
        PotentialType::Fe => "FE",
        PotentialType::LPlus => "L+",
        PotentialType::LMinus => "L-",
        PotentialType::M => "M",
        PotentialType::Signal => "signal",
        PotentialType::Undefined => "undefined",
        PotentialType::Unknown(_) => "other",
    }
}

fn class_name(c: &CircuitClass) -> &'static str {
    match c {
        CircuitClass::Mains => "mains",
        CircuitClass::Control => "control",
        CircuitClass::Analogue => "analogue",
        CircuitClass::Bus => "bus",
        CircuitClass::Pe => "PE",
        CircuitClass::Unknown(_) => "other",
    }
}
