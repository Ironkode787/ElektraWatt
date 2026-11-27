// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock dialog's rows (doc 07 §2.5 "The workshop stock", Project ▸ Edit stock…): each stock part
//! with what it is for, read from its own sections; a part typed by its number, held or from a library, with the
//! documents that embed it.

use ew_ids::{Id, Uuid};
use ew_schema::{LibDocument, Part, PartKind};
use serde_json::Value;

use crate::Session;

/// One row of the stock dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StockRow {
    /// The part copy (the ID it has or will have once embedded).
    pub copy: Uuid,
    /// The part kind as the file spells it (`wire`, `ferrule`, `marker`).
    pub kind: String,
    /// The part number.
    pub number: String,
    /// The description's neutral text, else its first language's.
    pub description: String,
    /// What it is for, in numbers: a wire's or a ferrule's cross-section (`1.5`, `0.5 – 1`, a twin ferrule's
    /// `2 × 1.5`), in mm²; empty for a marker.
    pub size: String,
    /// A marker's marks word (`wire`, `terminal`, `device`, `cable`); empty for the others.
    pub marks: String,
    /// Its colour codes (`BK`, `GNYE`), empty without one.
    pub colour: String,
}

/// The row of part `p` under the copy ID `copy`.
fn row_of(copy: Uuid, p: &Part) -> StockRow {
    let s = &p.sections;
    let colour = |c: Option<&ew_schema::ColourSpec>| {
        c.and_then(|c| serde_json::to_value(c).ok())
            .map(|v| match v {
                Value::Array(a) => a.iter().filter_map(Value::as_str).collect::<String>(),
                Value::String(t) => t,
                _ => String::new(),
            })
            .unwrap_or_default()
    };
    let (size, marks, colour) = if let Some(w) = &s.wire {
        (w.csa.to_string(), String::new(), colour(w.colour.as_ref()))
    } else if let Some(f) = &s.ferrule {
        let range = if f.csa_min == f.csa_max {
            f.csa_min.to_string()
        } else {
            format!("{} \u{2013} {}", f.csa_min, f.csa_max)
        };
        let size = if f.twin {
            format!("2 \u{d7} {range}")
        } else {
            range
        };
        (size, String::new(), colour(f.colour.as_ref()))
    } else if let Some(m) = &s.marker {
        (
            String::new(),
            crate::part_editor::stored_word(&m.marks),
            String::new(),
        )
    } else {
        (String::new(), String::new(), String::new())
    };
    StockRow {
        copy,
        kind: crate::part_editor::stored_word(&p.kind),
        number: p.part_number.as_str().to_owned(),
        description: p.description.neutral_or_first().unwrap_or_default().to_owned(),
        size,
        marks,
        colour,
    }
}

impl Session {
    /// The project's workshop stock as the dialog's rows, in its order of preference.
    pub fn stock_rows(&self) -> Vec<StockRow> {
        let q = self.query();
        q.project()
            .settings
            .workshop
            .stock
            .iter()
            .filter_map(|r| q.parts().get(r.copy).map(|c| row_of(r.copy.as_uuid(), &c.item)))
            .collect()
    }

    /// The project's cut increment in millimetres as the dialog shows it; empty for none (5 mm).
    pub fn stock_cut(&self) -> String {
        self.query()
            .project()
            .settings
            .workshop
            .cut_increment
            .map(|n| ew_derive::mm_exact(i128::from(n.0)))
            .unwrap_or_default()
    }

    /// The part numbered `number` as a stock row, with the library documents that embed it when the project does not
    /// hold it.
    ///
    /// # Errors
    ///
    /// The refusal in words: no part has the number (`error-part-accessory`), or it is no wire, ferrule, marker or gland
    /// (`refusal-stock-kind`, the command's words).
    pub fn stock_part(&self, number: &str) -> Result<(StockRow, Vec<Value>), crate::Text> {
        let q = self.query();
        let t = number.trim();
        let Some((r, kind, docs)) = self.part_by_number(&q, t) else {
            return Err(text!("error-part-accessory", t));
        };
        if !kind.is_consumable() && kind != PartKind::Gland {
            return Err(text!("error-stock-kind", t));
        }
        let copy: Id<Part> = ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
        let part = match q.parts().get(copy) {
            Some(c) => c.item.clone(),
            None => docs
                .iter()
                .find_map(|d| match LibDocument::from_value(d.clone()) {
                    Ok(LibDocument::Part(c)) if c.id == copy => Some(c.item),
                    _ => None,
                })
                .ok_or_else(|| text!("error-part-accessory", t))?,
        };
        Ok((row_of(copy.as_uuid(), &part), docs))
    }
}

impl Session {
    /// The starter's generic ferrules and markers as stock rows ([`crate::new_project`]'s generic stock), with the
    /// documents that embed them: Add the generic stock in the stock dialog.
    pub fn generic_stock_rows(&self) -> (Vec<StockRow>, Vec<Value>) {
        let (stock, docs) = crate::new_project::generic_stock();
        let rows = stock
            .iter()
            .filter_map(|r| {
                docs.iter()
                    .find_map(|d| match LibDocument::from_value(d.clone()) {
                        Ok(LibDocument::Part(c)) if c.id == r.copy => Some(row_of(r.copy.as_uuid(), &c.item)),
                        _ => None,
                    })
            })
            .collect();
        (rows, docs)
    }
}
