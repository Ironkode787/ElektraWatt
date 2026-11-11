// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The QET project as the mapper sees it: plain owned data read from the `.qet` DOM by [`super::read`],
//! before any mapping decision. Strings are as written in the file (trimmed only where noted); UUIDs are
//! normalised by [`norm_uuid`]. Every item keeps its 1-based source line for report locations.

use std::ops::Range;

use crate::decimal::Decimal;

/// A whole `<project>`.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct QetProject {
    /// `<project version>`, such as `"0.100"`.
    pub version: Option<String>,
    /// `<project title>`.
    pub title: Option<String>,
    /// `<project uuid>` (QET 0.200+) normalised by [`norm_uuid`].
    pub uuid: Option<String>,
    /// `/project/properties/property`: `name` attribute → text, document order.
    pub properties: Vec<(String, String)>,
    /// `/project/newdiagrams/report/@label`, the folio-report cross-reference format (`%f-%l%c`).
    pub report_label: Option<String>,
    /// The `<diagram>`s in document order.
    pub diagrams: Vec<QetDiagram>,
    /// The embedded title-block templates (`/project/titleblocktemplates/titleblocktemplate`), document
    /// order.
    pub templates: Vec<TbTemplate>,
    /// The embedded element definitions of `/project/collection`, in document order.
    pub definitions: Vec<EmbeddedDef>,
    /// Counts of top-level members the importer does not map (`terminal_strips`, …),
    /// as `(element name, count)` in document order of first occurrence.
    pub unmapped_top_level: Vec<(String, u32)>,
}

/// One embedded element definition of the collection.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmbeddedDef {
    /// The collection path the elements' `type` attribute names: `embed://` followed by the `name`
    /// attributes of the enclosing `<category>`s and of the `<element>`, joined by `/`
    /// (`embed://import/10_electric/switch.elmt`).
    pub path: String,
    /// The byte range of the `<definition …>…</definition>` element in the input text (start tag to end tag
    /// inclusive), so that it can be read on its own by the element reader.
    pub range: Range<usize>,
    /// 1-based line of the `<definition>` start tag.
    pub line: u32,
}

/// One `<diagram>` (a QET folio).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct QetDiagram {
    /// 0-based position among the diagrams in document order.
    pub index: usize,
    /// The `order` attribute, when it parses as an integer.
    pub order: Option<i64>,
    /// The `title` attribute ("" when absent).
    pub title: String,
    /// The raw `folio` attribute ("" when absent): a number, text or a formula such as `%id/%total`.
    pub folio: String,
    /// Title-block attributes present on the `<diagram>`, by name, in this fixed name order: `author`,
    /// `date`, `indexrev`, `filename`, `plant`, `locmach`, `titleblocktemplate`, `auto_page_num`,
    /// `displayAt`. Absent attributes are not listed.
    pub titleblock: Vec<(String, String)>,
    /// `/diagram/properties/property`: `name` → text, document order.
    pub properties: Vec<(String, String)>,
    /// The border attributes of the `<diagram>`.
    pub border: Border,
    /// `titleblocktemplateCollection`: `embedded` names a template of the project ("" when absent).
    pub template_collection: String,
    /// The placed elements, document order.
    pub elements: Vec<QetElement>,
    /// The conductors, document order.
    pub conductors: Vec<QetConductor>,
    /// The free texts (`/diagram/inputs/input`), document order.
    pub free_texts: Vec<QetInput>,
    /// The shapes (`/diagram/shapes/shape`), document order.
    pub shapes: Vec<QetShape>,
    /// The images (`/diagram/images/image`), document order.
    pub images: Vec<QetImage>,
    /// The tables (`/diagram/tables/graphics_table`), document order.
    pub tables: Vec<QetTable>,
    /// 1-based line of the `<diagram>` start tag.
    pub line: u32,
}

/// The border attributes of a diagram (`cols colsize rows rowsize height displaycols displayrows`), each
/// when present and parsable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Border {
    /// `cols`.
    pub cols: Option<u32>,
    /// `colsize`, px.
    pub colsize: Option<Decimal>,
    /// `rows`.
    pub rows: Option<u32>,
    /// `rowsize`, px.
    pub rowsize: Option<Decimal>,
    /// `height`, px.
    pub height: Option<Decimal>,
    /// `displaycols`: `false` hides the column header band.
    pub display_cols: Option<bool>,
    /// `displayrows`: `false` hides the row header band.
    pub display_rows: Option<bool>,
}

/// One `<titleblocktemplate>`: a grid of cells (doc 14 §4.9).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TbTemplate {
    /// The `name` attribute.
    pub name: String,
    /// `<grid cols>` split at `;`, empty parts dropped (`46px`, `t15%`, `r100%`).
    pub cols: Vec<String>,
    /// `<grid rows>` split at `;`, empty parts dropped (`17`, px).
    pub rows: Vec<String>,
    /// The `<field>` and `<logo>` children of `<grid>`, document order.
    pub cells: Vec<TbCell>,
    /// The `<logos><logo>` resources: `name` → (`storage`, the text, white space removed).
    pub logos: Vec<(String, String, String)>,
    /// 1-based line of the start tag.
    pub line: u32,
}

/// One cell of a title-block template.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TbCell {
    /// `row`, 0-based.
    pub row: u32,
    /// `col`, 0-based.
    pub col: u32,
    /// `rowspan`: the rows spanned **beyond** the first (QET's convention).
    pub rowspan: u32,
    /// `colspan`: the columns spanned beyond the first.
    pub colspan: u32,
    /// `Some(resource)` for a `<logo>`.
    pub logo: Option<String>,
    /// The `<value>` text: the `en` translation, else the first ("" when absent).
    pub value: String,
    /// The `<label>` text, chosen like `value`.
    pub label: String,
    /// `displaylabel="true"`.
    pub display_label: bool,
    /// `fontsize`, points, when it parses.
    pub font_size: Option<u16>,
    /// `align`: `left`, `center` or `right` ("" when absent).
    pub align: String,
    /// `valign`: `top`, `center` or `bottom` ("" when absent).
    pub valign: String,
    /// `hadjust="true"`: shrink to fit.
    pub hadjust: bool,
}

/// One `<graphics_table>` of a diagram (QET's nomenclature and summary tables).
#[derive(Clone, PartialEq, Debug)]
pub struct QetTable {
    /// The `uuid` attribute normalised by [`norm_uuid`].
    pub uuid: Option<String>,
    /// `x`, `y`, `width`, `height`, px.
    pub rect: [Option<Decimal>; 4],
    /// The model's `<query>` text; `None` on a table that continues another.
    pub query: Option<String>,
    /// `<previous_table uuid>` normalised: the table this one continues.
    pub previous: Option<String>,
    /// 1-based line of the start tag.
    pub line: u32,
}

/// One placed `<element>` of a diagram.
#[derive(Clone, PartialEq, Debug)]
pub struct QetElement {
    /// 0-based position among the diagram's `<element>`s in document order, elements the reader skipped
    /// included (the `element <n>` of report items and the `#<n>` of keys of elements without UUID).
    pub n: usize,
    /// The `type` attribute: the definition's collection path (`embed://import/…`).
    pub type_path: String,
    /// The `uuid` attribute normalised by [`norm_uuid`]; `None` when absent or not a UUID.
    pub uuid: Option<String>,
    /// `x`, px (the definition origin on the page).
    pub x: Decimal,
    /// `y`, px.
    pub y: Decimal,
    /// `orientation` 0–3 (quarter turns clockwise); a missing or invalid value is 0 and reported by the
    /// reader.
    pub orientation: u8,
    /// The `prefix` attribute ("" when absent).
    pub prefix: String,
    /// `<terminals><terminal>` in document order.
    pub terminals: Vec<InstTerminal>,
    /// `<elementInformations><elementInformation name>` → text (trimmed), document order; entries with an
    /// empty text are kept.
    pub infos: Vec<(String, String)>,
    /// `<dynamic_texts><dynamic_elmt_text>` in document order.
    pub dyn_texts: Vec<DynText>,
    /// Whether the element has a `<dynamic_texts>` member (QET 0.7+ writes one on every element, empty or
    /// not): its texts are then its own, not the definition's.
    pub has_dyn_texts: bool,
    /// The old-style `<inputs><input>` (QET 0.6 and older): the values of the definition's texts.
    pub inputs: Vec<QetInput>,
    /// `<links_uuids><link_uuid uuid>` normalised by [`norm_uuid`] (non-UUIDs skipped), document order.
    pub links: Vec<String>,
    /// 1-based line of the `<element>` start tag.
    pub line: u32,
}

impl QetElement {
    /// The text of element information `name` (the first entry with that name), if any.
    pub fn info(&self, name: &str) -> Option<&str> {
        self.infos
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The text of element information `name` when present and not empty after trimming.
    pub fn info_nonempty(&self, name: &str) -> Option<&str> {
        self.info(name).map(str::trim).filter(|v| !v.is_empty())
    }
}

/// One `<terminal>` of a placed element (QET writes one per definition terminal).
#[derive(Clone, PartialEq, Debug)]
pub struct InstTerminal {
    /// The `id` attribute as written (an integer in the files read so far), if present.
    pub id: Option<String>,
    /// `x`, px, in definition coordinates, if present and parsable.
    pub x: Option<Decimal>,
    /// `y`, px, in definition coordinates, if present and parsable.
    pub y: Option<Decimal>,
}

/// One `<dynamic_elmt_text>`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DynText {
    /// `text_from`: `UserText`, `ElementInfo` or `CompositeText` ("" when absent).
    pub text_from: String,
    /// The `<text>` child's text (trimmed; "" when absent).
    pub text: String,
    /// The `<info_name>` child's text (trimmed), if present.
    pub info_name: Option<String>,
    /// The `<composite_text>` child's text (QET's `%{name}` format), if present.
    pub composite: Option<String>,
    /// Where QET draws it: the top-left corner of its text box in element coordinates, size and turn.
    pub at: TextAt,
}

/// The placement of a QET text item: the top-left corner of its text box, its font and its rotation.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TextAt {
    /// `x`, px; `None` when absent or unparsable.
    pub x: Option<Decimal>,
    /// `y`, px; `None` when absent or unparsable.
    pub y: Option<Decimal>,
    /// `rotation`, degrees clockwise (Qt); `None` when absent or unparsable (0).
    pub rotation: Option<Decimal>,
    /// The `font` attribute (`QFont::toString()`), if present.
    pub font: Option<String>,
}

/// One free text: a diagram's `<input>` (an independent text) or an element's old-style `<input>`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QetInput {
    /// The `text` attribute ("" when absent).
    pub text: String,
    /// Where QET draws it.
    pub at: TextAt,
    /// 1-based line of the `<input>` start tag.
    pub line: u32,
}

/// One diagram `<shape>` (`QetShapeItem`): its points and pen and brush, as written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QetShape {
    /// `type`: `Line`, `Rectangle`, `Ellipse`, `Polygon` (or the older numbers 0 to 3).
    pub kind: String,
    /// `x1 y1 x2 y2` for a line, rectangle or ellipse; the `<points><point x y>` of a polygon; each point
    /// `None` when a coordinate is missing or unparsable.
    pub points: Vec<Option<(Decimal, Decimal)>>,
    /// `rx`, `ry`: the corner radii of a rectangle, px (absent: 0).
    pub radii: (Option<Decimal>, Option<Decimal>),
    /// `closed="1"` (a polygon).
    pub closed: bool,
    /// `<pen style>` (`SolidLine`, `DashLine`, …; "" when absent).
    pub pen_style: String,
    /// `<pen widthF>`, px.
    pub pen_width: Option<Decimal>,
    /// `<pen color>` ("" when absent).
    pub pen_color: String,
    /// `<brush style>` (`NoBrush`, `SolidPattern`, …; "" when absent).
    pub brush_style: String,
    /// 1-based line of the `<shape>` start tag.
    pub line: u32,
}

/// One diagram `<image>` (`DiagramImageItem`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QetImage {
    /// `x`, `y`, px: the item position.
    pub x: Option<Decimal>,
    /// See `x`.
    pub y: Option<Decimal>,
    /// `size`: the scale (absent: 1).
    pub scale: Option<Decimal>,
    /// `rotation`, degrees (absent: 0).
    pub rotation: Option<Decimal>,
    /// The base64 text of the image file, white space removed.
    pub data: String,
    /// 1-based line of the `<image>` start tag.
    pub line: u32,
}

/// One `<conductor>`.
#[derive(Clone, PartialEq, Debug)]
pub struct QetConductor {
    /// End 1.
    pub end1: End,
    /// End 2.
    pub end2: End,
    /// `num` ("" when absent).
    pub num: String,
    /// `function` ("" when absent).
    pub function: String,
    /// `tension_protocol` ("" when absent).
    pub tension_protocol: String,
    /// `conductor_color` ("" when absent).
    pub color: String,
    /// `conductor_section` ("" when absent).
    pub section: String,
    /// `<segment>`s in document order; a segment with an unparsable length is dropped and reported.
    pub segments: Vec<Segment>,
    /// 1-based line of the `<conductor>` start tag.
    pub line: u32,
}

/// One conductor end: `element{i}` and `terminal{i}`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct End {
    /// `element{i}` normalised by [`norm_uuid`]; `None` when absent or not a UUID.
    pub element: Option<String>,
    /// `terminal{i}`.
    pub terminal: TerminalRef,
}

/// The `terminal{i}` attribute of a conductor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TerminalRef {
    /// A definition terminal UUID, normalised by [`norm_uuid`].
    Uuid(String),
    /// A per-folio integer terminal ID (the `id` of an element's `<terminal>`), as written.
    Id(String),
    /// Absent or empty.
    Missing,
    /// Anything else, as written.
    Other(String),
}

/// One `<segment orientation length>`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Segment {
    /// `orientation="horizontal"` (else vertical).
    pub horizontal: bool,
    /// `length`, px (signed: negative goes left or up).
    pub length: Decimal,
}

/// A UUID as QET writes it (`{6f1c9a0e-…}`, any case) in lower case without braces; `None` when the text
/// is not 36 characters of hex digits and hyphens in the 8-4-4-4-12 layout after removing the braces.
pub fn norm_uuid(s: &str) -> Option<String> {
    let t = s.trim();
    let t = t.strip_prefix('{').unwrap_or(t);
    let t = t.strip_suffix('}').unwrap_or(t);
    let b = t.as_bytes();
    if b.len() != 36 {
        return None;
    }
    for (i, c) in b.iter().enumerate() {
        let hyphen = matches!(i, 8 | 13 | 18 | 23);
        if hyphen != (*c == b'-') || (!hyphen && !c.is_ascii_hexdigit()) {
            return None;
        }
    }
    Some(t.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::norm_uuid;

    #[test]
    fn uuids_are_normalised() {
        assert_eq!(
            norm_uuid("{6F1C9A0E-3B7D-4E2F-8A5C-1D0E9B8A7C6F}").as_deref(),
            Some("6f1c9a0e-3b7d-4e2f-8a5c-1d0e9b8a7c6f")
        );
        assert_eq!(
            norm_uuid("6f1c9a0e-3b7d-4e2f-8a5c-1d0e9b8a7c6f").as_deref(),
            Some("6f1c9a0e-3b7d-4e2f-8a5c-1d0e9b8a7c6f")
        );
        assert_eq!(norm_uuid("12"), None);
        assert_eq!(norm_uuid("6f1c9a0e3b7d-4e2f-8a5c-1d0e9b8a7c6f-"), None);
        assert_eq!(norm_uuid(""), None);
    }
}
