// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The DXF for drilling a surface (doc 13 §9.1): typed entities per layer, written in the writer's own file (its
//! version, header, tables, handles and number format), in millimetres with the origin at the surface's datum corner
//! and y up. It knows no record: the caller names the layers (`HOLE_4.5`, `TAP_M5`, `CUT_22.3`, …) and gives the
//! entities in the order they are written.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ew_dl::Destination;
use ew_units::{Nm, Pt2};
use uuid::Uuid;

use super::picture::{PagePictures, Pictures};
use super::text::dxf_text;
use super::{DxfFile, DxfOptions, DxfSheet, Ent, Set, Writer, assemble, mm};

/// One entity of a drilling DXF, in the surface frame (millimetres from the datum corner, y up).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DrillEntity {
    /// A circle of diameter `d` about `c`.
    Circle {
        /// The centre.
        c: Pt2,
        /// The diameter.
        d: Nm,
    },
    /// A polyline through `pts`, closed or not; `bulges` per vertex in parts per 10⁹ (the tangent of a quarter of the
    /// arc's angle to the next vertex; 0 for a straight segment), empty for straight segments only.
    Polyline {
        /// The vertices.
        pts: Vec<Pt2>,
        /// Whether it closes.
        closed: bool,
        /// The bulge of each vertex's segment, ppb.
        bulges: Vec<i64>,
    },
    /// A text of height `h` from `at`.
    Text {
        /// Its insertion point (the start of its baseline).
        at: Pt2,
        /// Its height.
        h: Nm,
        /// The text.
        s: String,
    },
}

/// A drilling DXF: its file name, the extent of the surface (from the origin) and its layers with their entities, in
/// the order they are written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrillSheet {
    /// The file name, `.dxf` included.
    pub name: String,
    /// The surface's width and height from the origin.
    pub extent: (Nm, Nm),
    /// The layers in order, each with its entities in order.
    pub layers: Vec<(String, Vec<DrillEntity>)>,
}

/// A bulge in parts per 10⁹ as DXF writes it.
fn bulge(ppb: i64) -> String {
    let whole = ppb / 1_000_000_000;
    let frac = (ppb % 1_000_000_000).abs();
    let sign = if ppb < 0 && whole == 0 { "-" } else { "" };
    let frac = format!("{frac:09}");
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{frac}")
    }
}

/// The body of one entity, y up as given.
fn entity(e: &DrillEntity, layer: &str) -> Ent {
    match e {
        DrillEntity::Circle { c, d } => Ent::new(
            "CIRCLE",
            layer.to_owned(),
            format!(
                "100\nAcDbCircle\n 10\n{}\n 20\n{}\n 30\n0\n 40\n{}\n",
                mm(c.x.0),
                mm(c.y.0),
                mm(d.0 / 2)
            ),
        ),
        DrillEntity::Polyline { pts, closed, bulges } => {
            let mut body = format!(
                "100\nAcDbPolyline\n 90\n{}\n 70\n{}\n 43\n0\n",
                pts.len(),
                u8::from(*closed)
            );
            for (i, p) in pts.iter().enumerate() {
                let _ = write!(body, " 10\n{}\n 20\n{}\n", mm(p.x.0), mm(p.y.0));
                if let Some(b) = bulges.get(i).filter(|b| **b != 0) {
                    let _ = write!(body, " 42\n{}\n", bulge(*b));
                }
            }
            Ent::new("LWPOLYLINE", layer.to_owned(), body)
        }
        DrillEntity::Text { at, h, s } => Ent::new(
            "TEXT",
            layer.to_owned(),
            format!(
                "100\nAcDbText\n 10\n{}\n 20\n{}\n 30\n0\n 40\n{}\n  1\n{}\n  7\nStandard\n100\nAcDbText\n",
                mm(at.x.0),
                mm(at.y.0),
                mm(h.0),
                dxf_text(s)
            ),
        ),
    }
}

/// The DXF of `d` (the module documentation). The same sheet gives the same bytes.
pub fn drilling_dxf(d: &DrillSheet) -> DxfFile {
    let opts = DxfOptions::default();
    let set = Set {
        links: BTreeMap::new(),
        names: BTreeMap::new(),
    };
    let sheet = DxfSheet {
        label: String::new(),
        width: d.extent.0,
        height: d.extent.1,
        destination: Destination::Page(Uuid::nil()),
        layers: Vec::new(),
        geometry: BTreeMap::new(),
        scale_ppb: 1_000_000_000,
        attributes: BTreeMap::new(),
    };
    let mut pictures = Pictures::default();
    let mut w = Writer {
        opts: &opts,
        set: &set,
        sheet: &sheet,
        pictures: &mut pictures,
        height: d.extent.1,
        next: 0x20,
        dashes: Vec::new(),
        faces: Vec::new(),
        skipped_images: 0,
        block_texts: BTreeMap::new(),
        attdefs: BTreeMap::new(),
        inserted: BTreeSet::new(),
        page_pictures: PagePictures::default(),
    };
    let model: Vec<Ent> = d
        .layers
        .iter()
        .flat_map(|(layer, ents)| ents.iter().map(move |e| entity(e, layer)))
        .collect();
    let layers = d.layers.iter().map(|(name, _)| name.clone()).collect();
    let text = assemble(&mut w, model, Vec::new(), layers, d.extent);
    DxfFile {
        name: d.name.clone(),
        text,
        skipped_images: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bulge_is_written_without_trailing_zeros() {
        assert_eq!(bulge(414_213_562), "0.414213562");
        assert_eq!(bulge(-500_000_000), "-0.5");
        assert_eq!(bulge(1_000_000_000), "1");
    }
}
