// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connection-point instantiation: `function_cps(f)` (ARCH §6.4, doc 03 §7, FR-DEV-002, FR-DEV-008).

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{ClampKey, Discipline, Function, PotentialType, Side};

use crate::Q;
use crate::query::{Policy, Query};

/// One instantiated connection point of a function.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cp {
    /// The CP index (unique within the function).
    pub idx: u16,
    /// The printed designation (duplicates allowed, REF-029).
    pub designation: String,
    /// The alias ("L1").
    pub alias: Option<String>,
    /// The description.
    pub description: Option<MlText>,
    /// Unused by design (an override).
    pub unused: bool,
    /// Discipline.
    pub discipline: Discipline,
    /// Potential role.
    pub potential_role: Option<PotentialType>,
    /// Output of a source function.
    pub source_output: bool,
    /// May stay free by design.
    pub optional: bool,
    /// Terminal level.
    pub level: Option<u8>,
    /// Terminal side.
    pub side: Option<Side>,
    /// Terminal clamp.
    pub clamp: Option<ClampKey>,
    /// A drawn, wireable point.
    pub physical: bool,
    /// Whether an override of the function changed this CP.
    pub overridden: bool,
}

/// `function_cps(f)`: the function's connection points, in `idx` order.
#[derive(Debug)]
pub struct FunctionCps;

impl Query for FunctionCps {
    type Key = Id<Function>;
    type Value = Vec<Cp>;
    const FAMILY: &'static str = "function_cps";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<Cp>> {
        let _ = (q, key);
        Arc::new(Vec::new())
    }
}
