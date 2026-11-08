// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page display lists per layer (doc 09 §4.1, ARCH §6.4 `page_display_list(page, layer, lang)`, ADR-0015
//! decision 1 and amendment 2; WP-S3.04).
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
//! | [`Layer::Wires`] | `reps` (CP positions), `connections`, `junctions` | per connection (by ID): its polyline; then per junction (by ID): its dot or bevel symbol |
//! | [`Layer::Labels`] | `reps`, `connections`, `wire_marker`, `items`, `structure_boxes`, `strip_tags` | per representation (by ID): its visible slot texts (a [`Link::Source`] where the slot value names a destination); then the pin texts; then per strip tag (by representation ID): the strip's tag at its terminal ([`StripTagView`]); then per connection with a marker (by ID): the marker box and text; then the free texts, wire-marker groups and structure-box captions ([`crate::items`]) |
//! | [`Layer::Tables`] | `table`, `embedded` | [`crate::table::lower_table`]: the house report table of a generated page; then per embedded block (by its rectangle): [`crate::embedded::lower_embedded`] (the cover's colour and standard-size tables, or the boxed report table of any other kind: PART DATA, the layout legend) |
//! | [`Layer::Surface`], [`Layer::Carriers`], [`Layer::Placements`] | `panel` | the surface view of a panel-layout page ([`crate::panel`]); the labels layer adds its placement labels after the texts above |
//! | others | — | empty: revision marks, highlights, features and dimensions arrive with their work packages |
//!
//! Panel-layout pages ([`PageKind::Panel`]) use [`Layer::PANEL`]; their surface, carrier, placement and label
//! layers draw the page's surface view (WP-S4.01 part 3), the feature and dimension layers stay empty (1.0).
//!
//! All geometry is in **page coordinates** (integer nm, y down). The form's page-to-sheet transform
//! ([`SheetMap`]) is applied once, by [`PageScene::sheet_list`], for every writer.
//!
//! # Rules
//!
//! - **Order.** Representations, connections and junctions are sorted by ID, whatever order the caller
//!   gives them in, so a record shuffle never changes a display list (AC-14).
//! - **CP positions** follow `ew-commands`' `rep_cps`: symbol CP `c` of a representation sits at
//!   `pos + orient(c.pos)` and draws the function CP of its `pin_map` entry, else `(functions[0], c.fn_cp)`;
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
//!   provisional orthogonal route of [`crate::route`] (a placeholder for doc 06 §5.4). Ends are clamped to
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
//! - **Slot texts.** A slot is shown when its override says so, else when it belongs to the
//!   representation's arrangement (FR-SYM-012), else by the symbol's default; an empty value prints
//!   nothing. Position and angle overrides are in the symbol frame. Placement follows
//!   [`crate::symbol`] in the representation's text frame (`ew_derive::text_frame`): the position turns with the
//!   orientation, the angle is made readable, the alignment flips (doc 07 §4.1). The text frame is the
//!   representation's own, except on a black box turned by a half turn without a mirror (FT-867, ID-1137): its texts
//!   are placed as on the same box unturned (origin `2C − pos`, `C` the box's centre on the page, no turn), so they
//!   keep their place relative to the box and their collision rules; the style is the slot's own `style` when it names a known text style, else
//!   [`symbol::default_style_key`] of the slot kind (an unknown name is reported as
//!   [`PageFinding::UnknownSlotStyle`], ID-227). `max_width` is not applied yet.
//!   A text that lists several items carries one link per item ([`SlotValue::item_links`], FT-86): an invisible
//!   path item over the item's glyph span (the pen travel of the text up to the item's ends), after the text's
//!   own items, with the text's entity and a [`Link::Source`].
//! - **Slot text collisions** (FT-687). A cross-reference slot (`xref`) without a position override follows rules 1
//!   and 2, a technical-data slot without one rule 3; every other text stays where its symbol puts it. A black box's
//!   pin texts ("Pin texts" below) take part as fixed obstacles of the cross-reference texts. (1) A list of
//!   several lines aligned to the middle keeps its first line where a one-line text would sit and grows downward in
//!   its own frame (a coil's contact mirror never climbs into the technical data above it). (2) A horizontal one
//!   whose layout box overlaps another slot text's box on the page (its own symbol's or another representation's; the
//!   texts in print order, each seen where it ended), or stands beside another representation's text less than
//!   [`SLOT_TEXT_GAP`] away (cycle 37: "(101-D6)" ran into "-101K1"), drops straight down until its top is
//!   [`SLOT_TEXT_GAP`] below the lowest box it so collided with; when it meets only its own symbol's texts there
//!   (its CP designation reaching under it) it drops below those too, at most three times, and it stays where it was
//!   when another representation's text is there or it finds no free place. At one-column spacing a contact's back-reference so moves below the neighbour contact's
//!   tag. (3) A horizontal technical-data text follows rule 2 against other representations' texts only (cycle 37: a
//!   valve's "24 V" ran into the next valve's tag); within one symbol its author's layout stands. A black box's pin
//!   texts are fixed obstacles of rule 3 as of rule 2 (FT-701): a technical-data text that overlaps one drops below it.
//!   (4) A horizontal cross-reference text that a horizontal wire runs through (FT-1436: on folio 101 the "102-A1" of
//!   L-1 stood across N-1's wire, by N-1's own "102-A1") rises [`SLOT_TEXT_GAP`] above what is in its way, at most
//!   three times, to the first place the device-tag rule judges free and that overlaps no other representation's
//!   symbol body (FT-1519), inside the page's drawing area; without one it stays. Printed text order
//!   and every other position are unchanged. The rules run after "Black-box captions" and "Device tags" below and
//!   see those texts where they ended.
//! - **Black-box captions** (FT-734). A black box's tag, part-number and manufacturer texts without a position or
//!   angle override, printing horizontally, stack outward from the box's top edge in that order, each
//!   [`SLOT_TEXT_GAP`] above the one before, the tag's anchor [`BLACK_BOX_CAPTION_GAP`] above the outline. Without a
//!   CP on the top edge they are left-aligned on the outline's left edge (the tag where the symbol puts it); else
//!   right-aligned one grid step left of the leftmost top CP, so no caption text stands over a CP, its pin text or
//!   its wire. A stack that a stored wire's segment crosses (FT-772) moves left in grid steps, at most until its anchor
//!   reaches the outline's left edge, to the first place no stored wire crosses; without one it stays. A text with an
//!   override stays where it says. A stack that collides with another representation's pin
//!   text (FT-1138) rises [`SLOT_TEXT_GAP`] above what is in its way, at most three times, as a raised device tag
//!   does, to the first place the device-tag rule judges free (ID-1250: no wire runs through it, it collides with no
//!   text or pin text of another representation; a movable device tag yields to it in its turn) and that stays inside
//!   the page's drawing area (FT-1215: the frame's drawing area, else the page); without a free place it stays.
//! - **Device tags** (FT-758). A device tag of a symbol that is no black box, without a position or angle override
//!   and printing horizontally, moves when a wire runs through it, or when it collides (as in rule 2 above) with a
//!   fixed text of another representation (a black-box caption, a CP designation, a pin text) or with the tag of a
//!   representation printed before it. It takes the first free place of: turned to read bottom to top on its side of
//!   the symbol; mirrored about the symbol's vertical axis; mirrored and turned; raised above what is in its way;
//!   dropped below it. A place is free when no wire runs through it, it overlaps no other text of its own symbol and
//!   collides with no text or pin text of another representation (a movable tag printed later yields in its own
//!   turn). A raise or drop goes [`SLOT_TEXT_GAP`] past the texts and horizontal wires in the way, at most three
//!   times, and stays within one tag height of the symbol's top or bottom. Without a free place the
//!   tag stays (at two grid steps' spacing the neighbours leave none).
//! - **Pin texts** (WP-S1.21 D7). A black box prints each [`RepView::pins`] entry whose index is one of its box CPs
//!   (drawn as in "Black boxes") in style `data.small`, inside the outline, [`PIN_TEXT_GAP`] from the pin along its
//!   inward normal: a pin leaving up or down reads bottom to top (90°), its end towards the pin (aligned middle right
//!   below a pin leaving up, middle left above a pin leaving down), but a text of signs only ("+", "−", no letter or
//!   digit, FT-758: a turned "−" read as a tick) reads horizontally, centred on the pin at the same gap (aligned top
//!   centre below a pin leaving up, bottom centre above a pin leaving down); a pin leaving left or right reads
//!   horizontally, its start towards the pin (middle left right of a pin leaving left, middle right left of a pin
//!   leaving right). They print after the slot texts and never move; in the slot-text collision rules they are fixed
//!   obstacles of the cross-reference texts (FT-707): a cross-reference text that overlaps one drops below it as
//!   below a text of its own symbol (rule 2); a device tag yields to one ("Device tags"). An entry without its box
//!   CP, or a pin of an unknown direction, prints nothing.
//! - **Wire markers** (FR-OUT-011 keeps them in monochrome): the caller's marker text, centred in style
//!   `marker` on the midpoint of the polyline's longest segment (the first of equal ones), turned 90° on a
//!   vertical segment, over the marker symbol in the same orientation. Provisional, like the routes.
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
    Page, Representation, SlotKind, SymbolDef,
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
    /// One link per listed item of the text (a cross-reference list, FT-86): each covers the glyph span of
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
    /// For a black box: the derived texts printed beside its pins (the CP designations of FT-707 and the PLC
    /// addresses of WP-S1.21 D7); empty otherwise. See the module documentation ("Pin texts").
    pub pins: Vec<PinText>,
}

/// A derived text printed beside one pin of a black box (WP-S1.21 D7, FT-707): the designation of function CP `idx`
/// of the box's first function and, when that CP is bound to a PLC channel, the channel's printed address after one
/// space. Never stored (FR-PLC-007).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinText {
    /// The function CP index (a box CP's `idx`).
    pub idx: u16,
    /// The text.
    pub text: String,
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
}

/// One junction on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionView {
    /// The record.
    pub junction: Arc<Junction>,
    /// The symbol of its style (`ew.gen.junction-dot` or `ew.gen.junction-bevel`), if available.
    pub symbol: Option<SymbolRef>,
}

/// What one valve position of a module's overview glyph shows (doc 11 §5.5 step 3, WP-S1.20 D19).
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

/// The valve glyphs of one overview representation of a module (`valve_slots`, WP-S1.20 D19): one per position, the
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
    /// The wire-marker symbol (`ew.gen.wire-marker`); markers draw their text only without it.
    pub wire_marker: Option<SymbolRef>,
    /// The report table of a generated page (FT-53); `None` on every other page.
    pub table: Option<TableView>,
    /// The embedded report blocks of a drawn page (FR-REP-028), drawn into the `tables` layer.
    pub embedded: Vec<EmbeddedView>,
    /// The break marks of assembly overviews on this page (doc 11 §5.5, D-41), drawn in [`Layer::Symbols`].
    pub break_marks: Vec<BreakMarkView>,
    /// The valve glyphs of the module overviews on this page (WP-S1.20 D19), drawn in [`Layer::Symbols`] after the
    /// break marks.
    pub valve_glyphs: Vec<ValveGlyphsView>,
    /// The connection points of this page that a link of the export points at (a PLC overview row links to
    /// its coil, doc 10 §5): each gets a named destination ([`Destination::Cp`]) at its drawn position, else
    /// at its representation's position (a CP the symbol does not draw, such as the internal coil 14).
    pub cp_targets: Vec<CpTarget>,
    /// How crossing wires are drawn (FR-CON-031, WP-S2.04 §8.6): the project's drawing setting; the default draws
    /// them plain, byte-equal to a project without the setting.
    pub crossings: Crossings,
    /// The page items of a drawn page with what they print (WP-S2.04 part 4a): graphics in [`Layer::Symbols`],
    /// free texts and wire-marker groups in [`Layer::Labels`] ([`crate::items`]).
    pub items: Vec<ItemView>,
    /// The structure boxes of a drawn page with their captions (WP-S2.04 part 4a): outlines in
    /// [`Layer::Symbols`], captions in [`Layer::Labels`].
    pub structure_boxes: Vec<StructureBoxView>,
    /// The surface view of a panel-layout page (WP-S4.01 part 3, [`crate::panel`]): drawn into [`Layer::Surface`],
    /// [`Layer::Carriers`], [`Layer::Placements`] and [`Layer::Labels`]; `None` on every other page.
    pub panel: Option<PanelView>,
    /// The strip tags printed at terminals drawn alone (doc 08 §6.7, decision log ID-1151), drawn into
    /// [`Layer::Labels`] after the slot and pin texts.
    pub strip_tags: Vec<StripTagView>,
}

/// A strip's tag printed once on a page that shows its terminals without a segment (doc 08 §6.7, decision log
/// ID-1151): at the tag-slot anchor of `rep`, the strip's first terminal representation on the page whose tag slot is
/// shown (`ew_derive::tag_slot_shown`), moved one connection-grid step up in the text's own frame, in the slot's style
/// and horizontal alignment, bottom-aligned.
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
    /// A text slot whose `style` names no text style; the style of the slot's kind was used (ID-227).
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
    /// An image item the export job could not draw: its bytes are not held, or its SVG is refused (FT-384; the
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
    /// The labels layer's anchor of each representation's device tag where it prints, after the tag rule (ID-898)
    /// moved it, by representation ID: what `TableRep::tag_at` reports (FT-771, WP-S2.11 D12). Empty in the other
    /// layers.
    pub tags: BTreeMap<Uuid, Pt2>,
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

/// The page's drawing area in page coordinates (FT-1215): the frame's drawing area inside its grid header, else
/// [`page_rect`].
fn drawing_area(view: &PageView) -> Rect {
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
    styles: &'a TextStyleTable,
    out: LayerList,
}

/// A drawn CP: its position and the direction a wire leaves it.
type CpMap = BTreeMap<(Uuid, u16), (Pt2, Dir4)>;

/// The point of every function CP the page draws, by `(function, CP index)`: the CP positions of the page scene
/// without the directions. `ew-derive`'s `cp_point` restates these rules over `Q` (the wire numbers order
/// wires by the cell of their ends), and a test holds the two equal on the golden pages.
pub fn cp_points(view: &PageView) -> BTreeMap<(Uuid, u16), Pt2> {
    cp_map(view).into_iter().map(|(k, (pos, _))| (k, pos)).collect()
}

fn sorted_reps(view: &PageView) -> Vec<&RepView> {
    let mut reps: Vec<&RepView> = view.reps.iter().collect();
    reps.sort_by_key(|r| r.rep.id);
    reps
}

fn sorted_connections(view: &PageView) -> Vec<&ConnectionView> {
    let mut c: Vec<&ConnectionView> = view.connections.iter().collect();
    c.sort_by_key(|c| c.connection.id);
    c
}

/// The drawn function CPs of the page (see the module documentation).
fn cp_map(view: &PageView) -> CpMap {
    let mut map = CpMap::new();
    for rv in sorted_reps(view) {
        let Some(def) = &rv.symbol else { continue };
        let rep = &rv.rep;
        let mut put = |function: Id<Function>, idx: u16, offset: Pt2, dir: &Dir4| {
            let pos = rep.pos + rep.orient.apply(offset);
            let dir = symbol::transform_dir(dir, rep.orient);
            map.entry((function.as_uuid(), idx)).or_insert((pos, dir));
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
                    map.entry((t.function.as_uuid(), idx)).or_insert((at, dir));
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

fn polyline_of(view: &PageView, cps: &CpMap, c: &Connection, page: Rect) -> Option<Vec<Pt2>> {
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
            let lowered = lower_table(table, sheet, self.fonts, self.styles);
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

    /// The representations' symbols. **A black box drawn with its stored outline** (WP-S2.04 part 3, FR-DEV-039): when
    /// `Representation.box` is set and its outline differs from the symbol's rectangle (its first `rect` primitive),
    /// the rectangle is replaced by the outline as a closed polygon in the rectangle's stroke and fill, under a geometry
    /// key of its own (`SymbolGeomId.extra`: a 16-bit value derived from the outline, probed upwards past a key that
    /// holds other geometry), so it is never shared with another box of that symbol. A box whose outline equals the
    /// rectangle keeps the symbol's own instance (Golden A's four boxes: the output is byte-equal). The text slots stay
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
        // The valve glyphs of the module overviews (WP-S1.20 D19): position p of n is the p-th of n equal cells of the
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
        // The page items' graphics and the structure-box outlines (WP-S2.04 part 4a).
        self.item_graphics(view);
    }

    fn wires(&mut self, view: &PageView) {
        let cps = cp_map(view);
        let page = page_rect(view);
        let stroke = wire_stroke(view.grid);
        // The drawn polylines in connection-ID order (the hop rule of `crate::hops` reads them all first), each with
        // its stroke: a fluid pilot or drain line is dashed 4:2 in the page's wire pen (WP-S1.20 D17). An unrouted
        // connection (WP-S2.19 D3) is the dashed straight line between its ends in the thinnest pen, and no wire: it
        // takes and makes no hop.
        let mut drawn: Vec<(Id<Connection>, Vec<Pt2>)> = Vec::new();
        let mut strokes: Vec<Stroke> = Vec::new();
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
        for (k, (id, pts)) in drawn.iter().enumerate() {
            let path = match hopped.get(k) {
                Some(h) if !h.is_empty() => hop_path(pts, h),
                _ => Path::polyline(pts),
            };
            self.push(Item::path(path, Some(strokes[k].clone()), None).with_entity(EntityRef(id.as_uuid())));
        }
        for (id, pts) in &unrouted {
            let dashed = Stroke::pen(Pen::P013).dashed(DashPattern::Dash42);
            self.push(
                Item::path(Path::polyline(pts), Some(dashed), None).with_entity(EntityRef(id.as_uuid())),
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
    }

    fn labels(&mut self, view: &PageView) {
        // FT-687: the slot texts of every representation are gathered first, so the placement rules of
        // [`SlotText`] can see the other representations' texts; they print in the same order as before.
        let mut texts: Vec<SlotText<'_>> = Vec::new();
        for rv in sorted_reps(view) {
            let Some(def) = &rv.symbol else { continue };
            let rep = &rv.rep;
            let entity = EntityRef(rep.id.as_uuid());
            let black_box = def.cps.is_empty() && rep.box_.is_some();
            // FT-867 (ID-1137): a half-turned black box places its texts as on the same box unturned.
            let frame = ew_derive::text_frame(rep, def);
            let mut values: Vec<&SlotValue> = rv.slots.iter().collect();
            values.sort_by_key(|v| v.slot);
            for value in values {
                let Some(slot) = def.text_slots.get(usize::from(value.slot)) else {
                    continue;
                };
                if value.text.is_empty() {
                    continue;
                }
                let ov = rep.text_slots.iter().find(|o| o.slot == value.slot);
                if !ew_derive::slot_shown(rep, def, value.slot) {
                    continue;
                }
                let moved = ov.and_then(|o| o.pos).is_some();
                let turned = ov.and_then(|o| o.angle).is_some();
                let pos = ov.and_then(|o| o.pos).unwrap_or(slot.pos);
                let rot = ov.and_then(|o| o.angle).unwrap_or(slot.rot);
                let (style_key, unknown) = symbol::slot_style_key(slot, self.styles);
                texts.push(SlotText {
                    symbol: rep.symbol.as_uuid(),
                    value,
                    at: frame.origin + frame.orient.apply(pos),
                    angle: symbol::readable_angle(rot, frame.orient),
                    align: symbol::placed_align(&slot.align, rot, frame.orient),
                    style_key,
                    unknown,
                    entity,
                    xref: matches!(slot.kind, SlotKind::Xref) && !moved,
                    data: matches!(slot.kind, SlotKind::TechData) && !moved,
                    caption: caption_rank(&slot.kind).filter(|_| black_box && !moved && !turned),
                    tag: matches!(slot.kind, SlotKind::Tag),
                    movable_tag: matches!(slot.kind, SlotKind::Tag) && !black_box && !moved && !turned,
                    designation: matches!(slot.kind, SlotKind::CpDesignation(_)),
                });
            }
        }
        let pins = pin_texts(view);
        // FT-707: the pin texts never move; they are fixed obstacles of the cross-reference texts.
        let pin_boxes: Vec<(EntityRef, Rect)> = pins
            .iter()
            .filter_map(|(entity, text, at, angle, align)| {
                let spec = TextSpec {
                    text,
                    at: *at,
                    angle: *angle,
                    align,
                    style_key: "data.small",
                    entity: *entity,
                    link: None,
                    items: &[],
                };
                self.text_box(&spec, false).map(|b| (*entity, b))
            })
            .collect();
        self.place_black_box_captions(view, &mut texts, &pin_boxes);
        self.place_tags_off_own_mark(view, &mut texts);
        self.place_tags(view, &mut texts, &pin_boxes);
        self.place_slot_texts(&mut texts, &pin_boxes);
        self.place_xrefs_off_wires(view, &mut texts, &pin_boxes);
        for t in texts.iter().filter(|t| t.tag) {
            self.out.tags.entry(t.entity.0).or_insert(t.at);
        }
        for t in &texts {
            if let Some(name) = t.unknown {
                self.out.findings.push(PageFinding::UnknownSlotStyle {
                    entity: t.entity.0,
                    symbol: t.symbol,
                    slot: t.value.slot,
                    name: name.to_owned(),
                });
            }
            let conflict = t.value.link.is_some() && !t.value.item_links.is_empty();
            if conflict {
                self.out.findings.push(PageFinding::ConflictingLinks {
                    entity: t.entity.0,
                    slot: t.value.slot,
                });
            }
            let spec = t.spec(conflict);
            self.text_placed(&spec, t.xref);
        }
        for (entity, text, at, angle, align) in &pins {
            self.text(&TextSpec {
                text,
                at: *at,
                angle: *angle,
                align,
                style_key: "data.small",
                entity: *entity,
                link: None,
                items: &[],
            });
        }
        self.strip_tags(view);
        self.connection_markers(view);
        // The free texts, wire-marker groups and structure-box captions (WP-S2.04 part 4a).
        self.item_labels(view);
    }

    /// The strip tags of terminals drawn alone ([`PageView::strip_tags`], ID-1151), by representation ID. They are
    /// no obstacle of the placement rules above and are not moved by them.
    fn strip_tags(&mut self, view: &PageView) {
        let step = view.grid.at(1).unwrap_or(Nm::ZERO);
        let mut tags: Vec<&StripTagView> = view.strip_tags.iter().collect();
        tags.sort_by_key(|t| t.rep);
        for tag in tags {
            let Some(rv) = view.reps.iter().find(|r| r.rep.id == tag.rep) else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let rep = &rv.rep;
            // The caller picks a terminal whose tag slot is shown; a hidden one prints no strip tag either.
            let Some(i) = ew_derive::tag_slot_shown(rep, def) else {
                continue;
            };
            let Some(slot) = def.text_slots.get(usize::from(i)) else {
                continue;
            };
            let frame = ew_derive::text_frame(rep, def);
            let ov = rep.text_slots.iter().find(|o| o.slot == i);
            let pos = ov.and_then(|o| o.pos).unwrap_or(slot.pos);
            let rot = ov.and_then(|o| o.angle).unwrap_or(slot.rot);
            let angle = symbol::readable_angle(rot, frame.orient);
            let align = bottom_align(&symbol::placed_align(&slot.align, rot, frame.orient));
            let at = frame.origin + frame.orient.apply(pos) + up_by(angle, step);
            let (style_key, _) = symbol::slot_style_key(slot, self.styles);
            self.text(&TextSpec {
                text: &tag.text,
                at,
                angle,
                align: &align,
                style_key,
                entity: EntityRef(rep.id.as_uuid()),
                link: None,
                items: &[],
            });
        }
    }

    /// The caption of each black box (FT-734, module documentation, "Black-box captions"): its tag, part-number and
    /// manufacturer texts, each without a position or angle override and printing horizontally, stacked outward from
    /// the box's top edge in that order, [`SLOT_TEXT_GAP`] apart, the tag's anchor [`BLACK_BOX_CAPTION_GAP`] above
    /// the outline. Left-aligned on the outline's left edge when the top edge has no CP; else right-aligned one grid
    /// step left of the leftmost top CP, so that no caption text stands over a CP, its pin text or its wire. A stack
    /// that collides with another representation's pin text (`pins`, FT-1138) rises [`SLOT_TEXT_GAP`] above what is in
    /// its way, at most [`SLOT_TEXT_DROPS`] times, as a raised device tag does, to the first place the tag rule's free
    /// test ([`in_the_way`], ID-1250) passes, never above the page's drawing area ([`drawing_area`], FT-1215); without a
    /// free place it stays. The stacks rise after all are stacked.
    fn place_black_box_captions(
        &self,
        view: &PageView,
        texts: &mut [SlotText<'_>],
        pins: &[(EntityRef, Rect)],
    ) {
        let step = view.grid.at(1).unwrap_or(Nm::ZERO);
        let mut stacks: Vec<(EntityRef, Vec<usize>, Rect)> = Vec::new();
        let mut stored: Option<Vec<(Pt2, Pt2)>> = None;
        for rv in sorted_reps(view) {
            let rep = &rv.rep;
            let (Some(def), Some(b)) = (&rv.symbol, &rep.box_) else {
                continue;
            };
            if !def.cps.is_empty() {
                continue;
            }
            let entity = EntityRef(rep.id.as_uuid());
            let mut lines: Vec<(u8, usize)> = texts
                .iter()
                .enumerate()
                .filter(|(_, t)| t.entity == entity && t.angle == Angle2::ZERO)
                .filter_map(|(i, t)| t.caption.map(|r| (r, i)))
                .collect();
            if lines.is_empty() {
                continue;
            }
            lines.sort_unstable();
            let corners: Vec<Pt2> = b.outline.iter().map(|p| rep.pos + rep.orient.apply(*p)).collect();
            let (Some(left), Some(top)) = (
                corners.iter().map(|p| p.x).min(),
                corners.iter().map(|p| p.y).min(),
            ) else {
                continue;
            };
            let first_top_cp = b
                .cps
                .iter()
                .filter(|cp| symbol::transform_dir(&cp.dir, rep.orient) == Dir4::Up)
                .map(|cp| (rep.pos + rep.orient.apply(cp.pos)).x)
                .min();
            let (x, align) = match first_top_cp {
                Some(cx) => (cx - step, Align::BottomRight),
                None => (left, Align::BottomLeft),
            };
            // The first line's anchor; each further line's box ends `SLOT_TEXT_GAP` above the previous one's.
            let mut anchor = top - BLACK_BOX_CAPTION_GAP;
            let mut above: Option<Nm> = None;
            let mut stack: Option<Rect> = None;
            for &(_, i) in &lines {
                texts[i].align = align.clone();
                texts[i].at = Pt2::new(x, anchor);
                let Some(bx) = self.text_box(&texts[i].spec(false), false) else {
                    continue;
                };
                if let Some(limit) = above {
                    let dy = Nm(limit.0 - SLOT_TEXT_GAP.0 - bx.max().y.0);
                    texts[i].at = Pt2::new(x, anchor + dy);
                    above = Some(bx.min().y + dy);
                } else {
                    above = Some(bx.min().y);
                }
                anchor = texts[i].at.y;
                if let Some(bx) = self.text_box(&texts[i].spec(false), false) {
                    stack = Some(stack.map_or(bx, |s| {
                        Rect::from_points(
                            Pt2::new(s.min().x.min(bx.min().x), s.min().y.min(bx.min().y)),
                            Pt2::new(s.max().x.max(bx.max().x), s.max().y.max(bx.max().y)),
                        )
                    }));
                }
            }
            if let Some(mut own) = stack {
                // FT-772 (WP-S2.11 D11): a stack across a stored wire moves left in grid steps, at most to the
                // outline's left edge, to the first place where no caption box crosses one; else it stays.
                let wires = stored.get_or_insert_with(|| stored_wires(view));
                let boxes: Vec<Rect> = lines
                    .iter()
                    .filter_map(|&(_, i)| self.text_box(&texts[i].spec(false), false))
                    .collect();
                let crosses = |dx: Nm| {
                    boxes
                        .iter()
                        .any(|b| wires.iter().any(|w| wire_through(shifted(*b, dx), *w)))
                };
                if step > Nm::ZERO && crosses(Nm::ZERO) {
                    let mut dx = Nm(-step.0);
                    while x + dx >= left {
                        if !crosses(dx) {
                            for &(_, i) in &lines {
                                texts[i].at = texts[i].at + Pt2::new(dx, Nm::ZERO);
                            }
                            own = shifted(own, dx);
                            break;
                        }
                        dx -= step;
                    }
                }
                stacks.push((entity, lines.iter().map(|&(_, i)| i).collect(), own));
            }
        }
        // FT-1138 (ID-1228 item 6, ID-1250): a stack over another representation's pin text rises to the first place
        // the tag rule judges free. A movable device tag does not count: it yields to the caption in its turn.
        if stacks.is_empty() {
            return;
        }
        let wires = page_wires(view);
        let area = drawing_area(view);
        let entities: Vec<EntityRef> = texts.iter().map(|t| t.entity).collect();
        let movable: Vec<bool> = texts
            .iter()
            .map(|t| t.movable_tag && t.angle == Angle2::ZERO)
            .collect();
        for (entity, lines, own) in stacks {
            if !pins.iter().any(|(e, o)| *e != entity && collides(own, *o)) {
                continue;
            }
            let boxes: Vec<Option<Rect>> = texts
                .iter()
                .map(|t| self.text_box(&t.spec(false), t.xref))
                .collect();
            let in_way = |b: Rect| {
                in_the_way(
                    b,
                    entity,
                    (&boxes, &entities),
                    |k| movable[k] || lines.contains(&k),
                    pins,
                    &wires,
                )
            };
            let mut b = own;
            let mut found = false;
            for _ in 0..SLOT_TEXT_DROPS {
                let Some(top) = in_way(b).and_then(|w| w.iter().map(|w| w.0).min()) else {
                    break;
                };
                let dy = Nm(top.0 - SLOT_TEXT_GAP.0 - b.max().y.0);
                b = Rect::from_points(
                    Pt2::new(b.min().x, b.min().y + dy),
                    Pt2::new(b.max().x, b.max().y + dy),
                );
                // FT-1215: the stack never rises out of the drawing area (into the frame, or off the sheet).
                if b.min().y < area.min().y {
                    break;
                }
                if in_way(b).is_some_and(|w| w.is_empty()) {
                    found = true;
                    break;
                }
            }
            if found {
                let dy = b.min().y - own.min().y;
                for &i in &lines {
                    texts[i].at = texts[i].at + Pt2::new(Nm::ZERO, dy);
                }
            }
        }
    }

    /// The tag rule (FT-758, module documentation, "Device tags"): a movable device tag that a wire runs through, or
    /// that collides with a fixed text of another representation (a black box's caption or pin text, a CP
    /// designation) or with the tag of a representation printed before it, takes the first free place of: turned to
    /// read bottom to top on its side of its symbol; mirrored to the other side; mirrored and turned; raised above
    /// what is in its way; dropped below it. A place is free when no wire runs through it, it overlaps no other text
    /// of its own symbol and collides with no other representation's text or pin text (a movable tag printed later
    /// does not count: it yields in its turn). A raise or drop goes [`SLOT_TEXT_GAP`] past the texts and horizontal
    /// wires in the way, at most [`SLOT_TEXT_DROPS`] times, and keeps within one tag height of its symbol's top or
    /// bottom; another wire in the way ends it. Without a free place the tag stays.
    /// A centred device tag whose anchor is inside its own symbol's body but whose text runs out across the body's
    /// outline (a cable marker's tag, wider than its mark: "-1[06W]1", cycle 39 window check) moves beside the body, as
    /// the other tags stand: [`SLOT_TEXT_GAP`] right of it, middle left, at the body's vertical centre. A tag inside its
    /// body, or one already outside it, stays; the tag rule ([`Self::place_tags`]) then judges the new place.
    fn place_tags_off_own_mark(&self, view: &PageView, texts: &mut [SlotText<'_>]) {
        for t in texts.iter_mut() {
            if !t.movable_tag || t.angle != Angle2::ZERO || t.align != Align::Centre {
                continue;
            }
            let Some(rv) = view
                .reps
                .iter()
                .find(|r| EntityRef(r.rep.id.as_uuid()) == t.entity)
            else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let b = symbol::lower(def, rv.rep.orient).bounds;
            let pos = rv.rep.pos;
            let body = Rect::from_points(
                Pt2::new(pos.x + b.min().x, pos.y + b.min().y),
                Pt2::new(pos.x + b.max().x, pos.y + b.max().y),
            );
            let inside =
                |p: Pt2| p.x > body.min().x && p.x < body.max().x && p.y > body.min().y && p.y < body.max().y;
            if !inside(t.at) {
                continue;
            }
            let Some(own) = self.text_box(&t.spec(false), false) else {
                continue;
            };
            let within = own.min().x >= body.min().x
                && own.max().x <= body.max().x
                && own.min().y >= body.min().y
                && own.max().y <= body.max().y;
            if within {
                continue;
            }
            t.at = Pt2::new(
                body.max().x + SLOT_TEXT_GAP,
                Nm((body.min().y.0 + body.max().y.0) / 2),
            );
            t.align = Align::MiddleLeft;
        }
    }

    fn place_tags(&self, view: &PageView, texts: &mut [SlotText<'_>], pins: &[(EntityRef, Rect)]) {
        if !texts.iter().any(|t| t.movable_tag) {
            return;
        }
        let wires = page_wires(view);
        let mut boxes: Vec<Option<Rect>> = texts
            .iter()
            .map(|t| self.text_box(&t.spec(false), t.xref))
            .collect();
        let entities: Vec<EntityRef> = texts.iter().map(|t| t.entity).collect();
        let movable: Vec<bool> = texts
            .iter()
            .map(|t| t.movable_tag && t.angle == Angle2::ZERO)
            .collect();
        for i in 0..texts.len() {
            let Some(own) = boxes[i] else { continue };
            if !movable[i] {
                continue;
            }
            let entity = entities[i];
            // Where it stands it yields to the fixed texts (captions, pins, CP designations) and to tags printed
            // before it.
            let yields = boxes.iter().enumerate().any(|(k, b)| {
                k != i
                    && entities[k] != entity
                    && (texts[k].caption.is_some() || texts[k].designation || (texts[k].tag && k < i))
                    && b.is_some_and(|b| collides(own, b))
            }) || pins.iter().any(|(e, b)| *e != entity && collides(own, *b))
                || wires.iter().any(|w| wire_through(own, *w));
            if !yields {
                continue;
            }
            // A movable tag printed later yields to this one in its turn.
            let in_way = |b: Rect, boxes: &[Option<Rect>]| {
                in_the_way(
                    b,
                    entity,
                    (boxes, &entities),
                    |k| k == i || (k > i && movable[k]),
                    pins,
                    &wires,
                )
            };
            let free = |b: Rect, boxes: &[Option<Rect>]| in_way(b, boxes).is_some_and(|v| v.is_empty());
            let Some(rv) = view.reps.iter().find(|r| EntityRef(r.rep.id.as_uuid()) == entity) else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let bounds = symbol::lower(def, rv.rep.orient).bounds;
            // Twice the x of the symbol's vertical axis, and the band a raised or dropped tag keeps within.
            let axis2 = Nm(2 * rv.rep.pos.x.0 + bounds.min().x.0 + bounds.max().x.0);
            let height = own.max().y - own.min().y;
            let band = (
                rv.rep.pos.y + bounds.min().y - height,
                rv.rep.pos.y + bounds.max().y + height,
            );
            let at = texts[i].at;
            let (angle, align) = (texts[i].angle, texts[i].align.clone());
            let mirror_at = Pt2::new(axis2 - at.x, at.y);
            let mirror_align = mirrored_align(&align);
            let mut places: Vec<(Pt2, Angle2, Align)> = Vec::new();
            if let Some(a) = turned_align(&align) {
                places.push((at, TURNED, a));
            }
            places.push((mirror_at, Angle2::ZERO, mirror_align.clone()));
            if let Some(a) = turned_align(&mirror_align) {
                places.push((mirror_at, TURNED, a));
            }
            let mut found: Option<(Pt2, Angle2, Align, Rect)> = None;
            for (to, angle, align) in places {
                texts[i].at = to;
                texts[i].angle = angle;
                texts[i].align = align.clone();
                if let Some(b) = self
                    .text_box(&texts[i].spec(false), false)
                    .filter(|b| free(*b, &boxes))
                {
                    found = Some((to, angle, align, b));
                    break;
                }
            }
            // Raised above, then dropped below, what is in its way.
            for up in [true, false] {
                if found.is_some() {
                    break;
                }
                let mut b = own;
                for _ in 0..SLOT_TEXT_DROPS {
                    let Some(way) = in_way(b, &boxes) else { break };
                    let dy = if up {
                        let Some(top) = way.iter().map(|w| w.0).min() else {
                            break;
                        };
                        Nm(top.0 - SLOT_TEXT_GAP.0 - b.max().y.0)
                    } else {
                        let Some(bottom) = way.iter().map(|w| w.1).max() else {
                            break;
                        };
                        Nm(bottom.0 + SLOT_TEXT_GAP.0 - b.min().y.0)
                    };
                    b = Rect::from_points(
                        Pt2::new(b.min().x, b.min().y + dy),
                        Pt2::new(b.max().x, b.max().y + dy),
                    );
                    if b.max().y <= band.0 || b.min().y >= band.1 {
                        break;
                    }
                    if free(b, &boxes) {
                        let to = at + Pt2::new(Nm::ZERO, b.min().y - own.min().y);
                        found = Some((to, angle, align.clone(), b));
                        break;
                    }
                }
            }
            let t = &mut texts[i];
            match found {
                Some((to, angle, align, b)) => {
                    t.at = to;
                    t.angle = angle;
                    t.align = align;
                    boxes[i] = Some(b);
                }
                None => {
                    t.at = at;
                    t.angle = angle;
                    t.align = align;
                }
            }
        }
    }

    /// The placement rules of the slot texts (FT-687, module documentation, "Slot text collisions"): a
    /// horizontal cross-reference text that overlaps another slot text on the page (its own symbol's or another
    /// representation's) drops below the lowest of those texts, [`SLOT_TEXT_GAP`] apart, when it overlaps no
    /// slot text there; texts are taken in print order, each seeing the earlier ones where they ended. The black
    /// boxes' pin texts (`pins`, FT-707) are fixed obstacles of the cross-reference texts and, since FT-701, of the
    /// technical-data texts: one that such a text overlaps sends it down like a slot text of the same
    /// representation does.
    fn place_slot_texts(&self, texts: &mut [SlotText<'_>], pins: &[(EntityRef, Rect)]) {
        let mut boxes: Vec<Option<Rect>> = texts
            .iter()
            .map(|t| self.text_box(&t.spec(false), t.xref))
            .collect();
        for i in 0..texts.len() {
            let Some(own) = boxes[i] else { continue };
            if !(texts[i].xref || texts[i].data) || texts[i].angle != Angle2::ZERO {
                continue;
            }
            // Each other box with whether it is the same representation's: within one symbol only an overlap
            // collides (its author placed the texts), between representations also a gap below [`SLOT_TEXT_GAP`].
            // A technical-data text yields only to other representations' texts.
            // A pin text counts as a text of the same representation: only an overlap collides, and it sends the
            // text further down; FT-701 (WP-S2.11 D10): so for a technical-data text too (`pin`).
            let others = || {
                boxes
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| *k != i)
                    .filter_map(|(k, b)| b.map(|b| (texts[k].entity == texts[i].entity, false, b)))
                    .chain(pins.iter().map(|(_, b)| (true, true, *b)))
            };
            let hits = |at: Rect, (same, pin, b): (bool, bool, Rect)| {
                if pin {
                    overlaps(at, b)
                } else if same {
                    !texts[i].data && overlaps(at, b)
                } else {
                    collides(at, b)
                }
            };
            let Some(mut bottom) = others()
                .filter(|o| hits(own, *o))
                .map(|(_, _, b)| b.max().y)
                .max()
            else {
                continue;
            };
            // Where it lands it may still meet a text of its own symbol (its CP designation reaching under it): it
            // drops below that one too, at most [`SLOT_TEXT_DROPS`] times; else it stays where it was.
            let blocked = |at: Rect, (same, _, b): (bool, bool, Rect)| {
                if same { overlaps(at, b) } else { collides(at, b) }
            };
            for _ in 0..SLOT_TEXT_DROPS {
                let drop = Nm(bottom.0 + SLOT_TEXT_GAP.0 - own.min().y.0);
                let moved = Rect::from_points(
                    Pt2::new(own.min().x, own.min().y + drop),
                    Pt2::new(own.max().x, own.max().y + drop),
                );
                let blockers: Vec<(bool, bool, Rect)> = others().filter(|o| blocked(moved, *o)).collect();
                if blockers.is_empty() {
                    texts[i].at = texts[i].at + Pt2::new(Nm::ZERO, drop);
                    boxes[i] = Some(moved);
                    break;
                }
                // Only its own symbol's texts send it further down; another representation's text there means no
                // free place nearby, and it stays.
                if blockers.iter().any(|(same, _, _)| !same) {
                    break;
                }
                bottom = blockers.iter().map(|(_, _, b)| b.max().y).max().unwrap_or(bottom);
            }
        }
    }

    /// The wire rule of the cross-reference texts (FT-1436, module documentation, "Slot text collisions" rule 4): a
    /// horizontal cross-reference text that a horizontal wire runs through rises [`SLOT_TEXT_GAP`] above what is in
    /// its way, at most [`SLOT_TEXT_DROPS`] times, to the first place the device-tag rule judges free (no wire runs
    /// through it, it overlaps no other text of its own symbol and collides with no text or pin text of another
    /// representation, and overlaps no other representation's symbol body, FT-1519) inside the page's drawing area;
    /// without a free place it stays.
    fn place_xrefs_off_wires(&self, view: &PageView, texts: &mut [SlotText<'_>], pins: &[(EntityRef, Rect)]) {
        if !texts.iter().any(|t| t.xref && t.angle == Angle2::ZERO) {
            return;
        }
        let wires = page_wires(view);
        let bodies = symbol_bodies(view);
        let area = drawing_area(view);
        let mut boxes: Vec<Option<Rect>> = texts
            .iter()
            .map(|t| self.text_box(&t.spec(false), t.xref))
            .collect();
        let entities: Vec<EntityRef> = texts.iter().map(|t| t.entity).collect();
        for i in 0..texts.len() {
            if !texts[i].xref || texts[i].angle != Angle2::ZERO {
                continue;
            }
            // The printed text, every line of a list where the list prints (C52-p3#1: grown down from its first line),
            // is judged against the wires.
            let Some(own) = boxes[i] else {
                continue;
            };
            if !wires
                .iter()
                .any(|&(a, c)| a.y == c.y && wire_through(own, (a, c)))
            {
                continue;
            }
            let entity = entities[i];
            let in_way = |b: Rect, boxes: &[Option<Rect>]| {
                with_bodies(
                    in_the_way(b, entity, (boxes, &entities), |k| k == i, pins, &wires),
                    b,
                    entity,
                    &bodies,
                )
            };
            let mut b = own;
            let mut found = false;
            for _ in 0..SLOT_TEXT_DROPS {
                let Some(top) = in_way(b, &boxes).and_then(|w| w.iter().map(|w| w.0).min()) else {
                    break;
                };
                let dy = Nm(top.0 - SLOT_TEXT_GAP.0 - b.max().y.0);
                b = Rect::from_points(
                    Pt2::new(b.min().x, b.min().y + dy),
                    Pt2::new(b.max().x, b.max().y + dy),
                );
                if b.min().y < area.min().y {
                    break;
                }
                if in_way(b, &boxes).is_some_and(|w| w.is_empty()) {
                    found = true;
                    break;
                }
            }
            if found {
                let dy = b.min().y - own.min().y;
                texts[i].at = texts[i].at + Pt2::new(Nm::ZERO, dy);
                boxes[i] = boxes[i].map(|r| {
                    Rect::from_points(
                        Pt2::new(r.min().x, r.min().y + dy),
                        Pt2::new(r.max().x, r.max().y + dy),
                    )
                });
            }
        }
    }

    /// The per-connection wire markers, by connection ID.
    fn connection_markers(&mut self, view: &PageView) {
        let markers: Vec<&ConnectionView> = sorted_connections(view)
            .into_iter()
            .filter(|c| c.marker.as_deref().is_some_and(|m| !m.is_empty()))
            .collect();
        if markers.is_empty() {
            return;
        }
        let cps = cp_map(view);
        let page = page_rect(view);
        for cv in markers {
            let c = &cv.connection;
            let Some(pts) = polyline_of(view, &cps, c, page) else {
                continue;
            };
            let Some((at, vertical)) = marker_point(&pts) else {
                continue;
            };
            let entity = EntityRef(c.id.as_uuid());
            let (orient, angle) = if vertical {
                (Orient2::R270, Angle2(90_000))
            } else {
                (Orient2::R0, Angle2::ZERO)
            };
            if let Some(s) = &view.wire_marker {
                let key = self.geometry(s, orient);
                self.push(Item::instance(key, Transform::from_translation(at)).with_entity(entity));
            }
            let text = cv.marker.clone().unwrap_or_default();
            self.text(&TextSpec {
                text: &text,
                at,
                angle,
                align: &Align::Centre,
                style_key: MARKER_STYLE,
                entity,
                link: None,
                items: &[],
            });
        }
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
    /// where a one-line text would sit and grows downward in its own frame (FT-687, module documentation).
    fn text_placed(&mut self, spec: &TextSpec<'_>, first_line: bool) {
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
    fn text_box(&self, spec: &TextSpec<'_>, first_line: bool) -> Option<Rect> {
        if spec.text.is_empty() {
            return None;
        }
        let style = self.style_quiet(spec.style_key);
        let shaped = self.fonts.shape(spec.text, &style, None);
        let lines = self.lay_out(spec, &style, &shaped, first_line);
        let mut out: Option<Rect> = None;
        for item in &lines {
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

/// The boxes of the glyph spans `ranges` of `text` as placed (ADR-0015 amendment 9), one per range in their order: over
/// the span's glyphs where they are drawn, from the ascent to the descent `m` of the text's style, mapped to the parent
/// through the placed line item that holds the span (so it turns with the text). `shaped` is `text` shaped as drawn
/// and `lines` its placed line items, one per shaped line. `None` for a range that no line holds whole, whose line is
/// not placed, or that holds no glyph of a line with a right-to-left run. The cross-reference texts' item links and
/// the report tables' item links (WP-S3.06 ruling 3) both take their boxes from here.
///
/// It shapes nothing (C44-p3#1, C44-p5#2, C44-p6#1): the boxes come from the drawn shaping's glyph positions and
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

/// The distance of a black box's pin text from its pin, inward along the pin's normal (WP-S1.21 D7): 1 mm.
pub const PIN_TEXT_GAP: Nm = Nm(1_000_000);

/// A placed pin text: its entity, text, point, angle and alignment.
type PlacedPin = (EntityRef, String, Pt2, Angle2, Align);

/// The pin texts of the page's black boxes, by representation ID then box-CP order (module documentation, "Pin
/// texts").
fn pin_texts(view: &PageView) -> Vec<PlacedPin> {
    let mut out = Vec::new();
    for rv in sorted_reps(view) {
        let rep = &rv.rep;
        let (Some(def), Some(b)) = (&rv.symbol, &rep.box_) else {
            continue;
        };
        if rv.pins.is_empty() || !def.cps.is_empty() {
            continue;
        }
        for cp in &b.cps {
            let Some(pin) = rv.pins.iter().find(|p| p.idx == cp.idx) else {
                continue;
            };
            if pin.text.is_empty() {
                continue;
            }
            let at = rep.pos + rep.orient.apply(cp.pos);
            let g = PIN_TEXT_GAP;
            // FT-758: a text of signs only ("+", "−") reads the same upright, and turned a minus would read as a tick.
            let signs = !pin.text.chars().any(char::is_alphanumeric);
            let (offset, angle, align) = match symbol::transform_dir(&cp.dir, rep.orient) {
                Dir4::Up if signs => (Pt2::new(Nm::ZERO, g), Angle2::ZERO, Align::TopCentre),
                Dir4::Down if signs => (Pt2::new(Nm::ZERO, Nm(-g.0)), Angle2::ZERO, Align::BottomCentre),
                Dir4::Up => (Pt2::new(Nm::ZERO, g), Angle2(90_000), Align::MiddleRight),
                Dir4::Down => (Pt2::new(Nm::ZERO, Nm(-g.0)), Angle2(90_000), Align::MiddleLeft),
                Dir4::Left => (Pt2::new(g, Nm::ZERO), Angle2::ZERO, Align::MiddleLeft),
                Dir4::Right => (Pt2::new(Nm(-g.0), Nm::ZERO), Angle2::ZERO, Align::MiddleRight),
                Dir4::Unknown(_) => continue,
            };
            out.push((
                EntityRef(rep.id.as_uuid()),
                pin.text.clone(),
                at + offset,
                angle,
                align,
            ));
        }
    }
    out
}

/// How far a black box's caption stands above its outline (FT-734): the black-box symbol's own tag-slot offset, a
/// quarter of a QET grid step (0.661 mm).
pub const BLACK_BOX_CAPTION_GAP: Nm = Nm(661_458);

/// The gap a dropped cross-reference text keeps below the text it moved away from (FT-687): 0.5 mm.
pub const SLOT_TEXT_GAP: Nm = Nm(500_000);

/// The most times one slot text drops (FT-687, cycle 37): below the text it collided with, then below a text it
/// meets there, and so on.
const SLOT_TEXT_DROPS: usize = 3;

/// One visible slot text of a representation before it prints (FT-687): what [`TextSpec`] needs and whether the
/// slot is a cross-reference the placement rules may move.
struct SlotText<'v> {
    symbol: Uuid,
    value: &'v SlotValue,
    at: Pt2,
    angle: Angle2,
    align: Align,
    style_key: &'v str,
    unknown: Option<&'v str>,
    entity: EntityRef,
    /// A cross-reference slot at the symbol's own position (no position override).
    xref: bool,
    /// A technical-data slot at the symbol's own position (no position override): it drops like a cross-reference
    /// when it collides with another representation's text (cycle 37: a valve's "24 V" ran into the next tag).
    data: bool,
    /// A black box's caption line at the symbol's own position and angle (no override): its place in the caption
    /// stack ([`caption_rank`], FT-734).
    caption: Option<u8>,
    /// A device-tag slot.
    tag: bool,
    /// A device tag the tag rule may move (FT-758): not a black box's, without a position or angle override.
    movable_tag: bool,
    /// A CP designation slot: a fixed text the tag rule yields to.
    designation: bool,
}

/// `align` with its horizontal part kept and its vertical part at the bottom (a strip tag, ID-1151).
fn bottom_align(align: &Align) -> Align {
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
fn up_by(angle: Angle2, step: Nm) -> Pt2 {
    // `normalized` is below 360°, so adding a quarter turn cannot overflow.
    let up = Angle2(angle.normalized().0 + 90_000);
    ew_dl::num::arc_point(Pt2::ORIGIN, step, step, up)
}

/// `align` mirrored left to right.
fn mirrored_align(align: &Align) -> Align {
    match align {
        Align::TopLeft => Align::TopRight,
        Align::TopRight => Align::TopLeft,
        Align::MiddleLeft => Align::MiddleRight,
        Align::MiddleRight => Align::MiddleLeft,
        Align::BottomLeft => Align::BottomRight,
        Align::BottomRight => Align::BottomLeft,
        other => other.clone(),
    }
}

/// The angle of a turned device tag (FT-758): it reads bottom to top.
const TURNED: Angle2 = Angle2(90_000);

/// The alignment of a horizontal tag aligned `align` once turned to read bottom to top on the same side of its symbol
/// (FT-758): a tag that ends towards its symbol on the left (middle right) keeps its right edge there (its descent
/// side, bottom centre), one that starts towards it on the right (middle left) its left edge (top centre); `None` for
/// any other alignment.
fn turned_align(align: &Align) -> Option<Align> {
    match align {
        Align::MiddleRight => Some(Align::BottomCentre),
        Align::MiddleLeft => Some(Align::TopCentre),
        _ => None,
    }
}

/// The page's wire segments, connections in ID order (FT-758): the obstacles of the tag rule.
fn page_wires(view: &PageView) -> Vec<(Pt2, Pt2)> {
    let cps = cp_map(view);
    let page = page_rect(view);
    let mut wires: Vec<(Pt2, Pt2)> = Vec::new();
    // An unrouted connection has no wire geometry (WP-S2.19 D3).
    for cv in sorted_connections(view)
        .into_iter()
        .filter(|cv| !cv.connection.route.is_empty())
    {
        if let Some(pts) = polyline_of(view, &cps, &cv.connection, page) {
            wires.extend(pts.windows(2).map(|w| (w[0], w[1])));
        }
    }
    wires
}

/// The segments of the page's stored wires (a connection with a stored route), connections in ID order: the obstacles
/// of a black box's caption stack (FT-772).
fn stored_wires(view: &PageView) -> Vec<(Pt2, Pt2)> {
    let cps = cp_map(view);
    let page = page_rect(view);
    let mut wires: Vec<(Pt2, Pt2)> = Vec::new();
    for cv in sorted_connections(view)
        .into_iter()
        .filter(|cv| !cv.connection.route.is_empty())
    {
        if let Some(pts) = polyline_of(view, &cps, &cv.connection, page) {
            wires.extend(pts.windows(2).map(|w| (w[0], w[1])));
        }
    }
    wires
}

/// `r` moved across by `dx`.
fn shifted(r: Rect, dx: Nm) -> Rect {
    Rect::from_points(
        Pt2::new(r.min().x + dx, r.min().y),
        Pt2::new(r.max().x + dx, r.max().y),
    )
}

/// The tag rule's free test (FT-758, module documentation, "Device tags"; FT-1138's risen caption, ID-1250): what is
/// in the way of a place `b` of a text of `entity`, as the vertical extent of each text (`texts`: the boxes and their
/// entities, but those `skip` names) and pin text there, and of each horizontal wire through it; `None` when another
/// wire runs through it. Empty: the place is free. A text or pin text of `entity` is in the way when it overlaps the
/// place; another representation's when it collides with it.
fn in_the_way(
    b: Rect,
    entity: EntityRef,
    (boxes, entities): (&[Option<Rect>], &[EntityRef]),
    skip: impl Fn(usize) -> bool,
    pins: &[(EntityRef, Rect)],
    wires: &[(Pt2, Pt2)],
) -> Option<Vec<(Nm, Nm)>> {
    let mut out: Vec<(Nm, Nm)> = Vec::new();
    for (k, o) in boxes.iter().enumerate() {
        if skip(k) {
            continue;
        }
        let Some(o) = *o else { continue };
        let hit = if entities[k] == entity {
            overlaps(b, o)
        } else {
            collides(b, o)
        };
        if hit {
            out.push((o.min().y, o.max().y));
        }
    }
    for (e, o) in pins {
        let hit = if *e == entity {
            overlaps(b, *o)
        } else {
            collides(b, *o)
        };
        if hit {
            out.push((o.min().y, o.max().y));
        }
    }
    for &(a, c) in wires {
        if wire_through(b, (a, c)) {
            if a.y != c.y {
                return None;
            }
            out.push((a.y, a.y));
        }
    }
    Some(out)
}

/// The outline bounds of each representation's symbol on the page (`symbol::lower(def, orient).bounds` at its
/// position, as the tag rule reads them), with the representation: the symbol bodies the rise of the cross-reference
/// texts keeps clear of (FT-1519, ID-1455).
fn symbol_bodies(view: &PageView) -> Vec<(EntityRef, Rect)> {
    view.reps
        .iter()
        .filter_map(|rv| {
            let def = rv.symbol.as_ref()?;
            let b = symbol::lower(def, rv.rep.orient).bounds;
            let pos = rv.rep.pos;
            Some((
                EntityRef(rv.rep.id.as_uuid()),
                Rect::from_points(
                    Pt2::new(pos.x + b.min().x, pos.y + b.min().y),
                    Pt2::new(pos.x + b.max().x, pos.y + b.max().y),
                ),
            ))
        })
        .collect()
}

/// [`in_the_way`]'s answer for a place `b` of a text of `entity`, with the vertical extent of each other
/// representation's symbol body (`bodies`) that the place overlaps added (FT-1519): the rise's free test. Never the
/// text's own symbol's body; only the rise reads it, not the captions or the tags.
fn with_bodies(
    in_way: Option<Vec<(Nm, Nm)>>,
    b: Rect,
    entity: EntityRef,
    bodies: &[(EntityRef, Rect)],
) -> Option<Vec<(Nm, Nm)>> {
    let mut out = in_way?;
    out.extend(
        bodies
            .iter()
            .filter(|(e, r)| *e != entity && overlaps(b, *r))
            .map(|(_, r)| (r.min().y, r.max().y)),
    );
    Some(out)
}

/// Whether a wire segment `(a, b)` meets the open inside of `r` (FT-758).
fn wire_through(r: Rect, (a, b): (Pt2, Pt2)) -> bool {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    // A degenerate extent along an axis meets the open interval at a point inside it; a run by an overlap.
    let meets = |lo: Nm, hi: Nm, rlo: Nm, rhi: Nm| {
        if lo == hi {
            rlo < lo && lo < rhi
        } else {
            lo < rhi && rlo < hi
        }
    };
    meets(x0, x1, r.min().x, r.max().x) && meets(y0, y1, r.min().y, r.max().y)
}

/// The place of a black box's caption text in its stack, counted from the box outward (FT-734): the tag, the part
/// number, the manufacturer; `None` for any other slot kind.
fn caption_rank(kind: &SlotKind) -> Option<u8> {
    match kind {
        SlotKind::Tag => Some(0),
        SlotKind::PartNumber => Some(1),
        SlotKind::Manufacturer => Some(2),
        _ => None,
    }
}

impl SlotText<'_> {
    /// The text spec; `conflict` drops the whole-text link (a value with both kinds of link).
    fn spec(&self, conflict: bool) -> TextSpec<'_> {
        TextSpec {
            text: &self.value.text,
            at: self.at,
            angle: self.angle,
            align: &self.align,
            style_key: self.style_key,
            entity: self.entity,
            link: self.value.link.filter(|_| !conflict).map(Link::Source),
            items: &self.value.item_links,
        }
    }
}

/// Whether two rectangles share interior points (touching edges do not count).
fn overlaps(a: Rect, b: Rect) -> bool {
    a.min().x < b.max().x && b.min().x < a.max().x && a.min().y < b.max().y && b.min().y < a.max().y
}

/// Whether two slot-text boxes of two representations collide (FT-687, cycle 37): they share interior points, or they sit side by side
/// (their heights overlap) less than [`SLOT_TEXT_GAP`] apart, so the two texts would read as one ("(101-D6)-101K1").
/// Texts stacked one above the other at any distance do not collide unless they overlap.
fn collides(a: Rect, b: Rect) -> bool {
    let gap = SLOT_TEXT_GAP.0;
    a.min().x.0 < b.max().x.0 + gap
        && b.min().x.0 < a.max().x.0 + gap
        && a.min().y < b.max().y
        && b.min().y < a.max().y
}

pub(crate) struct TextSpec<'a> {
    pub(crate) text: &'a str,
    pub(crate) at: Pt2,
    pub(crate) angle: Angle2,
    pub(crate) align: &'a Align,
    pub(crate) style_key: &'a str,
    pub(crate) entity: EntityRef,
    pub(crate) link: Option<Link>,
    /// One link per listed item of `text` (FT-86).
    pub(crate) items: &'a [ItemLink],
}

/// The midpoint of the longest segment of `pts` (the first of equal ones) and whether that segment is
/// vertical; `None` for fewer than two points.
fn marker_point(pts: &[Pt2]) -> Option<(Pt2, bool)> {
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

    /// C46-p4#1: the strip tag's step up (ID-1151) is exact on the quarter turns and, at any other angle, the
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
