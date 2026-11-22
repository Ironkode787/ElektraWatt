// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The glue rules of the page view (doc 09 §4.1): the parts
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
//! | [`PageSlotsOf`] (page) | Memo | the slot texts and black-box pin texts of every representation | labels |
//! | [`PageConnectionsOf`] (page) | Cold | the connections with their wire-marker texts | wires, labels |
//! | [`PageJunctionsOf`] (page) | Cold | the junctions with their symbols | wires, labels |
//! | [`PageCpTargetsOf`] (page) | Memo | the CPs a report row links to, the page's part of [`CpTargetsByPage`] (`()`, Memo) | symbols |
//! | [`PageTableOf`] (`PageRef`) | Memo | the printed table of a generated page | tables |
//! | [`PageEmbeddedOf`] (page) | Memo | the embedded report blocks of a drawn page | tables |
//!
//! # The rules (formerly the test glue of `tests/page_output/view.rs`)
//!
//! - **Page order**: `page_order` of a `Q` with the report engine installed (`Session::query()`), so the generated
//!   report pages take part in Prev/Next, the outline and `/PageLabels` (ARCH §6.4).
//! - **Page IDs** ([`page_uuid`]): a drawn page's ID, a divider's section ID, a generated page's v5 ID
//!   `v5(block, index)` (`GeneratedPage::id`); every PDF destination is unique.
//! - **Frame form**: `Page.form`, else the frame defaults of the project settings (doc 03 §5.1 `FrameDefaults`,
//!   resolved by `ew_derive::frame_form_of` like the cell grid: the entry of the page's type, else `default`), else the
//!   no-grid frame [`NOGRID_FRAME`] (the form of the project library with that key); a generated page's
//!   `GeneratedPage::form` (its block's form on the set's paper, `ew_reports::layout::list_frame`), else the no-grid
//!   frame; a divider the frame defaults' `section_divider` entry, else the
//!   no-grid frame. A frame-defaults form the project does not hold falls through to the next rule. The
//!   title block by `FormDef.title_block`.
//! - **Bindings**: every `title_block(page)` value, the project's date format, the project's language
//!   (`ew_derive::print_language`: its source language, doc 09 §4.7), which the multi-language values and the cells'
//!   labels print in.
//! - **Report tables**: a generated page's rows are the `GeneratedPage::rows` range of
//!   `report_rows(block)`, each cell printed by `ew_reports::render::render` for that page (the project's language,
//!   displayed tags); a page with a `note` prints the note instead. The printed columns are the block's own
//!   (`ReportBlock.columns`: those its rows have, in its order, with its widths and its headings or the kind's), else
//!   every column with its weight; the headings, the caption and the legend print in the project's language (doc 09
//!   §3.8). Every row link of a printed cell that resolves is drawn (FR-REP-042): a connection point, a page, a
//!   device's main representation or a representation, over the whole cell or over one listed folio of a `Folios`
//!   cell.
//! - **Embedded blocks** (FR-REP-028): every embedded block of a drawn page is drawn into its rectangle.
//!   A `profile_tables` block draws the cover: the rows split by their `table` column into the colour table, the
//!   control sizes and the mains sizes; a group-header row is the title of its table, a group-footer row of the
//!   colours its caption and the group-footer row of the group `note` the note under the tables. Every other block
//!   draws a boxed table ([`EmbeddedContent::Table`]) from its `BlockRows`, each cell printed by
//!   `ew_reports::render::render` for that page (the project's language, displayed tags; the cover too), as a generated
//!   page's table: the printed columns, title, header rows and flow columns are the kind's
//!   `ReportKindDef::embedded_layout` (a source
//!   without a registered kind prints every column under a header row), the block's own non-empty `title` replaces the
//!   kind's title, and a block whose rows do not derive prints the reason in words as its note (`NotDerived::words`, as a generated
//!   page's placeholder). An embedded block draws no back-links: only a PLC overview box's
//!   connection-point links.
//! - **Slot texts**: `tag` → `displayed_tag(device, page)`,
//!   except that a terminal's own representation prints its label, never the full tag (doc 08 §6.1: its function
//!   text when its strip's `label_mode` is FunctionText and the text is not empty, its `function_text` slot then
//!   empty; else its designation), a strip segment of a strip with `tag_print` PerPage prints it only at the strip's
//!   first segment on its page in (row, column, ID) order, and an interruption point has no `tag`;
//!   `cp_designation(i)` → the designation of the function CP that symbol CP `i` draws; `function_text` → the
//!   device's function text, or an interruption point's pairing name; `process_tag` → the device's process tag;
//!   `xref` → `xref_text(rep)` with one link per listed item; `tech_data` → `tech_data_text(device)`;
//!   `manufacturer`, `part_number` and `type_designation` → the device's main part (its first part use);
//!   `order_code` → `order_code_text(device)` (the order code of its first assembly config, D-36); `plc_address` →
//!   the first drawn function's `ew_derive::PlcAddressText` text; every other kind has no value. An
//!   empty text is not listed.
//! - **Strip tags at terminals drawn alone** (doc 08 §6.7, [`page_strip_tags`]): on a page that shows
//!   terminals of a strip, no segment of it and no text bound to its name (doc 06 §3.8), the strip's displayed tag for
//!   the page, once, at the strip's first
//!   terminal representation on the page (designation in natural order, then IDs) whose tag slot is shown
//!   (`ew_derive::tag_slot_shown`; none shown, no tag); the scene prints it one connection-grid step above that
//!   terminal's tag-slot anchor, so an unnumbered strip's terminals name it ("-X101?" on page 101 under a strip rule
//!   with `{page}`, "-X??" without a page token).
//! - **Bridges** (doc 08 §6.5, [`page_bridges`]): every bridge group of a strip that has a terminal on the page, with
//!   the clamp it joins on each terminal; the scene draws it where the terminals stand in one row.
//! - **Pin texts**: a black box (`Representation.box` with CPs, a symbol without CPs) lists, per
//!   box CP, the designation of that CP of its first function as stored and, when the function CP is bound to a PLC
//!   channel, the channel's printed address (`PlcAddressText::pins` of its function) after one space
//!   (`L+ I0.1`), in [`PageSlotsOf`] beside the slot texts; the scene prints them beside the pins. A designation the
//!   symbol's own `cp_designation` slot prints, or an empty one, is left out.
//! - **Break marks** (D-41, doc 11 §5.5 step 5): the `assembly_break_marks()` of a drawn page ([`page_break_marks`]),
//!   drawn by the scene at the side of their representation.
//! - **Wire markers** (`ew_derive::connection_marker`): with the project's `marker_mode`
//!   absent, connections with `overrides.show_marker` print the explicit cross-section and the colour codes (their
//!   file spelling, `GNYE`), joined by a space, and their effective values where they state neither; in a mode, the
//!   connections the mode selects and those with `show_marker` print their effective values (`none`: no marker). A connection a wire-marker item lists prints no marker of its own: the item prints the group.
//! - **Page items and structure boxes** ([`page_items`], [`page_boxes`]): a drawn page's items and
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
use ew_derive::ew_units::{Grid, Nm};
use ew_derive::{
    DisplayedTag, FunctionCps, GeneratedPage, MainRep, PageOrder, PageRef, PlcAddressText, Policy, Q, Query,
    RepAnchorCell, TbValue, TitleBlockValues, XrefTextOf, box_caption, connection_marker, connection_number,
    frame_form_of, free_text, group_marker, grouped_connections, main_part, manufacturer_name,
    order_code_part, order_code_text, tech_data_text,
};
use ew_dl::Destination;
use ew_reports::render::{RenderOptions, folios_printed, render};
use ew_reports::{BlockRows, Cell, EmbeddedLayout, LinkTarget, Registry, Row, RowKind, RowSet, RowSetStatus};
use ew_scene::embedded::{CoverTables, EmbeddedContent, EmbeddedTable, EmbeddedView, Lookup};
use ew_scene::forms::{AssetRef, BoundValue, PageBindings};
use ew_scene::page::{
    BreakMarkView, BreakSide, BridgeView, CellView, ConnectionView, CpTarget, FrameView, ItemLink,
    JunctionView, PinText, PlugNoteView, RepView, SegmentTerminal, SlotValue, StripTagView, SymbolRef,
    ValveCellsView, ValveGlyph, ValveGlyphsView,
};
use ew_scene::table::{TableColumn, TableLink, TableRow, TableRowKind, TableView};
use ew_schema::{
    Device, FnRole, FormDef, Function, FunctionKind, JunctionStyle, LabelMode, LibCopy, LibDocument, Page,
    PageType, RepType, ReportBlock, ReportKind, ReportSource, Representation, Side, SlotKind, SymbolDef,
    TagPrint,
};

/// The tag of the cable a bus line is linked to (doc 10 §2.7), printed beside the line as a wire number is; `None` for
/// another connection, a line without a cable and one whose cable is drawn on the line's page (its own symbol prints
/// the tag there).
fn bus_cable_text(q: &Q, c: &ew_schema::Connection) -> Option<String> {
    let ew_schema::ConnectionKind::Bus(line) = &c.kind else {
        return None;
    };
    let cable = line.cable?;
    let drawn = q.idx().functions_of_device(cable).any(|f| {
        q.idx()
            .reps_of_function(f)
            .any(|r| q.reps().get(r).is_some_and(|r| Some(r.page) == c.page))
    });
    (!drawn).then(|| super::properties::device_tag_text(q, cable))
}

/// The key of the frame form of dividers, and of pages without a form.
pub const NOGRID_FRAME: &str = "ew.frame.qet-a4-landscape-nogrid";
/// The key of the shipped frame with the column and row grid.
pub const GRID_FRAME: &str = "ew.frame.qet-a4-landscape-grid";
/// The key of the shipped cover frame.
pub const COVER_FRAME: &str = "ew.frame.qet-a4-landscape-cover";
/// The key of the shipped title block, which the shipped A4 frames name.
pub const TITLE_BLOCK: &str = "ew.titleblock.qet-a4";
/// The key of the shipped A3 frame with the grid (FT-1826), which names the A3 title block.
pub const A3_GRID_FRAME: &str = "ew.frame.qet-a3-landscape-grid";

/// The shipped frame and title-block forms of `content/forms` (the QElectroTech-compatible A4 landscape frames with
/// and without the grid, the cover frame and the title block they name; the A3 landscape frame with the grid and its
/// title block, doc 09 §3.8), in the `lib/` JSON form that `library.embed` takes. A new empty project holds them and
/// names the A4 ones in its frame defaults (`crate::shipped_frames`); the wizard embeds them into a template.
pub const SHIPPED_FORMS: [&str; 6] = [
    include_str!("../../../../content/forms/ew.frame.qet-a3-landscape-grid.json"),
    include_str!("../../../../content/forms/ew.frame.qet-a4-landscape-cover.json"),
    include_str!("../../../../content/forms/ew.frame.qet-a4-landscape-grid.json"),
    include_str!("../../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"),
    include_str!("../../../../content/forms/ew.titleblock.qet-a3.json"),
    include_str!("../../../../content/forms/ew.titleblock.qet-a4.json"),
];

/// The built-in generic symbols the page view draws without a record naming them.
#[derive(Clone, Debug)]
pub struct GenericSymbols {
    /// `ew.gen.junction-dot@qet10`.
    pub junction_dot: Option<SymbolRef>,
    /// `ew.gen.junction-bevel@qet10`.
    pub junction_bevel: Option<SymbolRef>,
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

/// The derivation profile the compiled-in generic symbols belong to: their seals below are part of that
/// profile's output. Regenerating `content/symbols/qet10` changes a seal, and the seal test then fails until the seal
/// here is turned: until 1.0 under the one shipped profile, the change said in the release notes (ADR-0003
/// amendment 3); from 1.0 with a new profile.
#[cfg(test)]
const GENERIC_SYMBOLS_PROFILE: &str = "2027.1";

/// The `sha256` seals (doc 05 §13.2) of the compiled-in generic symbols of [`GENERIC_SYMBOLS_PROFILE`], by key.
#[cfg(test)]
const GENERIC_SYMBOL_SEALS: [(&str, &str); 2] = [
    (
        "ew.gen.junction-dot@qet10",
        "sha256:5c96763c7eb1041383074d1ba74a8148a5c764c0e5f9f7feed8e56cafc0e2ceb",
    ),
    (
        "ew.gen.junction-bevel@qet10",
        "sha256:1ff672d67bff0c12dd1626c6f96150638f6e232bbecdfde0054a5857b9a49ac4",
    ),
];

/// The built-in generic symbols, parsed once (compiled in from `content/symbols/qet10`, sealed by
/// the seals pinned in this module's tests).
pub fn generic_symbols() -> &'static GenericSymbols {
    static SET: OnceLock<GenericSymbols> = OnceLock::new();
    SET.get_or_init(|| GenericSymbols {
        junction_dot: builtin_symbol(JUNCTION_DOT),
        junction_bevel: builtin_symbol(JUNCTION_BEVEL),
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

/// The held frame form a page of the page order prints on, by its ID ([`PageFrameOf`]'s, which the Pages navigator
/// names): a drawn page's own or its type's default, a generated page's block's, a section divider's default, else the
/// no-grid frame; `None` when the project holds none of them.
pub(crate) fn page_frame_id(q: &Q, key: &PageRef) -> Option<Id<FormDef>> {
    let own = match *key {
        PageRef::Drawn(p) => q
            .pages()
            .get(p)
            .and_then(|pg| frame_form_of(q, pg.form, &pg.kind, true)),
        PageRef::Generated { .. } => q.get::<PageOrder>(()).generated(*key).map(|g| g.form),
        PageRef::Divider(_) => frame_form_of(q, None, &PageType::SectionDivider, false),
    };
    own.filter(|id| q.forms().get(*id).is_some()).or_else(|| {
        q.forms()
            .iter()
            .find(|(_, f)| {
                f.item
                    .meta
                    .key
                    .as_ref()
                    .is_some_and(|k| k.as_str() == NOGRID_FRAME)
            })
            .map(|(id, _)| id)
    })
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
        let frame = page_frame_id(q, key)
            .and_then(|id| form(q, id))
            .map(|form| FrameForms {
                title_block: form.title_block.and_then(|id| self::form(q, id)),
                form,
            });
        Arc::new(frame)
    }
}

/// The page-to-sheet map of a new page of type `kind` without a form of its own: the map of the frame that
/// [`PageFrameOf`] gives it once it exists, the identity without one.
pub(crate) fn new_page_map(q: &Q, kind: &PageType) -> ew_scene::forms::SheetMap {
    frame_form_of(q, None, kind, true)
        .and_then(|id| form(q, id))
        .or_else(|| form_by_key(q, NOGRID_FRAME))
        .map_or(ew_scene::forms::SheetMap::IDENTITY, |f| {
            ew_scene::forms::SheetMap::of(&f)
        })
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
        b.language = ew_derive::print_language(q);
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
        // In the digest branch of doc 15 §5.6 the page's change status comes from its digest, which
        // `ew-derive` cannot see (the title block keeps the no-digest value).
        if q.get::<TitleBlockValues>(*key)
            .get("page.last_changed_in")
            .is_some()
            && let Some(label) = super::revisions::digest_last_changed_in(q, *key)
        {
            let _ = b.set("page.last_changed_in", BoundValue::Text(label));
        }
        Arc::new(b)
    }
}

/// The form layer's reference of an asset: the record's reference, hash, format and intrinsic size (doc 05 §13.3:
/// readers trust it and never sniff).
fn asset_ref(r: &ew_schema::AssetRef) -> AssetRef {
    let format = match r.format {
        ew_schema::AssetFormat::Svg => ew_dl::ImageFormat::Svg,
        // An underlay source or a font named as a logo (a hand-edited file) reads as a PNG that does not decode: reported.
        ew_schema::AssetFormat::Png
        | ew_schema::AssetFormat::Pdf
        | ew_schema::AssetFormat::Dxf
        | ew_schema::AssetFormat::Ttf
        | ew_schema::AssetFormat::Otf => ew_dl::ImageFormat::Png,
        ew_schema::AssetFormat::Jpeg => ew_dl::ImageFormat::Jpeg,
    };
    let base = AssetRef::new(r.sha256.0, format);
    match r.size {
        Some((w, h)) => base.with_size(w, h),
        None => base,
    }
}

/// A valve standing in a cell of a valve terminal's box drawn on `page` stands with its insertion point on the cell's
/// top line, so a PLC address place of its symbol higher than one grid step under that point ran across the line (the
/// valves with one solenoid). There each such place is drawn one step of the symbol's grid under it, the row of the
/// two-solenoid valves' places, inside the cell; a place a person moved, a turned valve and a valve drawn anywhere else
/// keep the symbol's place. Drawn only: the record is not changed.
fn addresses_in_cell(
    q: &Q,
    page: Id<Page>,
    rep: Arc<Representation>,
    def: &SymbolDef,
) -> Arc<Representation> {
    let Some(step) = def.grid.at(1).zip(def.grid.at(0)).map(|(a, b)| a - b) else {
        return rep;
    };
    let device_of = |r: &Representation| r.functions.first().and_then(|f| q.functions().get(*f)?.device);
    // The modules that hold the valve: its parent and theirs.
    let holders: Vec<Id<Device>> = device_of(&rep)
        .map(|d| ew_derive::parent_chain(q, d).into_iter().skip(1).collect())
        .unwrap_or_default();
    let in_cell = rep.orient == ew_derive::ew_units::Orient2::R0
        && !holders.is_empty()
        && q.idx().reps_by_page(page).any(|r| {
            q.reps().get(r).is_some_and(|b| {
                b.box_.as_ref().is_some_and(|x| x.slots.is_some())
                    && device_of(b).is_some_and(|d| holders.contains(&d))
            })
        });
    if !in_cell {
        return rep;
    }
    let mut out = (*rep).clone();
    for (i, slot) in def.text_slots.iter().enumerate() {
        let Ok(index) = u16::try_from(i) else { break };
        if slot.kind == SlotKind::PlcAddress && slot.pos.y < step && out.text_slots.get(&index).is_none() {
            out.text_slots.insert(ew_schema::SlotOverride {
                slot: index,
                pos: Some(ew_derive::ew_units::Pt2::new(slot.pos.x, step)),
                angle: None,
                visible: None,
                extra: Default::default(),
            });
        }
    }
    Arc::new(out)
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
            let rep = match &symbol {
                Some(def) => addresses_in_cell(q, *key, rep, def),
                None => rep,
            };
            out.push(RepView {
                rep,
                symbol,
                slots: Vec::new(),
                terminals,
                pins: Vec::new(),
            });
        }
        Arc::new(out)
    }
}

/// `page_slots(page)`: the slot texts and the pin texts of every representation of a drawn page that prints one, by
/// representation ID (see the module documentation).
#[derive(Debug)]
pub struct PageSlotsOf;

impl Query for PageSlotsOf {
    type Key = Id<Page>;
    type Value = Vec<(Id<Representation>, Vec<SlotValue>, Vec<PinText>)>;
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
            let pins = pin_texts(q, &rep, &symbol.item);
            if !slots.is_empty() || !pins.is_empty() {
                out.push((rep.id, slots, pins));
            }
        }
        out.sort_by_key(|(id, _, _)| *id);
        Arc::new(out)
    }
}

/// `page_connections(page)`: the connections drawn on a page with their wire-marker texts and, when the project prints
/// them, their wire numbers; a bus line that no cable mark lists prints its cable's tag where it has no number
/// ([`bus_cable_text`]).
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
        let grid = page_grid(q, PageRef::Drawn(*key));
        for cid in q.idx().connections_by_page(*key) {
            // As read: a vertex within 1 nm of a grid position on the grid's own number (06a §2.1).
            let Some(c) = q
                .connections()
                .get(cid)
                .map(|c| ew_derive::read_connection(c, grid))
            else {
                continue;
            };
            let marker = connection_marker(q, &c, grouped.contains(&cid));
            // A bus line in its cable's mark prints the tag there, not beside the line.
            let number = connection_number(q, cid)
                .or_else(|| (!grouped.contains(&cid)).then(|| bus_cable_text(q, &c)).flatten());
            out.push(ConnectionView {
                connection: c,
                marker,
                number,
            });
        }
        Arc::new(out)
    }
}

/// The page items of a drawn page in ID order. With `texts`, each with what it prints (a free
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
            // A marker over a cable's cores is the cable's mark, drawn whatever the marker mode.
            let cable = texts.then(|| ew_derive::cable_mark(q, &item)).flatten();
            ew_scene::items::ItemView {
                item,
                text,
                shown,
                asset,
                cable,
            }
        })
        .collect()
}

/// The structure boxes of a drawn page in ID order, with their captions when `captions` is set.
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
        let grid = page_grid(q, PageRef::Drawn(*key));
        let mut out = Vec::new();
        for jid in q.idx().junctions_by_page(*key) {
            // As read: a point within 1 nm of a grid position on the grid's own number (06a §2.1).
            let Some(j) = q.junctions().get(jid).map(|j| ew_derive::read_junction(j, grid)) else {
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
                    // An internal coil without a representation is drawn in its valve's symbol.
                    let drawn = ew_derive::coil_host(q, function).unwrap_or(function);
                    let mut seen = std::collections::BTreeSet::new();
                    for rid in q.idx().reps_of_function(drawn) {
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

/// The valve glyphs of the module overviews on a drawn page (doc 11 §5.5 step 3): for each overview
/// representation of a module with valve positions, one glyph per position from `valve_slots`, in representation-ID
/// order; a valve prints the part number of its first part use. A plain function of the tracked reads of `q`, like
/// `page_break_marks`.
pub fn page_valve_glyphs(q: &Q, page: Id<Page>) -> Vec<ValveGlyphsView> {
    let reps = q.reps();
    let functions = q.functions();
    let devices = q.devices();
    let parts = q.parts();
    let mut ids: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
    ids.sort();
    let entity = |rep: Id<Representation>, name: &[u8]| ew_ids::Uuid::new_v5(&rep.as_uuid(), name);
    let mut out = Vec::new();
    for rid in ids {
        let Some(rep) = reps.get(rid) else { continue };
        if rep.rep_type != RepType::Overview {
            continue;
        }
        let Some(module) = rep
            .functions
            .first()
            .and_then(|f| functions.get(*f))
            .and_then(|f| f.device)
        else {
            continue;
        };
        let slots = q.get::<ew_derive::ValveSlots>(module);
        if slots.slots.is_empty() {
            continue;
        }
        let glyphs = slots
            .slots
            .iter()
            .enumerate()
            .map(|(i, slot)| match slot {
                ew_derive::ValveSlot::Valve { device, .. } => ValveGlyph::Valve {
                    entity: entity(rid, device.as_uuid().as_bytes()),
                    part_number: devices
                        .get(*device)
                        .and_then(|d| d.parts.first())
                        .and_then(|u| parts.get(u.part.copy))
                        .map(|c| c.item.part_number.as_str().to_owned())
                        .unwrap_or_default(),
                },
                ew_derive::ValveSlot::Blank { device } => ValveGlyph::Blank {
                    entity: entity(rid, device.as_uuid().as_bytes()),
                },
                ew_derive::ValveSlot::Empty => ValveGlyph::Empty {
                    entity: entity(rid, format!("position\u{1F}{}", i + 1).as_bytes()),
                },
            })
            .collect();
        out.push(ValveGlyphsView { rep: rid, glyphs });
    }
    out
}

/// The cells of the valve terminals drawn as their positions on a drawn page (doc 11 §3.2): for each box
/// representation with `slots`, in representation-ID order, its frame and per cell (`terminal_cells`) its number and
/// what it holds (a valve not drawn on this page with its tag and where it is drawn), whether its supply stands on the
/// page, and the valves' ports that end on its sub-base with the gallery each joins. A plain
/// function of the tracked reads of `q`, like `page_valve_glyphs`.
pub fn page_valve_cells(q: &Q, page: Id<Page>) -> Vec<ValveCellsView> {
    let reps = q.reps();
    let functions = q.functions();
    let mut ids: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
    ids.sort();
    let drawn_here = |device: Id<Device>| {
        q.idx().functions_of_device(device).any(|f| {
            q.idx()
                .reps_of_function(f)
                .any(|r| reps.get(r).is_some_and(|r| r.page == page))
        })
    };
    let mut out = Vec::new();
    for rid in ids {
        let Some(rep) = reps.get(rid) else { continue };
        if rep.box_.as_ref().and_then(|b| b.slots.as_ref()).is_none() {
            continue;
        }
        let Some(terminal) = rep
            .functions
            .first()
            .and_then(|f| functions.get(*f))
            .and_then(|f| f.device)
        else {
            continue;
        };
        // The page is read only where a box is drawn as its positions: a page record's change (a renumber) leaves the
        // symbol layers of the other pages alone. Each cell is as wide as its valve needs.
        let Some(frame) = ew_derive::box_frame(q, rep) else {
            continue;
        };
        let (_, band, _) = frame.rows();
        let mut taps: Vec<(Nm, usize)> = Vec::new();
        let cells = ew_derive::terminal_cells(q, terminal)
            .into_iter()
            .map(|c| {
                let view = match c.slot {
                    ew_derive::ValveSlot::Valve { device, .. } if drawn_here(device) => {
                        // The ports that end on the sub-base, each joined to its gallery.
                        for f in ew_derive::valve_functions(q, device) {
                            for cp in q.get::<ew_derive::FunctionCps>(f).iter() {
                                let k = ew_derive::GALLERY_PORTS
                                    .iter()
                                    .position(|g| cp.port_role.as_ref() == Some(g));
                                if let (Some(k), Some(at)) = (k, ew_derive::cp_point(q, f, cp.idx, page))
                                    && at.y == band
                                {
                                    taps.push((at.x, k));
                                }
                            }
                        }
                        CellView::Valve {
                            elsewhere: None,
                            drawn_at: None,
                        }
                    }
                    ew_derive::ValveSlot::Valve { device, .. } => CellView::Valve {
                        elsewhere: Some(super::properties::device_tag_text(q, device)),
                        drawn_at: drawn_elsewhere(q, device),
                    },
                    ew_derive::ValveSlot::Blank { .. } => CellView::Blank,
                    ew_derive::ValveSlot::Empty => CellView::Empty,
                };
                (c.position, view)
            })
            .collect();
        let supply = ew_derive::terminal_supply(q, terminal).is_some_and(|f| {
            q.idx()
                .reps_of_function(f)
                .any(|r| reps.get(r).is_some_and(|r| r.page == page))
        });
        out.push(ValveCellsView {
            rep: rid,
            frame,
            cells,
            supply,
            taps,
        });
    }
    out
}

/// Where a valve of a cell that is not drawn on the cell's page is drawn: the place of its main symbol
/// (`<folio>-<cell>`, the folio alone off the frame grid) and that symbol; `None` for a valve drawn nowhere.
fn drawn_elsewhere(q: &Q, device: Id<Device>) -> Option<(String, Id<Representation>)> {
    let rep = (*q.get::<ew_derive::MainRep>(device))?;
    let r = q.reps().get(rep)?;
    let folio = super::properties::folio_of(q, r.page)?;
    let place = match q.get::<ew_derive::CellOf>((r.page, r.pos)).as_ref() {
        Some(cell) => format!("{folio}-{cell}"),
        None => folio,
    };
    Some((place, rep))
}

/// The texts beside the pins of the multipin valve terminals' plugs on a drawn page: for each box
/// representation of a multipin terminal that is no box drawn as its positions, in representation-ID order, a text
/// per box CP that a pin of the plug is (`ew_derive::plug_pins`): the valve's tag and the solenoid where a valve holds
/// the position (`-201K2-Q1 14`), else the position and the solenoid (`1 · 14`), and `COM` for the common. A plain
/// function of the tracked reads of `q`, like `page_valve_cells`.
pub fn page_plug_notes(q: &Q, page: Id<Page>) -> Vec<PlugNoteView> {
    let reps = q.reps();
    let functions = q.functions();
    let mut ids: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
    ids.sort();
    let mut out = Vec::new();
    for rid in ids {
        let Some(rep) = reps.get(rid) else { continue };
        let Some(b) = rep
            .box_
            .as_ref()
            .filter(|b| b.slots.is_none() && !b.cps.is_empty())
        else {
            continue;
        };
        let Some(&function) = rep.functions.first() else {
            continue;
        };
        let Some(terminal) = functions.get(function).and_then(|f| f.device) else {
            continue;
        };
        let pins = ew_derive::plug_pins(q, terminal);
        if pins.is_empty() {
            continue;
        }
        let cps = q.get::<ew_derive::FunctionCps>(function);
        for cp in &b.cps {
            let Some(designation) = cps
                .iter()
                .find(|c| c.idx == cp.idx)
                .map(|c| c.designation.clone())
            else {
                continue;
            };
            let Some((_, carried)) = pins.iter().find(|(d, _)| *d == designation) else {
                continue;
            };
            let text = match carried {
                ew_derive::PlugPin::Solenoid {
                    valve: Some(v),
                    solenoid,
                    ..
                } => {
                    format!("{} {solenoid}", super::properties::device_tag_text(q, *v))
                }
                ew_derive::PlugPin::Solenoid {
                    position, solenoid, ..
                } => format!("{position} · {solenoid}"),
                ew_derive::PlugPin::Common => "COM".to_owned(),
            };
            let o = rep.orient;
            let at = rep.pos + o.apply(cp.pos);
            let dir = crate::geometry::orient_dir(
                crate::geometry::orient_index(o),
                super::editing::dir_of(&cp.dir),
            );
            let (vertical, forward) = match dir {
                crate::geometry::Dir::Up => (true, false),
                crate::geometry::Dir::Down => (true, true),
                crate::geometry::Dir::Left => (false, false),
                crate::geometry::Dir::Right => (false, true),
            };
            out.push(PlugNoteView {
                rep: rid,
                at,
                vertical,
                forward,
                text,
            });
        }
    }
    out
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

/// `page_embedded(page)`: the embedded blocks of a drawn page, each drawn by the scene into its rectangle (see the
/// module documentation). Its own memo: it reads every report block and the rows of the page's blocks.
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
                let content = if matches!(b.source, ReportSource::Kind(ReportKind::ProfileTables)) {
                    EmbeddedContent::Cover(Box::new(cover_tables(q, b)))
                } else {
                    EmbeddedContent::Table(boxed_table(q, b, page))
                };
                Some(EmbeddedView {
                    rect: embed.rect.0,
                    content,
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

/// The pin texts of a black box: per box CP of a representation whose symbol has no CPs, the
/// designation of that CP of its first function as stored (repeated designations print as they are), then, when the
/// CP is bound to a PLC channel, the channel's printed address after one space; in box-CP order. A designation that
/// a `cp_designation` slot of the symbol already prints, or an empty one, is left out (no text twice); a box CP with
/// neither a designation to print nor an address has no entry.
fn pin_texts(q: &Q, rep: &Representation, def: &SymbolDef) -> Vec<PinText> {
    let (Some(b), Some(&function)) = (&rep.box_, rep.functions.first()) else {
        return Vec::new();
    };
    if !def.cps.is_empty() || b.cps.is_empty() {
        return Vec::new();
    }
    // The one socket of an interface's box (an interface that counts no ports, doc 10 §2.7) is named by the
    // interface, as a single-line bus page names it, not by the port function's first pin ("A" of the DP port).
    if let ([cp], Some(label)) = (b.cps.as_slice(), port_box_label(q, rep, def)) {
        return vec![PinText {
            idx: cp.idx,
            text: label,
            cell: None,
        }];
    }
    let texts = q.get::<PlcAddressText>(function);
    let cps = q.get::<FunctionCps>(function);
    let designation = |idx: u16| cps.iter().find(|c| c.idx == idx).map(|c| c.designation.as_str());
    let mut pins = compose_pin_texts(rep, def, function, designation, &texts.pins);
    card_cells(q, rep, function, &mut pins);
    pins
}

/// The least width and the least height in grid steps of the black box `r` that keep the pin texts of its opposite
/// sides apart and inside it (the box collapsed through them), by one rule for the four sides: a pin's text reaches
/// in from its pin by the scene's `PIN_TEXT_GAP` and its extent along the pin's inward normal as the scene prints it
/// (on a left or right side horizontal, so its length; on a top or bottom side turned a quarter, so its length, or one
/// line's height for a text of signs only, which the scene prints horizontally); across the box the longest text of
/// each of the two sides, with half a millimetre between them. 0 across a box whose two sides there print no pin text,
/// and both for a representation that is no black box.
pub(crate) fn box_text_room(q: &Q, r: &Representation) -> (i64, i64) {
    let (Some(symbol), Some(b)) = (q.symbols().get(r.symbol), r.box_.as_ref()) else {
        return (0, 0);
    };
    let step = page_grid(q, PageRef::Drawn(r.page))
        .at(1)
        .map_or(1, |n| n.0.max(1));
    let texts = pin_texts(q, r, &symbol.item);
    let line = crate::part_box::pin_text_line().unwrap_or(0);
    // How far the longest text of the side `dir` reaches into the box from it.
    let side = |dir: &ew_schema::Dir4| {
        let turned = matches!(dir, ew_schema::Dir4::Up | ew_schema::Dir4::Down);
        b.cps
            .iter()
            .filter(|cp| ew_scene::symbol::transform_dir(&cp.dir, r.orient) == *dir)
            .filter_map(|cp| texts.iter().find(|t| t.idx == cp.idx))
            .map(|t| {
                let signs = !t.text.chars().any(char::is_alphanumeric);
                let reach = if t.cell.is_some() {
                    // Level in a card's cell: its line and the function text's under it.
                    2 * line
                } else if turned && signs {
                    line
                } else {
                    crate::part_box::pin_text_length(&t.text)
                };
                ew_scene::page::PIN_TEXT_GAP.0 + reach
            })
            .max()
            .unwrap_or(0)
    };
    let steps = |a: ew_schema::Dir4, b: ew_schema::Dir4| match side(&a) + side(&b) {
        0 => 0,
        reach => (reach + 500_000 + step - 1) / step,
    };
    (
        steps(ew_schema::Dir4::Left, ew_schema::Dir4::Right),
        steps(ew_schema::Dir4::Up, ew_schema::Dir4::Down),
    )
}

/// The interface a box of a port function draws, in words ("X1", "X1 P1R"; doc 10 §2.7): a black box (a symbol
/// without CPs on a representation with a box) whose first function carries a bus port; `None` otherwise.
fn port_box_label(q: &Q, rep: &Representation, def: &SymbolDef) -> Option<String> {
    if !def.cps.is_empty() || rep.box_.is_none() {
        return None;
    }
    let port = q.functions().get(*rep.functions.first()?)?.bus_port.clone()?;
    Some(match &port.port_label {
        Some(l) => format!("{} {}", port.interface.as_str(), l.as_str()),
        None => port.interface.as_str().to_owned(),
    })
}

/// [`pin_texts`] from the function's CP designations (`designation`, by function CP index) and the printed
/// addresses of its bound CPs (`addresses`, `(idx, text)`).
fn compose_pin_texts<'a>(
    rep: &Representation,
    def: &SymbolDef,
    function: Id<Function>,
    designation: impl Fn(u16) -> Option<&'a str>,
    addresses: &[(u16, String)],
) -> Vec<PinText> {
    let Some(b) = &rep.box_ else {
        return Vec::new();
    };
    // The function CPs the symbol's own `cp_designation` slots print, resolved as `slot_values` does: through the
    // representation's pin map, else (a symbol without CPs) slot CP `i` is function CP `i`. A slot that the pin map
    // sends to another function prints none of this function's CPs.
    let slotted: Vec<u16> = def
        .text_slots
        .iter()
        .filter_map(|s| match s.kind {
            SlotKind::CpDesignation(i) => match rep.pin_map.iter().find(|p| p.symbol_cp == i) {
                Some(p) => (p.function == function).then_some(p.idx),
                None => Some(i),
            },
            _ => None,
        })
        .collect();
    b.cps
        .iter()
        .filter_map(|cp| {
            let designation = designation(cp.idx).filter(|d| !d.is_empty() && !slotted.contains(&cp.idx));
            let address = addresses
                .iter()
                .find(|(idx, _)| *idx == cp.idx)
                .map(|(_, a)| a.as_str());
            let text = match (designation, address) {
                (Some(d), Some(a)) => format!("{d} {a}"),
                (Some(d), None) => d.to_owned(),
                (None, Some(a)) => a.to_owned(),
                (None, None) => return None,
            };
            Some(PinText {
                idx: cp.idx,
                text,
                cell: None,
            })
        })
        .collect()
}

/// The cells of a PLC card drawn in one row (doc 10 §3.6, "A card drawn in cells"): when at least two of the box's CPs
/// are bound to channels and all of those leave its upper edge, or all its lower edge, each of their pin texts gets its
/// channel's function text (doc 10 §3.4; empty when it has none) as its `cell`. Any other box keeps its pin texts.
fn card_cells(q: &Q, rep: &Representation, function: Id<Function>, pins: &mut [PinText]) {
    let Some(b) = &rep.box_ else {
        return;
    };
    let bound = q.get::<ew_derive::ChannelsByCp>(());
    let channels: Vec<(u16, Id<ew_schema::Channel>, &ew_schema::Dir4)> = b
        .cps
        .iter()
        .filter_map(|cp| Some((cp.idx, bound.at_cp(function, cp.idx).next()?, &cp.dir)))
        .collect();
    let one_edge = |d: &ew_schema::Dir4| channels.iter().all(|(_, _, dir)| *dir == d);
    if channels.len() < 2 || !(one_edge(&ew_schema::Dir4::Up) || one_edge(&ew_schema::Dir4::Down)) {
        return;
    }
    let table = q.table::<ew_schema::Channel>();
    for (idx, channel, _) in channels {
        let text = table
            .get(channel)
            .map(|c| q.get::<ew_derive::ChannelTexts>(c.module))
            .and_then(|texts| {
                texts
                    .iter()
                    .find(|(id, _)| *id == channel)
                    .and_then(|(_, t)| t.function_text.as_ref())
                    .map(crate::output::properties::neutral_or_first)
            })
            .unwrap_or_default();
        if let Some(pin) = pins.iter_mut().find(|p| p.idx == idx) {
            pin.cell = Some(text);
        }
    }
}

fn slot_values(q: &Q, page: Id<Page>, rep: &Representation, def: &SymbolDef) -> Vec<SlotValue> {
    let Some(&function) = rep.functions.first() else {
        return Vec::new();
    };
    let f = q.functions().get(function).cloned();
    let device = f.as_ref().and_then(|f| f.device);
    let dev = device.and_then(|d| q.devices().get(d).cloned());
    let label = dev
        .as_ref()
        .filter(|_| rep.rep_type != RepType::StripSegment)
        .and_then(|d| terminal_label(q, d, rep));
    let mut out = Vec::new();
    for (i, slot) in def.text_slots.iter().enumerate() {
        let mut item_links = Vec::new();
        let text = match &slot.kind {
            SlotKind::Tag => match (&f, device) {
                (Some(f), _) if matches!(f.kind, FunctionKind::Ip(_)) => None,
                // Doc 08 §6.1, §6.7: a terminal's own representation prints its label, never
                // the full tag.
                (_, Some(_)) if label.is_some() => label.as_ref().map(|(l, _)| l.clone()),
                // `tag_print` PerPage prints a strip's tag at its first segment on each page only.
                (_, Some(d)) if !prints_segment_tag(q, d, rep) => None,
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
            // name.
            SlotKind::FunctionText => match f.as_ref().map(|f| &f.kind) {
                Some(FunctionKind::Ip(ip)) => Some(ip.name.as_str().to_owned()),
                // The box of an interface with several sockets names it in its middle (doc 10 §2.7); a box of one
                // socket names it at the socket.
                _ if rep.box_.as_ref().is_some_and(|b| b.cps.len() > 1)
                    && port_box_label(q, rep, def).is_some() =>
                {
                    port_box_label(q, rep, def)
                }
                // A terminal labelled by its function text prints it once, in its tag slot.
                _ if label.as_ref().is_some_and(|(_, by_text)| *by_text) => None,
                // A slot with the fallback `process_tag` prints, when the function text is empty, the
                // process tag of the valve's tracked target, else the valve's own.
                _ => {
                    let own = dev.as_ref().map(|d| neutral(&d.function_text));
                    match (&own, &slot.fallback) {
                        (Some(t), Some(ew_schema::SlotFallback::ProcessTag)) if t.is_empty() => {
                            device.and_then(|d| ew_derive::tracked_process_tag(q, d)).or(own)
                        }
                        _ => own,
                    }
                }
            },
            SlotKind::ProcessTag => dev
                .as_ref()
                .and_then(|d| d.process_tag.as_ref().map(|t| t.as_str().to_owned())),
            SlotKind::TechData => device.map(|d| tech_data_text(q, d)),
            // Beside an `order_code` slot the manufacturer is the configured part's of that order code (the
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
                // The starter library's "Generic" names no maker, so the slot prints nothing for it
                // (the chooser and the tables keep the word; `manufacturer_name` is unchanged).
                .filter(|p| p.item.manufacturer.item.as_uuid() != ew_library::STARTER_GENERIC_MANUFACTURER)
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
            // The drawn function's bus port, its station address and its termination (FR-BUS-004).
            SlotKind::BusAddress => f
                .as_ref()
                .and_then(|f| f.bus_port.as_ref())
                .and_then(ew_derive::bus_address_text),
            SlotKind::BusTermination => f
                .as_ref()
                .and_then(|f| f.bus_port.as_ref())
                .and_then(ew_derive::bus_termination_text),
            // The first drawn function's derived PLC address text; a slot at one solenoid of a valve, that solenoid's
            // address alone.
            SlotKind::PlcAddress => rep.functions.iter().find_map(|f| {
                let texts = q.get::<PlcAddressText>(*f);
                match &slot.solenoid {
                    None => texts.text.clone(),
                    Some(s) => texts
                        .solenoids
                        .iter()
                        .find(|(d, _)| d == s.as_str())
                        .map(|(_, t)| t.clone()),
                }
            }),
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

/// The label a terminal's own representation `rep` prints in its tag slot (doc 08 §6.1, FR-TRM-021):
/// its function text when its strip's `label_mode` is FunctionText and the text is not empty (then `true`: the
/// function-text slot stays empty), else the designation of the level the representation draws (doc 08 §6.3: the
/// symbol of a block's second level prints that level's number); `None` for a device that is not a terminal.
fn terminal_label(q: &Q, dev: &ew_schema::Device, rep: &Representation) -> Option<(String, bool)> {
    let t = dev.aspects.terminal.as_ref()?;
    let by_text = q
        .devices()
        .get(t.strip)
        .and_then(|s| s.aspects.strip.as_ref())
        .is_some_and(|s| s.label_mode == LabelMode::FunctionText);
    let text = neutral(&dev.function_text);
    Some(if by_text && !text.is_empty() {
        (text, true)
    } else {
        let level = ew_derive::rep_level(q, rep);
        (t.level_designation(level).as_str().to_owned(), false)
    })
}

/// The (row, column) cell of a representation's anchor, the frame order of doc 08 §6.7; a representation outside
/// the frame grid sorts last.
fn cell_key(q: &Q, rep: Id<Representation>) -> (u32, u32) {
    q.get::<RepAnchorCell>(rep)
        .as_ref()
        .as_ref()
        .map_or((u32::MAX, u32::MAX), |c| (c.row, c.column))
}

/// Whether strip segment `rep` of device `device` prints the strip's tag (doc 08 §6.7): every segment
/// with `tag_print` PerSegment; with PerPage only the strip's first segment on its page in (row, column, ID)
/// order. A representation that is not a strip segment prints its tag.
fn prints_segment_tag(q: &Q, device: Id<ew_schema::Device>, rep: &Representation) -> bool {
    if rep.rep_type != RepType::StripSegment {
        return true;
    }
    let per_page = q
        .devices()
        .get(device)
        .and_then(|d| d.aspects.strip.as_ref())
        .is_some_and(|s| s.tag_print == TagPrint::PerPage);
    if !per_page {
        return true;
    }
    let reps = q.reps();
    let first = q
        .idx()
        .functions_of_device(device)
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
        .filter(|r| {
            reps.get(*r)
                .is_some_and(|r| r.rep_type == RepType::StripSegment && r.page == rep.page)
        })
        .min_by_key(|r| (cell_key(q, *r), *r));
    first == Some(rep.id)
}

/// `page_strip_tags(page)`: the strip tags of the terminals drawn alone on a drawn page (doc 08 §6.7), by
/// representation ID. A strip with a terminal representation on the page, no segment there and no text bound to its
/// name there (`strip.tag`, doc 06 §3.8) prints its displayed
/// tag for the page once, at the first terminal representation on the page whose tag slot is shown
/// (`ew_derive::tag_slot_shown`), in the strip's own order (designation in natural order, then terminal ID, as
/// `ew_derive::MainRep` takes a strip's terminal; then representation ID); a strip without such a terminal on the page
/// prints no tag there. So an unnumbered strip's terminals name it: its displayed tag for the page, with "?" for what
/// is not derived ("-X101?" under a strip rule with `{page}`, "-X??" without a page token).
pub fn page_strip_tags(q: &Q, page: Id<Page>) -> Vec<StripTagView> {
    let (reps, functions, devices, symbols) = (q.reps(), q.functions(), q.devices(), q.symbols());
    let mut segmented = std::collections::BTreeSet::new();
    let mut firsts = BTreeMap::new();
    for rid in q.idx().reps_by_page(page) {
        let Some(rep) = reps.get(rid) else { continue };
        let Some(d) = rep
            .functions
            .first()
            .and_then(|f| functions.get(*f))
            .and_then(|f| f.device)
        else {
            continue;
        };
        match rep.rep_type {
            RepType::StripSegment => {
                segmented.insert(d);
                continue;
            }
            RepType::ContactImage => continue,
            _ => {}
        }
        let Some(t) = devices.get(d).and_then(|d| d.aspects.terminal.as_ref()) else {
            continue;
        };
        let shown = symbols
            .get(rep.symbol)
            .and_then(|s| ew_derive::tag_slot_shown(rep, &s.item));
        if shown.is_none() {
            continue;
        }
        let key = (t.designation.natural_key(), d, rid);
        let first = firsts.entry(t.strip).or_insert_with(|| key.clone());
        if key < *first {
            *first = key;
        }
    }
    // A strip a text bound to its terminal names on the page (doc 06 §3.8) is named there by that text alone.
    let named = ew_derive::strips_named_on(q, page);
    let mut out: Vec<StripTagView> = firsts
        .into_iter()
        .filter(|(strip, _)| !segmented.contains(strip) && !named.contains(strip))
        .map(|(strip, (_, _, rep))| StripTagView {
            rep,
            text: (*q.get::<DisplayedTag>((strip, page))).clone(),
        })
        .filter(|t| !t.text.is_empty())
        .collect();
    out.sort_by_key(|t| t.rep);
    out
}

/// The bridges of the strips whose terminals the drawn page `page` shows, alone or in a strip segment (doc 08 §6.5,
/// FT-1908, FT-2020): every bridge group of such a strip (`ew_derive::StripBridges`, by strip ID, then group) with
/// the clamp its connections name on each of its terminals, the first in the group's connection order (a bridge shaft,
/// which no symbol draws, as the terminal's lowest side-1 clamp), in the group's terminal order. The scene draws a
/// group where its terminals stand in one row.
pub fn page_bridges(q: &Q, page: Id<Page>) -> Vec<BridgeView> {
    let (reps, functions, devices, connections) = (q.reps(), q.functions(), q.devices(), q.connections());
    // A terminal's strip, or the strip a segment draws (a segment is a representation of the strip's function).
    let strips: std::collections::BTreeSet<Id<Device>> = q
        .idx()
        .reps_by_page(page)
        .filter_map(|rid| reps.get(rid))
        .flat_map(|rep| rep.functions.iter())
        .filter_map(|f| functions.get(*f)?.device)
        .filter_map(|d| {
            let aspects = &devices.get(d)?.aspects;
            match (&aspects.terminal, &aspects.strip) {
                (Some(t), _) => Some(t.strip),
                (None, Some(_)) => Some(d),
                (None, None) => None,
            }
        })
        .collect();
    let mut out = Vec::new();
    for strip in strips {
        for group in q.get::<ew_derive::StripBridges>(strip).iter() {
            // The clamp of each terminal: the first end of the group's connections on it.
            let ends: Vec<_> = group
                .connections
                .iter()
                .filter_map(|id| connections.get(*id))
                .flat_map(|c| [&c.a, &c.b])
                .filter_map(|e| match e {
                    ew_schema::Endpoint::Cp(f, idx) => Some((functions.get(*f)?.device?, (*f, *idx))),
                    _ => None,
                })
                .collect();
            // A bridge on the bridge shaft, which no symbol draws, is drawn on the lead of the terminal's lowest side-1
            // connection point, where `strip.bridge` bridges a terminal without a shaft.
            let drawn = |(f, idx): (Id<Function>, u16)| {
                let cps = q.get::<FunctionCps>(f);
                match cps.iter().find(|c| c.idx == idx) {
                    Some(c) if c.side.is_none() => cps
                        .iter()
                        .filter(|c| c.side == Some(Side::Side1))
                        .map(|c| c.idx)
                        .min()
                        .map_or((f, idx), |side1| (f, side1)),
                    _ => (f, idx),
                }
            };
            let clamps: Vec<(Id<Function>, u16)> = group
                .terminals
                .iter()
                .filter_map(|t| ends.iter().find(|(d, _)| d == t).map(|(_, cp)| drawn(*cp)))
                .collect();
            if clamps.len() >= 2 {
                out.push(BridgeView { clamps });
            }
        }
    }
    out
}

/// The three tables of a `profile_tables` block as printed texts.
fn cover_tables(q: &Q, block: &ReportBlock) -> CoverTables {
    let set = q.get::<BlockRows>(block.id);
    let opts = RenderOptions {
        lang: ew_derive::print_language(q),
        ..RenderOptions::default()
    };
    let cell = |row: &Row, key: &str| -> String {
        set.column(key)
            .and_then(|i| row.cells.get(i))
            .map(|c| render(q, c, &opts))
            .unwrap_or_default()
    };
    let label = |key: &str| -> String {
        set.column(key)
            .map(|i| ew_derive::text_in(&set.columns[i].label, opts.lang.as_ref()).to_owned())
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

/// The boxed table of an embedded block other than the cover, as printed texts on `page` (see the module
/// documentation).
pub(crate) fn boxed_table(q: &Q, block: &ReportBlock, page: Id<Page>) -> EmbeddedTable {
    boxed_table_of(q, block, &q.get::<BlockRows>(block.id), page)
}

/// [`boxed_table`] of `block` with the rows `set` (a block the project does not hold yet: the legend Add legend would
/// make, whose fit it tries).
pub(crate) fn boxed_table_of(q: &Q, block: &ReportBlock, set: &RowSet, page: Id<Page>) -> EmbeddedTable {
    let layout = match &block.source {
        ReportSource::Kind(kind) => Registry::builtin().get(kind).map(|k| k.embedded_layout()),
        _ => None,
    }
    .unwrap_or_else(|| EmbeddedLayout::every_column(&set.columns));
    // A block's own printed columns replace the kind's (Add legend's description, doc 09 §2.2).
    let layout = if block.columns.is_empty() {
        layout
    } else {
        EmbeddedLayout {
            columns: block.columns.iter().map(|c| (c.key.clone(), c.weight)).collect(),
            ..layout
        }
    };
    let opts = RenderOptions {
        page: Some(PageRef::Drawn(page)),
        lang: ew_derive::print_language(q),
        ..RenderOptions::default()
    };
    let in_lang = |m: &MlText| ew_derive::text_in(m, opts.lang.as_ref()).to_owned();
    // The printed columns: the layout's keys that the row set has, in the layout's order.
    let printed: Vec<(usize, u32)> = layout
        .columns
        .iter()
        .filter_map(|(key, weight)| set.column(key).map(|i| (i, *weight)))
        .collect();
    let columns = printed
        .iter()
        .map(|&(i, weight)| TableColumn {
            header: in_lang(&set.columns[i].label),
            weight,
        })
        .collect();
    let title = if block.title.is_empty() {
        in_lang(&layout.title)
    } else {
        in_lang(&block.title)
    };
    let note = match &set.status {
        RowSetStatus::NotDerived(reason) => Some(in_lang(&reason.words())),
        RowSetStatus::Derived => None,
    };
    let rows = if note.is_some() {
        Vec::new()
    } else {
        set.rows
            .iter()
            .map(|r| {
                let kind = match r.kind {
                    RowKind::Data => TableRowKind::Data,
                    RowKind::Heading | RowKind::GroupHeader | RowKind::GroupFooter => TableRowKind::Heading,
                };
                let cells = match kind {
                    // A heading prints its first cell across the row.
                    TableRowKind::Heading => {
                        r.cells.first().map(|c| render(q, c, &opts)).into_iter().collect()
                    }
                    TableRowKind::Data => printed
                        .iter()
                        .map(|&(i, _)| r.cells.get(i).map(|c| render(q, c, &opts)).unwrap_or_default())
                        .collect(),
                };
                // An embedded block draws no back-links: only the connection-point links
                // of a PLC overview box as built (FR-PLC-052), whole-cell, renumbered to their printed position.
                let links = row_links(q, r)
                    .into_iter()
                    .filter(|l| matches!(l.dest, Destination::Cp(_)) && l.span.is_none())
                    .filter_map(|l| {
                        let p = printed.iter().position(|&(i, _)| i == l.column)?;
                        Some(TableLink { column: p, ..l })
                    })
                    .collect();
                TableRow { kind, cells, links }
            })
            .collect()
    };
    // A block with its own printed columns (Add legend's, doc 09 §2.2) is a lookup table (doc 12 §5.4): a column of
    // text (the description) is prose and wraps; of the identifiers the position and the tag give way last, every
    // other first.
    let lookup = (!block.columns.is_empty()).then(|| {
        let rank = |i: usize| match set.columns[i].key.as_str() {
            "pos" => 2,
            "tag" => 1,
            _ => 0,
        };
        let mut order: Vec<usize> = (0..printed.len()).collect();
        order.sort_by_key(|&c| rank(printed[c].0));
        let prose = (0..printed.len())
            .filter(|&c| set.columns[printed[c].0].ty == ew_reports::CellType::Text)
            .collect();
        Lookup { order, prose }
    });
    EmbeddedTable {
        title,
        columns,
        headers: layout.headers,
        flow_columns: layout.flow_columns,
        rows,
        note,
        lookup,
    }
}

/// The drawn row links of a report row on a generated page (FR-REP-042), in the
/// row's link order:
///
/// - `Cp`: the connection point, which the PLC overview sets to the coil of each bound channel (doc 10 §5);
/// - `Page(p)`: the page, when the page order holds it;
/// - `MainRep(d)`: the device's main representation, when it has one;
/// - `Rep(r)`: the representation, when it exists;
/// - `Placement(p)`: the device's footprint on the first panel page that shows its surface; the row is then also where
///   Follow reference from the device lands (`Destination::LegendRow`), on its first printed cell without a link of its own
///   (doc 12 §5.4).
///
/// A link with an `item` covers that page's printed span of a `Folios` cell ([`folios_printed`], the function the
/// printer uses); a page inside a printed run has no span and no link. A link that would not resolve is not drawn:
/// never a dead link.
fn row_links(q: &Q, r: &Row) -> Vec<TableLink> {
    let order = q.get::<PageOrder>(());
    let reps = q.reps();
    let rep = |rid: Id<Representation>| {
        reps.get(rid)
            .filter(|x| order.position(PageRef::Drawn(x.page)).is_some())
            .map(|_| Destination::Representation(rid.as_uuid()))
    };
    let mut spans: BTreeMap<usize, Vec<Option<std::ops::Range<usize>>>> = BTreeMap::new();
    r.links
        .iter()
        .filter_map(|l| {
            let column = usize::from(l.column);
            let dest = match l.target {
                LinkTarget::Cp { function, idx } => {
                    let cp: Id<()> = ew_ids::v5::cp_id(function, u32::from(idx));
                    Some(Destination::Cp(cp.as_uuid()))
                }
                LinkTarget::Page(p) => order.position(p).map(|_| page_destination(p)),
                LinkTarget::MainRep(d) => (*q.get::<MainRep>(d)).and_then(rep),
                LinkTarget::Rep(rid) => rep(rid),
                LinkTarget::Placement(p) => super::panel::placement_page(q, p)
                    .filter(|page| order.position(PageRef::Drawn(*page)).is_some())
                    .map(|_| Destination::Placement(p.as_uuid())),
            }?;
            let span = match l.item {
                None => None,
                Some(n) => {
                    let items = spans.entry(column).or_insert_with(|| match r.cells.get(column) {
                        Some(Cell::Folios(ps)) => folios_printed(q, ps).1,
                        _ => Vec::new(),
                    });
                    Some(items.get(usize::from(n))?.clone()?)
                }
            };
            Some(TableLink {
                column,
                span,
                dest,
                target: false,
            })
        })
        .collect()
}

/// The printed table of a generated page (see the module documentation).
fn table_view(q: &Q, g: &GeneratedPage, page: PageRef) -> TableView {
    let set = q.get::<BlockRows>(g.block);
    let lang = ew_derive::print_language(q);
    let in_lang = |m: &MlText| ew_derive::text_in(m, lang.as_ref()).to_owned();
    let opts = RenderOptions {
        page: Some(page),
        lang: lang.clone(),
        ..RenderOptions::default()
    };
    let block = q.report_blocks().get(g.block);
    // The printed columns (doc 09 §3.8): the block's own, those its rows have, in its order, each with its width and
    // its heading or the kind's; else every column of the rows.
    let printed: Vec<(usize, u32, &MlText)> = match block.filter(|b| !b.columns.is_empty()) {
        Some(b) => b
            .columns
            .iter()
            .filter_map(|c| {
                let i = set.column(&c.key)?;
                let heading = if c.heading.is_empty() {
                    &set.columns[i].label
                } else {
                    &c.heading
                };
                Some((i, c.weight, heading))
            })
            .collect(),
        None => set
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.weight, &c.label))
            .collect(),
    };
    let columns = printed
        .iter()
        .map(|&(_, weight, heading)| TableColumn {
            header: in_lang(heading),
            weight,
        })
        .collect();
    let line = |m: Option<&MlText>| m.filter(|m| !m.is_empty()).map(in_lang);
    let caption = line(block.map(|b| &b.caption));
    let legend = line(block.map(|b| &b.legend));
    if let Some(note) = &g.note {
        return TableView {
            columns,
            rows: Vec::new(),
            note: Some(in_lang(note)),
            caption,
            legend,
        };
    }
    let end = (g.rows.end as usize).min(set.rows.len());
    let start = (g.rows.start as usize).min(end);
    let overview =
        block.is_some_and(|b| matches!(b.source, ReportSource::Kind(ReportKind::RevisionOverview)));
    let pages_column = set.column("pages").filter(|_| overview);
    let cell = |r: &Row, i: usize| match pages_column
        .filter(|p| *p == i)
        .and_then(|_| revision_pages(q, r))
    {
        Some(pages) => render(q, &pages, &opts),
        None => r.cells.get(i).map(|c| render(q, c, &opts)).unwrap_or_default(),
    };
    let rows = set.rows[start..end]
        .iter()
        .map(|r| {
            let kind = match r.kind {
                RowKind::Data => TableRowKind::Data,
                RowKind::Heading | RowKind::GroupHeader | RowKind::GroupFooter => TableRowKind::Heading,
            };
            let cells = match kind {
                // A heading prints its first cell across the row.
                TableRowKind::Heading => vec![cell(r, 0)],
                TableRowKind::Data => printed.iter().map(|&(i, _, _)| cell(r, i)).collect(),
            };
            // The links of the printed cells, at their printed column.
            let mut links: Vec<TableLink> = row_links(q, r)
                .into_iter()
                .filter_map(|l| {
                    let column = printed.iter().position(|&(i, _, _)| i == l.column)?;
                    Some(TableLink { column, ..l })
                })
                .collect();
            // A row that links to a placement is where Follow reference from that device lands (doc 12 §5.4): its first
            // printed cell without a link of its own.
            let placement = r.links.iter().find_map(|l| match l.target {
                LinkTarget::Placement(p) => Some(p),
                _ => None,
            });
            let free = (0..printed.len()).find(|c| !links.iter().any(|l| l.column == *c && l.span.is_none()));
            if let (Some(p), Some(column)) = (placement, free) {
                links.push(TableLink {
                    column,
                    span: None,
                    dest: Destination::LegendRow(p.as_uuid()),
                    target: true,
                });
            }
            TableRow { kind, cells, links }
        })
        .collect();
    TableView {
        columns,
        rows,
        note: None,
        caption,
        legend,
    }
}

/// The `pages` cell of a revision overview row at draw time: the revision's `changed_pages`, which
/// sees the page digests and the generated pages the row source inside the pagination cannot
/// (`ew_reports::kinds::revision_rows`); `None` for a row that names no revision.
pub(super) fn revision_pages(q: &Q, row: &Row) -> Option<Cell> {
    let id = row.id.entities().next()?;
    let rev = q.table::<ew_schema::Revision>().get(Id::from_uuid(id))?;
    Some(Cell::Folios(super::revisions::changed_pages(q, rev)))
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
    /// How many connections carry a wire marker of their own under the project's current marker mode: the
    /// count that `project.marker_mode` reports after a switch (`0 wires carry a marker`).
    pub fn marker_wire_count(&self) -> usize {
        let q = self.query();
        let mode = ew_derive::marker_mode(&q);
        q.connections()
            .iter()
            .filter(|(_, c)| ew_derive::shows_marker(&q, c, mode))
            .count()
    }

    /// The frame form a new page of type `kind` made from the page `from` takes as its own (FT-1716): `from`'s own
    /// form, when the project's frame defaults give a page of `kind` none and the project holds no no-grid frame, so
    /// that it would have no frame at all (an imported QElectroTech project, whose folios each carry their template).
    /// `None` when the defaults or the no-grid frame frame the page, or `from` has no form of its own.
    pub fn new_page_form(&self, from: Uuid, kind: &str) -> Option<Uuid> {
        let q = self.query();
        let kind: PageType = serde_json::from_value(serde_json::Value::String(kind.to_owned())).ok()?;
        if frame_form_of(&q, None, &kind, true).is_some() || form_by_key(&q, NOGRID_FRAME).is_some() {
            return None;
        }
        let own = q.pages().get(Id::from_uuid(from))?.form?;
        q.forms().get(own).is_some().then(|| own.as_uuid())
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
        for (json, (key, seal)) in [JUNCTION_DOT, JUNCTION_BEVEL]
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
                "{key}: the compiled-in symbol changed; turn its seal here and say so in the release notes"
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

    /// A black box's pin texts carry the CP designation, then (when the CP is bound) the printed address
    /// after one space; repeated designations print as stored; an empty designation, or one a `cp_designation` slot
    /// of the symbol already prints, is left out, and a pin with nothing left has no entry.
    #[test]
    fn pin_texts_carry_the_designation_then_the_address_and_skip_a_slotted_designation() {
        const BLACKBOX: &str = include_str!("../../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
        let Ok(LibDocument::Symbol(copy)) = LibDocument::from_json(BLACKBOX) else {
            panic!("the black box is a symbol copy");
        };
        let mut def = copy.item;
        assert!(def.cps.is_empty(), "a black-box symbol has no CPs");
        let function = Id::<Function>::from_uuid(Uuid::from_u128(0x21));
        let rep: Representation = serde_json::from_value(serde_json::json!({
            "id": Uuid::from_u128(0x11).hyphenated().to_string(),
            "functions": [function.as_uuid().hyphenated().to_string()],
            "page": Uuid::from_u128(0x100).hyphenated().to_string(),
            "symbol": copy.id.as_uuid().hyphenated().to_string(),
            "pos": [0, 0],
            "box": {
                "outline": [[0, 0], [10_000_000, 0], [10_000_000, 10_000_000], [0, 10_000_000]],
                "cps": (0..5u16)
                    .map(|i| serde_json::json!({"idx": i, "pos": [i64::from(i) * 2_000_000, 0], "dir": "up"}))
                    .collect::<Vec<_>>(),
            },
        }))
        .expect("a representation");
        let designations = ["L+", "+", "+", "", "13"];
        let designation = |idx: u16| designations.get(usize::from(idx)).copied();
        let addresses = vec![(1, "I0.1".to_owned()), (3, "I0.3".to_owned())];
        let texts = |def: &SymbolDef| -> Vec<(u16, String)> {
            compose_pin_texts(&rep, def, function, designation, &addresses)
                .into_iter()
                .map(|p| (p.idx, p.text))
                .collect()
        };
        let own = |v: &[(u16, &str)]| -> Vec<(u16, String)> {
            v.iter().map(|(i, t)| (*i, (*t).to_owned())).collect()
        };
        assert_eq!(
            texts(&def),
            own(&[(0, "L+"), (1, "+ I0.1"), (2, "+"), (3, "I0.3"), (4, "13")]),
            "designation then address; repeated designations as stored; an empty one leaves the address alone"
        );
        // A `cp_designation` slot for CP 4 and one for CP 1: those designations print in the slot, not at the pin.
        let mut slot = def.text_slots[0].clone();
        slot.kind = SlotKind::CpDesignation(4);
        def.text_slots.push(slot.clone());
        slot.kind = SlotKind::CpDesignation(1);
        def.text_slots.push(slot);
        assert_eq!(
            texts(&def),
            own(&[(0, "L+"), (1, "I0.1"), (2, "+"), (3, "I0.3")]),
            "a slotted designation is left out; CP 4 has nothing left to print"
        );
    }
}
