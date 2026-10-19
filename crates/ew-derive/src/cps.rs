// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connection-point instantiation: `function_cps(f)` (ARCH §6.4, doc 03 §7, FR-DEV-002, FR-DEV-008).
//!
//! # Limit
//!
//! Template origin is detected only through the part template naming the function's FnDef copy: a function
//! whose FnDef is not named by a `function_template` entry of one of its device's parts keeps the FnDef
//! CPs (no other origin marker exists in this format version).

use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_mltext::MlText;
use ew_schema::{ClampKey, CpData, Discipline, Function, FunctionKind, PotentialType, Side};

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
/// - A function is *template-origin* when the first part use of its device (in `Device.parts` order) whose
///   part copy has a `function_template` entry naming the function's FnDef copy (the copy of the entry's
///   `fndef` reference) is found. Its CPs start from that part's CP pattern: the entries named by the
///   template's `cps` (CP indices, `CpData.idx`; empty means the whole pattern), in `idx` order. Each takes
///   `idx`, designation, level, side and clamp from the pattern, the potential role from the pattern else
///   from the FnDef CP with the same `idx`, and discipline, alias, source output, optional and physical
///   from the FnDef CP with the same `idx` if there is one, else the defaults of a black-box pattern CP. A
///   template naming no pattern CP gives the CPs of the function definition.
/// - Any other normal function or interruption point instantiates the CPs of its function-definition copy;
///   a missing copy gives none.
/// - A black box that is not template-origin takes the CPs of the first part use of its device (in
///   `Device.parts` order) whose part copy has a CP pattern; without one it falls back to its function
///   definition.
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
        let pattern = template_cps(q, function).or_else(|| match function.kind {
            FunctionKind::BlackBox => pattern_cps(q, function),
            _ => None,
        });
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

/// The CPs of a template-origin function: the part pattern entries its template names, merged with the
/// FnDef CPs of the same index. `None` when no part of the device has a template naming the function's
/// FnDef copy, or when the template selects no pattern CP.
fn template_cps(q: &Q, function: &Function) -> Option<Vec<Cp>> {
    let device = q.devices().get(function.device?)?;
    let parts = q.parts();
    let (part, template) = device.parts.iter().find_map(|u| {
        let part = parts.get(u.part.copy)?;
        let template = part.item.function_template.iter().find(|t| {
            let copy: Id<ew_schema::FnDef> =
                v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid());
            copy == function.def
        })?;
        Some((part, template))
    })?;
    let fndefs = q.fndefs();
    let def = fndefs.get(function.def);
    let mut pattern: Vec<&CpData> = part
        .item
        .cp_pattern
        .iter()
        .filter(|c| template.cps.is_empty() || template.cps.contains(&c.idx))
        .collect();
    pattern.sort_by_key(|c| c.idx);
    let cps: Vec<Cp> = pattern
        .into_iter()
        .map(|c| {
            let def_cp = def
                .as_ref()
                .and_then(|d| d.item.cps.iter().find(|f| f.idx == c.idx));
            Cp {
                idx: c.idx,
                designation: c.designation.as_str().to_owned(),
                alias: def_cp.and_then(|f| f.alias.as_ref().map(|a| a.as_str().to_owned())),
                description: None,
                unused: false,
                discipline: def_cp.map(|f| f.discipline.clone()).unwrap_or_default(),
                potential_role: c
                    .potential_role
                    .clone()
                    .or_else(|| def_cp.and_then(|f| f.potential_role.clone())),
                source_output: def_cp.is_some_and(|f| f.source_output),
                optional: def_cp.is_some_and(|f| f.optional),
                level: c.level,
                side: c.side.clone(),
                clamp: c.clamp.clone(),
                physical: def_cp.is_none_or(|f| f.physical),
                overridden: false,
            }
        })
        .collect();
    (!cps.is_empty()).then_some(cps)
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
