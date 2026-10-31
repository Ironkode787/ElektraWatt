// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard actions of WP-S2.05 part 3 (spec §8.5 to §8.8): copy, cut, paste, duplicate, copy tag list and the
//! headless clipboard.
//!
//! **Reads** (§9 item 4): a copy is one `Session::copy_fragment`, a tag list one `Session::tag_list`, `edit.paste_clip`
//! one `parse_fragment`, duplicate one of each of the first and third; the paste tool's moves make no `Session` call,
//! a dwell that changes the anchor or the revision one `preview_with`.

use std::sync::Arc;

use ew_api::output::ew_dl::EntityRef;
use ew_api::output::ew_dl::ew_units::{Grid, Nm};
use ew_api::output::{CopiedFragment, FragmentError, parse_fragment};
use serde_json::{Map, Value};

use super::navigation::bump;
use super::{Mode, Outcome, ShellModel, failed, string_param};
use crate::clipboard::{ClipKind, ClipboardState, copy_refusal, fragment_banner};
use crate::properties::{
    Card, CardOrigin, EditRoute, FieldReadOnly, PasteChoice, PasteConflict, PendingEdit,
};
use crate::strings;
use crate::tools::PreviewState;
use crate::tools::{PasteTool, SelectionChange, ToolOutput, tool_named};
use ew_api::DerivedDiff;
use ew_api::output::Uuid;

impl ShellModel {
    /// The shell's clipboard (§8.7).
    pub fn clipboard(&self) -> &ClipboardState {
        &self.clip
    }

    /// The page kind on show when it cannot take a paste (`edit.paste` refuses its kind); `None` on a page that can.
    ///
    /// The list mirrors the command's two page-kind checks: the kinds whose list is empty in
    /// `crates/ew-commands/src/commands/device.rs`'s `allowed_rep_types` (the title page, panel layout, model view,
    /// external document and the generated TableOfContents/SectionDivider/Report), and the generated-page arm of
    /// `crates/ew-commands/src/commands/paste.rs`'s `PasteFragment::place` (`Report | TableOfContents |
    /// SectionDivider`). `tests/clipboard.rs` checks it against a real `edit.paste` dry run for every page kind the
    /// shell knows.
    pub fn paste_page_kind(&self) -> Option<&str> {
        let kind = self.page_rows().get(self.page_index())?.kind.as_str();
        matches!(
            kind,
            "title_page"
                | "report"
                | "table_of_contents"
                | "section_divider"
                | "panel_layout"
                | "model_view"
                | "external_document"
        )
        .then_some(kind)
    }

    /// The refusal text of a paste on a page that cannot take one, with the page kind's name (`Nothing can be pasted on
    /// a title page`); `None` on a page that can.
    pub fn paste_page_reason(&self) -> Option<String> {
        self.paste_page_kind().map(|kind| {
            strings::format(
                "reason-paste-page-kind",
                &[strings::kind_label(kind).trim_end_matches(" page")],
            )
        })
    }

    /// The text a copy asked the window to put on the system clipboard, once (egui's `Context::copy_text`).
    pub fn take_clipboard_outbox(&mut self) -> Option<String> {
        self.clip.take_outbox()
    }

    /// What a copy takes of the selection: each entity the page shown draws as itself, a device or a function (a
    /// `select.set` by tag) as its representations on the page shown, anything else as it is; in selection order
    /// without repeats.
    pub(super) fn copy_targets(&self) -> Vec<EntityRef> {
        let tables = self.canvas().map(|c| Arc::clone(&c.page().tables));
        let mut out: Vec<EntityRef> = Vec::new();
        for e in &self.tools.selection.items {
            let drawn: Vec<EntityRef> = match &tables {
                Some(t) if crate::tools::entity_kind(t, *e).is_none() => t
                    .reps
                    .iter()
                    .filter(|r| r.device == Some(e.0) || r.functions.contains(&e.0))
                    .map(|r| EntityRef(r.id))
                    .collect(),
                _ => Vec::new(),
            };
            let items = if drawn.is_empty() { vec![*e] } else { drawn };
            for x in items {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
        }
        out
    }

    /// The selection as a fragment: one `copy_fragment` call; a refusal with the copy's reason.
    fn copy_selection(&mut self) -> Result<CopiedFragment, Outcome> {
        let ids: Vec<_> = self.copy_targets().iter().map(|e| e.0).collect();
        if ids.is_empty() {
            return Err(Outcome::Refused {
                reason_key: "reason-nothing-selected",
            });
        }
        let Some(project) = &self.project else {
            return Err(Outcome::Refused {
                reason_key: "reason-no-project",
            });
        };
        let copied = project.session.copy_fragment(&ids);
        bump(&self.calls, |c| c.copy_fragment += 1);
        copied.map_err(|e| Outcome::Refused {
            reason_key: copy_refusal(e),
        })
    }

    /// Writes `text` to the clipboard, and in the window to the system clipboard.
    fn write_clipboard(&mut self, text: String, kind: Option<ClipKind>, dropped: usize) {
        let gui = self.mode == Mode::Gui;
        self.clip.set(text, kind, dropped, gui);
    }

    /// `edit.copy` (Ctrl+C): the selection's fragment to the clipboard.
    pub(super) fn act_copy(&mut self) -> Outcome {
        let copied = match self.copy_selection() {
            Ok(c) => c,
            Err(out) => return out,
        };
        self.write_copied_fragment(copied);
        Outcome::Done
    }

    /// Publishes a prepared copy and announces its record count.
    fn write_copied_fragment(&mut self, copied: CopiedFragment) {
        let records = copied.records.to_string();
        self.write_clipboard(
            copied.text,
            Some(ClipKind::Fragment {
                records: copied.records,
                copies: copied.copies,
            }),
            copied.dropped_connections,
        );
        self.tools.announcement = Some(strings::format("announce-copied", &[&records]));
    }

    /// `edit.cut` (Ctrl+X): copy, then `edit.delete` of the selection through `edit.delete_selection`'s path (one undo
    /// step); the clipboard keeps the fragment.
    pub(super) fn act_cut(&mut self) -> Outcome {
        let copied = match self.copy_selection() {
            Ok(c) => c,
            Err(out) => return out,
        };
        if copied.incomplete_page_items > 0 {
            return Outcome::Refused {
                reason_key: "reason-cut-incomplete-items",
            };
        }
        self.write_copied_fragment(copied);
        let targets = self.copy_targets();
        // A device selected by tag is cut as the representations the copy took.
        if targets != self.tools.selection.items
            && self.tools.selection.apply(&SelectionChange::Replace(targets))
        {
            self.announce_selection();
        }
        self.act_delete_selection()
    }

    /// `edit.copy_tags` (Edit ▸ Copy tag list): the tags of the selected devices as text (FR-UI-040).
    pub(super) fn act_copy_tags(&mut self) -> Outcome {
        let ids: Vec<_> = self.tools.selection.items.iter().map(|e| e.0).collect();
        if ids.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-nothing-selected",
            };
        }
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let text = project.session.tag_list(&ids);
        bump(&self.calls, |c| c.tag_list += 1);
        if text.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-tags",
            };
        }
        self.write_clipboard(text, None, 0);
        Outcome::Done
    }

    /// `clipboard.text {text}` (headless): sets the clipboard as an external copy would.
    pub(super) fn act_clipboard_text(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "text") {
            Ok(Some(t)) => {
                self.clip.set(t.to_owned(), None, 0, false);
                Outcome::Done
            }
            Ok(None) => failed("error-missing-param", &["text"]),
            Err(e) => e,
        }
    }

    /// `edit.paste_clip {text?}` (Ctrl+V): the window passes its paste event's text, which becomes the clipboard's;
    /// headless the clipboard is read. One `parse_fragment`; a fragment arms the paste tool, plain text is refused
    /// (`reason-not-a-fragment`), any other error shows in the banner.
    pub(super) fn act_paste_clip(&mut self, p: &Map<String, Value>) -> Outcome {
        // A page whose kind takes no paste says so before the clipboard is read and no paste tool starts (window
        // check, item 3).
        if let Some(reason) = self.paste_page_reason() {
            self.tools.notice = Some(reason);
            self.tools.notice_sticky = false;
            return Outcome::Refused {
                reason_key: "reason-paste-page-kind",
            };
        }
        let text = match string_param(p, "text") {
            Ok(Some(t)) => {
                // The window's Ctrl+V brings the system clipboard's text; when it is what this instance copied, the
                // clipboard keeps what the copy knew (its kind and the dropped connections, doc 06 §3.6 item 5).
                if self.clip.text() != Some(t) {
                    self.clip.set(t.to_owned(), None, 0, false);
                }
                t.to_owned()
            }
            Ok(None) => match self.clip.text() {
                Some(t) => t.to_owned(),
                None => {
                    return Outcome::Refused {
                        reason_key: "reason-clipboard-empty",
                    };
                }
            },
            Err(e) => return e,
        };
        let dropped = self.clip.dropped();
        self.arm_paste(text, dropped, &[])
    }

    /// Parses `text` (one `parse_fragment`) and arms the paste tool, with `adapt` (WP-S2.07 D22). A fragment that
    /// carries another version of a held library copy that `adapt` does not name opens the paste choice card instead
    /// (one `Session::paste_versions`) and is refused with `reason-paste-versions` and the copies in the status bar; the
    /// window shows the card, a script answers it with `paste.resolve`. Plain text is refused (`reason-not-a-fragment`),
    /// any other error shows in the banner.
    pub(super) fn arm_paste(&mut self, text: String, dropped: usize, adapt: &[Uuid]) -> Outcome {
        let parsed = parse_fragment(&text);
        bump(&self.calls, |c| c.parse_fragment += 1);
        let f = match parsed {
            Ok(f) => f,
            Err(FragmentError::NotAFragment) => {
                return Outcome::Refused {
                    reason_key: "reason-not-a-fragment",
                };
            }
            Err(e) => {
                let message = fragment_banner(&e);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let conflicts: Vec<ew_api::VersionConflict> = match &self.project {
            Some(project) => project
                .session
                .paste_versions(&f.params)
                .into_iter()
                .filter(|c| !adapt.contains(&c.copy))
                .collect(),
            None => Vec::new(),
        };
        if !conflicts.is_empty() {
            return self.open_paste_choice(text, dropped, &conflicts);
        }
        self.activate_tool(Box::new(
            PasteTool::new(Arc::new(f), dropped).adapting(adapt.to_vec()),
        ));
        Outcome::Done
    }

    /// Opens the paste choice card (WP-S2.07 D22) for the copies `conflicts` and refuses the paste with
    /// `reason-paste-versions`, its list in the status bar.
    fn open_paste_choice(
        &mut self,
        text: String,
        dropped: usize,
        conflicts: &[ew_api::VersionConflict],
    ) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let class_word = |c: ew_api::ChangeClass| match c {
            ew_api::ChangeClass::Patch => "patch",
            ew_api::ChangeClass::Minor => "minor",
            ew_api::ChangeClass::Major => "major",
        };
        let choice = PasteChoice {
            text,
            dropped,
            conflicts: conflicts
                .iter()
                .map(|c| PasteConflict {
                    copy: c.copy,
                    title: c.title.clone(),
                    held: c.held.clone(),
                    pasted: c.pasted.clone(),
                    class: class_word(c.class),
                })
                .collect(),
            adapt: conflicts
                .iter()
                .any(|c| c.class == ew_api::ChangeClass::Major)
                .then_some("reason-paste-adapt-major"),
            update: conflicts
                .iter()
                .any(|c| !c.newer)
                .then_some("reason-paste-update-older"),
        };
        let list = conflicts
            .iter()
            .map(|c| c.title.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let message = strings::format("reason-paste-versions", &[&list]);
        let edit = PendingEdit {
            row: String::new(),
            kind: String::new(),
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: "edit.paste".to_owned(),
            params: Value::Null,
            typed: String::new(),
            label: String::new(),
        };
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: Value::Null,
                name: "edit.paste".to_owned(),
                result: Err(message),
                derived: DerivedDiff::default(),
            },
            edit,
            targets: 0,
            changes: 0,
            reason: "reason-paste-versions",
            origin: CardOrigin::PasteVersions(Box::new(choice)),
        });
        // Cycle 39 (FIX39b-1): the HUD's prompt follows the card ([`ShellModel::card_prompt`]), short, since the card
        // holds the choice; a stale tool notice must not stand over it.
        self.tools.notice = None;
        self.tools.notice_sticky = false;
        Outcome::Refused {
            reason_key: "reason-paste-versions",
        }
    }

    /// The prompt an open card of the paste choice puts in the HUD and the status hint in place of the tool's own
    /// (cycle 39, FIX39b-1): the choice card names the copies and points at the card; the update card that "Update the
    /// project's copy" opened says the paste follows its Apply. `None` for any other card or none.
    pub fn card_prompt(&self) -> Option<String> {
        match &self.props.card()?.origin {
            CardOrigin::PasteVersions(choice) => {
                let list = choice
                    .conflicts
                    .iter()
                    .map(|c| c.title.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let key = if choice.conflicts.len() == 1 {
                    "hud-paste-versions-one"
                } else {
                    "hud-paste-versions-many"
                };
                Some(strings::format(key, &[&list]))
            }
            CardOrigin::PasteUpdate { .. } => Some(strings::text("hud-paste-update").to_owned()),
            _ => None,
        }
    }

    /// `paste.resolve {choice}` (WP-S2.07 D22, the paste choice card's two buttons): `adapt` (Use the project's copy)
    /// closes the card and arms the paste tool with `adapt` naming the listed copies, so the paste is one `edit.paste`;
    /// `update` (Update the project's copy) opens the `library.update` card built from the fragment's documents
    /// (`Session::paste_update`), after whose Apply the shell arms the paste again (two commands, two undo steps). A
    /// disabled choice is refused with its reason.
    pub(super) fn act_paste_resolve(&mut self, p: &Map<String, Value>) -> Outcome {
        let choice = match string_param(p, "choice") {
            Ok(Some(c)) => c.to_owned(),
            Ok(None) => return failed("error-missing-param", &["choice"]),
            Err(e) => return e,
        };
        let Some(CardOrigin::PasteVersions(pending)) = self.props.card().map(|c| c.origin.clone()) else {
            return Outcome::Refused {
                reason_key: "reason-no-paste-choice",
            };
        };
        match choice.as_str() {
            "adapt" => {
                if let Some(reason_key) = pending.adapt {
                    return Outcome::Refused { reason_key };
                }
                self.props.card = None;
                let adapt: Vec<Uuid> = pending.conflicts.iter().map(|c| c.copy).collect();
                self.arm_paste(pending.text, pending.dropped, &adapt)
            }
            "update" => {
                if let Some(reason_key) = pending.update {
                    return Outcome::Refused { reason_key };
                }
                let plan = match parse_fragment(&pending.text) {
                    Ok(f) => {
                        bump(&self.calls, |c| c.parse_fragment += 1);
                        match &self.project {
                            Some(project) => project.session.paste_update(&f.params),
                            None => {
                                return Outcome::Refused {
                                    reason_key: "reason-no-project",
                                };
                            }
                        }
                    }
                    Err(e) => return failed("error-command", &[&e.to_string()]),
                };
                match plan {
                    Ok(plan) => {
                        self.props.card = None;
                        self.preview_host_card_as(
                            &plan.command.name,
                            plan.command.params,
                            "reason-library-update",
                            CardOrigin::PasteUpdate { text: pending.text },
                        )
                    }
                    Err(ew_api::ApiError::Cmd(e)) => super::tools::command_failed(&e, &Value::Null),
                    Err(e) => Outcome::Failed {
                        message: e.to_string(),
                    },
                }
            }
            other => failed("error-bad-param", &[&format!("choice {other}")]),
        }
    }

    /// Apply on the `library.update` card of "Update the project's copy" (WP-S2.07 D22): the update commits as its own
    /// step (a stale card is previewed again, as every bulk card), then the pending paste is armed again.
    pub(super) fn apply_paste_update_card(&mut self, card: Card) -> Outcome {
        let CardOrigin::PasteUpdate { text } = &card.origin else {
            return self.apply_bulk_card(card);
        };
        let text = text.clone();
        let before = self.revision();
        let out = self.apply_bulk_card(card);
        if out == Outcome::Done && self.revision() != before {
            let dropped = self.clip.dropped();
            return self.arm_paste(text, dropped, &[]);
        }
        out
    }

    /// `edit.duplicate` (Ctrl+D): the selection's fragment without touching the clipboard, floating in the paste tool
    /// two grid steps right and down of its anchor.
    pub(super) fn act_duplicate(&mut self) -> Outcome {
        let copied = match self.copy_selection() {
            Ok(c) => c,
            Err(out) => return out,
        };
        let parsed = parse_fragment(&copied.text);
        bump(&self.calls, |c| c.parse_fragment += 1);
        let f = match parsed {
            Ok(f) => f,
            Err(e) => return failed("error-command", &[&e.to_string()]),
        };
        let anchor = (
            f.params["anchor"][0].as_i64().unwrap_or(0),
            f.params["anchor"][1].as_i64().unwrap_or(0),
        );
        let num = u64::try_from(f.grid.0).unwrap_or(1);
        let den = u64::try_from(f.grid.1).unwrap_or(1);
        let start = Grid::new(num, den).ok().and_then(|g| {
            let x = g.at(g.snap(Nm(anchor.0)).checked_add(2)?)?;
            let y = g.at(g.snap(Nm(anchor.1)).checked_add(2)?)?;
            Some((x.0, y.0))
        });
        self.activate_tool(Box::new(PasteTool::new(Arc::new(f), copied.dropped_connections)));
        if let Some(start) = start {
            let out = self
                .with_tool(|t, cx| {
                    t.as_any_mut()
                        .and_then(|a| a.downcast_mut::<PasteTool>())
                        .map(|p| p.start_at(start, cx))
                })
                .flatten();
            if let Some(ToolOutput::Transient(o)) = out {
                self.tools.overlay = o;
            }
        }
        Outcome::Done
    }

    /// After a committed paste (§8.5): Select takes over with the pasted representations selected, one selection change.
    /// The pasted representations are those the page tables hold now and did not before the commit.
    pub(super) fn after_paste(&mut self) {
        if !std::mem::take(&mut self.tools.pasted) {
            return;
        }
        let before = self.tools.before_commit.clone();
        let Some(now) = self.canvas().map(|c| Arc::clone(&c.page().tables)) else {
            return;
        };
        let pasted: Vec<EntityRef> = now
            .reps
            .iter()
            .filter(|r| {
                before
                    .as_ref()
                    .is_none_or(|b| !b.reps.iter().any(|o| o.id == r.id))
            })
            .map(|r| EntityRef(r.id))
            .collect();
        if let Some(select) = tool_named("select") {
            self.activate_tool(select);
        }
        if self.tools.selection.apply(&SelectionChange::Replace(pasted)) {
            self.announce_selection();
        }
    }
}
