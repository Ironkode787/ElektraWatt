// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reads the editing tools of the canvas prepare once per (revision, folio) or per symbol copy:
//! what can be placed on a page ([`Session::place_catalogue`]), the `library.embed` parameters of a
//! starter symbol ([`Session::starter_embed`]), a symbol copy's shape ([`Session::symbol_shape`]) and the records
//! drawn on a page as tables ([`Session::page_tables`]). A tool never calls the `Session`: the shell's tool host
//! fetches these and hands them to the tools, which compute with them and with the pure functions of
//! [`crate::geometry`].
//!
//! **Coordinates.** Page nanometres, y down (the tools convert with the page's page-to-sheet map). A route is the
//! full vertex list from end `a` to end `b`.
//!
//! **Cost.** Each read is a pass over tracked reads and memoised families of the session's `Q`; the shell's
//! page-content cache fetches the page tables once per (revision, folio) and a shape once per symbol copy per
//! revision (NFR-PERF-20).

use ew_derive::{PhysicalWiresOf, PotentialPartition, WireNumbers, cp_label};
use ew_ids::{Id, Uuid};
use ew_model::EndpointKey;
use ew_schema::Junction;

use crate::Session;

mod catalogue;
mod embeds;
mod shape;
mod supply;
mod tables;
pub(crate) use catalogue::catalogue_plc;
pub(super) use catalogue::starter_fndef;
pub use catalogue::{
    CatalogueAccessory, CatalogueInterface, CataloguePart, CataloguePlc, CatalogueSymbol,
    CatalogueTemplateFn, PlaceCatalogue, PlacedBy, PlcGroup, address_copy, bus_port_symbol_key,
    ip_direction_word, page_type_takes, part_interfaces,
};
pub use catalogue::starter_piece_symbol;
pub(crate) use catalogue::{starter_fndef_by_key, starter_piece_fndefs, starter_symbol_name};
pub(crate) use shape::dir_of;
pub use shape::{ShapeCp, SymbolShape, exchange_map};
pub use supply::SupplyPoint;
pub use tables::{
    PageTables, SlotCell, TableBox, TableConnection, TableCp, TableEnd, TableItem, TableJunction,
    TablePotential, TableRep, TableStructureBox,
};

/// The flattening tolerance of a symbol outline, nm.
pub const OUTLINE_TOLERANCE_NM: f64 = 50_000.0;

// -------------------------------------------------------------------------------------------------------------------
// The physical wires through a junction

/// One physical wire through a junction, as the junction sequence editor lists it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionWire {
    /// The printed wire number (`W13`, `102.5`), `?` while it has none.
    pub label: String,
    /// The CP label of the wire's first end (`-102G1:+#1`).
    pub from: String,
    /// The CP label of its second end.
    pub to: String,
}

impl Session {
    /// The physical wires through `junction` (`physical_wires_of` of its potential, the wires whose path passes the
    /// junction, doc 06 §5.5), by printed wire number, then the ends' labels, the ID last: the printed number
    /// and the CP labels of both ends. Empty for an unknown
    /// junction or one on no potential. Reads memoised families only.
    pub fn junction_wires(&self, junction: Uuid) -> Vec<JunctionWire> {
        let q = self.query();
        let j: Id<Junction> = Id::from_uuid(junction);
        if q.junctions().get(j).is_none() {
            return Vec::new();
        }
        let partition = q.get::<PotentialPartition>(());
        let Some(potential) = partition.potential_at(EndpointKey::Junction(j)).map(|p| p.id) else {
            return Vec::new();
        };
        let wires = q.get::<PhysicalWiresOf>(potential);
        let numbers = q.get::<WireNumbers>(());
        let mut out: Vec<(JunctionWire, Id<ew_schema::Connection>)> = wires
            .wires
            .iter()
            .filter(|w| w.junctions.contains(&j))
            .map(|w| {
                let wire = JunctionWire {
                    label: numbers
                        .number(w.id)
                        .map_or_else(|| "?".to_owned(), |n| n.label.clone()),
                    from: cp_label(&q, w.ends[0].cp),
                    to: cp_label(&q, w.ends[1].cp),
                };
                (wire, w.id)
            })
            .collect();
        out.sort_by(|(a, ia), (b, ib)| junction_wire_order(a, b).then_with(|| ia.cmp(ib)));
        out.into_iter().map(|(w, _)| w).collect()
    }
}

/// The order of the junction editor's wires: the printed wire number (natural order, then its bytes), then
/// the first end's label, then the second's; a wire without a number (`?`) after every numbered one (as
/// `number_order` in `output/properties.rs` puts an unnumbered wire last); the caller breaks a remaining tie by wire
/// ID. Never the carrier ID alone: a split mints its new half and the new wire in one transaction, in no visible order.
pub(crate) fn junction_wire_order(a: &JunctionWire, b: &JunctionWire) -> std::cmp::Ordering {
    let text = |x: &str, y: &str| ew_mltext::natural_cmp(x, y).then_with(|| x.cmp(y));
    let unnumbered = |w: &JunctionWire| w.label == "?";
    unnumbered(a)
        .cmp(&unnumbered(b))
        .then_with(|| text(&a.label, &b.label))
        .then_with(|| text(&a.from, &b.from))
        .then_with(|| text(&a.to, &b.to))
}

#[cfg(test)]
mod tests {
    use ew_dl::PathEl;
    use ew_units::{Nm, Pt2};

    use super::shape::{ExchangeMap, flatten};
    use super::*;
    use crate::geometry::Dir;

    /// A shape drawing FnDef `def` with CPs `(symbol CP, FnDef CP, designation)`.
    fn shape(def: u128, cps: &[(u16, u16, &str)]) -> SymbolShape {
        SymbolShape {
            copy: Uuid::from_u128(def + 100),
            key: String::new(),
            cps: cps
                .iter()
                .map(|&(symbol_cp, fn_cp, d)| ShapeCp {
                    symbol_cp,
                    fn_cp,
                    at: (0, 0),
                    dir: Dir::Up,
                    designation: d.to_owned(),
                    fluid: false,
                    valve_port: None,
                })
                .collect(),
            body: [0; 4],
            outline: Vec::new(),
            rep_type: String::new(),
            fndef: Uuid::from_u128(def),
            valve: false,
            contact: false,
            fitting: false,
        }
    }

    #[test]
    fn exchange_map_pairs_the_cps_of_the_two_symbols() {
        // Same FnDef, by index: the function draws CPs 0, 1 and 2; the new symbol draws 0 and 2.
        let current = shape(1, &[(0, 0, "13"), (1, 1, "14"), (2, 2, "21")]);
        let new = shape(1, &[(0, 2, "21"), (1, 0, "13")]);
        assert_eq!(
            exchange_map(&current, &new),
            ExchangeMap {
                drawn: vec![0, 2],
                unmapped: vec![1],
                unmatched: None,
            },
            "the same definition"
        );
        // Another FnDef, by designation without regard to case: its CP "a1" draws function CP 0 ("A1"); its CP 1
        // ("X9", FnDef CP 7) has no counterpart and its default names no function CP, so it draws nothing;
        // function CP 1 ("A2") is unmapped.
        let current = shape(1, &[(0, 0, "A1"), (1, 1, "A2")]);
        let new = shape(2, &[(0, 3, "a1"), (1, 7, "X9")]);
        assert_eq!(
            exchange_map(&current, &new),
            ExchangeMap {
                drawn: vec![0],
                unmapped: vec![1],
                unmatched: None,
            },
            "by designation"
        );
        // A CP whose FnDef is not embedded (no designation) has no counterpart either.
        let new = shape(2, &[(0, 3, "A1"), (1, 8, "")]);
        assert_eq!(exchange_map(&current, &new).unmapped, vec![1]);
        // The new symbol's CP 1 matches nothing by designation and its default FnDef CP 1 names function CP 1.
        let current = shape(1, &[(0, 0, "1"), (1, 1, "2")]);
        let new = shape(2, &[(0, 0, "X1"), (1, 1, "X2")]);
        assert_eq!(
            exchange_map(&current, &new),
            ExchangeMap {
                drawn: vec![],
                unmapped: vec![],
                unmatched: Some((0, 0)),
            },
            "the unmatched CP the command refuses"
        );
    }

    #[test]
    fn a_cubic_quarter_circle_flattens_within_the_tolerance() {
        let r = 10_000_000_i64;
        let h = ew_dl::kappa_handle(Nm(r)).0;
        let p = |x, y| Pt2::new(Nm(x), Nm(y));
        let els = [
            PathEl::MoveTo(p(r, 0)),
            PathEl::CubicTo(p(r, h), p(h, r), p(0, r)),
        ];
        let mut out = Vec::new();
        flatten(&els, OUTLINE_TOLERANCE_NM, &mut out);
        assert_eq!(out.len(), 1);
        let line = &out[0];
        assert_eq!(line.first(), Some(&(r, 0)));
        assert_eq!(line.last(), Some(&(0, r)));
        assert!(line.len() > 3, "{}", line.len());
        // Every chord's midpoint lies within the tolerance of the circle (plus κ's own error, 0.03 %).
        for w in line.windows(2) {
            let mx = (w[0].0 + w[1].0) as f64 / 2.0;
            let my = (w[0].1 + w[1].1) as f64 / 2.0;
            let d = r as f64 - (mx * mx + my * my).sqrt();
            assert!(d.abs() <= OUTLINE_TOLERANCE_NM + 0.0003 * r as f64, "{d}");
        }
        // A closed square ends at its first point.
        let mut out = Vec::new();
        flatten(
            &[
                PathEl::MoveTo(p(0, 0)),
                PathEl::LineTo(p(5, 0)),
                PathEl::LineTo(p(5, 5)),
                PathEl::ClosePath,
            ],
            OUTLINE_TOLERANCE_NM,
            &mut out,
        );
        assert_eq!(out, vec![vec![(0, 0), (5, 0), (5, 5), (0, 0)]]);
    }

    /// A wire without a printed number (`?`) sorts after every numbered wire; between two `?` wires the ends'
    /// labels decide as before.
    #[test]
    fn unnumbered_wires_sort_last_in_junction_wires() {
        let wire = |label: &str, from: &str, to: &str| JunctionWire {
            label: label.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
        };
        let mut wires = [
            wire("W20", "-102G1:+#1", "-102F2:+#2"),
            wire("?", "-102G1:+#3", "-102F2:+#4"),
            wire("W3", "-102G1:+#5", "-102F2:+#6"),
        ];
        wires.sort_by(junction_wire_order);
        let labels: Vec<&str> = wires.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(labels, ["W3", "W20", "?"]);
        // Two unnumbered wires keep the end-label order.
        let mut unnumbered = [
            wire("?", "-102G1:+#2", "-102F2:+#2"),
            wire("?", "-102G1:+#1", "-102F2:+#1"),
        ];
        unnumbered.sort_by(junction_wire_order);
        assert_eq!(unnumbered[0].from, "-102G1:+#1");
    }
}
