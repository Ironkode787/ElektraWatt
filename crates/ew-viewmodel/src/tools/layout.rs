// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placing on a panel page (WP-S4.02 part 3, D4, D9; doc 12 §12.1): the gesture that the layout place tool
//! ([`LayoutPlaceTool`], armed by `tool.place_layout {device}` or a drop from the Unplaced devices navigator) and
//! Select's drag of a placement share, its ghost (doc 16a §7.3) and its command.
//!
//! **One resolver** (D4). The gesture reads the page's panel tables ([`ew_api::output::PanelTables`], D9) and nothing
//! else. On a rail it builds the moved item from the device's rail extent and runs [`resolve`], the function
//! `layout.place` and `layout.move` run, over the rail's items with the chosen mode; it commits the same snapped
//! input (the clip point `t.x = s_left + clip`, R1), so the command re-resolves exactly what the ghost shows: the
//! ghost's predicted poses are the committed ones.
//!
//! **Target.** The nearest rail within 15 points on screen whose axis the pointer projects onto between its ends,
//! else the plate. A device whose mount kind is a TS kind off every rail is refused in the tool ("F55 mounts on a TS
//! 35×7.5 rail; drop it on a rail.", ruling 6); a screw-mounted device always targets the plate; a device of unknown
//! size either.
//!
//! **Target snapping on a rail** (D4). The footprint's left edge snaps within 15 points to (1) where it touches a
//! neighbour at its minimum gap, (2) the rail ends at their gaps, (4) the plate grid along the axis (5 mm or 1 mm);
//! the nearest wins, at equal distance (1) before (2) before (4). Target (3), the column alignment with the rail
//! above, is de-scoped in this part (§13's order). Shift suspends the snapping: the pointer's `s` rounded to 0.1 mm.
//! On the plate the footprint snaps to the grid and, within 15 points, to the edges and centres of the other
//! footprints and of the ducts.
//!
//! **Modes.** Free by default; Ctrl held pushes, Alt held swaps; else the sticky mode of the tool options card. An
//! assembly member whose siblings sit on the target rail defaults to push at its member-order slot (ruling 4) while no
//! modifier is held and the sticky mode is free.
//!
//! **Refusals** (ruling 1). A refused drop is drawn at once from [`resolve`]'s error, from [`fits_rail`] or from the
//! tool's own off-rail rule; its text is the command's own, from the host's preview of the command the tool would
//! commit (asked only when that command changes), or the tool's off-rail text.

use ew_api::AnyCommand;
use ew_api::output::ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_api::output::{
    Fit, PanelAxis, PanelTables, Profile, RailItem, SnapMode, Uuid, brackets, end_gap, fits_rail, min_gap,
    resolve, start_gap,
};
use serde_json::{Value, json};

use super::{
    Anchor, Button, CanvasEvent, Ghost, Mods, Overlay, OverlayItem, PreviewInfo, Style, Token, Tool, ToolCx,
    ToolKey, ToolOutput, clamp64,
};
use crate::key;
use crate::strings;

/// How near a rail or a snap target the pointer must come, in screen points (D4).
pub const LAYOUT_SNAP_PT: f64 = 15.0;

/// Shift's step: the pointer's `s` rounded to 0.1 mm.
const FINE_NM: i128 = 100_000;

/// `cos 45° = sin 45°` in 10⁻⁹, rounded, as `ew_derive::layout::Quarter::quat` stores a quarter turn.
const HALF_SQRT2: i64 = 707_106_781;

/// The stored quaternion `[w, x, y, z]` (10⁻⁹) of `k` quarter turns about the surface normal, as
/// `ew_derive::layout::Quarter::quat` gives it.
pub fn quarter_quat(k: u8) -> [i64; 4] {
    match k % 4 {
        0 => [1_000_000_000, 0, 0, 0],
        1 => [HALF_SQRT2, 0, 0, HALF_SQRT2],
        2 => [0, 0, 0, 1_000_000_000],
        _ => [HALF_SQRT2, 0, 0, -HALF_SQRT2],
    }
}

/// The placing options of the tool options card (doc 16a §5.2, §6.20): the sticky mode and the grid step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LayoutOptions {
    /// The sticky mode (free by default).
    pub mode: SnapMode,
    /// The plate grid's step, in millimetres: 5 or 1.
    pub grid_mm: u8,
    // WP-S4.02 part 4 (D11): the draw tools' options.
    /// The rail part Draw rail draws with (its part copy's ID); `None`: the first one offered.
    pub rail_part: Option<Uuid>,
    /// The duct part Draw duct draws with; `None`: the first one offered.
    pub duct_part: Option<Uuid>,
    /// The duct width and height when the part gives none, nm (40 × 60 mm, then the last used).
    pub duct_size: (i128, i128),
}

impl Default for LayoutOptions {
    fn default() -> LayoutOptions {
        LayoutOptions {
            mode: SnapMode::Free,
            grid_mm: 5,
            rail_part: None,
            duct_part: None,
            duct_size: super::duct_draw::DEFAULT_DUCT_SIZE,
        }
    }
}

impl LayoutOptions {
    /// The grid step in nanometres.
    pub fn grid(self) -> i128 {
        i128::from(self.grid_mm.max(1)) * 1_000_000
    }
}

/// A mode's word: `free`, `push`, `swap` (the commands' `mode` and the describe text's).
pub fn mode_word(mode: SnapMode) -> &'static str {
    match mode {
        SnapMode::Free => "free",
        SnapMode::Push => "push",
        SnapMode::Swap => "swap",
    }
}

/// What a gesture places: an unplaced device, or a placement it moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// An unplaced device (`layout.place`).
    Device(Uuid),
    /// A placement (`layout.move`).
    Placement(Uuid),
}

/// A rectangle of the surface plane, `(u0, v0, u1, v1)` with `u0 ≤ u1`, `v0 ≤ v1`, surface nm.
pub type SurfaceBox = (i128, i128, i128, i128);

/// Where a gesture drops, as resolved.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Drop {
    /// On a rail.
    Rail {
        /// The rail's carrier ID.
        carrier: Uuid,
        /// The snapped left edge the command is given (`t.x = s_input + clip`).
        s_input: i128,
        /// The resolved left edge (`None` when refused).
        s_left: Option<i128>,
        /// The clip point's distance from the left edge.
        clip: i128,
        /// The quarter turns (0 or 2).
        turn: u8,
        /// The mode.
        mode: SnapMode,
        /// The moved neighbours' placements with their new left edges.
        moved: Vec<(Uuid, i128)>,
    },
    /// On the plate.
    Plate {
        /// The pose's `(u, v)`.
        at: (i128, i128),
        /// The quarter turns.
        turn: u8,
    },
}

/// The resolved state of a gesture at one pointer position.
#[derive(Clone, PartialEq, Debug)]
pub struct Resolved {
    /// The drop.
    pub drop: Drop,
    /// The target's word for the ghost: the rail's label, or `plate`.
    pub target: String,
    /// The ghost's footprint, surface nm.
    pub footprint: SurfaceBox,
    /// The moved neighbours' footprints at their new places, surface nm.
    pub neighbours: Vec<SurfaceBox>,
    /// A strip's brackets in the candidate arrangement, surface nm.
    pub brackets: Vec<SurfaceBox>,
    /// The tool's own refusal text, or whether the resolver or the fit table refuses (its text from the preview).
    pub local_text: Option<String>,
    /// Whether the drop is refused.
    pub refused: bool,
    /// The command a release commits (also for a drop the resolver refuses: its preview gives the text).
    pub command: Option<AnyCommand>,
    /// The same boxes in sheet nm, for the overlay.
    pub sheet: SheetGeom,
    /// On the plate, how far the snapping moved the drawn footprint from where the pointer put it, surface nm: the
    /// keyboard steps from where the ghost is drawn (FT-994). `(0, 0)` on a rail.
    pub drawn_shift: (i128, i128),
}

/// Where an arrow takes a floating ghost ([`LayoutGesture::step`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// The surface point the pointer moves to; the ghost snaps there as under the pointer.
    To((i128, i128)),
    /// The surface point the pointer moves to, the ghost resolved there without snapping (as with Shift): one grid
    /// step on the plate from where the ghost is drawn stays exactly one grid step (FT-994).
    Exact((i128, i128)),
    /// Nothing moves: blocked by this neighbour's placement, or by the rail end (`None`), as a selected placement's
    /// nudge is (FT-992).
    Blocked(Option<Uuid>),
}

/// Where a free nudge along a rail stops (the selected placement's arrows and a floating ghost's, FT-992).
pub(crate) enum Stop {
    /// At this left edge.
    Free(i128),
    /// Blocked by this neighbour's placement (`None`: the rail end).
    Blocked(Option<Uuid>),
}

/// A free nudge of `own` towards `target` (`sign` its direction): it stops where it would touch the next item at its
/// minimum gap, or at the rail end's gap; blocked when it cannot move at all. `own` without a placement (a device's
/// ghost) meets every item of the rail.
pub(crate) fn nudge_stop(items: &[RailItem], own: &RailItem, length: i128, target: i128, sign: i128) -> Stop {
    let width = own.width();
    let others = items
        .iter()
        .filter(|i| own.placement.is_none() || i.placement != own.placement);
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

/// A resolution's boxes in sheet nm (the overlay draws them without the page).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SheetGeom {
    /// The footprint's corners, in order around it.
    pub corners: [(i64, i64); 4],
    /// The footprint.
    pub footprint: Option<Rect>,
    /// The moved neighbours.
    pub neighbours: Vec<Rect>,
    /// A strip's brackets.
    pub brackets: Vec<Rect>,
}

/// One drag of a device or a placement on a panel page (D4).
#[derive(Clone, PartialEq, Debug)]
pub struct LayoutGesture {
    /// What it places.
    pub source: Source,
    /// The offset from the pointer to the anchor (a rail's clip point, the plate pose's `(u, v)`), surface nm: a
    /// placement keeps where it was grabbed; a device floats with its clip point (its centre on the plate) under the
    /// pointer.
    grab: Option<(i128, i128)>,
    /// The quarter turns on the plate (R while it floats) and on a rail (0 or 2, X).
    turn: u8,
    /// The current resolution.
    pub current: Option<Resolved>,
    /// The last command previewed and its refusal text from the host's preview.
    previewed: Option<(Value, Option<String>)>,
    /// The device's tag as a sentence names it.
    label: String,
    /// The chip's layout label: a placement's label on the page, a device's tag.
    chip: String,
    /// The pointer of the last update, surface nm (the keyboard steps from it).
    pointer: Option<(i128, i128)>,
}

impl LayoutGesture {
    /// A gesture of `source`; a placement's grab offset is set by [`LayoutGesture::grab_at`].
    pub fn new(source: Source, panel: &PanelTables) -> LayoutGesture {
        let turn = match source {
            Source::Placement(p) => panel.placement(p).map_or(0, |p| p.pose.2),
            Source::Device(_) => 0,
        };
        let (label, chip) = match source {
            Source::Device(d) => {
                let l = panel
                    .unplaced_device(d)
                    .map(|u| u.label.clone())
                    .unwrap_or_default();
                (l.clone(), l)
            }
            Source::Placement(p) => panel
                .placement(p)
                .map(|p| (p.tag.clone(), p.label.clone()))
                .unwrap_or_default(),
        };
        LayoutGesture {
            source,
            grab: None,
            turn,
            current: None,
            previewed: None,
            label,
            chip,
            pointer: None,
        }
    }

    /// Grabs a placement at the sheet point `at`: the anchor keeps its offset from the pointer.
    pub fn grab_at(&mut self, at: (i64, i64), cx: &ToolCx<'_>) {
        let (Some(panel), Source::Placement(p)) = (cx.page.panel.as_deref(), self.source) else {
            return;
        };
        let Some(row) = panel.placement(p) else { return };
        let ptr = surface_of_sheet(cx, panel, at);
        let anchor = match row.carrier.and_then(|c| panel.carrier(c)).and_then(|c| c.axis) {
            Some(axis) => axis.point_at(i128::from(row.pose.0)),
            None => (i128::from(row.pose.0), i128::from(row.pose.1)),
        };
        self.grab = Some((anchor.0 - ptr.0, anchor.1 - ptr.1));
    }

    /// Turns the floating footprint: R a quarter turn on the plate, X a half turn on a rail. `false` when the key does
    /// not apply to the current target (R on a rail, X on the plate).
    pub fn turn_key(&mut self, key: ToolKey) -> bool {
        let on_rail = matches!(self.current.as_ref().map(|r| &r.drop), Some(Drop::Rail { .. }));
        match key {
            ToolKey::RotateCw if !on_rail => {
                self.turn = (self.turn + 1) % 4;
                true
            }
            ToolKey::MirrorX if on_rail => {
                self.turn = if self.turn == 2 { 0 } else { 2 };
                true
            }
            _ => false,
        }
    }

    /// Resolves the gesture with the pointer at the sheet point `at`; `None` without panel tables or a source row.
    pub fn update(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> Option<&Resolved> {
        let panel = cx.page.panel.as_deref()?;
        self.pointer = Some(surface_of_sheet(cx, panel, at));
        let mut resolved = resolve_at(self, panel, at, mods, cx)?;
        let (u0, v0, u1, v1) = resolved.footprint;
        let map = |p: (i128, i128)| sheet_of_surface(cx, panel, p);
        resolved.sheet = SheetGeom {
            corners: [map((u0, v0)), map((u1, v0)), map((u1, v1)), map((u0, v1))],
            footprint: Some(sheet_rect(cx, panel, resolved.footprint)),
            neighbours: resolved
                .neighbours
                .iter()
                .map(|b| sheet_rect(cx, panel, *b))
                .collect(),
            brackets: resolved
                .brackets
                .iter()
                .map(|b| sheet_rect(cx, panel, *b))
                .collect(),
        };
        self.current = Some(resolved);
        // A new command (or none) forgets the last preview's text; the same command keeps it.
        if let Some((json, text)) = &self.previewed {
            let same = self
                .current
                .as_ref()
                .and_then(|r| r.command.as_ref())
                .is_some_and(|c| c.params == *json);
            if same
                && text.is_some()
                && let Some(r) = self.current.as_mut()
            {
                r.refused = true;
            }
        }
        self.current.as_ref()
    }

    /// The command the host should preview: the current command when it differs from the last one previewed.
    pub fn wants_preview(&mut self) -> Option<AnyCommand> {
        let cmd = self.current.as_ref()?.command.clone()?;
        if self
            .previewed
            .as_ref()
            .is_some_and(|(json, _)| *json == cmd.params)
        {
            return None;
        }
        self.previewed = Some((cmd.params.clone(), None));
        Some(cmd)
    }

    /// The host's preview of the current command: a refusal's text refuses the ghost (ruling 1).
    pub fn on_preview(&mut self, info: &PreviewInfo) {
        // The command's own sentence, without the command name the error's display leads with.
        let name = self
            .current
            .as_ref()
            .and_then(|r| r.command.as_ref())
            .map(|c| format!("{}: ", c.name));
        let text = (!info.ok).then(|| {
            let m = info.message.clone().unwrap_or_default();
            match name.as_deref().and_then(|n| m.strip_prefix(n)) {
                Some(rest) => rest.to_owned(),
                None => m,
            }
        });
        if let Some((_, t)) = self.previewed.as_mut() {
            t.clone_from(&text);
        }
        if let Some(r) = self.current.as_mut()
            && text.is_some()
        {
            r.refused = true;
        }
    }

    /// The refusal's text: the tool's own, else the preview's.
    pub fn refusal(&self) -> Option<String> {
        let r = self.current.as_ref()?;
        if !r.refused {
            return None;
        }
        r.local_text
            .clone()
            .or_else(|| self.previewed.as_ref().and_then(|(_, t)| t.clone()))
            .or_else(|| Some(String::new()))
    }

    /// The command a release commits: none for a refused drop.
    pub fn commit(&self) -> Option<AnyCommand> {
        let r = self.current.as_ref()?;
        if r.refused {
            return None;
        }
        r.command.clone()
    }

    /// The ghost (doc 16a §7.3): the footprint in `canvas.ghost`, a strip's brackets, each moved neighbour as a dashed
    /// outline at its new place, one chip with the layout label; a refused drop shows only its chip with the refusal's
    /// text here (its outline and cross are [`LayoutGesture::conflict_lines`]).
    pub fn overlay(&self) -> Overlay {
        let Some(r) = self.current.as_ref() else {
            return Overlay::default();
        };
        let Some(top) = r.sheet.footprint else {
            return Overlay::default();
        };
        let mut items = Vec::new();
        let refusal = self.refusal();
        if refusal.is_none() {
            items.push(OverlayItem::Rect {
                r: top,
                style: Style::solid(Token::Ghost),
            });
            for b in &r.sheet.brackets {
                items.push(OverlayItem::Rect {
                    r: *b,
                    style: Style::solid(Token::Ghost),
                });
            }
            for n in &r.sheet.neighbours {
                items.push(OverlayItem::Rect {
                    r: *n,
                    style: Style::dashed(Token::RubberBand),
                });
            }
        }
        let at = ((top.min().x.0 + top.max().x.0) / 2, top.min().y.0);
        let text = refusal
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| self.chip.clone());
        items.push(OverlayItem::Label {
            at,
            text,
            anchor: Anchor::Above,
        });
        Overlay::new(items)
    }

    /// A refused drop's outline and its diagonal cross, in sheet nm; empty otherwise.
    pub fn conflict_lines(&self) -> Vec<Vec<(i64, i64)>> {
        let Some(r) = self.current.as_ref() else {
            return Vec::new();
        };
        if self.refusal().is_none() || r.sheet.footprint.is_none() {
            return Vec::new();
        }
        let [a, b, c, d] = r.sheet.corners;
        vec![vec![a, b, c, d, a], vec![a, c], vec![b, d]]
    }

    /// The ghost of the transcript and the describe text.
    pub fn ghost(&self) -> Option<Ghost> {
        let r = self.current.as_ref()?;
        let (mode, at, moved) = match &r.drop {
            Drop::Rail {
                s_input,
                s_left,
                mode,
                moved,
                ..
            } => (mode_word(*mode), (s_left.unwrap_or(*s_input), None), moved.len()),
            Drop::Plate { at, .. } => ("free", (at.0, Some(at.1)), 0),
        };
        Some(Ghost::Layout {
            device: self.label.clone(),
            carrier: r.target.clone(),
            mode,
            at,
            moved,
            refused: self.refusal(),
        })
    }

    /// The keyboard's step of the floating ghost (doc 12 §12.4, "arrows move by snap step"): where the pointer moves
    /// for the arrow `(dx, dy)` (page directions). Along a rail, the next snap target in that direction (a neighbour
    /// touched at its minimum gap, a rail end at its gap, the next grid point), stopped as a selected placement's free
    /// nudge is (FT-992): in free mode at a neighbour and at the rail ends, in push and swap mode at the rail ends;
    /// across it, the nearest parallel rail on that side at the same clip point, else one grid step; on the plate, one
    /// grid step from where the ghost is drawn (FT-994). `None` when nothing floats or nothing lies further that way.
    pub fn step(&self, panel: &PanelTables, (dx, dy): (i64, i64), grid: i128) -> Option<Step> {
        let r = self.current.as_ref()?;
        let ptr = self.pointer?;
        let grid = grid.max(1);
        // The page's y grows downwards, the surface's v upwards.
        let way = (i128::from(dx), -i128::from(dy));
        let Drop::Rail {
            carrier,
            s_input,
            clip,
            mode,
            ..
        } = &r.drop
        else {
            let (du, dv) = r.drawn_shift;
            return Some(Step::Exact((
                ptr.0 + du + way.0 * grid,
                ptr.1 + dv + way.1 * grid,
            )));
        };
        let c = panel.carrier(*carrier)?;
        let (axis, length) = (c.axis?, c.length?);
        let along = way.0 * axis.dir.0 + way.1 * axis.dir.1;
        if along != 0 {
            let item = self.rail_item(panel)?;
            let width = item.width();
            let s = *s_input;
            let mut cands = vec![start_gap(&item), length - end_gap(&item) - width];
            for i in c
                .items
                .iter()
                .filter(|i| item.placement.is_none() || i.placement != item.placement)
            {
                cands.push(i.s0 - min_gap(&item, i) - width);
                cands.push(i.s1 + min_gap(i, &item));
            }
            let origin = axis.start.0 * axis.dir.0 + axis.start.1 * axis.dir.1;
            let x = origin + s;
            cands.push(if along > 0 {
                (x.div_euclid(grid) + 1) * grid - origin
            } else {
                (x - 1).div_euclid(grid) * grid - origin
            });
            let next = if along > 0 {
                cands.into_iter().filter(|v| *v > s).min()?
            } else {
                cands.into_iter().filter(|v| *v < s).max()?
            };
            // The stop: a free ghost meets its neighbours, a pushing or swapping one passes them (the resolver moves
            // them); every ghost meets the rail ends.
            let own = RailItem {
                s0: s,
                s1: s + width,
                ..item
            };
            let met: &[RailItem] = if *mode == SnapMode::Free { &c.items } else { &[] };
            return Some(match nudge_stop(met, &own, length, next, along.signum()) {
                Stop::Free(to) => Step::To(axis.point_at(to + clip)),
                Stop::Blocked(by) => Step::Blocked(by),
            });
        }
        // Across: the nearest parallel rail on that side.
        let here = axis.point_at(s_input + clip);
        let side = |p: (i128, i128)| (p.0 - here.0) * way.0 + (p.1 - here.1) * way.1;
        let other = panel
            .carriers
            .iter()
            .filter(|o| o.id != c.id)
            .filter_map(|o| Some((o.axis?, o.length?)))
            .filter(|(a, _)| a.dir.0 * axis.dir.1 == a.dir.1 * axis.dir.0)
            .filter(|(a, _)| side(a.start) > 0)
            .min_by_key(|(a, _)| side(a.start));
        Some(Step::To(match other {
            Some((a, len)) => a.point_at(a.project(here).0.clamp(0, len)),
            None => (ptr.0 + way.0 * grid, ptr.1 + way.1 * grid),
        }))
    }

    /// The rail item of the source at the current turn, at `s0 = 0`.
    fn rail_item(&self, panel: &PanelTables) -> Option<RailItem> {
        let (e0, e2, strip_w, locked, placement) = match self.source {
            Source::Device(d) => {
                let u = panel.unplaced_device(d)?;
                (u.extent, u.extent_flipped, u.bracket_width, false, None)
            }
            Source::Placement(p) => {
                let r = panel.placement(p)?;
                (
                    r.extent,
                    r.extent_flipped,
                    r.bracket_width,
                    r.locked,
                    Some(r.placement),
                )
            }
        };
        let ext = if self.turn == 2 { e2 } else { e0 };
        Some(RailItem {
            s0: 0,
            s1: ext.width,
            strip: ext.strip,
            w: if ext.strip { strip_w } else { None },
            pad: ext.pad,
            locked,
            placement,
        })
    }

    /// The poses the ghost predicts, `(placement or device ID, (t.x, t.y, quarter turns))`: the dropped item first
    /// (its placement, or its device for a new one), then each moved neighbour, as the command writes them (R1).
    pub fn predicted_poses(&self, panel: &PanelTables) -> Vec<(Uuid, (i64, i64, u8))> {
        let Some(r) = self.current.as_ref() else {
            return Vec::new();
        };
        let id = match self.source {
            Source::Device(d) => d,
            Source::Placement(p) => p,
        };
        match &r.drop {
            Drop::Rail {
                carrier,
                s_left: Some(s),
                clip,
                turn,
                moved,
                ..
            } => {
                let mut out = vec![(id, (clamp64(s + clip), 0, *turn))];
                let items = panel
                    .carrier(*carrier)
                    .map(|c| c.items.clone())
                    .unwrap_or_default();
                for (p, s0) in moved {
                    let (Some(row), Some(old)) = (
                        panel.placement(*p),
                        items
                            .iter()
                            .find(|i| i.placement.is_some_and(|x| x.as_uuid() == *p)),
                    ) else {
                        continue;
                    };
                    out.push((
                        *p,
                        (clamp64(i128::from(row.pose.0) + (s0 - old.s0)), 0, row.pose.2),
                    ));
                }
                out
            }
            Drop::Rail { .. } => Vec::new(),
            Drop::Plate { at, turn } => vec![(id, (clamp64(at.0), clamp64(at.1), *turn))],
        }
    }
}

/// The surface point of the sheet point `at` (the frame's map, then the inverse projection).
pub fn surface_of_sheet(cx: &ToolCx<'_>, panel: &PanelTables, at: (i64, i64)) -> (i128, i128) {
    panel.to_surface(cx.snapper.sheet_to_page(at))
}

/// The sheet point of the surface point `p`.
pub fn sheet_of_surface(cx: &ToolCx<'_>, panel: &PanelTables, p: (i128, i128)) -> (i64, i64) {
    cx.snapper.page_to_sheet(panel.to_page(p))
}

/// The sheet rectangle of a surface box.
pub(crate) fn sheet_rect(cx: &ToolCx<'_>, panel: &PanelTables, (u0, v0, u1, v1): SurfaceBox) -> Rect {
    let a = sheet_of_surface(cx, panel, (u0, v0));
    let b = sheet_of_surface(cx, panel, (u1, v1));
    Rect::from_points(Pt2::new(Nm(a.0), Nm(a.1)), Pt2::new(Nm(b.0), Nm(b.1)))
}

/// The surface box of a footprint rectangle.
pub fn surface_box(r: Rect) -> SurfaceBox {
    (
        i128::from(r.min().x.0),
        i128::from(r.min().y.0),
        i128::from(r.max().x.0),
        i128::from(r.max().y.0),
    )
}

/// `v` rounded to the nearest multiple of `step`, halves away from zero.
pub fn round_to(v: i128, step: i128) -> i128 {
    if step <= 0 {
        return v;
    }
    let q = v.div_euclid(step);
    let r = v.rem_euclid(step);
    if r * 2 > step || (r * 2 == step && v > 0) {
        (q + 1) * step
    } else {
        q * step
    }
}

/// How many surface nm one sheet nm stands for near `at` (the frame's factor and the projection's scale).
pub(crate) fn surface_per_sheet(cx: &ToolCx<'_>, panel: &PanelTables, at: (i64, i64)) -> f64 {
    let a = surface_of_sheet(cx, panel, at);
    let b = surface_of_sheet(cx, panel, (at.0.saturating_add(1_000_000), at.1));
    let d = ((b.0 - a.0).abs() + (b.1 - a.1).abs()) as f64;
    d / 1_000_000.0
}

/// The perpendicular of an axis direction (the surface's `v` for a horizontal rail).
fn normal(axis: &PanelAxis) -> (i128, i128) {
    (-axis.dir.1, axis.dir.0)
}

/// The box spanned along `axis` from `s0` to `s1` and across from `t0` to `t1`.
fn rail_box(axis: &PanelAxis, (s0, s1): (i128, i128), (t0, t1): (i128, i128)) -> SurfaceBox {
    let n = normal(axis);
    let p = |s: i128, t: i128| {
        let a = axis.point_at(s);
        (a.0 + n.0 * t, a.1 + n.1 * t)
    };
    let pts = [p(s0, t0), p(s1, t0), p(s1, t1), p(s0, t1)];
    let u0 = pts.iter().map(|p| p.0).min().unwrap_or(0);
    let u1 = pts.iter().map(|p| p.0).max().unwrap_or(0);
    let v0 = pts.iter().map(|p| p.1).min().unwrap_or(0);
    let v1 = pts.iter().map(|p| p.1).max().unwrap_or(0);
    (u0, v0, u1, v1)
}

/// The across range `(t0, t1)` of a footprint relative to `axis`.
fn across_of(axis: &PanelAxis, fp: SurfaceBox) -> (i128, i128) {
    let n = normal(axis);
    let t = |p: (i128, i128)| (p.0 - axis.start.0) * n.0 + (p.1 - axis.start.1) * n.1;
    let a = t((fp.0, fp.1));
    let b = t((fp.2, fp.3));
    (a.min(b), a.max(b))
}

/// Resolves `g` at the sheet point `at` (see the module documentation).
fn resolve_at(
    g: &LayoutGesture,
    panel: &PanelTables,
    at: (i64, i64),
    mods: Mods,
    cx: &ToolCx<'_>,
) -> Option<Resolved> {
    let label = g.label.clone();
    // The source's data.
    let (extent0, extent2, strip_w, size, mount, mount_label, placement, locked, footprint) = match g.source {
        Source::Device(d) => {
            let u = panel.unplaced_device(d)?;
            (
                u.extent,
                u.extent_flipped,
                u.bracket_width,
                u.size,
                u.mount_kind.clone(),
                u.mount_label.clone(),
                None,
                false,
                None,
            )
        }
        Source::Placement(p) => {
            let row = panel.placement(p)?;
            (
                row.extent,
                row.extent_flipped,
                row.bracket_width,
                None,
                row.mount_kind.clone(),
                row.mount_label.clone(),
                Some(row),
                row.locked,
                Some(surface_box(row.footprint)),
            )
        }
    };
    let ptr = surface_of_sheet(cx, panel, at);
    let anchor_ptr = match g.grab {
        Some((du, dv)) => (ptr.0 + du, ptr.1 + dv),
        None => ptr,
    };
    let reach_sheet = cx.nm_of_points(LAYOUT_SNAP_PT);
    let k = surface_per_sheet(cx, panel, at);
    let reach = (reach_sheet * k).min(9.0e30) as i128;
    let rail_profile = mount.as_ref().and_then(Profile::of);
    // The nearest rail within reach whose axis the pointer projects onto between its ends. A mount kind that is no
    // rail kind and is known (screw-mounted, a busbar adapter, a door cut-out) always targets the plate.
    let mut best: Option<(i128, usize)> = None;
    {
        for (i, c) in panel.carriers.iter().enumerate() {
            let (Some(axis), Some(len), Some(profile)) = (c.axis, c.length, c.profile.as_ref()) else {
                continue;
            };
            if let Some(m) = mount.as_ref()
                && Profile::of(m).is_none()
                && fits_rail(Some(m), profile) != Fit::Unknown
            {
                continue;
            }
            let (s, _) = axis.project(anchor_ptr);
            if s < 0 || s > len {
                continue;
            }
            let a = sheet_of_surface(cx, panel, axis.point_at(0));
            let b = sheet_of_surface(cx, panel, axis.point_at(len));
            let d2 = super::segment_dist2(at, a, b);
            let r = reach_sheet.floor().min(9.0e15) as i128;
            if d2 <= r * r && best.is_none_or(|(bd, _)| d2 < bd) {
                best = Some((d2, i));
            }
        }
    }
    let grid = cx.layout.grid();
    match best {
        Some((_, ci)) => {
            let c = &panel.carriers[ci];
            let axis = c.axis?;
            let length = c.length?;
            let flipped = g.turn == 2;
            let turn = if flipped { 2 } else { 0 };
            let ext = if flipped { extent2 } else { extent0 };
            let moved_item = RailItem {
                s0: 0,
                s1: ext.width,
                strip: ext.strip,
                w: if ext.strip { strip_w } else { None },
                pad: ext.pad,
                locked,
                placement: placement.map(|p| p.placement),
            };
            let width = ext.width;
            let (s_ptr, _) = axis.project(anchor_ptr);
            // The mode, and ruling 4's slot default.
            let slot = match g.source {
                Source::Device(d) => panel
                    .unplaced_device(d)
                    .and_then(|u| u.slots.iter().find(|(r, _)| *r == c.id).map(|(_, s)| *s)),
                Source::Placement(_) => None,
            };
            let free_choice = !mods.shift && !mods.ctrl && !mods.alt && cx.layout.mode == SnapMode::Free;
            let (mode, s_input) = if let (Some(s), true) = (slot, free_choice) {
                (SnapMode::Push, s)
            } else {
                let mode = if mods.ctrl {
                    SnapMode::Push
                } else if mods.alt {
                    SnapMode::Swap
                } else {
                    cx.layout.mode
                };
                let s = if mods.shift {
                    round_to(s_ptr, FINE_NM) - ext.clip
                } else {
                    snap_left(
                        &c.items,
                        length,
                        &moved_item,
                        s_ptr - ext.clip,
                        reach,
                        grid,
                        axis.start.0 * axis.dir.0 + axis.start.1 * axis.dir.1,
                    )
                };
                (mode, s)
            };
            // Across the rail: a placement keeps its offset from its rail's axis, a device is centred on it.
            let across = match (placement, footprint) {
                (Some(row), Some(fp)) => {
                    match row.carrier.and_then(|x| panel.carrier(x)).and_then(|x| x.axis) {
                        Some(old) => across_of(&old, fp),
                        None => {
                            let a = (fp.3 - fp.1).max(fp.2 - fp.0);
                            (-a / 2, a - a / 2)
                        }
                    }
                }
                _ => {
                    let a = size.map_or(width, |(_, h)| h);
                    (-a / 2, a - a / 2)
                }
            };
            let fit = c
                .profile
                .as_ref()
                .map_or(Fit::Unknown, |profile| fits_rail(mount.as_ref(), profile));
            let result = resolve(&c.items, length, moved_item, s_input, mode);
            let tx = s_input + ext.clip;
            let mut params = match g.source {
                Source::Device(d) => json!({ "device": d.to_string() }),
                Source::Placement(p) => json!({ "placement": p.to_string() }),
            };
            params["carrier"] = json!({ "carrier": c.id.to_string() });
            params["pose"] = json!({ "t": [clamp64(tx), 0, 0], "q": quarter_quat(turn) });
            params["mode"] = json!(mode_word(mode));
            let name = match g.source {
                Source::Device(_) => "layout.place",
                Source::Placement(_) => "layout.move",
            };
            let command = Some(AnyCommand {
                name: name.into(),
                params,
            });
            let (s_left, moved, neighbours, brackets_box) = match &result {
                Ok(res) => {
                    let mut neighbours = Vec::new();
                    let mut moved = Vec::new();
                    for (p, s0) in &res.moved {
                        let pid = p.as_uuid();
                        moved.push((pid, *s0));
                        if let (Some(row), Some(old)) = (
                            panel.placement(pid),
                            c.items.iter().find(|i| i.placement == Some(*p)),
                        ) {
                            let d = s0 - old.s0;
                            let fp = surface_box(row.footprint);
                            neighbours.push((
                                fp.0 + axis.dir.0 * d,
                                fp.1 + axis.dir.1 * d,
                                fp.2 + axis.dir.0 * d,
                                fp.3 + axis.dir.1 * d,
                            ));
                        }
                    }
                    // A strip's brackets in the candidate arrangement.
                    let mut bracket_boxes = Vec::new();
                    if moved_item.strip {
                        let mut arrangement: Vec<RailItem> = c
                            .items
                            .iter()
                            .filter(|i| moved_item.placement.is_none() || i.placement != moved_item.placement)
                            .map(|i| {
                                let shift = res
                                    .moved
                                    .iter()
                                    .find(|(p, _)| i.placement == Some(*p))
                                    .map_or(0, |(_, s0)| s0 - i.s0);
                                RailItem {
                                    s0: i.s0 + shift,
                                    s1: i.s1 + shift,
                                    ..*i
                                }
                            })
                            .collect();
                        arrangement.push(RailItem {
                            s0: res.s_left,
                            s1: res.s_left + width,
                            ..moved_item
                        });
                        let half = c.width.unwrap_or(35_000_000) / 2;
                        for b in brackets(&arrangement) {
                            bracket_boxes.push(rail_box(&axis, (b.s0, b.s1), (-half, half)));
                        }
                    }
                    (Some(res.s_left), moved, neighbours, bracket_boxes)
                }
                Err(_) => (None, Vec::new(), Vec::new(), Vec::new()),
            };
            let shown = s_left.unwrap_or(s_input);
            let footprint = rail_box(&axis, (shown, shown + width), across);
            Some(Resolved {
                drop: Drop::Rail {
                    carrier: c.id,
                    s_input,
                    s_left,
                    clip: ext.clip,
                    turn,
                    mode,
                    moved,
                },
                target: c.label.clone(),
                footprint,
                neighbours,
                brackets: brackets_box,
                local_text: None,
                refused: result.is_err() || fit == Fit::DoesNotFit,
                command,
                sheet: SheetGeom::default(),
                drawn_shift: (0, 0),
            })
        }
        None => {
            // The plate: the footprint (its centre under the pointer for a device, its grab for a placement) snapped.
            let turn = g.turn % 4;
            let (w, h) = match (size, footprint) {
                (Some(s), _) => s,
                (None, Some(fp))
                    if placement.is_some_and(|p| !p.unknown) && turn == placement.map_or(0, |p| p.pose.2) =>
                {
                    (fp.2 - fp.0, fp.3 - fp.1)
                }
                _ => (extent0.width, extent0.width),
            };
            let unknown = size.is_none() && placement.is_none_or(|p| p.unknown);
            // The footprint's corners relative to the anchor (the pose's (u, v)), as `placement_geometry` places it.
            let rot = |(x, y): (i128, i128)| match turn {
                0 => (x, y),
                1 => (-y, x),
                2 => (-x, -y),
                _ => (y, -x),
            };
            let (lo, hi) = if unknown {
                ((-w / 2, -w / 2), (w - w / 2, w - w / 2))
            } else {
                let a = rot((0, 0));
                let b = rot((w, h));
                ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
            };
            let anchor_guess = match g.grab {
                Some(_) => anchor_ptr,
                None => (ptr.0 - (lo.0 + hi.0) / 2, ptr.1 - (lo.1 + hi.1) / 2),
            };
            let (bw, bh) = (hi.0 - lo.0, hi.1 - lo.1);
            let min0 = (anchor_guess.0 + lo.0, anchor_guess.1 + lo.1);
            let others: Vec<SurfaceBox> = panel
                .placements
                .iter()
                .filter(|p| placement.is_none_or(|own| own.id != p.id))
                .map(|p| surface_box(p.footprint))
                .chain(
                    panel
                        .carriers
                        .iter()
                        .filter(|c| c.kind == "duct")
                        .flat_map(|c| c.outline.iter().map(|r| surface_box(*r))),
                )
                .collect();
            let snap_axis = |m: i128, size: i128, pick: fn(&SurfaceBox) -> (i128, i128)| -> i128 {
                if mods.shift {
                    return round_to(m, FINE_NM);
                }
                let mut best = (round_to(m, grid) - m).abs();
                let mut out = round_to(m, grid);
                for o in &others {
                    let (a, b) = pick(o);
                    for target in [a, (a + b) / 2, b] {
                        for (edge, off) in [(m, 0), (m + size / 2, size / 2), (m + size, size)] {
                            let d = (target - edge).abs();
                            if d <= reach && d < best {
                                best = d;
                                out = target - off;
                            }
                        }
                    }
                }
                out
            };
            let u_min = snap_axis(min0.0, bw, |o| (o.0, o.2));
            let v_min = snap_axis(min0.1, bh, |o| (o.1, o.3));
            let anchor = (u_min - lo.0, v_min - lo.1);
            let footprint = (u_min, v_min, u_min + bw, v_min + bh);
            let local_text =
                rail_profile.map(|_| strings::format(key!("layout-off-rail"), &[&label, &mount_label]));
            let mut params = match g.source {
                Source::Device(d) => json!({ "device": d.to_string() }),
                Source::Placement(p) => json!({ "placement": p.to_string() }),
            };
            params["carrier"] = json!({ "surface": panel.surface.to_string() });
            params["pose"] = json!({
                "t": [clamp64(anchor.0), clamp64(anchor.1), 0],
                "q": quarter_quat(turn),
            });
            params["mode"] = json!("free");
            let name = match g.source {
                Source::Device(_) => "layout.place",
                Source::Placement(_) => "layout.move",
            };
            Some(Resolved {
                drop: Drop::Plate { at: anchor, turn },
                target: "plate".to_owned(),
                footprint,
                neighbours: Vec::new(),
                brackets: Vec::new(),
                refused: local_text.is_some(),
                command: local_text.is_none().then(|| AnyCommand {
                    name: name.into(),
                    params,
                }),
                local_text,
                sheet: SheetGeom::default(),
                drawn_shift: (u_min - min0.0, v_min - min0.1),
            })
        }
    }
}

/// D4's target snapping of a footprint's left edge on a rail: the candidate nearest to `s_left` among (1) the
/// positions where it touches a neighbour at its minimum gap and (2) the rail ends at their gaps, each within
/// `reach`, and (4) the nearest grid point along the axis (`origin` is the rail start's coordinate along its
/// direction, so the grid is the plate's); at equal distance the lower class wins. Target (3), the column alignment,
/// is de-scoped (§13).
pub fn snap_left(
    items: &[RailItem],
    length: i128,
    moved: &RailItem,
    s_left: i128,
    reach: i128,
    grid: i128,
    origin: i128,
) -> i128 {
    let width = moved.width();
    let mut cands: Vec<(i128, u8, i128)> = Vec::new();
    let mut push = |class: u8, s: i128| {
        let d = (s - s_left).abs();
        if class == 4 || d <= reach {
            cands.push((d, class, s));
        }
    };
    for i in items
        .iter()
        .filter(|i| moved.placement.is_none() || i.placement != moved.placement)
    {
        push(1, i.s0 - min_gap(moved, i) - width);
        push(1, i.s1 + min_gap(i, moved));
    }
    push(2, start_gap(moved));
    push(2, length - end_gap(moved) - width);
    push(4, round_to(origin + s_left, grid) - origin);
    cands.sort();
    cands.first().map_or(s_left, |c| c.2)
}

/// The layout place tool (WP-S4.02 part 3): floats an unplaced device over a panel page and places it on release.
#[derive(Clone, Debug)]
pub struct LayoutPlaceTool {
    device: Option<Uuid>,
    gesture: Option<LayoutGesture>,
    /// The last pointer or cursor point and the modifiers held.
    last: Option<((i64, i64), Mods)>,
}

impl LayoutPlaceTool {
    /// The tool armed with `device` (`None`: it floats nothing until a device is chosen).
    pub fn new(device: Option<Uuid>) -> LayoutPlaceTool {
        LayoutPlaceTool {
            device,
            gesture: None,
            last: None,
        }
    }

    /// The device it places.
    pub fn device(&self) -> Option<Uuid> {
        self.device
    }

    /// The gesture, while the ghost floats.
    pub fn gesture(&self) -> Option<&LayoutGesture> {
        self.gesture.as_ref()
    }

    /// Moves the ghost to `at`: the overlay, or the preview of a new command (ruling 1).
    fn track(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        self.last = Some((at, mods));
        let Some(device) = self.device else {
            return ToolOutput::None;
        };
        let Some(panel) = cx.page.panel.as_deref() else {
            return ToolOutput::None;
        };
        let g = self
            .gesture
            .get_or_insert_with(|| LayoutGesture::new(Source::Device(device), panel));
        g.update(at, mods, cx);
        track_output(g)
    }
}

/// What a gesture answers after an update: the preview of a new command, else its overlay.
pub fn track_output(g: &mut LayoutGesture) -> ToolOutput {
    match g.wants_preview() {
        Some(cmd) => ToolOutput::Preview(Box::new(cmd)),
        None => ToolOutput::Transient(g.overlay()),
    }
}

impl Tool for LayoutPlaceTool {
    fn name(&self) -> &'static str {
        "layout_place"
    }

    fn state(&self) -> &'static str {
        match (self.device, &self.gesture) {
            (None, _) => "idle",
            (Some(_), None) => "armed",
            (Some(_), Some(_)) => "placing",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        if cx.page.panel.is_none() {
            return match ev {
                CanvasEvent::Press { .. } | CanvasEvent::Activate => {
                    ToolOutput::Refused(key!("reason-no-panel-page"))
                }
                _ => ToolOutput::None,
            };
        }
        match ev {
            CanvasEvent::PointerMove { at, mods } => self.track(*at, *mods, cx),
            CanvasEvent::CursorMove { at } => self.track(*at, Mods::default(), cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                if self.device.is_none() {
                    return ToolOutput::Refused(key!("reason-no-unplaced-device"));
                }
                self.track(*at, *mods, cx)
            }
            CanvasEvent::Release {
                at,
                button: Button::Primary,
                mods,
            } => self.release(*at, *mods, cx),
            CanvasEvent::Activate => match self.last {
                Some((at, mods)) => self.release(at, mods, cx),
                None => ToolOutput::None,
            },
            CanvasEvent::Key(key @ (ToolKey::RotateCw | ToolKey::MirrorX)) => {
                let Some((at, mods)) = self.last else {
                    return ToolOutput::None;
                };
                let Some(g) = self.gesture.as_mut() else {
                    return ToolOutput::None;
                };
                if !g.turn_key(*key) {
                    return ToolOutput::Refused(key!("reason-layout-turn"));
                }
                g.update(at, mods, cx);
                track_output(g)
            }
            // FT-993: Escape drops the ghost and the tool gives way to Select (the host's answer to `None`), as the
            // schematic Place tool's Escape does.
            CanvasEvent::Escape => {
                self.gesture = None;
                ToolOutput::None
            }
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> super::Hint {
        match self.device {
            Some(_) => super::Hint::new(key!("hint-layout-place"), &[super::ESCAPE]),
            None => super::Hint::plain(key!("hint-layout-idle")),
        }
    }

    fn ghost(&self) -> Option<Ghost> {
        self.gesture.as_ref()?.ghost()
    }

    fn on_preview(&mut self, info: &PreviewInfo) -> Option<Overlay> {
        let g = self.gesture.as_mut()?;
        g.on_preview(info);
        Some(g.overlay())
    }

    fn on_commit(&mut self, done: bool) {
        if done {
            self.gesture = None;
            self.device = None;
        }
    }

    fn conflict_lines(&self) -> Vec<Vec<(i64, i64)>> {
        self.gesture
            .as_ref()
            .map(LayoutGesture::conflict_lines)
            .unwrap_or_default()
    }

    fn layout_gesture(&self) -> Option<&LayoutGesture> {
        self.gesture.as_ref()
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

impl LayoutPlaceTool {
    /// Release: commits the ghost's command; a refused ghost commits nothing.
    fn release(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        let Some(device) = self.device else {
            return ToolOutput::None;
        };
        let Some(panel) = cx.page.panel.as_deref() else {
            return ToolOutput::None;
        };
        let g = self
            .gesture
            .get_or_insert_with(|| LayoutGesture::new(Source::Device(device), panel));
        g.update(at, mods, cx);
        match g.commit() {
            Some(cmd) => ToolOutput::Commit(Box::new(cmd)),
            None => ToolOutput::Refused(key!("reason-layout-refused")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::round_to;

    /// Halves round away from zero, as `round_to`'s doc says (cycle-42 review part 3, nit 4).
    #[test]
    fn halves_round_away_from_zero() {
        assert_eq!(round_to(5, 10), 10);
        assert_eq!(round_to(-5, 10), -10);
        assert_eq!(round_to(4, 10), 0);
        assert_eq!(round_to(-4, 10), 0);
        assert_eq!(round_to(6, 10), 10);
        assert_eq!(round_to(-6, 10), -10);
        assert_eq!(round_to(-15, 10), -20);
        assert_eq!(round_to(7, 0), 7, "no step leaves the value");
    }
}
