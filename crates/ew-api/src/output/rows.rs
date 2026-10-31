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
//! heading row `heading,<strip tag>,,…`. Within a block every run of a strip is kept (runs concatenated in row order);
//! a strip a later block lists again keeps the earlier block's rows, and [`RowsCsv::notices`] says so. `--strip`
//! takes a full tag exactly, else the printed tag, and refuses a printed tag two strips share.

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
    /// Sentences for the user about rows left out (a strip a later block lists again); the CLI writes each on stderr.
    pub notices: Vec<String>,
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
    /// More than one strip prints this tag (their full tags).
    AmbiguousStrip(String, Vec<String>),
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
            RowsError::AmbiguousStrip(tag, fulls) => write!(
                f,
                "more than one terminal strip has the tag `{tag}` ({}); name it by its full tag",
                fulls.join(", ")
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
        notices: Vec::new(),
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

/// The strips of the terminal blocks, each once (cycle-40 review, part 3, finding 3).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct GatheredStrips {
    /// Each strip with the label of the block its rows come from and its rows: every run of the strip in that
    /// block, concatenated in the block's row order. In order of first appearance, block by block.
    strips: Vec<(Id<Device>, String, Vec<Row>)>,
    /// A strip that a later block lists again, with that block's label: its rows there are left out.
    repeats: Vec<(Id<Device>, String)>,
}

/// The strips of `blocks` (each block's label, its first folio, and its rows): within a block every run of a strip
/// is kept, runs concatenated; a strip a later block lists again keeps the earlier block's rows and is a repeat.
fn gather_strips(blocks: &[(String, &RowSet)]) -> GatheredStrips {
    let mut out = GatheredStrips::default();
    for (label, set) in blocks {
        let mut here: Vec<(Id<Device>, Vec<Row>)> = Vec::new();
        for (s, mut rows) in split_by_strip(set) {
            match here.iter_mut().find(|(t, _)| *t == s) {
                Some((_, kept)) => kept.append(&mut rows),
                None => here.push((s, rows)),
            }
        }
        for (s, rows) in here {
            if out.strips.iter().any(|(t, _, _)| *t == s) {
                out.repeats.push((s, label.clone()));
            } else {
                out.strips.push((s, label.clone(), rows));
            }
        }
    }
    out
}

/// The one strip `tag` names among `tags` (each strip's printed and full tag): the full tag exactly, else the printed
/// tag; `None` when no strip has it.
///
/// # Errors
///
/// [`RowsError::AmbiguousStrip`] when two strips print the tag (their full tags).
fn pick_strip(tag: &str, tags: &[(String, String)]) -> Result<Option<usize>, RowsError> {
    if let Some(i) = tags.iter().position(|(_, full)| full == tag) {
        return Ok(Some(i));
    }
    let printed: Vec<usize> = (0..tags.len()).filter(|&i| tags[i].0 == tag).collect();
    match printed.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(*one)),
        many => Err(RowsError::AmbiguousStrip(
            tag.to_owned(),
            many.iter().map(|&i| tags[i].1.clone()).collect(),
        )),
    }
}

/// The full tag of a strip (`=P01+L1-X1`).
fn strip_full_tag(q: &Q, strip: Id<Device>) -> String {
    q.get::<DeviceTag>(strip)
        .as_ref()
        .as_ref()
        .map(|t| t.full.clone())
        .unwrap_or_default()
}

/// The terminal rows of `chosen`'s blocks: one strip (`--strip`) or every strip after its heading row.
fn terminals(
    q: &Q,
    chosen: &[(Id<ReportBlock>, Arc<ReportBlock>)],
    strip: Option<&str>,
    kind: &str,
    blocks: Vec<String>,
) -> Result<RowsCsv, RowsError> {
    let mut sets: Vec<(String, RowSet)> = Vec::with_capacity(chosen.len());
    for (id, b) in chosen {
        let label = b.first_folio.map_or_else(|| id.to_string(), |n| n.to_string());
        sets.push((label, derived((*rows_of(q, *id)).clone(), kind)?));
    }
    let Some((_, template)) = sets.first() else {
        return Err(RowsError::NoBlock("terminal_diagram"));
    };
    let gathered = gather_strips(&sets.iter().map(|(l, s)| (l.clone(), s)).collect::<Vec<_>>());
    // Every strip once, in block order, with its printed tag and the block its rows come from.
    let strips: Vec<(Id<Device>, String, String, Vec<Row>)> = gathered
        .strips
        .into_iter()
        .map(|(s, label, rows)| (s, strip_tag(q, s), label, rows))
        .collect();
    let notices_of = |only: Option<Id<Device>>| -> Vec<String> {
        gathered
            .repeats
            .iter()
            .filter(|(s, _)| only.is_none_or(|o| o == *s))
            .filter_map(|(s, later)| {
                let (_, tag, from, _) = strips.iter().find(|(t, _, _, _)| t == s)?;
                Some(format!(
                    "{tag} is also in the block on folio {later}; its rows come from folio {from}."
                ))
            })
            .collect()
    };
    let of = |rows: Vec<Row>| {
        let mut set = template.clone();
        set.rows = rows;
        set.groups = Vec::new();
        set
    };
    if let Some(tag) = strip {
        let tags: Vec<(String, String)> = strips
            .iter()
            .map(|(s, t, _, _)| (t.clone(), strip_full_tag(q, *s)))
            .collect();
        let Some(i) = pick_strip(tag, &tags)? else {
            return Err(RowsError::UnknownStrip(
                tag.to_owned(),
                strips.into_iter().map(|(_, t, _, _)| t).collect(),
            ));
        };
        let (id, name, _, rows) = strips[i].clone();
        let set = of(rows);
        return Ok(RowsCsv {
            csv: rows_csv(q, &set),
            rows: set.rows.len(),
            blocks,
            kind: kind.to_owned(),
            strips: vec![name],
            notices: notices_of(Some(id)),
        });
    }
    let notices = notices_of(None);
    let columns = template.columns.len();
    let mut csv = rows_csv(q, &of(Vec::new()));
    let mut count = 0;
    let mut names = Vec::with_capacity(strips.len());
    for (_, name, _, rows) in strips {
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
        notices,
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

#[cfg(test)]
mod tests {
    use super::*;
    use ew_derive::ew_ids::Uuid;
    use ew_reports::{Cell, RowId};

    fn strip(n: u128) -> Id<Device> {
        Id::from_uuid(Uuid::from_u128(n))
    }

    /// A data row of strip `s` with the terminal number `t`.
    fn row(s: u128, t: u32) -> Row {
        Row::data(
            RowId(vec![RowKey::Entity(Uuid::from_u128(s)), RowKey::Index(t)]),
            vec![Cell::Number(i64::from(t))],
        )
    }

    fn set(rows: Vec<Row>) -> RowSet {
        let mut set = RowSet::new(Vec::new());
        set.rows = rows;
        set
    }

    /// Cycle-40 review, part 3, finding 3: every run of a strip in a block is kept, runs concatenated; a strip a
    /// later block lists again keeps the earlier rows and is a repeat naming the later block.
    #[test]
    fn every_run_of_a_strip_is_kept_and_a_repeat_across_blocks_is_named() {
        let first = set(vec![row(1, 1), row(1, 2), row(2, 1), row(1, 3)]);
        let second = set(vec![row(2, 5), row(3, 1)]);
        let got = gather_strips(&[("910".to_owned(), &first), ("912".to_owned(), &second)]);
        assert_eq!(
            got,
            GatheredStrips {
                strips: vec![
                    (strip(1), "910".to_owned(), vec![row(1, 1), row(1, 2), row(1, 3)]),
                    (strip(2), "910".to_owned(), vec![row(2, 1)]),
                    (strip(3), "912".to_owned(), vec![row(3, 1)]),
                ],
                repeats: vec![(strip(2), "912".to_owned())],
            }
        );
    }

    /// `--strip` takes the full tag exactly, else the one printed tag; two strips printing the tag are refused with
    /// their full tags.
    #[test]
    fn a_printed_tag_two_strips_share_is_refused() {
        let tags = vec![
            ("-X1".to_owned(), "=A+L1-X1".to_owned()),
            ("-X2".to_owned(), "=A+L1-X2".to_owned()),
            ("-X1".to_owned(), "=A+L2-X1".to_owned()),
        ];
        assert_eq!(pick_strip("-X2", &tags), Ok(Some(1)));
        assert_eq!(pick_strip("=A+L2-X1", &tags), Ok(Some(2)));
        assert_eq!(pick_strip("-X9", &tags), Ok(None));
        let e = pick_strip("-X1", &tags).unwrap_err();
        assert_eq!(
            e.to_string(),
            "more than one terminal strip has the tag `-X1` (=A+L1-X1, =A+L2-X1); name it by its full tag"
        );
    }
}
