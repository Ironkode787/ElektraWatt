// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connection-point instantiation: `function_cps(f)` (ARCH §6.4, doc 03 §7, FR-DEV-002, FR-DEV-008).

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{ClampKey, Discipline, Function, FunctionKind, PotentialType, Side};

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
///
/// - A missing function has none.
/// - A normal function or an interruption point instantiates the CPs of its function-definition copy;
///   a missing copy gives none.
/// - A black box takes the CPs of the first part use of its device (in `Device.parts` order) whose part
///   copy has a CP pattern; without one it falls back to its function definition.
/// - `Function.cp_overrides` then replace designation, alias and description where given, copy the
///   `unused` flag and mark the CP `overridden`. An override whose index has no CP is ignored (finding it
///   is a normaliser's job).
#[derive(Debug)]
pub struct FunctionCps;

impl Query for FunctionCps {
    type Key = Id<Function>;
    type Value = Vec<Cp>;
    const FAMILY: &'static str = "function_cps";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<Cp>> {
        let Some(function) = q.functions().get(*key) else {
            return Arc::new(Vec::new());
        };
        let pattern = match function.kind {
            FunctionKind::BlackBox => pattern_cps(q, function),
            _ => None,
        };
        let mut cps = pattern.unwrap_or_else(|| fndef_cps(q, function));
        for o in &function.cp_overrides {
            let Some(cp) = cps.iter_mut().find(|c| c.idx == o.idx) else {
                continue;
            };
            if let Some(d) = &o.designation {
                cp.designation = d.as_str().to_owned();
            }
            if let Some(a) = &o.alias {
                cp.alias = Some(a.as_str().to_owned());
            }
            if let Some(d) = &o.description {
                cp.description = Some(d.clone());
            }
            cp.unused = o.unused;
            cp.overridden = true;
        }
        Arc::new(cps)
    }
}

/// The CPs of the function's definition copy.
fn fndef_cps(q: &Q, function: &Function) -> Vec<Cp> {
    q.fndefs()
        .get(function.def)
        .map(|def| {
            def.item
                .cps
                .iter()
                .map(|c| Cp {
                    idx: c.idx,
                    designation: c.designation.as_str().to_owned(),
                    alias: c.alias.as_ref().map(|a| a.as_str().to_owned()),
                    description: None,
                    unused: false,
                    discipline: c.discipline.clone(),
                    potential_role: c.potential_role.clone(),
                    source_output: c.source_output,
                    optional: c.optional,
                    level: c.level,
                    side: c.side.clone(),
                    clamp: c.clamp.clone(),
                    physical: c.physical,
                    overridden: false,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The CPs of the first part pattern of the function's device (black boxes).
fn pattern_cps(q: &Q, function: &Function) -> Option<Vec<Cp>> {
    let device = q.devices().get(function.device?)?;
    let parts = q.parts();
    device.parts.iter().find_map(|u| {
        let part = parts.get(u.part.copy)?;
        let pattern = &part.item.cp_pattern;
        (!pattern.is_empty()).then(|| {
            pattern
                .iter()
                .map(|c| Cp {
                    idx: c.idx,
                    designation: c.designation.as_str().to_owned(),
                    alias: None,
                    description: None,
                    unused: false,
                    discipline: Discipline::default(),
                    potential_role: c.potential_role.clone(),
                    source_output: false,
                    optional: false,
                    level: c.level,
                    side: c.side.clone(),
                    clamp: c.clamp.clone(),
                    physical: true,
                    overridden: false,
                })
                .collect()
        })
    })
}
