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
    /// The embedded element definitions of `/project/collection`, in document order.
    pub definitions: Vec<EmbeddedDef>,
    /// Counts of top-level members the importer does not map (`titleblocktemplates`, `terminal_strips`, …),
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
    /// The placed elements, document order.
    pub elements: Vec<QetElement>,
    /// The conductors, document order.
    pub conductors: Vec<QetConductor>,
    /// Number of free texts (`/diagram/inputs/input`).
    pub free_texts: u32,
    /// Number of shapes (`/diagram/shapes/shape`).
    pub shapes: u32,
    /// Number of images (`/diagram/images/image`).
    pub images: u32,
    /// Number of tables (`/diagram/tables/graphics_table`).
    pub tables: u32,
    /// 1-based line of the `<diagram>` start tag.
    pub line: u32,
}

/// The border attributes of a diagram (`cols colsize rows rowsize height`), each when present and
/// parsable.
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
