// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The children of an element definition's `<description>`: one parser per QET primitive (doc 14 §4.3).
//!
//! # Mapping table
//!
//! Lengths are QET px, converted by [`Decimal::px_to_nm`]; positions are relative to the definition's
//! hotspot, as QET stores them ([`PrimCtx::x`], [`PrimCtx::y`]); angles are degrees
//! ([`Decimal::deg_to_angle`]). A missing required attribute or an unparsable number makes the item an
//! [`ItemError`]: the dispatcher reports it (EW-IMP-002, error) and skips the item. Unsupported values are
//! reported (EW-IMP-004, info) and the item is kept.
//!
//! | QET element | Attributes (default) | EW output |
//! |---|---|---|
//! | `line` | `x1 y1 x2 y2`; `end1 end2` (`none`); `length1 length2` (`1.5`); `style` | [`Primitive::Line`], then one end-marker primitive per end ([`shapes::end_marker`]) |
//! | `rect` | `x y width height`; `rx ry` (`0`); `style` | [`Primitive::Rect`] over `(x, y)`–`(x + width, y + height)` with min/max normalised; `rx`, `ry` as px lengths (their absolute value); `shadow = None` |
//! | `ellipse` | `x y width height`; `style` | [`Primitive::Ellipse`] over the normalised box |
//! | `circle` | `x y diameter`; `style` | [`Primitive::Circle`], centre `(x + d/2, y + d/2)` and radius `d/2` computed as exact decimals before conversion |
//! | `arc` | `x y width height start angle`; `style` | [`Primitive::Arc`]: box as `ellipse`, `start` and `sweep` from `start` and `angle` unchanged (both counter-clockwise on the sheet, 0° at 3 o'clock); a `filling` other than none is reported (EW-IMP-004) and dropped, arcs are never filled |
//! | `polygon` | `x1 y1 … xn yn` (consecutive from 1 until `xk` is missing); `closed` (`true`); `style` | `closed="false"` → [`Primitive::Polyline`] (fill dropped silently: Qt does not fill polylines); otherwise [`Primitive::Polygon`] with `closed: true`. Fewer than 2 points → [`ItemError`] |
//! | `text` | `x y text`; `font` or legacy `size`; `color` (`black`); `rotation` (`0`) | [`Primitive::Text`] ([`text::static_text`]) |
//! | `dynamic_text` | `x y`; `rotation` (`0`); `font`; `text_from` (`UserText`); `Halignment` (`AlignLeft`), `Valignment` (`AlignTop`); `text_width` (`-1`); `uuid`; children `<text>`, `<info_name>`, `<composite_text>` | a text slot or a static text ([`text::dynamic_text`]) |
//! | `input` (≤ 0.6) | `x y`; `text`; `size`; `tagg` (`none`); `rotation` (`0`) | a text slot or a static text ([`text::input`]) |
//! | `terminal` | `x y orientation`; `name` (``), `type` (`Generic`), `uuid` | a [`Terminal`] ([`terminal::terminal`]) |
//! | anything else | | reported (EW-IMP-004, info: "unsupported element `<name>` dropped") and skipped |
//!
//! Every primitive with a `style` attribute takes its stroke and fill from [`crate::style::parse_style`];
//! each unsupported style pair is reported (EW-IMP-004) and kept in the primitive's `extra` as
//! `"qet.style.<key>": "<value>"`. Primitives without fill in the schema (`Line`, `Polyline`, `Arc`) ignore
//! the fill except where the table says otherwise.

pub mod shapes;
pub mod terminal;
pub mod text;

use std::collections::BTreeSet;

use ew_interop_api::{Code, ReportEntry, Severity, SourceLocation};
use ew_schema::{Dir4, Primitive, TextSlot};
use ew_units::{Nm, Pt2};
use roxmltree::Node;

use crate::decimal::Decimal;

/// An item that cannot be mapped (missing required attribute, unparsable number, too few points). The
/// dispatcher reports it as EW-IMP-002 (error) and skips the item.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ItemError(pub String);

/// A terminal of the definition: becomes a `SymbolCp` and a `CpDef` with index = definition order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Terminal {
    /// Position relative to the hotspot.
    pub pos: Pt2,
    /// The direction a wire leaves the terminal: `n` → Up, `e` → Right, `s` → Down, `w` → Left.
    pub dir: Dir4,
    /// The `name` attribute verbatim (may be empty).
    pub name: String,
    /// The `type` attribute: `Generic` (default), `Inner` or `Outer`; other values are kept verbatim and
    /// reported (EW-IMP-004).
    pub kind: String,
    /// The `uuid` attribute verbatim (braces included), if present.
    pub uuid: Option<String>,
}

/// What the `<description>` children produce, in document order per vector.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Parsed {
    /// Graphics, static texts included.
    pub graphics: Vec<Primitive>,
    /// Text slots.
    pub slots: Vec<TextSlot>,
    /// Terminals.
    pub terminals: Vec<Terminal>,
}

/// The per-definition parsing context: the coordinate origin, where findings are located, and the findings.
pub struct PrimCtx<'a> {
    /// The x of the origin of definition coordinates, in px. QET stores primitive coordinates relative to the
    /// element's hotspot already (its editor puts the hotspot at the scene origin; `hotspot_x`/`hotspot_y`
    /// only place the bounding box around it), so [`crate::read_element`] passes zero.
    pub origin_x: Decimal,
    /// The y of the origin of definition coordinates, in px (zero, as for `origin_x`).
    pub origin_y: Decimal,
    /// The source file name, for [`SourceLocation::file`].
    pub file: &'a str,
    /// The definition's path (`embed://…`), for [`SourceLocation::member`].
    pub member: Option<&'a str>,
    /// Findings, in order.
    pub entries: Vec<ReportEntry>,
    /// Font families already reported as substituted in this definition (EW-IMP-006 once per family).
    pub reported_families: BTreeSet<String>,
}

impl<'a> PrimCtx<'a> {
    /// A context with the origin `(origin_x, origin_y)` and no findings.
    pub fn new(file: &'a str, member: Option<&'a str>, origin_x: Decimal, origin_y: Decimal) -> Self {
        PrimCtx {
            origin_x,
            origin_y,
            file,
            member,
            entries: Vec::new(),
            reported_families: BTreeSet::new(),
        }
    }

    /// The location of `node`: file, member, `item` = the element name, and its 1-based line.
    pub fn location(&self, node: Node<'_, '_>) -> SourceLocation {
        let line = node.document().text_pos_at(node.range().start).row;
        SourceLocation {
            file: self.file.to_owned(),
            member: self.member.map(str::to_owned),
            item: Some(node.tag_name().name().to_owned()),
            line: Some(line),
            ..SourceLocation::default()
        }
    }

    /// Records a finding at `node`.
    pub fn report(&mut self, node: Node<'_, '_>, code: Code, severity: Severity, message: impl Into<String>) {
        let entry = ReportEntry::new(code, severity, message).at(self.location(node));
        self.entries.push(entry);
    }

    /// Records an unsupported value (EW-IMP-004, info) at `node`.
    pub fn unsupported(&mut self, node: Node<'_, '_>, message: impl Into<String>) {
        self.report(node, Code::DROPPED_BY_DESIGN, Severity::Info, message);
    }

    /// The decimal value of the required attribute `name`.
    ///
    /// # Errors
    ///
    /// [`ItemError`] if it is missing or not a decimal.
    pub fn decimal(&self, node: Node<'_, '_>, name: &str) -> Result<Decimal, ItemError> {
        let s = node
            .attribute(name)
            .ok_or_else(|| ItemError(format!("missing attribute {name}")))?;
        Decimal::parse(s).map_err(|e| ItemError(format!("attribute {name}={s:?}: {e}")))
    }

    /// The decimal value of the optional attribute `name`, or `default` if it is absent.
    ///
    /// # Errors
    ///
    /// [`ItemError`] if it is present but not a decimal.
    pub fn decimal_or(&self, node: Node<'_, '_>, name: &str, default: Decimal) -> Result<Decimal, ItemError> {
        match node.attribute(name) {
            None => Ok(default),
            Some(_) => self.decimal(node, name),
        }
    }

    /// A pixel decimal converted to nm.
    ///
    /// # Errors
    ///
    /// [`ItemError`] if the value is out of range.
    pub fn nm(&self, d: Decimal, what: &str) -> Result<Nm, ItemError> {
        d.px_to_nm().map_err(|e| ItemError(format!("{what}: {e}")))
    }

    /// An x coordinate in px made relative to the origin, then converted (one rounding).
    ///
    /// # Errors
    ///
    /// [`ItemError`] if the value is out of range.
    pub fn rel_x(&self, x: Decimal) -> Result<Nm, ItemError> {
        let d = x
            .checked_sub(self.origin_x)
            .map_err(|e| ItemError(format!("x: {e}")))?;
        self.nm(d, "x")
    }

    /// A y coordinate in px made relative to the origin, then converted (one rounding).
    ///
    /// # Errors
    ///
    /// [`ItemError`] if the value is out of range.
    pub fn rel_y(&self, y: Decimal) -> Result<Nm, ItemError> {
        let d = y
            .checked_sub(self.origin_y)
            .map_err(|e| ItemError(format!("y: {e}")))?;
        self.nm(d, "y")
    }

    /// The required attribute `name` as an x coordinate (relative to the hotspot).
    ///
    /// # Errors
    ///
    /// [`ItemError`] if it is missing, not a decimal or out of range.
    pub fn x(&self, node: Node<'_, '_>, name: &str) -> Result<Nm, ItemError> {
        self.rel_x(self.decimal(node, name)?)
    }

    /// The required attribute `name` as a y coordinate (relative to the hotspot).
    ///
    /// # Errors
    ///
    /// [`ItemError`] if it is missing, not a decimal or out of range.
    pub fn y(&self, node: Node<'_, '_>, name: &str) -> Result<Nm, ItemError> {
        self.rel_y(self.decimal(node, name)?)
    }

    /// The point of the required attributes `xname`, `yname`, relative to the hotspot.
    ///
    /// # Errors
    ///
    /// [`ItemError`] as for [`PrimCtx::x`].
    pub fn point(&self, node: Node<'_, '_>, xname: &str, yname: &str) -> Result<Pt2, ItemError> {
        Ok(Pt2::new(self.x(node, xname)?, self.y(node, yname)?))
    }
}

/// Parses every element child of `<description>` in document order into `out`, reporting skipped and
/// unsupported items in `ctx`. Never fails.
pub fn parse_description(description: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) {
    for node in description.children().filter(Node::is_element) {
        let name = node.tag_name().name();
        let result = match name {
            "line" => shapes::line(node, ctx, out),
            "rect" => shapes::rect(node, ctx, out),
            "ellipse" => shapes::ellipse(node, ctx, out),
            "circle" => shapes::circle(node, ctx, out),
            "arc" => shapes::arc(node, ctx, out),
            "polygon" => shapes::polygon(node, ctx, out),
            "text" => text::static_text(node, ctx, out),
            "dynamic_text" => text::dynamic_text(node, ctx, out),
            "input" => text::input(node, ctx, out),
            "terminal" => terminal::terminal(node, ctx, out),
            other => {
                ctx.unsupported(node, format!("unsupported element <{other}> dropped"));
                Ok(())
            }
        };
        if let Err(ItemError(message)) = result {
            ctx.report(
                node,
                Code::MALFORMED_ITEM,
                Severity::Error,
                format!("<{name}> skipped: {message}"),
            );
        }
    }
}
