// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's geometry for the schematic router and its layout measures in words (WP-S2.19 part 1, decision log
//! ID-1498): [`Session::page_geometry`] builds `ew_derive::routing`'s [`PageGeometry`] from the page tables
//! ([`Session::page_tables`]), the labels layer's text boxes and the frame; [`Session::layout_report`] measures one
//! page or every schematic page and names what it finds the way the page prints it, for `ew layout`.
//!
//! - **Bodies:** each representation's body (`TableRep::body`) and a black box's outline by its bounds, as the Wire
//!   tool's obstacles without their growth (`ew-viewmodel`'s `rep_rects`).
//! - **Text boxes:** the labels layer's entity rectangles that are not a point of the page's representations, free
//!   texts and structure boxes: every line of a tag, a slot or pin text and a cross-reference text the scene prints
//!   for a representation, a free text's lines and a structure box's caption (ID-1502 (3)), in page coordinates. A
//!   wire marker's text moves with its wire and is left out.
//! - **Frame bands:** the frame's outer rectangle outside its drawing area (`ew_scene::forms::FrameGeometry`), up to
//!   four bands, as the Wire tool's `frame_rects` without its inset.
//! - **Connections:** each connection's drawn polyline (`TableConnection::polyline`, the scene's), ends included.

use std::collections::BTreeSet;
use std::fmt;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::PotentialType;
use ew_derive::ew_schema::{Dir4, PageType};
use ew_derive::ew_units::{Nm, Pt2};
use ew_derive::routing::{
    self, Body, CpPoint, DrawnConnection, JunctionPoint, Measures, PageGeometry, TextBox, TouchedPoint,
};
use ew_derive::routing::{Point, SUPPLY_ORDER};
use ew_derive::{CellOf, CpRef, DisplayedTag, PageOrder, PageRef, PotentialFlow, PotentialPartition, Q};
use ew_dl::Layer;
use ew_scene::forms::FrameGeometry;
use ew_scene::page::DEFAULT_SHEET;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::editing::{PageTables, TableCp, TableEnd};
use super::layers::page_layer;
use super::view::PageFrameOf;
use crate::Session;
use crate::geometry::Dir;

/// Why a layout report is refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LayoutError {
    /// No page of the page order has this folio.
    NoFolio(String),
    /// The folio names a page that is not drawn (a section divider or a generated report page).
    NotDrawn(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::NoFolio(folio) => write!(f, "the page order has no folio `{folio}`"),
            LayoutError::NotDrawn(folio) => write!(
                f,
                "folio `{folio}` is not a drawn page: a section divider or a generated page has no wires"
            ),
        }
    }
}

impl std::error::Error for LayoutError {}

/// A point in page nm, `[x, y]`.
pub type LayoutPoint = [i64; 2];

/// The layout measures of one page in words (`ew_derive::routing`'s module documentation defines them).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageLayout {
    /// The printed folio label.
    pub folio: String,
    /// (a) Hard rule: the connections touching a connection point or a junction they do not end at.
    pub touches: Vec<LayoutTouch>,
    /// (b) Hard rule: the pairs of connections running along each other.
    pub overlaps: Vec<LayoutPair>,
    /// (c) Hard rule: the connections through a symbol's body (or a black box), but along their own ends' escapes.
    pub through_symbols: Vec<LayoutThrough>,
    /// (d) Hard rule: the connections through a symbol's text.
    pub through_texts: Vec<LayoutThrough>,
    /// (g) Hard rule: the junctions with four or more arms.
    pub four_way_junctions: Vec<LayoutJunction>,
    /// (e) Cost: the points where two connections meet.
    pub crossings: Vec<LayoutCrossing>,
    /// (f) Cost: the connections with bends.
    pub bends: Vec<LayoutBends>,
}

impl PageLayout {
    /// The items that break a hard rule: (a) to (d) and (g).
    pub fn hard(&self) -> usize {
        self.touches.len()
            + self.overlaps.len()
            + self.through_symbols.len()
            + self.through_texts.len()
            + self.four_way_junctions.len()
    }

    /// The bends of every connection together.
    pub fn bend_total(&self) -> usize {
        self.bends.iter().map(|b| b.bends).sum()
    }
}

/// (a) A connection touching a point it does not end at.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutTouch {
    /// The connection, by its ends (`-E55:PE → -X1:PE`).
    pub connection: String,
    /// The connection point's label, or `the junction` (its cell names it).
    pub point: String,
    /// Where, in page nm.
    pub at: LayoutPoint,
    /// The frame cell there (`C4`); empty off the drawing area or without a frame grid.
    pub cell: String,
}

/// (b) Two connections running along each other.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutPair {
    /// The first connection, by its ends.
    pub connection: String,
    /// The other connection, by its ends.
    pub other: String,
    /// The stretches they share, each from its smaller end to its larger, in page nm.
    pub stretches: Vec<[LayoutPoint; 2]>,
    /// The frame cell at the first stretch's start.
    pub cell: String,
}

/// (c) or (d): a connection through a symbol's body or text.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutThrough {
    /// The connection, by its ends.
    pub connection: String,
    /// The symbol: its device's tag as the page prints it, `interruption point <name>`, else its symbol's key.
    pub symbol: String,
    /// The stretches inside, in the connection's direction, in page nm.
    pub stretches: Vec<[LayoutPoint; 2]>,
    /// The frame cell at the first stretch's start.
    pub cell: String,
}

/// (e) Two connections meeting at a point.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutCrossing {
    /// The first connection, by its ends.
    pub connection: String,
    /// The other connection, by its ends.
    pub other: String,
    /// Where, in page nm.
    pub at: LayoutPoint,
    /// The frame cell there.
    pub cell: String,
}

/// (f) A connection's bends.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutBends {
    /// The connection, by its ends.
    pub connection: String,
    /// Its bends, at least one.
    pub bends: usize,
}

/// (g) A junction with four or more arms.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutJunction {
    /// The junction's arms, at least four.
    pub arms: usize,
    /// Where, in page nm.
    pub at: LayoutPoint,
    /// The frame cell there.
    pub cell: String,
}

/// The direction of a [`Dir`] in the model's spelling.
fn dir4(d: Dir) -> Dir4 {
    match d {
        Dir::Up => Dir4::Up,
        Dir::Down => Dir4::Down,
        Dir::Left => Dir4::Left,
        Dir::Right => Dir4::Right,
    }
}

/// The page-order entry of `folio`.
fn entry_of(q: &Q, folio: &str) -> Result<PageRef, LayoutError> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .map(|e| e.page)
        .ok_or_else(|| LayoutError::NoFolio(folio.to_owned()))
}

/// The geometry of a page from its tables (see the module documentation).
fn geometry(q: &Q, page: PageRef, tables: &PageTables) -> PageGeometry {
    let mut bodies = Vec::new();
    for r in &tables.reps {
        bodies.push(Body {
            rep: r.id,
            rect: r.body,
        });
        if let Some(b) = &r.black_box {
            let xs = b.outline.iter().map(|p| p.0);
            let ys = b.outline.iter().map(|p| p.1);
            if let (Some(x0), Some(x1), Some(y0), Some(y1)) =
                (xs.clone().min(), xs.max(), ys.clone().min(), ys.max())
            {
                bodies.push(Body {
                    rep: r.id,
                    rect: [x0, y0, x1, y1],
                });
            }
        }
    }
    // The texts the labels layer draws for the page's representations, free texts and structure boxes (their
    // captions); wire markers move with their wire and are left out (ID-1502 (3)).
    let reps: BTreeSet<Uuid> = tables
        .reps
        .iter()
        .map(|r| r.id)
        .chain(
            tables
                .items
                .iter()
                .filter(|i| i.kind == "free_text")
                .map(|i| i.id),
        )
        .chain(tables.structure_boxes.iter().map(|b| b.id))
        .collect();
    // A cross-reference's link area repeats its text's box: each box once.
    let mut seen: BTreeSet<(Uuid, [i64; 4])> = BTreeSet::new();
    let texts = page_layer(q, page, Layer::Labels)
        .list
        .entity_bounds()
        .into_iter()
        .filter(|(e, _)| reps.contains(&e.0))
        .filter_map(|(e, r)| {
            let (a, b) = (r.min(), r.max());
            let rect = [a.x.0, a.y.0, b.x.0, b.y.0];
            (a.x != b.x && a.y != b.y && seen.insert((e.0, rect))).then_some(TextBox { rep: e.0, rect })
        })
        .collect();
    let frame = q.get::<PageFrameOf>(page);
    let frame_geometry = frame
        .as_ref()
        .as_ref()
        .and_then(|f| FrameGeometry::of(&f.form, f.title_block.as_deref()));
    let rect = |r: ew_derive::ew_units::Rect| [r.min().x.0, r.min().y.0, r.max().x.0, r.max().y.0];
    let sheet = match (&frame_geometry, frame.as_ref().as_ref()) {
        (Some(g), _) => rect(g.outer),
        (None, Some(f)) => [0, 0, f.form.sheet.width.0, f.form.sheet.height.0],
        (None, None) => [0, 0, DEFAULT_SHEET.0.0, DEFAULT_SHEET.1.0],
    };
    let drawing_area = frame_geometry.as_ref().map(|g| rect(g.drawing_area));
    let frame_bands = frame_geometry
        .as_ref()
        .map(|g| {
            let (o0, o1, d0, d1) = (
                g.outer.min(),
                g.outer.max(),
                g.drawing_area.min(),
                g.drawing_area.max(),
            );
            let (ox0, oy0, ox1, oy1) = (o0.x.0, o0.y.0, o1.x.0, o1.y.0);
            let (dx0, dy0, dx1, dy1) = (d0.x.0, d0.y.0, d1.x.0, d1.y.0);
            [
                [ox0, oy0, ox1, dy0],
                [ox0, dy1, ox1, oy1],
                [ox0, dy0, dx0, dy1],
                [dx1, dy0, ox1, dy1],
            ]
            .into_iter()
            .filter(|r| r[0] < r[2] && r[1] < r[3])
            .collect()
        })
        .unwrap_or_default();
    let junctions = tables
        .junctions
        .iter()
        .map(|j| JunctionPoint {
            id: j.id,
            at: j.at,
            connections: tables
                .connections
                .iter()
                .filter(|c| [c.a, c.b].contains(&TableEnd::Junction { id: j.id }))
                .map(|c| c.id)
                .collect(),
        })
        .collect();
    // The supply type of each connection's potential, by the potential of an end CP (§5.3).
    let partition = q.get::<PotentialPartition>(());
    let supply = |c: &super::editing::TableConnection| -> Option<u8> {
        [c.a, c.b].into_iter().find_map(|e| {
            let TableEnd::Cp { function, idx } = e else {
                return None;
            };
            let p = partition.potential_at(CpRef::new(Id::from_uuid(function), idx).key())?;
            let flow = q.get::<PotentialFlow>(p.id);
            let ptype = flow.as_ref().as_ref()?.ptype.as_ref()?.value.clone();
            let word = match ptype {
                PotentialType::L1 => "L1",
                PotentialType::L2 => "L2",
                PotentialType::L3 => "L3",
                PotentialType::L => "L",
                PotentialType::N => "N",
                PotentialType::Pen => "PEN",
                PotentialType::Pe => "PE",
                PotentialType::Fe => "FE",
                PotentialType::LPlus => "L+",
                PotentialType::M => "M",
                PotentialType::LMinus => "L-",
                _ => return None,
            };
            SUPPLY_ORDER
                .iter()
                .position(|w| *w == word)
                .and_then(|k| u8::try_from(k).ok())
        })
    };
    let end_id = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|cp| cp.function == *function && cp.idx == *idx)
            .map_or(Uuid::nil(), |cp| cp.id),
        TableEnd::Junction { id } => *id,
    };
    PageGeometry {
        grid: tables.grid,
        sheet,
        drawing_area,
        bodies,
        cps: tables
            .cps
            .iter()
            .map(|cp| CpPoint {
                id: cp.id,
                at: cp.at,
                dir: cp.dir.map(dir4),
                rep: cp.rep,
                connections: cp.connections.clone(),
            })
            .collect(),
        texts,
        junctions,
        frame_bands,
        connections: tables
            .connections
            .iter()
            .map(|c| DrawnConnection {
                id: c.id,
                ends: [end_id(&c.a), end_id(&c.b)],
                routed: c.stored_route,
                supply: supply(c),
                polyline: c.polyline.clone(),
            })
            .collect(),
    }
}

/// The frame cell of the point `p` on `page` as printed (`C4`); empty off the drawing area or without a frame grid.
fn cell_text(q: &Q, page: Id<ew_derive::ew_schema::Page>, p: (i64, i64)) -> String {
    q.get::<CellOf>((page, Pt2::new(Nm(p.0), Nm(p.1))))
        .as_ref()
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// A connection of `tables` named by its ends (`-E55:PE → -X1:PE`, `the junction in C11 → E56:X2`); empty for one
/// the page does not draw.
fn connection_text(q: &Q, tables: &PageTables, id: Uuid) -> String {
    let page: Id<ew_derive::ew_schema::Page> = Id::from_uuid(tables.page);
    let end = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|cp| cp.function == *function && cp.idx == *idx)
            .map_or_else(
                || "a connection point of another page".to_owned(),
                |cp| cp_text(q, cp),
            ),
        TableEnd::Junction { id } => match tables.junctions.iter().find(|j| j.id == *id) {
            Some(j) => match cell_text(q, page, j.at) {
                c if c.is_empty() => "a junction".to_owned(),
                c => format!("the junction in {c}"),
            },
            None => "a junction".to_owned(),
        },
    };
    tables
        .connections
        .iter()
        .find(|c| c.id == id)
        .map(|c| format!("{} → {}", end(&c.a), end(&c.b)))
        .unwrap_or_default()
}

/// The owner `id` of a body or a text box of `tables` in words: a representation's device tag as the page prints it,
/// `interruption point <name>`, else its symbol's key; a free text or a structure box by its text.
fn symbol_text(q: &Q, tables: &PageTables, id: Uuid) -> String {
    let page: Id<ew_derive::ew_schema::Page> = Id::from_uuid(tables.page);
    let Some(r) = tables.reps.iter().find(|r| r.id == id) else {
        // A free text or a structure box's caption (ID-1502 (3)).
        if let Some(t) = tables.items.iter().find(|i| i.id == id) {
            return format!("the free text \"{}\"", t.text.clone().unwrap_or_default());
        }
        return tables
            .structure_boxes
            .iter()
            .find(|b| b.id == id)
            .map(|b| format!("the structure box \"{}\"", b.caption))
            .unwrap_or_default();
    };
    if let Some(ip) = &r.ip {
        return format!("interruption point {}", ip.name);
    }
    match r.device {
        Some(d) => signed(
            q,
            Some(Id::from_uuid(d)),
            q.get::<DisplayedTag>((Id::from_uuid(d), page)).as_ref(),
        ),
        None => r.symbol_key.clone(),
    }
}

/// A connection point of the page in the layout words: its label with its device's tag signed ([`signed`]).
fn cp_text(q: &Q, cp: &TableCp) -> String {
    let device = q
        .functions()
        .get(Id::from_uuid(cp.function))
        .and_then(|f| f.device);
    signed(q, device, &cp.label)
}

/// `text`, which names a CP or a representation of `device` as the page prints its tag, with the tag's product sign
/// also where a frozen tag hides it on pages (`G1:+` → `-G1:+`): the layout rules and the route card name a device by
/// its tag with its prefix, as §12a's `{wire}` and the other rules do (ID-1509 (9), the window check's F4).
fn signed(q: &Q, device: Option<Id<ew_derive::ew_schema::Device>>, text: &str) -> String {
    let tag = device.and_then(|d| q.get::<ew_derive::DeviceTag>(d).as_ref().clone());
    let Some(tag) = tag.filter(|t| !t.sign_on_pages) else {
        return text.to_owned();
    };
    let shown = tag.short();
    let full = ew_derive::Tag {
        sign_on_pages: true,
        ..tag
    }
    .short();
    match text.strip_prefix(shown.as_str()) {
        Some(rest) if !shown.is_empty() => format!("{full}{rest}"),
        _ => text.to_owned(),
    }
}

/// The measures of a page in words (see [`PageLayout`]).
fn page_layout(q: &Q, tables: &PageTables, m: &Measures) -> PageLayout {
    let page: Id<ew_derive::ew_schema::Page> = Id::from_uuid(tables.page);
    let cell = |p: (i64, i64)| cell_text(q, page, p);
    let connection = |id: Uuid| connection_text(q, tables, id);
    let symbol = |id: Uuid| symbol_text(q, tables, id);
    let point = |p: (i64, i64)| [p.0, p.1];
    let stretches = |s: &[(Point, Point)]| s.iter().map(|(a, b)| [point(*a), point(*b)]).collect();
    let first = |s: &[(Point, Point)]| s.first().map(|(a, _)| cell(*a)).unwrap_or_default();
    let through = |list: &[routing::Through]| {
        list.iter()
            .map(|t| LayoutThrough {
                connection: connection(t.connection),
                symbol: symbol(t.rep),
                stretches: stretches(&t.stretches),
                cell: first(&t.stretches),
            })
            .collect()
    };
    PageLayout {
        folio: tables.folio.clone(),
        touches: m
            .touches
            .iter()
            .map(|t| LayoutTouch {
                connection: connection(t.connection),
                point: match t.point {
                    TouchedPoint::Cp(id) => tables
                        .cps
                        .iter()
                        .find(|cp| cp.id == id)
                        .map(|cp| cp_text(q, cp))
                        .unwrap_or_default(),
                    TouchedPoint::Junction(_) => "the junction".to_owned(),
                },
                at: point(t.at),
                cell: cell(t.at),
            })
            .collect(),
        overlaps: m
            .overlaps
            .iter()
            .map(|o| LayoutPair {
                connection: connection(o.connections[0]),
                other: connection(o.connections[1]),
                stretches: stretches(&o.stretches),
                cell: first(&o.stretches),
            })
            .collect(),
        through_symbols: through(&m.through_bodies),
        through_texts: through(&m.through_texts),
        four_way_junctions: m
            .four_way
            .iter()
            .map(|j| LayoutJunction {
                arms: j.arms,
                at: point(j.at),
                cell: cell(j.at),
            })
            .collect(),
        crossings: m
            .crossings
            .iter()
            .map(|c| LayoutCrossing {
                connection: connection(c.connections[0]),
                other: connection(c.connections[1]),
                at: point(c.at),
                cell: cell(c.at),
            })
            .collect(),
        bends: m
            .bends
            .iter()
            .map(|b| LayoutBends {
                connection: connection(b.connection),
                bends: b.bends,
            })
            .collect(),
    }
}

/// The text runs the labels layer draws on `page` for the owners in `owners`: each run's owner, its box in page nm and
/// its text.
fn text_runs(q: &Q, page: PageRef, owners: &BTreeSet<Uuid>) -> Vec<(Uuid, [i64; 4], String)> {
    type Runs = Vec<(Uuid, [i64; 4], String)>;
    fn walk(
        items: &[ew_dl::Item],
        map: &dyn Fn(Pt2) -> Pt2,
        owner: Option<Uuid>,
        owners: &BTreeSet<Uuid>,
        out: &mut Runs,
    ) {
        for item in items {
            let owner = item.entity.map(|e| e.0).or(owner);
            match &item.kind {
                ew_dl::ItemKind::Glyphs(run) => {
                    if let Some(o) = owner.filter(|o| owners.contains(o)) {
                        let p = run.layout_quad().map(map);
                        let (xs, ys) = (p.iter().map(|p| p.x.0), p.iter().map(|p| p.y.0));
                        let rect = [
                            xs.clone().min().unwrap_or(0),
                            ys.clone().min().unwrap_or(0),
                            xs.max().unwrap_or(0),
                            ys.max().unwrap_or(0),
                        ];
                        out.push((o, rect, run.text.clone()));
                    }
                }
                ew_dl::ItemKind::Group(g) => {
                    let t = g.transform.unwrap_or(ew_dl::Transform::IDENTITY);
                    walk(&g.items, &|p| map(t.apply(p)), owner, owners, out);
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &page_layer(q, page, Layer::Labels).list.items,
        &|p| p,
        None,
        owners,
        &mut out,
    );
    out
}

/// The layout rules' findings on the page of `tables` (WP-S2.19 §9), worded by `ew-checks`: the measures of its
/// routed connections (an unrouted one counts only as unrouted, D3), a touched terminal CP left to EW-TERM-006, and
/// EW-CORE-073 for each connection without a route between two different points.
fn layout_findings(q: &Q, page_ref: PageRef, tables: &PageTables, out: &mut Vec<crate::checks::Finding>) {
    use crate::checks::rules::page as rules;
    let page: Id<ew_derive::ew_schema::Page> = Id::from_uuid(tables.page);
    let mut g = geometry(q, page_ref, tables);
    g.connections.retain(|c| c.routed);
    let m = routing::measure(&g);
    let words: Vec<(Uuid, String)> = tables
        .connections
        .iter()
        .map(|c| (c.id, connection_text(q, tables, c.id)))
        .collect();
    let wire = |id: Uuid| rules::LayoutWire {
        id: Id::from_uuid(id),
        words: words
            .iter()
            .find(|(c, _)| *c == id)
            .map_or("", |(_, w)| w.as_str()),
    };
    for t in &m.touches {
        match t.point {
            TouchedPoint::Cp(id) => {
                let Some(cp) = tables.cps.iter().find(|cp| cp.id == id) else {
                    continue;
                };
                let terminal = q
                    .functions()
                    .get(Id::from_uuid(cp.function))
                    .and_then(|f| f.device)
                    .and_then(|d| q.devices().get(d))
                    .is_some_and(|d| d.aspects.terminal.is_some());
                if terminal {
                    continue;
                }
                let label = cp_text(q, cp);
                let point = rules::PassedPoint::Cp {
                    function: Id::from_uuid(cp.function),
                    idx: cp.idx,
                    label: &label,
                };
                out.push(rules::wire_through_cp(page, wire(t.connection), point));
            }
            TouchedPoint::Junction(id) => {
                let cell = cell_text(q, page, t.at);
                let point = rules::PassedPoint::Junction {
                    id: Id::from_uuid(id),
                    cell: &cell,
                };
                out.push(rules::wire_through_cp(page, wire(t.connection), point));
            }
        }
    }
    for o in &m.overlaps {
        out.push(rules::wire_along_wire(
            page,
            &tables.folio,
            wire(o.connections[0]),
            wire(o.connections[1]),
        ));
    }
    // An interruption point by its name, which each language words itself (ID-1509 (7)).
    let ip_name = |rep: Uuid| {
        tables
            .reps
            .iter()
            .find(|r| r.id == rep)
            .and_then(|r| r.ip.as_ref())
            .map(|ip| ip.name.as_str())
    };
    for t in &m.through_bodies {
        let device = symbol_text(q, tables, t.rep);
        let symbol = ip_name(t.rep).map_or(rules::SymbolName::Device(&device), rules::SymbolName::Ip);
        out.push(rules::wire_through_symbol(
            page,
            wire(t.connection),
            Id::from_uuid(t.rep),
            symbol,
        ));
    }
    if !m.through_texts.is_empty() {
        let owners: BTreeSet<Uuid> = m.through_texts.iter().map(|t| t.rep).collect();
        let runs = text_runs(q, page_ref, &owners);
        for t in &m.through_texts {
            // The run the first stretch inside runs through: the one holding the stretch's middle, else the owner's
            // first.
            let mid = t
                .stretches
                .first()
                .map(|(a, b)| ((a.0 + b.0) / 2, (a.1 + b.1) / 2));
            let own = || runs.iter().filter(|(o, _, _)| *o == t.rep);
            let text = own()
                .find(|(_, r, _)| mid.is_some_and(|(x, y)| r[0] <= x && x <= r[2] && r[1] <= y && y <= r[3]))
                .or_else(|| own().next())
                .map_or("", |(_, _, s)| s.as_str());
            let device = symbol_text(q, tables, t.rep);
            let owner = if let Some(name) = ip_name(t.rep) {
                rules::TextOwner::Ip {
                    rep: Id::from_uuid(t.rep),
                    name,
                }
            } else if tables.reps.iter().any(|r| r.id == t.rep) {
                rules::TextOwner::Symbol {
                    rep: Id::from_uuid(t.rep),
                    device: &device,
                }
            } else if tables.items.iter().any(|i| i.id == t.rep) {
                rules::TextOwner::FreeText(Id::from_uuid(t.rep))
            } else {
                rules::TextOwner::StructureBox(Id::from_uuid(t.rep))
            };
            out.push(rules::wire_through_text(page, wire(t.connection), owner, text));
        }
    }
    for j in &m.four_way {
        let cell = cell_text(q, page, j.at);
        out.push(rules::four_way_junction(
            page,
            &tables.folio,
            Id::from_uuid(j.junction),
            &cell,
        ));
    }
    for c in &tables.connections {
        if !c.stored_route && c.polyline.len() >= 2 && c.polyline.first() != c.polyline.last() {
            out.push(rules::unrouted_wire(page, wire(c.id)));
        }
    }
}

impl Session {
    /// `report` with the layout rules' findings of every schematic page joined (EW-CORE-067 to -070, -072 and -073;
    /// WP-S2.19 D19), when the run evaluated one of them; the scheme, the severities and the waivers apply as
    /// [`CheckReport::with_layout_evidence`](crate::checks::CheckReport::with_layout_evidence) says.
    pub(crate) fn with_layout_findings(
        &self,
        q: &Q,
        report: crate::checks::CheckReport,
    ) -> crate::checks::CheckReport {
        use crate::checks::rules::page as rules;
        let layout = [
            &rules::WIRE_THROUGH_CP,
            &rules::WIRE_ALONG_WIRE,
            &rules::WIRE_THROUGH_SYMBOL,
            &rules::WIRE_THROUGH_TEXT,
            &rules::FOUR_WAY_JUNCTION,
            &rules::UNROUTED_WIRE,
        ];
        if !report.rules.iter().any(|m| layout.iter().any(|l| l.id == m.id)) {
            return report;
        }
        let mut found = Vec::new();
        for folio in self.schematic_folios() {
            if let (Some(tables), Ok(page)) = (self.page_tables(&folio), entry_of(q, &folio)) {
                layout_findings(q, page, &tables, &mut found);
            }
        }
        report.with_layout_evidence(q, found)
    }

    /// The folios of every drawn page but the panel-layout pages, in page order: the pages `ew layout` measures and
    /// `ew route` routes by default.
    pub fn schematic_folios(&self) -> Vec<String> {
        self.query()
            .get::<PageOrder>(())
            .entries
            .iter()
            .filter(|e| matches!(e.page, PageRef::Drawn(_)) && e.kind != PageType::PanelLayout)
            .map(|e| e.folio.clone())
            .collect()
    }

    /// The connection `id` of the page labelled `folio` named by its ends, as `ew layout` names it
    /// (`E55:PE → PE-1.1`); `None` for an unknown folio or a connection the page does not draw.
    pub fn connection_words(&self, folio: &str, id: Uuid) -> Option<String> {
        let tables = self.page_tables(folio)?;
        let text = connection_text(&self.query(), &tables, id);
        (!text.is_empty()).then_some(text)
    }

    /// The geometry of the page labelled `folio` as the schematic router and the layout measures read it, in page nm
    /// (see the module documentation); `None` for an unknown folio or a page that is not drawn.
    pub fn page_geometry(&self, folio: &str) -> Option<PageGeometry> {
        let tables = self.page_tables(folio)?;
        let q = self.query();
        let page = entry_of(&q, folio).ok()?;
        Some(geometry(&q, page, &tables))
    }

    /// The layout measures of the page labelled `folio`, or with `None` of every drawn page but the panel-layout
    /// pages, in page order, each named in words ([`PageLayout`]).
    ///
    /// # Errors
    ///
    /// [`LayoutError::NoFolio`] when no page has the folio, [`LayoutError::NotDrawn`] when it names a section divider
    /// or a generated page.
    pub fn layout_report(&self, folio: Option<&str>) -> Result<Vec<PageLayout>, LayoutError> {
        let q = self.query();
        let folios: Vec<String> = match folio {
            Some(f) => {
                entry_of(&q, f)?;
                vec![f.to_owned()]
            }
            None => self.schematic_folios(),
        };
        let mut out = Vec::new();
        for f in folios {
            let tables = self
                .page_tables(&f)
                .ok_or_else(|| LayoutError::NotDrawn(f.clone()))?;
            let page = entry_of(&q, &f)?;
            let m = routing::measure(&geometry(&q, page, &tables));
            out.push(page_layout(&q, &tables, &m));
        }
        Ok(out)
    }
}
