// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of a part's fields, `reference/part-fields.md` (Parts ▸ Fields), from `part-fields.json` of `ew-derive`.

use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde_json::Value;

use super::{PART_FIELDS_SHEET, cell, code, header, str_of};

/// The page of a part's fields (Parts ▸ Fields): one table per group of a part's sheet, each field with its member's
/// path, how a person enters it, and what reads it with what it does without it.
pub(super) fn part_fields_text(sheet: &Value) -> Result<String> {
    let groups = sheet
        .get("groups")
        .and_then(Value::as_array)
        .with_context(|| format!("{PART_FIELDS_SHEET} has no `groups` list"))?;
    let mut sections = String::new();
    for g in groups {
        let _ = write!(
            sections,
            "## {}\n\n| Field | Member | Entered | Read by |\n|---|---|---|---|\n",
            str_of(g, "group")
        );
        for f in g.get("fields").and_then(Value::as_array).into_iter().flatten() {
            let readers: Vec<String> = f
                .get("readers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|r| {
                    format!(
                        "{} ({}): {}",
                        str_of(r, "feature"),
                        str_of(r, "without"),
                        str_of(r, "says")
                    )
                })
                .collect();
            let above = if f.get("above_zero").and_then(Value::as_bool) == Some(true) {
                ", above zero"
            } else {
                ""
            };
            let _ = writeln!(
                sections,
                "| {} | {} | {}{above} | {} |",
                cell(str_of(f, "word")),
                code(str_of(f, "path")),
                cell(str_of(f, "entry")),
                cell(&readers.join("<br>")),
            );
        }
        sections.push('\n');
    }
    Ok(format!(
        "# Parts ▸ Fields\n\n{}\n\n\
         Every field of a part, group by group as the sheet of Parts shows them: the member of the part it is (its path \
         in the part's JSON form, a list's item as `[]`), how a person enters it, and what reads it with what that \
         feature does while it is empty: it refuses the part, it skips a check, it assumes a value, or it only prints \
         it. A field nothing reads is not asked for; it is kept as it is and shown under About ▸ Also stored.\n\n{sections}",
        header("the declaration of a part's fields"),
    ))
}
