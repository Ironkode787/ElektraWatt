// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command palette in the shell (WP-S2.08 part 1, FR-UI-012; the matching in [`crate::palette`]): the entries of
//! a query by its prefix (D4), ranked with the aliases of the glossary (D5 (b)) and the recents (D3), and Enter: an
//! action runs (or opens its parameter form, D5 (a)), a page is shown, a cell is gone to, an object is selected and a
//! rule shows Messages filtered to it. The palette is transient widget state: nothing here is persisted.

use ew_api::output::FindQuery;
use serde_json::{Value, json};

use super::{Outcome, PaletteState, ShellModel};
use crate::actions::{self, ACTIONS, ActionMeta, Menu, PaletteEntry, PaletteRun};
use crate::glossary;
use crate::palette::{
    FormPlan, MAX_OBJECTS, PaletteForm, Prefix, RECENT_SHOWN, form_plan, order, rank, score, split_prefix,
};
use crate::strings;

/// An entry with its rank, before the order is applied.
struct Ranked {
    entry: PaletteEntry,
    rank: i64,
}

fn sort(ranked: &mut [Ranked]) {
    ranked.sort_by(|a, b| {
        order(
            (a.entry.enabled, a.rank, &a.entry.label, &a.entry.id),
            (b.entry.enabled, b.rank, &b.entry.label, &b.entry.id),
        )
    });
}

impl ShellModel {
    /// Sets the text of the palette's field and selects its first entry (an open parameter form closes).
    pub fn set_palette_query(&mut self, query: &str) {
        if let Some(p) = &mut self.palette {
            p.query = query.to_owned();
            p.selected = 0;
            p.form = None;
        }
    }

    /// Moves the palette's selection by `delta` entries, clamped to the entries of the query.
    pub fn move_palette_selection(&mut self, delta: isize) {
        if self.palette.as_ref().is_some_and(|p| p.form.is_some()) {
            return;
        }
        let count = self.palette_entries().len();
        if let Some(p) = &mut self.palette {
            let last = count.saturating_sub(1);
            p.selected = p.selected.saturating_add_signed(delta).min(last);
        }
    }

    /// Selects the palette entry at `index` (a click), clamped to the entries of the query.
    pub fn select_palette_entry(&mut self, index: usize) {
        let last = self.palette_entries().len().saturating_sub(1);
        if let Some(p) = &mut self.palette {
            p.selected = index.min(last);
        }
    }

    /// Sets the text of row `index` of the palette's open parameter form.
    pub fn set_palette_field(&mut self, index: usize, value: &str) {
        if let Some(field) = self
            .palette
            .as_mut()
            .and_then(|p| p.form.as_mut())
            .and_then(|f| f.fields.get_mut(index))
        {
            value.clone_into(&mut field.value);
        }
    }

    /// Counts a shortcut invocation of `action` for the palette's recents (D3, PL-4); the window calls it when a
    /// shortcut fires.
    pub fn note_shortcut_use(&mut self, action: &str) {
        if let Some(meta) = actions::find(action) {
            self.recents.note(meta.id);
        }
    }

    /// The palette entries for the current query (WP-S2.08 part 1).
    ///
    /// By the query's prefix (D4): none or `>` the registry actions the window can list, ranked by label, glossary
    /// alias and ID with the recency boost, and with an empty query the eight most recent distinct actions first
    /// (D3); `@` the objects of `Session::find` (at most 50, an exact match first); `#` the pages by folio or title;
    /// `:` the cell `Session::resolve_goto` finds; `?` the rules of the shipped catalogue, then the Help menu's
    /// actions. An action that needs parameters is listed: with a form when its signature allows one, disabled with
    /// its reason otherwise (D5 (a)). Disabled entries follow the enabled ones; each carries its breadcrumb.
    pub fn palette_entries(&self) -> Vec<PaletteEntry> {
        let query = self.palette.as_ref().map(|p| p.query.as_str()).unwrap_or("");
        let (prefix, rest) = split_prefix(query);
        match prefix {
            Prefix::Actions => self.action_entries(rest, |_| true, true),
            Prefix::Objects => self.object_entries(rest),
            Prefix::Pages => self.page_entries(rest),
            Prefix::Cell => self.cell_entries(rest),
            Prefix::Help => {
                let mut entries = self.rule_entries(rest);
                entries.extend(self.action_entries(rest, |m| m.category == Menu::Help, false));
                entries
            }
        }
    }

    /// The listable actions that pass `keep`, ranked; `recents` puts the recent ones first for an empty query.
    fn action_entries(
        &self,
        query: &str,
        keep: impl Fn(&ActionMeta) -> bool,
        recents: bool,
    ) -> Vec<PaletteEntry> {
        let mut ranked: Vec<Ranked> = Vec::new();
        for meta in ACTIONS
            .iter()
            .filter(|a| !a.headless_only && !a.hidden && keep(a))
        {
            let label = strings::text(meta.label_key);
            let aliases = glossary::aliases_of(meta.id);
            let state = match form_plan(meta) {
                FormPlan::Disabled(reason) => Err(reason),
                FormPlan::Direct | FormPlan::Form(_) => (meta.enabled)(self),
            };
            let Some(rank) = rank(
                query,
                label,
                aliases.iter().copied(),
                meta.id,
                self.recents.uses(meta.id),
                state.is_ok(),
            ) else {
                continue;
            };
            ranked.push(Ranked {
                entry: PaletteEntry {
                    run: PaletteRun::Action(meta.id),
                    label: label.to_owned(),
                    id: meta.id.to_owned(),
                    shortcut: self.shortcut_text(meta),
                    enabled: state.is_ok(),
                    reason: state.err().map(|k| strings::text(k).to_owned()),
                    breadcrumb: actions::breadcrumb(meta),
                },
                rank,
            });
        }
        sort(&mut ranked);
        let mut entries: Vec<PaletteEntry> = ranked.into_iter().map(|r| r.entry).collect();
        if recents && query.is_empty() {
            // D3: the eight most recent distinct actions first, most recent first; the rest keep their rank order.
            let mut front: Vec<PaletteEntry> = Vec::new();
            for id in self.recents.distinct() {
                if front.len() == RECENT_SHOWN {
                    break;
                }
                if let Some(at) = entries.iter().position(|e| e.id == id) {
                    front.push(entries.remove(at));
                }
            }
            front.append(&mut entries);
            entries = front;
        }
        entries
    }

    /// `@`: the records `Session::find` matches, one entry per record, an exact (case-insensitive) match first, then by
    /// score of the subject, then the subject and the ID.
    fn object_entries(&self, query: &str) -> Vec<PaletteEntry> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        if query.is_empty() {
            return Vec::new();
        }
        let results = project.session.find(&FindQuery {
            text: query.to_owned(),
            ..FindQuery::default()
        });
        let lower = query.to_lowercase();
        let mut ranked: Vec<(bool, i32, PaletteEntry)> = Vec::new();
        for hit in &results.hits {
            let id = hit.record.to_string();
            if ranked.iter().any(|(_, _, e)| e.id == id) {
                continue;
            }
            let exact = hit.subject.to_lowercase() == lower;
            let s = score(query, &hit.subject).unwrap_or(i32::MIN / 2);
            ranked.push((
                exact,
                s,
                PaletteEntry {
                    run: PaletteRun::Object {
                        record: id.clone(),
                        target: hit.cell.clone().or_else(|| hit.folio.clone()),
                    },
                    label: hit.subject.clone(),
                    id,
                    shortcut: String::new(),
                    enabled: true,
                    reason: None,
                    breadcrumb: hit.cell.clone().or_else(|| hit.folio.clone()).unwrap_or_default(),
                },
            ));
        }
        ranked.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(b.1.cmp(&a.1))
                .then_with(|| a.2.label.to_lowercase().cmp(&b.2.label.to_lowercase()))
                .then_with(|| a.2.id.cmp(&b.2.id))
        });
        ranked.truncate(MAX_OBJECTS);
        ranked.into_iter().map(|(_, _, e)| e).collect()
    }

    /// `#`: the pages whose folio or title the query aligns with, the exact folio first, then by the better of the two
    /// scores (the title weighs 0.9), then in page order.
    fn page_entries(&self, query: &str) -> Vec<PaletteEntry> {
        let mut ranked: Vec<(bool, i64, usize, PaletteEntry)> = Vec::new();
        for (pos, row) in self.page_rows().iter().enumerate() {
            let folio = score(query, &row.folio).map(|s| 10 * i64::from(s));
            let title = score(query, &row.title).map(|s| 9 * i64::from(s));
            let Some(best) = folio.into_iter().chain(title).max() else {
                continue;
            };
            let label = if row.title.is_empty() {
                row.folio.clone()
            } else {
                format!("{} {}", row.folio, row.title)
            };
            ranked.push((
                row.folio == query,
                best,
                pos,
                PaletteEntry {
                    run: PaletteRun::Page(row.folio.clone()),
                    label,
                    id: row.folio.clone(),
                    shortcut: String::new(),
                    enabled: true,
                    reason: None,
                    breadcrumb: row
                        .section
                        .as_ref()
                        .map(|(n, t)| format!("{n} {t}"))
                        .unwrap_or_default(),
                },
            ));
        }
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        ranked.into_iter().map(|(.., e)| e).collect()
    }

    /// `:`: the one cell `Session::resolve_goto` finds for the text, or nothing.
    fn cell_entries(&self, query: &str) -> Vec<PaletteEntry> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Ok(target) = project.session.resolve_goto(query) else {
            return Vec::new();
        };
        // M39: the page's title after the folio ("102" then "Control"), not the folio twice.
        let title = self
            .page_rows()
            .iter()
            .find(|r| r.folio == target.folio)
            .map(|r| r.title.clone())
            .unwrap_or_default();
        let breadcrumb = match (query.trim() == target.folio, title.is_empty()) {
            (true, _) => title,
            (false, true) => target.folio,
            (false, false) => format!("{} {title}", target.folio),
        };
        vec![PaletteEntry {
            run: PaletteRun::Cell(query.to_owned()),
            label: query.to_owned(),
            id: query.to_owned(),
            shortcut: String::new(),
            enabled: true,
            reason: None,
            breadcrumb,
        }]
    }

    /// The palette's line when it lists nothing (M39): for the `:` prefix the page jump's refusal ("No page has the
    /// folio 999.", "Page 101 has no cell Z9."), else "No matching command".
    pub fn palette_empty_text(&self) -> String {
        let query = self.palette.as_ref().map(|p| p.query.as_str()).unwrap_or("");
        let (prefix, rest) = split_prefix(query);
        let resolved = match (prefix, &self.project) {
            (Prefix::Cell, Some(project)) if !rest.is_empty() => project.session.resolve_goto(rest).err(),
            _ => None,
        };
        match resolved {
            Some(ew_api::output::GotoError::NotFound) if !rest.contains('-') => {
                strings::format("error-no-folio", &[rest])
            }
            Some(ew_api::output::GotoError::NotFound) => strings::format("error-goto-not-found", &[rest]),
            Some(ew_api::output::GotoError::NoSuchCell { folio, cell }) => {
                strings::format("error-goto-no-cell", &[&folio, &cell])
            }
            _ => strings::text("palette-no-match").to_owned(),
        }
    }

    /// `?`: the shipped rules whose ID or title the query aligns with (the title weighs 0.9), by rank, then ID.
    fn rule_entries(&self, query: &str) -> Vec<PaletteEntry> {
        let enabled = self.project.is_some();
        let mut ranked: Vec<Ranked> = Vec::new();
        for rule in ew_api::checks::rules() {
            let meta = rule.meta();
            let id = score(query, meta.id).map(|s| 10 * i64::from(s));
            let title = score(query, meta.title).map(|s| 9 * i64::from(s));
            let Some(best) = id.into_iter().chain(title).max() else {
                continue;
            };
            ranked.push(Ranked {
                entry: PaletteEntry {
                    run: PaletteRun::Rule(meta.id.to_owned()),
                    label: format!("{} {}", meta.id, meta.title),
                    id: meta.id.to_owned(),
                    shortcut: String::new(),
                    enabled,
                    reason: (!enabled).then(|| strings::text("reason-no-project").to_owned()),
                    breadcrumb: meta.pack.to_owned(),
                },
                rank: best,
            });
        }
        // The label starts with the ID, so the label order is the ID order.
        sort(&mut ranked);
        ranked.into_iter().map(|r| r.entry).collect()
    }

    /// Runs the selected palette entry (Enter), or the open parameter form.
    ///
    /// An enabled action closes the palette and runs, counted as a use (D3); one with a form opens the form (D5 (a));
    /// Enter in the form runs the action with its typed parameters and closes the palette, or keeps the form with the
    /// refusal or the error. A page is shown, a cell is gone to, an object is selected and its page shown, a rule
    /// shows Messages filtered to it. A disabled entry refuses with its reason.
    pub fn run_palette_selection(&mut self) -> Outcome {
        if let Some(form) = self.palette.as_ref().and_then(|p| p.form.clone()) {
            return self.run_palette_form(form);
        }
        let entries = self.palette_entries();
        let selected = self.palette.as_ref().map_or(0, |p| p.selected);
        let Some(entry) = entries.get(selected) else {
            return Outcome::Done;
        };
        if !entry.enabled {
            return match &entry.run {
                PaletteRun::Action(id) => match actions::find(id) {
                    Some(meta) => match form_plan(meta) {
                        FormPlan::Disabled(reason_key) => Outcome::Refused { reason_key },
                        _ => Outcome::Refused {
                            reason_key: (meta.enabled)(self).err().unwrap_or("reason-parameters-later"),
                        },
                    },
                    None => Outcome::Refused {
                        reason_key: "reason-parameters-later",
                    },
                },
                PaletteRun::Rule(_) => Outcome::Refused {
                    reason_key: "reason-no-project",
                },
                _ => Outcome::Refused {
                    reason_key: "reason-parameters-later",
                },
            };
        }
        match entry.run.clone() {
            PaletteRun::Action(id) => {
                if let Some(FormPlan::Form(specs)) = actions::find(id).map(form_plan) {
                    if let Some(p) = &mut self.palette {
                        p.form = Some(PaletteForm::new(id, &specs));
                    }
                    return Outcome::Done;
                }
                self.palette = None;
                self.recents.note(id);
                self.dispatch(id, &Value::Null)
            }
            PaletteRun::Command(_) => Outcome::Done,
            PaletteRun::Page(folio) => {
                self.palette = None;
                self.dispatch("page.goto", &json!({ "folio": folio }))
            }
            PaletteRun::Cell(text) => {
                self.palette = None;
                self.dispatch("nav.goto", &json!({ "text": text }))
            }
            PaletteRun::Object { record, target } => {
                self.palette = None;
                let out = self.dispatch("select.set", &json!({ "ids": [record] }));
                match (out, target) {
                    (Outcome::Done, Some(text)) => self.dispatch("nav.goto", &json!({ "text": text })),
                    (out, _) => out,
                }
            }
            PaletteRun::Rule(id) => {
                self.palette = None;
                let out = self.dispatch("messages.show", &json!({ "on": true }));
                if out != Outcome::Done {
                    return out;
                }
                self.dispatch("navigator.filter", &json!({ "kind": "messages", "text": id }))
            }
        }
    }

    /// Enter in the palette's parameter form.
    fn run_palette_form(&mut self, form: PaletteForm) -> Outcome {
        let params = match form.params() {
            Ok(p) => p,
            Err(name) => {
                let message = strings::format("error-bad-param", &[&name]);
                self.set_palette_form_error(Some(message.clone()));
                return Outcome::Failed { message };
            }
        };
        let out = self.dispatch(form.action, &Value::Object(params));
        match &out {
            Outcome::Done => {
                self.recents.note(form.action);
                // The action may have opened something else (a dialog); the palette closes either way.
                self.palette = None;
            }
            Outcome::Refused { reason_key } => {
                self.set_palette_form_error(Some(strings::text(reason_key).to_owned()))
            }
            Outcome::Failed { message } => self.set_palette_form_error(Some(message.clone())),
        }
        out
    }

    fn set_palette_form_error(&mut self, error: Option<String>) {
        if let Some(form) = self.palette.as_mut().and_then(|p| p.form.as_mut()) {
            form.error = error;
        }
    }

    /// Escape in the palette's parameter form: back to the list. `true` when a form was open.
    pub(super) fn close_palette_form(&mut self) -> bool {
        self.palette
            .as_mut()
            .is_some_and(|p: &mut PaletteState| p.form.take().is_some())
    }
}
