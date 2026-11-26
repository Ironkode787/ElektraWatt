// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of a wire's ends for the workshop (doc 13 §4.6), over the treatment of each end that `wire_record_set()`
//! gives: EW-MFG-008, an end the rule table cannot decide; EW-MFG-009, an end set by hand that does not fit; and
//! EW-POT-010, a clamp that takes fewer conductors than land on it, which reads the same pairing (a twin ferrule is one
//! conductor).

use ew_derive::{
    CpRef, EndTreatment, FunctionCps, Q, TreatedBy, Undecided, WireRecordSet, clamp_conductors, cp_label,
    stock_part,
};
use ew_l10n::TextArg;
use ew_schema::Kind;

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::pot::{owns_wire, wire_text};

// ---------------------------------------------------------------------------------------------------------
// EW-MFG-008

/// EW-MFG-008 "End cannot be made" (doc 15 §4.5.5): completeness, o r, warning, potential.
pub const END_CANNOT_BE_MADE: RuleMeta = RuleMeta {
    id: "EW-MFG-008",
    message: key!("rule-ew-mfg-008"),
    code: "140008",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-MFG-008: an end of a wire the rule table cannot decide (doc 13 §4.2 row 3: a fine-stranded wire into an
/// insulation-displacement clamp). One finding per end, target the connection that lands on it, discriminator the end
/// (`from`, `to`); a bond wire reported by the potential that prints first.
#[derive(Clone, Copy, Debug)]
pub struct EndCannotBeMade;

impl Rule for EndCannotBeMade {
    fn meta(&self) -> &'static RuleMeta {
        &END_CANNOT_BE_MADE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for r in q
            .get::<WireRecordSet>(())
            .iter()
            .filter(|r| owns_wire(q, p, &r.wire))
        {
            for (i, end) in r.ends.iter().enumerate() {
                let EndTreatment::Unknown { reason } = end.treatment else {
                    continue;
                };
                let Some((connection, _)) = ew_derive::end_connection(q, &r.wire, i) else {
                    continue;
                };
                let wire = wire_text(q, &r.wire);
                let at = cp_label(q, r.wire.ends[i].cp);
                let reason = match reason {
                    Undecided::FineIntoIdc => "fine_into_idc",
                };
                out.push(
                    Finding::new(
                        &END_CANNOT_BE_MADE,
                        [Target::new(Kind::Connection, connection)],
                        if i == 0 { "from" } else { "to" },
                        format!("{wire} {at}"),
                    )
                    .arg("wire", wire)
                    .arg("at", at)
                    .arg("reason", reason),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-MFG-009

/// EW-MFG-009 "End set by hand does not fit" (doc 15 §4.5.5): conflict, o r, warning, potential.
pub const END_SET_BY_HAND_MISFITS: RuleMeta = RuleMeta {
    id: "EW-MFG-009",
    message: key!("rule-ew-mfg-009"),
    code: "140009",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-MFG-009: an end set by hand (doc 13 §4.5) that does not fit: its ferrule part takes another size than the wire's,
/// a twin ferrule that no second wire on its clamp shares, or a ferrule part of another colour than the workshop
/// stock's ferrule for that end. One finding per end, target the connection that holds it, discriminator the end
/// (`from`, `to`); its quick fix, Reset to the rule, clears the end set by hand.
#[derive(Clone, Copy, Debug)]
pub struct EndSetByHandMisfits;

impl Rule for EndSetByHandMisfits {
    fn meta(&self) -> &'static RuleMeta {
        &END_SET_BY_HAND_MISFITS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let parts = q.parts();
        let mut out = Vec::new();
        for r in q
            .get::<WireRecordSet>(())
            .iter()
            .filter(|r| owns_wire(q, p, &r.wire))
        {
            let csa = match r.csa.as_ref().map(|c| &c.value) {
                Some(ew_schema::Csa::Mm2(d)) => Some(*d),
                _ => None,
            };
            for (i, end) in r.ends.iter().enumerate() {
                if end.treated_by != TreatedBy::Hand {
                    continue;
                }
                let Some((connection, at_a)) = ew_derive::end_connection(q, &r.wire, i) else {
                    continue;
                };
                let ferrule = end
                    .part
                    .and_then(|id| parts.get(id))
                    .and_then(|c| c.item.sections.ferrule.clone());
                let stock = stock_part(q, &end.treatment, csa)
                    .and_then(|id| parts.get(id))
                    .and_then(|c| c.item.sections.ferrule.clone());
                let (form, args) = match (&end.treatment, &ferrule, csa) {
                    (_, Some(f), Some(size)) if size < f.csa_min || f.csa_max < size => {
                        let range = if f.csa_min == f.csa_max {
                            f.csa_min.to_string()
                        } else {
                            format!("{}–{}", f.csa_min, f.csa_max)
                        };
                        (
                            "size".to_owned(),
                            vec![("range", range), ("csa", size.to_string())],
                        )
                    }
                    (EndTreatment::TwinFerrule { partner: None, .. }, _, _) => {
                        ("twin".to_owned(), Vec::new())
                    }
                    (_, Some(f), _)
                        if stock
                            .as_ref()
                            .is_some_and(|s| s.colour.is_some() && s.colour != f.colour) =>
                    {
                        let text = |c: &Option<ew_schema::ColourSpec>| {
                            c.as_ref().map(ew_derive::text::colour_text).unwrap_or_default()
                        };
                        (
                            "colour".to_owned(),
                            vec![
                                ("colour", text(&f.colour)),
                                (
                                    "stock",
                                    stock.as_ref().map(|s| text(&s.colour)).unwrap_or_default(),
                                ),
                            ],
                        )
                    }
                    _ => continue,
                };
                let wire = wire_text(q, &r.wire);
                let at = cp_label(q, r.wire.ends[i].cp);
                let path = if at_a {
                    "overrides.end_a"
                } else {
                    "overrides.end_b"
                };
                let mut f = Finding::new(
                    &END_SET_BY_HAND_MISFITS,
                    [Target::new(Kind::Connection, connection)],
                    if i == 0 { "from" } else { "to" },
                    format!("{wire} {at}"),
                )
                .arg("wire", wire)
                .arg("at", at)
                .arg("form", form);
                for name in ["range", "csa", "colour", "stock"] {
                    let value = args.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone());
                    f = f.arg(name, value.unwrap_or_default());
                }
                out.push(f.with_fix(QuickFix {
                    label: text!("fix-reset-end"),
                    command: "fields.set".to_owned(),
                    params: serde_json::json!({
                        "kind": "connection",
                        "targets": [connection.to_string()],
                        "path": path,
                        "value": null,
                    }),
                }));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-010

/// EW-POT-010 "Clamp capacity exceeded" (doc 15 §4.3): safety, o r, error, device.
pub const CLAMP_CAPACITY: RuleMeta = RuleMeta {
    id: "EW-POT-010",
    message: key!("rule-ew-pot-010"),
    code: "005010",
    kind: RuleKind::Safety,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-POT-010: a clamp on which more conductors land than its part says it takes (`max_conductors`, doc 13 §4.3): the
/// wire ends on the CP, a twin ferrule's two wires counted once. Only where the part names the clamp's capacity. One
/// finding per CP, target the function, discriminator the CP index.
#[derive(Clone, Copy, Debug)]
pub struct ClampCapacity;

impl Rule for ClampCapacity {
    fn meta(&self) -> &'static RuleMeta {
        &CLAMP_CAPACITY
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let mut functions: Vec<_> = q.idx().functions_of_device(device).collect();
        functions.sort();
        let counts = clamp_conductors(&q.get::<WireRecordSet>(()));
        let mut out = Vec::new();
        for f in functions {
            for cp in q.get::<FunctionCps>(f).iter() {
                let Some(max) = cp.clamp_data.as_ref().and_then(|c| c.max_conductors) else {
                    continue;
                };
                let at = CpRef::new(f, cp.idx);
                let count = counts.get(&at).copied().unwrap_or(0);
                if count <= usize::from(max) {
                    continue;
                }
                let label = cp_label(q, at);
                out.push(
                    Finding::new(
                        &CLAMP_CAPACITY,
                        [Target::new(Kind::Function, f)],
                        cp.idx.to_string(),
                        label.clone(),
                    )
                    .arg("cp", label)
                    .arg("max", TextArg::Number(i64::from(max)))
                    .arg("count", TextArg::Number(i64::try_from(count).unwrap_or(i64::MAX))),
                );
            }
        }
        out
    }
}
