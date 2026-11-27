// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock in Parts (doc 07 §2.5, §5.10). Project ▸ Edit stock…, the package dialog's Workshop stock… and the
//! project's Properties (`parts.stock`) open Parts on In this project narrowed to Workshop stock: its parts in its order
//! of preference, under a heading per kind that says the rule. On the sheet of a wire, a ferrule, a marker or a gland
//! the part is added, moved up or down or taken out (`parts.stock {op}`), each one `workshop.stock`, one undo step; Use
//! this stock for new projects keeps it as the window's setting `shop_stock`.

use ew_api::parts::PartScope;
use ew_api::stock::StockChange;
use serde_json::{Map, Value, json};

use super::parts_place::Named;
use super::{Dialog, Outcome, ShellModel, failed, string_param};
use crate::{key, strings};

/// Where the open part stands in the project's workshop stock.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StockPlace {
    /// It stands in it: its place among the stock's parts of its kind (from 1), and how many of its kind there are.
    In {
        /// Its place, from 1: the first is taken first.
        place: usize,
        /// How many parts of its kind the stock holds.
        of: usize,
    },
    /// It is of a kind the stock holds and is not in it.
    Out,
}

/// The rule of the stock's kind `kind` (as the file spells it) in words, the heading's second half ("the first that
/// fits"); `None` for a kind the stock does not hold.
pub fn stock_rule(kind: &str) -> Option<&'static str> {
    Some(strings::text(match kind {
        "wire" => key!("parts-stock-rule-wire"),
        "ferrule" => key!("parts-stock-rule-ferrule"),
        "marker" => key!("parts-stock-rule-marker"),
        "gland" => key!("parts-stock-rule-gland"),
        _ => return None,
    }))
}

impl ShellModel {
    /// How many parts the project's workshop stock holds (the project's Properties say so beside Edit stock…).
    pub fn workshop_stock_count(&self) -> Option<usize> {
        Some(self.project.as_ref()?.session.stock().len())
    }

    /// Where the open part of Parts stands in the open project's workshop stock; `None` without a project, or for a part
    /// of a kind the stock does not hold.
    pub fn open_stock_place(&self) -> Option<StockPlace> {
        let project = self.project.as_ref()?;
        let open = self.parts.as_deref()?.sheet.as_ref()?;
        if open.sheet.new {
            return None;
        }
        let kind = serde_json::from_value(json!(open.sheet.text("kind"))).ok()?;
        if !ew_api::stock::holds(&kind) {
            return None;
        }
        Some(match project.session.stock_place(open.sheet.copy) {
            Some((place, of)) => StockPlace::In { place, of },
            None => StockPlace::Out,
        })
    }

    /// `parts.stock {op?, part?, to?}` (Project ▸ Edit stock…, the Workshop stock view, module documentation): without
    /// `op` Parts opens on the Workshop stock; `add`, `remove` and `move` (to place `to` among its kind) change it by the
    /// open part, or the part `part` names as `parts.open` names it; `generic` adds the starter's generic stock;
    /// `default` keeps the stock, in its order, for new projects.
    pub(crate) fn act_parts_stock(&mut self, p: &Map<String, Value>) -> Outcome {
        let op = match string_param(p, "op") {
            Err(e) => return e,
            Ok(op) => op.map(str::to_owned),
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let change = match op.as_deref() {
            None => {
                // The package dialog's Workshop stock… leads away from it.
                if matches!(self.dialog, Some(Dialog::Package(_))) {
                    self.dialog = None;
                }
                self.enter_place();
                return self.dispatch("parts.find", &json!({ "scope": PartScope::Stock.word() }));
            }
            Some("generic") => {
                let out = self.act_workshop_add_generic();
                self.parts_refresh();
                return out;
            }
            Some("default") => {
                let items = project.session.stock_items();
                self.settings.shop_stock = items.iter().map(|(l, i)| format!("{l}/{i}")).collect();
                self.save_settings();
                self.notify(
                    strings::format(key!("parts-stock-kept"), &[&items.len().to_string()]),
                    false,
                );
                return Outcome::Done;
            }
            Some(op) => op.to_owned(),
        };
        if !self.is_writable() {
            return Outcome::refused(key!("reason-read-only"));
        }
        // The part: the one `part` names, else the open one.
        let (copy, source) = if p.contains_key("part") {
            match self.named_part(p) {
                Ok(Some(Named::Held(c))) => (c, None),
                Ok(Some(Named::Library(l, i))) => (
                    ew_api::output::v5::lib_copy_id::<ew_api::symbols::schema::Part>(l, i).as_uuid(),
                    Some((l, i)),
                ),
                Ok(None) => return failed(key!("error-bad-param"), &["part"]),
                Err(out) => return out,
            }
        } else {
            match self.parts.as_deref().and_then(|p| p.sheet.as_ref()) {
                Some(open) if !open.sheet.new => (open.sheet.copy, open.sheet.source),
                _ => return Outcome::refused(key!("reason-no-part-open")),
            }
        };
        let change = match change.as_str() {
            "add" => StockChange::Add { copy, source },
            "remove" => StockChange::Remove(copy),
            _ => match p.get("to").and_then(Value::as_u64).and_then(|t| usize::try_from(t).ok()) {
                Some(to) if to >= 1 => StockChange::Move { copy, to },
                _ => return failed(key!("error-missing-param"), &["to"]),
            },
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let cmd = match project.session.stock_command(change) {
            Ok(cmd) => cmd,
            Err(reason) => return Outcome::Refused { reason },
        };
        let out = self.run_commit(&cmd);
        self.parts_refresh();
        out
    }
}
