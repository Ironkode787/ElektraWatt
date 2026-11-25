// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The glue rules of the page view (doc 09 §4.1): the parts
//! of an [`ew_scene::page::PageView`] filled from the tracked read context `Q`.
//!
//! A page is drawn as a pure function of a view (ARCH §3.2, the seam of a page's view): the session layer, which holds
//! `Q`, fills the view, and `ew-scene` draws it. Each part is its own
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
//!   no-grid frame [`NOGRID_FRAME`](ew_schema::starter::NOGRID_FRAME) (the form of the project library with that key); a generated page's
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

use ew_derive::{
    DisplayedTag, FunctionCps, GeneratedPage, MainRep, PageOrder, PageRef, PlcAddressText, Policy, Q, Query,
    RepAnchorCell, TbValue, TitleBlockValues, XrefTextOf, box_caption, connection_marker, connection_number,
    frame_form_of, free_text, group_marker, grouped_connections, main_part, manufacturer_name,
    order_code_part, order_code_text, tech_data_text,
};
use ew_dl::Destination;
use ew_ids::{self, Id, Uuid};
use ew_mltext::MlText;
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
use ew_units::{Grid, Nm};

mod connections;
mod items;
mod reps;
mod tables;

pub use connections::*;
pub use items::*;
pub use reps::*;
pub use tables::*;

/// The shipped frame and title-block forms of `content/forms` (the QElectroTech-compatible A4 landscape frames with
/// and without the grid, the cover frame and the title block they name; the A3 landscape frame with the grid and its
/// title block, doc 09 §3.8), in the `lib/` JSON form that `library.embed` takes, in key order: the starter library's
/// forms (`ew_library::starter::FORMS`). A new empty project holds them and names the A4 ones in its frame defaults
/// (`crate::shipped_frames`); the wizard embeds them into a template.
pub static SHIPPED_FORMS: std::sync::LazyLock<Vec<&'static str>> =
    std::sync::LazyLock::new(|| ew_library::starter::FORMS.iter().map(|(_, json)| *json).collect());

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
                    .is_some_and(|k| k.as_str() == ew_schema::starter::NOGRID_FRAME)
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
        .or_else(|| form_by_key(q, ew_schema::starter::NOGRID_FRAME))
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

    /// The frame form a new page of type `kind` made from the page `from` takes as its own: `from`'s own
    /// form, when the project's frame defaults give a page of `kind` none and the project holds no no-grid frame, so
    /// that it would have no frame at all (an imported QElectroTech project, whose folios each carry their template).
    /// `None` when the defaults or the no-grid frame frame the page, or `from` has no form of its own.
    pub fn new_page_form(&self, from: Uuid, kind: &str) -> Option<Uuid> {
        let q = self.query();
        let kind: PageType = serde_json::from_value(serde_json::Value::String(kind.to_owned())).ok()?;
        if frame_form_of(&q, None, &kind, true).is_some()
            || form_by_key(&q, ew_schema::starter::NOGRID_FRAME).is_some()
        {
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
