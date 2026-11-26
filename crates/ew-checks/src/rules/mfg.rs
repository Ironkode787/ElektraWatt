// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of a wire's ends for the workshop (doc 13 §4.6), over the treatment of each end that `wire_record_set()`
//! gives: EW-MFG-008, an end the rule table cannot decide; and EW-POT-010, a clamp that takes fewer conductors than land
//! on it, which reads the same pairing (a twin ferrule is one conductor).

use ew_derive::{CpRef, EndTreatment, FunctionCps, Q, Undecided, WireRecordSet, clamp_conductors, cp_label};
use ew_l10n::TextArg;
use ew_schema::Kind;

use crate::finding::{Finding, Severity, Target};
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
