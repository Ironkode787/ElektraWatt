// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window reads of a page's frame while it is changed on the page (doc 09 §3.9): the frame and title-block
//! copies with their items, whether each is the project's own, the pages that print each, where the scene puts the
//! title block's cells and lines and the rulers on the sheet, each cell's printed text, and the item IDs a first
//! change forks the copies under.

use ew_derive::{PageOrder, PageRef, Q, page_frame_form};
use ew_ids::{Id, Uuid, v5};
use ew_mltext::MlText;
use ew_scene::forms::{FrameHits, SheetMap, fill, title_block_origin};
use ew_schema::{FormDef, LibCopy};
use ew_units::{Pt2, Rect};
use serde_json::Value;

use super::view::PageBindingsOf;
use crate::Session;

/// A rectangle on the sheet, nm: `[x0, y0, x1, y1]`.
pub type SheetRect = [i64; 4];

/// A line on the sheet, nm.
pub type SheetLine = ((i64, i64), (i64, i64));

/// One cell of a title block as its page prints it.
#[derive(Clone, PartialEq, Debug)]
pub struct FrameCell {
    /// Its index in the title block's `cells`.
    pub index: usize,
    /// Where it stands on the sheet.
    pub rect: SheetRect,
    /// Its width and height in the title block, nm.
    pub size: (i64, i64),
    /// Its text (its label).
    pub label: Option<MlText>,
    /// What it shows: its placeholder template (`{project.drawing_id}`), if any.
    pub value: Option<String>,
    /// What the page prints in it: the label, one space and the value as this page fills it.
    pub printed: String,
}

/// The columns and rows of a frame, as its Properties show them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FrameAxis {
    /// How many.
    pub count: u16,
    /// Counted with letters (else numbers).
    pub letters: bool,
    /// The first label (`0`, `1`, `A`).
    pub first: String,
    /// Letters skip I and O.
    pub skip_io: bool,
}

/// The frame of a page and its title block, for changing them on the page (see the module documentation).
#[derive(Clone, PartialEq, Debug)]
pub struct FrameFacts {
    /// The page's folio label.
    pub folio: String,
    /// The drawn page, `None` for a generated page.
    pub page: Option<Uuid>,
    /// The frame copy.
    pub frame: Uuid,
    /// Its item (the `FormDef` without the copy metadata), in its JSON form.
    pub frame_item: Value,
    /// Whether the frame is the project's own (else a first change forks it).
    pub frame_local: bool,
    /// The title-block copy, if the frame names one the project holds.
    pub block: Option<Uuid>,
    /// Its item, in its JSON form.
    pub block_item: Option<Value>,
    /// Whether the title block is the project's own.
    pub block_local: bool,
    /// The item IDs a change of the title block forks its copies under (`form.edit`'s `forks`): none for a block of
    /// the project's own, else the block's, then each frame's that names it and is not the project's own.
    pub block_forks: Vec<Uuid>,
    /// The item ID a change of the frame forks it under; none for a frame of the project's own.
    pub frame_forks: Vec<Uuid>,
    /// The item IDs a change of the frame and its title block together (`frame.set_grid` with the block's height)
    /// forks them and the other frames of the block under.
    pub grid_forks: Vec<Uuid>,
    /// The item ID a copy of the title block of the frame's own takes (`form.make_own`'s `frame`).
    pub own_block_item: Option<Uuid>,
    /// The item ID a copy of the frame of the page's own takes (`form.make_own`'s `page`).
    pub own_frame_item: Uuid,
    /// Whether the page names this frame itself (a frame of its own), not by the frame defaults.
    pub page_own: bool,
    /// The title block the project's frame defaults share: the block of the frame a page of this page's type prints on
    /// without a frame of its own.
    pub shared_block: Option<Uuid>,
    /// The folio labels of the pages that print the frame, in page order.
    pub frame_pages: Vec<String>,
    /// The folio labels of the pages that print the title block, by any frame, in page order.
    pub block_pages: Vec<String>,
    /// The title block's cells.
    pub cells: Vec<FrameCell>,
    /// The title block's lines between its cells, in `FormDef::cell_edges` order.
    pub lines: Vec<SheetLine>,
    /// The column band, while the rulers are drawn.
    pub columns_band: Option<SheetRect>,
    /// The row band, while the rulers are drawn.
    pub rows_band: Option<SheetRect>,
    /// The line between the drawing area and the title block.
    pub block_top: Option<SheetLine>,
    /// The drawing area.
    pub drawing_area: Option<SheetRect>,
    /// The columns, along the top; `None` for a frame without a grid.
    pub columns: Option<FrameAxis>,
    /// The rows, along the left.
    pub rows: Option<FrameAxis>,
    /// Whether the rulers are drawn.
    pub drawn: bool,
    /// The language the project prints in (`en-GB`), whose entry of a cell's text Properties shows and writes; `None`:
    /// the neutral text.
    pub language: Option<String>,
}

fn rect_on(map: SheetMap, r: Rect) -> SheetRect {
    let r = map.apply_rect(r);
    [r.min().x.0, r.min().y.0, r.max().x.0, r.max().y.0]
}

fn point_on(map: SheetMap, p: Pt2) -> (i64, i64) {
    let p = map.apply(p);
    (p.x.0, p.y.0)
}

/// The item IDs the forks of `copies` take: for each, the first free one of its sequence (`v5::fork_item_id`) whose copy
/// the project does not hold.
fn fork_items(q: &Q, project: Uuid, copies: &[Id<FormDef>]) -> Vec<Uuid> {
    let held = |item: Uuid| q.forms().get(v5::lib_copy_id::<FormDef>(project, item)).is_some();
    copies
        .iter()
        .map(|c| {
            std::iter::once(None)
                .chain((1u64..).map(Some))
                .map(|n| v5::fork_item_id(c.as_uuid(), n))
                .find(|item| !held(*item))
                .expect("an unbounded sequence has a free item")
        })
        .collect()
}

fn axis(grid: &ew_schema::FrameGrid, columns: bool) -> FrameAxis {
    let (count, counting, skip_io) = if columns {
        (grid.columns.count, &grid.columns.counting, grid.columns.skip_io)
    } else {
        (grid.rows.count, &grid.rows.counting, grid.rows.skip_io)
    };
    let first = if columns {
        ew_derive::column_label(grid, 0)
    } else {
        ew_derive::row_label(grid, 0)
    };
    FrameAxis {
        count,
        letters: *counting == ew_schema::Counting::Letters,
        first,
        skip_io,
    }
}

impl Session {
    /// The frame of the page labelled `folio` and its title block (see [`FrameFacts`]); `None` for no such page or a page
    /// whose frame the project does not hold.
    pub fn frame_facts(&self, folio: &str) -> Option<FrameFacts> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let entry = order.entries.iter().find(|e| e.folio == folio)?;
        let key = entry.page;
        let frame_id = page_frame_form(&q, &key)?;
        let frame = q.forms().get(frame_id)?;
        let block = frame
            .item
            .title_block
            .and_then(|b| q.forms().get(b).map(|c| (b, c)));
        let project = q.project().id.as_uuid();
        let local = |c: &LibCopy<FormDef>| c.library_id.as_uuid() == project;
        let map = super::navigation::sheet_map(&q, key);
        let hits = FrameHits::of(&frame.item, block.as_ref().map(|(_, b)| &b.item));
        let bindings = q.get::<PageBindingsOf>(key);
        let cells = block
            .as_ref()
            .map(|(_, b)| {
                let origin = title_block_origin(&frame.item, &b.item);
                b.item
                    .cells
                    .iter()
                    .enumerate()
                    .map(|(index, c)| {
                        let label = c
                            .label
                            .as_ref()
                            .map(|l| ew_scene::forms::ml_text(l, bindings.language()).to_owned())
                            .unwrap_or_default();
                        let value = c
                            .value
                            .as_deref()
                            .map(|t| fill(t, &bindings).text)
                            .unwrap_or_default();
                        let printed = [label, value]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                        FrameCell {
                            index,
                            rect: rect_on(
                                map,
                                Rect::from_points(c.rect.0.min() + origin, c.rect.0.max() + origin),
                            ),
                            size: (c.rect.0.width().0, c.rect.0.height().0),
                            label: c.label.clone(),
                            value: c.value.clone(),
                            printed,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        // The pages of the order that print the frame, and those whose frame names the block.
        let mut frame_pages = Vec::new();
        let mut block_pages = Vec::new();
        for e in &order.entries {
            let Some(f) = page_frame_form(&q, &e.page) else {
                continue;
            };
            if f == frame_id {
                frame_pages.push(e.folio.clone());
            }
            let names_block = block
                .as_ref()
                .is_some_and(|(b, _)| q.forms().get(f).is_some_and(|c| c.item.title_block == Some(*b)));
            if names_block {
                block_pages.push(e.folio.clone());
            }
        }
        // A first change of the block forks it and each frame that names it and is not the project's own, in copy ID
        // order (as `form.edit` takes them).
        // The forks a first change takes, by `form.edit`'s and `frame.set_grid`'s one rule.
        let held = q.forms();
        let copies: Vec<(Id<FormDef>, &LibCopy<FormDef>)> = held.iter().map(|(id, c)| (id, &**c)).collect();
        let forks = |edited: &[Id<FormDef>]| {
            fork_items(
                &q,
                project,
                &ew_commands::commands::forked_copies(&copies, edited, project),
            )
        };
        let block_forks = block.as_ref().map(|(b, _)| forks(&[*b])).unwrap_or_default();
        let frame_forks = forks(&[frame_id]);
        let grid_forks = match &block {
            Some((b, _)) => forks(&[frame_id, *b]),
            None => frame_forks.clone(),
        };
        let grid = frame.item.grid.as_ref();
        let record = match key {
            PageRef::Drawn(p) => q.pages().get(p),
            _ => None,
        };
        let shared_block = record
            .and_then(|r| ew_derive::frame_form_of(&q, None, &r.kind, true))
            .and_then(|f| q.forms().get(f))
            .and_then(|f| f.item.title_block)
            .map(|b| b.as_uuid());
        Some(FrameFacts {
            own_block_item: block
                .as_ref()
                .and_then(|(b, _)| fork_items(&q, project, &[*b]).first().copied()),
            own_frame_item: fork_items(&q, project, &[frame_id])[0],
            page_own: record.is_some_and(|r| r.form == Some(frame_id)),
            shared_block,
            folio: entry.folio.clone(),
            page: match key {
                PageRef::Drawn(p) => Some(p.as_uuid()),
                _ => None,
            },
            frame: frame_id.as_uuid(),
            frame_item: serde_json::to_value(&frame.item).ok()?,
            frame_local: local(frame),
            block: block.as_ref().map(|(b, _)| b.as_uuid()),
            block_item: block
                .as_ref()
                .and_then(|(_, c)| serde_json::to_value(&c.item).ok()),
            block_local: block.as_ref().is_some_and(|(_, c)| local(c)),
            block_forks,
            frame_forks,
            grid_forks,
            frame_pages,
            block_pages,
            cells,
            lines: hits
                .lines
                .iter()
                .map(|(a, b)| (point_on(map, *a), point_on(map, *b)))
                .collect(),
            columns_band: hits.columns.map(|r| rect_on(map, r)),
            rows_band: hits.rows.map(|r| rect_on(map, r)),
            block_top: hits.block_top.map(|(a, b)| (point_on(map, a), point_on(map, b))),
            drawing_area: hits.drawing_area.map(|r| rect_on(map, r)),
            columns: grid.map(|g| axis(g, true)),
            rows: grid.map(|g| axis(g, false)),
            drawn: grid.is_some_and(ew_schema::FrameGrid::shows_grid),
            language: bindings.language().map(ToString::to_string),
        })
    }
}

impl Session {
    /// The parameters of `frames.adopt` that take the frames of the template in `dir` (doc 09 §3.9, Use the frames of a
    /// template…): its frame and title-block copies, and the report forms its lists print on, in their `lib/` JSON
    /// form; its frame defaults; the form of its lists; its fields of its own and their values of the whole project; its
    /// company notice and logo. The template is read and nothing of it written.
    ///
    /// # Errors
    ///
    /// The template cannot be opened.
    pub fn adopt_frames_params(dir: &std::path::Path) -> Result<Value, crate::OpenError> {
        let (template, _) = Session::open_read_only(dir, "frames")?;
        let q = template.query();
        let project = q.project();
        let lists_form = q
            .table::<ew_schema::ReportBlock>()
            .iter()
            .find(|(_, b)| b.embedded.is_none())
            .map(|(_, b)| b.form);
        let forms: Vec<Value> = q
            .forms()
            .iter()
            .filter(|(id, f)| {
                matches!(
                    f.item.kind,
                    ew_schema::FormKind::Frame | ew_schema::FormKind::TitleBlock
                ) || Some(*id) == lists_form
            })
            .filter_map(|(_, f)| serde_json::to_value(ew_schema::LibDocument::Form((**f).clone())).ok())
            .collect();
        let company = project.extra.get(ew_schema::COMPANY_MEMBER);
        let notice = company.and_then(|c| c.get("notice")).cloned();
        let logo = ew_schema::company_logo(&project.extra, |sha| q.logo_asset(sha))
            .and_then(|r| template.asset(&r.sha256))
            .map(|(_, bytes)| ew_schema::base64_encode(&bytes));
        let mut params = serde_json::json!({
            "forms": forms,
            "frames": project.settings.extra.get("frames").cloned().unwrap_or(serde_json::json!({})),
            "fields": project.user_prop_defs,
            "values": project.user_props,
        });
        if let Some(form) = lists_form {
            params["lists_form"] = Value::String(form.to_string());
        }
        if let Some(n) = notice {
            params["notice"] = n;
        }
        if let Some(l) = logo {
            params["logo"] = Value::String(l);
        }
        Ok(params)
    }
}
