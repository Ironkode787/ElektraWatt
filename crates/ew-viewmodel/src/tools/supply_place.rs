// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Supply tool (the Draw tab's Supply): the supply marks of what comes in from the mains (L, N, PE; L1, L2,
//! L3, N, PE; …) float as one group under the pointer or the grid cursor, one under the other two grid steps apart,
//! each drawn with the target form of an interruption point and its name.
//!
//! State `floating` (not sticky). `PointerMove` or `CursorMove` → `Transient`; `Press` or `Activate` → the host step
//! [`HostStep::SupplyPlace`]: the host places the points, marks them as where the supply comes in (`ip.supply_in`) in
//! one undo step, and their Potential panel opens. `Escape` answers nothing, so the Escape chain ends the tool.

use std::sync::Arc;

use super::{Anchor, Button, CanvasEvent, HostStep, Overlay, OverlayItem, Tool, ToolCx, ToolOutput};
use crate::key;
use ew_api::output::SymbolShape;

/// The marks' spacing in grid steps.
pub const SUPPLY_STEPS: i64 = 2;

/// What the Supply tool's marks are placed with (the host keeps it while the tool floats them): where the supply
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
}

impl SupplyTool {
    /// A tool floating the marks `names`, drawn with `shape`.
    pub fn new(names: Vec<String>, shape: Arc<SymbolShape>) -> SupplyTool {
        let outline: Arc<[Vec<(i64, i64)>]> = shape.outline.clone().into();
        SupplyTool {
            names,
            shape,
            outline,
            at: None,
        }
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

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap_grid(at);
        self.at = Some((s.at_nm, s.page_nm));
        ToolOutput::Transient(self.overlay(cx))
    }

    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let Some((_, first)) = self.at else {
            return Overlay::default();
        };
        let mut items = Vec::new();
        for (name, pos) in self.names.iter().zip(self.positions(first, cx)) {
            items.push(OverlayItem::Paths {
                paths: Arc::clone(&self.outline),
                transform: (0, pos),
            });
            items.push(OverlayItem::Label {
                at: cx.snapper.page_to_sheet(pos),
                text: name.clone(),
                anchor: Anchor::Above,
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
            CanvasEvent::PointerMove { at, .. } | CanvasEvent::CursorMove { at } => self.moved(*at, cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => {
                let _ = self.moved(*at, cx);
                self.place()
            }
            CanvasEvent::Activate => self.place(),
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> super::Hint {
        super::Hint::new(key!("hint-supply"), &[super::ENTER, super::ESCAPE])
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
