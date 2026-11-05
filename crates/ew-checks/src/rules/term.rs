// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-TERM rules (doc 15 §4.4; content in doc 08 §6.8, §8.1).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{CaseSensitivity, natural_cmp};
use ew_derive::ew_schema::{
    Connection, ConnectionKind, Device, DeviceState, Endpoint, Function, Kind, Side, TagCase,
};
use ew_derive::ew_units::Pt2;
use ew_derive::{
    DeriveNote, DeviceTag, EndpointKey, FunctionCps, PhysicalWiresOf, PotentialPartition, Q, RailTerminalOf,
    StripBridges, WireNumbers, cp_point, drawn_path, on_segment,
};

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

/// FT-1480: the `end` argument of EW-TERM-002's pattern: `named` for a strip with a printed designation, `untagged`
/// for one without, which each language words itself ("the strip", "de klemmenstrook").
fn strip_end(q: &Q, strip: Id<Device>) -> &'static str {
    if q.get::<DeviceTag>(strip).as_ref().is_some() {
        "named"
    } else {
        "untagged"
    }
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
    docs: "reference/rules/ew-term-001",
};

/// EW-TERM-001: a terminal of a strip whose designation is empty. `Ident` is non-empty, so a model built
/// through the command layer rarely holds one; the rule stays for the registry and for lenient data (a
/// designation of white space only). One finding per terminal, no discriminator. Its prevent half (doc 15 §4.4
/// "prevent, lenient on import"; ID-1392, FT-1399) refuses a batch that leaves a touched terminal without a
/// designation it had before; `Ident` already refuses an empty or blank designation, so it is silent on every snapshot
/// the schema reads.
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

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let blank = |q: &Q, id: Id<Device>| {
            q.devices()
                .get(id)
                .and_then(|d| d.aspects.terminal.as_ref())
                .map(|t| (t.strip, t.designation.as_str().trim().is_empty()))
        };
        touched_ids::<Device>(touched, Kind::Device)
            .filter_map(|id| {
                let (strip, true) = blank(after, id)? else {
                    return None;
                };
                if blank(before, id).is_some_and(|(_, was)| was) {
                    return None;
                }
                let strip_tag = full_tag(after, strip, "the strip");
                Some(Finding::new(
                    &TERMINAL_WITHOUT_DESIGNATION,
                    [Target::new(Kind::Device, id)],
                    "",
                    full_tag(after, id, &strip_tag),
                    format!("a terminal of {strip_tag} has no designation"),
                ))
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
    docs: "reference/rules/ew-term-002",
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
                // FT-1334: the arguments of the rule's pattern.
                .arg("form", "used")
                .arg("designation", designation)
                .arg("count", members.len().to_string())
                .arg("strip", strip_tag.clone())
                .arg("end", strip_end(q, strip))
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
            let free = next_free_number(groups.values().map(|(d, _)| d.as_str()));
            if let Some(free) = &free {
                remedy.push(format!("use {free}"));
            }
            let mut swap = None;
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
                swap = Some(old_text);
            }
            let mut message = format!(
                "designation {designation} is taken on {strip_tag} ({} terminals)",
                members.len()
            );
            if !remedy.is_empty() {
                message.push_str("; ");
                message.push_str(&remedy.join(" or "));
            }
            // FT-1417: the arguments of the rule's pattern, so that each language words the prevent finding too.
            let remedy = match (&free, &swap) {
                (Some(_), Some(_)) => "both",
                (Some(_), None) => "free",
                (None, Some(_)) => "swap",
                (None, None) => "none",
            };
            out.push(
                Finding::new(
                    &DUPLICATE_DESIGNATION,
                    members.iter().map(|m| Target::new(Kind::Device, *m)),
                    "",
                    format!("{strip_tag}:{designation}"),
                    message,
                )
                .arg("form", "taken")
                .arg("designation", designation.clone())
                .arg("count", members.len().to_string())
                .arg("strip", strip_tag)
                .arg("end", strip_end(after, term.strip))
                .arg("remedy", remedy)
                .arg("free", free.map(|f| f.to_string()).unwrap_or_default())
                .arg("old", swap.unwrap_or_default()),
            );
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
    docs: "reference/rules/ew-term-005",
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
    docs: "reference/rules/ew-term-003",
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
    docs: "reference/rules/ew-term-004",
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

// ---------------------------------------------------------------------------------------------------------
// EW-TERM-006

/// EW-TERM-006 "Terminal on a wire it does not join" (doc 15 §4.4): conflict, o r, warning, page.
pub const TERMINAL_ON_UNJOINED_WIRE: RuleMeta = RuleMeta {
    id: "EW-TERM-006",
    code: "001006",
    pack: "EW-TERM",
    title: "Terminal on a wire it does not join",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
    docs: "reference/rules/ew-term-006",
};

/// The axis-parallel segments of the page's wires on one line (the y of a horizontal one, the x of a vertical one):
/// `(start, end, wire)` along the line, `start ≤ end`, sorted, read as a balanced search tree over the sorted order
/// (the node of a range is its middle element) with the largest end of each node's subtree.
#[derive(Default)]
struct Line {
    segments: Vec<(i64, i64, Id<Connection>)>,
    reach: Vec<i64>,
}

impl Line {
    /// Sorts the segments and computes each subtree's largest end.
    fn seal(&mut self) {
        self.segments.sort_unstable();
        self.reach = vec![i64::MIN; self.segments.len()];
        self.build(0, self.segments.len());
    }

    fn build(&mut self, lo: usize, hi: usize) -> i64 {
        if lo >= hi {
            return i64::MIN;
        }
        let mid = lo + (hi - lo) / 2;
        let reach = self.segments[mid]
            .1
            .max(self.build(lo, mid))
            .max(self.build(mid + 1, hi));
        self.reach[mid] = reach;
        reach
    }

    /// The wires of the segments that hold `v` (`start ≤ v ≤ end`), pushed onto `out`: a search down the tree that
    /// enters a subtree only when one of its segments reaches `v`, and its right half only when the node starts at or
    /// before `v`, so it visits the segments that hold `v` and the tree's depth above each, not every segment starting
    /// before `v`.
    fn holding(&self, v: i64, out: &mut Vec<Id<Connection>>) {
        self.search(0, self.segments.len(), v, out);
    }

    fn search(&self, lo: usize, hi: usize, v: i64, out: &mut Vec<Id<Connection>>) {
        if lo >= hi {
            return;
        }
        let mid = lo + (hi - lo) / 2;
        if self.reach[mid] < v {
            return;
        }
        self.search(lo, mid, v, out);
        let (start, end, wire) = self.segments[mid];
        if start <= v {
            if v <= end {
                out.push(wire);
            }
            self.search(mid + 1, hi, v, out);
        }
    }
}

/// EW-TERM-006 (FT-1020, decision log ID-1104, ID-1110): a connection point of a terminal, drawn on the page by the
/// terminal's own symbol, lies on the path a wire or cable core of the page is drawn along (`ew_derive::drawn_path`:
/// its stored route, else the provisional path between its ends), and that wire does not join the terminal: the page
/// shows the terminal in the wire, but it is not connected. A wire joins the terminal when an end is a CP of any
/// function of the terminal's device, or a junction standing on one of the terminal's CP points on the page that a
/// connection joins to a CP of the terminal's device. One finding per terminal and wire: targets the terminal and the
/// wire, no discriminator, shown on the page; the message leads with the subject, so a list that prints the subject
/// before a message names the terminal once. The points a strip segment draws for its strip's terminals are not read.
///
/// Bounded work: the wires' horizontal and vertical segments are kept by their line (`Line`), so a CP is looked up
/// only among the segments on its own two lines; other segments are tested one by one.
#[derive(Clone, Copy, Debug)]
pub struct TerminalOnUnjoinedWire;

impl Rule for TerminalOnUnjoinedWire {
    fn meta(&self) -> &'static RuleMeta {
        &TERMINAL_ON_UNJOINED_WIRE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(page) = *scope else {
            return Vec::new();
        };
        // The CPs the terminals' own representations draw on the page.
        let mut functions: BTreeSet<Id<Function>> = BTreeSet::new();
        for r in q.idx().reps_by_page(page) {
            if let Some(rep) = q.reps().get(r) {
                functions.extend(rep.functions.iter().copied());
            }
        }
        let mut points: Vec<(Pt2, Id<Device>)> = Vec::new();
        for f in functions {
            let Some(terminal) = q.functions().get(f).and_then(|f| f.device) else {
                continue;
            };
            if !q
                .devices()
                .get(terminal)
                .is_some_and(|d| d.aspects.terminal.is_some())
            {
                continue;
            }
            for c in q.get::<FunctionCps>(f).iter() {
                if let Some(at) = cp_point(q, f, c.idx, page) {
                    points.push((at, terminal));
                }
            }
        }
        if points.is_empty() {
            return Vec::new();
        }
        // Each terminal's CP points on the page, for a junction standing on one.
        let mut cp_points: BTreeMap<Id<Device>, BTreeSet<(i64, i64)>> = BTreeMap::new();
        for (at, terminal) in &points {
            cp_points.entry(*terminal).or_default().insert((at.x.0, at.y.0));
        }
        // The drawn segments of the page's wires and cores, by line.
        let mut vertical: BTreeMap<i64, Line> = BTreeMap::new();
        let mut horizontal: BTreeMap<i64, Line> = BTreeMap::new();
        let mut other: Vec<(Pt2, Pt2, Id<Connection>)> = Vec::new();
        for id in q.idx().connections_by_page(page) {
            let Some(c) = q.connections().get(id) else {
                continue;
            };
            if !matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_)) {
                continue;
            }
            let Some(path) = drawn_path(q, c, page) else {
                continue;
            };
            for s in path.windows(2) {
                let (a, b) = (s[0], s[1]);
                if a == b {
                    continue;
                }
                if a.x == b.x {
                    vertical.entry(a.x.0).or_default().segments.push((
                        a.y.0.min(b.y.0),
                        a.y.0.max(b.y.0),
                        id,
                    ));
                } else if a.y == b.y {
                    horizontal.entry(a.y.0).or_default().segments.push((
                        a.x.0.min(b.x.0),
                        a.x.0.max(b.x.0),
                        id,
                    ));
                } else {
                    other.push((a, b, id));
                }
            }
        }
        for line in vertical.values_mut().chain(horizontal.values_mut()) {
            line.seal();
        }
        // Whether an endpoint is a CP of a function of `terminal`'s device.
        let of_terminal = |e: &Endpoint, terminal: Id<Device>| matches!(e, Endpoint::Cp(f, _) if q.functions().get(*f).and_then(|f| f.device) == Some(terminal));
        // Whether connection `c` joins `terminal`: an end at its CP, or at a junction standing on one of its CP points
        // that a connection joins to its CP (decision log ID-1110).
        let joins = |c: &Connection, terminal: Id<Device>| {
            [&c.a, &c.b].into_iter().any(|e| match e {
                Endpoint::Junction(j) => {
                    q.junctions().get(*j).is_some_and(|j| {
                        j.page == page
                            && cp_points
                                .get(&terminal)
                                .is_some_and(|p| p.contains(&(j.at.x.0, j.at.y.0)))
                    }) && q
                        .idx()
                        .connections_at(EndpointKey::Junction(*j))
                        .filter_map(|other| q.connections().get(other))
                        .any(|other| [&other.a, &other.b].into_iter().any(|e| of_terminal(e, terminal)))
                }
                e => of_terminal(e, terminal),
            })
        };
        let mut hits: BTreeSet<(Id<Device>, Id<Connection>)> = BTreeSet::new();
        let mut on: Vec<Id<Connection>> = Vec::new();
        for (at, terminal) in points {
            on.clear();
            if let Some(line) = vertical.get(&at.x.0) {
                line.holding(at.y.0, &mut on);
            }
            if let Some(line) = horizontal.get(&at.y.0) {
                line.holding(at.x.0, &mut on);
            }
            on.extend(other.iter().filter(|s| on_segment(at, s.0, s.1)).map(|s| s.2));
            for &id in &on {
                if hits.contains(&(terminal, id)) {
                    continue;
                }
                if q.connections().get(id).is_some_and(|c| !joins(c, terminal)) {
                    hits.insert((terminal, id));
                }
            }
        }
        hits.into_iter()
            .filter_map(|(terminal, id)| {
                let aspect = q.devices().get(terminal)?.aspects.terminal.clone()?;
                let subject = format!(
                    "{}:{}",
                    full_tag(q, aspect.strip, "strip"),
                    aspect.designation.as_str()
                );
                let wire = wire_number(q, id).unwrap_or_else(|| "?".to_owned());
                Some(
                    Finding::new(
                        &TERMINAL_ON_UNJOINED_WIRE,
                        [
                            Target::new(Kind::Device, terminal),
                            Target::new(Kind::Connection, id),
                        ],
                        "",
                        subject.clone(),
                        format!(
                            "{subject} lies on wire {wire} without joining it; connect it to the wire or move it"
                        ),
                    )
                    .on_page(page),
                )
            })
            .collect()
    }
}

/// The printed number of the physical wire connection `id` belongs to (`102.3`), or `None` (a cable core, a wire on
/// no potential).
fn wire_number(q: &Q, id: Id<Connection>) -> Option<String> {
    let c = q.connections().get(id)?;
    let partition = q.get::<PotentialPartition>(());
    let potential = [&c.a, &c.b]
        .into_iter()
        .filter_map(EndpointKey::of)
        .find_map(|e| partition.potential_at(e).map(|p| p.id))?;
    let wire = q
        .get::<PhysicalWiresOf>(potential)
        .wires
        .iter()
        .find(|w| w.constituents.contains(&id))
        .map(|w| w.id)?;
    q.get::<WireNumbers>(()).number(wire).map(|n| n.label.clone())
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

    /// C45-p1#5: a line's search finds exactly the segments holding a coordinate, ends included, with a long segment
    /// over short ones, nested and touching segments and empty lines; every coordinate from before the first start to
    /// after the last end, against a scan of every segment.
    #[test]
    fn a_line_finds_the_segments_holding_a_coordinate() {
        let wire = |n: u128| Id::<Connection>::from_uuid(ew_derive::ew_ids::Uuid::from_u128(n));
        let mut line = Line::default();
        for (start, end, n) in [
            (0, 100, 1),
            (10, 20, 2),
            (30, 40, 3),
            (50, 60, 4),
            (55, 70, 5),
            (60, 60, 6),
            (70, 90, 7),
            (95, 120, 8),
        ] {
            line.segments.push((start, end, wire(n)));
        }
        line.seal();
        for v in -2..=122 {
            let mut got = Vec::new();
            line.holding(v, &mut got);
            got.sort();
            let mut want: Vec<Id<Connection>> = line
                .segments
                .iter()
                .filter(|s| s.0 <= v && v <= s.1)
                .map(|s| s.2)
                .collect();
            want.sort();
            assert_eq!(got, want, "at {v}");
        }
        let mut got = Vec::new();
        line.holding(60, &mut got);
        got.sort();
        assert_eq!(got, [wire(1), wire(4), wire(5), wire(6)]);
        let mut empty = Line::default();
        empty.seal();
        empty.holding(0, &mut got);
        assert_eq!(got.len(), 4, "an empty line adds nothing");
    }
}
