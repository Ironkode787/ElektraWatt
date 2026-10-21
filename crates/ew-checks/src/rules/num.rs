// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-NUM rules, pack EW-IEC81346 (doc 15 §4.3; numbering in doc 07, doc 03 §8).

use ew_derive::ew_schema::Kind;
use ew_derive::{Q, rule_product};

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
