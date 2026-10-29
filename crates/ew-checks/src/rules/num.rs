// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-NUM rules, pack EW-IEC81346 (doc 15 §4.3; numbering in doc 07, doc 03 §8).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_cmp;
use ew_derive::ew_schema::{Connection, Kind, NumberSpec};
use ew_derive::{Q, WireNumbers, rule_product};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// EW-NUM-001 "Tag deviates from the scheme" (doc 15 §4.3): hygiene, o r, warning (frozen: info), device;
/// supersedes EW-NUM-002 and EW-RDS-001 for tags and frozen tags (doc 15 §3.6).
pub const TAG_DEVIATES: RuleMeta = RuleMeta {
    id: "EW-NUM-001",
    code: "017101",
    pack: "EW-IEC81346",
    title: "Tag deviates from the scheme",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &["EW-NUM-002", "EW-RDS-001"],
    release: Release::Mvp,
};

/// EW-NUM-001: a tag that does not match the active numbering scheme (FR-CHK-025).
///
/// - **Frozen tag** (`TagSpec.frozen`, doc 03 §8.5): when its text differs from the rule-derived product
///   designation (`ew_derive::rule_product`: what the scheme derives with the frozen tag left out; "?" where
///   the scheme cannot derive a value), one finding at **info**, "frozen, deviates from scheme", with the
///   rule-derived value (EF-15). A frozen tag equal to the rule's value deviates from nothing.
/// - **Derived tag**: a tag without a frozen override is the scheme's own value, so it matches by
///   construction; nothing is reported. (A tag that cannot be numbered prints "?", which EW-IMP-122 and
///   the numbering commands own.)
///
/// Target: the device; discriminator `frozen` (no tag text enters the fingerprint, so a renumber keeps it).
#[derive(Clone, Copy, Debug)]
pub struct TagDeviates;

impl Rule for TagDeviates {
    fn meta(&self) -> &'static RuleMeta {
        &TAG_DEVIATES
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(frozen) = q
            .devices()
            .get(device)
            .and_then(|d| d.tag.frozen.as_ref().map(|f| f.text.as_str().to_owned()))
        else {
            return Vec::new();
        };
        let Some((rule, _)) = rule_product(q, device) else {
            return Vec::new();
        };
        if rule == frozen {
            return Vec::new();
        }
        let tag = full_tag(q, device, "device");
        vec![
            Finding::new(
                &TAG_DEVIATES,
                [Target::new(Kind::Device, device)],
                "frozen",
                tag,
                format!("{frozen}: frozen, deviates from scheme (rule-derived value {rule})"),
            )
            .with_severity(Severity::Info),
        ]
    }
}

/// EW-NUM-002 "Frozen value deviates" (doc 15 §4.3): hygiene, o r, info, project.
pub const FROZEN_VALUE_DEVIATES: RuleMeta = RuleMeta {
    id: "EW-NUM-002",
    code: "017102",
    pack: "EW-IEC81346",
    title: "Frozen value deviates",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-NUM-002: a frozen wire number (`NumberSpec::Frozen` on a connection, doc 08 §5.3) that differs from the number
/// the numbering rule derives for the wire (`wire_numbers`; FR-CHK-037, FR-NUM-022). The same comparison as the
/// numbering report's `status` column (`ew-reports`, FR-REP-022): a wire the rule does not number (a cable core, a
/// wire drawn on no page, a constituent that is not the wire's carrier) has no rule value and deviates. Frozen tags
/// are EW-NUM-001's (it supersedes this rule for tags); a manual PLC address is EW-PLC-012's (WP-S1.21), which
/// supersedes it for addresses. One finding per connection: target the connection, discriminator `wire-number`.
#[derive(Clone, Copy, Debug)]
pub struct FrozenValueDeviates;

impl Rule for FrozenValueDeviates {
    fn meta(&self) -> &'static RuleMeta {
        &FROZEN_VALUE_DEVIATES
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let frozen: Vec<(Id<Connection>, String)> = q
            .connections()
            .iter()
            .filter_map(|(id, c)| match &c.number {
                NumberSpec::Frozen(t) => Some((id, t.as_str().to_owned())),
                _ => None,
            })
            .collect();
        if frozen.is_empty() {
            return Vec::new();
        }
        let numbers = q.get::<WireNumbers>(());
        let mut rows: Vec<(String, String, Id<Connection>)> = frozen
            .into_iter()
            .filter_map(|(id, text)| {
                let rule = numbers.number(id).map(|n| n.label.clone()).unwrap_or_default();
                (rule != text).then_some((text, rule, id))
            })
            .collect();
        rows.sort_by(|a, b| {
            natural_cmp(&a.0, &b.0)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| natural_cmp(&a.1, &b.1))
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        rows.into_iter()
            .map(|(text, rule, id)| {
                let derived = if rule.is_empty() {
                    "the rule derives no number for the wire".to_owned()
                } else {
                    format!("rule-derived value {rule}")
                };
                Finding::new(
                    &FROZEN_VALUE_DEVIATES,
                    [Target::new(Kind::Connection, id)],
                    "wire-number",
                    format!("wire {text}"),
                    format!("wire number {text}: frozen, deviates from the rule ({derived})"),
                )
            })
            .collect()
    }
}
