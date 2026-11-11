// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! QET's tables (`graphics_table`) → report blocks (doc 14 §4.13).
//!
//! A table that holds a query is the head of a chain; a table naming it as `previous_table` continues it on
//! a later folio, and so on. The query's view gives the block's kind: `project_summary_view` → table of
//! contents, `element_nomenclature_view` → parts list, `wiring_list_view` → wire list; any other query is
//! not imported. The block lists EW's own columns and rows in EW's table style (EW-IMP-004 says so once).
//!
//! - When every folio of the chain holds nothing but its table, the chain's drawn pages give way to one
//!   page-oriented block: first folio the head's folio number, then the following folios when QET's folios
//!   are consecutive (else the sub-pages 2.1, 2.2 …), the head's frame, and the head's title (the titles
//!   `T 1`, `T 2` … of a chain become `T {n}`).
//! - Otherwise the block is embedded in the head's folio, in the head table's rectangle; continuations on
//!   other folios are reported, not drawn.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::MlText;
use ew_schema::{
    Bounds, ContinuationPolicy, EmbedSpec, FormDef, Page, Record, ReportBlock, ReportKind, ReportSource,
};
use ew_units::Pt2;

use super::cx::{Findings, Ids};
use super::model::{QetProject, QetTable};

/// What the tables become.
#[derive(Default)]
pub struct TablesOut {
    /// The report blocks.
    pub records: Vec<Record>,
    /// The diagrams (by index) a page-oriented block replaces: they get no page record.
    pub replaced: BTreeSet<usize>,
}

/// One diagram's facts the mapping needs.
pub struct DiagramFacts {
    /// The page ID.
    pub page: Id<Page>,
    /// The folio number.
    pub number: u32,
    /// The printed folio.
    pub folio: String,
    /// The frame form.
    pub form: Option<Id<FormDef>>,
}

/// The report kind of a QET query, by the view it reads.
fn kind_of(query: &str) -> Option<ReportKind> {
    let q = query.to_ascii_lowercase();
    if q.contains("project_summary_view") {
        Some(ReportKind::Toc)
    } else if q.contains("element_nomenclature_view") {
        Some(ReportKind::PartsList)
    } else if q.contains("wiring_list_view") {
        Some(ReportKind::WireList)
    } else {
        None
    }
}

/// The block title of a chain's diagram titles: `T {n}` when they read `T 1`, `T 2` …, else the first.
fn title(titles: &[&str]) -> MlText {
    let first = titles.first().map_or("", |t| t.trim());
    let stem = first.trim_end_matches(|c: char| c.is_ascii_digit());
    let numbered = titles.len() > 1
        && stem.len() < first.len()
        && titles
            .iter()
            .enumerate()
            .all(|(i, t)| t.trim().strip_prefix(stem) == Some((i + 1).to_string().as_str()));
    let text = if numbered {
        format!("{stem}{{n}}")
    } else {
        first.to_owned()
    };
    if text.is_empty() {
        MlText::default()
    } else {
        MlText::neutral(&text)
    }
}

/// The page rectangle of a table (`x y width height` px); `None` when a value is missing or out of range.
fn rect(t: &QetTable) -> Option<Bounds> {
    let [x, y, w, h] = t.rect.map(|v| v.and_then(|v| v.px_to_nm().ok()));
    let (x, y, w, h) = (x?, y?, w?, h?);
    let min = Pt2::new(x, y);
    Bounds::new(min, Pt2::new(x.checked_add(w)?, y.checked_add(h)?)).ok()
}

/// Maps the tables of `project`; `facts` by diagram index.
pub fn map(project: &QetProject, facts: &[DiagramFacts], ids: &Ids, findings: &mut Findings) -> TablesOut {
    let mut out = TablesOut::default();
    let tables: Vec<(usize, &QetTable)> = project
        .diagrams
        .iter()
        .flat_map(|d| d.tables.iter().map(move |t| (d.index, t)))
        .collect();
    let mut next: BTreeMap<&str, usize> = BTreeMap::new();
    for (k, (_, t)) in tables.iter().enumerate() {
        if let Some(p) = t.previous.as_deref() {
            next.entry(p).or_insert(k);
        }
    }
    let mut placed: BTreeSet<usize> = BTreeSet::new();
    let mut style_reported = false;
    for (k, &(di, head)) in tables.iter().enumerate() {
        let Some(query) = head.query.as_deref() else {
            continue;
        };
        let Some(f) = facts.get(di) else { continue };
        // The chain: the head, then each table that names the last one as its previous.
        let mut chain = vec![k];
        placed.insert(k);
        while let Some(&n) = tables[*chain.last().unwrap_or(&k)]
            .1
            .uuid
            .as_deref()
            .and_then(|u| next.get(u))
        {
            if !placed.insert(n) {
                break;
            }
            chain.push(n);
        }
        let loc = findings.loc(Some(&f.folio), None, Some(head.line));
        let Some(kind) = kind_of(query) else {
            findings.push(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                loc,
                "a table with its own query is not imported",
            );
            continue;
        };
        let Some(form) = f.form else { continue };
        if !style_reported {
            style_reported = true;
            findings.file_level(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                "QET's tables are imported as report blocks: they list ElektraWatt's columns in ElektraWatt's \
                 table style",
            );
        }
        let diagrams: Vec<usize> = chain.iter().map(|&c| tables[c].0).collect();
        let distinct: BTreeSet<usize> = diagrams.iter().copied().collect();
        let table_only = distinct.len() == diagrams.len()
            && diagrams.iter().all(|&i| {
                let d = &project.diagrams[i];
                d.elements.is_empty()
                    && d.conductors.is_empty()
                    && d.free_texts == 0
                    && d.shapes == 0
                    && d.images == 0
                    && d.tables.len() == 1
            });
        let key = format!("qet:table:{}", head.uuid.as_deref().unwrap_or(&f.folio));
        let mut block = ReportBlock {
            id: ids.id(&key),
            v: 1,
            source: ReportSource::Kind(kind),
            manual: Default::default(),
            sort: Vec::new(),
            group: None,
            form,
            section: None,
            first_folio: None,
            continuation: ContinuationPolicy::SubPage,
            title: MlText::default(),
            empty: Default::default(),
            embedded: None,
            source_ref: Some(ids.source_ref(&key)),
            extra: Default::default(),
        };
        if table_only {
            let numbers: Vec<u32> = diagrams
                .iter()
                .filter_map(|&i| facts.get(i).map(|f| f.number))
                .collect();
            let consecutive = numbers.windows(2).all(|w| w[0].checked_add(1) == Some(w[1]));
            block.first_folio = Some(f.number);
            if consecutive && numbers.len() > 1 {
                block.continuation = ContinuationPolicy::NextFolio;
            }
            let titles: Vec<&str> = diagrams
                .iter()
                .map(|&i| project.diagrams[i].title.as_str())
                .collect();
            block.title = title(&titles);
            out.replaced.extend(diagrams.iter().copied());
        } else {
            let Some(r) = rect(head) else {
                findings.push(
                    Code::MALFORMED_ITEM,
                    Severity::Warning,
                    loc,
                    "a table without a usable position and size is not imported",
                );
                continue;
            };
            block.embedded = Some(EmbedSpec {
                page: f.page,
                rect: r,
                extra: Default::default(),
            });
            for &i in diagrams.iter().skip(1) {
                let folio = facts.get(i).map_or("?", |f| f.folio.as_str());
                let loc = findings.loc(Some(folio), None, None);
                findings.push(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Info,
                    loc,
                    format!(
                        "this table continues the table on folio {}; only that one is drawn",
                        f.folio
                    ),
                );
            }
        }
        out.records.push(Record::ReportBlock(block));
    }
    for (k, &(di, t)) in tables.iter().enumerate() {
        if !placed.contains(&k) {
            let folio = facts.get(di).map_or("?", |f| f.folio.as_str());
            let loc = findings.loc(Some(folio), None, Some(t.line));
            findings.push(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                loc,
                "a table that continues no table of the project is not imported",
            );
        }
    }
    out
}
