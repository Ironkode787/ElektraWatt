// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-POT rules (doc 15 §4.3; content in doc 08 §3, §8.1, §8.2; WP-S1.16 part 2).
//!
//! # Determinism (WP-S1.16 §10)
//!
//! Every order, text, target and fingerprint here follows the project's content and printed keys, never the IDs of
//! a new project (ID-677): definitions rank in natural order of their names (D9, FT-587), the discriminator member
//! of a potential without a definition is its first member CP by printed text (D8, FT-588), and a note of a bond
//! closure is reported by the potential of its subject that prints first (FT-188). An ID decides only between two
//! things that print alike.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_cmp;
use ew_derive::ew_schema::{
    BondKind, ColourCode, Connection, Csa, Device, Discipline, Function, FunctionKind, IecColour, Kind, Page,
    PotentialEarthing, PotentialType, StructureNode,
};
use ew_derive::ew_units::Dec3;
use ew_derive::{
    BondCarrierQ, ConnWireProps, CpRef, DeriveNote, DeviceTag, DrawnPageOrder, EndKind, EndpointKey, FlowBfs,
    FunctionCps, FunctionLinksOf, PageRef, PhysicalWire, PhysicalWiresOf, PotentialAttrsOf, PotentialFlow,
    PotentialId, PotentialNameOf, PotentialPartition, Q, RailLocation, RailTerminalOf, StructurePath,
    WireNumbers, bonds_of_potential, colour_text, cp_label, cp_text, csa_text, device_location,
    potential_id_at, potential_part, ranked_defs, wire_props_below_overrides,
};

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

// ---------------------------------------------------------------------------------------------------------
// The targets of a potential

/// The targets of a potential's findings, the key their fingerprints are computed from, and the suffix of their
/// discriminators.
#[derive(Clone, Debug, Default)]
pub(crate) struct PotTargets {
    /// The display targets: every definition (D9), or, without one, the function of every member CP (interruption
    /// points included), so a finding shows on whichever of them is selected.
    pub targets: Vec<Target>,
    /// The identity the fingerprint is computed from (D8, FT-588): the potential's identity definition (its smallest
    /// definition ID, the potential's ID, as before), or the function of its discriminator member. A waiver keeps
    /// matching when a member or a further definition is added or removed.
    pub key: Vec<Target>,
    /// `@<function>:<idx>` of the discriminator member of a potential without a definition, empty otherwise. It
    /// enters only the fingerprint and the report's discriminator, never a printed text or an order.
    pub suffix: String,
}

impl PotTargets {
    /// A finding of `meta` about the potential, with `extra` targets (a function, the bonds counted), which join both
    /// the display targets and the fingerprint key.
    fn finding(
        &self,
        meta: &'static RuleMeta,
        extra: &[Target],
        discriminator: String,
        subject: String,
        message: String,
    ) -> Finding {
        let key: Vec<Target> = self.key.iter().chain(extra).copied().collect();
        Finding::new(
            meta,
            self.targets.iter().chain(extra).copied(),
            discriminator,
            subject,
            message,
        )
        .keyed_on(&key)
    }
}

/// The targets of a potential (see [`PotTargets`]).
///
/// The discriminator member of a potential without a definition is its first member CP in printed order (WP-S1.16
/// D8): device CPs before interruption points, then `<tag>:<designation>` in natural order (D1's printed key), then
/// the CP index, the CP ID only between two CPs that print alike on two functions. Before, it was the smallest CP ID,
/// random in a new project. The potential is read as its own part of the partition (FT-236).
pub(crate) fn potential_targets(q: &Q, p: PotentialId) -> PotTargets {
    let part = potential_part(q, p);
    let Some(potential) = part.get(p) else {
        return PotTargets::default();
    };
    match p {
        PotentialId::Def(d) => PotTargets {
            targets: potential
                .defs
                .iter()
                .map(|x| Target::new(Kind::PotentialDef, *x))
                .collect(),
            key: vec![Target::new(Kind::PotentialDef, d)],
            suffix: String::new(),
        },
        PotentialId::Cp(_) => {
            let is_ip = |f: Id<Function>| potential.ips.binary_search(&f).is_ok();
            let member = potential
                .cps
                .iter()
                .map(|c| ((is_ip(c.function), cp_text(q, *c)), *c))
                .min_by(|((ia, ta), a), ((ib, tb), b)| {
                    ia.cmp(ib)
                        .then_with(|| natural_cmp(ta, tb))
                        .then_with(|| ta.cmp(tb))
                        .then_with(|| a.idx.cmp(&b.idx))
                        .then_with(|| a.id().cmp(&b.id()))
                })
                .map(|(_, c)| c);
            let Some(member) = member else {
                return PotTargets::default();
            };
            PotTargets {
                targets: potential
                    .cps
                    .iter()
                    .map(|c| Target::new(Kind::Function, c.function))
                    .collect(),
                key: vec![Target::new(Kind::Function, member.function)],
                suffix: format!("@{}:{}", member.function, member.idx),
            }
        }
    }
}

/// The display name of a potential, as the derivation prints it.
fn potential_name(q: &Q, p: PotentialId) -> String {
    q.get::<PotentialNameOf>(p).name.text.clone()
}

/// The order of potentials by their printed names: natural order, then bytes, then the ID.
fn by_name(q: &Q, a: PotentialId, b: PotentialId) -> std::cmp::Ordering {
    let (na, nb) = (potential_name(q, a), potential_name(q, b));
    natural_cmp(&na, &nb)
        .then_with(|| na.cmp(&nb))
        .then_with(|| a.cmp(&b))
}

/// The potential, of `candidates`, that reports a finding they share: the first by printed name (FT-188).
fn owner(q: &Q, candidates: impl IntoIterator<Item = PotentialId>) -> Option<PotentialId> {
    candidates.into_iter().min_by(|a, b| by_name(q, *a, *b))
}

/// Whether the potential `p` reports the findings about `wire`: a wire of one potential is its own; a bond wire,
/// which belongs to both potentials of its bond (and is extracted with every potential of the bond closure), is
/// reported by the one that prints first (FT-188).
fn owns_wire(q: &Q, p: PotentialId, wire: &PhysicalWire) -> bool {
    owner(q, wire.potentials.iter().copied()) == Some(p)
}

/// The final type of a potential: its flow entry's (own or propagated), else its own.
fn final_type(q: &Q, p: PotentialId) -> Option<PotentialType> {
    q.get::<PotentialFlow>(p)
        .as_ref()
        .as_ref()
        .and_then(|e| e.ptype.as_ref().map(|a| a.value.clone()))
        .or_else(|| {
            q.get::<PotentialAttrsOf>(p)
                .ptype
                .as_ref()
                .map(|a| a.value.clone())
        })
}

/// A wire as a message prints it: its wire number (`101.3`), a core as `<cable>/<core>` (`-3W1/BU`), else its two
/// ends (`-X1:1/a–-K1:A1`).
fn wire_text(q: &Q, wire: &PhysicalWire) -> String {
    if let Some(n) = q.get::<WireNumbers>(()).number(wire.id) {
        return n.label.clone();
    }
    if let Some(c) = wire.core.and_then(|c| q.conductors().get(c)) {
        let cable = q
            .get::<DeviceTag>(c.cable)
            .as_ref()
            .as_ref()
            .map(|t| t.short())
            .unwrap_or_else(|| "?".to_owned());
        return format!("{cable}/{}", c.core.as_str());
    }
    format!(
        "{}–{}",
        cp_label(q, wire.ends[0].cp),
        cp_label(q, wire.ends[1].cp)
    )
}

/// The connection point record of a CP.
fn cp_of(q: &Q, cp: CpRef) -> Option<ew_derive::Cp> {
    q.get::<FunctionCps>(cp.function)
        .iter()
        .find(|c| c.idx == cp.idx)
        .cloned()
}

/// The findings of the `Connectivity` notes of `PhysicalWiresOf(p)` with `code`: targets are the potential's, the
/// discriminator is the note's subject (a connection or function ID).
///
/// `PhysicalWiresOf(p)` extracts the wires of the whole bond closure of `p` (doc 08 §3.8), so every potential of the
/// closure sees the same notes; each note is reported once, by the potential of its subject that prints first
/// (FT-188, [`note_owner`]).
fn wire_note_findings(q: &Q, meta: &'static RuleMeta, p: PotentialId) -> Vec<Finding> {
    let wires = q.get::<PhysicalWiresOf>(p);
    let pt = potential_targets(q, p);
    let name = potential_name(q, p);
    wires
        .notes
        .iter()
        .filter_map(|note| match note {
            DeriveNote::Connectivity {
                code,
                subject,
                detail,
            } if *code == meta.id && note_owner(q, p, &wires.wires, subject) == Some(p) => {
                Some(pt.finding(meta, &[], subject.clone(), name.clone(), detail.clone()))
            }
            _ => None,
        })
        .collect()
}

/// The potential that reports a wire note about `subject` (FT-188): of the potentials the subject lies in (a wire's
/// or a connection's ends, a function's CPs, a junction), the one that prints first; `p` itself when the subject is
/// none of these.
fn note_owner(q: &Q, p: PotentialId, wires: &[PhysicalWire], subject: &str) -> Option<PotentialId> {
    let mut pots: BTreeSet<PotentialId> = BTreeSet::new();
    for w in wires {
        if w.id.to_string() == subject || w.constituents.iter().any(|c| c.to_string() == subject) {
            pots.extend(w.potentials.iter().copied());
        }
    }
    if pots.is_empty()
        && let Ok(uuid) = subject.parse::<ew_derive::ew_ids::Uuid>()
    {
        let partition = q.get::<PotentialPartition>(());
        let at = |e: EndpointKey| partition.potential_at(e).map(|x| x.id);
        let conn: Id<Connection> = Id::from_uuid(uuid);
        if let Some(c) = q.connections().get(conn) {
            pots.extend(
                [&c.a, &c.b]
                    .into_iter()
                    .filter_map(EndpointKey::of)
                    .filter_map(at),
            );
        }
        let f: Id<Function> = Id::from_uuid(uuid);
        if q.functions().contains(f) {
            pots.extend(
                q.get::<FunctionCps>(f)
                    .iter()
                    .filter_map(|c| at(EndpointKey::Cp(f, c.idx))),
            );
        }
        pots.extend(at(EndpointKey::Junction(Id::from_uuid(uuid))));
    }
    if pots.is_empty() {
        return Some(p);
    }
    owner(q, pots)
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-001

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
///   suffix of a potential without a definition), message the conflict details joined with "; ". The definitions
///   rank in natural order of their names (D9, FT-587): the first applies its values, every definition is a target,
///   and the texts name definitions and CPs by their printed text in that order.
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
        let pt = potential_targets(q, p);
        let suffix = &pt.suffix;
        let attrs = q.get::<PotentialAttrsOf>(p);
        if !attrs.conflicts.is_empty() {
            let (shown, more) = shown_conflicts(&attrs.conflicts);
            let mut message = shown.clone();
            if more > 0 {
                message.push_str(&format!("; and {more} more member CPs"));
            }
            let mut hint = "none";
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
                    None => {
                        message.push_str("; declare the bond with Draw ▸ Bond");
                        hint = "bond";
                    }
                }
            }
            // The text names at most [`SHOWN_MEMBER_CPS`] member CPs; the `members` argument keeps every conflict.
            let finding = pt
                .finding(
                    &TYPE_OR_VOLTAGE_CONFLICT,
                    &[],
                    format!("attrs{suffix}"),
                    potential_name(q, p),
                    message,
                )
                .arg("conflicts", shown)
                .arg(
                    "more",
                    if more > 0 {
                        more.to_string()
                    } else {
                        "none".to_owned()
                    },
                )
                .arg("hint", hint)
                .arg("members", attrs.conflicts.join("; "));
            out.push(match fix {
                Some(fix) => finding.with_fix(fix),
                None => finding,
            });
        }
        let flow = q.get::<FlowBfs>(());
        for c in flow.conflicts.iter().filter(|c| c.a == p) {
            // The device tag, never the function's ID: no finding text depends on IDs (WP-S1.16 §10.1, ID-677).
            let tag = q
                .functions()
                .get(c.function)
                .and_then(|f| f.device)
                .map_or_else(|| "?".to_owned(), |d| full_tag(q, d, "?"));
            out.push(pt.finding(
                &TYPE_OR_VOLTAGE_CONFLICT,
                &[Target::new(Kind::Function, c.function)],
                format!("flow{suffix}"),
                potential_name(q, p),
                format!(
                    "pass-through of function {tag} joins type {} and type {} without a converting function",
                    c.a_type, c.b_type
                ),
            ));
        }
        out
    }
}

/// The most member-CP conflicts an EW-POT-001 message names; the rest are counted ("and 23 more member CPs").
pub(crate) const SHOWN_MEMBER_CPS: usize = 3;

/// The conflicts an EW-POT-001 message shows, joined with "; " in the order found: every definition conflict and the
/// first [`SHOWN_MEMBER_CPS`] member-CP conflicts (`member CP …`); and how many member-CP conflicts it leaves out.
fn shown_conflicts(conflicts: &[String]) -> (String, usize) {
    let mut members = 0;
    let mut shown = Vec::new();
    for c in conflicts {
        if c.starts_with("member CP ") {
            members += 1;
            if members > SHOWN_MEMBER_CPS {
                continue;
            }
        }
        shown.push(c.as_str());
    }
    (shown.join("; "), members.saturating_sub(SHOWN_MEMBER_CPS))
}

/// Whether the member CPs of a potential (interruption points aside) carry both the role `M` and the role `PE`: a
/// 0 V joined to PE (WP-S1.16 D2).
fn zero_volt_meets_pe(q: &Q, p: PotentialId) -> bool {
    let part = potential_part(q, p);
    let Some(potential) = part.get(p) else {
        return false;
    };
    let (mut m, mut pe) = (false, false);
    for cp in &potential.cps {
        if potential.ips.binary_search(&cp.function).is_ok() {
            continue;
        }
        match cp_of(q, *cp).and_then(|c| c.potential_role) {
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

// ---------------------------------------------------------------------------------------------------------
// EW-POT-002

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
/// Targets: the potential's (see `potential_targets`) and the bonds counted. The bonds and the potential are read as
/// their own parts of the partition (`BondsOf`, FT-236), so a topology change elsewhere does not re-run the rule.
/// Golden A: 0V is PELV with B1.
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
        let bonds: Vec<_> = bonds_of_potential(q, p)
            .into_iter()
            .filter(|b| b.kind == BondKind::Pelv0vPe)
            .map(|b| b.bond)
            .collect();
        // The definitions of the potential: a plain wire that merges 0V into PE (D-22) leaves one potential
        // whose type is the first definition's, but the 0 V definition still declares an M-type PELV circuit.
        let defs = potential_part(q, p)
            .get(p)
            .map(|x| x.defs.clone())
            .unwrap_or_default();
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
        let pt = potential_targets(q, p);
        let bond_targets: Vec<Target> = bonds.iter().map(|b| Target::new(Kind::Bond, *b)).collect();
        vec![pt.finding(
            &PELV_SELV_BONDING,
            &bond_targets,
            format!("{discriminator}{}", pt.suffix),
            potential_name(q, p),
            message,
        )]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-003

/// EW-POT-003 "FE pin on PE" (doc 15 §4.3): conflict, o r, warning, potential.
pub const FE_PIN_ON_PE: RuleMeta = RuleMeta {
    id: "EW-POT-003",
    code: "005003",
    pack: "EW-POT",
    title: "FE pin on PE",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-POT-003: a connection point with the role FE on a PE (or PEN) potential without an FE–PE bond at it (doc 08
/// §8.1, FR-CHK-030, D-16). An FE role on a PE potential is compatible as a type (doc 08 §3.7), so it is this
/// warning, not EW-POT-001: functional earth should reach PE through a declared `fe_pe` bond. One finding per CP,
/// target its function, discriminator `fe:<idx>`; EW-PLC-013 supersedes it for PLC supply CPs at 1.0. Golden A:
/// -201U1:3 and -202U1:3 (EF-10).
#[derive(Clone, Copy, Debug)]
pub struct FePinOnPe;

impl Rule for FePinOnPe {
    fn meta(&self) -> &'static RuleMeta {
        &FE_PIN_ON_PE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        if !matches!(final_type(q, p), Some(PotentialType::Pe | PotentialType::Pen)) {
            return Vec::new();
        }
        let part = potential_part(q, p);
        let Some(potential) = part.get(p) else {
            return Vec::new();
        };
        let name = potential_name(q, p);
        let mut out = Vec::new();
        for cp in &potential.cps {
            if potential.ips.binary_search(&cp.function).is_ok() {
                continue;
            }
            if cp_of(q, *cp).and_then(|c| c.potential_role) != Some(PotentialType::Fe) {
                continue;
            }
            let bonded = q
                .idx()
                .bonds_at(cp.key())
                .filter_map(|b| q.bonds().get(b))
                .any(|b| b.kind == BondKind::FePe);
            if bonded {
                continue;
            }
            let label = cp_label(q, *cp);
            out.push(Finding::new(
                &FE_PIN_ON_PE,
                [Target::new(Kind::Function, cp.function)],
                format!("fe:{}", cp.idx),
                label.clone(),
                format!(
                    "{label} has the role FE and is wired to the PE potential {name}; declare an FE–PE bond or \
                     connect it to functional earth"
                ),
            )
            .arg("cp", label)
            .arg("potential", name.clone()));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-004, EW-POT-005: colour codes

/// EW-POT-004 "GNYE on a non-PE potential" (doc 15 §4.3): safety, o r, error, potential.
pub const GNYE_ON_NON_PE: RuleMeta = RuleMeta {
    id: "EW-POT-004",
    code: "005004",
    pack: "EW-POT",
    title: "GNYE on a non-PE potential",
    kind: RuleKind::Safety,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// Whether a colour is exactly the IEC code(s) `codes`.
fn colour_is(colour: Option<&ew_derive::ew_schema::ColourSpec>, codes: &[IecColour]) -> bool {
    colour.is_some_and(|c| {
        c.codes().len() == codes.len()
            && c.codes()
                .iter()
                .zip(codes)
                .all(|(a, b)| matches!(a, ColourCode::Iec(x) if x == b))
    })
}

/// The wires a potential reports about (its own, a bond wire by the potential that prints first), each with its
/// effective properties, in wire ID order.
fn owned_wires(q: &Q, p: PotentialId) -> Vec<(PhysicalWire, ew_derive::EffectiveProps)> {
    let wires = q.get::<PhysicalWiresOf>(p);
    let props = q.get::<ew_derive::WirePropsOf>(p);
    wires
        .wires
        .iter()
        .filter(|w| owns_wire(q, p, w))
        .filter_map(|w| props.wires.get(&w.id).map(|e| (w.clone(), e.clone())))
        .collect()
}

/// EW-POT-004: a wire whose effective colour is GNYE on a potential that is not PE or PEN (doc 08 §8.1, FR-CHK-031,
/// STD M-09). Bond wires are exempt (W27, the 0 V–PE bond wire of Golden A, takes the bond colour). One finding per
/// wire, target the wire's carrier connection.
#[derive(Clone, Copy, Debug)]
pub struct GnyeOnNonPe;

impl Rule for GnyeOnNonPe {
    fn meta(&self) -> &'static RuleMeta {
        &GNYE_ON_NON_PE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let ptype = final_type(q, p);
        if matches!(ptype, Some(PotentialType::Pe | PotentialType::Pen)) {
            return Vec::new();
        }
        let name = potential_name(q, p);
        owned_wires(q, p)
            .into_iter()
            .filter(|(w, props)| {
                w.bond.is_none()
                    && colour_is(
                        props.colour.as_ref().map(|c| &c.value),
                        &[IecColour::Gn, IecColour::Ye],
                    )
            })
            .map(|(w, _)| {
                let text = wire_text(q, &w);
                Finding::new(
                    &GNYE_ON_NON_PE,
                    [Target::new(Kind::Connection, w.id)],
                    "",
                    text.clone(),
                    format!(
                        "wire {text} is GNYE on the potential {name}{}; GNYE is reserved for protective conductors",
                        type_suffix(ptype.as_ref())
                    ),
                )
            })
            .collect()
    }
}

/// ` (type <T>)`, empty for a potential without a type.
fn type_suffix(ptype: Option<&PotentialType>) -> String {
    ptype.map(|t| format!(" (type {t:?})")).unwrap_or_default()
}

/// EW-POT-005 "BU on a non-N conductor" (doc 15 §4.3): conflict, o r, warning, potential.
pub const BU_ON_NON_N: RuleMeta = RuleMeta {
    id: "EW-POT-005",
    code: "005005",
    pack: "EW-POT",
    title: "BU on a non-N conductor",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-POT-005: a wire or core whose effective colour is exactly the IEC code BU (not a company code such as DBU) on a
/// potential that is not N, in a project that has an N potential (doc 08 §8.1, FR-CHK-031, STD M-10). One finding
/// per wire, target the carrier connection. Golden A: the vendor cores W36 and W41, BU on 0V (EF-11).
///
/// The rule judges a colour the project chose (an override, a core's conductor, a potential definition, a colour
/// profile row, the project default), not the program's built-in fallback (level 0 of doc 08 §4.1): the built-in
/// table gives BU to L+, L-, M and DC signals by EN 60204-1 13.2.4 (doc 08 §4.3) when the project has no colour of
/// its own, and a warning on the program's own default is not one the drafter chose (cycle-35 play-test).
#[derive(Clone, Copy, Debug)]
pub struct BuOnNonN;

impl Rule for BuOnNonN {
    fn meta(&self) -> &'static RuleMeta {
        &BU_ON_NON_N
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let ptype = final_type(q, p);
        if ptype == Some(PotentialType::N) {
            return Vec::new();
        }
        let wires: Vec<_> = owned_wires(q, p)
            .into_iter()
            .filter(|(w, props)| {
                let chosen = props
                    .colour
                    .as_ref()
                    .filter(|c| !matches!(c.source, ew_derive::PropSource::BuiltIn { .. }));
                w.bond.is_none() && colour_is(chosen.map(|c| &c.value), &[IecColour::Bu])
            })
            .collect();
        if wires.is_empty() || !project_has_n(q) {
            return Vec::new();
        }
        let name = potential_name(q, p);
        wires
            .into_iter()
            .map(|(w, _)| {
                let text = wire_text(q, &w);
                Finding::new(
                    &BU_ON_NON_N,
                    [Target::new(Kind::Connection, w.id)],
                    "",
                    text.clone(),
                    format!(
                        "wire {text} is BU on the potential {name}{}; in a project with a neutral, BU marks the \
                         neutral conductor",
                        type_suffix(ptype.as_ref())
                    ),
                )
                .arg("wire", text)
                .arg("potential", name.clone())
                .arg(
                    "type",
                    ptype
                        .as_ref()
                        .map_or_else(|| "none".to_owned(), |t| format!("{t:?}")),
                )
            })
            .collect()
    }
}

/// Whether a potential of the project has the final type N.
fn project_has_n(q: &Q) -> bool {
    q.get::<FlowBfs>(())
        .entries
        .values()
        .any(|e| e.ptype.as_ref().is_some_and(|t| t.value == PotentialType::N))
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-006: PE sizing

/// EW-POT-006 "PE undersized" (doc 15 §4.3): safety, o r, error, potential.
pub const PE_UNDERSIZED: RuleMeta = RuleMeta {
    id: "EW-POT-006",
    code: "005006",
    pack: "EW-POT",
    title: "PE undersized",
    kind: RuleKind::Safety,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// The rule data of EW-POT-006 (`data/pe_sizing.toml`, WP-S1.16 D5).
const PE_SIZING_TOML: &str = include_str!("../../data/pe_sizing.toml");

/// One row of the PE sizing table: up to which line cross-section it applies, and the PE it requires.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SizingRow {
    up_to: Option<Dec3>,
    pe: PeRule,
}

/// The minimum PE of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeRule {
    /// Equal to the line cross-section.
    Same,
    /// Half of it.
    Half,
    /// A fixed value.
    Fixed(Dec3),
}

/// The PE sizing data pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeSizing {
    /// Whether a person checked the values against the standards (NFR-COMP-05).
    pub verified: bool,
    /// The clauses the table implements (cited, not reproduced).
    pub source: String,
    /// The potential types whose wires are line conductors.
    pub line_types: Vec<PotentialType>,
    rows: Vec<SizingRow>,
}

impl PeSizing {
    /// The minimum PE cross-section (mm²) for a largest line cross-section `s` (mm²).
    pub fn required(&self, s: Dec3) -> Option<Dec3> {
        let row = self.rows.iter().find(|r| r.up_to.is_none_or(|u| s <= u))?;
        match row.pe {
            PeRule::Same => Some(s),
            PeRule::Half => s.checked_div(Dec3::from_int(2)?),
            PeRule::Fixed(v) => Some(v),
        }
    }
}

/// The shipped PE sizing data (`data/pe_sizing.toml`), parsed once.
///
/// # Panics
///
/// When the shipped file does not parse; a unit test parses it, so a release cannot ship a broken one.
pub fn pe_sizing() -> &'static PeSizing {
    pe_sizing_data().expect("data/pe_sizing.toml parses")
}

/// The shipped PE sizing data, parsed once, or why it does not load: EW-POT-006 is then not evaluated and the engine
/// reports EW-CORE-084 instead ([`data_tables`](crate::rules::system::data_tables)).
pub fn pe_sizing_data() -> Result<&'static PeSizing, &'static str> {
    static DATA: OnceLock<Result<PeSizing, String>> = OnceLock::new();
    DATA.get_or_init(|| parse_pe_sizing(PE_SIZING_TOML))
        .as_ref()
        .map_err(String::as_str)
}

/// Parses a PE sizing table.
fn parse_pe_sizing(text: &str) -> Result<PeSizing, String> {
    let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let verified = v
        .get("verified")
        .and_then(toml::Value::as_bool)
        .ok_or("`verified` is missing")?;
    let source = v
        .get("source")
        .and_then(toml::Value::as_str)
        .ok_or("`source` is missing")?
        .to_owned();
    let line_types = v
        .get("line_types")
        .and_then(toml::Value::as_array)
        .ok_or("`line_types` is missing")?
        .iter()
        .map(|t| {
            t.as_str()
                .and_then(|s| serde_json::from_value::<PotentialType>(serde_json::json!(s)).ok())
                .ok_or_else(|| format!("line type {t} is no potential type"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dec = |s: &str| s.parse::<Dec3>().map_err(|e| format!("{s}: {e}"));
    let mut rows = Vec::new();
    for r in v
        .get("rows")
        .and_then(toml::Value::as_array)
        .ok_or("`rows` is missing")?
    {
        let up_to = match r.get("up_to_mm2").and_then(toml::Value::as_str) {
            Some(s) => Some(dec(s)?),
            None => None,
        };
        let pe = match r.get("pe").and_then(toml::Value::as_str) {
            Some("s") => PeRule::Same,
            Some("half") => PeRule::Half,
            Some(s) => PeRule::Fixed(dec(s)?),
            None => return Err("a row without `pe`".to_owned()),
        };
        rows.push(SizingRow { up_to, pe });
    }
    if rows.last().is_none_or(|r| r.up_to.is_some()) {
        return Err("the last row must cover every larger cross-section".to_owned());
    }
    Ok(PeSizing {
        verified,
        source,
        line_types,
        rows,
    })
}

/// The cross-section in mm², when it is given in mm².
fn mm2(c: &Csa) -> Option<Dec3> {
    match c {
        Csa::Mm2(d) => Some(*d),
        _ => None,
    }
}

/// EW-POT-006: a PE wire whose cross-section is below what the sizing table requires for its associated line
/// conductors (doc 08 §8.2, FR-CHK-020, STD M-04, D-38).
///
/// - **Associated line conductors**, at each device end of the PE wire: at a terminal end, the line-type wires on
///   the same side of the same strip; at a device end, the line-type wires on the device's CPs. A line-type wire
///   is one whose potential's final type is one of the table's `line_types` (L, L1, L2, L3; L+ excluded).
/// - **Rule.** The largest associated line cross-section S (mm²) gives the minimum PE from `data/pe_sizing.toml`
///   (equal up to 16 mm², then 16, then S/2). Cross-sections in AWG or kcmil are not compared.
/// - **Unverified data** (WP-S1.16 D5, NFR-COMP-05): while the table is `verified = false`, the message says so.
/// - One finding per PE wire at the end with the largest requirement, target the wire's carrier connection.
///   Golden A: W6 (1.5 mm² at 1X1, next to the 2.5 mm² W4; EF-06).
#[derive(Clone, Copy, Debug)]
pub struct PeUndersized;

impl Rule for PeUndersized {
    fn meta(&self) -> &'static RuleMeta {
        &PE_UNDERSIZED
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        if final_type(q, p) != Some(PotentialType::Pe) {
            return Vec::new();
        }
        let Ok(table) = pe_sizing_data() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (w, props) in owned_wires(q, p) {
            if w.bond.is_some() {
                continue;
            }
            let Some(pe) = props.csa.as_ref().and_then(|c| mm2(&c.value)) else {
                continue;
            };
            // The end with the largest requirement; ties by the end's printed text.
            let mut worst: Option<(Dec3, Dec3, String)> = None;
            for end in &w.ends {
                if end.kind != EndKind::Device {
                    continue;
                }
                let Some((line, at)) = associated_line(q, table, end.cp, w.id) else {
                    continue;
                };
                let Some(required) = table.required(line) else {
                    continue;
                };
                let better = match &worst {
                    None => true,
                    Some((r, _, t)) => required > *r || (required == *r && natural_cmp(&at, t).is_lt()),
                };
                if better {
                    worst = Some((required, line, at));
                }
            }
            let Some((required, line, at)) = worst else {
                continue;
            };
            if pe >= required {
                continue;
            }
            let text = wire_text(q, &w);
            let unverified = if table.verified {
                String::new()
            } else {
                " (the sizing table is not verified against the standard)".to_owned()
            };
            out.push(Finding::new(
                &PE_UNDERSIZED,
                [Target::new(Kind::Connection, w.id)],
                "",
                text.clone(),
                format!(
                    "PE wire {text} has {pe} mm², below the {required} mm² required for the {line} mm² line \
                     conductors at {at}{unverified}"
                ),
            )
            .arg("wire", text)
            .arg("pe", pe.to_string())
            .arg("required", required.to_string())
            .arg("line", line.to_string())
            .arg("at", at)
            .arg("verified", if table.verified { "yes" } else { "no" }));
        }
        out
    }
}

/// The largest line cross-section (mm²) associated with the PE wire `pe_wire` at its end `cp` (see [`PeUndersized`]),
/// with the printed text of where: the strip at a terminal end, the device's tag at a device end.
fn associated_line(q: &Q, table: &PeSizing, cp: CpRef, pe_wire: Id<Connection>) -> Option<(Dec3, String)> {
    let device = q.functions().get(cp.function)?.device?;
    let dev = q.devices().get(device)?;
    let tag = |d: Id<Device>| {
        q.get::<DeviceTag>(d)
            .as_ref()
            .as_ref()
            .map(|t| t.short())
            .unwrap_or_else(|| "?".to_owned())
    };
    // The candidate CPs and the text of the place.
    let (cps, at): (Vec<CpRef>, String) = match &dev.aspects.terminal {
        Some(t) => {
            let side = cp_of(q, cp).and_then(|c| c.side);
            let cps = q
                .idx()
                .terminals_of_strip(t.strip)
                .flat_map(|term| q.idx().functions_of_device(term).collect::<Vec<_>>())
                .flat_map(|f| {
                    q.get::<FunctionCps>(f)
                        .iter()
                        .filter(|c| c.side == side)
                        .map(|c| CpRef::new(f, c.idx))
                        .collect::<Vec<_>>()
                })
                .collect();
            (cps, tag(t.strip))
        }
        None => {
            let cps = q
                .idx()
                .functions_of_device(device)
                .flat_map(|f| {
                    q.get::<FunctionCps>(f)
                        .iter()
                        .map(|c| CpRef::new(f, c.idx))
                        .collect::<Vec<_>>()
                })
                .collect();
            (cps, tag(device))
        }
    };
    let partition = q.get::<PotentialPartition>(());
    let mut largest: Option<Dec3> = None;
    let mut seen: BTreeSet<Id<Connection>> = BTreeSet::new();
    for c in cps {
        let Some(potential) = partition.potential_at(c.key()) else {
            continue;
        };
        let line = final_type(q, potential.id).is_some_and(|t| table.line_types.contains(&t));
        if !line {
            continue;
        }
        for conn in q.idx().connections_at(c.key()) {
            for (wire, props) in q.get::<ConnWireProps>(conn).iter() {
                if *wire == pe_wire || !seen.insert(*wire) {
                    continue;
                }
                if let Some(s) = props.csa.as_ref().and_then(|x| mm2(&x.value)) {
                    largest = Some(largest.map_or(s, |l| l.max(s)));
                }
            }
        }
    }
    largest.map(|l| (l, at))
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-008

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

/// EW-POT-008 (doc 08 §8.1, FR-CHK-055; decision log ID-185), both halves:
///
/// - **A rail-bonded terminal without a location**, from the `rail_terminal` notes (the terminal joins no location
///   PE node; since cycle 33 only in a project with two or more own locations, else it joins the implicit main
///   location). One finding per terminal, target the terminal.
/// - **More than one PE potential in one location** (FT-189): the PE potentials (final type PE) with a member CP
///   of a device in the location (its own location; without one, the location its rail would join,
///   `device_location`); one finding per location that holds two or more, targets the location node and each
///   potential's identity, discriminator `island` (the main location `island@main`), the potentials named in
///   natural order. Golden A has none (the rails R1–R3 join every PE CP); clearing `rail_bonded` on -X1032 gives
///   one, naming the two PE potentials of +J-E-2416 (D-21, MVP part).
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
        out.extend(pe_islands(q));
        out
    }
}

/// The PE-island half of EW-POT-008 (see [`PeIsland`]).
fn pe_islands(q: &Q) -> Vec<Finding> {
    let partition = q.get::<PotentialPartition>(());
    let mut by_location: BTreeMap<RailLocation, BTreeSet<PotentialId>> = BTreeMap::new();
    let mut located: BTreeMap<Id<Device>, Option<RailLocation>> = BTreeMap::new();
    for potential in partition.potentials() {
        if final_type(q, potential.id) != Some(PotentialType::Pe) {
            continue;
        }
        for cp in &potential.cps {
            let Some(device) = q.functions().get(cp.function).and_then(|f| f.device) else {
                continue;
            };
            let location = *located
                .entry(device)
                .or_insert_with(|| device_location(q, device));
            if let Some(location) = location {
                by_location.entry(location).or_default().insert(potential.id);
            }
        }
    }
    let mut out = Vec::new();
    for (location, potentials) in by_location {
        if potentials.len() < 2 {
            continue;
        }
        let mut potentials: Vec<PotentialId> = potentials.into_iter().collect();
        potentials.sort_by(|a, b| by_name(q, *a, *b));
        let names: Vec<String> = potentials.iter().map(|p| potential_name(q, *p)).collect();
        let (node, text, discriminator): (Option<Id<StructureNode>>, String, &str) = match location {
            RailLocation::Node(n) => (Some(n), location_text(q, n), "island"),
            RailLocation::Main => (None, "the main location".to_owned(), "island@main"),
        };
        let identities: Vec<Target> = potentials
            .iter()
            .flat_map(|p| potential_targets(q, *p).key)
            .collect();
        let key: Vec<Target> = node
            .map(|n| Target::new(Kind::StructureNode, n))
            .into_iter()
            .collect();
        out.push(
            Finding::new(
                &PE_ISLAND,
                key.iter().copied().chain(identities),
                discriminator,
                text.clone(),
                format!(
                    "{text} holds {} PE potentials: {}; join them through a bonded rail or a PE wire",
                    names.len(),
                    names.join(", ")
                ),
            )
            .keyed_on(&key),
        );
    }
    out
}

/// A location as a designation prints it: `+` and its path's codes (`+J-E-2416`).
fn location_text(q: &Q, node: Id<StructureNode>) -> String {
    let path = q.get::<StructurePath>(node);
    if path.codes.is_empty() {
        return "a location".to_owned();
    }
    format!("+{}", path.codes.join("+"))
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-009

/// EW-POT-009 "Unconnected CP" (doc 15 §4.3): completeness, o r, warning (unused: info), page.
pub const UNCONNECTED_CP: RuleMeta = RuleMeta {
    id: "EW-POT-009",
    code: "005009",
    pack: "EW-POT",
    title: "Unconnected CP",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-POT-009: an electrical connection point of a placed function with no connection (doc 08 §8.2, FR-CHK-018,
/// D-23). Left out: interruption points; terminal CPs (strip statistics cover them); CPs of unplaced functions;
/// non-physical CPs; optional CPs (`CpDef.optional`); CPs of another discipline (fluid ports are EW-FL-001). A CP
/// counts as connected when a connection ends at it or it is a node of the stitch graph (a bridge, a rail, a
/// declared link joins it). A CP marked `unused` is an info. Each function is checked on the first page of its
/// representations (page order), so a function drawn twice is reported once; one finding per CP, target the
/// function, discriminator the CP index, shown on that page. Golden A: -102F2:13 and :14 (EF-09).
#[derive(Clone, Copy, Debug)]
pub struct UnconnectedCp;

impl Rule for UnconnectedCp {
    fn meta(&self) -> &'static RuleMeta {
        &UNCONNECTED_CP
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(page) = *scope else {
            return Vec::new();
        };
        let order = q.get::<DrawnPageOrder>(());
        let position = |pg: Id<Page>| order.position(PageRef::Drawn(pg)).unwrap_or(usize::MAX);
        let mut functions: BTreeSet<Id<Function>> = BTreeSet::new();
        for r in q.idx().reps_by_page(page) {
            if let Some(rep) = q.reps().get(r) {
                functions.extend(rep.functions.iter().copied());
            }
        }
        let partition = q.get::<PotentialPartition>(());
        let mut out = Vec::new();
        for f in functions {
            let Some(function) = q.functions().get(f) else {
                continue;
            };
            if matches!(function.kind, FunctionKind::Ip(_)) {
                continue;
            }
            // The function's home page: the first of its pages in page order.
            let home = q
                .idx()
                .reps_of_function(f)
                .filter_map(|r| q.reps().get(r).map(|x| x.page))
                .min_by_key(|pg| (position(*pg), *pg));
            if home != Some(page) {
                continue;
            }
            let terminal = function
                .device
                .and_then(|d| q.devices().get(d))
                .is_some_and(|d| d.aspects.terminal.is_some())
                || q.get::<FunctionLinksOf>(f).terminal_contact;
            if terminal {
                continue;
            }
            for c in q.get::<FunctionCps>(f).iter() {
                if !c.physical || c.optional || c.discipline != Discipline::Electrical {
                    continue;
                }
                let at = EndpointKey::Cp(f, c.idx);
                if q.idx().connections_at(at).next().is_some() || partition.is_node(at) {
                    continue;
                }
                let label = cp_label(q, CpRef::new(f, c.idx));
                let finding = Finding::new(
                    &UNCONNECTED_CP,
                    [Target::new(Kind::Function, f)],
                    c.idx.to_string(),
                    label.clone(),
                    if c.unused {
                        format!("{label} is not connected (marked unused)")
                    } else {
                        format!("{label} is not connected; connect it or mark it unused")
                    },
                )
                .arg("cp", label)
                .arg("unused", if c.unused { "yes" } else { "no" })
                .on_page(page);
                out.push(if c.unused {
                    finding.with_severity(Severity::Info)
                } else {
                    finding
                });
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-011, EW-POT-012, EW-POT-014: wire notes

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

// ---------------------------------------------------------------------------------------------------------
// EW-POT-013

/// EW-POT-013 "N switching" (doc 15 §4.3): safety, o r, error, potential.
pub const N_SWITCHING: RuleMeta = RuleMeta {
    id: "EW-POT-013",
    code: "005013",
    pack: "EW-POT",
    title: "N switching",
    kind: RuleKind::Safety,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// The line types of the N-switching rule (doc 08 §8.2).
fn is_line(t: &PotentialType) -> bool {
    matches!(
        t,
        PotentialType::L | PotentialType::L1 | PotentialType::L2 | PotentialType::L3
    )
}

/// One opening pass-through pair of a function, oriented by the flow: the CPs and potentials upstream and
/// downstream.
#[derive(Clone, Copy, Debug)]
struct OpeningPair {
    up_idx: u16,
    down_idx: u16,
    up: PotentialId,
    down: PotentialId,
}

/// The opening pass-through pairs of `f` between two potentials, oriented by the flow (the downstream potential's
/// flow entry is reached over the pair); a pair the flow does not cross is left out.
fn opening_pairs(q: &Q, f: Id<Function>) -> Vec<OpeningPair> {
    let partition = q.get::<PotentialPartition>(());
    let flow = q.get::<FlowBfs>(());
    let mut out = Vec::new();
    for link in &q.get::<FunctionLinksOf>(f).declared {
        let ew_derive::DeclaredLink::PassThrough { a, b, opens: true } = *link else {
            continue;
        };
        let (Some(pa), Some(pb)) = (
            partition.potential_at(EndpointKey::Cp(f, a)).map(|p| p.id),
            partition.potential_at(EndpointKey::Cp(f, b)).map(|p| p.id),
        ) else {
            continue;
        };
        if pa == pb {
            continue;
        }
        let entered = |down: PotentialId, up: PotentialId, to: u16| {
            flow.entries
                .get(&down)
                .and_then(|e| e.upstream)
                .is_some_and(|u| u.function == f && u.from == up && u.to_idx == to)
        };
        if entered(pb, pa, b) {
            out.push(OpeningPair {
                up_idx: a,
                down_idx: b,
                up: pa,
                down: pb,
            });
        } else if entered(pa, pb, a) {
            out.push(OpeningPair {
                up_idx: b,
                down_idx: a,
                up: pb,
                down: pa,
            });
        }
    }
    out
}

/// EW-POT-013: an opening pass-through pair whose upstream potential is N (or PEN) without the associated L
/// (doc 08 §8.2, FR-CHK-019, D-13).
///
/// For each function with an opening pair whose upstream side (by the flow) is the potential `p` of type N or PEN:
/// it is accepted when its definition switches N with L (`switches_n_with_l`) and the function also opens a pair
/// whose upstream type is L, L1, L2 or L3; a neutral-isolating terminal (`n_isolation`) is an info; otherwise an
/// error. The loads are the devices with a CP in the downstream potential whose line CPs lie in a potential the
/// function does not switch, named in natural order. One finding per pair, target the function, discriminator
/// `pair:<up>-<down>` (the CP indices). There is no quick fix: the correct wiring is a human decision (▲U-11).
/// Golden A: T55 (N-55 → N-55T, E55 stays on L-55) and S56 (N-55 → N-55S, E56 stays on L-55) (EF-01).
#[derive(Clone, Copy, Debug)]
pub struct NSwitching;

impl Rule for NSwitching {
    fn meta(&self) -> &'static RuleMeta {
        &N_SWITCHING
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        if !matches!(final_type(q, p), Some(PotentialType::N | PotentialType::Pen)) {
            return Vec::new();
        }
        let part = potential_part(q, p);
        let Some(potential) = part.get(p) else {
            return Vec::new();
        };
        let functions: BTreeSet<Id<Function>> = potential.cps.iter().map(|c| c.function).collect();
        let mut out = Vec::new();
        for f in functions {
            let pairs = opening_pairs(q, f);
            let Some(function) = q.functions().get(f) else {
                continue;
            };
            let flags = q.fndefs().get(function.def).map(|d| d.item.flags.clone());
            let switches_n = flags.as_ref().is_some_and(|x| x.switches_n_with_l);
            let isolating = flags.as_ref().is_some_and(|x| x.n_isolation);
            let opens_line = pairs
                .iter()
                .any(|x| final_type(q, x.up).as_ref().is_some_and(is_line));
            let switched: BTreeSet<PotentialId> = pairs.iter().map(|x| x.down).collect();
            let name = function
                .device
                .map_or_else(|| "?".to_owned(), |d| full_or_short(q, d));
            for pair in pairs.iter().filter(|x| x.up == p) {
                if switches_n && opens_line {
                    continue;
                }
                let (up, down) = (potential_name(q, pair.up), potential_name(q, pair.down));
                let discriminator = format!("pair:{}-{}", pair.up_idx, pair.down_idx);
                if isolating {
                    out.push(
                        Finding::new(
                            &N_SWITCHING,
                            [Target::new(Kind::Function, f)],
                            discriminator,
                            name.clone(),
                            format!("{name} is an N-isolating terminal on {up} → {down}"),
                        )
                        .with_severity(Severity::Info),
                    );
                    continue;
                }
                let loads = stranded_loads(q, pair.down, function.device, &switched);
                let tail = if loads.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; {}",
                        loads
                            .iter()
                            .map(|(load, line)| format!("load {load} stays connected to {line}"))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                };
                let finding = Finding::new(
                    &N_SWITCHING,
                    [Target::new(Kind::Function, f)],
                    discriminator,
                    name.clone(),
                    format!("{name} switches the neutral {up} → {down}{tail}."),
                );
                // The pattern covers no stranded load and one; with more the finding keeps its en-GB text only.
                let finding = match loads.as_slice() {
                    [] => finding.arg("loads", "none"),
                    [(load, line)] => finding
                        .arg("loads", "one")
                        .arg("load", load.clone())
                        .arg("line", line.clone()),
                    _ => finding,
                };
                out.push(if loads.len() > 1 {
                    finding
                } else {
                    finding
                        .arg("device", name.clone())
                        .arg("from", up.to_string())
                        .arg("to", down.to_string())
                });
            }
        }
        out
    }
}

/// A device's tag as a report prints it (`T55`, `-102F2`).
fn full_or_short(q: &Q, d: Id<Device>) -> String {
    q.get::<DeviceTag>(d)
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .unwrap_or_else(|| "?".to_owned())
}

/// The loads of an N-switching pair: every device (other than the switching one) with a CP in `down` whose
/// line-type CPs lie in a potential that the switching function does not switch (`switched`), as `(load, line)`
/// texts in natural order.
fn stranded_loads(
    q: &Q,
    down: PotentialId,
    switching: Option<Id<Device>>,
    switched: &BTreeSet<PotentialId>,
) -> Vec<(String, String)> {
    let partition = q.get::<PotentialPartition>(());
    let Some(potential) = partition.get(down) else {
        return Vec::new();
    };
    let devices: BTreeSet<Id<Device>> = potential
        .cps
        .iter()
        .filter_map(|c| q.functions().get(c.function).and_then(|f| f.device))
        .filter(|d| Some(*d) != switching)
        .filter(|d| q.devices().get(*d).is_some_and(|x| x.aspects.terminal.is_none()))
        .collect();
    let mut out: BTreeSet<(String, String)> = BTreeSet::new();
    for d in devices {
        for f in q.idx().functions_of_device(d) {
            for c in q.get::<FunctionCps>(f).iter() {
                let Some(line) = partition.potential_at(EndpointKey::Cp(f, c.idx)).map(|x| x.id) else {
                    continue;
                };
                if switched.contains(&line) || !final_type(q, line).as_ref().is_some_and(is_line) {
                    continue;
                }
                out.insert((full_or_short(q, d), potential_name(q, line)));
            }
        }
    }
    let mut out: Vec<(String, String)> = out.into_iter().collect();
    out.sort_by(|(a, x), (b, y)| {
        natural_cmp(a, b)
            .then_with(|| a.cmp(b))
            .then_with(|| natural_cmp(x, y))
            .then_with(|| x.cmp(y))
    });
    out
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-014

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

// ---------------------------------------------------------------------------------------------------------
// EW-POT-015

/// EW-POT-015 "Wire-number conflict" (doc 15 §4.3): conflict, o r, error, project.
pub const WIRE_NUMBER_CONFLICT: RuleMeta = RuleMeta {
    id: "EW-POT-015",
    code: "005015",
    pack: "EW-POT",
    title: "Wire-number conflict",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-POT-015: one printed wire number on two numbering units, or two numbers within one unit (doc 08 §5.3,
/// FR-NUM-019, FR-NUM-020). The numbering unit of the shipped scheme is the physical wire (connection-oriented
/// `{page}.{counter}`), and a wire carries one derived number, so the rule checks the first half: a label printed
/// on two or more wires is one finding, targets those wires' carrier connections. The derivation gives unique
/// labels; stored and frozen numbers (FR-NUM-011, FR-NUM-022) and the other numbering units arrive with the
/// numbering schemes of FR-NUM-020, which this rule then covers unchanged. Golden A: none.
#[derive(Clone, Copy, Debug)]
pub struct WireNumberConflict;

impl Rule for WireNumberConflict {
    fn meta(&self) -> &'static RuleMeta {
        &WIRE_NUMBER_CONFLICT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let numbers = q.get::<WireNumbers>(());
        duplicate_numbers(
            numbers
                .wires
                .iter()
                .filter_map(|w| w.number.as_ref().map(|n| (n.label.clone(), w.wire))),
        )
    }
}

/// The EW-POT-015 findings of `(label, wire)` pairs: one per label on two or more wires, in label order.
pub(crate) fn duplicate_numbers(numbers: impl IntoIterator<Item = (String, Id<Connection>)>) -> Vec<Finding> {
    let mut by_label: BTreeMap<String, BTreeSet<Id<Connection>>> = BTreeMap::new();
    for (label, wire) in numbers {
        by_label.entry(label).or_default().insert(wire);
    }
    by_label
        .into_iter()
        .filter(|(_, wires)| wires.len() > 1)
        .map(|(label, wires)| {
            Finding::new(
                &WIRE_NUMBER_CONFLICT,
                wires.iter().map(|w| Target::new(Kind::Connection, *w)),
                "",
                label.clone(),
                format!(
                    "wire number {label} is printed on {} wires; a wire number names one numbering unit",
                    wires.len()
                ),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-016

/// EW-POT-016 "Redundant override" (doc 15 §4.3): hygiene, o, info, potential.
pub const REDUNDANT_OVERRIDE: RuleMeta = RuleMeta {
    id: "EW-POT-016",
    code: "005016",
    pack: "EW-POT",
    title: "Redundant override",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-POT-016: a wire override (level 7, CDP) equal to the value below it in the inheritance chain (doc 08 §4.1,
/// §8.1, FR-CON-025, D-08): removing it changes nothing. One finding per wire and member (cross-section, colour,
/// wire part, insulation, function, shielded, twisted), target the connection that carries the override,
/// discriminator the member. Golden A: none (its CDPs set cross-sections the profile does not give).
#[derive(Clone, Copy, Debug)]
pub struct RedundantOverride;

impl Rule for RedundantOverride {
    fn meta(&self) -> &'static RuleMeta {
        &REDUNDANT_OVERRIDE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (w, props) in owned_wires(q, p) {
            let o = &w.overrides;
            let any = o.csa.is_some()
                || o.colour.is_some()
                || o.wire_part.is_some()
                || o.insulation.is_some()
                || o.function.is_some()
                || o.flags.shielded
                || o.flags.twisted;
            if !any {
                continue;
            }
            let below = wire_props_below_overrides(q, p, &w);
            let s = &w.override_sources;
            let text = wire_text(q, &w);
            let mut redundant: Vec<(&str, Option<Id<Connection>>, String)> = Vec::new();
            let same = |a: Option<String>, b: Option<String>| a.is_some() && a == b;
            if o.csa.is_some() && props.csa.as_ref().map(|x| &x.value) == below.csa.as_ref().map(|x| &x.value)
            {
                let v = o.csa.as_ref().and_then(csa_text).unwrap_or_default();
                redundant.push(("csa", s.csa, format!("cross-section {v}")));
            }
            if o.colour.is_some()
                && same(
                    props.colour.as_ref().and_then(|x| colour_text(&x.value)),
                    below.colour.as_ref().and_then(|x| colour_text(&x.value)),
                )
            {
                let v = o.colour.as_ref().and_then(colour_text).unwrap_or_default();
                redundant.push(("colour", s.colour, format!("colour {v}")));
            }
            if o.wire_part.is_some()
                && props.wire_part.as_ref().map(|x| &x.value) == below.wire_part.as_ref().map(|x| &x.value)
            {
                redundant.push(("wire_part", s.wire_part, "wire part".to_owned()));
            }
            if o.insulation.is_some()
                && props.insulation.as_ref().map(|x| &x.value) == below.insulation.as_ref().map(|x| &x.value)
            {
                redundant.push(("insulation", s.insulation, "insulation".to_owned()));
            }
            if o.function.is_some()
                && props.function.as_ref().map(|x| &x.value) == below.function.as_ref().map(|x| &x.value)
            {
                redundant.push(("function", s.function, "wire function".to_owned()));
            }
            if o.flags.shielded && below.shielded.as_ref().is_some_and(|x| x.value) {
                redundant.push(("shielded", s.shielded, "shielded".to_owned()));
            }
            if o.flags.twisted && below.twisted.as_ref().is_some_and(|x| x.value) {
                redundant.push(("twisted", s.twisted, "twisted".to_owned()));
            }
            for (member, source, what) in redundant {
                let at = source.unwrap_or(w.id);
                out.push(Finding::new(
                    &REDUNDANT_OVERRIDE,
                    [Target::new(Kind::Connection, at)],
                    member,
                    text.clone(),
                    format!(
                        "the override {what} of wire {text} equals the value below it in the inheritance chain; \
                         remove the override"
                    ),
                ));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-017

/// EW-POT-017 "Two potential names" (doc 15 §4.3): hygiene, o r, warning, potential.
pub const TWO_POTENTIAL_NAMES: RuleMeta = RuleMeta {
    id: "EW-POT-017",
    code: "005017",
    pack: "EW-POT",
    title: "Two potential names",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-POT-017: definitions with different names in one potential (doc 08 §3.5, FR-CON-012, D-24); the first in
/// natural order names the potential (D9). Before, a derivation note only. One finding per potential, every
/// definition a target, keyed on the potential's identity, discriminator `names`, the names in natural order.
#[derive(Clone, Copy, Debug)]
pub struct TwoPotentialNames;

impl Rule for TwoPotentialNames {
    fn meta(&self) -> &'static RuleMeta {
        &TWO_POTENTIAL_NAMES
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p @ PotentialId::Def(_)) = *scope else {
            return Vec::new();
        };
        let part = potential_part(q, p);
        let Some(potential) = part.get(p) else {
            return Vec::new();
        };
        let defs = ranked_defs(q, potential);
        let mut names: Vec<String> = Vec::new();
        for (_, d) in &defs {
            let n = d.name.as_str().to_owned();
            if !names.contains(&n) {
                names.push(n);
            }
        }
        if names.len() < 2 {
            return Vec::new();
        }
        let pt = potential_targets(q, p);
        vec![pt.finding(
            &TWO_POTENTIAL_NAMES,
            &[],
            "names".to_owned(),
            names[0].clone(),
            format!(
                "the definitions of one potential name it {}; the first in natural order ({}) applies",
                names.join(", "),
                names[0]
            ),
        )]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-021

/// EW-POT-021 "Load not returned" (doc 15 §4.4, FT-661): completeness, o r, warning, potential.
pub const LOAD_NOT_RETURNED: RuleMeta = RuleMeta {
    id: "EW-POT-021",
    code: "005021",
    pack: "EW-POT",
    title: "Load not returned",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::Mvp,
};

/// Whether a type is a protective or functional earth (PEN is a return conductor): an earth potential is never a
/// return path, so neither its CPs nor an earth potential count (EW-POT-007 checks PE continuity).
fn is_earth(t: &PotentialType) -> bool {
    matches!(t, PotentialType::Pe | PotentialType::Fe)
}

/// The CPs of a two-terminal load (a coil, a lamp, a heater): its physical electrical CPs without an earth role, when
/// there are exactly two; `None` for any other function.
fn two_terminal(q: &Q, f: Id<Function>) -> Option<[u16; 2]> {
    let cps: Vec<u16> = q
        .get::<FunctionCps>(f)
        .iter()
        .filter(|c| {
            c.physical
                && c.discipline == Discipline::Electrical
                && !c.potential_role.as_ref().is_some_and(is_earth)
        })
        .map(|c| c.idx)
        .collect();
    match cps.as_slice() {
        [a, b] => Some([*a, *b]),
        _ => None,
    }
}

/// Whether a function only passes a potential on: a terminal (its device's terminal aspect or a terminal contact).
fn is_terminal(q: &Q, f: Id<Function>) -> bool {
    q.functions()
        .get(f)
        .and_then(|x| x.device)
        .and_then(|d| q.devices().get(d))
        .is_some_and(|d| d.aspects.terminal.is_some())
        || q.get::<FunctionLinksOf>(f).terminal_contact
}

/// Whether the potential `p` has no source (EW-POT-021): the flow does not reach it (`potential_flow` distance ∞), its
/// type is not an earth, no member CP carries a potential role (the outputs of a supply, the return pin of a module,
/// as their CP data declares them), and every member other than an interruption point or a terminal is a two-terminal
/// load. A potential that holds a CP of any other function (a PLC module, a power supply, a switch) is not judged:
/// that function may be its source. At least one load must be on it.
fn sourceless(q: &Q, p: PotentialId) -> bool {
    let entry = q.get::<PotentialFlow>(p);
    let Some(entry) = entry.as_ref().as_ref() else {
        return false;
    };
    if entry.distance.is_some() || entry.ptype.as_ref().is_some_and(|a| is_earth(&a.value)) {
        return false;
    }
    let part = potential_part(q, p);
    let Some(potential) = part.get(p) else {
        return false;
    };
    let mut loads = 0usize;
    for m in &potential.cps {
        if potential.ips.binary_search(&m.function).is_ok() || is_terminal(q, m.function) {
            continue;
        }
        let cps = q.get::<FunctionCps>(m.function);
        if cps.iter().any(|c| c.idx == m.idx && c.potential_role.is_some()) {
            return false;
        }
        if !entry.loads.iter().any(|l| l.function == m.function) || two_terminal(q, m.function).is_none() {
            return false;
        }
        loads += 1;
    }
    loads > 0
}

/// EW-POT-021: a load fed on one side whose other side reaches no source, an unreturned coil (FT-661; doc 08 §3.6).
/// `scope` is a potential with no source (`sourceless`: unreached by the flow, no CP with a potential role, only
/// two-terminal loads besides terminals and interruption points), for example the A2 net of two valve coils that was
/// never linked to 0 V. Each two-terminal load on it whose other CP lies on a potential that has a source is reported
/// once, target the load function, discriminator the CP index on `scope`, subject the CP's label. A load whose two
/// sides both have no source is left out (not fed at all); an unconnected CP has no potential (EW-POT-009). A return
/// inside a module counts as returned: the module's declared link puts the CP in the module's potential, which holds
/// a module CP and so is not judged. The flow alone cannot decide this: a supply whose CP data declares no source
/// output (the starter PSU, Golden A's -102G1) leaves its whole DC side unreached.
#[derive(Clone, Copy, Debug)]
pub struct LoadNotReturned;

impl Rule for LoadNotReturned {
    fn meta(&self) -> &'static RuleMeta {
        &LOAD_NOT_RETURNED
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        if !sourceless(q, p) {
            return Vec::new();
        }
        let part = potential_part(q, p);
        let Some(potential) = part.get(p) else {
            return Vec::new();
        };
        let name = potential_name(q, p);
        let mut out = Vec::new();
        let functions: BTreeSet<Id<Function>> = potential.cps.iter().map(|c| c.function).collect();
        for f in functions {
            let Some(cps) = two_terminal(q, f) else {
                continue;
            };
            for (here, there) in [(cps[0], cps[1]), (cps[1], cps[0])] {
                if !potential.cps.contains(&CpRef::new(f, here)) {
                    continue;
                }
                let Some(other) = potential_id_at(q, EndpointKey::Cp(f, there)) else {
                    continue;
                };
                if other == p || sourceless(q, other) {
                    continue;
                }
                let label = cp_label(q, CpRef::new(f, here));
                out.push(
                    Finding::new(
                        &LOAD_NOT_RETURNED,
                        [Target::new(Kind::Function, f)],
                        here.to_string(),
                        label.clone(),
                        format!(
                            "{label} is on {name}, which reaches no supply: the load is fed but not returned; \
                             connect it to the return conductor (0 V, N)"
                        ),
                    )
                    .arg("cp", label)
                    .arg("potential", name.clone()),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_pe_sizing_table_parses_and_is_unverified() {
        let t = pe_sizing();
        assert!(!t.verified, "NFR-COMP-05: nobody has checked the values yet");
        assert!(t.source.contains("IEC 60364-5-54"));
        assert_eq!(
            t.line_types,
            vec![
                PotentialType::L,
                PotentialType::L1,
                PotentialType::L2,
                PotentialType::L3
            ]
        );
        let d = |s: &str| s.parse::<Dec3>().unwrap();
        assert_eq!(t.required(d("1.5")), Some(d("1.5")));
        assert_eq!(t.required(d("16")), Some(d("16")));
        assert_eq!(t.required(d("25")), Some(d("16")));
        assert_eq!(t.required(d("35")), Some(d("16")));
        assert_eq!(t.required(d("50")), Some(d("25")));
        assert_eq!(t.required(d("120")), Some(d("60")));
    }

    #[test]
    fn a_sizing_table_without_an_open_last_row_is_refused() {
        let text = "verified = false\nsource = \"x\"\nline_types = [\"l\"]\n[[rows]]\nup_to_mm2 = \"16\"\npe = \"s\"\n";
        assert!(parse_pe_sizing(text).is_err());
        assert!(parse_pe_sizing("verified = true").is_err());
    }

    #[test]
    fn duplicate_wire_numbers_are_one_finding_per_label() {
        let w = |n: u128| Id::<Connection>::from_uuid(ew_derive::ew_ids::Uuid::from_u128(n));
        let f = duplicate_numbers([
            ("101.2".to_owned(), w(2)),
            ("101.1".to_owned(), w(1)),
            ("101.2".to_owned(), w(3)),
            ("101.3".to_owned(), w(4)),
        ]);
        assert_eq!(f.len(), 1, "{f:#?}");
        assert_eq!(f[0].subject, "101.2");
        assert_eq!(f[0].targets.len(), 2);
        assert_eq!(
            f[0].message,
            "wire number 101.2 is printed on 2 wires; a wire number names one numbering unit"
        );
        assert!(duplicate_numbers([("1.1".to_owned(), w(1)), ("1.1".to_owned(), w(1))]).is_empty());
    }
}
