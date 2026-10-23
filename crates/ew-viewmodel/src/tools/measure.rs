// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Measure tool (`measure`; WP-S2.03 §7.6): a distance between two snapped points, read in sheet millimetres.
//!
//! armed → `Press` or `Activate` at A (snapped) → measuring: every move a `Transient` with a `measure` line from A
//! to B (snapped), the two snap markers and the label `dx <dx> dy <dy> mm, <d> mm` → `Press` or `Activate` fixes B
//! and returns to armed, keeping the overlay until the next move → `Escape` (or Backspace, the tool's step back)
//! from measuring returns to armed, from armed answers `None` (and the host returns to Select). While armed, a move
//! shows the snap marker where A would land (FR-UI-018's snap indicator). Measure never previews or commits.
//!
//! The readout: `dx` and `dy` the sheet displacement in millimetres with three decimals, `d` the Euclidean distance
//! rounded to the nearest nm (`i128::isqrt` of the squared sum, halves up) in millimetres with three decimals.

use super::{
    Anchor, Button, CanvasEvent, Overlay, OverlayItem, Snap, SnapKind, Style, Token, Tool, ToolCx,
    ToolOutput, dist2,
};
use crate::canvas::mm3;
use crate::strings;
use crate::tools::MarkerKind;

/// The Measure tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    Armed,
    Measuring { a: Snap },
}

/// The Measure tool.
#[derive(Clone, Debug)]
pub struct MeasureTool {
    state: State,
    /// The grid cursor as the last `CursorMove` placed it.
    cursor: Option<(i64, i64)>,
}

impl Default for MeasureTool {
    fn default() -> MeasureTool {
        MeasureTool::new()
    }
}

/// The distance of two sheet points rounded to the nearest nm: `isqrt` of the squared sum, halves up.
pub fn distance_nm(a: (i64, i64), b: (i64, i64)) -> i128 {
    let s = dist2(a, b);
    let n = s.isqrt();
    // n² ≤ s < (n + 1)²; s ≥ (n + ½)² = n² + n + ¼ exactly when s − n² > n.
    if s - n * n > n { n + 1 } else { n }
}

/// The readout of a measurement from `a` to `b`: `dx 12.700 dy 0.000 mm, 12.700 mm`.
pub fn measure_label(a: (i64, i64), b: (i64, i64)) -> String {
    let d = distance_nm(a, b);
    strings::format(
        "measure-label",
        &[
            &mm3(b.0.saturating_sub(a.0)),
            &mm3(b.1.saturating_sub(a.1)),
            &mm3(i64::try_from(d).unwrap_or(i64::MAX)),
        ],
    )
}

fn marker(s: &Snap) -> OverlayItem {
    OverlayItem::Marker {
        at: s.at_nm,
        kind: match s.kind {
            SnapKind::Grid => MarkerKind::Grid,
            SnapKind::Cp => MarkerKind::Cp,
        },
    }
}

/// The overlay of a measurement from A to B.
fn measuring(a: &Snap, b: &Snap) -> Overlay {
    let mid = (
        a.at_nm.0 + (b.at_nm.0 - a.at_nm.0) / 2,
        a.at_nm.1 + (b.at_nm.1 - a.at_nm.1) / 2,
    );
    Overlay::new(vec![
        OverlayItem::Line {
            a: a.at_nm,
            b: b.at_nm,
            style: Style::solid(Token::Measure),
        },
        marker(a),
        marker(b),
        OverlayItem::Label {
            at: mid,
            text: measure_label(a.at_nm, b.at_nm),
            anchor: Anchor::Above,
        },
    ])
}

impl MeasureTool {
    /// The tool in its first state.
    pub fn new() -> MeasureTool {
        MeasureTool {
            state: State::Armed,
            cursor: None,
        }
    }

    /// A move to `at` (snapped with `alt` held or not).
    fn moved(&self, at: (i64, i64), alt: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap(at, cx.points_per_nm, alt);
        ToolOutput::Transient(match &self.state {
            State::Armed => Overlay::new(vec![marker(&s)]),
            State::Measuring { a } => measuring(a, &s),
        })
    }

    /// A click at `at`: A while armed, B while measuring.
    fn click(&mut self, at: (i64, i64), alt: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap(at, cx.points_per_nm, alt);
        match std::mem::replace(&mut self.state, State::Armed) {
            State::Armed => {
                let overlay = Overlay::new(vec![marker(&s)]);
                self.state = State::Measuring { a: s };
                ToolOutput::Transient(overlay)
            }
            State::Measuring { a } => ToolOutput::Transient(measuring(&a, &s)),
        }
    }

    fn back(&mut self) -> ToolOutput {
        match self.state {
            State::Armed => ToolOutput::None,
            State::Measuring { .. } => {
                self.state = State::Armed;
                ToolOutput::Cancel
            }
        }
    }
}

impl Tool for MeasureTool {
    fn name(&self) -> &'static str {
        "measure"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed => "armed",
            State::Measuring { .. } => "measuring",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match *ev {
            CanvasEvent::PointerMove { at, mods } => self.moved(at, mods.alt, cx),
            CanvasEvent::CursorMove { at } => {
                self.cursor = Some(at);
                self.moved(at, false, cx)
            }
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => self.click(at, mods.alt, cx),
            CanvasEvent::Activate => match self.cursor {
                Some(at) => self.click(at, false, cx),
                None => ToolOutput::None,
            },
            CanvasEvent::Escape | CanvasEvent::Back => self.back(),
            CanvasEvent::Press { .. }
            | CanvasEvent::Release { .. }
            | CanvasEvent::Dwell { .. }
            | CanvasEvent::Key(_)
            | CanvasEvent::Text(_) => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Armed => "hint-measure-armed",
            State::Measuring { .. } => "hint-measure-measuring",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distance_rounds_to_the_nearest_nanometre() {
        assert_eq!(distance_nm((0, 0), (3, 4)), 5);
        assert_eq!(distance_nm((0, 0), (1, 1)), 1, "1.414 rounds down");
        assert_eq!(distance_nm((0, 0), (1, 2)), 2, "2.236 rounds down");
        assert_eq!(distance_nm((0, 0), (2, 2)), 3, "2.828 rounds up");
        assert_eq!(
            distance_nm((0, 0), (12_700_000, 12_700_000)),
            17_960_512,
            "12.7 × √2 mm"
        );
        assert_eq!(
            measure_label((10_000_000, 20_000_000), (22_700_000, 20_000_000)),
            "dx 12.700 dy 0.000 mm, 12.700 mm"
        );
        assert_eq!(
            measure_label((0, 0), (-3_000_000, -4_000_000)),
            "dx -3.000 dy -4.000 mm, 5.000 mm"
        );
    }
}
