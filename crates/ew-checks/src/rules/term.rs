// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-TERM rules (doc 15 §4.4; content in doc 08 §6.8, §8.1).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::CaseSensitivity;
use ew_derive::ew_schema::{Device, Kind, TagCase};
use ew_derive::{DeriveNote, DeviceTag, Q, RailTerminalOf};

use crate::engine::touched_ids;
use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};

/// The project's case setting for identifier uniqueness (FR-NUM-016, FR-NUM-017): sensitive unless the
/// project says insensitive.
pub(crate) fn case_setting(q: &Q) -> CaseSensitivity {
    match q.project().settings.formatting.tag_case {
        Some(TagCase::Insensitive) => CaseSensitivity::Insensitive,
        _ => CaseSensitivity::Sensitive,
    }
}

/// The printed full designation of a device, or `fallback` when it has none.
pub(crate) fn full_tag(q: &Q, device: Id<Device>, fallback: &str) -> String {
    q.get::<DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map_or_else(|| fallback.to_owned(), |t| t.full.clone())
}

/// The terminals of `strip` grouped by the comparison key of their designation, each group sorted by ID;
/// the designation text of each group is the first member's.
fn designation_groups(q: &Q, strip: Id<Device>) -> BTreeMap<String, (String, Vec<Id<Device>>)> {
    let case = case_setting(q);
    let mut groups: BTreeMap<String, (String, Vec<Id<Device>>)> = BTreeMap::new();
    let mut terminals: Vec<Id<Device>> = q.idx().terminals_of_strip(strip).collect();
    terminals.sort();
    for t in terminals {
        let Some(term) = q.devices().get(t).and_then(|d| d.aspects.terminal.as_ref()) else {
            continue;
        };
        let key = term.designation.comparison_key(case).into_owned();
        groups
            .entry(key)
            .or_insert_with(|| (term.designation.as_str().to_owned(), Vec::new()))
            .1
            .push(t);
    }
    groups
}

/// EW-TERM-001 "Terminal without designation" (doc 15 §4.4): conflict, por, error, strip.
pub const TERMINAL_WITHOUT_DESIGNATION: RuleMeta = RuleMeta {
    id: "EW-TERM-001",
    code: "001001",
    pack: "EW-TERM",
    title: "Terminal without designation",
    kind: RuleKind::Conflict,
    modes: Modes::letters("por"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Strip,
    supersedes: &[],
};

/// EW-TERM-001: a terminal of a strip whose designation is empty. `Ident` is non-empty, so a model built
/// through the command layer never holds one; the rule stays for the registry and for lenient data (a
/// designation of white space only). One finding per terminal, no discriminator. No prevent
/// implementation: the parameter layer refuses an empty designation before a transaction exists.
#[derive(Clone, Copy, Debug)]
pub struct TerminalWithoutDesignation;

impl Rule for TerminalWithoutDesignation {
    fn meta(&self) -> &'static RuleMeta {
        &TERMINAL_WITHOUT_DESIGNATION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Strip(strip) = *scope else {
            return Vec::new();
        };
        let strip_tag = full_tag(q, strip, "strip");
        let mut terminals: Vec<Id<Device>> = q.idx().terminals_of_strip(strip).collect();
        terminals.sort();
        terminals
            .into_iter()
            .filter(|t| {
                q.devices()
                    .get(*t)
                    .and_then(|d| d.aspects.terminal.as_ref())
                    .is_some_and(|term| term.designation.as_str().trim().is_empty())
            })
            .map(|t| {
                Finding::new(
                    &TERMINAL_WITHOUT_DESIGNATION,
                    [Target::new(Kind::Device, t)],
                    "",
                    full_tag(q, t, &strip_tag),
                    format!("a terminal of {strip_tag} has no designation"),
                )
            })
            .collect()
    }
}

/// Whether the strip carries a frozen designation with `?`, as the QET importer's fallback strips `X?<page>`
/// do (doc 14 §14 item 5). The stored tag is read, not the derived one: an unplaced strip also derives a
/// designation with `?`, and its terminals are still one strip.
fn is_unnumbered_strip(q: &Q, strip: Id<Device>) -> bool {
    q.devices()
        .get(strip)
        .and_then(|d| d.tag.frozen.as_ref())
        .is_some_and(|f| f.text.as_str().contains('?'))
}

/// EW-TERM-002 "Duplicate designation" (doc 15 §4.4): conflict, p o r, error, strip.
pub const DUPLICATE_DESIGNATION: RuleMeta = RuleMeta {
    id: "EW-TERM-002",
    code: "001002",
    pack: "EW-TERM",
    title: "Duplicate designation",
    kind: RuleKind::Conflict,
    modes: Modes::letters("por"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Strip,
    supersedes: &[],
};

/// EW-TERM-002: a designation used by more than one terminal of a strip (FR-TRM-002, D-12; doc 08 §6.8).
/// Designations compare under the project's case setting.
///
/// - **Online and release:** one finding per duplicated designation, targets every terminal of the group,
///   no discriminator (renaming a member makes it another defect). A strip whose frozen designation is
///   unnumbered (it contains `?`) is skipped: the QET importer's fallback strip `X?<page>` (EW-IMP-005)
///   collects the unlabelled terminals of a folio, which are not one strip (decision log ID-276).
/// - **Prevent:** a touched terminal whose (strip, designation) the batch changed, or that the batch
///   created, and that now shares its designation within its strip. A duplicate the base snapshot already
///   held (a lenient import) does not refuse an edit that leaves it alone. The rule runs on the candidate,
///   so a swap needs no temporary names (FR-TRM-022). The message names the duplicate set and proposes
///   the next free number or the swap.
#[derive(Clone, Copy, Debug)]
pub struct DuplicateDesignation;

impl Rule for DuplicateDesignation {
    fn meta(&self) -> &'static RuleMeta {
        &DUPLICATE_DESIGNATION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Strip(strip) = *scope else {
            return Vec::new();
        };
        if is_unnumbered_strip(q, strip) {
            return Vec::new();
        }
        let strip_tag = full_tag(q, strip, "strip");
        designation_groups(q, strip)
            .into_values()
            .filter(|(_, members)| members.len() > 1)
            .map(|(designation, members)| {
                let subject = format!("{strip_tag}:{designation}");
                let message = format!(
                    "designation {designation} is used by {} terminals of {strip_tag}",
                    members.len()
                );
                Finding::new(
                    &DUPLICATE_DESIGNATION,
                    members.iter().map(|m| Target::new(Kind::Device, *m)),
                    "",
                    subject,
                    message,
                )
            })
            .collect()
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let case = case_setting(after);
        let mut reported: BTreeSet<(Id<Device>, String)> = BTreeSet::new();
        let mut out = Vec::new();
        for id in touched_ids::<Device>(touched, Kind::Device) {
            let Some(term) = after.devices().get(id).and_then(|d| d.aspects.terminal.clone()) else {
                continue;
            };
            let key = term.designation.comparison_key(case).into_owned();
            let old = before
                .devices()
                .get(id)
                .and_then(|d| d.aspects.terminal.as_ref())
                .map(|t| (t.strip, t.designation.comparison_key(case).into_owned()));
            if old.as_ref() == Some(&(term.strip, key.clone())) {
                continue;
            }
            if reported.contains(&(term.strip, key.clone())) {
                continue;
            }
            let groups = designation_groups(after, term.strip);
            let Some((designation, members)) = groups.get(&key) else {
                continue;
            };
            if members.len() < 2 {
                continue;
            }
            reported.insert((term.strip, key.clone()));
            let strip_tag = full_tag(after, term.strip, "the strip");
            let mut remedy = Vec::new();
            if let Some(free) = next_free_number(groups.values().map(|(d, _)| d.as_str())) {
                remedy.push(format!("use {free}"));
            }
            if let Some((strip, old_key)) = &old
                && *strip == term.strip
                && !groups.contains_key(old_key)
                && let Some(old_text) = before
                    .devices()
                    .get(id)
                    .and_then(|d| d.aspects.terminal.as_ref())
                    .map(|t| t.designation.as_str().to_owned())
            {
                remedy.push(format!(
                    "swap {old_text} \u{2194} {designation} in one transaction (terminal.designate)"
                ));
            }
            let mut message = format!(
                "designation {designation} is taken on {strip_tag} ({} terminals)",
                members.len()
            );
            if !remedy.is_empty() {
                message.push_str("; ");
                message.push_str(&remedy.join(" or "));
            }
            out.push(Finding::new(
                &DUPLICATE_DESIGNATION,
                members.iter().map(|m| Target::new(Kind::Device, *m)),
                "",
                format!("{strip_tag}:{designation}"),
                message,
            ));
        }
        out
    }
}

/// EW-TERM-005 "Rail bonding on a non-PE terminal" (doc 15 §4.4): conflict, o, error, strip.
pub const RAIL_BONDING_ON_NON_PE: RuleMeta = RuleMeta {
    id: "EW-TERM-005",
    code: "001005",
    pack: "EW-TERM",
    title: "Rail bonding on a non-PE terminal",
    kind: RuleKind::Conflict,
    modes: Modes::letters("o"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Strip,
    supersedes: &[],
};

/// EW-TERM-005: a terminal with `rail_bonded` set that has no PE connection point (doc 08 §6.8). The
/// derivation notes it (`rail_terminal`); the rule reports each such note once per terminal, no
/// discriminator. Online only, no prevent implementation.
#[derive(Clone, Copy, Debug)]
pub struct RailBondingOnNonPe;

impl Rule for RailBondingOnNonPe {
    fn meta(&self) -> &'static RuleMeta {
        &RAIL_BONDING_ON_NON_PE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Strip(strip) = *scope else {
            return Vec::new();
        };
        let mut terminals: Vec<Id<Device>> = q.idx().terminals_of_strip(strip).collect();
        terminals.sort();
        let mut out = Vec::new();
        for t in terminals {
            let bonded = q
                .devices()
                .get(t)
                .and_then(|d| d.aspects.terminal.as_ref())
                .is_some_and(|term| term.rail_bonded);
            if !bonded {
                continue;
            }
            let rail = q.get::<RailTerminalOf>(t);
            for note in &rail.notes {
                if let DeriveNote::Connectivity {
                    code: "EW-TERM-005",
                    detail,
                    ..
                } = note
                {
                    out.push(Finding::new(
                        &RAIL_BONDING_ON_NON_PE,
                        [Target::new(Kind::Device, t)],
                        "",
                        full_tag(q, t, "terminal"),
                        detail.clone(),
                    ));
                }
            }
        }
        out
    }
}

/// One more than the largest designation that is a plain decimal number, or 1 when there is none.
fn next_free_number<'a>(designations: impl Iterator<Item = &'a str>) -> Option<u64> {
    let max = designations
        .filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|d| d.parse::<u64>().ok())
        .max();
    match max {
        Some(m) => m.checked_add(1),
        None => Some(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_free_number_follows_the_largest_number() {
        assert_eq!(
            next_free_number(["1", "2", "20", "PE", "3a"].into_iter()),
            Some(21)
        );
        assert_eq!(next_free_number(["PE", "N"].into_iter()), Some(1));
        assert_eq!(next_free_number(std::iter::empty()), Some(1));
    }
}
