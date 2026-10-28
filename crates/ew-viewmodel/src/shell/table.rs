// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Table mode in the shell (WP-S2.06 part 3, §8): `navigator.table` and the fourteen `table.*` actions over a
//! navigator's table, the gestures (a cell, a column over the marked rows, fill down, pattern fill, paste, clear), their
//! one compiled command, the policy and the table card.
//!
//! **One gesture, one command** (D11): every gesture builds its cells in (row display order, column visible order),
//! marks the same-record conflicts (D13), compiles the applicable cells into `fields.set` or `fields.set_many`, previews
//! that command once and decides by FR-UI-044's rules as built for the property grid, plus rule 3: any invalid or
//! ambiguous cell opens the card. A direct gesture commits the kept preview (the window `Session::commit`, headless a
//! one-line command log through `Session::run_script`) and shows the toast "Set <columns> on <n> <rows>".
//!
//! **The card** (§8.7) holds the gesture: `table.resolve` picks a reading of an ambiguous cell and `table.drop` removes
//! a cell, each compiling and previewing again; Apply is refused while a cell blocks; a stale card retargets its rows
//! on the current table and shows a fresh card (a second Apply).

use std::sync::Arc;

use ew_api::output::{InputError, NavKind, NavTable, Uuid};
use ew_api::{AnyCommand, Exposure, Submit};
use serde_json::{Map, Value};

use super::navigation::bump;
use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::navigators::Navigator;
use crate::navigators::paste::{parse_tsv, write_tsv};
use crate::navigators::pattern::Template;
use crate::navigators::table::{
    CellEdit, CellValue, Compiled, Gesture, GestureCell, GestureKind, MemberBase, TableMode, cell_reason,
    cell_text, compile, gesture_rows, mark_conflicts, pattern_texts, route_type, row_targets, rows_word,
};
use crate::properties::{
    Card, CardOrigin, Decision, EditRoute, FieldReadOnly, PendingEdit, RowValue, find_row, policy,
};
use crate::strings;
use crate::tools::PreviewState;
use ew_api::output::FieldType;

/// A navigator's table as a gesture reads it.
struct TableView {
    /// The table.
    table: Arc<NavTable>,
    /// The rows on show (node indices), in display order.
    rows: Vec<u32>,
    /// The column indices on show, in order.
    shown: Vec<usize>,
    /// The cursor row's position in `rows`.
    cursor_row: Option<usize>,
    /// The cursor column (a column index).
    cursor_col: Option<usize>,
    /// The rows a gesture acts on (node indices, display order): the marked ones, else the cursor row.
    acted: Vec<u32>,
}

impl TableView {
    /// The 1-based display position of node `i`.
    fn number(&self, i: u32) -> usize {
        self.rows.iter().position(|r| *r == i).map_or(0, |p| p + 1)
    }
}

/// The value a clear gives a field of type `ty` (§8.3): `null` for an optional, the empty text for a text, an
/// identifier or a language text, else nothing ("cannot be cleared").
fn cleared(ty: Option<&FieldType>) -> Option<Value> {
    match ty {
        Some(FieldType::Optional(_)) => Some(Value::Null),
        Some(FieldType::Text | FieldType::Ident | FieldType::MlText) => Some(Value::String(String::new())),
        _ => None,
    }
}

/// A 1-based unsigned parameter.
fn index_param(p: &Map<String, Value>, name: &str) -> Result<usize, Outcome> {
    match p.get(name) {
        None => Err(failed("error-missing-param", &[name])),
        Some(v) => v
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
            .filter(|i| *i >= 1)
            .ok_or_else(|| failed("error-bad-param", &[name])),
    }
}

/// A list-of-text parameter: `Ok(None)` when absent.
fn list_param(p: &Map<String, Value>, name: &str) -> Result<Option<Vec<String>>, Outcome> {
    match p.get(name) {
        None => Ok(None),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| x.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .map(Some)
            .ok_or_else(|| failed("error-bad-param", &[name])),
        Some(_) => Err(failed("error-bad-param", &[name])),
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // State

    /// The navigator of `kind` in table mode, or the refusal "switch to the table first".
    fn table_nav(&self, kind: NavKind) -> Result<&Navigator, Outcome> {
        match self.generic(kind) {
            Some(n) if n.table_mode.is_some() => Ok(n),
            Some(_) => Err(Outcome::Refused {
                reason_key: "reason-table-off",
            }),
            None if kind == NavKind::Pages => Err(Outcome::Refused {
                reason_key: "reason-table-off",
            }),
            None => Err(failed("error-nav-closed", &[kind.word()])),
        }
    }

    /// The table mode of navigator `kind`, mutable.
    fn mode_mut(&mut self, kind: NavKind) -> Option<&mut TableMode> {
        self.generic_mut(kind).and_then(|n| n.table_mode.as_mut())
    }

    /// The navigator from the `kind` parameter (else the focused one), in table mode.
    fn table_kind(&self, p: &Map<String, Value>) -> Result<NavKind, Outcome> {
        let kind = self.kind_param(p)?;
        self.table_nav(kind)?;
        Ok(kind)
    }

    /// What a gesture of navigator `kind` reads.
    fn table_view(&self, kind: NavKind) -> Option<TableView> {
        let n = self.generic(kind)?;
        let mode = n.table_mode.as_ref()?;
        let table = Arc::clone(n.table()?);
        let rows: Vec<u32> = n.visible().iter().map(|r| r.node).collect();
        let cursor = n.cursor_key();
        let cursor_row = cursor.and_then(|k| rows.iter().position(|i| table.nodes[*i as usize].key == k));
        Some(TableView {
            shown: mode.shown_columns(&table),
            cursor_col: mode.cursor_column(&table),
            acted: gesture_rows(n),
            table,
            rows,
            cursor_row,
        })
    }

    /// A cell of node `i`, column `col`, with `text` parsed for the column's route (§8.2, §8.3); a read-only cell is
    /// invalid with its reason.
    fn text_cell(&self, view: &TableView, i: u32, col: usize, text: &str) -> GestureCell {
        let value = match self.cell_route(view, i, col) {
            Err(reason) => CellValue::Invalid(reason),
            Ok((kind, path)) => match &self.project {
                None => CellValue::Invalid(strings::text("reason-no-project").to_owned()),
                Some(project) => match project.session.parse_field_input(&kind, &path, text) {
                    Ok(parsed) => CellValue::Valid(parsed.value),
                    Err(InputError::Invalid(message)) => CellValue::Invalid(message),
                    Err(InputError::Ambiguous { choices }) => CellValue::Ambiguous(choices),
                },
            },
        };
        self.cell(view, i, col, text, value)
    }

    /// The route of an editable cell, or why it is read-only.
    fn cell_route(&self, view: &TableView, i: u32, col: usize) -> Result<(String, String), String> {
        let node = &view.table.nodes[i as usize];
        match cell_reason(&view.table, node, col) {
            Some(reason) => Err(reason),
            None => view.table.columns[col]
                .route
                .clone()
                .ok_or_else(|| "no command yet".to_owned()),
        }
    }

    /// A gesture cell of node `i`, column `col`, holding `value`.
    fn cell(&self, view: &TableView, i: u32, col: usize, text: &str, value: CellValue) -> GestureCell {
        let node = &view.table.nodes[i as usize];
        let column = &view.table.columns[col];
        let route = match &value {
            CellValue::Invalid(_) => None,
            _ => self.cell_route(view, i, col).ok(),
        };
        let targets = route
            .as_ref()
            .map(|(k, _)| row_targets(node, k))
            .unwrap_or_default();
        GestureCell {
            row: node.key.clone(),
            row_label: node.label.clone(),
            row_number: view.number(i),
            column: column.key.clone(),
            column_label: column.label.clone(),
            text: text.to_owned(),
            route,
            targets,
            value,
            conflict: None,
            dropped: false,
        }
    }

    /// A target's stored member and the project's source language, for a composed leaf or a language text (one
    /// property sheet of that record).
    fn member_base(&self, kind: &str, target: Uuid, member: &str) -> MemberBase {
        let Some(project) = &self.project else {
            return MemberBase {
                value: None,
                language: "en".to_owned(),
            };
        };
        bump(&self.calls, |c| c.property_sheet += 1);
        let sheet = project.session.property_sheet(&[target]);
        let value = find_row(&sheet, &format!("{kind}/{member}")).and_then(|(_, r)| match &r.value {
            RowValue::One { json, .. } => Some(json.clone()),
            _ => None,
        });
        let language = sheet
            .languages
            .first()
            .cloned()
            .or_else(|| match &value {
                Some(Value::Object(o)) => o.keys().next().cloned(),
                _ => None,
            })
            .unwrap_or_else(|| "en".to_owned());
        MemberBase { value, language }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The commit path (§8.3)

    /// Compiles, previews and decides a gesture: a direct commit with the toast, the card, or the error. `card` forces
    /// the card with that reason when the policy would commit (a rebuilt card).
    fn run_gesture(&mut self, mut gesture: Gesture, card: Option<&'static str>) -> Outcome {
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
        }
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        mark_conflicts(&mut gesture);
        let compiled = compile(&gesture, &mut |k, t, m| self.member_base(k, t, m));
        let blocking = gesture.blocking();
        let Some(c) = compiled else {
            if blocking == 0 {
                // Nothing applies (every cell dropped): the gesture ends.
                self.close_edit(gesture.nav);
                return Outcome::Done;
            }
            self.show_table_card(
                gesture,
                rev,
                None,
                Err(strings::text("table-nothing").to_owned()),
                Default::default(),
                0,
                "reason-table-invalid",
            );
            return Outcome::Done;
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(meta) = project.session.command_meta(c.name) else {
            return failed("error-unknown-action", &[c.name]);
        };
        if meta.exposure != Exposure::Edit {
            return Outcome::Refused {
                reason_key: "reason-human-only",
            };
        }
        let cmd = AnyCommand {
            name: c.name.to_owned(),
            params: c.params.clone(),
        };
        let submit = Submit::from(super::mode_origin(self.mode, &self.origin_file));
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        bump(calls, |x| x.preview_with += 1);
        let result = project.session.preview_with(&cmd, submit);
        let derived = match &result {
            Ok(p) => project.session.derived_diff(p),
            Err(_) => Default::default(),
        };
        let changes = result.as_ref().map_or(0, |p| p.diff().len());
        let decision = if blocking > 0 {
            Decision::Card("reason-table-invalid")
        } else {
            policy(
                c.targets,
                c.identifier,
                derived.tags.len(),
                meta.cost,
                result.is_ok(),
            )
        };
        match (decision, result) {
            (Decision::Error, Err(e)) => {
                let message = self.refusal(&e, None);
                if gesture.kind == GestureKind::Edit
                    && let Some(m) = self.mode_mut(gesture.nav)
                {
                    m.error = Some(message.clone());
                }
                Outcome::Failed { message }
            }
            (Decision::Direct, Ok(preview)) if card.is_none() => {
                self.props.card = None;
                let state = PreviewState {
                    base_rev: rev,
                    command: c.params.clone(),
                    name: c.name.to_owned(),
                    result: Ok(preview),
                    derived,
                };
                self.commit_gesture(state, &c, &gesture)
            }
            (decision, result) => {
                let reason = match decision {
                    Decision::Card(r) => r,
                    _ => card.unwrap_or("reason-rebuilt"),
                };
                let result = result.map_err(|e| self.refusal(&e, None));
                self.show_table_card(gesture, rev, Some(&c), result, derived, changes, reason);
                Outcome::Done
            }
        }
    }

    /// Shows the table card of `gesture`.
    #[allow(clippy::too_many_arguments)]
    fn show_table_card(
        &mut self,
        gesture: Gesture,
        rev: u64,
        compiled: Option<&Compiled>,
        result: Result<ew_api::Preview, String>,
        derived: ew_api::DerivedDiff,
        changes: usize,
        reason: &'static str,
    ) {
        let (name, params, targets) = compiled.map_or_else(
            || (String::new(), Value::Null, 0),
            |c| (c.name.to_owned(), c.params.clone(), c.targets),
        );
        let kind = gesture
            .cells
            .iter()
            .find_map(|c| c.route.as_ref().map(|r| r.0.clone()))
            .unwrap_or_default();
        let edit = PendingEdit {
            row: String::new(),
            kind,
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: name.clone(),
            params: params.clone(),
            typed: String::new(),
            label: gesture.column_labels(),
        };
        self.close_edit(gesture.nav);
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name,
                result,
                derived,
            },
            edit,
            targets,
            changes,
            reason,
            origin: CardOrigin::Table { gesture },
        });
    }

    /// Commits a kept, fresh preview of a gesture's command and shows the toast.
    fn commit_gesture(&mut self, state: PreviewState, c: &Compiled, gesture: &Gesture) -> Outcome {
        let out = self.commit_kept(state, c.name, &c.params, None);
        match &out {
            Outcome::Done => {
                self.close_edit(gesture.nav);
                let rows = gesture.rows();
                self.notify(
                    strings::format(
                        "toast-set",
                        &[
                            &gesture.column_labels(),
                            &rows.to_string(),
                            &rows_word(gesture.nav, rows),
                        ],
                    ),
                    true,
                );
            }
            Outcome::Failed { message } if gesture.kind == GestureKind::Edit => {
                if let Some(m) = self.mode_mut(gesture.nav) {
                    m.error = Some(message.clone());
                }
            }
            _ => {}
        }
        out
    }

    /// Ends the cell edit and the pattern dialog of navigator `kind`.
    fn close_edit(&mut self, kind: NavKind) {
        if let Some(m) = self.mode_mut(kind) {
            m.edit = None;
            m.error = None;
            m.pattern = None;
        }
    }

    /// Apply on a table card (§8.7): refused while a cell blocks; a fresh card commits; a stale one retargets its rows
    /// on the current table and shows a fresh card.
    pub(super) fn apply_table_card(&mut self, card: Card) -> Outcome {
        let CardOrigin::Table { gesture } = &card.origin else {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        };
        let blocking = gesture.blocking();
        if blocking > 0 {
            self.props.card = Some(card);
            if blocking == 1 {
                return failed("error-table-blocking-one", &[]);
            }
            return failed("error-table-blocking", &[&blocking.to_string()]);
        }
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        if card.preview.base_rev == rev {
            if let Err(message) = &card.preview.result {
                return Outcome::Failed {
                    message: message.clone(),
                };
            }
            let Card {
                preview,
                edit,
                targets,
                origin,
                ..
            } = card;
            let CardOrigin::Table { gesture } = origin else {
                return Outcome::Done;
            };
            let c = Compiled {
                name: if edit.name == "fields.set" {
                    "fields.set"
                } else {
                    "fields.set_many"
                },
                params: edit.params,
                targets,
                identifier: false,
            };
            return self.commit_gesture(preview, &c, &gesture);
        }
        let CardOrigin::Table { gesture } = card.origin else {
            return Outcome::Done;
        };
        let gesture = self.retarget(gesture);
        self.run_gesture(gesture, Some("reason-rebuilt"))
    }

    /// A gesture's cells on the current table (a stale card): each row by its key, its records again; a row that is
    /// gone is invalid ("no row").
    fn retarget(&self, mut gesture: Gesture) -> Gesture {
        let Some(view) = self.table_view(gesture.nav) else {
            return gesture;
        };
        for cell in &mut gesture.cells {
            let Some((kind, _)) = cell.route.clone() else {
                continue;
            };
            match view.table.nodes.iter().find(|n| n.key == cell.row) {
                None => {
                    cell.value = CellValue::Invalid(strings::text("table-no-row").to_owned());
                    cell.route = None;
                    cell.targets.clear();
                }
                Some(node) => {
                    cell.targets = row_targets(node, &kind);
                    if cell.targets.is_empty() {
                        cell.value = CellValue::Invalid("no record of this kind".to_owned());
                        cell.route = None;
                    }
                }
            }
        }
        gesture
    }

    /// Cells of `view` over `rows` (node indices) in column `col`, each with `text`.
    fn column_cells(&self, view: &TableView, rows: &[u32], col: usize, text: &str) -> Vec<GestureCell> {
        rows.iter().map(|i| self.text_cell(view, *i, col, text)).collect()
    }

    /// A gesture of navigator `kind`.
    fn gesture(kind: GestureKind, nav: NavKind, cells: Vec<GestureCell>) -> Gesture {
        Gesture {
            kind,
            nav,
            cells,
            ignored: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The actions

    /// `navigator.table {kind, on?}`: switches a navigator between its tree (or flat list) and its table; without
    /// `on` it toggles.
    pub(super) fn act_navigator_table(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        // WP-S2.09 part 2 meets WP-S2.06 part 3: the Messages pane's rows are read-only findings with their own
        // header, detail block and quick-fix card, so it has no table mode.
        if kind == NavKind::Messages {
            return failed("error-nav-no-table", &[kind.word()]);
        }
        let now = self.generic(kind).is_some_and(|n| n.table_mode.is_some());
        let on = p.get("on").and_then(Value::as_bool).unwrap_or(!now);
        if kind == NavKind::Pages {
            if !on {
                self.pages_table = None;
            } else if self.pages_table.is_none() {
                let mut n = Navigator::new(NavKind::Pages);
                n.table_mode = Some(TableMode::default());
                self.pages_table = Some(n);
            }
            return Outcome::Done;
        }
        let Some(n) = self.generic_mut(kind) else {
            return failed("error-nav-closed", &[kind.word()]);
        };
        if !on {
            n.table_mode = None;
        } else if n.table_mode.is_none() {
            n.table_mode = Some(TableMode::default());
        }
        Outcome::Done
    }

    /// `table.columns {kind, show?, hide?, order?}`: the column chooser. `show` adds columns at the end, `hide`
    /// removes them (the key column stays), `order` puts the listed columns first, in that order (showing them), the
    /// other shown ones after them.
    pub(super) fn act_table_columns(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let (show, hide, order) = match (
            list_param(p, "show"),
            list_param(p, "hide"),
            list_param(p, "order"),
        ) {
            (Ok(s), Ok(h), Ok(o)) => (s, h, o),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        let keys: Vec<String> = view.table.columns.iter().map(|c| c.key.clone()).collect();
        let key_column = view
            .table
            .columns
            .iter()
            .find(|c| c.key_column)
            .map(|c| c.key.clone());
        let mut current: Vec<String> = view
            .shown
            .iter()
            .map(|i| view.table.columns[*i].key.clone())
            .filter(|k| Some(k) != key_column.as_ref())
            .collect();
        let check = |k: &String| -> Result<(), Outcome> {
            if keys.contains(k) {
                Ok(())
            } else {
                Err(failed("error-nav-value", &["column", k, &keys.join(", ")]))
            }
        };
        for k in show.iter().flatten() {
            if let Err(e) = check(k) {
                return e;
            }
            if Some(k) != key_column.as_ref() && !current.contains(k) {
                current.push(k.clone());
            }
        }
        for k in hide.iter().flatten() {
            if let Err(e) = check(k) {
                return e;
            }
            if Some(k) == key_column.as_ref() {
                return failed("error-table-key-column", &[k]);
            }
            current.retain(|c| c != k);
        }
        if let Some(order) = order {
            let mut next: Vec<String> = Vec::new();
            for k in &order {
                if let Err(e) = check(k) {
                    return e;
                }
                if Some(k) != key_column.as_ref() && !next.contains(k) {
                    next.push(k.clone());
                }
            }
            next.extend(current.into_iter().filter(|k| !order.contains(k)));
            current = next;
        }
        if let Some(m) = self.mode_mut(kind) {
            m.columns = Some(current);
        }
        Outcome::Done
    }

    /// Moves the cell cursor from the parameters `row`, `column`, `step` and `across`; the cell edit ends (reverted).
    fn move_cell(&mut self, kind: NavKind, p: &Map<String, Value>) -> Result<(), Outcome> {
        let row = self.nav_key_param(kind, p, "row")?;
        let column = string_param(p, "column")?.map(str::to_owned);
        let step = match p.get("step") {
            None => None,
            Some(v) => Some(v.as_i64().ok_or_else(|| failed("error-bad-param", &["step"]))?),
        };
        let across = match p.get("across") {
            None => None,
            Some(v) => Some(v.as_i64().ok_or_else(|| failed("error-bad-param", &["across"]))?),
        };
        let Some(view) = self.table_view(kind) else {
            return Ok(());
        };
        if let Some(k) = row {
            self.set_cursor(kind, &k);
        } else if let Some(s) = step
            && !view.rows.is_empty()
        {
            let here = i64::try_from(view.cursor_row.unwrap_or(0)).unwrap_or(0);
            let last = i64::try_from(view.rows.len() - 1).unwrap_or(0);
            let at = usize::try_from(here.saturating_add(s).clamp(0, last)).unwrap_or(0);
            let key = view.table.nodes[view.rows[at] as usize].key.clone();
            self.set_cursor(kind, &key);
        }
        let mut col = view.cursor_col;
        if let Some(c) = column {
            match view.shown.iter().find(|i| view.table.columns[**i].key == c) {
                Some(i) => col = Some(*i),
                None => {
                    let shown: Vec<&str> = view
                        .shown
                        .iter()
                        .map(|i| view.table.columns[*i].key.as_str())
                        .collect();
                    return Err(failed("error-nav-value", &["column", &c, &shown.join(", ")]));
                }
            }
        } else if let Some(a) = across
            && !view.shown.is_empty()
        {
            let here = col
                .and_then(|c| view.shown.iter().position(|i| *i == c))
                .and_then(|x| i64::try_from(x).ok())
                .unwrap_or(0);
            let last = i64::try_from(view.shown.len() - 1).unwrap_or(0);
            let at = usize::try_from(here.saturating_add(a).clamp(0, last)).unwrap_or(0);
            col = Some(view.shown[at]);
        }
        let key = col.map(|c| view.table.columns[c].key.clone());
        if let Some(m) = self.mode_mut(kind) {
            if key.is_some() {
                m.column = key;
            }
            m.edit = None;
            m.error = None;
        }
        Ok(())
    }

    /// `table.cell {kind?, row?, column?, step?, across?}`: moves the cell cursor (arrows, Tab, Home, End).
    pub(super) fn act_table_cell(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        match self.move_cell(kind, p) {
            Ok(()) => Outcome::Done,
            Err(e) => e,
        }
    }

    /// Starts editing the cursor cell of navigator `kind` with `buffer` (else the cell's text); a read-only cell
    /// refuses with its reason.
    fn begin_cell_edit(&mut self, kind: NavKind, buffer: Option<String>) -> Outcome {
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        let (Some(r), Some(col)) = (view.cursor_row, view.cursor_col) else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        let i = view.rows[r];
        if let Err(reason) = self.cell_route(&view, i, col) {
            return failed(
                "error-table-read-only",
                &[&view.table.columns[col].label, &reason],
            );
        }
        let node = &view.table.nodes[i as usize];
        let edit = CellEdit {
            row: node.key.clone(),
            column: view.table.columns[col].key.clone(),
            buffer: buffer.unwrap_or_else(|| node.cells.get(col).cloned().unwrap_or_default()),
        };
        if let Some(m) = self.mode_mut(kind) {
            m.edit = Some(edit);
            m.error = None;
            m.pattern = None;
        }
        Outcome::Done
    }

    /// `table.edit {kind?, row?, column?}` (F2): edits the cursor cell (or the given one), the buffer starting with the
    /// cell's text.
    pub(super) fn act_table_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        if (p.contains_key("row") || p.contains_key("column"))
            && let Err(e) = self.move_cell(kind, p)
        {
            return e;
        }
        self.begin_cell_edit(kind, None)
    }

    /// `table.input {text}`: the whole buffer of the edited cell (typing a character starts editing the cursor cell
    /// with it), or the pattern dialog's pattern while that is open.
    pub(super) fn act_table_input(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["text"]),
            Ok(Some(t)) => t.to_owned(),
        };
        let Some(m) = self.mode_mut(kind) else {
            return Outcome::Done;
        };
        if m.pattern.is_some() {
            m.pattern = Some(text);
            m.error = None;
            return Outcome::Done;
        }
        if let Some(e) = &mut m.edit {
            e.buffer = text;
            m.error = None;
            return Outcome::Done;
        }
        self.begin_cell_edit(kind, Some(text))
    }

    /// `table.commit {all?}` (Enter, Ctrl+Enter while editing): the buffer for the cursor row, or (`all`) for every
    /// marked row; with the pattern dialog open, the pattern fill.
    pub(super) fn act_table_commit(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some(mode) = self.generic(kind).and_then(|n| n.table_mode.clone()) else {
            return Outcome::Done;
        };
        if let Some(pattern) = mode.pattern {
            return self.pattern_fill(kind, &pattern);
        }
        let Some(edit) = mode.edit else {
            return Outcome::Refused {
                reason_key: "reason-table-not-editing",
            };
        };
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        let Some(col) = view.table.columns.iter().position(|c| c.key == edit.column) else {
            return Outcome::Done;
        };
        let Some(row) = view.table.nodes.iter().position(|n| n.key == edit.row) else {
            self.close_edit(kind);
            return failed("error-nav-key", &[&edit.row, kind.word()]);
        };
        let row = u32::try_from(row).unwrap_or(u32::MAX);
        let all = flag(p, "all");
        // The cell's own text committed unchanged: nothing to write.
        if !all && view.table.nodes[row as usize].cells.get(col) == Some(&edit.buffer) {
            self.close_edit(kind);
            return Outcome::Done;
        }
        // A parse error stays in the field (§8.3).
        let first = self.text_cell(&view, row, col, &edit.buffer);
        if let CellValue::Invalid(message) = &first.value
            && first.route.is_none()
            && self.cell_route(&view, row, col).is_ok()
        {
            let message = message.clone();
            if let Some(m) = self.mode_mut(kind) {
                m.error = Some(message.clone());
            }
            return Outcome::Failed { message };
        }
        let rows = if all { view.acted.clone() } else { vec![row] };
        let cells = self.column_cells(&view, &rows, col, &edit.buffer);
        self.run_gesture(Self::gesture(GestureKind::Edit, kind, cells), None)
    }

    /// `table.set {kind, column, value, rows?, all?}`: sets a column's cells to a typed text: the listed rows, the
    /// marked rows (`all`), else the cursor row.
    pub(super) fn act_table_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let column = match string_param(p, "column") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["column"]),
            Ok(Some(c)) => c.to_owned(),
        };
        let value = match string_param(p, "value") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["value"]),
            Ok(Some(v)) => v.to_owned(),
        };
        let listed = match list_param(p, "rows") {
            Ok(l) => l,
            Err(e) => return e,
        };
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        let Some(col) = view.table.columns.iter().position(|c| c.key == column) else {
            let keys: Vec<&str> = view.table.columns.iter().map(|c| c.key.as_str()).collect();
            return failed("error-nav-value", &["column", &column, &keys.join(", ")]);
        };
        let rows: Vec<u32> = match listed {
            Some(keys) => {
                let mut out = Vec::new();
                for k in &keys {
                    match view
                        .rows
                        .iter()
                        .find(|i| view.table.nodes[**i as usize].key == *k)
                    {
                        Some(i) => out.push(*i),
                        None => return failed("error-nav-key", &[k, kind.word()]),
                    }
                }
                out.sort_by_key(|i| view.number(*i));
                out.dedup();
                out
            }
            None if flag(p, "all") => view.acted.clone(),
            None => view.cursor_row.map(|r| view.rows[r]).into_iter().collect(),
        };
        if rows.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        }
        let cells = self.column_cells(&view, &rows, col, &value);
        self.run_gesture(Self::gesture(GestureKind::Edit, kind, cells), None)
    }

    /// `table.fill_down` (Ctrl+D): the first marked row's cell of the cursor column into every marked row (§8.4).
    pub(super) fn act_table_fill_down(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        let marked = self.generic(kind).map_or(0, |n| n.marked().len());
        let Some(col) = view.cursor_col else {
            return Outcome::Done;
        };
        if marked < 2 || view.acted.len() < 2 {
            return Outcome::Refused {
                reason_key: "reason-table-mark-fill",
            };
        }
        let text = view.table.nodes[view.acted[0] as usize]
            .cells
            .get(col)
            .cloned()
            .unwrap_or_default();
        let cells = self.column_cells(&view, &view.acted, col, &text);
        self.run_gesture(Self::gesture(GestureKind::FillDown, kind, cells), None)
    }

    /// `table.pattern_fill {pattern?}` (Ctrl+Shift+D): without a pattern it opens the dialog; with one it fills the
    /// cursor column of the marked rows (§8.5).
    pub(super) fn act_table_pattern_fill(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        match string_param(p, "pattern") {
            Err(e) => e,
            Ok(Some(pattern)) => {
                let pattern = pattern.to_owned();
                self.pattern_fill(kind, &pattern)
            }
            Ok(None) => {
                if let Some(m) = self.mode_mut(kind) {
                    m.edit = None;
                    m.error = None;
                    m.pattern = Some(String::new());
                }
                Outcome::Done
            }
        }
    }

    /// The pattern fill of `pattern` over the marked rows (else the cursor row) in the cursor column.
    fn pattern_fill(&mut self, kind: NavKind, pattern: &str) -> Outcome {
        let template = Template::parse(pattern).and_then(|t| {
            let keys: Vec<&str> = self
                .generic(kind)
                .and_then(|n| n.table())
                .map(|t| t.columns.iter().map(|c| c.key.as_str()).collect())
                .unwrap_or_default();
            t.check_columns(&keys).map(|()| t)
        });
        let template = match template {
            Ok(t) => t,
            Err(e) => {
                let message = e.message();
                if let Some(m) = self.mode_mut(kind)
                    && m.pattern.is_some()
                {
                    m.error = Some(message.clone());
                }
                return Outcome::Failed { message };
            }
        };
        let (Some(view), Some(nav)) = (self.table_view(kind), self.generic(kind)) else {
            return Outcome::Done;
        };
        let Some(col) = view.cursor_col else {
            return Outcome::Done;
        };
        if view.acted.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        }
        let texts = pattern_texts(nav, &template);
        let cells: Vec<GestureCell> = view
            .acted
            .iter()
            .zip(texts)
            .map(|(i, t)| match t {
                Ok(text) => self.text_cell(&view, *i, col, &text),
                Err(e) => self.cell(&view, *i, col, "", CellValue::Invalid(e.message().to_owned())),
            })
            .collect();
        self.run_gesture(Self::gesture(GestureKind::PatternFill, kind, cells), None)
    }

    /// `table.paste {text?}` (Ctrl+V): the tab-separated text (else the clipboard's) from the cursor cell (§8.6). A
    /// first row naming at least half of the columns on show is a header and maps the columns by label; otherwise
    /// they map by position from the cursor column. A cell whose text equals the cell's own is unchanged and left out.
    pub(super) fn act_table_paste(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => match self.clip.text() {
                Some(t) => t.to_owned(),
                None => {
                    return Outcome::Refused {
                        reason_key: "reason-clipboard-empty",
                    };
                }
            },
        };
        let mut grid = match parse_tsv(&text) {
            Ok(g) => g,
            Err(e) => return Outcome::Failed { message: e.message() },
        };
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        let fold = |s: &str| s.trim().to_lowercase();
        let labels: Vec<String> = view
            .shown
            .iter()
            .map(|i| fold(&view.table.columns[*i].label))
            .collect();
        let first: Vec<String> = grid
            .first()
            .map(|r| r.iter().map(|c| fold(c)).collect())
            .unwrap_or_default();
        let matched = labels.iter().filter(|l| first.contains(l)).count();
        let header = matched > 0 && matched * 2 >= labels.len();
        let mut ignored = Vec::new();
        // Pasted column j → a column index on show, or `None` past the right edge.
        let map: Vec<Option<usize>> = if header {
            let names = grid.remove(0);
            let mut used: Vec<usize> = Vec::new();
            names
                .iter()
                .map(|n| {
                    let at = labels
                        .iter()
                        .position(|l| *l == fold(n))
                        .map(|k| view.shown[k])
                        .filter(|c| !used.contains(c));
                    match at {
                        Some(c) => used.push(c),
                        None => ignored.push(n.clone()),
                    }
                    at
                })
                .collect()
        } else {
            let start = view
                .cursor_col
                .and_then(|c| view.shown.iter().position(|i| *i == c))
                .unwrap_or(0);
            let width = grid.iter().map(Vec::len).max().unwrap_or(0);
            (0..width).map(|j| view.shown.get(start + j).copied()).collect()
        };
        let start = view.cursor_row.unwrap_or(0);
        let mut cells: Vec<GestureCell> = Vec::new();
        for (k, pasted) in grid.iter().enumerate() {
            let at = view.rows.get(start + k).copied();
            for (j, text) in pasted.iter().enumerate() {
                if header && map.get(j).copied().flatten().is_none() {
                    continue;
                }
                let col = map.get(j).copied().flatten();
                match (at, col) {
                    (Some(i), Some(c)) => {
                        if view.table.nodes[i as usize].cells.get(c) == Some(text) {
                            continue;
                        }
                        cells.push(self.text_cell(&view, i, c, text));
                    }
                    (at, col) => {
                        let reason = if at.is_none() {
                            "table-no-row"
                        } else {
                            "table-no-column"
                        };
                        cells.push(GestureCell {
                            row: at.map_or_else(String::new, |i| view.table.nodes[i as usize].key.clone()),
                            row_label: at
                                .map_or_else(String::new, |i| view.table.nodes[i as usize].label.clone()),
                            row_number: start + k + 1,
                            column: col.map_or_else(String::new, |c| view.table.columns[c].key.clone()),
                            column_label: col
                                .map_or_else(String::new, |c| view.table.columns[c].label.clone()),
                            text: text.clone(),
                            route: None,
                            targets: Vec::new(),
                            value: CellValue::Invalid(strings::text(reason).to_owned()),
                            conflict: None,
                            dropped: false,
                        });
                    }
                }
            }
        }
        if cells.is_empty() {
            // Nothing differs from the table: the paste writes nothing.
            return Outcome::Done;
        }
        let mut gesture = Self::gesture(GestureKind::Paste, kind, cells);
        gesture.ignored = ignored;
        self.run_gesture(gesture, None)
    }

    /// `table.copy` (Ctrl+C): the marked rows' cells on show (else the cursor row's) as tab-separated text with a
    /// header row of the column labels, onto the clipboard (§8.6).
    pub(super) fn act_table_copy(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        if view.acted.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        }
        let mut grid: Vec<Vec<String>> = vec![
            view.shown
                .iter()
                .map(|i| view.table.columns[*i].label.clone())
                .collect(),
        ];
        for i in &view.acted {
            let node = &view.table.nodes[*i as usize];
            grid.push(
                view.shown
                    .iter()
                    .map(|c| node.cells.get(*c).cloned().unwrap_or_default())
                    .collect(),
            );
        }
        let to_system = self.mode == super::Mode::Gui;
        self.clip.set(write_tsv(&grid), None, 0, to_system);
        Outcome::Done
    }

    /// `table.clear` (Delete): the cursor column of the marked rows (else the cursor row) cleared (§8.3).
    pub(super) fn act_table_clear(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.table_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some(view) = self.table_view(kind) else {
            return Outcome::Done;
        };
        let Some(col) = view.cursor_col else {
            return Outcome::Done;
        };
        if view.acted.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        }
        let cells: Vec<GestureCell> = view
            .acted
            .iter()
            .map(|i| {
                let value = match self.cell_route(&view, *i, col) {
                    Err(reason) => CellValue::Invalid(reason),
                    Ok((k, path)) => match cleared(route_type(&k, &path).as_ref()) {
                        Some(v) => CellValue::Valid(v),
                        None => CellValue::Invalid(strings::text("table-not-clearable").to_owned()),
                    },
                };
                self.cell(&view, *i, col, "", value)
            })
            .collect();
        self.run_gesture(Self::gesture(GestureKind::Clear, kind, cells), None)
    }

    /// The table card's gesture.
    fn table_card(&self) -> Option<&Gesture> {
        match &self.props.card()?.origin {
            CardOrigin::Table { gesture } => Some(gesture),
            _ => None,
        }
    }

    /// The table card's gesture, taken out of the card (`None`: no table card).
    fn take_table_card(&mut self) -> Option<Gesture> {
        self.table_card()?;
        match self.props.card.take()?.origin {
            CardOrigin::Table { gesture } => Some(gesture),
            _ => None,
        }
    }

    /// `table.resolve {cell, choice}`: picks reading `choice` (from 1) of the card's ambiguous cell `cell` (from 1)
    /// and rebuilds the command and its preview.
    pub(super) fn act_table_resolve(&mut self, p: &Map<String, Value>) -> Outcome {
        let (cell, choice) = match (index_param(p, "cell"), index_param(p, "choice")) {
            (Ok(c), Ok(x)) => (c, x),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let Some(g) = self.table_card() else {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        };
        let value = match g.cells.get(cell - 1) {
            None => return failed("error-bad-param", &["cell"]),
            Some(c) if c.dropped => return failed("error-bad-param", &["cell"]),
            Some(c) => match &c.value {
                CellValue::Ambiguous(readings) => match readings.get(choice - 1) {
                    Some((_, v)) => v.clone(),
                    None => return failed("error-bad-param", &["choice"]),
                },
                _ => return failed("error-table-not-ambiguous", &[&cell.to_string()]),
            },
        };
        let Some(mut gesture) = self.take_table_card() else {
            return Outcome::Done;
        };
        gesture.cells[cell - 1].value = CellValue::Valid(value);
        self.run_gesture(gesture, Some("reason-rebuilt"))
    }

    /// `table.drop {cell}`: removes the card's cell `cell` (from 1) from the gesture and rebuilds the command and its
    /// preview.
    pub(super) fn act_table_drop(&mut self, p: &Map<String, Value>) -> Outcome {
        let cell = match index_param(p, "cell") {
            Ok(c) => c,
            Err(e) => return e,
        };
        let Some(g) = self.table_card() else {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        };
        if !g.cells.get(cell - 1).is_some_and(|c| !c.dropped) {
            return failed("error-bad-param", &["cell"]);
        }
        let Some(mut gesture) = self.take_table_card() else {
            return Outcome::Done;
        };
        gesture.cells[cell - 1].dropped = true;
        self.run_gesture(gesture, Some("reason-rebuilt"))
    }

    /// The navigator pane's part of the Escape chain in table mode: the pattern dialog or the cell edit, then the table
    /// card. `None` when neither is open.
    pub(super) fn dismiss_table(&mut self, kind: NavKind) -> Option<Outcome> {
        if let Some(m) = self.mode_mut(kind)
            && (m.edit.is_some() || m.pattern.is_some() || m.error.is_some())
        {
            m.edit = None;
            m.pattern = None;
            m.error = None;
            return Some(Outcome::Done);
        }
        self.take_table_card().map(|_| Outcome::Done)
    }

    /// The table card's gesture (the card in the navigator's pane), for the window and the describe text.
    pub fn table_gesture(&self) -> Option<&Gesture> {
        self.table_card()
    }

    /// The first five texts of the open pattern dialog's pattern (the dialog's preview), or the pattern's error.
    pub fn pattern_preview(&self, kind: NavKind) -> Option<(String, Result<Vec<String>, String>)> {
        let n = self.generic(kind)?;
        let pattern = n.table_mode.as_ref()?.pattern.clone()?;
        let keys: Vec<&str> = n
            .table()
            .map(|t| t.columns.iter().map(|c| c.key.as_str()).collect())
            .unwrap_or_default();
        let values = Template::parse(&pattern)
            .and_then(|t| t.check_columns(&keys).map(|()| t))
            .map_err(|e| e.message())
            .map(|t| {
                pattern_texts(n, &t)
                    .into_iter()
                    .take(5)
                    .map(|r| r.unwrap_or_else(|e| e.message().to_owned()))
                    .collect()
            });
        Some((pattern, values))
    }

    /// The text of cell (`row`, `column`) of navigator `kind`'s table.
    pub fn table_cell_text(&self, kind: NavKind, row: &str, column: &str) -> String {
        let Some(table) = self.generic(kind).and_then(|n| n.table()) else {
            return String::new();
        };
        table
            .nodes
            .iter()
            .position(|n| n.key == row)
            .map(|i| cell_text(table, u32::try_from(i).unwrap_or(u32::MAX), column))
            .unwrap_or_default()
    }
}
