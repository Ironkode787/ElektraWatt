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
//! | [`WiresLayer`] | [`Layer::Wires`] | `page_reps`, `page_connections`, `page_junctions`, `page_grid`, `page_frame` (the page rectangle) |
//! | [`LabelsLayer`] | [`Layer::Labels`] | `page_reps`, `page_slots`, `page_connections`, `page_junctions`, `page_grid` (the step of a black box's caption, FT-734), `page_frame`, the page items with texts and the structure boxes with captions; on a panel-layout page also the placement labels (`panel.rs`) |
//! | [`TablesLayer`] | [`Layer::Tables`] | `page_frame` (the sheet map), `page_table` or `page_embedded` |
//! | [`SurfaceLayer`] | [`Layer::Surface`] | the surface outlines of the page's surface view (`panel.rs`) |
//! | [`CarriersLayer`] | [`Layer::Carriers`] | the carriers of the page's surface view (`panel.rs`) |
//! | [`PlacementsLayer`] | [`Layer::Placements`] | the placements of the page's surface view (`panel.rs`) |
//!
//! The other layers (revision marks, highlights, features, dimensions) are drawn by no rule of `ew-scene` yet and are
//! empty. Each family is `ew_scene::page::page_display_list` over a partial view; a test holds every layer equal
//! to the same function over the full view ([`page_view`]) on every golden page. The display language is the
//! neutral one (no `lang` key yet: the views print neutral texts until multi-language output, FR-I18N-005).
//!
//! The families are pure functions of `Q` and two process constants: the bundled font set
//! (`ew_text::bundled()`) and the built-in generic symbols (`view::generic_symbols`).

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::{DrawingSettingsQ, PageEntry, PageRef, Policy, Q, Query};
use ew_dl::{DisplayList, Layer, SymbolGeomId};
use ew_scene::forms::{PageBindings, SheetMap};
use ew_scene::hops::Crossings;
use ew_scene::page::{DEFAULT_SHEET, PageFinding, PageKind, PageScene, PageView, RepView, page_display_list};
use ew_schema::PageType;

use super::panel::{Parts, panel_view};
use super::view::{
    PageBindingsOf, PageConnectionsOf, PageCpTargetsOf, PageEmbeddedOf, PageFrameOf, PageJunctionsOf,
    PageRepsOf, PageSlotsOf, PageTableOf, generic_symbols, page_boxes, page_break_marks, page_grid,
    page_items, page_uuid,
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
}

/// An empty view of `page`: the parts a layer reads are filled by its family.
fn base_view(page: PageRef) -> PageView {
    PageView {
        page: Id::from_uuid(page_uuid(page)),
        kind: PageKind::Schematic,
        grid: ew_derive::ew_units::Grid::MM4,
        language: None,
        frame: None,
        reps: Vec::new(),
        connections: Vec::new(),
        junctions: Vec::new(),
        wire_marker: None,
        table: None,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Crossings::Plain,
        items: Vec::new(),
        structure_boxes: Vec::new(),
        panel: None,
    }
}

/// How the project draws crossing wires (`drawing_settings`, WP-S2.04 §8.6).
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

/// The partial view `layer` of `page` reads (see the module documentation); `None` for a layer that draws
/// nothing.
fn layer_view(q: &Q, page: PageRef, layer: Layer) -> Option<PageView> {
    let mut v = base_view(page);
    let drawn = match page {
        PageRef::Drawn(p) => Some(p),
        _ => None,
    };
    match layer {
        Layer::Frame => {
            let bindings = q.get::<PageBindingsOf>(page);
            v.frame = q
                .get::<PageFrameOf>(page)
                .as_ref()
                .as_ref()
                .map(|f| f.view((*bindings).clone()));
        }
        Layer::Symbols => {
            let p = drawn?;
            v.reps = (*q.get::<PageRepsOf>(p)).clone();
            v.cp_targets = (*q.get::<PageCpTargetsOf>(p)).clone();
            v.break_marks = page_break_marks(q, p);
            // The break marks and the graphics are stroked with the page's wire pen.
            v.grid = page_grid(q, page);
            v.items = page_items(q, p, false);
            v.structure_boxes = page_boxes(q, p, false);
        }
        Layer::Wires => {
            let p = drawn?;
            v.frame = bare_frame(q, page);
            v.grid = page_grid(q, page);
            v.reps = (*q.get::<PageRepsOf>(p)).clone();
            v.connections = (*q.get::<PageConnectionsOf>(p)).clone();
            v.junctions = (*q.get::<PageJunctionsOf>(p)).clone();
            v.crossings = crossings(q);
        }
        Layer::Labels => {
            let p = drawn?;
            v.frame = bare_frame(q, page);
            v.grid = page_grid(q, page);
            v.reps = reps_with_slots(q, p);
            v.connections = (*q.get::<PageConnectionsOf>(p)).clone();
            v.junctions = (*q.get::<PageJunctionsOf>(p)).clone();
            v.wire_marker = generic_symbols().wire_marker.clone();
            v.items = page_items(q, p, true);
            v.structure_boxes = page_boxes(q, p, true);
            v.panel = panel_view(
                q,
                p,
                Parts {
                    labels: true,
                    ..Parts::NONE
                },
            );
        }
        Layer::Tables => {
            v.frame = bare_frame(q, page);
            match drawn {
                Some(p) => v.embedded = (*q.get::<PageEmbeddedOf>(p)).clone(),
                None => v.table = (*q.get::<PageTableOf>(page)).clone(),
            }
        }
        Layer::Surface | Layer::Carriers | Layer::Placements => {
            let parts = Parts {
                surface: layer == Layer::Surface,
                carriers: layer == Layer::Carriers,
                placements: layer == Layer::Placements,
                labels: false,
            };
            v.panel = Some(panel_view(q, drawn?, parts)?);
        }
        _ => return None,
    }
    Some(v)
}

/// The project's effective text-style table (ADR-0032) as the scene's table: the rows of `ew_derive::text_styles`
/// in their order. Every page layer, the editing outputs and the exports resolve text styles through it.
pub fn text_style_table(q: &Q) -> ew_text::TextStyleTable {
    ew_text::TextStyleTable::from_rows(ew_derive::text_styles(q).iter().map(|s| {
        let weight = if s.is_bold() {
            ew_text::Weight::Bold
        } else {
            ew_text::Weight::Regular
        };
        (s.key.as_str(), s.em, weight)
    }))
}

fn build(q: &Q, page: PageRef, layer: Layer) -> Arc<PageLayer> {
    let Some(view) = layer_view(q, page, layer) else {
        return Arc::new(PageLayer::default());
    };
    let l = page_display_list(&view, layer, ew_text::bundled(), &text_style_table(q));
    Arc::new(PageLayer {
        list: Arc::new(l.list),
        geometry: l.geometry,
        findings: l.findings,
    })
}

/// The heap and inline bytes of display-list items: each item, its path elements, its glyph run (glyphs and text)
/// and, recursively, the children of a group; image bytes are shared and counted once per item.
fn items_bytes(items: &[ew_dl::Item]) -> usize {
    use ew_dl::ItemKind;
    items
        .iter()
        .map(|item| {
            std::mem::size_of::<ew_dl::Item>()
                + match &item.kind {
                    ItemKind::Path(p) => p.path.els.len() * std::mem::size_of::<ew_dl::PathEl>(),
                    ItemKind::Glyphs(run) => {
                        std::mem::size_of::<ew_dl::ShapedRun>()
                            + run.glyphs.len() * std::mem::size_of::<ew_dl::Glyph>()
                            + run.text.len()
                    }
                    ItemKind::Image(i) => i.image.bytes.len(),
                    ItemKind::Instance(_) => 0,
                    ItemKind::Group(g) => items_bytes(&g.items),
                }
        })
        .sum()
}

/// The bytes of a layer (the memory gate, `Query::bytes`): its items and the geometry its instances name, with
/// paths, glyph runs, texts and group children, plus its findings.
fn layer_bytes(v: &PageLayer) -> usize {
    std::mem::size_of::<PageLayer>()
        + items_bytes(&v.list.items)
        + v.geometry.values().map(|g| items_bytes(&g.items)).sum::<usize>()
        + v.findings.len() * std::mem::size_of::<PageFinding>()
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
            const POLICY: Policy = Policy::MemoLru;

            fn run(q: &Q, key: &PageRef) -> Arc<PageLayer> {
                build(q, *key, $layer)
            }

            fn bytes(value: &PageLayer) -> usize {
                layer_bytes(value)
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

/// The full view of a page of the page order: every part of `view.rs` (the union of the layers' views). For
/// callers that need the whole view (hit-testing, the CP positions, tests); the export reads the layers.
pub fn page_view(q: &Q, entry: &PageEntry) -> PageView {
    let page = entry.page;
    let mut v = base_view(page);
    v.kind = page_kind(entry);
    v.grid = page_grid(q, page);
    v.wire_marker = generic_symbols().wire_marker.clone();
    let bindings = q.get::<PageBindingsOf>(page);
    v.frame = q
        .get::<PageFrameOf>(page)
        .as_ref()
        .as_ref()
        .map(|f| f.view((*bindings).clone()));
    match page {
        PageRef::Drawn(p) => {
            v.reps = reps_with_slots(q, p);
            v.connections = (*q.get::<PageConnectionsOf>(p)).clone();
            v.junctions = (*q.get::<PageJunctionsOf>(p)).clone();
            v.cp_targets = (*q.get::<PageCpTargetsOf>(p)).clone();
            v.break_marks = page_break_marks(q, p);
            v.embedded = (*q.get::<PageEmbeddedOf>(p)).clone();
            v.crossings = crossings(q);
            v.items = page_items(q, p, true);
            v.structure_boxes = page_boxes(q, p, true);
            v.panel = panel_view(q, p, Parts::ALL);
        }
        _ => v.table = (*q.get::<PageTableOf>(page)).clone(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ew_derive::ew_units::{Nm, Pt2};
    use ew_dl::{Item, Path, Stroke};

    /// A path's elements and a group's children count, not only the top-level items.
    #[test]
    fn layer_bytes_count_path_elements_and_group_children() {
        let line = |n: i64| {
            let pts: Vec<Pt2> = (0..n).map(|i| Pt2::new(Nm(i), Nm(0))).collect();
            Item::path(Path::polyline(&pts), Some(Stroke::solid(Nm(1))), None)
        };
        let small = PageLayer {
            list: Arc::new(DisplayList { items: vec![line(2)] }),
            ..PageLayer::default()
        };
        let long = PageLayer {
            list: Arc::new(DisplayList {
                items: vec![line(200)],
            }),
            ..PageLayer::default()
        };
        let grouped = PageLayer {
            list: Arc::new(DisplayList {
                items: vec![Item::group(None, None, vec![line(200), line(200)])],
            }),
            ..PageLayer::default()
        };
        assert!(layer_bytes(&long) > layer_bytes(&small) + 100 * std::mem::size_of::<ew_dl::PathEl>());
        assert!(layer_bytes(&grouped) > 2 * (layer_bytes(&long) - std::mem::size_of::<PageLayer>()));
    }
}
