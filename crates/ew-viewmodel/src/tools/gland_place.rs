// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The glands of cables dragged from Unplaced devices over a panel page ([`GlandPlaceTool`], armed by
//! `tool.place_glands {cables}` while the row hovers the canvas, as a device's row arms the layout place tool): the
//! outline of each gland's nut, in the row `layout.glands_place` lays, floats under the pointer until the drop, where
//! the host places them as the action does ([`HostStep::GlandsPlace`]).

use super::{
    Button, CanvasEvent, Hint, HostStep, Overlay, OverlayItem, Style, Token, Tool, ToolCx, ToolOutput,
};

/// The corners of a nut's outline: a hexagon.
const CORNERS: usize = 6;

/// The tool that floats the glands of cables over a panel page and places them on release.
#[derive(Clone, Debug)]
pub struct GlandPlaceTool {
    /// The cables, as the dragged row names them (`id:<ID>`, separated by commas).
    cables: String,
    /// Their glands in their row: each nut's centre along the surface's first axis from the first's, and its room
    /// across its corners, surface nm (`Session::gland_row`).
    row: Vec<(i128, i128)>,
    /// What a single gland snaps to (`Session::gland_snap_of`): its outline stands where its drop puts it.
    snap: Option<ew_api::output::GlandSnap>,
    /// The last pointer or cursor point, sheet nm.
    last: Option<(i64, i64)>,
}

impl GlandPlaceTool {
    /// The tool armed with the glands of `cables` laid as `row`, a single gland snapping by `snap`.
    pub fn new(
        cables: String,
        row: Vec<(i128, i128)>,
        snap: Option<ew_api::output::GlandSnap>,
    ) -> GlandPlaceTool {
        GlandPlaceTool {
            cables,
            row,
            snap,
            last: None,
        }
    }

    /// The outline of the glands with the first centred at `at` (sheet nm), a single gland where its drop snaps to:
    /// each nut a dashed hexagon in the ghost's colour.
    fn track(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        self.last = Some(at);
        let Some(panel) = cx.page.panel.as_deref() else {
            return ToolOutput::None;
        };
        let at = super::layout::surface_of_sheet(cx, panel, at);
        let (u, v) = self.snap.as_ref().map_or(at, |s| s.snap(at));
        let items = self
            .row
            .iter()
            .map(|(du, nut)| {
                let (cu, r) = (u + du, (nut / 2) as f64);
                let points = (0..=CORNERS)
                    .map(|k| {
                        let a = std::f64::consts::TAU * (k % CORNERS) as f64 / CORNERS as f64;
                        let p = (
                            cu + (r * a.cos()).round() as i128,
                            v + (r * a.sin()).round() as i128,
                        );
                        super::layout::sheet_of_surface(cx, panel, p)
                    })
                    .collect();
                OverlayItem::Polyline {
                    points,
                    style: Style::dashed(Token::Ghost),
                }
            })
            .collect();
        ToolOutput::Transient(Overlay::new(items))
    }
}

impl Tool for GlandPlaceTool {
    fn name(&self) -> &'static str {
        "gland_place"
    }

    fn state(&self) -> &'static str {
        match self.last {
            None => "armed",
            Some(_) => "placing",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        if cx.page.panel.is_none() {
            return match ev {
                CanvasEvent::Press { .. } | CanvasEvent::Activate => {
                    ToolOutput::Refused(crate::key!("reason-no-panel-page"))
                }
                _ => ToolOutput::None,
            };
        }
        match ev {
            CanvasEvent::PointerMove { at, .. }
            | CanvasEvent::CursorMove { at }
            | CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => self.track(*at, cx),
            CanvasEvent::Release {
                at,
                button: Button::Primary,
                ..
            } => ToolOutput::Host(HostStep::GlandsPlace {
                cables: self.cables.clone(),
                at: *at,
            }),
            CanvasEvent::Activate => match self.last {
                Some(at) => ToolOutput::Host(HostStep::GlandsPlace {
                    cables: self.cables.clone(),
                    at,
                }),
                None => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    // The drop ends the drag that armed it.
    fn gives_way_after_step(&self) -> bool {
        true
    }

    fn status_hint(&self) -> Hint {
        Hint::new(crate::key!("hint-gland-place"), &[super::ESCAPE])
    }
}
