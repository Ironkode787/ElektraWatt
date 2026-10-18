// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #12, online wire numbering (doc 06 §3.2, FR-NUM-019, FR-NUM-021), v0 subset.
//!
//! The wire-numbering schemes arrive with WP-S1.11, so online wire numbering is off: a connection created
//! by this transaction stays `NumberSpec::Unassigned` and prints "?". A new connection that carries an
//! assigned counter (a split part copied from its original, a command that copied a record) is reset to
//! `Unassigned`; a frozen number is an explicit user value and stays.

use ew_schema::{Connection, Kind, NumberSpec};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser #12.
#[derive(Clone, Copy, Debug, Default)]
pub struct WireNumbering;

impl Normaliser for WireNumbering {
    fn id(&self) -> &'static str {
        "wire_numbering"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut changed = false;
        for key in tx.changed_of(Kind::Connection) {
            if !tx.is_new(&key) {
                continue;
            }
            let id = key.1.cast::<Connection>();
            let Some(c) = tx.get::<Connection>(id) else {
                continue;
            };
            if matches!(c.number, NumberSpec::Assigned(_)) {
                tx.update::<Connection>(id, |c| c.number = NumberSpec::Unassigned)?;
                changed = true;
            }
        }
        Ok(changed)
    }
}
