// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared builders for the symbol lowering tests.

#![allow(dead_code)]

use ew_dl::{Item, ItemKind, Transform};
use ew_ids::{Id, Uuid};
use ew_mltext::MlText;
use ew_schema::{
    Align, AnchorPolicy, Colour, Dash, Dir4, Discipline, Extra, ItemMeta, LibRef, Pen, Primitive, RepType,
    SlotKind, Stroke, SymbolCp, SymbolDef, TextSlot,
};
use ew_units::{Angle2, Grid, Nm, Pt2};

/// The QET 10 px grid position `k`.
pub fn at(k: i64) -> Nm {
    Grid::QET_10PX.at(k).expect("on the grid")
}

/// A point from nm.
pub fn pt(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x), Nm(y))
}

/// A point at QET grid indices.
pub fn gp(kx: i64, ky: i64) -> Pt2 {
    Pt2::new(at(kx), at(ky))
}

/// A solid 0.26 mm stroke by layer colour.
pub fn stroke() -> Stroke {
    Stroke {
        pen: Pen::P026,
        dash: Dash::Solid,
        colour: Colour::ByLayer,
        extra: Extra::new(),
    }
}

/// A symbol with the given parts and defaults everywhere else.
pub fn symbol(graphics: Vec<Primitive>, cps: Vec<SymbolCp>, text_slots: Vec<TextSlot>) -> SymbolDef {
    SymbolDef {
        meta: ItemMeta::default(),
        name: MlText::default(),
        keywords: Default::default(),
        rep_type: RepType::MultiLine,
        fndef: LibRef {
            library: Id::from_uuid(Uuid::nil()),
            item: Id::from_uuid(Uuid::nil()),
            version_req: "^1".into(),
            extra: Extra::new(),
        },
        grid: Grid::QET_10PX,
        graphics,
        cps: ew_schema::KeyedList::from_vec(cps).expect("distinct CP indexes"),
        text_slots,
        arrangements: vec![],
        anchor: AnchorPolicy::Origin,
        link_points: vec![],
        extra_variants: vec![],
        iec60617_ref: None,
        iso1219_ref: None,
        extra: Extra::new(),
    }
}

/// A connection point.
pub fn cp(idx: u16, fn_cp: u16, pos: Pt2, dir: Dir4) -> SymbolCp {
    SymbolCp {
        idx,
        fn_cp,
        pos,
        dir,
        discipline: Discipline::Electrical,
        extra: Extra::new(),
    }
}

/// A visible text slot without width limit.
pub fn slot(kind: SlotKind, pos: Pt2, rot: Angle2, align: Align) -> TextSlot {
    TextSlot {
        kind,
        pos,
        rot,
        align,
        visible: true,
        max_width: None,
        style: None,
        extra: Extra::new(),
    }
}

/// The points of an item: path points, or those of grouped items under the group's transform.
pub fn item_points(item: &Item) -> Vec<Pt2> {
    match &item.kind {
        ItemKind::Path(p) => p.path.points().collect(),
        ItemKind::Group(g) => {
            let t = g.transform.unwrap_or(Transform::IDENTITY);
            g.items.iter().flat_map(item_points).map(|p| t.apply(p)).collect()
        }
        _ => Vec::new(),
    }
}
