// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page display lists per layer (doc 09 §4.1, ARCH §6.4 `page_display_list(page, layer, lang)`, ADR-0015
//! decision 1 and amendment 2).
//!
//! # The view
//!
//! A page is drawn as a pure function of a view (ARCH §3.2, the seam of a page's view): `ew-scene` calls the
//! derivation's pure rules (the order allows the edge to `ew-derive`) but holds no tracked read context `Q`, so the
//! caller, which holds it, fills a [`PageView`]: the page's records by reference (representations,
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
//!   gives them in, so a record shuffle never changes a display list.
//! - **CP positions** follow the rule of where a function CP is drawn, whose home is `ew-derive` (`cp_point`,
//!   ARCH §3.4); this module places them from the view by that rule (`ew_derive::rep_drawn_cps`,
//!   `ew_derive::segment_point`) and hands them out ([`drawn_cps`]): symbol CP `c` of a representation sits at
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
//!   `@qet10` symbols), else the 0.26 mm house pen.
//! - **Break marks** (doc 11 §5.5 step 5, FR-DEV-023, D-41). A [`BreakMarkView`] closes an assembly
//!   overview segment at one side of a representation's symbol bounds `b` (in its orientation): at
//!   `x = pos.x + b.max.x + BREAK_GAP` for the right side, `pos.x + b.min.x − BREAK_GAP` for the left, a vertical
//!   stroke from `b.min.y − BREAK_GAP` to `b.max.y + BREAK_GAP` (relative to `pos.y`) with a zig-zag of
//!   amplitude [`BREAK_ZIGZAG`] at its middle, in the page's [`wire_stroke`], tagged with the representation.
//!   A mark whose representation or symbol is not in the view draws nothing.
//! - **Junctions** draw the caller's junction symbol at `at` in the bevel's orientation; without a symbol a
//!   dot is a filled black disc of radius [`JUNCTION_DOT_RADIUS`].
//! - **Bridges** (doc 08 §6.5): the program's drawing of a strip's bridge, no record and nothing to select.
//!   Each clamp of a [`BridgeView`] that the page draws gives a point half way between the terminal's centre and the
//!   clamp's point, on the lead the bridge joins: the centre of a terminal drawn alone is its insertion point, that of
//!   a terminal a strip segment draws half way between its side-1 and side-2 points, so a segment's
//!   bridged terminals make the bus across its leads. A symbol of a block's level from 2 (drawn through a pin map)
//!   gives its point three quarters of the way instead, so that a block's level-1 and level-2 bridges along the same
//!   leads run apart, each with its own dots. Points whose clamps leave the same way and that lie on one line
//!   across those leads (one height for leads that leave up or down, one x for leads that leave left or right) are one
//!   row; a row of two or more is drawn as a straight line from its first point to its last in the page's
//!   [`wire_stroke`], and a dot of [`JUNCTION_DOT_RADIUS`] at each of its points, after the junctions. A terminal of
//!   the bridge in another row or on another page joins no line here; a terminal that is no part of the bridge and
//!   stands between two of the row is crossed without a dot, a row across up or down leads hopping over its lead
//!   (a half circle a quarter of a grid step high, towards the bodies from a level-1 row and towards the clamps from
//!   a level-2 row), so the line reads as joining only the dotted leads.
//! - **Level ties** (doc 08 §6.3): the two symbols of a block of two levels read as one block. Neighbouring symbols of
//!   one function, one drawn through a pin map, on one height at most three grid steps apart, are joined between
//!   their bodies, half a grid step above their middle (clear of the numbers beside the bodies and of a bridge across
//!   the lower leads), by a dashed line in pen 0.13 (`level_ties`), no record and nothing to select, after the
//!   bridges.
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
//!   (drawn as in "Black boxes") in [`PIN_TEXT_STYLE`] (`data.small`), inside the outline, [`PIN_TEXT_GAP`] from the pin along its
//!   inward normal: a pin leaving up or down reads bottom to top (90°), its end towards the pin (aligned middle right
//!   below a pin leaving up, middle left above a pin leaving down), but a text of signs only ("+", "−", no letter or
//!   digit: a turned "−" read as a tick) reads horizontally, centred on the pin at the same gap (aligned top
//!   centre below a pin leaving up, bottom centre above a pin leaving down); a pin leaving left or right reads
//!   horizontally, its start towards the pin (middle left right of a pin leaving left, middle right left of a pin
//!   leaving right); [`pin_text_place`], which the symbol editor also gives a new connection point's number text. They
//!   print after the slot texts and never move: they are fixed texts. An entry without its box
//!   CP, or a pin of an unknown direction, prints nothing.
//! - **Wire markers** (FR-OUT-011 keeps them in monochrome; the text stands clear of its wires, as drawing
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
//! - **Wire numbers**: a connection's [`ConnectionView::number`] prints once, in style `marker`, along its
//!   wire where the text placer sets it: right of a vertical stretch turned to read from the bottom, above a
//!   horizontal one; beside the dashed line of an unrouted wire; with a marker of its own it is that marker's text,
//!   the number first (`101.1 1.5 BK`). A bus line whose cable has no mark on the page prints the cable's tag there in
//!   style [`CABLE_TAG_STYLE`] (it was too small to read).
//! - **Stored symbol variants** (`Representation.variant`, contact-image variants) are not drawn yet: the
//!   main graphics are used, and the geometry key's `extra` is `None`.
//!
//! [`MARKER_REACH`]: ew_drafting::consts::MARKER_REACH
//! [`MARKER_GAP`]: ew_drafting::consts::MARKER_GAP
//! [`CABLE_TICK`]: ew_drafting::consts::CABLE_TICK

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

mod cps;
mod draw;
mod pins;
mod text;
mod wires;

pub use cps::*;
pub use pins::*;
pub(crate) use text::*;

/// The radius of a junction dot drawn without a junction symbol: 1.25 QET px (0.33 mm), the radius of
/// `ew.gen.junction-dot@qet10`.
pub const JUNCTION_DOT_RADIUS: Nm = Nm(330_729);

/// The 1.0 QET px pen (`7_937_500 / 30` nm, rounded): wires on QET-grid pages and the `@qet10` edition's
/// replacement of the 0.26 mm house pen, so that after the QET frame's sheet map it prints at 0.261 mm
///
pub const QET_PEN_1PX: Nm = Nm(264_583);

/// The wire pen of a page with connection grid `grid`: [`QET_PEN_1PX`] on the QET 10 px grid, else the
/// 0.26 mm house pen (`@qet10` symbols carry the same QET-px pens).
pub fn wire_stroke(grid: Grid) -> Stroke {
    if grid == Grid::QET_10PX {
        Stroke::solid(QET_PEN_1PX)
    } else {
        Stroke::pen(Pen::P026)
    }
}

/// The sheet of a page whose frame form is `form`: its width and height and its page-to-sheet map; without a form
/// the default sheet ([`DEFAULT_SHEET`]) and the identity.
pub fn sheet_of(form: Option<&FormDef>) -> (Nm, Nm, SheetMap) {
    match form {
        Some(form) => (form.sheet.width, form.sheet.height, SheetMap::of(form)),
        None => (DEFAULT_SHEET.0, DEFAULT_SHEET.1, SheetMap::IDENTITY),
    }
}

/// The sheet used when a page has no frame form: A4 landscape (297 mm × 210 mm).
pub const DEFAULT_SHEET: (Nm, Nm) = (Nm(297_000_000), Nm(210_000_000));

/// The text style of wire-marker texts.
pub const MARKER_STYLE: &str = "marker";

/// The text style of a cable mark's tag: the device tag's.
pub const CABLE_TAG_STYLE: &str = "tag.main";

/// The built-in text style of a symbol's small data (`ew_schema`'s style table), which the scene's small texts take.
pub const DATA_SMALL_STYLE: &str = "data.small";

/// The text style of a cable mark's type line.
pub const CABLE_KIND_STYLE: &str = DATA_SMALL_STYLE;

/// Where a wire marker's text stands beyond its line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MarkerText {
    /// The line is horizontal (across vertical wires): the text reads horizontally right of its right end.
    Right,
    /// The line is vertical (across horizontal wires): the text reads horizontally above its top end.
    Above,
    /// The line is vertical: the text reads bottom to top (90°) upwards from its top end.
    Up,
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
    /// Whether the symbol draws a terminal (`ew_derive::terminal_symbol`): its tag prints in the terminal style.
    pub terminal: bool,
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
    /// text, else its symbol name, empty when it has neither; the pin's texts then print level in its cell. `None` for
    /// every other pin.
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
    /// The wire's printed number, when the project prints wire numbers.
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

/// The room a block's level tie leaves to each symbol's body: 0.5 mm.
pub const LEVEL_TIE_GAP: Nm = Nm(500_000);

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
    /// ([`crate::knockout`]): the labels layer's [`LayerList::knockouts`], filled by the caller; empty draws
    /// every line whole.
    pub knockouts: Vec<Rect>,
    /// The bridges of the strips whose terminals the page draws, drawn into [`Layer::Wires`] after the junctions
    /// where their terminals stand in one row (module documentation, "Bridges").
    pub bridges: Vec<BridgeView>,
    /// What the layout reads of the page's records beside their geometry, which the labels layer's page
    /// ([`LayerList::draft`]) carries; the default is a page of none.
    pub draft: DraftFacts,
}

/// What the layout reads of a page's records beside their geometry (docs/design/06a-drafting.md §1.6, §2.2), filled
/// by the caller: the default reads an electrical page whose wires carry no supply and whose parts are of no role.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DraftFacts {
    /// Which rules of §1.6 the page is bound by.
    pub kind: ew_drafting::PageKind,
    /// The supply type of each wire's potential, by connection ID (a signal, undefined or unknown type left out).
    pub supply: BTreeMap<Uuid, ew_schema::PotentialType>,
    /// What its records say each representation is, by representation ID.
    pub parts: BTreeMap<Uuid, PartFacts>,
}

/// What a representation's records say it is, for the placer's rules.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PartFacts {
    /// The direction of the interruption point it draws.
    pub ip: Option<ew_schema::IpDirection>,
    /// Its role by its function definitions or its part on a fluid or a bus page (air preparation, valve,
    /// actuator, controller, station), or as a PLC module's box with channels (a card); `None` for no such role.
    pub role: Option<ew_drafting::Role>,
}

/// A strip's bridge (doc 08 §6.5): the clamp it joins on each of its terminals, by the function and CP index
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
    /// The labels layer's texts of representations, each by the items of `list` that print it and what printed it, in
    /// print order. Empty in the other layers.
    pub printed: Vec<PrintedText>,
    /// The labels layer's page as the layout sees it ([`crate::draft`]), whose labels it placed; `None` in the other
    /// layers and on a panel-layout page.
    pub draft: Option<Arc<ew_drafting::DraftPage>>,
    /// The labels layer's boxes of the texts a line of the page runs through, in print order: what the wires layer cuts
    /// its lines round ([`PageView::knockouts`], [`crate::knockout`]). Empty in the other layers.
    pub knockouts: Vec<Rect>,
}

/// What printed a text of a representation on the labels layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Printer {
    /// The symbol's text slot of this index (a cable mark's tag: slot 0 of its wire marker).
    Slot(u16),
    /// A drawn box's pin, by its connection point's index: its designation, and in a card's cell the channel's
    /// function text with it.
    Pin(u16),
    /// A terminal's strip tag.
    StripTag,
}

/// One text of a representation on the labels layer: the items of the layer's list that print it (its lines and the
/// invisible paths of its item links) and what printed it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrintedText {
    /// The representation (a cable mark's wire marker).
    pub rep: Uuid,
    /// What printed it.
    pub by: Printer,
    /// Its items in the layer's list.
    pub items: std::ops::Range<usize>,
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
    /// The anchor a drop stores for the text: the point that gives the same box with the alignment a stored position
    /// prints with (its slot's own, where a text placed anew round a symbol turned a quarter prints otherwise), so a
    /// dragged text lands where its ghost showed it; else `at`.
    pub drop_at: Pt2,
    /// The text turned one, two and three quarters by the Rotate key while it is dragged: the anchor a drop
    /// stores, and the box the text prints there at its new angle and with that angle's alignment, which is its box
    /// turned counter-clockwise on the sheet about `drop_at` (the drag's ghost).
    pub turned: [(Pt2, Rect); 3],
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
    frame_rect(view.frame.as_ref().map(|f| (&*f.form, f.title_block.as_deref())))
}

/// [`page_rect`] of a page drawn in frame `form` with its title block, or without a frame (`None`).
pub fn frame_rect(frame: Option<(&FormDef, Option<&FormDef>)>) -> Rect {
    let sheet = |w: Nm, h: Nm| Rect::from_points(Pt2::ORIGIN, Pt2::new(w, h));
    match frame {
        Some((form, title_block)) => FrameGeometry::of(form, title_block)
            .map_or_else(|| sheet(form.sheet.width, form.sheet.height), |g| g.outer),
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
    let (sheet_width, sheet_height, map) = sheet_of(view.frame.as_ref().map(|f| &*f.form));
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

/// Shapes a symbol-editor text in the style `style_key` (the text's own, else its slot kind's,
/// [`symbol::default_style_key`]) with the same styles, fonts, alignment and readable rotation as page placement.
/// Dynamic slot values are supplied by the editor's preview data.
pub fn symbol_preview_text(
    text: &symbol::PlacedText,
    style_key: &str,
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
            style_key,
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
