// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Image tool (`image`; WP-S2.04 §11, FR-SYM-025): an image asset of the project in a rectangle of two
//! fine-snapped corners that keeps the asset's aspect ratio, committed as an `item.place` image.
//!
//! **The asset** is chosen by the host (`image.pick`, which resolves the hash through the session and hands the tool
//! its hash and intrinsic size, [`ImageAsset`]); the tool itself never reads the project.
//!
//! **The rectangle.** All points are page nm, fine-snapped ([`crate::tools::Snapper::snap_fine`],
//! `FineKind::Graphic`). The first corner is the first click `p0`; the second click `p` fixes the box
//! `|p.x - p0.x|` by `|p.y - p0.y|`, which must have both sides greater than 0. With an intrinsic size `w` by `h`
//! ([`ImageAsset::size`]) the image is the largest rectangle of that ratio that fits the box, with its corner at
//! `p0` and growing towards `p` ([`fit_rect`]): when `|dx|·h <= |dy|·w` the width is `|dx|` and the height
//! `round(|dx|·h / w)`, else the height is `|dy|` and the width `round(|dy|·w / h)` (exact in `i128`, halves up). An
//! asset without a known size fills the box as it is. The second corner of the committed rectangle is therefore
//! generally not the click; the overlay shows the fitted rectangle.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | picking | move to p | picking (at p) | `Transient`: a grid marker |
//! | picking | a click | picking | `None` |
//! | armed | move to p | armed (at p) | `Transient`: a grid marker |
//! | armed | a click at p | placing (`corner p`, at p) | `Transient` |
//! | placing | move to p | placing (at p) | `Transient`: the fitted rectangle, dashed |
//! | placing | a click at p with a box of area | armed (at p) | `Commit` `item.place` image |
//! | placing | a click at p without area | placing | `None` |
//! | placing | `Dwell { ms ≥ 80 }` | placing | `Preview` of the command a click at `at` would commit now |
//! | placing | `Back` or `Escape` | armed (at kept) | `Cancel` |
//! | armed or picking | `Escape` | — | `None` (the host returns to Select) |
//!
//! `set_asset` (the host's `image.pick`) makes the tool armed with that asset, from any state, keeping the pointer
//! point; an unfinished rectangle is dropped.

use ew_api::AnyCommand;
use serde_json::json;

use super::{
    Button, CanvasEvent, FineKind, Ghost, MarkerKind, Overlay, OverlayItem, Style, Token, Tool, ToolCx,
    ToolOutput, rect_of,
};

/// The asset the tool places: its hash (`sha256:<64 hex>`) and, when known, its intrinsic width and height (any
/// common unit; only the ratio is used).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImageAsset {
    /// The hash text, as `item.place` takes it.
    pub hash: String,
    /// The intrinsic size, `None` when the asset does not say.
    pub size: Option<(u32, u32)>,
}

/// The rectangle of an image of intrinsic `size` whose corner is `p0` and which fits the box towards `p`: `(min,
/// max)` in page nm; `None` when the box has no area (`p` on a line through `p0`) or the fitted side rounds to 0.
/// Without a size (or with a side 0) the box itself.
pub fn fit_rect(p0: (i64, i64), p: (i64, i64), size: Option<(u32, u32)>) -> Option<((i64, i64), (i64, i64))> {
    let dx = i128::from(p.0) - i128::from(p0.0);
    let dy = i128::from(p.1) - i128::from(p0.1);
    if dx == 0 || dy == 0 {
        return None;
    }
    let (aw, ah) = (dx.abs(), dy.abs());
    let (w, h) = match size {
        Some((w, h)) if w > 0 && h > 0 => (i128::from(w), i128::from(h)),
        _ => return Some(((p0.0.min(p.0), p0.1.min(p.1)), (p0.0.max(p.0), p0.1.max(p.1)))),
    };
    let (fw, fh) = if aw * h <= ah * w {
        (aw, (aw * h * 2 + w) / (2 * w))
    } else {
        ((ah * w * 2 + h) / (2 * h), ah)
    };
    if fw == 0 || fh == 0 {
        return None;
    }
    let x = i128::from(p0.0) + dx.signum() * fw;
    let y = i128::from(p0.1) + dy.signum() * fh;
    let (x, y) = (i64::try_from(x).ok()?, i64::try_from(y).ok()?);
    Some(((p0.0.min(x), p0.1.min(y)), (p0.0.max(x), p0.1.max(y))))
}

/// The Image tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    /// No asset chosen yet.
    Picking { at: Option<(i64, i64)> },
    /// An asset chosen, no corner fixed.
    Armed { at: Option<(i64, i64)> },
    /// The first corner fixed and the current point (page nm).
    Placing { corner: (i64, i64), at: (i64, i64) },
}

/// The Image tool (Insert ▸ Image…).
#[derive(Clone, Debug)]
pub struct ImageTool {
    state: State,
    asset: Option<ImageAsset>,
}

impl ImageTool {
    /// The tool, without an asset.
    pub fn new() -> ImageTool {
        ImageTool {
            state: State::Picking { at: None },
            asset: None,
        }
    }

    /// The asset it places.
    pub fn asset(&self) -> Option<&ImageAsset> {
        self.asset.as_ref()
    }

    /// Chooses the asset: the tool is armed with it, an unfinished rectangle dropped.
    pub fn set_asset(&mut self, asset: ImageAsset) {
        self.asset = Some(asset);
        self.state = State::Armed { at: self.point() };
    }

    /// The current point (page nm).
    fn point(&self) -> Option<(i64, i64)> {
        match self.state {
            State::Picking { at } | State::Armed { at } => at,
            State::Placing { at, .. } => Some(at),
        }
    }

    /// The overlay at the current point after a field changed (the host's refresh after `image.pick`).
    pub fn refresh(&mut self, cx: &ToolCx<'_>) -> Overlay {
        self.overlay(cx)
    }

    /// The overlay of the state: the grid marker, and the fitted rectangle dashed while placing.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let Some(at) = self.point() else {
            return Overlay::default();
        };
        let sheet = |p: (i64, i64)| cx.snapper.page_to_sheet(p);
        let mut items = Vec::new();
        if let State::Placing { corner, .. } = &self.state {
            let size = self.asset.as_ref().and_then(|a| a.size);
            let rect = fit_rect(*corner, at, size).unwrap_or((*corner, at));
            items.push(OverlayItem::Rect {
                r: rect_of(sheet(rect.0), sheet(rect.1)),
                style: Style::dashed(Token::Ghost),
            });
        }
        items.push(OverlayItem::Marker {
            at: sheet(at),
            kind: MarkerKind::Grid,
        });
        Overlay::new(items)
    }

    /// The `item.place` of the image in the box from the first corner to the current point; `None` before the first
    /// corner, without an asset or for a box without area.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        let State::Placing { corner, at } = &self.state else {
            return None;
        };
        let asset = self.asset.as_ref()?;
        let (min, max) = fit_rect(*corner, *at, asset.size)?;
        Some(Box::new(AnyCommand {
            name: "item.place".into(),
            params: json!({
                "page": cx.page.tables.page.to_string(),
                "kind": {"image": {
                    "asset": asset.hash,
                    "rect": [[min.0, min.1], [max.0, max.1]],
                }},
            }),
        }))
    }

    /// A click at the current point.
    fn click(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        match self.state.clone() {
            State::Picking { .. } | State::Armed { at: None } => ToolOutput::None,
            State::Armed { at: Some(p) } => {
                self.state = State::Placing { corner: p, at: p };
                ToolOutput::Transient(self.overlay(cx))
            }
            State::Placing { at, .. } => match self.command(cx) {
                Some(cmd) => {
                    self.state = State::Armed { at: Some(at) };
                    ToolOutput::Commit(cmd)
                }
                None => ToolOutput::None,
            },
        }
    }

    /// A move to `at`: fine-snapped with the pointer's Alt.
    fn moved(&mut self, at: (i64, i64), alt: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.snap_fine(at, FineKind::Graphic, alt).1;
        match &mut self.state {
            State::Picking { at } | State::Armed { at } => *at = Some(p),
            State::Placing { at, .. } => *at = p,
        }
        ToolOutput::Transient(self.overlay(cx))
    }

    /// `Back` and `Escape` while placing: the first corner is dropped, the tool is armed again.
    fn cancel(&mut self) -> ToolOutput {
        match self.state {
            State::Placing { at, .. } => {
                self.state = State::Armed { at: Some(at) };
                ToolOutput::Cancel
            }
            _ => ToolOutput::None,
        }
    }
}

impl Default for ImageTool {
    fn default() -> ImageTool {
        ImageTool::new()
    }
}

impl Tool for ImageTool {
    fn name(&self) -> &'static str {
        "image"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Picking { .. } => "picking",
            State::Armed { .. } => "armed",
            State::Placing { .. } => "placing",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, mods } => self.moved(*at, mods.alt, cx),
            CanvasEvent::CursorMove { at } => self.moved(*at, false, cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                let _ = self.moved(*at, mods.alt, cx);
                self.click(cx)
            }
            CanvasEvent::Activate => self.click(cx),
            CanvasEvent::Back => self.cancel(),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Escape => self.cancel(),
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Picking { .. } => "hint-image-picking",
            State::Armed { .. } => "hint-image-armed",
            State::Placing { .. } => "hint-image-placing",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Image {
            asset: self.asset.as_ref().map(|a| a.hash.clone()),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_box_is_limited_by_its_height() {
        // 2:1 asset, box 100 by 20: the height 20 gives the width 40.
        assert_eq!(
            fit_rect((0, 0), (100, 20), Some((200, 100))),
            Some(((0, 0), (40, 20)))
        );
    }

    #[test]
    fn a_tall_box_is_limited_by_its_width() {
        // 2:1 asset, box 20 by 100: the width 20 gives the height 10.
        assert_eq!(
            fit_rect((0, 0), (20, 100), Some((200, 100))),
            Some(((0, 0), (20, 10)))
        );
    }

    #[test]
    fn the_rectangle_grows_towards_the_second_corner() {
        assert_eq!(
            fit_rect((100, 100), (60, 120), Some((200, 100))),
            Some(((60, 100), (100, 120)))
        );
        assert_eq!(
            fit_rect((100, 100), (120, 40), Some((100, 200))),
            Some(((100, 60), (120, 100)))
        );
    }

    #[test]
    fn the_fitted_side_is_rounded_half_up() {
        // 3:2 asset, width 5: height 3.33 -> 3; width 7: height 4.67 -> 5; width 3: 2.
        assert_eq!(fit_rect((0, 0), (5, 100), Some((3, 2))), Some(((0, 0), (5, 3))));
        assert_eq!(fit_rect((0, 0), (7, 100), Some((3, 2))), Some(((0, 0), (7, 5))));
        // 1:1 asset, box 3 by 1: width 1; the half case 1:2 with width 1 gives 2.
        assert_eq!(fit_rect((0, 0), (1, 100), Some((1, 2))), Some(((0, 0), (1, 2))));
        assert_eq!(fit_rect((0, 0), (1, 100), Some((2, 1))), Some(((0, 0), (1, 1))));
    }

    #[test]
    fn a_box_without_area_gives_nothing() {
        assert_eq!(fit_rect((0, 0), (10, 0), Some((1, 1))), None);
        assert_eq!(fit_rect((0, 0), (0, 10), None), None);
        // A height that rounds to 0.
        assert_eq!(fit_rect((0, 0), (1, 100), Some((10, 1))), None);
    }

    #[test]
    fn an_unknown_size_fills_the_box() {
        assert_eq!(fit_rect((10, 10), (0, 40), None), Some(((0, 10), (10, 40))));
        assert_eq!(
            fit_rect((10, 10), (0, 40), Some((0, 5))),
            Some(((0, 10), (10, 40)))
        );
    }
}
