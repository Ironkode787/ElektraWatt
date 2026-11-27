// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The column map: which column of a supplier's table fills which member of a part, `member ← header`,
//! and the TOML file it is saved as per source (`<config dir>/imports/<source key>.toml`; the host reads and writes
//! the file, this crate only parses and prints its text).
//!
//! ```toml
//! # The sheet of an XLSX workbook; the first sheet when absent.
//! sheet = "Parts"
//!
//! [columns]
//! manufacturer = "Hersteller"
//! part_number = "Artikelnummer"
//! "description.de" = "Bezeichnung"
//! "tech_data.i_n" = "Nennstrom"
//! ```

use std::collections::BTreeMap;
use std::fmt;

use ew_interop_api::ew_schema::{FreeProp, TechValue};
use serde::{Deserialize, Serialize};

/// A member of a part that a column can fill.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Member {
    /// A field of a part by its path as the part's declaration names it (`manufacturer`, `part_number`, `mech.width`),
    /// or a word several kinds share (`width`: the field of that name the row's kind has). The host names the fields
    /// (the declaration is the host's); this reader reads [`Member::PLAIN`] itself, every other through the host.
    Field(String),
    /// `description.<language>`: the description in one language, one column per language.
    Description(String),
    /// `tech_data.<key>`: a technical value, `<number> <unit>` or an identifier.
    TechData(String),
    /// `free_props.<key>`: a free property, kept as text.
    FreeProp(String),
    /// `like`: the number of a part a new row starts as, taking from it everything its own columns do not give.
    Like,
}

impl Member {
    /// The fields this reader reads without its host, as the plain reader of a table always has: the manufacturer
    /// (its name, key, alias or brand, resolved through the master list; required), the part number (required), the
    /// type designation, the order number, the GTIN (8, 12, 13 or 14 digits), the description without a language, the
    /// kind (a kind's name, `device` when empty) and the order unit (`piece`, `m`, `set`; `piece` when empty).
    pub const PLAIN: [&'static str; 8] = [
        "manufacturer",
        "part_number",
        "type_designation",
        "order_number",
        "gtin",
        "description",
        "kind",
        "unit",
    ];

    /// Whether a part needs this member mapped (`manufacturer`, `part_number`; [`MapError::Missing`]).
    pub fn required(&self) -> bool {
        matches!(self, Member::Field(f) if f == "manufacturer" || f == "part_number")
    }

    /// The field `name` ([`Member::Field`]).
    pub fn field(name: &str) -> Member {
        Member::Field(name.to_owned())
    }

    /// Whether this reader reads the member without its host: a field of [`Member::PLAIN`], a description per
    /// language, a technical value or a free property.
    pub fn plain(&self) -> bool {
        match self {
            Member::Field(f) => Member::PLAIN.contains(&f.as_str()),
            Member::Like => false,
            _ => true,
        }
    }

    /// The member's name in a map, such as `part_number`, `mech.width` or `tech_data.i_n`.
    pub fn name(&self) -> String {
        match self {
            Member::Field(f) => f.clone(),
            Member::Description(lang) => format!("description.{lang}"),
            Member::TechData(key) => format!("tech_data.{key}"),
            Member::FreeProp(key) => format!("free_props.{key}"),
            Member::Like => "like".into(),
        }
    }

    /// The member named `name`: `like`, `description.<language>`, `tech_data.<key>`, `free_props.<key>`, else a field
    /// path of one to four lower-case identifiers joined by `.`.
    ///
    /// # Errors
    ///
    /// [`MapError::UnknownMember`] for a name that is no member, [`MapError::BadKey`] for a technical-data or
    /// free-property key that is not an identifier, [`MapError::BadLanguage`] for a description language that is
    /// not a canonical language tag.
    pub fn parse(name: &str) -> Result<Member, MapError> {
        let unknown = || MapError::UnknownMember(name.to_owned());
        if name == "like" {
            return Ok(Member::Like);
        }
        if let Some((head, tail)) = name.split_once('.') {
            match head {
                "description" => {
                    check_language(tail)?;
                    return Ok(Member::Description(tail.to_owned()));
                }
                "tech_data" => {
                    check_key(name, tail)?;
                    return Ok(Member::TechData(tail.to_owned()));
                }
                "free_props" => {
                    check_key(name, tail)?;
                    return Ok(Member::FreeProp(tail.to_owned()));
                }
                _ => {}
            }
        }
        let segment = |s: &str| {
            !s.is_empty()
                && s.starts_with(|c: char| c.is_ascii_lowercase())
                && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        };
        let segments: Vec<&str> = name.split('.').collect();
        if segments.len() > 4 || !segments.iter().all(|s| segment(s)) {
            return Err(unknown());
        }
        Ok(Member::Field(name.to_owned()))
    }
}

impl fmt::Display for Member {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// A key of technical data or a free property: an identifier the format reads.
fn check_key(member: &str, key: &str) -> Result<(), MapError> {
    let probe = serde_json::json!({"key": key, "value": "x"});
    serde_json::from_value::<TechValue>(probe)
        .map(|_| ())
        .map_err(|_| MapError::BadKey(member.to_owned()))
}

/// A description language: a canonical BCP 47 tag, as a multi-language text's key.
fn check_language(lang: &str) -> Result<(), MapError> {
    let probe = serde_json::json!({"key": "k", "value": {lang: "x"}});
    serde_json::from_value::<FreeProp>(probe)
        .map(|_| ())
        .map_err(|_| MapError::BadLanguage(lang.to_owned()))
}

/// Why a column map is not usable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MapError {
    /// The text is not a map file (TOML syntax, an unknown top-level member, a header that is not a string).
    Syntax(String),
    /// A member name that is no part member.
    UnknownMember(String),
    /// A technical-data or free-property key that is not an identifier.
    BadKey(String),
    /// A description language that is not a canonical language tag (`en`, `de`, `nl-NL`).
    BadLanguage(String),
    /// `manufacturer` or `part_number` has no column.
    Missing(&'static str),
    /// A member's header is empty.
    EmptyHeader(String),
    /// `description` and `description.<language>` together: a text has a neutral value or languages, never both.
    MixedDescription,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::Syntax(m) => write!(f, "the column map cannot be read: {m}"),
            MapError::UnknownMember(m) => write!(
                f,
                "`{m}` is not a part member (manufacturer, part_number, type_designation, order_number, gtin, description[.<language>], kind, unit, tech_data.<key>, free_props.<key>, like, or a field's path)"
            ),
            MapError::BadKey(m) => write!(f, "`{m}`: the key is not an identifier"),
            MapError::BadLanguage(l) => write!(f, "`description.{l}`: `{l}` is not a canonical language tag"),
            MapError::Missing(m) => write!(f, "the column map needs a column for `{m}`"),
            MapError::EmptyHeader(m) => write!(f, "`{m}`: the column header is empty"),
            MapError::MixedDescription => {
                f.write_str("`description` and `description.<language>` cannot be mapped together")
            }
        }
    }
}

impl std::error::Error for MapError {}

/// A column map: `member ← header`, and the sheet of a workbook. Several members may read one column
/// (a supplier whose order number is the part number). Columns the map does not name are reported, never guessed.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ColumnMap {
    /// The sheet of an XLSX workbook; the first sheet when `None`. Ignored for CSV.
    pub sheet: Option<String>,
    columns: BTreeMap<Member, String>,
}

/// The file form of a [`ColumnMap`].
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sheet: Option<String>,
    columns: BTreeMap<String, String>,
}

impl ColumnMap {
    /// The map of `pairs` (`member name`, `header`).
    ///
    /// # Errors
    ///
    /// [`MapError`] for an unknown member, a bad key or language, an empty header, a description both neutral and
    /// per language, or without `manufacturer` or `part_number`.
    pub fn new<'a>(
        sheet: Option<&str>,
        pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<ColumnMap, MapError> {
        let mut columns = BTreeMap::new();
        for (member, header) in pairs {
            let m = Member::parse(member)?;
            let header = header.trim();
            if header.is_empty() {
                return Err(MapError::EmptyHeader(member.to_owned()));
            }
            columns.insert(m, header.to_owned());
        }
        for name in ["manufacturer", "part_number"] {
            if !columns.contains_key(&Member::field(name)) {
                return Err(MapError::Missing(name));
            }
        }
        let neutral = columns.contains_key(&Member::field("description"));
        let per_language = columns.keys().any(|m| matches!(m, Member::Description(_)));
        if neutral && per_language {
            return Err(MapError::MixedDescription);
        }
        Ok(ColumnMap {
            sheet: sheet.map(str::to_owned),
            columns,
        })
    }

    /// Reads a map file's text.
    ///
    /// # Errors
    ///
    /// [`MapError::Syntax`] for text that is not a map file; otherwise as [`ColumnMap::new`].
    pub fn from_toml(text: &str) -> Result<ColumnMap, MapError> {
        let file: MapFile = toml::from_str(text).map_err(|e| MapError::Syntax(e.message().to_owned()))?;
        ColumnMap::new(
            file.sheet.as_deref(),
            file.columns.iter().map(|(m, h)| (m.as_str(), h.as_str())),
        )
    }

    /// The map file's text: canonical, the members in name order, so a saved map diffs cleanly.
    pub fn to_toml(&self) -> String {
        let file = MapFile {
            sheet: self.sheet.clone(),
            columns: self.columns.iter().map(|(m, h)| (m.name(), h.clone())).collect(),
        };
        toml::to_string(&file).unwrap_or_default()
    }

    /// The members and their headers, in member order.
    pub fn columns(&self) -> impl Iterator<Item = (&Member, &str)> + '_ {
        self.columns.iter().map(|(m, h)| (m, h.as_str()))
    }

    /// The header mapped to `member`.
    pub fn header(&self, member: &Member) -> Option<&str> {
        self.columns.get(member).map(String::as_str)
    }
}

/// Checks a source key, the name a column map is saved under (`<config dir>/imports/<source key>.toml`): 1 to 64
/// characters of ASCII lower-case letters, digits, `-`, `_` and `.`, not starting with `.`.
///
/// # Errors
///
/// A message saying what is wrong.
pub fn check_source_key(key: &str) -> Result<(), String> {
    let ok_chars = key
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.'));
    if key.is_empty() || key.len() > 64 || !ok_chars || key.starts_with('.') {
        return Err(format!(
            "`{key}` is not a source key (1 to 64 of a-z, 0-9, `-`, `_`, `.`; not starting with `.`)"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_parse_and_print_alike() {
        for name in [
            "manufacturer",
            "part_number",
            "type_designation",
            "order_number",
            "gtin",
            "description",
            "description.de",
            "description.nl-NL",
            "kind",
            "unit",
            "tech_data.i_n",
            "free_props.colour",
            "mech.width",
            "width",
            "like",
        ] {
            assert_eq!(Member::parse(name).unwrap().name(), name);
        }
        for bad in ["Price", "mech..width", "a.b.c.d.e", "", "9w"] {
            assert_eq!(
                Member::parse(bad),
                Err(MapError::UnknownMember(bad.into())),
                "{bad}"
            );
        }
        assert_eq!(
            Member::parse("tech_data. x"),
            Err(MapError::BadKey("tech_data. x".into()))
        );
        assert_eq!(
            Member::parse("description.EN_us"),
            Err(MapError::BadLanguage("EN_us".into()))
        );
    }

    #[test]
    fn a_map_needs_the_manufacturer_and_the_part_number() {
        assert_eq!(
            ColumnMap::new(None, [("part_number", "PN")]),
            Err(MapError::Missing("manufacturer"))
        );
        assert_eq!(
            ColumnMap::new(None, [("manufacturer", "M")]),
            Err(MapError::Missing("part_number"))
        );
        assert_eq!(
            ColumnMap::new(None, [("manufacturer", "M"), ("part_number", " ")]),
            Err(MapError::EmptyHeader("part_number".into()))
        );
        assert_eq!(
            ColumnMap::new(
                None,
                [
                    ("manufacturer", "M"),
                    ("part_number", "P"),
                    ("description", "D"),
                    ("description.en", "E")
                ]
            ),
            Err(MapError::MixedDescription)
        );
    }

    #[test]
    fn the_file_form_round_trips_canonically() {
        let text = "sheet = \"Parts\"\n\n[columns]\n\"tech_data.i_n\" = \"Rated current\"\npart_number = \"Article\"\nmanufacturer = \"Maker\"\n";
        let map = ColumnMap::from_toml(text).unwrap();
        assert_eq!(map.sheet.as_deref(), Some("Parts"));
        assert_eq!(map.header(&Member::field("part_number")), Some("Article"));
        let printed = map.to_toml();
        assert_eq!(ColumnMap::from_toml(&printed).unwrap(), map);
        // Members in name order.
        let at = |s: &str| printed.find(s).unwrap();
        assert!(at("manufacturer") < at("part_number") && at("part_number") < at("tech_data.i_n"));
        assert!(matches!(
            ColumnMap::from_toml("[columns]\nmanufacturer = 1\n"),
            Err(MapError::Syntax(_))
        ));
        assert!(matches!(
            ColumnMap::from_toml("other = 1\n[columns]\n"),
            Err(MapError::Syntax(_))
        ));
    }

    #[test]
    fn source_keys_are_file_names_without_paths() {
        assert!(check_source_key("acme-2026.q3").is_ok());
        for bad in ["", ".hidden", "a/b", "..", "A", "a b", &"x".repeat(65)] {
            assert!(check_source_key(bad).is_err(), "{bad}");
        }
    }
}
