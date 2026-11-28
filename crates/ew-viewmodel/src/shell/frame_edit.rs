// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Edit frame (`frame.edit`, doc 09 §3.9): the page's frame and its title block changed on the page, with the Edit
//! frame tool ([`FrameTool`]). What is chosen (a cell, several, the rulers, the frame) stands in Properties, whose rows
//! write the title block with `form.edit`: a cell's Text, Shows, Text size and Alignment; the HUD splits, joins, clears
//! and gives a cell a picture; a dragged line moves the cells on either side. A first change of a starter frame or
//! title block makes them the project's own (the forks `FrameFacts` names), and the toast says so once.

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::output::FrameFacts;
use serde_json::{Map, Value, json};

use super::{FormField, Mode, Outcome, Prompt, ShellModel, failed, string_param};
use crate::key;
use crate::properties::{
    Category, EditRoute, FieldReadOnly, FieldType, PropRow, PropertySheet, Provenance, RowAction, RowValue,
};
use crate::strings;
use crate::tools::frame::{FrameChoice, FrameTool};

/// The kind of the Properties section of the frame (its row keys are `frame/<section>.<row>`).
pub const FRAME_KIND: &str = "frame";

/// The groups of the frame's Properties in display order, by the first word of a row's path (doc 09 §3.9): a chosen
/// cell's rows, the columns', the rows', what both have (Drawn, the title block's height) and the frame's own.
pub const FRAME_SECTIONS: &[(&str, crate::strings::Key<'static>)] = &[
    ("cell", key!("frame-section-cell")),
    ("columns", key!("frame-section-columns")),
    ("rows", key!("frame-section-rows")),
    ("both", key!("frame-section-both")),
    ("frame", key!("frame-section-frame")),
];

/// A chosen cell's Text row: its label.
pub const TEXT_ROW: &str = "frame/cell.text";
/// A chosen cell's Shows row: the value it prints.
pub const SHOWS_ROW: &str = "frame/cell.shows";
/// A chosen cell's Text size row.
pub const SIZE_ROW: &str = "frame/cell.size";
/// A chosen cell's Alignment row.
pub const ALIGN_ROW: &str = "frame/cell.align";
/// The frame's Columns and rows row: a click chooses the rulers.
pub const RULERS_ROW: &str = "frame/frame.rulers";
/// The frame's Title block row: Shared with the other frames, or Its own.
pub const TITLE_BLOCK_ROW: &str = "frame/frame.title_block";
/// The frame's Give this page its own frame row.
pub const OWN_FRAME_ROW: &str = "frame/frame.own_frame";
/// The frame's Use the frames of a template… row.
pub const ADOPT_ROW: &str = "frame/frame.adopt";

/// The alignments a cell takes, by the stored word.
const ALIGNS: &[(&str, crate::strings::Key<'static>)] = &[
    ("middle_left", key!("frame-align-left")),
    ("centre", key!("frame-align-centre")),
    ("middle_right", key!("frame-align-right")),
];

/// What a cell's Shows offers besides the paths of the title block: a field of the project's own, New field of your
/// own…, the company logo, A picture… and Text only, each by the value of its choice.
const SHOWS_NEW_FIELD: &str = "new_field";
const SHOWS_PICTURE: &str = "picture";
const SHOWS_TEXT_ONLY: &str = "text_only";
const LOGO: &str = "project.logo";

/// The text of a cell's label in the language `language` the project prints in: a neutral text itself, else that
/// language's entry, else the first.
fn label_text(ml: &Value, language: Option<&str>) -> String {
    match ml {
        Value::String(s) => s.clone(),
        Value::Object(m) => language
            .and_then(|l| m.get(l))
            .or_else(|| m.values().next())
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    }
}

/// A cell's label with its text in `language` set to `text`: a label of several languages keeps the others (they stand
/// under More), a neutral label or none becomes the neutral text.
fn with_label(label: Option<&Value>, language: Option<&str>, text: &str) -> Value {
    match (label, language) {
        (Some(Value::Object(m)), Some(l)) => {
            let mut m = m.clone();
            m.insert(l.to_owned(), json!(text));
            Value::Object(m)
        }
        _ => json!(text),
    }
}

/// A placeholder template's one path (`{project.drawing_id}`), if it is just one.
fn template_path(template: &str) -> Option<&str> {
    let inner = template.trim().strip_prefix('{')?.strip_suffix('}')?;
    let path = inner.split('|').next()?.trim();
    (!path.is_empty() && !path.contains(['{', '}'])).then_some(path)
}

impl ShellModel {
    /// The frame of the page in view, as the canvas holds it.
    pub fn frame_facts(&self) -> Option<Arc<FrameFacts>> {
        self.canvas().and_then(|c| c.page().frame.clone())
    }

    /// What is chosen on the frame while Edit frame is active; `None` while another tool is.
    pub fn frame_choice(&self) -> Option<&FrameChoice> {
        self.tools.tool.frame_choice()
    }

    /// `frame.edit` (Page ▸ Frame ▸ Edit frame, a double click on the title block or the rulers, the title block's
    /// menu): the Edit frame tool on the page in view, or, while it is active, Done.
    pub(crate) fn act_frame_edit(&mut self) -> Outcome {
        if self.frame_choice().is_some() {
            return self.dismiss_tool();
        }
        let Some(facts) = self.frame_facts() else {
            return Outcome::refused(key!("reason-frame-none"));
        };
        self.activate_tool(Box::new(FrameTool::new()));
        self.notify(self.frame_card(&facts), false);
        self.props.key = None;
        Outcome::Done
    }

    /// Whether the pointer is on the title block or the rulers of the page in view (where a double click with Select
    /// changes the frame).
    pub fn pointer_on_frame(&self) -> bool {
        let Some(c) = self.canvas() else {
            return false;
        };
        let (Some(facts), Some(at)) = (&c.page().frame, c.pointer()) else {
            return false;
        };
        matches!(
            crate::tools::frame::frame_hit(facts, at, 0),
            Some(crate::tools::frame::FrameHit::Cell(_) | crate::tools::frame::FrameHit::Rulers)
        )
    }

    /// The card's sentence: which frame the page prints on and where a change shows.
    pub fn frame_card(&self, facts: &FrameFacts) -> String {
        let what = if facts.block_local && facts.frame_local {
            key!("frame-card-own")
        } else {
            key!("frame-card-project")
        };
        strings::format(key!("frame-card"), &[&facts.folio, strings::text(what)])
    }

    /// Chooses `choice` on the frame (the tool's click, or `frame.choose` of a script): Properties shows it.
    pub(super) fn frame_choose(&mut self, choice: FrameChoice) -> Outcome {
        let facts = self.frame_facts();
        let overlay = self
            .with_tool(|t, _| {
                let tool = t.as_any_mut().and_then(|a| a.downcast_mut::<FrameTool>())?;
                tool.choose(choice);
                facts.map(|f| tool.overlay(&f))
            })
            .flatten();
        if let Some(o) = overlay {
            self.tools.overlay = o;
        }
        self.props.key = None;
        self.props.edit = None;
        self.props.choices = None;
        Outcome::Done
    }

    /// `frame.choose {cells?, rulers?}`: chooses cells of the title block by their index, the rulers, or (neither)
    /// the frame, as clicks on the page do.
    pub(crate) fn act_frame_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.frame_choice().is_none() {
            return Outcome::refused(key!("reason-frame-tool-only"));
        }
        let choice = if p.get("rulers").and_then(Value::as_bool) == Some(true) {
            FrameChoice::Rulers
        } else {
            match p.get("cells") {
                None => FrameChoice::Frame,
                Some(Value::Array(a)) => {
                    let Some(cells) = a
                        .iter()
                        .map(|v| v.as_u64().map(|i| i as usize))
                        .collect::<Option<Vec<_>>>()
                    else {
                        return failed(key!("error-bad-param"), &["cells"]);
                    };
                    let known = self.frame_facts().map_or(0, |f| f.cells.len());
                    if cells.iter().any(|c| *c >= known) {
                        return failed(key!("error-bad-param"), &["cells"]);
                    }
                    if cells.is_empty() {
                        FrameChoice::Frame
                    } else {
                        FrameChoice::Cells(cells)
                    }
                }
                Some(_) => return failed(key!("error-bad-param"), &["cells"]),
            }
        };
        self.frame_choose(choice)
    }

    /// The chosen cells, when cells are chosen.
    fn chosen_cells(&self) -> Option<Vec<usize>> {
        match self.frame_choice() {
            Some(FrameChoice::Cells(c)) => Some(c.clone()),
            _ => None,
        }
    }

    /// Writes the title block as `change` makes it of its item (`form.edit`, with the forks of a first change), with
    /// `field` and `picture` beside; the toast of a first change says that the frame and title block are the
    /// project's own now.
    fn edit_block(
        &mut self,
        change: impl FnOnce(&mut Value) -> Result<(), String>,
        extra: Map<String, Value>,
    ) -> Outcome {
        let Some(facts) = self.frame_facts() else {
            return Outcome::refused(key!("reason-frame-none"));
        };
        let (Some(block), Some(mut item)) = (facts.block, facts.block_item.clone()) else {
            return Outcome::refused(key!("reason-frame-no-block"));
        };
        if let Err(message) = change(&mut item) {
            return Outcome::Failed { message };
        }
        let mut params = json!({"copy": block.to_string(), "form": item});
        if !facts.block_forks.is_empty() {
            params["forks"] = json!(
                facts
                    .block_forks
                    .iter()
                    .map(|u| u.to_string())
                    .collect::<Vec<_>>()
            );
        }
        for (k, v) in extra {
            params[k] = v;
        }
        let first = !facts.block_forks.is_empty();
        let out = self.run_commit(&AnyCommand {
            name: "form.edit".to_owned(),
            params,
        });
        if matches!(out, Outcome::Done) && first {
            self.notify(strings::text(key!("toast-frame-forked")).to_owned(), true);
        }
        self.props.key = None;
        out
    }

    /// The cell `i` of a title block's JSON item.
    fn cell_mut(item: &mut Value, i: usize) -> Result<&mut Value, String> {
        item["cells"]
            .get_mut(i)
            .ok_or_else(|| strings::text(key!("reason-frame-no-cell")).to_owned())
    }

    /// `frame.split {dir}` (HUD Split side by side, `side_by_side`; Split one above the other, `one_above`): the chosen cell in two
    /// halves, the new one empty, the old one keeping its text and value; the new half is chosen after it.
    pub(crate) fn act_frame_split(&mut self, p: &Map<String, Value>) -> Outcome {
        let side = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some("side_by_side") | None) => true,
            Ok(Some("one_above")) => false,
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["dir"]),
        };
        let Some(&[i]) = self.chosen_cells().as_deref() else {
            return Outcome::refused(key!("reason-frame-one-cell"));
        };
        let out = self.edit_block(
            |item| {
                let cell = Self::cell_mut(item, i)?;
                let r = cell["rect"].clone();
                let (x0, y0, x1, y1) = (
                    r[0][0].as_i64().unwrap_or(0),
                    r[0][1].as_i64().unwrap_or(0),
                    r[1][0].as_i64().unwrap_or(0),
                    r[1][1].as_i64().unwrap_or(0),
                );
                let (old, new) = if side {
                    let m = x0 + (x1 - x0) / 2;
                    (json!([[x0, y0], [m, y1]]), json!([[m, y0], [x1, y1]]))
                } else {
                    let m = y0 + (y1 - y0) / 2;
                    (json!([[x0, y0], [x1, m]]), json!([[x0, m], [x1, y1]]))
                };
                let mut half = cell.clone();
                cell["rect"] = old;
                half["rect"] = new;
                if let Some(m) = half.as_object_mut() {
                    m.remove("label");
                    m.remove("value");
                }
                item["cells"]
                    .as_array_mut()
                    .ok_or_else(String::new)?
                    .insert(i + 1, half);
                Ok(())
            },
            Map::new(),
        );
        if matches!(out, Outcome::Done) {
            self.frame_choose(FrameChoice::Cells(vec![i + 1]));
        }
        out
    }

    /// `frame.join` (HUD Join): the chosen cells become the first of them, where together they make a rectangle; the
    /// first keeps its text and value, and a text that goes is named in the toast, with Undo.
    pub(crate) fn act_frame_join(&mut self) -> Outcome {
        let Some(cells) = self.chosen_cells().filter(|c| c.len() > 1) else {
            return Outcome::refused(key!("reason-frame-join-two"));
        };
        let Some(facts) = self.frame_facts() else {
            return Outcome::refused(key!("reason-frame-none"));
        };
        let rects: Vec<[i64; 4]> = cells
            .iter()
            .filter_map(|i| {
                facts.block_item.as_ref()?["cells"]
                    .get(*i)
                    .map(|c| c["rect"].clone())
            })
            .map(|r| {
                [
                    r[0][0].as_i64().unwrap_or(0),
                    r[0][1].as_i64().unwrap_or(0),
                    r[1][0].as_i64().unwrap_or(0),
                    r[1][1].as_i64().unwrap_or(0),
                ]
            })
            .collect();
        let union = rects.iter().fold(rects[0], |u, r| {
            [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]
        });
        let area = |r: &[i64; 4]| i128::from(r[2] - r[0]) * i128::from(r[3] - r[1]);
        if rects.iter().map(area).sum::<i128>() != area(&union) {
            return Outcome::refused(key!("reason-frame-join-rectangle"));
        }
        let dropped: Vec<String> = cells[1..]
            .iter()
            .filter_map(|i| facts.cells.iter().find(|c| c.index == *i))
            .map(|c| c.printed.clone())
            .filter(|t| !t.trim().is_empty())
            .collect();
        let first = cells[0];
        let mut gone: Vec<usize> = cells[1..].to_vec();
        gone.sort_unstable_by(|a, b| b.cmp(a));
        let out = self.edit_block(
            |item| {
                Self::cell_mut(item, first)?["rect"] = json!([[union[0], union[1]], [union[2], union[3]]]);
                let list = item["cells"].as_array_mut().ok_or_else(String::new)?;
                for i in &gone {
                    list.remove(*i);
                }
                Ok(())
            },
            Map::new(),
        );
        if matches!(out, Outcome::Done) {
            let kept = first - gone.iter().filter(|g| **g < first).count();
            self.frame_choose(FrameChoice::Cells(vec![kept]));
            if !dropped.is_empty() {
                self.notify(
                    strings::format(key!("toast-frame-join-dropped"), &[&dropped.join(", ")]),
                    true,
                );
            }
        }
        out
    }

    /// `frame.clear` (Del on a chosen cell): its text and its value go.
    pub(crate) fn act_frame_clear(&mut self) -> Outcome {
        let Some(cells) = self.chosen_cells() else {
            return Outcome::refused(key!("reason-frame-one-cell"));
        };
        self.edit_block(
            |item| {
                for i in &cells {
                    if let Some(m) = Self::cell_mut(item, *i)?.as_object_mut() {
                        m.remove("label");
                        m.remove("value");
                    }
                }
                Ok(())
            },
            Map::new(),
        )
    }

    /// `frame.picture {path}` (HUD Picture…, Shows ▸ A picture…): an SVG, PNG or JPEG file of at most 512 KiB put into
    /// the chosen cell, fitted at its proportions and centred, for this title block alone. In the window without a
    /// path the native file dialog asks for it.
    pub(crate) fn act_frame_picture(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(&[i]) = self.chosen_cells().as_deref() else {
            return Outcome::refused(key!("reason-frame-one-cell"));
        };
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => return self.need_path("frame.picture", Prompt::OpenImage, "path", p),
        };
        let name = path.display().to_string();
        let limit = ew_api::MAX_LOGO_BYTES;
        let bytes = match std::fs::metadata(&path) {
            Ok(m) if !m.is_file() => return failed(key!("error-image-file"), &[&name, "not a regular file"]),
            Ok(m) if m.len() > limit as u64 => {
                return failed(
                    key!("error-image-too-large"),
                    &[&name, &(limit / 1024).to_string()],
                );
            }
            Ok(_) => match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => return failed(key!("error-image-file"), &[&name, &e.to_string()]),
            },
            Err(e) => return failed(key!("error-image-file"), &[&name, &e.to_string()]),
        };
        let mut extra = Map::new();
        extra.insert(
            "picture".to_owned(),
            json!({"cell": i, "image": ew_api::base64_encode(&bytes)}),
        );
        self.edit_block(|_| Ok(()), extra)
    }

    /// The HUD's buttons while Edit frame is active: Split side by side, Split one above the other, Join, Picture… and
    /// Done, each as its action.
    pub(crate) fn frame_hud_buttons(&self) -> Vec<crate::command_bar::HudButton> {
        let b = |label: crate::strings::Key<'static>, action: &'static str, params: Value| {
            crate::command_bar::HudButton {
                label: strings::text(label).to_owned(),
                action,
                params,
            }
        };
        vec![
            b(
                key!("frame-split-side"),
                "frame.split",
                json!({"dir": "side_by_side"}),
            ),
            b(
                key!("frame-split-above"),
                "frame.split",
                json!({"dir": "one_above"}),
            ),
            b(key!("frame-join"), "frame.join", json!({})),
            b(key!("frame-picture"), "frame.picture", json!({})),
            b(key!("frame-done"), "frame.edit", json!({})),
        ]
    }

    /// The released drag of the Edit frame tool: the title block's line `line` at its new place `to` (sheet nm); every
    /// cell on either side of it grows or shrinks with it.
    pub(super) fn frame_line_moved(&mut self, line: usize, to: i64) -> Outcome {
        let Some(facts) = self.frame_facts() else {
            return Outcome::refused(key!("reason-frame-none"));
        };
        let Some(&((ax, ay), (bx, by))) = facts.lines.get(line) else {
            return Outcome::Done;
        };
        let vertical = ax == bx;
        let from = if vertical { ax } else { ay };
        // The sheet's shift as the block's shift: the scale of the page-to-sheet map, from the block's width.
        let Some(item) = facts.block_item.clone() else {
            return Outcome::Done;
        };
        let block_w = item["sheet"]["width"].as_i64().unwrap_or(0);
        let block_h = item["sheet"]["height"].as_i64().unwrap_or(0);
        let (sheet_x0, sheet_x1) = facts.cells.iter().fold((i64::MAX, i64::MIN), |(a, b), c| {
            (a.min(c.rect[0]), b.max(c.rect[2]))
        });
        let (sheet_y0, sheet_y1) = facts.cells.iter().fold((i64::MAX, i64::MIN), |(a, b), c| {
            (a.min(c.rect[1]), b.max(c.rect[3]))
        });
        let to_block = |v: i64, s0: i64, s1: i64, size: i64| -> i64 {
            if s1 <= s0 {
                return v;
            }
            ((i128::from(v - s0) * i128::from(size)) / i128::from(s1 - s0)) as i64
        };
        let (old, new, span) = if vertical {
            (
                to_block(from, sheet_x0, sheet_x1, block_w),
                to_block(to, sheet_x0, sheet_x1, block_w),
                (
                    to_block(ay.min(by), sheet_y0, sheet_y1, block_h),
                    to_block(ay.max(by), sheet_y0, sheet_y1, block_h),
                ),
            )
        } else {
            (
                to_block(from, sheet_y0, sheet_y1, block_h),
                to_block(to, sheet_y0, sheet_y1, block_h),
                (
                    to_block(ax.min(bx), sheet_x0, sheet_x1, block_w),
                    to_block(ax.max(bx), sheet_x0, sheet_x1, block_w),
                ),
            )
        };
        if old == new {
            return Outcome::Done;
        }
        // A coordinate within half a millimetre of the line is on it (the sheet's rounding).
        let near = |a: i64, b: i64| (a - b).abs() <= 250_000;
        let (axis, other) = if vertical {
            (0usize, 1usize)
        } else {
            (1usize, 0usize)
        };
        let moved_from = old;
        self.edit_block(
            |item| {
                let cells = item["cells"].as_array_mut().ok_or_else(String::new)?;
                for c in cells.iter_mut() {
                    let lo = c["rect"][0][other].as_i64().unwrap_or(0);
                    let hi = c["rect"][1][other].as_i64().unwrap_or(0);
                    if lo < span.0 - 250_000 || hi > span.1 + 250_000 {
                        continue;
                    }
                    for corner in 0..2 {
                        let v = c["rect"][corner][axis].as_i64().unwrap_or(0);
                        if near(v, moved_from) {
                            c["rect"][corner][axis] = json!(new);
                        }
                    }
                }
                Ok(())
            },
            Map::new(),
        )
    }

    /// The Properties sheet of what is chosen on the frame: a chosen cell's rows, else the frame's.
    pub(super) fn frame_sheet(&self, choice: &FrameChoice) -> Option<PropertySheet> {
        let facts = self.frame_facts()?;
        let read_only = self.read_only_reason().map(str::to_owned);
        let row =
            |key: &str, label: String, ty: FieldType, text: String, json: Value, editable: bool| PropRow {
                key: format!("{FRAME_KIND}/{key}"),
                path: key.to_owned(),
                category: Category::Other,
                depth: 0,
                ty,
                value: RowValue::One { json, text },
                provenance: Provenance::Authored,
                actions: if editable {
                    vec![RowAction::Edit]
                } else {
                    Vec::new()
                },
                route: if editable {
                    EditRoute::Command {
                        command: "form.edit".to_owned(),
                    }
                } else {
                    EditRoute::ReadOnly(FieldReadOnly::NoCommand)
                },
                identifier: false,
                source: None,
                named_reference: false,
                label: Some(label),
                link: None,
                note: None,
            };
        let text = |k: crate::strings::Key<'static>| strings::text(k).to_owned();
        let mut header = ew_api::output::SheetHeader::default();
        let rows = match choice {
            FrameChoice::Cells(cells) => {
                let cell = facts.cells.iter().find(|c| Some(&c.index) == cells.first())?;
                let item = facts.block_item.as_ref()?["cells"].get(cell.index)?.clone();
                header.title = if cell.printed.is_empty() {
                    text(key!("frame-cell-empty"))
                } else {
                    cell.printed.clone()
                };
                header.subtitle = Some(strings::format_count(
                    crate::count_key!("frame-cell-where"),
                    facts.block_pages.len(),
                ));
                let label = item
                    .get("label")
                    .map(|l| label_text(l, facts.language.as_deref()))
                    .unwrap_or_default();
                let shows = self.shows_words(item.get("value").and_then(Value::as_str));
                let style = item["style"].as_str().unwrap_or_default().to_owned();
                let style_name = self
                    .text_style_rows()
                    .into_iter()
                    .find(|r| r.key == style)
                    .map_or_else(|| style.clone(), |r| r.name);
                let align = item["align"].as_str().unwrap_or_default().to_owned();
                let align_words = ALIGNS
                    .iter()
                    .find(|(a, _)| *a == align)
                    .map_or_else(|| align.clone(), |(_, k)| text(*k));
                let mm = |nm: i64| self.frame_mm(nm);
                vec![
                    row(
                        "cell.text",
                        text(key!("frame-row-text")),
                        FieldType::Text,
                        label.clone(),
                        json!(label),
                        true,
                    ),
                    row(
                        "cell.shows",
                        text(key!("frame-row-shows")),
                        FieldType::Enum { options: Vec::new() },
                        shows.clone(),
                        json!(shows),
                        true,
                    ),
                    row(
                        "cell.size",
                        text(key!("frame-row-size")),
                        FieldType::Enum { options: Vec::new() },
                        style_name,
                        json!(style),
                        true,
                    ),
                    row(
                        "cell.align",
                        text(key!("frame-row-align")),
                        FieldType::Enum {
                            options: ALIGNS.iter().map(|(a, _)| (*a).to_owned()).collect(),
                        },
                        align_words,
                        json!(align),
                        true,
                    ),
                    row(
                        "cell.typed_in",
                        text(key!("frame-row-typed-in")),
                        FieldType::Text,
                        text(key!("frame-typed-in")),
                        Value::Null,
                        false,
                    ),
                    row(
                        "cell.size_mm",
                        text(key!("frame-row-size-mm")),
                        FieldType::Text,
                        strings::format(key!("frame-size-mm"), &[&mm(cell.size.0), &mm(cell.size.1)]),
                        Value::Null,
                        false,
                    ),
                ]
            }
            FrameChoice::Rulers => {
                header.title = text(key!("frame-row-rulers"));
                header.subtitle = Some(strings::format_count(
                    crate::count_key!("frame-pages"),
                    facts.frame_pages.len(),
                ));
                let typed = |key: &str, label: String, ty: FieldType, text: String, json: Value| {
                    row(key, label, ty, text, json, true)
                };
                self.ruler_rows(&facts, &typed)
            }
            FrameChoice::Frame => {
                header.title = text(key!("frame-title"));
                header.subtitle = Some(strings::format_count(
                    crate::count_key!("frame-pages"),
                    facts.frame_pages.len(),
                ));
                let sheet = &facts.frame_item["sheet"];
                let mm = |v: &Value| self.frame_mm(v.as_i64().unwrap_or(0));
                let own = facts.block.is_some() && facts.block != facts.shared_block;
                let block = if own {
                    text(key!("frame-block-own"))
                } else {
                    text(key!("frame-block-shared"))
                };
                let mut rows = vec![
                    row(
                        "frame.paper",
                        text(key!("frame-row-paper")),
                        FieldType::Text,
                        strings::format(
                            key!("frame-size-mm"),
                            &[&mm(&sheet["width"]), &mm(&sheet["height"])],
                        ),
                        Value::Null,
                        false,
                    ),
                    row(
                        "frame.title_block",
                        text(key!("frame-row-title-block")),
                        FieldType::Enum { options: Vec::new() },
                        block,
                        json!(if own { "own" } else { "shared" }),
                        facts.block.is_some(),
                    ),
                    row(
                        "frame.own_frame",
                        text(key!("frame-row-own-frame")),
                        FieldType::Text,
                        text(if facts.page_own {
                            key!("frame-block-own")
                        } else {
                            key!("frame-own-frame-give")
                        }),
                        Value::Null,
                        !facts.page_own && facts.page.is_some(),
                    ),
                ];
                if let (Some(c), Some(r)) = (&facts.columns, &facts.rows) {
                    let axis =
                        |a: &ew_api::output::FrameAxis, letters: crate::strings::Key<'static>, numbers| {
                            strings::format(
                                if a.letters { letters } else { numbers },
                                &[&a.count.to_string(), &a.first],
                            )
                        };
                    rows.push(row(
                        "frame.rulers",
                        text(key!("frame-row-rulers")),
                        FieldType::Text,
                        format!(
                            "{}, {}",
                            axis(c, key!("frame-axis-letters"), key!("frame-axis-numbers")),
                            axis(r, key!("frame-axis-letters"), key!("frame-axis-numbers"))
                        ),
                        Value::Null,
                        true,
                    ));
                }
                rows.push(row(
                    "frame.adopt",
                    text(key!("frame-row-adopt")),
                    FieldType::Text,
                    text(key!("action-frame-adopt")),
                    Value::Null,
                    true,
                ));
                rows
            }
        };
        Some(PropertySheet {
            rev: self.revision().unwrap_or(0),
            header,
            sections: vec![ew_api::output::SheetSection {
                kind: FRAME_KIND.to_owned(),
                targets: Vec::new(),
                rows,
            }],
            missing: 0,
            read_only,
            languages: Vec::new(),
        })
    }

    /// The words of what a cell shows (`value` its template): a path of the Shows list by its words, a field of the
    /// project's own by its name, the company logo, text of its own (Text only).
    fn shows_words(&self, value: Option<&str>) -> String {
        let Some(path) = value.and_then(template_path) else {
            return strings::text(key!("shows-text-only")).to_owned();
        };
        if path == LOGO {
            return strings::text(key!("shows-company-logo")).to_owned();
        }
        self.shows_choices()
            .into_iter()
            .find(|(_, v)| v.as_str() == Some(path))
            .map_or_else(|| path.to_owned(), |(w, _)| w)
    }

    /// What a cell's Shows offers, in words with their values: the paths of the title block in their groups, the fields of
    /// the project's own, New field of your own…, Company logo, A picture… and Text only (doc 09 §3.9).
    pub fn shows_choices(&self) -> Vec<(String, Value)> {
        let mut out: Vec<(String, Value)> = super::title_block::shows_paths()
            .into_iter()
            .map(|(path, words)| (words, Value::String(path)))
            .collect();
        let own = strings::text(key!("shows-group-own"));
        if let Some(p) = &self.project {
            for page in [true, false] {
                for f in p.session.own_field_values(page, &[]) {
                    let name = strings::render(&ew_api::Text::bare(ew_api::TextArg::Ml(f.label)));
                    let path = if page {
                        format!("page.prop.{}", f.key)
                    } else {
                        format!("project.prop.{}", f.key)
                    };
                    out.push((format!("{own} \u{b7} {name}"), Value::String(path)));
                }
            }
        }
        let other = strings::text(key!("shows-group-other"));
        out.push((
            format!("{own} \u{b7} {}", strings::text(key!("action-field-new"))),
            Value::String(SHOWS_NEW_FIELD.to_owned()),
        ));
        for (k, v) in [
            (key!("shows-company-logo"), LOGO),
            (key!("shows-picture"), SHOWS_PICTURE),
            (key!("shows-text-only"), SHOWS_TEXT_ONLY),
        ] {
            out.push((
                format!("{other} \u{b7} {}", strings::text(k)),
                Value::String(v.to_owned()),
            ));
        }
        out
    }

    /// The choices of a frame row, filtered by `filter`: Shows', the project's text styles by their names, the three
    /// alignments.
    pub(super) fn frame_row_choices(&self, row: &str, filter: &str) -> Option<Vec<(String, Value)>> {
        let f = filter.to_lowercase();
        let items: Vec<(String, Value)> = match row {
            SHOWS_ROW => self.shows_choices(),
            SIZE_ROW => self
                .text_style_rows()
                .into_iter()
                .map(|r| (r.name, Value::String(r.key)))
                .collect(),
            ALIGN_ROW => ALIGNS
                .iter()
                .map(|(a, k)| (strings::text(*k).to_owned(), Value::String((*a).to_owned())))
                .collect(),
            "frame/columns.counting" | "frame/rows.counting" => vec![
                (strings::text(key!("frame-numbers")).to_owned(), json!("numbers")),
                (strings::text(key!("frame-letters")).to_owned(), json!("letters")),
            ],
            TITLE_BLOCK_ROW => vec![
                (
                    strings::text(key!("frame-block-shared")).to_owned(),
                    json!("shared"),
                ),
                (strings::text(key!("frame-block-own")).to_owned(), json!("own")),
            ],
            "frame/rows.skip_io" | "frame/both.drawn" => vec![
                (strings::text(key!("frame-yes")).to_owned(), json!(true)),
                (strings::text(key!("frame-no")).to_owned(), json!(false)),
            ],
            _ => return None,
        };
        Some(
            items
                .into_iter()
                .filter(|(w, _)| w.to_lowercase().contains(&f))
                .collect(),
        )
    }

    /// The value a text typed into the frame row `row` names: the Text row's text itself; for a row of choices, the
    /// choice `choice` of its list, else the one whose words or value the text is. `None` for another row or no match.
    pub(super) fn frame_typed_value(&self, row: &str, text: &str, choice: Option<usize>) -> Option<Value> {
        let typed_rows = [
            TEXT_ROW,
            "frame/columns.count",
            "frame/columns.first",
            "frame/rows.count",
            "frame/rows.first",
            "frame/both.block_height",
        ];
        if typed_rows.contains(&row) {
            return Some(Value::String(text.to_owned()));
        }
        let items = self.frame_row_choices(row, "")?;
        if let Some(i) = choice {
            return items.get(i).map(|(_, v)| v.clone());
        }
        let t = text.trim();
        items
            .into_iter()
            .find(|(w, v)| w.eq_ignore_ascii_case(t) || v.as_str() == Some(t))
            .map(|(_, v)| v)
    }

    /// A frame row's value written (`row` its key): the chosen cells' text, value, style or alignment, in one
    /// `form.edit`; Shows' New field of your own… opens its form and A picture… the file dialog. `None` for another
    /// row.
    pub(super) fn commit_frame_row(&mut self, row: &str, value: &Value) -> Option<Outcome> {
        if !row.starts_with(&format!("{FRAME_KIND}/")) {
            return None;
        }
        if let Some(out) = self.commit_ruler_row(&row[FRAME_KIND.len() + 1..], value) {
            return Some(out);
        }
        if row == TITLE_BLOCK_ROW {
            self.props.edit = None;
            self.props.choices = None;
            return Some(self.frame_block_own(value.as_str() == Some("own")));
        }
        let cells = self.chosen_cells()?;
        self.props.edit = None;
        self.props.choices = None;
        let text = value.as_str().unwrap_or_default().to_owned();
        Some(match row {
            // The cell waits for the field New field of your own… makes: its form in the window, `field.new` in a script.
            SHOWS_ROW if text == SHOWS_NEW_FIELD => {
                self.frame_field_cell = cells.first().copied();
                match self.mode {
                    Mode::Gui => self.act_field_new(&Map::new()),
                    Mode::Headless => Outcome::Done,
                }
            }
            SHOWS_ROW if text == SHOWS_PICTURE => self.act_frame_picture(&Map::new()),
            _ => {
                let row = row.to_owned();
                let language = self.frame_facts().and_then(|f| f.language.clone());
                self.edit_block(
                    move |item| {
                        for i in &cells {
                            let cell = Self::cell_mut(item, *i)?;
                            match row.as_str() {
                                TEXT_ROW if text.trim().is_empty() => {
                                    cell.as_object_mut().map(|m| m.remove("label"));
                                }
                                TEXT_ROW => {
                                    cell["label"] = with_label(cell.get("label"), language.as_deref(), &text)
                                }
                                SHOWS_ROW if text == SHOWS_TEXT_ONLY => {
                                    cell.as_object_mut().map(|m| m.remove("value"));
                                }
                                SHOWS_ROW => cell["value"] = json!(format!("{{{text}}}")),
                                SIZE_ROW => cell["style"] = json!(text),
                                ALIGN_ROW => cell["align"] = json!(text),
                                _ => {}
                            }
                        }
                        Ok(())
                    },
                    Map::new(),
                )
            }
        })
    }

    /// New field of your own… from a cell's Shows: the field made and the cell given it in one `form.edit`
    /// (`field`), the cell's text becoming the name and a colon where it had none. `None` while no cell asked.
    pub(super) fn frame_field_from_cell(&mut self, name: &str, scope: &str, value: &str) -> Option<Outcome> {
        let cell = self.frame_field_cell.take()?;
        let mut field = json!({"cell": cell, "label": name, "scope": scope});
        if !value.is_empty() {
            field["value"] = json!(value);
            if scope == "page" {
                let Some(page) = self.frame_facts().and_then(|f| f.page) else {
                    return Some(Outcome::refused(key!("reason-no-page")));
                };
                field["page"] = json!(page.to_string());
            }
        }
        let mut extra = Map::new();
        extra.insert("field".to_owned(), field);
        Some(self.edit_block(|_| Ok(()), extra))
    }

    /// The double click on a chosen cell: the free-text editor opens on its text (the window's text popup; in a
    /// script `text.input` and `tool.finish`).
    pub(super) fn frame_type(&mut self) -> Outcome {
        // The text the Text row shows: the cell's label in the language the project prints in.
        let text = self
            .frame_facts()
            .and_then(|f| {
                let i = *self.chosen_cells()?.first()?;
                let label = f.block_item.as_ref()?["cells"].get(i)?.get("label")?.clone();
                Some(label_text(&label, f.language.as_deref()))
            })
            .unwrap_or_default();
        if let Some(tool) = self
            .tools
            .tool
            .as_any_mut()
            .and_then(|t| t.downcast_mut::<FrameTool>())
        {
            tool.type_text(text);
        }
        Outcome::Done
    }

    /// Enter in the free-text editor of a cell: the cell's text written as its Text row writes it.
    pub(super) fn frame_text(&mut self, text: &str) -> Outcome {
        self.commit_frame_row(TEXT_ROW, &json!(text))
            .unwrap_or_else(|| Outcome::refused(key!("reason-frame-none")))
    }
}

/// The rulers' rows: `frame/<axis>.<member>` for the columns and the rows, `frame/drawn` and `frame/block_height`.
const RULER_ROWS: &[&str] = &[
    "columns.count",
    "columns.counting",
    "columns.first",
    "rows.count",
    "rows.counting",
    "rows.first",
    "rows.skip_io",
    "both.drawn",
    "both.block_height",
];

/// The letters of an axis counted with letters: `A … Z`, without I and O when `skip_io`.
fn axis_letters(skip_io: bool) -> Vec<char> {
    ('A'..='Z')
        .filter(|c| !(skip_io && matches!(c, 'I' | 'O')))
        .collect()
}

/// The index of a letter label (`A` 0, `Z` 25, `AA` 26 …) over `letters`; `None` for a text that is no such label.
fn letter_index(text: &str, letters: &[char]) -> Option<u16> {
    let mut index: u64 = 0;
    let mut any = false;
    for c in text.trim().chars() {
        let d = letters.iter().position(|l| l.eq_ignore_ascii_case(&c))? as u64;
        index = index.checked_mul(letters.len() as u64)?.checked_add(d + 1)?;
        any = true;
    }
    any.then(|| index - 1).and_then(|i| u16::try_from(i).ok())
}

impl ShellModel {
    /// The parameters of the open Columns and rows card (`frame.set_grid`), if one is open.
    fn grid_pending(&self) -> Option<Value> {
        self.props
            .card
            .as_ref()
            .filter(|c| c.edit.name == "frame.set_grid")
            .map(|c| c.edit.params.clone())
    }

    /// The rulers' rows: Columns and Rows, each How many, Counted with, First (the rows' Skip I and O), Drawn and the
    /// title block's height, each with the value of the open card where it changes one.
    /// A length of the frame as every Properties row writes one: the shortest millimetres, with the project's
    /// decimal separator ([`ew_api::output::mm_words`]); the unit stands in the row's words.
    fn frame_mm(&self, nm: i64) -> String {
        let comma = self.project.as_ref().is_some_and(|p| p.session.decimal_comma());
        ew_api::output::mm_words(i128::from(nm), comma)
    }

    fn ruler_rows(
        &self,
        facts: &FrameFacts,
        row: &dyn Fn(&str, String, FieldType, String, Value) -> PropRow,
    ) -> Vec<PropRow> {
        let pending = self.grid_pending().unwrap_or(Value::Null);
        let text = |k: crate::strings::Key<'static>| strings::text(k).to_owned();
        let mut rows = Vec::new();
        for (axis, facts_axis) in [("columns", &facts.columns), ("rows", &facts.rows)] {
            let Some(a) = facts_axis else { continue };
            let p = &pending[axis];
            let count = p["count"].as_u64().map_or(a.count.to_string(), |c| c.to_string());
            let letters = p["counting"].as_str().map_or(a.letters, |c| c == "letters");
            let skip = p["skip_io"].as_bool().unwrap_or(a.skip_io);
            let first = match p["first"].as_u64() {
                Some(i) if letters => {
                    let l = axis_letters(skip);
                    let mut k = i as usize;
                    let mut out = Vec::new();
                    loop {
                        out.push(l[k % l.len()]);
                        if k < l.len() {
                            break;
                        }
                        k = k / l.len() - 1;
                    }
                    out.iter().rev().collect()
                }
                Some(i) => i.to_string(),
                None => a.first.clone(),
            };
            let counting = if letters {
                key!("frame-letters")
            } else {
                key!("frame-numbers")
            };
            rows.push(row(
                &format!("{axis}.count"),
                text(key!("frame-how-many")),
                FieldType::Text,
                count.clone(),
                json!(count),
            ));
            rows.push(row(
                &format!("{axis}.counting"),
                text(key!("frame-counted-with")),
                FieldType::Enum { options: Vec::new() },
                text(counting),
                json!(if letters { "letters" } else { "numbers" }),
            ));
            rows.push(row(
                &format!("{axis}.first"),
                text(key!("frame-first")),
                FieldType::Text,
                first.clone(),
                json!(first),
            ));
            if axis == "rows" {
                let yes = if skip { key!("frame-yes") } else { key!("frame-no") };
                rows.push(row(
                    "rows.skip_io",
                    text(key!("frame-skip-io")),
                    FieldType::Enum { options: Vec::new() },
                    text(yes),
                    json!(skip),
                ));
            }
        }
        let drawn = pending["drawn"].as_bool().unwrap_or(facts.drawn);
        rows.push(row(
            "both.drawn",
            text(key!("frame-drawn")),
            FieldType::Enum { options: Vec::new() },
            text(if drawn {
                key!("frame-yes")
            } else {
                key!("frame-no")
            }),
            json!(drawn),
        ));
        if let Some(block) = &facts.block_item {
            let h = pending["title_block_height"]
                .as_i64()
                .unwrap_or_else(|| block["sheet"]["height"].as_i64().unwrap_or(0));
            let mm = self.frame_mm(h);
            rows.push(row(
                "both.block_height",
                text(key!("frame-block-height")),
                FieldType::Text,
                mm.clone(),
                json!(mm),
            ));
        }
        rows
    }

    /// A value given to a rulers' row: the open Columns and rows card's parameters with it, previewed again
    /// (`frame.set_grid`, Bulk); a count, a first label or a height that cannot be taken is refused under the row in
    /// words. `None` for another row.
    fn commit_ruler_row(&mut self, member: &str, value: &Value) -> Option<Outcome> {
        if !RULER_ROWS.contains(&member) {
            return None;
        }
        let facts = self.frame_facts()?;
        let key = format!("{FRAME_KIND}/{member}");
        let mut params = self
            .grid_pending()
            .unwrap_or_else(|| json!({"frame": facts.frame.to_string()}));
        let text = match value {
            Value::String(s) => s.trim().to_owned(),
            other => other.to_string(),
        };
        let refuse = |s: &mut Self, k: crate::strings::Key<'static>| {
            let message = strings::text(k).to_owned();
            s.error_under_row(key.clone(), message.clone());
            Outcome::Failed { message }
        };
        let (axis, part) = member.split_once('.').unwrap_or((member, ""));
        match (axis, part) {
            (_, "count") => match text.parse::<u16>() {
                Ok(n) if (1..=ew_api::MAX_AXIS).contains(&n) => params[axis]["count"] = json!(n),
                _ => return Some(refuse(self, key!("error-frame-count"))),
            },
            (_, "counting") => params[axis]["counting"] = json!(text),
            (_, "first") => {
                let facts_axis = if axis == "columns" {
                    &facts.columns
                } else {
                    &facts.rows
                };
                let letters = params[axis]["counting"]
                    .as_str()
                    .map_or(facts_axis.as_ref().is_some_and(|a| a.letters), |c| c == "letters");
                let skip = params[axis]["skip_io"]
                    .as_bool()
                    .unwrap_or(facts_axis.as_ref().is_some_and(|a| a.skip_io));
                let index = if letters {
                    letter_index(&text, &axis_letters(skip))
                } else {
                    text.parse::<u16>().ok()
                };
                match index.filter(|i| *i <= ew_api::MAX_FIRST) {
                    Some(i) => params[axis]["first"] = json!(i),
                    None if letters => return Some(refuse(self, key!("error-frame-first-letter"))),
                    None => return Some(refuse(self, key!("error-frame-first-number"))),
                }
            }
            ("rows", "skip_io") => {
                params["rows"]["skip_io"] = json!(value.as_bool().unwrap_or(text == "true"))
            }
            ("both", "drawn") => params["drawn"] = json!(value.as_bool().unwrap_or(text == "true")),
            ("both", "block_height") => match ew_api::output::parse_mm(&text) {
                Some(nm) if nm >= i128::from(ew_api::MIN_CELL.0) => {
                    params["title_block_height"] = json!(nm);
                }
                _ => return Some(refuse(self, key!("error-frame-block-height"))),
            },
            _ => return None,
        }
        // The forks of the copies the change makes the project's own: the title block's too with its height.
        let forks = if params.get("title_block_height").is_some() {
            &facts.grid_forks
        } else {
            &facts.frame_forks
        };
        params["forks"] = json!(forks.iter().map(|u| u.to_string()).collect::<Vec<_>>());
        self.props.edit = None;
        self.props.choices = None;
        let out = self.preview_host_card("frame.set_grid", params, key!("reason-frame-grid"));
        // The rows show the card's values.
        self.props.key = None;
        Some(out)
    }
}

impl ShellModel {
    /// Title block ▸ Its own (`own`): a copy of the block for this frame alone (`form.make_own`, the frame forked first
    /// when it is not the project's own); Shared with the other frames: the frame names the block the frame defaults
    /// share again (`form.edit`), the copy staying unused until purged.
    pub(super) fn frame_block_own(&mut self, own: bool) -> Outcome {
        let Some(facts) = self.frame_facts() else {
            return Outcome::refused(key!("reason-frame-none"));
        };
        let forks = |list: &[ew_api::output::Uuid]| list.iter().map(|u| u.to_string()).collect::<Vec<_>>();
        let cmd = if own {
            let Some(item) = facts.own_block_item else {
                return Outcome::refused(key!("reason-frame-no-block"));
            };
            let mut all = vec![item];
            all.extend(facts.frame_forks.iter().copied());
            AnyCommand {
                name: "form.make_own".to_owned(),
                params: json!({"frame": facts.frame.to_string(), "forks": forks(&all)}),
            }
        } else {
            let Some(shared) = facts.shared_block.filter(|b| Some(*b) != facts.block) else {
                return Outcome::Done;
            };
            let mut item = facts.frame_item.clone();
            item["title_block"] = json!(shared.to_string());
            let mut params = json!({"copy": facts.frame.to_string(), "form": item});
            if !facts.frame_forks.is_empty() {
                params["forks"] = json!(forks(&facts.frame_forks));
            }
            AnyCommand {
                name: "form.edit".to_owned(),
                params,
            }
        };
        let out = self.run_commit(&cmd);
        self.props.key = None;
        out
    }

    /// `frame.own_page` (Give this page its own frame): a copy of the frame for the page in view alone, which its Frame
    /// row then names (`form.make_own`).
    pub(crate) fn act_frame_own_page(&mut self) -> Outcome {
        let Some(facts) = self.frame_facts() else {
            return Outcome::refused(key!("reason-frame-none"));
        };
        let Some(page) = facts.page else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let out = self.run_commit(&AnyCommand {
            name: "form.make_own".to_owned(),
            params: json!({"page": page.to_string(), "forks": [facts.own_frame_item.to_string()]}),
        });
        self.props.key = None;
        out
    }

    /// `frame.adopt {template}` (Use the frames of a template…): the frames, title blocks, frame defaults, the lists'
    /// frame, the fields of its own and the company data of the template in the folder `template`, previewed on the
    /// card (`frames.adopt`, Bulk) and applied in one step. In the window without a template a form chooses one of the
    /// templates of the list.
    pub(crate) fn act_frame_adopt(&mut self, p: &Map<String, Value>) -> Outcome {
        let template = match string_param(p, "template") {
            Err(e) => return e,
            Ok(t) => t.map(str::to_owned),
        };
        let in_form = self.in_form_of(FRAME_ADOPT);
        let Some(template) = template.filter(|t| !t.is_empty()) else {
            if self.mode == Mode::Gui && !in_form {
                let options: Vec<(String, String)> = self
                    .settings
                    .templates
                    .iter()
                    .map(|t| (t.dir.clone(), t.name.clone()))
                    .collect();
                if options.is_empty() {
                    return Outcome::refused(key!("reason-frame-no-templates"));
                }
                let value = options[0].0.clone();
                return self.open_dialog(super::Dialog::Form(super::FormDialog {
                    action: FRAME_ADOPT,
                    title_key: key!("action-frame-adopt"),
                    ok_key: key!("dialog-preview"),
                    fields: vec![FormField {
                        label: None,
                        name: "template".into(),
                        label_key: key!("frame-template"),
                        choices: &super::choices::NONE,
                        options,
                        value,
                    }],
                    error: None,
                    table: None,
                }));
            }
            return failed(key!("error-missing-param"), &["template"]);
        };
        let params = match ew_api::Session::adopt_frames_params(std::path::Path::new(&template)) {
            Ok(p) => p,
            Err(e) => {
                let message = strings::format(key!("error-frame-template"), &[&template, &e.to_string()]);
                return if in_form {
                    self.form_failed(FRAME_ADOPT, message)
                } else {
                    Outcome::Failed { message }
                };
            }
        };
        if in_form {
            self.dialog = None;
        }
        self.preview_host_card("frames.adopt", params, key!("reason-frame-adopt"))
    }
}

/// Use the frames of a template…'s action.
const FRAME_ADOPT: &str = "frame.adopt";
