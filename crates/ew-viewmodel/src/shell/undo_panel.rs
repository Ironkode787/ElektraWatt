// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 3: the history panel's host duties and context restore (§8.4, §8.5, D13; FR-UI-023).
//!
//! **The panel** fetches `Session::history_view` once per (project session, revision) while it is open
//! ([`ShellModel::sync_history`], counted) and none while it is closed. `history.row` moves its row cursor over the
//! entries (newest first, as shown), `history.undo_to` undoes or redoes to an entry, `history.changes` opens Show changes
//! (the entry's records, `Session::history_entities`). Entries are numbered from 1 (oldest kept) as the describe text
//! prints them; Undo to here on entry `n` leaves the history's cursor at `n` (entry `n` is the state `now`).
//!
//! **Context restore.** [`ShellModel::take_context`] runs before every action; [`ShellModel::keep_contexts`] after it
//! keeps that context under the revision of every history entry the action committed (same project session only).
//! Undo and redo record the revision of the step they cross; [`ShellModel::restore_context`] then opens that step's page
//! in the focused tab as a jump (one back-history entry), sets its viewport (a fitted view fits again) and selects the
//! items of its selection that still exist (`select.set`'s resolution).

use ew_api::output::ew_dl::EntityRef;
use serde_json::{Map, Value};

use super::navigation::bump;
use super::{Outcome, ShellModel, failed};
use crate::layout::PaneKind;
use crate::undo_history::{HistoryContext, HistoryPane};

/// An integer parameter (`step`): `Ok(None)` when absent.
fn int_param(p: &Map<String, Value>, name: &str) -> Result<Option<i64>, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_i64()
            .map(Some)
            .ok_or_else(|| failed("error-bad-param", &[name])),
    }
}

/// An entry number parameter (from 1): `Ok(None)` when absent.
fn entry_param(p: &Map<String, Value>) -> Result<Option<usize>, Outcome> {
    match p.get("entry") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n >= 1)
            .map(Some)
            .ok_or_else(|| failed("error-bad-param", &["entry"])),
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The history panel's state.
    pub fn history_pane(&self) -> &HistoryPane {
        &self.history
    }

    /// Whether the history panel is in the layout.
    pub fn history_shown(&self) -> bool {
        self.layout.panes().contains(&PaneKind::History)
    }

    /// Annotates the history entry committed at `rev` with the origin text the panel shows instead of `Session`'s
    /// (`quick fix EW-CORE-023`, §7.4 and §8.4); view state, cleared with the project.
    pub fn note_history_origin(&mut self, rev: u64, text: &str) {
        self.history.origins.insert(rev, text.to_owned());
    }

    // -----------------------------------------------------------------------------------------------------------
    // The panel

    /// The panel follows the revision while it is open (one `Session::history_view` per project session and revision)
    /// and forgets its view while it is closed.
    pub(super) fn sync_history(&mut self) {
        let key = self.project.as_ref().map(|p| (p.epoch, p.session.world().rev.0));
        let Some((epoch, rev)) = key.filter(|_| self.history_shown()) else {
            self.history.view = None;
            self.history.changes = None;
            return;
        };
        if self
            .history
            .view
            .as_ref()
            .is_some_and(|(e, r, _)| *e == epoch && *r == rev)
        {
            return;
        }
        let Some(project) = &self.project else { return };
        bump(&self.calls, |c| c.history_view += 1);
        let view = project.session.history_view();
        if self.history.row.is_some_and(|i| i >= view.rows.len()) {
            self.history.row = view.rows.len().checked_sub(1);
        }
        self.history.view = Some((epoch, rev, view));
    }

    /// `history.show {on?}` (Edit, History): opens or closes the panel in the bottom dock.
    pub(super) fn act_history_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self.history_shown();
        let want = match super::jobs::bool_param(p, "on", !shown) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let layout = if want {
            self.layout
                .with_bottom(PaneKind::History)
                .showing(PaneKind::History)
        } else {
            self.layout.without(PaneKind::History)
        };
        self.replace_layout(layout);
        Outcome::Done
    }

    /// Whether any pane of the bottom dock is on show (`view.bottom_dock`'s checked state).
    pub fn bottom_dock_shown(&self) -> bool {
        self.layout.panes().iter().any(|p| PaneKind::BOTTOM.contains(p))
    }

    /// `view.bottom_dock {on?}` (doc 16a §5.7, Ctrl+J; a toggle without `on`): closing closes every bottom pane
    /// through its own action and remembers them; opening reopens the remembered panes, or Messages.
    pub(super) fn act_bottom_dock(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self.bottom_dock_shown();
        let want = match super::jobs::bool_param(p, "on", !shown) {
            Ok(w) => w,
            Err(e) => return e,
        };
        if want == shown {
            return Outcome::Done;
        }
        let toggle = |pane: PaneKind, on: bool| -> Option<(&'static str, Value)> {
            let flag = serde_json::json!({ "on": on });
            match pane {
                PaneKind::Find if on => Some(("find.open", Value::Null)),
                PaneKind::Find => Some(("find.close", Value::Null)),
                PaneKind::Jobs => Some(("jobs.show", flag)),
                PaneKind::Navigator(crate::navigators::NavKind::Messages) => Some(("messages.show", flag)),
                PaneKind::Preview => Some(("preview.show", flag)),
                PaneKind::History => Some(("history.show", flag)),
                _ => None,
            }
        };
        let panes: Vec<PaneKind> = if want {
            let mut kept = std::mem::take(&mut self.bottom_closed);
            if kept.is_empty() {
                kept.push(PaneKind::Navigator(crate::navigators::NavKind::Messages));
            }
            kept
        } else {
            let open: Vec<PaneKind> = self
                .layout
                .panes()
                .into_iter()
                .filter(|p| PaneKind::BOTTOM.contains(p))
                .collect();
            self.bottom_closed.clone_from(&open);
            open
        };
        for pane in panes {
            if let Some((action, params)) = toggle(pane, want) {
                // A pane that cannot open now (Preview without a card) stays closed; the others still open.
                let _ = self.dispatch(action, &params);
            }
        }
        Outcome::Done
    }

    /// The number of entries of the open project's history.
    fn history_len(&self) -> usize {
        self.project
            .as_ref()
            .map_or(0, |p| p.session.history().entries().len())
    }

    /// `history.row {step?, to?}`: moves the row cursor `step` rows down the list (to older entries; negative: up), or
    /// to the `first` (newest) or `last` (oldest) entry, clamped. Without a cursor a step starts at the `now` entry.
    pub(super) fn act_history_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let len = self.history_len();
        if len == 0 {
            return Outcome::Refused {
                reason_key: "reason-undo-history-empty",
            };
        }
        let to = match super::string_param(p, "to") {
            Err(e) => return e,
            Ok(t) => t,
        };
        let step = match int_param(p, "step") {
            Err(e) => return e,
            Ok(s) => s,
        };
        let index = match (to, step) {
            (Some("first"), None) => len - 1,
            (Some("last"), None) => 0,
            (None, Some(step)) => {
                let cursor = self.project.as_ref().map_or(0, |p| p.session.history().cursor());
                let from = self.history.row.unwrap_or(cursor.saturating_sub(1).min(len - 1));
                // Down the list is toward older entries: a smaller index.
                let moved = i64::try_from(from).unwrap_or(0).saturating_sub(step);
                usize::try_from(moved.clamp(0, i64::try_from(len - 1).unwrap_or(0))).unwrap_or(0)
            }
            (None, None) => return failed("error-missing-param", &["step"]),
            _ => return failed("error-bad-param", &["to"]),
        };
        self.history.row = Some(index);
        Outcome::Done
    }

    /// The entry index of `history.undo_to`'s or `history.changes`' `entry` (from 1), else the row cursor's.
    fn history_entry(&self, p: &Map<String, Value>) -> Result<usize, Outcome> {
        let len = self.history_len();
        match entry_param(p)? {
            Some(n) if n <= len => Ok(n - 1),
            Some(n) => Err(failed("error-unknown-entry", &[&n.to_string(), &len.to_string()])),
            None => self
                .history
                .row
                .filter(|i| *i < len)
                .ok_or_else(|| failed("error-missing-param", &["entry"])),
        }
    }

    /// `history.undo_to {entry?}` (Undo to here; Enter on a row): undoes while the history's cursor is above the
    /// entry's number, or redoes while it is below; stops at the first refusal (a barrier, a `HumanOnly` step from a
    /// script) with its reason. Only the last step crossed restores its context.
    pub(super) fn act_history_undo_to(&mut self, p: &Map<String, Value>) -> Outcome {
        let index = match self.history_entry(p) {
            Ok(i) => i,
            Err(e) => return e,
        };
        let target = index + 1;
        self.history.quiet = true;
        self.history.crossed = None;
        let mut out = Outcome::Done;
        let cursor_now = |m: &ShellModel| m.project.as_ref().map(|p| p.session.history().cursor());
        // One direction only: a step of several entries may carry the cursor past the target, and the walk ends there.
        let down = cursor_now(self).is_some_and(|c| c > target);
        while let Some(cursor) = cursor_now(self).filter(|c| if down { *c > target } else { *c < target }) {
            let step = if down { self.act_undo() } else { self.act_redo() };
            let after = self.project.as_ref().map(|p| p.session.history().cursor());
            if step != Outcome::Done || after == Some(cursor) {
                out = step;
                break;
            }
        }
        self.history.quiet = false;
        if let Some(rev) = self.history.crossed.take() {
            self.restore_context(rev);
        }
        out
    }

    /// `history.changes {entry?}` (Show changes): the entry's records with their labels and changed members, in a
    /// popup that Escape closes.
    pub(super) fn act_history_changes(&mut self, p: &Map<String, Value>) -> Outcome {
        let index = match self.history_entry(p) {
            Ok(i) => i,
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let records = project.session.history_entities(index);
        self.history.changes = Some((index, records));
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // Context restore

    /// The context of an action about to run: the focused tab's page and viewport and the window selection; `None`
    /// without a project or a canvas.
    pub(super) fn take_context(&self) -> Option<(u64, u64, HistoryContext)> {
        let project = self.project.as_ref()?;
        let area = self.area.as_ref()?;
        let tab = area.current();
        let viewport = tab.canvas.as_ref()?.viewport();
        Some((
            project.epoch,
            project.session.world().rev.0,
            HistoryContext {
                page: tab.page,
                viewport,
                selection: self.tools.selection.items.clone(),
            },
        ))
    }

    /// Keeps `before` (taken before the action) under the revision of every history entry the action committed in the
    /// same project session.
    pub(super) fn keep_contexts(&mut self, before: Option<(u64, u64, HistoryContext)>) {
        let Some((epoch, rev, context)) = before else {
            return;
        };
        let Some(project) = self.project.as_ref().filter(|p| p.epoch == epoch) else {
            return;
        };
        let new: Vec<u64> = project
            .session
            .history()
            .entries()
            .iter()
            .map(|e| e.rev().0)
            .filter(|r| *r > rev)
            .collect();
        for r in new {
            self.history.keep_context(r, context.clone());
        }
    }

    /// After an undo or redo that crossed the step committed at `rev`: restores that step's context, unless Undo to
    /// here walks (it restores the last step's only).
    pub(super) fn crossed_step(&mut self, rev: Option<u64>) {
        self.history.crossed = rev.or(self.history.crossed);
        if !self.history.quiet
            && let Some(rev) = self.history.crossed.take()
        {
            self.restore_context(rev);
        }
    }

    /// Restores the context kept under `rev` (§8.5): the page as a jump if it still exists, its viewport, and the
    /// items of its selection that still exist. Nothing without a kept context.
    pub(super) fn restore_context(&mut self, rev: u64) {
        let Some(context) = self.history.contexts.get(&rev).cloned() else {
            return;
        };
        if let Some(g) = self.area.as_ref().map(|a| a.focused)
            && self.row_index(context.page).is_some()
        {
            self.restore_location(g, context.page, context.viewport);
        }
        let Some(project) = &self.project else { return };
        let items: Vec<EntityRef> = context
            .selection
            .into_iter()
            .filter(|e| project.session.resolve_reference(&format!("id:{}", e.0)).is_ok())
            .collect();
        if items != self.tools.selection.items
            && self
                .tools
                .selection
                .apply(&crate::tools::SelectionChange::Replace(items))
        {
            self.announce_selection();
        }
    }
}
