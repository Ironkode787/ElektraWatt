// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! List columns… and Rows of your own… (`report_block.columns`, `report_block.rows`, doc 09 §3.8): what the list on
//! show prints, each in one form, written to its report block in one undo step (`fields.set_many`).
//!
//! - **List columns…**: the caption and the legend, and a table of every column of the list's report kind, one row each
//!   (its heading in the window's language): a tick to print it, a heading of its own (empty: the kind's), its width (a
//!   share of the table's width) and its order (a number; empty after the numbered ones, in the table's order).
//! - **Rows of your own…**: a table of the printed columns, one row per row the list holds that the model does not, and
//!   three empty rows to fill; a row left empty is left out. A cell of a column the list does not print stays as it is.
//!
//! The forms' table carries its cells under the window's words for its columns, so a script sets the same members with
//! `fields.set` on the block (paths `columns`, `caption`, `legend`, `added_rows`). Neither form shows on a page that is
//! not a list's, and a table of contents takes no rows of its own.

use ew_api::AnyCommand;
use ew_api::output::ListLook;
use serde_json::{Map, Value, json};

use super::{Dialog, FormDialog, FormField, FormRow, FormTable, Outcome, ShellModel, TICK, TICKED};
use crate::key;
use crate::strings::{self, Key};

/// List columns…'s action.
const COLUMNS: &str = "report_block.columns";
/// Rows of your own…'s action.
const ROWS: &str = "report_block.rows";
/// The empty rows Rows of your own… offers under the rows the list holds.
const SPARE_ROWS: usize = 3;

/// The words of List columns…'s table columns, in its order: print, heading, width, order.
fn column_words() -> [String; 4] {
    [
        key!("list-columns-print"),
        key!("list-columns-heading"),
        key!("list-columns-width"),
        key!("list-columns-order"),
    ]
    .map(|k| strings::text(k).to_owned())
}

/// The headings of the columns `look` prints, in its order, each made unique by a number where two read alike (a wire
/// list's two "CP"), so that a form table can key its cells by them.
fn printed_headings(look: &ListLook) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for c in look.columns.iter().filter(|c| c.shown) {
        let base = if c.heading.is_empty() {
            c.label.clone()
        } else {
            c.heading.clone()
        };
        let mut name = base.clone();
        let mut n = 1;
        while out.iter().any(|(_, h)| *h == name) {
            n += 1;
            name = format!("{base} {n}");
        }
        out.push((c.key.clone(), name));
    }
    out
}

/// The cells of row `i` of a form table's parameter, by column word.
fn row_cells(rows: &[Value], i: usize) -> Map<String, Value> {
    rows.get(i)
        .and_then(|r| r.get("values"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// A text field of a form.
fn text_field(name: &'static str, label_key: Key<'static>, value: String) -> FormField {
    FormField {
        name,
        label_key,
        choices: &super::choices::NONE,
        options: Vec::new(),
        value,
    }
}

/// One edit of `fields.set_many` on the block `block`.
fn edit(block: &str, path: &str, value: Value) -> Value {
    json!({"kind": "report_block", "targets": [block], "path": path, "value": value})
}

/// A text member's value: the text, or `null` (none) when it is empty.
fn text_or_null(text: &str) -> Value {
    if text.is_empty() {
        Value::Null
    } else {
        Value::String(text.to_owned())
    }
}

impl ShellModel {
    /// Whether the page on show is a page of a list (a generated page).
    pub(crate) fn shows_list(&self) -> bool {
        self.page_rows()
            .get(self.page_index())
            .is_some_and(|r| r.generated)
    }

    /// The look of the list on show, its texts in the window's language; `None` on any other page.
    fn list_on_show(&self) -> Option<ListLook> {
        let page = self.focused_page_id()?;
        self.project
            .as_ref()?
            .session
            .list_look(page, strings::rule_language())
    }

    /// The folio of the page on show.
    fn folio_on_show(&self) -> String {
        self.page_rows()
            .get(self.page_index())
            .map(|r| r.folio.clone())
            .unwrap_or_default()
    }

    /// `edits` in one `fields.set_many`, then `toast`; a failure stays in the form of `action`.
    fn commit_look(&mut self, action: &str, edits: Vec<Value>, toast: String) -> Outcome {
        if !edits.is_empty() {
            let cmd = AnyCommand {
                name: "fields.set_many".to_owned(),
                params: json!({ "edits": edits }),
            };
            match self.run_commit(&cmd) {
                Outcome::Done => self.notify(toast, true),
                Outcome::Failed { message } => return self.form_failed(action, message),
                other => return other,
            }
        }
        self.form_done(action, Outcome::Done)
    }

    /// `report_block.columns {caption?, legend?, columns?}` (Output ▸ List columns…): without a parameter the
    /// window opens the form of the list on show (see the module documentation); its Apply writes the caption, the
    /// legend and the printed columns in one step, a field left empty clearing its member. Refused in words: no list on
    /// show, a width that is not a whole number of 1 or more, an order that is not a whole number, no column ticked.
    pub(crate) fn act_report_block_columns(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(look) = self.list_on_show() else {
            return Outcome::refused(key!("reason-no-list-page"));
        };
        let words = column_words();
        if p.is_empty() && !self.in_form_of(COLUMNS) {
            let rows = look
                .columns
                .iter()
                .enumerate()
                .map(|(i, c)| FormRow {
                    key: c.label.clone(),
                    cells: vec![
                        if c.shown { TICKED } else { "" }.to_owned(),
                        c.heading.clone(),
                        c.width.to_string(),
                        if c.shown {
                            (i + 1).to_string()
                        } else {
                            String::new()
                        },
                    ],
                })
                .collect();
            return self.open_dialog(Dialog::Form(FormDialog {
                action: COLUMNS,
                title_key: key!("list-columns-title"),
                ok_key: key!("dialog-apply"),
                fields: vec![
                    text_field("caption", key!("form-list-caption"), look.caption.clone()),
                    text_field("legend", key!("form-list-legend"), look.legend.clone()),
                ],
                error: None,
                table: Some(FormTable {
                    name: "columns",
                    heading_key: key!("list-columns-table"),
                    fixed: true,
                    key_label: key!("list-columns-column"),
                    add_key: key!("form-value-set-add"),
                    columns: vec![
                        (words[0].clone(), TICK.to_owned()),
                        (words[1].clone(), String::new()),
                        (words[2].clone(), String::new()),
                        (words[3].clone(), String::new()),
                    ],
                    rows,
                }),
            }));
        }
        // In the form a field left empty clears its member; given as parameters, a member not named keeps its value.
        let in_form = self.in_form_of(COLUMNS);
        let text = |name: &str, now: &str| match p.get(name) {
            Some(Value::String(s)) => s.trim().to_owned(),
            None if !in_form => now.to_owned(),
            _ => String::new(),
        };
        let (caption, legend) = (text("caption", &look.caption), text("legend", &look.legend));
        let block = look.block.to_string();
        let mut edits = Vec::new();
        if caption != look.caption {
            edits.push(edit(&block, "caption", text_or_null(&caption)));
        }
        if legend != look.legend {
            edits.push(edit(&block, "legend", text_or_null(&legend)));
        }
        let toast = strings::format(key!("toast-list-columns"), &[&self.folio_on_show()]);
        let Some(table) = p.get("columns").and_then(Value::as_array) else {
            return self.commit_look(COLUMNS, edits, toast);
        };
        // Each of the kind's columns by its row of the form: printed or not, its heading, width and order.
        let mut chosen: Vec<(u32, usize, Value)> = Vec::new();
        for (i, c) in look.columns.iter().enumerate() {
            let cells = row_cells(table, i);
            let cell = |w: &str| {
                cells
                    .get(w)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            };
            if cell(&words[0]) != TICKED {
                continue;
            }
            let width = cell(&words[2]);
            let weight = if width.is_empty() {
                c.width
            } else {
                match width.parse::<u32>() {
                    Ok(w) if w >= 1 => w,
                    _ => {
                        let message = strings::format(key!("error-list-width"), &[&c.label, &width]);
                        return self.form_failed(COLUMNS, message);
                    }
                }
            };
            let order = cell(&words[3]);
            let place = if order.is_empty() {
                u32::MAX
            } else {
                match order.parse::<u32>() {
                    Ok(o) => o,
                    Err(_) => {
                        let message = strings::format(key!("error-list-order"), &[&c.label, &order]);
                        return self.form_failed(COLUMNS, message);
                    }
                }
            };
            let mut column = json!({"key": c.key, "weight": weight});
            let heading = cell(&words[1]);
            if !heading.is_empty() {
                column["heading"] = Value::String(heading);
            }
            chosen.push((place, i, column));
        }
        if chosen.is_empty() {
            return self.form_failed(COLUMNS, strings::text(key!("error-list-no-column")).to_owned());
        }
        chosen.sort_by_key(|(place, i, _)| (*place, *i));
        let columns: Vec<Value> = chosen.into_iter().map(|(_, _, c)| c).collect();
        // The columns as the list prints them now, to write only what changed.
        let now: Vec<Value> = look
            .columns
            .iter()
            .filter(|c| c.shown)
            .map(|c| {
                let mut column = json!({"key": c.key, "weight": c.width});
                if !c.heading.is_empty() {
                    column["heading"] = Value::String(c.heading.clone());
                }
                column
            })
            .collect();
        if columns != now {
            edits.push(edit(&block, "columns", Value::Array(columns)));
        }
        self.commit_look(COLUMNS, edits, toast)
    }

    /// `report_block.rows {rows?}` (Output ▸ Rows of your own…): without a parameter the window opens the form
    /// of the list on show (see the module documentation); its Apply writes the rows the list holds that the model does
    /// not, in one step, the empty ones left out. Refused in words: no list on show, a table of contents.
    pub(crate) fn act_report_block_rows(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(look) = self.list_on_show() else {
            return Outcome::refused(key!("reason-no-list-page"));
        };
        if look.contents {
            return Outcome::refused(key!("reason-list-contents"));
        }
        let headings = printed_headings(&look);
        if p.is_empty() && !self.in_form_of(ROWS) {
            let rows = look
                .added
                .iter()
                .map(|cells| {
                    headings
                        .iter()
                        .map(|(key, _)| {
                            cells
                                .iter()
                                .find(|(k, _)| k == key)
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default()
                        })
                        .collect::<Vec<String>>()
                })
                .chain(std::iter::repeat_n(
                    vec![String::new(); headings.len()],
                    SPARE_ROWS,
                ))
                .enumerate()
                .map(|(i, cells)| FormRow {
                    key: (i + 1).to_string(),
                    cells,
                })
                .collect();
            return self.open_dialog(Dialog::Form(FormDialog {
                action: ROWS,
                title_key: key!("list-rows-title"),
                ok_key: key!("dialog-apply"),
                fields: Vec::new(),
                error: None,
                table: Some(FormTable {
                    name: "rows",
                    heading_key: key!("list-rows-table"),
                    fixed: true,
                    key_label: key!("list-rows-row"),
                    add_key: key!("form-value-set-add"),
                    columns: headings.iter().map(|(_, h)| (h.clone(), String::new())).collect(),
                    rows,
                }),
            }));
        }
        let table = p
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut rows: Vec<Value> = Vec::new();
        for i in 0..table.len() {
            let cells = row_cells(&table, i);
            // The cells of the columns the list does not print stay as the row holds them.
            let mut out: Map<String, Value> = look
                .added
                .get(i)
                .into_iter()
                .flatten()
                .filter(|(k, _)| !headings.iter().any(|(key, _)| key == k))
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            for (key, heading) in &headings {
                if let Some(text) = cells.get(heading).and_then(Value::as_str).map(str::trim)
                    && !text.is_empty()
                {
                    out.insert(key.clone(), Value::String(text.to_owned()));
                }
            }
            if !out.is_empty() {
                rows.push(json!({ "cells": out }));
            }
        }
        let n = rows.len();
        let value = if rows.is_empty() {
            Value::Null
        } else {
            Value::Array(rows)
        };
        let now: Vec<Value> = look
            .added
            .iter()
            .map(|cells| {
                let map: Map<String, Value> = cells
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect();
                json!({ "cells": map })
            })
            .collect();
        let same = match &value {
            Value::Array(rows) => *rows == now,
            _ => now.is_empty(),
        };
        let edits = if same {
            Vec::new()
        } else {
            vec![edit(&look.block.to_string(), "added_rows", value)]
        };
        let folio = self.folio_on_show();
        let toast = match n {
            0 => strings::format(key!("toast-list-rows-none"), &[&folio]),
            1 => strings::format(key!("toast-list-rows-one"), &[&folio]),
            _ => strings::format(key!("toast-list-rows"), &[&folio, &n.to_string()]),
        };
        self.commit_look(ROWS, edits, toast)
    }
}
