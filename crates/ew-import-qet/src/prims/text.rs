// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text primitives: static `text`, `dynamic_text` and the legacy `input` (doc 14 §4.3, §4.10).
//!
//! # Angles
//!
//! QET's `rotation` is Qt's angle, **clockwise-positive** on screen; `Angle2` is
//! counter-clockwise-positive (ADR-0005 amendment 2), so every text angle is the **negated** decimal:
//! `rotation="90"` → `Angle2(-90_000)`. Absent = 0.
//!
//! # Static `text`
//!
//! [`Primitive::Text`] with `pos = (x, y)` relative to the hotspot, `text = MlText::neutral(text)`,
//! `align = Align::BottomLeft` (QET draws static texts from their baseline origin; verify on the corpus),
//! `angle = −rotation`, `style` from [`font_style`]. `extra`: `"qet.font"` = the raw `font` attribute if
//! present; `"qet.color"` = the `color` attribute if it is present and not `black`, which is also reported
//! (EW-IMP-004, info: "text colour `<c>` dropped"). A missing `text` attribute is an empty text.
//!
//! # `dynamic_text`
//!
//! `pos = (x, y)`, `angle = −rotation`, alignment from `Halignment` × `Valignment`:
//!
//! | | `AlignLeft` (default) | `AlignHCenter` | `AlignRight` |
//! |---|---|---|---|
//! | `AlignTop` (default) | `TopLeft` | `TopCentre` | `TopRight` |
//! | `AlignVCenter` | `MiddleLeft` | `Centre` | `MiddleRight` |
//! | `AlignBottom` | `BottomLeft` | `BottomCentre` | `BottomRight` |
//!
//! An unknown alignment value counts as the default and is reported (EW-IMP-004).
//!
//! | `text_from` | Result |
//! |---|---|
//! | `UserText` (default) | static [`Primitive::Text`] with the content of the `<text>` child (empty if absent), the alignment above, `style` from `font` |
//! | `ElementInfo` | [`TextSlot`] whose kind comes from the `<info_name>` child by [`slot_kind_for_info`] |
//! | `CompositeText` | [`TextSlot`] of kind `UserProp("qet.composite")` with `extra["qet.format"]` = the `<composite_text>` child's text verbatim (QET's `%{name}` format) |
//! | anything else | [`ItemError`] |
//!
//! A slot has `rot = −rotation`, the alignment above, `visible = true`, `max_width` = `text_width` converted
//! when it is greater than 0 (else `None`). Slot `extra` (only the members that apply): `"qet.uuid"` (the
//! `uuid` attribute), `"qet.info_name"` (ElementInfo), `"qet.format"` (CompositeText), `"qet.style"` (the
//! style key from `font`, as a string), `"qet.font"` (the raw `font`), `"qet.frame": true` when
//! `frame="true"`. A static text from `UserText` carries `"qet.font"` and `"qet.uuid"` the same way.
//!
//! # Legacy `input` (QET ≤ 0.6)
//!
//! `pos = (x, y)`, `align = TopLeft`, angle `−rotation` (absent = 0), style from the legacy `size`
//! ([`size_style`]). The `tagg` attribute decides:
//!
//! | `tagg` | Result |
//! |---|---|
//! | `label` | [`TextSlot`] of kind `Tag` |
//! | `function` | [`TextSlot`] of kind `FunctionText` |
//! | absent, empty or `none` | static [`Primitive::Text`] of the `text` attribute |
//! | anything else `t` | [`TextSlot`] of kind `UserProp("qet.<t>")` |
//!
//! Slots get `visible = true`, `max_width = None`, and `extra["qet.style"]` when a size is given; a
//! `rotate` attribute is kept as `extra["qet.rotate"]` (its string) on slots and static texts.
//!
//! # Fonts (doc 14 §4.10)
//!
//! See [`font_style`] and [`size_style`]. Each substituted family is reported **once per definition**
//! (EW-IMP-006, info), tracked in [`PrimCtx::reported_families`].

use ew_interop_api::{Code, Severity};
use ew_mltext::{Ident, MlText};
use ew_schema::{Align, Extra, Primitive, SlotKind, TextPrim, TextSlot};
use ew_units::Angle2;
use roxmltree::Node;

use super::{ItemError, Parsed, PrimCtx};
use crate::decimal::Decimal;

/// Sets `extra[key]` to a string value.
fn put(extra: &mut Extra, key: &str, value: &str) {
    extra.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
}

/// The text angle of `rotation` (absent = 0): the negated decimal, in 10⁻³ degree.
fn rotation(node: Node<'_, '_>, ctx: &PrimCtx<'_>) -> Result<Angle2, ItemError> {
    let a = ctx
        .decimal_or(node, "rotation", Decimal::ZERO)?
        .deg_to_angle()
        .map_err(|e| ItemError(format!("rotation: {e}")))?;
    a.0.checked_neg()
        .map(Angle2)
        .ok_or_else(|| ItemError("rotation: value out of range".to_owned()))
}

/// The style of `font`, else of the legacy `size`, of `node`.
fn node_style(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>) -> Option<Ident> {
    if let Some(font) = node.attribute("font") {
        font_style(node, font, ctx)
    } else if let Some(size) = node.attribute("size") {
        size_style(node, size, ctx)
    } else {
        None
    }
}

/// The alignment of `Halignment` × `Valignment`; an unknown value counts as the default and is reported.
fn alignment(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>) -> Align {
    let h = node.attribute("Halignment").unwrap_or("AlignLeft");
    let v = node.attribute("Valignment").unwrap_or("AlignTop");
    let hi = match h {
        "AlignLeft" => 0,
        "AlignHCenter" => 1,
        "AlignRight" => 2,
        _ => {
            ctx.unsupported(node, format!("Halignment {h} not supported, AlignLeft used"));
            0
        }
    };
    let vi = match v {
        "AlignTop" => 0,
        "AlignVCenter" => 1,
        "AlignBottom" => 2,
        _ => {
            ctx.unsupported(node, format!("Valignment {v} not supported, AlignTop used"));
            0
        }
    };
    [
        [Align::TopLeft, Align::TopCentre, Align::TopRight],
        [Align::MiddleLeft, Align::Centre, Align::MiddleRight],
        [Align::BottomLeft, Align::BottomCentre, Align::BottomRight],
    ][vi][hi]
        .clone()
}

/// The text of the child element `name` (empty if absent).
fn child_text<'a>(node: Node<'a, '_>, name: &str) -> &'a str {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
        .map_or("", |c| c.text().unwrap_or(""))
}

/// The bundled default family that every non-bundled family maps to (Noto Sans, proposed ADR-0028).
pub const DEFAULT_FAMILY: &str = "Noto Sans";

/// The families of the font table of doc 14 §4.10 that map to [`DEFAULT_FAMILY`]. A family neither here nor
/// bundled is substituted too; the report message then says it is not in the table.
pub const SUBSTITUTED_FAMILIES: [&str; 6] = [
    "Tahoma",
    "Sans Serif",
    "MS Shell Dlg 2",
    "Arial",
    "Helvetica",
    "Liberation Sans",
];

/// A Qt `QFont::toString()` font, as far as styles need it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QtFont {
    /// Field 0, trimmed.
    pub family: String,
    /// The point size: field 1, or field 2 (pixel size) × 0.75 when field 1 is `-1`.
    pub point_size: Decimal,
    /// Bold: weight (field 4) ≥ 63 in the Qt 5 form (exactly 10 fields), ≥ 600 in the Qt 6 form (11 or more).
    pub bold: bool,
    /// Italic: style (field 5) is not `0`.
    pub italic: bool,
}

/// Parses a `QFont::toString()` string: comma-separated, at least 6 fields; `None` if it has fewer fields,
/// a field that should be a number is not one, or the resulting point size is not positive.
///
/// ```
/// # use ew_import_qet::prims::text::parse_qt_font;
/// let f = parse_qt_font("Sans Serif,9,-1,5,50,0,0,0,0,0").unwrap();
/// assert_eq!((f.family.as_str(), f.point_size.to_plain_string(), f.bold, f.italic), ("Sans Serif", "9".to_owned(), false, false));
/// let f = parse_qt_font("Liberation Sans,-1,13,5,700,1,0,0,0,0,Bold Italic").unwrap();
/// assert_eq!((f.point_size.to_plain_string(), f.bold, f.italic), ("9.75".to_owned(), true, true));
/// ```
pub fn parse_qt_font(font: &str) -> Option<QtFont> {
    let fields: Vec<&str> = font.split(',').map(str::trim).collect();
    if fields.len() < 6 {
        return None;
    }
    let mut point_size = Decimal::parse(fields[1]).ok()?;
    if point_size == Decimal::from_int(-1) {
        let pixels = Decimal::parse(fields[2]).ok()?;
        point_size = pixels.checked_mul(Decimal::parse("0.75").ok()?).ok()?;
    }
    if point_size.is_zero() || point_size.is_negative() {
        return None;
    }
    let weight: i64 = fields[4].parse().ok()?;
    let style: i64 = fields[5].parse().ok()?;
    // Qt 6 writes 11 or more fields with weights on the 1..1000 scale; Qt 5 writes 10 with 0..99. Anything
    // shorter than 10 fields is read like Qt 5.
    let bold_from = if fields.len() >= 11 { 600 } else { 63 };
    Some(QtFont {
        family: fields[0].to_owned(),
        point_size,
        bold: weight >= bold_from,
        italic: style != 0,
    })
}

/// The import style key of a font: `qet.<size>[.b][.i]` with `<size>` the point size as
/// [`Decimal::to_plain_string`], such as `qet.9`, `qet.9.75.b`, `qet.12.i`.
pub fn style_key(point_size: Decimal, bold: bool, italic: bool) -> String {
    format!(
        "qet.{}{}{}",
        point_size.to_plain_string(),
        if bold { ".b" } else { "" },
        if italic { ".i" } else { "" }
    )
}

/// The style of a `font` attribute: parses it, reports its family once per definition when it is
/// substituted (EW-IMP-006, info, message `font family "<family>" substituted by Noto Sans` plus ` (not in
/// the font table)` for families outside [`SUBSTITUTED_FAMILIES`]), and returns the style key as an
/// [`Ident`]. An unparsable font is reported (EW-IMP-004, info: `font "<font>" not understood, default
/// style used`) and gives `None`.
pub fn font_style(node: Node<'_, '_>, font: &str, ctx: &mut PrimCtx<'_>) -> Option<Ident> {
    let Some(f) = parse_qt_font(font) else {
        ctx.unsupported(
            node,
            format!("font \"{font}\" not understood, default style used"),
        );
        return None;
    };
    if f.family != DEFAULT_FAMILY && ctx.reported_families.insert(f.family.clone()) {
        let table = if SUBSTITUTED_FAMILIES.contains(&f.family.as_str()) {
            ""
        } else {
            " (not in the font table)"
        };
        let message = format!(
            "font family \"{}\" substituted by {DEFAULT_FAMILY}{table}",
            f.family
        );
        ctx.report(node, Code::FONT_SUBSTITUTED, Severity::Info, message);
    }
    Ident::from_input(&style_key(f.point_size, f.bold, f.italic)).ok()
}

/// The style of a legacy `size` attribute (points): `qet.<size>`; `None` (and an EW-IMP-004 info report) if
/// it is not a positive decimal.
pub fn size_style(node: Node<'_, '_>, size: &str, ctx: &mut PrimCtx<'_>) -> Option<Ident> {
    match Decimal::parse(size.trim()) {
        Ok(d) if !d.is_zero() && !d.is_negative() => Ident::from_input(&style_key(d, false, false)).ok(),
        _ => {
            ctx.unsupported(
                node,
                format!("size \"{size}\" not understood, default style used"),
            );
            None
        }
    }
}

/// The slot kind of an `ElementInfo` name (seed table, extensible: add rows, never change one):
///
/// | `info_name` | Kind |
/// |---|---|
/// | `label` | `Tag` |
/// | `function` | `FunctionText` |
/// | `manufacturer` | `Manufacturer` |
/// | `manufacturer_reference` | `PartNumber` |
/// | `designation` | `TypeDesignation` |
/// | any other `n` | `UserProp("qet.<n>")` |
///
/// # Errors
///
/// [`ItemError`] if `qet.<n>` is not a valid [`Ident`] (empty name, control characters).
pub fn slot_kind_for_info(info_name: &str) -> Result<SlotKind, ItemError> {
    Ok(match info_name {
        "label" => SlotKind::Tag,
        "function" => SlotKind::FunctionText,
        "manufacturer" => SlotKind::Manufacturer,
        "manufacturer_reference" => SlotKind::PartNumber,
        "designation" => SlotKind::TypeDesignation,
        n => user_prop(n)?,
    })
}

/// `UserProp("qet.<n>")`; an empty or blank name or an invalid identifier is an [`ItemError`].
fn user_prop(n: &str) -> Result<SlotKind, ItemError> {
    if n.trim().is_empty() {
        return Err(ItemError("empty name for a user property".to_owned()));
    }
    Ident::from_input(&format!("qet.{n}"))
        .map(SlotKind::UserProp)
        .map_err(|e| ItemError(format!("user property qet.{n}: {e}")))
}

/// Static `text`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or rotation.
pub fn static_text(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let pos = ctx.point(node, "x", "y")?;
    let angle = rotation(node, ctx)?;
    let style = node_style(node, ctx);
    let mut extra = Extra::new();
    if let Some(font) = node.attribute("font") {
        put(&mut extra, "qet.font", font);
    }
    if let Some(colour) = node.attribute("color")
        && colour != "black"
    {
        ctx.unsupported(node, format!("text colour {colour} dropped"));
        put(&mut extra, "qet.color", colour);
    }
    out.graphics.push(Primitive::Text(TextPrim {
        pos,
        text: MlText::neutral(node.attribute("text").unwrap_or("")),
        style,
        align: Align::BottomLeft,
        angle,
        extra,
    }));
    Ok(())
}

/// `dynamic_text`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or rotation, or an unknown `text_from`.
pub fn dynamic_text(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let pos = ctx.point(node, "x", "y")?;
    let angle = rotation(node, ctx)?;
    let text_from = node.attribute("text_from").unwrap_or("UserText");
    if !matches!(text_from, "UserText" | "ElementInfo" | "CompositeText") {
        return Err(ItemError(format!("text_from {text_from:?} not understood")));
    }
    let align = alignment(node, ctx);
    let style = node.attribute("font").and_then(|f| font_style(node, f, ctx));
    let mut extra = Extra::new();
    if let Some(font) = node.attribute("font") {
        put(&mut extra, "qet.font", font);
    }
    if let Some(uuid) = node.attribute("uuid") {
        put(&mut extra, "qet.uuid", uuid);
    }
    if text_from == "UserText" {
        out.graphics.push(Primitive::Text(TextPrim {
            pos,
            text: MlText::neutral(child_text(node, "text")),
            style,
            align,
            angle,
            extra,
        }));
        return Ok(());
    }
    let kind = if text_from == "ElementInfo" {
        let name = child_text(node, "info_name");
        put(&mut extra, "qet.info_name", name);
        slot_kind_for_info(name)?
    } else {
        put(&mut extra, "qet.format", child_text(node, "composite_text"));
        SlotKind::UserProp(
            Ident::from_input("qet.composite").map_err(|e| ItemError(format!("qet.composite: {e}")))?,
        )
    };
    if let Some(style) = &style {
        put(&mut extra, "qet.style", style.as_str());
    }
    if node.attribute("frame") == Some("true") {
        extra.insert("qet.frame".to_owned(), serde_json::Value::Bool(true));
    }
    let width = ctx.decimal_or(node, "text_width", Decimal::from_int(-1))?;
    let max_width = if width.is_zero() || width.is_negative() {
        None
    } else {
        Some(ctx.nm(width, "text_width")?)
    };
    out.slots.push(TextSlot {
        kind,
        pos,
        rot: angle,
        align,
        visible: true,
        max_width,
        extra,
    });
    Ok(())
}

/// Legacy `input`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or rotation.
pub fn input(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let pos = ctx.point(node, "x", "y")?;
    let angle = rotation(node, ctx)?;
    let tagg = node.attribute("tagg").unwrap_or("none");
    let kind = match tagg {
        "" | "none" => None,
        "label" => Some(SlotKind::Tag),
        "function" => Some(SlotKind::FunctionText),
        other => Some(user_prop(other)?),
    };
    let style = node.attribute("size").and_then(|s| size_style(node, s, ctx));
    let mut extra = Extra::new();
    if let Some(rotate) = node.attribute("rotate") {
        put(&mut extra, "qet.rotate", rotate);
    }
    let Some(kind) = kind else {
        out.graphics.push(Primitive::Text(TextPrim {
            pos,
            text: MlText::neutral(node.attribute("text").unwrap_or("")),
            style,
            align: Align::TopLeft,
            angle,
            extra,
        }));
        return Ok(());
    };
    if let Some(style) = &style {
        put(&mut extra, "qet.style", style.as_str());
    }
    out.slots.push(TextSlot {
        kind,
        pos,
        rot: angle,
        align: Align::TopLeft,
        visible: true,
        max_width: None,
        extra,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use ew_interop_api::{Code, ReportEntry};
    use ew_units::{Nm, Pt2};

    use super::*;

    type Parser = fn(Node<'_, '_>, &mut PrimCtx<'_>, &mut Parsed) -> Result<(), ItemError>;

    fn run(xml: &str, f: Parser) -> (Result<(), ItemError>, Parsed, Vec<ReportEntry>) {
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut ctx = PrimCtx::new("t.elmt", None, Decimal::ZERO, Decimal::ZERO);
        let mut out = Parsed::default();
        let r = f(doc.root_element(), &mut ctx, &mut out);
        (r, out, ctx.entries)
    }

    fn px(s: &str) -> Nm {
        Decimal::parse(s).unwrap().px_to_nm().unwrap()
    }

    fn s(v: &str) -> serde_json::Value {
        serde_json::Value::String(v.to_owned())
    }

    fn text_of(out: &Parsed) -> &TextPrim {
        assert_eq!(out.graphics.len(), 1);
        let Primitive::Text(t) = &out.graphics[0] else {
            panic!()
        };
        t
    }

    fn slot_of(out: &Parsed) -> &TextSlot {
        assert_eq!(out.slots.len(), 1);
        assert!(out.graphics.is_empty());
        &out.slots[0]
    }

    fn ident(v: &str) -> Ident {
        Ident::from_input(v).unwrap()
    }

    #[test]
    fn static_text_rotation_is_negated() {
        let (r, out, entries) = run(r#"<text x="10" y="-20" text="Hi" rotation="90"/>"#, static_text);
        r.unwrap();
        assert!(entries.is_empty());
        let t = text_of(&out);
        assert_eq!(t.angle, Angle2(-90_000));
        assert_eq!(t.pos, Pt2::new(px("10"), px("-20")));
        assert_eq!(t.text, MlText::neutral("Hi"));
        assert_eq!(t.align, Align::BottomLeft);
        assert_eq!(t.style, None);
        assert!(t.extra.is_empty());
        let (r, out, _) = run(r#"<text x="0" y="0"/>"#, static_text);
        r.unwrap();
        assert_eq!(text_of(&out).angle, Angle2(0));
        assert_eq!(text_of(&out).text, MlText::neutral(""));
        let (r, ..) = run(r#"<text x="0" y="0" rotation="zz"/>"#, static_text);
        assert!(r.is_err());
    }

    #[test]
    fn font_qt5_and_qt6_forms() {
        let font = |f: &str| {
            let xml = format!(r#"<text x="0" y="0" text="a" font="{f}"/>"#);
            let (r, out, _) = run(&xml, static_text);
            r.unwrap();
            text_of(&out).style.clone()
        };
        assert_eq!(font("Sans Serif,9,-1,5,50,0,0,0,0,0"), Some(ident("qet.9")));
        assert_eq!(
            font("Liberation Sans,-1,13,5,700,1,0,0,0,0,Bold Italic"),
            Some(ident("qet.9.75.b.i"))
        );
        assert_eq!(font("Arial,8,-1,5,75,0,0,0,0,0"), Some(ident("qet.8.b")));
        // The Qt 6 threshold is 600: weight 500 is not bold, 600 is.
        assert_eq!(
            font("Noto Sans,10,-1,5,500,0,0,0,0,0,Regular"),
            Some(ident("qet.10"))
        );
        assert_eq!(
            font("Noto Sans,10,-1,5,600,0,0,0,0,0,Regular"),
            Some(ident("qet.10.b"))
        );
        // Qt 5 weight 63 is bold, 62 is not.
        assert_eq!(font("Noto Sans,10,-1,5,63,0,0,0,0,0"), Some(ident("qet.10.b")));
        assert_eq!(font("Noto Sans,10,-1,5,62,0,0,0,0,0"), Some(ident("qet.10")));
    }

    #[test]
    fn parse_qt_font_details() {
        let f = parse_qt_font("Tahoma,-1,13,5,50,1,0,0,0,0").unwrap();
        assert_eq!(f.family, "Tahoma");
        assert_eq!(f.point_size.to_plain_string(), "9.75");
        assert!(!f.bold && f.italic);
        assert!(parse_qt_font("Tahoma,9,-1,5,50").is_none());
        assert!(parse_qt_font("Tahoma,0,-1,5,50,0,0,0,0,0").is_none());
        assert!(parse_qt_font("Tahoma,-3,-1,5,50,0,0,0,0,0").is_none());
        assert!(parse_qt_font("Tahoma,x,-1,5,50,0,0,0,0,0").is_none());
        assert!(parse_qt_font("Tahoma,9,-1,5,heavy,0,0,0,0,0").is_none());
        assert!(parse_qt_font("Tahoma,-1,-1,5,50,0,0,0,0,0").is_none());
        assert_eq!(style_key(Decimal::from_int(12), false, true), "qet.12.i");
    }

    #[test]
    fn family_substitution_reported_once_per_family() {
        let doc = roxmltree::Document::parse(
            r#"<description>
                <text x="0" y="0" text="a" font="Arial,8,-1,5,50,0,0,0,0,0"/>
                <text x="0" y="0" text="b" font="Arial,10,-1,5,50,0,0,0,0,0"/>
                <text x="0" y="0" text="c" font="Noto Sans,10,-1,5,50,0,0,0,0,0"/>
                <text x="0" y="0" text="d" font="Comic Sans,10,-1,5,50,0,0,0,0,0"/>
                <text x="0" y="0" text="e" font="Comic Sans,12,-1,5,50,0,0,0,0,0"/>
            </description>"#,
        )
        .unwrap();
        let mut ctx = PrimCtx::new("t.elmt", None, Decimal::ZERO, Decimal::ZERO);
        let mut out = Parsed::default();
        super::super::parse_description(doc.root_element(), &mut ctx, &mut out);
        assert_eq!(out.graphics.len(), 5);
        let msgs: Vec<_> = ctx.entries.iter().map(|e| (e.code, e.message.as_str())).collect();
        assert_eq!(
            msgs,
            vec![
                (
                    Code::FONT_SUBSTITUTED,
                    "font family \"Arial\" substituted by Noto Sans"
                ),
                (
                    Code::FONT_SUBSTITUTED,
                    "font family \"Comic Sans\" substituted by Noto Sans (not in the font table)"
                ),
            ]
        );
        assert!(ctx.entries.iter().all(|e| e.severity == Severity::Info));
    }

    #[test]
    fn unparsable_font_is_reported_and_style_none() {
        let (r, out, entries) = run(r#"<text x="0" y="0" text="a" font="garbage"/>"#, static_text);
        r.unwrap();
        assert_eq!(text_of(&out).style, None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::DROPPED_BY_DESIGN);
        assert_eq!(
            entries[0].message,
            "font \"garbage\" not understood, default style used"
        );
        assert_eq!(text_of(&out).extra["qet.font"], s("garbage"));
    }

    #[test]
    fn legacy_size_style() {
        let (r, out, entries) = run(r#"<text x="0" y="0" text="a" size="9"/>"#, static_text);
        r.unwrap();
        assert!(entries.is_empty());
        assert_eq!(text_of(&out).style, Some(ident("qet.9")));
        let (r, out, entries) = run(r#"<text x="0" y="0" text="a" size="-2"/>"#, static_text);
        r.unwrap();
        assert_eq!(text_of(&out).style, None);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn text_colour_reported_and_kept() {
        let (r, out, entries) = run(r#"<text x="0" y="0" text="a" color="red"/>"#, static_text);
        r.unwrap();
        assert_eq!(text_of(&out).extra["qet.color"], s("red"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "text colour red dropped");
        let (r, out, entries) = run(r#"<text x="0" y="0" text="a" color="black"/>"#, static_text);
        r.unwrap();
        assert!(entries.is_empty());
        assert!(out.graphics.len() == 1 && text_of(&out).extra.is_empty());
    }

    #[test]
    fn dynamic_element_info_label() {
        let xml = r#"<dynamic_text x="5" y="-5" rotation="270" text_from="ElementInfo" text_width="40"
            font="Sans Serif,9,-1,5,50,0,0,0,0,0" uuid="{u-1}" frame="true" Halignment="AlignHCenter" Valignment="AlignBottom">
            <text></text><info_name>label</info_name></dynamic_text>"#;
        let (r, out, entries) = run(xml, dynamic_text);
        r.unwrap();
        assert_eq!(entries.len(), 1); // Sans Serif substituted
        let sl = slot_of(&out);
        assert_eq!(sl.kind, SlotKind::Tag);
        assert_eq!(sl.pos, Pt2::new(px("5"), px("-5")));
        assert_eq!(sl.rot, Angle2(-270_000));
        assert_eq!(sl.align, Align::BottomCentre);
        assert!(sl.visible);
        assert_eq!(sl.max_width, Some(px("40")));
        assert_eq!(sl.extra["qet.uuid"], s("{u-1}"));
        assert_eq!(sl.extra["qet.info_name"], s("label"));
        assert_eq!(sl.extra["qet.style"], s("qet.9"));
        assert_eq!(sl.extra["qet.font"], s("Sans Serif,9,-1,5,50,0,0,0,0,0"));
        assert_eq!(sl.extra["qet.frame"], serde_json::Value::Bool(true));
    }

    #[test]
    fn dynamic_text_width_minus_one_is_none() {
        let xml = r#"<dynamic_text x="0" y="0" text_from="ElementInfo" text_width="-1"><info_name>function</info_name></dynamic_text>"#;
        let (r, out, entries) = run(xml, dynamic_text);
        r.unwrap();
        assert!(entries.is_empty());
        let sl = slot_of(&out);
        assert_eq!(sl.max_width, None);
        assert_eq!(sl.kind, SlotKind::FunctionText);
        assert_eq!(sl.align, Align::TopLeft);
        assert!(!sl.extra.contains_key("qet.frame"));
    }

    #[test]
    fn alignment_table_and_unknown_alignment() {
        let want = [
            ("AlignLeft", "AlignTop", Align::TopLeft),
            ("AlignHCenter", "AlignTop", Align::TopCentre),
            ("AlignRight", "AlignTop", Align::TopRight),
            ("AlignLeft", "AlignVCenter", Align::MiddleLeft),
            ("AlignHCenter", "AlignVCenter", Align::Centre),
            ("AlignRight", "AlignVCenter", Align::MiddleRight),
            ("AlignLeft", "AlignBottom", Align::BottomLeft),
            ("AlignHCenter", "AlignBottom", Align::BottomCentre),
            ("AlignRight", "AlignBottom", Align::BottomRight),
        ];
        for (h, v, a) in want {
            let xml = format!(
                r#"<dynamic_text x="0" y="0" Halignment="{h}" Valignment="{v}"><text>t</text></dynamic_text>"#
            );
            let (r, out, entries) = run(&xml, dynamic_text);
            r.unwrap();
            assert!(entries.is_empty());
            assert_eq!(text_of(&out).align, a, "{h} {v}");
        }
        let xml = r#"<dynamic_text x="0" y="0" Halignment="AlignJustify" Valignment="AlignBottom"><text>t</text></dynamic_text>"#;
        let (r, out, entries) = run(xml, dynamic_text);
        r.unwrap();
        assert_eq!(text_of(&out).align, Align::BottomLeft);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::DROPPED_BY_DESIGN);
    }

    #[test]
    fn dynamic_composite_text() {
        let xml = r#"<dynamic_text x="0" y="0" text_from="CompositeText"><composite_text>%{label} %{comment}</composite_text></dynamic_text>"#;
        let (r, out, _) = run(xml, dynamic_text);
        r.unwrap();
        let sl = slot_of(&out);
        assert_eq!(sl.kind, SlotKind::UserProp(ident("qet.composite")));
        assert_eq!(sl.extra["qet.format"], s("%{label} %{comment}"));
    }

    #[test]
    fn dynamic_user_text_is_static_text() {
        let xml = r#"<dynamic_text x="1" y="2" uuid="{x}" font="Noto Sans,12,-1,5,50,1,0,0,0,0" Valignment="AlignVCenter">
            <text>Hello</text></dynamic_text>"#;
        let (r, out, entries) = run(xml, dynamic_text);
        r.unwrap();
        assert!(entries.is_empty()); // Noto Sans is the bundled family
        assert!(out.slots.is_empty());
        let t = text_of(&out);
        assert_eq!(t.text, MlText::neutral("Hello"));
        assert_eq!(t.align, Align::MiddleLeft);
        assert_eq!(t.style, Some(ident("qet.12.i")));
        assert_eq!(t.extra["qet.uuid"], s("{x}"));
        assert_eq!(t.extra["qet.font"], s("Noto Sans,12,-1,5,50,1,0,0,0,0"));
        // No <text> child: empty text.
        let (r, out, _) = run(r#"<dynamic_text x="1" y="2"/>"#, dynamic_text);
        r.unwrap();
        assert_eq!(text_of(&out).text, MlText::neutral(""));
    }

    #[test]
    fn dynamic_unknown_text_from_is_item_error() {
        let (r, out, _) = run(r#"<dynamic_text x="0" y="0" text_from="Magic"/>"#, dynamic_text);
        assert!(r.unwrap_err().0.contains("Magic"));
        assert!(out.graphics.is_empty() && out.slots.is_empty());
    }

    #[test]
    fn info_name_table() {
        let want = [
            ("label", SlotKind::Tag),
            ("function", SlotKind::FunctionText),
            ("manufacturer", SlotKind::Manufacturer),
            ("manufacturer_reference", SlotKind::PartNumber),
            ("designation", SlotKind::TypeDesignation),
            ("comment", SlotKind::UserProp(ident("qet.comment"))),
        ];
        for (n, k) in want {
            assert_eq!(slot_kind_for_info(n), Ok(k), "{n}");
        }
        assert!(slot_kind_for_info("").is_err());
        assert!(slot_kind_for_info("a\nb").is_err());
    }

    #[test]
    fn legacy_input_tagg() {
        let input_xml = |tagg: &str| {
            format!(r#"<input x="3" y="4" text="T" size="9" tagg="{tagg}" rotation="45" rotate="true"/>"#)
        };
        let (r, out, _) = run(&input_xml("label"), input);
        r.unwrap();
        let sl = slot_of(&out);
        assert_eq!(sl.kind, SlotKind::Tag);
        assert_eq!(sl.pos, Pt2::new(px("3"), px("4")));
        assert_eq!(sl.rot, Angle2(-45_000));
        assert_eq!(sl.align, Align::TopLeft);
        assert!(sl.visible);
        assert_eq!(sl.max_width, None);
        assert_eq!(sl.extra["qet.style"], s("qet.9"));
        assert_eq!(sl.extra["qet.rotate"], s("true"));

        let (r, out, _) = run(&input_xml("function"), input);
        r.unwrap();
        assert_eq!(slot_of(&out).kind, SlotKind::FunctionText);

        for tagg in ["none", ""] {
            let (r, out, _) = run(&input_xml(tagg), input);
            r.unwrap();
            assert!(out.slots.is_empty());
            let t = text_of(&out);
            assert_eq!(t.text, MlText::neutral("T"));
            assert_eq!(t.align, Align::TopLeft);
            assert_eq!(t.style, Some(ident("qet.9")));
            assert_eq!(t.angle, Angle2(-45_000));
            assert_eq!(t.extra["qet.rotate"], s("true"));
        }
        let (r, out, _) = run(r#"<input x="0" y="0" text="T"/>"#, input);
        r.unwrap();
        assert_eq!(text_of(&out).angle, Angle2(0));

        let (r, out, _) = run(&input_xml("comment"), input);
        r.unwrap();
        assert_eq!(slot_of(&out).kind, SlotKind::UserProp(ident("qet.comment")));
    }

    #[test]
    fn size_style_reports_bad_size() {
        let doc = roxmltree::Document::parse(r#"<input/>"#).unwrap();
        let mut ctx = PrimCtx::new("t.elmt", None, Decimal::ZERO, Decimal::ZERO);
        assert_eq!(size_style(doc.root_element(), "x", &mut ctx), None);
        assert_eq!(ctx.entries.len(), 1);
        assert_eq!(
            size_style(doc.root_element(), "12", &mut ctx),
            Some(ident("qet.12"))
        );
        assert_eq!(ctx.entries.len(), 1);
    }
}
