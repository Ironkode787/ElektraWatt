// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The panel page's host duties (WP-S4.02 part 3, D4, D10; doc 12 §12.4): arming the layout place tool, the keyboard
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
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::{
    Fit, PanelPlacement, PanelTables, RailItem, SnapMode, Uuid, end_gap, fits_rail, min_gap, start_gap,
};
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::strings;
use crate::strings::Key;
use crate::tools::layout::{mode_word, quarter_quat};
use crate::tools::{
    CanvasEvent, EntityKind, LayoutOptions, LayoutPlaceTool, SelectionChange, ToolKey, page_entity_kind,
};

/// Millimetres as the project prints them (FT-366): the shortest decimal with the project's separator.
pub(crate) fn mm_text(nm: i128, comma: bool) -> String {
    let sign = if nm < 0 { "-" } else { "" };
    let a = nm.unsigned_abs();
    let (int, frac) = (a / 1_000_000, a % 1_000_000);
    let text = if frac == 0 {
        format!("{sign}{int}")
    } else {
        let digits = format!("{frac:06}");
        format!("{sign}{int}.{}", digits.trim_end_matches('0'))
    };
    if comma { text.replace('.', ",") } else { text }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The panel tables of the focused canvas's page (a panel-layout page, D9); `None` on any other page.
    pub fn panel_tables(&self) -> Option<Arc<PanelTables>> {
        self.canvas()?.page().panel.clone()
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
    fn selected_placement(&self) -> Option<(Arc<PanelTables>, PanelPlacement)> {
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
                    n => strings::format_count(key!("layout-findings"), n),
                };
                Some(strings::format(
                    key!("layout-live-rail"),
                    &[
                        &row.tag,
                        &rail.sentence,
                        &mm_text(i128::from(row.pose.0), comma),
                        &findings,
                    ],
                ))
            }
            None => Some(strings::format(
                key!("layout-live-plate"),
                &[
                    &row.tag,
                    &mm_text(i128::from(row.pose.0), comma),
                    &mm_text(i128::from(row.pose.1), comma),
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
        Outcome::Refused { reason_key }
    }

    /// `tool.place_layout {device}` (Place device, Enter on an Unplaced devices row, a drop of the row): arms the
    /// layout place tool with the device on the focused panel page; refused without one ("Open a panel layout page").
    pub(crate) fn act_place_layout(&mut self, p: &Map<String, Value>) -> Outcome {
        let device = match string_param(p, "device") {
            Ok(Some(d)) => d.trim().trim_start_matches("id:").to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["device"]),
            Err(e) => return e,
        };
        let Ok(device) = device.parse::<Uuid>() else {
            return failed(key!("error-bad-param"), &["device"]);
        };
        self.place_layout_device(device)
    }

    /// Arms the layout place tool with `device` (see [`ShellModel::act_place_layout`]).
    pub(super) fn place_layout_device(&mut self, device: Uuid) -> Outcome {
        let Some(panel) = self.panel_tables() else {
            let text = strings::text(key!("reason-no-panel-page")).to_owned();
            return self.refuse_with(text, key!("reason-no-panel-page"));
        };
        if panel.unplaced_device(device).is_none() {
            let text = strings::text(key!("reason-no-unplaced-device")).to_owned();
            return self.refuse_with(text, key!("reason-no-unplaced-device"));
        }
        self.activate_tool(Box::new(LayoutPlaceTool::new(Some(device))));
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
    /// a cursor move; nothing further that way is no error.
    fn step_layout_ghost(&mut self, dx: i64, dy: i64) -> Outcome {
        let Some(panel) = self.panel_tables() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-panel-page"),
            };
        };
        let grid = self.tools.layout_options.grid();
        let Some(p) = self
            .tools
            .tool
            .layout_gesture()
            .and_then(|g| g.step(&panel, (dx, dy), grid))
        else {
            return Outcome::Done;
        };
        let Some(at) = self
            .canvas()
            .map(|c| c.page().snapper.page_to_sheet(panel.to_page(p)))
        else {
            return Outcome::Done;
        };
        self.send_tool(&CanvasEvent::CursorMove { at })
    }

    /// `layout.next` (Tab on a panel page) and `layout.previous` (Shift+Tab, ruling 5): the next or previous placement
    /// of the focused rail (the selected placement's rail, else Rail 1) in carrier order (by `s0`), wrapping.
    pub(crate) fn act_layout_step(&mut self, step: i64) -> Outcome {
        let Some(panel) = self.panel_tables() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-panel-page"),
            };
        };
        let current = self.selected_placement().map(|(_, r)| r);
        let rail = current
            .as_ref()
            .and_then(|r| r.carrier)
            .or_else(|| panel.carriers.iter().find(|c| c.axis.is_some()).map(|c| c.id));
        let Some(rail) = rail else {
            return Outcome::Refused {
                reason_key: key!("reason-no-placement"),
            };
        };
        let mut on: Vec<&PanelPlacement> = panel
            .placements
            .iter()
            .filter(|p| p.carrier == Some(rail))
            .collect();
        on.sort_by_key(|p| (i128::from(p.pose.0) - p.clip, p.id));
        if on.is_empty() {
            return Outcome::Refused {
                reason_key: key!("reason-no-placement"),
            };
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
            return Outcome::Refused {
                reason_key: key!("reason-no-placement"),
            };
        };
        let grid = self.tools.layout_options.grid();
        let mode = self.tools.layout_options.mode;
        let q = quarter_quat(row.pose.2);
        let cmd = match row.carrier.and_then(|c| panel.carrier(c)) {
            Some(rail) => {
                let (Some(axis), Some(length)) = (rail.axis, rail.length) else {
                    return Outcome::Refused {
                        reason_key: key!("reason-no-placement"),
                    };
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
                    return Outcome::Refused {
                        reason_key: key!("reason-no-placement"),
                    };
                };
                let s0 = own.s0;
                let target = s0 + sign * grid;
                let s_left = if mode == SnapMode::Push && sign > 0 {
                    target
                } else {
                    match nudge_stop(&rail.items, own, length, target, sign) {
                        Stop::Free(s) => s,
                        Stop::Blocked(by) => {
                            let text = match by.and_then(|p| panel.placement(p)) {
                                Some(n) => strings::format(key!("layout-blocked"), &[&n.tag]),
                                None => strings::text(key!("layout-rail-end")).to_owned(),
                            };
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
        self.commit_layout("layout.move", cmd)
    }

    /// `layout.rotate` (R) and `layout.flip` (X) on a panel page: to a floating ghost, its turn; to the selected
    /// placement, R turns it 90° on the plate and X turns it 180° on a rail (`layout.move`); R on a rail and X on the
    /// plate are refused in words with the keys of the keymap (ruling 7).
    pub(crate) fn act_layout_turn(&mut self, key: ToolKey) -> Outcome {
        if matches!(self.tools.tool.ghost(), Some(crate::tools::Ghost::Layout { .. })) {
            return self.send_tool(&CanvasEvent::Key(key));
        }
        let Some((panel, row)) = self.selected_placement() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-placement"),
            };
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
        self.commit_layout("layout.move", cmd)
    }

    /// The words of a refused turn (R on a rail, X on the plate) with the keys of the keymap (ruling 7): the text of
    /// `reason-layout-turn` wherever it shows, from `layout.rotate` and `layout.flip` here and from the layout place
    /// and Select tools' turn keys during a drag ([`ShellModel::refusal_text`]).
    pub(super) fn layout_turn_refusal(&self) -> String {
        strings::format(
            key!("layout-wrong-turn"),
            &[
                &crate::command_bar::key_of(self, "tool.rotate_cw"),
                &crate::command_bar::key_of(self, "tool.mirror_x"),
            ],
        )
    }

    /// `layout.lock_selection` and `layout.unlock_selection` (Lock and Unlock in the canvas context menu over a
    /// placement): `layout.lock {placement, locked}` of the selected placement.
    pub(crate) fn act_layout_lock(&mut self, locked: bool) -> Outcome {
        let Some((_, row)) = self.selected_placement() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-placement"),
            };
        };
        self.commit_layout(
            "layout.lock",
            json!({ "placement": row.id.to_string(), "locked": locked }),
        )
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

    /// Commits a `layout.*` command built by the host.
    fn commit_layout(&mut self, name: &str, params: Value) -> Outcome {
        let cmd = AnyCommand {
            name: name.into(),
            params,
        };
        let out = self.commit_from_host(&cmd);
        self.tools.layout_announce |= out == Outcome::Done;
        out
    }

    /// The after-commit duty of a `layout.*` commit (at the end of its dispatch, the page read again): the live region
    /// says where the selected placement now is ("-102F1, rail 1 at 163 mm, no findings").
    pub(super) fn after_layout_commit(&mut self) {
        if std::mem::take(&mut self.tools.layout_announce)
            && let Some(text) = self.layout_live_text()
        {
            self.tools.announcement = Some(text);
        }
    }
}

/// Where a free nudge stops.
enum Stop {
    /// At this left edge.
    Free(i128),
    /// Blocked by this neighbour's placement (`None`: the rail end).
    Blocked(Option<Uuid>),
}

/// A free nudge of `own` towards `target` (`sign` its direction): it stops where it would touch the next item at its
/// minimum gap, or at the rail end's gap; blocked when it cannot move at all.
fn nudge_stop(items: &[RailItem], own: &RailItem, length: i128, target: i128, sign: i128) -> Stop {
    let width = own.width();
    let others = items.iter().filter(|i| i.placement != own.placement);
    if sign > 0 {
        let next = others
            .filter(|i| i.s0 >= own.s1 || (i.s0 > own.s0))
            .min_by_key(|i| (i.s0, i.s1));
        let (limit, by) = match next {
            Some(n) => (n.s0 - min_gap(own, n) - width, n.placement.map(|p| p.as_uuid())),
            None => (length - end_gap(own) - width, None),
        };
        let s = target.min(limit);
        if s <= own.s0 {
            Stop::Blocked(by)
        } else {
            Stop::Free(s)
        }
    } else {
        let prev = others
            .filter(|i| i.s1 <= own.s0 || (i.s0 < own.s0))
            .max_by_key(|i| (i.s1, i.s0));
        let (limit, by) = match prev {
            Some(n) => (n.s1 + min_gap(n, own), n.placement.map(|p| p.as_uuid())),
            None => (start_gap(own), None),
        };
        let s = target.max(limit);
        if s >= own.s0 {
            Stop::Blocked(by)
        } else {
            Stop::Free(s)
        }
    }
}

/// An `i128` in the `i64` range, saturating.
fn clamp64(v: i128) -> i64 {
    i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
}
