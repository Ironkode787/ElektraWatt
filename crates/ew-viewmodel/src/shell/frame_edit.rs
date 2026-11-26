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

use super::{Mode, Outcome, Prompt, ShellModel, failed, string_param};
use crate::key;
use crate::properties::{
    Category, EditRoute, FieldReadOnly, FieldType, PropRow, PropertySheet, Provenance, RowAction, RowValue,
};
use crate::strings;
use crate::tools::frame::{FrameChoice, FrameTool};

/// The kind of the Properties section of the frame (its row keys are `frame/<row>`).
pub const FRAME_KIND: &str = "frame";

/// A chosen cell's Text row: its label.
pub const TEXT_ROW: &str = "frame/text";
/// A chosen cell's Shows row: the value it prints.
pub const SHOWS_ROW: &str = "frame/shows";
/// A chosen cell's Text size row.
pub const SIZE_ROW: &str = "frame/size";
/// A chosen cell's Alignment row.
pub const ALIGN_ROW: &str = "frame/align";

/// The alignments a cell takes, by the stored word.
const ALIGNS: &[(&str, crate::strings::Key<'static>)] = &[
    ("left", key!("frame-align-left")),
    ("centre", key!("frame-align-centre")),
    ("right", key!("frame-align-right")),
];

/// What a cell's Shows offers besides the paths of the title block: a field of the project's own, New field of your
/// own…, the company logo, A picture… and Text only, each by the value of its choice.
const SHOWS_NEW_FIELD: &str = "new_field";
const SHOWS_PICTURE: &str = "picture";
const SHOWS_TEXT_ONLY: &str = "text_only";
const LOGO: &str = "project.logo";

/// The text of a multi-language text in the language the project prints in, else neutral.
fn label_text(ml: &Value) -> String {
    match ml {
        Value::String(s) => s.clone(),
        Value::Object(m) => m.values().find_map(Value::as_str).unwrap_or_default().to_owned(),
        _ => String::new(),
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
        (self.tools.tool.name() == "frame").then_some(&self.frame_choice)
    }

    /// `frame.edit` (Page ▸ Frame ▸ Edit frame, a double click on the title block or the rulers, the title block's
    /// menu): the Edit frame tool on the page in view, or, while it is active, Done.
    pub(crate) fn act_frame_edit(&mut self) -> Outcome {
        if self.tools.tool.name() == "frame" {
            return self.dismiss_tool();
        }
        let Some(facts) = self.frame_facts() else {
            return Outcome::refused(key!("reason-frame-none"));
        };
        self.frame_choice = FrameChoice::Frame;
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
        self.frame_choice = choice.clone();
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
        if self.tools.tool.name() != "frame" {
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
                header.subtitle = Some(strings::format(
                    key!("frame-cell-where"),
                    &[&facts.block_pages.len().to_string()],
                ));
                let label = item.get("label").map(label_text).unwrap_or_default();
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
                let mm = |nm: i64| crate::canvas::mm1(nm);
                vec![
                    row(
                        "text",
                        text(key!("frame-row-text")),
                        FieldType::Text,
                        label.clone(),
                        json!(label),
                        true,
                    ),
                    row(
                        "shows",
                        text(key!("frame-row-shows")),
                        FieldType::Enum { options: Vec::new() },
                        shows.clone(),
                        json!(shows),
                        true,
                    ),
                    row(
                        "size",
                        text(key!("frame-row-size")),
                        FieldType::Enum { options: Vec::new() },
                        style_name,
                        json!(style),
                        true,
                    ),
                    row(
                        "align",
                        text(key!("frame-row-align")),
                        FieldType::Enum {
                            options: ALIGNS.iter().map(|(a, _)| (*a).to_owned()).collect(),
                        },
                        align_words,
                        json!(align),
                        true,
                    ),
                    row(
                        "typed_in",
                        text(key!("frame-row-typed-in")),
                        FieldType::Text,
                        text(key!("frame-typed-in")),
                        Value::Null,
                        false,
                    ),
                    row(
                        "size_mm",
                        text(key!("frame-row-size-mm")),
                        FieldType::Text,
                        strings::format(key!("frame-size-mm"), &[&mm(cell.size.0), &mm(cell.size.1)]),
                        Value::Null,
                        false,
                    ),
                ]
            }
            FrameChoice::Frame | FrameChoice::Rulers => {
                header.title = text(key!("frame-title"));
                header.subtitle = Some(strings::format(
                    key!("frame-pages"),
                    &[&facts.frame_pages.len().to_string()],
                ));
                let sheet = &facts.frame_item["sheet"];
                let mm = |v: &Value| crate::canvas::mm1(v.as_i64().unwrap_or(0));
                let block = if facts.block_local {
                    text(key!("frame-block-own"))
                } else {
                    text(key!("frame-block-shared"))
                };
                let mut rows = vec![
                    row(
                        "paper",
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
                        "title_block",
                        text(key!("frame-row-title-block")),
                        FieldType::Text,
                        block,
                        Value::Null,
                        false,
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
                        "rulers",
                        text(key!("frame-row-rulers")),
                        FieldType::Text,
                        format!(
                            "{}, {}",
                            axis(c, key!("frame-axis-letters"), key!("frame-axis-numbers")),
                            axis(r, key!("frame-axis-letters"), key!("frame-axis-numbers"))
                        ),
                        Value::Null,
                        false,
                    ));
                }
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
        if row == TEXT_ROW {
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
                self.edit_block(
                    move |item| {
                        for i in &cells {
                            let cell = Self::cell_mut(item, *i)?;
                            match row.as_str() {
                                TEXT_ROW if text.trim().is_empty() => {
                                    cell.as_object_mut().map(|m| m.remove("label"));
                                }
                                TEXT_ROW => cell["label"] = json!(text),
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

    /// The double click on a chosen cell: its Text row of Properties opens for typing.
    pub(super) fn frame_type(&mut self) -> Outcome {
        if self.mode == Mode::Headless {
            return Outcome::Done;
        }
        self.begin_edit(TEXT_ROW, RowAction::Edit, None)
    }
}
