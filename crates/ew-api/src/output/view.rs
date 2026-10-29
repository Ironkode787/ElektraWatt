// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The glue rules of the page view (doc 09 §4.1; decision log ID-125, ID-144, ID-315; FT-59, FT-206): the parts
//! of an [`ew_scene::page::PageView`] filled from the tracked read context `Q`.
//!
//! `ew-scene` has no `ew-derive` edge (ARCH §3 rule 7), so the session layer fills the view. Each part is its own
//! query family, so that the per-layer display lists of `layers.rs` read only what their layer draws:
//!
//! | Family (key) | Policy | Part | Read by the layers |
//! |---|---|---|---|
//! | [`PageFrameOf`] (`PageRef`) | Memo | the frame form and its title-block form | frame, wires, labels, tables |
//! | [`PageBindingsOf`] (`PageRef`) | Memo | the title-block bindings | frame |
//! | [`PageGridOf`] (`PageRef`) | Memo | the connection grid | wires |
//! | [`PageRepsOf`] (page) | Memo | the representations with their symbols and strip-segment terminals, no slot texts | symbols, wires, labels |
//! | [`PageSlotsOf`] (page) | Memo | the slot texts of every representation | labels |
//! | [`PageConnectionsOf`] (page) | Cold | the connections with their wire-marker texts | wires, labels |
//! | [`PageJunctionsOf`] (page) | Cold | the junctions with their symbols | wires, labels |
//! | [`PageCpTargetsOf`] (page) | Memo | the CPs a report row links to, the page's part of [`CpTargetsByPage`] (`()`, Memo) | symbols |
//! | [`PageTableOf`] (`PageRef`) | Memo | the printed table of a generated page | tables |
//! | [`PageEmbeddedOf`] (page) | Memo | the embedded `profile_tables` blocks of a drawn page | tables |
//!
//! # The rules (formerly the test glue of `tests/page_output/view.rs`)
//!
//! - **Page order**: `page_order` of a `Q` with the report engine installed (`Session::query()`), so the generated
//!   report pages take part in Prev/Next, the outline and `/PageLabels` (ARCH §6.4).
//! - **Page IDs** ([`page_uuid`]): a drawn page's ID, a divider's section ID, a generated page's v5 ID
//!   `v5(block, index)` (`GeneratedPage::id`); every PDF destination is unique.
//! - **Frame form**: `Page.form`, else the frame defaults of the project settings (doc 03 §5.1 `FrameDefaults`,
//!   resolved by `ew_derive::frame_form_of` like the cell grid: the entry of the page's type, else `default`; FT-336), else the
//!   no-grid frame [`NOGRID_FRAME`] (the form of the project library with that key); a generated page's
//!   `GeneratedPage::form`, else the no-grid frame; a divider the frame defaults' `section_divider` entry, else the
//!   no-grid frame (ID-144). A frame-defaults form the project does not hold falls through to the next rule. The
//!   title block by `FormDef.title_block`.
//! - **Bindings**: every `title_block(page)` value, the project's date format, no display language (neutral).
//! - **Report tables** (FT-53): a generated page's rows are the `GeneratedPage::rows` range of
//!   `report_rows(block)`, each cell printed by `ew_reports::render::render` for that page (neutral language,
//!   displayed tags); a page with a `note` prints the note instead; the column widths are the columns' weights. A
//!   row link to a connection point is drawn (FR-REP-042; the TOC's page links and the parts list's
//!   representation links are not drawn yet).
//! - **Embedded blocks** (FR-REP-028): a drawn page's `profile_tables` block is drawn into its rectangle: the rows
//!   split by their `table` column into the colour table, the control sizes and the mains sizes; a group-header
//!   row is the title of its table, a group-footer row of the colours its caption and the group-footer row of the
//!   group `note` the note under the tables. PART DATA blocks are not drawn yet.
//! - **Slot texts** (ID-162, ID-166, ID-228, ID-315; FT-206): `tag` → `displayed_tag(device, page)`, except that a
//!   terminal's own representation prints only the terminal's designation and an interruption point has no `tag`;
//!   `cp_designation(i)` → the designation of the function CP that symbol CP `i` draws; `function_text` → the
//!   device's function text, or an interruption point's pairing name; `process_tag` → the device's process tag;
//!   `xref` → `xref_text(rep)` with one link per listed item; `tech_data` → `tech_data_text(device)`;
//!   `manufacturer`, `part_number` and `type_designation` → the device's main part (its first part use, FT-162);
//!   `order_code` → `order_code_text(device)` (the order code of its first assembly config, D-36); every other kind
//!   has no value. An empty text is not listed.
//! - **Break marks** (D-41, doc 11 §5.5 step 5): the `assembly_break_marks()` of a drawn page ([`page_break_marks`]),
//!   drawn by the scene at the side of their representation.
//! - **Wire markers** (ID-144; WP-S2.04 part 4a, `ew_derive::connection_marker`): with the project's `marker_mode`
//!   absent, connections with `overrides.show_marker` print the explicit cross-section and the colour codes (their
//!   file spelling, `GNYE`), joined by a space; in a mode, the connections the mode selects print their effective
//!   values. A connection a wire-marker item lists prints no marker of its own: the item prints the group.
//! - **Page items and structure boxes** (WP-S2.04 part 4a; [`page_items`], [`page_boxes`]): a drawn page's items and
//!   boxes in ID order; with texts, a free text prints its text or its bound value (`ew_derive::free_text`), a wire
//!   marker its group text and whether it is shown (`ew_derive::group_marker`), a box its caption
//!   (`ew_derive::box_caption`); the symbols layer reads them without texts (geometry only), so a text edit never
//!   rebuilds it.
//! - **Junction and wire-marker symbols**: the built-in generic symbols `ew.gen.junction-dot@qet10`,
//!   `ew.gen.junction-bevel@qet10` and `ew.gen.wire-marker@qet10` of `content/symbols/qet10`, compiled in
//!   ([`generic_symbols`]): no record names them, so projects never embed them.
//! - **Strip segments** (doc 08 §6.7, FR-TRM-011): the terminals of the strip (`terminals_of_strip`, in
//!   `(Terminal.seq, ID)` order) are split over the strip's segment representations, taken in
//!   `(page-order position, pos.y, pos.x, ID)` order, as contiguous runs: segment `j` of `m` draws the terminals
//!   `⌊j·N/m⌋ … ⌊(j+1)·N/m⌋ − 1` of `N`. Each drawn terminal is its device's main function (else its first
//!   function) with the CPs of `function_cps` on side 1 and side 2.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use ew_derive::ew_ids::{self, Id, Uuid};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_units::Grid;
use ew_derive::{
    DisplayedTag, FunctionCps, GeneratedPage, PageOrder, PageRef, Policy, Q, Query, TbValue,
    TitleBlockValues, XrefTextOf, box_caption, connection_marker, frame_form_of, free_text, group_marker,
    grouped_connections, main_part, manufacturer_name, order_code_part, order_code_text, tech_data_text,
};
use ew_dl::Destination;
use ew_reports::render::{RenderOptions, render};
use ew_reports::{BlockRows, LinkTarget, Row, RowKind};
use ew_scene::embedded::{CoverTables, EmbeddedContent, EmbeddedView};
use ew_scene::forms::{AssetRef, BoundValue, PageBindings};
use ew_scene::page::{
    BreakMarkView, BreakSide, ConnectionView, CpTarget, FrameView, ItemLink, JunctionView, RepView,
    SegmentTerminal, SlotValue, SymbolRef,
};
use ew_scene::table::{TableColumn, TableRow, TableRowKind, TableView};
use ew_schema::{
    FnRole, FormDef, Function, FunctionKind, JunctionStyle, LibCopy, LibDocument, Page, PageType, RepType,
    ReportBlock, ReportKind, ReportSource, Representation, Side, SlotKind, SymbolDef,
};

/// The key of the frame form of dividers, and of pages without a form (ID-144).
pub const NOGRID_FRAME: &str = "ew.frame.qet-a4-landscape-nogrid";

/// The built-in generic symbols the page view draws without a record naming them.
#[derive(Clone, Debug)]
pub struct GenericSymbols {
    /// `ew.gen.junction-dot@qet10`.
    pub junction_dot: Option<SymbolRef>,
    /// `ew.gen.junction-bevel@qet10`.
    pub junction_bevel: Option<SymbolRef>,
    /// `ew.gen.wire-marker@qet10`.
    pub wire_marker: Option<SymbolRef>,
}

fn builtin_symbol(json: &str) -> Option<SymbolRef> {
    match LibDocument::from_json(json).ok()? {
        LibDocument::Symbol(LibCopy { id, item, .. }) => Some(SymbolRef {
            id,
            def: Arc::new(item),
        }),
        _ => None,
    }
}

const JUNCTION_DOT: &str = include_str!("../../../../content/symbols/qet10/ew.gen.junction-dot@qet10.json");
const JUNCTION_BEVEL: &str =
    include_str!("../../../../content/symbols/qet10/ew.gen.junction-bevel@qet10.json");
const WIRE_MARKER: &str = include_str!("../../../../content/symbols/qet10/ew.gen.wire-marker@qet10.json");

/// The derivation profile the compiled-in generic symbols belong to: their seals below are part of that
/// profile's output. Regenerating `content/symbols/qet10` changes a seal, and the seal test then fails until the
/// new symbols ship with a new derivation profile (ADR-0003: a project's printed output changes only with the
/// profile it pins).
pub const GENERIC_SYMBOLS_PROFILE: &str = "2027.1";

/// The `sha256` seals (doc 05 §13.2) of the compiled-in generic symbols of [`GENERIC_SYMBOLS_PROFILE`], by key.
pub const GENERIC_SYMBOL_SEALS: [(&str, &str); 3] = [
    (
        "ew.gen.junction-dot@qet10",
        "sha256:5c96763c7eb1041383074d1ba74a8148a5c764c0e5f9f7feed8e56cafc0e2ceb",
    ),
    (
        "ew.gen.junction-bevel@qet10",
        "sha256:cd5dd5c572e2c82ce2a7e401b57782e163f69f80849791b2b989d55c9b18706b",
    ),
    (
        "ew.gen.wire-marker@qet10",
        "sha256:8724aa49d6e6d50f96311c4c6a0630f68132a3e8bd859389031027c45b7910b8",
    ),
];

/// The built-in generic symbols, parsed once (compiled in from `content/symbols/qet10`, sealed by
/// [`GENERIC_SYMBOL_SEALS`]).
pub fn generic_symbols() -> &'static GenericSymbols {
    static SET: OnceLock<GenericSymbols> = OnceLock::new();
    SET.get_or_init(|| GenericSymbols {
        junction_dot: builtin_symbol(JUNCTION_DOT),
        junction_bevel: builtin_symbol(JUNCTION_BEVEL),
        wire_marker: builtin_symbol(WIRE_MARKER),
    })
}

/// The stable UUID of a page of the page order: the page's, the section's for its divider, and for a
/// generated page its v5 ID `v5(block, index)` (ADR-0004, `GeneratedPage::id`), so 2 and 2.1 or 901 and
/// 901.1 have distinct destinations.
pub fn page_uuid(page: PageRef) -> Uuid {
    match page {
        PageRef::Drawn(p) => p.as_uuid(),
        PageRef::Divider(s) => s.as_uuid(),
        PageRef::Generated { block, index } => ew_ids::v5::report_page_id::<_, ()>(block, index).as_uuid(),
    }
}

/// The page's own PDF destination (`ew:p:<uuid>`).
pub fn page_destination(page: PageRef) -> Destination {
    Destination::Page(page_uuid(page))
}

/// The neutral text of a multi-language text.
pub(crate) fn neutral(m: &MlText) -> String {
    ew_scene::forms::ml_text(m, None).to_owned()
}

/// The frame of a page: its frame form and the form's title block.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FrameForms {
    /// The frame form.
    pub form: Arc<FormDef>,
    /// The title-block form the frame names.
    pub title_block: Option<Arc<FormDef>>,
}

impl FrameForms {
    /// The frame view with `bindings`.
    pub fn view(&self, bindings: PageBindings) -> FrameView {
        FrameView {
            form: Arc::clone(&self.form),
            title_block: self.title_block.clone(),
            bindings,
        }
    }
}

fn form(q: &Q, id: Id<FormDef>) -> Option<Arc<FormDef>> {
    q.forms().get(id).map(|f| Arc::new(f.item.clone()))
}

fn form_by_key(q: &Q, key: &str) -> Option<Arc<FormDef>> {
    q.forms()
        .iter()
        .find(|(_, f)| f.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key))
        .map(|(_, f)| Arc::new(f.item.clone()))
}

/// `page_frame(page)`: the frame form of a page of the page order and its title block (see the module
/// documentation); `None` when the page has no form and the project library holds no no-grid frame.
#[derive(Debug)]
pub struct PageFrameOf;

impl Query for PageFrameOf {
    type Key = PageRef;
    type Value = Option<FrameForms>;
    const FAMILY: &'static str = "page_frame";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<Option<FrameForms>> {
        let own = match *key {
            PageRef::Drawn(p) => q
                .pages()
                .get(p)
                .and_then(|pg| frame_form_of(q, pg.form, &pg.kind, true))
                .and_then(|id| form(q, id)),
            PageRef::Generated { .. } => q
                .get::<PageOrder>(())
                .generated(*key)
                .and_then(|g| form(q, g.form)),
            PageRef::Divider(_) => {
                frame_form_of(q, None, &PageType::SectionDivider, false).and_then(|id| form(q, id))
            }
        };
        let frame = own
            .or_else(|| form_by_key(q, NOGRID_FRAME))
            .map(|form| FrameForms {
                title_block: form.title_block.and_then(|id| self::form(q, id)),
                form,
            });
        Arc::new(frame)
    }
}

/// `page_bindings(page)`: the title-block bindings of a page: every `title_block(page)` value and the project's
/// date format, neutral language. Its own memo: it reads the whole project record (the date format), so an edit of
/// the project record that keeps the bindings is cut off here and does not rebuild the frame layers.
#[derive(Debug)]
pub struct PageBindingsOf;

impl Query for PageBindingsOf {
    type Key = PageRef;
    type Value = PageBindings;
    const FAMILY: &'static str = "page_bindings";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<PageBindings> {
        let mut b = PageBindings::new();
        b.date_format = q.project().settings.formatting.date.clone();
        for (path, value) in &q.get::<TitleBlockValues>(*key).values {
            let v = match value {
                TbValue::Text(t) => BoundValue::Text(t.clone()),
                TbValue::Ml(m) => BoundValue::Ml(m.clone()),
                TbValue::Date(d) => BoundValue::Date(*d),
                TbValue::Tag { full, displayed } => BoundValue::Tag {
                    full: full.clone(),
                    displayed: displayed.clone(),
                },
                TbValue::Asset(r) => BoundValue::Asset(asset_ref(r)),
            };
            // `title_block(page)` names valid placeholder paths; a path the form layer refuses stays unbound.
            let _ = b.set(path, v);
        }
        Arc::new(b)
    }
}

/// The form layer's reference of an asset: the record's reference, hash, format and intrinsic size (doc 05 §13.3:
/// readers trust it and never sniff).
fn asset_ref(r: &ew_schema::AssetRef) -> AssetRef {
    let format = match r.format {
        ew_schema::AssetFormat::Svg => ew_dl::ImageFormat::Svg,
        ew_schema::AssetFormat::Png => ew_dl::ImageFormat::Png,
        ew_schema::AssetFormat::Jpeg => ew_dl::ImageFormat::Jpeg,
    };
    let base = AssetRef::new(r.sha256.0, format);
    match r.size {
        Some((w, h)) => base.with_size(w, h),
        None => base,
    }
}

/// `page_reps(page)`: the representations of a drawn page with their symbol copies and, for a strip segment, the
/// terminals it draws; no slot texts (those are [`PageSlotsOf`]).
#[derive(Debug)]
pub struct PageRepsOf;

impl Query for PageRepsOf {
    type Key = Id<Page>;
    type Value = Vec<RepView>;
    const FAMILY: &'static str = "page_reps";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<RepView>> {
        let mut out = Vec::new();
        for rid in q.idx().reps_by_page(*key) {
            let Some(rep) = q.reps().get(rid).cloned() else {
                continue;
            };
            let symbol = q.symbols().get(rep.symbol).map(|s| Arc::new(s.item.clone()));
            let terminals = segment_terminals(q, &rep);
            out.push(RepView {
                rep,
                symbol,
                slots: Vec::new(),
                terminals,
            });
        }
        Arc::new(out)
    }
}

/// `page_slots(page)`: the slot texts of every representation of a drawn page that prints one, by
/// representation ID (see the module documentation).
#[derive(Debug)]
pub struct PageSlotsOf;

impl Query for PageSlotsOf {
    type Key = Id<Page>;
    type Value = Vec<(Id<Representation>, Vec<SlotValue>)>;
    const FAMILY: &'static str = "page_slots";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Self::Value> {
        let mut out = Vec::new();
        for rid in q.idx().reps_by_page(*key) {
            let Some(rep) = q.reps().get(rid).cloned() else {
                continue;
            };
            let Some(symbol) = q.symbols().get(rep.symbol) else {
                continue;
            };
            let slots = slot_values(q, *key, &rep, &symbol.item);
            if !slots.is_empty() {
                out.push((rep.id, slots));
            }
        }
        out.sort_by_key(|(id, _)| *id);
        Arc::new(out)
    }
}

/// `page_connections(page)`: the connections drawn on a page with their wire-marker texts.
#[derive(Debug)]
pub struct PageConnectionsOf;

impl Query for PageConnectionsOf {
    type Key = Id<Page>;
    type Value = Vec<ConnectionView>;
    const FAMILY: &'static str = "page_connections";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<ConnectionView>> {
        let mut out = Vec::new();
        let grouped = grouped_connections(q, *key);
        for cid in q.idx().connections_by_page(*key) {
            let Some(c) = q.connections().get(cid).cloned() else {
                continue;
            };
            let marker = connection_marker(q, &c, grouped.contains(&cid));
            out.push(ConnectionView {
                connection: c,
                marker,
            });
        }
        Arc::new(out)
    }
}

/// The page items of a drawn page in ID order (WP-S2.04 part 4a). With `texts`, each with what it prints (a free
/// text's text or bound value, a wire marker's group text and whether it is shown); without, the records alone
/// (the geometry the symbols layer draws).
pub fn page_items(q: &Q, page: Id<Page>, texts: bool) -> Vec<ew_scene::items::ItemView> {
    let items = q.page_items();
    let mut ids: Vec<_> = q.idx().page_items_by_page(page).collect();
    ids.sort();
    ids.into_iter()
        .filter_map(|id| items.get(id).cloned())
        .map(|item| {
            // An image's format comes from the asset the project holds (assets are only added; the read is recorded
            // against the project row and the asset store, `Q::logo_asset`: the asset can arrive with an item of
            // another page).
            let asset = match &item.kind {
                ew_schema::PageItemKind::Image(i) => q.logo_asset(&i.asset),
                _ => None,
            };
            let (text, shown) = if texts {
                match &item.kind {
                    ew_schema::PageItemKind::FreeText(t) => (free_text(q, page, t), true),
                    ew_schema::PageItemKind::WireMarker(_) => {
                        group_marker(q, &item).map_or((String::new(), false), |g| (g.text, g.shown))
                    }
                    _ => (String::new(), false),
                }
            } else {
                (String::new(), false)
            };
            ew_scene::items::ItemView {
                item,
                text,
                shown,
                asset,
            }
        })
        .collect()
}

/// The structure boxes of a drawn page in ID order (WP-S2.04 part 4a), with their captions when `captions` is set.
pub fn page_boxes(q: &Q, page: Id<Page>, captions: bool) -> Vec<ew_scene::items::StructureBoxView> {
    let boxes = q.structure_boxes();
    let mut ids: Vec<_> = q.idx().structure_boxes_by_page(page).collect();
    ids.sort();
    ids.into_iter()
        .filter_map(|id| boxes.get(id).cloned())
        .map(|sbox| {
            let caption = if captions {
                box_caption(q, &sbox)
            } else {
                String::new()
            };
            ew_scene::items::StructureBoxView { sbox, caption }
        })
        .collect()
}

/// `page_junctions(page)`: the junctions of a page with their generic symbols.
#[derive(Debug)]
pub struct PageJunctionsOf;

impl Query for PageJunctionsOf {
    type Key = Id<Page>;
    type Value = Vec<JunctionView>;
    const FAMILY: &'static str = "page_junctions";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<JunctionView>> {
        let generic = generic_symbols();
        let mut out = Vec::new();
        for jid in q.idx().junctions_by_page(*key) {
            let Some(j) = q.junctions().get(jid).cloned() else {
                continue;
            };
            let symbol = match j.style {
                JunctionStyle::Dot => generic.junction_dot.clone(),
                JunctionStyle::Bevel(_) => generic.junction_bevel.clone(),
                JunctionStyle::Unknown(_) => None,
            };
            out.push(JunctionView { junction: j, symbol });
        }
        Arc::new(out)
    }
}

/// `cp_targets_by_page()`: every CP a report row links to, with the representation that draws its function on
/// each drawn page (not a contact image), by page: one pass over the rows of every report block for the project.
#[derive(Debug)]
pub struct CpTargetsByPage;

impl Query for CpTargetsByPage {
    type Key = ();
    type Value = BTreeMap<Id<Page>, Vec<CpTarget>>;
    const FAMILY: &'static str = "cp_targets_by_page";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, (): &()) -> Arc<Self::Value> {
        let mut out: BTreeMap<Id<Page>, Vec<CpTarget>> = BTreeMap::new();
        for (block, _) in q.report_blocks().iter() {
            let set = q.get::<BlockRows>(block);
            for r in &set.rows {
                for l in &r.links {
                    let LinkTarget::Cp { function, idx } = l.target else {
                        continue;
                    };
                    // The smallest-ID representation of the function on each page that draws it.
                    let mut seen = std::collections::BTreeSet::new();
                    for rid in q.idx().reps_of_function(function) {
                        let Some(x) = q.reps().get(rid) else { continue };
                        if x.rep_type == RepType::ContactImage || !seen.insert(x.page) {
                            continue;
                        }
                        out.entry(x.page).or_default().push(CpTarget {
                            rep: rid,
                            function,
                            idx,
                        });
                    }
                }
            }
        }
        for targets in out.values_mut() {
            targets.sort();
            targets.dedup();
        }
        Arc::new(out)
    }
}

/// The break marks of the assembly overviews on a drawn page (`assembly_break_marks()`, D-41), in the derived
/// order. A plain function of the tracked reads of `q`: the symbols layer that calls it depends on the marks.
pub fn page_break_marks(q: &Q, page: Id<Page>) -> Vec<BreakMarkView> {
    q.get::<ew_derive::AssemblyBreakMarks>(())
        .iter()
        .filter(|m| m.page == page)
        .map(|m| BreakMarkView {
            rep: m.rep,
            side: match m.side {
                ew_derive::BreakSide::Left => BreakSide::Left,
                ew_derive::BreakSide::Right => BreakSide::Right,
            },
        })
        .collect()
}

/// `page_cp_targets(page)`: the CPs of a drawn page that a report row links to, with the representation that
/// draws each one's function there (the named destinations `ew:cp:<uuid>` of doc 09 §4.2): the page's part of
/// [`CpTargetsByPage`], its own memo, so a row change that keeps a page's targets is cut off here.
#[derive(Debug)]
pub struct PageCpTargetsOf;

impl Query for PageCpTargetsOf {
    type Key = Id<Page>;
    type Value = Vec<CpTarget>;
    const FAMILY: &'static str = "page_cp_targets";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<CpTarget>> {
        Arc::new(q.get::<CpTargetsByPage>(()).get(key).cloned().unwrap_or_default())
    }
}

/// `page_table(page)`: the printed table of a generated page; `None` for other pages. Its own memo: it reads
/// `page_order` and the block's rows, so a page-order or row change that keeps this page's printed table is cut
/// off here and does not rebuild its tables layer.
#[derive(Debug)]
pub struct PageTableOf;

impl Query for PageTableOf {
    type Key = PageRef;
    type Value = Option<TableView>;
    const FAMILY: &'static str = "page_table";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<Option<TableView>> {
        // A drawn page or a divider has no table. Its record is still read, so that deleting it marks this memo
        // and the store collects it (memo keys of deleted records must not accumulate).
        match *key {
            PageRef::Drawn(p) => {
                let _ = q.pages().get(p);
                return Arc::new(None);
            }
            PageRef::Divider(s) => {
                let _ = q.sections().get(s);
                return Arc::new(None);
            }
            PageRef::Generated { .. } => {}
        }
        let order = q.get::<PageOrder>(());
        Arc::new(order.generated(*key).map(|g| table_view(q, g, *key)))
    }
}

/// `page_embedded(page)`: the embedded blocks of a drawn page that the scene draws, the `profile_tables` blocks.
/// Its own memo: it reads every report block.
#[derive(Debug)]
pub struct PageEmbeddedOf;

impl Query for PageEmbeddedOf {
    type Key = Id<Page>;
    type Value = Vec<EmbeddedView>;
    const FAMILY: &'static str = "page_embedded";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<EmbeddedView>> {
        let page = *key;
        let out = q
            .report_blocks()
            .iter()
            .filter_map(|(_, b)| {
                let embed = b.embedded.as_ref().filter(|e| e.page == page)?;
                matches!(b.source, ReportSource::Kind(ReportKind::ProfileTables)).then(|| EmbeddedView {
                    rect: embed.rect.0,
                    content: EmbeddedContent::Cover(cover_tables(q, b)),
                })
            })
            .collect();
        Arc::new(out)
    }
}

/// `page_grid(page)`: the connection grid of a page of the page order: a drawn page's own, else the 4 mm grid. Its
/// own memo, so that a page edit that keeps the grid (a renumber, a title) does not rebuild the wires.
#[derive(Debug)]
pub struct PageGridOf;

impl Query for PageGridOf {
    type Key = PageRef;
    type Value = Grid;
    const FAMILY: &'static str = "page_grid";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<Grid> {
        Arc::new(match *key {
            PageRef::Drawn(p) => q.pages().get(p).map_or(Grid::MM4, |pg| pg.grid),
            _ => Grid::MM4,
        })
    }
}

/// The connection grid of a page ([`PageGridOf`]).
pub(crate) fn page_grid(q: &Q, page: PageRef) -> Grid {
    *q.get::<PageGridOf>(page)
}

fn slot_values(q: &Q, page: Id<Page>, rep: &Representation, def: &SymbolDef) -> Vec<SlotValue> {
    let Some(&function) = rep.functions.first() else {
        return Vec::new();
    };
    let f = q.functions().get(function).cloned();
    let device = f.as_ref().and_then(|f| f.device);
    let dev = device.and_then(|d| q.devices().get(d).cloned());
    let mut out = Vec::new();
    for (i, slot) in def.text_slots.iter().enumerate() {
        let mut item_links = Vec::new();
        let text = match &slot.kind {
            SlotKind::Tag => match (&f, device) {
                (Some(f), _) if matches!(f.kind, FunctionKind::Ip(_)) => None,
                // ID-162 (doc 08 §6.7): a terminal's own representation prints only its designation.
                (_, Some(_)) if dev.as_ref().is_some_and(|d| d.aspects.terminal.is_some()) => dev
                    .as_ref()
                    .and_then(|d| d.aspects.terminal.as_ref())
                    .map(|t| t.designation.as_str().to_owned()),
                (_, Some(d)) => Some((*q.get::<DisplayedTag>((d, page))).clone()),
                _ => None,
            },
            SlotKind::CpDesignation(idx) => {
                let (fun, fidx) = match rep.pin_map.iter().find(|p| p.symbol_cp == *idx) {
                    Some(p) => (p.function, p.idx),
                    None => (
                        function,
                        def.cps.iter().find(|c| c.idx == *idx).map_or(*idx, |c| c.fn_cp),
                    ),
                };
                q.get::<FunctionCps>(fun)
                    .iter()
                    .find(|c| c.idx == fidx)
                    .map(|c| c.designation.clone())
            }
            // Interruption points have `function_text` and `xref` slots, no `tag`; the former prints the pairing
            // name (ID-166).
            SlotKind::FunctionText => match f.as_ref().map(|f| &f.kind) {
                Some(FunctionKind::Ip(ip)) => Some(ip.name.as_str().to_owned()),
                _ => dev.as_ref().map(|d| neutral(&d.function_text)),
            },
            SlotKind::ProcessTag => dev
                .as_ref()
                .and_then(|d| d.process_tag.as_ref().map(|t| t.as_str().to_owned())),
            SlotKind::TechData => device.map(|d| tech_data_text(q, d)),
            // FT-265: beside an `order_code` slot the manufacturer is the configured part's of that order code (the
            // DP port of folio 3), else the main part's.
            SlotKind::Manufacturer => device
                .and_then(|d| {
                    def.text_slots
                        .iter()
                        .any(|s| matches!(s.kind, SlotKind::OrderCode))
                        .then(|| order_code_part(q, d))
                        .flatten()
                        .or_else(|| main_part(q, d))
                })
                .map(|p| manufacturer_name(q, &p.item.manufacturer)),
            SlotKind::PartNumber => device
                .and_then(|d| main_part(q, d))
                .map(|p| p.item.part_number.as_str().to_owned()),
            SlotKind::OrderCode => device.map(|d| order_code_text(q, d)),
            SlotKind::TypeDesignation => device
                .and_then(|d| main_part(q, d))
                .and_then(|p| p.item.type_designation.as_ref().map(|t| t.as_str().to_owned())),
            SlotKind::Xref => {
                let x = q.get::<XrefTextOf>(rep.id);
                // One link per listed item, so the main list "∞ 3-B2, 106-G3" links both targets.
                item_links = x
                    .links
                    .iter()
                    .map(|l| ItemLink {
                        range: l.range.clone(),
                        dest: Destination::Representation(l.rep.as_uuid()),
                    })
                    .collect();
                Some(x.text.clone())
            }
            _ => None,
        };
        if let Some(text) = text.filter(|t| !t.is_empty()) {
            out.push(SlotValue {
                slot: u16::try_from(i).unwrap_or(u16::MAX),
                text,
                link: None,
                item_links,
            });
        }
    }
    out
}

/// The three tables of a `profile_tables` block as printed texts.
fn cover_tables(q: &Q, block: &ReportBlock) -> CoverTables {
    let set = q.get::<BlockRows>(block.id);
    let opts = RenderOptions::default();
    let cell = |row: &Row, key: &str| -> String {
        set.column(key)
            .and_then(|i| row.cells.get(i))
            .map(|c| render(q, c, &opts))
            .unwrap_or_default()
    };
    let label = |key: &str| -> String {
        set.column(key)
            .map(|i| neutral(&set.columns[i].label))
            .unwrap_or_default()
    };
    let colour_keys = ["voltage_class", "function", "potential_type", "colour"];
    let mut cover = CoverTables::default();
    cover.colours.header = colour_keys.map(label).to_vec();
    for row in &set.rows {
        let text = cell(row, "voltage_class");
        match (cell(row, "table").as_str(), row.kind) {
            ("colours", RowKind::GroupHeader) => cover.colours.title = text,
            ("control", RowKind::GroupHeader) => cover.control.title = text,
            ("mains", RowKind::GroupHeader) => cover.mains.title = text,
            ("colours", RowKind::GroupFooter) => cover.colours.caption = Some(text),
            ("note", RowKind::GroupFooter) => cover.note = Some(text),
            ("colours", RowKind::Data) => cover
                .colours
                .rows
                .push(colour_keys.map(|k| cell(row, k)).to_vec()),
            ("control", RowKind::Data) => cover
                .control
                .rows
                .push(["colour", "size"].map(|k| cell(row, k)).to_vec()),
            ("mains", RowKind::Data) => cover
                .mains
                .rows
                .push(["colour", "size"].map(|k| cell(row, k)).to_vec()),
            _ => {}
        }
    }
    cover
}

/// The drawn row links of a report row (FR-REP-042): the links to a connection point, which the PLC overview
/// sets to the coil of each bound channel (doc 10 §5).
fn row_links(r: &Row) -> Vec<(usize, Destination)> {
    r.links
        .iter()
        .filter_map(|l| match l.target {
            LinkTarget::Cp { function, idx } => {
                let cp: Id<()> = ew_ids::v5::cp_id(function, u32::from(idx));
                Some((usize::from(l.column), Destination::Cp(cp.as_uuid())))
            }
            _ => None,
        })
        .collect()
}

/// The printed table of a generated page (see the module documentation).
fn table_view(q: &Q, g: &GeneratedPage, page: PageRef) -> TableView {
    let set = q.get::<BlockRows>(g.block);
    let opts = RenderOptions {
        page: Some(page),
        ..RenderOptions::default()
    };
    let columns = set
        .columns
        .iter()
        .map(|c| TableColumn {
            header: neutral(&c.label),
            weight: c.weight,
        })
        .collect();
    if let Some(note) = &g.note {
        return TableView {
            columns,
            rows: Vec::new(),
            note: Some(neutral(note)),
        };
    }
    let end = (g.rows.end as usize).min(set.rows.len());
    let start = (g.rows.start as usize).min(end);
    let rows = set.rows[start..end]
        .iter()
        .map(|r| TableRow {
            kind: match r.kind {
                RowKind::Data => TableRowKind::Data,
                RowKind::Heading | RowKind::GroupHeader | RowKind::GroupFooter => TableRowKind::Heading,
            },
            cells: r.cells.iter().map(|c| render(q, c, &opts)).collect(),
            links: row_links(r),
        })
        .collect();
    TableView {
        columns,
        rows,
        note: None,
    }
}

/// The terminals a strip-segment representation draws (see the module documentation); empty for any other
/// representation. The page order is read only for a strip segment.
fn segment_terminals(q: &Q, rep: &Representation) -> Vec<SegmentTerminal> {
    if rep.rep_type != RepType::StripSegment {
        return Vec::new();
    }
    let Some(strip) = rep
        .functions
        .first()
        .and_then(|f| q.functions().get(*f))
        .and_then(|f| f.device)
    else {
        return Vec::new();
    };
    let order = q.get::<PageOrder>(());
    // The strip's segments in (page-order position, y, x, ID) order.
    let mut segments: Vec<(usize, i64, i64, Id<Representation>)> = q
        .idx()
        .functions_of_device(strip)
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
        .filter_map(|r| q.reps().get(r).cloned())
        .filter(|r| r.rep_type == RepType::StripSegment)
        .map(|r| {
            let at = order.position(PageRef::Drawn(r.page)).unwrap_or(usize::MAX);
            (at, r.pos.y.0, r.pos.x.0, r.id)
        })
        .collect();
    segments.sort_unstable();
    segments.dedup();
    let Some(j) = segments.iter().position(|s| s.3 == rep.id) else {
        return Vec::new();
    };
    // The strip's terminals in (seq, ID) order.
    let mut terminals: Vec<(u32, Id<ew_schema::Device>)> = q
        .idx()
        .terminals_of_strip(strip)
        .filter_map(|t| {
            let seq = q.devices().get(t)?.aspects.terminal.as_ref()?.seq;
            Some((seq, t))
        })
        .collect();
    terminals.sort_unstable();
    let (n, m) = (terminals.len(), segments.len());
    terminals[j * n / m..(j + 1) * n / m]
        .iter()
        .filter_map(|&(_, t)| {
            let fns: Vec<Id<Function>> = q.idx().functions_of_device(t).collect();
            let main = fns
                .iter()
                .copied()
                .find(|f| q.functions().get(*f).is_some_and(|f| f.role == FnRole::Main))
                .or_else(|| fns.first().copied())?;
            let cps = q.get::<FunctionCps>(main);
            let side = |want: Side| {
                cps.iter()
                    .filter(|c| c.side == Some(want.clone()))
                    .map(|c| c.idx)
                    .collect()
            };
            Some(SegmentTerminal {
                function: main,
                side1: side(Side::Side1),
                side2: side(Side::Side2),
            })
        })
        .collect()
}

impl crate::Session {
    /// How many connections carry a wire marker of their own under the project's current marker mode (FT-554): the
    /// count that `project.marker_mode` reports after a switch (`0 wires carry a marker`).
    pub fn marker_wire_count(&self) -> usize {
        let q = self.query();
        let mode = ew_derive::marker_mode(&q);
        q.connections()
            .iter()
            .filter(|(_, c)| ew_derive::shows_marker(&q, c, mode))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// The seal of each compiled-in generic symbol (doc 05 §13.2: the SHA-256 of the item's compact JSON) holds,
    /// equals the pinned [`GENERIC_SYMBOL_SEALS`], and the pinned profile is the newest shipped derivation profile:
    /// regenerating `content/symbols/qet10` fails here instead of silently changing every project's output.
    #[test]
    fn the_compiled_in_generic_symbols_keep_their_seals_and_profile() {
        for (json, (key, seal)) in [JUNCTION_DOT, JUNCTION_BEVEL, WIRE_MARKER]
            .into_iter()
            .zip(GENERIC_SYMBOL_SEALS)
        {
            let Ok(LibDocument::Symbol(copy)) = LibDocument::from_json(json) else {
                panic!("{key} is a symbol copy");
            };
            assert_eq!(copy.item.meta.key.as_ref().map(|k| k.as_str()), Some(key));
            let compact = serde_json::to_string(&copy.item).expect("a symbol serialises");
            let own = format!("sha256:{:x}", Sha256::digest(compact.as_bytes()));
            let stated = serde_json::to_value(copy.sha256).expect("a seal serialises");
            assert_eq!(
                stated.as_str(),
                Some(own.as_str()),
                "{key}: the copy's seal does not hold"
            );
            assert_eq!(
                own, seal,
                "{key}: the compiled-in symbol changed; ship it with a new derivation profile"
            );
        }
        let newest = ew_derive::shipped_profiles()
            .iter()
            .map(|p| p.version.as_str())
            .max()
            .expect("a shipped profile");
        assert_eq!(
            newest, GENERIC_SYMBOLS_PROFILE,
            "re-seal the generic symbols for the new profile"
        );
    }
}
