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
use ew_api::{CmdError, DynCommand, Origin, Submit};
use serde_json::{Map, Value};

use super::canvas_actions::point_mm;
use super::navigation::bump;
use super::{Focus, Mode, Outcome, RecordedCommand, ShellModel, failed, flag};
use crate::canvas::{CanvasModel, CanvasPage, PageId, reading_key};
use crate::strings;
use crate::tools::{
    Button, CanvasEvent, LINK_PREVIEW_DWELL_MS, LINK_PREVIEW_PANE, LinkPreview, MarkerKind, Mods, Overlay,
    PreviewState, PreviewStatus, Selection, Tool, ToolCx, ToolOutput, tool_label, tool_named,
};

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

    /// The active tool's status hint.
    pub fn tool_hint(&self) -> &'static str {
        strings::text(self.tools.tool.status_hint())
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
        if let Some(out) = self.run_tool(&CanvasEvent::Escape) {
            let _ = self.apply_tool_output(out);
        }
        self.tools.overlay = Overlay::default();
        self.tools.preview = None;
        self.tools.link_preview = None;
        self.tools.announcement = None;
        self.tools.last_cp = None;
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
        Some(out)
    }

    /// Announces the active tool and its hint through the live region.
    fn announce_tool(&mut self) {
        let label = tool_label(self.tools.tool.name());
        self.tools.announcement = Some(strings::format(
            "announce-tool",
            &[&label, strings::text(self.tools.tool.status_hint())],
        ));
    }

    /// Hands `ev` to the active tool and acts on its output (§7.2).
    pub(super) fn send_tool(&mut self, ev: &CanvasEvent) -> Outcome {
        match self.run_tool(ev) {
            Some(out) => self.apply_tool_output(out),
            None => Outcome::Refused {
                reason_key: "reason-no-page",
            },
        }
    }

    /// The host's action on a tool's output (§7.2).
    fn apply_tool_output(&mut self, out: ToolOutput) -> Outcome {
        match out {
            ToolOutput::None => Outcome::Done,
            ToolOutput::Transient(o) => {
                self.tools.overlay = o;
                Outcome::Done
            }
            ToolOutput::Preview(cmd) => {
                self.run_preview(cmd.as_ref());
                Outcome::Done
            }
            ToolOutput::Commit(cmd) => {
                let out = self.run_commit(cmd.as_ref());
                self.tools.overlay = Overlay::default();
                self.tools.preview = None;
                out
            }
            ToolOutput::Cancel => {
                self.tools.overlay = Overlay::default();
                self.tools.preview = None;
                Outcome::Done
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

    /// Announces the selection's size through the live region.
    fn announce_selection(&mut self) {
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
    fn tool_submit(&self) -> Submit {
        match self.mode {
            Mode::Gui => Submit::from(Origin::Gui),
            Mode::Headless => Submit::from(Origin::CmdLog {
                file: self.origin_file.clone(),
            }),
        }
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
        self.tools.preview = Some(PreviewState {
            base_rev: rev,
            command: json,
            name: cmd.name().to_owned(),
            result: result.map_err(|e| e.to_string()),
            derived,
        });
    }

    /// `Commit(cmd)` (§7.3): in the window `Session::commit` of a fresh, equal preview, else `Session::execute_with`
    /// (a preview and its commit for a bulk command); headless a one-line command log through `Session::run_script`,
    /// exactly `command.run`'s path, so the exposure rules and `--record` apply.
    fn run_commit(&mut self, cmd: &dyn DynCommand) -> Outcome {
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
            Err(e) => return failed("error-command", &[&e.to_string()]),
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
                match project.session.execute_with(cmd, Submit::from(Origin::Gui)) {
                    Err(CmdError::NeedsPreview { .. }) => {
                        bump(calls, |c| {
                            c.preview_with += 1;
                            c.commit += 1;
                        });
                        project
                            .session
                            .preview_with(cmd, Submit::from(Origin::Gui))
                            .and_then(|p| project.session.commit(p))
                    }
                    other => other,
                }
            }
        };
        let outcome = match result {
            Ok(_) => {
                self.record.undone.clear();
                self.record.steps.push(RecordedCommand {
                    cmd: name,
                    params: json,
                });
                Outcome::Done
            }
            Err(e) => failed("error-command", &[&e.to_string()]),
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
        self.tools.tool = tool;
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
    /// no tool (it pans).
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

    /// `canvas.activate` (Enter): `Activate` to the tool, at the grid cursor.
    pub(super) fn act_activate(&mut self) -> Outcome {
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
