// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #2, CP instantiation (doc 06 §3.2): a recorder only.
//!
//! Connection points are derived from the function's FnDef or the part's pin pattern (FR-DEV-002, doc 07
//! §4.2) and are never stored; only `Function.cp_overrides` persist, and a command writes those itself.
//! The normaliser therefore keeps its place in the registry order and changes nothing. When derived CP
//! data arrives (WP-S1.05 to WP-S1.08), validating overrides against the instantiated CPs plugs in here.

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser #2.
#[derive(Clone, Copy, Debug, Default)]
pub struct CpInstantiation;

impl Normaliser for CpInstantiation {
    fn id(&self) -> &'static str {
        "cp_instantiation"
    }

    fn run(&self, _tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        Ok(false)
    }
}
