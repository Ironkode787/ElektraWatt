// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-RDS rules, pack EW-IEC81346 (doc 15 §4.2; numbering in doc 07).

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::Ident;
use ew_derive::ew_schema::{Device, Kind};
use ew_derive::{DeviceTag, Q};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::case_setting;

/// EW-RDS-004 "Duplicate designation" (doc 15 §4.2): conflict, p o r, error, project.
pub const DUPLICATE_DESIGNATION: RuleMeta = RuleMeta {
    id: "EW-RDS-004",
    code: "017004",
    pack: "EW-IEC81346",
    title: "Duplicate designation",
    kind: RuleKind::Conflict,
    modes: Modes::letters("opr"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
};

/// EW-RDS-004: two devices print the same full designation (FR-NUM-016). Terminals are excluded: their
/// designation is unique per strip (EW-TERM-002). Designations compare under the project's case setting;
/// an unnumbered designation (it still contains `?`, EW-IMP-122) is skipped.
///
/// One finding per duplicated designation, targets every device of the group, no discriminator.
///
/// Deviations: the catalogue lists the scope as device, but the grouping is project-wide, so the rule is
/// evaluated once in the project scope. Prevent mode is not wired at MVP: tag uniqueness on commit needs
/// every derived tag, which is a decision for the design pass; the rule declares the mode and the default
/// prevent reports nothing.
#[derive(Clone, Copy, Debug)]
pub struct DuplicateDesignation;

impl Rule for DuplicateDesignation {
    fn meta(&self) -> &'static RuleMeta {
        &DUPLICATE_DESIGNATION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let case = case_setting(q);
        let mut groups: BTreeMap<String, (String, Vec<Id<Device>>)> = BTreeMap::new();
        for (id, device) in q.devices().iter() {
            if device.aspects.terminal.is_some() {
                continue;
            }
            let tag = q.get::<DeviceTag>(id);
            let Some(tag) = tag.as_ref().as_ref() else {
                continue;
            };
            if tag.full.contains('?') {
                continue;
            }
            let key = match tag.full.parse::<Ident>() {
                Ok(ident) => ident.comparison_key(case).into_owned(),
                Err(_) => tag.full.clone(),
            };
            groups
                .entry(key)
                .or_insert_with(|| (tag.full.clone(), Vec::new()))
                .1
                .push(id);
        }
        groups
            .into_values()
            .filter(|(_, members)| members.len() > 1)
            .map(|(full, members)| {
                let message = format!("`{full}` is printed by {} devices", members.len());
                Finding::new(
                    &DUPLICATE_DESIGNATION,
                    members.iter().map(|m| Target::new(Kind::Device, *m)),
                    "",
                    full,
                    message,
                )
            })
            .collect()
    }
}
