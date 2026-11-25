// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `page_display_list(page, layer, lang)` (ARCH §6.4, doc 09 §4.1; ADR-0015 decision 1 and amendment 7): one
//! memoised query family per layer, each building the view of its own layer from the parts it reads (`view.rs`),
//! so a wire edit never rebuilds the title block and a renumber rebuilds only the frames whose Prev/Next changed.
//!
//! | Family | Layer | Reads |
//! |---|---|---|
//! | [`FrameLayer`] | [`Layer::Frame`] | `page_frame`, `page_bindings` (`title_block(page)`, the date format) |
//! | [`SymbolsLayer`] | [`Layer::Symbols`] | `page_reps`, `page_cp_targets`, `assembly_break_marks`, `page_grid` (the pen of the break marks and graphics), the page items and structure boxes without texts |
//! | [`WiresLayer`] | [`Layer::Wires`] | `page_reps`, `page_connections`, `page_junctions`, `page_grid`, `page_frame` (the page rectangle), `page_knockouts` (the texts its lines are cut round, [`PageKnockoutsOf`]), the bridges of the strips its terminals belong to (`page_bridges`) |
//! | [`LabelsLayer`] | [`Layer::Labels`] | `page_reps`, `page_slots`, `page_connections`, `page_junctions`, `page_grid` (the step of a black box's caption), `page_frame`, the page items with texts, the structure boxes with captions the strip tags of terminals drawn alone (`page_strip_tags`), the valve cells, the plug notes and the strips' bridges (for the layout's page, `ew_scene::page::LayerList::draft`); on a panel-layout page also the placement labels (`panel.rs`) |
//! | [`TablesLayer`] | [`Layer::Tables`] | `page_frame` (the sheet map), `page_table` or `page_embedded` |
//! | [`SurfaceLayer`] | [`Layer::Surface`] | the surface outlines of the page's surface view (`panel.rs`) |
//! | [`CarriersLayer`] | [`Layer::Carriers`] | the carriers of the page's surface view and its placement labels, which the carriers are cut around (`panel.rs`) |
//! | [`PlacementsLayer`] | [`Layer::Placements`] | the placements of the page's surface view (`panel.rs`) |
//!
//! The other layers (revision marks, highlights, features, dimensions) are drawn by no rule of `ew-scene` yet and are
//! empty. Each family is `ew_scene::page::page_display_list` over a partial view; a test holds every layer equal
//! to the same function over the full view ([`page_view`]) on every page of the sample project. The display
//! language is the neutral one (no `lang` key yet: the views print neutral texts until multi-language output,
//! FR-I18N-005).
//!
//! The families are pure functions of `Q` (the project's font set among it, [`super::ProjectFontsQ`]) and the built-in
//! generic symbols (`view::generic_symbols`), a process constant.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_derive::{DrawingSettingsQ, PageEntry, PageRef, Policy, Q, Query};
use ew_dl::{DisplayList, Layer, SymbolGeomId};
use ew_ids::{Id, Uuid};
use ew_scene::forms::{PageBindings, SheetMap};
use ew_scene::hops::Crossings;
use ew_scene::page::{DEFAULT_SHEET, PageFinding, PageKind, PageScene, PageView, RepView, page_display_list};
use ew_schema::PageType;
use ew_units::Pt2;

use super::panel::{Parts, panel_view};
use super::view::{
    PageBindingsOf, PageConnectionsOf, PageCpTargetsOf, PageEmbeddedOf, PageFrameOf, PageJunctionsOf,
    PageRepsOf, PageSlotsOf, PageTableOf, page_boxes, page_break_marks, page_bridges, page_grid, page_items,
    page_plug_notes, page_strip_tags, page_uuid, page_valve_cells, page_valve_glyphs,
};

/// One layer of one page: the display list in page coordinates, the symbol geometry its instances name and the
/// findings (`ew_scene::page::LayerList` with the list behind an `Arc`, so a page scene shares it).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageLayer {
    /// The items in paint order, in page coordinates.
    pub list: Arc<DisplayList>,
    /// `symbol_geometry(symbol, variant)` of every instance in `list`.
    pub geometry: BTreeMap<SymbolGeomId, Arc<DisplayList>>,
    /// What to report, in paint order.
    pub findings: Vec<PageFinding>,
    /// The labels layer's anchor of each representation's device tag after the tag rule, by representation
    /// ID (`ew_scene::page::LayerList::tags`).
    pub tags: BTreeMap<Uuid, Pt2>,
    /// The labels layer's box of each placement's label on a panel-layout page, by placement ID
    /// (`ew_scene::page::LayerList::panel_labels`).
    pub panel_labels: BTreeMap<Uuid, ew_units::Rect>,
    /// The labels layer's slot texts where they print (`ew_scene::page::LayerList::slot_texts`).
    pub slot_texts: Vec<ew_scene::page::SlotTextBox>,
    /// The labels layer's texts of representations by their items and what printed them
    /// (`ew_scene::page::LayerList::printed`).
    pub printed: Vec<ew_scene::page::PrintedText>,
    /// The labels layer's page as the layout sees it (`ew_scene::page::LayerList::draft`).
    pub draft: Option<Arc<ew_drafting::DraftPage>>,
    /// The labels layer's boxes of the texts a line of the page runs through (`ew_scene::page::LayerList::knockouts`).
    pub knockouts: Vec<ew_units::Rect>,
}

/// An empty view of `page`: the parts a layer reads are filled by its family.
fn base_view(page: PageRef) -> PageView {
    PageView {
        page: Id::from_uuid(page_uuid(page)),
        kind: PageKind::Schematic,
        grid: ew_units::Grid::MM4,
        language: None,
        frame: None,
        reps: Vec::new(),
        connections: Vec::new(),
        junctions: Vec::new(),
        table: None,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        valve_glyphs: Vec::new(),
        valve_cells: Vec::new(),
        plug_notes: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Crossings::Plain,
        items: Vec::new(),
        structure_boxes: Vec::new(),
        panel: None,
        strip_tags: Vec::new(),
        knockouts: Vec::new(),
        bridges: Vec::new(),
    }
}

/// How the project draws crossing wires (`drawing_settings`).
fn crossings(q: &Q) -> Crossings {
    if q.get::<DrawingSettingsQ>(()).hops {
        Crossings::Hops
    } else {
        Crossings::Plain
    }
}

/// The frame view of `page` without bindings: what the layers that need only the form's geometry read.
fn bare_frame(q: &Q, page: PageRef) -> Option<ew_scene::page::FrameView> {
    q.get::<PageFrameOf>(page)
        .as_ref()
        .as_ref()
        .map(|f| f.view(PageBindings::new()))
}

/// The representations of a drawn page with their slot texts.
fn reps_with_slots(q: &Q, page: Id<ew_schema::Page>) -> Vec<RepView> {
    let slots = q.get::<PageSlotsOf>(page);
    let mut reps = (*q.get::<PageRepsOf>(page)).clone();
    for r in &mut reps {
        if let Ok(i) = slots.binary_search_by_key(&r.rep.id, |(id, _, _)| *id) {
            r.slots.clone_from(&slots[i].1);
            r.pins.clone_from(&slots[i].2);
        }
    }
    reps
}

/// How a view holds a part that comes in two forms: not at all, in its plain form, or in its full one (the frame with
/// the title block's bindings, the representations with their slot texts, the page items and structure boxes with
/// their texts).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Form {
    No,
    Plain,
    Full,
}

/// The parts of `view.rs` a view is filled with: what one layer reads ([`ViewParts::of`]), or, for [`page_view`], the
/// union of what every layer reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ViewParts {
    /// Whether the parts exist only on a drawn page: the layer draws nothing on another.
    drawn_only: bool,
    frame: Form,
    grid: bool,
    reps: Form,
    cp_targets: bool,
    break_marks: bool,
    valve_glyphs: bool,
    valve_cells: bool,
    plug_notes: bool,
    connections: bool,
    junctions: bool,
    crossings: bool,
    knockouts: bool,
    bridges: bool,
    /// The page items and the structure boxes.
    items: Form,
    strip_tags: bool,
    /// The report table of a generated page, or the embedded blocks of a drawn one.
    tables: bool,
    panel: Parts,
    /// Whether the layer draws nothing without a surface view.
    needs_panel: bool,
}

impl ViewParts {
    const NONE: ViewParts = ViewParts {
        drawn_only: false,
        frame: Form::No,
        grid: false,
        reps: Form::No,
        cp_targets: false,
        break_marks: false,
        valve_glyphs: false,
        valve_cells: false,
        plug_notes: false,
        connections: false,
        junctions: false,
        crossings: false,
        knockouts: false,
        bridges: false,
        items: Form::No,
        strip_tags: false,
        tables: false,
        panel: Parts::NONE,
        needs_panel: false,
    };

    /// What `layer` reads (see the module documentation); `None` for a layer that draws nothing.
    fn of(layer: Layer) -> Option<ViewParts> {
        let none = ViewParts::NONE;
        let drawn = ViewParts {
            drawn_only: true,
            ..none
        };
        Some(match layer {
            Layer::Frame => ViewParts {
                frame: Form::Full,
                ..none
            },
            // The break marks and the graphics are stroked with the page's wire pen.
            Layer::Symbols => ViewParts {
                reps: Form::Plain,
                cp_targets: true,
                break_marks: true,
                valve_glyphs: true,
                valve_cells: true,
                plug_notes: true,
                grid: true,
                items: Form::Plain,
                ..drawn
            },
            Layer::Wires => ViewParts {
                frame: Form::Plain,
                grid: true,
                reps: Form::Plain,
                connections: true,
                junctions: true,
                crossings: true,
                knockouts: true,
                bridges: true,
                ..drawn
            },
            // The layout's page: a valve terminal drawn as its cells is an outline round its valves, the texts beside a
            // plug's pins are fixed texts the others keep clear of, and a strip's drawn bridge is a line the texts keep
            // clear of.
            Layer::Labels => ViewParts {
                frame: Form::Plain,
                grid: true,
                reps: Form::Full,
                connections: true,
                junctions: true,
                items: Form::Full,
                strip_tags: true,
                valve_cells: true,
                plug_notes: true,
                bridges: true,
                panel: Parts {
                    labels: true,
                    ..Parts::NONE
                },
                ..drawn
            },
            Layer::Tables => ViewParts {
                frame: Form::Plain,
                tables: true,
                ..none
            },
            Layer::Surface | Layer::Carriers | Layer::Placements => ViewParts {
                panel: Parts {
                    surface: layer == Layer::Surface,
                    carriers: layer == Layer::Carriers,
                    placements: layer == Layer::Placements,
                    // The carriers are cut around the labels' knockout boxes.
                    labels: layer == Layer::Carriers,
                },
                needs_panel: true,
                ..drawn
            },
            _ => return None,
        })
    }

    /// What either reads.
    fn union(self, o: ViewParts) -> ViewParts {
        ViewParts {
            drawn_only: self.drawn_only && o.drawn_only,
            frame: self.frame.max(o.frame),
            grid: self.grid || o.grid,
            reps: self.reps.max(o.reps),
            cp_targets: self.cp_targets || o.cp_targets,
            break_marks: self.break_marks || o.break_marks,
            valve_glyphs: self.valve_glyphs || o.valve_glyphs,
            valve_cells: self.valve_cells || o.valve_cells,
            plug_notes: self.plug_notes || o.plug_notes,
            connections: self.connections || o.connections,
            junctions: self.junctions || o.junctions,
            crossings: self.crossings || o.crossings,
            knockouts: self.knockouts || o.knockouts,
            bridges: self.bridges || o.bridges,
            items: self.items.max(o.items),
            strip_tags: self.strip_tags || o.strip_tags,
            tables: self.tables || o.tables,
            panel: Parts {
                surface: self.panel.surface || o.panel.surface,
                carriers: self.panel.carriers || o.panel.carriers,
                placements: self.panel.placements || o.panel.placements,
                labels: self.panel.labels || o.panel.labels,
            },
            needs_panel: self.needs_panel && o.needs_panel,
        }
    }
}

/// The view of `page` filled with `parts`; `None` where the parts draw nothing on this page (a drawn page's parts on
/// another page, a surface view's layer without one).
fn view_of(q: &Q, page: PageRef, parts: ViewParts) -> Option<PageView> {
    let mut v = base_view(page);
    v.language = ew_derive::print_language(q);
    let drawn = match page {
        PageRef::Drawn(p) => Some(p),
        _ => None,
    };
    if parts.drawn_only && drawn.is_none() {
        return None;
    }
    match parts.frame {
        Form::No => {}
        Form::Plain => v.frame = bare_frame(q, page),
        Form::Full => {
            let bindings = q.get::<PageBindingsOf>(page);
            v.frame = q
                .get::<PageFrameOf>(page)
                .as_ref()
                .as_ref()
                .map(|f| f.view((*bindings).clone()));
        }
    }
    if parts.grid {
        v.grid = page_grid(q, page);
    }
    if parts.tables {
        match drawn {
            Some(p) => v.embedded = (*q.get::<PageEmbeddedOf>(p)).clone(),
            None => v.table = (*q.get::<PageTableOf>(page)).clone(),
        }
    }
    let Some(p) = drawn else {
        return Some(v);
    };
    match parts.reps {
        Form::No => {}
        Form::Plain => v.reps = (*q.get::<PageRepsOf>(p)).clone(),
        Form::Full => v.reps = reps_with_slots(q, p),
    }
    if parts.connections {
        v.connections = (*q.get::<PageConnectionsOf>(p)).clone();
    }
    if parts.junctions {
        v.junctions = (*q.get::<PageJunctionsOf>(p)).clone();
    }
    if parts.cp_targets {
        v.cp_targets = (*q.get::<PageCpTargetsOf>(p)).clone();
    }
    if parts.break_marks {
        v.break_marks = page_break_marks(q, p);
    }
    if parts.valve_glyphs {
        v.valve_glyphs = page_valve_glyphs(q, p);
    }
    if parts.valve_cells {
        v.valve_cells = page_valve_cells(q, p);
    }
    if parts.plug_notes {
        v.plug_notes = page_plug_notes(q, p);
    }
    if parts.crossings {
        v.crossings = crossings(q);
    }
    if parts.items != Form::No {
        let texts = parts.items == Form::Full;
        v.items = page_items(q, p, texts);
        v.structure_boxes = page_boxes(q, p, texts);
    }
    if parts.strip_tags {
        v.strip_tags = page_strip_tags(q, p);
    }
    if parts.knockouts {
        v.knockouts = (*q.get::<PageKnockoutsOf>(page)).clone();
    }
    if parts.bridges {
        v.bridges = page_bridges(q, p);
    }
    if parts.panel != Parts::NONE {
        v.panel = panel_view(q, p, parts.panel);
        if parts.needs_panel && v.panel.is_none() {
            return None;
        }
    }
    Some(v)
}

/// The partial view `layer` of `page` reads (see the module documentation); `None` for a layer that draws
/// nothing.
fn layer_view(q: &Q, page: PageRef, layer: Layer) -> Option<PageView> {
    view_of(q, page, ViewParts::of(layer)?)
}

/// The project's effective text-style table (ADR-0032) as the scene's table: the rows of `ew_derive::text_styles`
/// in their order. Every page layer, the editing outputs and the exports resolve text styles through it.
pub fn text_style_table(q: &Q) -> ew_text::TextStyleTable {
    table_from_style_rows(&ew_derive::text_styles(q))
}

/// The effective styles of a new basic project, for a document preview before any project is open.
pub fn new_project_text_style_table() -> ew_text::TextStyleTable {
    let settings = ew_schema::ProjectSettings {
        text_styles: ew_schema::new_project_text_styles(),
        ..Default::default()
    };
    table_from_style_rows(&ew_schema::effective_text_styles(&settings))
}

fn table_from_style_rows(rows: &[ew_schema::EffectiveStyle]) -> ew_text::TextStyleTable {
    ew_text::TextStyleTable::from_rows(rows.iter().map(|s| {
        let weight = if s.is_bold() {
            ew_text::Weight::Bold
        } else {
            ew_text::Weight::Regular
        };
        let family = match s.face {
            ew_schema::Typeface::Narrow => ew_text::Family::Narrow,
            ew_schema::Typeface::Mono => ew_text::Family::Mono,
            ew_schema::Typeface::TechnicalDrawing => ew_text::Family::Technical,
            ew_schema::Typeface::Standard | ew_schema::Typeface::Unknown(_) => ew_text::Family::Default,
        };
        (s.key.as_str(), s.em, weight, family)
    }))
}

fn build(q: &Q, page: PageRef, layer: Layer) -> Arc<PageLayer> {
    let Some(view) = layer_view(q, page, layer) else {
        return Arc::new(PageLayer::default());
    };
    let l = page_display_list(&view, layer, super::fonts(q), &text_style_table(q));
    Arc::new(PageLayer {
        list: Arc::new(l.list),
        geometry: l.geometry,
        findings: l.findings,
        tags: l.tags,
        panel_labels: l.panel_labels,
        slot_texts: l.slot_texts,
        printed: l.printed,
        draft: l.draft,
        knockouts: l.knockouts,
    })
}

/// `page_knockouts(page)`: the labels layer's boxes of the texts a line of the page runs through, which the wires layer
/// cuts its lines round (`ew_scene::knockout`). A family of its own, so that a change of the labels that leaves those
/// boxes as they were (a renumber's cross-references) runs no wires layer again.
#[derive(Debug)]
pub struct PageKnockoutsOf;

impl Query for PageKnockoutsOf {
    type Key = PageRef;
    type Value = Vec<ew_units::Rect>;
    const FAMILY: &'static str = "page_knockouts";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<Vec<ew_units::Rect>> {
        Arc::new(page_layer(q, *key, Layer::Labels).knockouts.clone())
    }
}

macro_rules! layer_family {
    ($(#[$doc:meta])* $name:ident, $family:literal, $layer:expr) => {
        $(#[$doc])*
        #[derive(Debug)]
        pub struct $name;

        impl Query for $name {
            type Key = PageRef;
            type Value = PageLayer;
            const FAMILY: &'static str = $family;
            const POLICY: Policy = Policy::Memo;

            fn run(q: &Q, key: &PageRef) -> Arc<PageLayer> {
                build(q, *key, $layer)
            }
        }
    };
}

layer_family!(
    /// `page_display_list(page, frame)`: the frame form, the grid labels and the title block.
    FrameLayer,
    "page_display_list_frame",
    Layer::Frame
);
layer_family!(
    /// `page_display_list(page, symbols)`: the symbol instances, their static texts and the CP destinations.
    SymbolsLayer,
    "page_display_list_symbols",
    Layer::Symbols
);
layer_family!(
    /// `page_display_list(page, wires)`: the connections and the junctions.
    WiresLayer,
    "page_display_list_wires",
    Layer::Wires
);
layer_family!(
    /// `page_display_list(page, labels)`: the slot texts and the wire markers.
    LabelsLayer,
    "page_display_list_labels",
    Layer::Labels
);
layer_family!(
    /// `page_display_list(page, tables)`: the report table of a generated page and the embedded blocks.
    TablesLayer,
    "page_display_list_tables",
    Layer::Tables
);
layer_family!(
    /// `page_display_list(page, surface)`: the enclosure, plate and usable outlines of a panel page's surface view.
    SurfaceLayer,
    "page_display_list_surface",
    Layer::Surface
);
layer_family!(
    /// `page_display_list(page, carriers)`: the rails and ducts of a panel page's surface view.
    CarriersLayer,
    "page_display_list_carriers",
    Layer::Carriers
);
layer_family!(
    /// `page_display_list(page, placements)`: the footprints of a panel page's surface view.
    PlacementsLayer,
    "page_display_list_placements",
    Layer::Placements
);

/// `page_display_list(page, layer)` through the layer's memoised family; an empty layer for the layers no rule
/// draws yet.
pub fn page_layer(q: &Q, page: PageRef, layer: Layer) -> Arc<PageLayer> {
    match layer {
        Layer::Frame => q.get::<FrameLayer>(page),
        Layer::Symbols => q.get::<SymbolsLayer>(page),
        Layer::Wires => q.get::<WiresLayer>(page),
        Layer::Labels => q.get::<LabelsLayer>(page),
        Layer::Tables => q.get::<TablesLayer>(page),
        Layer::Surface => q.get::<SurfaceLayer>(page),
        Layer::Carriers => q.get::<CarriersLayer>(page),
        Layer::Placements => q.get::<PlacementsLayer>(page),
        _ => Arc::new(PageLayer::default()),
    }
}

/// The page kind of a page-order entry.
pub fn page_kind(entry: &PageEntry) -> PageKind {
    if entry.kind == PageType::PanelLayout {
        PageKind::Panel
    } else {
        PageKind::Schematic
    }
}

/// The full view of a page of the page order: every part of `view.rs`, the union of what the layers read
/// ([`ViewParts::of`]). For callers that need the whole view (hit-testing, the CP positions, tests); the export reads
/// the layers.
pub fn page_view(q: &Q, entry: &PageEntry) -> PageView {
    let all = Layer::ALL
        .iter()
        .filter_map(|l| ViewParts::of(*l))
        .fold(ViewParts::NONE, ViewParts::union);
    let mut v = view_of(q, entry.page, all).unwrap_or_else(|| base_view(entry.page));
    v.kind = page_kind(entry);
    v
}

/// The page scene of a page of the page order from its memoised layers: the layers of its kind in paint order,
/// the symbol geometry, the findings and the sheet of its frame form. It equals `ew_scene::page::page_scene` over
/// [`page_view`].
pub fn page_scene(q: &Q, entry: &PageEntry) -> PageScene {
    let page = entry.page;
    let mut layers = Vec::new();
    let mut geometry = BTreeMap::new();
    let mut findings = Vec::new();
    for &layer in page_kind(entry).layers() {
        let l = page_layer(q, page, layer);
        geometry.extend(l.geometry.iter().map(|(k, v)| (*k, Arc::clone(v))));
        findings.extend(l.findings.iter().cloned());
        layers.push((layer, Arc::clone(&l.list)));
    }
    let frame = q.get::<PageFrameOf>(page);
    let (sheet_width, sheet_height, map) = match frame.as_ref() {
        Some(f) => (f.form.sheet.width, f.form.sheet.height, SheetMap::of(&f.form)),
        None => (DEFAULT_SHEET.0, DEFAULT_SHEET.1, SheetMap::IDENTITY),
    };
    PageScene {
        page: Id::from_uuid(page_uuid(page)),
        sheet_width,
        sheet_height,
        map,
        layers,
        geometry,
        findings,
    }
}
