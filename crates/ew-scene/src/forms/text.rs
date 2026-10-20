// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placing shaped text in a box: alignment, line splitting, shrink-to-fit and ellipsis.

use std::sync::Arc;

use ew_dl::{Item, ShapedRun, Transform};
use ew_schema::{Align, Overflow};
use ew_text::{FaceSpan, FontSet, Shaped, TextStyle};
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
    if avail_w <= Nm::ZERO && !text.is_empty() {
        // The box is not wider than its two insets: nothing can be drawn in it. Report it under the
        // policy's own kind and draw nothing (never a zero-size run).
        return Laid {
            items: Vec::new(),
            shaped,
            overflow: Some(match spec.overflow {
                Some(Overflow::Ellipsis) => OverflowKind::Truncated,
                Some(Overflow::Error) => OverflowKind::Error,
                Some(Overflow::Unknown(_)) => OverflowKind::UnknownPolicy,
                Some(Overflow::ShrinkToFit(_)) | None => OverflowKind::ShrinkLimit,
            }),
        };
    }
    if !fits(&shaped, &style) {
        match spec.overflow {
            Some(Overflow::ShrinkToFit(sh)) => {
                // The lowest permitted size: the one whose printed cap height is the minimum, but never
                // above the style's own size (shrinking must not enlarge).
                let floor = min_size(fonts, &style, sh.min_cap_height, spec.sheet).min(style.size);
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
                    if next <= Nm::ZERO {
                        // Not shrinkable to a drawable size: draw nothing.
                        return Laid {
                            items: Vec::new(),
                            shaped,
                            overflow: Some(OverflowKind::ShrinkLimit),
                        };
                    }
                    if next < floor {
                        style = style.with_size(floor);
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
                // The longest prefix (cut at a char boundary, trailing whitespace trimmed) whose
                // "…"-terminated text fits, or the bare ellipsis when none does. A shorter prefix is never
                // wider, so a binary search over the cut index finds the cut that dropping one char at a
                // time would, with O(log n) shapings instead of O(n).
                let chars: Vec<(usize, char)> = text.char_indices().collect();
                let candidate = |keep: usize| {
                    let cut = chars.get(keep).map_or(text.len(), |c| c.0);
                    format!("{}{ELLIPSIS}", text[..cut].trim_end())
                };
                let (mut lo, mut hi) = (0, chars.len().saturating_sub(1));
                while lo < hi {
                    let mid = lo + (hi - lo).div_ceil(2);
                    if fonts.shape(&candidate(mid), &style, None).advance() <= avail_w {
                        lo = mid;
                    } else {
                        hi = mid - 1;
                    }
                }
                shown = candidate(lo);
                shaped = fonts.shape(&shown, &style, None);
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
            let glyphs: Vec<ew_dl::Glyph> = run.glyphs[line.glyphs.clone()]
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
            let spans: Vec<&FaceSpan> = shaped
                .faces
                .iter()
                .filter(|f| f.glyphs.start >= line.glyphs.start && f.glyphs.end <= line.glyphs.end)
                .collect();
            let make = |font, glyphs: Vec<ew_dl::Glyph>, text: &str, layout_box, origin, transform| {
                Arc::new(ShapedRun {
                    font,
                    size: run.size,
                    origin,
                    transform,
                    glyphs,
                    text: text.trim_end_matches(['\n', '\r']).to_owned(),
                    brush: run.brush,
                    layout_box,
                })
            };
            if spans.len() <= 1 {
                // One face: a single run, drawn with the face that shaped it (the symbol fallback if the
                // style's face had none of the characters).
                let font = spans.first().map_or(run.font, |f| f.font);
                return Item::glyphs(make(
                    font,
                    glyphs,
                    line_text,
                    layout_box,
                    origin,
                    Transform::from_angle(angle),
                ));
            }
            // Several faces: one run per face in a group that carries the placement, so the line is one
            // item for its link and entity. Each run's box covers its own stretch of the pen travel; their
            // union is the line's box.
            let first = spans[0].glyphs.start;
            let runs = spans
                .iter()
                .enumerate()
                .map(|(i, span)| {
                    let x0 = if i == 0 {
                        Nm::ZERO
                    } else {
                        run.glyphs[span.glyphs.start].x
                    };
                    let x1 = spans
                        .get(i + 1)
                        .map_or(line.advance, |n| run.glyphs[n.glyphs.start].x);
                    let glyphs = glyphs[span.glyphs.start - first..span.glyphs.end - first].to_vec();
                    let span_text = text.get(span.text.clone()).unwrap_or(line_text);
                    let layout_box =
                        Rect::from_points(Pt2::new(x0, Nm(-m.ascent.0)), Pt2::new(x1, m.descent));
                    Item::glyphs(make(
                        span.font,
                        glyphs,
                        span_text,
                        layout_box,
                        Pt2::ORIGIN,
                        Transform::IDENTITY,
                    ))
                })
                .collect();
            let placement = Transform {
                translate: origin,
                ..Transform::from_angle(angle)
            };
            Item::group(Some(placement), None, runs)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_text::bundled;

    /// The straightforward algorithm: drop one char at a time and re-shape the whole candidate.
    fn reference_ellipsis(fonts: &FontSet, text: &str, style: &TextStyle, avail_w: Nm) -> String {
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let mut keep = chars.len();
        loop {
            keep = keep.saturating_sub(1);
            let cut = chars.get(keep).map_or(text.len(), |c| c.0);
            let shown = format!("{}{ELLIPSIS}", text[..cut].trim_end());
            if keep == 0 || fonts.shape(&shown, style, None).advance() <= avail_w {
                return shown;
            }
        }
    }

    #[test]
    fn a_mixed_line_is_one_group_with_one_run_per_font() {
        let fonts = bundled();
        let style = TextStyle::regular(Nm(3_175_000));
        let text = "\u{221E} 3-B2, 106-G3";
        let shaped = fonts.shape(text, &style, None);
        assert_eq!(shaped.faces.len(), 2);
        let at = Pt2::new(Nm(10_000_000), Nm(20_000_000));
        let spec = BoxSpec {
            rect: Rect::from_point(at),
            halign: -1,
            valign: -1,
            padding: Nm::ZERO,
            overflow: None,
            max_lines: 1,
            sheet: SheetMap::IDENTITY,
        };
        let items = place(fonts, &shaped, &style, text, &spec, at, Angle2::ZERO);
        // One item per line; callers put the link and the entity on it.
        assert_eq!(items.len(), 1);
        let link = ew_dl::Link::Source(ew_dl::Destination::Page(Default::default()));
        let item = items.into_iter().next().unwrap().with_link(link);
        assert_eq!(item.link, Some(link));
        let ew_dl::ItemKind::Group(group) = &item.kind else {
            panic!("a mixed line is a group")
        };
        let m = fonts.vertical_metrics(&style);
        assert_eq!(
            group.transform,
            Some(Transform::from_translation(Pt2::new(at.x, at.y + m.ascent)))
        );
        assert!(group.items.iter().all(|i| i.link.is_none() && i.entity.is_none()));
        let runs: Vec<&ShapedRun> = group
            .items
            .iter()
            .map(|i| match &i.kind {
                ew_dl::ItemKind::Glyphs(r) => &**r,
                other => panic!("a group member is a run, not {other:?}"),
            })
            .collect();
        // One run per font, in text order: the math face for the "∞", the style's face for the rest.
        let keys: Vec<_> = runs.iter().map(|r| r.font).collect();
        let want: Vec<_> = shaped.faces.iter().map(|f| f.font).collect();
        assert_eq!(keys, want);
        assert_ne!(keys[0], keys[1]);
        assert_eq!(runs[0].text, "\u{221E}");
        assert_eq!(runs[1].text, " 3-B2, 106-G3");
        assert_eq!(
            runs[0].glyphs.len() + runs[1].glyphs.len(),
            shaped.run.glyphs.len()
        );
        // The union of the boxes is the box a single run of the line would have had.
        let line = &shaped.lines[0];
        let single = Rect::from_points(
            Pt2::new(Nm::ZERO, Nm(-m.ascent.0)),
            Pt2::new(line.advance, m.descent),
        );
        assert_eq!(runs[0].layout_box.union(runs[1].layout_box), single);
        // The first run ends where the next one starts, which is where its first glyph is.
        assert_eq!(runs[0].layout_box.max().x, runs[1].layout_box.min().x);
        assert_eq!(runs[1].layout_box.min().x, runs[1].glyphs[0].x);
        // The members sit in the group's own frame: no origin, no turn.
        assert!(
            runs.iter()
                .all(|r| r.origin == Pt2::ORIGIN && r.transform == Transform::IDENTITY)
        );
    }

    #[test]
    fn ellipsis_cut_equals_the_one_char_at_a_time_reference() {
        let fonts = bundled();
        let texts = [
            "Power Supply 1 - Main Current",
            "A page title far too long for the large description cell of the title block.",
            "Zuführung für Überwachung — größere Längen",
            "Température 0.75 mm² ±5 % (€ 12)",
            "Trailing spaces   here    and   there   ",
            "   leading spaces and more words after them",
            "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
            "iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii",
            "x",
            "ab",
            "日本語のテキストと mixed ⇄ glyphs ∞ missing",
            "emoji 🙂🙂🙂 and combining e\u{301}e\u{301}e\u{301}e\u{301} marks",
            "a\tb\u{7}c\u{FE0F}d",
            "AVAVAVAVAVAVAVAVAVAVAVAVAVAVAVAVAV To To To Ty Ty",
        ];
        let style = TextStyle::regular(Nm(3_175_000));
        let mut compared = 0;
        for text in texts {
            let full = fonts.shape(text, &style, None).advance().0;
            // Cell widths from "nothing fits" up to "everything fits".
            for avail in [
                1,
                2_000_000,
                5_000_000,
                9_000_000,
                15_000_000,
                30_000_000,
                full - 1,
                full + 1,
            ] {
                let avail_w = Nm(avail.max(1));
                let rect = Rect::from_points(
                    Pt2::ORIGIN,
                    Pt2::new(Nm(avail_w.0 + 2 * CELL_PADDING.0), Nm(8_000_000)),
                );
                let spec = BoxSpec {
                    rect,
                    halign: -1,
                    valign: 0,
                    padding: CELL_PADDING,
                    overflow: Some(&Overflow::Ellipsis),
                    max_lines: 1,
                    sheet: SheetMap::IDENTITY,
                };
                let laid = lay_out(fonts, text, &style, &spec);
                if fonts.shape(text, &style, None).advance() <= avail_w {
                    assert_eq!(laid.overflow, None, "{text:?} at {avail}");
                    assert_eq!(laid.shaped.run.text, text);
                    continue;
                }
                let want = reference_ellipsis(fonts, text, &style, avail_w);
                assert_eq!(laid.overflow, Some(OverflowKind::Truncated));
                assert_eq!(laid.shaped.run.text, want, "{text:?} at {avail}");
                assert!(want.ends_with(ELLIPSIS));
                compared += 1;
            }
        }
        assert!(compared >= 60, "{compared}");
    }
}
