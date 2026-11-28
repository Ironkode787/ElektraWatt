// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host in the shell: one active tool per window acting on the
//! focused group's canvas, its overlay, the preview protocol over `Session::preview_with`, the Escape chain, the
//! keyboard tool input, the link hover preview and the selection.
//!
//! **Outputs**. `Transient` replaces the overlay; `Preview` runs `Session::preview_with` once (a repeat of
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
//! **Time.** Nothing here is timed: pointer rest is the action `canvas.dwell` with its duration.

use std::sync::Arc;

use ew_api::{AnyCommand, CmdError, DynCommand, Submit};
use ew_dl::Destination;
use ew_units::Rect;
use serde_json::{Map, Value};

use super::canvas_actions::point_mm;
use super::navigation::bump;
use super::{Focus, Mode, Outcome, RecordedCommand, ShellModel, failed, flag};
use crate::canvas::{CanvasModel, CanvasPage, PageId, reading_key};
use crate::key;
use crate::notifications::{Note, NoteTone};
use crate::properties::SheetSection;
use crate::strings;
use crate::strings::Key;
use crate::tools::{
    Button, CanvasEvent, Follows, LINK_PREVIEW_DWELL_MS, LINK_PREVIEW_PANE, LinkPreview, MarkerKind, Mods,
    Overlay, PreviewInfo, PreviewState, PreviewStatus, ResultNote, Selection, SelectionChange, Tool, ToolCx,
    ToolOutput, tool_label, tool_named,
};
use ew_dl::EntityRef;

/// The toast key of why a command did nothing ([`ShellModel::explain_disabled`]): a second one replaces the first.
const DISABLED: &str = "disabled";

/// A connection-point step of the keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CpStep {
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

    /// Whether the window sends a double click's second press as `tool.finish` (`Tool::finishes_on_double_click`).
    /// The Select tool finishes only with a free text selected, which the double click opens: on a symbol the second
    /// press stays a press, so a drag started right after the click that selected it moves it (a drag from
    /// the lamp's centre just after selecting it there did nothing).
    pub fn finishes_on_double_click(&self) -> bool {
        self.tools.tool.finishes_on_double_click()
            && (self.tools.tool.name() != "select"
                || self.text_edit_target().is_ok()
                || self.pointer_on_frame())
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

    /// The active tool's status hint; while the picker's strip chooser is open, what it waits for instead.
    /// The keys it names are the keymap's.
    pub fn tool_hint(&self) -> String {
        if self.strip_chooser_open() {
            return strings::text(key!("hint-choose-strip")).to_owned();
        }
        let hint = self.tools.tool.status_hint();
        // Over the free part of a panel page where its legend would go, Select at rest says how to add it (F21).
        if hint == crate::tools::Hint::plain(key!("hint-select"))
            && let ew_api::output::LegendOffer::At(r) = self.legend_offer()
            && let Some((x, y)) = self
                .canvas()
                .and_then(|c| Some(c.page().snapper.sheet_to_page(c.pointer()?)))
            && (r.min().x.0..=r.max().x.0).contains(&x)
            && (r.min().y.0..=r.max().y.0).contains(&y)
        {
            return strings::text(key!("hint-select-legend")).to_owned();
        }
        // With one wire selected, Select says how to reach its handles; with one in the keyboard's focus, how to move
        // it.
        if hint == crate::tools::Hint::plain(key!("hint-select"))
            && let Some((wire, line)) = self.wire_handles()
        {
            // The handle with the focus by its place along the wire as Tab goes (so a person can tell which
            // dot it is) and the arrows that move it.
            if let Some((_, k)) = self.tools.handle.filter(|(w, _)| *w == wire) {
                use crate::tools::route_edit::{GripMove, grip_move, handle_parts, reads_backwards};
                let parts = handle_parts(&line);
                let place = if reads_backwards(&line) {
                    parts.len().saturating_sub(k)
                } else {
                    k + 1
                };
                let text = match parts.get(k).map(|p| grip_move(&line, *p)) {
                    Some(GripMove::UpDown) => key!("hint-select-handle-up-down"),
                    Some(GripMove::Sideways) => key!("hint-select-handle-sideways"),
                    _ => key!("hint-select-handle-bend"),
                };
                let (tab, esc) = (
                    crate::command_bar::key_name(self, "tool.next_proposal"),
                    crate::command_bar::key_name(self, crate::tools::ESCAPE),
                );
                return strings::format(text, &[&place.to_string(), &parts.len().to_string(), &tab, &esc]);
            }
            return self.hint_text(crate::tools::Hint::new(
                key!("hint-select-wire"),
                &["tool.next_proposal"],
            ));
        }
        self.hint_text(hint)
    }

    /// The text of hint `hint` with the keys of its actions from the keymap, in their keycap form ("Enter", "Esc";
    /// "(no key)" for an action the keymap binds to none).
    pub(crate) fn hint_text(&self, hint: crate::tools::Hint) -> String {
        let keys: Vec<String> = hint
            .keys
            .iter()
            .map(|id| crate::command_bar::key_name(self, id))
            .collect();
        let args: Vec<&str> = keys.iter().map(String::as_str).collect();
        strings::format(hint.key, &args)
    }

    /// Whether the picker is open as the strip chooser of a terminal placement.
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

    /// The drawn polylines (sheet nm) of the selected connections on group `group`'s page: a wire or a fluid line is
    /// selected as a band along its path ([`Self::selected_rects`] leaves it out). The wires the last Route changed
    /// are drawn so too while its toast is up.
    pub fn selected_paths(&self, group: usize) -> Vec<Vec<(i64, i64)>> {
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        let page = c.page();
        let last_routed = self.last_routed_wires();
        page.tables
            .connections
            .iter()
            .filter(|t| self.tools.selection.contains(EntityRef(t.id)) || last_routed.contains(&t.id))
            .map(|t| {
                t.polyline
                    .iter()
                    .map(|p| page.snapper.page_to_sheet(*p))
                    .collect()
            })
            .collect()
    }

    /// The rectangles of the selected entities drawn on group `group`'s page (the `selected` outline), the
    /// connections left out ([`Self::selected_paths`]). A device or a
    /// function selected by its tag (the command search, `select.set tag:`) is outlined where the page draws it, as
    /// Delete takes it (the status bar counted it and the page drew nothing).
    pub fn selected_rects(&self, group: usize) -> Vec<Rect> {
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        let selection = &self.tools.selection;
        if selection.is_empty() {
            return Vec::new();
        }
        let page = c.page();
        let by_tag: Vec<EntityRef> = page
            .tables
            .reps
            .iter()
            .filter(|r| {
                r.device.is_some_and(|d| selection.contains(EntityRef(d)))
                    || r.functions.iter().any(|f| selection.contains(EntityRef(*f)))
            })
            .map(|r| EntityRef(r.id))
            .collect();
        let line = |e: &EntityRef| page.tables.connections.iter().any(|t| t.id == e.0);
        page.entities
            .iter()
            .filter(|(e, _)| (selection.contains(*e) || by_tag.contains(e)) && !line(e))
            .map(|(_, r)| *r)
            .collect()
    }

    /// How many selected items the canvas's status field and HUD count: the selection, or none while
    /// nothing of it is drawn on the focused group's page (a page chosen in the page list selects its page record,
    /// which draws nothing there). A device or a function selected by its tag counts where the page draws it, as
    /// Delete takes it. The live region and the page list's Delete keep the whole selection.
    pub fn canvas_selected(&self) -> usize {
        let n = self.tools.selection.items.len();
        let group = self.area.as_ref().map_or(0, |a| a.focused);
        if n > 0 && self.selected_rects(group).is_empty() && !self.selection_drawn_here() {
            0
        } else {
            n
        }
    }

    /// Whether the page shown draws something of the selection as Delete and Cut take it
    /// ([`Self::copy_targets`]: a device or a function selected by its tag as its representations on the page).
    pub(crate) fn selection_drawn_here(&self) -> bool {
        let Some(page) = self.canvas().map(|c| Arc::clone(c.page())) else {
            return false;
        };
        self.copy_targets()
            .into_iter()
            .any(|e| crate::tools::entity_kind(&page.tables, e).is_some())
    }

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

    /// Makes `tool` the active tool, as a tool of the registry would be (a debug build only: the probe tool of
    /// the tests exercises the commit path, which no shipped tool takes).
    #[cfg(debug_assertions)]
    pub fn set_tool_for_tests(&mut self, tool: Box<dyn Tool>) {
        self.activate_tool(tool);
        self.sync_views();
        self.sync_tool_context();
    }

    // -----------------------------------------------------------------------------------------------------------
    // The host

    /// A click on the disabled command `action` (doc 16a §5.2): why it is greyed (the sentence its tooltip ends with),
    /// as an information toast of key `disabled`, so a second click replaces it rather than stacking. A call that names
    /// its target in `params` (a tab) says why that call is refused, a bar button without them why the button is
    /// greyed (a menu button whose entries are all greyed, its first entry's reason); nothing when it is not.
    pub fn explain_disabled(&mut self, action: &str, params: &Value) {
        let reason = match params.as_object().filter(|p| !p.is_empty()) {
            Some(p) => crate::actions::find(action)
                .and_then(|m| crate::actions::enabled_with(m, self, p).err())
                .map(|r| strings::render(&r)),
            None => crate::command_bar::action_button(self, action)
                .filter(|b| !b.enabled)
                .and_then(|b| b.reason),
        };
        if let Some(text) = reason {
            self.disabled_toast(text);
        }
    }

    /// `text`, why something did nothing, as the information toast of key `disabled`.
    pub(super) fn disabled_toast(&mut self, text: String) {
        self.notes.push(
            crate::notifications::Note::new(DISABLED, text).toned(crate::notifications::NoteTone::Info),
        );
    }

    /// The reason [`ShellModel::explain_disabled`] shows while its toast is on screen: the status line says it too.
    pub fn refusal_shown(&self) -> Option<&str> {
        self.notes
            .visible()
            .into_iter()
            .rev()
            .find(|n| n.key == DISABLED)
            .map(|n| n.text.as_str())
    }

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
        // A refusal clears when the state it refused changes: a committed command, a Properties
        // edit, undo or redo (each moves the revision) or a selection change (its generation). A refusal set by this
        // dispatch, which changed neither, stands.
        let basis = (self.revision(), self.tools.selection.generation);
        if self.tools.notice_basis.is_some_and(|b| b != basis) {
            self.tools.notice = None;
            self.tools.notice_sticky = false;
            self.tools.notice_at = None;
        }
        self.tools.notice_basis = Some(basis);
        let now = self.tool_context();
        if now == self.tools.context {
            return;
        }
        let first = self.tools.context.is_none();
        self.tools.context = now;
        if first {
            return;
        }
        // A press held across the change: the tool is escaped below and the release that follows is dropped.
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
        // The supply marks' Potential panel stands at marks of the page it was opened on.
        self.supply_panel = None;
        self.end_editing_tool();
    }

    /// Whether something is in flight on the pointer: a gesture begun (a wire at its first click, a box at its
    /// first corner, a text being typed), or what the tool was armed with hanging there before it put anything down
    /// (a floating symbol, pasted block or interruption point, a box). A tool merely armed (the Wire tool after a
    /// finished wire, the Place tool between two placements) has nothing in flight.
    pub fn tool_in_flight(&self) -> bool {
        let tool = &self.tools.tool;
        let state = tool.state();
        let begun = !matches!(state, "armed" | "idle" | "floating" | "picking");
        let carries = state == "floating" || matches!(tool.ghost(), Some(crate::tools::Ghost::Box { .. }));
        tool.name() != "select" && (begun || (carries && !self.tools.put_down))
    }

    /// Undo or Redo with something in flight on the pointer ([`ShellModel::tool_in_flight`]): it is put away as Esc
    /// puts it away, the tool ends where it holds nothing more, nothing is undone, and the status line says so.
    pub(super) fn put_away_in_flight(&mut self, redo: bool) {
        if let Some(out) = self.run_tool(&CanvasEvent::Escape) {
            let _ = self.apply_tool_output(out);
        }
        if self.tool_in_flight()
            && let Some(select) = tool_named("select")
        {
            self.activate_tool(select);
        }
        let words = if redo {
            key!("status-put-away-redo")
        } else {
            key!("status-put-away-undo")
        };
        // As a refused Enter's notice: it stands until the pointer or the grid cursor moves.
        self.tools.notice = Some(strings::text(words).to_owned());
        self.tools.notice_sticky = true;
        self.tools.notice_at = Some((i64::MIN, i64::MIN));
    }

    /// After an undo, a redo or any dispatch that commits (a Delete in a navigator): a black box armed before it whose records ([`crate::tools::black_box::BlackBoxTool::records`]) the step took
    /// out of the project is escaped and ended, so that it is armed again from the project as it is (put
    /// down, it named a symbol copy the Undo had taken). A tool whose records are all there stays armed.
    pub(super) fn end_tool_without_records(&mut self) {
        let records: Vec<String> = self
            .tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<crate::tools::black_box::BlackBoxTool>())
            .map(|t| t.records().into_iter().map(str::to_owned).collect())
            .unwrap_or_default();
        let gone = self.project_session().is_some_and(|s| {
            records
                .iter()
                .filter_map(|r| r.parse::<ew_api::output::Uuid>().ok())
                .any(|id| !s.record_exists(id))
        });
        if !gone {
            return;
        }
        if let Some(out) = self.run_tool(&CanvasEvent::Escape) {
            let _ = self.apply_tool_output(out);
        }
        self.end_editing_tool();
    }

    /// Ends an editing tool (Wire, Place, Change symbol, and the part-4b Text, Text edit, Graphic, Wire marker,
    /// Structure box and part-5 Image tools): Select takes over, the selection survives. Such a tool holds what it
    /// was armed with (a function, a symbol copy, a page, the copies it embeds), which a page or tab change leaves
    /// behind and an undo or a redo may take out of the project (a box armed by the station view's To do,
    /// put down after an Undo, named a symbol copy that was gone).
    pub(super) fn end_editing_tool(&mut self) {
        if matches!(
            self.tools.tool.name(),
            "wire"
                | "place"
                | "exchange"
                | "paste"
                | "ip"
                | "black_box"
                | "text"
                | "text_edit"
                | "box_pin"
                | "graphic"
                | "marker"
                | "structure_box"
                | "image"
                | "layout_place"
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
        // The IP tool's proposal reads the project's source points of this revision.
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
                layout: host.layout_options,
                connect_on_move: host.connect_on_move,
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
            key!("announce-tool"),
            &[&label, &self.hint_text(self.tools.tool.status_hint())],
        ));
    }

    /// Hands `ev` to the active tool and acts on its output.
    pub(super) fn send_tool(&mut self, ev: &CanvasEvent) -> Outcome {
        // A refused key's notice stands in the status bar until the next canvas input. A tool
        // refusal is sticky for the rest of its gesture: its release keeps it and the post-release hover keeps it, so
        // the user reads it after the gesture; the next primary press, key press, page change or tool change clears it
        // (window re-check W1). While the tool is still busy a move or a dwell clears it first: a valid move ends the
        // refusal and brings the ghost back, and a refused one sets the notice again at once.
        let idle = self.tools.tool.state() == "idle";
        let keeps = self.tools.notice_sticky
            && match ev {
                CanvasEvent::Release { .. } => true,
                // A move to the refused press's own point is no move (the window reports one after each
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
        // A place tool that asks for the next designation refuses a click or Enter with the reason.
        if matches!(ev, CanvasEvent::Press { .. } | CanvasEvent::Activate) && self.place_needs_designation() {
            return Outcome::refused(key!("reason-designation-needed"));
        }
        // The IP tool's click on another interruption point, or an external point's on a pin, is refused in
        // words before it reaches the tool, and so is a Supply tool's mark there; the ghost stays.
        if let Some(out) = self.ip_drop_refused(ev) {
            return out;
        }
        match self.run_tool(ev) {
            Some(out) => {
                let refused = matches!(out, ToolOutput::Refused(_));
                let committed = matches!(out, ToolOutput::Commit(_));
                let out = self.apply_tool_output(out);
                // The refused press's point; a refused Enter (no pointer point) is kept until the pointer or the
                // grid cursor moves. A refused commit (an interruption point a prevent rule refuses) too.
                if refused || (committed && matches!(out, Outcome::Refused { .. })) {
                    self.tools.notice_at = match ev {
                        CanvasEvent::Press { at, .. } => Some(*at),
                        // A refused key, Enter's finish of the Duct tool among them, has no pointer point either.
                        CanvasEvent::Activate | CanvasEvent::Key(_) => Some((i64::MIN, i64::MIN)),
                        _ => None,
                    };
                }
                // The place tool says what it floats after a key or a cursor move.
                if self.tools.tool.name() == "place"
                    && matches!(ev, CanvasEvent::Key(_) | CanvasEvent::CursorMove { .. })
                {
                    self.announce_place();
                }
                // The Wire tool says where it routes from once it starts.
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
            None => Outcome::refused(key!("reason-no-page")),
        }
    }

    /// The words of a tool's refusal: the reason's text, except where it names keys, which come from the keymap
    /// (a refused turn during a layout drag reads as `layout.rotate`'s and `layout.flip`'s refusal).
    pub(super) fn refusal_text(&self, reason_key: Key<'static>) -> String {
        if reason_key == key!("reason-layout-turn") {
            self.layout_turn_refusal()
        } else {
            strings::text(reason_key).to_owned()
        }
    }

    /// The host's action on a tool's output; after a host step the tool gives way to Select where it says so
    /// ([`crate::tools::Tool::gives_way_after_step`]).
    pub(super) fn apply_tool_output(&mut self, out: ToolOutput) -> Outcome {
        let gives_way = matches!(out, ToolOutput::Host(_)) && self.tools.tool.gives_way_after_step();
        let done = self.apply_tool_step(out);
        if gives_way {
            let _ = self.act_tool("select");
        }
        done
    }

    /// What the host does with a tool's output ([`ShellModel::apply_tool_output`]).
    fn apply_tool_step(&mut self, out: ToolOutput) -> Outcome {
        match out {
            ToolOutput::None => Outcome::Done,
            ToolOutput::Transient(o) => {
                self.tools.overlay = o;
                Outcome::Done
            }
            ToolOutput::Preview(cmd) => {
                let cmd = self.with_terminal(self.without_held_copies(cmd));
                self.run_preview(cmd.as_ref());
                Outcome::Done
            }
            // The macro tool's click is a host step, no command.
            ToolOutput::Host(crate::tools::HostStep::MacroPlace { key, at }) => {
                self.tools.put_down = true;
                self.macro_place_from_tool(&key, at)
            }
            // The Supply tool's click places its marks in one undo step.
            ToolOutput::Host(crate::tools::HostStep::SupplyPlace { at }) => self.supply_place_from_tool(at),
            // The glands' drop places them as `layout.glands_place` does at a drop point.
            ToolOutput::Host(crate::tools::HostStep::GlandsPlace { cables, at }) => {
                self.glands_place(Some(&cables), Some(at), (None, None))
            }
            ToolOutput::Host(crate::tools::HostStep::PinForWire { rep, pin }) => self.pin_for_wire(rep, pin),
            // The Edit frame tool's click, released drag and double click.
            ToolOutput::Host(crate::tools::HostStep::FrameChoose { choice }) => self.frame_choose(choice),
            ToolOutput::Host(crate::tools::HostStep::FrameLine { line, to }) => {
                self.frame_line_moved(line, to)
            }
            ToolOutput::Host(crate::tools::HostStep::FrameType) => self.frame_type(),
            ToolOutput::Host(crate::tools::HostStep::FrameText { text }) => self.frame_text(&text),
            ToolOutput::Commit(cmd) => {
                let cmd = self.with_terminal(self.without_held_copies(cmd));
                let follows = self.tools.tool.follows(cmd.as_ref());
                self.commit_tool(cmd.as_ref(), follows)
            }
            ToolOutput::Cancel => {
                self.tools.overlay = Overlay::default();
                self.tools.preview = None;
                Outcome::Done
            }
            ToolOutput::RefusedWith(reason_key, text) => {
                self.tools.overlay = Overlay::default();
                self.tools.preview = None;
                self.tools.notice = Some(text.clone());
                self.tools.notice_sticky = true;
                // A part in a cut-out dropped on the plate or a rail: the toast offers the door's page (doc 12 §3.3).
                if reason_key == key!("reason-layout-cutout") {
                    let door = self.session_offer(
                        "open_door",
                        key!("layout-open-door"),
                        vec![("layout.surface_page", serde_json::json!({ "surface": "door" }))],
                    );
                    self.notify_offer(text.clone(), door);
                }
                self.tools.announcement = Some(text);
                Outcome::Refused {
                    reason: reason_key.into(),
                }
            }
            ToolOutput::Refused(reason_key) => {
                // The same path as a refused canvas key: the status bar shows the reason until the next canvas input.
                // A refusal ends the gesture it answers: its ghost and its kept preview go, as a refused release's
                // `Select` and the old `Transient(Overlay::default())` did (review fix, F1). A refused turn (R on a
                // rail, X on the plate) is no end: the layout ghost and its gesture stay with the reason.
                if reason_key != key!("reason-layout-turn") {
                    self.tools.overlay = Overlay::default();
                    self.tools.preview = None;
                }
                self.tools.notice = Some(self.refusal_text(reason_key));
                self.tools.notice_sticky = true;
                Outcome::Refused {
                    reason: reason_key.into(),
                }
            }
            ToolOutput::Select(change) => {
                self.tools.overlay = Overlay::default();
                if self.tools.selection.apply(&change) {
                    self.announce_selection();
                } else {
                    // A click on what is selected already, or on the empty page with nothing selected, brings Properties
                    // back to it from the project's sheet (Properties must not stay on the project until something else
                    // is selected first).
                    self.props.project_mode = None;
                }
                // A click on an empty cell of a valve terminal drawn as its positions fills it.
                if let SelectionChange::Replace(one) = &change
                    && let [e] = one.as_slice()
                    && let Some(params) = self.empty_cell_under_pointer(e.0)
                {
                    return self.dispatch("fluid.cell_fill", &params);
                }
                Outcome::Done
            }
            ToolOutput::Follow(dest) => {
                self.tools.overlay = Overlay::default();
                self.follow_dest(dest)
            }
        }
    }

    /// The host's commit of a tool's command `cmd` with what the tool said follows it ([`Tool::follows`], architecture
    /// §9.2): what the toast and the selection need of the page before it changes is read, the command is committed,
    /// and each part of `follows` is applied, for whichever tool set it.
    fn commit_tool(&mut self, cmd: &dyn DynCommand, follows: Follows) -> Outcome {
        let json = cmd.to_json().ok();
        // A drop into a valve terminal's box says where the device sits now.
        let into = json.as_ref().is_some_and(|j| j.get("into").is_some());
        self.tools.before_commit = self.canvas().map(|c| Arc::clone(&c.page().tables));
        // The wires a placement draws by its autoconnect hints, said in the toast, and the far hits it offers.
        let (connects, far) = match &follows.placed {
            Some(p) if p.autoconnect => (self.autoconnect_words(cmd), self.far_offers(cmd)),
            _ => (None, Vec::new()),
        };
        self.tools.put_down = true;
        self.tools.drew_existing = follows.drew_existing;
        let (retag, moved_back) = self.tags_before_commit(cmd, json.as_ref(), &follows);
        // The online report is memoised per revision.
        let errors_before = if follows.wire_errors {
            self.open_errors()
        } else {
            Vec::new()
        };
        // An earlier toast stays (its Undo expires with this commit); a new toast of key `edit` replaces it.
        let out = self.run_commit(cmd);
        let done = out == Outcome::Done;
        // A wire without a route is said in a toast; an error it brings, said next, takes the toast's place.
        if done && follows.unrouted {
            let text = strings::format_count(crate::count_key!("toast-wires-unrouted"), 1);
            self.notify(text, true);
        }
        if done && follows.wire_errors {
            self.say_wire_errors(&errors_before);
        }
        if done {
            self.say_retagged(cmd, json.as_ref(), &follows, &retag, &moved_back);
        }
        // A refused interruption point keeps its ghost where it was and says why.
        let (out, keep_ghost) = match follows.ip {
            Some(_) => self.ip_commit_refused(out),
            None => (out, false),
        };
        let done = out == Outcome::Done;
        if done
            && follows.line_tip
            && let Some(json) = &json
            && super::tips::line_ends_on_cp(json, self.tools.before_commit.as_deref())
        {
            self.offer_tip(super::Tip::LineOnCp, Vec::new());
        }
        if done && let (Some(placed), Some(json)) = (follows.placed, &json) {
            self.note_placement(json, placed, connects, into, far);
        }
        if let Some(ip) = follows.ip.clone() {
            self.note_ip(done, ip, json.as_ref());
        }
        self.tools.tool.on_commit(done);
        if !keep_ghost {
            self.tools.overlay = Overlay::default();
        }
        self.tools.preview = None;
        // A committed paste returns to Select with the pasted representations selected.
        self.tools.pasted = done && follows.paste;
        self.tools.paste_fit = self
            .tools
            .pasted
            .then_some(json.as_ref())
            .flatten()
            .and_then(|json| self.paste_fit_of(json));
        // What a placement made is the selection, replacing it, at the end of the dispatch.
        self.tools.placed = done && follows.select_placed;
        self.layout_committed(done, &follows.layout);
        // The IP tool's memory and its next name.
        if done && follows.ip.is_some() {
            self.after_ip_commit();
        }
        if done
            && follows.gives_way
            && let Some(select) = tool_named("select")
        {
            self.tools.tool = select;
            self.tool_epoch = self.tool_epoch.wrapping_add(1);
            self.announce_tool();
        }
        if done && let Some(moved) = follows.moved {
            self.after_reps_moved(&moved, into);
        }
        out
    }

    /// The tags of the devices a commit draws again, before it ([`ShellModel::redrawn_tags`]); a paste that brings
    /// back what a cut left unplaced compares with the tags before the cut, once, and names the devices it brings back.
    fn tags_before_commit(
        &mut self,
        cmd: &dyn DynCommand,
        json: Option<&Value>,
        follows: &Follows,
    ) -> (Vec<(ew_api::output::Uuid, String)>, Vec<ew_api::output::Uuid>) {
        let mut retag = match json {
            Some(j) if follows.retag => self.redrawn_tags(cmd.name(), j),
            _ => Vec::new(),
        };
        let mut moved_back = Vec::new();
        if follows.paste {
            let cut = std::mem::take(&mut self.tools.cut_tags);
            moved_back = cut.iter().map(|(d, _)| *d).collect();
            for (d, tag) in &mut retag {
                if let Some((_, was)) = cut.iter().find(|(c, _)| c == d) {
                    tag.clone_from(was);
                }
            }
        }
        (retag, moved_back)
    }

    /// A drawn wire that brought an error finding says so at once, in the finding's words, with Undo, in the error's
    /// tone: the findings that were not open before it (`before`). What is only still to be given (an open item: a bus
    /// address TBD while the devices are wired first) is no error of the wire: it is said as a notice, in the
    /// information tone, without Undo; the check keeps the finding.
    fn say_wire_errors(&mut self, before: &[(String, String, bool)]) {
        let (open, new): (Vec<_>, Vec<_>) = self
            .open_errors()
            .into_iter()
            .filter(|(f, _, _)| !before.iter().any(|(b, _, _)| b == f))
            .partition(|(_, _, open)| *open);
        let words = |list: Vec<(String, String, bool)>| -> Vec<String> {
            list.into_iter()
                .map(|(_, message, _)| message.trim_end_matches('.').to_owned())
                .collect()
        };
        let (open, new) = (words(open), words(new));
        if !new.is_empty() {
            let text = strings::format(key!("toast-wire-error"), &[&new.join("; ")]);
            let undo = self.undo_offer();
            self.notes
                .push(Note::new("edit", text).offering([undo]).toned(NoteTone::Error));
        } else if !open.is_empty() {
            let text = strings::format(key!("toast-wire-to-give"), &[&open.join("; ")]);
            self.notes.push(Note::new("to-give", text).toned(NoteTone::Info));
        }
    }

    /// A commit that changed tags says so: a paste first what it moved back from a cut, then the tags it changed, then
    /// what it did besides (the wires it removed with the devices it brought back), and offers to keep the tags it
    /// changed as they read, beside Undo; another commit the tags its boxes changed.
    fn say_retagged(
        &mut self,
        cmd: &dyn DynCommand,
        json: Option<&Value>,
        follows: &Follows,
        retag: &[(ew_api::output::Uuid, String)],
        moved_back: &[ew_api::output::Uuid],
    ) {
        let notices = (follows.paste && !self.tools.commit_notices.is_empty()).then(|| {
            strings::format(
                key!("toast-paste-notices"),
                &[&self.tools.commit_notices.join("; ")],
            )
        });
        let moved = json
            .filter(|_| !moved_back.is_empty())
            .and_then(|j| self.moved_words(moved_back, j));
        let parts: Vec<String> = moved
            .into_iter()
            .chain(self.retagged_words(cmd.name(), retag))
            .chain(notices)
            .collect();
        let kept: Vec<(ew_api::output::Uuid, String)> = if follows.paste {
            self.retagged(retag)
                .into_iter()
                .map(|(d, was, _)| (d, was))
                .collect()
        } else {
            Vec::new()
        };
        if !kept.is_empty() {
            self.notify_keep_tags(parts.join(" "), &kept);
        } else if !parts.is_empty() {
            self.notify(parts.join(" "), true);
        }
    }

    /// Every placement answers with its result line (the tag and the cell, and the wires it drew), said at the end of
    /// the dispatch once the page tables show it. A Terminal… row places its terminals in one dispatch and names the
    /// last: the wires of the terminals before it are said too.
    fn note_placement(
        &mut self,
        json: &Value,
        placed: crate::tools::Placement,
        connects: Option<(bool, Vec<(String, String)>)>,
        into: bool,
        far: Vec<((i64, i64), String, ew_api::output::Uuid, u16)>,
    ) {
        let (Some(x), Some(y)) = (json["pos"][0].as_i64(), json["pos"][1].as_i64()) else {
            return;
        };
        let connects = match (self.tools.result_note.take(), connects) {
            (
                Some(ResultNote::Placed {
                    wires: Some((t, mut earlier)),
                    ..
                }),
                now,
            ) => {
                let (tubes, list) = now.unwrap_or((t, Vec::new()));
                earlier.extend(list);
                Some((tubes && t, earlier))
            }
            (_, now) => now,
        };
        self.tools.result_note = Some(ResultNote::Placed {
            pos: (x, y),
            wires: connects,
            moved: placed.moved,
            into,
            far,
        });
    }

    /// The IP tool's point after its commit (`done`): one dropped on a pin says where it stands and what it connects
    /// to at the end of the dispatch; one placed free says what it became and where its partner is, after the reason
    /// of a renamed point, so a click made without reading the pointer's label still tells.
    fn note_ip(&mut self, done: bool, ip: crate::tools::IpCommit, json: Option<&Value>) {
        if done
            && let Some(pin) = ip.pin
            && let Some(json) = json
        {
            self.tools.result_note = Some(ResultNote::IpConnected {
                name: json["ip"]["name"].as_str().unwrap_or_default().to_owned(),
                pin,
                external: !json["ip"]["external"].is_null(),
                taken: ip.taken,
                // What it pairs with, as a point placed free says it (a point dropped on a pin said nothing of its
                // partner).
                placed: self.ip_placed_text(&json["ip"], false),
            });
        } else if done && (ip.taken.is_some() || ip.free) {
            let placed = json
                .filter(|_| ip.free)
                .and_then(|json| self.ip_placed_text(json, true));
            let text: Vec<String> = ip.taken.into_iter().chain(placed).chain(ip.beside).collect();
            if !text.is_empty() {
                self.notify(text.join(" "), true);
            }
        }
    }

    /// After a committed command of the layout (`done`): the device it placed, the quiet pick stack under the drop
    /// point, the layout's announcement, and a moved placement selected.
    fn layout_committed(&mut self, done: bool, layout: &crate::tools::LayoutCommit) {
        if done && layout.place {
            self.tools.placed_device = layout.device;
        }
        if done && layout.quiet_stack {
            self.tools.quiet_stack_at = self.canvas().and_then(CanvasModel::pointer);
        }
        self.tools.layout_announce |= done && layout.announce;
        if done
            && let Some(id) = layout.select
            && self
                .tools
                .selection
                .apply(&SelectionChange::Replace(vec![EntityRef(id)]))
        {
            self.announce_selection();
        }
    }

    /// After Select moved or turned representations: a move dropped into a valve terminal's box (`into`) says where its
    /// device sits now; a turn's wires, which the command led out of the turned pins and round the body, are routed
    /// again as Route routes a selection's, where that is better, in the turn's undo step (the move's re-solve,
    /// docs/design/06a-texts-window.md §7.4); the moved representations are the selection (one selection change, no
    /// command).
    fn after_reps_moved(&mut self, moved: &crate::tools::MovedReps, into: bool) {
        if moved.turn {
            self.route_turned(&moved.reps);
        }
        if into
            && moved.seat
            && let Some(device) = moved.reps.first().and_then(|r| {
                self.canvas()?
                    .page()
                    .tables
                    .reps
                    .iter()
                    .find(|x| x.id.to_string() == *r)?
                    .device
            })
            && let Some(text) = self.seat_words(device)
        {
            self.notify(text, true);
        }
        let reps: Vec<EntityRef> = moved
            .reps
            .iter()
            .filter_map(|id| {
                self.canvas()?
                    .page()
                    .entities
                    .iter()
                    .find(|(e, _)| e.0.to_string() == *id)
                    .map(|(e, _)| *e)
            })
            .collect();
        if !reps.is_empty() && self.tools.selection.apply(&SelectionChange::Replace(reps)) {
            self.announce_selection();
        }
    }

    /// Routes the wires at the pins of the representations `reps` (ID texts) of the page on show, as Route routes a
    /// selection's, and folds the routes into the step before (the turn).
    fn route_turned(&mut self, reps: &[String]) {
        let ids: Vec<ew_api::output::Uuid> = reps.iter().filter_map(|r| r.parse().ok()).collect();
        let Some(folio) = self.current_folio().map(str::to_owned) else {
            return;
        };
        let plan = self.project.as_ref().and_then(|pr| {
            pr.session
                .route_plan(&folio, &ew_api::routing::RouteScope::Selection(ids))
        });
        let Some(params) = plan.and_then(|p| p.params) else {
            return;
        };
        let rev = self.revision();
        let routed = self.run_commit(&AnyCommand {
            name: "layout.apply".into(),
            params,
        });
        if routed == Outcome::Done
            && self.revision() != rev
            && let Some(pr) = &mut self.project
        {
            pr.session.fold_last_step();
        }
    }

    /// A placement (`device.place`, `box.black`, `ip.place`, `connection.draw_to_ip`) or an exchange
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

    /// A valve placed inside a valve terminal's box must not stay a loose device: a `device.place` whose
    /// insertion point lies inside the box of a valve terminal of the page, and a `rep.move` of one representation whose
    /// new insertion point does, name that terminal as `into` (the command seats a valve or a supply plate there and
    /// leaves any other device). Any other command, and a move of the terminal's own box, is returned as it is.
    fn with_terminal(&self, cmd: Box<dyn DynCommand>) -> Box<dyn DynCommand> {
        if !matches!(cmd.name(), "device.place" | "rep.move") {
            return cmd;
        }
        let (Ok(mut params), Some(tables)) =
            (cmd.to_json(), self.canvas().map(|c| Arc::clone(&c.page().tables)))
        else {
            return cmd;
        };
        let at = |v: &Value| Some((v[0].as_i64()?, v[1].as_i64()?));
        let (point, moved) = if cmd.name() == "device.place" {
            (at(&params["pos"]), None)
        } else {
            let grid = crate::tools::wire::PageGridMap::new(tables.grid);
            let rep = match params["reps"].as_array().map(Vec::as_slice) {
                Some([one]) => one
                    .as_str()
                    .and_then(|r| tables.reps.iter().find(|x| x.id.to_string() == r)),
                _ => None,
            };
            let point = rep.zip(at(&params["steps"])).map(|(r, (kx, ky))| {
                (
                    grid.coord(grid.index(r.pos.0) + kx),
                    grid.coord(grid.index(r.pos.1) + ky),
                )
            });
            (point, rep)
        };
        let Some(point) = point else { return cmd };
        let inside = |outline: &[(i64, i64)]| {
            let (xs, ys) = (outline.iter().map(|p| p.0), outline.iter().map(|p| p.1));
            matches!((xs.clone().min(), xs.max(), ys.clone().min(), ys.max()),
                (Some(x0), Some(x1), Some(y0), Some(y1)) if x0 < point.0 && point.0 < x1 && y0 < point.1 && point.1 < y1)
        };
        let terminal = tables
            .reps
            .iter()
            .filter(|r| moved.is_none_or(|m| m.id != r.id && m.device != r.device))
            .filter(|r| {
                r.black_box
                    .as_ref()
                    .is_some_and(|b| b.valve_terminal && inside(&b.outline))
            })
            .find_map(|r| r.device);
        let (Some(terminal), Some(m)) = (terminal, params.as_object_mut()) else {
            return cmd;
        };
        m.insert("into".to_owned(), Value::String(terminal.to_string()));
        Box::new(AnyCommand {
            name: cmd.name().to_owned(),
            params,
        })
    }

    /// The cell of the box representation `rep` (a valve terminal drawn as its positions) at the sheet point `at`, with
    /// what holds it: (terminal, its cell); `None` for any other representation or outside its cells.
    pub(crate) fn cell_at(
        &self,
        rep: ew_api::output::Uuid,
        at: (i64, i64),
    ) -> Option<(ew_api::output::Uuid, ew_api::output::SlotCell)> {
        let c = self.canvas()?;
        let p = c.page().snapper.sheet_to_page(at);
        let r = c.page().tables.reps.iter().find(|r| r.id == rep)?;
        let cell = r
            .black_box
            .as_ref()?
            .cells
            .iter()
            .find(|x| x.rect[0] <= p.0 && p.0 < x.rect[2] && x.rect[1] <= p.1 && p.1 <= x.rect[3])?;
        Some((r.device?, cell.clone()))
    }

    /// The `fluid.cell_fill` parameters of the empty cell of `rep` under the pointer; `None` elsewhere.
    fn empty_cell_under_pointer(&self, rep: ew_api::output::Uuid) -> Option<Value> {
        let at = self.canvas()?.pointer()?;
        let (terminal, cell) = self.cell_at(rep, at).filter(|(_, c)| c.holder.is_none())?;
        Some(
            serde_json::json!({ "terminal": terminal.to_string(), "module": cell.module.to_string(),
            "position": cell.position }),
        )
    }

    /// The sentence that says where a device dropped into a valve terminal's box now sits (a toast
    /// that names the position), or that the terminal had no free position; `None` for a device that is neither a valve
    /// nor on a valve terminal.
    pub(super) fn seat_words(&self, device: ew_api::output::Uuid) -> Option<String> {
        use ew_api::output::TerminalSeat;
        Some(match self.project.as_ref()?.session.terminal_seat(device)? {
            TerminalSeat::On {
                device,
                terminal,
                position: Some(p),
            } => strings::format(key!("toast-seat-valve"), &[&device, &p.to_string(), &terminal]),
            TerminalSeat::On { device, terminal, .. } => {
                strings::format(key!("toast-seat-plate"), &[&device, &terminal])
            }
            TerminalSeat::Loose { device } => strings::format(key!("toast-seat-full"), &[&device]),
            TerminalSeat::Own { device } => strings::format(key!("toast-seat-own"), &[&device]),
        })
    }

    /// The far autoconnect hits a `device.place` does not join, offered by its toast; none at its point moved or with
    /// Alt (a placement without its proposals' hints although it has some). Not inlined: the dispatch's frame stays as
    /// it was (the main thread's stack is small).
    #[inline(never)]
    fn far_offers(&self, cmd: &dyn DynCommand) -> Vec<((i64, i64), String, ew_api::output::Uuid, u16)> {
        let Some(crate::tools::Ghost::Place {
            far, hints, at_nm, ..
        }) = self.tools.tool.ghost()
        else {
            return Vec::new();
        };
        let json = cmd.to_json().unwrap_or_default();
        let without_hints = hints > 0 && json.get("connect").is_none();
        let same_point = at_nm.is_some_and(|(x, y)| json["pos"] == serde_json::json!([x, y]));
        if without_hints || !same_point {
            Vec::new()
        } else {
            far
        }
    }

    /// The wires (or tubes, the `bool`) a placement's autoconnect hints draw, each as its own pin and the pin it goes
    /// to in words: the ghost's pin designation, or for a terminal its strip and designation as the ghost names them
    /// (`-X??:1` before the strip has a tag; the toast names the terminal by the placed strip's tag), since a row of
    /// terminals names its last; `None` without hints.
    fn autoconnect_words(&self, cmd: &dyn DynCommand) -> Option<(bool, Vec<(String, String)>)> {
        let json = cmd.to_json().ok()?;
        let connect = json["connect"].as_array().cloned().unwrap_or_default();
        let hints = connect.len();
        if hints == 0 {
            return None;
        }
        // Tubes where every hint ends at a fluid port of the page.
        let tubes = self.canvas().is_some_and(|c| {
            let cps = &c.page().tables.cps;
            connect.iter().all(|h| {
                let to = &h["to"]["cp"];
                cps.iter().any(|cp| {
                    cp.fluid
                        && to["function"].as_str() == Some(cp.function.to_string().as_str())
                        && to["idx"].as_u64() == Some(u64::from(cp.idx))
                })
            })
        });
        let Some(crate::tools::Ghost::Place { proposals, strip, .. }) = self.tools.tool.ghost() else {
            return None;
        };
        let list: Vec<(String, String)> = proposals
            .iter()
            // A shared pin designation reads by its position in words in the sentence, not as `-#2`.
            .filter_map(|(cp, target)| {
                let own = strip.clone().unwrap_or_else(|| cp.trim_matches('"').to_owned());
                target
                    .as_ref()
                    .map(|t| (own, crate::describe::pin_in_sentence(t)))
            })
            .collect();
        Some((tubes, list))
    }

    /// After a committed placement of the schematic Place tool: the representations the page tables
    /// hold now and did not before the commit are the selection, one selection change; the tool stays armed.
    pub(super) fn after_place(&mut self) {
        if !std::mem::take(&mut self.tools.placed) {
            return;
        }
        let before = self.tools.before_commit.clone();
        let Some(now) = self.canvas().map(|c| Arc::clone(&c.page().tables)) else {
            return;
        };
        let placed: Vec<EntityRef> = now
            .reps
            .iter()
            .filter(|r| {
                before
                    .as_ref()
                    .is_none_or(|b| !b.reps.iter().any(|o| o.id == r.id))
            })
            .map(|r| EntityRef(r.id))
            .collect();
        if !placed.is_empty() && self.tools.selection.apply(&SelectionChange::Replace(placed)) {
            self.announce_selection();
        }
    }

    /// Whether the HUD keeps quiet about the pick stack under the pointer: it rests where a drop left it.
    pub(crate) fn stack_quiet(&self) -> bool {
        self.tools.quiet_stack_at.is_some()
    }

    /// Announces the selection through the live region: [`crate::status::selection_text`].
    pub(super) fn announce_selection(&mut self) {
        let text = crate::status::selection_text(self);
        self.tools.selection_announced = Some((self.revision(), strings::generation(), text.clone()));
        self.tools.announcement = Some(text);
    }

    /// Words the selection's announcement again at another revision or in another language while the live region
    /// still holds it (a rename in Properties keeps the selection); the end of every dispatch.
    pub(super) fn sync_selection_announcement(&mut self) {
        let Some((rev, generation, text)) = &self.tools.selection_announced else {
            return;
        };
        if self.tools.announcement.as_deref() != Some(text.as_str()) {
            self.tools.selection_announced = None;
        } else if *rev != self.revision() || *generation != strings::generation() {
            self.announce_selection();
        }
    }

    /// The live description's values of the one selected record, from the session; `None` unless
    /// exactly one record is selected and it is a device (or one of its functions or representations), a connection or
    /// a page item.
    pub fn selection_words(&self) -> Option<ew_api::output::SelectionWords> {
        let [one] = self.tools.selection.items.as_slice() else {
            return None;
        };
        self.project.as_ref()?.session.selection_words(one.0)
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
            None => Outcome::refused(key!("reason-no-link")),
        }
    }

    /// The submission of the tool and grid commands: the GUI in the window, the script's command log headless.
    pub(super) fn tool_submit(&self) -> Submit {
        Submit::from(super::ShellModel::origin_of(
            self.mode,
            self.person,
            &self.origin_file,
        ))
    }

    /// `Preview(cmd)`: `Session::preview_with` once, the semantic ghost from `Session::derived_diff`; a
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
        // What the tool may show of its preview (the predicted tag, the unmapped CPs).
        let mut info = PreviewInfo {
            ok: result.is_ok(),
            tags: derived
                .tags
                .iter()
                .map(|c| (c.before.clone(), c.after.clone()))
                .collect(),
            notices: result
                .as_ref()
                .map(|p| notices_words(&p.outcome().notices))
                .unwrap_or_default(),
            message: result.as_ref().err().map(|e| command_error_message(e, &json)),
        };
        // A paste that brings a cut device back to another page says before the click that its tag follows the page,
        // as its toast says after it.
        if cmd.name() == "edit.paste" {
            for c in &derived.tags {
                if let Some((_, was)) = self.tools.cut_tags.iter().find(|(d, _)| *d == c.key.as_uuid())
                    && *was != c.after
                {
                    info.notices
                        .push(strings::format(key!("label-paste-retag"), &[was, &c.after]));
                }
            }
        }
        self.tools.preview = Some(PreviewState {
            base_rev: rev,
            result: result.map_err(|e| command_error_message(&e, &json)),
            command: json,
            name: cmd.name().to_owned(),
            derived,
        });
        if let Some(overlay) = self.tools.tool.on_preview(&info) {
            self.tools.overlay = overlay;
        }
    }

    /// `Commit(cmd)` of the tool host: [`ShellModel::commit`] with the tool's kept preview of `cmd` when it is
    /// fresh, else `cmd` executed. A read-only project refuses before anything runs.
    pub(super) fn run_commit(&mut self, cmd: &dyn DynCommand) -> Outcome {
        if !self.is_writable() {
            return Outcome::refused(if self.has_project() {
                key!("reason-read-only")
            } else {
                key!("reason-no-project")
            });
        }
        let json = match cmd.to_json() {
            Ok(v) => v,
            Err(e) => return command_failed(&e, &Value::Null),
        };
        self.commit(cmd.name().to_owned(), json, Kept::Tool(cmd))
    }

    /// The one commit path of the shell, for the tool host and the cards alike: headless a one-line command log
    /// through `Session::run_script`, exactly `command.run`'s path, so the exposure rules and `--record` apply; in the
    /// window `Session::commit` of the preview `kept` names, else `Session::execute_with` of the tool's command (a
    /// preview and its commit for a bulk command). A committed step joins the record, and the rows refresh.
    pub(super) fn commit(&mut self, name: String, json: Value, kept: Kept<'_>) -> Outcome {
        // Another commit ends a Terminal… row's single step for Undo and Redo.
        self.tools.row_steps = None;
        if self.mode == Mode::Headless {
            return self.run_logged(name, json);
        }
        // A card's section, which its refusal names a target of (`None` for the tool host's commit), and the tool's
        // kept preview when it is fresh.
        let (card, fresh_tool) = match &kept {
            Kept::Card(_, section) => (Some(*section), None),
            Kept::Tool(_) => {
                let rev = self.revision();
                let fresh = self.tools.preview.as_ref().is_some_and(|p| {
                    Some(p.base_rev) == rev && p.name == name && p.command == json && p.result.is_ok()
                });
                (None, if fresh { self.tools.preview.take() } else { None })
            }
        };
        // The submission's own origin (a scripted window submits as its command log).
        let origin = self.origin();
        let calls = &self.calls;
        let project = or_return!(crate::shell::gated_mut(&mut self.project));
        let result = match (kept, fresh_tool.map(|p| p.result)) {
            (Kept::Card(state, _), _) => match state.result {
                Ok(preview) => {
                    bump(calls, |c| c.commit += 1);
                    project.session.commit(preview)
                }
                Err(message) => return Outcome::Failed { message },
            },
            (Kept::Tool(_), Some(Ok(preview))) => {
                bump(calls, |c| c.commit += 1);
                project.session.commit(preview)
            }
            (Kept::Tool(cmd), _) => {
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
        self.tools.refused_by = match &result {
            Err(CmdError::Prevented { rule, .. }) => Some(rule.to_string()),
            _ => None,
        };
        self.tools.commit_notices = match &result {
            Ok(receipt) => notices_words(&receipt.outcome.notices),
            Err(_) => Vec::new(),
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
            Err(e) => {
                let out = match card {
                    Some(section) => Outcome::Failed {
                        message: self.refusal(&e, section),
                    },
                    None => command_failed(&e, &json),
                };
                self.command_refused(&e, out)
            }
        };
        self.refresh_rows();
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
        self.tools.put_down = false;
        self.tool_epoch = self.tool_epoch.wrapping_add(1);
        // A tool works on the canvas: its keys (D, R, Enter) go there, not to the navigator or the Properties row that
        // had the focus (D after choosing Interruption jumped in the page list).
        if self.tools.tool.name() != "select"
            && self
                .focus
                .as_ref()
                .is_some_and(|f| !matches!(f, Focus::Canvas(_)))
        {
            let g = self.area.as_ref().map_or(0, |a| a.focused);
            self.focus = Some(Focus::Canvas(g));
        }
        self.announce_tool();
        if let Some(at) = self.canvas().and_then(CanvasModel::cursor) {
            let _ = self.send_tool(&CanvasEvent::CursorMove { at });
        }
    }

    /// The tool's part of the Escape chain: `Escape` to the tool; when it answers `None` (its first state), a
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

    /// The grid cursor of group `g` moved: the active tool learns it when `g` is the focused group.
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
        // A move away from a drop's point lets the HUD name the pick stack again.
        if self.tools.quiet_stack_at.is_some_and(|q| q != at) {
            self.tools.quiet_stack_at = None;
        }
        self.send_tool(&CanvasEvent::PointerMove { at, mods })
    }

    // -----------------------------------------------------------------------------------------------------------
    // Actions

    /// `tool.select`, `tool.measure`.
    pub(crate) fn act_tool(&mut self, name: &str) -> Outcome {
        match tool_named(name) {
            Some(tool) => {
                self.activate_tool(tool);
                Outcome::Done
            }
            None => failed(key!("error-unknown-action"), &[name]),
        }
    }

    /// `canvas.press` and `canvas.release`: `x_mm`, `y_mm`, optional `button` (`primary`, `middle`),
    /// `shift`, `ctrl`, `alt` and `group`. A press in the other group focuses it first; the middle button reaches
    /// no tool (it pans). A primary release after the tool's context changed under its press is dropped: no focus,
    /// pointer, tool or selection change.
    pub(crate) fn act_press(&mut self, p: &Map<String, Value>, release: bool) -> Outcome {
        let at = match point_mm(p) {
            Ok(Some(at)) => at,
            Ok(None) => return failed(key!("error-missing-param"), &["x_mm"]),
            Err(e) => return e,
        };
        let button = match p.get("button") {
            None => Button::Primary,
            Some(v) => match v.as_str() {
                Some("primary") => Button::Primary,
                Some("middle") => Button::Middle,
                _ => return failed(key!("error-bad-param"), &["button"]),
            },
        };
        if button == Button::Primary {
            if release && std::mem::take(&mut self.tools.drop_release) {
                return Outcome::Done;
            }
            // While the strip chooser waits for a strip, a press on the canvas starts
            // nothing (the Select tool under it began a rubber band); the hint says what is wanted, and the press's
            // release is dropped with it.
            if !release && self.strip_chooser_open() {
                self.tools.press_held = false;
                self.tools.drop_release = true;
                return Outcome::refused(key!("reason-choose-strip"));
            }
            // So does the symbol chooser of a part without a symbol.
            if !release && self.tools.picker.as_ref().is_some_and(|p| p.part.is_some()) {
                self.tools.press_held = false;
                self.tools.drop_release = true;
                return Outcome::refused(key!("reason-choose-symbol"));
            }
            // A new press: a release that never came is forgotten (before the focus moves, so the press's own
            // context change drops nothing).
            self.tools.press_held = false;
            self.tools.drop_release = false;
        }
        let g = or_return!(self.group_param(p));
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
            return Outcome::refused(key!("reason-no-page"));
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

    /// `canvas.dwell`: `ms`, a positive integer. At 300 ms and more with a hovered link (while the tool
    /// wants hover) the link preview shows its target; the tool gets `Dwell` either way.
    pub(crate) fn act_dwell(&mut self, p: &Map<String, Value>) -> Outcome {
        let ms = match p.get("ms") {
            None => return failed(key!("error-missing-param"), &["ms"]),
            Some(v) => match v.as_u64().and_then(|m| u32::try_from(m).ok()).filter(|m| *m > 0) {
                Some(ms) => ms,
                None => return failed(key!("error-bad-param"), &["ms"]),
            },
        };
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
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
                // With no key, the command by its menu path, as the `external-none` text names one.
                let key = crate::command_bar::key_of(self, "nav.follow");
                let go = if key.is_empty() {
                    strings::text(key!("link-preview-go-none")).to_owned()
                } else {
                    strings::format(key!("link-preview-go"), &[&key])
                };
                self.tools.link_preview = Some(LinkPreview {
                    target,
                    canvas: view,
                    go,
                });
            }
        }
        self.send_tool(&CanvasEvent::Dwell { ms })
    }

    /// `canvas.activate` (Enter): `Activate` to the tool, at the grid cursor; with Select idle and exactly one junction
    /// selected, the junction sequence editor opens instead. With `alt` (headless) it
    /// is a click with Alt held at the grid cursor (a press and a release): the keyboard's way to place a symbol without
    /// its autoconnect proposals (FR-CON-003) or to fix a wire's bend without object snaps.
    pub(crate) fn act_activate_with(&mut self, p: &Map<String, Value>) -> Outcome {
        if !flag(p, "alt") {
            return self.act_activate();
        }
        let Some(at) = self.canvas().and_then(CanvasModel::cursor) else {
            return Outcome::refused(key!("reason-no-cursor"));
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
        // On a panel page (doc 12 §12.4), Enter with a placement selected focuses Properties.
        if self.tools.tool.name() == "select" && self.selected_placements() > 0 {
            // Enter is the next action; a nudge's "Blocked by T55" ends with it.
            self.tools.notice = None;
            self.tools.notice_sticky = false;
            self.tools.notice_at = None;
            return self.act_props_focus();
        }
        // The paste tool commits without a grid cursor (its ghost stays where duplicate or the pointer put it), and so
        // does the Wire marker typing over the
        // wires a dragged line crossed (Enter placed nothing after a drag by the pointer).
        let marker_typing = self.tools.tool.name() == "marker" && self.tools.tool.state() == "typing";
        if (marker_typing || matches!(self.tools.tool.name(), "paste" | "layout_place"))
            && self.canvas().and_then(CanvasModel::cursor).is_none()
        {
            return self.send_tool(&CanvasEvent::Activate);
        }
        // Without a grid cursor the Duct tool's Enter finishes the line its clicks drew, as its hint says, or says why
        // not.
        if self.tools.tool.name() == "duct_draw" && self.canvas().and_then(CanvasModel::cursor).is_none() {
            return self.send_tool(&CanvasEvent::Key(crate::tools::ToolKey::Finish));
        }
        let Some(at) = self.canvas().and_then(CanvasModel::cursor) else {
            return Outcome::refused(key!("reason-no-cursor"));
        };
        let _ = self.send_tool(&CanvasEvent::CursorMove { at });
        self.send_tool(&CanvasEvent::Activate)
    }

    /// `tool.back` (Backspace): `Back` to the tool.
    pub(crate) fn act_tool_back(&mut self) -> Outcome {
        self.send_tool(&CanvasEvent::Back)
    }

    /// `cp.next`, `cp.previous`, `cp.next_unconnected`: the grid cursor goes to the next or previous
    /// connection point of the page in reading order after the cursor (from the view centre without one), wrapping;
    /// `cp.next_unconnected` only to a connection point no connection ends at.
    pub(crate) fn act_cp_step(&mut self, p: &Map<String, Value>, step: CpStep) -> Outcome {
        let g = or_return!(self.group_param(p));
        let last = self.tools.last_cp.clone();
        let Some(c) = self.canvas_mut(g) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let page = Arc::clone(c.page());
        let candidates: Vec<usize> = (0..page.cps.len())
            .filter(|&i| step != CpStep::NextUnconnected || !page.cps[i].connected)
            .collect();
        let (Some(&first), Some(&end)) = (candidates.first(), candidates.last()) else {
            return Outcome::refused(if page.cps.is_empty() {
                key!("reason-no-cp")
            } else {
                key!("reason-no-unconnected-cp")
            });
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

/// What [`ShellModel::commit`] commits in the window, and how a refusal of it reads: the one difference between the
/// tool host's commits and the cards'.
#[allow(clippy::large_enum_variant)] // a transient argument, moved once into the commit
pub(super) enum Kept<'a> {
    /// The tool host's command: the tool's kept preview of it when that is fresh, else the command executed; a
    /// refusal in the command's words ([`command_failed`]).
    Tool(&'a dyn DynCommand),
    /// A card's or the property grid's kept, fresh preview (a refused preview fails with its own message); a refusal
    /// of the commit names a target of the section by its label.
    Card(PreviewState, Option<&'a SheetSection>),
}

/// The toast of a refused command: an invalid parameter names its field in words, never its JSON Pointer
/// (the command error itself stays machine-readable for the CLI); any other error as it prints. Every shell path that
/// shows a command error goes through it: the tool host, `command.run`, the creation dialogs, paste, undo and redo.
pub(super) fn command_failed(e: &CmdError, params: &Value) -> Outcome {
    Outcome::Failed {
        message: command_error_message(e, params),
    }
}

/// A command error with `params` in the window's language: what [`command_failed`] says, and what a refused preview's
/// card shows.
pub(super) fn command_error_message(e: &CmdError, params: &Value) -> String {
    match e {
        // The reason in the window's language.
        CmdError::InvalidParams { command, path, text } => strings::format(
            key!("error-command-field"),
            &[
                strings::command_title(command),
                &param_words(params, path),
                &rejected_words("", Some(&**text)),
            ],
        ),
        // A refusal shows its reason, never the error's Display form ("<source>: <reason>"), and
        // in the window's language
        CmdError::Rejected { text, .. } => {
            strings::format(key!("error-command"), &[&rejected_words("", Some(&**text))])
        }
        // Every other reason through its own key in the window's language,
        // never the error's Display form (which names the exposure class and the origin, `cmdlog:script.uiscript`).
        other => command_error_words(other),
    }
}

/// A command's own refusal in the window's language: its keyed text through the one renderer
/// ([`strings::render_with`]), with the shell's words for the keys it names in its own terms ([`own_words`]); a
/// refusal without a key (one of the API's own) as its English `reason`.
pub(crate) fn rejected_words(reason: &str, text: Option<&ew_api::Text>) -> String {
    text.map_or_else(
        || reason.to_owned(),
        |t| own_words(t).unwrap_or_else(|| strings::render_with(t, &own_words)),
    )
}

/// The shell's words for a text whose argument it names in its own terms: a JSON Pointer (`refusal-words-pointer`) as
/// its member in words, as an invalid parameter names it, and a part kind (`refusal-words-part-kind`) by its choice's
/// words; `None` for any other text.
fn own_words(t: &ew_api::Text) -> Option<String> {
    match (t.key(), t.args()) {
        ("refusal-words-pointer", [ew_api::TextArg::Plain(pointer)]) => {
            Some(param_words(&Value::Null, pointer))
        }
        ("refusal-words-part-kind", [ew_api::TextArg::Plain(kind)]) => {
            Some(strings::in_sentence(&strings::choice_text(kind)))
        }
        _ => None,
    }
}

/// A command's notices in the window's language as the Preview pane shows them: each with the words before and after
/// the arrow of its change (`notice-change`, marked while it renders), where it says one.
pub(crate) fn preview_notices(notices: &[ew_api::Text]) -> Vec<crate::preview::PreviewNotice> {
    const MARK: char = '\u{1f}';
    let marked = |t: &ew_api::Text| -> Option<String> {
        if t.key() != "notice-change" {
            return own_words(t);
        }
        let side = |a: &ew_api::TextArg| match a {
            ew_api::TextArg::Text(x) => rejected_words("", Some(x)),
            other => other.to_string(),
        };
        match t.args() {
            [before, after] => Some(format!("{}{MARK}{}", side(before), side(after))),
            _ => None,
        }
    };
    notices
        .iter()
        .map(|t| crate::preview::PreviewNotice {
            words: rejected_words(&t.to_string(), Some(t)),
            change: strings::render_with(t, &marked)
                .split_once(MARK)
                .map(|(a, b)| (a.to_owned(), b.to_owned())),
        })
        .collect()
}

/// A command's notices (`Outcome::notices`, what it dropped or adjusted instead of refusing) in the window's
/// language, in their order.
pub(crate) fn notices_words(notices: &[ew_api::Text]) -> Vec<String> {
    notices
        .iter()
        .map(|t| rejected_words(&t.to_string(), Some(t)))
        .collect()
}

/// A prevent rule's refusal in the window's language, as a sentence without the rule's code (the code belongs in the
/// findings list and the help): a finding of the commands' own by its key, a finding of the check catalogue by the
/// rule's message rendered from its arguments, else the finding as it prints.
pub(crate) fn prevented_words(text: &ew_api::Text) -> String {
    // A rule message of the check catalogue.
    if ew_api::checks::has_key(text.key()) {
        ew_api::checks::messages::text(text, strings::rule_language())
    } else {
        rejected_words("", Some(text))
    }
}

/// A command error that is neither an invalid parameter nor a command's refusal in the window's language, as
/// [`command_error_text`] words it: the property grid's and Find's refusals read it.
pub(crate) fn command_error_words(e: &CmdError) -> String {
    let (key, args) = command_error_text(e);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    strings::format(key, &args)
}

/// The keys [`command_error_text`] renders a command error by, one per reason: a test checks each has its
/// text in both languages with the same arguments.
#[cfg(test)]
const COMMAND_ERROR_KEYS: &[&str] = &[
    "error-command-unknown",
    "error-command-not-found",
    "error-command-human-only",
    "error-command-import-only",
    "error-command-internal-only",
    "error-command-needs-preview",
    "error-command-history",
    "error-command-internal",
    "error-history-checkpoint",
    "error-history-changed",
    "reason-nothing-to-undo",
    "reason-nothing-to-redo",
];

/// The key and the arguments of a command error that is neither an invalid parameter nor a command's refusal:
/// the command by its title, a history or internal error with its text, a prevent rule or a model
/// error in its words without its rule code. The match names every reason, so a new one does not build until it has its key.
fn command_error_text(e: &CmdError) -> (Key<'static>, Vec<String>) {
    let title = |c: &str| strings::command_title(c).to_owned();
    match e {
        CmdError::UnknownCommand { name } => (key!("error-command-unknown"), vec![name.clone()]),
        // A step on something deleted meanwhile names it by what it is: never "an item the command names".
        CmdError::NotFound { key } => {
            let thing = match key.0.name() {
                "device" | "function" | "channel" => key!("error-gone-device"),
                "representation" | "symbol" => key!("error-gone-symbol"),
                "page" => key!("error-gone-page"),
                "connection" | "conductor" => key!("error-gone-wire"),
                "part" => key!("error-gone-part"),
                _ => key!("error-gone-item"),
            };
            (
                key!("error-command-not-found"),
                vec![strings::text(thing).to_owned()],
            )
        }
        CmdError::Exposure {
            command, exposure, ..
        } => match exposure {
            ew_api::Exposure::Import => (key!("error-command-import-only"), vec![title(command)]),
            ew_api::Exposure::Internal => (key!("error-command-internal-only"), vec![title(command)]),
            ew_api::Exposure::HumanOnly | ew_api::Exposure::Edit => {
                (key!("error-command-human-only"), vec![title(command)])
            }
        },
        CmdError::NeedsPreview { command } => (key!("error-command-needs-preview"), vec![title(command)]),
        // A prevent rule's and the model's own words, without their rule code, which the IP tool and the card read,
        // in the window's language.
        CmdError::Prevented { text, .. } => (key!("error-command"), vec![prevented_words(text)]),
        CmdError::Model(m, text) => (
            key!("error-command"),
            vec![
                text.as_deref()
                    .map_or_else(|| m.to_string(), |t| rejected_words("", Some(t))),
            ],
        ),
        CmdError::History { block, .. } => match block {
            ew_api::HistoryBlock::NothingToUndo => (key!("reason-nothing-to-undo"), Vec::new()),
            ew_api::HistoryBlock::NothingToRedo => (key!("reason-nothing-to-redo"), Vec::new()),
            ew_api::HistoryBlock::Checkpoint { label } => (
                key!("error-command-history"),
                vec![strings::format(
                    key!("error-history-checkpoint"),
                    &[&title(label)],
                )],
            ),
            ew_api::HistoryBlock::Changed { label } => (
                key!("error-command-history"),
                vec![strings::format(key!("error-history-changed"), &[&title(label)])],
            ),
        },
        CmdError::Internal { reason } => (key!("error-command-internal"), vec![reason.clone()]),
        CmdError::InvalidParams { text, .. } | CmdError::Rejected { text, .. } => {
            (key!("error-command"), vec![rejected_words("", Some(&**text))])
        }
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
        return strings::text(key!("error-param-whole")).to_owned();
    };
    // The words of the UI language.
    let base = match segments[at] {
        "ips" => strings::text(key!("param-ips")).to_owned(),
        "cps" => strings::text(key!("param-cps")).to_owned(),
        "reps" => strings::text(key!("param-reps")).to_owned(),
        "rep" => strings::text(key!("param-rep")).to_owned(),
        other => strings::field_label("", other),
    };
    match segments.get(at + 1).and_then(|s| s.parse::<usize>().ok()) {
        Some(i) => strings::format(key!("error-param-item"), &[&base, &(i + 1).to_string()]),
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

    /// The text of `key` with `args`, as a command builds it with `text!`.
    fn keyed(key: &'static str, args: Vec<ew_api::TextArg>) -> ew_api::Text {
        ew_api::Text::checked_by_macro(
            key,
            ew_commands::text_english(key).unwrap_or("?"),
            Some(ew_commands::catalogue),
            args,
        )
    }

    /// The toast of an invalid parameter names the field in words: a field set by `fields.set` by its label,
    /// any other parameter by its member humanised with its list position; never a JSON Pointer.
    #[test]
    fn an_invalid_parameter_toast_names_the_field_in_words() {
        let set =
            serde_json::json!({"kind": "function", "targets": [], "path": "kind.ip.sort", "value": "x"});
        let e = CmdError::invalid_params(
            "fields.set",
            "/value",
            keyed("refusal-params-detail", vec!["a whole number is needed".into()]),
        );
        assert_eq!(
            message(command_failed(&e, &set)),
            "Set property: Chain order: a whole number is needed"
        );
        let e = CmdError::invalid_params("ip.arrange", "/ips/2", keyed("refusal-params-ip-twice", vec![]));
        assert_eq!(
            message(command_failed(&e, &Value::Null)),
            "Arrange interruption points: Interruption points, item 3: the interruption point is listed twice"
        );
        let e = CmdError::invalid_params(
            "connection.delete",
            "/connections",
            keyed("refusal-params-no-connection", vec![]),
        );
        assert_eq!(
            message(command_failed(&e, &Value::Null)),
            "Delete wire: Connections: give at least one connection"
        );
    }

    /// An invalid parameter's reason reads in the window's language, a text inside it too, as a
    /// refusal's does; the error keeps its English message for the CLI.
    #[test]
    fn an_invalid_parameter_reads_in_the_windows_language() {
        let subject = keyed("refusal-fields-function-named", vec!["-Q1".into()]);
        let e = CmdError::invalid_params(
            "fields.set",
            "/path",
            keyed("refusal-fields-no-port", vec![subject.into()]),
        );
        assert_eq!(
            e.to_string(),
            "fields.set: invalid parameters at `/path`: function -Q1 has no bus port; set `bus_port` first"
        );
        let reason = || command_error_text(&e).1;
        assert_eq!(
            reason(),
            vec!["function -Q1 has no bus port; set `bus_port` first".to_owned()]
        );
        assert!(strings::set_locale("nl"));
        let nl = reason();
        assert!(strings::set_locale("en"));
        assert_eq!(
            nl,
            vec!["functie -Q1 heeft geen buspoort; stel eerst `bus_port` in".to_owned()]
        );
    }

    /// A schema violation inside a refusal names its member in words, never its JSON Pointer; the CLI's
    /// English keeps the pointer.
    #[test]
    fn a_schema_violation_names_its_member_in_words() {
        // `schema_error_text` of `SchemaError::UnsortedList { path: "/ports/1" }`.
        let violation = keyed(
            "refusal-schema-unsorted",
            vec![keyed("refusal-words-pointer", vec!["/ports/1".into()]).into()],
        );
        let e = CmdError::invalid_params(
            "parts.import",
            "/copy",
            keyed("refusal-params-detail", vec![violation.into()]),
        );
        assert!(e.to_string().contains("\"/ports/1\""), "{e}");
        let shown = message(command_failed(&e, &Value::Null));
        assert!(!shown.contains("/ports"), "{shown}");
        assert!(shown.contains("Ports, item 2"), "{shown}");
    }

    /// Another crate's error under `refusal-params-detail` (here serde's) reads in a
    /// Dutch window, the error keeping its English for the CLI.
    #[test]
    fn another_crates_detail_reads_in_a_dutch_window() {
        let serde = serde_json::from_value::<u32>(serde_json::json!("x")).unwrap_err();
        let e = CmdError::invalid_params(
            "fields.set",
            "/value",
            keyed(
                "refusal-params-detail",
                vec![ew_api::serde_error_text(&serde).into()],
            ),
        );
        assert_eq!(
            e.to_string(),
            "fields.set: invalid parameters at `/value`: invalid type: string \"x\", expected a whole number"
        );
        assert!(strings::set_locale("nl"));
        let nl = command_error_text(&e).1;
        assert!(strings::set_locale("en"));
        assert_eq!(
            nl,
            vec!["ongeldige invoer: ongeldig type: de tekst \"x\", verwacht: een geheel getal".to_owned()]
        );
    }

    /// A prevent rule's refusal reads in the window's language, the rule's message rendered from the
    /// finding's arguments after its code (which the IP tool reads); a history block by its own words; the errors keep
    /// their English for the CLI.
    #[test]
    fn a_prevented_edit_and_a_history_block_read_in_a_dutch_window() {
        let finding = "interruption point 0V: 2 source points and no star source; set all but one to target, or flag \
                       one as the star source";
        let meta = ew_api::checks::rule("EW-CORE-052").expect("registered").meta();
        let prevented = CmdError::Prevented {
            rule: "EW-CORE-052".into(),
            keys: Vec::new(),
            text: Box::new(
                ew_api::checks::Finding::new(meta, [], "", "")
                    .arg("name", "0V")
                    .arg("form", "sources")
                    .arg("count", "2")
                    .text,
            ),
        };
        let checkpoint = CmdError::History {
            reason: "`revision.release` is a non-undoable checkpoint; nothing before it can be undone".into(),
            block: ew_api::HistoryBlock::Checkpoint {
                label: "revision.release".into(),
            },
        };
        let nothing = CmdError::History {
            reason: "nothing to undo".into(),
            block: ew_api::HistoryBlock::NothingToUndo,
        };
        let words = || [&prevented, &checkpoint, &nothing].map(|e| message(command_failed(e, &Value::Null)));
        let en = words();
        assert!(strings::set_locale("nl"));
        let nl = words();
        assert!(strings::set_locale("en"));
        assert_eq!(prevented.to_string(), format!("EW-CORE-052: {finding}"));
        // The window reads the sentence; the code stays in the error as it prints and in the findings.
        assert_eq!(en[0], finding);
        let release = strings::command_title("revision.release");
        assert_eq!(
            en[1],
            format!(
                "Undo or redo cannot apply: {release} is a checkpoint that cannot be undone; nothing before it can be \
                 undone"
            )
        );
        assert_eq!(en[2], "Nothing to undo");
        assert_eq!(
            nl[0],
            "onderbrekingspunt 0V: 2 bronpunten en geen sterbron; maak ze op één na tot doel, of markeer \
             er één als sterbron"
        );
        assert_eq!(nl[2], "Niets om ongedaan te maken");
    }

    /// The `{ $pN }` arguments of the message `key` in the Fluent source `ftl`; `None` when it has no such message.
    fn arguments(ftl: &str, key: &str) -> Option<Vec<String>> {
        let line = ftl
            .lines()
            .find(|l| l.split_once(" = ").is_some_and(|(k, _)| k == key))?;
        let mut args: Vec<String> = line
            .split("{ $")
            .skip(1)
            .filter_map(|r| r.split_once(" }").map(|(a, _)| a.to_owned()))
            .collect();
        args.sort();
        args.dedup();
        Some(args)
    }

    /// Every reason a command error is rendered by has its text in en-GB and nl-NL, with
    /// the same arguments; the exhaustive match of `command_error_text` makes a new reason name its key, and this
    /// list holds it.
    #[test]
    fn every_command_error_reason_has_its_text_in_both_languages() {
        let en: String = strings::sources("en-GB").collect();
        let nl: String = strings::sources("nl-NL").collect();
        for key in COMMAND_ERROR_KEYS {
            let a = arguments(&en, key).unwrap_or_else(|| panic!("{key} has no en-GB text"));
            assert_eq!(arguments(&nl, key).as_ref(), Some(&a), "{key} in nl-NL");
        }
        let errors = [
            CmdError::UnknownCommand { name: "x.y".into() },
            CmdError::NeedsPreview {
                command: "part.assign".into(),
            },
            CmdError::History {
                reason: "r".into(),
                block: ew_api::HistoryBlock::NothingToUndo,
            },
            CmdError::History {
                reason: "r".into(),
                block: ew_api::HistoryBlock::Checkpoint { label: "x.y".into() },
            },
            CmdError::Internal { reason: "r".into() },
        ];
        for e in &errors {
            let (key, _) = command_error_text(e);
            assert!(COMMAND_ERROR_KEYS.contains(&key.as_str()), "{key:?}");
        }
    }

    /// A human-only command refused for a script's origin is worded by its title, in the window's
    /// language, never "exposure HumanOnly is refused for origin cmdlog:script.uiscript".
    #[test]
    fn a_refused_exposure_names_the_command_in_the_windows_language() {
        let e = CmdError::Exposure {
            command: "check.waive".into(),
            exposure: ew_api::Exposure::HumanOnly,
            origin: ew_api::Origin::CmdLog {
                file: "script.uiscript".into(),
            },
        };
        assert_eq!(
            message(command_failed(&e, &Value::Null)),
            "Only a person can run Waive finding, in the window; a script or a plugin cannot"
        );
        assert!(strings::set_locale("nl"));
        let nl = message(command_failed(&e, &Value::Null));
        assert!(strings::set_locale("en"));
        assert_eq!(
            nl,
            "Alleen een persoon kan Bevinding vrijstellen uitvoeren, in het venster; een script of een plug-in kan dat \
             niet"
        );
    }

    /// A command error names what it changes or misses as items ("elementen"), never records; what is gone
    /// meanwhile by what it is, never as "an item the command names".
    #[test]
    fn a_command_error_says_items_not_records() {
        let preview = CmdError::NeedsPreview {
            command: "part.assign".into(),
        };
        let texts = || {
            (
                message(command_failed(&preview, &Value::Null)),
                strings::format(
                    key!("error-command-not-found"),
                    &[strings::text(key!("error-gone-device"))],
                ),
            )
        };
        assert_eq!(
            texts(),
            (
                "Assign part changes many items: it is previewed first, then applied".to_owned(),
                "No longer in the project: the device this step works on. It was deleted or undone meanwhile; try \
                 again on what the window shows now"
                    .to_owned()
            )
        );
        assert!(strings::set_locale("nl"));
        let nl = texts();
        assert!(strings::set_locale("en"));
        assert_eq!(
            nl,
            (
                "Artikel toewijzen wijzigt veel elementen: eerst volgt een voorbeeld, daarna toepassen".to_owned(),
                "Niet meer in het project: het apparaat waarmee deze stap werkt. Het is intussen verwijderd of \
                 ongedaan gemaakt; probeer het opnieuw op wat het venster nu toont"
                    .to_owned()
            )
        );
    }

    /// A refused preview's card holds the refusal in the window's language, as a toast says it, never the error's
    /// English Display form.
    #[test]
    fn a_refused_previews_card_reads_in_a_dutch_window() {
        let e = CmdError::rejected("edit.paste", keyed("refusal-paste-generated-page", vec![]));
        assert_eq!(e.to_string(), "edit.paste: a generated page takes no paste");
        assert!(strings::set_locale("nl"));
        let nl = command_error_message(&e, &Value::Null);
        assert!(strings::set_locale("en"));
        assert_eq!(nl, "op een gegenereerde pagina kan niets worden geplakt");
    }
}
