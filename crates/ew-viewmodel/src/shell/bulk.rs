// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 3: the Preview tab's host duties (§8.1, D11), the renumber dialogs and their `bulk` card (§8.2, §8.3,
//! D12) and the GUI purge step (§8.6, FT-417).
//!
//! **The Preview tab.** [`ShellModel::sync_preview`] runs at the end of every dispatch and after the window changed the
//! layout: a new card (by [`crate::preview::card_key`]) fetches its lists once (`Session::preview_entities`, counted) and
//! shows the tab; a card whose tab was closed is cancelled; when the last card is applied or cancelled, a tab the cards
//! opened closes and a tab the user opened by hand stays. The card itself stays the property grid's state and its
//! `card` describe line is unchanged.
//!
//! **The renumber dialogs** build `page.renumber` and `numbering.renumber` with explicit values (UUIDs, never
//! references), preview them through the host's path and open the `bulk` card; Apply commits one transaction through
//! [`ShellModel::commit_kept`] (headless one command-log line, so `--record` replays it). Like the creation dialogs
//! (FT-408), a dialog opens only in the window, when the action lacks its required parameter (`start`, `scheme`);
//! headless the step fails naming it.
//!
//! **The purge step** lists `Session::purge_preview`'s files in a confirmation and commits `project.assets.purge` with
//! exactly the listed hashes from the GUI origin. The command is `HumanOnly`: headless the action refuses.

use std::collections::BTreeSet;

use ew_api::output::Uuid;
use ew_api::{AnyCommand, PurgeError, Submit};
use serde_json::{Map, Value, json};

use super::creation::{FormDialog, FormField};
use super::navigation::bump;
use super::{Dialog, Mode, Outcome, ShellModel, failed, string_param};
use crate::layout::PaneKind;
use crate::preview::{PreviewPane, XrefChange, card_key};
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit};
use crate::strings;
use crate::tools::PreviewState;

/// The words a boolean field of a renumber dialog offers.
const BOOL_CHOICES: &[&str] = &["false", "true"];

/// The scopes of Renumber devices.
const DEVICE_SCOPES: &[&str] = &["project", "page", "selection"];

/// The schemes of `numbering.renumber` (its `scheme` schema).
const SCHEMES: &[&str] = &["page_based"];

/// The most file names the purge confirmation lists before "and `<n>` more" (§8.6).
pub const PURGE_LISTED: usize = 20;

/// A whole-number parameter: a number or its decimal text; `Ok(None)` when absent or empty.
fn uint_param(p: &Map<String, Value>, name: &str) -> Result<Option<u32>, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .map(Some)
            .ok_or_else(|| failed("error-bad-param", &[name])),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s
            .trim()
            .parse::<u32>()
            .map(Some)
            .map_err(|_| failed("error-bad-param", &[name])),
        Some(_) => Err(failed("error-bad-param", &[name])),
    }
}

/// A boolean parameter: a JSON boolean or the dialog's `true` / `false`; `default` when absent or empty.
fn bool_text_param(p: &Map<String, Value>, name: &str, default: bool) -> Result<bool, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::String(s)) => match s.trim() {
            "" => Ok(default),
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(failed("error-bad-param", &[name])),
        },
        Some(_) => Err(failed("error-bad-param", &[name])),
    }
}

/// The leading number of a folio (`101` of `101.1`).
fn folio_number(folio: &str) -> Option<u32> {
    let digits: String = folio.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The text of a purge refusal: the store's own text, except a read error, whose text names a path.
fn purge_reason(e: &PurgeError) -> String {
    match e {
        PurgeError::Io { .. } => strings::text("purge-unreadable").to_owned(),
        other => other.to_string(),
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // The Preview tab

    /// The Preview tab's lists of the open card.
    pub fn preview_pane(&self) -> &PreviewPane {
        &self.preview
    }

    /// The Preview tab's summary line of the open card (§8.1): its records, tags, cross-reference texts and pages
    /// (`preview-summary`), or `None` without a card. The window paints it under the tab's header; the text interface
    /// prints it for a bulk or table card, which has no inline card of its own (FT-580).
    pub fn preview_summary(&self) -> Option<String> {
        let card = self.props.card()?;
        // FT-759: a parts import says what it imports in words ("29 new parts · 3 already in the project"), from
        // the importer's counts, not the records of its diff (the parts and their manufacturers).
        if let crate::properties::CardOrigin::Import { created, skipped, .. } = &card.origin {
            return Some(strings::format(
                "preview-import-summary",
                &[
                    &strings::format_count("preview-parts-new", *created),
                    &strings::format_count("preview-parts-skipped", *skipped),
                ],
            ));
        }
        let records = card.preview.result.as_ref().map_or(0, |p| p.diff().len());
        // Cycle 37 (window check W37): a count of one takes the singular ("1 record · 1 page").
        Some(strings::format(
            "preview-summary",
            &[
                &strings::format_count("preview-records", records),
                &strings::format_count("preview-tags-count", self.preview.tags().len()),
                &strings::format_count("preview-xrefs-count", self.preview.xrefs().len()),
                &strings::format_count("preview-pages-count", self.preview.pages().len()),
            ],
        ))
    }

    /// Whether the Preview tab is in the layout.
    pub fn preview_shown(&self) -> bool {
        self.layout.panes().contains(&PaneKind::Preview)
    }

    /// The Preview tab follows the card (module documentation): a new card fetches its lists and shows the tab; a card
    /// whose tab was closed is cancelled; no card forgets the lists and, when the card chain opened the tab, closes it
    /// (a tab the user opened by hand stays; a fitted, untouched view refits to the freed space, ID-904).
    pub(super) fn sync_preview(&mut self) {
        let Some(key) = self.props.card().map(card_key) else {
            let close = self.preview.key.is_some() && self.preview.opened_by_card;
            self.preview.clear();
            if close && self.preview_shown() {
                let layout = self.layout.without(PaneKind::Preview);
                self.replace_layout(layout);
            }
            return;
        };
        if self.preview.key.as_ref() == Some(&key) {
            if !self.preview_shown() {
                // The tab was closed with the card pending: the card is cancelled (§8.1).
                self.props.card = None;
                self.preview.clear();
            }
            return;
        }
        // A card following another (a paste choice's update card) keeps who opened the tab.
        let opened_by_card = if self.preview.key.is_some() {
            self.preview.opened_by_card
        } else {
            !self.preview_shown()
        };
        self.fetch_preview_lists();
        self.preview.key = Some(key);
        self.preview.opened_by_card = opened_by_card;
        let layout = self
            .layout
            .with_bottom(PaneKind::Preview)
            .showing(PaneKind::Preview);
        self.replace_layout(layout);
    }

    /// The open card's lists: its records (one `Session::preview_entities`), its tag and cross-reference changes in
    /// display order, the folios of its affected pages.
    fn fetch_preview_lists(&mut self) {
        self.preview.clear();
        let rows = self.page_rows().to_vec();
        let calls = &self.calls;
        let (Some(project), Some(card)) = (&self.project, self.props.card.as_ref()) else {
            return;
        };
        let Ok(preview) = &card.preview.result else {
            return;
        };
        bump(calls, |c| c.preview_entities += 1);
        self.preview.entities = project.session.preview_entities(preview);
        let position = |page: Uuid| rows.iter().position(|r| r.id == page);
        let mut pages: Vec<usize> = preview
            .affected_pages()
            .iter()
            .filter_map(|p| position(p.as_uuid()))
            .collect();
        pages.sort_unstable();
        self.preview.pages = pages.into_iter().map(|i| rows[i].folio.clone()).collect();
        let mut tags: Vec<(String, String)> = card
            .preview
            .derived
            .tags
            .iter()
            .map(|t| (t.before.clone(), t.after.clone()))
            .collect();
        tags.sort_by(|a, b| crate::shell::natural_cmp(&a.0, &b.0).then_with(|| a.cmp(b)));
        self.preview.tags = tags;
        let q = project.session.query();
        let mut xrefs: Vec<(usize, XrefChange)> = card
            .preview
            .derived
            .xrefs
            .iter()
            .map(|x| {
                let page = q.reps().get(x.key).map(|r| r.page.as_uuid());
                let at = page.and_then(position);
                (
                    at.unwrap_or(usize::MAX),
                    XrefChange {
                        folio: at.map(|i| rows[i].folio.clone()).unwrap_or_default(),
                        before: x.before.clone(),
                        after: x.after.clone(),
                    },
                )
            })
            .collect();
        xrefs.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| crate::shell::natural_cmp(&a.1.before, &b.1.before))
                .then_with(|| a.1.before.cmp(&b.1.before))
                .then_with(|| a.1.after.cmp(&b.1.after))
        });
        self.preview.xrefs = xrefs.into_iter().map(|(_, x)| x).collect();
    }

    /// `preview.show {on?}`: shows or closes the Preview tab; closing it with a card pending cancels the card.
    pub(super) fn act_preview_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self.preview_shown();
        let want = match super::jobs::bool_param(p, "on", !shown) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let layout = if want {
            self.layout
                .with_bottom(PaneKind::Preview)
                .showing(PaneKind::Preview)
        } else {
            self.layout.without(PaneKind::Preview)
        };
        self.replace_layout(layout);
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The bulk card

    /// Previews the `Bulk` command `name` with `params` through the host's path and shows its `bulk` card; a refused
    /// preview opens no card and is the outcome's message.
    pub(super) fn preview_bulk(&mut self, name: &str, params: Value) -> Outcome {
        self.preview_host_card(name, params, "reason-bulk")
    }

    /// Previews the command `name` that a host action built (a renumber dialog, Draw ▸ Bond) and shows its card with
    /// the reason `reason`; Apply goes through [`ShellModel::apply_bulk_card`]. A refused preview opens no card and is
    /// the outcome's message.
    pub(super) fn preview_host_card(&mut self, name: &str, params: Value, reason: &'static str) -> Outcome {
        self.preview_host_card_as(name, params, reason, CardOrigin::Bulk)
    }

    /// [`ShellModel::preview_host_card`] with the card's origin (WP-S2.07 part 4a: the part card).
    pub(super) fn preview_host_card_as(
        &mut self,
        name: &str,
        params: Value,
        reason: &'static str,
        origin: CardOrigin,
    ) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let cmd = AnyCommand {
            name: name.to_owned(),
            params: params.clone(),
        };
        let submit = Submit::from(super::ShellModel::origin_of(
            self.mode,
            self.person,
            &self.origin_file,
        ));
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        bump(calls, |c| c.preview_with += 1);
        let preview = match project.session.preview_with(&cmd, submit) {
            Ok(p) => p,
            Err(e) => {
                return Outcome::Failed {
                    message: e.to_string(),
                };
            }
        };
        let derived = project.session.derived_diff(&preview);
        let changes = preview.diff().len();
        let edit = PendingEdit {
            row: String::new(),
            kind: String::new(),
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: name.to_owned(),
            params: params.clone(),
            typed: String::new(),
            label: String::new(),
        };
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name: name.to_owned(),
                result: Ok(preview),
                derived,
            },
            edit,
            targets: changes,
            changes,
            reason,
            origin,
        });
        Outcome::Done
    }

    /// Apply on a bulk card: a fresh card commits one transaction; a stale one previews the same parameters again and
    /// shows a fresh card (a second Apply), as the reorder card does.
    pub(super) fn apply_bulk_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            return Outcome::Failed {
                message: message.clone(),
            };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            return self.commit_kept(preview, &edit.name, &edit.params, None);
        }
        self.preview_host_card_as(&card.edit.name, card.edit.params, card.reason, card.origin)
    }

    // -----------------------------------------------------------------------------------------------------------
    // The renumber dialogs

    /// Whether the open dialog is the form of `action`.
    fn in_bulk_form(&self, action: &str) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == action)
    }

    /// The end of a renumber action: a failure stays in its dialog, a success (the card is open) closes it.
    fn bulk_form_done(&mut self, action: &str, out: Outcome) -> Outcome {
        if self.in_bulk_form(action) {
            match &out {
                Outcome::Done => self.dialog = None,
                Outcome::Failed { message } => {
                    if let Some(Dialog::Form(f)) = &mut self.dialog {
                        f.error = Some(message.clone());
                    }
                }
                Outcome::Refused { .. } => {}
            }
        }
        out
    }

    /// The section a renumber of pages starts from: the current page's section number, else `front`.
    fn current_section_text(&self) -> String {
        self.current_page()
            .and_then(|p| self.row_index(p))
            .and_then(|i| self.page_rows()[i].section.as_ref().map(|s| s.0.to_string()))
            .unwrap_or_else(|| "front".to_owned())
    }

    /// The page records of section `section` (`front`: of no section), in page order, without generated pages and the
    /// section's divider.
    fn section_pages(&self, section: &str) -> Result<Vec<(Uuid, String)>, Outcome> {
        let number = match section {
            "front" => None,
            text => Some(
                text.parse::<u32>()
                    .map_err(|_| failed("error-bad-param", &["section"]))?,
            ),
        };
        let pages: Vec<(Uuid, String)> = self
            .page_rows()
            .iter()
            // A section's divider page holds the range's first folio and keeps it (the renumber's own pages follow).
            .filter(|r| {
                !r.generated && r.kind != "section_divider" && r.section.as_ref().map(|s| s.0) == number
            })
            .map(|r| (r.id, r.folio.clone()))
            .collect();
        if pages.is_empty() {
            return Err(match number {
                Some(_) => failed("error-no-section", &[section]),
                None => failed("error-no-front-matter", &[]),
            });
        }
        Ok(pages)
    }

    /// `pages.renumber {section?, start?, step?, keep_gaps?}` (Page, Renumber pages…): `page.renumber` of a section's
    /// pages (`front`: the pages of no section) from `start`, opening the `bulk` card. Without `start` the window opens
    /// the dialog (Section, Start, Step, Keep gaps); headless the step fails.
    pub(super) fn act_pages_renumber(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "pages.renumber";
        let section = match string_param(p, "section") {
            Err(e) => return e,
            Ok(Some(s)) if !s.trim().is_empty() => s.trim().to_owned(),
            Ok(_) => self.current_section_text(),
        };
        let start = match uint_param(p, "start") {
            Err(e) => return self.bulk_form_done(ACTION, e),
            Ok(s) => s,
        };
        let Some(start) = start else {
            if self.mode == Mode::Headless || self.in_bulk_form(ACTION) {
                let out = failed("error-missing-param", &["start"]);
                return self.bulk_form_done(ACTION, out);
            }
            let first = self
                .section_pages(&section)
                .ok()
                .and_then(|pages| pages.first().and_then(|(_, f)| folio_number(f)))
                .map(|n| n.to_string())
                .unwrap_or_default();
            let form = FormDialog {
                action: ACTION,
                title_key: "renumber-pages-title",
                ok_key: "dialog-renumber",
                fields: vec![
                    FormField {
                        name: "section",
                        label_key: "form-renumber-section",
                        choices: &[],
                        value: section,
                    },
                    FormField {
                        name: "start",
                        label_key: "form-start",
                        choices: &[],
                        value: first,
                    },
                    FormField {
                        name: "step",
                        label_key: "form-step",
                        choices: &[],
                        value: "1".to_owned(),
                    },
                    FormField {
                        name: "keep_gaps",
                        label_key: "form-keep-gaps",
                        choices: BOOL_CHOICES,
                        value: "false".to_owned(),
                    },
                ],
                error: None,
                table: None,
            };
            return self.open_dialog(Dialog::Form(form));
        };
        let out = self.renumber_pages(&section, start, p);
        self.bulk_form_done(ACTION, out)
    }

    /// The `page.renumber` of [`ShellModel::act_pages_renumber`] once its start is known.
    fn renumber_pages(&mut self, section: &str, start: u32, p: &Map<String, Value>) -> Outcome {
        let step = match uint_param(p, "step") {
            Err(e) => return e,
            Ok(s) => s.unwrap_or(1),
        };
        let keep_gaps = match bool_text_param(p, "keep_gaps", false) {
            Err(e) => return e,
            Ok(k) => k,
        };
        let pages = match self.section_pages(section) {
            Err(e) => return e,
            Ok(v) => v,
        };
        let scope = if section == "front" {
            json!({ "pages": pages.iter().map(|(id, _)| id.to_string()).collect::<Vec<_>>() })
        } else {
            let number: u16 = section.parse().unwrap_or(u16::MAX);
            let id = self.project.as_ref().and_then(|pr| {
                pr.session
                    .query()
                    .sections()
                    .iter()
                    .find(|(_, s)| s.number == number)
                    .map(|(id, _)| id.as_uuid().to_string())
            });
            let Some(id) = id else {
                return failed("error-no-section", &[section]);
            };
            json!({ "section": id })
        };
        self.preview_bulk(
            "page.renumber",
            json!({ "scope": scope, "start": start, "step": step, "keep_gaps": keep_gaps }),
        )
    }

    /// The devices of the selection: a selected device, the device of a selected function or of a selected
    /// representation's function; sorted, once each.
    fn selected_devices(&self) -> Vec<String> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let mut out: BTreeSet<Uuid> = BTreeSet::new();
        for e in &self.tools.selection.items {
            // The typed row IDs come from the canonical text, fixed by inference (the shell names no schema type).
            let text = e.0.to_string();
            let device = text
                .parse()
                .ok()
                .filter(|d| q.devices().get(*d).is_some())
                .or_else(|| {
                    text.parse()
                        .ok()
                        .and_then(|f| q.functions().get(f))
                        .and_then(|f| f.device)
                })
                .or_else(|| {
                    text.parse()
                        .ok()
                        .and_then(|r| q.reps().get(r))
                        .and_then(|r| r.functions.first().copied())
                        .and_then(|f| q.functions().get(f))
                        .and_then(|f| f.device)
                });
            if let Some(d) = device {
                out.insert(d.as_uuid());
            }
        }
        out.into_iter().map(|u| u.to_string()).collect()
    }

    /// `devices.renumber {scope?, scheme?, include_frozen?, only_nonconforming?}` (Tools, Renumber devices…):
    /// `numbering.renumber` of the project, the current page or the selected devices, opening the `bulk` card. Without
    /// `scheme` the window opens the dialog (Scope, Scheme, Include frozen tags, Only non-conforming); headless the step
    /// fails.
    pub(super) fn act_devices_renumber(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "devices.renumber";
        let scope = match string_param(p, "scope") {
            Err(e) => return e,
            Ok(Some(s)) if !s.trim().is_empty() => s.trim().to_owned(),
            Ok(_) => "project".to_owned(),
        };
        let scheme = match string_param(p, "scheme") {
            Err(e) => return e,
            Ok(s) => s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned),
        };
        let Some(scheme) = scheme else {
            if self.mode == Mode::Headless || self.in_bulk_form(ACTION) {
                let out = failed("error-missing-param", &["scheme"]);
                return self.bulk_form_done(ACTION, out);
            }
            let field = |name, label_key, choices, value: &str| FormField {
                name,
                label_key,
                choices,
                value: value.to_owned(),
            };
            let form = FormDialog {
                action: ACTION,
                title_key: "renumber-devices-title",
                ok_key: "dialog-renumber",
                fields: vec![
                    field("scope", "form-scope", DEVICE_SCOPES, &scope),
                    field("scheme", "form-scheme", SCHEMES, SCHEMES[0]),
                    field("include_frozen", "form-include-frozen", BOOL_CHOICES, "false"),
                    field(
                        "only_nonconforming",
                        "form-only-nonconforming",
                        BOOL_CHOICES,
                        "true",
                    ),
                ],
                error: None,
                table: None,
            };
            return self.open_dialog(Dialog::Form(form));
        };
        let out = self.renumber_devices(&scope, &scheme, p);
        self.bulk_form_done(ACTION, out)
    }

    /// The `numbering.renumber` of [`ShellModel::act_devices_renumber`] once its scheme is known.
    fn renumber_devices(&mut self, scope: &str, scheme: &str, p: &Map<String, Value>) -> Outcome {
        if !SCHEMES.contains(&scheme) {
            return failed("error-bad-param", &["scheme"]);
        }
        let include_frozen = match bool_text_param(p, "include_frozen", false) {
            Err(e) => return e,
            Ok(b) => b,
        };
        let only = match bool_text_param(p, "only_nonconforming", true) {
            Err(e) => return e,
            Ok(b) => b,
        };
        let scope = match scope {
            "project" => json!("project"),
            "page" => {
                let page = self
                    .current_page()
                    .and_then(|p| self.row_index(p))
                    .map(|i| &self.page_rows()[i])
                    .filter(|r| !r.generated);
                match page {
                    Some(r) => json!({ "pages": [r.id.to_string()] }),
                    None => {
                        return Outcome::Refused {
                            reason_key: "reason-no-page",
                        };
                    }
                }
            }
            "selection" => {
                let devices = self.selected_devices();
                if devices.is_empty() {
                    return Outcome::Refused {
                        reason_key: "reason-no-devices-selected",
                    };
                }
                json!({ "selection": devices })
            }
            _ => return failed("error-bad-param", &["scope"]),
        };
        self.preview_bulk(
            "numbering.renumber",
            json!({
                "scope": scope,
                "scheme": scheme,
                "include_frozen": include_frozen,
                "options": { "only_nonconforming": only },
            }),
        )
    }

    // -----------------------------------------------------------------------------------------------------------
    // The purge step (FT-417)

    /// `project.purge_assets {confirm?}` (Tools, Purge unused images…): without `confirm` the confirmation lists
    /// `Session::purge_preview`'s image files; with it `project.assets.purge` of exactly those files is committed from
    /// the GUI origin (its own marker line enters the history), and without an open confirmation it refuses. An empty
    /// plan refuses; headless the action refuses (`HumanOnly`).
    pub(super) fn act_purge_assets(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.mode == Mode::Headless {
            return Outcome::Refused {
                reason_key: "reason-human-only",
            };
        }
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c,
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        if !confirm {
            let plan = match project.session.purge_preview() {
                Ok(plan) => plan,
                Err(e) => return failed("error-purge", &[&purge_reason(&e)]),
            };
            if plan.remove.is_empty() {
                return Outcome::Refused {
                    reason_key: "reason-nothing-to-purge",
                };
            }
            let files = plan
                .remove
                .iter()
                .map(|f| {
                    let path = f.path.as_str();
                    path.rsplit('/').next().unwrap_or(path).to_owned()
                })
                .collect();
            let assets = plan.remove.iter().map(|f| f.sha256.to_string()).collect();
            return self.open_dialog(Dialog::ConfirmPurge { files, assets });
        }
        // Cycle 41, ruling 4 ("the purge's origin"): judged per submission like the revision steps' (ruling 1). A
        // window no person drives (the visual driver) opens and frames the confirmation, but its confirm is refused,
        // and the purge commits from `self.origin()`, never a hard-coded GUI origin.
        if !self.person {
            return Outcome::Refused {
                reason_key: "reason-human-only",
            };
        }
        // `confirm` purges exactly the files the open confirmation lists; without it nothing is purged (an empty
        // `project.assets.purge` would remove every purgeable file unconfirmed; R1-5 of cycle 31).
        let Some(Dialog::ConfirmPurge { assets, .. }) = &self.dialog else {
            return Outcome::Refused {
                reason_key: "reason-no-purge-dialog",
            };
        };
        let cmd = AnyCommand {
            name: "project.assets.purge".to_owned(),
            params: json!({ "assets": assets }),
        };
        let origin = self.origin();
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        bump(calls, |c| c.preview_with += 1);
        let result = project
            .session
            .preview_with(&cmd, Submit::from(origin))
            .and_then(|preview| {
                bump(calls, |c| c.commit += 1);
                project.session.commit(preview)
            });
        match result {
            Ok(_) => {
                if matches!(self.dialog, Some(Dialog::ConfirmPurge { .. })) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            Err(e) => failed("error-purge", &[&e.to_string()]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_read_numbers_and_dialog_texts() {
        let p = json!({"a": 3, "b": "7", "c": " ", "d": -1, "t": "true", "f": false, "x": "maybe"});
        let p = p.as_object().unwrap();
        assert_eq!(uint_param(p, "a"), Ok(Some(3)));
        assert_eq!(uint_param(p, "b"), Ok(Some(7)));
        assert_eq!(uint_param(p, "c"), Ok(None));
        assert_eq!(uint_param(p, "none"), Ok(None));
        assert!(uint_param(p, "d").is_err());
        assert_eq!(bool_text_param(p, "t", false), Ok(true));
        assert_eq!(bool_text_param(p, "f", true), Ok(false));
        assert_eq!(bool_text_param(p, "none", true), Ok(true));
        assert!(bool_text_param(p, "x", true).is_err());
        assert_eq!(folio_number("101.1"), Some(101));
        assert_eq!(folio_number("A"), None);
    }
}
