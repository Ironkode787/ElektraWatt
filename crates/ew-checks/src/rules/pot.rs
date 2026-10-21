// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-POT rules (doc 15 §4.3; content in doc 08 §3, §8.1).

use ew_derive::ew_schema::Kind;
use ew_derive::{
    DeriveNote, FlowBfs, PhysicalWiresOf, PotentialAttrsOf, PotentialId, PotentialNameOf, PotentialPartition,
    Q, RailTerminalOf,
};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// The targets of a potential and the suffix its findings' discriminators carry.
///
/// A potential with a definition is identified by its smallest definition: that `PotentialDef` is the
/// target and the suffix is empty. A potential without one is identified by the smallest of its member
/// connection points: the function of that CP is the target and the CP index is the suffix (`@3`), so two
/// findings about two potentials of one function stay apart. Neither holds a tag, a name, a folio or a
/// count, so a renumber or a move keeps the fingerprint.
pub(crate) fn potential_targets(q: &Q, p: PotentialId) -> (Vec<Target>, String) {
    match p {
        PotentialId::Def(d) => (vec![Target::new(Kind::PotentialDef, d)], String::new()),
        PotentialId::Cp(cp) => {
            let partition = q.get::<PotentialPartition>(());
            let Some(potential) = partition.get(p) else {
                return (Vec::new(), String::new());
            };
            let member = potential
                .cps
                .iter()
                .find(|c| c.id() == cp)
                .or_else(|| potential.cps.iter().min_by_key(|c| c.id()));
            match member {
                Some(c) => (
                    vec![Target::new(Kind::Function, c.function)],
                    format!("@{}", c.idx),
                ),
                None => (Vec::new(), String::new()),
            }
        }
    }
}

/// The display name of a potential, as the derivation prints it.
fn potential_name(q: &Q, p: PotentialId) -> String {
    q.get::<PotentialNameOf>(p).name.text.clone()
}

/// The findings of the `Connectivity` notes of `PhysicalWiresOf(p)` with `code`: targets are the
/// potential's, the discriminator is the note's subject (a connection or function ID).
fn wire_note_findings(q: &Q, meta: &'static RuleMeta, p: PotentialId) -> Vec<Finding> {
    let wires = q.get::<PhysicalWiresOf>(p);
    let (targets, _) = potential_targets(q, p);
    let name = potential_name(q, p);
    wires
        .notes
        .iter()
        .filter_map(|note| match note {
            DeriveNote::Connectivity {
                code,
                subject,
                detail,
            } if *code == meta.id => Some(Finding::new(
                meta,
                targets.iter().copied(),
                subject.clone(),
                name.clone(),
                detail.clone(),
            )),
            _ => None,
        })
        .collect()
}

/// EW-POT-001 "Type or voltage conflict" (doc 15 §4.3): conflict, o r, error, potential.
pub const TYPE_OR_VOLTAGE_CONFLICT: RuleMeta = RuleMeta {
    id: "EW-POT-001",
    code: "005001",
    pack: "EW-POT",
    title: "Type or voltage conflict",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
};

/// EW-POT-001: two definitions or member roles of one potential disagree on type, voltage, environment or
/// circuit class (doc 08 §3.6), or a pass-through function joins two potentials of different types.
///
/// - **Attributes:** one finding per potential with conflicts, discriminator `attrs` (plus the CP index
///   suffix of a potential without a definition), message the conflict details joined with "; ".
/// - **Flow:** one finding per pass-through conflict whose first side is the potential, discriminator
///   `flow` (plus the suffix), targets the potential's and the function.
#[derive(Clone, Copy, Debug)]
pub struct TypeOrVoltageConflict;

impl Rule for TypeOrVoltageConflict {
    fn meta(&self) -> &'static RuleMeta {
        &TYPE_OR_VOLTAGE_CONFLICT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let (targets, suffix) = potential_targets(q, p);
        let attrs = q.get::<PotentialAttrsOf>(p);
        if !attrs.conflicts.is_empty() {
            out.push(Finding::new(
                &TYPE_OR_VOLTAGE_CONFLICT,
                targets.iter().copied(),
                format!("attrs{suffix}"),
                potential_name(q, p),
                attrs.conflicts.join("; "),
            ));
        }
        let flow = q.get::<FlowBfs>(());
        for c in flow.conflicts.iter().filter(|c| c.a == p) {
            out.push(Finding::new(
                &TYPE_OR_VOLTAGE_CONFLICT,
                targets
                    .iter()
                    .copied()
                    .chain([Target::new(Kind::Function, c.function)]),
                format!("flow{suffix}"),
                potential_name(q, p),
                format!(
                    "pass-through of function {} joins type {} and type {} without a converting function",
                    c.function, c.a_type, c.b_type
                ),
            ));
        }
        out
    }
}

/// EW-POT-008 "PE island" (doc 15 §4.3): conflict, o r, warning, project.
pub const PE_ISLAND: RuleMeta = RuleMeta {
    id: "EW-POT-008",
    code: "005008",
    pack: "EW-POT",
    title: "PE island",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
};

/// EW-POT-008, **partial**: only the half "a rail-bonded terminal without a location", from the
/// `rail_terminal` notes (the terminal joins no location PE node). The other half, more than one PE
/// potential in one location, is not derived yet. One finding per terminal, target the terminal.
#[derive(Clone, Copy, Debug)]
pub struct PeIsland;

impl Rule for PeIsland {
    fn meta(&self) -> &'static RuleMeta {
        &PE_ISLAND
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (id, device) in q.devices().iter() {
            if !device.aspects.terminal.as_ref().is_some_and(|t| t.rail_bonded) {
                continue;
            }
            let rail = q.get::<RailTerminalOf>(id);
            for note in &rail.notes {
                if let DeriveNote::Connectivity {
                    code: "EW-POT-008",
                    detail,
                    ..
                } = note
                {
                    out.push(Finding::new(
                        &PE_ISLAND,
                        [Target::new(Kind::Device, id)],
                        "",
                        full_tag(q, id, "terminal"),
                        detail.clone(),
                    ));
                }
            }
        }
        out
    }
}

/// EW-POT-011 "Junction feed ambiguous" (doc 15 §4.3): conflict, o, warning, potential.
pub const JUNCTION_FEED_AMBIGUOUS: RuleMeta = RuleMeta {
    id: "EW-POT-011",
    code: "005011",
    pack: "EW-POT",
    title: "Junction feed ambiguous",
    kind: RuleKind::Conflict,
    modes: Modes::letters("o"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
};

/// EW-POT-011: the arm between two junctions feeds both or neither, or a source feeds a star without a
/// hub (doc 08 §3.2). One finding per note of the potential's physical wires; the discriminator is the
/// note's subject, an ID.
#[derive(Clone, Copy, Debug)]
pub struct JunctionFeedAmbiguous;

impl Rule for JunctionFeedAmbiguous {
    fn meta(&self) -> &'static RuleMeta {
        &JUNCTION_FEED_AMBIGUOUS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        match *scope {
            Scope::Potential(p) => wire_note_findings(q, &JUNCTION_FEED_AMBIGUOUS, p),
            _ => Vec::new(),
        }
    }
}

/// EW-POT-012 "Ring in the drawing graph" (doc 15 §4.3): hygiene, o, warning, potential.
pub const RING_IN_DRAWING_GRAPH: RuleMeta = RuleMeta {
    id: "EW-POT-012",
    code: "005012",
    pack: "EW-POT",
    title: "Ring in the drawing graph",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("o"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
};

/// EW-POT-012: the connections of a potential form a ring; the highest connection of the ring forms no
/// wire (doc 08 §3.8). One finding per note; the discriminator is the broken connection's ID.
#[derive(Clone, Copy, Debug)]
pub struct RingInDrawingGraph;

impl Rule for RingInDrawingGraph {
    fn meta(&self) -> &'static RuleMeta {
        &RING_IN_DRAWING_GRAPH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        match *scope {
            Scope::Potential(p) => wire_note_findings(q, &RING_IN_DRAWING_GRAPH, p),
            _ => Vec::new(),
        }
    }
}

/// EW-POT-014 "Physical-wire conflict" (doc 15 §4.3): conflict, o r, error, potential.
pub const PHYSICAL_WIRE_CONFLICT: RuleMeta = RuleMeta {
    id: "EW-POT-014",
    code: "005014",
    pack: "EW-POT",
    title: "Physical-wire conflict",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
};

/// EW-POT-014: the constituents of one physical wire name different conductors or carry different
/// overrides (doc 08 §3.8); the carrier's value applies. One finding per note; the discriminator is the
/// wire's ID.
#[derive(Clone, Copy, Debug)]
pub struct PhysicalWireConflict;

impl Rule for PhysicalWireConflict {
    fn meta(&self) -> &'static RuleMeta {
        &PHYSICAL_WIRE_CONFLICT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        match *scope {
            Scope::Potential(p) => wire_note_findings(q, &PHYSICAL_WIRE_CONFLICT, p),
            _ => Vec::new(),
        }
    }
}
