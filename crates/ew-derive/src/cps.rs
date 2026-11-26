// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connection-point instantiation: `function_cps(f)` (ARCH §6.4, doc 03 §7, FR-DEV-002, FR-DEV-008).
//!
//! # Limit
//!
//! Template origin is detected through the part template naming the function's FnDef copy: a function
//! whose FnDef is not named by a `function_template` entry of one of its device's parts keeps the FnDef
//! CPs. `Function.template_entry` only chooses among the entries of one FnDef; it does not make a
//! function template-origin on its own.

use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_mltext::MlText;
use ew_schema::{
    ClampKey, ClampType, CpData, CpDef, Discipline, FluidPortRole, FnDef, Function, FunctionKind,
    PotentialType, Representation, Side, TemplateFn, template_entry_of,
};
use ew_units::Dec3;

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
    /// The role of a fluid port, from its FnDef CP.
    pub port_role: Option<FluidPortRole>,
    /// Whether an override of the function changed this CP.
    pub overridden: bool,
    /// The clamp data of the part pattern entry the CP is drawn as (FR-PRT-008); `None` for a CP no part pattern
    /// gives, and for one whose entry names none.
    pub clamp_data: Option<ClampData>,
}

/// What a part says of a CP's clamp (FR-PRT-008, `CpData`): what the end treatment and EW-POT-010 read (doc 13 §4.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClampData {
    /// The kind of clamp.
    pub kind: Option<ClampType>,
    /// The smallest and the largest cross-section it takes, in mm².
    pub csa: (Option<Dec3>, Option<Dec3>),
    /// The most conductors it takes.
    pub max_conductors: Option<u8>,
    /// Whether it takes a twin ferrule.
    pub twin_ferrule: bool,
}

impl ClampData {
    /// The clamp data of a pattern entry; `None` where it names none.
    fn of(c: &CpData) -> Option<ClampData> {
        let named = c.clamp_type.is_some()
            || c.csa_min.is_some()
            || c.csa_max.is_some()
            || c.max_conductors.is_some()
            || c.twin_ferrule;
        named.then(|| ClampData {
            kind: c.clamp_type.clone(),
            csa: (c.csa_min, c.csa_max),
            max_conductors: c.max_conductors,
            twin_ferrule: c.twin_ferrule,
        })
    }
}

/// `function_cps(f)`: the function's connection points, in `idx` order.
///
/// - A missing function has none.
/// - A function whose `bus_port` names an interface that a part of its device declares with a port count `n`
///   (`PartBusPort.ports`, doc 10 §2.7 "One interface, many ports") has the CPs `P1` … `Pn`, indices 0 … n − 1, each
///   as its FnDef's CP of that index or, beyond the FnDef's, as its first CP (bus discipline): the sockets of the
///   interface, which carries one address.
/// - A function is *template-origin* when a part use of its device (in `Device.parts` order) has a part copy
///   whose `function_template` has an entry naming the function's FnDef copy (the copy of the entry's `fndef`
///   reference). The entry is the one at `Function.template_entry` when a part use has one there naming the
///   FnDef copy, else the first naming it. Its CPs start from that part's CP pattern: the entries
///   named by the entry's `cps` (CP indices, `CpData.idx`; empty means the whole pattern). When `cps` lists as
///   many pattern entries as the FnDef has CPs, they pair with the FnDef's CPs by position and each function CP
///   keeps the FnDef CP's `idx`; otherwise they are taken in `idx` order with their own `idx` and pair with the
///   FnDef CP of the same `idx`. Each takes its designation from the pattern, level, side, clamp and the potential
///   role from the pattern else from its FnDef CP, and discipline, alias, source output, optional, physical and
///   port role from its FnDef CP if there is one, else the defaults of a black-box pattern CP. A template naming no
///   pattern CP gives the CPs of the function definition.
/// - Any other normal function or interruption point instantiates the CPs of its function-definition copy;
///   a missing copy gives none.
/// - A black box that is not template-origin takes the CPs of the first part use of its device (in
///   `Device.parts` order) whose part copy has a CP pattern; without one it falls back to its function
///   definition.
/// - A black box that takes no CPs from a part (a box drawn without one) adds the named pins of its boxes
///   (`BoxCp.name`, in representation ID order, the first of one index): a CP of that index printed with that name,
///   with the defaults of a black-box pattern CP. An index its FnDef has already is kept as the FnDef's.
/// - A function of a terminal whose CPs carry one level gains a copy of them for every other level its terminal lists
///   (`Terminal::level_copies`, doc 08 §6.3): the second level of a two-level block drawn from a one-level FnDef or
///   part.
/// - `Function.cp_overrides` then replace designation, alias and description where given, copy the
///   `unused` flag and mark the CP `overridden`. An override whose index has no CP is ignored (finding it
///   is a normaliser's job).
/// - Before the overrides, a CP with no potential role from its pattern or FnDef takes `L+` or `M` from the first
///   supply group of its device's PLC part that names its designation (FR-PLC-024).
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
        let pattern = port_cps(q, function)
            .or_else(|| template_cps(q, function))
            .or_else(|| match function.kind {
                FunctionKind::BlackBox => pattern_cps(q, function),
                _ => None,
            });
        let own = pattern.is_none() && matches!(function.kind, FunctionKind::BlackBox);
        let mut cps = pattern.unwrap_or_else(|| fndef_cps(q, function));
        if own {
            box_pin_cps(q, *key, &mut cps);
        }
        level_cps(q, function, &mut cps);
        supply_role_fallback(q, function, &mut cps);
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

/// The CPs the listed levels of the function's terminal add (`Terminal::level_copies`): each a copy of the CP it
/// stands for, with its own index and level.
fn level_cps(q: &Q, function: &Function, cps: &mut Vec<Cp>) {
    let devices = q.devices();
    let Some(terminal) = function
        .device
        .and_then(|d| devices.get(d))
        .and_then(|d| d.aspects.terminal.as_ref())
    else {
        return;
    };
    let have: Vec<(u16, Option<u8>)> = cps.iter().map(|c| (c.idx, c.level)).collect();
    for (idx, of, level) in terminal.level_copies(&have) {
        if let Some(base) = cps.iter().find(|c| c.idx == of).cloned() {
            cps.push(Cp {
                idx,
                level: Some(level),
                ..base
            });
        }
    }
}

/// The potential roles that the supply groups of the device's PLC part give its pins (FR-PLC-024):
/// pin designation → `L+` or `M`, in group order. The part is found by this rule: the part use named by
/// `plc_module.part_use`, else the first part use whose copy has a PLC section with at least one group.
fn supply_roles(q: &Q, function: &Function) -> Vec<(String, PotentialType)> {
    let Some(device) = function.device.and_then(|d| q.devices().get(d)) else {
        return Vec::new();
    };
    let parts = q.parts();
    let groups = |i: usize| {
        let copy = parts.get(device.parts.get(i)?.part.copy)?;
        let plc = copy.item.sections.plc.as_ref()?;
        (!plc.supply_groups.is_empty()).then(|| plc.supply_groups.clone())
    };
    let named = device.aspects.plc_module.as_ref().and_then(|m| m.part_use);
    let found = match named {
        Some(i) => groups(usize::from(i)),
        None => (0..device.parts.len()).find_map(groups),
    };
    found
        .unwrap_or_default()
        .into_iter()
        .flat_map(|g| {
            [
                (g.cps.l_plus.as_str().to_owned(), PotentialType::LPlus),
                (g.cps.m.as_str().to_owned(), PotentialType::M),
            ]
        })
        .collect()
}

/// The supply-group fallback of a CP's potential role: a CP
/// takes its role from its authored part-pattern CP, else from its definition CP (the instantiation above), else, when
/// it has neither, from the first supply group of the device's PLC part that names its designation as `l_plus` (`L+`)
/// or `m` (`M`). An authored role always wins. Applied to the CPs of every instantiation path (FnDef, template, black
/// box) before the overrides, so the group matches the part's pin designation, not an overridden one. It reads the
/// device row and the part copies the instantiation reads already: no new memo input.
fn supply_role_fallback(q: &Q, function: &Function, cps: &mut [Cp]) {
    if cps.iter().all(|c| c.potential_role.is_some()) {
        return;
    }
    let roles = supply_roles(q, function);
    if roles.is_empty() {
        return;
    }
    for cp in cps.iter_mut().filter(|c| c.potential_role.is_none()) {
        cp.potential_role = roles
            .iter()
            .find(|(pin, _)| *pin == cp.designation)
            .map(|(_, role)| role.clone());
    }
}

/// Whether the function's CPs are the same pins of one device pattern (doc 03 §9.3): a black box, or a
/// template-origin function (its CPs come from a part's CP pattern). Duplicated printed designations of such
/// a function are one pin; elsewhere (terminals, FnDef CPs) they are separate points.
pub(crate) fn pattern_origin(q: &Q, function: &Function) -> bool {
    matches!(function.kind, FunctionKind::BlackBox) || template_cps(q, function).is_some()
}

/// The sockets of a multi-port interface (doc 10 §2.7): `P1` … `Pn` when the function's port names an interface that a
/// part of its device declares with `ports` = n ≥ 1, each the FnDef CP of its index or, beyond those, a copy of the
/// FnDef's first CP; `None` otherwise, and for a FnDef without CPs.
fn port_cps(q: &Q, function: &Function) -> Option<Vec<Cp>> {
    let port = function.bus_port.as_ref()?;
    let device = q.devices().get(function.device?)?;
    let parts = q.parts();
    let n = device.parts.iter().find_map(|u| {
        let bus = parts.get(u.part.copy)?.item.sections.bus.as_ref()?;
        bus.ports
            .iter()
            .find(|p| p.interface == port.interface && p.port_label == port.port_label)?
            .ports
    })?;
    let own = fndef_cps(q, function);
    let first = own.first()?.clone();
    let cps: Vec<Cp> = (0..n)
        .map(|i| {
            let mut cp = own.iter().find(|c| c.idx == i).cloned().unwrap_or_else(|| Cp {
                optional: true,
                ..first.clone()
            });
            cp.idx = i;
            cp.designation = format!("P{}", u32::from(i) + 1);
            cp
        })
        .collect();
    (!cps.is_empty()).then_some(cps)
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
                    port_role: c.port_role.clone(),
                    overridden: false,
                    clamp_data: None,
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
/// index, an entry naming the function's FnDef copy; without the member, or when the index names no such
/// entry in any part use (out of range, or another FnDef), it is the first entry naming the FnDef copy, in
/// `Device.parts` order. A stale index therefore never panics and never needs a finding: it derives as a file
/// without the member does.
///
/// How the entry's `cps` pair with the FnDef's CPs: when `cps` is not empty, names as many pattern entries
/// as the FnDef copy has CPs and every index it lists is in the pattern, they pair **by position**: the FnDef's
/// `k`-th CP (in `idx` order) is drawn as pattern entry `cps[k]`, keeping the FnDef CP's `idx`, so that a symbol's
/// `fn_cp` and the FnDef's internal links still name it (a relay's second changeover contact, pattern `[5, 6, 7]`,
/// gets CPs 0, 1 and 2 printed "21", "22" and "24"). Otherwise (an empty `cps`, a missing FnDef copy, another
/// count, an index outside the pattern) the pattern entries keep their own `idx` and merge with the FnDef CP of the
/// same `idx`, as before; where the pattern's indices are the FnDef's, both readings agree. The entry choice
/// and the pairing are `ew_schema::template_entry_of` and `TemplateFn::function_cps`, the one rule the importer,
/// `library.update` and the place catalogue also read.
fn template_cps(q: &Q, function: &Function) -> Option<Vec<Cp>> {
    let device = q.devices().get(function.device?)?;
    let parts = q.parts();
    let names_def = |t: &TemplateFn| {
        let copy: Id<FnDef> = v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid());
        copy == function.def
    };
    let uses = device
        .parts
        .iter()
        .filter_map(|u| parts.get(u.part.copy).map(|p| ((), &p.item)));
    let ((), part, n) = template_entry_of(uses, function.template_entry, names_def)?;
    let template = &part.function_template[n];
    let fndefs = q.fndefs();
    let def = fndefs.get(function.def);
    let def_cps: Vec<&CpDef> = def.map(|d| d.item.cps.iter().collect()).unwrap_or_default();
    let def_idx: Vec<u16> = def_cps.iter().map(|c| c.idx).collect();
    let pattern_idx: Vec<u16> = part.cp_pattern.iter().map(|c| c.idx).collect();
    // Each pair: the pattern entry drawn, the function CP index, and the FnDef CP it stands for.
    let pairs: Vec<(&CpData, u16, Option<&CpDef>)> = template
        .function_cps(&pattern_idx, &def_idx)
        .into_iter()
        .filter_map(|t| {
            let c = part.cp_pattern.iter().find(|c| c.idx == t.pattern)?;
            let f = t.def.and_then(|d| def_cps.iter().copied().find(|f| f.idx == d));
            Some((c, t.idx, f))
        })
        .collect();
    let cps: Vec<Cp> = pairs
        .into_iter()
        .map(|(c, idx, def_cp)| Cp {
            idx,
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
            // A pattern that names only designations (a generic terminal part) keeps its FnDef's level, side
            // and clamp, so the terminal diagram can still tell clamp a from clamp b.
            level: c.level.or_else(|| def_cp.and_then(|f| f.level)),
            side: c.side.clone().or_else(|| def_cp.and_then(|f| f.side.clone())),
            clamp: c.clamp.clone().or_else(|| def_cp.and_then(|f| f.clamp.clone())),
            physical: def_cp.is_none_or(|f| f.physical),
            port_role: def_cp.and_then(|f| f.port_role.clone()),
            overridden: false,
            clamp_data: ClampData::of(c),
        })
        .collect();
    (!cps.is_empty()).then_some(cps)
}

/// The named pins of the boxes that draw the black box `f` (see [`FunctionCps`]), added to `cps` in `idx` order.
fn box_pin_cps(q: &Q, f: Id<Function>, cps: &mut Vec<Cp>) {
    let reps = q.reps();
    let mut ids: Vec<Id<Representation>> = q.idx().reps_of_function(f).collect();
    ids.sort();
    for r in ids {
        let Some(b) = reps.get(r).and_then(|r| r.box_.as_ref()) else {
            continue;
        };
        for pin in &b.cps {
            let Some(name) = &pin.name else { continue };
            if cps.iter().any(|c| c.idx == pin.idx) {
                continue;
            }
            cps.push(Cp {
                idx: pin.idx,
                designation: name.as_str().to_owned(),
                alias: None,
                description: None,
                unused: false,
                discipline: Discipline::default(),
                potential_role: None,
                source_output: false,
                optional: false,
                level: None,
                side: None,
                clamp: None,
                physical: true,
                port_role: None,
                overridden: false,
                clamp_data: None,
            });
        }
    }
    cps.sort_by_key(|c| c.idx);
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
                    port_role: None,
                    overridden: false,
                    clamp_data: ClampData::of(c),
                })
                .collect()
        })
    })
}
