// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placing shaped text in a box: alignment, line splitting, shrink-to-fit and ellipsis.

use std::sync::Arc;

use ew_dl::{Item, ShapedRun, Transform};
use ew_schema::{Align, Overflow};
use ew_text::{FontSet, Shaped, TextStyle};
use ew_units::{Angle2, Nm, Pt2, Rect};

use super::sheet::{PPB_ONE, SheetMap};

/// The horizontal inset of cell text from the cell's left and right edges, and the vertical inset of top-
/// and bottom-aligned text: 2 QET px (0.529 mm). Doc 20 gives no inset; this is the lead's choice.
/// `round(2 × 793 750 / 3)` nm (= `Grid::QET_1PX.at(2)`).
pub const CELL_PADDING: Nm = Nm(529_167);

/// The ellipsis appended by [`Overflow::Ellipsis`].
pub const ELLIPSIS: &str = "\u{2026}";

/// How the text of a box ended up relative to its policy.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum OverflowKind {
    /// Shrink-to-fit reached the minimum printed cap height and the text still does not fit.
    ShrinkLimit,
    /// The text was cut and an ellipsis appended.
    Truncated,
    /// The policy is `error`: the text is drawn as is and reported.
    Error,
    /// The policy is of a newer format: handled like `error`.
    UnknownPolicy,
}

/// Horizontal and vertical alignment parts of an [`Align`]; `None` for an unknown alignment.
pub(crate) fn split_align(a: &Align) -> Option<(i8, i8)> {
    Some(match a {
        Align::TopLeft => (-1, -1),
        Align::TopCentre => (0, -1),
        Align::TopRight => (1, -1),
        Align::MiddleLeft => (-1, 0),
        Align::Centre => (0, 0),
        Align::MiddleRight => (1, 0),
        Align::BottomLeft => (-1, 1),
        Align::BottomCentre => (0, 1),
        Align::BottomRight => (1, 1),
        Align::Unknown(_) => return None,
    })
}

/// The result of laying a text into a box.
pub(crate) struct Laid {
    /// One glyph-run item per line.
    pub items: Vec<Item>,
    /// The shaped text at its final size.
    pub shaped: Shaped,
    /// The overflow outcome, if not a clean fit.
    pub overflow: Option<OverflowKind>,
}

/// The parameters of a box layout.
pub(crate) struct BoxSpec<'a> {
    pub rect: Rect,
    pub halign: i8,
    pub valign: i8,
    pub padding: Nm,
    pub overflow: Option<&'a Overflow>,
    pub max_lines: usize,
    /// The page-to-sheet map, for the printed minimum cap height of shrink-to-fit.
    pub sheet: SheetMap,
}

/// Lays `text` into `spec.rect` (see the module documentation of [`crate::forms`]).
pub(crate) fn lay_out(fonts: &FontSet, text: &str, style: &TextStyle, spec: &BoxSpec<'_>) -> Laid {
    let avail_w = Nm((spec.rect.width().0 - 2 * spec.padding.0).max(0));
    let avail_h = spec.rect.height();
    let wrap = (spec.max_lines > 1).then_some(avail_w);
    let fits = |s: &Shaped, st: &TextStyle| {
        let m = fonts.vertical_metrics(st);
        let n = s.lines.len();
        s.advance() <= avail_w
            && n <= spec.max_lines.max(1)
            && (n <= 1 || Nm(m.line_pitch.0 * n as i64) <= avail_h)
    };
    let mut style = *style;
    let mut shaped = fonts.shape(text, &style, wrap);
    let mut overflow = None;
    let mut shown = text.to_owned();
    if !fits(&shaped, &style) {
        match spec.overflow {
            Some(Overflow::ShrinkToFit(sh)) => {
                let min = min_size(fonts, &style, sh.min_cap_height, spec.sheet);
                let mut size = style.size;
                loop {
                    let next = if spec.max_lines <= 1 && shaped.advance().0 > 0 {
                        // Width is linear in the size: jump close to the fitting size, then step.
                        let guess =
                            i128::from(size.0) * i128::from(avail_w.0) / i128::from(shaped.advance().0);
                        Nm((guess as i64).min(size.0 - 1))
                    } else {
                        Nm(size.0 - (size.0 / 50).max(1))
                    };
                    if next < min {
                        style = style.with_size(min.max(Nm(1)));
                        shaped = fonts.shape(text, &style, wrap);
                        if !fits(&shaped, &style) {
                            overflow = Some(OverflowKind::ShrinkLimit);
                        }
                        break;
                    }
                    size = next;
                    style = style.with_size(size);
                    shaped = fonts.shape(text, &style, wrap);
                    if fits(&shaped, &style) {
                        break;
                    }
                }
            }
            Some(Overflow::Ellipsis) => {
                let chars: Vec<(usize, char)> = text.char_indices().collect();
                let mut keep = chars.len();
                loop {
                    keep = keep.saturating_sub(1);
                    let cut = chars.get(keep).map_or(text.len(), |c| c.0);
                    shown = format!("{}{ELLIPSIS}", text[..cut].trim_end());
                    shaped = fonts.shape(&shown, &style, None);
                    if keep == 0 || shaped.advance() <= avail_w {
                        break;
                    }
                }
                overflow = Some(OverflowKind::Truncated);
            }
            Some(Overflow::Error) => overflow = Some(OverflowKind::Error),
            Some(Overflow::Unknown(_)) => overflow = Some(OverflowKind::UnknownPolicy),
            None => {}
        }
    }
    let items = place(fonts, &shaped, &style, &shown, spec, Pt2::ORIGIN, Angle2::ZERO);
    Laid {
        items,
        shaped,
        overflow,
    }
}

/// The smallest em height whose printed cap height (through `sheet`) is at least `min_cap`.
fn min_size(fonts: &FontSet, style: &TextStyle, min_cap: Nm, sheet: SheetMap) -> Nm {
    let m = fonts.face_for(style).metrics;
    let cap = i128::from(m.cap_height.max(1));
    let num = i128::from(min_cap.0.max(0)) * i128::from(m.units_per_em) * i128::from(PPB_ONE);
    let den = cap * i128::from(sheet.scale_ppb.max(1));
    Nm(((num + den - 1) / den) as i64)
}

/// Glyph-run items for `shaped` aligned in `spec.rect` (a zero-size rect aligns at a point), turned by
/// `angle` about `pivot` (`Pt2::ORIGIN` and zero for cells).
///
/// Horizontal: left at `min.x + padding`, right at `max.x − padding`, centre at the midpoint (halving
/// toward −∞), per line. Vertical: top puts the first line's ascent at `min.y + padding`; bottom puts the
/// last line's descent at `max.y − padding`; middle centres the block from the first line's cap height
/// to the last baseline on the box's midpoint.
pub(crate) fn place(
    fonts: &FontSet,
    shaped: &Shaped,
    style: &TextStyle,
    text: &str,
    spec: &BoxSpec<'_>,
    pivot: Pt2,
    angle: Angle2,
) -> Vec<Item> {
    let m = fonts.vertical_metrics(style);
    let r = spec.rect;
    let last = shaped.lines.last().map_or(Nm::ZERO, |l| l.baseline);
    let mid = |a: Nm, b: Nm| Nm(a.0 + (b.0 - a.0).div_euclid(2));
    let baseline0 = match spec.valign {
        -1 => r.min().y + spec.padding + m.ascent,
        1 => r.max().y - spec.padding - m.descent - last,
        _ => {
            // Block from (baseline0 − cap) to (baseline0 + last), centred on the box's middle.
            let c = mid(r.min().y, r.max().y);
            Nm(c.0 + (m.cap_height.0 - last.0).div_euclid(2))
        }
    };
    let run = &shaped.run;
    shaped
        .lines
        .iter()
        .map(|line| {
            let x = match spec.halign {
                -1 => r.min().x + spec.padding,
                1 => r.max().x - spec.padding - line.advance,
                _ => Nm(mid(r.min().x, r.max().x).0 - line.advance.0.div_euclid(2)),
            };
            let y0 = run.glyphs.get(line.glyphs.start).map_or(line.baseline, |g| g.y);
            let glyphs = run.glyphs[line.glyphs.clone()]
                .iter()
                .map(|g| ew_dl::Glyph { y: g.y - y0, ..*g })
                .collect();
            let offset = Pt2::new(x, baseline0 + line.baseline);
            let origin = if angle == Angle2::ZERO {
                offset
            } else {
                pivot + ew_dl::num::rotate(offset - pivot, angle, Pt2::ORIGIN)
            };
            let line_text = text.get(line.text.clone()).unwrap_or(text);
            let layout_box = Rect::from_points(
                Pt2::new(Nm::ZERO, Nm(-m.ascent.0)),
                Pt2::new(line.advance, m.descent),
            );
            Item::glyphs(Arc::new(ShapedRun {
                font: run.font,
                size: run.size,
                origin,
                transform: Transform::from_angle(angle),
                glyphs,
                text: line_text.trim_end_matches(['\n', '\r']).to_owned(),
                brush: run.brush,
                layout_box,
            }))
        })
        .collect()
}
