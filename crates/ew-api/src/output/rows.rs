// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The row exports of a chosen report block (WP-S1.22 part 2, D8; FT-241's CSV half, FR-EXT-027):
//! `ew export wires`, `ew export terminals [--strip <tag>]`, `--block <name or id>` on `bom`, `cables`, `wires` and
//! `terminals`, and `ew export report --block <name or id> --format csv|pdf`.
//!
//! The rows are a block's rows as its folios print them ([`ew_reports::rows_of`], the BOM in the export's mode through
//! [`ew_reports::arranged`]), written by the one CSV writer of the golden dialect ([`ew_reports::csv::rows_csv`]), so
//! `ew export wires` is `expected/csv/wires.csv` byte for byte. Without `--block` the block is the project's
//! page-oriented block of the kind with the lowest first folio (ties by ID), the rule of `ew export cables|bom`.
//!
//! **Naming a block** ([`find_block`]): its ID (any UUID spelling), else its first folio (`908`). **Terminals**: the
//! rows of each terminal-diagram block are split by strip (a data row's first row-ID component is its strip; a heading
//! row goes with the strip of the next data row); with `--strip <tag>` the output is that strip's rows alone (the
//! golden `terminals_<strip>.csv`), without it every strip of the blocks in block order, each strip's rows after a
//! heading row `heading,<strip tag>,,…`.

use std::fmt;
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Device, ReportBlock, ReportSource};
use ew_derive::{BomMode, DeviceTag, PageOrder, PageRef, Q};
use ew_reports::csv::{csv_field, rows_csv};
use ew_reports::kinds::PartsSummary;
use ew_reports::{Row, RowKey, RowKind, RowSet, RowSetStatus, arranged, kind_name, rows_of};

/// What a row export writes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowExport {
    /// The cable list (`ew export cables`, kind `cable_overview_diagrams`).
    Cables,
    /// The bill of materials in a mode (`ew export bom`, kind `parts_summary`).
    Bom(BomMode),
    /// The wire list (`ew export wires`, kind `wire_list`).
    Wires,
    /// The terminal diagrams (`ew export terminals`, kind `terminal_diagram`), one strip or every strip.
    Terminals {
        /// The strip's printed tag (`--strip`); `None`: every strip, each after a heading row.
        strip: Option<String>,
    },
    /// The rows of a block of any kind (`ew export report --format csv`); needs a named block.
    Report,
}

impl RowExport {
    /// The report kind whose block supplies the rows; `None` for [`RowExport::Report`] (any kind).
    pub fn kind(&self) -> Option<&'static str> {
        match self {
            RowExport::Cables => Some("cable_overview_diagrams"),
            RowExport::Bom(_) => Some("parts_summary"),
            RowExport::Wires => Some("wire_list"),
            RowExport::Terminals { .. } => Some("terminal_diagram"),
            RowExport::Report => None,
        }
    }
}

/// The CSV of a row export.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowsCsv {
    /// The CSV: the header line, then one line per row.
    pub csv: String,
    /// The rows (data and heading rows, the header line excluded).
    pub rows: usize,
    /// The IDs of the blocks the rows come from, in block order.
    pub blocks: Vec<String>,
    /// The kind of the blocks.
    pub kind: String,
    /// For terminals: the printed tags of the strips written, in output order; else empty.
    pub strips: Vec<String>,
}

/// Why a row export could not produce rows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowsError {
    /// The project has no page-oriented report block of the kind.
    NoBlock(&'static str),
    /// No report block has this ID or first folio.
    UnknownBlock(String),
    /// More than one report block has this first folio (their IDs).
    AmbiguousBlock(String, Vec<String>),
    /// The named block is of another kind.
    WrongKind {
        /// The block as named.
        block: String,
        /// Its kind.
        kind: String,
        /// The kind the export reads.
        want: &'static str,
    },
    /// The block's rows do not derive (the reason).
    NotDerived {
        /// The block's kind.
        kind: String,
        /// Why.
        why: String,
    },
    /// No strip of the terminal diagrams has this tag (the tags there are).
    UnknownStrip(String, Vec<String>),
    /// The block has no pages (an embedded block), so it has no folios to write.
    NoPages(String),
}

impl fmt::Display for RowsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowsError::NoBlock(kind) => write!(f, "the project has no report block of kind `{kind}`"),
            RowsError::UnknownBlock(name) => write!(
                f,
                "no report block `{name}` (name a block by its ID or its first folio)"
            ),
            RowsError::AmbiguousBlock(name, ids) => write!(
                f,
                "more than one report block has the first folio `{name}` ({}); name it by its ID",
                ids.join(", ")
            ),
            RowsError::WrongKind { block, kind, want } => {
                write!(f, "the report block `{block}` is of kind `{kind}`, not `{want}`")
            }
            RowsError::NotDerived { kind, why } => {
                write!(f, "the rows of the `{kind}` block are not derived ({why})")
            }
            RowsError::UnknownStrip(tag, tags) if tags.is_empty() => {
                write!(f, "no terminal strip `{tag}` in the terminal diagrams")
            }
            RowsError::UnknownStrip(tag, tags) => write!(
                f,
                "no terminal strip `{tag}` in the terminal diagrams (they list {})",
                tags.join(", ")
            ),
            RowsError::NoPages(block) => write!(f, "the report block `{block}` has no pages"),
        }
    }
}

impl std::error::Error for RowsError {}

/// The kind name of a block's source (`sql_query` for a user SQL report, `unknown` for a source of a newer format).
pub fn block_kind(b: &ReportBlock) -> &'static str {
    match &b.source {
        ReportSource::Kind(k) => kind_name(k).unwrap_or("unknown"),
        ReportSource::SqlQuery(_) => "sql_query",
        ReportSource::Unknown(_) => "unknown",
    }
}

/// The report block `name` names: its ID (any UUID spelling), else its first folio.
///
/// # Errors
///
/// [`RowsError::UnknownBlock`] when none matches; [`RowsError::AmbiguousBlock`] when two blocks share the first folio.
pub fn find_block(q: &Q, name: &str) -> Result<(Id<ReportBlock>, Arc<ReportBlock>), RowsError> {
    let blocks = q.report_blocks();
    if let Ok(id) = Id::<ReportBlock>::parse_lenient(name) {
        return blocks
            .get(id)
            .map(|b| (id, Arc::clone(b)))
            .ok_or_else(|| RowsError::UnknownBlock(name.to_owned()));
    }
    let matching: Vec<(Id<ReportBlock>, Arc<ReportBlock>)> = blocks
        .iter()
        .filter(|(_, b)| b.first_folio.is_some_and(|n| n.to_string() == name))
        .map(|(id, b)| (id, Arc::clone(b)))
        .collect();
    match matching.as_slice() {
        [] => Err(RowsError::UnknownBlock(name.to_owned())),
        [one] => Ok(one.clone()),
        many => Err(RowsError::AmbiguousBlock(
            name.to_owned(),
            many.iter().map(|(id, _)| id.to_string()).collect(),
        )),
    }
}

/// The page-oriented blocks of `kind`, by first folio, then ID.
fn blocks_of_kind(q: &Q, kind: &str) -> Vec<(Id<ReportBlock>, Arc<ReportBlock>)> {
    let mut found: Vec<(Id<ReportBlock>, Arc<ReportBlock>)> = q
        .report_blocks()
        .iter()
        .filter(|(_, b)| b.embedded.is_none() && block_kind(b) == kind)
        .map(|(id, b)| (id, Arc::clone(b)))
        .collect();
    found.sort_by_key(|(id, b)| (b.first_folio.unwrap_or(u32::MAX), *id));
    found
}

/// A derived row set, or why not.
fn derived(set: RowSet, kind: &str) -> Result<RowSet, RowsError> {
    match &set.status {
        RowSetStatus::Derived => Ok(set),
        RowSetStatus::NotDerived(reason) => Err(RowsError::NotDerived {
            kind: kind.to_owned(),
            why: reason.to_string(),
        }),
    }
}

/// The rows of `what` (D8), from the block `block` names or, without one, the default block of the kind.
///
/// # Errors
///
/// [`RowsError`]: no block of the kind, an unknown, ambiguous or wrong-kind block, rows that do not derive, an unknown
/// strip. [`RowExport::Report`] without a block is [`RowsError::UnknownBlock`] of the empty name.
pub fn row_export(q: &Q, what: &RowExport, block: Option<&str>) -> Result<RowsCsv, RowsError> {
    let chosen: Vec<(Id<ReportBlock>, Arc<ReportBlock>)> = match (block, what.kind()) {
        (Some(name), want) => {
            let (id, b) = find_block(q, name)?;
            if let Some(want) = want
                && block_kind(&b) != want
            {
                return Err(RowsError::WrongKind {
                    block: name.to_owned(),
                    kind: block_kind(&b).to_owned(),
                    want,
                });
            }
            vec![(id, b)]
        }
        (None, Some(kind)) => {
            let all = blocks_of_kind(q, kind);
            match what {
                RowExport::Terminals { .. } if !all.is_empty() => all,
                _ => all.into_iter().take(1).collect(),
            }
        }
        (None, None) => return Err(RowsError::UnknownBlock(String::new())),
    };
    let Some((first_id, first)) = chosen.first().cloned() else {
        return Err(RowsError::NoBlock(what.kind().unwrap_or("unknown")));
    };
    let kind = block_kind(&first).to_owned();
    let blocks: Vec<String> = chosen.iter().map(|(id, _)| id.to_string()).collect();
    match what {
        RowExport::Terminals { strip } => terminals(q, &chosen, strip.as_deref(), &kind, blocks),
        RowExport::Bom(mode) => {
            let set = derived(arranged(q, &first, PartsSummary.rows_in_mode(q, *mode)), &kind)?;
            Ok(whole(q, &set, kind, blocks))
        }
        RowExport::Cables | RowExport::Wires | RowExport::Report => {
            let set = derived((*rows_of(q, first_id)).clone(), &kind)?;
            Ok(whole(q, &set, kind, blocks))
        }
    }
}

/// The CSV of a whole row set.
fn whole(q: &Q, set: &RowSet, kind: String, blocks: Vec<String>) -> RowsCsv {
    RowsCsv {
        csv: rows_csv(q, set),
        rows: set.rows.len(),
        blocks,
        kind,
        strips: Vec::new(),
    }
}

/// The printed tag of a strip.
fn strip_tag(q: &Q, strip: Id<Device>) -> String {
    q.get::<DeviceTag>(strip)
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .unwrap_or_default()
}

/// The strip of a data row: its first row-ID component.
fn row_strip(row: &Row) -> Option<Id<Device>> {
    match row.id.0.first() {
        Some(RowKey::Entity(u)) => Some(Id::from_uuid(*u)),
        _ => None,
    }
}

/// The rows of `set` split by strip, in row order; a heading row goes with the next data row's strip (else the last).
fn split_by_strip(set: &RowSet) -> Vec<(Id<Device>, Vec<Row>)> {
    let mut out: Vec<(Id<Device>, Vec<Row>)> = Vec::new();
    let mut pending: Vec<Row> = Vec::new();
    for row in &set.rows {
        let strip = (row.kind == RowKind::Data).then(|| row_strip(row)).flatten();
        let Some(strip) = strip else {
            pending.push(row.clone());
            continue;
        };
        match out.last_mut() {
            Some((s, rows)) if *s == strip => {
                rows.append(&mut pending);
                rows.push(row.clone());
            }
            _ => {
                let mut rows = std::mem::take(&mut pending);
                rows.push(row.clone());
                out.push((strip, rows));
            }
        }
    }
    if let Some((_, rows)) = out.last_mut() {
        rows.append(&mut pending);
    }
    out
}

/// The terminal rows of `chosen`'s blocks: one strip (`--strip`) or every strip after its heading row.
fn terminals(
    q: &Q,
    chosen: &[(Id<ReportBlock>, Arc<ReportBlock>)],
    strip: Option<&str>,
    kind: &str,
    blocks: Vec<String>,
) -> Result<RowsCsv, RowsError> {
    let mut sets: Vec<RowSet> = Vec::with_capacity(chosen.len());
    for (id, _) in chosen {
        sets.push(derived((*rows_of(q, *id)).clone(), kind)?);
    }
    let Some(template) = sets.first() else {
        return Err(RowsError::NoBlock("terminal_diagram"));
    };
    // Every strip once, in block order.
    let mut strips: Vec<(Id<Device>, String, Vec<Row>)> = Vec::new();
    for set in &sets {
        for (s, rows) in split_by_strip(set) {
            if !strips.iter().any(|(t, _, _)| *t == s) {
                strips.push((s, strip_tag(q, s), rows));
            }
        }
    }
    let of = |rows: Vec<Row>| {
        let mut set = template.clone();
        set.rows = rows;
        set.groups = Vec::new();
        set
    };
    if let Some(tag) = strip {
        let Some((_, name, rows)) = strips.iter().find(|(_, t, _)| t == tag).cloned() else {
            return Err(RowsError::UnknownStrip(
                tag.to_owned(),
                strips.into_iter().map(|(_, t, _)| t).collect(),
            ));
        };
        let set = of(rows);
        return Ok(RowsCsv {
            csv: rows_csv(q, &set),
            rows: set.rows.len(),
            blocks,
            kind: kind.to_owned(),
            strips: vec![name],
        });
    }
    let columns = template.columns.len();
    let mut csv = rows_csv(q, &of(Vec::new()));
    let mut count = 0;
    let mut names = Vec::with_capacity(strips.len());
    for (_, name, rows) in strips {
        let mut heading = vec!["heading".to_owned(), csv_field(&name)];
        heading.resize(columns + 1, String::new());
        csv.push_str(&heading.join(","));
        csv.push('\n');
        let set = of(rows);
        let text = rows_csv(q, &set);
        csv.push_str(text.split_once('\n').map_or("", |(_, rest)| rest));
        count += 1 + set.rows.len();
        names.push(name);
    }
    Ok(RowsCsv {
        csv,
        rows: count,
        blocks,
        kind: kind.to_owned(),
        strips: names,
    })
}

/// The folio labels of the pages of the block `block` names, in page order (`ew export report --format pdf`).
///
/// # Errors
///
/// As [`find_block`]; [`RowsError::NoPages`] for a block without pages.
pub fn report_folios(q: &Q, block: &str) -> Result<Vec<String>, RowsError> {
    let (id, _) = find_block(q, block)?;
    let folios: Vec<String> = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Generated { block: b, .. } if b == id))
        .map(|e| e.folio.clone())
        .collect();
    if folios.is_empty() {
        return Err(RowsError::NoPages(block.to_owned()));
    }
    Ok(folios)
}
