// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The panel page's host duties (doc 12 §12.4): arming the layout place tool, the keyboard
//! placement (Tab and Shift+Tab cycle a rail's placements, the arrows nudge, R turns on the plate, X flips on a rail),
//! Lock and Unlock, the placing options and the canvas's live region for a selected placement.
//!
//! **Reads.** The page's panel tables ([`crate::canvas::CanvasPage::panel`], fetched once per (session, revision,
//! folio)) and, for the live region's findings, the online check of the revision (memoised in the session).
//!
//! **Commits** go through the host's commit path, one `layout.move` or `layout.lock` per key (one undo step each).

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::checks::{CheckMode, FindingState};
use ew_api::output::{Fit, PanelPlacement, PanelTables, SnapMode, Uuid, fits_rail};
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::navigation::bump;
use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::strings;
use crate::strings::Key;
use crate::tools::layout::{Step, Stop, mode_word, nudge_stop, quarter_quat};
use crate::tools::{
    CanvasEvent, EntityKind, LayoutOptions, LayoutPlaceTool, SelectionChange, ToolKey, clamp64,
    page_entity_kind,
};
use crate::{count_key, key};

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The panel tables of the focused canvas's page (a panel-layout page); `None` on any other page.
    pub fn panel_tables(&self) -> Option<Arc<PanelTables>> {
        self.canvas()?.page().panel.clone()
    }

    /// While the preview card of a layout command (`layout.*`: Route; a delete of a space, an enclosure or a carrier
    /// that takes many placements with it) is open over a panel page, the focused canvas shows the page's surface whole
    /// in the room the card leaves it (doc 12-3d §12.5: Route's card shows the cabinet whole); when the card closes the
    /// view the person had comes back. A card of another command (a frame's grid, an edit in Properties) keeps the whole
    /// page, as a fitted view refits to the smaller pane. A wire selected itself whose way the page draws (doc 12-3d
    /// §12.5: a row of the Connections table beside the page) shows the surface whole too. Run after every action.
    pub(super) fn sync_card_view(&mut self) {
        let group = self.area.as_ref().map_or(0, |a| a.focused);
        let layout = self
            .props
            .card
            .as_ref()
            .is_some_and(|c| c.preview.name.starts_with("layout."));
        // So does a wire selected itself (a row of the Connections table) whose way the panel page draws.
        let wire = self.props.card.is_none() && self.shown_ways(group).iter().any(|w| w.chip.is_some());
        let region = if layout || wire {
            self.panel_tables().and_then(|t| {
                let snapper = &self.canvas()?.page().snapper;
                let sheet: Vec<(i64, i64)> = t
                    .usable
                    .iter()
                    .map(|&(u, v)| snapper.page_to_sheet(t.to_page((i128::from(u), i128::from(v)))))
                    .collect();
                let (x0, x1) = (sheet.iter().map(|p| p.0).min()?, sheet.iter().map(|p| p.0).max()?);
                let (y0, y1) = (sheet.iter().map(|p| p.1).min()?, sheet.iter().map(|p| p.1).max()?);
                // A tenth more on each side: the enclosure's outline around the surface, and the ways' marks at its edge.
                let (mx, my) = ((x1 - x0) / 10, (y1 - y0) / 10);
                Some([x0 - mx, y0 - my, x1 + mx, y1 + my])
            })
        } else {
            None
        };
        let Some(canvas) = self.canvas_mut(group) else {
            return;
        };
        match region {
            Some(r) if canvas.region() != Some(r) => canvas.show_region(r),
            Some(_) => {}
            None => canvas.release_region(),
        }
    }

    /// How many placements of the panel page shown the selection holds.
    pub fn selected_placements(&self) -> usize {
        let Some(c) = self.canvas() else { return 0 };
        let page = c.page();
        if page.panel.is_none() {
            return 0;
        }
        self.tools
            .selection
            .items
            .iter()
            .filter(|e| page_entity_kind(page, **e) == Some(EntityKind::Placement))
            .count()
    }

    /// Whether the selected placement is locked (the canvas context menu offers Unlock then).
    pub fn selected_placement_locked(&self) -> bool {
        self.selected_placement().is_some_and(|(_, r)| r.locked)
    }

    /// Whether the selected placement's label was moved by a person (its menu offers Reset label position).
    pub fn selected_label_moved(&self) -> bool {
        self.selected_placement().is_some_and(|(_, r)| r.label_moved)
    }

    /// Whether the pointer, on a panel page with Select, is over a label that a drag moves on its own
    /// (`tools::grabbed_label`): the window shows the grab pointer there.
    pub fn label_under_pointer(&self) -> bool {
        if self.tools.tool.name() != "select" {
            return false;
        }
        let Some(c) = self.canvas() else { return false };
        let Some(at) = c.pointer() else { return false };
        let page = c.page();
        let cx = crate::tools::ToolCx {
            page,
            points_per_nm: c.points_per_nm(),
            pane: c.pane(),
            snapper: &page.snapper,
            selection: &self.tools.selection,
            layout: self.tools.layout_options,
            connect_on_move: self.tools.connect_on_move,
        };
        crate::tools::grabbed_label(&cx, at).is_some()
    }

    /// Whether the project has a mounting plate a layout page can show (an enclosure brings one).
    pub fn has_plates(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| !p.session.surface_choices().is_empty())
    }

    /// `layout.choose_plate` (Draw ▸ Layout ▸ Plate): the page's Surface row in Properties, opened on its choice of
    /// the project's mounting plates (the page selected, so Properties shows it).
    pub(crate) fn act_layout_choose_plate(&mut self) -> Outcome {
        if !self.has_plates() {
            return Outcome::refused(key!("reason-no-plate"));
        }
        let out = self.dispatch("select.set", &json!({"ids": []}));
        if out != Outcome::Done {
            return out;
        }
        // The Surface row on show whatever the pane held: the pane shown and its filter cleared.
        let out = self.dispatch("props.show", &json!({"on": true}));
        if out != Outcome::Done {
            return out;
        }
        if !self.properties_filter().is_empty() {
            let out = self.dispatch("props.filter", &json!({"text": ""}));
            if out != Outcome::Done {
                return out;
            }
        }
        self.dispatch("props.edit", &json!({"row": "page/derived.surface"}))
    }

    /// The placements of the panel page shown whose labels were moved by a person.
    pub fn moved_labels(&self) -> Vec<Uuid> {
        self.panel_tables()
            .map(|p| {
                p.placements
                    .iter()
                    .filter(|r| r.label_moved)
                    .map(|r| r.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Where Add legend would put the focused page's legend (`Session::legend_offer`); a page of another kind than a
    /// panel layout asks nothing.
    pub fn legend_offer(&self) -> ew_api::output::LegendOffer {
        match (self.focused_page_id(), self.project_session()) {
            (Some(page), Some(s)) if self.on_panel_page() => s.legend_offer(page),
            _ => ew_api::output::LegendOffer::NoLayout,
        }
    }

    /// The poses the floating layout ghost predicts (`LayoutGesture::predicted_poses`: the dropped item first, then
    /// each moved neighbour); empty without one. The tests compare them with the committed poses (R1).
    pub fn layout_predicted_poses(&self) -> Vec<(Uuid, (i64, i64, u8))> {
        match (self.tools.tool.layout_gesture(), self.panel_tables()) {
            (Some(g), Some(panel)) => g.predicted_poses(&panel),
            _ => Vec::new(),
        }
    }

    /// The placing options of the tool options card (the sticky mode and the grid step).
    pub fn layout_options(&self) -> LayoutOptions {
        self.tools.layout_options
    }

    /// What the tool options card shows: the sticky options, with the mode of the floating ghost while one floats (Ctrl
    /// held shows Push, Alt Swap, a member's slot Push).
    pub fn layout_card_options(&self) -> LayoutOptions {
        let mut options = self.tools.layout_options;
        if let Some(crate::tools::Ghost::Layout { mode, .. }) = self.tools.tool.ghost() {
            options.mode = match mode {
                "push" => SnapMode::Push,
                "swap" => SnapMode::Swap,
                _ => SnapMode::Free,
            };
        }
        options
    }

    /// Whether the tool options card shows the placing options: the layout place tool runs, or Select drags a
    /// placement (doc 16a §5.2).
    pub fn layout_options_shown(&self) -> bool {
        self.tools.tool.name() == "layout_place"
            || matches!(self.tools.tool.ghost(), Some(crate::tools::Ghost::Layout { .. }))
    }

    /// The selected placement (the primary entity, else the first placement of the selection) with the panel tables.
    pub(crate) fn selected_placement(&self) -> Option<(Arc<PanelTables>, PanelPlacement)> {
        let panel = self.panel_tables()?;
        let sel = &self.tools.selection;
        let row = sel
            .primary
            .and_then(|e| panel.placement(e.0))
            .or_else(|| sel.items.iter().find_map(|e| panel.placement(e.0)))?
            .clone();
        Some((panel, row))
    }

    /// The live region's text of the selected placement (doc 12 §12.4): "-102F1, rail 1 at 158 mm, no findings"
    /// (its clip point along the rail), on the plate "E55, plate at 120, 300 mm"; the findings are the online check's
    /// open findings that target the placement or its device. `None` unless exactly one placement is selected.
    pub fn layout_live_text(&self) -> Option<String> {
        if self.tools.selection.items.len() != 1 {
            return None;
        }
        let (panel, row) = self.selected_placement()?;
        let comma = panel.decimal_comma;
        match row.carrier.and_then(|c| panel.carrier(c)) {
            Some(rail) => {
                let n = self.findings_of(&[row.id, row.device]);
                let findings = match n {
                    0 => strings::text(key!("layout-findings-none")).to_owned(),
                    n => strings::format_count(count_key!("layout-findings"), n),
                };
                Some(strings::format(
                    key!("layout-live-rail"),
                    &[
                        &row.tag,
                        &strings::render(&rail.sentence),
                        &ew_api::output::mm_words(i128::from(row.pose.0), comma),
                        &findings,
                    ],
                ))
            }
            None => Some(strings::format(
                key!("layout-live-plate"),
                &[
                    &row.tag,
                    &ew_api::output::mm_words(i128::from(row.pose.0), comma),
                    &ew_api::output::mm_words(i128::from(row.pose.1), comma),
                ],
            )),
        }
    }

    /// The number of open findings of the online check of this revision that target any of `ids`.
    fn findings_of(&self, ids: &[Uuid]) -> usize {
        let Some(project) = self.project.as_ref() else {
            return 0;
        };
        let report = project
            .session
            .check(project.session.online_scheme(), CheckMode::Online);
        report
            .findings
            .iter()
            .filter(|f| matches!(f.state, FindingState::Open))
            .filter(|f| f.targets.iter().any(|t| ids.contains(&t.id.as_uuid())))
            .count()
    }

    // -----------------------------------------------------------------------------------------------------------
    // Actions

    /// Refuses with `text` in the HUD and the live region (a reason in words that names devices or keys), under the
    /// static `reason_key` for the transcript.
    fn refuse_with(&mut self, text: String, reason_key: Key<'static>) -> Outcome {
        self.tools.notice = Some(text.clone());
        self.tools.notice_sticky = false;
        self.tools.announcement = Some(text);
        Outcome::Refused {
            reason: reason_key.into(),
        }
    }

    /// `tool.place_layout {device}` (Place device, Enter on an Unplaced devices row, a drop of the row): arms the
    /// layout place tool with the device on the focused panel page; refused without one ("Open a panel layout page").
    pub(crate) fn act_place_layout(&mut self, p: &Map<String, Value>) -> Outcome {
        let device = match string_param(p, "device") {
            Ok(Some(d)) => d.trim().trim_start_matches("id:").to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["device"]),
            Err(e) => return e,
        };
        // An Unplaced devices row of a second part use drags as `<device>/<part>`.
        let (device, part) = match device.split_once('/') {
            Some((d, p)) => (d.to_owned(), Some(p.to_owned())),
            None => (
                device,
                string_param(p, "part")
                    .ok()
                    .flatten()
                    .map(|s| s.trim().to_owned()),
            ),
        };
        // A device by its ID or by a reference (`tag:-102S1`), as a script names it.
        let resolved = device.parse::<Uuid>().ok().or_else(|| {
            self.project
                .as_ref()
                .and_then(|p| p.session.resolve_reference(&device).ok())
        });
        let Some(device) = resolved else {
            return failed(key!("error-bad-param"), &["device"]);
        };
        let part = match part.map(|p| p.parse::<Uuid>()) {
            Some(Ok(p)) => Some(p),
            Some(Err(_)) => return failed(key!("error-bad-param"), &["part"]),
            None => None,
        };
        self.place_layout_device(device, part)
    }

    /// Arms the layout place tool with `device` (see [`ShellModel::act_place_layout`]).
    pub(super) fn place_layout_device(&mut self, device: Uuid, part: Option<Uuid>) -> Outcome {
        let Some(panel) = self.panel_tables() else {
            let text = strings::text(key!("reason-no-panel-page")).to_owned();
            return self.refuse_with(text, key!("reason-no-panel-page"));
        };
        if panel.unplaced_device(device).is_none() {
            let text = strings::text(key!("reason-no-unplaced-device")).to_owned();
            return self.refuse_with(text, key!("reason-no-unplaced-device"));
        }
        self.activate_tool(Box::new(LayoutPlaceTool::new(Some(device), part)));
        // The ghost floats at once: under the pointer when the canvas knows where it is, else at the grid cursor, else
        // at the start of the first rail it fits (on the plate's centre for a device that no rail takes); the canvas
        // takes the focus, so the arrows step it and Enter drops it (doc 16 §4.8's keyboard twin).
        if let Some(g) = self.area.as_ref().map(|a| a.focused) {
            self.focus = Some(super::Focus::Canvas(g));
        }
        let canvas = self.canvas();
        if let Some(at) = canvas.and_then(crate::canvas::CanvasModel::pointer) {
            let _ = self.send_tool(&CanvasEvent::PointerMove {
                at,
                mods: crate::tools::Mods::default(),
            });
        } else if let Some(at) = canvas
            .and_then(crate::canvas::CanvasModel::cursor)
            .or_else(|| self.layout_start(&panel, device))
        {
            let _ = self.send_tool(&CanvasEvent::CursorMove { at });
        }
        Outcome::Done
    }

    /// Where a device armed from the keyboard floats first, as a sheet point: the clip point at the start of the first
    /// rail (in carrier order) that the device fits, else the centre of the plate's usable area.
    fn layout_start(&self, panel: &PanelTables, device: Uuid) -> Option<(i64, i64)> {
        let u = panel.unplaced_device(device)?;
        let rail = panel.carriers.iter().find(|c| {
            c.axis.is_some()
                && c.profile
                    .as_ref()
                    .is_some_and(|p| fits_rail(u.mount_kind.as_ref(), p) == Fit::Fits)
        });
        let p = match rail.and_then(|c| c.axis) {
            Some(axis) => axis.point_at(u.extent.clip),
            None => {
                let (us, vs): (Vec<i64>, Vec<i64>) = panel.usable.iter().copied().unzip();
                let mid = |v: &[i64]| {
                    (i128::from(v.iter().copied().min().unwrap_or(0))
                        + i128::from(v.iter().copied().max().unwrap_or(0)))
                        / 2
                };
                (mid(&us), mid(&vs))
            }
        };
        Some(self.canvas()?.page().snapper.page_to_sheet(panel.to_page(p)))
    }

    /// The arrows while a layout ghost floats: the gesture's keyboard step (`LayoutGesture::step`), sent to the tool as
    /// a cursor move; nothing further that way is no error. Along a rail the ghost stops at a neighbour and at the rail
    /// end in words, as a selected placement does ("Blocked by -102F2", "At the rail end"); the ghost stays.
    fn step_layout_ghost(&mut self, dx: i64, dy: i64) -> Outcome {
        let Some(panel) = self.panel_tables() else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        let grid = self.tools.layout_options.grid();
        let (p, exact) = match self
            .tools
            .tool
            .layout_gesture()
            .and_then(|g| g.step(&panel, (dx, dy), grid))
        {
            None => return Outcome::Done,
            Some(Step::To(p)) => (p, false),
            Some(Step::Exact(p)) => (p, true),
            Some(Step::Blocked(by)) => {
                let text = blocked_text(&panel, by);
                return self.refuse_with(text, key!("reason-layout-blocked"));
            }
        };
        let Some(at) = self
            .canvas()
            .map(|c| c.page().snapper.page_to_sheet(panel.to_page(p)))
        else {
            return Outcome::Done;
        };
        if exact {
            // The tool resolves the step without snapping, as a pointer move with Shift held.
            let mods = crate::tools::Mods {
                shift: true,
                ..crate::tools::Mods::default()
            };
            return self.send_tool(&CanvasEvent::PointerMove { at, mods });
        }
        self.send_tool(&CanvasEvent::CursorMove { at })
    }

    /// `layout.next` (Tab on a panel page) and `layout.previous` (Shift+Tab): the next or previous placement
    /// of the focused rail (the selected placement's rail, else Rail 1) in carrier order (by `s0`), wrapping.
    pub(crate) fn act_layout_step(&mut self, step: i64) -> Outcome {
        let Some(panel) = self.panel_tables() else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        let current = self.selected_placement().map(|(_, r)| r);
        let rail = current
            .as_ref()
            .and_then(|r| r.carrier)
            .or_else(|| panel.carriers.iter().find(|c| c.axis.is_some()).map(|c| c.id));
        let Some(rail) = rail else {
            return Outcome::refused(key!("reason-no-placement"));
        };
        let mut on: Vec<&PanelPlacement> = panel
            .placements
            .iter()
            .filter(|p| p.carrier == Some(rail))
            .collect();
        on.sort_by_key(|p| (i128::from(p.pose.0) - p.clip, p.id));
        if on.is_empty() {
            return Outcome::refused(key!("reason-no-placement"));
        }
        let n = on.len();
        let at = current.and_then(|c| on.iter().position(|p| p.id == c.id));
        let next = match at {
            Some(i) if step > 0 => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None if step > 0 => 0,
            None => n - 1,
        };
        let e = EntityRef(on[next].id);
        if self.tools.selection.apply(&SelectionChange::Replace(vec![e])) {
            self.announce_selection();
        }
        Outcome::Done
    }

    /// `layout.nudge {dx, dy}` (the arrow keys with a placement selected): one grid step in the sticky mode. On a rail
    /// the arrow along the rail moves it; free stops where it would touch a neighbour at its minimum gap ("Blocked by
    /// -102F2") or the rail end, push pushes. On the plate it moves by the step in the arrow's direction.
    pub(crate) fn act_layout_nudge(&mut self, p: &Map<String, Value>) -> Outcome {
        let dir = |name: &str| p.get(name).and_then(Value::as_i64).unwrap_or(0).signum();
        let (dx, dy) = (dir("dx"), dir("dy"));
        if self.tools.tool.layout_gesture().is_some() {
            return self.step_layout_ghost(dx, dy);
        }
        let Some((panel, row)) = self.selected_placement() else {
            return Outcome::refused(key!("reason-no-placement"));
        };
        let grid = self.tools.layout_options.grid();
        let mode = self.tools.layout_options.mode;
        let q = quarter_quat(row.pose.2);
        let cmd = match row.carrier.and_then(|c| panel.carrier(c)) {
            Some(rail) => {
                let (Some(axis), Some(length)) = (rail.axis, rail.length) else {
                    return Outcome::refused(key!("reason-no-placement"));
                };
                // The page's y grows downwards, the surface's v upwards.
                let sign = i128::from(dx) * axis.dir.0 + i128::from(-dy) * axis.dir.1;
                if sign == 0 {
                    return Outcome::Done;
                }
                let Some(own) = rail
                    .items
                    .iter()
                    .find(|i| i.placement.is_some_and(|x| x.as_uuid() == row.id))
                else {
                    return Outcome::refused(key!("reason-no-placement"));
                };
                let s0 = own.s0;
                let target = s0 + sign * grid;
                let s_left = if mode == SnapMode::Push && sign > 0 {
                    target
                } else {
                    match nudge_stop(&rail.items, own, length, target, sign) {
                        Stop::Free(s) => s,
                        Stop::Blocked(by) => {
                            let text = blocked_text(&panel, by);
                            return self.refuse_with(text, key!("reason-layout-blocked"));
                        }
                    }
                };
                let mode = if mode == SnapMode::Push && sign > 0 {
                    SnapMode::Push
                } else {
                    SnapMode::Free
                };
                json!({
                    "placement": row.id.to_string(),
                    "pose": {"t": [clamp64(s_left + row.clip), 0, 0], "q": q},
                    "mode": mode_word(mode),
                })
            }
            None => json!({
                "placement": row.id.to_string(),
                "pose": {
                    "t": [
                        clamp64(i128::from(row.pose.0) + i128::from(dx) * grid),
                        clamp64(i128::from(row.pose.1) - i128::from(dy) * grid),
                        0
                    ],
                    "q": q,
                },
            }),
        };
        let said = strings::format(key!("toast-layout-moved"), &[&row.tag]);
        self.commit_layout_said("layout.move", cmd, Some(said))
    }

    /// `layout.rotate` (R) and `layout.flip` (X) on a panel page: to a floating ghost, its turn; to the selected
    /// placement, R turns it 90° on the plate and X turns it 180° on a rail (`layout.move`); R on a rail and X on the
    /// plate are refused in words with the keys of the keymap.
    pub(crate) fn act_layout_turn(&mut self, key: ToolKey) -> Outcome {
        if matches!(self.tools.tool.ghost(), Some(crate::tools::Ghost::Layout { .. })) {
            return self.send_tool(&CanvasEvent::Key(key));
        }
        let Some((panel, row)) = self.selected_placement() else {
            return Outcome::refused(key!("reason-no-placement"));
        };
        let on_rail = row.carrier.and_then(|c| panel.carrier(c)).is_some();
        let cmd = match (key, on_rail) {
            (ToolKey::RotateCw, false) => json!({
                "placement": row.id.to_string(),
                "pose": {"t": [row.pose.0, row.pose.1, 0], "q": quarter_quat((row.pose.2 + 1) % 4)},
            }),
            (ToolKey::MirrorX, true) => json!({
                "placement": row.id.to_string(),
                "pose": {"t": [row.pose.0, 0, 0], "q": quarter_quat(if row.pose.2 == 2 { 0 } else { 2 })},
                "mode": "free",
            }),
            _ => {
                let text = self.layout_turn_refusal();
                return self.refuse_with(text, key!("reason-layout-turn"));
            }
        };
        let said = strings::format(key!("toast-layout-turned"), &[&row.tag]);
        self.commit_layout_said("layout.move", cmd, Some(said))
    }

    /// The words of a refused turn (R on a rail, X on the plate) with the keys of the keymap: the text of
    /// `reason-layout-turn` wherever it shows, from `layout.rotate` and `layout.flip` here and from the layout place
    /// and Select tools' turn keys during a drag ([`ShellModel::refusal_text`]).
    pub(super) fn layout_turn_refusal(&self) -> String {
        strings::format(
            key!("layout-wrong-turn"),
            &[
                &crate::command_bar::key_name(self, "tool.rotate_cw"),
                &crate::command_bar::key_name(self, "tool.mirror_x"),
            ],
        )
    }

    /// `layout.lock_selection` and `layout.unlock_selection` (Lock and Unlock in the canvas context menu over a
    /// placement): `layout.lock {placement, locked}` of the selected placement.
    pub(crate) fn act_layout_lock(&mut self, locked: bool) -> Outcome {
        let Some((_, row)) = self.selected_placement() else {
            return Outcome::refused(key!("reason-no-placement"));
        };
        self.commit_layout(
            "layout.lock",
            json!({ "placement": row.id.to_string(), "locked": locked }),
        )
    }

    /// The one selected symbol and its texts where the page prints them; `None` unless exactly one representation is
    /// selected on a drawn page.
    pub fn selected_rep_texts(&self) -> Option<(Uuid, Vec<ew_api::output::SlotTextBox>)> {
        let canvas = self.canvas()?;
        let page = canvas.page();
        if page.panel.is_some() {
            return None;
        }
        let reps = crate::tools::transform::selected_reps(&self.tools.selection, page);
        let [rep] = reps.as_slice() else { return None };
        Some((
            rep.0,
            page.texts.iter().filter(|t| t.rep == rep.0).copied().collect(),
        ))
    }

    /// The selected symbols with a text moved or turned by an override.
    pub fn selected_reps_with_moved_texts(&self) -> Vec<Uuid> {
        let Some(canvas) = self.canvas() else {
            return Vec::new();
        };
        let page = canvas.page();
        let mut out: Vec<Uuid> = crate::tools::transform::selected_reps(&self.tools.selection, page)
            .into_iter()
            .map(|e| e.0)
            .filter(|r| page.texts.iter().any(|t| t.rep == *r && t.moved))
            .collect();
        out.dedup();
        out
    }

    /// The slot of the selected symbol's text under the pointer, for the canvas context menu.
    pub fn text_under_pointer(&self) -> Option<u16> {
        let canvas = self.canvas()?;
        let at = canvas.pointer()?;
        let p = canvas.page().snapper.sheet_to_page(at);
        let (_, texts) = self.selected_rep_texts()?;
        texts
            .iter()
            .rev()
            .find(|t| {
                (t.rect.min().x.0..=t.rect.max().x.0).contains(&p.0)
                    && (t.rect.min().y.0..=t.rect.max().y.0).contains(&p.1)
            })
            .map(|t| t.slot)
    }

    /// `rep.hide_text {slot}` (Hide text in the canvas context menu over a text of the one selected symbol):
    /// `rep.text_set {rep, slot, visible: false}`.
    pub(crate) fn act_rep_hide_text(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some((rep, _)) = self.selected_rep_texts() else {
            return Outcome::refused(key!("reason-text-one-symbol"));
        };
        let Some(slot) = p.get("slot").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["slot"]);
        };
        self.commit_from_host(&AnyCommand {
            name: "rep.text_set".into(),
            params: json!({"rep": rep.to_string(), "slot": slot, "visible": false}),
        })
    }

    /// `rep.reset_texts` (Put the text back in the canvas context menu over a text or a symbol): `rep.texts_reset` of the
    /// selected symbols whose texts were moved or turned.
    pub(crate) fn act_rep_reset_texts(&mut self) -> Outcome {
        let reps = self.selected_reps_with_moved_texts();
        if reps.is_empty() {
            return Outcome::refused(key!("reason-texts-not-moved"));
        }
        let reps: Vec<String> = reps.iter().map(ToString::to_string).collect();
        self.commit_from_host(&AnyCommand {
            name: "rep.texts_reset".into(),
            params: json!({ "reps": reps }),
        })
    }

    /// The selected symbols of a drawn page, in selection order; none on a panel layout.
    pub fn selected_rep_rows(&self) -> Vec<&ew_api::output::TableRep> {
        let Some(canvas) = self.canvas() else {
            return Vec::new();
        };
        let page = canvas.page();
        if page.panel.is_some() {
            return Vec::new();
        }
        crate::tools::transform::selected_reps(&self.tools.selection, page)
            .into_iter()
            .filter_map(|e| page.tables.reps.iter().find(|r| r.id == e.0))
            .collect()
    }

    /// Whether every selected symbol is kept in place (and one is selected): Keep in place shows its check.
    pub fn selection_kept_in_place(&self) -> bool {
        let reps = self.selected_rep_rows();
        !reps.is_empty() && reps.iter().all(|r| r.locked)
    }

    /// `rep.keep_in_place` (Keep in place in the canvas context menu over a symbol, doc 06a §2.4): `fields.set` of
    /// `locked` on the selected symbols, one undo step: kept when one of them is not yet, else let go. A toast with
    /// Undo names them by their tags.
    pub(crate) fn act_rep_keep_in_place(&mut self) -> Outcome {
        let reps = self.selected_rep_rows();
        if reps.is_empty() {
            return Outcome::refused(key!("reason-keep-in-place-no-symbol"));
        }
        let keep = !reps.iter().all(|r| r.locked);
        let targets: Vec<String> = reps.iter().map(|r| r.id.to_string()).collect();
        let mut names: Vec<String> = Vec::new();
        for r in &reps {
            let name = r
                .device
                .and_then(|d| self.project.as_ref()?.session.device_tag(d))
                .or_else(|| r.ip.as_ref().map(|ip| ip.name.clone()))
                .unwrap_or_else(|| strings::text(key!("keep-in-place-untagged")).to_owned());
            if !names.contains(&name) {
                names.push(name);
            }
        }
        let out = self.commit_from_host(&AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "representation", "targets": targets, "path": "locked", "value": keep}),
        });
        if out == Outcome::Done {
            let (count, names) = (names.len(), names.join(", "));
            let text = if keep {
                strings::format_counted(crate::count_key!("toast-kept-in-place"), count, &[&names])
            } else {
                strings::format(key!("toast-let-go"), &[&names])
            };
            self.notify(text, true);
        }
        out
    }

    /// `layout.reset_label` (Reset label position in the canvas context menu over a placement): `layout.label_move
    /// {placement}` of the selected placement, which puts its label back where the label rules place it.
    pub(crate) fn act_layout_reset_label(&mut self) -> Outcome {
        let Some((_, row)) = self.selected_placement() else {
            return Outcome::refused(key!("reason-no-placement"));
        };
        self.commit_layout("layout.label_move", json!({ "placement": row.id.to_string() }))
    }

    /// `layout.labels {show}` (Labels: Tags, Item numbers or Both in a panel page's menu): the page's Labels row set
    /// through its own edit (`Session::layout_edit`), one undo step.
    pub(crate) fn act_layout_labels(&mut self, p: &Map<String, Value>) -> Outcome {
        let row_word = match p.get("show").and_then(Value::as_str) {
            Some("tags") => "Tags",
            Some("item_numbers") => "Item numbers",
            Some("both") => "Both",
            Some("tags_or_numbers") => "Tags or numbers",
            Some(_) => return failed(key!("error-bad-param"), &["show"]),
            None => return failed(key!("error-missing-param"), &["show"]),
        };
        let (Some(page), Some(project)) = (self.focused_page_id(), &self.project) else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        let edit = ew_api::output::LayoutEdit {
            kind: "page".to_owned(),
            target: page,
            others: Vec::new(),
            row: "derived.labels".to_owned(),
            text: row_word.to_owned(),
        };
        match project.session.layout_edit(&edit) {
            Ok(cmd) => self.commit_from_host(&cmd),
            Err(e) => Outcome::Failed {
                message: e.to_string(),
            },
        }
    }

    /// `layout.reset_labels` (Reset label positions in the canvas context menu over an empty spot of a panel page):
    /// `layout.labels_reset` of every placement of the page whose label was moved, one undo step.
    pub(crate) fn act_layout_reset_labels(&mut self) -> Outcome {
        let moved = self.moved_labels();
        if moved.is_empty() {
            return Outcome::refused(key!("reason-labels-not-moved"));
        }
        let ids: Vec<String> = moved.iter().map(ToString::to_string).collect();
        self.commit_layout("layout.labels_reset", json!({ "placements": ids }))
    }

    /// `layout.fit_sheet` (Fit the layout to the sheet, in a panel page's menu and Page ▸ Order): the focused page's
    /// view at the fitting scale, centred on the sheet (`Session::sheet_fit`), one undo step; refused in words where
    /// the layout takes the whole sheet already.
    pub(crate) fn act_layout_fit_sheet(&mut self) -> Outcome {
        let (Some(page), Some(project)) = (self.focused_page_id(), &self.project) else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        match project.session.sheet_fit(page) {
            Some(cmd) => self.commit_from_host(&cmd),
            None => Outcome::refused(key!("reason-layout-fits-sheet")),
        }
    }

    /// `page.add_legend` (Page ▸ Add legend): the legend of the focused panel page's layout. Where it prints every row
    /// and every text whole beside the layout (`Session::legend_offer`), an embedded legend block there
    /// (`report_block.create` of kind `enclosure_legend`, the no-grid frame's form as Report… takes it); else its legend
    /// page, and a toast that says why (doc 12 §5.4). One undo step.
    pub(crate) fn act_page_add_legend(&mut self) -> Outcome {
        let Some(page) = self.focused_page_id() else {
            return Outcome::refused(key!("reason-legend-no-layout"));
        };
        let r = match self.legend_offer() {
            ew_api::output::LegendOffer::At(r) => r,
            ew_api::output::LegendOffer::NoRoom => {
                let out = self.add_legend_page(page);
                if out == Outcome::Done {
                    let folio = self.legend_page_folio(page);
                    self.notify(strings::format(key!("toast-legend-own-page"), &[&folio]), false);
                }
                return out;
            }
            ew_api::output::LegendOffer::Exists(_) => return Outcome::refused(key!("reason-legend-exists")),
            ew_api::output::LegendOffer::Paged => return Outcome::refused(key!("reason-legend-paged")),
            ew_api::output::LegendOffer::NoLayout => {
                return Outcome::refused(key!("reason-legend-no-layout"));
            }
        };
        let Some(form) = crate::wizard::shipped_form_id(crate::wizard_data::NOGRID_FRAME) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let held = self.project_session().is_some_and(|s| {
            form.as_str().is_some_and(|id| {
                s.query()
                    .forms()
                    .iter()
                    .any(|(f, _)| f.as_uuid().to_string() == id)
            })
        });
        let mut params = json!({
            "source": { "kind": "enclosure_legend" },
            "form": form,
            "embedded": {
                "page": page.to_string(),
                "rect": [[r.min().x.0, r.min().y.0], [r.max().x.0, r.max().y.0]],
            },
            "columns": legend_columns(),
        });
        if !held && let Some(session) = self.project_session() {
            let keys = [crate::wizard_data::NOGRID_FRAME, crate::wizard_data::TITLE_BLOCK];
            params["form_copies"] = Value::Array(crate::wizard::shipped_forms_lacking(session, &keys));
        }
        let out = self.commit_from_host(&AnyCommand {
            name: "report_block.create".into(),
            params,
        });
        // A legend beside devices that show their tags offers their numbers.
        if out == Outcome::Done && !self.panel_view_shows_numbers() {
            let numbers = self.session_offer(
                "item_numbers",
                key!("layout-show-item-numbers"),
                vec![("layout.labels", serde_json::json!({ "show": "item_numbers" }))],
            );
            self.notify_offer(strings::text(key!("toast-legend-tags")).to_owned(), numbers);
        }
        out
    }

    /// The legend page of the panel page `page`'s layout (doc 12 §5.4): a page block of kind `enclosure_legend` that
    /// names `page` (`layout_page`; its folio and section follow the page), on the set's own frame as Report… makes a
    /// list, with the legend's five columns and the title that names its layout. One undo step.
    fn add_legend_page(&mut self, page: Uuid) -> Outcome {
        self.add_legend_list(page, legend_columns())
    }

    /// [`ShellModel::add_legend_page`] with the printed `columns`.
    fn add_legend_list(&mut self, page: Uuid, columns: Value) -> Outcome {
        let Some((form, copies)) = self.list_form() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let mut params = json!({
            "source": { "kind": "enclosure_legend" },
            "form": form,
            "layout_page": page.to_string(),
            "columns": columns,
            "title": { "en-GB": LEGEND_TITLE.0, "nl-NL": LEGEND_TITLE.1 },
        });
        if let Some(copies) = copies {
            params["form_copies"] = copies;
        }
        self.commit_from_host(&AnyCommand {
            name: "report_block.create".into(),
            params,
        })
    }

    /// The printed folio of the first legend page of the panel page `page`'s layout; empty without one.
    fn legend_page_folio(&self, page: Uuid) -> String {
        self.project_session()
            .and_then(|s| s.legend_page_folio(page))
            .unwrap_or_default()
    }

    /// The legend page a new layout view gets (doc 12 §5.4), in the undo step of the act that made the view, so that
    /// one undo takes back both.
    pub(super) fn add_view_legend(&mut self) {
        let Some(page) = self.focused_page_id() else {
            return;
        };
        if matches!(
            self.legend_offer(),
            ew_api::output::LegendOffer::NoLayout
                | ew_api::output::LegendOffer::Exists(_)
                | ew_api::output::LegendOffer::Paged
        ) {
            return;
        }
        let rev = self.revision();
        if self.add_legend_page(page) == Outcome::Done
            && self.revision() != rev
            && let Some(p) = &mut self.project
        {
            p.session.fold_last_step();
        }
    }

    /// `report_block.legend_to_page` (Move the legend to its own page, in the legend's menu and Page ▸ Order): the
    /// embedded legend of the focused panel page becomes its legend page (doc 12 §5.4): the page block is made, with
    /// the box's own printed columns (else the legend's five), and the box removed, in one undo step.
    pub(crate) fn act_report_block_legend_to_page(&mut self) -> Outcome {
        let (Some(page), ew_api::output::LegendOffer::Exists(block)) =
            (self.focused_page_id(), self.legend_offer())
        else {
            return Outcome::refused(key!("reason-legend-no-box"));
        };
        let own: Vec<Value> = self
            .project_session()
            .and_then(|s| {
                let q = s.query();
                let (_, b) = q.report_blocks().iter().find(|(id, _)| id.as_uuid() == block)?;
                Some(
                    b.columns
                        .iter()
                        .filter_map(|c| serde_json::to_value(c).ok())
                        .collect(),
                )
            })
            .unwrap_or_default();
        let columns = if own.is_empty() {
            legend_columns()
        } else {
            Value::Array(own)
        };
        let rev = self.revision();
        let out = self.add_legend_list(page, columns);
        if out != Outcome::Done {
            return out;
        }
        let gone = self.commit_from_host(&AnyCommand {
            name: "edit.delete".into(),
            params: json!({ "targets": [{"kind": "report_block", "id": block.to_string()}] }),
        });
        if gone != Outcome::Done {
            let _ = self.act_undo();
            return gone;
        }
        if self.revision() != rev
            && let Some(p) = &mut self.project
        {
            p.session.fold_last_step();
        }
        self.tools.selection.clear();
        let folio = self.legend_page_folio(page);
        self.notify(strings::format(key!("toast-legend-moved"), &[&folio]), true);
        Outcome::Done
    }

    /// `layout.options {mode?, grid?}` (the tool options card's segmented controls): the sticky mode (`free`, `push`,
    /// `swap`) and the grid step (`5`, `1` mm).
    pub(crate) fn act_layout_options(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut options = self.tools.layout_options;
        match p.get("mode").and_then(Value::as_str) {
            None => {}
            Some("free") => options.mode = SnapMode::Free,
            Some("push") => options.mode = SnapMode::Push,
            Some("swap") => options.mode = SnapMode::Swap,
            Some(_) => return failed(key!("error-bad-param"), &["mode"]),
        }
        match p.get("grid").and_then(Value::as_u64) {
            None => {}
            Some(g @ (1 | 5)) => options.grid_mm = g as u8,
            Some(_) => return failed(key!("error-bad-param"), &["grid"]),
        }
        self.tools.layout_options = options;
        Outcome::Done
    }

    /// Commits a `layout.*` command built by the host ([`ShellModel::commit_layout_said`] without words of its own).
    fn commit_layout(&mut self, name: &str, params: Value) -> Outcome {
        self.commit_layout_said(name, params, None)
    }

    /// Commits a `layout.*` command built by the host. A commit that leaves ways of the cabinet's wires no longer
    /// holding says so, after `said` (what the step did: "Turned -X1031."), with Undo and Route (doc 12-3d §12.5:
    /// "Turned -X1031. The ways of 5 wires no longer hold.").
    fn commit_layout_said(&mut self, name: &str, params: Value, said: Option<String>) -> Outcome {
        let cmd = AnyCommand {
            name: name.into(),
            params,
        };
        let stale = |s: &Self| {
            s.current_folio()
                .and_then(|f| s.project_session().and_then(|p| p.cabinet_due(f)))
                .map_or(0, |(_, stale)| stale)
        };
        let before = stale(self);
        let out = self.commit_from_host(&cmd);
        self.tools.layout_announce |= out == Outcome::Done;
        let after = stale(self);
        if out == Outcome::Done && after > before {
            let text = strings::format_count(count_key!("toast-ways-stale"), after);
            let route = self.session_offer(
                "route",
                key!("action-layout-route"),
                vec![("layout.route", Value::Null)],
            );
            let undo = self.undo_offer();
            let sentences: Vec<String> = said.into_iter().chain([text]).collect();
            self.notes
                .push(crate::notifications::Note::of("edit", sentences).offering([undo, route]));
        }
        out
    }

    /// The after-commit duty of a `layout.*` commit (at the end of its dispatch, the page read again): the live region
    /// says where the selected placement now is ("-102F1, rail 1 at 163 mm, no findings").
    pub(super) fn after_layout_commit(&mut self) {
        // The placement a layout drop made is the selection, replacing it.
        if let Some(device) = self.tools.placed_device.take()
            && let Some(id) = self
                .panel_tables()
                .and_then(|p| p.placements.iter().find(|r| r.device == device).map(|r| r.id))
            && self
                .tools
                .selection
                .apply(&SelectionChange::Replace(vec![EntityRef(id)]))
        {
            self.announce_selection();
        }
        if std::mem::take(&mut self.tools.layout_announce)
            && let Some(text) = self.layout_live_text()
        {
            self.tools.announcement = Some(text);
        }
    }
}

/// The title of a legend page, in the languages the program ships (doc 12 §5.4): `{layout}` prints the layout page's
/// folio, so a renumbered layout renames its legend.
const LEGEND_TITLE: (&str, &str) = ("Legend of layout {layout}", "Legenda bij layout {layout}");

/// The printed columns of a legend a person makes ([`ew_api::output::LEGEND_COLUMNS`]) as `report_block.create`
/// takes them.
fn legend_columns() -> Value {
    ew_api::output::LEGEND_COLUMNS
        .iter()
        .map(|(key, weight)| json!({"key": key, "weight": weight}))
        .collect()
}

/// The words of a stopped nudge: "Blocked by -102F2" for a neighbour's placement, "At the rail end" for the end.
fn blocked_text(panel: &PanelTables, by: Option<Uuid>) -> String {
    match by.and_then(|p| panel.placement(p)) {
        Some(n) => strings::format(key!("layout-blocked"), &[&n.tag]),
        None => strings::text(key!("layout-rail-end")).to_owned(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Setting the panel up from the window.

/// A command's refusal as a sentence under a set-up dialog's fields: without the command's name before it
/// (`layout.enclosure.create_from_part: location J-E-2416 already has …` reads "Location J-E-2416 already has …."),
/// capitalised and with its full stop.
fn in_words(message: &str) -> String {
    let rest = match message.split_once(": ") {
        Some((name, rest)) if name.starts_with("layout.") && !name.contains(' ') => rest,
        _ => message,
    };
    let mut chars = rest.chars();
    let mut out: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    };
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// The orders of the Auto-fill rail dialog, `(value, words)` (a choice list built at run time).
fn fill_orders() -> Vec<(String, String)> {
    vec![
        (
            "schematic".to_owned(),
            strings::text(key!("auto-fill-order-schematic")).to_owned(),
        ),
        (
            "dt".to_owned(),
            strings::text(key!("auto-fill-order-dt")).to_owned(),
        ),
    ]
}

/// The value of the Create enclosure dialog's first location choice, "New location": a location node made with the
/// code of the field under it. A location's own value is its ID, so the word never names one.
pub(super) const NEW_LOCATION: &str = "new";

/// The Create enclosure form's field of the new location's code, under its location: there while the location is
/// "New location" (with `code` proposed when it comes), gone for an existing one.
pub(super) fn new_location_field(form: &mut super::FormDialog, code: &str) {
    let new = form
        .fields
        .iter()
        .any(|f| f.name == "location" && f.value == NEW_LOCATION);
    let at = form.fields.iter().position(|f| f.name == "new_location");
    match (new, at) {
        (true, None) => {
            let after = form
                .fields
                .iter()
                .position(|f| f.name == "location")
                .map_or(0, |i| i + 1);
            form.fields.insert(
                after,
                super::FormField {
                    label: None,
                    name: "new_location".into(),
                    label_key: key!("form-enclosure-new-location"),
                    choices: &super::choices::NONE,
                    options: Vec::new(),
                    value: code.to_owned(),
                },
            );
        }
        (false, Some(i)) => {
            form.fields.remove(i);
        }
        _ => {}
    }
}

/// The draw tool's carrier kind (`rail`, `duct`) of the tool name `name`.
fn draw_kind(name: &str) -> Option<&'static str> {
    match name {
        "rail_draw" => Some("rail"),
        "duct_draw" => Some("duct"),
        _ => None,
    }
}

impl ShellModel {
    /// The page the focused canvas shows, by ID; `None` without one.
    pub(super) fn focused_page_id(&self) -> Option<Uuid> {
        match self.current_page()? {
            crate::canvas::PageId(ew_dl::Destination::Page(id)) => Some(id),
            _ => None,
        }
    }

    /// The parts a draw tool of `kind` (`rail`, `duct`) offers: the project's copies of the kind, then the
    /// installed libraries' parts, embedded through `copies` when drawn.
    ///
    /// The offers are kept for (session, revision, kind): arming the tool or changing its options computes
    /// them afresh, a new revision too; the options card's every frame reuses them.
    pub fn draw_part_offers(&self, kind: &str) -> Vec<crate::tools::DrawPart> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let Some(kind) = ["rail", "duct"].into_iter().find(|k| *k == kind) else {
            return Vec::new();
        };
        let key = (
            self.project_epoch().unwrap_or(0),
            self.revision().unwrap_or(0),
            kind,
        );
        if let Some((k, offers)) = self.tools.draw_offers.borrow().as_ref()
            && *k == key
        {
            return offers.clone();
        }
        bump(&self.calls, |c| c.layout_part_offers += 1);
        let offers: Vec<crate::tools::DrawPart> = project
            .session
            .layout_part_offers(kind)
            .into_iter()
            .map(|o| crate::tools::DrawPart {
                part: o.part,
                number: o.number,
                profile: o.profile.and_then(|p| serde_json::to_value(p).ok()),
                size: o.size,
                copies: o.copies,
            })
            .collect();
        *self.tools.draw_offers.borrow_mut() = Some((key, offers.clone()));
        offers
    }

    /// Drops the draw tools' kept part offers: the next read computes them (a draw tool armed, its options changed).
    fn forget_draw_offers(&self) {
        *self.tools.draw_offers.borrow_mut() = None;
    }

    /// The part a draw tool of `kind` draws with: the tool option's when it is still offered, else the first offer.
    fn draw_part(&self, kind: &str) -> Option<crate::tools::DrawPart> {
        let offers = self.draw_part_offers(kind);
        let sticky = match kind {
            "rail" => self.tools.layout_options.rail_part,
            _ => self.tools.layout_options.duct_part,
        };
        sticky
            .and_then(|s| offers.iter().find(|o| o.part == s).cloned())
            .or_else(|| offers.into_iter().next())
    }

    /// The tool options card of Draw rail or Draw duct; `None` while another tool runs.
    pub fn draw_card(&self) -> Option<crate::tools::DrawCard> {
        let kind = draw_kind(self.tools.tool.name())?;
        let parts: Vec<String> = self
            .draw_part_offers(kind)
            .into_iter()
            .map(|o| o.number)
            .collect();
        let part = self.draw_part(kind);
        let comma = self.panel_tables().is_some_and(|p| p.decimal_comma);
        let size = (kind == "duct" && part.as_ref().and_then(|p| p.size).is_none()).then(|| {
            let (w, h) = self.tools.layout_options.duct_size;
            (
                ew_api::output::mm_words(w, comma),
                ew_api::output::mm_words(h, comma),
            )
        });
        Some(crate::tools::DrawCard {
            kind,
            parts,
            part: part.map(|p| p.number),
            size,
        })
    }

    /// `tool.rail_draw` (Draw ▸ Layout ▸ Rail): arms Draw rail with the tool option's rail part.
    pub(crate) fn act_tool_rail_draw(&mut self) -> Outcome {
        self.forget_draw_offers();
        let part = self.draw_part("rail");
        self.activate_tool(Box::new(crate::tools::RailDrawTool::new(part)));
        Outcome::Done
    }

    /// `tool.duct_draw` (Draw ▸ Layout ▸ Duct): arms Draw duct with the tool option's duct part and size.
    pub(crate) fn act_tool_duct_draw(&mut self) -> Outcome {
        self.forget_draw_offers();
        let part = self.draw_part("duct");
        let size = self.tools.layout_options.duct_size;
        self.activate_tool(Box::new(crate::tools::DuctDrawTool::new(part, size)));
        Outcome::Done
    }

    /// `layout.draw_options {part?, width?, height?}` (the draw tools' options card): the part by its number, a
    /// duct's width and height in millimetres; the choice stays for the next drawing, and the running tool takes it.
    pub(crate) fn act_layout_draw_options(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(kind) = draw_kind(self.tools.tool.name()) else {
            return Outcome::refused(key!("reason-no-draw-tool"));
        };
        self.forget_draw_offers();
        if let Some(number) = p.get("part").and_then(Value::as_str) {
            let Some(offer) = self
                .draw_part_offers(kind)
                .into_iter()
                .find(|o| o.number == number)
            else {
                return failed(key!("error-bad-param"), &["part"]);
            };
            match kind {
                "rail" => self.tools.layout_options.rail_part = Some(offer.part),
                _ => self.tools.layout_options.duct_part = Some(offer.part),
            }
        }
        let mut size = self.tools.layout_options.duct_size;
        for (name, first) in [("width", true), ("height", false)] {
            let Some(v) = p.get(name) else { continue };
            let text = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return failed(key!("error-bad-param"), &[name]),
            };
            match ew_api::output::parse_mm(&text).filter(|n| *n > 0 && *n <= 10_000_000_000) {
                Some(nm) if first => size.0 = nm,
                Some(nm) => size.1 = nm,
                None => return failed(key!("error-bad-param"), &[name]),
            }
        }
        self.tools.layout_options.duct_size = size;
        let part = self.draw_part(kind);
        if let Some(any) = self.tools.tool.as_any_mut() {
            if let Some(t) = any.downcast_mut::<crate::tools::RailDrawTool>() {
                t.set_part(part);
            } else if let Some(t) = any.downcast_mut::<crate::tools::DuctDrawTool>() {
                t.set_part(part);
                t.set_option_size(size);
            }
        }
        Outcome::Done
    }

    /// `layout.enclosure_new {location, part, new_location?}` (Project ▸ Enclosure…):
    /// `layout.enclosure.create_from_part` of the location (its code or ID) with the enclosure part (its number or ID:
    /// a project copy, or an installed library's part, embedded through `copies` with its plate part), the focused
    /// page's function as the enclosure device's. The location `new` (the form's first choice, "New location"), or
    /// `new_location` given alone, makes the location node of the code `new_location` first (`structure.create_node`,
    /// one undo step more; a code an existing location has takes that location), so a new project gets its panel
    /// layout without a detour through Structure node… (Enclosure… is not disabled in a
    /// project without a location). If the enclosure is refused after the node is made, the node is taken back. In the
    /// window without a location and a part the Create enclosure dialog opens (the focused page's location, else a new
    /// one, and the first part chosen); a refusal stays under its fields.
    pub(crate) fn act_layout_enclosure_new(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "layout.enclosure_new";
        let given = |name: &str| {
            p.get(name)
                .and_then(Value::as_str)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let new_code = given("new_location");
        // Change its cabinet… of the dialog on a location that has its enclosure: the part dialog of that enclosure's
        // device, whose choice of a cabinet changes the enclosure with its plate and its surfaces.
        if flag(p, "change_part") {
            let tag = given("location").and_then(|l| {
                (self.project.as_ref()?.session.location_enclosures().into_iter())
                    .find(|(id, ..)| id.to_string() == l)
                    .map(|(_, tag, ..)| tag)
            });
            let Some(tag) = tag else {
                return failed(key!("error-bad-param"), &["location"]);
            };
            self.dialog = None;
            let out = self.dispatch("select.set", &json!({ "refs": [format!("tag:{tag}")] }));
            if out != Outcome::Done {
                return out;
            }
            return self.dispatch("part.choose", &json!({}));
        }
        let (location, part) = (
            given("location").or_else(|| new_code.as_ref().map(|_| NEW_LOCATION.to_owned())),
            given("part"),
        );
        let (Some(location), Some(part)) = (location.clone(), part.clone()) else {
            let missing = if location.is_none() { "location" } else { "part" };
            return match self.mode {
                super::Mode::Headless => failed(key!("error-missing-param"), &[missing]),
                // Create is disabled while a field is empty; Enter in the form says why in the same words.
                super::Mode::Gui if self.in_form_of(ACTION) => {
                    let message = strings::text(if location.is_none() {
                        key!("reason-enclosure-location")
                    } else {
                        key!("reason-enclosure-part")
                    })
                    .to_owned();
                    self.form_failed(ACTION, message)
                }
                super::Mode::Gui => {
                    let form = self.enclosure_form(location, part);
                    self.open_dialog(super::Dialog::Form(form))
                }
            };
        };
        let page = self.focused_page_id();
        let project = or_return!(crate::shell::gated(&self.project));
        let locations = project.session.layout_locations();
        let named = |text: &str| {
            locations
                .iter()
                .find(|(id, code)| {
                    id.to_string() == text || code == text || code.trim_start_matches('+') == text
                })
                .cloned()
        };
        // An existing location, else the new one's code (its sign taken off, as Structure node… takes it).
        let target = match named(&location) {
            Some(found) => Ok(found),
            None if location == NEW_LOCATION => {
                let Some(code) = new_code else {
                    let message = strings::text(key!("reason-enclosure-new-location")).to_owned();
                    return self.form_failed(ACTION, message);
                };
                match super::creation::strip_sign("location", &code) {
                    Ok(code) if code.is_empty() => {
                        let message = strings::text(key!("reason-enclosure-new-location")).to_owned();
                        return self.form_failed(ACTION, message);
                    }
                    Ok(code) => named(&code).ok_or(code),
                    Err(message) => return self.form_failed(ACTION, message),
                }
            }
            None => {
                let message = strings::format(key!("error-no-location"), &[&location]);
                return self.form_failed(ACTION, message);
            }
        };
        // A location that has an enclosure: drawn on this panel layout page when it does not show it yet, or on a new
        // one (the enclosure made from another page could not be drawn on the layout page).
        let new_page = p.get("new_page").and_then(Value::as_bool).unwrap_or(false);
        let existing = target.as_ref().ok().and_then(|(id, code)| {
            project
                .session
                .location_enclosures()
                .into_iter()
                .find(|(l, ..)| l == id)
                .map(|(l, tag, surface, _)| (tag, code.clone(), surface, l))
        });
        if let Some((tag, code, surface, location)) = existing {
            // The dialog takes no part it would not use (doc 12 §3.1): the enclosure is drawn as it is, so another
            // part named for it is refused in words.
            if let Some((copy, number)) = self.enclosure_part(&location.to_string()) {
                let named = (project.session.layout_part_offers("enclosure").into_iter())
                    .find(|o| o.part.to_string() == part || o.number == part);
                if named.as_ref().is_none_or(|o| o.part != copy) {
                    // The part named by its number, whether the script named it by its number or by its copy.
                    let other = named.map_or_else(|| part.clone(), |o| o.number);
                    return self.form_failed(
                        ACTION,
                        strings::format(
                            key!("error-enclosure-other-part"),
                            &[&tag, &code, &number, &other],
                        ),
                    );
                }
            }
            let shown = self.panel_tables().map(|t| t.surface);
            if new_page || (self.on_panel_page() && shown != Some(surface)) {
                return self.draw_enclosure(ACTION, new_page, &tag, &code, surface);
            }
        }
        let project = or_return!(crate::shell::gated(&self.project));
        let offer = project
            .session
            .layout_part_offers("enclosure")
            .into_iter()
            .find(|o| o.part.to_string() == part || o.number == part);
        let Some(offer) = offer else {
            let message = strings::format(key!("error-no-enclosure-part"), &[&part]);
            return self.form_failed(ACTION, message);
        };
        let function = page.and_then(|pg| project.session.page_structure(pg).1);
        // The new location node first, its own undo step.
        let made = target.is_err();
        let (location_id, location_code) = match target {
            Ok(found) => found,
            Err(code) => {
                let out = self.host_create(
                    "structure.create_node",
                    json!({"aspect": "location", "code": code}),
                );
                if out != Outcome::Done {
                    return self.layout_form_done(ACTION, out);
                }
                let node = self.project.as_ref().and_then(|p| {
                    p.session.layout_locations().into_iter().find(|(id, c)| {
                        c.trim_start_matches('+') == code && !locations.iter().any(|(l, _)| l == id)
                    })
                });
                let Some(node) = node else {
                    return self.layout_form_done(ACTION, Outcome::refused(key!("reason-no-project")));
                };
                node
            }
        };
        let mut params = json!({
            "location": location_id.to_string(),
            "part": offer.part.to_string(),
        });
        if let Some(f) = function {
            params["function"] = Value::String(f.to_string());
        }
        if !offer.copies.is_empty() {
            params["copies"] = Value::Array(offer.copies);
        }
        let before = self
            .project
            .as_ref()
            .map(|p| p.session.surface_choices())
            .unwrap_or_default();
        let words = if offer.description.is_empty() {
            offer.number.clone()
        } else {
            offer.description.clone()
        };
        // Off a panel layout page, Create and draw makes the layout page after the page shown first, its own undo step.
        let paged = new_page && {
            let out = self.new_layout_page(&location_code);
            if out != Outcome::Done {
                if made {
                    let _ = self.act_undo();
                }
                return self.layout_form_done(ACTION, out);
            }
            true
        };
        let page = if paged { self.focused_page_id() } else { page };
        let out = self.host_create("layout.enclosure.create_from_part", params);
        // All or nothing: a refused enclosure takes back the layout page and the location node made for it.
        if out != Outcome::Done {
            for _ in 0..(usize::from(made) + usize::from(paged)) {
                let _ = self.act_undo();
            }
        }
        let out = self.layout_form_done(ACTION, out);
        if out == Outcome::Done {
            let shown = self.show_new_surface(page, &before);
            if shown {
                self.add_view_legend();
            }
            // One toast for the step; it replaces the toast of an earlier step. Its Undo takes back the
            // enclosure, so it is offered only when the plate shown on the page is not the newer step.
            let folio = self.current_folio().unwrap_or_default().to_owned();
            let text = match (shown, paged) {
                (true, true) => strings::format(
                    key!("toast-enclosure-new-page"),
                    &[&words, &location_code, &folio],
                ),
                (true, false) => {
                    strings::format(key!("toast-enclosure-new-shown"), &[&words, &location_code])
                }
                _ => strings::format(key!("toast-enclosure-new"), &[&words, &location_code]),
            };
            self.notify(text, !shown);
        }
        out
    }

    /// Whether the page shown is a panel layout page.
    fn on_panel_page(&self) -> bool {
        self.page_rows()
            .get(self.page_index())
            .is_some_and(|r| r.kind == ew_api::PageType::PanelLayout)
    }

    /// A panel layout page where a made page of its kind goes (New page…'s proposed folio, else the first free number),
    /// titled after the location `code` and shown: one undo step.
    fn new_layout_page(&mut self, code: &str) -> Outcome {
        let (proposal, _) = self.new_page_proposal(None, Some("panel_layout"));
        let folio = if proposal.is_empty() {
            let labels = self.page_labels();
            (1u64..)
                .map(|n| n.to_string())
                .find(|n| !labels.contains(n))
                .unwrap_or_default()
        } else {
            proposal
        };
        let mut p = Map::new();
        p.insert("folio".into(), Value::String(folio));
        p.insert("type".into(), Value::String("panel_layout".into()));
        p.insert(
            "title".into(),
            Value::String(strings::format(key!("enclosure-layout-title"), &[code])),
        );
        self.act_page_new(&p)
    }

    /// Draws the enclosure `tag` of the location `code` on the panel layout page shown, or on a new one after it
    /// (`new_page`): the page's Surface view shows its mounting plate `surface`, as the page's Surface row would, with
    /// the legend where the page numbers its items. One toast says where; the new page and the view are an undo step
    /// each, and the page is taken back when the view cannot be set.
    fn draw_enclosure(
        &mut self,
        action: &str,
        new_page: bool,
        tag: &str,
        code: &str,
        surface: Uuid,
    ) -> Outcome {
        if new_page {
            let out = self.new_layout_page(code);
            if out != Outcome::Done {
                return self.layout_form_done(action, out);
            }
        }
        if !self
            .focused_page_id()
            .is_some_and(|page| self.show_surface(page, surface))
        {
            if new_page {
                let _ = self.act_undo();
            }
            return self.layout_form_done(action, Outcome::refused(key!("reason-no-panel-page")));
        }
        self.add_view_legend();
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(
            strings::format(key!("toast-enclosure-drawn"), &[tag, code, &folio]),
            false,
        );
        self.layout_form_done(action, Outcome::Done)
    }

    /// Whether the focused panel page's surface view labels its placements with item numbers (alone or with tags).
    /// Read from the session, not the canvas: the canvas's tables follow at the next frame, after the step that set the
    /// view.
    fn panel_view_shows_numbers(&self) -> bool {
        match (self.current_folio(), &self.project) {
            (Some(folio), Some(p)) => p.session.panel_tables(folio).is_some_and(|t| t.numbered),
            _ => false,
        }
    }

    /// After Enclosure… on a panel layout page that shows no enclosure yet, the page shows the new enclosure's
    /// mounting plate, centred at its fitting scale, as the page's Surface row would (one more undo step): the
    /// person lands where the rails, ducts and devices are drawn. Another page, or one that shows a plate already,
    /// is left as it is. Whether the page now shows the plate.
    fn show_new_surface(&mut self, page: Option<Uuid>, before: &[(Uuid, ew_api::Text)]) -> bool {
        let Some(page) = page else { return false };
        if !self.on_panel_page() || self.panel_tables().is_some() {
            return false;
        }
        let Some(project) = &self.project else {
            return false;
        };
        let Some((surface, _)) = project
            .session
            .surface_choices()
            .into_iter()
            .find(|(id, _)| !before.iter().any(|(b, _)| b == id))
        else {
            return false;
        };
        self.show_surface(page, surface)
    }

    /// The panel layout page `page` shows the mounting surface `surface`, as its Surface row sets it: one undo step.
    /// Whether it does.
    pub(super) fn show_surface(&mut self, page: Uuid, surface: Uuid) -> bool {
        let Some(project) = &self.project else {
            return false;
        };
        let edit = ew_api::output::LayoutEdit {
            kind: "page".to_owned(),
            target: page,
            others: Vec::new(),
            row: "derived.surface".to_owned(),
            text: surface.to_string(),
        };
        match project.session.layout_edit(&edit) {
            Ok(cmd) => self.run_commit(&cmd) == Outcome::Done,
            Err(_) => false,
        }
    }

    /// The code Create enclosure proposes for a new location: `CAB1`, else the first `CAB<n>` no location of the
    /// project has (Structure node… proposes no code; a cabinet is what the location of a panel layout names).
    pub(super) fn proposed_location_code(&self) -> String {
        let locations = self
            .project
            .as_ref()
            .map(|p| p.session.layout_locations())
            .unwrap_or_default();
        (1..=locations.len() + 1)
            .map(|n| format!("CAB{n}"))
            .find(|c| {
                !locations
                    .iter()
                    .any(|(_, code)| code.trim_start_matches('+') == c)
            })
            .unwrap_or_default()
    }

    /// The card of giving an enclosure another cabinet (doc 12 §3.1): the sizes of its surfaces that change, what then no
    /// longer lies inside its surface, a plate kept where the new cabinet names none; Apply makes it one step. The part
    /// dialog gives way to it.
    pub(super) fn enclosure_part_card(&mut self, plan: ew_api::output::EnclosurePartPlan) -> Outcome {
        self.dialog = None;
        let mut lines: Vec<String> = plan.outside.iter().map(strings::render).collect();
        if plan.plate_kept {
            lines.push(strings::format(
                key!("enclosure-part-plate-kept"),
                &[&plan.number],
            ));
        }
        let card = crate::properties::PartCard {
            title: strings::format(key!("enclosure-part-card-title"), &[&plan.tag, &plan.number]),
            fields: (plan.sizes.iter())
                .map(|(w, old, new)| (strings::render(w), old.clone(), new.clone()))
                .collect(),
            devices: vec![plan.tag.clone()],
            removed: Vec::new(),
            added: Vec::new(),
            changed: Vec::new(),
            lost: Vec::new(),
            boxes: lines,
            kept: Vec::new(),
            fork: false,
        };
        self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            key!("reason-enclosure-set-part"),
            crate::properties::CardOrigin::Part(Box::new(card)),
        )
    }

    /// Whether location `location` (its node's ID) has its enclosure: Create enclosure then offers Change its cabinet….
    pub fn location_has_enclosure(&self, location: &str) -> bool {
        (self.project.as_ref()).is_some_and(|p| {
            p.session
                .location_enclosures()
                .iter()
                .any(|(id, ..)| id.to_string() == location)
        })
    }

    /// The part of the enclosure that location `location` (its node's ID) has, by its copy and its number; `None` for a
    /// location without one.
    fn enclosure_part(&self, location: &str) -> Option<(Uuid, String)> {
        let project = self.project.as_ref()?;
        let (.., part) =
            (project.session.location_enclosures().into_iter()).find(|(l, ..)| l.to_string() == location)?;
        let copy = part?;
        let offer = (project.session.layout_part_offers("enclosure").into_iter()).find(|o| o.part == copy)?;
        Some((copy, offer.number))
    }

    /// The Create enclosure dialog's part choices for location `location`: the part its enclosure has, alone and
    /// chosen, where it has one (the dialog draws it as it is); else every enclosure part, the project's copies first,
    /// with no choice made.
    pub(super) fn enclosure_part_choices(&self, location: &str) -> (Vec<(String, String)>, Option<String>) {
        let parts =
            (self.project.as_ref()).map_or_else(Vec::new, |p| p.session.layout_part_offers("enclosure"));
        let held = self.enclosure_part(location).map(|(copy, _)| copy);
        let options = (parts.iter())
            .filter(|o| held.is_none_or(|c| o.part == c))
            .map(|o| {
                let words = if o.description.is_empty() {
                    o.number.clone()
                } else {
                    format!("{} \u{b7} {}", o.number, o.description)
                };
                (o.part.to_string(), words)
            })
            .collect();
        (options, held.map(|c| c.to_string()))
    }

    /// The Create enclosure dialog: the location (the given one, else the focused page's, else the first, else a new
    /// one: the list's first choice "New location", with its code proposed in the field under it) and the enclosure
    /// part (the part of the location's enclosure, where it has one; else the given one, else the first: the project's
    /// copies, then the libraries' parts).
    fn enclosure_form(&self, location: Option<String>, part: Option<String>) -> super::FormDialog {
        let page = self.focused_page_id();
        let (locations, page_location) = match &self.project {
            Some(project) => (
                project.session.layout_locations(),
                page.and_then(|pg| project.session.page_structure(pg).0),
            ),
            None => (Vec::new(), None),
        };
        let location = location
            .or_else(|| page_location.map(|l| l.to_string()))
            .filter(|l| locations.iter().any(|(id, _)| id.to_string() == *l))
            .or_else(|| locations.first().map(|(id, _)| id.to_string()))
            .unwrap_or_else(|| NEW_LOCATION.to_owned());
        let (parts, held) = self.enclosure_part_choices(&location);
        let part = held
            .or(part)
            .or_else(|| parts.first().map(|(v, _)| v.clone()))
            .unwrap_or_default();
        let options = std::iter::once((
            NEW_LOCATION.to_owned(),
            strings::text(key!("enclosure-new-location")).to_owned(),
        ))
        .chain(locations.iter().map(|(id, code)| (id.to_string(), code.clone())))
        .collect();
        let mut form = super::FormDialog {
            action: "layout.enclosure_new",
            title_key: key!("enclosure-new-title"),
            ok_key: key!("enclosure-new-ok"),
            fields: vec![
                super::FormField {
                    label: None,
                    name: "location".into(),
                    label_key: key!("form-locate-location"),
                    choices: &super::choices::NONE,
                    options,
                    value: location,
                },
                super::FormField {
                    label: None,
                    name: "part".into(),
                    label_key: key!("form-enclosure-part"),
                    choices: &super::choices::NONE,
                    options: parts,
                    value: part,
                },
            ],
            error: None,
            table: None,
        };
        new_location_field(&mut form, &self.proposed_location_code());
        form
    }

    /// The rail Auto-fill rail fills by default: the selected rail, else the page's first rail.
    fn fill_rail(&self, panel: &PanelTables) -> Option<Uuid> {
        let sel = &self.tools.selection;
        sel.items
            .iter()
            .find_map(|e| panel.carrier(e.0).filter(|c| c.axis.is_some()).map(|c| c.id))
            .or_else(|| panel.carriers.iter().find(|c| c.axis.is_some()).map(|c| c.id))
    }

    /// The devices Auto-fill rail offers on the page shown (`Session::fill_candidates`: the cabinet's unplaced devices,
    /// then those without a location), each with its tag and whether it starts ticked: the devices marked in Unplaced
    /// devices where some are, else the first guess.
    fn fill_offer(&self, panel: &PanelTables) -> Vec<(Uuid, String, bool)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let marked = self.fill_devices();
        project
            .session
            .fill_candidates(panel.surface)
            .into_iter()
            .map(|(d, guess)| {
                let tag = project.session.device_tag(d).unwrap_or_default();
                let ticked = if marked.is_empty() {
                    guess
                } else {
                    marked.contains(&d)
                };
                (d, tag, ticked)
            })
            .collect()
    }

    /// The devices marked in the Unplaced devices navigator: what Auto-fill rail ticks where some are.
    fn fill_devices(&self) -> Vec<Uuid> {
        let Some(nav) = self.nav(crate::navigators::NavKind::Unplaced) else {
            return Vec::new();
        };
        let Some(table) = nav.table() else {
            return Vec::new();
        };
        table
            .nodes
            .iter()
            .filter(|n| n.role == ew_api::output::NavRole::Device && nav.marked().contains(&n.key))
            .filter_map(|n| n.records.first().copied())
            .collect()
    }

    /// Why the Create enclosure dialog's Create is disabled: its location is empty ("Choose a location"), or new
    /// without a code, else its part ("Type or choose a part number"); `None` while all are given or another dialog
    /// is open.
    pub fn enclosure_create_reason(&self) -> Option<String> {
        let Some(super::Dialog::Form(f)) = &self.dialog else {
            return None;
        };
        if f.action != "layout.enclosure_new" {
            return None;
        }
        let empty = |name: &str| {
            f.fields
                .iter()
                .any(|x| x.name == name && x.value.trim().is_empty())
        };
        let new = f
            .fields
            .iter()
            .any(|x| x.name == "location" && x.value == NEW_LOCATION);
        if empty("location") {
            Some(strings::text(key!("reason-enclosure-location")).to_owned())
        } else if new && empty("new_location") {
            Some(strings::text(key!("reason-enclosure-new-location")).to_owned())
        } else if empty("part") {
            Some(strings::text(key!("reason-enclosure-part")).to_owned())
        } else {
            None
        }
    }

    /// The Create enclosure dialog's line when its part list has nothing to offer: no enclosure part in the project
    /// or an installed library (the field then takes a typed part number); and the line of [`Self::enclosure_choice`].
    pub fn enclosure_form_notes(&self) -> Vec<String> {
        let Some(super::Dialog::Form(f)) = &self.dialog else {
            return Vec::new();
        };
        let empty = |name: &str| f.fields.iter().any(|x| x.name == name && x.options.is_empty());
        let mut notes = Vec::new();
        if empty("part") {
            notes.push(strings::text(key!("enclosure-new-no-part")).to_owned());
        }
        notes.extend(self.enclosure_choice().and_then(|c| c.3));
        notes
    }

    /// What the Create enclosure dialog's Primary does where it does not just create: `(label, new_page,
    /// create_only, line)`. A location with an enclosure that the page does not show draws it: "Draw -UC1 of +CAB1 on
    /// this page", or off a panel layout page "… on a new panel layout page" (`new_page`). Off a panel layout page a
    /// location without one is made and drawn on a new panel layout page, with Create only beside it
    /// (`create_only`). `line` says so. `None` while the Primary is Create: on a panel layout page for a location
    /// without an enclosure, or one the page shows already (Create then says why not).
    pub fn enclosure_choice(&self) -> Option<(String, bool, bool, Option<String>)> {
        let Some(super::Dialog::Form(f)) = &self.dialog else {
            return None;
        };
        if f.action != "layout.enclosure_new" {
            return None;
        }
        let location = f.fields.iter().find(|x| x.name == "location")?.value.clone();
        let on_panel = self.on_panel_page();
        let project = self.project.as_ref()?;
        let code = project
            .session
            .layout_locations()
            .into_iter()
            .find(|(id, _)| id.to_string() == location)
            .map(|(_, code)| code);
        let existing = project
            .session
            .location_enclosures()
            .into_iter()
            .find(|(l, ..)| l.to_string() == location);
        match (existing, code) {
            (Some((_, tag, surface, _)), Some(code)) => {
                if on_panel && self.panel_tables().is_some_and(|t| t.surface == surface) {
                    return None;
                }
                let key = if on_panel {
                    key!("enclosure-draw-here")
                } else {
                    key!("enclosure-draw-new-page")
                };
                let number = self.enclosure_part(&location).map(|(_, n)| n).unwrap_or_default();
                let line = strings::format(key!("enclosure-has-one"), &[&tag, &code, &number]);
                Some((strings::format(key, &[&tag, &code]), !on_panel, false, Some(line)))
            }
            _ if on_panel => None,
            _ => Some((
                strings::text(key!("enclosure-create-new-page")).to_owned(),
                true,
                true,
                Some(strings::text(key!("enclosure-not-layout")).to_owned()),
            )),
        }
    }

    /// Why Auto-fill rail on the page shown has nothing to offer, in words; `None` while it offers a device (a device
    /// that does not fit says so in the fill's own words).
    pub fn fill_cause(&self) -> Option<String> {
        let panel = self.panel_tables()?;
        if !self.fill_offer(&panel).is_empty() {
            return None;
        }
        let location = panel.location.as_str();
        Some(if panel.placements.is_empty() {
            strings::format(key!("auto-fill-cause-none"), &[location])
        } else {
            strings::format(key!("auto-fill-cause-all-placed"), &[location])
        })
    }

    /// The Auto-fill rail dialog's line on the devices it fills with.
    pub fn fill_devices_text(&self) -> String {
        match self.fill_devices().len() {
            0 => strings::text(key!("auto-fill-devices-all")).to_owned(),
            n => strings::format_count(count_key!("auto-fill-devices-marked"), n),
        }
    }

    /// `layout.auto_fill_rail {rail?, order, devices?}` (Draw ▸ Layout ▸ Auto-fill rail…):
    /// `layout.auto_fill` of the rail (its label or ID; by default the selected rail, else the page's first) in
    /// `order` (`schematic` or `dt`) with the devices chosen of those it offers ([`Self::fill_offer`]: the cabinet's
    /// unplaced devices, then those without a location). `devices` are device IDs or tags, or the rows of the
    /// window's table in its order (a ticked row's device); absent: the ticked ones (the marked devices, else the first
    /// guess). A chosen device without a location takes the cabinet's first (`fields.set` of `structure.location`),
    /// in the fill's undo step; a refused fill takes it back. A toast says what it placed and what did not fit. In the
    /// window without `order` the Auto-fill rail dialog opens with the devices each with its tick, saying why there is
    /// nothing to offer where that is so; a refusal stays under its fields.
    pub(crate) fn act_layout_auto_fill_rail(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "layout.auto_fill_rail";
        let Some(panel) = self.panel_tables() else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        let given = |name: &str| {
            p.get(name)
                .and_then(Value::as_str)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let rails: Vec<(String, String)> = panel
            .carriers
            .iter()
            .filter(|c| c.axis.is_some())
            .map(|c| (c.id.to_string(), strings::render(&c.label)))
            .collect();
        let offer = self.fill_offer(&panel);
        let Some(order) = given("order") else {
            return match self.mode {
                super::Mode::Headless => failed(key!("error-missing-param"), &["order"]),
                super::Mode::Gui if self.in_form_of(ACTION) => {
                    let message = strings::format(key!("error-missing-param"), &["order"]);
                    self.form_failed(ACTION, message)
                }
                super::Mode::Gui => {
                    let rail = given("rail")
                        .or_else(|| self.fill_rail(&panel).map(|r| r.to_string()))
                        .unwrap_or_default();
                    let form = super::FormDialog {
                        action: ACTION,
                        title_key: key!("auto-fill-title"),
                        ok_key: key!("auto-fill-ok"),
                        fields: vec![
                            super::FormField {
                                label: None,
                                name: "rail".into(),
                                label_key: key!("form-auto-fill-rail"),
                                choices: &super::choices::NONE,
                                options: rails,
                                value: rail,
                            },
                            super::FormField {
                                label: None,
                                name: "order".into(),
                                label_key: key!("form-auto-fill-order"),
                                choices: &super::choices::NONE,
                                options: fill_orders(),
                                value: "schematic".to_owned(),
                            },
                        ],
                        error: None,
                        // Each device it offers with its tick: the person chooses what goes on the rail.
                        table: (!offer.is_empty()).then(|| super::FormTable {
                            name: "devices",
                            heading_key: key!("form-auto-fill-devices"),
                            fixed: true,
                            key_label: key!("form-device"),
                            add_key: key!("form-value-set-add"),
                            columns: vec![(
                                strings::text(key!("form-auto-fill-tick")).to_owned(),
                                super::TICK.to_owned(),
                            )],
                            rows: offer
                                .iter()
                                .map(|(_, tag, ticked)| super::FormRow {
                                    key: tag.clone(),
                                    cells: vec![if *ticked {
                                        super::TICKED.to_owned()
                                    } else {
                                        String::new()
                                    }],
                                })
                                .collect(),
                        }),
                    };
                    self.open_dialog(super::Dialog::Form(form))
                }
            };
        };
        if order != "schematic" && order != "dt" {
            return failed(key!("error-bad-param"), &["order"]);
        }
        let devices: Vec<Uuid> = match p.get("devices") {
            None | Some(Value::Null) => offer.iter().filter(|o| o.2).map(|o| o.0).collect(),
            Some(Value::Array(items)) => {
                let mut out = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    match item {
                        // A row of the window's table: its device when ticked.
                        Value::Object(row) => {
                            let ticked = row
                                .get("values")
                                .and_then(Value::as_object)
                                .is_some_and(|v| !v.is_empty());
                            match offer.get(i) {
                                Some((d, ..)) if ticked => out.push(*d),
                                Some(_) => {}
                                None => return failed(key!("error-bad-param"), &["devices"]),
                            }
                        }
                        // A device the script names by its ID or its tag, of those offered.
                        Value::String(s) => {
                            let s = s.trim();
                            match offer.iter().find(|(d, tag, _)| d.to_string() == s || tag == s) {
                                Some((d, ..)) => out.push(*d),
                                None => {
                                    let message = strings::format(key!("error-fill-device"), &[s]);
                                    return self.form_failed(ACTION, message);
                                }
                            }
                        }
                        _ => return failed(key!("error-bad-param"), &["devices"]),
                    }
                }
                out
            }
            Some(_) => return failed(key!("error-bad-param"), &["devices"]),
        };
        if devices.is_empty() {
            let message = strings::text(key!("reason-fill-none")).to_owned();
            return self.form_failed(ACTION, message);
        }
        let rail = match given("rail") {
            Some(r) => rails
                .iter()
                .find(|(id, label)| *id == r || label.eq_ignore_ascii_case(&r))
                .and_then(|(id, _)| id.parse::<Uuid>().ok()),
            None => self.fill_rail(&panel),
        };
        let Some(rail) = rail else {
            let message = strings::text(key!("error-no-rail")).to_owned();
            return self.form_failed(ACTION, message);
        };
        let sentence = panel
            .carrier(rail)
            .map(|c| strings::render(&c.sentence))
            .unwrap_or_default();
        let before = panel
            .placements
            .iter()
            .filter(|x| x.carrier == Some(rail))
            .count();
        // The chosen devices without a location take the cabinet's first, in the fill's undo step.
        let unlocated: Vec<Uuid> = self.project.as_ref().map_or_else(Vec::new, |p| {
            let none = p.session.unlocated_devices();
            devices.iter().copied().filter(|d| none.contains(d)).collect()
        });
        let location = self.project.as_ref().and_then(|p| {
            p.session
                .layout_locations()
                .into_iter()
                .find(|(_, code)| *code == panel.location)
                .map(|(id, _)| id)
        });
        let located = match location {
            Some(location) if !unlocated.is_empty() => {
                let out = self.give_location(&unlocated, location);
                if out != Outcome::Done {
                    return self.layout_form_done(ACTION, out);
                }
                true
            }
            _ => false,
        };
        let params = json!({
            "carrier": rail.to_string(),
            "order": order,
            "devices": devices.iter().map(ToString::to_string).collect::<Vec<_>>(),
        });
        let (out, notices) = self.host_create_noticed("layout.auto_fill", params);
        if located {
            // All or nothing: one undo takes back the location with the fill, and a refused fill takes it back.
            if out == Outcome::Done {
                if let Some(p) = &mut self.project {
                    p.session.fold_last_step();
                }
            } else {
                let _ = self.act_undo();
            }
        }
        if out == Outcome::Done {
            // The page's tables follow at the next frame; the session's are current.
            let after = self
                .project
                .as_ref()
                .and_then(|p| p.session.panel_tables(&panel.folio))
                .map_or(before, |t| {
                    t.placements.iter().filter(|x| x.carrier == Some(rail)).count()
                });
            let devices =
                strings::format_count(count_key!("auto-fill-devices"), after.saturating_sub(before));
            let text = if notices.is_empty() {
                strings::format(key!("toast-auto-fill"), &[&devices, &sentence])
            } else {
                strings::format(
                    key!("toast-auto-fill-left"),
                    &[&devices, &sentence, &notices.join("; ")],
                )
            };
            self.notify(text, true);
        }
        self.layout_form_done(ACTION, out)
    }

    /// Whether devices drawn on the wiring pages have no location: what a layout page's Give location… offers.
    pub fn has_devices_to_locate(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| !p.session.unlocated_drawn_devices().is_empty())
    }

    /// Gives `devices` the location node `location`: one `fields.set` of `structure.location`, one undo step.
    fn give_location(&mut self, devices: &[Uuid], location: Uuid) -> Outcome {
        self.commit_from_host(&AnyCommand {
            name: "fields.set".into(),
            params: json!({
                "kind": "device",
                "targets": devices.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "path": "structure.location",
                "value": location.to_string(),
            }),
        })
    }

    /// `layout.locate_devices {location?, devices?}` (a layout page's Give location…: many devices at once,
    /// not Properties for each): gives devices the location `location` (its `+` path or node ID; by
    /// default the cabinet's the page shows) in one step. `devices` are device IDs, or the rows of the window's table
    /// in its order (a ticked row's device); by default the devices drawn on the wiring pages without a location
    /// (`Session::unlocated_drawn_devices`) but those behind terminals in the field. In the window without `devices`
    /// the preview opens: the location, and those devices each with its tick, the field devices unticked, a device
    /// whose tag will then print that location in front saying so (`Session::would_print_location`, for the location
    /// the preview opened with). A toast says how many took the location and where they wait. A drawn tag stays as it
    /// was where the pages draw only the cabinet's devices and devices without a location (`PageDisplayStructure`).
    pub(crate) fn act_layout_locate_devices(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "layout.locate_devices";
        let Some(panel) = self.panel_tables() else {
            return Outcome::refused(key!("reason-no-panel-page"));
        };
        let project = or_return!(crate::shell::gated(&self.project));
        let candidates = project.session.unlocated_drawn_devices();
        let locations = project.session.layout_locations();
        let location = match string_param(p, "location") {
            Err(e) => return e,
            Ok(Some(l)) => locations
                .iter()
                .find(|(id, code)| id.to_string() == l || code.eq_ignore_ascii_case(l)),
            Ok(None) => locations.iter().find(|(_, code)| *code == panel.location),
        };
        let Some((location, code)) = location.cloned() else {
            return failed(key!("error-bad-param"), &["location"]);
        };
        // Each row names its device, and says where its tag will print the location in front: on a page that
        // draws another location (the prefix came unannounced).
        let tags: Vec<String> = candidates
            .iter()
            .map(|(d, _)| {
                let tag = project.session.device_tag(*d).unwrap_or_default();
                if project.session.would_print_location(*d, location) {
                    strings::format(key!("locate-row-prefixed"), &[&tag, &code])
                } else {
                    tag
                }
            })
            .collect();
        let devices: Vec<Uuid> = match p.get("devices") {
            None | Some(Value::Null) if self.mode == super::Mode::Gui && !self.in_form_of(ACTION) => {
                if candidates.is_empty() {
                    return Outcome::refused(key!("reason-locate-nothing"));
                }
                let tick = strings::text(key!("form-locate-tick")).to_owned();
                let form = super::FormDialog {
                    action: ACTION,
                    title_key: key!("locate-title"),
                    ok_key: key!("locate-ok"),
                    fields: vec![super::FormField {
                        label: None,
                        name: "location".into(),
                        label_key: key!("form-locate-location"),
                        choices: &super::choices::NONE,
                        options: locations
                            .iter()
                            .map(|(id, c)| (id.to_string(), c.clone()))
                            .collect(),
                        value: location.to_string(),
                    }],
                    error: None,
                    table: Some(super::FormTable {
                        name: "devices",
                        heading_key: key!("form-locate-devices"),
                        fixed: true,
                        key_label: key!("form-device"),
                        add_key: key!("form-value-set-add"),
                        columns: vec![(tick, super::TICK.to_owned())],
                        rows: candidates
                            .iter()
                            .zip(&tags)
                            .map(|((_, field), tag)| super::FormRow {
                                key: tag.clone(),
                                cells: vec![if *field {
                                    String::new()
                                } else {
                                    super::TICKED.to_owned()
                                }],
                            })
                            .collect(),
                    }),
                };
                return self.open_dialog(super::Dialog::Form(form));
            }
            None | Some(Value::Null) => candidates.iter().filter(|(_, f)| !f).map(|(d, _)| *d).collect(),
            Some(Value::Array(items)) => {
                let mut out = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    match item {
                        // A row of the window's table: its device when ticked.
                        Value::Object(row) => {
                            let ticked = row
                                .get("values")
                                .and_then(Value::as_object)
                                .is_some_and(|v| !v.is_empty());
                            match candidates.get(i) {
                                Some((d, _)) if ticked => out.push(*d),
                                Some(_) => {}
                                None => return failed(key!("error-bad-param"), &["devices"]),
                            }
                        }
                        Value::String(s) => match s.trim().parse::<Uuid>() {
                            Ok(d) => out.push(d),
                            Err(_) => return failed(key!("error-bad-param"), &["devices"]),
                        },
                        _ => return failed(key!("error-bad-param"), &["devices"]),
                    }
                }
                out
            }
            Some(_) => return failed(key!("error-bad-param"), &["devices"]),
        };
        if devices.is_empty() {
            let message = strings::text(key!("reason-locate-none")).to_owned();
            return self.form_failed(ACTION, message);
        }
        let out = self.give_location(&devices, location);
        if out == Outcome::Done {
            let n = strings::format_count(count_key!("auto-fill-devices"), devices.len());
            self.notify(strings::format(key!("toast-locate-devices"), &[&n, &code]), true);
        }
        self.layout_form_done(ACTION, out)
    }

    /// The end of a set-up action: a failure stays in its dialog, a success closes it.
    pub(super) fn layout_form_done(&mut self, action: &str, out: Outcome) -> Outcome {
        match out {
            Outcome::Failed { message } => self.form_failed(action, in_words(&message)),
            Outcome::Done => {
                if self.in_form_of(action) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            other => other,
        }
    }
}
