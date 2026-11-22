// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page display lists per layer (doc 09 §4.1, ARCH §6.4 `page_display_list(page, layer, lang)`, ADR-0015
//! decision 1 and amendment 2).
//!
//! # The view
//!
//! `ew-scene` does not depend on `ew-derive` (ARCH §3 lists no such edge), so the caller, which holds the
//! tracked read context `Q`, fills a [`PageView`]: the page's records by reference (representations,
//! connections, junctions; ARCH §3 rule 5: the model entities are the schema records), the symbol copies
//! they draw, the frame and title-block forms, the title-block bindings of `title_block(page)`, and the
//! **derived** slot texts (device tags as `device_tag(dev).displayed(page)`, CP designations, cross-references
//! with their link destinations). [`page_display_list`] is then a pure, deterministic function of the view,
//! the layer and the font set; the memoised query of ARCH §6.4 wraps it (one query per layer, so a wire
//! edit never rebuilds the title block: [`Layer::Frame`] reads only [`PageView::frame`]).
//!
//! # Layers and paint order
//!
//! | Layer | Reads | Items, in order |
//! |---|---|---|
//! | [`Layer::Frame`] | `frame` | [`crate::forms::lower_form`]: frame, grid labels, form graphics and cells, title block |
//! | [`Layer::Symbols`] | `reps`, `break_marks`, `items`, `structure_boxes` | per representation (by ID): one [`Item::instance`] of `symbol_geometry(symbol, orient)` at `pos`, tagged with the representation and its [`Link::Target`]; then its static symbol texts; then per break mark (by representation and side): its path; then the CP destinations; then the graphics and structure-box outlines ([`crate::items`]) |
//! | [`Layer::Wires`] | `reps` (CP positions), `connections`, `junctions`, `knockouts`, `bridges` | per connection (by ID): its polyline, cut round the texts it runs through ([`crate::knockout`]); then per junction (by ID): its dot or bevel symbol; then per bridge (in the view's order): its line and dots in each row of its terminals |
//! | [`Layer::Labels`] | `reps`, `connections`, `wire_marker`, `items`, `structure_boxes`, `strip_tags` | per representation (by ID): its visible slot texts (a [`Link::Source`] where the slot value names a destination); then the pin texts; then per strip tag (by representation ID): the strip's tag at its terminal ([`StripTagView`]); then per connection with a marker (by ID): the marker box and text; then the free texts, wire-marker groups and structure-box captions ([`crate::items`]) |
//! | [`Layer::Tables`] | `frame`, `table`, `embedded` | [`crate::table::lower_table`]: the report table of a generated page in its frame's list area; then per embedded block (by its rectangle): [`crate::embedded::lower_embedded`] (the cover's colour and standard-size tables, or the boxed report table of any other kind: PART DATA, the layout legend) |
//! | [`Layer::Surface`], [`Layer::Carriers`], [`Layer::Placements`] | `panel` | the surface view of a panel-layout page ([`crate::panel`]); the labels layer adds its placement labels after the texts above |
//! | others | — | empty: revision marks, highlights, features and dimensions arrive with their work packages |
//!
//! Panel-layout pages ([`PageKind::Panel`]) use [`Layer::PANEL`]; their surface, carrier, placement and label
//! layers draw the page's surface view, the feature and dimension layers stay empty (1.0).
//!
//! All geometry is in **page coordinates** (integer nm, y down). The form's page-to-sheet transform
//! ([`SheetMap`]) is applied once, by [`PageScene::sheet_list`], for every writer.
//!
//! # Rules
//!
//! - **Order.** Representations, connections and junctions are sorted by ID, whatever order the caller
//!   gives them in, so a record shuffle never changes a display list (AC-14).
//! - **CP positions** follow `ew-commands`' `rep_cps`: symbol CP `c` of a representation sits at
//!   `pos + orient(c.pos)`, on the grid's own number where that lies within 1 nm of a position of the page's grid
//!   (`Grid::settle_point`; this holds for every CP below), and draws the function CP of its `pin_map` entry, else `(functions[0], c.fn_cp)`;
//!   when several representations on the page draw the same function CP, the smallest representation ID
//!   wins.
//! - **Black boxes.** A representation whose symbol has no CPs and that carries `Representation.box` draws
//!   box CP `c` of its first function at `pos + orient(c.pos)`, leaving in `orient(c.dir)`.
//! - **Strip segments** (doc 08 §6.7, FR-TRM-011). The caller lists the terminals a segment draws
//!   ([`RepView::terminals`], in drawing order). With `g` the segment symbol's grid and `h` the bottom of
//!   its bounds in `R0` (snapped to `g`), terminal `k` (0-based) has its side-1 point at `(g·(k+1), 0)`,
//!   leaving `Up`, and its side-2 point at `(g·(k+1), h)`, leaving `Down`, in the symbol frame: one
//!   connection-grid step per terminal along the segment's x axis, starting one step in. The step vector
//!   turns with the orientation and lands on the exact grid point (no 1 nm rounding drift). Every function CP the caller lists for a side is drawn at that side's
//!   point; CPs without a side (shaft, rail foot) are not drawn.
//! - **Routes.** A connection with a stored route draws `[a, route…, b]`; without one it draws the
//!   unrouted line of [`crate::route`], dashed (the scene never routes, ADR-0033). Ends are clamped to
//!   the page. A connection whose end cannot be resolved on this page is reported and not drawn.
//! - **Strokes.** Wires are solid black in [`wire_stroke`] of the page's grid: on the QET 10 px grid the 1.0
//!   QET px pen [`QET_PEN_1PX`] (printed 0.261 mm through the QET frame's sheet map, like the frame and the
//!   `@qet10` symbols), else the 0.26 mm house pen (doc 20 §17.4).
//! - **Break marks** (doc 11 §5.5 step 5, FR-DEV-023, D-41). A [`BreakMarkView`] closes an assembly
//!   overview segment at one side of a representation's symbol bounds `b` (in its orientation): at
//!   `x = pos.x + b.max.x + BREAK_GAP` for the right side, `pos.x + b.min.x − BREAK_GAP` for the left, a vertical
//!   stroke from `b.min.y − BREAK_GAP` to `b.max.y + BREAK_GAP` (relative to `pos.y`) with a zig-zag of
//!   amplitude [`BREAK_ZIGZAG`] at its middle, in the page's [`wire_stroke`], tagged with the representation.
//!   A mark whose representation or symbol is not in the view draws nothing.
//! - **Junctions** draw the caller's junction symbol at `at` in the bevel's orientation; without a symbol a
//!   dot is a filled black disc of radius [`JUNCTION_DOT_RADIUS`].
//! - **Bridges** (doc 08 §6.5, FT-1908): the program's drawing of a strip's bridge, no record and nothing to select.
//!   Each clamp of a [`BridgeView`] that the page draws gives a point half way between the terminal's centre and the
//!   clamp's point, on the lead the bridge joins: the centre of a terminal drawn alone is its insertion point, that of
//!   a terminal a strip segment draws half way between its side-1 and side-2 points (FT-2020), so a segment's
//!   bridged terminals make the bus across its leads. Points whose clamps leave the same way and that lie on one line
//!   across those leads (one height for leads that leave up or down, one x for leads that leave left or right) are one
//!   row; a row of two or more is drawn as a straight line from its first point to its last in the page's
//!   [`wire_stroke`], and a dot of [`JUNCTION_DOT_RADIUS`] at each of its points, after the junctions. A terminal of
//!   the bridge in another row or on another page joins no line here; a terminal that is no part of the bridge and
//!   stands between two of the row is crossed without a dot.
//! - **Slot texts.** A slot is shown when its override says so, else when it belongs to the
//!   representation's arrangement (FR-SYM-012), else by the symbol's default; an empty value prints
//!   nothing. Position and angle overrides are in the symbol frame. Placement follows
//!   [`crate::symbol`] in the representation's text frame (`ew_derive::text_frame`): the position turns with the
//!   orientation, the angle stays the symbol frame's (a text stays upright), the alignment keeps the text's side
//!   (doc 07 §4.1). The text frame is the
//!   representation's own, except on a black box turned by a half turn without a mirror: its texts
//!   are placed as on the same box unturned (origin `2C − pos`, `C` the box's centre on the page, no turn), so they
//!   keep their place relative to the box and their collision rules; the style is the slot's own `style` when it names a known text style, else
//!   [`symbol::default_style_key`] of the slot kind (an unknown name is reported as
//!   [`PageFinding::UnknownSlotStyle`]). `max_width` is not applied yet.
//!   A text that lists several items carries one link per item ([`SlotValue::item_links`]): an invisible
//!   path item over the item's glyph span (the pen travel of the text up to the item's ends), after the text's
//!   own items, with the text's entity and a [`Link::Source`].
//! - **Where the texts stand** (docs/design/06a-drafting.md §5). The labels layer builds the page as the layout sees it
//!   ([`crate::draft`]) and places every text the program may place with [`ew_drafting::settle_labels`]: the slot
//!   texts that no override moves or turns, a black box's texts, the strip tags, the wire numbers and a cable mark's
//!   tag and type line. Each takes the first of its places (its slot as its author set it, then the places of its
//!   kind) that keeps clear of the other parts' bodies, the wires, the fixed texts and the texts placed before it, in
//!   the drawing area. A slot text that an override moves or turns, the pin texts, the plug notes, free texts, marker
//!   texts, core names and structure-box captions are fixed: they stand where they are, and the others keep clear of
//!   them. [`LayerList::draft`] is that page.
//! - **Pin texts**. A black box prints each [`RepView::pins`] entry whose index is one of its box CPs
//!   (drawn as in "Black boxes") in style `data.small`, inside the outline, [`PIN_TEXT_GAP`] from the pin along its
//!   inward normal: a pin leaving up or down reads bottom to top (90°), its end towards the pin (aligned middle right
//!   below a pin leaving up, middle left above a pin leaving down), but a text of signs only ("+", "−", no letter or
//!   digit: a turned "−" read as a tick) reads horizontally, centred on the pin at the same gap (aligned top
//!   centre below a pin leaving up, bottom centre above a pin leaving down); a pin leaving left or right reads
//!   horizontally, its start towards the pin (middle left right of a pin leaving left, middle right left of a pin
//!   leaving right); [`pin_text_place`], which the symbol editor also gives a new connection point's number text. They
//!   print after the slot texts and never move: they are fixed texts. An entry without its box
//!   CP, or a pin of an unknown direction, prints nothing.
//! - **Wire markers** (FR-OUT-011 keeps them in monochrome; FT-1706: the text stands clear of its wires, as drawing
//!   practice has it): a short line across the wire at the midpoint of the polyline's longest segment (the first of
//!   equal ones), [`MARKER_REACH`] past the wire on both sides in the page's wire stroke, and the caller's marker text
//!   in style `marker` [`MARKER_GAP`] beyond the line's end, horizontal: right of the line across a vertical wire
//!   (aligned middle left), above the line across a horizontal wire (aligned bottom centre). A wire-marker group
//!   ([`crate::items`]) draws the same line across all its wires.
//! - **Cable marks** (as panel builders draw a cable): a wire-marker group whose connections are all cores or
//!   bus lines of one cable ([`ew_derive::CableMark`]) is drawn whatever the marker mode: the line across the wires it
//!   crosses, [`MARKER_REACH`] past the outer ones; on each core a slanted stroke (`/`, [`CABLE_TICK`] each way); each
//!   core's designation (`1`, `BN`, `GNYE`; a bus line has none) in style `marker` just past the stroke's upper end
//!   (aligned bottom left); the cable's tag in style [`CABLE_TAG_STYLE`] and its type line in [`CABLE_KIND_STYLE`]
//!   (`TYPE 3G1.5`) where the text placer sets them (the type before the line, the tag after it), or the tag at the
//!   marker's `tag_at` once moved, with its type line under it. Every text is upright.
//! - **Wire numbers** (FT-1692): a connection's [`ConnectionView::number`] prints once, in style `marker`, along its
//!   wire where the text placer sets it: right of a vertical stretch turned to read from the bottom, above a
//!   horizontal one; beside the dashed line of an unrouted wire; with a marker of its own it is that marker's text,
//!   the number first (`101.1 1.5 BK`). A bus line whose cable has no mark on the page prints the cable's tag there in
//!   style [`CABLE_TAG_STYLE`] (it was too small to read).
//! - **Stored symbol variants** (`Representation.variant`, contact-image variants) are not drawn yet: the
//!   main graphics are used, and the geometry key's `extra` is `None`.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use ew_dl::{
    Brush, DashPattern, Destination, DisplayList, EntityRef, Item, ItemKind, Layer, Link, Path, Pen, Stroke,
    SymbolGeomId, Transform,
};
use ew_ids::{Id, Uuid};
use ew_mltext::LangTag;
use ew_schema::{
    Align, Connection, ConnectionKind, Dir4, Endpoint, FormDef, Function, Junction, JunctionStyle, LineKind,
    ListArea, Page, Representation, SymbolDef,
};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, LineSpan, Shaped, TextStyle, TextStyleTable, VerticalMetrics};
use ew_units::{Angle2, Grid, Nm, Orient2, Pt2, Rect};

use crate::embedded::{EmbeddedView, lower_embedded};
use crate::forms::{
    BoxSpec, FormFinding, FrameGeometry, PageBindings, SheetMap, lower_form, ml_text, place, split_align,
};
use crate::hops::{Crossings, hop_path, hops};
use crate::items::{ItemView, StructureBoxView};
use crate::panel::PanelView;
use crate::route::{RouteEnd, connection_polyline};
use crate::symbol::{self, TextSource};
use crate::table::{TableFinding, TableView, lower_table};

/// The radius of a junction dot drawn without a junction symbol: 1.25 QET px (0.33 mm), the radius of
/// `ew.gen.junction-dot@qet10`.
pub const JUNCTION_DOT_RADIUS: Nm = Nm(330_729);

/// The 1.0 QET px pen (`7_937_500 / 30` nm, rounded): wires on QET-grid pages and the `@qet10` edition's
/// replacement of the 0.26 mm house pen, so that after the QET frame's sheet map it prints at 0.261 mm
/// (doc 20 §17.4).
pub const QET_PEN_1PX: Nm = Nm(264_583);

/// The wire pen of a page with connection grid `grid`: [`QET_PEN_1PX`] on the QET 10 px grid, else the
/// 0.26 mm house pen (doc 20 §17.4; `@qet10` symbols carry the same QET-px pens).
pub fn wire_stroke(grid: Grid) -> Stroke {
    if grid == Grid::QET_10PX {
        Stroke::solid(QET_PEN_1PX)
    } else {
        Stroke::pen(Pen::P026)
    }
}

/// The sheet used when a page has no frame form: A4 landscape (297 mm × 210 mm).
pub const DEFAULT_SHEET: (Nm, Nm) = (Nm(297_000_000), Nm(210_000_000));

/// The text style of wire-marker texts (doc 20 §17.5).
pub const MARKER_STYLE: &str = "marker";

/// How far a wire marker's line reaches past its outer wires (FT-1706).
pub const MARKER_REACH: Nm = Nm(1_500_000);

/// The gap between a wire marker's line and its text (FT-1706).
pub const MARKER_GAP: Nm = Nm(1_000_000);

/// The text style of a cable mark's tag: the device tag's.
pub const CABLE_TAG_STYLE: &str = "tag.main";

/// The text style of a cable mark's type line.
pub const CABLE_KIND_STYLE: &str = "data.small";

/// Half the run of a cable mark's slanted stroke on a core, along and across the line.
pub const CABLE_TICK: Nm = Nm(1_000_000);

/// The gap between a cable mark's stroke or line and the texts beside them.
pub const CABLE_TEXT_GAP: Nm = Nm(500_000);

/// Where a wire marker's text stands beyond its line (FT-1706).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MarkerText {
    /// The line is horizontal (across vertical wires): the text reads horizontally right of its right end.
    Right,
    /// The line is vertical (across horizontal wires): the text reads horizontally above its top end.
    Above,
    /// The line is vertical: the text reads bottom to top (90°) upwards from its top end.
    Up,
}

/// The coordinate where `pts` crosses the line through `at` that runs horizontally (`horizontal`) or vertically: the
/// crossing's `x` on a horizontal line, its `y` on a vertical one, the one nearest `at` of several; `None` when no
/// segment crosses it (a segment lying on the line does not).
pub(crate) fn crossing_of(pts: &[Pt2], at: Pt2, horizontal: bool) -> Option<Nm> {
    // `(along the line, across it)` of a point.
    let split = |p: Pt2| {
        if horizontal {
            (i128::from(p.x.0), i128::from(p.y.0))
        } else {
            (i128::from(p.y.0), i128::from(p.x.0))
        }
    };
    let (target, line) = split(at);
    let mut best: Option<i128> = None;
    for w in pts.windows(2) {
        let ((a_al, a_ac), (b_al, b_ac)) = (split(w[0]), split(w[1]));
        if a_ac == b_ac || line < a_ac.min(b_ac) || line > a_ac.max(b_ac) {
            continue;
        }
        let x = a_al + (line - a_ac) * (b_al - a_al) / (b_ac - a_ac);
        if best.is_none_or(|b| (x - target).abs() < (b - target).abs()) {
            best = Some(x);
        }
    }
    best.and_then(|x| i64::try_from(x).ok()).map(Nm)
}

/// A symbol copy and its ID (the key of its geometry).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolRef {
    /// The symbol copy's ID.
    pub id: Id<SymbolDef>,
    /// The definition.
    pub def: Arc<SymbolDef>,
}

/// The frame of a page: its frame form, the title block that form names, and the title-block bindings
/// (`title_block(page)` mapped to [`PageBindings`] by the caller).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FrameView {
    /// The frame form.
    pub form: Arc<FormDef>,
    /// The title-block form `form.title_block` names, if found.
    pub title_block: Option<Arc<FormDef>>,
    /// The placeholder values.
    pub bindings: PageBindings,
}

/// The derived value of one text slot of a representation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotValue {
    /// The slot index in the symbol's `text_slots`.
    pub slot: u16,
    /// The printed text.
    pub text: String,
    /// Where the whole text links to, if anywhere (the text items carry the link). Exclusive with
    /// [`item_links`](SlotValue::item_links): a value sets one of the two kinds, never both.
    pub link: Option<Destination>,
    /// One link per listed item of the text (a cross-reference list): each covers the glyph span of
    /// its item, as an invisible path item over the item's box next to the text items. Exclusive with
    /// [`link`](SlotValue::link). A value that sets both is reported as [`PageFinding::ConflictingLinks`]
    /// and prints with its item links only.
    pub item_links: Vec<ItemLink>,
}

/// The link of one item of a slot text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ItemLink {
    /// The item's byte range in [`SlotValue::text`].
    pub range: Range<usize>,
    /// Where the item links to.
    pub dest: Destination,
}

/// One representation on the page with what it needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RepView {
    /// The record.
    pub rep: Arc<Representation>,
    /// The symbol copy `rep.symbol` names; `None` if it is missing (reported, nothing drawn).
    pub symbol: Option<Arc<SymbolDef>>,
    /// The derived slot texts; a slot without a value prints nothing.
    pub slots: Vec<SlotValue>,
    /// For a strip-segment representation ([`ew_schema::RepType::StripSegment`]): the terminals it draws, in drawing
    /// order (doc 08 §6.7, FR-TRM-011); empty otherwise. See the module documentation for their CPs.
    pub terminals: Vec<SegmentTerminal>,
    /// For a black box: the derived texts printed beside its pins (the CP designations and the PLC
    /// addresses); empty otherwise. See the module documentation ("Pin texts").
    pub pins: Vec<PinText>,
}

/// A derived text printed beside one pin of a black box: the designation of function CP `idx`
/// of the box's first function and, when that CP is bound to a PLC channel, the channel's printed address after one
/// space. Never stored (FR-PLC-007).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinText {
    /// The function CP index (a box CP's `idx`).
    pub idx: u16,
    /// The text.
    pub text: String,
    /// For a channel pin of a PLC card drawn in one row (doc 10 §3.6, "A card drawn in cells"): the channel's function
    /// text, empty when it has none; the pin's texts then print level in its cell. `None` for every other pin.
    pub cell: Option<String>,
}

/// One terminal drawn by a strip segment: its function and the function CPs of each side.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SegmentTerminal {
    /// The terminal's function.
    pub function: Id<Function>,
    /// The function CPs on side 1 (all drawn at the terminal's side-1 point).
    pub side1: Vec<u16>,
    /// The function CPs on side 2 (all drawn at the terminal's side-2 point).
    pub side2: Vec<u16>,
}

/// One connection drawn on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConnectionView {
    /// The record.
    pub connection: Arc<Connection>,
    /// The wire-marker text, when the connection shows a marker.
    pub marker: Option<String>,
    /// The wire's printed number, when the project prints wire numbers (FT-1692).
    pub number: Option<String>,
}

/// One junction on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionView {
    /// The record.
    pub junction: Arc<Junction>,
    /// The symbol of its style (`ew.gen.junction-dot` or `ew.gen.junction-bevel`), if available.
    pub symbol: Option<SymbolRef>,
}

/// What one valve position of a module's overview glyph shows (doc 11 §5.5 step 3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ValveGlyph {
    /// A valve: a rectangle with a circle and the valve part's part number; the entity is `v5(rep, valve)`.
    Valve {
        /// The entity reference.
        entity: Uuid,
        /// The part number printed in the glyph.
        part_number: String,
    },
    /// A blanking plate: the rectangle with both diagonals; the entity is `v5(rep, plate)`.
    Blank {
        /// The entity reference.
        entity: Uuid,
    },
    /// An empty position: the rectangle dashed; the entity is `v5(rep, "position" ␟ p)`.
    Empty {
        /// The entity reference.
        entity: Uuid,
    },
}

/// The valve glyphs of one overview representation of a module (`valve_slots`): one per position, the
/// p-th from the left, in a band above the module glyph.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ValveGlyphsView {
    /// The module's overview representation.
    pub rep: Id<Representation>,
    /// The positions 1 … n in order.
    pub glyphs: Vec<ValveGlyph>,
}

/// The text style of a valve glyph's part number: the CP-label style.
pub const VALVE_GLYPH_STYLE: &str = "cp.small";

/// What one cell of a valve terminal drawn as its positions shows (doc 11 §3.2); everything a cell draws is the
/// box representation's entity, so that a click or a right-click on a cell reaches the terminal's box.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CellView {
    /// A valve: its own symbol stands in the cell; `elsewhere` is its tag when no symbol of it stands on this page.
    Valve {
        /// The tag of a valve not drawn on this page.
        elsewhere: Option<String>,
        /// Where such a valve is drawn: the place its symbol stands at on another page (`3-B2`) and that
        /// symbol, which the place links to; `None` for a valve drawn nowhere.
        drawn_at: Option<(String, Id<Representation>)>,
    },
    /// A blanking plate, drawn as one: a plate with both diagonals.
    Blank,
    /// An empty position: a dashed outline.
    Empty,
}

/// The text beside one pin of a multipin valve terminal's plug: what the pin carries (`-201K2-Q1 14`, `1 · 14`,
/// `COM`), read along the wire that leaves the pin, just beside it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlugNoteView {
    /// The plug's representation (the text's entity).
    pub rep: Id<Representation>,
    /// The pin's point on the page.
    pub at: Pt2,
    /// Whether a wire leaves the pin vertically (up or down), else horizontally.
    pub vertical: bool,
    /// Whether the wire leaves towards larger coordinates (down, or right).
    pub forward: bool,
    /// The text.
    pub text: String,
}

/// The cells of one valve terminal's box drawn as its positions (`BoxSpec.slots`): the sub-base along the bottom with
/// its galleries and each cell's number in it, the supply end on the left, and per cell what it holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ValveCellsView {
    /// The box representation.
    pub rep: Id<Representation>,
    /// Its geometry on the page.
    pub frame: ew_derive::SlotFrame,
    /// The cells, left to right: each with its printed number (the position on its module) and what it holds.
    pub cells: Vec<(u32, CellView)>,
    /// Whether a supply plate stands in the supply end; without, the end is drawn dashed.
    pub supply: bool,
    /// The valves' ports that end on the sub-base: each port's x on the page and the gallery it joins (its
    /// index in `ew_derive::GALLERY_PORTS`).
    pub taps: Vec<(Nm, usize)>,
}

/// The side of a representation a break mark closes (see [`BreakMarkView`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum BreakSide {
    /// Before the representation: the overview continues from an earlier page.
    Left,
    /// After the representation: the overview continues on a later page.
    Right,
}

/// A break mark of an assembly overview that continues on another page (the caller derives them,
/// `ew_derive::AssemblyBreakMarks`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BreakMarkView {
    /// The representation at whose side the mark is drawn.
    pub rep: Id<Representation>,
    /// The side.
    pub side: BreakSide,
}

/// The gap between a symbol's bounds and its break mark, and the mark's overhang: 1 mm.
pub const BREAK_GAP: Nm = Nm(1_000_000);

/// The amplitude of a break mark's zig-zag: 1.5 mm.
pub const BREAK_ZIGZAG: Nm = Nm(1_500_000);

/// Which layer set a page has (ADR-0015 amendment 2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PageKind {
    /// Schematic and report pages: [`Layer::SCHEMATIC`].
    #[default]
    Schematic,
    /// Panel-layout pages: [`Layer::PANEL`].
    Panel,
}

impl PageKind {
    /// The layers of this kind of page, in paint order.
    pub const fn layers(self) -> &'static [Layer] {
        match self {
            PageKind::Schematic => &Layer::SCHEMATIC,
            PageKind::Panel => &Layer::PANEL,
        }
    }
}

/// Everything [`page_display_list`] reads for one page, filled by the caller.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageView {
    /// The page (a generated page uses its v5 ID).
    pub page: Id<Page>,
    /// Its layer set.
    pub kind: PageKind,
    /// The page's connection grid (`Page.grid`; [`Grid::MM4`] for generated pages and dividers). It
    /// chooses the wire pen: [`QET_PEN_1PX`] on the QET 10 px grid, the 0.26 mm house pen otherwise.
    pub grid: Grid,
    /// The display language of static multi-language symbol texts.
    pub language: Option<LangTag>,
    /// The frame; `None` draws no frame on a [`DEFAULT_SHEET`].
    pub frame: Option<FrameView>,
    /// The representations on the page.
    pub reps: Vec<RepView>,
    /// The connections drawn on the page.
    pub connections: Vec<ConnectionView>,
    /// The junctions on the page.
    pub junctions: Vec<JunctionView>,
    /// The report table of a generated page; `None` on every other page.
    pub table: Option<TableView>,
    /// The embedded report blocks of a drawn page (FR-REP-028), drawn into the `tables` layer.
    pub embedded: Vec<EmbeddedView>,
    /// The break marks of assembly overviews on this page (doc 11 §5.5, D-41), drawn in [`Layer::Symbols`].
    pub break_marks: Vec<BreakMarkView>,
    /// The valve glyphs of the module overviews on this page, drawn in [`Layer::Symbols`] after the
    /// break marks.
    pub valve_glyphs: Vec<ValveGlyphsView>,
    /// The cells of the valve terminals drawn as their positions on this page, drawn in [`Layer::Symbols`] after the
    /// valve glyphs.
    pub valve_cells: Vec<ValveCellsView>,
    /// The texts beside the pins of the multipin valve terminals' plugs on this page, drawn in
    /// [`Layer::Symbols`] after the valve cells.
    pub plug_notes: Vec<PlugNoteView>,
    /// The connection points of this page that a link of the export points at (a PLC overview row links to
    /// its coil, doc 10 §5): each gets a named destination ([`Destination::Cp`]) at its drawn position, else
    /// at its representation's position (a CP the symbol does not draw, such as the internal coil 14).
    pub cp_targets: Vec<CpTarget>,
    /// How crossing wires are drawn (FR-CON-031): the project's drawing setting; the default draws
    /// them plain, byte-equal to a project without the setting.
    pub crossings: Crossings,
    /// The page items of a drawn page with what they print: graphics in [`Layer::Symbols`],
    /// free texts and wire-marker groups in [`Layer::Labels`] ([`crate::items`]).
    pub items: Vec<ItemView>,
    /// The structure boxes of a drawn page with their captions: outlines in
    /// [`Layer::Symbols`], captions in [`Layer::Labels`].
    pub structure_boxes: Vec<StructureBoxView>,
    /// The surface view of a panel-layout page ([`crate::panel`]): drawn into [`Layer::Surface`],
    /// [`Layer::Carriers`], [`Layer::Placements`] and [`Layer::Labels`]; `None` on every other page.
    pub panel: Option<PanelView>,
    /// The strip tags printed at terminals drawn alone (doc 08 §6.7), drawn into
    /// [`Layer::Labels`] after the slot and pin texts.
    pub strip_tags: Vec<StripTagView>,
    /// The boxes of the texts a line of the page runs through, which [`Layer::Wires`] cuts its lines round
    /// ([`crate::knockout`], FT-1786): the labels layer's [`LayerList::knockouts`], filled by the caller; empty draws
    /// every line whole.
    pub knockouts: Vec<Rect>,
    /// The bridges of the strips whose terminals the page draws, drawn into [`Layer::Wires`] after the junctions
    /// where their terminals stand in one row (module documentation, "Bridges").
    pub bridges: Vec<BridgeView>,
}

/// A strip's bridge (doc 08 §6.5, FT-1908): the clamp it joins on each of its terminals, by the function and CP index
/// its connections name there, in the strip's order (`ew_derive::StripBridges`, one bridge per group).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BridgeView {
    /// The clamps, one per terminal.
    pub clamps: Vec<(Id<Function>, u16)>,
}

/// A strip's tag printed once on a page that shows its terminals without a segment (doc 08 §6.7): at the
/// tag-slot anchor of `rep`, the strip's first terminal representation on the page whose tag slot is shown
/// (`ew_derive::tag_slot_shown`), moved one connection-grid step up in the text's own frame, in the slot's
/// style and horizontal alignment, bottom-aligned.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StripTagView {
    /// The terminal representation it stands at.
    pub rep: Id<Representation>,
    /// The strip's displayed tag.
    pub text: String,
}

/// A connection point that is a link destination on its page (see [`PageView::cp_targets`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct CpTarget {
    /// The representation that draws the CP's function on the page.
    pub rep: Id<Representation>,
    /// The function.
    pub function: Id<Function>,
    /// The CP index.
    pub idx: u16,
}

/// Something worth reporting while building a page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PageFinding {
    /// A finding of the frame or title block.
    Form(FormFinding),
    /// A representation whose symbol copy is missing; nothing is drawn for it.
    MissingSymbol {
        /// The representation.
        rep: Id<Representation>,
    },
    /// A connection end that is not drawn on this page; the connection is not drawn.
    UnresolvedEnd {
        /// The connection.
        connection: Id<Connection>,
    },
    /// A style key not in the style table; `text.normal` was used.
    UnknownStyle {
        /// The entity the text belongs to.
        entity: Uuid,
        /// The key.
        key: String,
    },
    /// A slot value that sets both a whole-text link and item links; the item links were used.
    ConflictingLinks {
        /// The entity the text belongs to.
        entity: Uuid,
        /// The index of the slot in the symbol's `text_slots`.
        slot: u16,
    },
    /// A text slot whose `style` names no text style; the style of the slot's kind was used.
    UnknownSlotStyle {
        /// The entity the text belongs to.
        entity: Uuid,
        /// The symbol of the representation.
        symbol: Uuid,
        /// The index of the slot in the symbol's `text_slots`.
        slot: u16,
        /// The unknown style name.
        name: String,
    },
    /// Characters without a glyph in the font set (FR-OUT-027).
    MissingGlyphs {
        /// The entity the text belongs to.
        entity: Uuid,
        /// The text.
        text: String,
        /// The characters, each once.
        chars: Vec<char>,
    },
    /// A finding of the report table.
    Table(TableFinding),
    /// An image item the export job could not draw: its bytes are not held, or its SVG is refused (the
    /// job adds these after resolving the sheet list, `crate::images::resolve_images`).
    Image(crate::images::ImageIssue),
}

/// One layer of one page: the display list in page coordinates, the symbol geometry its instances name,
/// and the findings.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LayerList {
    /// The items in paint order, in page coordinates.
    pub list: DisplayList,
    /// `symbol_geometry(symbol, variant)` of every instance in `list`.
    pub geometry: BTreeMap<SymbolGeomId, Arc<DisplayList>>,
    /// What to report, in paint order.
    pub findings: Vec<PageFinding>,
    /// The labels layer's anchor of each representation's device tag where it prints, where the text placer
    /// moved it, by representation ID: what `TableRep::tag_at` reports. Empty in the other
    /// layers.
    pub tags: BTreeMap<Uuid, Pt2>,
    /// The labels layer's box of each placement's label on a panel-layout page where it prints (its knockout box,
    /// `crate::panel`), by placement ID: what a drag of the label grabs. Empty in the other layers and pages.
    pub panel_labels: BTreeMap<Uuid, Rect>,
    /// The labels layer's slot texts where they print, in print order: what a drag of a symbol's text grabs; a cable
    /// mark's tag is one too, of its wire marker (slot 0). Empty in the other layers.
    pub slot_texts: Vec<SlotTextBox>,
    /// The labels layer's page as the layout sees it ([`crate::draft`]), whose labels it placed; `None` in the other
    /// layers and on a panel-layout page.
    pub draft: Option<Arc<ew_drafting::DraftPage>>,
    /// The labels layer's boxes of the texts a line of the page runs through, in print order: what the wires layer cuts
    /// its lines round ([`PageView::knockouts`], [`crate::knockout`]). Empty in the other layers.
    pub knockouts: Vec<Rect>,
}

/// A representation's slot text where the labels layer prints it, or a cable mark's tag (`rep` the wire marker, `slot`
/// 0, `frame` the marker's point unturned).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotTextBox {
    /// The representation (a cable mark's wire marker).
    pub rep: Uuid,
    /// The symbol's slot index.
    pub slot: u16,
    /// The text's anchor on the page.
    pub at: Pt2,
    /// The angle it prints at.
    pub angle: Angle2,
    /// Whether the representation's override moves or turns it.
    pub moved: bool,
    /// Whether it is a pin's label (the slot of a connection point's designation).
    pub pin: bool,
    /// Its box on the page.
    pub rect: Rect,
    /// The representation's text frame (`ew_derive::text_frame`): a page point `p` is `frame.orient⁻¹(p − origin)` in
    /// the symbol's coordinates, where a slot override's position is.
    pub frame: ew_derive::TextFrame,
}

/// `page_display_list(page, layer, lang)` for a drawn page (see the module documentation). The language is
/// the view's; the result is deterministic and independent of the order of the view's records. Text style keys
/// resolve through `styles`, the project's effective table (ADR-0032: the caller builds it from
/// `ew_derive::text_styles`; `ew_text::builtin()` for a caller without a project).
pub fn page_display_list(
    view: &PageView,
    layer: Layer,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> LayerList {
    let mut b = Builder {
        fonts,
        styles,
        out: LayerList::default(),
    };
    match layer {
        Layer::Frame => b.frame(view),
        Layer::Symbols => b.symbols(view),
        Layer::Wires => b.wires(view),
        Layer::Labels => {
            b.labels(view);
            b.panel_labels(view);
        }
        Layer::Tables => b.tables(view),
        Layer::Surface => b.panel_surface(view),
        Layer::Carriers => b.panel_carriers(view),
        Layer::Placements => b.panel_placements(view),
        _ => {}
    }
    b.out
}

/// The symbol geometry of `id` in `variant`: the lowered graphics of [`symbol::lower`] (ADR-0015's
/// `symbol_geometry(symbol, variant)`; texts are placed per representation).
pub fn symbol_geometry(def: &SymbolDef, variant: Orient2) -> Arc<DisplayList> {
    Arc::new(symbol::lower(def, variant).graphics)
}

/// The page rectangle in page coordinates: the frame's outer rectangle, else the form's sheet, else
/// [`DEFAULT_SHEET`].
pub fn page_rect(view: &PageView) -> Rect {
    let sheet = |w: Nm, h: Nm| Rect::from_points(Pt2::ORIGIN, Pt2::new(w, h));
    match &view.frame {
        Some(f) => FrameGeometry::of(&f.form, f.title_block.as_deref())
            .map_or_else(|| sheet(f.form.sheet.width, f.form.sheet.height), |g| g.outer),
        None => sheet(DEFAULT_SHEET.0, DEFAULT_SHEET.1),
    }
}

/// The page's drawing area in page coordinates: the frame's drawing area inside its grid header, else
/// [`page_rect`].
pub(crate) fn drawing_area(view: &PageView) -> Rect {
    view.frame
        .as_ref()
        .and_then(|f| FrameGeometry::of(&f.form, f.title_block.as_deref()))
        .map_or_else(|| page_rect(view), |g| g.drawing_area)
}

/// All layers of one page, ready for the writers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageScene {
    /// The page.
    pub page: Id<Page>,
    /// The paper width.
    pub sheet_width: Nm,
    /// The paper height.
    pub sheet_height: Nm,
    /// The page-to-sheet map of the frame form.
    pub map: SheetMap,
    /// The page's layers in paint order, in page coordinates.
    pub layers: Vec<(Layer, Arc<DisplayList>)>,
    /// The symbol geometry of every layer's instances.
    pub geometry: BTreeMap<SymbolGeomId, Arc<DisplayList>>,
    /// The findings of every layer, in paint order.
    pub findings: Vec<PageFinding>,
}

/// Every layer of `view`'s page kind, with the sheet size and map of its frame form; text styles resolve through
/// `styles` ([`page_display_list`]).
pub fn page_scene(view: &PageView, fonts: &FontSet, styles: &TextStyleTable) -> PageScene {
    let mut layers = Vec::new();
    let mut geometry = BTreeMap::new();
    let mut findings = Vec::new();
    for &layer in view.kind.layers() {
        let l = page_display_list(view, layer, fonts, styles);
        geometry.extend(l.geometry);
        findings.extend(l.findings);
        layers.push((layer, Arc::new(l.list)));
    }
    let (sheet_width, sheet_height, map) = match &view.frame {
        Some(f) => (f.form.sheet.width, f.form.sheet.height, SheetMap::of(&f.form)),
        None => (DEFAULT_SHEET.0, DEFAULT_SHEET.1, SheetMap::IDENTITY),
    };
    PageScene {
        page: view.page,
        sheet_width,
        sheet_height,
        map,
        layers,
        geometry,
        findings,
    }
}

impl PageScene {
    /// The exported layers (every layer but [`Layer::Highlights`]) in paint order, mapped onto the sheet
    /// with [`SheetMap::apply_display_list`] and their symbol instances inlined: the one list every writer
    /// draws (PNG, PDF, print).
    pub fn sheet_list(&self) -> DisplayList {
        let mut out = DisplayList::new();
        for (layer, list) in &self.layers {
            if layer.is_exported() {
                out.items
                    .extend(self.map.apply_display_list(list, Some(&self.geometry)).items);
            }
        }
        out
    }

    /// The layer `layer`, if the page has it.
    pub fn layer(&self, layer: Layer) -> Option<&Arc<DisplayList>> {
        self.layers.iter().find(|(l, _)| *l == layer).map(|(_, d)| d)
    }
}

/// Shapes a symbol-editor text with the same styles, fonts, alignment and readable rotation as page placement.
/// Dynamic slot values are supplied by the editor's preview data.
pub fn symbol_preview_text(
    text: &symbol::PlacedText,
    content: &str,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> LayerList {
    let mut builder = Builder {
        fonts,
        styles,
        out: LayerList::default(),
    };
    if text.visible {
        builder.text(&TextSpec {
            text: content,
            at: text.pos,
            angle: text.angle,
            align: &text.align,
            style_key: &text.style_key,
            entity: EntityRef(Uuid::nil()),
            link: None,
            items: &[],
        });
    }
    builder.out
}

pub(crate) struct Builder<'a> {
    pub(crate) fonts: &'a FontSet,
    pub(crate) styles: &'a TextStyleTable,
    pub(crate) out: LayerList,
}

impl<'a> Builder<'a> {
    /// A builder of an empty layer list.
    pub(crate) fn new(fonts: &'a FontSet, styles: &'a TextStyleTable) -> Builder<'a> {
        Builder {
            fonts,
            styles,
            out: LayerList::default(),
        }
    }

    /// The layer list built.
    pub(crate) fn finish(self) -> LayerList {
        self.out
    }
}

/// A drawn CP: its position and the direction a wire leaves it.
pub(crate) type CpMap = BTreeMap<(Uuid, u16), (Pt2, Dir4)>;

/// The point of every function CP the page draws, by `(function, CP index)`: the CP positions of the page scene
/// without the directions. `ew-derive`'s `cp_point` restates these rules over `Q` (the wire numbers order
/// wires by the cell of their ends), and a test holds the two equal on the sample's pages.
pub fn cp_points(view: &PageView) -> BTreeMap<(Uuid, u16), Pt2> {
    cp_map(view).into_iter().map(|(k, (pos, _))| (k, pos)).collect()
}

pub(crate) fn sorted_reps(view: &PageView) -> Vec<&RepView> {
    let mut reps: Vec<&RepView> = view.reps.iter().collect();
    reps.sort_by_key(|r| r.rep.id);
    reps
}

pub(crate) fn sorted_connections(view: &PageView) -> Vec<&ConnectionView> {
    let mut c: Vec<&ConnectionView> = view.connections.iter().collect();
    c.sort_by_key(|c| c.connection.id);
    c
}

/// The drawn function CPs of the page (see the module documentation).
pub(crate) fn cp_map(view: &PageView) -> CpMap {
    drawn_cps(view)
        .into_iter()
        .map(|(k, (pos, dir, _))| (k, (pos, dir)))
        .collect()
}

/// [`cp_map`] with the representation that draws each CP.
fn drawn_cps(view: &PageView) -> BTreeMap<(Uuid, u16), (Pt2, Dir4, Uuid)> {
    let mut map: BTreeMap<(Uuid, u16), (Pt2, Dir4, Uuid)> = BTreeMap::new();
    for rv in sorted_reps(view) {
        let Some(def) = &rv.symbol else { continue };
        let rep = &rv.rep;
        let mut put = |function: Id<Function>, idx: u16, offset: Pt2, dir: &Dir4| {
            let pos = view.grid.settle_point(rep.pos + rep.orient.apply(offset));
            let dir = symbol::transform_dir(dir, rep.orient);
            map.entry((function.as_uuid(), idx))
                .or_insert((pos, dir, rep.id.as_uuid()));
        };
        for cp in def.cps.iter() {
            let (function, idx) = match rep.pin_map.iter().find(|p| p.symbol_cp == cp.idx) {
                Some(p) => (p.function, p.idx),
                None => match rep.functions.first() {
                    Some(f) => (*f, cp.fn_cp),
                    None => continue,
                },
            };
            put(function, idx, cp.pos, &cp.dir);
        }
        // A black box: the CPs of `Representation.box`, when the symbol has none.
        if def.cps.is_empty()
            && let (Some(b), Some(&function)) = (&rep.box_, rep.functions.first())
        {
            for cp in &b.cps {
                put(function, cp.idx, cp.pos, &cp.dir);
            }
        }
        // A strip segment: one point per drawn terminal and side.
        if !rv.terminals.is_empty() {
            let grid = def.grid;
            let bottom = grid.snap(symbol::lower(def, Orient2::R0).bounds.max().y);
            for (k, t) in rv.terminals.iter().enumerate() {
                let Ok(step) = i64::try_from(k + 1) else { break };
                let points = (
                    segment_point(grid, rep, step, 0),
                    segment_point(grid, rep, step, bottom),
                );
                let (Some(side1), Some(side2)) = points else { break };
                for (idx, at, dir) in t
                    .side1
                    .iter()
                    .map(|&i| (i, side1, Dir4::Up))
                    .chain(t.side2.iter().map(|&i| (i, side2, Dir4::Down)))
                {
                    let dir = symbol::transform_dir(&dir, rep.orient);
                    map.entry((t.function.as_uuid(), idx)).or_insert((
                        view.grid.settle_point(at),
                        dir,
                        rep.id.as_uuid(),
                    ));
                }
            }
        }
    }
    map
}

/// The page point `(i, j)` grid steps from a strip segment's origin in its symbol frame. The step vector
/// turns with the orientation; on a grid-point origin the result is the exact grid point (the rounding
/// of each absolute coordinate, so no 1 nm drift), else the origin plus the rounded offset. `None` on
/// overflow.
fn segment_point(grid: Grid, rep: &Representation, i: i64, j: i64) -> Option<Pt2> {
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

pub(crate) fn polyline_of(view: &PageView, cps: &CpMap, c: &Connection, page: Rect) -> Option<Vec<Pt2>> {
    let junction_at = |j: Id<Junction>| {
        view.junctions
            .iter()
            .find(|jv| jv.junction.id == j)
            .map(|jv| jv.junction.at)
    };
    let end = |e: &Endpoint| match e {
        Endpoint::Cp(f, idx) => cps.get(&(f.as_uuid(), *idx)).map(|(pos, dir)| RouteEnd {
            pos: *pos,
            dir: Some(dir.clone()),
        }),
        Endpoint::Junction(j) => junction_at(*j).map(|pos| RouteEnd { pos, dir: None }),
        Endpoint::Unknown(_) => None,
    };
    let (a, b) = (end(&c.a)?, end(&c.b)?);
    Some(connection_polyline(&a, &b, &c.route, page))
}

impl Builder<'_> {
    pub(crate) fn push(&mut self, item: Item) {
        self.out.list.push(item);
    }

    fn frame(&mut self, view: &PageView) {
        let Some(f) = &view.frame else { return };
        let lowered = lower_form(
            &f.form,
            f.title_block.as_deref(),
            &f.bindings,
            self.fonts,
            self.styles,
        );
        self.out.list = lowered.display_list;
        self.out
            .findings
            .extend(lowered.findings.into_iter().map(PageFinding::Form));
    }

    fn tables(&mut self, view: &PageView) {
        let sheet = view
            .frame
            .as_ref()
            .map_or(SheetMap::IDENTITY, |f| SheetMap::of(&f.form));
        if let Some(table) = &view.table {
            let (caption, legend) = (table.caption.is_some(), table.legend.is_some());
            let area = view.frame.as_ref().map_or_else(
                || ListArea::house(caption, legend),
                |f| f.form.list_area(f.title_block.as_deref(), caption, legend),
            );
            let lowered = lower_table(table, &area, sheet, self.fonts, self.styles);
            for item in lowered.items {
                self.push(item);
            }
            self.out
                .findings
                .extend(lowered.findings.into_iter().map(PageFinding::Table));
        }
        // The embedded blocks in the order of their rectangles, whatever order the caller gives them in.
        let mut embedded: Vec<&EmbeddedView> = view.embedded.iter().collect();
        embedded.sort_by_key(|e| (e.rect.min().y, e.rect.min().x, e.rect.max().y, e.rect.max().x));
        for e in embedded {
            let lowered = lower_embedded(e, sheet, self.fonts, self.styles);
            for item in lowered.items {
                self.push(item);
            }
            self.out
                .findings
                .extend(lowered.findings.into_iter().map(PageFinding::Table));
        }
    }

    /// The representations' symbols. **A black box drawn with its stored outline** (FR-DEV-039): when
    /// `Representation.box` is set and its outline differs from the symbol's rectangle (its first `rect` primitive),
    /// the rectangle is replaced by the outline as a closed polygon in the rectangle's stroke and fill, under a geometry
    /// key of its own (`SymbolGeomId.extra`: a 16-bit value derived from the outline, probed upwards past a key that
    /// holds other geometry), so it is never shared with another box of that symbol. A box whose outline equals the
    /// rectangle keeps the symbol's own instance (the output is byte-equal). The text slots stay
    /// where the symbol puts them (a follow-up moves them with a wider box).
    fn symbols(&mut self, view: &PageView) {
        /// The black-box symbol of `rep` with its rectangle replaced by the stored outline; `None` when the
        /// representation has no box, the symbol no rectangle, or the outline is that rectangle.
        fn boxed(def: &SymbolDef, rep: &Representation) -> Option<SymbolDef> {
            let b = rep.box_.as_ref()?;
            let (i, r) = def.graphics.iter().enumerate().find_map(|(i, p)| match p {
                ew_schema::Primitive::Rect(r) => Some((i, r)),
                _ => None,
            })?;
            let (a, c) = (r.rect.0.min(), r.rect.0.max());
            let rect = [
                Pt2::new(a.x, a.y),
                Pt2::new(c.x, a.y),
                Pt2::new(c.x, c.y),
                Pt2::new(a.x, c.y),
            ];
            if b.outline.as_slice() == rect.as_slice() || b.outline.len() < 3 {
                return None;
            }
            let mut d = def.clone();
            d.graphics[i] = ew_schema::Primitive::Polygon(ew_schema::PolygonPrim {
                points: b.outline.clone(),
                closed: true,
                stroke: r.stroke.clone(),
                fill: r.fill.clone(),
                extra: Default::default(),
            });
            Some(d)
        }
        /// A 16-bit value of an outline (FNV-1a over its coordinates, folded).
        fn outline_key(outline: &[Pt2]) -> u16 {
            let mut h: u32 = 0x811c_9dc5;
            for p in outline {
                for v in [p.x.0, p.y.0] {
                    for b in v.to_le_bytes() {
                        h ^= u32::from(b);
                        h = h.wrapping_mul(0x0100_0193);
                    }
                }
            }
            ((h >> 16) ^ (h & 0xffff)) as u16
        }
        for rv in sorted_reps(view) {
            let rep = &rv.rep;
            let Some(def) = &rv.symbol else {
                self.out.findings.push(PageFinding::MissingSymbol { rep: rep.id });
                continue;
            };
            let outlined = boxed(def, rep);
            let lowered = symbol::lower(outlined.as_ref().unwrap_or(def), rep.orient);
            let key = match &outlined {
                None => {
                    let key = SymbolGeomId {
                        symbol: rep.symbol.as_uuid(),
                        variant: rep.orient,
                        extra: None,
                    };
                    self.out
                        .geometry
                        .entry(key)
                        .or_insert_with(|| Arc::new(lowered.graphics.clone()));
                    key
                }
                Some(_) => {
                    let mut k = outline_key(&rep.box_.as_ref().map_or(Vec::new(), |b| b.outline.clone()));
                    loop {
                        let key = SymbolGeomId {
                            symbol: rep.symbol.as_uuid(),
                            variant: rep.orient,
                            extra: Some(k),
                        };
                        match self.out.geometry.get(&key) {
                            Some(g) if **g == lowered.graphics => break key,
                            Some(_) => k = k.wrapping_add(1),
                            None => {
                                self.out.geometry.insert(key, Arc::new(lowered.graphics.clone()));
                                break key;
                            }
                        }
                    }
                }
            };
            let entity = EntityRef(rep.id.as_uuid());
            self.push(
                Item::instance(key, Transform::from_translation(rep.pos))
                    .with_entity(entity)
                    .with_link(Link::Target(Destination::Representation(rep.id.as_uuid()))),
            );
            for t in &lowered.texts {
                let TextSource::Static(ml) = &t.source else {
                    continue;
                };
                if !t.visible {
                    continue;
                }
                let text = ml_text(ml, view.language.as_ref()).to_owned();
                self.text(&TextSpec {
                    text: &text,
                    at: rep.pos + t.pos,
                    angle: t.angle,
                    align: &t.align,
                    style_key: &t.style_key,
                    entity,
                    link: None,
                    items: &[],
                });
            }
        }
        // Break marks, after the symbols.
        let mut marks = view.break_marks.clone();
        marks.sort();
        marks.dedup();
        for m in marks {
            let Some(rv) = view.reps.iter().find(|r| r.rep.id == m.rep) else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let b = symbol::lower(def, rv.rep.orient).bounds;
            let pos = rv.rep.pos;
            let x = match m.side {
                BreakSide::Right => pos.x + b.max().x + BREAK_GAP,
                BreakSide::Left => pos.x + b.min().x - BREAK_GAP,
            };
            let y0 = pos.y + b.min().y - BREAK_GAP;
            let y1 = pos.y + b.max().y + BREAK_GAP;
            let ym = Nm((y0.0 + y1.0) / 2);
            let a = BREAK_ZIGZAG;
            let half = Nm(a.0 / 2);
            let pts = [
                Pt2::new(x, y0),
                Pt2::new(x, ym - a),
                Pt2::new(x + a, ym - half),
                Pt2::new(x - a, ym + half),
                Pt2::new(x, ym + a),
                Pt2::new(x, y1),
            ];
            self.push(
                Item::path(Path::polyline(&pts), Some(wire_stroke(view.grid)), None)
                    .with_entity(EntityRef(m.rep.as_uuid())),
            );
        }
        // The valve glyphs of the module overviews: position p of n is the p-th of n equal cells of the
        // module glyph's width w / n, twice as high, standing on the glyph's top edge.
        for g in &view.valve_glyphs {
            let Some(rv) = view.reps.iter().find(|r| r.rep.id == g.rep) else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let n = i64::try_from(g.glyphs.len()).unwrap_or(0);
            if n == 0 {
                continue;
            }
            // the module glyph is the symbol's graphics, without its texts
            let Some(b) = symbol::lower(def, rv.rep.orient).graphics.bounds() else {
                continue;
            };
            let pos = rv.rep.pos;
            let x0 = pos.x + b.min().x;
            let top = pos.y + b.min().y;
            let w = Nm((b.max().x.0 - b.min().x.0) / n);
            let stroke = wire_stroke(view.grid);
            for (i, glyph) in (0i64..).zip(&g.glyphs) {
                let left = x0 + Nm(w.0 * i);
                let cell = Rect::from_points(Pt2::new(left, top - Nm(2 * w.0)), Pt2::new(left + w, top));
                let cx = left + Nm(w.0 / 2);
                match glyph {
                    ValveGlyph::Valve { entity, part_number } => {
                        let entity = EntityRef(*entity);
                        self.push(
                            Item::path(Path::rect(cell), Some(stroke.clone()), None).with_entity(entity),
                        );
                        let centre = Pt2::new(cx, top - Nm(2 * w.0) + Nm(w.0 / 2));
                        self.push(
                            Item::path(Path::circle(centre, Nm(w.0 / 4)), Some(stroke.clone()), None)
                                .with_entity(entity),
                        );
                        // the part number reads upwards in the cell below the circle
                        self.text(&TextSpec {
                            text: part_number,
                            at: Pt2::new(cx, top - Nm(w.0 * 5 / 8)),
                            angle: Angle2(90_000),
                            align: &Align::Centre,
                            style_key: VALVE_GLYPH_STYLE,
                            entity,
                            link: None,
                            items: &[],
                        });
                    }
                    ValveGlyph::Blank { entity } => {
                        let entity = EntityRef(*entity);
                        self.push(
                            Item::path(Path::rect(cell), Some(stroke.clone()), None).with_entity(entity),
                        );
                        for (a, c) in [
                            (cell.min(), cell.max()),
                            (
                                Pt2::new(cell.min().x, cell.max().y),
                                Pt2::new(cell.max().x, cell.min().y),
                            ),
                        ] {
                            self.push(
                                Item::path(Path::polyline(&[a, c]), Some(stroke.clone()), None)
                                    .with_entity(entity),
                            );
                        }
                    }
                    ValveGlyph::Empty { entity } => {
                        let dashed = stroke.clone().dashed(DashPattern::Dash42);
                        self.push(
                            Item::path(Path::rect(cell), Some(dashed), None).with_entity(EntityRef(*entity)),
                        );
                    }
                }
            }
        }
        self.valve_cells(view);
        self.plug_notes(view);
        // The CP destinations of the page, after the symbols (an empty group at the CP: invisible).
        if !view.cp_targets.is_empty() {
            let cps = cp_map(view);
            let mut targets = view.cp_targets.clone();
            targets.sort();
            targets.dedup();
            for t in targets {
                let at = cps
                    .get(&(t.function.as_uuid(), t.idx))
                    .map(|(p, _)| *p)
                    .or_else(|| view.reps.iter().find(|r| r.rep.id == t.rep).map(|r| r.rep.pos));
                let Some(at) = at else { continue };
                let cp: Id<()> = ew_ids::v5::cp_id(t.function, u32::from(t.idx));
                self.push(
                    Item::group(Some(Transform::from_translation(at)), None, Vec::new())
                        .with_link(Link::Target(Destination::Cp(cp.as_uuid()))),
                );
            }
        }
        // The page items' graphics and the structure-box outlines.
        self.item_graphics(view);
    }

    /// The cells of the valve terminals drawn as their positions (doc 11 §3.2): the sub-base along the
    /// bottom of the cells with its galleries (supply 1, exhausts 3 and 5) drawn through it, one row each, each valve
    /// port that ends on it joined to its gallery by a line with a dot, and each cell's number centred under the
    /// galleries; a thin line between two cells above it, the line that closes the supply end (dashed while no supply
    /// stands there), and per cell: nothing more for a valve whose symbol stands in it, its tag for one drawn on no page or
    /// on another page, with the place it is drawn at under it as a jump to it, a plate with both diagonals for a
    /// blanking plate, a dashed outline for an empty position. The box's outline is the black box's own.
    fn valve_cells(&mut self, view: &PageView) {
        let stroke = wire_stroke(view.grid);
        let thin = Stroke {
            width: Nm(stroke.width.0 / 2),
            ..stroke.clone()
        };
        for v in &view.valve_cells {
            let f = &v.frame;
            let n = v.cells.len();
            if n == 0 {
                continue;
            }
            let e = EntityRef(v.rep.as_uuid());
            let (top, band, bottom) = f.rows();
            let end = f.end_x();
            let right = f.cell_x(n - 1).1;

            self.push(
                Item::path(
                    Path::rect(Rect::from_points(Pt2::new(end, band), Pt2::new(right, bottom))),
                    Some(stroke.clone()),
                    None,
                )
                .with_entity(e),
            );
            for k in 0..ew_derive::GALLERY_PORTS.len() {
                let y = f.gallery_y(k);
                self.push(
                    Item::path(
                        Path::polyline(&[Pt2::new(end, y), Pt2::new(right, y)]),
                        Some(thin.clone()),
                        None,
                    )
                    .with_entity(e),
                );
            }
            for (x, k) in &v.taps {
                let y = f.gallery_y(*k);
                self.push(
                    Item::path(
                        Path::polyline(&[Pt2::new(*x, band), Pt2::new(*x, y)]),
                        Some(thin.clone()),
                        None,
                    )
                    .with_entity(e),
                );
                self.push(
                    Item::path(
                        Path::circle(Pt2::new(*x, y), JUNCTION_DOT_RADIUS),
                        None,
                        Some(Brush::BLACK),
                    )
                    .with_entity(e),
                );
            }
            let end_stroke = if v.supply {
                stroke.clone()
            } else {
                stroke.clone().dashed(DashPattern::Dash42)
            };
            self.push(
                Item::path(
                    Path::polyline(&[Pt2::new(end, top), Pt2::new(end, bottom)]),
                    Some(end_stroke),
                    None,
                )
                .with_entity(e),
            );
            // A quarter of a grid step: the margin of a plate or an empty outline inside its cell.
            let margin = Nm((band.0 - top.0) / 16);
            // The numbers stand under the last gallery.
            let last = f.gallery_y(ew_derive::GALLERY_PORTS.len() - 1);
            for (i, (number, cell)) in v.cells.iter().enumerate() {
                let (l, r) = f.cell_x(i);
                if i > 0 {
                    self.push(
                        Item::path(
                            Path::polyline(&[Pt2::new(l, top), Pt2::new(l, band)]),
                            Some(thin.clone()),
                            None,
                        )
                        .with_entity(e),
                    );
                }
                let cx = Nm(l.0 + (r.0 - l.0) / 2);
                let number = number.to_string();
                self.text(&TextSpec {
                    text: &number,
                    at: Pt2::new(cx, Nm(last.0 + (bottom.0 - last.0) / 2)),
                    angle: Angle2(0),
                    align: &Align::Centre,
                    style_key: VALVE_GLYPH_STYLE,
                    entity: e,
                    link: None,
                    items: &[],
                });
                let inner = Rect::from_points(
                    Pt2::new(l + margin, top + Nm(margin.0 * 4)),
                    Pt2::new(r - margin, band - margin),
                );
                match cell {
                    CellView::Valve { elsewhere: None, .. } => {}
                    CellView::Valve {
                        elsewhere: Some(tag),
                        drawn_at,
                    } => {
                        let mid = inner.min().y.0 + (inner.max().y.0 - inner.min().y.0) / 2;
                        self.text(&TextSpec {
                            text: tag,
                            at: Pt2::new(cx, Nm(mid)),
                            angle: Angle2(0),
                            align: &Align::BottomCentre,
                            style_key: VALVE_GLYPH_STYLE,
                            entity: e,
                            link: None,
                            items: &[],
                        });
                        if let Some((place, rep)) = drawn_at {
                            self.text(&TextSpec {
                                text: &format!("({place})"),
                                at: Pt2::new(cx, Nm(mid)),
                                angle: Angle2(0),
                                align: &Align::TopCentre,
                                style_key: VALVE_GLYPH_STYLE,
                                entity: e,
                                link: Some(Link::Source(Destination::Representation(rep.as_uuid()))),
                                items: &[],
                            });
                        }
                    }
                    CellView::Blank => {
                        self.push(Item::path(Path::rect(inner), Some(stroke.clone()), None).with_entity(e));
                        for (a, c) in [
                            (inner.min(), inner.max()),
                            (
                                Pt2::new(inner.min().x, inner.max().y),
                                Pt2::new(inner.max().x, inner.min().y),
                            ),
                        ] {
                            self.push(
                                Item::path(Path::polyline(&[a, c]), Some(stroke.clone()), None)
                                    .with_entity(e),
                            );
                        }
                    }
                    CellView::Empty => {
                        let dashed = thin.clone().dashed(DashPattern::Dash42);
                        self.push(Item::path(Path::rect(inner), Some(dashed), None).with_entity(e));
                    }
                }
            }
        }
    }

    /// The texts beside the pins of the multipin valve terminals' plugs: each starts one millimetre out from
    /// its pin along the wire's way and stands just beside the wire (turned upright for a pin whose wire leaves up or
    /// down, on the wire for one that leaves sideways), at the size of the box's own pin designations.
    fn plug_notes(&mut self, view: &PageView) {
        for n in &view.plug_notes {
            let (at, angle, align) = plug_note_place(n);
            self.text(&TextSpec {
                text: &n.text,
                at,
                angle,
                align: &align,
                style_key: "data.small",
                entity: EntityRef(n.rep.as_uuid()),
                link: None,
                items: &[],
            });
        }
    }

    fn wires(&mut self, view: &PageView) {
        let cps = cp_map(view);
        let page = page_rect(view);
        let stroke = wire_stroke(view.grid);
        // The drawn polylines in connection-ID order (the hop rule of `crate::hops` reads them all first), each with
        // its stroke: a fluid pilot or drain line is dashed 4:2 in the page's wire pen. An unrouted
        // connection is the dashed right-angled line between its ends in the thinnest pen, and no wire: it
        // takes and makes no hop.
        let mut drawn: Vec<(Id<Connection>, Vec<Pt2>)> = Vec::new();
        let mut strokes: Vec<Stroke> = Vec::new();
        // A flexible fluid line (a hose, ISO 1219-1) is drawn as an arc between its ends (FT-1733).
        let mut flexible: Vec<bool> = Vec::new();
        let mut unrouted: Vec<(Id<Connection>, Vec<Pt2>)> = Vec::new();
        for cv in sorted_connections(view) {
            let c = &cv.connection;
            let Some(pts) = polyline_of(view, &cps, c, page) else {
                self.out
                    .findings
                    .push(PageFinding::UnresolvedEnd { connection: c.id });
                continue;
            };
            if pts.len() < 2 {
                continue;
            }
            if c.route.is_empty() {
                unrouted.push((c.id, pts));
                continue;
            }
            drawn.push((c.id, pts));
            flexible.push(matches!(&c.kind, ConnectionKind::Fluid(f) if f.line_kind == LineKind::Flexible));
            strokes.push(match &c.kind {
                ConnectionKind::Fluid(f) if matches!(f.line_kind, LineKind::Pilot | LineKind::Drain) => {
                    stroke.clone().dashed(DashPattern::Dash42)
                }
                _ => stroke.clone(),
            });
        }
        let hopped = match view.crossings {
            Crossings::Plain => Vec::new(),
            Crossings::Hops => hops(&drawn.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>()),
        };
        // The wires of one pin that share their run out of it part with a bevel (docs/design/06a-drafting.md §3.4).
        let mut lines: Vec<Vec<Pt2>> = drawn.iter().map(|(_, p)| p.clone()).collect();
        let mut at_pin: BTreeMap<(Uuid, u16), Vec<usize>> = BTreeMap::new();
        for (k, (id, _)) in drawn.iter().enumerate() {
            if let Some(cv) = view.connections.iter().find(|c| c.connection.id == *id) {
                for e in [&cv.connection.a, &cv.connection.b] {
                    if let Endpoint::Cp(f, idx) = e {
                        at_pin.entry((f.as_uuid(), *idx)).or_default().push(k);
                    }
                }
            }
        }
        let pins: Vec<(Pt2, Vec<usize>)> = at_pin
            .into_iter()
            .filter_map(|(key, ks)| Some((cps.get(&key)?.0, ks)))
            .collect();
        let step = Nm(view.grid.at(1).map_or(0, |n| n.0) - view.grid.at(0).map_or(0, |n| n.0));
        crate::route::bevel_partings(&mut lines, &pins, step);
        // A text a line runs through stays readable: the line is cut round it.
        let cut = |path: Path| crate::knockout::knock_out(&path, &view.knockouts).unwrap_or(path);
        for (k, (id, _)) in drawn.iter().enumerate() {
            let pts = &lines[k];
            let path = match hopped.get(k) {
                _ if flexible[k] => flexible_arc(pts),
                Some(h) if !h.is_empty() => hop_path(&drawn[k].1, h),
                _ => Path::polyline(pts),
            };
            self.push(
                Item::path(cut(path), Some(strokes[k].clone()), None).with_entity(EntityRef(id.as_uuid())),
            );
        }
        for (id, pts) in &unrouted {
            let dashed = Stroke::pen(Pen::P013).dashed(DashPattern::Dash42);
            self.push(
                Item::path(cut(Path::polyline(pts)), Some(dashed), None).with_entity(EntityRef(id.as_uuid())),
            );
        }
        let mut junctions: Vec<&JunctionView> = view.junctions.iter().collect();
        junctions.sort_by_key(|j| j.junction.id);
        for jv in junctions {
            let j = &jv.junction;
            let entity = EntityRef(j.id.as_uuid());
            let orient = match &j.style {
                JunctionStyle::Bevel(b) => b.orient,
                _ => Orient2::R0,
            };
            match &jv.symbol {
                Some(s) => {
                    let key = self.geometry(s, orient);
                    self.push(Item::instance(key, Transform::from_translation(j.at)).with_entity(entity));
                }
                None if matches!(j.style, JunctionStyle::Dot) => self.push(
                    Item::path(Path::circle(j.at, JUNCTION_DOT_RADIUS), None, Some(Brush::BLACK))
                        .with_entity(entity),
                ),
                None => {}
            }
        }
        self.bridges(view);
    }

    /// The bridges of the page's strips (module documentation, "Bridges").
    fn bridges(&mut self, view: &PageView) {
        let stroke = wire_stroke(view.grid);
        for points in Self::bridge_rows(view) {
            let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
                continue;
            };
            self.push(Item::path(Path::line(first, last), Some(stroke.clone()), None));
            for p in points {
                self.push(Item::path(
                    Path::circle(p, JUNCTION_DOT_RADIUS),
                    None,
                    Some(Brush::BLACK),
                ));
            }
        }
    }

    /// The lines of the page's bridges (module documentation, "Bridges"): per bridge in the view's order, per row of
    /// its terminals, the points it joins along the line, at least two; the line runs from the first to the last.
    pub(crate) fn bridge_rows(view: &PageView) -> Vec<Vec<Pt2>> {
        let cps = drawn_cps(view);
        let reps: BTreeMap<Uuid, &RepView> = view.reps.iter().map(|r| (r.rep.id.as_uuid(), r)).collect();
        let mut out = Vec::new();
        for bridge in &view.bridges {
            // Each terminal's point half way along its bridged lead, by the way the lead leaves and the line across it.
            let mut rows: BTreeMap<(u8, i64), Vec<Pt2>> = BTreeMap::new();
            for (function, idx) in &bridge.clamps {
                let Some((at, dir, rep)) = cps.get(&(function.as_uuid(), *idx)) else {
                    continue;
                };
                let Some(rv) = reps.get(rep) else {
                    continue;
                };
                // The terminal's centre: its insertion point where it is drawn alone; in a strip segment, half way
                // between its side-1 and side-2 points.
                let c = if rv.terminals.is_empty() {
                    rv.rep.pos
                } else {
                    let Some(t) = rv.terminals.iter().find(|t| t.function == *function) else {
                        continue;
                    };
                    let side = |idxs: &[u16]| {
                        idxs.iter()
                            .find_map(|i| cps.get(&(function.as_uuid(), *i)))
                            .map(|(p, _, _)| *p)
                    };
                    let (Some(a), Some(b)) = (side(&t.side1), side(&t.side2)) else {
                        continue;
                    };
                    Pt2::new(
                        Nm(a.x.0 + (b.x.0 - a.x.0).div_euclid(2)),
                        Nm(a.y.0 + (b.y.0 - a.y.0).div_euclid(2)),
                    )
                };
                let mid = Pt2::new(
                    Nm(c.x.0 + (at.x.0 - c.x.0).div_euclid(2)),
                    Nm(c.y.0 + (at.y.0 - c.y.0).div_euclid(2)),
                );
                let key = match dir {
                    Dir4::Up => (0, mid.y.0),
                    Dir4::Down => (1, mid.y.0),
                    Dir4::Left => (2, mid.x.0),
                    Dir4::Right => (3, mid.x.0),
                    Dir4::Unknown(_) => continue,
                };
                rows.entry(key).or_default().push(mid);
            }
            for ((side, _), mut points) in rows {
                points.sort_by_key(|p| if side < 2 { p.x } else { p.y });
                points.dedup();
                if points.len() >= 2 {
                    out.push(points);
                }
            }
        }
        out
    }

    /// The crossings of the connections `ids` drawn on the page with the line through `at` (horizontal or
    /// vertical), along the line ([`crossing_of`]); a connection the page does not draw, or one the line misses,
    /// has none.
    pub(crate) fn marker_crossings(
        view: &PageView,
        ids: &[Id<Connection>],
        at: Pt2,
        horizontal: bool,
    ) -> Vec<Nm> {
        let cps = cp_map(view);
        let page = page_rect(view);
        ids.iter()
            .filter_map(|id| view.connections.iter().find(|c| c.connection.id == *id))
            .filter_map(|cv| polyline_of(view, &cps, &cv.connection, page))
            .filter_map(|pts| crossing_of(&pts, at, horizontal))
            .collect()
    }

    pub(crate) fn geometry(&mut self, s: &SymbolRef, orient: Orient2) -> SymbolGeomId {
        let key = SymbolGeomId {
            symbol: s.id.as_uuid(),
            variant: orient,
            extra: None,
        };
        self.out
            .geometry
            .entry(key)
            .or_insert_with(|| symbol_geometry(&s.def, orient));
        key
    }

    pub(crate) fn style(&mut self, key: &str, entity: EntityRef) -> TextStyle {
        self.styles.resolve(key).unwrap_or_else(|| {
            self.out.findings.push(PageFinding::UnknownStyle {
                entity: entity.0,
                key: key.to_owned(),
            });
            self.styles
                .resolve(DEFAULT_STYLE_KEY)
                .unwrap_or(TextStyle::regular(Nm(3_175_000)))
        })
    }

    /// [`style`](Builder::style) without the report: the style of `key`, else the default style.
    pub(crate) fn style_quiet(&self, key: &str) -> TextStyle {
        self.styles
            .resolve(key)
            .or_else(|| self.styles.resolve(DEFAULT_STYLE_KEY))
            .unwrap_or(TextStyle::regular(Nm(3_175_000)))
    }

    /// Shapes `spec.text` and places it at `spec.at` with its alignment and angle, as form graphics do.
    pub(crate) fn text(&mut self, spec: &TextSpec<'_>) {
        self.text_placed(spec, false);
    }

    /// [`text`](Builder::text); with `first_line` a text of several lines aligned to the middle keeps its first line
    /// where a one-line text would sit and grows downward in its own frame (module documentation).
    pub(crate) fn text_placed(&mut self, spec: &TextSpec<'_>, first_line: bool) {
        if spec.text.is_empty() {
            return;
        }
        let style = self.style(spec.style_key, spec.entity);
        let shaped = self.fonts.shape(spec.text, &style, None);
        self.emit_text(spec, &style, &shaped, first_line);
    }

    /// [`text`](Builder::text) of a text already shaped in `style` (a label of a panel page, shaped by the memoised
    /// `shaped_text`).
    pub(crate) fn text_shaped(&mut self, spec: &TextSpec<'_>, style: &TextStyle, shaped: &Shaped) {
        if spec.text.is_empty() {
            return;
        }
        self.emit_text(spec, style, shaped, false);
    }

    fn emit_text(&mut self, spec: &TextSpec<'_>, style: &TextStyle, shaped: &Shaped, first_line: bool) {
        if !shaped.missing.is_empty() {
            self.out.findings.push(PageFinding::MissingGlyphs {
                entity: spec.entity.0,
                text: spec.text.to_owned(),
                chars: shaped.missing.clone(),
            });
        }
        let lines = self.lay_out(spec, style, shaped, first_line);
        let carriers = self.item_carriers(spec, style, shaped, &lines);
        for item in lines {
            let item = item.with_entity(spec.entity);
            self.push(match spec.link {
                Some(l) => item.with_link(l),
                None => item,
            });
        }
        for item in carriers {
            self.push(item);
        }
    }

    /// The line items of `spec` shaped as `shaped` in `style` (see [`text_placed`](Builder::text_placed)).
    fn lay_out(
        &self,
        spec: &TextSpec<'_>,
        style: &TextStyle,
        shaped: &Shaped,
        first_line: bool,
    ) -> Vec<Item> {
        let (halign, valign) = split_align(spec.align).unwrap_or((0, 0));
        // A middle-aligned block is centred from the first line's cap height to the last baseline; dropping the
        // box's middle by half the last baseline puts the first line where a one-line text would be.
        let last = shaped.lines.last().map_or(Nm::ZERO, |l| l.baseline);
        let rect = if first_line && valign == 0 && shaped.lines.len() > 1 {
            Rect::from_point(spec.at + Pt2::new(Nm::ZERO, Nm(last.0.div_euclid(2))))
        } else {
            Rect::from_point(spec.at)
        };
        let boxed = BoxSpec {
            rect,
            halign,
            valign,
            padding: Nm::ZERO,
            overflow: None,
            max_lines: 1,
            sheet: SheetMap::IDENTITY,
        };
        place(self.fonts, shaped, style, spec.text, &boxed, spec.at, spec.angle)
    }

    /// The bounds of the layout boxes of `spec`'s lines as [`text_placed`](Builder::text_placed) would place
    /// them; `None` for an empty text. Reports nothing (an unknown style measures in the default style).
    pub(crate) fn text_box(&self, spec: &TextSpec<'_>, first_line: bool) -> Option<Rect> {
        self.text_box_in(spec, &self.style_quiet(spec.style_key), first_line)
    }

    /// [`text_box`](Builder::text_box) of `spec` shaped in `style`.
    pub(crate) fn text_box_in(
        &self,
        spec: &TextSpec<'_>,
        style: &TextStyle,
        first_line: bool,
    ) -> Option<Rect> {
        if spec.text.is_empty() {
            return None;
        }
        let shaped = self.fonts.shape(spec.text, style, None);
        layout_bounds(&self.lay_out(spec, style, &shaped, first_line))
    }

    /// One invisible path item per entry of `spec.items`, over the box of that item's glyph span and
    /// carrying its link (the box turns with the text). `lines` are the placed line items of the text.
    fn item_carriers(
        &self,
        spec: &TextSpec<'_>,
        style: &TextStyle,
        shaped: &Shaped,
        lines: &[Item],
    ) -> Vec<Item> {
        let ranges: Vec<&Range<usize>> = spec.items.iter().map(|link| &link.range).collect();
        span_quads(
            self.fonts.vertical_metrics(style),
            spec.text,
            shaped,
            lines,
            &ranges,
        )
        .into_iter()
        .zip(spec.items)
        .filter_map(|(quad, link)| {
            Some(
                Item::path(Path::polygon(&quad?), None, None)
                    .with_entity(spec.entity)
                    .with_link(Link::Source(link.dest)),
            )
        })
        .collect()
    }
}

/// The bounds of the layout boxes of the glyph runs among `items` as placed (a run of a group under the group's
/// transform); `None` without a run: a text's box as [`Builder::text_box`] measures it and the labels layer prints it.
pub(crate) fn layout_bounds(items: &[Item]) -> Option<Rect> {
    let mut out: Option<Rect> = None;
    for item in items {
        let quad: Vec<Pt2> = match &item.kind {
            ItemKind::Glyphs(run) => run.layout_quad().to_vec(),
            ItemKind::Group(g) => {
                let t = g.transform.unwrap_or(Transform::IDENTITY);
                g.items
                    .iter()
                    .filter_map(|i| match &i.kind {
                        ItemKind::Glyphs(run) => Some(run.layout_quad().map(|q| t.apply(q))),
                        _ => None,
                    })
                    .flatten()
                    .collect()
            }
            _ => Vec::new(),
        };
        for q in quad {
            let r = Rect::from_point(q);
            out = Some(out.map_or(r, |o| o.union(r)));
        }
    }
    out
}

/// The boxes of the glyph spans `ranges` of `text` as placed (ADR-0015 amendment 9), one per range in their order: over
/// the span's glyphs where they are drawn, from the ascent to the descent `m` of the text's style, mapped to the parent
/// through the placed line item that holds the span (so it turns with the text). `shaped` is `text` shaped as drawn
/// and `lines` its placed line items, one per shaped line. `None` for a range that no line holds whole, whose line is
/// not placed, or that holds no glyph of a line with a right-to-left run. The cross-reference texts' item links and
/// the report tables' item links both take their boxes from here.
///
/// It shapes nothing: the boxes come from the drawn shaping's glyph positions and
/// clusters, line by line ([`LinePens`]), so they lie where the glyphs are drawn whatever the text's base direction.
pub(crate) fn span_quads(
    m: VerticalMetrics,
    text: &str,
    shaped: &Shaped,
    lines: &[Item],
    ranges: &[&Range<usize>],
) -> Vec<Option<[Pt2; 4]>> {
    let mut pens: Vec<Option<LinePens>> = vec![None; shaped.lines.len()];
    ranges
        .iter()
        .map(|range| {
            let (k, line) = shaped
                .lines
                .iter()
                .enumerate()
                .find(|(_, l)| l.text.start <= range.start && range.end <= l.text.end)?;
            let line_text = text.get(line.text.clone())?;
            let p = pens
                .get_mut(k)?
                .get_or_insert_with(|| LinePens::new(shaped, line, line_text));
            let (x0, x1) = p.extent(range.start - line.text.start, range.end - line.text.start)?;
            let to_parent = |q: Pt2| match &lines.get(k)?.kind {
                ItemKind::Glyphs(run) => Some(run.to_parent(q)),
                ItemKind::Group(g) => Some(g.transform.unwrap_or(Transform::IDENTITY).apply(q)),
                _ => None,
            };
            let corner = |x: Nm, y: Nm| to_parent(Pt2::new(x, y));
            Some([
                corner(x0, Nm(-m.ascent.0))?,
                corner(x0, m.descent)?,
                corner(x1, m.descent)?,
                corner(x1, Nm(-m.ascent.0))?,
            ])
        })
        .collect()
}

/// One drawn line's glyphs: where each is drawn and which cluster it belongs to, taken from the text's shaping as
/// drawn ([`span_quads`]).
///
/// In a left-to-right line (the clusters do not decrease along it) a span runs from the travel up to its start to the
/// travel up to its end. The travel up to a byte is the x of the first glyph of the cluster that starts there, kerning
/// included: the line's first glyph's x at its start, the line's advance at its end (or past it). A span's start inside
/// a cluster (a ligature, a combining or joiner sequence) takes that cluster's start and its end the next cluster's
/// start, so the box covers whole glyphs. In a line with a right-to-left run the box spans the extent of the glyphs
/// whose clusters the span covers (each glyph reaching to the next one's x, the last to the line's advance); a span
/// that covers no glyph there has none.
#[derive(Clone)]
struct LinePens {
    /// The byte length of the line's text without its trailing line breaks.
    len: usize,
    /// The line's advance as drawn (where its last glyph ends).
    advance: Nm,
    /// Each glyph's x, in the order drawn.
    xs: Vec<Nm>,
    /// Each glyph's cluster start, in bytes of the line, in the order drawn.
    clusters: Vec<usize>,
    /// Whether the clusters do not decrease along the line (a left-to-right line).
    ltr: bool,
    /// `(cluster start, glyph index)` of every glyph, ascending (the right-to-left extent's index).
    by_cluster: Vec<(usize, usize)>,
}

impl LinePens {
    fn new(shaped: &Shaped, line: &LineSpan, line_text: &str) -> LinePens {
        let glyphs = shaped.run.glyphs.get(line.glyphs.clone()).unwrap_or_default();
        let xs: Vec<Nm> = glyphs.iter().map(|g| g.x).collect();
        let clusters: Vec<usize> = shaped
            .clusters
            .get(line.glyphs.clone())
            .unwrap_or_default()
            .iter()
            .map(|c| c.saturating_sub(line.text.start))
            .collect();
        let ltr = clusters.windows(2).all(|w| w[0] <= w[1]);
        let mut by_cluster: Vec<(usize, usize)> = clusters.iter().copied().zip(0..).collect();
        by_cluster.sort_unstable();
        LinePens {
            len: line_text.trim_end_matches(is_line_break).len(),
            advance: line.advance,
            xs,
            clusters,
            ltr,
            by_cluster,
        }
    }

    /// The start of the cluster that holds byte `b`: the largest cluster start not after it (`b` before every cluster).
    fn cluster_of(&self, b: usize) -> usize {
        let i = self.by_cluster.partition_point(|&(c, _)| c <= b);
        i.checked_sub(1).map_or(b, |i| self.by_cluster[i].0)
    }

    /// The x of the first glyph whose cluster starts at or after `b`; the advance when none does.
    fn x_from(&self, b: usize) -> Nm {
        let g = self.clusters.partition_point(|&c| c < b);
        self.xs.get(g).copied().unwrap_or(self.advance)
    }

    /// The x range of the span `[start, end)` of the line (see the type's documentation).
    fn extent(&self, start: usize, end: usize) -> Option<(Nm, Nm)> {
        if self.ltr {
            let travel = |b: usize, x: &dyn Fn(usize) -> Nm| if b >= self.len { self.advance } else { x(b) };
            let x0 = travel(start, &|b| self.x_from(self.cluster_of(b)));
            let x1 = travel(end, &|b| self.x_from(b));
            return Some((x0, x1));
        }
        let lo = self.cluster_of(start);
        let from = self.by_cluster.partition_point(|&(c, _)| c < lo);
        let to = self.by_cluster.partition_point(|&(c, _)| c < end);
        let glyphs = self.by_cluster.get(from..to)?;
        let right = |g: usize| self.xs.get(g + 1).copied().unwrap_or(self.advance);
        let x0 = glyphs.iter().map(|&(_, g)| self.xs[g]).min()?;
        let x1 = glyphs.iter().map(|&(_, g)| right(g)).max()?;
        Some((x0, x1))
    }
}

/// A character that ends a line (a mandatory break).
fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// The distance of a black box's pin text from its pin, inward along the pin's normal: 1 mm.
pub const PIN_TEXT_GAP: Nm = Nm(1_000_000);

/// A placed pin text: its entity, text, point, angle and alignment, and for a text in a card's cell the width it keeps
/// within.
pub(crate) type PlacedPin = (EntityRef, String, Pt2, Angle2, Align, Option<Nm>);

/// The cells of the PLC cards drawn in one row (doc 10 §3.6, "A card drawn in cells"): a thin line between each two
/// neighbouring channel pins, at the middle between them, across the box from edge to edge. The labels layer draws them
/// with the pin texts, which tell which pins are a card's channels.
pub(crate) fn cell_dividers(view: &PageView) -> Vec<Item> {
    let stroke = wire_stroke(view.grid);
    let thin = Stroke {
        width: Nm(stroke.width.0 / 2),
        ..stroke
    };
    let mut out = Vec::new();
    for rv in sorted_reps(view) {
        let pins = cell_pins(rv);
        let Some(outline) = rv.rep.box_.as_ref().and_then(|b| {
            b.outline
                .iter()
                .map(|p| Rect::from_point(rv.rep.pos + rv.rep.orient.apply(*p)))
                .reduce(Rect::union)
        }) else {
            continue;
        };
        for pair in pins.windows(2) {
            let x = Nm(pair[0].0.x.0 + (pair[1].0.x.0 - pair[0].0.x.0) / 2);
            out.push(
                Item::path(
                    Path::polyline(&[Pt2::new(x, outline.min().y), Pt2::new(x, outline.max().y)]),
                    Some(thin.clone()),
                    None,
                )
                .with_entity(EntityRef(rv.rep.id.as_uuid())),
            );
        }
    }
    out
}

/// The channel pins of a PLC card drawn in one row (its [`PinText`]s with a `cell`), on the page and with the way their
/// wires leave, from left to right; empty for any other representation. Only a pin whose wire leaves up or down on the
/// page has a cell: a card's box turned a quarter keeps its pin texts as any box does.
fn cell_pins(rv: &RepView) -> Vec<(Pt2, Dir4, &PinText)> {
    let Some(b) = &rv.rep.box_ else {
        return Vec::new();
    };
    let mut out: Vec<(Pt2, Dir4, &PinText)> = b
        .cps
        .iter()
        .filter_map(|cp| {
            let pin = rv.pins.iter().find(|p| p.idx == cp.idx && p.cell.is_some())?;
            let dir = symbol::transform_dir(&cp.dir, rv.rep.orient);
            if !matches!(dir, Dir4::Up | Dir4::Down) {
                return None;
            }
            Some((rv.rep.pos + rv.rep.orient.apply(cp.pos), dir, pin))
        })
        .collect();
    out.sort_by_key(|(at, _, pin)| (at.x, pin.idx));
    out
}

/// Where the text `text` of a pin that a wire leaves along `dir` stands (module documentation, "Pin texts"): its
/// anchor from the pin, its angle and its alignment, inside the body [`PIN_TEXT_GAP`] from the pin. The one rule for a
/// black box's pin texts and for the number text the symbol editor gives a new connection point; `None` for a pin of
/// an unknown direction.
pub fn pin_text_place(dir: &Dir4, text: &str) -> Option<(Pt2, Angle2, Align)> {
    let g = PIN_TEXT_GAP;
    // A text of signs only ("+", "−") reads the same horizontally, and turned a minus would read as a tick.
    let signs = !text.chars().any(char::is_alphanumeric);
    Some(match dir {
        Dir4::Up if signs => (Pt2::new(Nm::ZERO, g), Angle2::ZERO, Align::TopCentre),
        Dir4::Down if signs => (Pt2::new(Nm::ZERO, Nm(-g.0)), Angle2::ZERO, Align::BottomCentre),
        Dir4::Up => (Pt2::new(Nm::ZERO, g), Angle2(90_000), Align::MiddleRight),
        Dir4::Down => (Pt2::new(Nm::ZERO, Nm(-g.0)), Angle2(90_000), Align::MiddleLeft),
        Dir4::Left => (Pt2::new(g, Nm::ZERO), Angle2::ZERO, Align::MiddleLeft),
        Dir4::Right => (Pt2::new(Nm(-g.0), Nm::ZERO), Angle2::ZERO, Align::MiddleRight),
        Dir4::Unknown(_) => return None,
    })
}

/// The pin texts of the page's black boxes, by representation ID then box-CP order (module documentation, "Pin
/// texts").
pub(crate) fn pin_texts(view: &PageView) -> Vec<PlacedPin> {
    let mut out = Vec::new();
    for rv in sorted_reps(view) {
        let rep = &rv.rep;
        let (Some(def), Some(b)) = (&rv.symbol, &rep.box_) else {
            continue;
        };
        if rv.pins.is_empty() || !def.cps.is_empty() {
            continue;
        }
        // A card drawn in cells: each channel pin's texts level in its cell, the designation and address next to the
        // pin and the function text under it inside the box, kept within the cell's width.
        let cells = cell_pins(rv);
        let pitch = cells
            .windows(2)
            .map(|w| w[1].0.x.0 - w[0].0.x.0)
            .filter(|d| *d > 0)
            .min();
        for (at, dir, pin) in &cells {
            let (Some(function_text), Some(pitch)) = (&pin.cell, pitch) else {
                continue;
            };
            let g = PIN_TEXT_GAP;
            let lines = |first: &str, second: &str| match (first.is_empty(), second.is_empty()) {
                (true, _) => second.to_owned(),
                (_, true) => first.to_owned(),
                _ => format!("{first}\n{second}"),
            };
            let (text, offset, align) = match dir {
                Dir4::Up => (
                    lines(&pin.text, function_text),
                    Pt2::new(Nm::ZERO, g),
                    Align::TopCentre,
                ),
                Dir4::Down => (
                    lines(function_text, &pin.text),
                    Pt2::new(Nm::ZERO, Nm(-g.0)),
                    Align::BottomCentre,
                ),
                _ => continue,
            };
            if !text.is_empty() {
                let width = Nm(pitch.saturating_sub(2 * g.0));
                out.push((
                    EntityRef(rep.id.as_uuid()),
                    text,
                    *at + offset,
                    Angle2::ZERO,
                    align,
                    Some(width),
                ));
            }
        }
        for cp in &b.cps {
            if pitch.is_some() && cells.iter().any(|(_, _, p)| p.idx == cp.idx) {
                continue;
            }
            let Some(pin) = rv.pins.iter().find(|p| p.idx == cp.idx) else {
                continue;
            };
            if pin.text.is_empty() {
                continue;
            }
            let at = rep.pos + rep.orient.apply(cp.pos);
            let Some((offset, angle, align)) =
                pin_text_place(&symbol::transform_dir(&cp.dir, rep.orient), &pin.text)
            else {
                continue;
            };
            out.push((
                EntityRef(rep.id.as_uuid()),
                pin.text.clone(),
                at + offset,
                angle,
                align,
                None,
            ));
        }
    }
    out
}

/// Where the text beside a plug's pin stands (module documentation of [`Builder::plug_notes`]): its anchor, angle
/// and alignment.
pub(crate) fn plug_note_place(n: &PlugNoteView) -> (Pt2, Angle2, Align) {
    let gap = Nm(1_000_000);
    let side = Nm(700_000);
    let out = if n.forward { gap } else { Nm(-gap.0) };
    match (n.vertical, n.forward) {
        (true, false) => (
            Pt2::new(n.at.x - side, n.at.y + out),
            Angle2(90_000),
            Align::MiddleLeft,
        ),
        (true, true) => (
            Pt2::new(n.at.x - side, n.at.y + out),
            Angle2(90_000),
            Align::MiddleRight,
        ),
        (false, true) => (
            Pt2::new(n.at.x + out, n.at.y - Nm(gap.0 / 2)),
            Angle2(0),
            Align::BottomLeft,
        ),
        (false, false) => (
            Pt2::new(n.at.x + out, n.at.y - Nm(gap.0 / 2)),
            Angle2(0),
            Align::BottomRight,
        ),
    }
}

/// A drawn function CP: its function, its index, its point and the direction a wire leaves it.
pub(crate) type DrawnCp = (Id<Function>, u16, Pt2, Dir4);

/// The function CPs each representation draws, by representation ID. A CP that several representations draw is the
/// first one's by ID, as [`cp_map`] has it.
pub(crate) fn cp_owners(view: &PageView) -> BTreeMap<Uuid, Vec<DrawnCp>> {
    let mut out: BTreeMap<Uuid, Vec<DrawnCp>> = BTreeMap::new();
    for ((function, idx), (pos, dir, rep)) in drawn_cps(view) {
        out.entry(rep)
            .or_default()
            .push((Id::from_uuid(function), idx, pos, dir));
    }
    out
}

/// The arc of a flexible fluid line through the ends of its route `pts` (ISO 1219-1, a hose): a quadratic curve whose
/// control point stands a quarter of the chord off the chord's middle, to the left of the direction from the first
/// end to the last; the straight line where both ends coincide.
fn flexible_arc(pts: &[Pt2]) -> Path {
    let (Some(a), Some(b)) = (pts.first(), pts.last()) else {
        return Path::polyline(pts);
    };
    let (dx, dy) = (i128::from(b.x.0 - a.x.0), i128::from(b.y.0 - a.y.0));
    if dx == 0 && dy == 0 {
        return Path::polyline(pts);
    }
    // The middle of the chord, moved by a quarter of the chord turned a quarter left: (-dy, dx) / 4.
    let mid = |p: i64, q: i64| (i128::from(p) + i128::from(q)) / 2;
    let sat = |v: i128| Nm(i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX }));
    let control = Pt2::new(sat(mid(a.x.0, b.x.0) - dy / 4), sat(mid(a.y.0, b.y.0) + dx / 4));
    Path::builder().move_to(*a).quad_to(control, *b).build()
}

/// `align` with its horizontal part kept and its vertical part at the bottom (a strip tag).
pub(crate) fn bottom_align(align: &Align) -> Align {
    match align {
        Align::TopLeft | Align::MiddleLeft | Align::BottomLeft => Align::BottomLeft,
        Align::TopCentre | Align::Centre | Align::BottomCentre => Align::BottomCentre,
        Align::TopRight | Align::MiddleRight | Align::BottomRight => Align::BottomRight,
        other => other.clone(),
    }
}

/// `step` towards the top of a text at `angle` (y down; a text at 90° reads bottom to top, so its top is to the
/// left): the point of the circle of radius `step` at `angle + 90°`, by `ew_dl::num::arc_point` (doc 09 §4.3 rule 4:
/// no platform trigonometry, nanometre ties toward zero; a quarter turn lands exactly on the page axes).
pub(crate) fn up_by(angle: Angle2, step: Nm) -> Pt2 {
    // `normalized` is below 360°, so adding a quarter turn cannot overflow.
    let up = Angle2(angle.normalized().0 + 90_000);
    ew_dl::num::arc_point(Pt2::ORIGIN, step, step, up)
}

pub(crate) struct TextSpec<'a> {
    pub(crate) text: &'a str,
    pub(crate) at: Pt2,
    pub(crate) angle: Angle2,
    pub(crate) align: &'a Align,
    pub(crate) style_key: &'a str,
    pub(crate) entity: EntityRef,
    pub(crate) link: Option<Link>,
    /// One link per listed item of `text`.
    pub(crate) items: &'a [ItemLink],
}

/// The midpoint of the longest segment of `pts` (the first of equal ones) and whether that segment is
/// vertical; `None` for fewer than two points.
pub(crate) fn marker_point(pts: &[Pt2]) -> Option<(Pt2, bool)> {
    let mut best: Option<(i128, Pt2, bool)> = None;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dx = i128::from(b.x.0) - i128::from(a.x.0);
        let dy = i128::from(b.y.0) - i128::from(a.y.0);
        let len = dx * dx + dy * dy;
        if best.is_none_or(|(l, _, _)| len > l) {
            let mid = |p: Nm, q: Nm| Nm(p.0 + (q.0 - p.0).div_euclid(2));
            best = Some((len, Pt2::new(mid(a.x, b.x), mid(a.y, b.y)), dx == 0 && dy != 0));
        }
    }
    best.map(|(_, p, v)| (p, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One step of the QET 10 px grid (7937500/3 nm), as `Grid::at(1)` gives it.
    const STEP: Nm = Nm(2_645_833);

    /// The strip tag's step up is exact on the quarter turns and, at any other angle, the
    /// deterministic `arc_point` rounded to the nanometre, ties toward zero (doc 09 §4.3 rule 4).
    #[test]
    fn up_by_is_exact_on_quarter_turns_and_deterministic_between() {
        assert_eq!(up_by(Angle2(0), STEP), Pt2::new(Nm(0), Nm(-2_645_833)));
        assert_eq!(up_by(Angle2(90_000), STEP), Pt2::new(Nm(-2_645_833), Nm(0)));
        assert_eq!(up_by(Angle2(180_000), STEP), Pt2::new(Nm(0), Nm(2_645_833)));
        assert_eq!(up_by(Angle2(270_000), STEP), Pt2::new(Nm(2_645_833), Nm(0)));
        assert_eq!(up_by(Angle2(-90_000), STEP), Pt2::new(Nm(2_645_833), Nm(0)));
        assert_eq!(
            up_by(Angle2(45_000), STEP),
            Pt2::new(Nm(-1_870_886), Nm(-1_870_886))
        );
        assert_eq!(
            up_by(Angle2(30_000), STEP),
            Pt2::new(Nm(-1_322_916), Nm(-2_291_359))
        );
    }
}
