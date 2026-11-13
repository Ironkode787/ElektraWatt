// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser N-FL-5 `tube_tags` (doc 11 §4.2, FR-FLU-011): the line tags of fluid lines.
//!
//! The tag of a fluid line is its `Connection.number`. Under the project's line-tag scheme `page_counter`
//! (`ProjectSettings.fluid.line_tags`), a fluid line of the transaction that has no counter, or a counter another
//! fluid line of its page holds (a copy, a move to another page), gets the lowest counter free on its page (in ID
//! order of the lines); a line on no page gets none. Without a scheme a new fluid line carries no counter (a copied one is reset). A frozen tag
//! is an explicit user value and stays.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{AssignedNumber, Connection, ConnectionKind, Kind, LineTagScheme, NumberSpec};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser N-FL-5.
#[derive(Clone, Copy, Debug, Default)]
pub struct TubeTags;

impl Normaliser for TubeTags {
    fn id(&self) -> &'static str {
        "tube_tags"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let scheme = tx.project().settings.fluid.line_tags == Some(LineTagScheme::PageCounter);
        let mut changed = false;
        let mut keys: Vec<Id<Connection>> = tx
            .changed_of(Kind::Connection)
            .into_iter()
            .map(|k| k.1.cast::<Connection>())
            .collect();
        keys.sort();
        for id in keys {
            let Some(c) = tx.get::<Connection>(id) else {
                continue;
            };
            if !matches!(c.kind, ConnectionKind::Fluid(_)) || matches!(c.number, NumberSpec::Frozen(_)) {
                continue;
            }
            let key = (Kind::Connection, id.cast());
            if !scheme {
                if tx.is_new(&key) && matches!(c.number, NumberSpec::Assigned(_)) {
                    tx.update::<Connection>(id, |c| c.number = NumberSpec::Unassigned)?;
                    changed = true;
                }
                continue;
            }
            let Some(page) = c.page else { continue };
            let held: BTreeSet<u32> = tx
                .connections_on_page(page)
                .into_iter()
                .filter(|o| *o != id)
                .filter_map(|o| tx.get::<Connection>(o))
                .filter(|o| matches!(o.kind, ConnectionKind::Fluid(_)))
                .filter_map(|o| match &o.number {
                    NumberSpec::Assigned(a) => Some(a.counter),
                    _ => None,
                })
                .collect();
            if matches!(&c.number, NumberSpec::Assigned(a) if !held.contains(&a.counter)) {
                continue;
            }
            let counter = (1..).find(|n| !held.contains(n)).unwrap_or(1);
            tx.update::<Connection>(id, |c| {
                c.number = NumberSpec::Assigned(AssignedNumber::new(counter))
            })?;
            changed = true;
        }
        Ok(changed)
    }
}
