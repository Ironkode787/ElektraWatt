// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A diagram's free texts, shapes and images → page items (doc 14 §4.13), and where a QET text item lands
//! (doc 14 §4.4.2), which the texts placed elements print share.
//!
//! | QET | Page item |
//! |---|---|
//! | `<input>` | free text in its import style `qet.<size>…` (§4.10), left-aligned on [`text_anchor`] |
//! | `<shape type="Line">` | graphic line |
//! | `<shape type="Rectangle">` | graphic rectangle; with corner radii `rx ry` a polygon through the rounded corners ([`rounded`]) |
//! | `<shape type="Ellipse">` | graphic circle when its box is square, else a polygon through the ellipse ([`rounded`]) |
//! | `<shape type="Polygon">` | graphic polygon (`closed`) or polyline |
//! | `<image>` | image item over the scaled picture; its bytes become a project asset |
//!
//! A pen maps to the dash (`SolidLine` solid, `DashLine` dashed, `DotLine` dotted, `DashDotLine` and
//! `DashDotDotLine` dash-dot) and the stroke width (`widthF` px; 0 or absent: the page's wire stroke); a brush
//! other than `NoBrush` fills the shape. What a page item cannot carry is a report entry and the item is kept
//! without it: a pen colour other than black, a dash-dot-dot or missing pen, a brush pattern, an image's rotation.
//! Source keys: `qet:item:<folio key>#text<n>`, `#shape<n>`, `#image<n>` (n = position among the diagram's
//! texts, shapes or images).

use std::collections::BTreeSet;

use ew_interop_api::{Code, Severity};
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Bounds, FreeText, Graphic, GraphicShape, GraphicStroke, ImageItem, LineDash, PageItem, PageItemKind,
    Record, Sha256, ShapeCircle, ShapeLine, ShapePoints, ShapeRect,
};
use ew_units::{Angle2, Nm, Pt2, Rect};
use sha2::Digest;

use super::cx::{Findings, Ids};
use super::elements::orientation;
use super::model::{QetDiagram, QetImage, QetShape, TextAt};
use crate::decimal::{Decimal, div_round_half_even};
use crate::prims::text::{em_of, parse_qt_font, style_key, substitution};

/// The margin QET's text items keep around their text (Qt's default document margin), px.
pub const TEXT_MARGIN_PX: i64 = 4;

/// From the top of a text line to the middle between its cap height and its baseline, per mille of the em
/// (Qt's ascent of the substituted families, about 0.905 em, less half their cap height, about 0.716 em).
pub const LINE_MIDDLE_PER_MILLE: i128 = 547;

/// Half the distance between two lines, per mille of the em (Qt's line spacing, about 1.15 em).
pub const HALF_LINE_PER_MILLE: i128 = 575;

/// The point size of a QET text without a usable font: QET's default diagram font, 9 pt.
pub const DEFAULT_POINT_SIZE: i64 = 9;

/// What the page-item mapper emits.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ItemsOut {
    /// Page-item records.
    pub records: Vec<Record>,
    /// The image files, each once.
    pub assets: Vec<Vec<u8>>,
}

/// A QET text as it prints: its plain text, style and em height. QET stores a text as written or as Qt rich
/// text (an HTML document, [`rich_text`]); the style is that of the HTML body's size and weight when it sets
/// them, else of the `font` attribute; an absent or unreadable font is QET's default 9 pt in the default
/// style. A substituted family is reported once per project (`fonts` holds the messages already given).
pub(crate) fn qet_text(
    raw: &str,
    font: Option<&str>,
    fonts: &mut BTreeSet<String>,
    findings: &mut Findings,
) -> (String, Option<Ident>, Nm) {
    let default = || em_of(Decimal::from_int(DEFAULT_POINT_SIZE)).unwrap_or(Nm::ZERO);
    let (text, html) = match rich_text(raw) {
        Some((text, size, bold)) => (text, Some((size, bold))),
        None => (raw.to_owned(), None),
    };
    let f = font.and_then(parse_qt_font);
    if let Some(message) = f.as_ref().and_then(|f| substitution(&f.family))
        && fonts.insert(message.clone())
    {
        findings.file_level(Code::FONT_SUBSTITUTED, Severity::Info, message);
    }
    let size = html.and_then(|(s, _)| s).or(f.as_ref().map(|f| f.point_size));
    let Some(size) = size else {
        return (text, None, default());
    };
    let bold = html.map_or(f.as_ref().is_some_and(|f| f.bold), |(_, b)| b);
    let italic = f.as_ref().is_some_and(|f| f.italic);
    let style = Ident::from_input(&style_key(size, bold, italic)).ok();
    (text, style, em_of(size).unwrap_or_else(default))
}

/// The plain text of Qt rich text (`<!DOCTYPE HTML …><html>…<body style="…">…</body></html>`), with its
/// `font-size` in points and whether its `font-weight` is bold (600 or more), each from the first span that
/// sets it, else from the body; `None` for a text that is not rich text. Each paragraph is a line and
/// `<br />` a line break; tags are dropped and the five XML entities and `&nbsp;` decoded. A text has one
/// style, so further spans that change the size or weight are not kept.
pub(crate) fn rich_text(raw: &str) -> Option<(String, Option<Decimal>, bool)> {
    let at = raw.find("<body")?;
    let body = &raw[at..];
    let open_end = body.find('>')?;
    let content = &body[open_end + 1..];
    let content = &content[..content.find("</body>").unwrap_or(content.len())];
    // The first span that sets a value wins over the body (a text in one span is common), else the body.
    let css = |name: &str| {
        let value = |s: &str| {
            let v = &s[s.find(name)? + name.len()..];
            Some(v[..v.find([';', '"'])?].trim().to_owned())
        };
        value(content).or_else(|| value(&body[..open_end]))
    };
    let size = css("font-size:").and_then(|v| Decimal::parse(v.strip_suffix("pt")?).ok());
    let bold = css("font-weight:")
        .and_then(|v| v.parse::<u32>().ok())
        .is_some_and(|w| w >= 600);
    let content = content.replace(['\n', '\r'], "");
    let mut lines: Vec<String> = Vec::new();
    for piece in content.split("</p>") {
        let Some(p) = piece.rfind("<p") else { continue };
        let inner = &piece[p..];
        let inner = &inner[inner.find('>').map_or(inner.len(), |i| i + 1)..];
        let inner = inner
            .replace("<br />", "\n")
            .replace("<br/>", "\n")
            .replace("<br>", "\n");
        let mut text = String::new();
        let mut in_tag = false;
        for c in inner.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                c if !in_tag => text.push(c),
                _ => {}
            }
        }
        let text = text
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&apos;", "'")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&");
        // An empty paragraph is written as one line break.
        lines.push(if text == "\n" { String::new() } else { text });
    }
    Some((lines.join("\n"), size, bold))
}

/// The point where line `line` (from 0) of a QET text item stands, in its parent's frame, and its angle: QET
/// places the top-left corner of the item's box at `(x, y)` and turns the item by `rotation` (clockwise)
/// about it; EW draws a free text or an import slot left-aligned on the middle of a line. So the point is
/// `(x, y)` plus the margin and, downward, [`LINE_MIDDLE_PER_MILLE`] of the em and twice
/// [`HALF_LINE_PER_MILLE`] per line before it, that offset turned by the rotation rounded to a quarter turn.
/// The angle is the negated rotation (Qt counts clockwise, `Angle2` counter-clockwise). `None` when `x` or
/// `y` is missing or out of range. A text of several lines is placed line by line, so that its lines keep
/// QET's spacing whatever EW's line spacing is.
pub(crate) fn text_anchor(at: &TextAt, line: usize, em: Nm) -> Option<(Pt2, Angle2)> {
    let x = at.x?.px_to_nm().ok()?;
    let y = at.y?.px_to_nm().ok()?;
    let margin = i128::from(Decimal::from_int(TEXT_MARGIN_PX).px_to_nm().ok()?.0);
    let line = i128::try_from(line).ok()?;
    let em = i128::from(em.0);
    let down = margin
        + div_round_half_even(em * LINE_MIDDLE_PER_MILLE, 1000)
        + div_round_half_even(em * HALF_LINE_PER_MILLE * 2 * line, 1000);
    let rotation = at.rotation.unwrap_or(Decimal::ZERO).deg_to_angle().ok()?.0;
    let quarters = u8::try_from(div_round_half_even(i128::from(rotation), 90_000).rem_euclid(4)).ok()?;
    let offset = orientation(quarters).apply(Pt2::new(
        Nm(i64::try_from(margin).ok()?),
        Nm(i64::try_from(down).ok()?),
    ));
    let point = Pt2::new(Nm(x.0.checked_add(offset.x.0)?), Nm(y.0.checked_add(offset.y.0)?));
    Some((point, Angle2(rotation.checked_neg()?)))
}

/// The page item `kind` with ID from `key` on the diagram's page.
fn item(ids: &Ids, page: ew_ids::Id<ew_schema::Page>, key: &str, kind: PageItemKind) -> Record {
    Record::PageItem(PageItem {
        id: ids.id(key),
        v: 1,
        page,
        layer: None,
        kind,
        extra: Default::default(),
    })
}

/// The source key of line `line` of the text keyed `key`: the key itself for the first line, `<key>.<line>`
/// for the others.
pub(crate) fn line_key(key: &str, line: usize) -> String {
    if line == 0 {
        key.to_owned()
    } else {
        format!("{key}.{line}")
    }
}

/// A free text of `text` at `at` (page coordinates) and `angle` (taken into 0 … 360°), in `style`.
pub(crate) fn free_text(text: &str, at: Pt2, angle: Angle2, style: Option<Ident>) -> PageItemKind {
    PageItemKind::FreeText(FreeText {
        text: MlText::neutral(text),
        bind: None,
        at,
        angle: Angle2(angle.0.rem_euclid(360_000)),
        style,
        align: None,
        extra: Default::default(),
    })
}

/// Maps the free texts, shapes and images of diagram `d` on `page` (folio `folio`, folio key `folio_key`).
pub(crate) fn map(
    d: &QetDiagram,
    page: ew_ids::Id<ew_schema::Page>,
    (folio, folio_key): (&str, &str),
    ids: &Ids,
    fonts: &mut BTreeSet<String>,
    findings: &mut Findings,
    out: &mut ItemsOut,
) {
    let note = |findings: &mut Findings, line: u32, item: String, code: Code, message: String| {
        let loc = findings.loc(Some(folio), Some(item), Some(line));
        let severity = if code == Code::MALFORMED_ITEM {
            Severity::Warning
        } else {
            Severity::Info
        };
        findings.push(code, severity, loc, message);
    };
    for (n, t) in d.free_texts.iter().enumerate() {
        let (text, style, em) = qet_text(&t.text, t.at.font.as_deref(), fonts, findings);
        for (i, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let Some((at, angle)) = text_anchor(&t.at, i, em) else {
                note(
                    findings,
                    t.line,
                    format!("text {n}"),
                    Code::MALFORMED_ITEM,
                    "free text skipped: bad x/y".into(),
                );
                break;
            };
            let key = line_key(&format!("qet:item:{folio_key}#text{n}"), i);
            out.records.push(item(
                ids,
                page,
                &key,
                free_text(line.trim_end(), at, angle, style.clone()),
            ));
        }
    }
    for (n, s) in d.shapes.iter().enumerate() {
        let what = format!("shape {n}");
        match graphic(s) {
            Ok((g, notes)) => {
                for m in notes {
                    note(findings, s.line, what.clone(), Code::DROPPED_BY_DESIGN, m);
                }
                let key = format!("qet:item:{folio_key}#shape{n}");
                out.records.push(item(ids, page, &key, PageItemKind::Graphic(g)));
            }
            Err(m) => note(
                findings,
                s.line,
                what,
                Code::MALFORMED_ITEM,
                format!("shape skipped: {m}"),
            ),
        }
    }
    for (n, i) in d.images.iter().enumerate() {
        let what = format!("image {n}");
        match image(i, out) {
            Ok((sha, rect)) => {
                if i.rotation.is_some_and(|r| !r.is_zero()) {
                    note(
                        findings,
                        i.line,
                        what,
                        Code::DROPPED_BY_DESIGN,
                        "image rotation dropped: an image item is not turned".into(),
                    );
                }
                let key = format!("qet:item:{folio_key}#image{n}");
                let kind = PageItemKind::Image(ImageItem {
                    asset: sha,
                    rect,
                    extra: Default::default(),
                });
                out.records.push(item(ids, page, &key, kind));
            }
            Err(m) => note(
                findings,
                i.line,
                what,
                Code::MALFORMED_ITEM,
                format!("image skipped: {m}"),
            ),
        }
    }
}

/// A px decimal in nm.
fn nm(d: Decimal) -> Result<Nm, String> {
    d.px_to_nm().map_err(|e| e.to_string())
}

/// The graphic of a shape and the notes on what it could not carry.
fn graphic(s: &QetShape) -> Result<(Graphic, Vec<String>), String> {
    let mut notes = Vec::new();
    let points = s
        .points
        .iter()
        .map(|p| {
            let (x, y) = p.ok_or("a point without x/y")?;
            Ok(Pt2::new(nm(x)?, nm(y)?))
        })
        .collect::<Result<Vec<Pt2>, String>>()?;
    let corners = || match points.as_slice() {
        [a, b, ..] => Ok(Rect::from_points(*a, *b)),
        _ => Err("fewer than two points".to_owned()),
    };
    let shape = match s.kind.as_str() {
        "Line" | "0" => match points.as_slice() {
            [a, b, ..] => GraphicShape::Line(ShapeLine {
                a: *a,
                b: *b,
                extra: Default::default(),
            }),
            _ => return Err("fewer than two points".into()),
        },
        "Rectangle" | "1" => {
            let r = corners()?;
            let radius = |d: Option<Decimal>, half: i64| -> Result<i64, String> {
                Ok(nm(d.unwrap_or(Decimal::ZERO))?.0.abs().min(half))
            };
            let rx = radius(s.radii.0, r.width().0 / 2)?;
            let ry = radius(s.radii.1, r.height().0 / 2)?;
            if rx == 0 || ry == 0 {
                GraphicShape::Rect(ShapeRect {
                    min: r.min(),
                    max: r.max(),
                    extra: Default::default(),
                })
            } else {
                polygon(rounded(r, rx, ry))
            }
        }
        "Ellipse" | "2" => {
            let r = corners()?;
            if r.width() == r.height() {
                GraphicShape::Circle(ShapeCircle {
                    centre: Pt2::new(r.min().x + Nm(r.width().0 / 2), r.min().y + Nm(r.height().0 / 2)),
                    r: Nm(r.width().0 / 2),
                    extra: Default::default(),
                })
            } else {
                polygon(rounded(r, r.width().0 / 2, r.height().0 / 2))
            }
        }
        "Polygon" | "3" => {
            if points.len() < 2 {
                return Err("fewer than two points".into());
            }
            let p = ShapePoints {
                points,
                extra: Default::default(),
            };
            if s.closed {
                GraphicShape::Polygon(p)
            } else {
                GraphicShape::Polyline(p)
            }
        }
        other => return Err(format!("shape type {other:?} not understood")),
    };
    let dash = match s.pen_style.as_str() {
        "" | "SolidLine" => LineDash::Solid,
        "DashLine" => LineDash::Dashed,
        "DotLine" => LineDash::Dotted,
        "DashDotLine" => LineDash::DashDot,
        "DashDotDotLine" => {
            notes.push("dash-dot-dot line drawn dash-dot".to_owned());
            LineDash::DashDot
        }
        other => {
            notes.push(format!("pen style {other} drawn solid"));
            LineDash::Solid
        }
    };
    if !matches!(
        s.pen_color.to_ascii_lowercase().as_str(),
        "" | "#000000" | "#ff000000" | "black"
    ) {
        notes.push(format!("pen colour {} drawn black", s.pen_color));
    }
    let fill = match s.brush_style.as_str() {
        "" | "NoBrush" => false,
        "SolidPattern" => true,
        other => {
            notes.push(format!("brush {other} drawn solid"));
            true
        }
    };
    let stroke = match s.pen_width.filter(|w| !w.is_zero() && !w.is_negative()) {
        Some(w) => Some(GraphicStroke {
            width: nm(w)?,
            extra: Default::default(),
        }),
        None => None,
    };
    Ok((
        Graphic {
            shape,
            stroke,
            dash,
            fill,
            extra: Default::default(),
        },
        notes,
    ))
}

/// A closed polygon through `points`.
fn polygon(points: Vec<Pt2>) -> GraphicShape {
    GraphicShape::Polygon(ShapePoints {
        points,
        extra: Default::default(),
    })
}

/// Cosine of 0°, 15°, … 90°, in millionths (the sine is the same table backwards): seven points per quarter,
/// in integers, so the import is the same on every platform.
const QUARTER: [i128; 7] = [1_000_000, 965_926, 866_025, 707_107, 500_000, 258_819, 0];

/// The outline of rectangle `r` with elliptic corners of radii `rx`, `ry` (at most half the sides; with
/// both at half, an ellipse), clockwise from the left end of the top-left corner, without repeated points.
pub(crate) fn rounded(r: Rect, rx: i64, ry: i64) -> Vec<Pt2> {
    let (rx, ry) = (i128::from(rx), i128::from(ry));
    let (x0, y0) = (i128::from(r.min().x.0), i128::from(r.min().y.0));
    let (x1, y1) = (i128::from(r.max().x.0), i128::from(r.max().y.0));
    let s = |v: i128, f: i128| div_round_half_even(v * f, 1_000_000);
    let mut out: Vec<Pt2> = Vec::with_capacity(28);
    for k in 0..4 {
        for i in 0..7 {
            let (c, n) = (QUARTER[i], QUARTER[6 - i]);
            let (x, y) = match k {
                0 => (x0 + rx - s(rx, c), y0 + ry - s(ry, n)),
                1 => (x1 - rx + s(rx, n), y0 + ry - s(ry, c)),
                2 => (x1 - rx + s(rx, c), y1 - ry + s(ry, n)),
                _ => (x0 + rx - s(rx, n), y1 - ry + s(ry, c)),
            };
            let p = Pt2::new(
                Nm(i64::try_from(x).unwrap_or(i64::MAX)),
                Nm(i64::try_from(y).unwrap_or(i64::MAX)),
            );
            if out.last() != Some(&p) && out.first() != Some(&p) {
                out.push(p);
            }
        }
    }
    out
}

impl ItemsOut {
    /// Takes an image file given as base64 text (white space removed) into the batch's assets, once per
    /// content, and returns its SHA-256 and its pixel size (`None` for an SVG without one). The bytes are held
    /// to the limits an image given as data to `item.place` meets: at most 512 KiB, a known format, the pixel
    /// limit; `import.apply` stores them as project assets. The one path for every image a QET project
    /// brings (the diagram's images; a title-block logo).
    ///
    /// # Errors
    ///
    /// The reason in words when the text is not base64 or the file is refused.
    pub(crate) fn store_image(&mut self, base64: &str) -> Result<(Sha256, Option<(u32, u32)>), String> {
        let bytes = ew_schema::base64_decode(base64).map_err(|e| e.to_string())?;
        if bytes.len() > ew_schema::MAX_LOGO_BYTES {
            return Err(format!(
                "the image file has {} bytes, more than the limit of {}",
                bytes.len(),
                ew_schema::MAX_LOGO_BYTES
            ));
        }
        let (format, size) = ew_schema::sniff(&bytes).map_err(|e| e.to_string())?;
        ew_schema::pixel_limit(format, size).map_err(|e| e.to_string())?;
        let sha = Sha256(sha2::Sha256::digest(&bytes).into());
        if !self
            .assets
            .iter()
            .any(|a| Sha256(sha2::Sha256::digest(a).into()) == sha)
        {
            self.assets.push(bytes);
        }
        Ok((sha, size))
    }
}

/// The asset of an image ([`ItemsOut::store_image`]) and the rectangle QET draws it in: the picture's own
/// size in px scaled by `size` about its centre (QET turns and scales an image about its middle), the item
/// at `(x, y)`.
fn image(i: &QetImage, out: &mut ItemsOut) -> Result<(Sha256, Bounds), String> {
    let (x, y) = i.x.zip(i.y).ok_or("bad x/y")?;
    let (sha, size) = out.store_image(&i.data)?;
    let (w, h) = size.ok_or("the image has no size of its own")?;
    let scale = i.scale.unwrap_or(Decimal::from_int(1));
    let corner = |pos: Decimal, len: u32, sign: i64| -> Result<Nm, String> {
        let half = Decimal::from_int(i64::from(len))
            .half()
            .map_err(|e| e.to_string())?;
        let reach = half
            .checked_mul(scale)
            .and_then(|r| r.checked_mul(Decimal::from_int(sign)))
            .map_err(|e| e.to_string())?;
        nm(pos
            .checked_add(half)
            .and_then(|c| c.checked_add(reach))
            .map_err(|e| e.to_string())?)
    };
    let a = Pt2::new(corner(x, w, -1)?, corner(y, h, -1)?);
    let b = Pt2::new(corner(x, w, 1)?, corner(y, h, 1)?);
    Ok((sha, Bounds(Rect::from_points(a, b))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ellipse_outline_touches_its_box_on_four_sides() {
        let r = Rect::from_points(Pt2::new(Nm(0), Nm(0)), Pt2::new(Nm(400), Nm(200)));
        let p = rounded(r, 200, 100);
        for want in [
            Pt2::new(Nm(0), Nm(100)),
            Pt2::new(Nm(200), Nm(0)),
            Pt2::new(Nm(400), Nm(100)),
            Pt2::new(Nm(200), Nm(200)),
        ] {
            assert!(p.contains(&want), "{want:?} in {p:?}");
        }
        assert!(p.iter().all(|q| r.contains(*q)), "{p:?}");
        let unique: BTreeSet<_> = p.iter().map(|q| (q.x, q.y)).collect();
        assert_eq!(unique.len(), p.len(), "no repeated point");
    }
}
