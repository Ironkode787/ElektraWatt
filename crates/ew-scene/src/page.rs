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
//! | [`Layer::Labels`] | `reps`, `connections`, `wire_marker`, `items`, `structure_boxes` | per representation (by ID): its visible slot texts (a [`Link::Source`] where the slot value names a destination); then per connection with a marker (by ID): the marker box and text; then the free texts, wire-marker groups and structure-box captions ([`crate::items`]) |
//! | [`Layer::Tables`] | `table`, `embedded` | [`crate::table::lower_table`]: the house report table of a generated page; then per embedded block (by its rectangle): [`crate::embedded::lower_embedded`] (the cover's colour and standard-size tables) |
//! | others | — | empty: revision marks and highlights arrive with their work packages |
//!
//! Panel-layout pages ([`PageKind::Panel`]) use [`Layer::PANEL`]; their surface, carrier, placement,
//! feature and dimension layers are empty until the panel work package.
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
//!   [`crate::symbol`]: the position turns with the orientation, the angle is made readable, the alignment
//!   flips (doc 07 §4.1); the style is the slot's own `style` when it names a known text style, else
//!   [`symbol::default_style_key`] of the slot kind (an unknown name is reported as
//!   [`PageFinding::UnknownSlotStyle`], ID-227). `max_width` is not applied yet.
//!   A text that lists several items carries one link per item ([`SlotValue::item_links`], FT-86): an invisible
//!   path item over the item's glyph span (the pen travel of the text up to the item's ends), after the text's
//!   own items, with the text's entity and a [`Link::Source`].
//! - **Wire markers** (FR-OUT-011 keeps them in monochrome): the caller's marker text, centred in style
//!   `marker` on the midpoint of the polyline's longest segment (the first of equal ones), turned 90° on a
//!   vertical segment, over the marker symbol in the same orientation. Provisional, like the routes.
//! - **Stored symbol variants** (`Representation.variant`, contact-image variants) are not drawn yet: the
//!   main graphics are used, and the geometry key's `extra` is `None`.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use ew_dl::{
    Brush, Destination, DisplayList, EntityRef, Item, ItemKind, Layer, Link, Path, Pen, Stroke, SymbolGeomId,
    Transform,
};
use ew_ids::{Id, Uuid};
use ew_mltext::LangTag;
use ew_schema::{
    Align, Connection, Dir4, Endpoint, FormDef, Function, Junction, JunctionStyle, Page, Representation,
    SymbolDef,
};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, Shaped, TextStyle, TextStyleTable};
use ew_units::{Angle2, Grid, Nm, Orient2, Pt2, Rect};

use crate::embedded::{EmbeddedView, lower_embedded};
use crate::forms::{
    BoxSpec, FormFinding, FrameGeometry, PageBindings, SheetMap, lower_form, ml_text, place, split_align,
};
use crate::hops::{Crossings, hop_path, hops};
use crate::items::{ItemView, StructureBoxView};
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
        Layer::Labels => b.labels(view),
        Layer::Tables => b.tables(view),
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

pub(crate) struct Builder<'a> {
    fonts: &'a FontSet,
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
        // The drawn polylines in connection-ID order (the hop rule of `crate::hops` reads them all first).
        let mut drawn: Vec<(Id<Connection>, Vec<Pt2>)> = Vec::new();
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
            drawn.push((c.id, pts));
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
            self.push(Item::path(path, Some(stroke.clone()), None).with_entity(EntityRef(id.as_uuid())));
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
        for rv in sorted_reps(view) {
            let Some(def) = &rv.symbol else { continue };
            let rep = &rv.rep;
            let entity = EntityRef(rep.id.as_uuid());
            let arrangement: Option<BTreeSet<u16>> = rep.arrangement.as_ref().and_then(|name| {
                def.arrangements
                    .iter()
                    .find(|a| a.name == *name)
                    .map(|a| a.slots.iter().copied().collect())
            });
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
                let visible = ov.and_then(|o| o.visible).unwrap_or_else(|| {
                    arrangement
                        .as_ref()
                        .map_or(slot.visible, |a| a.contains(&value.slot))
                });
                if !visible {
                    continue;
                }
                let pos = ov.and_then(|o| o.pos).unwrap_or(slot.pos);
                let rot = ov.and_then(|o| o.angle).unwrap_or(slot.rot);
                let (style_key, unknown) = symbol::slot_style_key(slot, self.styles);
                if let Some(name) = unknown {
                    self.out.findings.push(PageFinding::UnknownSlotStyle {
                        entity: entity.0,
                        symbol: rep.symbol.as_uuid(),
                        slot: value.slot,
                        name: name.to_owned(),
                    });
                }
                let conflict = value.link.is_some() && !value.item_links.is_empty();
                if conflict {
                    self.out.findings.push(PageFinding::ConflictingLinks {
                        entity: entity.0,
                        slot: value.slot,
                    });
                }
                self.text(&TextSpec {
                    text: &value.text,
                    at: rep.pos + rep.orient.apply(pos),
                    angle: symbol::readable_angle(rot, rep.orient),
                    align: &symbol::placed_align(&slot.align, rot, rep.orient),
                    style_key,
                    entity,
                    link: value.link.filter(|_| !conflict).map(Link::Source),
                    items: &value.item_links,
                });
            }
        }
        self.connection_markers(view);
        // The free texts, wire-marker groups and structure-box captions (WP-S2.04 part 4a).
        self.item_labels(view);
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

    fn style(&mut self, key: &str, entity: EntityRef) -> TextStyle {
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

    /// Shapes `spec.text` and places it at `spec.at` with its alignment and angle, as form graphics do.
    pub(crate) fn text(&mut self, spec: &TextSpec<'_>) {
        if spec.text.is_empty() {
            return;
        }
        let style = self.style(spec.style_key, spec.entity);
        let (halign, valign) = split_align(spec.align).unwrap_or((0, 0));
        let shaped = self.fonts.shape(spec.text, &style, None);
        if !shaped.missing.is_empty() {
            self.out.findings.push(PageFinding::MissingGlyphs {
                entity: spec.entity.0,
                text: spec.text.to_owned(),
                chars: shaped.missing.clone(),
            });
        }
        let boxed = BoxSpec {
            rect: Rect::from_point(spec.at),
            halign,
            valign,
            padding: Nm::ZERO,
            overflow: None,
            max_lines: 1,
            sheet: SheetMap::IDENTITY,
        };
        let lines = place(
            self.fonts, &shaped, &style, spec.text, &boxed, spec.at, spec.angle,
        );
        let carriers = self.item_carriers(spec, &style, &shaped, &lines);
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

    /// One invisible path item per entry of `spec.items`, over the box of that item's glyph span and
    /// carrying its link (the box turns with the text). `lines` are the placed line items of the text.
    fn item_carriers(
        &self,
        spec: &TextSpec<'_>,
        style: &TextStyle,
        shaped: &Shaped,
        lines: &[Item],
    ) -> Vec<Item> {
        if spec.items.is_empty() {
            return Vec::new();
        }
        let m = self.fonts.vertical_metrics(style);
        spec.items
            .iter()
            .filter_map(|link| {
                let (k, line) = shaped
                    .lines
                    .iter()
                    .enumerate()
                    .find(|(_, l)| l.text.start <= link.range.start && link.range.end <= l.text.end)?;
                // The span ends are the pen travel of the text up to them, trailing spaces included.
                let travel = |end: usize| {
                    let prefix = spec.text.get(line.text.start..end)?;
                    Some(self.fonts.shape(prefix, style, None).advance())
                };
                let (x0, x1) = (travel(link.range.start)?, travel(link.range.end)?);
                let to_parent = |q: Pt2| match &lines.get(k)?.kind {
                    ItemKind::Glyphs(run) => Some(run.to_parent(q)),
                    ItemKind::Group(g) => Some(g.transform.unwrap_or(Transform::IDENTITY).apply(q)),
                    _ => None,
                };
                let corner = |x: Nm, y: Nm| to_parent(Pt2::new(x, y));
                let quad = [
                    corner(x0, Nm(-m.ascent.0))?,
                    corner(x0, m.descent)?,
                    corner(x1, m.descent)?,
                    corner(x1, Nm(-m.ascent.0))?,
                ];
                Some(
                    Item::path(Path::polygon(&quad), None, None)
                        .with_entity(spec.entity)
                        .with_link(Link::Source(link.dest)),
                )
            })
            .collect()
    }
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
