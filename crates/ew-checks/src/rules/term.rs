// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-TERM rules (doc 15 §4.4; content in doc 08 §6.8, §8.1).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{CaseSensitivity, natural_cmp};
use ew_derive::ew_schema::{ConnectionKind, Device, DeviceState, Endpoint, Kind, Side, TagCase};
use ew_derive::{DeriveNote, DeviceTag, FunctionCps, Q, RailTerminalOf, StripBridges};

use crate::engine::touched_ids;
use crate::finding::{Finding, Fingerprint, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};

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

/// The designation groups of a strip: comparison key → (designation text, terminals).
type Groups = BTreeMap<String, (String, Vec<Id<Device>>)>;

/// The terminals of `strip` grouped by the comparison key of their designation, each group sorted by ID;
/// the designation text of each group is the first member's.
fn designation_groups(q: &Q, strip: Id<Device>) -> Groups {
    let case = case_setting(q);
    let mut groups: Groups = BTreeMap::new();
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
    release: Release::Mvp,
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
    release: Release::Mvp,
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
        // The designation sets of the strips the batch touched, built once per strip (FT-196): a batch that
        // renumbers a whole strip reads it once, not once per terminal.
        let mut strips: BTreeMap<Id<Device>, Groups> = BTreeMap::new();
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
            let groups = strips
                .entry(term.strip)
                .or_insert_with(|| designation_groups(after, term.strip));
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
    release: Release::Mvp,
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

/// EW-TERM-003 "Spare wired on the field side" (doc 15 §4.4): conflict, o, warning, strip.
pub const SPARE_WIRED_ON_FIELD_SIDE: RuleMeta = RuleMeta {
    id: "EW-TERM-003",
    code: "001003",
    pack: "EW-TERM",
    title: "Spare wired on the field side",
    kind: RuleKind::Conflict,
    modes: Modes::letters("o"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Strip,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-TERM-003: a spare terminal (`Device.state = Spare`, FR-TRM-009) with a wire or a cable core at a CP of its
/// strip's field side (`Strip.field_side`, default side 2) (doc 08 §6.6: a spare may be fed on side 1, as Golden
/// -X1031's spares 5, 7 and 9 are). Bridges, bus lines and internal links are not wires. One finding per terminal:
/// target the terminal, discriminator `field-side`. Online only.
#[derive(Clone, Copy, Debug)]
pub struct SpareWiredOnFieldSide;

impl Rule for SpareWiredOnFieldSide {
    fn meta(&self) -> &'static RuleMeta {
        &SPARE_WIRED_ON_FIELD_SIDE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Strip(strip) = *scope else {
            return Vec::new();
        };
        let field = q
            .devices()
            .get(strip)
            .and_then(|s| s.aspects.strip.as_ref().map(|s| s.field_side.clone()))
            .unwrap_or(Side::Side2);
        let mut terminals: Vec<(u32, Id<Device>)> = q
            .idx()
            .terminals_of_strip(strip)
            .filter_map(|t| {
                let d = q.devices().get(t)?;
                (d.state == DeviceState::Spare).then(|| (d.aspects.terminal.as_ref().map_or(0, |a| a.seq), t))
            })
            .collect();
        terminals.sort_unstable();
        let connections = q.connections();
        let mut out = Vec::new();
        for (_, t) in terminals {
            let wired = q.idx().functions_of_device(t).any(|f| {
                let cps = q.get::<FunctionCps>(f);
                q.idx().connected_cps_of_function(f).iter().any(|(idx, conns)| {
                    cps.iter()
                        .any(|c| c.idx == *idx && c.side.as_ref() == Some(&field))
                        && conns.iter().any(|c| {
                            connections.get(*c).is_some_and(|c| {
                                matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_))
                            })
                        })
                })
            });
            if !wired {
                continue;
            }
            let tag = full_tag(q, t, "terminal");
            out.push(Finding::new(
                &SPARE_WIRED_ON_FIELD_SIDE,
                [Target::new(Kind::Device, t)],
                "field-side",
                tag.clone(),
                format!("spare terminal {tag} is wired on its field side; wire it on the other side or clear its spare state"),
            ));
        }
        out
    }
}

/// EW-TERM-004 "Bridge group invalid" (doc 15 §4.4): conflict, p o, error, strip.
pub const BRIDGE_GROUP_INVALID: RuleMeta = RuleMeta {
    id: "EW-TERM-004",
    code: "001004",
    pack: "EW-TERM",
    title: "Bridge group invalid",
    kind: RuleKind::Conflict,
    modes: Modes::letters("po"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Strip,
    supersedes: &[],
    release: Release::Mvp,
};

/// The EW-TERM-004 findings of one strip (see [`BridgeGroupInvalid`]).
fn bridge_findings(q: &Q, strip: Id<Device>) -> Vec<Finding> {
    let groups = q.get::<StripBridges>(strip);
    if groups.is_empty() {
        return Vec::new();
    }
    let devices = q.devices();
    let functions = q.functions();
    let connections = q.connections();
    // The strip's terminals in mounting order, and each one's position in it.
    let mut order: Vec<(u32, Id<Device>)> = q
        .idx()
        .terminals_of_strip(strip)
        .filter_map(|t| Some((devices.get(t)?.aspects.terminal.as_ref()?.seq, t)))
        .collect();
    order.sort_unstable();
    let position: BTreeMap<Id<Device>, usize> = order.iter().enumerate().map(|(i, (_, t))| (*t, i)).collect();
    let strip_tag = full_tag(q, strip, "strip");
    // Per group: its shafts, its positions on this strip and the strips it reaches beyond this one.
    struct Shape {
        group: u16,
        shafts: BTreeSet<u8>,
        positions: Vec<usize>,
        other_strips: BTreeSet<Id<Device>>,
        connections: Vec<Id<ew_derive::ew_schema::Connection>>,
    }
    let shapes: Vec<Shape> = groups
        .iter()
        .map(|g| {
            let mut shafts = BTreeSet::new();
            let mut other_strips = BTreeSet::new();
            for c in &g.connections {
                let Some(c) = connections.get(*c) else { continue };
                if let ConnectionKind::Bridge(spec) = &c.kind {
                    shafts.insert(spec.shaft);
                }
                for e in [&c.a, &c.b] {
                    let Endpoint::Cp(f, _) = e else {
                        continue;
                    };
                    let s = functions
                        .get(*f)
                        .and_then(|f| f.device)
                        .and_then(|d| devices.get(d))
                        .and_then(|d| d.aspects.terminal.as_ref().map(|t| t.strip));
                    match s {
                        Some(s) if s == strip => {}
                        Some(s) => {
                            other_strips.insert(s);
                        }
                        None => {
                            other_strips.insert(strip);
                        }
                    }
                }
            }
            let mut positions: Vec<usize> = g
                .terminals
                .iter()
                .filter_map(|t| position.get(t).copied())
                .collect();
            positions.sort_unstable();
            positions.dedup();
            Shape {
                group: g.group,
                shafts,
                positions,
                other_strips,
                connections: g.connections.clone(),
            }
        })
        .collect();
    let mut out = Vec::new();
    for s in &shapes {
        let mut why: Vec<String> = Vec::new();
        if s.positions.windows(2).any(|w| w[1] != w[0] + 1) {
            why.push("its terminals are not consecutive".to_owned());
        }
        if s.shafts.len() > 1 {
            why.push(format!("it runs over {} shafts", s.shafts.len()));
        }
        if !s.other_strips.is_empty() {
            let mut names: Vec<String> = s
                .other_strips
                .iter()
                .map(|o| {
                    if *o == strip {
                        "a device that is not a terminal".to_owned()
                    } else {
                        full_tag(q, *o, "another strip")
                    }
                })
                .collect();
            names.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
            names.dedup();
            why.push(format!("it reaches {}", names.join(", ")));
        }
        let (Some(lo), Some(hi)) = (s.positions.first(), s.positions.last()) else {
            continue;
        };
        let mut overlaps: Vec<u16> = shapes
            .iter()
            .filter(|o| o.group != s.group && !o.shafts.is_disjoint(&s.shafts))
            .filter(|o| match (o.positions.first(), o.positions.last()) {
                (Some(olo), Some(ohi)) => olo <= hi && lo <= ohi,
                _ => false,
            })
            .map(|o| o.group)
            .collect();
        overlaps.sort_unstable();
        if !overlaps.is_empty() {
            let list: Vec<String> = overlaps.iter().map(u16::to_string).collect();
            why.push(format!(
                "it overlaps bridge group {} on its shaft",
                list.join(", ")
            ));
        }
        if why.is_empty() {
            continue;
        }
        out.push(Finding::new(
            &BRIDGE_GROUP_INVALID,
            s.connections.iter().map(|c| Target::new(Kind::Connection, *c)),
            format!("group-{}", s.group),
            format!("{strip_tag} bridge group {}", s.group),
            format!(
                "bridge group {} of {strip_tag} is invalid: {}; a bridge group runs over consecutive terminals of \
                 one strip on one shaft",
                s.group,
                why.join("; ")
            ),
        ));
    }
    out
}

/// EW-TERM-004: a bridge group of a strip (`strip_bridges`, doc 08 §2.3) that breaks invariant CI-06 (FR-TRM-007,
/// doc 08 §8.1): its terminals are not consecutive in the strip's mounting order (`seq`), it runs over more than one
/// shaft, a bridge of it reaches another strip or a device that is not a terminal, or it overlaps another group on a
/// shaft they share (their ranges of mounting positions intersect). One finding per group with every reason: targets
/// the group's connections, discriminator `group-<n>`.
///
/// - **Online:** every invalid group of the strip (a lenient import's are reported here; CI-06 is prevent class).
/// - **Prevent:** the strips of the touched bridge connections and terminals; a finding the base snapshot already held
///   does not refuse an edit that leaves it alone.
#[derive(Clone, Copy, Debug)]
pub struct BridgeGroupInvalid;

impl Rule for BridgeGroupInvalid {
    fn meta(&self) -> &'static RuleMeta {
        &BRIDGE_GROUP_INVALID
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Strip(strip) = *scope else {
            return Vec::new();
        };
        bridge_findings(q, strip)
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let mut strips: BTreeSet<Id<Device>> = BTreeSet::new();
        let strip_of_end = |q: &Q, e: &Endpoint| -> Option<Id<Device>> {
            let Endpoint::Cp(f, _) = e else { return None };
            let d = q.functions().get(*f)?.device?;
            q.devices().get(d)?.aspects.terminal.as_ref().map(|t| t.strip)
        };
        for id in touched_ids::<ew_derive::ew_schema::Connection>(touched, Kind::Connection) {
            for q in [before, after] {
                if let Some(c) = q.connections().get(id)
                    && matches!(c.kind, ConnectionKind::Bridge(_))
                {
                    strips.extend([&c.a, &c.b].into_iter().filter_map(|e| strip_of_end(q, e)));
                }
            }
        }
        for id in touched_ids::<Device>(touched, Kind::Device) {
            if let Some(t) = after.devices().get(id).and_then(|d| d.aspects.terminal.as_ref()) {
                strips.insert(t.strip);
            }
        }
        let mut out = Vec::new();
        for strip in strips {
            if after.devices().get(strip).is_none() {
                continue;
            }
            let held: BTreeSet<(Fingerprint, String)> = bridge_findings(before, strip)
                .into_iter()
                .map(|f| (f.fingerprint, f.message))
                .collect();
            out.extend(
                bridge_findings(after, strip)
                    .into_iter()
                    .filter(|f| !held.contains(&(f.fingerprint, f.message.clone()))),
            );
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
