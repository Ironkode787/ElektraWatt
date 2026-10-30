// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connection-point instantiation: `function_cps(f)` (ARCH §6.4, doc 03 §7, FR-DEV-002, FR-DEV-008).
//!
//! # Limit
//!
//! Template origin is detected through the part template naming the function's FnDef copy: a function
//! whose FnDef is not named by a `function_template` entry of one of its device's parts keeps the FnDef
//! CPs. `Function.template_entry` (FT-655) only chooses among the entries of one FnDef; it does not make a
//! function template-origin on its own.

use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_mltext::MlText;
use ew_schema::{
    ClampKey, CpData, CpDef, Discipline, FnDef, Function, FunctionKind, PotentialType, Side, TemplateFn,
};

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
/// - A function is *template-origin* when a part use of its device (in `Device.parts` order) has a part copy
///   whose `function_template` has an entry naming the function's FnDef copy (the copy of the entry's `fndef`
///   reference). The entry is the one at `Function.template_entry` when a part use has one there naming the
///   FnDef copy, else the first naming it (FT-655). Its CPs start from that part's CP pattern: the entries
///   named by the entry's `cps` (CP indices, `CpData.idx`; empty means the whole pattern). When `cps` lists as
///   many pattern entries as the FnDef has CPs, they pair with the FnDef's CPs by position and each function CP
///   keeps the FnDef CP's `idx`; otherwise they are taken in `idx` order with their own `idx` and pair with the
///   FnDef CP of the same `idx`. Each takes designation, level, side and clamp from the pattern, the potential
///   role from the pattern else from its FnDef CP, and discipline, alias, source output, optional and physical
///   from its FnDef CP if there is one, else the defaults of a black-box pattern CP. A template naming no
///   pattern CP gives the CPs of the function definition.
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

/// Whether the function's CPs are the same pins of one device pattern (doc 03 §9.3): a black box, or a
/// template-origin function (its CPs come from a part's CP pattern). Duplicated printed designations of such
/// a function are one pin; elsewhere (terminals, FnDef CPs) they are separate points.
pub(crate) fn pattern_origin(q: &Q, function: &Function) -> bool {
    matches!(function.kind, FunctionKind::BlackBox) || template_cps(q, function).is_some()
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

/// The CPs of a template-origin function: the part pattern entries its template entry names, merged with the
/// FnDef CPs they stand for. `None` when no part of the device has a template entry naming the function's FnDef
/// copy, or when the entry selects no pattern CP.
///
/// The entry is the one at the function's `template_entry` index in the first part use whose template has, at that
/// index, an entry naming the function's FnDef copy (FT-655); without the member, or when the index names no such
/// entry in any part use (out of range, or another FnDef), it is the first entry naming the FnDef copy, in
/// `Device.parts` order. A stale index therefore never panics and never needs a finding: it derives as a file
/// without the member does.
///
/// How the entry's `cps` pair with the FnDef's CPs (FT-655): when `cps` is not empty, names as many pattern entries
/// as the FnDef copy has CPs and every index it lists is in the pattern, they pair **by position**: the FnDef's
/// `k`-th CP (in `idx` order) is drawn as pattern entry `cps[k]`, keeping the FnDef CP's `idx`, so that a symbol's
/// `fn_cp` and the FnDef's internal links still name it (a relay's second changeover contact, pattern `[5, 6, 7]`,
/// gets CPs 0, 1 and 2 printed "21", "22" and "24"). Otherwise (an empty `cps`, a missing FnDef copy, another
/// count, an index outside the pattern) the pattern entries keep their own `idx` and merge with the FnDef CP of the
/// same `idx`, as before FT-655; where the pattern's indices are the FnDef's, both readings agree.
fn template_cps(q: &Q, function: &Function) -> Option<Vec<Cp>> {
    let device = q.devices().get(function.device?)?;
    let parts = q.parts();
    let names_def = |t: &TemplateFn| {
        let copy: Id<FnDef> = v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid());
        copy == function.def
    };
    let by_entry = function.template_entry.and_then(|i| {
        device.parts.iter().find_map(|u| {
            let part = parts.get(u.part.copy)?;
            let template = part
                .item
                .function_template
                .get(usize::from(i))
                .filter(|t| names_def(t))?;
            Some((part, template))
        })
    });
    let (part, template) = by_entry.or_else(|| {
        device.parts.iter().find_map(|u| {
            let part = parts.get(u.part.copy)?;
            let template = part.item.function_template.iter().find(|t| names_def(t))?;
            Some((part, template))
        })
    })?;
    let fndefs = q.fndefs();
    let def = fndefs.get(function.def);
    let mut def_cps: Vec<&CpDef> = def.map(|d| d.item.cps.iter().collect()).unwrap_or_default();
    def_cps.sort_by_key(|c| c.idx);
    let in_pattern = |idx: u16| part.item.cp_pattern.iter().find(|c| c.idx == idx);
    let positional = !template.cps.is_empty()
        && template.cps.len() == def_cps.len()
        && template.cps.iter().all(|i| in_pattern(*i).is_some());
    // Each pair: the pattern entry drawn, and the FnDef CP it stands for (whose `idx` the function CP takes).
    let pairs: Vec<(&CpData, Option<&CpDef>)> = if positional {
        template
            .cps
            .iter()
            .zip(def_cps.iter())
            .filter_map(|(i, f)| in_pattern(*i).map(|c| (c, Some(*f))))
            .collect()
    } else {
        let mut pattern: Vec<&CpData> = part
            .item
            .cp_pattern
            .iter()
            .filter(|c| template.cps.is_empty() || template.cps.contains(&c.idx))
            .collect();
        pattern.sort_by_key(|c| c.idx);
        pattern
            .into_iter()
            .map(|c| (c, def_cps.iter().copied().find(|f| f.idx == c.idx)))
            .collect()
    };
    let cps: Vec<Cp> = pairs
        .into_iter()
        .map(|(c, def_cp)| Cp {
            idx: def_cp.filter(|_| positional).map_or(c.idx, |f| f.idx),
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
