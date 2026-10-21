// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `numbering_report`: frozen tags and frozen wire numbers with their rule-derived values (FR-REP-022,
//! FR-NUM-022, doc 09 §2.4).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::NaturalKey;
use ew_derive::ew_schema::{Connection, Device, NumberSpec, ReportBlock};
use ew_derive::{Q, WireNumbers, rule_product};

use super::conn::{tag_key, text_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowSet};

/// The numbering report.
///
/// One row per **frozen tag** (`TagSpec.frozen`, doc 03 §8.5) in natural order of the device's full tag,
/// then one row per **frozen wire number** (`NumberSpec::Frozen` on a carrier connection, doc 08 §5.3) in
/// connection ID order. `item` is the device tag (or empty for a wire), `kind` `tag` or `wire number`,
/// `frozen` the frozen text, `rule` the value the active scheme derives when the frozen value is left out
/// (`ew_derive::rule_product` for a tag, "?" where the scheme cannot derive one; the `wire_numbers` label for
/// a wire, empty when the wire is not numbered), and `status` `deviates` or `matches` (EW-NUM-001 reports
/// the tags that deviate, EW-NUM-002 the wire numbers).
#[derive(Clone, Copy, Debug)]
pub struct NumberingReport;

impl ReportKindDef for NumberingReport {
    fn name(&self) -> &'static str {
        "numbering_report"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("item", CellType::Tag, "Item").weighted(15),
            ColumnDef::new("kind", CellType::Text, "Kind").weighted(10),
            ColumnDef::new("frozen", CellType::Ident, "Frozen").weighted(12),
            ColumnDef::new("rule", CellType::Ident, "Rule value").weighted(12),
            ColumnDef::new("status", CellType::Text, "Status").weighted(10),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let mut tags: Vec<(NaturalKey, Id<Device>, String)> = q
            .devices()
            .iter()
            .filter_map(|(id, d)| {
                d.tag
                    .frozen
                    .as_ref()
                    .map(|f| (tag_key(q, id), id, f.text.as_str().to_owned()))
            })
            .collect();
        tags.sort();
        for (_, id, frozen) in tags {
            let rule = rule_product(q, id).map(|(p, _)| p).unwrap_or_default();
            set.rows
                .push(row(RowId::entity(id), Cell::Tag(id), "tag", &frozen, &rule));
        }
        let numbers = q.get::<WireNumbers>(());
        let mut wires: Vec<(Id<Connection>, String)> = q
            .connections()
            .iter()
            .filter_map(|(id, c)| match &c.number {
                NumberSpec::Frozen(t) => Some((id, t.as_str().to_owned())),
                _ => None,
            })
            .collect();
        wires.sort();
        for (id, frozen) in wires {
            let rule = numbers.number(id).map(|n| n.label.clone()).unwrap_or_default();
            set.rows
                .push(row(RowId::entity(id), Cell::Empty, "wire number", &frozen, &rule));
        }
        set
    }
}

fn row(id: RowId, item: Cell, kind: &str, frozen: &str, rule: &str) -> Row {
    let status = if frozen == rule { "matches" } else { "deviates" };
    Row::data(
        id,
        vec![
            item,
            text_cell(kind),
            Cell::Ident(frozen.to_owned()),
            if rule.is_empty() {
                Cell::Empty
            } else {
                Cell::Ident(rule.to_owned())
            },
            text_cell(status),
        ],
    )
}
