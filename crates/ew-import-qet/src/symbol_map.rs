// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol map (doc 14 §4.17): the person's table that binds element definitions of a QElectroTech set to starter
//! functions (coils, contacts, terminals and interruption points), their terminals to the function's connection points
//! and their own texts to the tag, the number and the cross-reference, the element's drawing, place and texts kept.
//!
//! The host reads `symbol-map.csv` beside the `.qet` ([`MAP_FILE`]) and hands its rows over as [`MapRow`]s (the
//! importer reads no file); [`SymbolMap::new`] checks them against the starter FnDefs, so that a wrong row refuses the
//! import in words before anything is read, and the project importer applies a definition's row
//! ([`crate::QetProjectImporter::with_map`]).

use std::fmt;

use ew_schema::{ContactKind, FnDef, LibCopy, LibDocument};

use crate::project::builtins::Builtins;

/// The file name of the symbol map, in the folder of the `.qet` it maps.
pub const MAP_FILE: &str = "symbol-map.csv";

/// The columns of the map, in their order; `element` and `function` are required.
pub const COLUMNS: [&str; 6] = ["element", "function", "cps", "tag", "number", "xref"];

/// One row of the map as the host read it: its line and its cells verbatim, empty for a column the table has not.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MapRow {
    /// The 1-based line of the row.
    pub line: u32,
    /// The element definition: a collection path or its end, or the definition's UUID.
    pub element: String,
    /// The key of the starter FnDef.
    pub function: String,
    /// The FnDef's CP designations, one per terminal, separated by spaces.
    pub cps: String,
    /// The text that prints the tag (a coil's, a contact's; an interruption point's name).
    pub tag: String,
    /// The text that prints a terminal's number.
    pub number: String,
    /// The text that prints an interruption point's cross-reference.
    pub xref: String,
}

/// Why a map is refused: the line, the column when one is at fault, and what is wrong in words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapError {
    /// The 1-based line; 1 for the header.
    pub line: u32,
    /// The column at fault.
    pub column: Option<&'static str>,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.column {
            Some(c) => write!(
                f,
                "{MAP_FILE}, line {}, column `{c}`: {}",
                self.line, self.message
            ),
            None => write!(f, "{MAP_FILE}, line {}: {}", self.line, self.message),
        }
    }
}

impl std::error::Error for MapError {}

/// What a row binds an element to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BindKind {
    /// A device's main function (contact kind coil).
    Coil,
    /// A contact (NO, NC, change-over): an auxiliary function of its master's device, else a device of its own.
    Contact,
    /// A terminal of a strip.
    Terminal,
    /// An interruption point.
    Ip,
}

/// Which of a placed element's own texts a row names (doc 14 §4.17).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum TextRef {
    /// The label information (also a composite text of it alone).
    Label,
    /// The first user text.
    User,
    /// Another element information, by name.
    Info(String),
    /// No text.
    None,
}

/// Which definitions a row names.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Element {
    /// A collection path, or its end after a `/`, without `embed://` and `import/`.
    Path(String),
    /// A definition UUID, lower case without braces.
    Uuid(String),
}

/// One checked row.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Binding {
    pub(crate) line: u32,
    element: Element,
    /// The starter FnDef.
    pub(crate) fndef: LibCopy<FnDef>,
    /// Its key.
    pub(crate) key: String,
    pub(crate) kind: BindKind,
    /// The FnDef CP index of each terminal, in definition order; `None`: terminal `i` is CP `i`.
    pub(crate) cps: Option<Vec<u16>>,
    pub(crate) tag: TextRef,
    pub(crate) number: TextRef,
    pub(crate) xref: TextRef,
}

/// The checked rows of a symbol map.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SymbolMap {
    rows: Vec<Binding>,
}

/// A collection path without `embed://` and a leading `import/`, the form rows and definitions are compared in.
fn bare_path(p: &str) -> &str {
    let p = p.trim();
    let p = p.strip_prefix("embed://").unwrap_or(p);
    p.strip_prefix("import/").unwrap_or(p)
}

/// A UUID text lower case without braces, when `s` is one.
pub(crate) fn bare_uuid(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches('{').trim_end_matches('}');
    ew_ids::Uuid::parse_str(t).ok().map(|u| u.to_string())
}

/// The kind of FnDef `def` the map binds, or why it binds none.
fn kind_of(key: &str, def: &FnDef) -> Result<BindKind, String> {
    let category = def.category.0.as_str();
    match (category, &def.contact_kind) {
        ("InterruptionPoint", _) => Ok(BindKind::Ip),
        ("Terminal" | "TerminalEarth", _) => Ok(BindKind::Terminal),
        (_, ContactKind::Coil) => Ok(BindKind::Coil),
        (_, ContactKind::No | ContactKind::Nc | ContactKind::Co) => Ok(BindKind::Contact),
        _ => Err(format!(
            "`{key}` is a function of the kind {category}: the map binds coils, contacts, terminals and interruption \
             points"
        )),
    }
}

/// A text column's cell: empty for the kind's default, `label`, `user`, `-` or an information name.
fn text_ref(cell: &str, default: TextRef) -> Result<TextRef, String> {
    match cell.trim() {
        "" => Ok(default),
        "label" => Ok(TextRef::Label),
        "user" => Ok(TextRef::User),
        "-" => Ok(TextRef::None),
        name if name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') =>
        {
            Ok(TextRef::Info(name.to_owned()))
        }
        other => Err(format!(
            "`{other}` names no text: write `label`, `user`, `-` or the name of an element information"
        )),
    }
}

impl SymbolMap {
    /// The map of `rows`, their functions resolved in the starter documents `docs`.
    ///
    /// # Errors
    ///
    /// [`MapError`] for the first wrong row (doc 14 §4.17): an empty element or function, a function no starter FnDef
    /// has or one the map binds none of (no coil, contact, terminal or interruption point), a CP designation the
    /// FnDef has not or one named twice, a text cell that names no text or a column the row's kind does not take, and
    /// an element named by an earlier row.
    pub fn new(
        rows: Vec<MapRow>,
        docs: impl IntoIterator<Item = LibDocument>,
    ) -> Result<SymbolMap, MapError> {
        let builtins = Builtins::new(docs);
        let mut out: Vec<Binding> = Vec::with_capacity(rows.len());
        for row in rows {
            let err = |column: &'static str, message: String| MapError {
                line: row.line,
                column: Some(column),
                message,
            };
            let named = row.element.trim();
            if named.is_empty() {
                return Err(err("element", "no element definition is named".to_owned()));
            }
            let element = match bare_uuid(named) {
                Some(u) => Element::Uuid(u),
                None => Element::Path(bare_path(named).to_owned()),
            };
            if let Some(earlier) = out.iter().find(|b| b.element == element) {
                return Err(err(
                    "element",
                    format!("`{named}` is mapped by line {} already", earlier.line),
                ));
            }
            let key = row.function.trim();
            if key.is_empty() {
                return Err(err("function", "no function definition is named".to_owned()));
            }
            let Some(fndef) = builtins.fndef(key) else {
                return Err(err(
                    "function",
                    format!("no starter function definition has the key `{key}`"),
                ));
            };
            let kind = kind_of(key, &fndef.item).map_err(|m| err("function", m))?;
            let cps = match row.cps.split_whitespace().collect::<Vec<_>>() {
                list if list.is_empty() => None,
                list => {
                    let mut idx = Vec::with_capacity(list.len());
                    for d in list {
                        let Some(cp) = fndef.item.cps.iter().find(|c| c.designation.as_str() == d) else {
                            let has: Vec<&str> =
                                fndef.item.cps.iter().map(|c| c.designation.as_str()).collect();
                            return Err(err(
                                "cps",
                                format!(
                                    "`{key}` has no connection point `{d}` (it has {})",
                                    has.join(", ")
                                ),
                            ));
                        };
                        if idx.contains(&cp.idx) {
                            return Err(err("cps", format!("connection point `{d}` is named twice")));
                        }
                        idx.push(cp.idx);
                    }
                    Some(idx)
                }
            };
            let unused = |column: &'static str, cell: &str, what: &str| {
                if cell.trim().is_empty() {
                    Ok(TextRef::None)
                } else {
                    Err(err(column, format!("{what} has no {column} text")))
                }
            };
            let what = match kind {
                BindKind::Coil => "a coil",
                BindKind::Contact => "a contact",
                BindKind::Terminal => "a terminal",
                BindKind::Ip => "an interruption point",
            };
            let (tag, number, xref) = match kind {
                BindKind::Coil | BindKind::Contact => (
                    text_ref(&row.tag, TextRef::Label).map_err(|m| err("tag", m))?,
                    unused("number", &row.number, what)?,
                    unused("xref", &row.xref, what)?,
                ),
                // A terminal prints its number as its tag.
                BindKind::Terminal => (
                    unused("tag", &row.tag, what)?,
                    text_ref(&row.number, TextRef::User).map_err(|m| err("number", m))?,
                    unused("xref", &row.xref, what)?,
                ),
                BindKind::Ip => (
                    text_ref(&row.tag, TextRef::User).map_err(|m| err("tag", m))?,
                    unused("number", &row.number, what)?,
                    text_ref(&row.xref, TextRef::Label).map_err(|m| err("xref", m))?,
                ),
            };
            out.push(Binding {
                line: row.line,
                element,
                fndef: fndef.clone(),
                key: key.to_owned(),
                kind,
                cps,
                tag,
                number,
                xref,
            });
        }
        Ok(SymbolMap { rows: out })
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the map has no row.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The rows, in table order.
    pub(crate) fn rows(&self) -> &[Binding] {
        &self.rows
    }

    /// The row of the definition at collection path `path` with the UUID `uuid`: the row of its UUID, else the row whose
    /// path is the longest end of its path.
    pub(crate) fn binding(&self, path: &str, uuid: Option<&str>) -> Option<&Binding> {
        let uuid = uuid.and_then(bare_uuid);
        if let Some(b) = self
            .rows
            .iter()
            .find(|b| matches!(&b.element, Element::Uuid(u) if Some(u) == uuid.as_ref()))
        {
            return Some(b);
        }
        let path = bare_path(path);
        self.rows
            .iter()
            .filter_map(|b| match &b.element {
                Element::Path(p) if path == p || path.ends_with(&format!("/{p}")) => Some((p.len(), b)),
                _ => None,
            })
            .max_by_key(|(n, _)| *n)
            .map(|(_, b)| b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(line: u32, element: &str, function: &str, cps: &str) -> MapRow {
        MapRow {
            line,
            element: element.into(),
            function: function.into(),
            cps: cps.into(),
            ..MapRow::default()
        }
    }

    fn starter() -> Vec<LibDocument> {
        [
            "ew.fn.coil",
            "ew.fn.contact-no",
            "ew.fn.terminal",
            "ew.fn.ip",
            "ew.fn.lamp",
        ]
        .iter()
        .map(|k| {
            let path = format!("{}/../../content/fndefs/{k}.json", env!("CARGO_MANIFEST_DIR"));
            LibDocument::from_json(&std::fs::read_to_string(path).expect("a starter FnDef")).expect("a FnDef")
        })
        .collect()
    }

    #[test]
    fn rows_resolve_their_kind_and_connection_points() {
        let map = SymbolMap::new(
            vec![
                row(2, "01_coils/coil.elmt", "ew.fn.coil", "A2 A1"),
                row(
                    3,
                    "{6F9619FF-8B86-D011-B42D-00C04FC964FF}",
                    "ew.fn.contact-no",
                    "",
                ),
                row(4, "coil.elmt", "ew.fn.terminal", ""),
            ],
            starter(),
        )
        .expect("a map");
        let b = map
            .binding("embed://import/a/01_coils/coil.elmt", None)
            .expect("bound");
        assert_eq!(
            (b.kind, b.cps.clone(), b.line),
            (BindKind::Coil, Some(vec![1, 0]), 2)
        );
        assert_eq!(
            map.binding("embed://import/b/coil.elmt", None).map(|b| b.line),
            Some(4)
        );
        let u = map.binding("x.elmt", Some("6f9619ff-8b86-d011-b42d-00c04fc964ff"));
        assert_eq!(
            u.map(|b| (b.kind, b.tag.clone())),
            Some((BindKind::Contact, TextRef::Label))
        );
        assert!(map.binding("embed://import/a/coil2.elmt", None).is_none());
    }

    #[test]
    fn a_wrong_row_is_refused_in_words() {
        let refused = |r: MapRow| {
            SymbolMap::new(vec![r], starter())
                .expect_err("refused")
                .to_string()
        };
        assert!(refused(row(2, "", "ew.fn.coil", "")).contains("column `element`"));
        assert!(refused(row(2, "a.elmt", "ew.fn.nowhere", "")).contains("no starter function definition"));
        assert!(refused(row(2, "a.elmt", "ew.fn.lamp", "")).contains("the map binds coils"));
        assert!(refused(row(2, "a.elmt", "ew.fn.coil", "A1 X9")).contains("no connection point `X9`"));
        assert!(refused(row(2, "a.elmt", "ew.fn.coil", "A1 A1")).contains("named twice"));
        let mut r = row(5, "a.elmt", "ew.fn.coil", "");
        r.number = "user".into();
        assert_eq!(
            refused(r),
            "symbol-map.csv, line 5, column `number`: a coil has no number text"
        );
        let twice = SymbolMap::new(
            vec![
                row(2, "a.elmt", "ew.fn.coil", ""),
                row(3, "a.elmt", "ew.fn.coil", ""),
            ],
            starter(),
        );
        assert!(
            twice
                .expect_err("refused")
                .to_string()
                .contains("by line 2 already")
        );
    }
}
