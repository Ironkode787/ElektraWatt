// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `profile_tables`: the embedded colour and standard-size tables of the cover (FR-REP-028, FR-REP-038; golden
//! folio 1; doc 08 §4.2, doc 20 §7.1, §7.2, §17.3).
//!
//! The rows are generated from the project's colour and size profiles (`ProjectSettings.extra`
//! `colour_profile` and `size_profile`, doc 03 §5.1), never typed on the cover. They fall into groups, told
//! apart by the last column `table`; the cover form gives each group its own region
//! ([doc 09](../../../../docs/design/09-reports-forms-output.md) §2.2). The kind supplies the texts of the
//! regions as rows of their own: a [`RowKind::GroupHeader`] row is the title of a table, a
//! [`RowKind::GroupFooter`] row a caption or note under one (not a `Heading`: the arrangement of a row set
//! reads a heading as the head of the rows after it and drops one without data rows); their text is in the first
//! cell.
//!
//! | `table` | Rows |
//! |---|---|
//! | `colours` | the title "CABLE COLOUR DEFINITIONS", one data row per profile row (voltage class, function, potential type, colour name, colour code), then the profile's standards text as a caption (doc 20 U-35; the basis text stays metadata) |
//! | `control`, `mains` | the title, then one data row per colour code that has the class's standard size, in the order of the size profile: colour name, colour code, size |
//! | `note` | the note "If size or sign is not indicated in diagram, standard size or sign is to be used." (doc 08 §4.2, doc 20 §7.2) |
//!
//! A project without a colour or size profile has no rows of that part (the built-in defaults of the
//! derivation profile have no cover table), and no note. The note is derived text of the wire-marker mode
//! "only explicit overrides", which the golden project has; the mode is not a setting of this format yet, so
//! the note is printed whenever the cover has a table. The row IDs are the group and the index inside it.

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{ColourSpec, ReportBlock, Unit};
use ew_derive::ew_units::Dec3;
use serde_json::Value;

use crate::kind::ReportKindDef;
use crate::render::colour_text;
use crate::rowset::{Cell, CellType, ColumnDef, QtyUnit, Row, RowId, RowKey, RowKind, RowSet};

/// The colour and size tables of the cover.
#[derive(Clone, Copy, Debug)]
pub struct ProfileTables;

/// The groups, in cover order.
pub const GROUPS: [&str; 4] = ["colours", "control", "mains", "note"];

/// The titles of the three tables.
const TITLES: [(&str, &str); 3] = [
    ("colours", "CABLE COLOUR DEFINITIONS"),
    ("control", "STANDARD SIZES CONTROL"),
    ("mains", "STANDARD SIZES MAINS"),
];

/// The note under the size tables.
const NOTE: &str = "If size or sign is not indicated in diagram, standard size or sign is to be used.";

impl ReportKindDef for ProfileTables {
    fn name(&self) -> &'static str {
        "profile_tables"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("voltage_class", CellType::Text, "Voltage class"),
            ColumnDef::new("function", CellType::Text, "Function"),
            ColumnDef::new("potential_type", CellType::Text, "Potential type"),
            ColumnDef::new("colour", CellType::Text, "Colour"),
            ColumnDef::new("code", CellType::Colour, "Code"),
            ColumnDef::new("size", CellType::Qty, "Size"),
            ColumnDef::new("table", CellType::Ident, "Table"),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let extra = &q.project().settings.extra;
        let mut names: BTreeMap<String, (MlText, ColourSpec)> = BTreeMap::new();
        if let Some(profile) = extra.get("colour_profile") {
            colours(profile, &mut set, &mut names);
        }
        if let Some(profile) = extra.get("size_profile") {
            sizes(profile, &mut set, &names);
        }
        if !set.rows.is_empty() {
            set.rows.push(text_row(
                "note",
                RowKind::GroupFooter,
                "note",
                MlText::neutral(NOTE),
            ));
        }
        set
    }
}

/// The text of a multi-language member (`{"en": "…"}`), empty when absent or malformed.
fn ml(v: Option<&Value>) -> MlText {
    v.and_then(|v| serde_json::from_value::<MlText>(v.clone()).ok())
        .unwrap_or_default()
}

fn id(group: &str, index: usize) -> RowId {
    RowId(vec![
        RowKey::Name(group.to_owned()),
        RowKey::Index(u32::try_from(index).unwrap_or(u32::MAX)),
    ])
}

/// The cells of a row of seven columns, with `group` last.
fn cells(group: &str, fill: Vec<(usize, Cell)>) -> Vec<Cell> {
    let mut out = vec![Cell::Empty; 7];
    for (i, c) in fill {
        out[i] = c;
    }
    out[6] = Cell::Ident(group.to_owned());
    out
}

/// A group header or heading row: `text` in the first cell; `tag` tells it from the data rows of its group.
fn text_row(group: &str, kind: RowKind, tag: &str, text: MlText) -> Row {
    Row {
        id: RowId(vec![RowKey::Name(group.to_owned()), RowKey::Name(tag.to_owned())]),
        kind,
        cells: cells(group, vec![(0, Cell::Text(text))]),
        links: Vec::new(),
        sources: Vec::new(),
    }
}

fn title_of(group: &str) -> MlText {
    TITLES
        .iter()
        .find(|(g, _)| *g == group)
        .map(|(_, t)| MlText::neutral(t))
        .unwrap_or_default()
}

fn colours(profile: &Value, set: &mut RowSet, names: &mut BTreeMap<String, (MlText, ColourSpec)>) {
    let mut rows = Vec::new();
    let list = profile
        .get("rows")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    for (i, row) in list.iter().enumerate() {
        let Some(colour) = row
            .get("colour")
            .and_then(|c| serde_json::from_value::<ColourSpec>(c.clone()).ok())
        else {
            continue;
        };
        let name = ml(row.get("colour_name"));
        names
            .entry(colour_text(&colour))
            .or_insert_with(|| (name.clone(), colour.clone()));
        rows.push(Row::data(
            id("colours", i),
            cells(
                "colours",
                vec![
                    (0, Cell::Text(ml(row.get("group")))),
                    (1, Cell::Text(ml(row.get("function")))),
                    (2, Cell::Text(ml(row.get("potential_type")))),
                    (3, Cell::Text(name)),
                    (4, Cell::Colour(colour)),
                ],
            ),
        ));
    }
    if rows.is_empty() {
        return;
    }
    set.rows.push(text_row(
        "colours",
        RowKind::GroupHeader,
        "title",
        title_of("colours"),
    ));
    set.rows.extend(rows);
    let standards = ml(profile.get("standards"));
    if !standards.is_empty() {
        set.rows
            .push(text_row("colours", RowKind::GroupFooter, "standards", standards));
    }
}

fn sizes(profile: &Value, set: &mut RowSet, names: &BTreeMap<String, (MlText, ColourSpec)>) {
    let list = profile
        .get("rows")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    for group in ["control", "mains"] {
        let Some(row) = list
            .iter()
            .find(|r| r.get("class").and_then(Value::as_str) == Some(group))
        else {
            continue;
        };
        let Some(csa) = row
            .get("csa")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<Dec3>().ok())
        else {
            continue;
        };
        // The codes with their order, sorted by it (stable: equal orders keep the table order).
        let mut codes: Vec<(u64, &str)> = row
            .get("codes")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|c| {
                Some((
                    c.get("order").and_then(Value::as_u64).unwrap_or(u64::MAX),
                    c.get("code")?.as_str()?,
                ))
            })
            .collect();
        codes.sort_by_key(|(order, _)| *order);
        let mut rows = Vec::new();
        for (i, (_, code)) in codes.into_iter().enumerate() {
            // A code the colour profile does not list prints as its text.
            let (name, colour) = match names.get(code) {
                Some(found) => found.clone(),
                None => match serde_json::from_value::<ColourSpec>(serde_json::json!([code])) {
                    Ok(colour) => (MlText::neutral(code), colour),
                    Err(_) => continue,
                },
            };
            rows.push(Row::data(
                id(group, i),
                cells(
                    group,
                    vec![
                        (3, Cell::Text(name)),
                        (4, Cell::Colour(colour)),
                        (5, Cell::Qty(csa, QtyUnit::Unit(Unit::Mm2))),
                    ],
                ),
            ));
        }
        if rows.is_empty() {
            continue;
        }
        set.rows
            .push(text_row(group, RowKind::GroupHeader, "title", title_of(group)));
        set.rows.extend(rows);
    }
}
