// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of a wire's ends for the workshop (doc 13 §4.6), over the treatment of each end that `wire_record_set()`
//! gives: EW-MFG-004, a marker whose text does not fit its marker part; EW-MFG-008, an end the rule table cannot decide; EW-MFG-009, an end set by hand that does not fit; and
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
// EW-MFG-004

/// EW-MFG-004 "Marker text does not fit" (doc 15 §4.5.5): conflict, o x, warning, potential.
pub const MARKER_DOES_NOT_FIT: RuleMeta = RuleMeta {
    id: "EW-MFG-004",
    message: key!("rule-ew-mfg-004"),
    code: "140004",
    kind: RuleKind::Conflict,
    modes: Modes::letters("ox"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Potential,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-MFG-004: a wire end's marker whose text does not fit the wire markers' part of the workshop stock after the
/// shortening of doc 13 §5.2 (its characters per line, else its lines). One finding per end, target the connection
/// that lands on it, discriminator the end (`from`, `to`).
#[derive(Clone, Copy, Debug)]
pub struct MarkerDoesNotFit;

impl Rule for MarkerDoesNotFit {
    fn meta(&self) -> &'static RuleMeta {
        &MARKER_DOES_NOT_FIT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Potential(p) = *scope else {
            return Vec::new();
        };
        let (lines, chars) = ew_derive::wire_marker_part(q).unwrap_or((None, None));
        let mut out = Vec::new();
        for r in q.get::<WireRecordSet>(()).iter().filter(|r| owns_wire(q, p, &r.wire)) {
            for (i, end) in r.ends.iter().enumerate() {
                let (false, ew_derive::MarkerText::Lines(text)) = (end.marker_fits, &end.marker) else {
                    continue;
                };
                let Some((connection, _)) = ew_derive::end_connection(q, &r.wire, i) else {
                    continue;
                };
                let longest = text.iter().map(|l| l.chars().count()).max().unwrap_or(0);
                let (form, needs, limit) = match chars {
                    Some(c) if longest > usize::from(c) => ("chars", longest, c),
                    _ => ("lines", text.len(), lines.unwrap_or(0)),
                };
                let wire = wire_text(q, &r.wire);
                let at = cp_label(q, r.wire.ends[i].cp);
                out.push(
                    Finding::new(
                        &MARKER_DOES_NOT_FIT,
                        [Target::new(Kind::Connection, connection)],
                        if i == 0 { "from" } else { "to" },
                        format!("{wire} {at}"),
                    )
                    .arg("wire", wire)
                    .arg("at", at)
                    .arg("text", text.join(" / "))
                    .arg("form", form)
                    .arg("needs", TextArg::Number(i64::try_from(needs).unwrap_or(i64::MAX)))
                    .arg("limit", TextArg::Number(i64::from(limit))),
                );
            }
        }
        out
    }
}

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
/// insulation-displacement clamp). One finding per end, target the connection that lands on it, discriminator the wire
/// and the clamp ([`end_discriminator`]); a bond wire reported by the potential that prints first.
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
                        end_discriminator(r, i),
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

/// The discriminator of a finding about the end `i` of the wire `r`: the wire by its ID and the clamp the end lands on,
/// so that the fingerprint moves neither with the wire's number nor with the wire turning round.
fn end_discriminator(r: &ew_derive::WireRecord, i: usize) -> String {
    let cp = r.wire.ends[i].cp;
    format!("{}/{}:{}", r.wire.id, cp.function, cp.idx)
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
/// stock's ferrule for that end. One finding per end, target the connection that holds it (`EndRecord::hand_at`),
/// discriminator the wire and the clamp ([`end_discriminator`]); its quick fix, Reset to the rule, clears the end set by
/// hand.
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
                if end.treated_by != TreatedBy::Hand && !end.part_by_hand {
                    continue;
                }
                let Some((connection, at_a)) = end.hand_at else {
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
                    end_discriminator(r, i),
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
/// wire ends on the CP, a twin ferrule's two wires counted once, one finding per CP, discriminator the CP index; and a
/// wire of a size in mm² outside the clamp's range (`csa_min`, `csa_max`), one finding per wire, discriminator the CP
/// index and the wire's ID. Only where the part names the capacity or the range; target the function.
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
        let records = q.get::<WireRecordSet>(());
        let counts = clamp_conductors(&records);
        let mut out = Vec::new();
        for f in functions {
            for cp in q.get::<FunctionCps>(f).iter() {
                let Some(clamp) = cp.clamp_data.as_ref() else {
                    continue;
                };
                let at = CpRef::new(f, cp.idx);
                let label = cp_label(q, at);
                let finding = |discriminator: String, form: &'static str| {
                    Finding::new(
                        &CLAMP_CAPACITY,
                        [Target::new(Kind::Function, f)],
                        discriminator,
                        label.clone(),
                    )
                    .arg("cp", label.clone())
                    .arg("form", form)
                };
                let count = counts.get(&at).copied().unwrap_or(0);
                if let Some(max) = clamp.max_conductors
                    && count > usize::from(max)
                {
                    out.push(
                        finding(cp.idx.to_string(), "count")
                            .arg("max", TextArg::Number(i64::from(max)))
                            .arg("count", TextArg::Number(i64::try_from(count).unwrap_or(i64::MAX)))
                            .arg("range", "")
                            .arg("wire", "")
                            .arg("csa", ""),
                    );
                }
                // A wire of a size outside the clamp's range, where its part names one.
                let (min, max) = clamp.csa;
                if min.is_none() && max.is_none() {
                    continue;
                }
                let range = match (min, max) {
                    (Some(a), Some(b)) if a == b => a.to_string(),
                    (Some(a), Some(b)) => format!("{a}–{b}"),
                    (Some(a), None) => format!("≥ {a}"),
                    (None, Some(b)) => format!("≤ {b}"),
                    (None, None) => String::new(),
                };
                for r in records.iter() {
                    let Some(ew_schema::Csa::Mm2(size)) = r.csa.as_ref().map(|c| &c.value) else {
                        continue;
                    };
                    let lands = r
                        .wire
                        .ends
                        .iter()
                        .any(|e| e.cp == at && e.kind == ew_derive::EndKind::Device);
                    let outside = min.is_some_and(|m| *size < m) || max.is_some_and(|m| *size > m);
                    if !lands || !outside {
                        continue;
                    }
                    let wire = wire_text(q, &r.wire);
                    out.push(
                        finding(format!("{}/{}", cp.idx, r.wire.id), "size")
                            .arg("max", TextArg::Number(0))
                            .arg("count", TextArg::Number(0))
                            .arg("range", range.clone())
                            .arg("wire", wire)
                            .arg("csa", size.to_string()),
                    );
                }
            }
        }
        out
    }
}
