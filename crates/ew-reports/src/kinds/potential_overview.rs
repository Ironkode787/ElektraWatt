// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `potential_overview`: the potential overview (FR-REP-016; folio 909 of the standard layout).

use std::collections::BTreeSet;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{CircuitClass, Page, PotentialType, ReportBlock};
use ew_derive::{DrawnPageOrder, PageRef, Potential, PotentialId, PropSource, Q};
use ew_l10n::Text;

use super::conn::{Wires, colour_cell, csa_cell, put, words_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The potential overview.
///
/// One row per potential of the partition in natural order of its name, then one row per bond, numbered B1, B2 …:
/// the declared bonds by ID, then the drawn ones (an earth wire) by their wire's ID. Member
/// counts, not member lists, so that rows never wrap (doc 20 §16): `aliases` counts the interruption-point
/// names on the potential besides its own name, `members` its device connection points (a terminal once,
/// whatever its clamps and sides) and its external points. `colour` and
/// `cross_section` are the profile's default for the potential: the effective values of its first wire (by
/// wire ID) whose value comes from the profile or a potential definition (not from a wire override, a core
/// or a bond rule); "(cable)" for a bus line without one, whose colour and size are its cable's (a `Bus`
/// connection with `BusLine.cable`, doc 20 §7.3); empty otherwise. `pages` are the pages of its wires and of its drawn `Bus`
/// connections (a bus line is not a physical wire: the PROFIBUS `DP` of doc 20 §7.3 is drawn on folio 3 as
/// one single-line connection, FR-CBL-008), in page order.
#[derive(Clone, Copy, Debug)]
pub struct PotentialOverview;

impl ReportKindDef for PotentialOverview {
    fn name(&self) -> &'static str {
        "potential_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("potential", CellType::Ident, text!("column-potential")).weighted(14),
            ColumnDef::new("kind", CellType::Text, text!("column-type")).weighted(12),
            ColumnDef::new("voltage", CellType::Ident, text!("column-voltage")).weighted(10),
            ColumnDef::new("class", CellType::Text, text!("column-class")).weighted(10),
            ColumnDef::new("colour", CellType::Colour, text!("column-colour")).weighted(8),
            ColumnDef::new("cross_section", CellType::Qty, text!("column-mm2")).weighted(8),
            ColumnDef::new("aliases", CellType::Number, text!("column-aliases")).weighted(8),
            ColumnDef::new("members", CellType::Number, text!("column-members")).weighted(8),
            ColumnDef::new("pages", CellType::Folios, text!("column-pages")).weighted(14),
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
                let pages = potential_pages(q, &wires, p, &order);
                (
                    (natural_key(wires.potential_name(p.id)), p.id),
                    potential_row(q, &wires, p, pages),
                )
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        let mut set = RowSet::new(columns);
        set.rows = keyed.into_iter().map(|(_, r)| r).collect();

        // The declared bonds first, in the order they were made (their IDs), then the drawn ones, in the order their
        // wires were drawn: a drawn bond never moves a declared one's number (doc 09 §2.4).
        let mut bonds: Vec<&ew_derive::BondLink> = wires.partition.bonds().iter().collect();
        bonds.sort_by_key(|b| (b.drawn, b.bond));
        for (i, bond) in bonds.into_iter().enumerate() {
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
            put(&set.columns, &mut cells, "kind", words_cell(&text!("cell-bond")));
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

fn potential_pages(q: &Q, wires: &Wires, p: &Potential, order: &ew_derive::PageOrderV) -> Vec<PageRef> {
    let mut pages: BTreeSet<(usize, Id<Page>)> = BTreeSet::new();
    let mut add = |page: Id<Page>| {
        pages.insert((order.position(PageRef::Drawn(page)).unwrap_or(usize::MAX), page));
    };
    for w in &wires.wires {
        if w.wire.potentials.contains(&p.id) {
            for page in &w.wire.pages {
                add(*page);
            }
        }
    }
    for page in ew_derive::bus_line_pages(q, &wires.partition, p.id) {
        add(page);
    }
    pages.into_iter().map(|(_, p)| PageRef::Drawn(p)).collect()
}

/// The members of a potential as folio 909 counts them (doc 20 §7.3; a potential a bus segment carries counts the
/// segment's members): [`ew_derive::potential_member_count`], one reader for the report and the view.
fn member_count(q: &Q, p: &Potential) -> usize {
    ew_derive::potential_member_count(q, p)
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
    // A bus line without a profile value takes colour and size from its cable: "(cable)" (doc 20 §7.3).
    let from_cable = || {
        (!ew_derive::bus_line_cables(q, &wires.partition, p.id).is_empty())
            .then(|| words_cell(&text!("cell-from-cable")))
    };
    let colour_value = match colour {
        Some(c) => colour_cell(Some(&c.value)),
        None => from_cable().unwrap_or_else(|| colour_cell(None)),
    };
    let csa_value = match csa {
        Some(c) => csa_cell(Some(&c.value)),
        None => from_cable().unwrap_or_else(|| csa_cell(None)),
    };
    let kind = attrs.ptype.as_ref().map_or(Cell::Empty, |t| type_name(&t.value));
    let voltage = attrs.voltage.as_ref().map_or(Cell::Empty, |v| {
        let ac_dc = match attrs.env.as_ref().and_then(|e| e.value.ac_dc.as_ref()) {
            Some(ew_derive::ew_schema::AcDc::Ac) => " V AC",
            Some(ew_derive::ew_schema::AcDc::Dc) => " V DC",
            _ => " V",
        };
        Cell::Ident(format!("{}{ac_dc}", v.value))
    });
    let class = attrs
        .circuit_class
        .as_ref()
        .map_or(Cell::Empty, |c| words_cell(&class_name(&c.value)));
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
            colour_value,
            csa_value,
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

/// A potential type: its code as every language writes it, else in words.
fn type_name(t: &PotentialType) -> Cell {
    if let Some(code) = ew_derive::potential_type_code(t) {
        return Cell::Text(MlText::neutral(code));
    }
    words_cell(&match t {
        PotentialType::Signal => text!("cell-potential-signal"),
        PotentialType::Undefined => text!("cell-potential-undefined"),
        _ => text!("cell-other"),
    })
}

/// A circuit class in words.
fn class_name(c: &CircuitClass) -> Text {
    match c {
        CircuitClass::Mains => text!("cell-class-mains"),
        CircuitClass::Control => text!("cell-class-control"),
        CircuitClass::Analogue => text!("cell-class-analogue"),
        CircuitClass::Bus => text!("cell-class-bus"),
        CircuitClass::Pe => text!("cell-class-pe"),
        CircuitClass::Unknown(_) => text!("cell-other"),
    }
}
