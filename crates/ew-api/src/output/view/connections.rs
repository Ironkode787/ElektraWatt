// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The connections of a page's view (see the module documentation of `view`): the wires with their markers and
//! numbers, the junctions, the connection points a report row links to and the strips' bridges.

#[allow(clippy::wildcard_imports)]
use super::*;

/// The tag of the cable a bus line is linked to (doc 10 §2.7), printed beside the line as a wire number is; `None` for
/// another connection, a line without a cable and one whose cable is drawn on the line's page (its own symbol prints
/// the tag there).
fn bus_cable_text(q: &Q, c: &ew_schema::Connection) -> Option<String> {
    let ew_schema::ConnectionKind::Bus(line) = &c.kind else {
        return None;
    };
    let cable = line.cable?;
    let drawn = q.idx().functions_of_device(cable).any(|f| {
        q.idx()
            .reps_of_function(f)
            .any(|r| q.reps().get(r).is_some_and(|r| Some(r.page) == c.page))
    });
    (!drawn).then(|| crate::output::properties::device_tag_text(q, cable))
}

/// `page_connections(page)`: the connections drawn on a page with their wire-marker texts and, when the project prints
/// them, their wire numbers; a bus line that no cable mark lists prints its cable's tag where it has no number
/// ([`bus_cable_text`]).
#[derive(Debug)]
pub struct PageConnectionsOf;

impl Query for PageConnectionsOf {
    type Key = Id<Page>;
    type Value = Vec<ConnectionView>;
    const FAMILY: &'static str = "page_connections";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<ConnectionView>> {
        let mut out = Vec::new();
        let grouped = grouped_connections(q, *key);
        let grid = page_grid(q, PageRef::Drawn(*key));
        for cid in q.idx().connections_by_page(*key) {
            // As read: a vertex within 1 nm of a grid position on the grid's own number (06a §2.1).
            let Some(c) = q
                .connections()
                .get(cid)
                .map(|c| ew_derive::read_connection(c, grid))
            else {
                continue;
            };
            let marker = connection_marker(q, &c, grouped.contains(&cid));
            // A bus line in its cable's mark prints the tag there, not beside the line.
            let number = connection_number(q, cid)
                .or_else(|| (!grouped.contains(&cid)).then(|| bus_cable_text(q, &c)).flatten());
            out.push(ConnectionView {
                connection: c,
                marker,
                number,
            });
        }
        Arc::new(out)
    }
}

/// `page_junctions(page)`: the junctions of a page with their generic symbols.
#[derive(Debug)]
pub struct PageJunctionsOf;

impl Query for PageJunctionsOf {
    type Key = Id<Page>;
    type Value = Vec<JunctionView>;
    const FAMILY: &'static str = "page_junctions";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<JunctionView>> {
        let generic = generic_symbols();
        let grid = page_grid(q, PageRef::Drawn(*key));
        let mut out = Vec::new();
        for jid in q.idx().junctions_by_page(*key) {
            // As read: a point within 1 nm of a grid position on the grid's own number (06a §2.1).
            let Some(j) = q.junctions().get(jid).map(|j| ew_derive::read_junction(j, grid)) else {
                continue;
            };
            let symbol = match j.style {
                JunctionStyle::Dot => generic.junction_dot.clone(),
                JunctionStyle::Bevel(_) => generic.junction_bevel.clone(),
                JunctionStyle::Unknown(_) => None,
            };
            out.push(JunctionView { junction: j, symbol });
        }
        Arc::new(out)
    }
}

/// `cp_targets_by_page()`: every CP a report row links to, with the representation that draws its function on
/// each drawn page (not a contact image), by page: one pass over the rows of every report block for the project.
#[derive(Debug)]
pub struct CpTargetsByPage;

impl Query for CpTargetsByPage {
    type Key = ();
    type Value = BTreeMap<Id<Page>, Vec<CpTarget>>;
    const FAMILY: &'static str = "cp_targets_by_page";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, (): &()) -> Arc<Self::Value> {
        let mut out: BTreeMap<Id<Page>, Vec<CpTarget>> = BTreeMap::new();
        for (block, _) in q.report_blocks().iter() {
            let set = q.get::<BlockRows>(block);
            for r in &set.rows {
                for l in &r.links {
                    let LinkTarget::Cp { function, idx } = l.target else {
                        continue;
                    };
                    // The smallest-ID representation of the function on each page that draws it.
                    // An internal coil without a representation is drawn in its valve's symbol.
                    let drawn = ew_derive::coil_host(q, function).unwrap_or(function);
                    let mut seen = std::collections::BTreeSet::new();
                    for rid in q.idx().reps_of_function(drawn) {
                        let Some(x) = q.reps().get(rid) else { continue };
                        if x.rep_type == RepType::ContactImage || !seen.insert(x.page) {
                            continue;
                        }
                        out.entry(x.page).or_default().push(CpTarget {
                            rep: rid,
                            function,
                            idx,
                        });
                    }
                }
            }
        }
        for targets in out.values_mut() {
            targets.sort();
            targets.dedup();
        }
        Arc::new(out)
    }
}

/// `page_cp_targets(page)`: the CPs of a drawn page that a report row links to, with the representation that
/// draws each one's function there (the named destinations `ew:cp:<uuid>` of doc 09 §4.2): the page's part of
/// [`CpTargetsByPage`], its own memo, so a row change that keeps a page's targets is cut off here.
#[derive(Debug)]
pub struct PageCpTargetsOf;

impl Query for PageCpTargetsOf {
    type Key = Id<Page>;
    type Value = Vec<CpTarget>;
    const FAMILY: &'static str = "page_cp_targets";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Vec<CpTarget>> {
        Arc::new(q.get::<CpTargetsByPage>(()).get(key).cloned().unwrap_or_default())
    }
}

/// The bridges of the strips whose terminals the drawn page `page` shows, alone or in a strip segment (doc 08 §6.5):
/// every bridge group of such a strip (`ew_derive::StripBridges`, by strip ID, then group) with
/// the clamp its connections name on each of its terminals, the first in the group's connection order (a bridge shaft,
/// which no symbol draws, as the terminal's lowest side-1 clamp), in the group's terminal order. The scene draws a
/// group where its terminals stand in one row.
pub fn page_bridges(q: &Q, page: Id<Page>) -> Vec<BridgeView> {
    let (reps, functions, devices, connections) = (q.reps(), q.functions(), q.devices(), q.connections());
    // A terminal's strip, or the strip a segment draws (a segment is a representation of the strip's function).
    let strips: std::collections::BTreeSet<Id<Device>> = q
        .idx()
        .reps_by_page(page)
        .filter_map(|rid| reps.get(rid))
        .flat_map(|rep| rep.functions.iter())
        .filter_map(|f| functions.get(*f)?.device)
        .filter_map(|d| {
            let aspects = &devices.get(d)?.aspects;
            match (&aspects.terminal, &aspects.strip) {
                (Some(t), _) => Some(t.strip),
                (None, Some(_)) => Some(d),
                (None, None) => None,
            }
        })
        .collect();
    let mut out = Vec::new();
    for strip in strips {
        for group in q.get::<ew_derive::StripBridges>(strip).iter() {
            // The clamp of each terminal: the first end of the group's connections on it.
            let ends: Vec<_> = group
                .connections
                .iter()
                .filter_map(|id| connections.get(*id))
                .flat_map(|c| [&c.a, &c.b])
                .filter_map(|e| match e {
                    ew_schema::Endpoint::Cp(f, idx) => Some((functions.get(*f)?.device?, (*f, *idx))),
                    _ => None,
                })
                .collect();
            // A bridge on the bridge shaft, which no symbol draws, is drawn on the lead of the terminal's lowest side-1
            // connection point, where `strip.bridge` bridges a terminal without a shaft.
            let drawn = |(f, idx): (Id<Function>, u16)| {
                let cps = q.get::<FunctionCps>(f);
                match cps.iter().find(|c| c.idx == idx) {
                    Some(c) if c.side.is_none() => cps
                        .iter()
                        .filter(|c| c.side == Some(Side::Side1))
                        .map(|c| c.idx)
                        .min()
                        .map_or((f, idx), |side1| (f, side1)),
                    _ => (f, idx),
                }
            };
            let clamps: Vec<(Id<Function>, u16)> = group
                .terminals
                .iter()
                .filter_map(|t| ends.iter().find(|(d, _)| d == t).map(|(_, cp)| drawn(*cp)))
                .collect();
            if clamps.len() >= 2 {
                out.push(BridgeView { clamps });
            }
        }
    }
    out
}
