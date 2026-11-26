// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Supply tool (the Draw tab's Supply): the supply marks of what comes in from the mains (L, N, PE; L1, L2,
//! L3, N, PE; …) float as one group under the pointer or the grid cursor, one under the other two grid steps apart,
//! each drawn with the target form of an interruption point and its name.
//!
//! State `floating` (not sticky). `PointerMove` or `CursorMove` → `Transient`; `Press` or `Activate` → the host step
//! [`HostStep::SupplyPlace`]: the host places the points, marks them as where the supply comes in (`ip.supply_in`) in
//! one undo step, and their Potential panel opens. `Escape` answers nothing, so the Escape chain ends the tool.
//!
//! **A mark on a pin joins it**, as a point of the IP tool does: the pointer within the catch distance of a pin
//! ([`super::snap::Snapper::snap_cp`]) puts the CP of the first mark that comes so near on it, the group on the grid
//! with it; a mark whose CP then lies on a pin joins it with `connection.draw_to_ip`, and the ghost names each pin a
//! mark joins ([`Ghost::Supply`](super::Ghost::Supply)). What a mark may join is the IP tool's rule
//! ([`super::ip::drop_on_cp`]): on another interruption point, or with its arrow into a drawn box, the click is refused
//! in that rule's words ([`SupplyTool::refusal`]); on a box's upper or lower side, off which the IP tool stands its
//! point with a wire, a mark stands free. Alt places the marks free.

use std::sync::Arc;

use super::{
    Anchor, Button, CanvasEvent, Ghost, HostStep, MarkerKind, Overlay, OverlayItem, Tool, ToolCx, ToolOutput,
};
use crate::key;
use ew_api::geometry::Dir;
use ew_api::output::{PageTables, SymbolShape, TableCp};

use super::ip::{DropOn, box_side, drop_on_cp};

/// The marks' spacing in grid steps.
pub const SUPPLY_STEPS: i64 = 2;

/// What the Supply tool's marks are placed with (the tool keeps it while it floats them): where the supply
/// comes from, and the library documents of the interruption-point forms the command carries.
#[derive(Clone, Debug, Default)]
pub struct SupplyPlan {
    /// The place's code.
    pub location: String,
    /// The device there.
    pub device: String,
    /// The library documents of the interruption-point forms.
    pub copies: Vec<serde_json::Value>,
}

/// The Supply tool.
pub struct SupplyTool {
    /// The marks' names, top to bottom.
    names: Vec<String>,
    /// The target form the marks are drawn with.
    shape: Arc<SymbolShape>,
    /// Its outline, shared with the overlay.
    outline: Arc<[Vec<(i64, i64)>]>,
    /// The snapped anchor of the first mark: (sheet nm, page nm), `None` before the first move.
    at: Option<((i64, i64), (i64, i64))>,
    /// What the marks are placed with.
    plan: SupplyPlan,
    /// Alt was held on the last move: the marks join no pin.
    free: bool,
    /// The pin each mark joins where the last overlay stood the group, in words, by the mark's name.
    shown_joins: Vec<(String, String)>,
}

impl SupplyTool {
    /// A tool floating the marks `names`, drawn with `shape`, placed with `plan`.
    pub fn new(names: Vec<String>, shape: Arc<SymbolShape>, plan: SupplyPlan) -> SupplyTool {
        let outline: Arc<[Vec<(i64, i64)>]> = shape.outline.clone().into();
        SupplyTool {
            names,
            shape,
            outline,
            at: None,
            plan,
            free: false,
            shown_joins: Vec::new(),
        }
    }

    /// What the marks are placed with.
    pub fn plan(&self) -> &SupplyPlan {
        &self.plan
    }

    /// The marks' names.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The symbol the marks are drawn with.
    pub fn shape(&self) -> &Arc<SymbolShape> {
        &self.shape
    }

    /// The page points of the marks with the first at `first` (page nm), [`SUPPLY_STEPS`] grid steps apart, each on
    /// the grid (a step of a QET grid is no whole number of nm).
    pub fn positions(&self, first: (i64, i64), cx: &ToolCx<'_>) -> Vec<(i64, i64)> {
        let step = grid_step(cx);
        (0..self.names.len())
            .map(|i| {
                let p = (first.0, first.1 + i as i64 * SUPPLY_STEPS * step);
                cx.snapper.snap_grid(cx.snapper.page_to_sheet(p)).page_nm
            })
            .collect()
    }

    /// The CP of a mark at `pos` (page nm), where a pin there stands.
    fn cp_of(&self, pos: (i64, i64), tables: &PageTables) -> Option<(i64, i64)> {
        let c = self.shape.cps.first()?;
        Some(super::ip::pin_point(tables, (pos.0 + c.at.0, pos.1 + c.at.1)))
    }

    /// What the mark at `pos` (page nm) does where its CP lands ([`mark_drop`]).
    fn drop_on(&self, pos: (i64, i64), tables: &PageTables) -> DropOn {
        self.cp_of(pos, tables)
            .map_or(DropOn::Free, |c| mark_drop(tables, c, self.free))
    }

    /// The pin each mark at `positions` (page nm) joins ([`Self::drop_on`]); none with Alt held.
    pub fn joins(&self, positions: &[(i64, i64)], tables: &PageTables) -> Vec<Option<TableCp>> {
        positions
            .iter()
            .map(|&pos| match self.drop_on(pos, tables) {
                DropOn::Pin(pin) => Some(pin),
                DropOn::Free | DropOn::Refused(_) => None,
            })
            .collect()
    }

    /// The words of the refusal of a click at the press point `at` (sheet nm, and whether Alt is held; `None` for Enter
    /// at the current point), with the overlay there (the ghost stays); `None` where the click places the marks. A
    /// mark the IP tool's rule refuses there refuses the click in that rule's words ([`Self::drop_on`]), as the IP
    /// tool's refusal does: the host asks before it sends the click.
    pub fn refusal(&mut self, at: Option<((i64, i64), bool)>, cx: &ToolCx<'_>) -> Option<(String, Overlay)> {
        if let Some((at, alt)) = at {
            let _ = self.moved(at, alt, true, cx);
        }
        let (_, first) = self.at?;
        let text = self.positions(first, cx).into_iter().find_map(|pos| {
            match self.drop_on(pos, &cx.page.tables) {
                DropOn::Refused(text) => Some(text),
                DropOn::Free | DropOn::Pin(_) => None,
            }
        })?;
        Some((text, self.overlay(cx)))
    }

    /// The group moved to the sheet point `at`: on the grid point under it; by the pointer (`pointer`) without Alt,
    /// with the CP of the first mark within the catch distance of a pin on that pin.
    fn moved(&mut self, at: (i64, i64), alt: bool, pointer: bool, cx: &ToolCx<'_>) -> ToolOutput {
        self.free = alt;
        let s = cx.snapper.snap_grid(at);
        self.at = Some((s.at_nm, s.page_nm));
        let tables = &cx.page.tables;
        if let (true, false, Some(c)) = (pointer, alt, self.shape.cps.first()) {
            let p = cx.snapper.sheet_to_page(at);
            let step = SUPPLY_STEPS * grid_step(cx);
            let caught = (0..self.names.len() as i64).find_map(|i| {
                let cp = (p.0 + c.at.0, p.1 + i * step + c.at.1);
                let pin = cx
                    .snapper
                    .snap_cp(cx.snapper.page_to_sheet(cp), cx.points_per_nm)?;
                let on = super::ip::pin_point(tables, pin.page_nm);
                matches!(mark_drop(tables, on, false), DropOn::Pin(_))
                    .then_some((pin.page_nm.0 - c.at.0, pin.page_nm.1 - c.at.1 - i * step))
            });
            if let Some(first) = caught {
                let g = cx.snapper.snap_grid(cx.snapper.page_to_sheet(first));
                self.at = Some((g.at_nm, g.page_nm));
            }
        }
        ToolOutput::Transient(self.overlay(cx))
    }

    fn overlay(&mut self, cx: &ToolCx<'_>) -> Overlay {
        let Some((_, first)) = self.at else {
            return Overlay::default();
        };
        let positions = self.positions(first, cx);
        let joins = self.joins(&positions, &cx.page.tables);
        self.shown_joins = self
            .names
            .iter()
            .zip(&joins)
            .filter_map(|(name, pin)| {
                pin.as_ref()
                    .map(|p| (name.clone(), crate::describe::pin_in_sentence(&p.label)))
            })
            .collect();
        let mut items = Vec::new();
        for ((name, pos), pin) in self.names.iter().zip(positions).zip(joins) {
            items.push(OverlayItem::Paths {
                paths: Arc::clone(&self.outline),
                transform: (0, pos),
            });
            if let Some(pin) = pin {
                items.push(OverlayItem::Marker {
                    at: cx.snapper.page_to_sheet(pin.at),
                    kind: MarkerKind::Cp,
                });
            }
            items.push(OverlayItem::Label {
                at: cx.snapper.page_to_sheet(pos),
                text: name.clone(),
                anchor: Anchor::Beside,
            });
        }
        Overlay::new(items)
    }

    fn place(&self) -> ToolOutput {
        match self.at {
            Some((_, at)) => ToolOutput::Host(HostStep::SupplyPlace { at }),
            None => ToolOutput::None,
        }
    }
}

/// What a mark whose CP lands on the page point `c` does there, by the IP tool's rule ([`drop_on_cp`]) for a target
/// of EW's form: it joins a pin, is refused, or stands free (no pin there, Alt held (`free`), or a pin on a drawn box's
/// upper or lower side, off which the IP tool stands its point with a wire and the marks, one under the other, do not).
fn mark_drop(tables: &PageTables, c: (i64, i64), free: bool) -> DropOn {
    match drop_on_cp(tables, c, true, free, || true) {
        Some(DropOn::Pin(pin)) if matches!(box_side(tables, &pin, true), Some(Dir::Up | Dir::Down)) => {
            DropOn::Free
        }
        Some(drop) => drop,
        None => DropOn::Free,
    }
}

/// The page's grid step (nm, rounded down), at least 1.
fn grid_step(cx: &ToolCx<'_>) -> i64 {
    let (num, den) = cx.page.tables.grid;
    (num / den.max(1)).max(1)
}

impl Tool for SupplyTool {
    fn name(&self) -> &'static str {
        "supply"
    }

    fn state(&self) -> &'static str {
        "floating"
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, mods } => self.moved(*at, mods.alt, true, cx),
            CanvasEvent::CursorMove { at } => self.moved(*at, false, false, cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                let _ = self.moved(*at, mods.alt, true, cx);
                self.place()
            }
            CanvasEvent::Activate => self.place(),
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> super::Hint {
        super::Hint::new(key!("hint-supply"), &[super::ENTER, super::ESCAPE])
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Supply {
            at_nm: self.at.map(|(_, p)| p),
            joins: self.shown_joins.clone(),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
