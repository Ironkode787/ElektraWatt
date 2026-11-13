// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The TIA Portal "PLC Tags" table (doc 10 §4.5, FR-PLC-036): the rows of the sheet [`SHEET`] with exactly the
//! columns [`COLUMNS`], one row per channel with a determined symbol and an address.
//!
//! The table is a fixed format with a round trip, so its rows are made and read here and not by a machine profile.
//! This module knows the rows only: the host writes them into a workbook and reads a workbook's sheet back with the
//! table readers it has. Addresses are in the notation of the built-in TIA profile's exchange form (`%Q0.1`),
//! whatever profile the project draws with. After the ten columns the sheet carries the hidden column [`ID_COLUMN`]
//! with the channel's ID (FR-PLC-056), which the import matches by before the name and the address.
//!
//! The three HMI flags are written `True`: a setting for them and the defaults of TIA Portal are not established
//! (doc 10 §4.5).

use ew_caex::{ExchangeImage, IoType, XTag};
use ew_derive::ew_ids::Id;
use ew_derive::ew_plcaddr::Purpose;
use ew_derive::ew_schema::Channel;

use crate::image::Projection;

/// The sheet's name.
pub const SHEET: &str = "PLC Tags";

/// The sheet's columns, in order.
pub const COLUMNS: [&str; 10] = [
    "Name",
    "Path",
    "Data Type",
    "Logical Address",
    "Comment",
    "Hmi Visible",
    "Hmi Accessible",
    "Hmi Writeable",
    "Typeobject ID",
    "Version ID",
];

/// The header of the hidden column after [`COLUMNS`]: the channel's ID.
pub const ID_COLUMN: &str = "EW ID";

/// The comment field a table in language `lang` carries: `comment.<lang>`, or `comment` without a language.
pub fn comment_field(lang: Option<&str>) -> String {
    match lang {
        Some(l) => format!("comment.{l}"),
        None => "comment".to_owned(),
    }
}

/// The rows of the sheet for `projection`, the header first; each row has the ten columns and the channel's ID.
/// The comment is the text in `lang`, else the text without a language.
pub fn tag_rows(projection: &Projection, lang: Option<&str>) -> Vec<Vec<String>> {
    let profile = ew_derive::builtin_profiles()
        .iter()
        .find(|p| p.reference().starts_with("siemens-tia-intl@"));
    let mut header: Vec<String> = COLUMNS.iter().map(|c| (*c).to_owned()).collect();
    header.push(ID_COLUMN.to_owned());
    let mut rows = vec![header];
    for (tag, info) in projection.image.tags.iter().zip(&projection.tags) {
        let comment = lang
            .and_then(|l| tag.comment.get(l))
            .or_else(|| tag.comment.get(""))
            .cloned()
            .unwrap_or_default();
        rows.push(vec![
            tag.name.clone(),
            projection.image.project.tag_table.clone(),
            tag.data_type.clone().unwrap_or_default(),
            profile
                .map(|p| p.render(&info.address, Purpose::Exchange))
                .unwrap_or_default(),
            comment,
            "True".to_owned(),
            "True".to_owned(),
            "True".to_owned(),
            String::new(),
            String::new(),
            info.channel.to_string(),
        ]);
    }
    rows
}

/// The image of a tag table's `rows` (the header first): one tag per row with a name. Columns are found by their
/// header, without regard to case; a table without the columns `Name` and `Logical Address` is refused in words.
/// The comment column is the comment in `lang` (else without a language); a row's ID column gives the tag the ID
/// the project writes for that channel's tag.
///
/// # Errors
///
/// The reason the rows are no tag table.
pub fn read_tags(rows: &[Vec<String>], lang: Option<&str>) -> Result<ExchangeImage, String> {
    let Some(header) = rows.first() else {
        return Err("the sheet is empty: a tag table starts with the header row".to_owned());
    };
    let column = |name: &str| header.iter().position(|h| h.trim().eq_ignore_ascii_case(name));
    let (Some(name), Some(address)) = (column(COLUMNS[0]), column(COLUMNS[3])) else {
        return Err(format!(
            "the sheet has no column `{}` or no column `{}`: it is not a tag table",
            COLUMNS[0], COLUMNS[3]
        ));
    };
    let (data_type, comment, id) = (column(COLUMNS[2]), column(COLUMNS[4]), column(ID_COLUMN));
    let cell = |row: &[String], i: Option<usize>| -> Option<String> {
        let text = row.get(i?)?.trim();
        (!text.is_empty()).then(|| text.to_owned())
    };
    let mut image = ExchangeImage::default();
    if let Some(path) = column(COLUMNS[1]) {
        image.project.tag_table = rows
            .iter()
            .skip(1)
            .find_map(|r| cell(r, Some(path)))
            .unwrap_or_default();
    }
    for row in rows.iter().skip(1) {
        let Some(name) = cell(row, Some(name)) else {
            continue;
        };
        let address = cell(row, Some(address));
        let io = address.as_deref().and_then(|a| {
            match a.trim_start_matches('%').chars().next()?.to_ascii_uppercase() {
                'I' | 'E' => Some(IoType::Input),
                'Q' | 'A' => Some(IoType::Output),
                _ => None,
            }
        });
        image.tags.push(XTag {
            id: cell(row, id)
                .and_then(|c| c.parse::<Id<Channel>>().ok())
                .map(|c| ew_derive::ew_ids::v5::plc_tag_id::<_, ()>(c).to_string())
                .unwrap_or_default(),
            name,
            data_type: cell(row, data_type),
            io,
            address,
            comment: cell(row, comment)
                .map(|c| (lang.unwrap_or("").to_owned(), c))
                .into_iter()
                .collect(),
            channel: None,
        });
    }
    Ok(image)
}
