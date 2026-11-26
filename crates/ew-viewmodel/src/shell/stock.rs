// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock dialog (doc 07 §2.5, Project ▸ Edit stock…): the wire, ferrule and marker parts the shop keeps in
//! its order of preference, each moved up or down or removed, a part added by its number, the starter's generic
//! stock added, and what the cut list cuts to; Apply is one `workshop.stock`, one undo step.

use serde_json::{Map, Value, json};

use super::{Dialog, Outcome, ShellModel, failed, string_param};
use crate::{key, strings};

/// The stock dialog's state.
#[derive(Clone, PartialEq, Debug)]
pub struct StockDialog {
    /// The stock in its order of preference.
    pub rows: Vec<ew_api::StockRow>,
    /// The library documents the parts added from a library need.
    pub docs: Vec<Value>,
    /// The cut increment in millimetres as typed; empty: none (5 mm).
    pub cut: String,
    /// The refusal of the last gesture, in words.
    pub error: Option<String>,
}

impl ShellModel {
    /// How many parts the project's workshop stock holds (the project's Properties say so beside Edit stock…).
    pub fn workshop_stock_count(&self) -> Option<usize> {
        Some(self.project.as_ref()?.session.stock_rows().len())
    }

    /// `workshop.edit_stock` (Project ▸ Edit stock…): opens the stock dialog on the project's stock.
    pub(crate) fn act_workshop_edit_stock(&mut self) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let dialog = StockDialog {
            rows: project.session.stock_rows(),
            docs: Vec::new(),
            cut: project.session.stock_cut(),
            error: None,
        };
        self.open_dialog(Dialog::WorkshopStock(Box::new(dialog)))
    }

    /// The open stock dialog.
    fn stock_dialog(&mut self) -> Result<&mut StockDialog, Outcome> {
        match &mut self.dialog {
            Some(Dialog::WorkshopStock(d)) => Ok(d),
            _ => Err(Outcome::refused(key!("reason-no-stock-dialog"))),
        }
    }

    /// `workshop.stock_row {row, op}`: moves row `row` (1-based) `up` or `down`, or `remove`s it.
    pub(crate) fn act_workshop_stock_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let row = match p
            .get("row")
            .and_then(Value::as_u64)
            .and_then(|r| usize::try_from(r).ok())
        {
            Some(r) if r >= 1 => r - 1,
            _ => return failed(key!("error-bad-param"), &["row"]),
        };
        let op = match string_param(p, "op") {
            Ok(Some(o)) => o.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["op"]),
            Err(e) => return e,
        };
        let d = match self.stock_dialog() {
            Ok(d) => d,
            Err(e) => return e,
        };
        if row >= d.rows.len() {
            return failed(key!("error-bad-param"), &["row"]);
        }
        match op.as_str() {
            "up" if row > 0 => d.rows.swap(row, row - 1),
            "down" if row + 1 < d.rows.len() => d.rows.swap(row, row + 1),
            "up" | "down" => {}
            "remove" => {
                d.rows.remove(row);
            }
            _ => return failed(key!("error-bad-param"), &["op"]),
        }
        d.error = None;
        Outcome::Done
    }

    /// `workshop.stock_add {part}`: adds the part numbered `part` (a wire, ferrule or marker, held or from a library)
    /// after the stock; refused in words under the table for another part, one already in it, or none of that number.
    pub(crate) fn act_workshop_stock_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let number = match string_param(p, "part") {
            Ok(Some(n)) => n.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["part"]),
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let found = project.session.stock_part(&number);
        let d = match self.stock_dialog() {
            Ok(d) => d,
            Err(e) => return e,
        };
        match found {
            Ok((row, docs)) => {
                if d.rows.iter().any(|r| r.copy == row.copy) {
                    let text = strings::format(key!("stock-twice"), &[&row.number]);
                    d.error = Some(text.clone());
                    return Outcome::Failed { message: text };
                }
                d.rows.push(row);
                d.docs.extend(docs);
                d.error = None;
                Outcome::Done
            }
            Err(why) => {
                let text = strings::render(&why);
                d.error = Some(text.clone());
                Outcome::Failed { message: text }
            }
        }
    }

    /// `workshop.stock_cut {text}`: what the cut list cuts to, in millimetres; empty: 5 mm.
    pub(crate) fn act_workshop_stock_cut(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Ok(t) => t.unwrap_or_default().to_owned(),
            Err(e) => return e,
        };
        let d = match self.stock_dialog() {
            Ok(d) => d,
            Err(e) => return e,
        };
        d.cut = text;
        d.error = None;
        Outcome::Done
    }

    /// Add the generic stock inside the open dialog: the starter's ferrules and markers it lacks join the rows.
    pub(crate) fn stock_dialog_add_generic(&mut self) -> Option<Outcome> {
        let (rows, docs) = self.project.as_ref()?.session.generic_stock_rows();
        let Ok(d) = self.stock_dialog() else { return None };
        let before = d.rows.len();
        for r in rows {
            if !d.rows.iter().any(|x| x.copy == r.copy) {
                d.rows.push(r);
            }
        }
        d.docs.extend(docs);
        if d.rows.len() == before {
            return Some(Outcome::refused(key!("reason-workshop-generic-held")));
        }
        Some(Outcome::Done)
    }

    /// `workshop.stock_apply` (Apply): the dialog's stock and cut increment as one `workshop.stock`; a cut increment
    /// that is no length stays in the dialog with its words.
    pub(crate) fn act_workshop_stock_apply(&mut self) -> Outcome {
        let d = match self.stock_dialog() {
            Ok(d) => d.clone(),
            Err(e) => return e,
        };
        let cut = d.cut.trim();
        let cut = if cut.is_empty() {
            Value::Null
        } else {
            match ew_api::output::parse_mm(cut).filter(|n| *n > 0) {
                Some(n) => json!(n),
                None => {
                    let text = strings::format(key!("stock-cut-length"), &[cut]);
                    if let Ok(d) = self.stock_dialog() {
                        d.error = Some(text.clone());
                    }
                    return Outcome::Failed { message: text };
                }
            }
        };
        let stock: Vec<Value> = d
            .rows
            .iter()
            .map(|r| json!({ "copy": r.copy.to_string() }))
            .collect();
        let cmd = ew_api::AnyCommand {
            name: "workshop.stock".into(),
            params: json!({ "stock": stock, "copies": d.docs, "cut_increment": cut }),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.dialog = None;
        } else if let (Outcome::Failed { message }, Ok(d)) = (&out, self.stock_dialog()) {
            d.error = Some(message.clone());
        }
        out
    }
}
