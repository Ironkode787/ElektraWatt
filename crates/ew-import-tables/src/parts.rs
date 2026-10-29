// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts tables into an existing project (FT-410; the golden-dialect slice of WP-S1.19a's parts import): the
//! `parts.csv` a person chose and, when they lie in the same folder, its sibling tables `part_cps.csv`,
//! `part_templates.csv`, `cable_cores.csv` and `part_plc.csv`, read into project-local part copies exactly as the
//! bootstrap mints them (`emit/parts.rs`): `library_id` = the project ID, `item_id = v5(ns, "parts|{key}")` in the
//! golden lineage's namespace, so the same key gives the same copy on every import (a re-import is idempotent).
//!
//! [`parse_parts_tables`] is path-free: the host reads the files and passes their text; the copies go to the
//! `parts.import` command as its parameter (commands stay pure). Unlike the bootstrap, which drops a bad row with a
//! finding and goes on, this import is **strict**: the first finding of any table (a missing or an unknown column, a
//! malformed cell, a duplicate key, a reference to an unknown part or FnDef, a foreign colour code) refuses the whole
//! import with its pointer (file, line, row key and the column the message names).
//!
//! A freely mapped supplier CSV with a column-mapping form is not this slice (WP-S2.09).

use std::sync::Arc;

use ew_ids::Id;
use ew_interop_api::{ImportError, ImportMode, ReportEntry};
use ew_schema::{LibCopy, LibDocument, Manufacturer, Part, Project};

use crate::csv::{self, TableLimits};
use crate::emit::{self, Cx};
use crate::import::{error_line, limit_error, limit_value};
use crate::library::BuiltinLibrary;
use crate::schema;
use crate::table::{Findings, Table};

/// The file name of the parts table of the golden dialect.
pub const PARTS_FILE: &str = "parts.csv";

/// The sibling tables of a `parts.csv`, as text; `None` for a table the folder does not have.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SiblingTables<'a> {
    /// `part_cps.csv`: the CP patterns.
    pub part_cps: Option<&'a str>,
    /// `part_templates.csv`: the function templates.
    pub part_templates: Option<&'a str>,
    /// `cable_cores.csv`: the cores of cable parts.
    pub cable_cores: Option<&'a str>,
    /// `part_plc.csv`: the PLC sections.
    pub part_plc: Option<&'a str>,
}

impl<'a> SiblingTables<'a> {
    /// The file names of the sibling tables, in field order.
    pub const FILE_NAMES: [&'static str; 4] = [
        "part_cps.csv",
        "part_templates.csv",
        "cable_cores.csv",
        "part_plc.csv",
    ];

    /// Sets the table of file name `name` (one of [`SiblingTables::FILE_NAMES`]) to `text`; `false` for another
    /// name.
    pub fn set(&mut self, name: &str, text: &'a str) -> bool {
        let slot = match name {
            "part_cps.csv" => &mut self.part_cps,
            "part_templates.csv" => &mut self.part_templates,
            "cable_cores.csv" => &mut self.cable_cores,
            "part_plc.csv" => &mut self.part_plc,
            _ => return false,
        };
        *slot = Some(text);
        true
    }
}

/// The base name of `source`: the text after its last `/` or `\` (a path never reaches a copy's provenance).
fn base_name(source: &str) -> &str {
    source.rsplit(['/', '\\']).next().unwrap_or(source)
}

/// The first finding as the refusal: `<file>[, row `<key>`]: <message>` at the finding's line. The message names the
/// column (`column `kind`: ...`, `required column(s) missing: ...`, `unknown column `x``).
fn refusal(entry: &ReportEntry) -> ImportError {
    let loc = entry.location.clone().unwrap_or_default();
    let message = entry
        .message
        .strip_prefix("row dropped: ")
        .or_else(|| entry.message.strip_prefix("table refused: "))
        .unwrap_or(&entry.message);
    let message = message.strip_suffix(" ignored").unwrap_or(message);
    let row = match loc.item.as_deref() {
        Some(key) if !key.is_empty() => format!(", row `{key}`"),
        _ => String::new(),
    };
    ImportError::Malformed {
        message: format!("{}{row}: {message}", loc.file),
        line: loc.line,
    }
}

/// Refuses on the first finding.
fn strict(findings: Findings) -> Result<(), ImportError> {
    match findings.finish().first() {
        None => Ok(()),
        Some(e) => Err(refusal(e)),
    }
}

/// Reads table `name` of `file` from `text` under the strict mode; `None` when a required column is missing (a
/// finding).
fn load(name: &str, file: &str, text: &str, findings: &mut Findings) -> Result<Option<Table>, ImportError> {
    let spec = *schema::ALL
        .iter()
        .find(|s| s.name == name)
        .expect("a table of the golden dialect");
    let bytes: Arc<[u8]> = Arc::from(text.as_bytes());
    match csv::parse(&bytes, &TableLimits::default()) {
        Ok(t) => Ok(Table::new(
            spec,
            file,
            Arc::clone(&bytes),
            t,
            ImportMode::Strict,
            findings,
        )),
        Err(e) => match limit_error(&e) {
            Some(what) => Err(ImportError::LimitExceeded {
                what: format!("{what} of {file}"),
                limit: limit_value(&e),
            }),
            None => Err(ImportError::Malformed {
                message: format!("{file}: {e}"),
                line: error_line(&e),
            }),
        },
    }
}

/// Parses a `parts.csv` of the golden dialect and its sibling tables into project-local part copies of project
/// `project`, in `parts.csv` row order, with the manufacturer copies they name ([`PartsTables`]).
///
/// `source` names the chosen file; its base name (never a path) is the copies' provenance (`meta.sources`, FR-PRT-017;
/// no import date: the revision carries the time) and the file name of the pointers. FnDef keys of the templates
/// resolve in `library` (the starter FnDefs). Every copy is the bootstrap's (`emit/parts.rs`) but for its provenance:
/// `library_id` = `project`, `item_id = v5(ns, "parts|{key}")`, `meta.key` = the row key, `sha256` the item hash.
///
/// # Errors
///
/// [`ImportError::LimitExceeded`] for a table above the [`TableLimits`]; [`ImportError::Malformed`] for the first
/// problem of any table, with its line and a message naming the file, the row key and the column: a table that is not
/// of the dialect, a missing or unknown column, a malformed cell, an empty or duplicate key, a child row of an
/// unknown part, an unknown FnDef, a foreign colour code (strict mode).
pub fn parse_parts_tables(
    parts_csv: &str,
    siblings: &SiblingTables<'_>,
    project: Id<Project>,
    source: &str,
    library: &BuiltinLibrary,
) -> Result<PartsTables, ImportError> {
    let file = base_name(source);
    let file = if file.is_empty() { PARTS_FILE } else { file };
    let mut findings = Findings::default();
    let parts = load("parts", file, parts_csv, &mut findings)?;
    let mut children: [Option<Table>; 4] = [None, None, None, None];
    let texts = [
        siblings.part_cps,
        siblings.part_templates,
        siblings.cable_cores,
        siblings.part_plc,
    ];
    for (i, (name, text)) in SiblingTables::FILE_NAMES.iter().zip(texts).enumerate() {
        if let Some(text) = text {
            let stem = name.strip_suffix(".csv").unwrap_or(name);
            children[i] = load(stem, name, text, &mut findings)?;
        }
    }
    strict(findings)?;
    let mut cx = Cx::new(library, project);
    cx.part_source = Some(file.to_owned());
    let [cps, templates, cores, plc] = &children;
    emit::parts::emit(
        &mut cx,
        parts.as_ref(),
        cps.as_ref(),
        templates.as_ref(),
        cores.as_ref(),
        plc.as_ref(),
    );
    strict(std::mem::take(&mut cx.findings))?;
    let mut out = PartsTables::default();
    for doc in std::mem::take(&mut cx.local_lib) {
        match doc {
            LibDocument::Part(c) => out.parts.push(c),
            LibDocument::Manufacturer(c) => out.manufacturers.push(c),
            _ => {}
        }
    }
    Ok(out)
}

/// What [`parse_parts_tables`] reads: the project-local part copies in `parts.csv` row order, and the project-local
/// manufacturer copies they name in name order (WP-S1.19 D10: `item_id = v5(ns, "manufacturers|<name>")`, the key
/// and name the table's manufacturer cell).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PartsTables {
    /// The part copies.
    pub parts: Vec<LibCopy<Part>>,
    /// The manufacturer copies the parts name.
    pub manufacturers: Vec<LibCopy<Manufacturer>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_name_drops_every_directory() {
        assert_eq!(base_name("a/b\\parts.csv"), "parts.csv");
        assert_eq!(base_name("parts.csv"), "parts.csv");
    }

    #[test]
    fn siblings_are_set_by_file_name() {
        let mut s = SiblingTables::default();
        assert!(s.set("cable_cores.csv", "x"));
        assert!(!s.set("devices.csv", "x"));
        assert_eq!(s.cable_cores, Some("x"));
    }
}
