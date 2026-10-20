// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conductors → connections and junctions (doc 14 §4.7).
//!
//! A conductor end names its element by UUID (`element{i}`) and its terminal by a definition terminal UUID or
//! by a per-folio integer terminal ID; the element mapper keys placed elements with
//! [`element_keys`]/[`terminal_ids`], so this module resolves ends against the same keys. A conductor with an
//! end that does not resolve to a connection point (or a junction) is dropped with EW-IMP-003; the others
//! become connections whose IDs derive from the pair of end keys, so a re-import reproduces them.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::Ident;
use ew_schema::{
    Connection, ConnectionKind, Endpoint, Junction, JunctionStyle, JunctionWiring, NumberSpec, WireOverrides,
};
use ew_units::{Nm, Pt2};
use serde_json::Value;

use super::cx::{ElementKey, Findings, Ids, Placed, Registry, Target, element_keys, terminal_ids};
use super::model::{End, QetConductor, QetDiagram, QetProject, Segment, TerminalRef};

/// What the conductor mapper emits.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct WireOut {
    /// The connections, in diagram then conductor document order.
    pub connections: Vec<Connection>,
    /// The junctions, one per [`super::cx::Target::Junction`] element, in registry key order.
    pub junctions: Vec<Junction>,
    /// Conductors dropped because an end did not resolve or both ends resolved to the same CP.
    pub lost: u32,
}

/// The number of conductor ends attached to each placed element, by element key, before mapping: the
/// element mapper turns a junction element (one terminal, at least three conductors) into a junction.
pub fn conductor_counts(project: &QetProject) -> BTreeMap<ElementKey, u32> {
    let mut counts = BTreeMap::new();
    for d in &project.diagrams {
        let keys = element_keys(d);
        let known: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
        let tids = terminal_ids(d, &keys);
        for c in &d.conductors {
            for end in [&c.end1, &c.end2] {
                let key = match (&end.element, &end.terminal) {
                    (Some(u), _) => known.contains(u.as_str()).then(|| u.clone()),
                    (None, TerminalRef::Id(id)) => tids.get(id.trim()).map(|(k, _)| k.clone()),
                    (None, _) => None,
                };
                if let Some(key) = key {
                    *counts.entry((d.index, key)).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

/// The lookups of one diagram for resolving conductor ends.
struct Lookup<'a> {
    diagram: &'a QetDiagram,
    /// Element key → element index.
    by_key: BTreeMap<&'a str, usize>,
    /// Terminal ID → (element key, terminal index in the instance list).
    tids: &'a BTreeMap<String, (String, usize)>,
}

/// A resolved conductor end.
struct Resolved<'a> {
    /// The definition terminal index.
    index: usize,
    placed: &'a Placed,
    endpoint: Endpoint,
}

/// An unresolved conductor end: the reason and the placed element when one was found.
struct Unresolved<'a> {
    reason: &'static str,
    placed: Option<&'a Placed>,
}

/// One candidate binding of a conductor end.
type EndResult<'a> = Result<Resolved<'a>, Unresolved<'a>>;

fn unresolved<'a>(reason: &'static str, placed: Option<&'a Placed>) -> Unresolved<'a> {
    Unresolved { reason, placed }
}

/// The definition terminal index of the instance terminal `inst` of element `key`: the unique definition
/// terminal at the instance terminal's coordinates, else the list position.
fn index_of_id(lk: &Lookup<'_>, placed: &Placed, key: &str, inst: usize) -> Option<usize> {
    let terminal = lk
        .by_key
        .get(key)
        .and_then(|&e| lk.diagram.elements.get(e))
        .and_then(|e| e.terminals.get(inst));
    if let Some((Some(x), Some(y))) = terminal.map(|t| (t.x, t.y))
        && let (Ok(x), Ok(y)) = (x.px_to_nm(), y.px_to_nm())
    {
        let want = Pt2::new(x, y);
        let mut matching = placed
            .terminals
            .iter()
            .enumerate()
            .filter(|(_, t)| t.local == want);
        if let (Some((j, _)), None) = (matching.next(), matching.next()) {
            return Some(j);
        }
    }
    (inst < placed.terminals.len()).then_some(inst)
}

/// The endpoint of the terminal `index` of `placed`.
fn bind(placed: &Placed, index: usize) -> EndResult<'_> {
    let endpoint = match &placed.target {
        Target::Function { function, cps } => match cps.get(index) {
            Some(Some(idx)) => Endpoint::Cp(*function, *idx),
            _ => {
                return Err(unresolved("terminal maps to no connection point", Some(placed)));
            }
        },
        Target::Junction { id, .. } => Endpoint::Junction(*id),
        Target::Dropped => return Err(unresolved("element was not imported", Some(placed))),
    };
    Ok(Resolved {
        index,
        placed,
        endpoint,
    })
}

/// Resolves the terminal of an end that names the element `key` (the placed element `placed`).
fn resolve_at<'a>(terminal: &TerminalRef, lk: &Lookup<'_>, key: &str, placed: &'a Placed) -> EndResult<'a> {
    let index = match terminal {
        TerminalRef::Uuid(t) => placed
            .terminals
            .iter()
            .position(|p| p.uuid.as_deref() == Some(t.as_str()))
            .ok_or_else(|| unresolved("terminal not found", Some(placed)))?,
        TerminalRef::Id(id) => {
            let inst = match lk.tids.get(id.trim()) {
                Some((k, inst)) if k == key => *inst,
                _ => {
                    return Err(unresolved(
                        "terminal id does not belong to the element",
                        Some(placed),
                    ));
                }
            };
            index_of_id(lk, placed, key, inst)
                .ok_or_else(|| unresolved("terminal not found", Some(placed)))?
        }
        TerminalRef::Missing | TerminalRef::Other(_) => {
            if placed.terminals.len() == 1 {
                0
            } else {
                return Err(unresolved("terminal not specified", Some(placed)));
            }
        }
    };
    bind(placed, index)
}

/// The candidate bindings of a conductor end, never empty. An end naming its element by UUID `u` has one
/// candidate per placed copy of `u` on the folio (element keys `u`, `u#2`, `u#3`, ...) in document order; every
/// other end has exactly one.
fn resolve_end<'a>(end: &End, lk: &Lookup<'_>, reg: &'a Registry) -> Vec<EndResult<'a>> {
    match (&end.element, &end.terminal) {
        (Some(u), terminal) => {
            let mut out = Vec::new();
            for n in 1u32.. {
                let key = if n == 1 { u.clone() } else { format!("{u}#{n}") };
                let Some(placed) = reg.get(&(lk.diagram.index, key.clone())) else {
                    break;
                };
                out.push(resolve_at(terminal, lk, &key, placed));
            }
            if out.is_empty() {
                out.push(Err(unresolved("element not found", None)));
            }
            out
        }
        (None, TerminalRef::Id(id)) => {
            let Some((key, inst)) = lk.tids.get(id.trim()) else {
                return vec![Err(unresolved("terminal id not found", None))];
            };
            let Some(placed) = reg.get(&(lk.diagram.index, key.clone())) else {
                return vec![Err(unresolved("element not found", None))];
            };
            let Some(index) = index_of_id(lk, placed, key, *inst) else {
                return vec![Err(unresolved("terminal not found", Some(placed)))];
            };
            vec![bind(placed, index)]
        }
        (None, _) => vec![Err(unresolved("no element and no terminal id", None))],
    }
}

/// The page position of a resolved end, if known.
fn end_pos(r: &Resolved<'_>) -> Option<Pt2> {
    match &r.placed.target {
        Target::Junction { at, .. } => Some(*at),
        _ => r.placed.terminals.get(r.index).and_then(|t| t.page_pos),
    }
}

/// The points of a conductor drawn from `start` through `segments`: the point after each segment. `None` when
/// a length does not convert or the arithmetic overflows.
fn points_of(start: Pt2, segments: &[Segment]) -> Option<Vec<Pt2>> {
    let mut p = start;
    let mut points = Vec::with_capacity(segments.len());
    for s in segments {
        let len = s.length.px_to_nm().ok()?;
        p = if s.horizontal {
            Pt2::new(p.x.checked_add(len)?, p.y)
        } else {
            Pt2::new(p.x, p.y.checked_add(len)?)
        };
        points.push(p);
    }
    Some(points)
}

/// The interior route points of a conductor drawn from `start` through `segments`: the point after each
/// segment except the last (which is the other end). Empty when `start` is unknown, a length does not
/// convert or the arithmetic overflows.
fn route_of(start: Option<Pt2>, segments: &[Segment]) -> Vec<Pt2> {
    let mut points = start.and_then(|p| points_of(p, segments)).unwrap_or_default();
    points.pop();
    points
}

/// The displacement from the first to the last point of a conductor drawn through `segments`, nm; `None`
/// without segments, when a length does not convert or the sum overflows.
fn displacement(segments: &[Segment]) -> Option<(i64, i64)> {
    if segments.is_empty() {
        return None;
    }
    let (mut dx, mut dy) = (0i64, 0i64);
    for seg in segments {
        let len = seg.length.px_to_nm().ok()?.0;
        if seg.horizontal {
            dx = dx.checked_add(len)?;
        } else {
            dy = dy.checked_add(len)?;
        }
    }
    Some((dx, dy))
}

/// The indices of the candidates that resolved.
fn resolved_indices(candidates: &[EndResult<'_>]) -> Vec<usize> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.is_ok().then_some(i))
        .collect()
}

/// Picks one candidate per end. An end with a single resolving candidate takes it. When an end has several
/// (a UUID repeated on the folio), the end-2 candidates are indexed by the page position of their terminal,
/// and each end-1 candidate looks up the position where the conductor's segments end from it: exactly one
/// match decides both ends. Otherwise the first resolving candidate of each ambiguous end is bound and that end
/// is flagged (the second value; EW-IMP-120). An end without a resolving candidate keeps its first candidate's
/// failure.
///
/// Cost: one index insertion per end-2 candidate and one lookup per end-1 candidate, each counted in
/// `comparisons`, so a conductor between k copies costs O(k log k), never the k² pairs (doc 14 §11, ID-115).
fn choose_ends<'a>(
    segments: &[Segment],
    candidates: [Vec<EndResult<'a>>; 2],
    comparisons: &mut u64,
) -> ([EndResult<'a>; 2], [bool; 2]) {
    let oks = [resolved_indices(&candidates[0]), resolved_indices(&candidates[1])];
    let mut pick = [
        oks[0].first().copied().unwrap_or(0),
        oks[1].first().copied().unwrap_or(0),
    ];
    let mut repeated = [oks[0].len() > 1, oks[1].len() > 1];
    if (repeated[0] || repeated[1])
        && let Some((dx, dy)) = displacement(segments)
    {
        let mut at: BTreeMap<Pt2, Vec<usize>> = BTreeMap::new();
        for &j in &oks[1] {
            *comparisons += 1;
            if let Ok(b) = &candidates[1][j]
                && let Some(pos) = end_pos(b)
            {
                at.entry(pos).or_default().push(j);
            }
        }
        let mut found: Option<(usize, usize)> = None;
        let mut matches = 0usize;
        for &i in &oks[0] {
            *comparisons += 1;
            let Ok(a) = &candidates[0][i] else { continue };
            let Some(from) = end_pos(a) else { continue };
            let (Some(x), Some(y)) = (from.x.0.checked_add(dx), from.y.0.checked_add(dy)) else {
                continue;
            };
            if let Some(js) = at.get(&Pt2::new(Nm(x), Nm(y))) {
                matches += js.len();
                found = Some((i, js[0]));
                if matches > 1 {
                    break;
                }
            }
        }
        if let (1, Some((i, j))) = (matches, found) {
            pick = [i, j];
            repeated = [false, false];
        }
    }
    let [c1, c2] = candidates;
    let take = |c: Vec<EndResult<'a>>, i: usize| {
        c.into_iter()
            .nth(i)
            .unwrap_or_else(|| Err(unresolved("element not found", None)))
    };
    ([take(c1, pick[0]), take(c2, pick[1])], repeated)
}

/// The source key of one conductor end: `<element key>/<terminal index>`, `/j` for a junction; the element
/// key is the project-wide one ([`Placed::ek`]).
fn end_key(r: &Resolved<'_>) -> String {
    let ek = &r.placed.ek;
    match r.placed.target {
        Target::Junction { .. } => format!("{ek}/j"),
        _ => format!("{ek}/{}", r.index),
    }
}

/// The conductor value of one text field kept as `extra` (see [`TEXT_FIELDS`]).
type FieldValue = fn(&QetConductor) -> &str;

/// The conductor properties kept as text: field name in the summary, `extra` key, value of a conductor.
const TEXT_FIELDS: [(&str, &str, FieldValue); 4] = [
    ("function", "qet.function", |c| &c.function),
    ("tension_protocol", "qet.tension_protocol", |c| {
        &c.tension_protocol
    }),
    ("conductor_color", "qet.color", |c| &c.color),
    ("conductor_section", "qet.section", |c| &c.section),
];

/// Maps every conductor of `project` against the placed elements of `reg`.
pub fn map_conductors(project: &QetProject, reg: &Registry, ids: &Ids, findings: &mut Findings) -> WireOut {
    map_conductors_counted(project, reg, ids, findings).0
}

/// [`map_conductors`], also returning the number of candidate comparisons made to bind the ends of repeated
/// UUIDs (for the structural complexity test).
fn map_conductors_counted(
    project: &QetProject,
    reg: &Registry,
    ids: &Ids,
    findings: &mut Findings,
) -> (WireOut, u64) {
    let mut comparisons = 0u64;
    let mut out = WireOut::default();
    let mut repeats: BTreeMap<String, u32> = BTreeMap::new();
    // Per text field: conductors with a value and the distinct values.
    let mut stats: [(u32, BTreeSet<String>); 4] = Default::default();

    for d in &project.diagrams {
        let keys = element_keys(d);
        let tids = terminal_ids(d, &keys);
        let lk = Lookup {
            diagram: d,
            by_key: keys.iter().enumerate().map(|(i, k)| (k.as_str(), i)).collect(),
            tids: &tids,
        };
        let diagram_folio = reg
            .range((d.index, String::new())..)
            .next()
            .filter(|((di, _), _)| *di == d.index)
            .map(|(_, p)| p.folio.as_str());

        for (n, c) in d.conductors.iter().enumerate() {
            let item = || Some(format!("conductor {n}"));
            let (ends, repeated) = choose_ends(
                &c.segments,
                [resolve_end(&c.end1, &lk, reg), resolve_end(&c.end2, &lk, reg)],
                &mut comparisons,
            );
            let failure = ends
                .iter()
                .enumerate()
                .find_map(|(i, r)| r.as_ref().err().map(|e| (i + 1, e)));
            if let Some((which, e)) = failure {
                let folio = e.placed.map(|p| p.folio.as_str()).or(diagram_folio);
                let loc = findings.loc(folio, item(), Some(c.line));
                findings.push(
                    Code::UNRESOLVED_REFERENCE,
                    Severity::Error,
                    loc,
                    format!("conductor dropped: end {which}: {}", e.reason),
                );
                out.lost += 1;
                continue;
            }
            let [Ok(e1), Ok(e2)] = ends else {
                continue;
            };
            let folio = Some(e1.placed.folio.as_str());
            if e1.endpoint == e2.endpoint {
                let loc = findings.loc(folio, item(), Some(c.line));
                findings.push(
                    Code::UNRESOLVED_REFERENCE,
                    Severity::Error,
                    loc,
                    "conductor dropped: both ends resolve to the same connection point",
                );
                out.lost += 1;
                continue;
            }

            for (i, (rep, e)) in repeated.iter().zip([&e1, &e2]).enumerate() {
                if *rep {
                    let loc = findings.loc(Some(e.placed.folio.as_str()), item(), Some(c.line));
                    findings.push(
                        Code::imp(120),
                        Severity::Warning,
                        loc,
                        format!(
                            "conductor end {}: element UUID repeated on this folio; bound to the first copy (geometry does not decide)",
                            i + 1
                        ),
                    );
                }
            }

            let (mut k1, mut k2) = (end_key(&e1), end_key(&e2));
            if k2 < k1 {
                std::mem::swap(&mut k1, &mut k2);
            }
            let base = format!("qet:wire:{k1}|{k2}");
            let seen = repeats.entry(base.clone()).or_insert(0);
            *seen += 1;
            let key = if *seen == 1 {
                base
            } else {
                format!("{base}#{seen}")
            };

            let mut extra = BTreeMap::new();
            for (i, (_, extra_key, value)) in TEXT_FIELDS.iter().enumerate() {
                let v = value(c).trim();
                if !v.is_empty() {
                    extra.insert((*extra_key).to_owned(), Value::String(v.to_owned()));
                    stats[i].0 += 1;
                    stats[i].1.insert(v.to_owned());
                }
            }
            let number = match c.num.trim() {
                "" => NumberSpec::Unassigned,
                num => match Ident::from_input(num) {
                    Ok(ident) => NumberSpec::Frozen(ident),
                    Err(_) => {
                        let loc = findings.loc(folio, item(), Some(c.line));
                        findings.push(
                            Code::FOREIGN_VALUE,
                            Severity::Warning,
                            loc,
                            format!("wire number {num:?} kept as text"),
                        );
                        extra.insert("qet.num".to_owned(), Value::String(num.to_owned()));
                        NumberSpec::Unassigned
                    }
                },
            };

            out.connections.push(Connection {
                id: ids.id(&key),
                v: 1,
                a: e1.endpoint.clone(),
                b: e2.endpoint.clone(),
                page: Some(e1.placed.page),
                route: route_of(end_pos(&e1), &c.segments),
                kind: ConnectionKind::default(),
                overrides: WireOverrides::default(),
                number,
                source_ref: Some(ids.source_ref(&key)),
                extra,
            });
        }
    }

    for ((name, _, _), (count, distinct)) in TEXT_FIELDS.iter().zip(&stats) {
        if *count > 0 {
            findings.file_level(
                Code::FOREIGN_VALUE,
                Severity::Warning,
                format!(
                    "conductor {name} kept as text on {count} conductors ({} distinct values); not mapped to wire properties",
                    distinct.len()
                ),
            );
        }
    }

    let mut at_junction: BTreeMap<Id<Junction>, Vec<Id<Connection>>> = BTreeMap::new();
    for c in &out.connections {
        for e in [&c.a, &c.b] {
            if let Endpoint::Junction(j) = e {
                at_junction.entry(*j).or_default().push(c.id);
            }
        }
    }
    for placed in reg.values() {
        let Target::Junction { id, key, at } = &placed.target else {
            continue;
        };
        out.junctions.push(Junction {
            id: *id,
            v: 1,
            page: placed.page,
            at: *at,
            style: JunctionStyle::Dot,
            wiring: Some(JunctionWiring::Star),
            sequence: at_junction.get(id).cloned().unwrap_or_default(),
            source_ref: Some(ids.source_ref(key)),
            extra: BTreeMap::new(),
        });
        let loc = findings.loc(Some(&placed.folio), Some(key.clone()), None);
        findings.push(
            Code::imp(118),
            Severity::Info,
            loc,
            "junction wiring assumed (star) at a QET junction element",
        );
    }
    (out, comparisons)
}

#[cfg(test)]
mod tests {
    use ew_ids::Uuid;
    use ew_interop_api::{Code, Severity};
    use ew_schema::{Function, Page};
    use ew_units::{Grid, Nm};
    use serde_json::json;

    use super::*;
    use crate::decimal::Decimal;
    use crate::project::cx::PlacedTerminal;
    use crate::project::model::{InstTerminal, QetElement};

    fn uid(n: u128) -> String {
        Uuid::from_u128(n).to_string()
    }

    fn ids() -> Ids {
        Ids {
            namespace: Uuid::from_u128(1),
            lineage: Id::from_uuid(Uuid::from_u128(2)),
        }
    }

    fn function(n: u128) -> Id<Function> {
        Id::from_uuid(Uuid::from_u128(n))
    }

    fn px(n: i64) -> Nm {
        Decimal::from_int(n).px_to_nm().unwrap()
    }

    fn inst(id: &str, xy: Option<(i64, i64)>) -> InstTerminal {
        InstTerminal {
            id: Some(id.to_owned()),
            x: xy.map(|(x, _)| Decimal::from_int(x)),
            y: xy.map(|(_, y)| Decimal::from_int(y)),
        }
    }

    fn element(uuid: Option<String>, terminals: Vec<InstTerminal>) -> QetElement {
        QetElement {
            n: 0,
            type_path: "embed://t.elmt".to_owned(),
            uuid,
            x: Decimal::ZERO,
            y: Decimal::ZERO,
            orientation: 0,
            prefix: String::new(),
            terminals,
            infos: Vec::new(),
            dyn_texts: Vec::new(),
            links: Vec::new(),
            line: 1,
        }
    }

    /// A placed terminal at definition position `(x, y)` px; its page position is offset by (1000, 2000) nm.
    fn pterm(uuid: Option<String>, x: i64, y: i64) -> PlacedTerminal {
        let local = Pt2::new(px(x), px(y));
        PlacedTerminal {
            uuid,
            local,
            page_pos: Some(Pt2::new(Nm(local.x.0 + 1000), Nm(local.y.0 + 2000))),
        }
    }

    /// A placed element of folio F1 with the project-wide element key `ek`.
    fn placed(ek: &str, target: Target, terminals: Vec<PlacedTerminal>) -> Placed {
        Placed {
            diagram: 0,
            page: Id::<Page>::from_uuid(Uuid::from_u128(9)),
            folio: "F1".to_owned(),
            ek: ek.to_owned(),
            target,
            terminals,
        }
    }

    fn func(n: u128, cps: Vec<Option<u16>>) -> Target {
        Target::Function {
            function: function(n),
            cps,
        }
    }

    fn end_u(element: &str, terminal: &str) -> End {
        End {
            element: Some(element.to_owned()),
            terminal: TerminalRef::Uuid(terminal.to_owned()),
        }
    }

    fn end_id(id: &str) -> End {
        End {
            element: None,
            terminal: TerminalRef::Id(id.to_owned()),
        }
    }

    fn cond(end1: End, end2: End) -> QetConductor {
        QetConductor {
            end1,
            end2,
            num: String::new(),
            function: String::new(),
            tension_protocol: String::new(),
            color: String::new(),
            section: String::new(),
            segments: Vec::new(),
            line: 7,
        }
    }

    fn diagram(elements: Vec<QetElement>, conductors: Vec<QetConductor>) -> QetDiagram {
        QetDiagram {
            elements,
            conductors,
            ..QetDiagram::default()
        }
    }

    fn project(diagrams: Vec<QetDiagram>) -> QetProject {
        QetProject {
            diagrams,
            ..QetProject::default()
        }
    }

    fn run(p: &QetProject, reg: &Registry) -> (WireOut, Findings) {
        let mut f = Findings::new("t.qet", 100);
        let out = map_conductors(p, reg, &ids(), &mut f);
        (out, f)
    }

    fn codes(f: &Findings) -> Vec<(Code, Severity)> {
        f.entries().iter().map(|e| (e.code, e.severity)).collect()
    }

    /// Two uuid elements A (terminals ta0, ta1) and B (terminal tb0), with registry.
    fn two_elements() -> (QetDiagram, Registry) {
        let (a, b) = (uid(10), uid(11));
        let d = diagram(
            vec![
                element(
                    Some(a.clone()),
                    vec![inst("1", Some((0, 0))), inst("2", Some((10, 0)))],
                ),
                element(Some(b.clone()), vec![inst("3", Some((0, 0)))]),
            ],
            Vec::new(),
        );
        let mut reg = Registry::new();
        reg.insert(
            (0, a.clone()),
            placed(
                &a,
                func(100, vec![Some(0), Some(1)]),
                vec![pterm(Some(uid(20)), 0, 0), pterm(Some(uid(21)), 10, 0)],
            ),
        );
        reg.insert(
            (0, b.clone()),
            placed(&b, func(101, vec![Some(0)]), vec![pterm(Some(uid(22)), 0, 0)]),
        );
        (d, reg)
    }

    #[test]
    fn uuid_ends_resolve_to_cp_endpoints() {
        let (mut d, reg) = two_elements();
        d.conductors
            .push(cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0);
        assert!(f.entries().is_empty());
        assert_eq!(out.connections.len(), 1);
        let c = &out.connections[0];
        assert_eq!(c.a, Endpoint::Cp(function(100), 1));
        assert_eq!(c.b, Endpoint::Cp(function(101), 0));
        assert_eq!(c.page, Some(Id::from_uuid(Uuid::from_u128(9))));
        let key = format!("qet:wire:{}/1|{}/0", uid(10), uid(11));
        assert_eq!(c.source_ref.as_ref().unwrap().key, key);
        assert_eq!(c.id, ids().id::<Connection>(&key));
        assert_eq!(c.number, NumberSpec::Unassigned);
    }

    #[test]
    fn integer_ends_use_coordinates_then_list_position() {
        // Instance terminal 0 sits at the definition position of terminal 1 (reordered list); terminals
        // 1 and 2 carry no coordinates and fall back to the list position.
        let b = uid(11);
        let d = diagram(
            vec![
                element(
                    None,
                    vec![inst("1", Some((10, 0))), inst("2", None), inst("3", None)],
                ),
                element(Some(b.clone()), vec![inst("9", Some((0, 0)))]),
            ],
            vec![
                cond(end_id("1"), end_u(&b, &uid(22))),
                cond(end_id("3"), end_u(&b, &uid(22))),
            ],
        );
        let mut reg = Registry::new();
        reg.insert(
            (0, "#0".to_owned()),
            placed(
                "d0#0",
                func(100, vec![Some(10), Some(11), Some(12)]),
                vec![pterm(None, 0, 0), pterm(None, 10, 0), pterm(None, 20, 0)],
            ),
        );
        reg.insert(
            (0, b.clone()),
            placed(&b, func(101, vec![Some(0)]), vec![pterm(Some(uid(22)), 0, 0)]),
        );
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0, "{:?}", f.entries());
        assert_eq!(out.connections[0].a, Endpoint::Cp(function(100), 11));
        assert_eq!(out.connections[1].a, Endpoint::Cp(function(100), 12));
        // The uuid-less element's key is folio-local: prefixed with the diagram index.
        let key = format!("qet:wire:{}/0|d0#0/1", uid(11));
        assert_eq!(out.connections[0].source_ref.as_ref().unwrap().key, key);
    }

    #[test]
    fn integer_end_with_element_must_name_that_element() {
        let (mut d, reg) = two_elements();
        // Terminal id 3 belongs to element B, not A.
        d.conductors.push(cond(
            End {
                element: Some(uid(10)),
                terminal: TerminalRef::Id("3".to_owned()),
            },
            end_u(&uid(11), &uid(22)),
        ));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 1);
        assert_eq!(codes(&f), vec![(Code::UNRESOLVED_REFERENCE, Severity::Error)]);
    }

    #[test]
    fn unresolved_element_is_lost_and_reported() {
        let (mut d, reg) = two_elements();
        d.conductors
            .push(cond(end_u(&uid(10), &uid(21)), end_u(&uid(99), &uid(22))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 1);
        assert!(out.connections.is_empty());
        let e = &f.entries()[0];
        assert_eq!(
            (e.code, e.severity),
            (Code::UNRESOLVED_REFERENCE, Severity::Error)
        );
        assert_eq!(e.message, "conductor dropped: end 2: element not found");
        let loc = e.location.as_ref().unwrap();
        assert_eq!(loc.folio.as_deref(), Some("F1"));
        assert_eq!(loc.item.as_deref(), Some("conductor 0"));
        assert_eq!(loc.line, Some(7));
    }

    #[test]
    fn terminal_without_cp_and_dropped_element_are_lost() {
        let (mut d, mut reg) = two_elements();
        reg.get_mut(&(0, uid(10))).unwrap().target = func(100, vec![Some(0), None]);
        reg.get_mut(&(0, uid(11))).unwrap().target = Target::Dropped;
        d.conductors
            .push(cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22))));
        d.conductors
            .push(cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 2);
        assert_eq!(
            f.entries()[0].message,
            "conductor dropped: end 1: terminal maps to no connection point"
        );
        assert_eq!(
            f.entries()[1].message,
            "conductor dropped: end 2: element was not imported"
        );
    }

    #[test]
    fn same_cp_at_both_ends_is_lost() {
        let (mut d, reg) = two_elements();
        d.conductors
            .push(cond(end_u(&uid(10), &uid(20)), end_u(&uid(10), &uid(20))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 1);
        assert!(out.connections.is_empty());
        assert_eq!(
            f.entries()[0].message,
            "conductor dropped: both ends resolve to the same connection point"
        );
    }

    #[test]
    fn parallel_conductors_get_distinct_ids() {
        let (mut d, reg) = two_elements();
        let c = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        // The same pair in the opposite direction is the same base key.
        let rev = cond(end_u(&uid(11), &uid(22)), end_u(&uid(10), &uid(20)));
        d.conductors.extend([c.clone(), rev, c]);
        let (out, _) = run(&project(vec![d]), &reg);
        let keys: Vec<&str> = out
            .connections
            .iter()
            .map(|c| c.source_ref.as_ref().unwrap().key.as_str())
            .collect();
        let base = format!("qet:wire:{}/0|{}/0", uid(10), uid(11));
        assert_eq!(keys, vec![base.clone(), format!("{base}#2"), format!("{base}#3")]);
        let distinct: BTreeSet<_> = out.connections.iter().map(|c| c.id).collect();
        assert_eq!(distinct.len(), 3);
    }

    #[test]
    fn route_replays_segments_without_the_last_point() {
        let (mut d, reg) = two_elements();
        let mut c = cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22)));
        c.segments = vec![
            Segment {
                horizontal: true,
                length: Decimal::from_int(10),
            },
            Segment {
                horizontal: false,
                length: Decimal::from_int(10),
            },
        ];
        d.conductors.push(c);
        let (out, _) = run(&project(vec![d]), &reg);
        let start = reg[&(0, uid(10))].terminals[1].page_pos.unwrap();
        let step = Grid::QET_10PX.at(1).unwrap();
        assert_eq!(step, px(10));
        assert_eq!(
            out.connections[0].route,
            vec![Pt2::new(Nm(start.x.0 + step.0), start.y)]
        );
    }

    #[test]
    fn route_is_empty_without_segments_or_on_overflow() {
        let seg = |horizontal, n| Segment {
            horizontal,
            length: Decimal::from_int(n),
        };
        assert!(route_of(Some(Pt2::ORIGIN), &[]).is_empty());
        assert!(route_of(None, &[seg(true, 1), seg(false, 1)]).is_empty());
        assert!(
            route_of(
                Some(Pt2::new(Nm(i64::MAX), Nm(0))),
                &[seg(true, 1), seg(false, 1)]
            )
            .is_empty()
        );
        assert!(route_of(Some(Pt2::ORIGIN), &[seg(true, 1)]).is_empty());
    }

    #[test]
    fn junction_element_gets_junction_with_both_connections() {
        let (a, j) = (uid(10), uid(12));
        let d = diagram(
            vec![
                element(
                    Some(a.clone()),
                    vec![inst("1", Some((0, 0))), inst("2", Some((10, 0)))],
                ),
                element(Some(j.clone()), vec![inst("3", Some((0, 0)))]),
            ],
            vec![
                cond(end_u(&a, &uid(20)), end_u(&j, &uid(23))),
                cond(end_u(&a, &uid(21)), end_id("3")),
            ],
        );
        let jid: Id<Junction> = Id::from_uuid(Uuid::from_u128(77));
        let at = Pt2::new(Nm(5), Nm(6));
        let mut reg = Registry::new();
        reg.insert(
            (0, a.clone()),
            placed(
                &a,
                func(100, vec![Some(0), Some(1)]),
                vec![pterm(Some(uid(20)), 0, 0), pterm(Some(uid(21)), 10, 0)],
            ),
        );
        reg.insert(
            (0, j.clone()),
            placed(
                &j,
                Target::Junction {
                    id: jid,
                    key: format!("qet:junction:{j}"),
                    at,
                },
                vec![pterm(Some(uid(23)), 0, 0)],
            ),
        );
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0);
        assert_eq!(out.connections.len(), 2);
        assert_eq!(out.connections[0].b, Endpoint::Junction(jid));
        assert_eq!(out.connections[1].b, Endpoint::Junction(jid));
        assert_eq!(out.junctions.len(), 1);
        let junction = &out.junctions[0];
        assert_eq!(junction.id, jid);
        assert_eq!(junction.at, at);
        assert_eq!(junction.style, JunctionStyle::Dot);
        assert_eq!(junction.wiring, Some(JunctionWiring::Star));
        assert_eq!(
            junction.sequence,
            vec![out.connections[0].id, out.connections[1].id]
        );
        assert_eq!(
            junction.source_ref.as_ref().unwrap().key,
            format!("qet:junction:{j}")
        );
        let e = f.entries().last().unwrap();
        assert_eq!((e.code, e.severity), (Code::imp(118), Severity::Info));
        assert_eq!(
            e.message,
            "junction wiring assumed (star) at a QET junction element"
        );
        assert_eq!(
            e.location.as_ref().unwrap().item.as_deref(),
            Some(format!("qet:junction:{j}").as_str())
        );
    }

    #[test]
    fn text_properties_are_kept_in_extra_and_summarised() {
        let (mut d, reg) = two_elements();
        let mut c1 = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        c1.function = " PE ".to_owned();
        c1.color = "BK".to_owned();
        c1.section = "1.5".to_owned();
        let mut c2 = cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22)));
        c2.function = "N".to_owned();
        c2.color = "BK".to_owned();
        d.conductors.extend([c1, c2]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.connections[0].extra["qet.function"], json!("PE"));
        assert_eq!(out.connections[0].extra["qet.color"], json!("BK"));
        assert_eq!(out.connections[0].extra["qet.section"], json!("1.5"));
        assert!(!out.connections[0].extra.contains_key("qet.tension_protocol"));
        let msgs: Vec<&str> = f.entries().iter().map(|e| e.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "conductor function kept as text on 2 conductors (2 distinct values); not mapped to wire properties",
                "conductor conductor_color kept as text on 2 conductors (1 distinct values); not mapped to wire properties",
                "conductor conductor_section kept as text on 1 conductors (1 distinct values); not mapped to wire properties",
            ]
        );
        assert!(
            f.entries()
                .iter()
                .all(|e| (e.code, e.severity) == (Code::FOREIGN_VALUE, Severity::Warning))
        );
        assert!(
            f.entries()
                .iter()
                .all(|e| e.location.as_ref().unwrap().folio.is_none())
        );
    }

    #[test]
    fn wire_number_is_frozen_or_kept_as_text() {
        let (mut d, reg) = two_elements();
        let mut c1 = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        c1.num = " 101 ".to_owned();
        let mut c2 = cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22)));
        c2.num = "a\tb".to_owned();
        d.conductors.extend([c1, c2]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(
            out.connections[0].number,
            NumberSpec::Frozen(Ident::from_input("101").unwrap())
        );
        assert_eq!(out.connections[1].number, NumberSpec::Unassigned);
        assert_eq!(out.connections[1].extra["qet.num"], json!("a\tb"));
        assert_eq!(codes(&f), vec![(Code::FOREIGN_VALUE, Severity::Warning)]);
        assert_eq!(
            f.entries()[0].location.as_ref().unwrap().item.as_deref(),
            Some("conductor 1")
        );
    }

    #[test]
    fn counts_include_integer_and_uuid_ends() {
        let (mut d, _) = two_elements();
        d.conductors.extend([
            cond(end_u(&uid(10), &uid(20)), end_id("3")),
            cond(end_id("2"), end_u(&uid(11), &uid(22))),
            // An unknown element and an unknown terminal id count for nothing.
            cond(end_u(&uid(99), &uid(20)), end_id("77")),
            cond(
                End {
                    element: None,
                    terminal: TerminalRef::Missing,
                },
                end_u(&uid(10), &uid(21)),
            ),
        ]);
        let counts = conductor_counts(&project(vec![d]));
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[&(0, uid(10))], 3);
        assert_eq!(counts[&(0, uid(11))], 2);
    }

    /// A placed terminal at definition position (0, 0) whose page position is `(x, y)` px.
    fn pterm_on_page(uuid: Option<String>, x: i64, y: i64) -> PlacedTerminal {
        PlacedTerminal {
            uuid,
            local: Pt2::ORIGIN,
            page_pos: Some(Pt2::new(px(x), px(y))),
        }
    }

    /// Element A (UUID 10) pasted twice on one folio, at page (0, 0) px and (100, 0) px, and element B at
    /// (100, 50) px. The copies share their terminal UUID; the second copy's element key is `a#2`.
    fn pasted_twice() -> (QetDiagram, Registry) {
        let (a, b) = (uid(10), uid(11));
        let d = diagram(
            vec![
                element(Some(a.clone()), vec![inst("1", Some((0, 0)))]),
                element(Some(a.clone()), vec![inst("2", Some((0, 0)))]),
                element(Some(b.clone()), vec![inst("3", Some((0, 0)))]),
            ],
            Vec::new(),
        );
        let mut reg = Registry::new();
        reg.insert(
            (0, a.clone()),
            placed(
                &a,
                func(100, vec![Some(0)]),
                vec![pterm_on_page(Some(uid(20)), 0, 0)],
            ),
        );
        reg.insert(
            (0, format!("{a}#2")),
            placed(
                "ek-a2",
                func(102, vec![Some(0)]),
                vec![pterm_on_page(Some(uid(20)), 100, 0)],
            ),
        );
        reg.insert(
            (0, b.clone()),
            placed(
                &b,
                func(101, vec![Some(0)]),
                vec![pterm_on_page(Some(uid(22)), 100, 50)],
            ),
        );
        (d, reg)
    }

    fn vertical(n: i64) -> Segment {
        Segment {
            horizontal: false,
            length: Decimal::from_int(n),
        }
    }

    #[test]
    fn repeated_uuid_ends_are_matched_by_position_in_linear_comparisons() {
        // k copies of A (terminal UUID 20) at (100 i, 0) px and k copies of B (terminal UUID 22) at
        // (100 i, 50 + 10 i) px; conductor j is drawn 50 + 10 j px down, so only A_j -> B_j fits it. Both ends
        // have k candidates: trying every candidate pair would make k³ comparisons over the k conductors; the
        // position index makes 2k per conductor.
        let k: usize = 50;
        let (a, b) = (uid(10), uid(11));
        let mut reg = Registry::new();
        let mut conductors = Vec::new();
        for i in 0..k {
            let n = i + 1;
            let ka = if n == 1 { a.clone() } else { format!("{a}#{n}") };
            let kb = if n == 1 { b.clone() } else { format!("{b}#{n}") };
            let x = i64::try_from(100 * i).unwrap();
            let down = 50 + 10 * i64::try_from(i).unwrap();
            let fa = 1_000 + u128::try_from(i).unwrap();
            let fb = 2_000 + u128::try_from(i).unwrap();
            let pa = vec![pterm_on_page(Some(uid(20)), x, 0)];
            let pb = vec![pterm_on_page(Some(uid(22)), x, down)];
            reg.insert((0, ka), placed(&format!("a{n}"), func(fa, vec![Some(0)]), pa));
            reg.insert((0, kb), placed(&format!("b{n}"), func(fb, vec![Some(0)]), pb));
            let mut c = cond(end_u(&a, &uid(20)), end_u(&b, &uid(22)));
            c.segments = vec![vertical(down)];
            conductors.push(c);
        }
        let d = diagram(Vec::new(), conductors);
        let mut f = Findings::new("t.qet", 1_000);
        let (out, comparisons) = map_conductors_counted(&project(vec![d]), &reg, &ids(), &mut f);
        let k64 = u64::try_from(k).unwrap();
        assert_eq!(
            comparisons,
            2 * k64 * k64,
            "one insertion and one lookup per candidate"
        );
        assert_eq!(out.lost, 0);
        assert!(f.entries().is_empty(), "{:?}", f.entries());
        for (i, c) in out.connections.iter().enumerate() {
            let i = u128::try_from(i).unwrap();
            assert_eq!(c.a, Endpoint::Cp(function(1_000 + i), 0));
            assert_eq!(c.b, Endpoint::Cp(function(2_000 + i), 0));
        }
    }

    #[test]
    fn repeated_uuid_is_bound_by_the_conductor_geometry() {
        let (mut d, reg) = pasted_twice();
        // A -> B, drawn 50 px down: only the copy at (100, 0) ends at B (100, 50).
        let mut c = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        c.segments = vec![vertical(50)];
        // B -> A, drawn 50 px up: the same copy, from the other side.
        let mut back = cond(end_u(&uid(11), &uid(22)), end_u(&uid(10), &uid(20)));
        back.segments = vec![vertical(-50)];
        d.conductors.extend([c, back]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0, "{:?}", f.entries());
        assert!(f.entries().is_empty(), "{:?}", f.entries());
        assert_eq!(out.connections[0].a, Endpoint::Cp(function(102), 0));
        assert_eq!(out.connections[0].b, Endpoint::Cp(function(101), 0));
        assert_eq!(out.connections[1].a, Endpoint::Cp(function(101), 0));
        assert_eq!(out.connections[1].b, Endpoint::Cp(function(102), 0));
        // The wire's source key names the second copy by its project-wide element key.
        let key = out.connections[0].source_ref.as_ref().unwrap().key.clone();
        assert!(key.contains("ek-a2/0"), "{key}");
    }

    #[test]
    fn repeated_uuid_without_deciding_geometry_binds_the_first_copy_and_warns() {
        let (mut d, reg) = pasted_twice();
        // No segments; and segments that fit neither copy.
        let plain = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        let mut off = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        off.segments = vec![vertical(7)];
        d.conductors.extend([plain, off]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0);
        assert_eq!(out.connections.len(), 2);
        for c in &out.connections {
            assert_eq!(c.a, Endpoint::Cp(function(100), 0));
            assert_eq!(c.b, Endpoint::Cp(function(101), 0));
        }
        assert_eq!(
            codes(&f),
            vec![
                (Code::imp(120), Severity::Warning),
                (Code::imp(120), Severity::Warning)
            ]
        );
        let e = &f.entries()[0];
        assert_eq!(
            e.message,
            "conductor end 1: element UUID repeated on this folio; bound to the first copy (geometry does not decide)"
        );
        let loc = e.location.as_ref().unwrap();
        assert_eq!(loc.folio.as_deref(), Some("F1"));
        assert_eq!(loc.item.as_deref(), Some("conductor 0"));
        assert_eq!(loc.line, Some(7));
        assert_eq!(
            f.entries()[1].location.as_ref().unwrap().item.as_deref(),
            Some("conductor 1")
        );
    }

    #[test]
    fn single_terminal_element_resolves_without_terminal_attribute() {
        let (mut d, reg) = two_elements();
        d.conductors.push(cond(
            end_u(&uid(10), &uid(20)),
            End {
                element: Some(uid(11)),
                terminal: TerminalRef::Missing,
            },
        ));
        d.conductors.push(cond(
            End {
                element: Some(uid(10)),
                terminal: TerminalRef::Missing,
            },
            end_u(&uid(11), &uid(22)),
        ));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.connections.len(), 1);
        assert_eq!(out.lost, 1);
        assert_eq!(
            f.entries()[0].message,
            "conductor dropped: end 1: terminal not specified"
        );
    }
}
