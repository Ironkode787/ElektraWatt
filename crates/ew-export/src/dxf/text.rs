// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The texts of a DXF page (doc 09 §4.8, §4.9): a glyph run as a `TEXT` at the start of its glyphs on its baseline, as
//! high as its face's capitals, at its angle, with the width factor that makes its face's own advances add up to its
//! width in the PDF; a device's text as an `ATTRIB` of its insert and an `ATTDEF` of its block; a link source as a
//! hyperlink (extended data of `PE_URL`); a face's `STYLE` with its family name (extended data of `ACAD`).

use std::fmt::Write as _;

use ew_dl::num::atan2_deg;
use ew_dl::{Brush, ShapedRun, Transform};
use ew_units::{Nm, Pt2, arith};
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use super::{DxfFace, mm, real};

/// A text as an entity writes it, in the frame of the space it is written in (DXF's: nanometres, y up).
#[derive(Clone, PartialEq, Debug)]
pub(super) struct Placed {
    /// The start of its first glyph on its baseline.
    pub x: i64,
    /// See [`Placed::x`].
    pub y: i64,
    /// The height of its face's capitals.
    pub height: i64,
    /// The angle of its baseline in degrees, counter-clockwise, in `[0, 360)`.
    pub angle: f64,
    /// The width factor (group 41), when it is not 1.
    pub factor: Option<f64>,
    /// The `STYLE` of its face.
    pub style: String,
    /// The text, as it prints.
    pub text: String,
    /// A colour other than black, as a true colour.
    pub colour: Option<u32>,
    /// The file of the page its link leads to.
    pub link: Option<String>,
}

impl Placed {
    /// This text in the frame of a block inserted at `(ax, ay)` at the scale `scale_ppb` (parts per 10⁹), unturned:
    /// where the insert draws it where it is now.
    pub(super) fn in_block(&self, ax: i64, ay: i64, scale_ppb: u32) -> Placed {
        Placed {
            x: unscale(self.x - ax, scale_ppb),
            y: unscale(self.y - ay, scale_ppb),
            height: unscale(self.height, scale_ppb),
            ..self.clone()
        }
    }

    /// Whether `other` is this text within a micrometre: what one insert's own texts must be to share a block's.
    pub(super) fn near(&self, other: &Placed) -> bool {
        let close = |a: i64, b: i64| (a - b).abs() <= 1_000;
        self.text == other.text
            && self.style == other.style
            && self.colour == other.colour
            && close(self.x, other.x)
            && close(self.y, other.y)
            && close(self.height, other.height)
            && (self.angle - other.angle).abs() < 1e-6
            && match (self.factor, other.factor) {
                (Some(a), Some(b)) => (a - b).abs() < 1e-6,
                (a, b) => a == b,
            }
    }
}

/// `n × 10⁹ / scale_ppb`, rounded to the nearest nanometre (ties toward zero); a scale of 0 counts as 1 ppb.
fn unscale(n: i64, scale_ppb: u32) -> i64 {
    let num = i128::from(n) * 1_000_000_000;
    let den = i128::from(scale_ppb.max(1));
    let q = arith::div_round_ties_toward_zero(num, den);
    i64::try_from(q).unwrap_or(if q < 0 { i64::MIN } else { i64::MAX })
}

/// Where a run under `t` starts on the sheet (its first glyph on the baseline: the start of its layout box) and the
/// angle of its baseline as DXF counts it (counter-clockwise, y up; the visual angle on the y-down sheet is the same).
pub(super) fn start(run: &ShapedRun, t: Transform) -> (Pt2, f64) {
    let rt = run
        .transform
        .then(Transform::from_translation(run.origin))
        .then(t);
    let x0 = run.layout_box.min().x;
    let at = rt.apply(Pt2::new(x0, Nm(0)));
    let ahead = rt.apply(Pt2::new(x0 + Nm(1_000_000), Nm(0)));
    let angle = atan2_deg(-((ahead.y.0 - at.y.0) as f64), (ahead.x.0 - at.x.0) as f64);
    (at, angle.rem_euclid(360.0))
}

/// The height of a face's capitals at the em height `size` (§4.8).
pub(super) fn cap_height(size: Nm, face: &DxfFace) -> i64 {
    let cap = i128::from(size.0) * i128::from(face.cap.0) / i128::from(face.cap.1.max(1));
    i64::try_from(cap).unwrap_or(i64::MAX)
}

/// The width factor of a text drawn by the runs `parts`, each in its face: their widths in the PDF (the layout boxes,
/// kerning included) over their glyphs' own advances in their faces at their sizes, to six decimals; `None` when it is
/// 1, or when a face's file is not at hand or reads no advances.
pub(super) fn width_factor(parts: &[(&ShapedRun, &DxfFace)]) -> Option<f64> {
    let (mut span, mut natural) = (0.0, 0.0);
    for (run, face) in parts {
        let file = face.font.as_ref()?;
        let font = FontRef::from_index(&file.data, file.index).ok()?;
        let upem = font
            .metrics(Size::unscaled(), LocationRef::default())
            .units_per_em;
        if upem == 0 {
            return None;
        }
        let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        let units: f64 = run
            .glyphs
            .iter()
            .map(|g| f64::from(metrics.advance_width(GlyphId::new(g.id)).unwrap_or(0.0)))
            .sum();
        natural += units * run.size.0 as f64 / f64::from(upem);
        span += (run.layout_box.max().x.0 - run.layout_box.min().x.0) as f64;
    }
    if !(natural > 0.0 && span > 0.0) {
        return None;
    }
    let factor = (span / natural * 1e6).round_ties_even() / 1e6;
    (factor.is_finite() && factor != 1.0).then_some(factor)
}

/// The colour of a brush other than black, as a true colour.
pub(super) fn colour(brush: Brush) -> Option<u32> {
    let Brush::Solid(c) = brush;
    (c != ew_dl::Color::BLACK).then(|| (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b))
}

/// The text of a DXF string: UTF-8 as it is (AutoCAD 2007 and later read a file of `AC1021` or later so), a line break as
/// a space, `%%` doubled to `%%%` (AutoCAD's control codes begin with `%%`).
pub(super) fn dxf_text(s: &str) -> String {
    s.replace(['\n', '\r'], " ").replace("%%", "%%%")
}

/// The longest string of extended data (group 1000) a DWG holds: 255 bytes.
const XDATA_STRING: usize = 255;

/// `s` cut to at most [`XDATA_STRING`] bytes, at a character's end.
fn xdata_string(s: &str) -> &str {
    let mut end = s.len().min(XDATA_STRING);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// The common part of a `TEXT`, an `ATTRIB` and an `ATTDEF`: the colour, the subclass and the text's own groups,
/// `value` the string it prints (an `ATTDEF`'s default).
fn text_groups(p: &Placed, value: &str) -> String {
    let mut body = String::new();
    if let Some(c) = p.colour {
        let _ = write!(body, "420\n{c}\n");
    }
    let _ = write!(
        body,
        "100\nAcDbText\n 10\n{}\n 20\n{}\n 30\n0\n 40\n{}\n  1\n{}\n 50\n{}\n",
        mm(p.x),
        mm(p.y),
        mm(p.height),
        dxf_text(value),
        real(p.angle),
    );
    if let Some(f) = p.factor {
        let _ = write!(body, " 41\n{}\n", real(f));
    }
    let _ = write!(body, "  7\n{}\n", p.style);
    body
}

/// The hyperlink of a text that links (AutoCAD's `PE_URL`): the file of the page it leads to, the text its description.
fn hyperlink(p: &Placed) -> String {
    p.link.as_ref().map_or_else(String::new, |file| {
        format!(
            "1001\nPE_URL\n1000\n{}\n1002\n{{\n1000\n{}\n1002\n}}\n",
            dxf_text(xdata_string(file)),
            dxf_text(xdata_string(&p.text))
        )
    })
}

/// The body of a `TEXT`.
pub(super) fn text_body(p: &Placed) -> String {
    let mut body = text_groups(p, &p.text);
    body.push_str("100\nAcDbText\n");
    body.push_str(&hyperlink(p));
    body
}

/// The body of an `ATTRIB` named `tag`.
pub(super) fn attrib_body(p: &Placed, tag: &str) -> String {
    let mut body = text_groups(p, &p.text);
    let _ = write!(body, "100\nAcDbAttribute\n  2\n{}\n 70\n0\n", dxf_text(tag));
    body.push_str(&hyperlink(p));
    body
}

/// The body of an `ATTDEF` named `tag` where `p` stands: no default value and no prompt (a CAD program then prompts
/// with the name).
pub(super) fn attdef_body(p: &Placed, tag: &str) -> String {
    let mut body = text_groups(p, "");
    let _ = write!(
        body,
        "100\nAcDbAttributeDefinition\n  3\n\n  2\n{}\n 70\n0\n",
        dxf_text(tag)
    );
    body
}

/// The extended data of a face's `STYLE`, as AutoCAD writes a TrueType style: its family (1000) and the bold flag
/// (1071), by which a reader finds the installed face whatever its file is called.
pub(super) fn style_xdata(face: &DxfFace) -> String {
    let flags: u32 = if face.bold { 0x0200_0000 } else { 0 };
    format!(
        "1001\nACAD\n1000\n{}\n1071\n{flags}\n",
        dxf_text(xdata_string(&face.family))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_unscaled_into_a_block_rounds_to_the_nearest_nanometre() {
        assert_eq!(unscale(986_282_578, 986_282_578), 1_000_000_000);
        assert_eq!(unscale(-1_500_000, 500_000_000), -3_000_000);
        assert_eq!(unscale(5, 0), 5_000_000_000);
    }

    #[test]
    fn text_is_written_as_itself_in_utf8_but_for_line_breaks_and_control_codes() {
        assert_eq!(dxf_text("∞ 2,5 mm² Ää°"), "∞ 2,5 mm² Ää°");
        assert_eq!(dxf_text("50%%"), "50%%%");
        assert_eq!(dxf_text("Müller\nÖl\r"), "Müller Öl ");
    }

    #[test]
    fn extended_data_strings_are_cut_at_a_character_within_255_bytes() {
        let long = "ä".repeat(200);
        let cut = xdata_string(&long);
        assert!(cut.len() <= 255 && long.starts_with(cut) && cut.chars().all(|c| c == 'ä'));
        assert_eq!(xdata_string("103.dxf"), "103.dxf");
    }
}
