// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The treatment of a wire's ends (doc 13 §4): what the fitter puts on each end of a wire, by the rule table
//! [`TREATMENT_RULES`] over the end's clamp and the wire's conductor class, the pairing of two wires of one size on one
//! clamp into a twin ferrule (§4.3) and the part of each end from the workshop stock (§4.4). [`treat`] fills the ends
//! of the records of `wire_record_set()`; [`clamp_conductors`] counts the conductors each clamp takes for EW-POT-010.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use ew_ids::Id;
use ew_schema::{ClampType, Connection, Csa, Endpoint, FerruleRule, Part, TwoWires};
use ew_units::Dec3;

use crate::cps::{ClampData, FunctionCps};
use crate::nets::CpRef;
use crate::q::Q;
use crate::wire_record::WireRecord;
use crate::wires::{EndKind, PhysicalWire};

/// What the fitter puts on an end (doc 13 §2.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EndTreatment {
    /// A ferrule of the wire's size (mm²; `None` for a size in another unit or none).
    Ferrule {
        /// The size.
        csa: Option<Dec3>,
    },
    /// A twin ferrule, the wire's and its partner's of one size on one clamp (§4.3).
    TwinFerrule {
        /// The size.
        csa: Option<Dec3>,
        /// The partner wire (its ID).
        partner: Id<Connection>,
    },
    /// A ring lug, for a ring stud.
    RingLug,
    /// Nothing: the clamp takes the wire as it is.
    Bare,
    /// A connector's own contact, which comes with the connector.
    Connector,
    /// An end made outside the project (an external end, or an interruption point without a partner).
    External,
    /// The table cannot decide: the end cannot be made as drawn (EW-MFG-008).
    Unknown {
        /// Why.
        reason: Undecided,
    },
}

/// Why the rule table cannot decide an end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Undecided {
    /// A fine-stranded wire into an insulation-displacement clamp (row 3).
    FineIntoIdc,
}

/// Who decided an end's treatment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreatedBy {
    /// A row of [`TREATMENT_RULES`] (its number), under the assumptions it made; the pairing of §4.3 keeps the row.
    Rule {
        /// The row's number (1 …).
        row: u8,
        /// What it assumed.
        assumed: Assumed,
    },
    /// Nothing decides it: the end is made outside the project.
    Outside,
}

/// The assumptions a rule made for an end (doc 13 §4.2), said in Properties.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Assumed {
    /// The wire's conductor class: no wire part names it, taken as fine-stranded.
    pub class: bool,
    /// The clamp's kind: its part names none, taken as a screw or spring clamp.
    pub clamp: bool,
}

/// The clamps a row of the rule table matches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClampMatch {
    /// A connector's pin.
    Plug,
    /// A ring stud.
    RingStud,
    /// An insulation-displacement clamp.
    Idc,
    /// A spring or push-in clamp.
    SpringOrPushIn,
    /// A screw, spring or push-in clamp, or one whose kind is not known.
    Other,
}

/// The conductors a row of the rule table matches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConductorMatch {
    /// Any.
    Any,
    /// Fine-stranded: IEC 60228 class 5 or 6, or no class named.
    Fine,
    /// Solid or stranded: class 1 or 2.
    SolidOrStranded,
}

/// What a row of the rule table gives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleGives {
    /// A connector's own contact.
    Connector,
    /// A ring lug.
    RingLug,
    /// Undecided.
    Unknown(Undecided),
    /// Nothing.
    Bare,
    /// A ferrule of the wire's size, or a twin ferrule by the pairing.
    Ferrule,
}

/// One row of the rule table (doc 13 §4.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TreatmentRule {
    /// The row's number.
    pub row: u8,
    /// The clamps it matches.
    pub clamp: ClampMatch,
    /// The conductors it matches.
    pub conductor: ConductorMatch,
    /// Whether it applies only where the shop sets "not into spring or push-in clamps".
    pub not_into_spring_clamps: bool,
    /// What it gives.
    pub gives: RuleGives,
    /// Its source, as doc 13 §4.2 cites it.
    pub source: &'static str,
    /// Whether a panel builder has confirmed it against his shop's practice: no row is yet (FR-CHK-046).
    pub verified: bool,
}

/// The rule table (doc 13 §4.2): the first row that matches an end gives its treatment.
pub const TREATMENT_RULES: [TreatmentRule; 7] = [
    TreatmentRule {
        row: 1,
        clamp: ClampMatch::Plug,
        conductor: ConductorMatch::Any,
        not_into_spring_clamps: false,
        gives: RuleGives::Connector,
        source: "the clamp is a contact, not a clamp",
        verified: false,
    },
    TreatmentRule {
        row: 2,
        clamp: ClampMatch::RingStud,
        conductor: ConductorMatch::Any,
        not_into_spring_clamps: false,
        gives: RuleGives::RingLug,
        source: "a stud takes a closed lug (DIN 46234 ring lugs, verify)",
        verified: false,
    },
    TreatmentRule {
        row: 3,
        clamp: ClampMatch::Idc,
        conductor: ConductorMatch::Fine,
        not_into_spring_clamps: false,
        gives: RuleGives::Unknown(Undecided::FineIntoIdc),
        source: "IDC clamps take solid and stranded conductors only",
        verified: false,
    },
    TreatmentRule {
        row: 4,
        clamp: ClampMatch::Idc,
        conductor: ConductorMatch::SolidOrStranded,
        not_into_spring_clamps: false,
        gives: RuleGives::Bare,
        source: "IDC clamps take solid and stranded conductors only",
        verified: false,
    },
    TreatmentRule {
        row: 5,
        clamp: ClampMatch::SpringOrPushIn,
        conductor: ConductorMatch::Fine,
        not_into_spring_clamps: true,
        gives: RuleGives::Bare,
        source: "the clamp holds fine strands itself (its maker's data)",
        verified: false,
    },
    TreatmentRule {
        row: 6,
        clamp: ClampMatch::Other,
        conductor: ConductorMatch::Fine,
        not_into_spring_clamps: false,
        gives: RuleGives::Ferrule,
        source: "DIN 46228 ferrules; EN 60204-1 13.1.1 (verify the clause): the strands held together",
        verified: false,
    },
    TreatmentRule {
        row: 7,
        clamp: ClampMatch::Other,
        conductor: ConductorMatch::SolidOrStranded,
        not_into_spring_clamps: false,
        gives: RuleGives::Bare,
        source: "a clamp takes them without a sleeve (IEC 60999-1, verify)",
        verified: false,
    },
];

/// The row of the rule table that decides an end on a clamp of `clamp` (its part's data, `None` where it names
/// none) for a wire of conductor class `class` (`None`: none named), under the shop's ferrule rule.
pub fn rule_for(
    clamp: Option<&ClampData>,
    class: Option<u8>,
    ferrules: &FerruleRule,
) -> (&'static TreatmentRule, Assumed) {
    let kind = clamp.and_then(|c| c.kind.as_ref());
    let solid = matches!(class, Some(1 | 2));
    let not_into_spring = *ferrules == FerruleRule::NotIntoSpringClamps;
    let rule = TREATMENT_RULES
        .iter()
        .find(|r| {
            let clamp_ok = match r.clamp {
                ClampMatch::Plug => kind == Some(&ClampType::Plug),
                ClampMatch::RingStud => kind == Some(&ClampType::RingStud),
                ClampMatch::Idc => kind == Some(&ClampType::Idc),
                ClampMatch::SpringOrPushIn => matches!(kind, Some(ClampType::Spring | ClampType::PushIn)),
                ClampMatch::Other => true,
            };
            let conductor_ok = match r.conductor {
                ConductorMatch::Any => true,
                ConductorMatch::Fine => !solid,
                ConductorMatch::SolidOrStranded => solid,
            };
            clamp_ok && conductor_ok && (!r.not_into_spring_clamps || not_into_spring)
        })
        .unwrap_or(&TREATMENT_RULES[TREATMENT_RULES.len() - 1]);
    let assumed = Assumed {
        class: class.is_none() && rule.conductor != ConductorMatch::Any,
        clamp: kind.is_none() && rule.clamp == ClampMatch::Other,
    };
    (rule, assumed)
}

/// The wire's size in mm², for a size given in mm².
fn mm2(r: &WireRecord) -> Option<Dec3> {
    match r.csa.as_ref().map(|c| &c.value) {
        Some(Csa::Mm2(d)) => Some(*d),
        _ => None,
    }
}

/// The clamp data of a CP.
fn clamp_of(q: &Q, cp: CpRef) -> Option<ClampData> {
    q.get::<FunctionCps>(cp.function)
        .iter()
        .find(|c| c.idx == cp.idx)
        .and_then(|c| c.clamp_data.clone())
}

/// Fills the treatment, the part and who decided it of every end of `records` (doc 13 §4): the rule table per end, then
/// the pairing per clamp (§4.3), then the part from the workshop stock (§4.4).
pub(crate) fn treat(q: &Q, records: &mut [WireRecord]) {
    let project = q.project();
    let workshop = &project.settings.workshop;
    let parts = q.parts();
    let class_of = |r: &WireRecord| {
        r.wire_part
            .as_ref()
            .and_then(|p| parts.get(p.copy))
            .and_then(|c| c.item.sections.wire.as_ref())
            .and_then(|w| w.class)
    };
    let mut clamps: BTreeMap<CpRef, Option<ClampData>> = BTreeMap::new();
    for r in records.iter_mut() {
        let csa = mm2(r);
        let class = class_of(r);
        let wire_ends = r.wire.ends;
        for (i, end) in wire_ends.iter().enumerate() {
            let (treatment, by) = if end.kind == EndKind::Device {
                let clamp = clamps.entry(end.cp).or_insert_with(|| clamp_of(q, end.cp));
                let (rule, assumed) = rule_for(clamp.as_ref(), class, &workshop.ferrules);
                let treatment = match rule.gives {
                    RuleGives::Connector => EndTreatment::Connector,
                    RuleGives::RingLug => EndTreatment::RingLug,
                    RuleGives::Unknown(reason) => EndTreatment::Unknown { reason },
                    RuleGives::Bare => EndTreatment::Bare,
                    RuleGives::Ferrule => EndTreatment::Ferrule { csa },
                };
                (
                    treatment,
                    TreatedBy::Rule {
                        row: rule.row,
                        assumed,
                    },
                )
            } else {
                (EndTreatment::External, TreatedBy::Outside)
            };
            r.ends[i].treatment = treatment;
            r.ends[i].treated_by = by;
        }
    }
    pair(records, &clamps, &workshop.two_wires);
    for r in records.iter_mut() {
        let csa = mm2(r);
        for end in &mut r.ends {
            end.part = stock_part(q, &end.treatment, csa);
        }
    }
}

/// The pairing of doc 13 §4.3: on every CP two or more ferrule ends land on, sorted by size (largest first), number,
/// wire ID, pairs of one size into twin ferrules, greedily, where the clamp takes a twin ferrule (or its part does not
/// say) and the shop's `two_wires` is a twin ferrule.
fn pair(records: &mut [WireRecord], clamps: &BTreeMap<CpRef, Option<ClampData>>, two_wires: &TwoWires) {
    if *two_wires != TwoWires::TwinFerrule {
        return;
    }
    let mut at: BTreeMap<CpRef, Vec<(usize, usize)>> = BTreeMap::new();
    for (n, r) in records.iter().enumerate() {
        for (i, end) in r.wire.ends.iter().enumerate() {
            if end.kind == EndKind::Device && matches!(r.ends[i].treatment, EndTreatment::Ferrule { .. }) {
                at.entry(end.cp).or_default().push((n, i));
            }
        }
    }
    for (cp, mut ends) in at {
        if ends.len() < 2
            || clamps
                .get(&cp)
                .is_some_and(|c| c.as_ref().is_some_and(|c| !c.twin_ferrule))
        {
            continue;
        }
        ends.sort_by(|a, b| {
            let (ra, rb) = (&records[a.0], &records[b.0]);
            mm2(rb)
                .cmp(&mm2(ra))
                .then_with(|| natural(ra.number.as_deref(), rb.number.as_deref()))
                .then_with(|| ra.wire.id.cmp(&rb.wire.id))
        });
        let mut taken = vec![false; ends.len()];
        for x in 0..ends.len() {
            if taken[x] {
                continue;
            }
            let size = mm2(&records[ends[x].0]);
            let Some(y) = (x + 1..ends.len())
                .find(|y| !taken[*y] && size.is_some() && mm2(&records[ends[*y].0]) == size)
            else {
                continue;
            };
            taken[x] = true;
            taken[y] = true;
            let (a, b) = (ends[x], ends[y]);
            let (ida, idb) = (records[a.0].wire.id, records[b.0].wire.id);
            records[a.0].ends[a.1].treatment = EndTreatment::TwinFerrule {
                csa: size,
                partner: idb,
            };
            records[b.0].ends[b.1].treatment = EndTreatment::TwinFerrule {
                csa: size,
                partner: ida,
            };
        }
    }
}

/// Two wire numbers in natural order, a wire without one last.
fn natural(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => ew_mltext::natural_key(a).cmp(&ew_mltext::natural_key(b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// The part of an end (doc 13 §4.4): a ferrule end the first stock ferrule without a stud that holds its size and is a
/// twin ferrule for a twin one and not for a single one; a ring lug the first stock ferrule part with a stud whose
/// range holds the wire's size; else none.
fn stock_part(q: &Q, treatment: &EndTreatment, csa: Option<Dec3>) -> Option<Id<Part>> {
    let (twin, stud) = match treatment {
        EndTreatment::Ferrule { .. } => (false, false),
        EndTreatment::TwinFerrule { .. } => (true, false),
        EndTreatment::RingLug => (false, true),
        _ => return None,
    };
    let csa = csa?;
    let parts = q.parts();
    q.project()
        .settings
        .workshop
        .stock
        .iter()
        .map(|r| r.copy)
        .find(|id| {
            parts
                .get(*id)
                .and_then(|c| c.item.sections.ferrule.as_ref())
                .is_some_and(|f| {
                    f.stud.is_some() == stud
                        && (stud || f.twin == twin)
                        && f.csa_min <= csa
                        && csa <= f.csa_max
                })
        })
}

/// The conductors each clamp takes (doc 13 §4.3): the wire ends that land on it, a twin ferrule's two wires counted
/// once; what EW-POT-010 compares with the clamp's maximum.
pub fn clamp_conductors(records: &[WireRecord]) -> BTreeMap<CpRef, usize> {
    let mut out: BTreeMap<CpRef, (usize, usize)> = BTreeMap::new();
    for r in records {
        for (i, end) in r.wire.ends.iter().enumerate() {
            if end.kind != EndKind::Device {
                continue;
            }
            let e = out.entry(end.cp).or_default();
            match r.ends[i].treatment {
                EndTreatment::TwinFerrule { .. } => e.1 += 1,
                _ => e.0 += 1,
            }
        }
    }
    out.into_iter()
        .map(|(cp, (single, twin))| (cp, single + twin.div_ceil(2)))
        .collect()
}

/// The connection of `wire` that lands on its end `end` (0 the From end, 1 the To end) and whether it lands there by its
/// endpoint `a`: where an end set by hand is stored (doc 13 §2.3: in `end_a` when that connection's endpoint `a` is the
/// wire's end, else in `end_b`), and what a finding about the end targets.
pub fn end_connection(q: &Q, wire: &PhysicalWire, end: usize) -> Option<(Id<Connection>, bool)> {
    let cp = wire.ends.get(end)?.cp;
    let at = Endpoint::Cp(cp.function, cp.idx);
    let conns = q.connections();
    wire.constituents.iter().find_map(|c| {
        let row = conns.get(*c)?;
        if row.a == at {
            Some((*c, true))
        } else if row.b == at {
            Some((*c, false))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clamp(kind: ClampType) -> ClampData {
        ClampData {
            kind: Some(kind),
            csa: (None, None),
            max_conductors: None,
            twin_ferrule: false,
        }
    }

    /// A case of the table: the clamp, the class, the shop's ferrule rule, the row that decides and what it assumes.
    type Case<'a> = (Option<ClampData>, Option<u8>, &'a FerruleRule, u8, Assumed);

    /// Each row of the table decides its own case, and says what it assumed.
    #[test]
    fn each_row_of_the_rule_table_decides_its_case() {
        let every = FerruleRule::EveryFineStrandedEnd;
        let spring = FerruleRule::NotIntoSpringClamps;
        let cases: [Case<'_>; 9] = [
            (
                Some(clamp(ClampType::Plug)),
                Some(5),
                &every,
                1,
                Assumed::default(),
            ),
            (
                Some(clamp(ClampType::RingStud)),
                None,
                &every,
                2,
                Assumed::default(),
            ),
            (
                Some(clamp(ClampType::Idc)),
                Some(5),
                &every,
                3,
                Assumed::default(),
            ),
            (
                Some(clamp(ClampType::Idc)),
                Some(1),
                &every,
                4,
                Assumed::default(),
            ),
            (
                Some(clamp(ClampType::PushIn)),
                Some(6),
                &spring,
                5,
                Assumed::default(),
            ),
            (
                Some(clamp(ClampType::PushIn)),
                Some(6),
                &every,
                6,
                Assumed::default(),
            ),
            (
                Some(clamp(ClampType::Screw)),
                None,
                &every,
                6,
                Assumed {
                    class: true,
                    clamp: false,
                },
            ),
            (
                None,
                Some(2),
                &every,
                7,
                Assumed {
                    class: false,
                    clamp: true,
                },
            ),
            (
                None,
                None,
                &spring,
                6,
                Assumed {
                    class: true,
                    clamp: true,
                },
            ),
        ];
        for (clamp, class, rule, row, assumed) in cases {
            let (r, a) = rule_for(clamp.as_ref(), class, rule);
            assert_eq!((r.row, a), (row, assumed), "{clamp:?} class {class:?} {rule:?}");
        }
        assert!(
            TREATMENT_RULES
                .iter()
                .all(|r| !r.verified && !r.source.is_empty())
        );
    }
}
