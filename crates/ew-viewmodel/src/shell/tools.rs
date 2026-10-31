// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host in the shell (WP-S2.03 part 2, spec §7.2 to §7.9): one active tool per window acting on the
//! focused group's canvas, its overlay, the preview protocol over `Session::preview_with`, the Escape chain, the
//! keyboard tool input, the link hover preview and the selection.
//!
//! **Outputs** (§7.2). `Transient` replaces the overlay; `Preview` runs `Session::preview_with` once (a repeat of
//! the kept command on the kept revision is ignored) and keeps the overlay; `Commit` commits a fresh, equal preview
//! with `Session::commit` in the window, else `Session::execute_with` (a preview and its commit for a bulk command),
//! and headless always runs a one-line command log through `Session::run_script`, exactly `command.run`'s path,
//! then clears the overlay and the preview; `Cancel` clears both; `Select` changes the selection and clears the
//! overlay; `Follow` follows the page's link to the destination.
//!
//! **Context.** The tool works on the focused group's active tab and its page. When that (group, tab, page) changes
//! (a click into the other group, a tab switch, a jump, a walk), a busy tool is sent `Escape` and the overlay, the
//! preview and the link preview are cleared, so an overlay never shows on a page it was not made on.
//!
//! **Time.** Nothing here is timed: pointer rest is the action `canvas.dwell` with its duration (D11).

use std::sync::Arc;

use ew_api::output::ew_dl::Destination;
use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::{AnyCommand, CmdError, DynCommand, Submit};
use serde_json::{Map, Value};

use super::canvas_actions::point_mm;
use super::navigation::bump;
use super::{Focus, Mode, Outcome, RecordedCommand, ShellModel, failed, flag};
use crate::canvas::{CanvasModel, CanvasPage, PageId, reading_key};
use crate::strings;
use crate::tools::{
    Button, CanvasEvent, LINK_PREVIEW_DWELL_MS, LINK_PREVIEW_PANE, LinkPreview, MarkerKind, Mods, Overlay,
    PreviewInfo, PreviewState, PreviewStatus, ResultNote, Selection, SelectionChange, Tool, ToolCx,
    ToolOutput, tool_label, tool_named,
};
use ew_api::output::ew_dl::EntityRef;

/// A connection-point step of the keyboard (§7.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CpStep {
    /// `cp.next`.
    Next,
    /// `cp.previous`.
    Previous,
    /// `cp.next_unconnected`.
    NextUnconnected,
}

/// The snap indicator the overlay shows (the transcript's `snap`, the describe text's `canvas snap` line).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SnapIndicator {
    /// The marker's shape.
    pub kind: MarkerKind,
    /// Where it stands on the sheet.
    pub at_nm: (i64, i64),
    /// The connection point's label for a connection-point marker.
    pub label: Option<String>,
}

/// The modifiers of an action's parameters (`shift`, `ctrl`, `alt`).
fn mods_param(p: &Map<String, Value>) -> Mods {
    Mods {
        shift: flag(p, "shift"),
        ctrl: flag(p, "ctrl"),
        alt: flag(p, "alt"),
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The active tool.
    pub fn tool(&self) -> &dyn Tool {
        self.tools.tool.as_ref()
    }

    /// The active-tool replacement generation; ordinary events and field edits leave it unchanged.
    pub fn tool_epoch(&self) -> u64 {
        self.tool_epoch
    }

    /// The active tool's overlay (sheet nm of the focused canvas's page).
    pub fn tool_overlay(&self) -> &Overlay {
        &self.tools.overlay
    }

    /// The window's selection.
    pub fn selection(&self) -> &Selection {
        &self.tools.selection
    }

    /// The kept preview of the active tool's command.
    pub fn preview_state(&self) -> Option<&PreviewState> {
        self.tools.preview.as_ref()
    }

    /// The kept preview's status at the current revision.
    pub fn preview_status(&self) -> Option<PreviewStatus> {
        let rev = self.revision()?;
        self.tools.preview.as_ref().map(|p| p.status(rev))
    }

    /// The link hover preview, while shown.
    pub fn link_preview(&self) -> Option<&LinkPreview> {
        self.tools.link_preview.as_ref()
    }

    /// What the canvas node's live region says instead of the cursor (a tool's state or a selection change).
    pub fn tool_announcement(&self) -> Option<&str> {
        self.tools.announcement.as_deref()
    }

    /// The active tool's status hint; while the picker's strip chooser is open (FT-421), what it waits for instead
    /// (cycle 36, play-test rv-txt-2: the switch to the strip list was silent).
    pub fn tool_hint(&self) -> &'static str {
        if self.strip_chooser_open() {
            return strings::text("hint-choose-strip");
        }
        strings::text(self.tools.tool.status_hint())
    }

    /// Whether the picker is open as the strip chooser of a terminal placement (FT-421).
    fn strip_chooser_open(&self) -> bool {
        self.tools
            .picker
            .as_ref()
            .is_some_and(|p| p.mode == crate::tools::PickerMode::Strip)
    }

    /// The snap indicator the overlay shows: its last marker, with the connection point's label.
    pub fn snap_indicator(&self) -> Option<SnapIndicator> {
        let (at, kind) = self.tools.overlay.snap_marker()?;
        let label = match kind {
            MarkerKind::Cp => self
                .canvas()
                .and_then(|c| c.page().cps.iter().find(|cp| cp.at_nm == at))
                .map(|cp| cp.label.clone()),
            _ => None,
        };
        Some(SnapIndicator {
            kind,
            at_nm: at,
            label,
        })
    }

    /// The rectangles of the selected entities drawn on group `group`'s page (the `selected` outline).
    pub fn selected_rects(&self, group: usize) -> Vec<Rect> {
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        if self.tools.selection.is_empty() {
            return Vec::new();
        }
        c.page()
            .entities
            .iter()
            .filter(|(e, _)| self.tools.selection.contains(*e))
            .map(|(_, r)| *r)
            .collect()
    }

    // FT-705
    /// The sheet points of the connection points of group `group`'s page that have no connection on the page, in the
    /// page tables' order: the window marks each open (a session overlay like the CP rings, doc 16a §7), so that a
    /// wire drawn across one is visibly not connected to it. The page's display list, its print and its export do not
    /// change. A terminal left unwired by design (a spare) shows the mark too.
    pub fn open_cps(&self, group: usize) -> Vec<(i64, i64)> {
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        let page = c.page();
        page.tables
            .cps
            .iter()
            .filter(|cp| cp.connections.is_empty())
            .map(|cp| page.snapper.page_to_sheet(cp.at))
            .collect()
    }

    /// Makes `tool` the active tool, as a tool of the registry would be (feature `testkit` only: the probe tool of
    /// the tests exercises the commit path, which no shipped tool takes; spec §7.6).
    #[cfg(feature = "testkit")]
    pub fn set_tool_for_tests(&mut self, tool: Box<dyn Tool>) {
        self.activate_tool(tool);
        self.sync_views();
        self.sync_tool_context();
    }

    // -----------------------------------------------------------------------------------------------------------
    // The host

    /// A fresh host: Select active, nothing selected (a project opened or closed).
    pub(super) fn reset_tools(&mut self) {
        self.tools = crate::tools::ToolHost::new();
        self.tool_epoch = self.tool_epoch.wrapping_add(1);
    }

    /// The (group, tab ID, page) the active tool works on.
    fn tool_context(&self) -> Option<(usize, u32, PageId)> {
        let a = self.area.as_ref()?;
        let tab = a.current();
        Some((a.focused, tab.id, tab.page))
    }

    /// Escapes a busy tool and clears the overlay, the preview and the link preview when the canvas the tool works
    /// on changed (the end of every dispatch).
    pub(super) fn sync_tool_context(&mut self) {
        let now = self.tool_context();
        if now == self.tools.context {
            return;
        }
        let first = self.tools.context.is_none();
        self.tools.context = now;
        if first {
            return;
        }
        // A press held across the change: the tool is escaped below and the release that follows is dropped (FT-392).
        if std::mem::take(&mut self.tools.press_held) {
            self.tools.drop_release = true;
        }
        if let Some(out) = self.run_tool(&CanvasEvent::Escape) {
            let _ = self.apply_tool_output(out);
        }
        self.tools.overlay = Overlay::default();
        self.tools.preview = None;
        self.tools.link_preview = None;
        self.tools.announcement = None;
        self.tools.last_cp = None;
        // A page or tab change ends the gesture a sticky refusal belonged to.
        self.tools.notice = None;
        self.tools.notice_sticky = false;
        // An editing tool (Wire, Edit route, Place, Change symbol, and the part-4b Text, Text edit, Graphic, Wire
        // marker, Structure box and part-5 Image tools) works on one page (WP-S2.04 part 2, window-check ruling): a page or tab
        // change ends it and Select takes over. The selection survives (WP-S2.03 §7.6).
        if matches!(
            self.tools.tool.name(),
            "wire"
                | "route_edit"
                | "place"
                | "exchange"
                | "paste"
                | "ip"
                | "black_box"
                | "text"
                | "text_edit"
                | "graphic"
                | "marker"
                | "structure_box"
                | "image"
        ) && let Some(select) = tool_named("select")
        {
            self.tools.tool = select;
            self.tool_epoch = self.tool_epoch.wrapping_add(1);
            self.tools.junction_editor = None;
            self.announce_tool();
        }
    }

    /// The page, zoom and pane of the focused canvas.
    fn focused_canvas(&self) -> Option<(Arc<CanvasPage>, f64, (f64, f64))> {
        let c = self.canvas()?;
        Some((Arc::clone(c.page()), c.points_per_nm(), c.pane()))
    }

    /// Hands `ev` to the active tool with the focused canvas's context; `None` without a canvas. A state change that
    /// changes the tool's hint is announced (`<tool>: <hint>`); one that leaves the hint as it was (Select's pressed
    /// and rubber-band states) is not, so a click or a drag does not repeat the hint to a screen reader.
    fn run_tool(&mut self, ev: &CanvasEvent) -> Option<ToolOutput> {
        let (page, points_per_nm, pane) = self.focused_canvas()?;
        // FT-822: the IP tool's proposal reads the project's source points of this revision.
        self.sync_ip_project();
        let before = self.tools.tool.status_hint();
        let out = {
            let host = &mut self.tools;
            let cx = ToolCx {
                page: &page,
                points_per_nm,
                pane,
                snapper: &page.snapper,
                selection: &host.selection,
            };
            host.tool.on_event(ev, &cx)
        };
        let after = self.tools.tool.status_hint();
        if after != before {
            self.announce_tool();
        }
        // A click that produces a readout (Measure's distance) says it: the result reaches a screen reader, not only
        // the pixels and `ui.describe` (NFR-A11Y-01).
        if matches!(ev, CanvasEvent::Press { .. } | CanvasEvent::Activate)
            && let ToolOutput::Transient(o) = &out
        {
            let labels: Vec<&str> = o.labels().collect();
            if !labels.is_empty() {
                self.tools.announcement = Some(labels.join("; "));
            }
        }
        Some(out)
    }

    /// Announces the active tool and its hint through the live region.
    pub(super) fn announce_tool(&mut self) {
        let label = tool_label(self.tools.tool.name());
        self.tools.announcement = Some(strings::format(
            "announce-tool",
            &[&label, strings::text(self.tools.tool.status_hint())],
        ));
    }

    /// Hands `ev` to the active tool and acts on its output (§7.2).
    pub(super) fn send_tool(&mut self, ev: &CanvasEvent) -> Outcome {
        // A refused key's notice stands in the status bar until the next canvas input (WP-S2.04 part 2). A tool
        // refusal is sticky for the rest of its gesture: its release keeps it and the post-release hover keeps it, so
        // the user reads it after the gesture; the next primary press, key press, page change or tool change clears it
        // (window re-check W1). While the tool is still busy a move or a dwell clears it first: a valid move ends the
        // refusal and brings the ghost back, and a refused one sets the notice again at once (round 5, R1).
        let idle = self.tools.tool.state() == "idle";
        let keeps = self.tools.notice_sticky
            && match ev {
                CanvasEvent::Release { .. } => true,
                // FT-751: a move to the refused press's own point is no move (the window reports one after each
                // press), so a busy tool's refusal stands until the pointer really moves.
                CanvasEvent::PointerMove { at, .. } => idle || self.tools.notice_at == Some(*at),
                CanvasEvent::Dwell { .. } => idle || self.tools.notice_at.is_some(),
                _ => false,
            };
        if !keeps {
            self.tools.notice = None;
            self.tools.notice_sticky = false;
            self.tools.notice_at = None;
        }
        // A place tool that asks for the next designation refuses a click or Enter with the reason (WP-S2.04 §7.2).
        if matches!(ev, CanvasEvent::Press { .. } | CanvasEvent::Activate) && self.place_needs_designation() {
            return Outcome::Refused {
                reason_key: "reason-designation-needed",
            };
        }
        match self.run_tool(ev) {
            Some(out) => {
                let refused = matches!(out, ToolOutput::Refused(_));
                let committed = matches!(out, ToolOutput::Commit(_));
                let out = self.apply_tool_output(out);
                // The refused press's point; a refused Enter (no pointer point) is kept until the pointer or the
                // grid cursor moves. A refused commit (FT-822: an interruption point a prevent rule refuses) too.
                if refused || (committed && matches!(out, Outcome::Refused { .. })) {
                    self.tools.notice_at = match ev {
                        CanvasEvent::Press { at, .. } => Some(*at),
                        CanvasEvent::Activate => Some((i64::MIN, i64::MIN)),
                        _ => None,
                    };
                }
                // The place tool says what it floats after a key or a cursor move (WP-S2.04 §7.2).
                if self.tools.tool.name() == "place"
                    && matches!(ev, CanvasEvent::Key(_) | CanvasEvent::CursorMove { .. })
                {
                    self.announce_place();
                }
                // The Wire tool says where it routes from once it starts (WP-S2.04 §8.2).
                if self.tools.tool.name() == "wire"
                    && self.tools.tool.state() == "routing"
                    && matches!(
                        ev,
                        CanvasEvent::Press { .. }
                            | CanvasEvent::Activate
                            | CanvasEvent::Key(crate::tools::ToolKey::Finish)
                    )
                {
                    self.announce_wire();
                }
                out
            }
            None => Outcome::Refused {
                reason_key: "reason-no-page",
            },
        }
    }

    /// The host's action on a tool's output (§7.2).
    pub(super) fn apply_tool_output(&mut self, out: ToolOutput) -> Outcome {
        match out {
            ToolOutput::None => Outcome::Done,
            ToolOutput::Transient(o) => {
                self.tools.overlay = o;
                Outcome::Done
            }
            ToolOutput::Preview(cmd) => {
                let cmd = self.without_held_copies(cmd);
                self.run_preview(cmd.as_ref());
                Outcome::Done
            }
            // WP-S1.19 part 6: the macro tool's click is a host step, no command.
            ToolOutput::Host(crate::tools::HostStep::MacroPlace { key, at }) => {
                self.macro_place_from_tool(&key, at)
            }
            ToolOutput::Commit(cmd) => {
                let cmd = self.without_held_copies(cmd);
                self.tools.before_commit = self.canvas().map(|c| Arc::clone(&c.page().tables));
                // FT-485 item 3: the wires a placement draws by its autoconnect hints, said in the toast.
                let connects = self.autoconnect_words(cmd.as_ref());
                // WP-S2.09 D6: an earlier toast stays (its Undo expires with this commit); a new toast of key `edit`
                // replaces it.
                let out = self.run_commit(cmd.as_ref());
                // FT-822: a refused interruption point keeps its ghost where it was and says why.
                let (out, keep_ghost) = self.ip_commit_refused(out);
                // FT-546: every placement answers with its result line (the tag and the cell, and the wires it drew),
                // said at the end of the dispatch, once the page tables show it.
                if out == Outcome::Done
                    && cmd.name() == "device.place"
                    && let Ok(json) = cmd.to_json()
                    && let (Some(x), Some(y)) = (json["pos"][0].as_i64(), json["pos"][1].as_i64())
                {
                    self.tools.result_note = Some(ResultNote::Placed {
                        pos: (x, y),
                        wires: connects,
                    });
                }
                self.tools.tool.on_commit(out == Outcome::Done);
                if !keep_ghost {
                    self.tools.overlay = Overlay::default();
                }
                self.tools.preview = None;
                // WP-S2.05 part 3: a committed paste returns to Select with the pasted representations selected.
                self.tools.pasted = out == Outcome::Done && self.tools.tool.name() == "paste";
                // The IP tool's memory and its next name (WP-S2.04 part 3, §9.1).
                if out == Outcome::Done {
                    self.after_ip_commit();
                }
                // After Select commits a move or a transform, the selection is the moved representations (one
                // selection change, no command; WP-S2.04 §6.4 item 3).
                if out == Outcome::Done
                    && self.tools.tool.name() == "select"
                    && matches!(cmd.name(), "rep.move" | "rep.transform")
                    && let Ok(params) = cmd.to_json()
                {
                    let reps: Vec<EntityRef> = params["reps"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| {
                            let id = v.as_str()?;
                            self.canvas()?
                                .page()
                                .entities
                                .iter()
                                .find(|(e, _)| e.0.to_string() == id)
                                .map(|(e, _)| *e)
                        })
                        .collect();
                    if !reps.is_empty() && self.tools.selection.apply(&SelectionChange::Replace(reps)) {
                        self.announce_selection();
                    }
                }
                out
            }
            ToolOutput::Cancel => {
                self.tools.overlay = Overlay::default();
                self.tools.preview = None;
                Outcome::Done
            }
            ToolOutput::Refused(reason_key) => {
                // The same path as a refused canvas key: the status bar shows the reason until the next canvas input.
                // A refusal ends the gesture it answers: its ghost and its kept preview go, as a refused release's
                // `Select` and the old `Transient(Overlay::default())` did (review fix, F1).
                self.tools.overlay = Overlay::default();
                self.tools.preview = None;
                self.tools.notice = Some(strings::text(reason_key).to_owned());
                self.tools.notice_sticky = true;
                Outcome::Refused { reason_key }
            }
            ToolOutput::Select(change) => {
                self.tools.overlay = Overlay::default();
                if self.tools.selection.apply(&change) {
                    self.announce_selection();
                }
                Outcome::Done
            }
            ToolOutput::Follow(dest) => {
                self.tools.overlay = Overlay::default();
                self.follow_dest(dest)
            }
        }
    }

    /// FT-656, FT-673: a placement (`device.place`, `box.black`, `ip.place`, `connection.draw_to_ip`) or an exchange
    /// (`rep.exchange_symbol`) with the `copies` its tool armed with, less those the project holds now: the first
    /// placement embeds them, a later one carries none, and one after an undo of the first carries them again. Any
    /// other command is returned as it is.
    fn without_held_copies(&self, cmd: Box<dyn DynCommand>) -> Box<dyn DynCommand> {
        if !matches!(
            cmd.name(),
            "device.place" | "box.black" | "ip.place" | "connection.draw_to_ip" | "rep.exchange_symbol"
        ) {
            return cmd;
        }
        let (Some(project), Ok(mut params)) = (self.project.as_ref(), cmd.to_json()) else {
            return cmd;
        };
        let Some(copies) = params.get("copies").and_then(Value::as_array) else {
            return cmd;
        };
        let missing = project.session.missing_copies(copies);
        if missing.len() == copies.len() {
            return cmd;
        }
        match params.as_object_mut() {
            Some(m) if missing.is_empty() => {
                m.remove("copies");
            }
            Some(m) => {
                m.insert("copies".to_owned(), Value::Array(missing));
            }
            None => return cmd,
        }
        Box::new(AnyCommand {
            name: cmd.name().to_owned(),
            params,
        })
    }

    /// For a `device.place` with autoconnect hints (`connect`): the number of wires it draws as words (`1 wire`,
    /// `2 wires`) and the proposals the ghost showed (`"2" -> "-101F2:2"`), from the Place tool's ghost; `None` for any
    /// other command or a placement without hints (Alt, or no proposal).
    fn autoconnect_words(&self, cmd: &dyn DynCommand) -> Option<(String, String)> {
        if cmd.name() != "device.place" {
            return None;
        }
        let hints = cmd.to_json().ok()?["connect"].as_array().map_or(0, Vec::len);
        if hints == 0 {
            return None;
        }
        let Some(crate::tools::Ghost::Place { proposals, .. }) = self.tools.tool.ghost() else {
            return None;
        };
        let list: Vec<String> = proposals
            .iter()
            // FX38d: a shared pin designation reads by its position in words in the sentence, not as `-#2`.
            .filter_map(|(cp, target)| {
                target
                    .as_ref()
                    .map(|t| format!("{cp} -> \"{}\"", crate::describe::pin_in_sentence(t)))
            })
            .collect();
        let count = strings::format(
            if hints == 1 { "count-wire" } else { "count-wires" },
            &[&hints.to_string()],
        );
        Some((count, list.join(", ")))
    }

    /// Announces the selection's size through the live region.
    pub(super) fn announce_selection(&mut self) {
        self.tools.announcement = Some(strings::format(
            "announce-selected",
            &[&self.tools.selection.items.len().to_string()],
        ));
    }

    /// Follows the focused canvas's link to `dest` (the first in reading order).
    fn follow_dest(&mut self, dest: Destination) -> Outcome {
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        let index = self
            .canvas_of(g)
            .and_then(|c| c.page().links.iter().position(|l| l.dest == dest));
        match index {
            Some(i) => self.follow_link(g, i),
            None => Outcome::Refused {
                reason_key: "reason-no-link",
            },
        }
    }

    /// The submission of the shell's tool commands: the GUI in the window, the script's command log headless.
    pub(super) fn tool_submit(&self) -> Submit {
        Submit::from(super::ShellModel::origin_of(
            self.mode,
            self.person,
            &self.origin_file,
        ))
    }

    /// `Preview(cmd)` (§7.3): `Session::preview_with` once, the semantic ghost from `Session::derived_diff`; a
    /// preview of the kept command on the kept revision is not run again.
    fn run_preview(&mut self, cmd: &dyn DynCommand) {
        let Some(rev) = self.revision() else { return };
        let json = match cmd.to_json() {
            Ok(v) => v,
            Err(e) => {
                self.tools.preview = Some(PreviewState {
                    base_rev: rev,
                    command: Value::Null,
                    name: cmd.name().to_owned(),
                    result: Err(e.to_string()),
                    derived: Default::default(),
                });
                return;
            }
        };
        if self
            .tools
            .preview
            .as_ref()
            .is_some_and(|p| p.base_rev == rev && p.name == cmd.name() && p.command == json)
        {
            return;
        }
        let submit = self.tool_submit();
        let Some(project) = &mut self.project else { return };
        let result = project.session.preview_with(cmd, submit);
        let derived = match &result {
            Ok(p) => project.session.derived_diff(p),
            Err(_) => Default::default(),
        };
        self.count(|c| c.preview_with += 1);
        // What the tool may show of its preview (WP-S2.04: the predicted tag, the unmapped CPs).
        let info = PreviewInfo {
            ok: result.is_ok(),
            tags: derived
                .tags
                .iter()
                .map(|c| (c.before.clone(), c.after.clone()))
                .collect(),
            notices: result
                .as_ref()
                .map(|p| p.outcome().notices.clone())
                .unwrap_or_default(),
            message: result.as_ref().err().map(ToString::to_string),
        };
        self.tools.preview = Some(PreviewState {
            base_rev: rev,
            command: json,
            name: cmd.name().to_owned(),
            result: result.map_err(|e| e.to_string()),
            derived,
        });
        if let Some(overlay) = self.tools.tool.on_preview(&info) {
            self.tools.overlay = overlay;
        }
    }

    /// `Commit(cmd)` (§7.3): in the window `Session::commit` of a fresh, equal preview, else `Session::execute_with`
    /// (a preview and its commit for a bulk command); headless a one-line command log through `Session::run_script`,
    /// exactly `command.run`'s path, so the exposure rules and `--record` apply.
    pub(super) fn run_commit(&mut self, cmd: &dyn DynCommand) -> Outcome {
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: if self.has_project() {
                    "reason-read-only"
                } else {
                    "reason-no-project"
                },
            };
        }
        let json = match cmd.to_json() {
            Ok(v) => v,
            Err(e) => return command_failed(&e, &Value::Null),
        };
        let name = cmd.name().to_owned();
        if self.mode == Mode::Headless {
            return self.run_logged(name, json);
        }
        let rev = self.revision();
        let fresh = self.tools.preview.as_ref().is_some_and(|p| {
            Some(p.base_rev) == rev && p.name == name && p.command == json && p.result.is_ok()
        });
        let kept = if fresh { self.tools.preview.take() } else { None };
        // Cycle 41, ruling 1: the submission's own origin (a scripted window submits as its command log).
        let origin = self.origin();
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let result = match kept.map(|p| p.result) {
            Some(Ok(preview)) => {
                bump(calls, |c| c.commit += 1);
                project.session.commit(preview)
            }
            _ => {
                bump(calls, |c| c.execute_with += 1);
                match project.session.execute_with(cmd, Submit::from(origin.clone())) {
                    Err(CmdError::NeedsPreview { .. }) => {
                        bump(calls, |c| {
                            c.preview_with += 1;
                            c.commit += 1;
                        });
                        project
                            .session
                            .preview_with(cmd, Submit::from(origin))
                            .and_then(|p| project.session.commit(p))
                    }
                    other => other,
                }
            }
        };
        let outcome = match result {
            Ok(receipt) => {
                self.tools.last_primary = receipt
                    .outcome
                    .primary
                    .and_then(|k| serde_json::to_value(k.1).ok())
                    .and_then(|v| v.as_str().map(str::to_owned));
                self.record.undone.clear();
                self.record.steps.push(RecordedCommand {
                    cmd: name,
                    params: json,
                });
                Outcome::Done
            }
            Err(e) => command_failed(&e, &json),
        };
        self.refresh();
        outcome
    }

    /// Makes `tool` the active tool: the current one is cancelled (sent `Escape`), the overlay and the preview are
    /// cleared, the new tool is announced and learns where the grid cursor is.
    pub(super) fn activate_tool(&mut self, tool: Box<dyn Tool>) {
        if let Some(out) = self.run_tool(&CanvasEvent::Escape) {
            let _ = self.apply_tool_output(out);
        }
        self.tools.overlay = Overlay::default();
        self.tools.preview = None;
        // A tool change ends the gesture a sticky refusal belonged to.
        self.tools.notice = None;
        self.tools.notice_sticky = false;
        self.tools.tool = tool;
        self.tool_epoch = self.tool_epoch.wrapping_add(1);
        self.announce_tool();
        if let Some(at) = self.canvas().and_then(CanvasModel::cursor) {
            let _ = self.send_tool(&CanvasEvent::CursorMove { at });
        }
    }

    /// The tool's part of the Escape chain (§7.2): `Escape` to the tool; when it answers `None` (its first state), a
    /// tool other than Select is replaced by Select, and Select with a selection clears it.
    pub(super) fn dismiss_tool(&mut self) -> Outcome {
        match self.run_tool(&CanvasEvent::Escape) {
            None => Outcome::Done,
            Some(ToolOutput::None) => {
                if self.tools.tool.name() != "select" {
                    if let Some(select) = tool_named("select") {
                        self.activate_tool(select);
                    }
                } else if self.tools.selection.clear() {
                    self.announce_selection();
                }
                Outcome::Done
            }
            Some(out) => self.apply_tool_output(out),
        }
    }

    /// The grid cursor of group `g` moved: the active tool learns it when `g` is the focused group (§7.7).
    pub(super) fn cursor_moved(&mut self, g: usize) {
        self.tools.announcement = None;
        if self.area.as_ref().map(|a| a.focused) != Some(g) {
            return;
        }
        if let Some(at) = self.canvas_of(g).and_then(CanvasModel::cursor) {
            let _ = self.send_tool(&CanvasEvent::CursorMove { at });
        }
    }

    /// The pointer of group `g` moved to `at`: the active tool learns it when `g` is the focused group.
    pub(super) fn pointer_moved(&mut self, g: usize, at: (i64, i64), mods: Mods) -> Outcome {
        if self.area.as_ref().map(|a| a.focused) != Some(g) {
            return Outcome::Done;
        }
        self.send_tool(&CanvasEvent::PointerMove { at, mods })
    }

    // -----------------------------------------------------------------------------------------------------------
    // Actions

    /// `tool.select`, `tool.measure`.
    pub(super) fn act_tool(&mut self, name: &str) -> Outcome {
        match tool_named(name) {
            Some(tool) => {
                self.activate_tool(tool);
                Outcome::Done
            }
            None => failed("error-unknown-action", &[name]),
        }
    }

    /// `canvas.press` and `canvas.release` (§7.9): `x_mm`, `y_mm`, optional `button` (`primary`, `middle`),
    /// `shift`, `ctrl`, `alt` and `group`. A press in the other group focuses it first; the middle button reaches
    /// no tool (it pans). A primary release after the tool's context changed under its press is dropped: no focus,
    /// pointer, tool or selection change (FT-392).
    pub(super) fn act_press(&mut self, p: &Map<String, Value>, release: bool) -> Outcome {
        let at = match point_mm(p) {
            Ok(Some(at)) => at,
            Ok(None) => return failed("error-missing-param", &["x_mm"]),
            Err(e) => return e,
        };
        let button = match p.get("button") {
            None => Button::Primary,
            Some(v) => match v.as_str() {
                Some("primary") => Button::Primary,
                Some("middle") => Button::Middle,
                _ => return failed("error-bad-param", &["button"]),
            },
        };
        if button == Button::Primary {
            if release && std::mem::take(&mut self.tools.drop_release) {
                return Outcome::Done;
            }
            // Cycle 36 (play-test rv-txt-2): while the strip chooser waits for a strip, a press on the canvas starts
            // nothing (the Select tool under it began a rubber band); the hint says what is wanted, and the press's
            // release is dropped with it.
            if !release && self.strip_chooser_open() {
                self.tools.press_held = false;
                self.tools.drop_release = true;
                return Outcome::Refused {
                    reason_key: "reason-choose-strip",
                };
            }
            // A new press: a release that never came is forgotten (before the focus moves, so the press's own
            // context change drops nothing).
            self.tools.press_held = false;
            self.tools.drop_release = false;
        }
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        if let Some(a) = &mut self.area
            && a.focused != g
        {
            a.focused = g;
        }
        if self.focus.is_some() {
            self.focus = Some(Focus::Canvas(g));
        }
        self.sync_tool_context();
        let Some(c) = self.canvas_mut(g) else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        c.set_pointer(Some(at));
        if let Some(area) = &mut self.area {
            for (k, group) in area.groups.iter_mut().enumerate() {
                if k != g
                    && let Some(c) = &mut group.active_tab_mut().canvas
                    && c.pointer().is_some()
                {
                    c.set_pointer(None);
                }
            }
        }
        if button != Button::Primary {
            return Outcome::Done;
        }
        let mods = mods_param(p);
        let ev = if release {
            CanvasEvent::Release { at, button, mods }
        } else {
            CanvasEvent::Press { at, button, mods }
        };
        self.tools.press_held = !release;
        self.send_tool(&ev)
    }

    /// `canvas.dwell` (§7.8, D11): `ms`, a positive integer. At 300 ms and more with a hovered link (while the tool
    /// wants hover) the link preview shows its target; the tool gets `Dwell` either way.
    pub(super) fn act_dwell(&mut self, p: &Map<String, Value>) -> Outcome {
        let ms = match p.get("ms") {
            None => return failed("error-missing-param", &["ms"]),
            Some(v) => match v.as_u64().and_then(|m| u32::try_from(m).ok()).filter(|m| *m > 0) {
                Some(ms) => ms,
                None => return failed("error-bad-param", &["ms"]),
            },
        };
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        if ms >= LINK_PREVIEW_DWELL_MS && self.tools.tool.wants_hover() {
            let target = c.hovered_link().and_then(|l| l.target.clone());
            let zoom = c.zoom();
            let ppp = c.ppp();
            if let Some(target) = target
                && let Ok(page) = self.page_for(&target.folio)
            {
                let mut view = CanvasModel::new(page, LINK_PREVIEW_PANE, ppp);
                view.set_ui_scale(self.canvas_ui_scale());
                if let Some(at) = target.at_nm {
                    view.jump_view(at, zoom);
                }
                self.tools.link_preview = Some(LinkPreview { target, canvas: view });
            }
        }
        self.send_tool(&CanvasEvent::Dwell { ms })
    }

    /// `canvas.activate` (Enter): `Activate` to the tool, at the grid cursor; with Select idle and exactly one junction
    /// selected, the junction sequence editor opens instead (WP-S2.04 §8.5). With `alt` (headless; WP-S2.04 part 2) it
    /// is a click with Alt held at the grid cursor (a press and a release): the keyboard's way to place a symbol without
    /// its autoconnect proposals (FR-CON-003) or to fix a wire's bend without object snaps.
    pub(super) fn act_activate_with(&mut self, p: &Map<String, Value>) -> Outcome {
        if !flag(p, "alt") {
            return self.act_activate();
        }
        let Some(at) = self.canvas().and_then(CanvasModel::cursor) else {
            return Outcome::Refused {
                reason_key: "reason-no-cursor",
            };
        };
        let mods = Mods {
            alt: true,
            ..Mods::default()
        };
        let _ = self.send_tool(&CanvasEvent::CursorMove { at });
        let out = self.send_tool(&CanvasEvent::Press {
            at,
            button: Button::Primary,
            mods,
        });
        if out != Outcome::Done {
            return out;
        }
        self.send_tool(&CanvasEvent::Release {
            at,
            button: Button::Primary,
            mods,
        })
    }

    /// `canvas.activate` without parameters (see [`ShellModel::act_activate_with`]).
    pub(super) fn act_activate(&mut self) -> Outcome {
        if let Some(out) = self.enter_opens_junction() {
            return out;
        }
        // The keyboard route editor commits without a grid cursor (its segment stays where it is), and so does the paste
        // tool (its ghost stays where duplicate or the pointer put it; WP-S2.05 part 3).
        if matches!(self.tools.tool.name(), "route_edit" | "paste")
            && self.canvas().and_then(CanvasModel::cursor).is_none()
        {
            return self.send_tool(&CanvasEvent::Activate);
        }
        let Some(at) = self.canvas().and_then(CanvasModel::cursor) else {
            return Outcome::Refused {
                reason_key: "reason-no-cursor",
            };
        };
        let _ = self.send_tool(&CanvasEvent::CursorMove { at });
        self.send_tool(&CanvasEvent::Activate)
    }

    /// `tool.back` (Backspace): `Back` to the tool.
    pub(super) fn act_tool_back(&mut self) -> Outcome {
        self.send_tool(&CanvasEvent::Back)
    }

    /// `cp.next`, `cp.previous`, `cp.next_unconnected` (§7.7): the grid cursor goes to the next or previous
    /// connection point of the page in reading order after the cursor (from the view centre without one), wrapping;
    /// `cp.next_unconnected` only to a connection point no connection ends at.
    pub(super) fn act_cp_step(&mut self, p: &Map<String, Value>, step: CpStep) -> Outcome {
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        let last = self.tools.last_cp.clone();
        let Some(c) = self.canvas_mut(g) else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let page = Arc::clone(c.page());
        let candidates: Vec<usize> = (0..page.cps.len())
            .filter(|&i| step != CpStep::NextUnconnected || !page.cps[i].connected)
            .collect();
        let (Some(&first), Some(&end)) = (candidates.first(), candidates.last()) else {
            return Outcome::Refused {
                reason_key: if page.cps.is_empty() {
                    "reason-no-cp"
                } else {
                    "reason-no-unconnected-cp"
                },
            };
        };
        let cursor = c.cursor();
        let key = (page.session, page.rev, page.folio.clone());
        // Where the last step left the cursor, while it is still there; else the cursor's reading position.
        let from = last
            .filter(|(s, r, f, i)| {
                (*s, *r, f) == (key.0, key.1, &key.2)
                    && page.cps.get(*i).map(|cp| Some(cp.at_nm)) == Some(cursor)
            })
            .map(|(_, _, _, i)| i);
        let at = match (from, step) {
            (Some(i), CpStep::Previous) => candidates.iter().rev().find(|&&j| j < i).copied().unwrap_or(end),
            (Some(i), _) => candidates.iter().find(|&&j| j > i).copied().unwrap_or(first),
            (None, _) => {
                let here = reading_key(page.cells.as_ref(), cursor.unwrap_or(c.centre()));
                let k = |j: usize| reading_key(page.cells.as_ref(), page.cps[j].at_nm);
                if step == CpStep::Previous {
                    candidates
                        .iter()
                        .rev()
                        .find(|&&j| k(j) < here)
                        .copied()
                        .unwrap_or(end)
                } else {
                    candidates
                        .iter()
                        .find(|&&j| k(j) > here)
                        .copied()
                        .unwrap_or(first)
                }
            }
        };
        c.put_cursor(Some(page.cps[at].at_nm));
        self.tools.last_cp = Some((key.0, key.1, key.2, at));
        self.cursor_moved(g);
        Outcome::Done
    }
}

/// The toast of a refused command (FT-779): an invalid parameter names its field in words, never its JSON Pointer
/// (the command error itself stays machine-readable for the CLI); any other error as it prints. Every shell path that
/// shows a command error goes through it: the tool host, `command.run`, the creation dialogs, paste, undo and redo.
pub(super) fn command_failed(e: &CmdError, params: &Value) -> Outcome {
    match e {
        CmdError::InvalidParams {
            command,
            path,
            message,
        } => failed(
            "error-command-field",
            &[
                strings::command_title(command),
                &param_words(params, path),
                message,
            ],
        ),
        other => failed("error-command", &[&other.to_string()]),
    }
}

/// The words of the parameter at the JSON Pointer `path` of a command's `params`: for a command that sets a field
/// of a record kind (`kind` and `path` members, as `fields.set`), its `/value` and `/path` are that field's label
/// in the property grid's words; any other parameter is its last member humanised as field labels are, with a list
/// position kept (`/ips/2` is "Interruption points, item 3").
pub(crate) fn param_words(params: &Value, path: &str) -> String {
    if matches!(path, "/value" | "/path")
        && let (Some(kind), Some(field)) = (
            params.get("kind").and_then(Value::as_str),
            params.get("path").and_then(Value::as_str),
        )
    {
        return strings::field_label(kind, field);
    }
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let Some(at) = segments.iter().rposition(|s| s.parse::<usize>().is_err()) else {
        return strings::text("error-param-whole").to_owned();
    };
    let base = match segments[at] {
        "ips" => "Interruption points".to_owned(),
        "cps" => "Connection points".to_owned(),
        "reps" => "Symbols".to_owned(),
        "rep" => "Symbol".to_owned(),
        other => strings::field_label("", other),
    };
    match segments.get(at + 1).and_then(|s| s.parse::<usize>().ok()) {
        Some(i) => strings::format("error-param-item", &[&base, &(i + 1).to_string()]),
        None => base,
    }
}

#[cfg(test)]
mod ft779 {
    use super::*;

    fn message(o: Outcome) -> String {
        match o {
            Outcome::Failed { message } => message,
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    /// FT-779: the toast of an invalid parameter names the field in words: a field set by `fields.set` by its label,
    /// any other parameter by its member humanised with its list position; never a JSON Pointer.
    #[test]
    fn an_invalid_parameter_toast_names_the_field_in_words() {
        let set =
            serde_json::json!({"kind": "function", "targets": [], "path": "kind.ip.sort", "value": "x"});
        let e = CmdError::invalid_params("fields.set", "/value", "a whole number is needed");
        assert_eq!(
            message(command_failed(&e, &set)),
            "Set property: Chain order: a whole number is needed"
        );
        let e = CmdError::invalid_params("ip.arrange", "/ips/2", "the interruption point is listed twice");
        assert_eq!(
            message(command_failed(&e, &Value::Null)),
            "Arrange interruption points: Interruption points, item 3: the interruption point is listed twice"
        );
        let e = CmdError::invalid_params(
            "connection.delete",
            "/connections",
            "list at least one connection",
        );
        assert_eq!(
            message(command_failed(&e, &Value::Null)),
            "Delete wire: Connections: list at least one connection"
        );
    }
}
