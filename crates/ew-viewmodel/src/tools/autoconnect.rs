// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Autoconnect proposals (FR-CON-003, FR-CON-008; doc 06 §5.2, ADR-0007): what a floating or dragged
//! symbol's free connection points would connect to, computed from the page tables alone (no `Session` call) and
//! drawn as dashed overlay stubs. They reach the model only as the `connect` hints of the committed `device.place`
//! (D4); during a move they are shown and never committed.
//!
//! **The walk.** For each free ghost CP, in ascending symbol CP index, the ray leaves the CP along its direction, one
//! grid step at a time, inside the search area: a representation body (other than the ghost's own) stops it; a free
//! CP whose direction is opposite to the ray's, at the walk point (equal grid indices on both axes), is a hit; a
//! connection segment collinear with the ray at the walk point stops it (the line is occupied); a segment crossing
//! the ray is passed. Several CPs at one walk point are hits in label order (the page tables' order; never
//! the v5 ID, which follows the minted function ID). A fluid port's hit is a fluid port and an electrical CP's an
//! electrical CP, so on a fluid page autoconnect joins fluid ports with a fluid line (FT-1733); a contact's point (a
//! switch, a push-button, a sensor) never takes a PLC output's pin, and a point that names a conductor (`N`, `PE`, `L1`,
//! a supply's `+`) never takes a point of a potential named for another one, nor do a terminal's two sides join two
//! potentials of different names (rc.3: a sensor joined to an output, a neutral to a phase); a valve's working port
//! never takes another valve's exhaust port as a hit, nor the other way round (EW-FL-012's conflict, the persona
//! re-test: two valves stacked had their exhausts joined to the working ports), and an in-line fitting's port (a flow
//! control, a check valve, a coupling: `ew_derive::is_fitting`) never takes another fitting's (rc.5: four flow controls set side
//! by side in a row were joined by tubes). A terminal's ghost CP never takes a CP of a terminal of its own
//! strip as a hit: two terminals of one strip are joined by a bridge, never by a
//! proposed wire; that terminal's body then stops the walk as any body does. The proposal of a ray is its
//! first hit; [`ToolKey::NextProposal`](super::ToolKey::NextProposal) (Tab) cycles every ray to its next hit further
//! along, wrapping.
//!
//! **Far hits.** A first hit more than [`SHORT_RUN`] grid steps away, or one whose run crosses a wire, is no proposal:
//! the placement does not join it, its toast offers it with Connect (rc.2). A far hit that Tab chose is a proposal.
//!
//! **Pole partners.** A free ghost CP whose ray finds no hit joins a free CP of a representation another proposal joins,
//! facing it, 2 to [`SHORT_RUN`] steps ahead and 1 to [`POLE_JOG`] steps aside, by a wire that runs out to halfway
//! between them, steps aside there and runs on in (rc.5 persona run: a two-pole breaker dropped under a two-pole
//! switch had one pole joined, because the other's poles stood a different distance apart); the nearest such CP, each
//! taken once. The proposal carries that route, and its hint sends it.
//!
//! **The split.** Two ghost CPs that lie on one segment of a connection W and face its two ends (their directions
//! along the segment, pointing apart) are the split of W: one proposal that sends no hint, because normaliser 7
//! splits W when the symbol lands on its stored route (FR-CON-008, doc 06 §5.3); a route-less W has no wire geometry
//! and is not split.

use ew_api::geometry::Dir;
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::{PageTables, TableCp, TableRep, Uuid};
use serde_json::{Value, json};

use super::wire::PageGridMap;

/// The most grid steps a ray walks.
pub const MAX_WALK: usize = 2_000;

/// The longest run, in grid steps, that a placement joins by itself (rc.2 persona run, entry 17: a sensor dropped under
/// an output card was joined to the card's pin by a long wire through other wires). A hit further away, or one whose
/// run crosses a wire, is far: it is offered after the placement, never drawn by it, unless Tab chose it.
pub const SHORT_RUN: i64 = 20;

/// The most grid steps aside a pole partner may stand (see the module documentation).
pub const POLE_JOG: i64 = 3;

/// A connection point of the ghost, in page nm.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GhostCp {
    /// The symbol CP index (the hint's `cp`).
    pub symbol_cp: u16,
    /// The designation of the FnDef CP it draws (the describe text names it).
    pub designation: String,
    /// Its point on the page.
    pub at: (i64, i64),
    /// The direction a wire leaves it.
    pub dir: Dir,
    /// Whether a connection already ends at it (a moved representation's CP); such a CP proposes nothing.
    pub connected: bool,
    /// The strip tag of the ghost's terminal as the page prints it (`-X1031`), `None` for a non-terminal: a CP of a
    /// terminal of that strip is never a hit.
    pub strip_tag: Option<String>,
    /// Whether it is a fluid port: a fluid port's hit is a fluid port, an electrical CP's an electrical CP (FT-1733).
    pub fluid: bool,
    /// Its side of EW-FL-012 (a valve's working or exhaust port): a port of the conflicting side is never its hit.
    pub valve_port: Option<ew_api::output::ValvePort>,
    /// Whether it is a contact's point (a switch, a push-button, a sensor): a PLC output's pin is never its hit.
    pub contact: bool,
    /// Whether it is a fitting's port: another fitting's port is never its hit.
    pub fitting: bool,
}

/// Whether the representation `rep` of the page is a terminal of the strip printed `strip_tag`: a terminal
/// representation whose connection points all print `<strip_tag>:…` (and that has one). The tables carry no strip
/// identity; two strips printed with one tag are a duplicate tag, which a check reports. Shared by autoconnect
///  and the Place tool's stacking of a terminal on one of its strip.
pub fn is_strip_terminal(tables: &PageTables, rep: &TableRep, strip_tag: &str) -> bool {
    let prefix = format!("{strip_tag}:");
    rep.terminal && {
        let mut cps = tables.cps.iter().filter(|c| c.rep == rep.id).peekable();
        cps.peek().is_some() && cps.all(|c| c.label.starts_with(&prefix))
    }
}

/// What a proposal connects to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProposalTarget {
    /// A free CP of the page.
    Cp {
        /// The function.
        function: EntityRef,
        /// The function's CP index.
        idx: u16,
        /// Its point on the page.
        at: (i64, i64),
        /// Its label (`1X1:1`).
        label: String,
    },
    /// The split of a connection by two ghost CPs (no hint: normaliser 7 splits it).
    Split {
        /// The connection.
        connection: EntityRef,
        /// The other ghost CP on its segment: its symbol CP index, designation and point.
        other: (u16, String, (i64, i64)),
    },
}

/// One proposal of a ghost CP.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Proposal {
    /// The ghost CP (the first of the two for a split).
    pub cp: GhostCp,
    /// What it connects to.
    pub target: ProposalTarget,
    /// The wire's route in page nm, from the ghost CP to the target, for a pole partner; empty: the straight run.
    pub route: Vec<(i64, i64)>,
}

/// The proposals of a ghost.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Proposals {
    /// The proposals in ascending symbol CP index; a split counts once.
    pub list: Vec<Proposal>,
    /// The far first hits ([`SHORT_RUN`]) that the placement does not join: offered after it, in the same order.
    pub far: Vec<Proposal>,
}

impl Proposals {
    /// The `connect` hints of `device.place`: one per CP proposal (a split sends none).
    pub fn hints(&self) -> Vec<Value> {
        self.list
            .iter()
            .filter_map(|p| match &p.target {
                ProposalTarget::Cp { function, idx, .. } => {
                    let mut hint = json!({
                        "cp": p.cp.symbol_cp,
                        "to": {"cp": {"function": function.0.to_string(), "idx": idx}},
                    });
                    if !p.route.is_empty() {
                        hint["route"] = json!(p.route.iter().map(|q| vec![q.0, q.1]).collect::<Vec<_>>());
                    }
                    Some(hint)
                }
                ProposalTarget::Split { .. } => None,
            })
            .collect()
    }

    /// The describe text's pairs: `("2", "1X1:1")`, a split as `("1" "2", "")`.
    pub fn lines(&self) -> Vec<(String, Option<String>)> {
        self.list
            .iter()
            .map(|p| match &p.target {
                ProposalTarget::Cp { label, .. } => {
                    (format!("\"{}\"", p.cp.designation), Some(label.clone()))
                }
                ProposalTarget::Split { other, .. } => {
                    (format!("\"{}\" \"{}\"", p.cp.designation, other.1), None)
                }
            })
            .collect()
    }
}

fn opposite(d: Dir) -> Dir {
    match d {
        Dir::Up => Dir::Down,
        Dir::Down => Dir::Up,
        Dir::Left => Dir::Right,
        Dir::Right => Dir::Left,
    }
}

fn vertical(d: Dir) -> bool {
    matches!(d, Dir::Up | Dir::Down)
}

/// The deduplicated polyline of a connection row (the scene repeats a stored route's ends).
pub fn dedup(polyline: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(polyline.len());
    for p in polyline {
        if out.last() != Some(p) {
            out.push(*p);
        }
    }
    out
}

/// The hits of one ray (see the module documentation), nearest first, each with whether it is far ([`SHORT_RUN`]).
fn ray_hits(
    cp: &GhostCp,
    tables: &PageTables,
    own: &[EntityRef],
    area: [i64; 4],
    grid: PageGridMap,
) -> Vec<(ProposalTarget, bool)> {
    let d = cp.dir.vector();
    let (ix, iy) = (grid.index(cp.at.0), grid.index(cp.at.1));
    let mut hits = Vec::new();
    // Whether the run so far crossed a wire.
    let mut crossed = false;
    for k in 1..=MAX_WALK as i64 {
        let (kx, ky) = (ix + d.0 * k, iy + d.1 * k);
        // The walk point: the ray keeps the CP's own coordinate across it.
        let (x, y) = if vertical(cp.dir) {
            (cp.at.0, grid.coord(ky))
        } else {
            (grid.coord(kx), cp.at.1)
        };
        if x < area[0] || x > area[2] || y < area[1] || y > area[3] {
            break;
        }
        // Free CPs facing the ray at the walk point, in label order (the rows' order); never a terminal of the
        // ghost's own strip.
        for q in &tables.cps {
            if grid.index(q.at.0) == kx && grid.index(q.at.1) == ky && may_join(cp, q, tables, own) {
                hits.push((
                    ProposalTarget::Cp {
                        function: EntityRef(q.function),
                        idx: q.idx,
                        at: q.at,
                        label: q.label.clone(),
                    },
                    crossed || k > SHORT_RUN,
                ));
            }
        }
        // A body stops the ray (a CP on its edge was looked at first).
        let in_body = tables.reps.iter().any(|r| {
            !own.contains(&EntityRef(r.id))
                && r.body[0] < x
                && x < r.body[2]
                && r.body[1] < y
                && y < r.body[3]
        });
        if in_body {
            break;
        }
        // A segment on the ray's line through the walk point occupies it.
        let occupied = tables.connections.iter().any(|c| {
            dedup(&c.polyline).windows(2).any(|w| {
                let (a, b) = (w[0], w[1]);
                if vertical(cp.dir) {
                    a.0 == b.0 && grid.index(a.0) == ix && a.1.min(b.1) <= y && y <= a.1.max(b.1)
                } else {
                    a.1 == b.1 && grid.index(a.1) == iy && a.0.min(b.0) <= x && x <= a.0.max(b.0)
                }
            })
        });
        if occupied {
            break;
        }
        // A segment across the ray at the walk point is passed, but the run is no longer free.
        crossed |= tables.connections.iter().any(|c| {
            dedup(&c.polyline).windows(2).any(|w| {
                let (a, b) = (w[0], w[1]);
                if vertical(cp.dir) {
                    a.1 == b.1 && grid.index(a.1) == ky && a.0.min(b.0) <= x && x <= a.0.max(b.0)
                } else {
                    a.0 == b.0 && grid.index(a.0) == kx && a.1.min(b.1) <= y && y <= a.1.max(b.1)
                }
            })
        });
    }
    hits
}

/// Whether the page's CP `q`, wherever it stands, may be the ghost CP `cp`'s hit: free, of another representation,
/// facing the ray, of its kind, and none of the exclusions of the module documentation.
fn may_join(cp: &GhostCp, q: &TableCp, tables: &PageTables, own: &[EntityRef]) -> bool {
    !own.contains(&EntityRef(q.rep))
        && q.connections.is_empty()
        && q.dir == Some(opposite(cp.dir))
        && q.fluid == cp.fluid
        && !ew_api::output::ValvePort::conflict(q.valve_port, cp.valve_port)
        && !(cp.contact && q.plc_output)
        && !(cp.fitting && q.fitting)
        && !other_conductor(&cp.designation, q.potential.as_ref())
        && !cp.strip_tag.as_deref().is_some_and(|tag| {
            tables
                .reps
                .iter()
                .any(|r| r.id == q.rep && is_strip_terminal(tables, r, tag))
        })
}

/// The pole partner of the ghost CP `cp` that found no hit (see the module documentation): the nearest free CP of a
/// representation a proposal of `list` joins, not yet taken, with the route that jogs over halfway; `None` when no
/// such CP stands within reach or the jog would run through a body.
fn pole_partner(
    cp: &GhostCp,
    list: &[Proposal],
    tables: &PageTables,
    own: &[EntityRef],
    grid: PageGridMap,
) -> Option<Proposal> {
    let taken = |q: &TableCp| {
        list.iter().any(|p| {
            matches!(&p.target, ProposalTarget::Cp { function, idx, .. } if function.0 == q.function && *idx == q.idx)
        })
    };
    let joined = |q: &TableCp| {
        list.iter().any(|p| match &p.target {
            ProposalTarget::Cp { function, idx, .. } => tables
                .cps
                .iter()
                .any(|c| c.function == function.0 && c.idx == *idx && c.rep == q.rep),
            ProposalTarget::Split { .. } => false,
        })
    };
    let d = cp.dir.vector();
    let (ix, iy) = (grid.index(cp.at.0), grid.index(cp.at.1));
    // (steps ahead, steps aside) of a page CP from the ghost CP.
    let offset = |q: &TableCp| {
        let (dx, dy) = (grid.index(q.at.0) - ix, grid.index(q.at.1) - iy);
        if vertical(cp.dir) {
            (dy * d.1, dx)
        } else {
            (dx * d.0, dy)
        }
    };
    let (q, (ahead, _)) = tables
        .cps
        .iter()
        .map(|q| (q, offset(q)))
        .filter(|(q, (ahead, aside))| {
            (2..=SHORT_RUN).contains(ahead)
                && (1..=POLE_JOG).contains(&aside.abs())
                && joined(q)
                && !taken(q)
                && may_join(cp, q, tables, own)
        })
        .min_by_key(|(_, (ahead, aside))| (*ahead, aside.abs()))?;
    let half = ahead / 2;
    let route = if vertical(cp.dir) {
        let y = grid.coord(iy + d.1 * half);
        vec![cp.at, (cp.at.0, y), (q.at.0, y), q.at]
    } else {
        let x = grid.coord(ix + d.0 * half);
        vec![cp.at, (x, cp.at.1), (x, q.at.1), q.at]
    };
    let (m, n) = (route[1], route[2]);
    let through_body = tables.reps.iter().any(|r| {
        !own.contains(&EntityRef(r.id))
            && m.0.max(n.0) > r.body[0]
            && m.0.min(n.0) < r.body[2]
            && m.1.max(n.1) > r.body[1]
            && m.1.min(n.1) < r.body[3]
    });
    (!through_body).then(|| Proposal {
        cp: cp.clone(),
        target: ProposalTarget::Cp {
            function: EntityRef(q.function),
            idx: q.idx,
            at: q.at,
            label: q.label.clone(),
        },
        route,
    })
}

/// Whether `p` lies strictly inside the axis-parallel segment `a`–`b` (on its line by grid index, between its ends);
/// never for a segment of another slope or of no length.
fn strictly_on(grid: PageGridMap, a: (i64, i64), b: (i64, i64), p: (i64, i64)) -> bool {
    if a.1 == b.1 && a.0 != b.0 {
        grid.index(p.1) == grid.index(a.1) && a.0.min(b.0) < p.0 && p.0 < a.0.max(b.0)
    } else if a.0 == b.0 && a.1 != b.1 {
        grid.index(p.0) == grid.index(a.0) && a.1.min(b.1) < p.1 && p.1 < a.1.max(b.1)
    } else {
        false
    }
}

/// Whether a ghost CP lies strictly inside a segment that the scene's clamp drew along the sheet's edge, off its
/// wire's path ([`TableConnection::off_sheet`](ew_api::output::TableConnection::off_sheet)): the
/// Place tool refuses that drop, because the symbol would look joined to a wire that does not run there. A CP at
/// such a segment's end is not inside it.
pub fn on_off_sheet_segment(cps: &[GhostCp], tables: &PageTables) -> bool {
    let grid = PageGridMap::new(tables.grid);
    tables.connections.iter().any(|c| {
        c.off_sheet.iter().any(|&i| {
            let (Some(a), Some(b)) = (c.polyline.get(i), c.polyline.get(i + 1)) else {
                return false;
            };
            cps.iter().any(|p| strictly_on(grid, *a, *b, p.at))
        })
    })
}

/// The conductor a connection point's designation or a potential's name stands for (`L`, `L1`…`L3`, `N`, `PE`, a DC
/// supply's plus or minus), `None` for any other word. `L` stands for any phase.
fn conductor(word: &str) -> Option<&'static str> {
    Some(match word.trim().to_ascii_uppercase().as_str() {
        "L" => "L",
        "L1" => "L1",
        "L2" => "L2",
        "L3" => "L3",
        "N" => "N",
        "PE" => "PE",
        "+" | "L+" | "24V" | "+24V" => "+",
        "-" | "M" | "0V" => "-",
        _ => return None,
    })
}

/// Whether a ghost CP designated `designation` would join the named potential `potential` of another conductor (rc.3:
/// a placement joined a neutral pin to a phase because they stood near): both stand for a conductor
/// ([`conductor`]) and not the same one; a phase `L` matches any phase. A potential named by its fallback is no
/// conductor's.
fn other_conductor(designation: &str, potential: Option<&ew_api::output::TablePotential>) -> bool {
    let Some(p) = potential.filter(|p| !p.fallback) else {
        return false;
    };
    let phase = |c: &str| c.starts_with('L');
    match (conductor(designation), conductor(&p.name)) {
        (Some(a), Some(b)) => a != b && !(phase(a) && phase(b) && (a == "L" || b == "L")),
        _ => false,
    }
}

/// The splits of the ghost: pairs of ghost CPs on one segment of a connection facing its two ends. Only a wire with a
/// stored route has wire geometry: an unrouted connection is not split (as normaliser 7).
fn splits(cps: &[GhostCp], tables: &PageTables, grid: PageGridMap) -> Vec<Proposal> {
    let mut out: Vec<Proposal> = Vec::new();
    for c in tables.connections.iter().filter(|c| c.stored_route) {
        let line = dedup(&c.polyline);
        for w in line.windows(2) {
            let (a, b) = (w[0], w[1]);
            let horizontal = a.1 == b.1 && a.0 != b.0;
            let vertical_seg = a.0 == b.0 && a.1 != b.1;
            // A stretch the clamp drew along the sheet's edge is not the wire's path; nothing splits there.
            if !(horizontal || vertical_seg) || c.is_off_sheet(a, b) {
                continue;
            }
            let on = |p: (i64, i64)| strictly_on(grid, a, b, p);
            let along = |p: &GhostCp| if horizontal { p.at.0 } else { p.at.1 };
            for (i, p) in cps.iter().enumerate() {
                for q in &cps[i + 1..] {
                    if p.connected || q.connected || !on(p.at) || !on(q.at) {
                        continue;
                    }
                    let axis_ok = if horizontal {
                        !vertical(p.dir) && !vertical(q.dir)
                    } else {
                        vertical(p.dir) && vertical(q.dir)
                    };
                    if !axis_ok || p.dir != opposite(q.dir) {
                        continue;
                    }
                    // They face the segment's two ends: the one nearer the smaller coordinate points towards it.
                    let (lo, hi) = if along(p) < along(q) { (p, q) } else { (q, p) };
                    let faces = matches!((lo.dir, hi.dir), (Dir::Left, Dir::Right) | (Dir::Up, Dir::Down));
                    if !faces {
                        continue;
                    }
                    let (first, second) = if p.symbol_cp <= q.symbol_cp {
                        (p, q)
                    } else {
                        (q, p)
                    };
                    out.push(Proposal {
                        cp: first.clone(),
                        target: ProposalTarget::Split {
                            connection: EntityRef(c.id),
                            other: (second.symbol_cp, second.designation.clone(), second.at),
                        },
                        route: Vec::new(),
                    });
                }
            }
        }
    }
    out
}

/// The proposals of `cps` on the page of `tables` (see the module documentation): `own` are the representations of
/// the ghost (those a move drags; none for a floating symbol), `area` the drawing area in page nm, `cycle` how many
/// times Tab was pressed since the ghost last moved.
pub fn proposals(
    cps: &[GhostCp],
    tables: &PageTables,
    own: &[EntityRef],
    area: [i64; 4],
    cycle: usize,
) -> Proposals {
    let grid = PageGridMap::new(tables.grid);
    let split_list = splits(cps, tables, grid);
    let in_split = |c: &GhostCp| {
        split_list.iter().any(|p| {
            p.cp.symbol_cp == c.symbol_cp
                || matches!(&p.target, ProposalTarget::Split { other, .. } if other.0 == c.symbol_cp)
        })
    };
    let mut sorted: Vec<&GhostCp> = cps.iter().collect();
    sorted.sort_by_key(|c| c.symbol_cp);
    let mut list: Vec<Proposal> = Vec::new();
    let mut far: Vec<Proposal> = Vec::new();
    // The ghost CPs whose ray found no hit: pole partners once the rays are walked.
    let mut lonely: Vec<&GhostCp> = Vec::new();
    for c in sorted {
        if c.connected {
            continue;
        }
        if let Some(p) = split_list.iter().find(|p| p.cp.symbol_cp == c.symbol_cp) {
            list.push(p.clone());
            continue;
        }
        if in_split(c) {
            continue;
        }
        let hits = ray_hits(c, tables, own, area, grid);
        if hits.is_empty() {
            lonely.push(c);
            continue;
        }
        let (target, is_far) = hits[cycle % hits.len()].clone();
        let p = Proposal {
            cp: c.clone(),
            target,
            route: Vec::new(),
        };
        // A far first hit is offered after the placement; one Tab chose is the drafter's.
        if is_far && cycle == 0 {
            far.push(p);
        } else {
            list.push(p);
        }
    }
    for c in lonely {
        if let Some(p) = pole_partner(c, &list, tables, own, grid) {
            list.push(p);
        }
    }
    list.sort_by_key(|p| p.cp.symbol_cp);
    // A terminal joins what its two sides reach: never two named potentials of different names, which it would short
    // (rc.3); the later proposal goes.
    if cps.iter().any(|c| c.strip_tag.is_some()) {
        let named = |p: &Proposal| match &p.target {
            ProposalTarget::Cp { function, idx, .. } => tables
                .cps
                .iter()
                .find(|c| c.function == function.0 && c.idx == *idx)
                .and_then(|c| c.potential.as_ref())
                .filter(|p| !p.fallback)
                .map(|p| p.name.clone()),
            ProposalTarget::Split { .. } => None,
        };
        let mut first: Option<String> = None;
        list.retain(|p| match (named(p), &first) {
            (Some(name), Some(seen)) => name == *seen,
            (Some(name), None) => {
                first = Some(name);
                true
            }
            (None, _) => true,
        });
    }
    Proposals { list, far }
}

/// The ghost CPs of the representations `reps` of the page moved by the page displacements `delta(rep)`: their table
/// CPs at the moved points (as the page will draw them, `ip::pin_point`), the designation the label prints after its colon, and the strip tag of a moved terminal
/// (see `moved_strip_tag`).
pub fn moved_cps(tables: &PageTables, delta: &dyn Fn(EntityRef) -> Option<(i64, i64)>) -> Vec<GhostCp> {
    let mut out = Vec::new();
    for (k, cp) in tables.cps.iter().enumerate() {
        let (Some(d), Some(dir)) = (delta(EntityRef(cp.rep)), cp.dir) else {
            continue;
        };
        let designation = cp
            .label
            .rsplit_once(':')
            .map_or(cp.label.as_str(), |(_, d)| d)
            .to_owned();
        out.push(GhostCp {
            symbol_cp: u16::try_from(k).unwrap_or(u16::MAX),
            designation,
            at: super::ip::pin_point(tables, (cp.at.0 + d.0, cp.at.1 + d.1)),
            dir,
            connected: !cp.connections.is_empty(),
            strip_tag: moved_strip_tag(tables, cp.rep),
            fluid: cp.fluid,
            valve_port: cp.valve_port,
            // A moved symbol's proposals are shown, never committed (module documentation).
            contact: false,
            fitting: cp.fitting,
        });
    }
    out
}

/// The strip tag of the terminal representation `rep`: the text before the last colon of its CP labels;
/// `None` for a non-terminal, a representation without CPs, a label without a colon, or labels that disagree.
fn moved_strip_tag(tables: &PageTables, rep: Uuid) -> Option<String> {
    if !tables.reps.iter().any(|r| r.id == rep && r.terminal) {
        return None;
    }
    let mut tag: Option<&str> = None;
    for c in tables.cps.iter().filter(|c| c.rep == rep) {
        let (t, _) = c.label.rsplit_once(':')?;
        if tag.is_some_and(|seen| seen != t) {
            return None;
        }
        tag = Some(t);
    }
    tag.map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::{TableConnection, TableCp, TableEnd};

    const G: i64 = 1_000_000;

    /// A UUID-typed field from a number (the type is inferred from the field).
    fn uuid<T: std::str::FromStr>(n: u128) -> T
    where
        T::Err: std::fmt::Debug,
    {
        format!("{n:032x}").parse().unwrap()
    }

    fn tables() -> PageTables {
        PageTables {
            folio: "1".into(),
            page: uuid(1),
            grid: (G, 1),
            reps: Vec::new(),
            cps: Vec::new(),
            connections: Vec::new(),
            junctions: Vec::new(),
            items: Vec::new(),
            structure_boxes: Vec::new(),
        }
    }

    fn cp(n: u128, at: (i64, i64), dir: Dir, label: &str) -> TableCp {
        TableCp {
            id: uuid(0x100 + n),
            function: uuid(0x200 + n),
            idx: 0,
            rep: uuid(0x300 + n),
            label: label.into(),
            at,
            dir: Some(dir),
            connections: Vec::new(),
            potential: None,
            side: None,
            fluid: false,
            valve_port: None,
            plc_address: None,
            plc_output: false,
            fitting: false,
        }
    }

    /// A stored wire between two junctions along `polyline`.
    fn wire(
        n: u128,
        polyline: Vec<(i64, i64)>,
        stored_route: bool,
        off_sheet: Vec<usize>,
    ) -> TableConnection {
        TableConnection {
            id: uuid(n),
            a: TableEnd::Junction { id: uuid(1) },
            b: TableEnd::Junction { id: uuid(2) },
            kind: "wire".into(),
            polyline,
            stored_route,
            off_sheet,
            line_kind: None,
        }
    }

    fn ghost(symbol_cp: u16, at: (i64, i64), dir: Dir) -> GhostCp {
        GhostCp {
            symbol_cp,
            designation: (symbol_cp + 1).to_string(),
            at,
            dir,
            connected: false,
            strip_tag: None,
            fluid: false,
            valve_port: None,
            contact: false,
            fitting: false,
        }
    }

    const AREA: [i64; 4] = [-100 * G, -100 * G, 100 * G, 100 * G];

    #[test]
    fn a_facing_free_cp_down_the_ray_is_proposed_and_tab_cycles_further() {
        let mut t = tables();
        t.cps.push(cp(1, (0, 10 * G), Dir::Up, "X1:1"));
        t.cps.push(cp(2, (0, 20 * G), Dir::Up, "X1:2"));
        let g = [ghost(0, (0, 0), Dir::Down)];
        let p = proposals(&g, &t, &[], AREA, 0);
        assert_eq!(p.list.len(), 1);
        assert!(matches!(&p.list[0].target, ProposalTarget::Cp { label, .. } if label == "X1:1"));
        assert_eq!(p.hints().len(), 1);
        let p = proposals(&g, &t, &[], AREA, 1);
        assert!(matches!(&p.list[0].target, ProposalTarget::Cp { label, .. } if label == "X1:2"));
        let p = proposals(&g, &t, &[], AREA, 2);
        assert!(
            matches!(&p.list[0].target, ProposalTarget::Cp { label, .. } if label == "X1:1"),
            "wraps"
        );
    }

    /// rc.5 persona run: a two-pole device under another whose poles stand a different distance apart had one pole
    /// joined; the other pole now joins its partner by a wire that jogs over halfway, and its hint sends that route.
    #[test]
    fn a_pole_out_of_line_joins_its_partner_by_a_jog() {
        let mut t = tables();
        let mut far_pole = cp(2, (3 * G, 6 * G), Dir::Up, "F1:3");
        far_pole.rep = uuid(0x301);
        t.cps.push(cp(1, (0, 6 * G), Dir::Up, "F1:1"));
        t.cps.push(far_pole);
        // A free CP of another representation out of line is no partner.
        t.cps.push(cp(3, (-2 * G, 6 * G), Dir::Up, "X9:1"));
        let g = [ghost(0, (0, 0), Dir::Down), ghost(1, (2 * G, 0), Dir::Down)];
        let p = proposals(&g, &t, &[], AREA, 0);
        assert_eq!(p.list.len(), 2, "{p:?}");
        assert!(matches!(&p.list[1].target, ProposalTarget::Cp { label, .. } if label == "F1:3"));
        assert_eq!(
            p.list[1].route,
            [(2 * G, 0), (2 * G, 3 * G), (3 * G, 3 * G), (3 * G, 6 * G)]
        );
        assert!(p.list[0].route.is_empty(), "the pole in line runs straight");
        assert_eq!(p.hints()[1]["route"][2], json!([3 * G, 3 * G]));
        // Without the pole in line joined, the out-of-line one is no partner of anything.
        let p = proposals(&g[1..], &t, &[], AREA, 0);
        assert!(p.list.is_empty(), "{p:?}");
    }

    /// rc.3 persona runs: a sensor placed near an output card was joined to an output, and a neutral pin to a phase.
    /// A contact's point never takes a PLC output's pin (a coil's still does); a point that names a conductor never
    /// takes a point of a potential named for another one (a phase `L` takes any phase; a fallback name is no
    /// conductor's); a terminal's two sides never join two potentials of different names.
    #[test]
    fn a_contact_never_takes_an_output_nor_a_point_another_conductor() {
        let named = |name: &str, fallback: bool| {
            Some(ew_api::output::TablePotential {
                name: name.into(),
                fallback,
            })
        };
        let mut t = tables();
        let mut output = cp(1, (0, 10 * G), Dir::Up, "-A2:6");
        output.plc_output = true;
        t.cps.push(output);
        let contact = GhostCp {
            contact: true,
            ..ghost(0, (0, 0), Dir::Down)
        };
        assert!(
            proposals(&[contact], &t, &[], AREA, 0).list.is_empty(),
            "a sensor"
        );
        assert_eq!(
            proposals(&[ghost(0, (0, 0), Dir::Down)], &t, &[], AREA, 0)
                .list
                .len(),
            1,
            "a coil or a lamp"
        );
        for (designation, potential, joins) in [
            ("N", named("L1", false), false),
            ("PE", named("N", false), false),
            ("N", named("N", false), true),
            ("L", named("L2", false), true),
            ("N", named("-X1:3", true), true),
            ("13", named("L1", false), true),
        ] {
            let mut t = tables();
            let mut target = cp(1, (0, 10 * G), Dir::Up, "-F1:1");
            target.potential = potential;
            t.cps.push(target);
            let g = GhostCp {
                designation: designation.into(),
                ..ghost(0, (0, 0), Dir::Down)
            };
            assert_eq!(
                proposals(&[g], &t, &[], AREA, 0).list.len(),
                usize::from(joins),
                "{designation}"
            );
        }
        // A terminal between L1 above and N below joins one of them.
        let mut t = tables();
        let mut above = cp(1, (0, -10 * G), Dir::Down, "-F1:2");
        above.potential = named("L1", false);
        let mut below = cp(2, (0, 10 * G), Dir::Up, "-K1:A1");
        below.potential = named("N", false);
        t.cps.extend([above, below]);
        let terminal = |cp: u16, at, dir| GhostCp {
            strip_tag: Some("-X1".into()),
            ..ghost(cp, at, dir)
        };
        let g = [terminal(0, (0, 0), Dir::Up), terminal(1, (0, 0), Dir::Down)];
        assert_eq!(proposals(&g, &t, &[], AREA, 0).list.len(), 1);
    }

    /// rc.2 persona run, entry 17: a CP facing the ghost within the short run is proposed, one further away is far.
    #[test]
    fn a_hit_beyond_the_short_run_is_far() {
        let g = [ghost(0, (0, 0), Dir::Down)];
        for (steps, near) in [(SHORT_RUN, true), (SHORT_RUN + 1, false)] {
            let mut t = tables();
            t.cps.push(cp(1, (0, steps * G), Dir::Up, "X1:1"));
            let p = proposals(&g, &t, &[], AREA, 0);
            assert_eq!(
                (p.list.len(), p.far.len()),
                if near { (1, 0) } else { (0, 1) },
                "{steps}"
            );
        }
    }

    #[test]
    fn bodies_and_occupied_lines_stop_the_ray_and_crossings_are_passed() {
        let mut t = tables();
        t.cps.push(cp(1, (0, 10 * G), Dir::Up, "X1:1"));
        // A body between: stops.
        t.reps.push(TableRep {
            id: uuid(0x400),
            functions: Vec::new(),
            device: None,
            symbol: uuid(0x500),
            symbol_key: String::new(),
            orient: 0,
            rep_type: "multi_line".into(),
            pos: (0, 5 * G),
            body: [-2 * G, 4 * G, 2 * G, 6 * G],
            tag_at: None,
            ip: None,
            black_box: None,
            terminal: false,
            locked: false,
        });
        let g = [ghost(0, (0, 0), Dir::Down)];
        assert!(proposals(&g, &t, &[], AREA, 0).list.is_empty());
        // The ghost's own body does not stop it.
        assert_eq!(
            proposals(&g, &t, &[EntityRef(uuid(0x400))], AREA, 0).list.len(),
            1
        );
        t.reps.clear();
        // A crossing segment is passed, but the hit behind it is far: offered, not proposed (rc.2); Tab proposes it.
        t.connections.push(wire(
            0x600,
            vec![(-5 * G, 5 * G), (5 * G, 5 * G)],
            true,
            Vec::new(),
        ));
        let p = proposals(&g, &t, &[], AREA, 0);
        assert_eq!((p.list.len(), p.far.len()), (0, 1));
        assert_eq!(proposals(&g, &t, &[], AREA, 1).list.len(), 1);
        // A segment along the ray occupies the line.
        t.connections
            .push(wire(0x601, vec![(0, 3 * G), (0, 7 * G)], true, Vec::new()));
        assert!(proposals(&g, &t, &[], AREA, 0).list.is_empty());
        // A CP facing the wrong way or connected is no hit.
        let mut t = tables();
        let mut taken = cp(1, (0, 10 * G), Dir::Up, "X1:1");
        taken.connections.push(uuid(9));
        t.cps.push(taken);
        t.cps.push(cp(2, (0, 12 * G), Dir::Down, "X1:2"));
        assert!(proposals(&g, &t, &[], AREA, 0).list.is_empty());
    }

    #[test]
    fn two_cps_on_a_routed_segment_facing_its_ends_are_one_split_without_hints() {
        let mut t = tables();
        t.connections.push(wire(
            0x600,
            vec![(0, 0), (0, 0), (0, 20 * G), (0, 20 * G)],
            true,
            Vec::new(),
        ));
        let g = [ghost(0, (0, 8 * G), Dir::Up), ghost(1, (0, 12 * G), Dir::Down)];
        let p = proposals(&g, &t, &[], AREA, 0);
        assert_eq!(p.list.len(), 1, "{p:?}");
        assert!(matches!(&p.list[0].target, ProposalTarget::Split { .. }));
        assert!(p.hints().is_empty());
        assert_eq!(p.lines(), vec![("\"1\" \"2\"".to_owned(), None)]);
        // Unrouted: no split (normaliser 7 does not split it either).
        t.connections[0].stored_route = false;
        assert!(proposals(&g, &t, &[], AREA, 0).list.is_empty());
    }

    #[test]
    fn a_routed_connection_is_split_before_a_route_less_one_on_the_same_line() {
        let mut t = tables();
        for (n, stored_route) in [(0x600, false), (0x601, true)] {
            t.connections
                .push(wire(n, vec![(0, 0), (0, 20 * G)], stored_route, Vec::new()));
        }
        let g = [ghost(0, (0, 8 * G), Dir::Up), ghost(1, (0, 12 * G), Dir::Down)];
        let p = proposals(&g, &t, &[], AREA, 0);
        assert_eq!(p.list.len(), 1, "{p:?}");
        assert!(
            matches!(&p.list[0].target, ProposalTarget::Split { connection, .. } if *connection == EntityRef(uuid(0x601))),
            "{p:?}"
        );
    }

    /// A segment the clamp drew along the sheet's edge proposes no split, and a ghost CP strictly
    /// inside it is on it; one at its end, or on the wire's own segment, is not.
    #[test]
    fn a_stretch_drawn_along_the_sheet_edge_is_no_split_and_refuses_a_drop() {
        let mut t = tables();
        // An L whose second segment, (0, 0)–(0, 20 G), is the clamp's.
        t.connections
            .push(wire(0x600, vec![(20 * G, 0), (0, 0), (0, 20 * G)], true, vec![1]));
        let on_edge = [ghost(0, (0, 8 * G), Dir::Up), ghost(1, (0, 12 * G), Dir::Down)];
        assert!(proposals(&on_edge, &t, &[], AREA, 0).list.is_empty());
        assert!(on_off_sheet_segment(&on_edge, &t));
        assert!(
            !on_off_sheet_segment(&[ghost(0, (0, 0), Dir::Up)], &t),
            "a segment's end"
        );
        assert!(
            !on_off_sheet_segment(&[ghost(0, (0, 20 * G), Dir::Up)], &t),
            "a segment's end"
        );
        let on_wire = [ghost(0, (8 * G, 0), Dir::Left), ghost(1, (12 * G, 0), Dir::Right)];
        assert!(!on_off_sheet_segment(&on_wire, &t));
        let p = proposals(&on_wire, &t, &[], AREA, 0);
        assert!(
            matches!(
                p.list.as_slice(),
                [Proposal {
                    target: ProposalTarget::Split { .. },
                    ..
                }]
            ),
            "the wire's own segment still splits: {p:?}"
        );
    }

    /// A representation of the tables (`terminal`: of a terminal) with the CPs `cps`.
    fn terminal_rep(t: &mut PageTables, n: u128, terminal: bool, cps: &[(u128, &str)]) {
        t.reps.push(TableRep {
            id: uuid(0x300 + n),
            functions: Vec::new(),
            device: None,
            symbol: uuid(0x500),
            symbol_key: String::new(),
            orient: 0,
            rep_type: "multi_line".into(),
            pos: (0, 0),
            body: [0, 0, 0, 0],
            tag_at: None,
            ip: None,
            black_box: None,
            terminal,
            locked: false,
        });
        for (k, label) in cps {
            let mut c = cp(*k, (0, 0), Dir::Up, label);
            c.rep = uuid(0x300 + n);
            t.cps.push(c);
        }
    }

    #[test]
    fn a_cp_of_a_terminal_of_the_ghost_s_strip_is_no_hit() {
        let mut t = tables();
        terminal_rep(&mut t, 1, true, &[(1, "-X1:1/a")]);
        t.cps[0].at = (0, 10 * G);
        let mut g = [ghost(0, (0, 0), Dir::Down)];
        g[0].strip_tag = Some("-X1".into());
        assert!(proposals(&g, &t, &[], AREA, 0).list.is_empty());
        // Another strip's terminal, and a ghost that is no terminal, take it as today.
        g[0].strip_tag = Some("-X2".into());
        assert_eq!(proposals(&g, &t, &[], AREA, 0).list.len(), 1);
        g[0].strip_tag = None;
        assert_eq!(proposals(&g, &t, &[], AREA, 0).list.len(), 1);
        // A non-terminal whose labels happen to print the tag is taken.
        t.reps[0].terminal = false;
        g[0].strip_tag = Some("-X1".into());
        assert_eq!(proposals(&g, &t, &[], AREA, 0).list.len(), 1);
    }

    #[test]
    fn a_moved_terminal_s_strip_tag_is_its_labels_text_before_the_last_colon() {
        let mut t = tables();
        terminal_rep(&mut t, 1, true, &[(1, "-X1:70/a"), (2, "-X1:70/b")]);
        terminal_rep(&mut t, 2, true, &[(3, "-X1:71/a"), (4, "-X2:71/b")]);
        terminal_rep(&mut t, 3, false, &[(5, "-K1:13"), (6, "-K1:14")]);
        terminal_rep(&mut t, 4, true, &[(7, "70")]);
        let tag = |n: u128| moved_strip_tag(&t, uuid(0x300 + n));
        assert_eq!(tag(1).as_deref(), Some("-X1"));
        assert_eq!(tag(2), None, "labels that disagree");
        assert_eq!(tag(3), None, "a non-terminal");
        assert_eq!(tag(4), None, "a label without a colon");
        assert_eq!(tag(5), None, "no representation");
        let all = moved_cps(&t, &|_| Some((0, 0)));
        assert_eq!(all[0].strip_tag.as_deref(), Some("-X1"));
    }
}
