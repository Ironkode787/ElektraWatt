// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip editor (WP-S2.07 part 1, §6): the Terminal strips navigator in table mode, opened by `strip.edit`, and its
//! row gestures `strip.insert`, `strip.delete_rows`, `strip.move` and `strip.add_accessory` (`strip.unbridge` lives with
//! the bridge rule in `bridge.rs`).
//!
//! **One gesture, one command.** Every gesture compiles through `Session::strip_edit` (the only door below the shell,
//! §4.2) into one `terminal.insert`, `terminal.move` or `fields.set`, committed directly with the toast and one undo
//! step (D12); deleting terminal rows is one `edit.delete` that always opens the host card (it removes their wires and
//! bridges). The shell names no schema type: rows are read from the navigator's table, records through the session's
//! query by their ID text.
//!
//! **Context** (§6.9): the Terminal strips navigator is open and focused (headless: open); otherwise the gestures refuse
//! `reason-not-strip-editor`.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::output::{NavKind, NavNode, NavRole, NavTable, StripEdit, StripEditError, Uuid};
use serde_json::{Map, Value};

use super::{Mode, Outcome, ShellModel, failed, flag, string_param};
use crate::navigators::table::{Chooser, TableMode};
use crate::strings;

/// The strip a gesture acts on and the rows it acts on, in display order.
struct Rows {
    /// The table.
    table: Arc<NavTable>,
    /// The acted rows' node indices (the marked rows, else the cursor row), in node order.
    rows: Vec<usize>,
}

impl Rows {
    fn node(&self, i: usize) -> &NavNode {
        &self.table.nodes[i]
    }

    /// The strip node of row `i` (itself for a strip row).
    fn strip_of(&self, i: usize) -> Option<usize> {
        let n = self.node(i);
        match n.role {
            NavRole::Strip => Some(i),
            _ => n.parent.map(|p| p as usize),
        }
    }

    /// The children of strip node `s` in node order (its rows in mounting order, D2, D3).
    fn children(&self, s: usize) -> Vec<usize> {
        (0..self.table.nodes.len())
            .filter(|i| self.table.nodes[*i].parent == u32::try_from(s).ok())
            .collect()
    }

    /// The text of row `i`'s cell in column `key`.
    fn cell(&self, i: usize, key: &str) -> &str {
        self.table
            .columns
            .iter()
            .position(|c| c.key == key)
            .and_then(|c| self.node(i).cells.get(c))
            .map_or("", String::as_str)
    }

    /// `<strip>:<designation>` of terminal row `i`.
    fn name(&self, i: usize) -> String {
        let strip = self.strip_of(i).map_or("", |s| self.node(s).label.as_str());
        format!("{strip}:{}", self.node(i).label)
    }
}

/// The first record of a node as a UUID.
fn record(node: &NavNode) -> Option<Uuid> {
    node.records.first().copied()
}

impl ShellModel {
    /// The context of the strip editor's gestures (§6.9): the Terminal strips navigator is open and focused (headless:
    /// open).
    pub(crate) fn strip_editor_shown(&self) -> Result<(), &'static str> {
        if !self.navigator_open(NavKind::Strips) {
            return Err("reason-not-strip-editor");
        }
        if self.mode != Mode::Headless && self.focused_navigator() != Some(NavKind::Strips) {
            return Err("reason-not-strip-editor");
        }
        Ok(())
    }

    /// The strip editor's table and acted rows: the marked rows, else the cursor row.
    fn strip_rows(&self) -> Result<Rows, Outcome> {
        if let Err(reason_key) = self.strip_editor_shown() {
            return Err(Outcome::Refused { reason_key });
        }
        let nav = self.generic(NavKind::Strips).ok_or(Outcome::Refused {
            reason_key: "reason-not-strip-editor",
        })?;
        let table = Arc::clone(nav.table().ok_or(Outcome::Refused {
            reason_key: "reason-no-nav-row",
        })?);
        let mut rows: Vec<usize> = nav
            .marked()
            .iter()
            .filter_map(|k| nav.node_of(k).map(|(i, _)| i as usize))
            .collect();
        if rows.is_empty()
            && let Some((i, _)) = nav.cursor_key().and_then(|k| nav.node_of(&k))
        {
            rows.push(i as usize);
        }
        rows.sort_unstable();
        if rows.is_empty() {
            return Err(Outcome::Refused {
                reason_key: "reason-no-nav-row",
            });
        }
        Ok(Rows { table, rows })
    }

    /// The devices of the window selection (D9): a device itself, a function's device, a representation's functions'
    /// devices; each once, in selection order.
    pub(super) fn selected_record_devices(&self) -> Vec<Uuid> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let mut out: Vec<Uuid> = Vec::new();
        let push = |u: Uuid, out: &mut Vec<Uuid>| {
            if !out.contains(&u) {
                out.push(u);
            }
        };
        for item in &self.tools.selection.items {
            let text = item.0.to_string();
            if let Ok(id) = text.parse()
                && q.devices().get(id).is_some()
            {
                push(item.0, &mut out);
                continue;
            }
            if let Ok(id) = text.parse()
                && let Some(f) = q.functions().get(id)
            {
                if let Some(d) = f.device {
                    push(d.as_uuid(), &mut out);
                }
                continue;
            }
            if let Ok(id) = text.parse()
                && let Some(rep) = q.reps().get(id)
            {
                for f in &rep.functions {
                    if let Some(d) = q.functions().get(*f).and_then(|f| f.device) {
                        push(d.as_uuid(), &mut out);
                    }
                }
            }
        }
        out
    }

    /// The strip of a device: itself when it is a strip, its strip when it is a terminal.
    fn strip_of_device(&self, d: Uuid) -> Option<Uuid> {
        let project = self.project.as_ref()?;
        let q = project.session.query();
        let device = q.devices().get(d.to_string().parse().ok()?)?;
        if device.aspects.strip.is_some() {
            return Some(d);
        }
        Some(device.aspects.terminal.as_ref()?.strip.as_uuid())
    }

    /// `strip.edit {strip?}` (Edit ▸ Strip editor, §6.1): the strip given (a printed tag), else the selected strip,
    /// else the strip of the selected terminal, else the strip of the navigator's cursor row; shows the Terminal strips
    /// navigator focused in table mode, filtered to the strip, its cursor on the strip's first row.
    pub(super) fn act_strip_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        if !self.has_project() {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        }
        let given = match string_param(p, "strip") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        // The strip from the selection or the cursor, read before the navigator opens.
        let selected = self
            .selected_record_devices()
            .into_iter()
            .find_map(|d| self.strip_of_device(d));
        let from_cursor = self.generic(NavKind::Strips).and_then(|n| {
            let (i, node) = n.node_of(&n.cursor_key()?)?;
            let t = n.table()?;
            let s = match node.role {
                NavRole::Strip => i as usize,
                _ => node.parent? as usize,
            };
            record(&t.nodes[s])
        });
        let mut show = Map::new();
        show.insert("kind".into(), Value::from(NavKind::Strips.word()));
        let out = self.act_navigator_show(&show);
        if out != Outcome::Done {
            return out;
        }
        self.sync_navigators();
        let Some(table) = self
            .generic(NavKind::Strips)
            .and_then(|n| n.table().map(Arc::clone))
        else {
            return Outcome::Done;
        };
        let strip_node = |pred: &dyn Fn(&NavNode) -> bool| {
            table
                .nodes
                .iter()
                .position(|n| n.role == NavRole::Strip && pred(n))
        };
        let at = match &given {
            Some(text) => {
                let folded = text.to_lowercase();
                match strip_node(&|n| {
                    n.label == *text
                        || n.label.to_lowercase() == folded
                        || record(n).is_some_and(|r| r.to_string() == *text)
                }) {
                    Some(i) => i,
                    None => return failed("error-nav-key", &[text, NavKind::Strips.word()]),
                }
            }
            None => match selected.or(from_cursor) {
                Some(s) => match strip_node(&|n| record(n) == Some(s)) {
                    Some(i) => i,
                    None => {
                        return Outcome::Refused {
                            reason_key: "reason-no-strip",
                        };
                    }
                },
                None => {
                    return Outcome::Refused {
                        reason_key: "reason-no-strip",
                    };
                }
            },
        };
        let tag = table.nodes[at].label.to_lowercase();
        let first = (0..table.nodes.len())
            .find(|i| table.nodes[*i].parent == u32::try_from(at).ok())
            .map(|i| table.nodes[i].key.clone());
        let generation = self.tools.selection.generation;
        if let Some(n) = self.generic_mut(NavKind::Strips) {
            if n.table_mode.is_none() {
                n.table_mode = Some(TableMode::default());
            }
            n.filter = format!("strip:{tag}");
            n.marked.clear();
            n.cursor = first.or_else(|| Some(table.nodes[at].key.clone()));
            // The cursor stays where the editor put it: the selection was seen.
            n.applied = Some(generation);
        }
        Outcome::Done
    }

    /// `strip.insert {count?, designation?}` (Insert, §6.4): `count` terminals before the cursor terminal row (on an
    /// accessory row or the strip row, before the next terminal; with none after the cursor, at the end), copying the
    /// reference terminal (D14); one `terminal.insert`, the toast, the cursor on the first new row.
    pub(super) fn act_strip_insert(&mut self, p: &Map<String, Value>) -> Outcome {
        let rows = match self.strip_rows() {
            Ok(r) => r,
            Err(e) => return e,
        };
        let count = match p.get("count") {
            None => 1,
            Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n >= 1) {
                Some(n) => n,
                None => return failed("error-bad-param", &["count"]),
            },
        };
        let designation = match string_param(p, "designation") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        if designation.is_some() && count != 1 {
            return failed("error-bad-param", &["designation"]);
        }
        let cursor = self
            .generic(NavKind::Strips)
            .and_then(|n| n.node_of(&n.cursor_key()?).map(|(i, _)| i as usize))
            .unwrap_or(rows.rows[0]);
        let Some(s) = rows.strip_of(cursor) else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        let Some(strip) = record(rows.node(s)) else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        // The next terminal row at or after the cursor.
        let before_row = rows
            .children(s)
            .into_iter()
            .find(|i| *i >= cursor && rows.node(*i).role == NavRole::Terminal);
        let before = before_row.and_then(|i| record(rows.node(i)));
        let edit = StripEdit::Insert {
            strip,
            before,
            count,
            designation,
        };
        let cmd = match self.strip_command(&edit) {
            Ok(c) => c,
            Err(e) => return e,
        };
        let names: Vec<String> = cmd.params["terminals"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t["designation"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let out = self.run_commit(&cmd);
        if out != Outcome::Done {
            return out;
        }
        let n = count.to_string();
        let text = match before_row {
            Some(i) => {
                let key = if count == 1 {
                    "toast-strip-inserted-one"
                } else {
                    "toast-strip-inserted"
                };
                strings::format(key, &[&n, &rows.name(i)])
            }
            None => {
                let key = if count == 1 {
                    "toast-strip-appended-one"
                } else {
                    "toast-strip-appended"
                };
                strings::format(key, &[&n, &rows.node(s).label])
            }
        };
        self.notify(text, true);
        // The cursor on the first new row.
        let strip_key = rows.node(s).key.clone();
        self.sync_navigators();
        if let Some(first) = names.first() {
            self.strip_cursor_to(&strip_key, |n| n.role == NavRole::Terminal && n.label == *first);
        }
        Outcome::Done
    }

    /// Moves the Terminal strips cursor to the first child of strip `strip_key` that `pred` accepts.
    fn strip_cursor_to(&mut self, strip_key: &str, pred: impl Fn(&NavNode) -> bool) {
        let Some(n) = self.generic_mut(NavKind::Strips) else {
            return;
        };
        let Some(t) = n.table().map(Arc::clone) else {
            return;
        };
        let Some(s) = t.nodes.iter().position(|x| x.key == strip_key) else {
            return;
        };
        let found = t
            .nodes
            .iter()
            .rfind(|x| x.parent == u32::try_from(s).ok() && pred(x))
            .map(|x| x.key.clone());
        if let Some(k) = found {
            n.cursor = Some(k);
        }
    }

    /// The one command of a strip edit, or the gesture's refusal.
    fn strip_command(&self, edit: &StripEdit) -> Result<ew_api::AnyCommand, Outcome> {
        let Some(project) = &self.project else {
            return Err(Outcome::Refused {
                reason_key: "reason-no-project",
            });
        };
        if !self.is_writable() {
            return Err(Outcome::Refused {
                reason_key: "reason-read-only",
            });
        }
        project.session.strip_edit(edit).map_err(|e| match e {
            StripEditError::Refused(reason_key) => Outcome::Refused { reason_key },
            StripEditError::Invalid(message) => Outcome::Failed { message },
            StripEditError::Ambiguous(choices) => failed(
                "error-accessory-part-ambiguous",
                &[&choices
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")],
            ),
        })
    }

    /// `strip.delete_rows` (Ctrl+Delete, §6.4, D12): terminal rows are one `edit.delete` of their devices, always the
    /// card; accessory rows one `fields.set` per strip, committed with the toast; a mix is refused.
    pub(super) fn act_strip_delete_rows(&mut self) -> Outcome {
        let rows = match self.strip_rows() {
            Ok(r) => r,
            Err(e) => return e,
        };
        let terminals: Vec<usize> = rows
            .rows
            .iter()
            .copied()
            .filter(|i| rows.node(*i).role == NavRole::Terminal)
            .collect();
        let accessories: Vec<usize> = rows
            .rows
            .iter()
            .copied()
            .filter(|i| rows.node(*i).role == NavRole::Accessory)
            .collect();
        if !terminals.is_empty() && !accessories.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-strip-mixed-rows",
            };
        }
        if !terminals.is_empty() {
            if !self.is_writable() {
                return Outcome::Refused {
                    reason_key: "reason-read-only",
                };
            }
            let targets: Vec<Value> = terminals
                .iter()
                .filter_map(|i| record(rows.node(*i)))
                .map(|d| serde_json::json!({"kind": "device", "id": d.to_string()}))
                .collect();
            return self.preview_host_card(
                "edit.delete",
                serde_json::json!({ "targets": targets }),
                "reason-strip-delete",
            );
        }
        if accessories.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        }
        // One RemoveAccessories per strip, in row order.
        let mut edits: Vec<StripEdit> = Vec::new();
        for i in &accessories {
            let (Some(s), Some(index)) = (
                rows.strip_of(*i).and_then(|s| record(rows.node(s))),
                crate::navigators::table::accessory_index(rows.node(*i)),
            ) else {
                continue;
            };
            match edits.iter_mut().find_map(|e| match e {
                StripEdit::RemoveAccessories { strip, indexes } if *strip == s => Some(indexes),
                _ => None,
            }) {
                Some(indexes) => indexes.push(index),
                None => edits.push(StripEdit::RemoveAccessories {
                    strip: s,
                    indexes: vec![index],
                }),
            }
        }
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
        }
        let cmd = match project.session.strip_edits(&edits) {
            Ok(c) => c,
            Err(StripEditError::Refused(reason_key)) => return Outcome::Refused { reason_key },
            Err(StripEditError::Invalid(message)) => return Outcome::Failed { message },
            Err(StripEditError::Ambiguous(_)) => return failed("error-bad-param", &["rows"]),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            let n = accessories.len();
            let text = if n == 1 {
                strings::text("toast-accessories-removed-one").to_owned()
            } else {
                strings::format("toast-accessories-removed", &[&n.to_string()])
            };
            self.notify(text, true);
            if let Some(n) = self.generic_mut(NavKind::Strips) {
                n.marked.clear();
            }
        }
        out
    }

    /// `strip.move {step?, before?}` (Alt+Up, Alt+Down, §6.4, D4): the marked terminal rows of one strip (else the
    /// cursor row) one terminal up (`step` −1) or down (+1), or before the terminal `before` (a printed designation of
    /// the same strip, or `end`); one `terminal.move` with the toast; the marks and the cursor follow the rows.
    pub(super) fn act_strip_move(&mut self, p: &Map<String, Value>) -> Outcome {
        let rows = match self.strip_rows() {
            Ok(r) => r,
            Err(e) => return e,
        };
        if rows.rows.iter().any(|i| rows.node(*i).role != NavRole::Terminal) {
            return Outcome::Refused {
                reason_key: "reason-strip-mixed-rows",
            };
        }
        let strips: BTreeSet<usize> = rows.rows.iter().filter_map(|i| rows.strip_of(*i)).collect();
        let [s] = strips.into_iter().collect::<Vec<_>>()[..] else {
            return Outcome::Refused {
                reason_key: "reason-bridge-strips",
            };
        };
        let order: Vec<usize> = rows
            .children(s)
            .into_iter()
            .filter(|i| rows.node(*i).role == NavRole::Terminal)
            .collect();
        let moved: Vec<usize> = order.iter().copied().filter(|i| rows.rows.contains(i)).collect();
        let at = |i: usize| order.iter().position(|x| *x == i).unwrap_or(0);
        let before_text = match string_param(p, "before") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        let step = p.get("step").and_then(Value::as_i64);
        let before: Option<usize> = match (before_text, step) {
            (Some(t), _) if t == "end" => None,
            (Some(t), _) => match order.iter().copied().find(|i| rows.node(*i).label == t) {
                Some(i) => Some(i),
                None => return failed("error-nav-key", &[&t, NavKind::Strips.word()]),
            },
            (None, Some(-1)) => {
                let first = at(moved[0]);
                if first == 0 {
                    return Outcome::Refused {
                        reason_key: "reason-strip-move-none",
                    };
                }
                Some(order[first - 1])
            }
            (None, Some(1)) => {
                let last = at(moved[moved.len() - 1]);
                // The first terminal after the last moved one that is not moved; the moved ones go after it.
                let Some(next) = order[last + 1..].iter().position(|i| !moved.contains(i)) else {
                    return Outcome::Refused {
                        reason_key: "reason-strip-move-none",
                    };
                };
                order.get(last + 1 + next + 1).copied()
            }
            (None, Some(_)) => return failed("error-bad-param", &["step"]),
            (None, None) => return failed("error-missing-param", &["step"]),
        };
        let edit = StripEdit::Move {
            terminals: moved.iter().filter_map(|i| record(rows.node(*i))).collect(),
            before: before.and_then(|i| record(rows.node(i))),
        };
        let cmd = match self.strip_command(&edit) {
            Ok(c) => c,
            Err(e) => return e,
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            let n = moved.len();
            let strip = rows.node(s).label.clone();
            let text = if n == 1 {
                strings::format("toast-strip-moved-one", &[&n.to_string(), &strip])
            } else {
                strings::format("toast-strip-moved", &[&n.to_string(), &strip])
            };
            self.notify(text, true);
        }
        out
    }

    /// `strip.add_accessory {kind?, part?, before?}` (Edit ▸ Add accessory, §6.5, D5): an accessory of `kind` (default
    /// `end_clamp`) after the cursor row (a terminal: its `seq`; an accessory: its `after_seq`; the strip row: 0), or
    /// with `before` directly before the cursor terminal row (FT-600), its
    /// part parsed by D7 or the one project copy of the kind's part kinds; several: the chooser list in the window, the
    /// error listing them headless.
    pub(super) fn act_strip_add_accessory(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Err(reason_key) = self.strip_editor_shown() {
            return Outcome::Refused { reason_key };
        }
        let kind = match string_param(p, "kind") {
            Ok(v) => v.unwrap_or("end_clamp").to_owned(),
            Err(e) => return e,
        };
        if !ew_api::output::ACCESSORY_KINDS.contains(&kind.as_str()) {
            return failed(
                "error-nav-value",
                &["kind", &kind, &ew_api::output::ACCESSORY_KINDS.join(", ")],
            );
        }
        let part = match string_param(p, "part") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        let Some(nav) = self.generic(NavKind::Strips) else {
            return Outcome::Refused {
                reason_key: "reason-not-strip-editor",
            };
        };
        let Some(table) = nav.table().map(Arc::clone) else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        let Some((cursor, _)) = nav
            .cursor_key()
            .and_then(|k| nav.node_of(&k).map(|(i, n)| (i as usize, n.key.clone())))
        else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        let rows = Rows {
            table,
            rows: vec![cursor],
        };
        let Some(s) = rows.strip_of(cursor) else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        let Some(strip) = record(rows.node(s)) else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        let pos = rows.cell(cursor, "pos");
        let seq_of = |i: usize| -> u32 {
            rows.cell(i, "pos")
                .split('.')
                .next()
                .and_then(|x| x.parse().ok())
                .unwrap_or(0)
        };
        // FT-600: `before` on a terminal row places the accessory directly before it (after the terminal before it,
        // behind the accessories already there; 0 before the strip's first terminal), so table mode reaches the
        // start of a strip. On an accessory row or the strip row it changes nothing.
        let before = flag(p, "before") && rows.node(cursor).role == NavRole::Terminal;
        let after_seq: u32 = match rows.node(cursor).role {
            NavRole::Strip => 0,
            NavRole::Terminal if before => rows
                .children(s)
                .into_iter()
                .take_while(|i| *i != cursor)
                .filter(|i| rows.node(*i).role == NavRole::Terminal)
                .last()
                .map_or(0, seq_of),
            _ => seq_of(cursor),
        };
        let edit = StripEdit::AddAccessory {
            strip,
            after_seq,
            kind: kind.clone(),
            part,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
        }
        let cmd = match project.session.strip_edit(&edit) {
            Ok(c) => c,
            Err(StripEditError::Refused(reason_key)) => return Outcome::Refused { reason_key },
            Err(StripEditError::Invalid(message)) => return Outcome::Failed { message },
            Err(StripEditError::Ambiguous(choices)) => {
                let list = choices
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                // The window: the chooser list at the row (16a §6.22); its pick dispatches this action again.
                if self.mode != Mode::Headless {
                    let row = rows.node(cursor).key.clone();
                    let items = choices
                        .into_iter()
                        .map(|c| (c.text, c.value.as_str().unwrap_or_default().to_owned()))
                        .collect();
                    if let Some(m) = self
                        .generic_mut(NavKind::Strips)
                        .and_then(|n| n.table_mode.as_mut())
                    {
                        m.chooser = Some(Box::new(Chooser { row, kind, items }));
                        return Outcome::Done;
                    }
                }
                return failed("error-accessory-part-ambiguous", &[&list]);
            }
        };
        let out = self.run_commit(&cmd);
        if out != Outcome::Done {
            return out;
        }
        if let Some(m) = self
            .generic_mut(NavKind::Strips)
            .and_then(|n| n.table_mode.as_mut())
        {
            m.chooser = None;
        }
        let word = ew_api::output::accessory_kind_word(&kind);
        let text = match rows.node(cursor).role {
            NavRole::Strip => strings::format("toast-accessory-added-first", &[word, &rows.node(s).label]),
            NavRole::Terminal if before => {
                strings::format("toast-accessory-added-before", &[word, &rows.name(cursor)])
            }
            NavRole::Terminal => strings::format("toast-accessory-added", &[word, &rows.name(cursor)]),
            _ => strings::format(
                "toast-accessory-added",
                &[word, &format!("{} {}", rows.node(s).label, pos)],
            ),
        };
        self.notify(text, true);
        let strip_key = rows.node(s).key.clone();
        self.sync_navigators();
        let prefix = format!("{after_seq}.");
        let cells_pos = |n: &NavNode, t: &NavTable| {
            t.columns
                .iter()
                .position(|c| c.key == "pos")
                .and_then(|c| n.cells.get(c))
                .is_some_and(|x| x.starts_with(&prefix))
        };
        let table = self
            .generic(NavKind::Strips)
            .and_then(|n| n.table().map(Arc::clone));
        if let Some(t) = table {
            self.strip_cursor_to(&strip_key, |n| n.role == NavRole::Accessory && cells_pos(n, &t));
        }
        Outcome::Done
    }
}
