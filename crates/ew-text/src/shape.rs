// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shaping: a string and a [`TextStyle`] to a [`ShapedRun`] in integer nanometres.

use std::ops::Range;
use std::sync::Arc;

use ew_dl::{Brush, FontKey, Glyph, ShapedRun, Transform};
use ew_units::{Nm, Pt2, Rect};
use parley::{
    Alignment, AlignmentOptions, FontFamily, FontStyle, FontWeight, Layout, LineHeight, PositionedLayoutItem,
    StyleProperty,
};
use skrifa::MetadataProvider;

use crate::fonts::{Family, FontSet, Weight, engine_family};

/// What a text is shaped with: a family role, the em height and the face.
///
/// `size` is the **em height in nm** (the font size), in the page's coordinates; a text style table maps
/// style keys to it ([`TextStyleTable`](crate::TextStyleTable)).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TextStyle {
    /// Family role.
    pub family: Family,
    /// Em height in nm.
    pub size: Nm,
    /// Weight.
    pub weight: Weight,
    /// Italic.
    pub italic: bool,
}

impl TextStyle {
    /// A regular upright style of the default family.
    pub const fn regular(size: Nm) -> TextStyle {
        TextStyle {
            family: Family::Default,
            size,
            weight: Weight::Regular,
            italic: false,
        }
    }

    /// This style at another size.
    pub const fn with_size(self, size: Nm) -> TextStyle {
        TextStyle { size, ..self }
    }
}

/// One line of a shaped text.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LineSpan {
    /// The line's glyphs in [`ShapedRun::glyphs`].
    pub glyphs: Range<usize>,
    /// The line's byte range in the source text.
    pub text: Range<usize>,
    /// The line's advance (pen travel, trailing spaces included), nm.
    pub advance: Nm,
    /// The line's baseline in run space (0 for the first line, y down).
    pub baseline: Nm,
}

/// A stretch of glyphs of one line that one face drew: the style's own face or the symbol fallback.
///
/// The spans of a [`Shaped`] partition [`ShapedRun::glyphs`] in order, and no span crosses a line. A text
/// without fallback glyphs has one span per line, all with the style's face.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FaceSpan {
    /// The face that drew these glyphs.
    pub font: FontKey,
    /// The glyphs, as an index range of [`ShapedRun::glyphs`].
    pub glyphs: Range<usize>,
    /// The source text they stand for, as a byte range of [`ShapedRun::text`].
    pub text: Range<usize>,
}

/// The result of [`FontSet::shape`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Shaped {
    /// The glyphs, at origin `(0, 0)` with the identity transform and a black brush; callers place and
    /// colour a copy. Positions are in run space: nm, y down, first baseline at y = 0.
    pub run: Arc<ShapedRun>,
    /// The lines (one unless `max_width` broke the text or it contains line breaks).
    pub lines: Vec<LineSpan>,
    /// Which face drew which glyphs. [`ShapedRun::font`] is the style's face; glyphs the face lacks come
    /// from the symbol fallback and show up here with that face's key. A writer or renderer that draws
    /// `run` must draw each span with its own font.
    pub faces: Vec<FaceSpan>,
    /// The characters neither the selected face nor the symbol fallback has a glyph for, in
    /// first-occurrence order, each once (FR-OUT-027). They are drawn as the selected face's `.notdef`
    /// glyph.
    pub missing: Vec<char>,
}

impl Shaped {
    /// The widest line's advance.
    pub fn advance(&self) -> Nm {
        self.lines.iter().map(|l| l.advance).max().unwrap_or(Nm::ZERO)
    }
}

impl FontSet {
    /// Shapes `text` in `style`, breaking lines greedily at `max_width` if given.
    ///
    /// Deterministic: the same font bytes, text, style and width give the same glyph ids and positions on
    /// every platform (see the crate documentation for the exact numeric pipeline). There is no cache
    /// here; memoisation belongs to the `shaped_text` query of `ew-memo` (WP-S1.05).
    ///
    /// A text without line breaks and without `max_width` gives one line; `\n` always breaks. A
    /// `max_width` narrower than a single word lets that word overflow (parley never breaks inside a word
    /// unless no other opportunity exists).
    pub fn shape(&self, text: &str, style: &TextStyle, max_width: Option<Nm>) -> Shaped {
        let face = self.select(style.family, style.weight, style.italic);
        let m = face.info.metrics;
        let upem = m.units_per_em;
        let size = style.size.0.max(0);
        let to_nm = |v: f32| Nm(units_to_nm(v, size, upem));
        // Shaping happens at a font size equal to the units per em, so parley's scale factor is exactly 1
        // and every position is a whole number of font units (see the crate documentation).
        let max_units = max_width.map(|w| nm_to_units_floor(w, size, upem));

        let mut engine = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        let engine = &mut *engine;
        let mut builder = engine.layout.ranged_builder(&mut engine.fonts, text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(engine_family(
            face.info.family,
        ))));
        builder.push_default(StyleProperty::FontSize(f32::from(upem)));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from(
            face.info.weight.value(),
        ))));
        builder.push_default(StyleProperty::FontStyle(if face.info.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(
            m.line_pitch() as f32
        )));
        let mut layout: Layout<()> = builder.build(text);
        layout.break_all_lines(max_units);
        layout.align(Alignment::Start, AlignmentOptions::default());

        let mut glyphs = Vec::new();
        let mut spans: Vec<FaceSpan> = Vec::new();
        let mut lines = Vec::new();
        let mut first_baseline: Option<f32> = None;
        for line in layout.lines() {
            let baseline = line.metrics().baseline;
            let base0 = *first_baseline.get_or_insert(baseline);
            let start = glyphs.len();
            let first_span = spans.len();
            let mut end_units = 0f32;
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let drawn_by = self.face_of(run.run().font()).unwrap_or(face).info.key;
                let run_start = glyphs.len();
                for g in run.positioned_glyphs() {
                    glyphs.push(Glyph {
                        id: g.id,
                        x: to_nm(g.x),
                        y: to_nm(g.y - base0),
                    });
                    end_units = end_units.max(g.x + g.advance);
                }
                end_units = end_units.max(run.offset() + run.advance());
                if glyphs.len() > run_start {
                    // Runs of one face that follow each other (a line split into several runs) merge.
                    let text = run.run().text_range();
                    match spans[first_span..].last_mut() {
                        Some(last) if last.font == drawn_by => {
                            last.glyphs.end = glyphs.len();
                            last.text.end = text.end.max(last.text.end);
                        }
                        _ => spans.push(FaceSpan {
                            font: drawn_by,
                            glyphs: run_start..glyphs.len(),
                            text,
                        }),
                    }
                }
            }
            lines.push(LineSpan {
                glyphs: start..glyphs.len(),
                text: line.text_range(),
                advance: to_nm(end_units),
                baseline: to_nm(baseline - base0),
            });
        }
        if lines.is_empty() {
            lines.push(LineSpan {
                glyphs: 0..0,
                text: 0..text.len(),
                advance: Nm::ZERO,
                baseline: Nm::ZERO,
            });
        }
        let ascent = Nm(units_to_nm(m.ascent as f32, size, upem));
        let descent = Nm(units_to_nm(m.descent as f32, size, upem));
        let last = lines.last().map_or(Nm::ZERO, |l| l.baseline);
        let width = lines.iter().map(|l| l.advance).max().unwrap_or(Nm::ZERO);
        let layout_box =
            Rect::from_points(Pt2::new(Nm::ZERO, Nm(-ascent.0)), Pt2::new(width, last + descent));
        let run = ShapedRun {
            font: face.info.key,
            size: Nm(size),
            origin: Pt2::ORIGIN,
            transform: Transform::IDENTITY,
            glyphs,
            text: text.to_owned(),
            brush: Brush::BLACK,
            layout_box,
        };
        let missing = self.missing_chars(text, &spans);
        Shaped {
            run: Arc::new(run),
            lines,
            faces: spans,
            missing,
        }
    }

    /// The vertical metrics of the face `style` selects, in nm at `style.size`: (ascent, descent, cap
    /// height, line pitch), each positive.
    pub fn vertical_metrics(&self, style: &TextStyle) -> VerticalMetrics {
        let m = self.select(style.family, style.weight, style.italic).info.metrics;
        let size = style.size.0.max(0);
        let n = |u: i32| Nm(units_to_nm(u as f32, size, m.units_per_em));
        VerticalMetrics {
            ascent: n(m.ascent),
            descent: n(m.descent),
            cap_height: n(m.cap_height),
            line_pitch: n(m.line_pitch()),
        }
    }
}

/// The vertical metrics of a style in nm ([`FontSet::vertical_metrics`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VerticalMetrics {
    /// Ascent above the baseline.
    pub ascent: Nm,
    /// Descent below the baseline.
    pub descent: Nm,
    /// Cap height.
    pub cap_height: Nm,
    /// Baseline-to-baseline distance of wrapped lines.
    pub line_pitch: Nm,
}

/// Code points with the Unicode `Default_Ignorable_Code_Point` property (UAX #44,
/// `DerivedCoreProperties.txt`) that a text stack renders invisibly: soft hyphen, joiners and marks,
/// variation selectors, bidi controls, the byte order mark and similar. A face needs no glyph for them, so
/// lacking one is not a missing glyph for [`Shaped::missing`]. This is a small explicit list of the ranges
/// below, not the full property; no Unicode data crate is involved.
const DEFAULT_IGNORABLE: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x061C, 0x061C),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xE0100, 0xE01EF),
];

fn is_default_ignorable(ch: char) -> bool {
    let c = u32::from(ch);
    DEFAULT_IGNORABLE.iter().any(|&(lo, hi)| (lo..=hi).contains(&c))
}

/// Characters that end a line instead of becoming a glyph (the mandatory breaks of the line breaker).
fn is_line_break(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

impl FontSet {
    /// The characters of `text` that were drawn as the `.notdef` glyph (id 0, a visible box), first
    /// occurrence order, each once, derived from what was shaped: a character is missing when the face that
    /// drew its stretch of text ([`FaceSpan`]) has no glyph for it. A character the symbol fallback drew is
    /// not missing; a character in a script without a fallback (the fallback serves Latin, Greek and
    /// Cyrillic text only) is drawn by the style's face and is missing there even if the fallback face
    /// maps it. This includes control characters such as tabs, which are shaped to `.notdef` like any other
    /// unmapped character. Not reported: line breaks (they end a line and produce no glyph) and
    /// default-ignorable code points ([`DEFAULT_IGNORABLE`]), which render invisibly.
    fn missing_chars(&self, text: &str, spans: &[FaceSpan]) -> Vec<char> {
        let mut out: Vec<char> = Vec::new();
        for span in spans {
            let Some(face) = self.faces.iter().find(|f| f.info.key == span.font) else {
                continue;
            };
            let font = face.font_ref();
            let charmap = font.charmap();
            for ch in text.get(span.text.clone()).unwrap_or_default().chars() {
                if charmap.map(ch).is_none()
                    && !is_line_break(ch)
                    && !is_default_ignorable(ch)
                    && !out.contains(&ch)
                {
                    out.push(ch);
                }
            }
        }
        out
    }
}

/// `v` font units at em height `size_nm` in nm, for a face with `upem` units per em:
/// `round(v × size_nm / upem)` with exact ties **toward zero**.
///
/// The `f32` is decomposed into its exact binary value `m × 2^e` and the product and quotient are formed in
/// `i128`, so no floating-point rounding occurs at all: the result depends only on the bits of `v`.
/// Non-finite values give 0.
pub fn units_to_nm(v: f32, size_nm: i64, upem: u16) -> i64 {
    if !v.is_finite() || v == 0.0 || upem == 0 {
        return 0;
    }
    let bits = v.to_bits();
    let negative = bits >> 31 == 1;
    let biased = ((bits >> 23) & 0xff) as i32;
    let frac = i128::from(bits & 0x7f_ffff);
    let (mantissa, exp) = if biased == 0 {
        (frac, -149)
    } else {
        (frac | 0x80_0000, biased - 150)
    };
    let mut num = mantissa * i128::from(size_nm);
    let mut den = i128::from(upem);
    if exp >= 0 {
        // |v| ≥ 2^23 font units: far beyond any text; saturate rather than overflow.
        num = num.checked_shl(exp as u32).unwrap_or(i128::MAX / 4);
    } else {
        let s = (-exp) as u32;
        if s > 64 {
            return 0; // |v| < 2^-40 font units: below 1 nm for any em height below 2^40 nm.
        }
        den <<= s;
    }
    let q = div_round_toward_zero(num, den);
    let q = if negative { -q } else { q };
    i64::try_from(q).unwrap_or(if q < 0 { i64::MIN } else { i64::MAX })
}

/// The largest whole number of font units not exceeding `w` nm at em height `size_nm`, as `f32` (exact:
/// widths are far below 2^24 units). A non-positive size gives 0.
fn nm_to_units_floor(w: Nm, size_nm: i64, upem: u16) -> f32 {
    if size_nm <= 0 {
        return 0.0;
    }
    let u = (i128::from(w.0.max(0)) * i128::from(upem)).div_euclid(i128::from(size_nm));
    u.min(1 << 24) as f32
}

/// `n / d` (d > 0) rounded to the nearest integer, exact ties toward zero.
pub(crate) fn div_round_toward_zero(n: i128, d: i128) -> i128 {
    let q = n / d;
    let r = n % d;
    if 2 * r.abs() > d { q + n.signum() } else { q }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_to_nm_is_exact() {
        // 1000 units at a 3.175 mm em with upem 1000 → 3.175 mm.
        assert_eq!(units_to_nm(1000.0, 3_175_000, 1000), 3_175_000);
        assert_eq!(units_to_nm(-1000.0, 3_175_000, 1000), -3_175_000);
        // 1 unit at 1 nm per 2 units: 0.5 nm, a tie, goes toward zero.
        assert_eq!(units_to_nm(1.0, 1, 2), 0);
        assert_eq!(units_to_nm(3.0, 1, 2), 1);
        assert_eq!(units_to_nm(-3.0, 1, 2), -1);
        // Fractions are exact: 0.5 units × 3 nm / 1 = 1.5 nm → 1.
        assert_eq!(units_to_nm(0.5, 3, 1), 1);
        assert_eq!(units_to_nm(0.75, 4, 1), 3);
        assert_eq!(units_to_nm(f32::NAN, 4, 1), 0);
        assert_eq!(units_to_nm(1e-30, 4, 1), 0);
    }

    #[test]
    fn floor_units() {
        assert_eq!(nm_to_units_floor(Nm(3_175_000), 3_175_000, 1000), 1000.0);
        assert_eq!(nm_to_units_floor(Nm(3_174_999), 3_175_000, 1000), 999.0);
        assert_eq!(nm_to_units_floor(Nm(-5), 3_175_000, 1000), 0.0);
    }
}
