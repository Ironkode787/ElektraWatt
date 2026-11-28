// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `hole_table`: every hole of every surface of the project, surface by surface (doc 13 §9.2, FR-MFG-029).

use ew_derive::layout::{Owner, SurfaceFeatures, carrier_labels, face_of, from_datum, surface_datum};
use ew_derive::{DrawnPageOrder, PageRef, Q};
use ew_l10n::{List, Text, TextArg};
use ew_schema::{Carrier, MountingSurface, Placement, ReportBlock, ReportKind, SurfaceFeature};

use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowKind, RowSet};

/// The hole table: for each surface a layout page shows, in the order of its first such page, a heading row that says
/// where its numbers are measured from ("Door, page 904, seen from outside, measured from the bottom-left corner"),
/// then its holes in their numbers' order (`ew_derive::layout::SurfaceFeatures`): the number, X and Y from the datum
/// to the 0.1 mm (a whole millimetre without its ".0"), how it is made, its kind and size, and its owners. A surface
/// without holes is not listed. The Surface column leads the CSV and the XLSX; the page prints the others.
#[derive(Clone, Copy, Debug)]
pub struct HoleTable;

impl ReportKindDef for HoleTable {
    fn kind(&self) -> ReportKind {
        ReportKind::HoleTable
    }

    fn workshop(&self) -> bool {
        true
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("surface", CellType::Text, text!("column-surface")).weighted(20),
            ColumnDef::new("no", CellType::Ident, text!("column-no")).weighted(8),
            ColumnDef::new("x", CellType::Qty, text!("column-x-mm")).weighted(10),
            ColumnDef::new("y", CellType::Qty, text!("column-y-mm")).weighted(10),
            ColumnDef::new("hole", CellType::Text, text!("column-hole")).weighted(20),
            ColumnDef::new("what", CellType::Text, text!("column-what")).weighted(22),
            ColumnDef::new("for", CellType::Text, text!("column-for")).weighted(30),
        ]
    }

    fn printed(&self) -> Option<&'static [&'static str]> {
        Some(&["no", "x", "y", "hole", "what", "for"])
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for (surface, folio) in surfaces_in_page_order(q) {
            surface_rows(q, surface, &folio, &mut set);
        }
        set
    }
}

/// Each surface a layout page shows, in the order of its first such page, with that page's folio: the order of the
/// hole table and of the assembly list's rails.
pub(super) fn surfaces_in_page_order(q: &Q) -> Vec<(ew_ids::Id<MountingSurface>, String)> {
    let order = q.get::<DrawnPageOrder>(());
    let mut listed: Vec<(ew_ids::Id<MountingSurface>, String)> = Vec::new();
    for entry in &order.entries {
        let PageRef::Drawn(page) = entry.page else {
            continue;
        };
        if let Some(surface) = ew_derive::layout::page_surface(q, page)
            && !listed.iter().any(|(s, _)| *s == surface)
        {
            listed.push((surface, entry.folio.clone()));
        }
    }
    listed
}

/// The heading and the hole rows of `surface`, whose first layout page is `folio`.
fn surface_rows(q: &Q, surface: ew_ids::Id<MountingSurface>, folio: &str, set: &mut RowSet) {
    let features = q.get::<SurfaceFeatures>(surface);
    let (Some(s), Some(face)) = (q.table::<MountingSurface>().get(surface), face_of(q, surface)) else {
        return;
    };
    if features.is_empty() {
        return;
    }
    let word = ew_derive::text::surface_word(&face.kind, face.door, face.doors);
    let words = |t: &Text| Cell::Text(ew_derive::text::in_languages(t));
    let heading = ew_derive::text::hole_table_heading(
        word.clone(),
        face.size,
        folio,
        ew_derive::text::surface_seen(&face.kind),
        &surface_datum(q, s),
    );
    let mut cells = vec![Cell::Empty; set.columns.len()];
    // Its size with the decimal sign the list prints, as the rows' numbers.
    cells[0] = super::conn::measured_cell(q, &heading);
    set.rows.push(Row {
        id: RowId::entity(surface).with(RowKey::Name("heading".to_owned())),
        kind: RowKind::Heading,
        cells,
        links: Vec::new(),
        sources: vec![surface.cast()],
    });
    let labels: std::collections::BTreeMap<_, _> = carrier_labels(q, s.id).into_iter().collect();
    for f in features.iter() {
        let (x, y) = from_datum(q, s, (i128::from(f.at.x.0), i128::from(f.at.y.0)));
        let owners: Vec<TextArg> = f
            .owners
            .iter()
            .filter_map(|o| owner_words(q, o, &labels, &f.hole))
            .collect();
        let between = || Text::bare(TextArg::Plain(", ".to_owned()));
        let mut r = Row::data(
            RowId::entity(surface).with(RowKey::Name(format!("H{}", f.no))),
            vec![
                words(&word),
                Cell::Ident(format!("H{}", f.no)),
                tenth(x),
                tenth(y),
                Cell::Text(crate::words::in_languages_measured(
                    q,
                    &ew_derive::text::made(&f.made),
                )),
                words(&ew_derive::text::hole_what(&f.hole)),
                words(&Text::bare(List::new(owners, between(), between()))),
            ],
        );
        r.sources = std::iter::once(surface.cast())
            .chain(f.owners.iter().map(|o| match o {
                Owner::Placement { p, .. } => p.cast(),
                Owner::Carrier { c, .. } => c.cast(),
                Owner::ByHand { feature } => feature.cast(),
            }))
            .collect();
        set.rows.push(r);
    }
}

/// A coordinate in millimetres to the tenth (`113.5`, `120`) as a number: the heading carries the unit, a page prints
/// the decimal sign of its language and an Excel file a number.
fn tenth(nm: i128) -> Cell {
    let tenths = ew_units::arith::div_round_ties_away(nm, 100_000);
    Cell::Qty(
        ew_units::Dec3(i64::try_from(tenths.saturating_mul(100)).unwrap_or(i64::MAX)),
        crate::rowset::QtyUnit::None,
    )
}

/// An owner of a hole in words (doc 12-3d §10.4): a device by its tag, a rail or a duct by its label, a hole by hand
/// by its purpose, a gland by its cable, else by its kind ("Gland M20").
pub fn owner_words(
    q: &Q,
    owner: &Owner,
    labels: &std::collections::BTreeMap<ew_ids::Id<Carrier>, ew_derive::layout::CarrierLabel>,
    hole: &ew_schema::Hole,
) -> Option<TextArg> {
    Some(match owner {
        Owner::Placement { p, .. } => {
            let device = q.table::<Placement>().get(*p)?.device;
            TextArg::Plain(ew_derive::short_tag(q, device))
        }
        Owner::Carrier { c, .. } => TextArg::Text(ew_derive::text::carrier_label(*labels.get(c)?)),
        Owner::ByHand { feature } => {
            let f = q.table::<SurfaceFeature>().get(*feature)?;
            match (f.cable, f.purpose.is_empty()) {
                (Some(cable), _) => TextArg::Plain(ew_derive::short_tag(q, cable)),
                (None, false) => TextArg::Ml(f.purpose.clone()),
                (None, true) => TextArg::Text(ew_derive::text::hole_what(hole)),
            }
        }
    })
}
