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
use crate::strings;
use crate::tools::{PasteTool, SelectionChange, ToolOutput, tool_named};

impl ShellModel {
    /// The shell's clipboard (§8.7).
    pub fn clipboard(&self) -> &ClipboardState {
        &self.clip
    }

    /// The text a copy asked the window to put on the system clipboard, once (egui's `Context::copy_text`).
    pub fn take_clipboard_outbox(&mut self) -> Option<String> {
        self.clip.take_outbox()
    }

    /// What a copy takes of the selection: each entity the page shown draws as itself, a device or a function (a
    /// `select.set` by tag) as its representations on the page shown, anything else as it is; in selection order
    /// without repeats.
    fn copy_targets(&self) -> Vec<EntityRef> {
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
        Outcome::Done
    }

    /// `edit.cut` (Ctrl+X): copy, then `edit.delete` of the selection through `edit.delete_selection`'s path (one undo
    /// step); the clipboard keeps the fragment.
    pub(super) fn act_cut(&mut self) -> Outcome {
        let targets = self.copy_targets();
        let out = self.act_copy();
        if out != Outcome::Done {
            // A refused copy (several pages, nothing copyable) leaves the selection as it was.
            return out;
        }
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
        let parsed = parse_fragment(&text);
        bump(&self.calls, |c| c.parse_fragment += 1);
        match parsed {
            Ok(f) => {
                let dropped = self.clip.dropped();
                self.activate_tool(Box::new(PasteTool::new(Arc::new(f), dropped)));
                Outcome::Done
            }
            Err(FragmentError::NotAFragment) => Outcome::Refused {
                reason_key: "reason-not-a-fragment",
            },
            Err(e) => {
                let message = fragment_banner(&e);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
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
