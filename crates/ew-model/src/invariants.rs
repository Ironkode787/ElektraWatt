// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The structural invariants of ADR-0009 decision 2, evaluated on the candidate snapshot at the end of a
//! batch, limited to the touched keys and their index neighbours (decision 1), or on every record for the
//! integrity check (decision 5). The catalogue with its tests is `ew-model/invariants.md`.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{
    Connection, Device, FnRole, Function, FunctionKind, Junction, Kind, Representation, StructureNode,
};

use crate::entity::{AnyKey, AnyRow, get_row, table_keys, typed};
use crate::error::{KeyFmt, ModelError};
use crate::index::EndpointKey;
use crate::links::{LINKS, Policy, links_from, links_to};
use crate::model::Model;

/// The class of a catalogue rule. `ew-model` holds the structural class only; prevent rules live in rule
/// packs (ADR-0019) and reach `apply` through [`crate::PreventHook`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleClass {
    /// Always on; a violation rejects the transaction.
    Structural,
}

/// One structural invariant of the catalogue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rule {
    /// The stable ID.
    pub id: &'static str,
    /// The class.
    pub class: RuleClass,
    /// The statement.
    pub statement: &'static str,
}

/// EW-INV-001: references resolve.
pub const REFS_RESOLVE: &str = "EW-INV-001";
/// EW-INV-002: one main function per device.
pub const ONE_MAIN_FUNCTION: &str = "EW-INV-002";
/// EW-INV-003: `Function.device` is `None` if and only if the kind is `Ip`.
pub const DEVICE_IFF_NOT_IP: &str = "EW-INV-003";
/// CI-01: connection endpoints exist and differ.
pub const ENDPOINTS_DIFFER: &str = "CI-01";
/// EW-INV-004: a representation's functions belong to one device.
pub const REP_ONE_DEVICE: &str = "EW-INV-004";
/// EW-INV-005: each symbol CP maps to at most one function CP.
pub const PIN_MAP_UNIQUE: &str = "EW-INV-005";
/// CI-04: a junction's sequence lists exactly the connections ending at it.
pub const JUNCTION_SEQUENCE: &str = "CI-04";
/// EW-INV-006: a conductor's cable has the cable aspect.
pub const CONDUCTOR_CABLE: &str = "EW-INV-006";
/// EW-INV-007: a terminal's strip has the strip aspect.
pub const TERMINAL_STRIP: &str = "EW-INV-007";
/// EW-INV-008: structure parent chains are acyclic.
pub const STRUCTURE_ACYCLIC: &str = "EW-INV-008";
/// EW-INV-009: device parent chains are acyclic (assembly graph).
pub const DEVICE_ACYCLIC: &str = "EW-INV-009";
/// EW-INV-010: the terminal, strip and cable aspects exclude each other.
pub const EXCLUSIVE_ASPECTS: &str = "EW-INV-010";
/// EW-INV-011: a structure node's parent has the same aspect.
pub const STRUCTURE_SAME_ASPECT: &str = "EW-INV-011";
/// EW-INV-012: a `Report` page is never stored.
pub const NO_STORED_REPORT_PAGE: &str = "EW-INV-012";
/// EW-INV-013: an interruption-point function has exactly one representation.
pub const IP_ONE_REPRESENTATION: &str = "EW-INV-013";
/// EW-INV-014: a page-oriented report block has a first folio.
pub const REPORT_BLOCK_FOLIO: &str = "EW-INV-014";

/// The structural catalogue in evaluation order.
pub static RULES: &[Rule] = &[
    Rule {
        id: REFS_RESOLVE,
        class: RuleClass::Structural,
        statement: "Every link field of a record names an existing record of its target kind, and a removed record \
                    has no referrer left; a referrer through a forbid link makes the removal Referenced",
    },
    Rule {
        id: ONE_MAIN_FUNCTION,
        class: RuleClass::Structural,
        statement: "A device that has functions has exactly one of role Main (a device without functions is allowed \
                    in v0)",
    },
    Rule {
        id: DEVICE_IFF_NOT_IP,
        class: RuleClass::Structural,
        statement: "Function.device is None if and only if the function's kind is Ip",
    },
    Rule {
        id: ENDPOINTS_DIFFER,
        class: RuleClass::Structural,
        statement: "Both endpoints of a connection exist (through EW-INV-001) and differ",
    },
    Rule {
        id: REP_ONE_DEVICE,
        class: RuleClass::Structural,
        statement: "The functions of a representation exist (through EW-INV-001) and belong to one device; \
                    interruption-point functions are excepted",
    },
    Rule {
        id: PIN_MAP_UNIQUE,
        class: RuleClass::Structural,
        statement: "Each symbol CP appears at most once in a representation's pin map",
    },
    Rule {
        id: JUNCTION_SEQUENCE,
        class: RuleClass::Structural,
        statement: "Every connection ending at a junction is in its sequence exactly once, and every sequence entry \
                    is a connection ending at it",
    },
    Rule {
        id: CONDUCTOR_CABLE,
        class: RuleClass::Structural,
        statement: "The cable of a conductor is a device with the cable aspect",
    },
    Rule {
        id: TERMINAL_STRIP,
        class: RuleClass::Structural,
        statement: "The strip of a terminal is a device with the strip aspect",
    },
    Rule {
        id: STRUCTURE_ACYCLIC,
        class: RuleClass::Structural,
        statement: "Structure node parent chains are acyclic",
    },
    Rule {
        id: DEVICE_ACYCLIC,
        class: RuleClass::Structural,
        statement: "Device parent chains are acyclic (the assembly graph)",
    },
    Rule {
        id: EXCLUSIVE_ASPECTS,
        class: RuleClass::Structural,
        statement: "A device carries at most one of the terminal, strip and cable aspects (a terminal is neither a                     strip nor a cable; a strip is neither a terminal nor a cable)",
    },
    Rule {
        id: STRUCTURE_SAME_ASPECT,
        class: RuleClass::Structural,
        statement: "The parent of a structure node has the same aspect",
    },
    Rule {
        id: NO_STORED_REPORT_PAGE,
        class: RuleClass::Structural,
        statement: "A page of kind Report is never stored; report pages are generated from report blocks",
    },
    Rule {
        id: IP_ONE_REPRESENTATION,
        class: RuleClass::Structural,
        statement: "A function of kind Ip has exactly one representation, which owns it: the function lives in the                     page of that representation and is removed with it",
    },
    Rule {
        id: REPORT_BLOCK_FOLIO,
        class: RuleClass::Structural,
        statement: "A page-oriented report block (one without embedded) has a first_folio",
    },
];

/// The keys to evaluate and the rows they had before the batch.
#[derive(Default)]
pub(crate) struct Scope {
    /// Every key the batch put, or every key of the model for the integrity check.
    pub(crate) keys: BTreeSet<AnyKey>,
    /// The row of each touched key before the batch (`None`: it did not exist).
    pub(crate) before: BTreeMap<AnyKey, Option<AnyRow>>,
}

impl Scope {
    /// Records a put; the first `before` of a key wins.
    pub(crate) fn touch(&mut self, key: AnyKey, before: Option<AnyRow>) {
        self.keys.insert(key);
        self.before.entry(key).or_insert(before);
    }

    /// Every key of the model.
    pub(crate) fn full(model: &Model) -> Scope {
        let mut keys: BTreeSet<AnyKey> = table_keys(model).into_iter().collect();
        keys.insert((Kind::Project, model.project.value.id.cast()));
        Scope {
            keys,
            before: BTreeMap::new(),
        }
    }

    fn of_kind(&self, kind: Kind) -> impl Iterator<Item = &AnyKey> + '_ {
        self.keys
            .range((kind, Id::from_uuid(ew_ids::Uuid::nil()))..)
            .take_while(move |k| k.0 == kind)
    }

    fn before_of<T: crate::Entity>(&self, key: &AnyKey) -> Option<&std::sync::Arc<T>> {
        self.before
            .get(key)
            .and_then(Option::as_ref)
            .and_then(|r| r.get::<T>())
    }
}

fn violation(id: &'static str, message: String, keys: Vec<AnyKey>) -> ModelError {
    ModelError::Invariant { id, message, keys }
}

fn key_of<T>(kind: Kind, id: Id<T>) -> AnyKey {
    (kind, id.cast())
}

/// Evaluates the catalogue in [`RULES`] order; the first violation is the error.
pub(crate) fn check(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    refs_resolve(m, scope)?;
    one_main_function(m, scope)?;
    device_iff_not_ip(m, scope)?;
    endpoints_differ(m, scope)?;
    rep_one_device(m, scope)?;
    pin_map_unique(m, scope)?;
    junction_sequence(m, scope)?;
    conductor_cable(m, scope)?;
    terminal_strip(m, scope)?;
    structure_acyclic(m, scope)?;
    device_acyclic(m, scope)?;
    exclusive_aspects(m, scope)?;
    structure_same_aspect(m, scope)?;
    no_stored_report_page(m, scope)?;
    ip_one_representation(m, scope)?;
    report_block_folio(m, scope)
}

/// EW-INV-001 with the forbid policy.
fn refs_resolve(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in &scope.keys {
        match get_row(m, key) {
            Some(row) => {
                for (_, link) in links_from(key.0) {
                    for t in link.targets_of(&row) {
                        let target = (link.to, t.id());
                        if get_row(m, &target).is_none() && !link.lib_ref {
                            return Err(violation(
                                REFS_RESOLVE,
                                format!(
                                    "{} of {} names missing {}",
                                    link.path(),
                                    KeyFmt(key),
                                    KeyFmt(&target)
                                ),
                                vec![*key, target],
                            ));
                        }
                    }
                }
            }
            None => {
                for (lid, link) in links_to(key.0) {
                    if let Some(src) = m.referrers_via(lid, key).first() {
                        let referrer = (link.from, *src);
                        return Err(if link.policy == Policy::Forbid {
                            ModelError::Referenced {
                                link: LINKS[lid].path(),
                                target: *key,
                                referrer,
                            }
                        } else {
                            violation(
                                REFS_RESOLVE,
                                format!(
                                    "{} was removed but {} still names it through {} (policy {:?}: include the delete \
                                     plan)",
                                    KeyFmt(key),
                                    KeyFmt(&referrer),
                                    link.path(),
                                    link.policy
                                ),
                                vec![referrer, *key],
                            )
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// EW-INV-002: touched devices and the old and new devices of touched functions.
fn one_main_function(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    let mut devices: BTreeSet<Id<Device>> = scope.of_kind(Kind::Device).map(typed).collect();
    for key in scope.of_kind(Kind::Function) {
        let id: Id<Function> = typed(key);
        devices.extend(scope.before_of::<Function>(key).and_then(|f| f.device));
        devices.extend(m.functions.get(&id).and_then(|f| f.device));
    }
    for d in devices {
        if !m.devices.contains(&d) {
            continue;
        }
        let fns: Vec<Id<Function>> = m.idx.functions_by_device.get(&d).copied().collect();
        if fns.is_empty() {
            continue;
        }
        let mains = fns
            .iter()
            .filter(|f| m.functions.get(f).is_some_and(|f| f.role == FnRole::Main))
            .count();
        if mains != 1 {
            let mut keys = vec![key_of(Kind::Device, d)];
            keys.extend(fns.iter().map(|f| key_of(Kind::Function, *f)));
            return Err(violation(
                ONE_MAIN_FUNCTION,
                format!(
                    "device {} has {mains} main functions among {} (exactly one required)",
                    d,
                    fns.len()
                ),
                keys,
            ));
        }
    }
    Ok(())
}

/// EW-INV-003: touched functions.
fn device_iff_not_ip(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::Function) {
        if let Some(f) = m.functions.get(&typed(key)) {
            let ip = matches!(f.kind, FunctionKind::Ip(_));
            if f.device.is_none() != ip {
                let message = if ip {
                    format!("interruption-point function {} has a device", f.id)
                } else {
                    format!("function {} has no device but is not an interruption point", f.id)
                };
                return Err(violation(DEVICE_IFF_NOT_IP, message, vec![*key]));
            }
        }
    }
    Ok(())
}

/// CI-01: touched connections (existence is EW-INV-001's).
fn endpoints_differ(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::Connection) {
        if let Some(c) = m.connections.get(&typed::<Connection>(key)) {
            if c.a == c.b {
                return Err(violation(
                    ENDPOINTS_DIFFER,
                    format!("connection {} starts and ends at the same endpoint", c.id),
                    vec![*key],
                ));
            }
        }
    }
    Ok(())
}

/// EW-INV-004: touched representations and the representations of touched functions.
fn rep_one_device(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    let mut reps: BTreeSet<Id<Representation>> = scope.of_kind(Kind::Representation).map(typed).collect();
    for key in scope.of_kind(Kind::Function) {
        reps.extend(m.idx.reps_by_function.get(&typed(key)).copied());
    }
    for r in reps {
        let Some(rep) = m.reps.get(&r) else { continue };
        let devices: BTreeSet<Id<Device>> = rep
            .functions
            .iter()
            .filter_map(|f| m.functions.get(f))
            .filter(|f| !matches!(f.kind, FunctionKind::Ip(_)))
            .filter_map(|f| f.device)
            .collect();
        if devices.len() > 1 {
            let mut keys = vec![key_of(Kind::Representation, r)];
            keys.extend(devices.iter().map(|d| key_of(Kind::Device, *d)));
            return Err(violation(
                REP_ONE_DEVICE,
                format!("representation {r} draws functions of {} devices", devices.len()),
                keys,
            ));
        }
    }
    Ok(())
}

/// EW-INV-005: touched representations.
fn pin_map_unique(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::Representation) {
        if let Some(rep) = m.reps.get(&typed(key)) {
            let mut seen = BTreeSet::new();
            if let Some(dup) = rep.pin_map.iter().find(|p| !seen.insert(p.symbol_cp)) {
                return Err(violation(
                    PIN_MAP_UNIQUE,
                    format!(
                        "representation {} maps symbol CP {} more than once",
                        rep.id, dup.symbol_cp
                    ),
                    vec![*key],
                ));
            }
        }
    }
    Ok(())
}

fn junctions_of(c: &Connection, out: &mut BTreeSet<Id<Junction>>) {
    for e in [&c.a, &c.b] {
        if let ew_schema::Endpoint::Junction(j) = e {
            out.insert(*j);
        }
    }
}

/// CI-04: touched junctions and the old and new junctions of touched connections.
fn junction_sequence(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    let mut junctions: BTreeSet<Id<Junction>> = scope.of_kind(Kind::Junction).map(typed).collect();
    for key in scope.of_kind(Kind::Connection) {
        if let Some(c) = scope.before_of::<Connection>(key) {
            junctions_of(c, &mut junctions);
        }
        if let Some(c) = m.connections.get(&typed(key)) {
            junctions_of(c, &mut junctions);
        }
    }
    for j in junctions {
        let Some(junction) = m.junctions.get(&j) else {
            continue;
        };
        let ending: BTreeSet<Id<Connection>> = m
            .idx
            .connections_by_endpoint
            .get(&EndpointKey::Junction(j))
            .copied()
            .collect();
        let mut listed = BTreeSet::new();
        let duplicate = junction.sequence.iter().find(|c| !listed.insert(**c));
        if duplicate.is_some() || listed != ending {
            let mut keys = vec![key_of(Kind::Junction, j)];
            keys.extend(
                listed
                    .symmetric_difference(&ending)
                    .map(|c| key_of(Kind::Connection, *c)),
            );
            let message = match duplicate {
                Some(c) => format!("junction {j} lists connection {c} twice"),
                None => format!(
                    "junction {j} lists {} connections in its sequence but {} end at it",
                    listed.len(),
                    ending.len()
                ),
            };
            return Err(violation(JUNCTION_SEQUENCE, message, keys));
        }
    }
    Ok(())
}

/// EW-INV-006: touched conductors and the conductors of touched devices.
fn conductor_cable(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    let mut conductors: BTreeSet<Id<ew_schema::Conductor>> =
        scope.of_kind(Kind::Conductor).map(typed).collect();
    for key in scope.of_kind(Kind::Device) {
        conductors.extend(m.idx.conductors_by_cable.get(&typed(key)).copied());
    }
    for c in conductors {
        let Some(conductor) = m.conductors.get(&c) else {
            continue;
        };
        if let Some(cable) = m.devices.get(&conductor.cable) {
            if cable.aspects.cable.is_none() {
                return Err(violation(
                    CONDUCTOR_CABLE,
                    format!("the cable {} of conductor {c} has no cable aspect", cable.id),
                    vec![key_of(Kind::Conductor, c), key_of(Kind::Device, cable.id)],
                ));
            }
        }
    }
    Ok(())
}

/// EW-INV-007: touched terminals and the terminals of touched devices.
fn terminal_strip(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    let mut terminals: BTreeSet<Id<Device>> = BTreeSet::new();
    for key in scope.of_kind(Kind::Device) {
        let d: Id<Device> = typed(key);
        terminals.insert(d);
        terminals.extend(m.idx.terminals_by_strip.get(&d).copied());
    }
    for t in terminals {
        let Some(strip_id) = m
            .devices
            .get(&t)
            .and_then(|d| d.aspects.terminal.as_ref())
            .map(|a| a.strip)
        else {
            continue;
        };
        if let Some(strip) = m.devices.get(&strip_id) {
            if strip.aspects.strip.is_none() {
                return Err(violation(
                    TERMINAL_STRIP,
                    format!("the strip {strip_id} of terminal {t} has no strip aspect"),
                    vec![key_of(Kind::Device, t), key_of(Kind::Device, strip_id)],
                ));
            }
        }
    }
    Ok(())
}

/// Follows `parent` from `start`; returns the cycle if the walk revisits a node.
fn find_cycle<T>(start: Id<T>, parent: impl Fn(Id<T>) -> Option<Id<T>>) -> Option<Vec<Id<T>>> {
    let mut path = vec![start];
    let mut seen = BTreeSet::from([start]);
    let mut at = start;
    while let Some(p) = parent(at) {
        if !seen.insert(p) {
            let from = path.iter().position(|x| *x == p).unwrap_or(0);
            return Some(path.split_off(from));
        }
        path.push(p);
        at = p;
    }
    None
}

/// EW-INV-008: walks up from every touched node.
fn structure_acyclic(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::StructureNode) {
        let start: Id<StructureNode> = typed(key);
        if !m.structure.contains(&start) {
            continue;
        }
        if let Some(cycle) = find_cycle(start, |n| m.structure.get(&n).and_then(|n| n.parent)) {
            return Err(violation(
                STRUCTURE_ACYCLIC,
                format!(
                    "structure node {start} is on a parent cycle of {} nodes",
                    cycle.len()
                ),
                cycle
                    .into_iter()
                    .map(|n| key_of(Kind::StructureNode, n))
                    .collect(),
            ));
        }
    }
    Ok(())
}

/// EW-INV-009: walks up from every touched device.
fn device_acyclic(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::Device) {
        let start: Id<Device> = typed(key);
        if !m.devices.contains(&start) {
            continue;
        }
        if let Some(cycle) = find_cycle(start, |d| m.devices.get(&d).and_then(|d| d.parent)) {
            return Err(violation(
                DEVICE_ACYCLIC,
                format!("device {start} is on a parent cycle of {} devices", cycle.len()),
                cycle.into_iter().map(|d| key_of(Kind::Device, d)).collect(),
            ));
        }
    }
    Ok(())
}

/// EW-INV-010: touched devices.
fn exclusive_aspects(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::Device) {
        if let Some(d) = m.devices.get(&typed(key)) {
            let a = &d.aspects;
            let present: Vec<&str> = [
                ("terminal", a.terminal.is_some()),
                ("strip", a.strip.is_some()),
                ("cable", a.cable.is_some()),
            ]
            .into_iter()
            .filter_map(|(name, on)| on.then_some(name))
            .collect();
            if present.len() > 1 {
                return Err(violation(
                    EXCLUSIVE_ASPECTS,
                    format!(
                        "device {} carries the {} aspects, which exclude each other",
                        d.id,
                        present.join(" and ")
                    ),
                    vec![*key],
                ));
            }
        }
    }
    Ok(())
}

/// EW-INV-011: touched nodes and the children of touched nodes.
fn structure_same_aspect(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    let mut nodes: BTreeSet<Id<StructureNode>> = BTreeSet::new();
    for key in scope.of_kind(Kind::StructureNode) {
        let n: Id<StructureNode> = typed(key);
        nodes.insert(n);
        nodes.extend(m.idx.structure_children.get(&n).copied());
    }
    for n in nodes {
        let Some(node) = m.structure.get(&n) else { continue };
        let Some(parent) = node.parent.and_then(|p| m.structure.get(&p)) else {
            continue;
        };
        if parent.aspect != node.aspect {
            return Err(violation(
                STRUCTURE_SAME_ASPECT,
                format!(
                    "structure node {n} and its parent {} have different aspects",
                    parent.id
                ),
                vec![
                    key_of(Kind::StructureNode, n),
                    key_of(Kind::StructureNode, parent.id),
                ],
            ));
        }
    }
    Ok(())
}

/// EW-INV-012: touched pages.
fn no_stored_report_page(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::Page) {
        if let Some(p) = m.pages.get(&typed(key)) {
            if p.kind == ew_schema::PageType::Report {
                return Err(violation(
                    NO_STORED_REPORT_PAGE,
                    format!(
                        "page {} is of kind Report; report pages are generated, never stored",
                        p.id
                    ),
                    vec![*key],
                ));
            }
        }
    }
    Ok(())
}

/// The functions a representation names (drawn functions and pin-map targets).
fn rep_functions(rep: &Representation, out: &mut BTreeSet<Id<Function>>) {
    out.extend(rep.functions.iter().copied());
    out.extend(rep.pin_map.iter().map(|p| p.function));
}

/// EW-INV-013: touched functions and the functions named by touched representations, before and after.
fn ip_one_representation(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    let mut fns: BTreeSet<Id<Function>> = scope.of_kind(Kind::Function).map(typed).collect();
    for key in scope.of_kind(Kind::Representation) {
        if let Some(r) = scope.before_of::<Representation>(key) {
            rep_functions(r, &mut fns);
        }
        if let Some(r) = m.reps.get(&typed(key)) {
            rep_functions(r, &mut fns);
        }
    }
    for f in fns {
        let Some(function) = m.functions.get(&f) else {
            continue;
        };
        if !matches!(function.kind, FunctionKind::Ip(_)) {
            continue;
        }
        let reps: Vec<Id<Representation>> = m.idx.reps_by_function.get(&f).copied().collect();
        if reps.len() != 1 {
            let mut keys = vec![key_of(Kind::Function, f)];
            keys.extend(reps.iter().map(|r| key_of(Kind::Representation, *r)));
            return Err(violation(
                IP_ONE_REPRESENTATION,
                format!(
                    "interruption-point function {f} has {} representations (exactly one required: it owns the                      function)",
                    reps.len()
                ),
                keys,
            ));
        }
    }
    Ok(())
}

/// EW-INV-014: touched report blocks.
fn report_block_folio(m: &Model, scope: &Scope) -> Result<(), ModelError> {
    for key in scope.of_kind(Kind::ReportBlock) {
        if let Some(b) = m.report_blocks.get(&typed(key)) {
            if b.embedded.is_none() && b.first_folio.is_none() {
                return Err(violation(
                    REPORT_BLOCK_FOLIO,
                    format!(
                        "report block {} is page-oriented (no embedded) but has no first_folio",
                        b.id
                    ),
                    vec![*key],
                ));
            }
        }
    }
    Ok(())
}
