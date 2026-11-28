// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The columns a list prints (doc 09 §3.8): what its page shows and what Export list… and the package write of it.

use ew_mltext::MlText;
use ew_schema::{ReportColumn, ReportKind};

use crate::kind::Registry;
use crate::rowset::RowSet;

/// One printed column: its column in the row set, its width and its heading.
pub type Printed<'s> = (usize, u32, &'s MlText);

/// The columns of `set` a list prints: the block's own `own`, those the rows have, in its order, each with its width
/// and its heading or the kind's; else the columns its kind `kind` prints ([`crate::ReportKindDef::printed`]), else
/// every column of the rows. A key of an earlier version of the kind is read as the column it became
/// (`ColumnDef::former`); several former keys of one column print it once, as wide as they were together.
pub fn printed<'s>(own: &'s [ReportColumn], kind: Option<&ReportKind>, set: &'s RowSet) -> Vec<Printed<'s>> {
    if !own.is_empty() {
        return own.iter().fold(Vec::new(), |mut out, c| {
            let Some(i) = set.column(&c.key) else {
                return out;
            };
            // A heading of one's own stays where one key became one column; keys that became one take its heading.
            let merged = set.columns[i].key != c.key && set.columns[i].former.len() > 1;
            let heading = if c.heading.is_empty() || merged {
                &set.columns[i].label
            } else {
                &c.heading
            };
            match out.iter_mut().find(|(at, _, _)| *at == i) {
                Some((_, weight, _)) => *weight = weight.saturating_add(c.weight),
                None => out.push((i, c.weight, heading)),
            }
            out
        });
    }
    match kind
        .and_then(|k| Registry::builtin().get(k))
        .and_then(|k| k.printed())
    {
        Some(keys) => keys
            .iter()
            .filter_map(|key| {
                let i = set.column(key)?;
                Some((i, set.columns[i].weight, &set.columns[i].label))
            })
            .collect(),
        None => set
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.weight, &c.label))
            .collect(),
    }
}
