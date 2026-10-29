// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-POT rules (doc 15 §4.3; content in doc 08 §3, §8.1).

use ew_derive::EndpointKey;
use ew_derive::ew_schema::{BondKind, Kind, PotentialEarthing, PotentialType};
use ew_derive::{
    BondCarrierQ, DeriveNote, FlowBfs, FunctionCps, PhysicalWiresOf, PotentialAttrsOf, PotentialFlow,
    PotentialId, PotentialNameOf, PotentialPartition, Q, RailTerminalOf,
};

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// The targets of a potential and the suffix its findings' discriminators carry.
///
/// A potential with a definition is identified by its smallest definition: that `PotentialDef` is the
/// target and the suffix is empty. A potential without one targets the function of every member connection
/// point (interruption points included), so the finding shows on whichever member is selected; picking one
/// member, as before, picked the smallest CP ID, which is random in a new project, so a properties sheet
/// showed the finding in one copy of a project and not in another (the blind replay `gpp-2`). The suffix
/// names the smallest member CP (`@<function>:<idx>`), so two potentials over the same functions stay apart;
/// it enters only the fingerprint and the report's discriminator, never a printed text or an order. Neither
/// holds a tag, a name, a folio or a count, so a renumber or a move keeps the fingerprint.
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
            let Some(member) = member else {
                return (Vec::new(), String::new());
            };
            let targets = potential
                .cps
                .iter()
                .map(|c| Target::new(Kind::Function, c.function))
                .collect();
            (targets, format!("@{}:{}", member.function, member.idx))
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
    release: Release::Mvp,
};

/// EW-POT-001: two definitions or member roles of one potential disagree on type, voltage, environment or
/// circuit class (doc 08 §3.6), or a pass-through function joins two potentials of different types.
///
/// - **Attributes:** one finding per potential with conflicts, discriminator `attrs` (plus the CP index
///   suffix of a potential without a definition), message the conflict details joined with "; ".
/// - **0 V joined to PE** (WP-S1.16 D2, D3): when the member CPs carry both the roles `M` and `PE`, the attribute
///   finding carries the quick fix "Declare 0 V–PE bond (PELV)", a `bond.create {a, b, kind: pelv_0v_pe, via}` on
///   the potential's carrier connection (`bond_carrier`), which keeps 0 V and PE as two potentials; without a
///   carrier (none, or a tie on the printed keys) it has no fix and its message ends with the hint "declare the
///   bond with Draw ▸ Bond". A conflict without both roles is unchanged.
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
            let mut message = attrs.conflicts.join("; ");
            let mut fix = None;
            if zero_volt_meets_pe(q, p) {
                match *q.get::<BondCarrierQ>(p) {
                    Some(c) => {
                        fix = Some(QuickFix {
                            label: "Declare 0 V–PE bond (PELV)".to_owned(),
                            command: "bond.create".to_owned(),
                            params: serde_json::json!({
                                "a": endpoint_param(c.a),
                                "b": endpoint_param(c.b),
                                "kind": "pelv_0v_pe",
                                "via": c.via.to_string(),
                            }),
                        });
                    }
                    None => message.push_str("; declare the bond with Draw ▸ Bond"),
                }
            }
            let finding = Finding::new(
                &TYPE_OR_VOLTAGE_CONFLICT,
                targets.iter().copied(),
                format!("attrs{suffix}"),
                potential_name(q, p),
                message,
            );
            out.push(match fix {
                Some(fix) => finding.with_fix(fix),
                None => finding,
            });
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

/// Whether the member CPs of a potential (interruption points aside) carry both the role `M` and the role `PE`: a
/// 0 V joined to PE (WP-S1.16 D2).
fn zero_volt_meets_pe(q: &Q, p: PotentialId) -> bool {
    let partition = q.get::<PotentialPartition>(());
    let Some(potential) = partition.get(p) else {
        return false;
    };
    let (mut m, mut pe) = (false, false);
    for cp in &potential.cps {
        if potential.ips.binary_search(&cp.function).is_ok() {
            continue;
        }
        let cps = q.get::<FunctionCps>(cp.function);
        match cps
            .iter()
            .find(|c| c.idx == cp.idx)
            .and_then(|c| c.potential_role.as_ref())
        {
            Some(PotentialType::M) => m = true,
            Some(PotentialType::Pe) => pe = true,
            _ => {}
        }
    }
    m && pe
}

/// An endpoint as a command parameter (`{"cp": {"function", "idx"}}`, `{"junction": {"junction"}}`).
fn endpoint_param(e: EndpointKey) -> serde_json::Value {
    match e {
        EndpointKey::Cp(function, idx) => {
            serde_json::json!({"cp": {"function": function.to_string(), "idx": idx}})
        }
        EndpointKey::Junction(junction) => {
            serde_json::json!({"junction": {"junction": junction.to_string()}})
        }
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
    release: Release::Mvp,
};

/// EW-POT-008, **partial**: only the half "a rail-bonded terminal without a location", from the
/// `rail_terminal` notes (the terminal joins no location PE node; since cycle 33 only in a project with two or more
/// own locations, else it joins the implicit main location). The other half, more than one PE
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
    release: Release::Mvp,
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
    release: Release::Mvp,
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
    release: Release::Mvp,
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

/// EW-POT-002 "PELV or SELV bonding" (doc 15 §4.3): safety, o r, error, potential.
pub const PELV_SELV_BONDING: RuleMeta = RuleMeta {
    id: "EW-POT-002",
    code: "005002",
    pack: "EW-POT",
    title: "PELV or SELV bonding",
    kind: RuleKind::Safety,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-POT-002: a PELV potential without exactly one 0 V–PE bond, a SELV potential with any (FR-CHK-028,
/// doc 08 §3.7).
///
/// - **PELV.** An M-type potential (its own type, else the type of its flow, or one of its definitions
///   declares type M) is PELV when it or one of its definitions declares `env.earthing = pelv` or a
///   `Pelv0vPe` bond ends at it; it must have exactly one such bond. None (the bond replaced by a plain wire,
///   which merges 0V into PE, D-22) or two are one finding, discriminator `pelv`.
/// - **SELV.** A potential that declares `env.earthing = selv` (itself or a definition) must have no
///   `Pelv0vPe` bond; one or more are one finding, discriminator `selv`.
///
/// Targets: the potential's (see `potential_targets`) and the bonds counted. Golden A: 0V is PELV with B1.
#[derive(Clone, Copy, Debug)]
pub struct PelvSelvBonding;

impl Rule for PelvSelvBonding {
    fn meta(&self) -> &'static RuleMeta {
        &PELV_SELV_BONDING
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let attrs = q.get::<PotentialAttrsOf>(p);
        let earthing = attrs.env.as_ref().and_then(|e| e.value.earthing.clone());
        let ptype = attrs.ptype.as_ref().map(|a| a.value.clone()).or_else(|| {
            q.get::<PotentialFlow>(p)
                .as_ref()
                .as_ref()
                .and_then(|e| e.ptype.as_ref().map(|a| a.value.clone()))
        });
        let partition = q.get::<PotentialPartition>(());
        let bonds: Vec<_> = partition
            .bonds()
            .iter()
            .filter(|b| b.kind == BondKind::Pelv0vPe && (b.a == Some(p) || b.b == Some(p)))
            .map(|b| b.bond)
            .collect();
        // The definitions of the potential: a plain wire that merges 0V into PE (D-22) leaves one potential
        // whose type is the first definition's, but the 0 V definition still declares an M-type PELV circuit.
        let defs = partition.get(p).map(|x| x.defs.clone()).unwrap_or_default();
        let declared = |pred: &dyn Fn(&ew_derive::ew_schema::PotentialDef) -> bool| {
            defs.iter()
                .filter_map(|d| q.potential_defs().get(*d))
                .any(|d| pred(d))
        };
        let m_type = ptype == Some(PotentialType::M) || declared(&|d| d.ptype == Some(PotentialType::M));
        let earthed = |e: PotentialEarthing| {
            earthing.as_ref() == Some(&e) || declared(&|d| d.env.earthing.as_ref() == Some(&e))
        };
        let pelv = m_type && (earthed(PotentialEarthing::Pelv) || !bonds.is_empty());
        let selv = earthed(PotentialEarthing::Selv);
        let (discriminator, message) = if pelv && bonds.len() != 1 {
            let found = if bonds.is_empty() {
                "has no 0 V–PE bond".to_owned()
            } else {
                format!("has {} 0 V–PE bonds", bonds.len())
            };
            (
                "pelv",
                format!(
                    "PELV potential {} {found}; a PELV circuit needs exactly one (declare the bond on the 0 V–PE wire)",
                    potential_name(q, p)
                ),
            )
        } else if selv && !bonds.is_empty() {
            (
                "selv",
                format!(
                    "SELV potential {} is bonded to PE ({} 0 V–PE bond(s)); a SELV circuit must not be earthed",
                    potential_name(q, p),
                    bonds.len()
                ),
            )
        } else {
            return Vec::new();
        };
        let (targets, suffix) = potential_targets(q, p);
        vec![Finding::new(
            &PELV_SELV_BONDING,
            targets
                .into_iter()
                .chain(bonds.iter().map(|b| Target::new(Kind::Bond, *b))),
            format!("{discriminator}{suffix}"),
            potential_name(q, p),
            message,
        )]
    }
}
