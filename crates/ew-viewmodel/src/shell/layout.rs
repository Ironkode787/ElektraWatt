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
use ew_api::output::{Fit, PanelPlacement, PanelTables, SnapMode, Uuid, fits_rail};
use serde_json::{Map, Value, json};

use super::navigation::bump;
use super::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::strings::Key;
use crate::tools::layout::{Step, Stop, mode_word, nudge_stop, quarter_quat};
use crate::tools::{
    CanvasEvent, EntityKind, LayoutOptions, LayoutPlaceTool, SelectionChange, ToolKey, page_entity_kind,
};
use crate::{count_key, key};

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
    /// a cursor move; nothing further that way is no error. Along a rail the ghost stops at a neighbour and at the rail
    /// end in words, as a selected placement does ("Blocked by -102F2", "At the rail end"; FT-992); the ghost stays.
    fn step_layout_ghost(&mut self, dx: i64, dy: i64) -> Outcome {
        let Some(panel) = self.panel_tables() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-panel-page"),
            };
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
            // FT-994: the tool resolves the step without snapping, as a pointer move with Shift held.
            let mods = crate::tools::Mods {
                shift: true,
                ..crate::tools::Mods::default()
            };
            return self.send_tool(&CanvasEvent::PointerMove { at, mods });
        }
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
                &crate::command_bar::key_name(self, "tool.rotate_cw"),
                &crate::command_bar::key_name(self, "tool.mirror_x"),
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
        // FT-900 (ID-1147): the placement a layout drop made is the selection, replacing it.
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

/// The words of a stopped nudge: "Blocked by -102F2" for a neighbour's placement, "At the rail end" for the end.
fn blocked_text(panel: &PanelTables, by: Option<Uuid>) -> String {
    match by.and_then(|p| panel.placement(p)) {
        Some(n) => strings::format(key!("layout-blocked"), &[&n.tag]),
        None => strings::text(key!("layout-rail-end")).to_owned(),
    }
}

/// An `i128` in the `i64` range, saturating.
fn clamp64(v: i128) -> i64 {
    i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
}

// ---------------------------------------------------------------------------------------------------------------
// WP-S4.02 part 4: setting the panel up from the window (§8; D10, D11; rulings 7, 8).

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

/// The orders of the Auto-fill rail dialog, `(value, words)` (ruling 7: a choice list built at run time).
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

/// The registry's label of Structure node… (`structure.new`), as the texts that name it print it.
fn structure_new_label() -> String {
    crate::actions::find("structure.new")
        .map(|a| strings::text(Key::run_time(&a.label_key())).to_owned())
        .unwrap_or_default()
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
    fn focused_page_id(&self) -> Option<Uuid> {
        match self.current_page()? {
            crate::canvas::PageId(ew_api::output::ew_dl::Destination::Page(id)) => Some(id),
            _ => None,
        }
    }

    /// The parts a draw tool of `kind` (`rail`, `duct`) offers (D11): the project's copies of the kind, then the
    /// installed libraries' parts, embedded through `copies` when drawn.
    ///
    /// The offers are kept for (session, revision, kind) (C44-p2#2): arming the tool or changing its options computes
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
            (mm_text(w, comma), mm_text(h, comma))
        });
        Some(crate::tools::DrawCard {
            kind,
            parts,
            part: part.map(|p| p.number),
            size,
        })
    }

    /// `tool.rail_draw` (Draw ▸ Layout ▸ Rail, D11): arms Draw rail with the tool option's rail part.
    pub(crate) fn act_tool_rail_draw(&mut self) -> Outcome {
        self.forget_draw_offers();
        let part = self.draw_part("rail");
        self.activate_tool(Box::new(crate::tools::RailDrawTool::new(part)));
        Outcome::Done
    }

    /// `tool.duct_draw` (Draw ▸ Layout ▸ Duct, D11): arms Draw duct with the tool option's duct part and size.
    pub(crate) fn act_tool_duct_draw(&mut self) -> Outcome {
        self.forget_draw_offers();
        let part = self.draw_part("duct");
        let size = self.tools.layout_options.duct_size;
        self.activate_tool(Box::new(crate::tools::DuctDrawTool::new(part, size)));
        Outcome::Done
    }

    /// `layout.draw_options {part?, width?, height?}` (the draw tools' options card, D11): the part by its number, a
    /// duct's width and height in millimetres; the choice stays for the next drawing, and the running tool takes it.
    pub(crate) fn act_layout_draw_options(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(kind) = draw_kind(self.tools.tool.name()) else {
            return Outcome::Refused {
                reason_key: key!("reason-no-draw-tool"),
            };
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

    /// `layout.enclosure_new {location, part}` (Project ▸ Enclosure…, D7, D10): `layout.enclosure.create_from_part` of
    /// the location (its code or ID) with the enclosure part (its number or ID: a project copy, or an installed
    /// library's part, embedded through `copies` with its plate part, ruling 8), the focused page's function as the
    /// enclosure device's. In the window without both the Create enclosure dialog opens (the focused page's location
    /// and the first part chosen); a refusal stays under its fields.
    pub(crate) fn act_layout_enclosure_new(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "layout.enclosure_new";
        // A project without a location has nowhere to put an enclosure: the menu disables the entry with this text
        // (W44.2 F1), and the action refuses with it.
        if let Some(reason) = self.enclosure_new_reason() {
            self.tools.notice = Some(reason);
            self.tools.notice_sticky = false;
            return Outcome::Refused {
                reason_key: key!("enclosure-new-no-location"),
            };
        }
        let given = |name: &str| {
            p.get(name)
                .and_then(Value::as_str)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let (location, part) = (given("location"), given("part"));
        let (Some(location), Some(part)) = (location.clone(), part.clone()) else {
            let missing = if location.is_none() { "location" } else { "part" };
            return match self.mode {
                super::Mode::Headless => failed(key!("error-missing-param"), &[missing]),
                // Create is disabled while a field is empty (W44.2 F1); Enter in the form says why in the same words.
                super::Mode::Gui if self.in_layout_form(ACTION) => {
                    let message = strings::text(if location.is_none() {
                        key!("reason-enclosure-location")
                    } else {
                        key!("reason-enclosure-part")
                    })
                    .to_owned();
                    self.layout_form_failed(ACTION, message)
                }
                super::Mode::Gui => {
                    let form = self.enclosure_form(location, part);
                    self.open_dialog(super::Dialog::Form(form))
                }
            };
        };
        let page = self.focused_page_id();
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let found = project.session.layout_locations().into_iter().find(|(id, code)| {
            id.to_string() == location || *code == location || code.trim_start_matches('+') == location
        });
        let Some((location_id, _)) = found else {
            let message = strings::format(key!("error-no-location"), &[&location]);
            return self.layout_form_failed(ACTION, message);
        };
        let offer = project
            .session
            .layout_part_offers("enclosure")
            .into_iter()
            .find(|o| o.part.to_string() == part || o.number == part);
        let Some(offer) = offer else {
            let message = strings::format(key!("error-no-enclosure-part"), &[&part]);
            return self.layout_form_failed(ACTION, message);
        };
        let function = page.and_then(|pg| project.session.page_structure(pg).1);
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
        let out = self.host_create("layout.enclosure.create_from_part", params);
        self.layout_form_done(ACTION, out)
    }

    /// The Create enclosure dialog (ruling 7): the location (the given one, else the focused page's, else the first)
    /// and the enclosure part (the given one, else the first: the project's copies, then the libraries' parts).
    fn enclosure_form(&self, location: Option<String>, part: Option<String>) -> super::FormDialog {
        let page = self.focused_page_id();
        let (locations, parts, page_location) = match &self.project {
            Some(project) => (
                project.session.layout_locations(),
                project.session.layout_part_offers("enclosure"),
                page.and_then(|pg| project.session.page_structure(pg).0),
            ),
            None => (Vec::new(), Vec::new(), None),
        };
        let location = location
            .or_else(|| page_location.map(|l| l.to_string()))
            .filter(|l| locations.iter().any(|(id, _)| id.to_string() == *l))
            .or_else(|| locations.first().map(|(id, _)| id.to_string()))
            .unwrap_or_default();
        let part = part
            .or_else(|| parts.first().map(|o| o.part.to_string()))
            .unwrap_or_default();
        super::FormDialog {
            action: "layout.enclosure_new",
            title_key: key!("enclosure-new-title"),
            ok_key: key!("enclosure-new-ok"),
            fields: vec![
                super::FormField {
                    name: "location",
                    label_key: key!("form-enclosure-location"),
                    choices: &super::choices::NONE,
                    options: locations
                        .iter()
                        .map(|(id, code)| (id.to_string(), code.clone()))
                        .collect(),
                    value: location,
                },
                super::FormField {
                    name: "part",
                    label_key: key!("form-enclosure-part"),
                    choices: &super::choices::NONE,
                    options: parts
                        .iter()
                        .map(|o| {
                            let words = if o.description.is_empty() {
                                o.number.clone()
                            } else {
                                format!("{} \u{b7} {}", o.number, o.description)
                            };
                            (o.part.to_string(), words)
                        })
                        .collect(),
                    value: part,
                },
            ],
            error: None,
            table: None,
        }
    }

    /// The rail Auto-fill rail fills by default: the selected rail, else the page's first rail.
    fn fill_rail(&self, panel: &PanelTables) -> Option<Uuid> {
        let sel = &self.tools.selection;
        sel.items
            .iter()
            .find_map(|e| panel.carrier(e.0).filter(|c| c.axis.is_some()).map(|c| c.id))
            .or_else(|| panel.carriers.iter().find(|c| c.axis.is_some()).map(|c| c.id))
    }

    /// The devices Auto-fill rail fills with: the Unplaced devices navigator's marked device rows; empty for none (the
    /// command then takes every unplaced device of the layout space that fits the rail).
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

    /// Why Project ▸ Enclosure… is disabled in a project that can be changed (W44.2 F1): the project has no location
    /// yet ("The project has no location yet; add one with Structure node… first.", named by the registry's label);
    /// `None` when it has one, or no project is open.
    pub fn enclosure_new_reason(&self) -> Option<String> {
        let project = self.project.as_ref()?;
        if !project.session.layout_locations().is_empty() {
            return None;
        }
        Some(strings::format(
            key!("enclosure-new-no-location"),
            &[&structure_new_label()],
        ))
    }

    /// Why the Create enclosure dialog's Create is disabled (W44.2 F1): its location is empty ("Choose a location"),
    /// else its part ("Type or choose a part number"); `None` while both are given or another dialog is open.
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
        if empty("location") {
            Some(strings::text(key!("reason-enclosure-location")).to_owned())
        } else if empty("part") {
            Some(strings::text(key!("reason-enclosure-part")).to_owned())
        } else {
            None
        }
    }

    /// The Create enclosure dialog's lines when a list has nothing to offer: no location yet (the user adds one with
    /// Structure node…, named by its registry label), no enclosure part in the project or an installed library. The
    /// fields then take typed text (a location's code, a part number).
    pub fn enclosure_form_notes(&self) -> Vec<String> {
        let Some(super::Dialog::Form(f)) = &self.dialog else {
            return Vec::new();
        };
        let empty = |name: &str| f.fields.iter().any(|x| x.name == name && x.options.is_empty());
        let mut notes = Vec::new();
        if empty("location") {
            notes.push(strings::format(
                key!("enclosure-new-no-location"),
                &[&structure_new_label()],
            ));
        }
        if empty("part") {
            notes.push(strings::text(key!("enclosure-new-no-part")).to_owned());
        }
        notes
    }

    /// The Auto-fill rail dialog's line on the devices it fills with.
    pub fn fill_devices_text(&self) -> String {
        match self.fill_devices().len() {
            0 => strings::text(key!("auto-fill-devices-all")).to_owned(),
            n => strings::format_count(count_key!("auto-fill-devices-marked"), n),
        }
    }

    /// `layout.auto_fill_rail {rail?, order}` (Draw ▸ Layout ▸ Auto-fill rail…, D6, §8): `layout.auto_fill` of the
    /// rail (its label or ID; by default the selected rail, else the page's first) in `order` (`schematic` or `dt`)
    /// with the Unplaced devices navigator's marked devices, else every unplaced device that fits; a toast says what it
    /// placed and what did not fit. In the window without `order` the Auto-fill rail dialog opens; a refusal stays
    /// under its fields.
    pub(crate) fn act_layout_auto_fill_rail(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "layout.auto_fill_rail";
        let Some(panel) = self.panel_tables() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-panel-page"),
            };
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
            .map(|c| (c.id.to_string(), c.label.clone()))
            .collect();
        let Some(order) = given("order") else {
            return match self.mode {
                super::Mode::Headless => failed(key!("error-missing-param"), &["order"]),
                super::Mode::Gui if self.in_layout_form(ACTION) => {
                    let message = strings::format(key!("error-missing-param"), &["order"]);
                    self.layout_form_failed(ACTION, message)
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
                                name: "rail",
                                label_key: key!("form-auto-fill-rail"),
                                choices: &super::choices::NONE,
                                options: rails,
                                value: rail,
                            },
                            super::FormField {
                                name: "order",
                                label_key: key!("form-auto-fill-order"),
                                choices: &super::choices::NONE,
                                options: fill_orders(),
                                value: "schematic".to_owned(),
                            },
                        ],
                        error: None,
                        table: None,
                    };
                    self.open_dialog(super::Dialog::Form(form))
                }
            };
        };
        if order != "schematic" && order != "dt" {
            return failed(key!("error-bad-param"), &["order"]);
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
            return self.layout_form_failed(ACTION, message);
        };
        let sentence = panel
            .carrier(rail)
            .map(|c| c.sentence.clone())
            .unwrap_or_default();
        let before = panel
            .placements
            .iter()
            .filter(|x| x.carrier == Some(rail))
            .count();
        let devices = self.fill_devices();
        let mut params = json!({ "carrier": rail.to_string(), "order": order });
        if !devices.is_empty() {
            params["devices"] = Value::Array(devices.iter().map(|d| Value::String(d.to_string())).collect());
        }
        let (out, notices) = self.host_create_noticed("layout.auto_fill", params);
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

    /// Whether the open dialog is the form of `action`.
    fn in_layout_form(&self, action: &str) -> bool {
        matches!(&self.dialog, Some(super::Dialog::Form(f)) if f.action == action)
    }

    /// A refusal of a set-up action: it stays under the fields when its form is open.
    fn layout_form_failed(&mut self, action: &str, message: String) -> Outcome {
        if let Some(super::Dialog::Form(f)) = &mut self.dialog
            && f.action == action
        {
            f.error = Some(message.clone());
        }
        Outcome::Failed { message }
    }

    /// The end of a set-up action: a failure stays in its dialog, a success closes it.
    fn layout_form_done(&mut self, action: &str, out: Outcome) -> Outcome {
        match out {
            Outcome::Failed { message } => self.layout_form_failed(action, in_words(&message)),
            Outcome::Done => {
                if self.in_layout_form(action) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            other => other,
        }
    }
}
