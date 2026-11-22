// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page tables (`Session::page_tables`): every record drawn on a page as rows in page nm, in an order a
//! person can see and a replay reproduces (representations, interruption points, boxes, potentials, CPs,
//! connections, junctions, items, structure boxes).
//!
//! Not here: a symbol's shape (`shape.rs`); the wires through a junction (`editing.rs`).

use std::collections::BTreeMap;

use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_derive::ew_schema::{
    ConnectionKind, Device, Dir4, Endpoint, Function, FunctionKind, Junction, JunctionStyle, JunctionWiring,
    Page, Representation, SymbolDef,
};
use ew_derive::ew_units::{Nm, Orient2, Pt2, Rect};
use ew_derive::{
    CpRef, FunctionCps, PageRef, PotentialNameOf, PotentialPartition, Q, box_caption, cp_label, cp_point,
    free_text, group_marker, live_connections,
};
use ew_scene::forms::FrameGeometry;
use ew_scene::page::DEFAULT_SHEET;
use ew_scene::route::{RouteEnd, connection_polyline};

use super::catalogue::{connection_kind_word, drawn_page, ip_direction_word, line_kind_word, rep_type_word};
use super::shape::{SymbolShape, dir_of, pt, shape_of};
use crate::Session;
use crate::geometry::{Dir, orient_dir, orient_index, orient_point};
use crate::output::navigation::cp_candidates;
use crate::output::view::{PageFrameOf, PageRepsOf};

/// The records drawn on a page as tables, in page nm, each list in an order a person can see and a
/// replay reproduces: representations, junctions and connections by place (y, then x;
/// a connection by its first, then its last vertex), connection points by label, ties by the ID as the last key.
/// Never by ID alone: the IDs one transaction mints are not in creation order, and a tool's first hit or a listed
/// proposal would then differ between two runs of one script.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageTables {
    /// The printed folio label.
    pub folio: String,
    /// The page's ID.
    pub page: Uuid,
    /// The page's connection grid `(num, den)` nm.
    pub grid: (i64, i64),
    /// The representations, by insertion point (y, then x), then ID.
    pub reps: Vec<TableRep>,
    /// The connection points drawn on the page, by label (natural order), then point (y, then x), then v5 ID.
    pub cps: Vec<TableCp>,
    /// The connections drawn on the page, by the first vertex of the drawn polyline, then the last (y, then x), then
    /// ID.
    pub connections: Vec<TableConnection>,
    /// The junctions of the page, by point (y, then x), then ID.
    pub junctions: Vec<TableJunction>,
    /// The page items of the page, by position: the top of their bounds, then the left, then ID.
    pub items: Vec<TableItem>,
    /// The structure boxes of the page, by position: their top-left vertex (the smallest `y`,
    /// then `x`), then ID.
    pub structure_boxes: Vec<TableStructureBox>,
}

/// A page-item row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableItem {
    /// The item's ID.
    pub id: Uuid,
    /// `graphic`, `free_text`, `wire_marker`, `image`, `surface_view` (its bounds the datum point), or `unknown` for a
    /// variant this build keeps verbatim.
    pub kind: String,
    /// Its bounds `[x0, y0, x1, y1]`: a graphic's shape (a circle or an arc by its full circle), a drawn text or
    /// marker group by its drawn items, else its anchor point; an unknown variant `[0, 0, 0, 0]`.
    pub bounds: [i64; 4],
    /// A wire marker's live connections (existing, on this page) in printed order; empty for the other kinds.
    pub connections: Vec<Uuid>,
    /// The printed text of a free text (its bound value, `ew_derive::free_text`) or of a wire marker (its group
    /// text, `ew_derive::group_marker`); `None` for a graphic or an unknown variant.
    pub text: Option<String>,
    /// A free text's binding name (`project.description`, …); `None` otherwise.
    pub bind: Option<String>,
}

/// A structure-box row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableStructureBox {
    /// The box's ID.
    pub id: Uuid,
    /// The outline's vertices, absolute.
    pub outline: Vec<(i64, i64)>,
    /// `dotted`, `dash_dot`, or `unknown`.
    pub style: String,
    /// The printed caption (the caption and the external reference, `ew_derive::box_caption`).
    pub caption: String,
}

/// The bounds of points, `[x0, y0, x1, y1]`; `None` for none.
fn bounds_of(points: impl IntoIterator<Item = (i64, i64)>) -> Option<[i64; 4]> {
    points.into_iter().fold(None, |acc, (x, y)| {
        Some(match acc {
            None => [x, y, x, y],
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
        })
    })
}

/// The bounds of a graphic's shape (a circle or an arc by its full circle).
fn shape_bounds(shape: &ew_derive::ew_schema::GraphicShape) -> Option<[i64; 4]> {
    use ew_derive::ew_schema::GraphicShape as S;
    let round = |c: Pt2, r: Nm| {
        let c = pt(c);
        bounds_of([
            (c.0.saturating_sub(r.0), c.1.saturating_sub(r.0)),
            (c.0.saturating_add(r.0), c.1.saturating_add(r.0)),
        ])
    };
    match shape {
        S::Line(l) => bounds_of([pt(l.a), pt(l.b)]),
        S::Polyline(p) | S::Polygon(p) => bounds_of(p.points.iter().map(|p| pt(*p))),
        S::Rect(r) => bounds_of([pt(r.min), pt(r.max)]),
        S::Circle(c) => round(c.centre, c.r),
        S::Arc(a) => round(a.centre, a.r),
        S::Unknown(_) => None,
    }
}

/// The page items of a page as table rows, by position (see [`PageTables::items`]).
fn item_rows(q: &Q, page: Id<Page>) -> Vec<TableItem> {
    use ew_derive::ew_schema::PageItemKind as K;
    let items = q.page_items();
    let mut ids: Vec<_> = q.idx().page_items_by_page(page).collect();
    ids.sort();
    let records: Vec<_> = ids.into_iter().filter_map(|id| items.get(id).cloned()).collect();
    // The drawn items of the page's texts and group markers (the labels layer), read only when the page has one.
    let texts = records
        .iter()
        .any(|i| matches!(i.kind, K::FreeText(_) | K::WireMarker(_)));
    let drawn: Vec<(ew_dl::EntityRef, Rect)> = if texts {
        crate::output::layers::page_layer(q, PageRef::Drawn(page), ew_dl::Layer::Labels)
            .list
            .entity_bounds()
    } else {
        Vec::new()
    };
    let drawn_bounds = |id: Uuid| {
        drawn
            .iter()
            .filter(|(e, _)| e.0 == id)
            .map(|(_, r)| r)
            .flat_map(|r| [pt(r.min()), pt(r.max())])
            .collect::<Vec<_>>()
    };
    let mut rows: Vec<TableItem> = records
        .into_iter()
        .map(|item| {
            let id = item.id.as_uuid();
            let (kind, bounds, text, bind) = match &item.kind {
                K::Graphic(g) => ("graphic", shape_bounds(&g.shape), None, None),
                K::FreeText(t) => (
                    "free_text",
                    bounds_of(drawn_bounds(id)).or(bounds_of([pt(t.at)])),
                    Some(free_text(q, page, t)),
                    t.bind.as_ref().map(|b| b.as_str().to_owned()),
                ),
                K::WireMarker(m) => (
                    "wire_marker",
                    bounds_of(drawn_bounds(id)).or(bounds_of([pt(m.at)])),
                    group_marker(q, &item).map(|g| g.text),
                    None,
                ),
                K::Image(i) => (
                    "image",
                    bounds_of([pt(i.rect.0.min()), pt(i.rect.0.max())]),
                    None,
                    None,
                ),
                // A surface view by its datum point (what it draws is derived).
                K::SurfaceView(v) => ("surface_view", bounds_of([pt(v.datum_at)]), None, None),
                K::Underlay(u) => (
                    "underlay",
                    bounds_of([pt(u.rect.0.min()), pt(u.rect.0.max())]),
                    None,
                    None,
                ),
                K::Unknown(_) => ("unknown", None, None, None),
            };
            TableItem {
                id,
                kind: kind.to_owned(),
                bounds: bounds.unwrap_or([0; 4]),
                connections: live_connections(q, &item)
                    .iter()
                    .map(|c| c.id.as_uuid())
                    .collect(),
                text,
                bind,
            }
        })
        .collect();
    rows.sort_by_key(|r| (r.bounds[1], r.bounds[0], r.id));
    rows
}

/// The structure boxes of a page as table rows, by position (see [`PageTables::structure_boxes`]).
fn box_rows(q: &Q, page: Id<Page>) -> Vec<TableStructureBox> {
    use ew_derive::ew_schema::BoxStyle;
    let boxes = q.structure_boxes();
    let mut ids: Vec<_> = q.idx().structure_boxes_by_page(page).collect();
    ids.sort();
    let mut rows: Vec<TableStructureBox> = ids
        .into_iter()
        .filter_map(|id| boxes.get(id).cloned())
        .map(|b| TableStructureBox {
            id: b.id.as_uuid(),
            outline: b.outline.iter().map(|p| pt(*p)).collect(),
            style: match b.style {
                BoxStyle::Dotted => "dotted",
                BoxStyle::DashDot => "dash_dot",
                BoxStyle::Unknown(_) => "unknown",
            }
            .to_owned(),
            caption: box_caption(q, &b),
        })
        .collect();
    let top_left = |r: &TableStructureBox| {
        r.outline
            .iter()
            .map(|(x, y)| (*y, *x))
            .min()
            .unwrap_or((i64::MAX, i64::MAX))
    };
    rows.sort_by_key(|r| (top_left(r), r.id));
    rows
}

/// The side of a terminal CP in the model's words: `field` when the CP's side is its strip's `field_side` (side 2
/// when the terminal has no strip aspect to read), `internal` for the other side, `None` for a CP without a side.
fn terminal_side(q: &Q, function: Id<Function>, idx: u16) -> Option<&'static str> {
    use ew_derive::ew_schema::Side;
    let cps = q.get::<FunctionCps>(function);
    let side = cps.iter().find(|c| c.idx == idx)?.side.clone()?;
    let field = q
        .functions()
        .get(function)
        .and_then(|f| f.device)
        .and_then(|d| q.devices().get(d))
        .and_then(|d| d.aspects.terminal.as_ref().map(|t| t.strip))
        .and_then(|s| q.devices().get(s))
        .and_then(|s| s.aspects.strip.as_ref().map(|s| s.field_side.clone()))
        .unwrap_or(Side::Side2);
    Some(if side == field { "field" } else { "internal" })
}

/// A representation row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableRep {
    /// The representation's ID.
    pub id: Uuid,
    /// Its functions.
    pub functions: Vec<Uuid>,
    /// The device of its first function.
    pub device: Option<Uuid>,
    /// The symbol copy.
    pub symbol: Uuid,
    /// The symbol's key (empty for a symbol copy the project does not hold).
    pub symbol_key: String,
    /// The orientation `0..=7` (A to H).
    pub orient: u8,
    /// The representation type in its JSON spelling.
    pub rep_type: String,
    /// The insertion point.
    pub pos: (i64, i64),
    /// The symbol's body under the orientation, translated to the insertion point, `[x0, y0, x1, y1]`.
    pub body: [i64; 4],
    /// Where the printed tag text stands: the anchor of the symbol's tag slot where the scene prints it
    /// (visible by the representation's override, else its arrangement, else the slot's default; at the override's
    /// position or the slot's, under the orientation, translated to the insertion point; where the scene's
    /// tag rule moved it, read from the page's labels layer), raised by the em height
    /// of the slot's text style, so that it is at or above the top of the text whatever its vertical alignment (for
    /// text that reads horizontally); `None` for a representation that prints no tag (no device, an interruption
    /// point, no visible tag slot). A preview chip stands above it.
    pub tag_at: Option<(i64, i64)>,
    /// The interruption point the representation draws.
    pub ip: Option<TableIp>,
    /// The black box's geometry, absolute.
    pub black_box: Option<TableBox>,
    /// Whether the device of its first function is a terminal of a strip: its connection points print the terminal's
    /// tag (`-X1011:2`, with `/clamp` where the function names clamps), not `tag:designation`.
    pub terminal: bool,
    /// Kept in place (`Representation.locked`, doc 06a §2.4): the canvas marks it while it is selected.
    pub locked: bool,
}

/// The interruption point of a [`TableRep`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableIp {
    /// The IP's name.
    pub name: String,
    /// Its direction in its JSON spelling (`source`, `target`, `undirected`).
    pub direction: String,
    /// Where it continues: its printed cross-reference to its partners, as the canvas prints it; empty for
    /// a point without a partner.
    pub continuation: String,
    /// Whether it is an external point: it names an external reference (a supply coming in from another drawing
    /// set, FT-1699).
    pub external: bool,
}

/// A black box's geometry, absolute on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableBox {
    /// The outline's vertices.
    pub outline: Vec<(i64, i64)>,
    /// The box CPs: function CP index, point, direction.
    pub cps: Vec<(u16, (i64, i64), Dir)>,
    /// Whether the box is a valve terminal's (its device a PLC station of role `valve_terminal`): a valve placed inside
    /// is a valve of its own that joins the terminal, never a function of the terminal's device.
    pub valve_terminal: bool,
    /// The cells of a box drawn as the terminal's valve positions, left to right; empty for a free box.
    pub cells: Vec<SlotCell>,
    /// The supply end of such a box (left, top, right, bottom on the page); `None` for a free box.
    pub supply_end: Option<[i64; 4]>,
    /// Whether the box's pins are its own: its function is a black box whose device has no part, so a pin is
    /// added, named and removed on the box itself; a box of a part draws the part's pins, edited in its device box.
    pub own_pins: bool,
    /// The names of the box's own pins (`BoxCp.name`): function CP index and name.
    pub names: Vec<(u16, String)>,
    /// The least width and the least height in grid steps that keep the pin texts of its left and right sides, and of
    /// its top and bottom sides, apart and inside it; 0 across where they print none.
    pub room: (i64, i64),
}

/// One cell of a [`TableBox`] drawn as a valve terminal's positions.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotCell {
    /// The module the position belongs to (the terminal, or a sub-base).
    pub module: Uuid,
    /// The position, 1-based, on its module.
    pub position: u32,
    /// Its rectangle on the page: left, top, right, bottom.
    pub rect: [i64; 4],
    /// The valve or blanking plate that holds it; `None` when empty.
    pub holder: Option<Uuid>,
    /// Whether the holder is a blanking plate.
    pub blank: bool,
}

/// The cells and the supply end of valve terminal `terminal`'s box drawn as its positions in `frame`
/// ([`TableBox::cells`]; `ew_derive::box_frame`: each cell as wide as its valve needs).
fn table_cells(
    q: &ew_derive::Q,
    terminal: Id<Device>,
    frame: &ew_derive::SlotFrame,
) -> (Vec<SlotCell>, Option<[i64; 4]>) {
    let (top, _, bottom) = frame.rows();
    let cells = ew_derive::terminal_cells(q, terminal)
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let (l, r) = frame.cell_x(i);
            let (holder, blank) = match c.slot {
                ew_derive::ValveSlot::Valve { device, .. } => (Some(device.as_uuid()), false),
                ew_derive::ValveSlot::Blank { device } => (Some(device.as_uuid()), true),
                ew_derive::ValveSlot::Empty => (None, false),
            };
            SlotCell {
                module: c.module.as_uuid(),
                position: c.position,
                rect: [l.0, top.0, r.0, bottom.0],
                holder,
                blank,
            }
        })
        .collect();
    let origin = frame.origin();
    (cells, Some([origin.x.0, top.0, frame.end_x().0, bottom.0]))
}

/// The derived potential of a [`TableCp`]: its name, and whether that name is only the fallback.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TablePotential {
    /// The derived name (`PotentialNameOf`).
    pub name: String,
    /// The name is the fallback `<tag>:<designation>` of a member CP (`NameSource::Fallback`), not one a definition,
    /// a source interruption point or the supply gives.
    pub fallback: bool,
}

/// A connection point row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableCp {
    /// The CP's v5 ID (`Destination::Cp`'s, equal to `PageCp::id`).
    pub id: Uuid,
    /// The function.
    pub function: Uuid,
    /// The function's CP index.
    pub idx: u16,
    /// The representation that draws it (the smallest ID).
    pub rep: Uuid,
    /// The CP as `cp_label` prints it (`-102F1:2`).
    pub label: String,
    /// Its point on the page (`cp_point`, the position `ew_commands::geometry::cp_position` gives a symbol CP).
    pub at: (i64, i64),
    /// The direction a wire leaves it.
    pub dir: Option<Dir>,
    /// The connections on this page ending at it, in the order of [`PageTables::connections`].
    pub connections: Vec<Uuid>,
    /// The derived name of its potential, and whether that name is a fallback.
    pub potential: Option<TablePotential>,
    /// A terminal CP's side in the model's words (doc 08 §2.1): `field` when its side is the strip's
    /// `field_side`, `internal` for the other side; `None` for a CP without a side.
    pub side: Option<&'static str>,
    /// Whether it is a fluid port (a pneumatic, vacuum or hydraulic CP): autoconnect joins a fluid port to a fluid port
    /// only (FT-1733).
    pub fluid: bool,
    /// Its side of EW-FL-012 (a valve's working or exhaust port): autoconnect never joins two sides that conflict.
    pub valve_port: Option<ew_derive::ValvePort>,
    /// The printed PLC address of the channel bound to it (`ew_derive::PlcAddressText`'s pins); `None`
    /// for an unbound CP or an address that does not resolve.
    pub plc_address: Option<String>,
    /// Whether a PLC output channel (digital or analogue) is bound to it: autoconnect never joins a contact to it
    /// (a sensor placed near an output card was joined to an output).
    pub plc_output: bool,
    /// Whether its function's FnDef is a fitting (`ew_derive::is_fitting`): autoconnect never joins a fitting's port
    /// to it (four flow controls placed in a row were joined by tubes).
    pub fitting: bool,
}

/// One end of a [`TableConnection`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TableEnd {
    /// A function CP.
    Cp {
        /// The function.
        function: Uuid,
        /// The CP index.
        idx: u16,
    },
    /// A junction.
    Junction {
        /// The junction.
        id: Uuid,
    },
}

/// A connection row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableConnection {
    /// The connection's ID.
    pub id: Uuid,
    /// End `a`.
    pub a: TableEnd,
    /// End `b`.
    pub b: TableEnd,
    /// The connection kind's JSON key (`wire`, `core`, `bridge`).
    pub kind: String,
    /// The drawn polyline: the stored route as the scene draws it, else the unrouted line it draws.
    pub polyline: Vec<(i64, i64)>,
    /// Whether the connection has a stored route.
    pub stored_route: bool,
    /// The polyline's segments (`i` for `polyline[i]` to `polyline[i + 1]`) that the scene's clamp drew along the
    /// sheet's edge, off the connection's path (an end or a route vertex off the sheet): the wire
    /// does not run there, so the Place tool neither splits nor drops on them. Ascending; empty for a path on the
    /// sheet.
    pub off_sheet: Vec<usize>,
    /// A fluid line's line kind (`working`, `pilot`, `exhaust`, `drain`, `flexible`, `unknown` for a kind of a newer
    /// format); `None` for any other connection.
    pub line_kind: Option<String>,
}

impl TableConnection {
    /// Whether the segment `a`–`b` of [`polyline`](Self::polyline) is one of [`off_sheet`](Self::off_sheet).
    pub fn is_off_sheet(&self, a: (i64, i64), b: (i64, i64)) -> bool {
        self.off_sheet
            .iter()
            .any(|&i| self.polyline.get(i) == Some(&a) && self.polyline.get(i + 1) == Some(&b))
    }
}

/// The segments of the drawn (clamped) `polyline` that lie on no segment of the connection's unclamped `path`
/// (`ew_derive::drawn_path`, the path normaliser 7 and EW-TERM-006 read). A stored route is drawn vertex
/// for vertex, so its segment `i` is compared with the path's segment `i` alone; the unrouted line (at most three
/// vertices, cleaned after the clamp) with every segment of the path.
fn off_sheet_segments(polyline: &[Pt2], path: &[Pt2], stored: bool) -> Vec<usize> {
    if polyline == path {
        return Vec::new();
    }
    let on = |a: Pt2, b: Pt2, s: &[Pt2]| {
        ew_derive::on_segment(a, s[0], s[1]) && ew_derive::on_segment(b, s[0], s[1])
    };
    polyline
        .windows(2)
        .enumerate()
        .filter(|(i, w)| {
            if stored && polyline.len() == path.len() {
                !on(w[0], w[1], &path[*i..*i + 2])
            } else {
                !path.windows(2).any(|s| on(w[0], w[1], s))
            }
        })
        .map(|(i, _)| i)
        .collect()
}

/// A junction row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableJunction {
    /// The junction's ID.
    pub id: Uuid,
    /// Its point.
    pub at: (i64, i64),
    /// `dot` or `bevel`.
    pub style: String,
    /// The bevel's orientation `0..=7` (0 for a dot).
    pub bevel_orient: u8,
    /// The effective wiring, `star` or `chain` (the stored one, else the style's default).
    pub wiring: String,
    /// Whether the wiring is stored.
    pub explicit_wiring: bool,
    /// The connections in wiring order.
    pub sequence: Vec<Uuid>,
}

/// What draws a function CP on the page: its point, direction and representation (the scene's rules: the
/// representations in ID order, the first that draws the CP wins; symbol CPs, else the box's CPs, then the CPs
/// of the terminals a strip segment draws, side 1 leaving up and side 2 down, turned with the segment).
type DrawnCps = BTreeMap<(Id<Function>, u16), (Pt2, Dir, Id<Representation>)>;

/// The drawn CPs of a page (see [`DrawnCps`]), from the page's memoised representation views (`page_reps`).
fn drawn_cps(q: &Q, page: Id<Page>) -> DrawnCps {
    // A point within 1 nm of a grid position is the grid's own number for it (the scene's rule).
    let grid = q
        .pages()
        .get(page)
        .map_or(ew_derive::ew_units::Grid::MM4, |p| p.grid);
    let mut map = DrawnCps::new();
    let views = q.get::<PageRepsOf>(page);
    let mut views: Vec<&ew_scene::page::RepView> = views.iter().collect();
    views.sort_by_key(|v| v.rep.id);
    for rv in views {
        let Some(def) = &rv.symbol else { continue };
        let rep = &rv.rep;
        let rid = rep.id;
        let o = orient_index(rep.orient);
        let mut put = |function: Id<Function>, idx: u16, at: Option<Pt2>, dir: Dir| {
            let Some(at) = at else { return };
            map.entry((function, idx))
                .or_insert((grid.settle_point(at), dir, rid));
        };
        let placed = |offset: Pt2| rep.pos.checked_add(rep.orient.checked_apply(offset)?);
        for cp in def.cps.iter() {
            let (function, idx) = match rep.pin_map.iter().find(|p| p.symbol_cp == cp.idx) {
                Some(p) => (p.function, p.idx),
                None => match rep.functions.first() {
                    Some(f) => (*f, cp.fn_cp),
                    None => continue,
                },
            };
            put(function, idx, placed(cp.pos), orient_dir(o, dir_of(&cp.dir)));
        }
        if def.cps.is_empty()
            && let (Some(b), Some(&function)) = (&rep.box_, rep.functions.first())
        {
            for cp in &b.cps {
                put(function, cp.idx, placed(cp.pos), orient_dir(o, dir_of(&cp.dir)));
            }
        }
        // A strip segment: one point per drawn terminal and side (the scene's rule).
        if !rv.terminals.is_empty() {
            let grid = def.grid;
            let bottom = grid.snap(ew_scene::symbol::lower(def, Orient2::R0).bounds.max().y);
            for (k, t) in rv.terminals.iter().enumerate() {
                let Ok(step) = i64::try_from(k + 1) else { break };
                let side1 = segment_point(grid, rep, step, 0);
                let side2 = segment_point(grid, rep, step, bottom);
                for &i in &t.side1 {
                    put(t.function, i, side1, orient_dir(o, Dir::Up));
                }
                for &i in &t.side2 {
                    put(t.function, i, side2, orient_dir(o, Dir::Down));
                }
            }
        }
    }
    map
}

/// The page point `(i, j)` grid steps from a strip segment's origin in its symbol frame (the scene's rule: the
/// step vector turns with the orientation; on a grid-point origin the exact grid point, else the origin plus the
/// rounded offset). `None` on overflow.
fn segment_point(grid: ew_derive::ew_units::Grid, rep: &Representation, i: i64, j: i64) -> Option<Pt2> {
    let v = rep.orient.apply(Pt2::new(Nm(i), Nm(j)));
    let axis = |origin: Nm, steps: i64| {
        let k = grid.snap(origin);
        if grid.at(k) == Some(origin) {
            grid.at(k.checked_add(steps)?)
        } else {
            Some(Nm(origin.0.checked_add(grid.at(steps)?.0)?))
        }
    };
    Some(Pt2::new(axis(rep.pos.x, v.x.0)?, axis(rep.pos.y, v.y.0)?))
}

/// The page rectangle in page coordinates (the scene's `page_rect`): the frame's outer rectangle, else the form's
/// sheet, else the default sheet.
fn page_rect(q: &Q, page: PageRef) -> Rect {
    let sheet = |w: Nm, h: Nm| Rect::from_points(Pt2::ORIGIN, Pt2::new(w, h));
    match q.get::<PageFrameOf>(page).as_ref() {
        Some(f) => FrameGeometry::of(&f.form, f.title_block.as_deref())
            .map_or_else(|| sheet(f.form.sheet.width, f.form.sheet.height), |g| g.outer),
        None => sheet(DEFAULT_SHEET.0, DEFAULT_SHEET.1),
    }
}

/// The page point of the tag text `rep` prints with `def`, by the scene's rules (`ew_scene::page`'s
/// labels): the first tag slot, visible by the representation's override, else by its named arrangement, else by the
/// slot's default; the override's position, else the slot's place (`ew_scene::symbol::slot_places`), in the
/// representation's text frame (`ew_derive::text_frame`: on a half-turned black box, as on the box unturned). With it
/// the em height (nm)
/// of the slot's text style in the project's table `styles`.
fn tag_anchor(rep: &Representation, def: &SymbolDef, styles: &ew_text::TextStyleTable) -> Option<(Pt2, i64)> {
    let (i, slot) = def
        .text_slots
        .iter()
        .enumerate()
        .find(|(_, s)| matches!(s.kind, ew_derive::ew_schema::SlotKind::Tag))?;
    let index = u16::try_from(i).ok()?;
    let ov = rep.text_slots.iter().find(|o| o.slot == index);
    let visible = ov.and_then(|o| o.visible).unwrap_or_else(|| {
        rep.arrangement
            .as_ref()
            .and_then(|name| def.arrangements.iter().find(|a| a.name == *name))
            .map_or(slot.visible, |a| a.slots.contains(&index))
    });
    let (style, _) = ew_scene::symbol::slot_style_key(def, slot, styles);
    let em = styles.resolve(style).map_or(0, |t| t.size.0.max(0));
    let frame = ew_derive::text_frame(rep, def);
    let at = match ov.and_then(|o| o.pos) {
        Some(pos) => frame.place(pos)?,
        None if ov.and_then(|o| o.angle).is_some() => frame.place(slot.pos)?,
        // Where the symbol puts it: on a symbol turned a quarter, placed anew round its body.
        None => frame.origin.checked_add(
            ew_scene::symbol::slot_places(def, frame.orient, |n| n == i)
                .swap_remove(i)
                .0,
        )?,
    };
    visible.then_some((at, em))
}

/// The body of a shape under orientation `o`, translated to `pos`.
pub(super) fn placed_body(body: [i64; 4], o: u8, pos: (i64, i64)) -> [i64; 4] {
    let a = orient_point(o, (body[0], body[1]));
    let b = orient_point(o, (body[2], body[3]));
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    [
        x0.saturating_add(pos.0),
        y0.saturating_add(pos.1),
        x1.saturating_add(pos.0),
        y1.saturating_add(pos.1),
    ]
}

fn table_end(e: &Endpoint) -> Option<TableEnd> {
    match e {
        Endpoint::Cp(f, idx) => Some(TableEnd::Cp {
            function: f.as_uuid(),
            idx: *idx,
        }),
        Endpoint::Junction(j) => Some(TableEnd::Junction { id: j.as_uuid() }),
        Endpoint::Unknown(_) => None,
    }
}

impl Session {
    /// Every record drawn on the page labelled `folio`, in page nm, each list in the order of [`PageTables`]
    /// (the page items and structure boxes of part 4a by position); `None` for an unknown
    /// folio or a page that is not drawn.
    ///
    /// A representation's `body` is its symbol shape's body under its orientation, translated; a black box's
    /// outline and CPs are absolute. A CP's `at` is `cp_point` (for a symbol CP the position
    /// `ew_commands::geometry::cp_position` computes inside a command), its `dir` the direction the scene draws a
    /// wire leaving it, its `connections` those on this page ending there, its `potential` the derived name. A
    /// connection's `polyline` is what the scene draws: its stored route (`[a] ++ route ++ [b]`, clamped to the
    /// page), else the unrouted line. A junction's `wiring` is the stored one, else the style's default (dot:
    /// star, bevel: chain). Reads tracked records and memoised families only; the shell fetches it once per
    /// (revision, folio) with the page content.
    pub fn page_tables(&self, folio: &str) -> Option<PageTables> {
        let q = self.query();
        let (page_ref, page_id) = drawn_page(&q, folio)?;
        let page = q.pages().get(page_id)?.clone();
        let grid = (
            i64::try_from(page.grid.num()).unwrap_or(i64::MAX),
            i64::try_from(page.grid.den()).unwrap_or(1),
        );
        let drawn = drawn_cps(&q, page_id);
        let styles = crate::output::layers::text_style_table(&q);
        // The tags where the scene's tag rule placed them, from the labels layer the page draws.
        let labels = crate::output::layers::page_layer(&q, page_ref, ew_dl::Layer::Labels);
        let mut shapes: BTreeMap<Id<SymbolDef>, Option<SymbolShape>> = BTreeMap::new();
        let mut rep_ids: Vec<(i64, i64, Id<Representation>)> = q
            .idx()
            .reps_by_page(page_id)
            .filter_map(|r| q.reps().get(r).map(|rep| (rep.pos.y.0, rep.pos.x.0, r)))
            .collect();
        rep_ids.sort();
        let mut reps = Vec::new();
        for (_, _, rid) in rep_ids {
            let Some(rep) = q.reps().get(rid).cloned() else {
                continue;
            };
            let shape = shapes
                .entry(rep.symbol)
                .or_insert_with(|| {
                    q.symbols()
                        .get(rep.symbol)
                        .map(|s| shape_of(&q, rep.symbol.as_uuid(), &s.item))
                })
                .clone();
            let o = orient_index(rep.orient);
            let pos = pt(rep.pos);
            let first = rep.functions.first().and_then(|f| q.functions().get(*f).cloned());
            let ip = first.as_ref().and_then(|f| match &f.kind {
                FunctionKind::Ip(ip) => Some(TableIp {
                    name: ip.name.as_str().to_owned(),
                    direction: ip_direction_word(&ip.direction),
                    continuation: q.get::<ew_derive::XrefTextOf>(rid).text.clone(),
                    external: ip.external.is_some(),
                }),
                _ => None,
            });
            let black_box = rep.box_.as_ref().map(|b| {
                let abs = |p: Pt2| {
                    let r = orient_point(o, pt(p));
                    (r.0.saturating_add(pos.0), r.1.saturating_add(pos.1))
                };
                TableBox {
                    outline: b.outline.iter().map(|p| abs(*p)).collect(),
                    cps: b
                        .cps
                        .iter()
                        .map(|cp| (cp.idx, abs(cp.pos), orient_dir(o, dir_of(&cp.dir))))
                        .collect(),
                    valve_terminal: first
                        .as_ref()
                        .and_then(|f| f.device)
                        .is_some_and(|d| ew_derive::is_fluid_terminal(&q, d)),
                    cells: Vec::new(),
                    supply_end: None,
                    room: crate::output::view::box_text_room(&q, &rep),
                    own_pins: first.as_ref().is_some_and(|f| {
                        matches!(f.kind, FunctionKind::BlackBox)
                            && f.device
                                .and_then(|d| q.devices().get(d))
                                .is_some_and(|d| d.parts.is_empty())
                    }),
                    names: b
                        .cps
                        .iter()
                        .filter_map(|cp| Some((cp.idx, cp.name.as_ref()?.as_str().to_owned())))
                        .collect(),
                }
            });
            let black_box = black_box.map(|mut b| {
                if let (Some(frame), Some(terminal)) = (
                    ew_derive::box_frame(&q, &rep),
                    first.as_ref().and_then(|f| f.device),
                ) {
                    (b.cells, b.supply_end) = table_cells(&q, terminal, &frame);
                }
                b
            });
            let tag_at = (first.as_ref().is_some_and(|f| f.device.is_some() && ip.is_none()))
                .then(|| {
                    q.symbols()
                        .get(rep.symbol)
                        .and_then(|s| tag_anchor(&rep, &s.item, &styles))
                })
                .flatten()
                .map(|(p, em)| {
                    let (x, y) = pt(labels.tags.get(&rid.as_uuid()).copied().unwrap_or(p));
                    (x, y.saturating_sub(em))
                });
            reps.push(TableRep {
                id: rid.as_uuid(),
                functions: rep.functions.iter().map(|f| f.as_uuid()).collect(),
                device: first.as_ref().and_then(|f| f.device).map(|d| d.as_uuid()),
                symbol: rep.symbol.as_uuid(),
                symbol_key: shape.as_ref().map(|s| s.key.clone()).unwrap_or_default(),
                orient: o,
                rep_type: rep_type_word(&rep.rep_type),
                pos,
                body: shape
                    .as_ref()
                    .map_or([pos.0, pos.1, pos.0, pos.1], |s| placed_body(s.body, o, pos)),
                tag_at,
                ip,
                black_box,
                terminal: first
                    .and_then(|f| f.device)
                    .and_then(|d| q.devices().get(d))
                    .is_some_and(|d| d.aspects.terminal.is_some()),
                locked: rep.locked,
            });
        }
        let partition = q.get::<PotentialPartition>(());
        let bound = q.get::<ew_derive::ChannelsByCp>(());
        let mut cps = Vec::new();
        for (function, idx) in cp_candidates(&q, page_id) {
            let Some(at) = cp_point(&q, function, idx, page_id) else {
                continue;
            };
            let Some(&(_, dir, rep)) = drawn.get(&(function, idx)) else {
                continue;
            };
            let dir = Some(dir);
            // Ordered below, by the connections' own order.
            let connections: Vec<Uuid> = q
                .idx()
                .connections_at_cp(function, idx)
                .filter(|c| q.connections().get(*c).is_some_and(|c| c.page == Some(page_id)))
                .map(|c| c.as_uuid())
                .collect();
            let id: Id<()> = v5::cp_id(function, u32::from(idx));
            let potential = partition.potential_at(CpRef::new(function, idx).key()).map(|p| {
                let name = &q.get::<PotentialNameOf>(p.id).name;
                TablePotential {
                    name: name.text.clone(),
                    fallback: matches!(name.source, ew_derive::NameSource::Fallback(_)),
                }
            });
            cps.push(TableCp {
                id: id.as_uuid(),
                function: function.as_uuid(),
                idx,
                rep: rep.as_uuid(),
                label: cp_label(&q, CpRef { function, idx }),
                at: pt(at),
                dir,
                connections,
                potential,
                side: terminal_side(&q, function, idx),
                fluid: q
                    .get::<ew_derive::FunctionCps>(function)
                    .iter()
                    .find(|c| c.idx == idx)
                    .is_some_and(|c| ew_derive::is_fluid(&c.discipline)),
                valve_port: q
                    .functions()
                    .get(function)
                    .and_then(|f| q.fndefs().get(f.def))
                    .and_then(|d| {
                        let cp = q
                            .get::<ew_derive::FunctionCps>(function)
                            .iter()
                            .find(|c| c.idx == idx)
                            .cloned()?;
                        ew_derive::ValvePort::of(&d.item, cp.port_role.as_ref())
                    }),
                plc_address: q
                    .get::<ew_derive::PlcAddressText>(function)
                    .pins
                    .iter()
                    .find(|(i, _)| *i == idx)
                    .map(|(_, t)| t.clone()),
                plc_output: bound.at_cp(function, idx).any(|ch| {
                    q.table::<ew_derive::ew_schema::Channel>()
                        .get(ch)
                        .is_some_and(|c| {
                            matches!(
                                c.kind,
                                ew_derive::ew_schema::ChannelKind::Do | ew_derive::ew_schema::ChannelKind::Ao
                            )
                        })
                }),
                fitting: q
                    .functions()
                    .get(function)
                    .and_then(|f| q.fndefs().get(f.def))
                    .is_some_and(|d| ew_derive::is_fitting(&d.item)),
            });
        }
        cps.sort_by(|a, b| {
            ew_mltext::natural_cmp(&a.label, &b.label)
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| (a.at.1, a.at.0, a.id).cmp(&(b.at.1, b.at.0, b.id)))
        });
        let rect = page_rect(&q, page_ref);
        let mut junction_ids: Vec<_> = q.idx().junctions_by_page(page_id).collect();
        // A junction and a route as read: a point within 1 nm of a grid position on the grid's own number (06a §2.1).
        let page_grid = ew_derive::page_grid(&q, page_id);
        let read_junction = |j: Id<Junction>| {
            q.junctions()
                .get(j)
                .map(|x| ew_derive::read_junction(x, page_grid))
        };
        junction_ids.sort_by_key(|j| (read_junction(*j).map(|x| (x.at.y.0, x.at.x.0)), *j));
        let mut junctions = Vec::new();
        let mut junction_at: BTreeMap<Uuid, Pt2> = BTreeMap::new();
        for jid in junction_ids {
            let Some(j) = read_junction(jid) else {
                continue;
            };
            junction_at.insert(jid.as_uuid(), j.at);
            let (style, bevel_orient, default_wiring) = match &j.style {
                JunctionStyle::Dot => ("dot", 0, "star"),
                JunctionStyle::Bevel(b) => ("bevel", orient_index(b.orient), "chain"),
                JunctionStyle::Unknown(_) => ("unknown", 0, "star"),
            };
            let wiring = match &j.wiring {
                Some(JunctionWiring::Star) => "star",
                Some(JunctionWiring::Chain) => "chain",
                _ => default_wiring,
            };
            junctions.push(TableJunction {
                id: jid.as_uuid(),
                at: pt(j.at),
                style: style.to_owned(),
                bevel_orient,
                wiring: wiring.to_owned(),
                explicit_wiring: matches!(j.wiring, Some(JunctionWiring::Star | JunctionWiring::Chain)),
                sequence: j.sequence.iter().map(|c| c.as_uuid()).collect(),
            });
        }
        let connection_ids: Vec<_> = q.idx().connections_by_page(page_id).collect();
        let mut connections = Vec::new();
        for cid in connection_ids {
            let Some(c) = q
                .connections()
                .get(cid)
                .map(|c| ew_derive::read_connection(c, page_grid))
            else {
                continue;
            };
            let (Some(a), Some(b)) = (table_end(&c.a), table_end(&c.b)) else {
                continue;
            };
            let end = |e: &Endpoint| -> Option<RouteEnd> {
                match e {
                    Endpoint::Cp(f, idx) => drawn.get(&(*f, *idx)).map(|(pos, dir, _)| RouteEnd {
                        pos: *pos,
                        dir: Some(match dir {
                            Dir::Up => Dir4::Up,
                            Dir::Down => Dir4::Down,
                            Dir::Left => Dir4::Left,
                            Dir::Right => Dir4::Right,
                        }),
                    }),
                    Endpoint::Junction(j) => junction_at
                        .get(&j.as_uuid())
                        .map(|pos| RouteEnd { pos: *pos, dir: None }),
                    Endpoint::Unknown(_) => None,
                }
            };
            let (polyline, off_sheet) = match (end(&c.a), end(&c.b)) {
                (Some(ea), Some(eb)) => {
                    let drawn = connection_polyline(&ea, &eb, &c.route, rect);
                    // The segments the clamp moved, against the path normaliser 7 reads; only a path with
                    // a vertex off the sheet is clamped at all.
                    let (lo, hi) = (rect.min(), rect.max());
                    let off = |p: &Pt2| p.x < lo.x || p.x > hi.x || p.y < lo.y || p.y > hi.y;
                    let off_sheet = ew_derive::drawn_path(&q, &c, page_id)
                        .filter(|path| path.iter().any(off))
                        .map(|path| off_sheet_segments(&drawn, &path, !c.route.is_empty()))
                        .unwrap_or_default();
                    (drawn.into_iter().map(pt).collect(), off_sheet)
                }
                _ => (c.route.iter().map(|p| pt(*p)).collect(), Vec::new()),
            };
            connections.push(TableConnection {
                id: cid.as_uuid(),
                a,
                b,
                kind: connection_kind_word(&c.kind),
                polyline,
                stored_route: !c.route.is_empty(),
                off_sheet,
                line_kind: match &c.kind {
                    ConnectionKind::Fluid(f) => Some(line_kind_word(&f.line_kind)),
                    _ => None,
                },
            });
        }
        // By place: the first vertex, then the last (y, then x), then ID; a CP lists its connections in this order.
        let key = |c: &TableConnection| {
            let yx = |p: Option<&(i64, i64)>| p.map(|p| (p.1, p.0));
            (yx(c.polyline.first()), yx(c.polyline.last()), c.id)
        };
        connections.sort_by_key(key);
        let rank: BTreeMap<Uuid, usize> = connections.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
        for cp in &mut cps {
            cp.connections
                .sort_by_key(|c| (rank.get(c).copied().unwrap_or(usize::MAX), *c));
        }
        Some(PageTables {
            folio: folio.to_owned(),
            page: page_id.as_uuid(),
            grid,
            reps,
            cps,
            connections,
            junctions,
            items: item_rows(&q, page_id),
            structure_boxes: box_rows(&q, page_id),
        })
    }
}
