// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test glue (WP-S3.04): the golden model loaded through the CSV bootstrap, [`PageView`]s filled from the
//! tracked read context `Q` (`page_order`, `title_block(page)`, `displayed_tag`, `function_cps`), the
//! bundled fonts as a [`FontResolver`] and the PDF page list.
//!
//! `ew-scene` has no `ew-derive` edge (ARCH §3), so this is where derived values meet the scene. It is test
//! code until the export job of the session facade exists; the mapping rules are the ones a production
//! implementation must follow:
//!
//! - page order: `page_order` of a `Q` with the report engine installed (`Session::query()`), so the
//!   generated report pages take part in Prev/Next, the outline and `/PageLabels` (ARCH §6.4);
//! - page IDs: a drawn page's ID, a divider's section ID, a generated page's v5 ID `v5(block, index)`
//!   (`GeneratedPage::id`); every PDF destination is unique;
//! - frame form: `Page.form`, else the no-grid frame (dividers); a generated page's `GeneratedPage::form`;
//!   the title block by `FormDef.title_block`;
//! - report tables (FT-53): a generated page's rows are the `GeneratedPage::rows` range of
//!   `report_rows(block)` (`BlockRows`, read after the page order exists), each cell printed by
//!   `ew_reports::render::render` for that page (neutral language, displayed tags); a page with a `note`
//!   prints the note in one row instead; the column widths are the columns' weights;
//! - bindings: every `title_block(page)` value, the project's date format, no display language (neutral);
//! - embedded blocks (FR-REP-028): a drawn page's `profile_tables` block is drawn into the `tables` layer in its
//!   rectangle: the rows of `BlockRows` split by their `table` column into the colour table, the control sizes
//!   and the mains sizes; a group-header row is the title of its table, a group-footer row of the colours its
//!   caption (the standards text) and the group-footer row of the group `note` the note under the tables; each cell
//!   printed by `render` (neutral language). PART DATA blocks are not drawn yet;
//! - slot values: `tag` → `displayed_tag(device, page)`, except that a terminal's own representation prints
//!   only the terminal's designation (ID-162, doc 08 §6.7; the strip tag appears once per strip
//!   representation) and an interruption point has no `tag` slot; `cp_designation(i)` → the designation of
//!   the function CP that symbol CP `i` draws, `function_text` → the device's function text, or an
//!   interruption point's pairing name, `process_tag` → the device's process tag; `xref` →
//!   `xref_text(rep)` with one link per listed item to that item's target representation (FT-86, ID-228);
//!   an external point prints the external target without a link; `tech_data` → `tech_data_text(device)` (the
//!   typed ratings of the device and its part, "C10", "40 A", "15 °C"), `manufacturer`, `part_number` and
//!   `type_designation` → the main part of the device (its first part use, FT-162; D-18, D-33); every other kind
//!   has no value;
//! - wire markers: connections with `overrides.show_marker`, printing the explicit cross-section in mm²
//!   and the colour codes, joined by a space (provisional format);
//! - junction and wire-marker symbols: the built-in `@qet10` symbols of `content/symbols`;
//! - strip segments (doc 08 §6.7, FR-TRM-011): the terminals of the strip device (`terminals_of_strip`,
//!   in `(Terminal.seq, ID)` order) are split over the strip's segment representations, taken in
//!   `(page-order position, pos.y, pos.x, ID)` order, as contiguous runs: segment `j` of `m` draws the
//!   terminals `⌊j·N/m⌋ … ⌊(j+1)·N/m⌋ − 1` of `N`. Each drawn terminal is its device's main function
//!   (else its first function) with the CPs of `function_cps` on side 1 and side 2; CPs without a side
//!   (shaft, rail foot) are not drawn. `ew-scene` places them (`page` module documentation).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ew_api::{Session, SessionOptions};
use ew_derive::{
    DisplayedTag, FunctionCps, GeneratedPage, PageEntry, PageOrder, PageOrderV, PageRef, PageTitle, Q,
    TbValue, TitleBlockValues, XrefTextOf, main_part, manufacturer_name, tech_data_text,
};
use ew_dl::{Destination, FontKey};
use ew_ids::Id;
use ew_import_tables::ew_interop_api::ImportInput;
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use ew_render_cpu::FontBytes;
use ew_reports::render::{RenderOptions, render};
use ew_reports::{BlockRows, Row, RowKind};
use ew_scene::embedded::{CoverTables, EmbeddedContent, EmbeddedView};
use ew_scene::forms::{AssetRef, BoundValue, PageBindings};
use ew_scene::page::{
    ConnectionView, FrameView, ItemLink, JunctionView, PageKind, PageView, RepView, SegmentTerminal,
    SlotValue, SymbolRef,
};
use ew_scene::table::{TableColumn, TableRow, TableRowKind, TableView};
use ew_schema::{
    Csa, FnRole, FormDef, Function, FunctionKind, JunctionStyle, LibCopy, LibDocument, Page, PageType,
    Record, RepType, ReportBlock, ReportKind, ReportSource, Representation, Side, SlotKind, SymbolDef,
};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    paths.sort();
    paths
}

fn library_docs() -> Vec<LibDocument> {
    let mut docs = Vec::new();
    for sub in ["symbols/qet10", "fndefs", "forms"] {
        for p in json_files(&repo().join("content").join(sub), "json") {
            docs.push(LibDocument::from_json(&std::fs::read_to_string(&p).unwrap()).unwrap());
        }
    }
    docs
}

/// The built-in symbol with `key` from `content/symbols/qet10`.
pub fn builtin_symbol(key: &str) -> Option<SymbolRef> {
    library_docs().into_iter().find_map(|d| match d {
        LibDocument::Symbol(LibCopy { id, item, .. })
            if item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) =>
        {
            Some(SymbolRef {
                id,
                def: Arc::new(item),
            })
        }
        _ => None,
    })
}

/// The golden session; with `shuffle`, the records are handed over in a permuted order (AC-14).
pub fn golden_session(shuffle: Option<u64>) -> Session {
    let inputs: Vec<ImportInput> = json_files(&repo().join("golden/J-E-2416/tables"), "csv")
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            ImportInput::new(&name, std::fs::read(p).unwrap()).unwrap()
        })
        .collect();
    let batch = TablesImporter::new(BuiltinLibrary::new(library_docs()).unwrap())
        .import(&inputs, &bootstrap_context())
        .expect("the golden tables import");
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    if let Some(seed) = shuffle {
        // A deterministic Fisher–Yates permutation (xorshift64).
        let mut s = seed | 1;
        for i in (1..records.len()).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            records.swap(i, (s % (i as u64 + 1)) as usize);
        }
    }
    Session::from_records(
        project.expect("the batch holds the project"),
        records,
        batch.lib.clone(),
        SessionOptions::default(),
    )
    .expect("the golden batch passes the model's integrity check")
}

/// The bundled fonts by key.
pub fn fonts() -> BTreeMap<FontKey, FontBytes> {
    let set = ew_text::bundled();
    set.faces()
        .filter_map(|f| {
            let data = set.face_data(f.key)?;
            Some((
                f.key,
                FontBytes {
                    data: Arc::from(data),
                    index: u32::from(f.key.face_index),
                },
            ))
        })
        .collect()
}

/// One page of the page order with its view and its outline texts.
pub struct GoldenPage {
    /// The page-order entry.
    pub entry: PageEntry,
    /// The view.
    pub view: PageView,
    /// "101 Power Supply 1 - Main Current".
    pub outline_title: String,
    /// "Section 1 - Wiring Definition"; `None` for front matter.
    pub section_title: Option<String>,
}

fn form_by_key(q: &Q, key: &str) -> Option<Arc<FormDef>> {
    q.forms()
        .iter()
        .find(|(_, f)| f.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key))
        .map(|(_, f)| Arc::new(f.item.clone()))
}

fn form(q: &Q, id: Id<FormDef>) -> Option<Arc<FormDef>> {
    q.forms().get(id).map(|f| Arc::new(f.item.clone()))
}

fn bindings(q: &Q, page: PageRef) -> PageBindings {
    let mut b = PageBindings::new();
    b.date_format = q.project().settings.formatting.date.clone();
    for (path, value) in &q.get::<TitleBlockValues>(page).values {
        let v = match value {
            TbValue::Text(t) => BoundValue::Text(t.clone()),
            TbValue::Ml(m) => BoundValue::Ml(m.clone()),
            TbValue::Date(d) => BoundValue::Date(*d),
            TbValue::Tag { full, displayed } => BoundValue::Tag {
                full: full.clone(),
                displayed: displayed.clone(),
            },
            TbValue::Asset(h) => BoundValue::Asset(AssetRef::new(h.0, ew_dl::ImageFormat::Png)),
        };
        b.set(path, v)
            .expect("title-block paths are valid placeholder paths");
    }
    b
}

fn frame_view(q: &Q, form: Option<Arc<FormDef>>, page: PageRef) -> Option<FrameView> {
    let form = form?;
    let title_block = form.title_block.and_then(|id| self::form(q, id));
    Some(FrameView {
        form,
        title_block,
        bindings: bindings(q, page),
    })
}

fn neutral(m: &ew_mltext::MlText) -> String {
    ew_scene::forms::ml_text(m, None).to_owned()
}

fn csa_text(c: &Csa) -> Option<String> {
    match c {
        Csa::Mm2(d) => Some(d.to_string()),
        Csa::Awg(n) => Some(format!("AWG{n}")),
        Csa::Kcmil(d) => Some(format!("{d} kcmil")),
        Csa::Unknown(_) => None,
    }
}

fn slot_values(q: &Q, page: Id<Page>, rep: &ew_schema::Representation, def: &SymbolDef) -> Vec<SlotValue> {
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
            // F2: interruption points have `function_text` and `xref` slots, no `tag`; the former prints the
            // pairing name.
            SlotKind::FunctionText => match f.as_ref().map(|f| &f.kind) {
                Some(FunctionKind::Ip(ip)) => Some(ip.name.as_str().to_owned()),
                _ => dev.as_ref().map(|d| neutral(&d.function_text)),
            },
            SlotKind::ProcessTag => dev
                .as_ref()
                .and_then(|d| d.process_tag.as_ref().map(|t| t.as_str().to_owned())),
            SlotKind::TechData => device.map(|d| tech_data_text(q, d)),
            SlotKind::Manufacturer => device
                .and_then(|d| main_part(q, d))
                .map(|p| manufacturer_name(&p.item.manufacturer)),
            SlotKind::PartNumber => device
                .and_then(|d| main_part(q, d))
                .map(|p| p.item.part_number.as_str().to_owned()),
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

/// The embedded blocks of a drawn page that the scene draws: the `profile_tables` blocks.
fn embedded_views(q: &Q, page: Id<Page>) -> Vec<EmbeddedView> {
    q.report_blocks()
        .iter()
        .filter_map(|(_, b)| {
            let embed = b.embedded.as_ref().filter(|e| e.page == page)?;
            matches!(b.source, ReportSource::Kind(ReportKind::ProfileTables)).then(|| EmbeddedView {
                rect: embed.rect.0,
                content: EmbeddedContent::Cover(cover_tables(q, b)),
            })
        })
        .collect()
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

/// Every page of the page order as a view.
pub fn golden_pages(q: &Q) -> Vec<GoldenPage> {
    let nogrid = form_by_key(q, "ew.frame.qet-a4-landscape-nogrid");
    let dot = builtin_symbol("ew.gen.junction-dot@qet10");
    let bevel = builtin_symbol("ew.gen.junction-bevel@qet10");
    let marker = builtin_symbol("ew.gen.wire-marker@qet10");
    let order = q.get::<PageOrder>(());
    let mut out = Vec::new();
    for entry in &order.entries {
        let title = neutral(&q.get::<PageTitle>(entry.page));
        let section_title = entry.section.and_then(|s| {
            q.sections()
                .get(s)
                .map(|s| format!("Section {} - {}", s.number, neutral(&s.title)))
        });
        let mut view = PageView {
            page: Id::from_uuid(uuid_of(entry.page)),
            kind: if entry.kind == PageType::PanelLayout {
                PageKind::Panel
            } else {
                PageKind::Schematic
            },
            grid: match entry.page {
                PageRef::Drawn(p) => q.pages().get(p).map_or(ew_units::Grid::MM4, |pg| pg.grid),
                _ => ew_units::Grid::MM4,
            },
            language: None,
            frame: None,
            reps: Vec::new(),
            connections: Vec::new(),
            junctions: Vec::new(),
            wire_marker: marker.clone(),
            table: None,
            embedded: Vec::new(),
        };
        match entry.page {
            PageRef::Drawn(p) => {
                let page = q.pages().get(p).cloned();
                let f = page.as_ref().and_then(|pg| pg.form).and_then(|id| form(q, id));
                let f = f.or_else(|| nogrid.clone());
                view.frame = frame_view(q, f, entry.page);
                view.embedded = embedded_views(q, p);
                for rid in q.idx().reps_by_page(p) {
                    let Some(rep) = q.reps().get(rid).cloned() else {
                        continue;
                    };
                    let symbol = q.symbols().get(rep.symbol).map(|s| Arc::new(s.item.clone()));
                    let slots = symbol
                        .as_ref()
                        .map(|d| slot_values(q, p, &rep, d))
                        .unwrap_or_default();
                    let terminals = segment_terminals(q, &order, &rep);
                    view.reps.push(RepView {
                        rep,
                        symbol,
                        slots,
                        terminals,
                    });
                }
                for cid in q.idx().connections_by_page(p) {
                    let Some(c) = q.connections().get(cid).cloned() else {
                        continue;
                    };
                    let marker = c.overrides.show_marker.then(|| {
                        let mut parts = Vec::new();
                        if let Some(t) = c.overrides.csa.as_ref().and_then(csa_text) {
                            parts.push(t);
                        }
                        // The colour's file spelling ("BU", "GNYE"), as the tables write it.
                        if let Some(col) = c
                            .overrides
                            .colour
                            .as_ref()
                            .and_then(|col| serde_json::to_value(col).ok()?.as_str().map(str::to_owned))
                        {
                            parts.push(col);
                        }
                        parts.join(" ")
                    });
                    view.connections.push(ConnectionView {
                        connection: c,
                        marker,
                    });
                }
                for jid in q.idx().junctions_by_page(p) {
                    let Some(j) = q.junctions().get(jid).cloned() else {
                        continue;
                    };
                    let symbol = match j.style {
                        JunctionStyle::Dot => dot.clone(),
                        JunctionStyle::Bevel(_) => bevel.clone(),
                        JunctionStyle::Unknown(_) => None,
                    };
                    view.junctions.push(JunctionView { junction: j, symbol });
                }
            }
            PageRef::Generated { .. } => {
                let g = order.generated(entry.page);
                let f = g.and_then(|g| form(q, g.form));
                view.frame = frame_view(q, f.or_else(|| nogrid.clone()), entry.page);
                view.table = g.map(|g| table_view(q, g, entry.page));
            }
            PageRef::Divider(_) => {
                view.frame = frame_view(q, nogrid.clone(), entry.page);
            }
        }
        out.push(GoldenPage {
            outline_title: format!("{} {title}", entry.folio),
            entry: entry.clone(),
            view,
            section_title,
        });
    }
    out
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
        })
        .collect();
    TableView {
        columns,
        rows,
        note: None,
    }
}

/// The stable UUID of a page of the page order: the page's, the section's for its divider, and for a
/// generated page its v5 ID `v5(block, index)` (ADR-0004, `GeneratedPage::id`), so 2 and 2.1 or 901 and
/// 901.1 have distinct destinations.
pub fn uuid_of(page: PageRef) -> ew_ids::Uuid {
    match page {
        PageRef::Drawn(p) => p.as_uuid(),
        PageRef::Divider(s) => s.as_uuid(),
        PageRef::Generated { block, index } => ew_ids::v5::report_page_id::<_, ()>(block, index).as_uuid(),
    }
}

/// The terminals a strip-segment representation draws (see the module documentation); empty for any
/// other representation.
fn segment_terminals(q: &Q, order: &PageOrderV, rep: &Representation) -> Vec<SegmentTerminal> {
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

/// The page's own destination.
pub fn destination(page: PageRef) -> Destination {
    Destination::Page(uuid_of(page))
}
