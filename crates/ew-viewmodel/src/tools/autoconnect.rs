// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Autoconnect proposals (WP-S2.04 §8.3, FR-CON-003, FR-CON-008; doc 06 §5.2, ADR-0007): what a floating or dragged
//! symbol's free connection points would connect to, computed from the page tables alone (no `Session` call) and
//! drawn as dashed overlay stubs. They reach the model only as the `connect` hints of the committed `device.place`
//! (D4); during a move they are shown and never committed.
//!
//! **The walk.** For each free ghost CP, in ascending symbol CP index, the ray leaves the CP along its direction, one
//! grid step at a time, inside the search area: a representation body (other than the ghost's own) stops it; a free
//! CP whose direction is opposite to the ray's, at the walk point (equal grid indices on both axes), is a hit; a
//! connection segment collinear with the ray at the walk point stops it (the line is occupied); a segment crossing
//! the ray is passed. Several CPs at one walk point are hits in label order (the page tables' order, FT-439; never
//! the v5 ID, which follows the minted function ID). The proposal of a ray is its
//! first hit; [`ToolKey::NextProposal`](super::ToolKey::NextProposal) (Tab) cycles every ray to its next hit further
//! along, wrapping.
//!
//! **The split.** Two ghost CPs that lie on one segment of a connection W and face its two ends (their directions
//! along the segment, pointing apart) are the split of W: with a stored route they are one proposal and send no hint,
//! because normaliser 7 splits W when the symbol lands on its route (FR-CON-008, doc 06 §5.3); on a route-less W no
//! split is proposed and the status hint asks for a route first.

use ew_api::geometry::Dir;
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::{PageTables, TableEnd};
use serde_json::{Value, json};

/// The most grid steps a ray walks.
pub const MAX_WALK: usize = 2_000;

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
    /// The split of a routed connection by two ghost CPs (no hint: normaliser 7 splits it).
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
}

/// The proposals of a ghost, and whether a split was not proposed because the connection has no route.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Proposals {
    /// The proposals in ascending symbol CP index; a split counts once.
    pub list: Vec<Proposal>,
    /// Two ghost CPs face the ends of a route-less connection (the hint: give it a route first).
    pub needs_route: bool,
}

impl Proposals {
    /// The `connect` hints of `device.place`: one per CP proposal (a split sends none).
    pub fn hints(&self) -> Vec<Value> {
        self.list
            .iter()
            .filter_map(|p| match &p.target {
                ProposalTarget::Cp { function, idx, .. } => Some(json!({
                    "cp": p.cp.symbol_cp,
                    "to": {"cp": {"function": function.0.to_string(), "idx": idx}},
                })),
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

/// `num / den` rounded to the nearest integer, ties toward zero (`ew_units::Grid`'s rule).
fn div_round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() > den { q + num.signum() } else { q }
}

/// The grid of the page tables: index and coordinate.
#[derive(Clone, Copy)]
struct GridMap {
    num: i128,
    den: i128,
}

impl GridMap {
    fn of(tables: &PageTables) -> GridMap {
        GridMap {
            num: i128::from(tables.grid.0.max(1)),
            den: i128::from(tables.grid.1.max(1)),
        }
    }

    fn index(self, v: i64) -> i64 {
        i64::try_from(div_round(i128::from(v) * self.den, self.num)).unwrap_or(0)
    }

    fn coord(self, k: i64) -> i64 {
        i64::try_from(div_round(i128::from(k) * self.num, self.den)).unwrap_or(0)
    }
}

fn step(d: Dir) -> (i64, i64) {
    d.vector()
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

/// The hits of one ray (see the module documentation), nearest first.
fn ray_hits(
    cp: &GhostCp,
    tables: &PageTables,
    own: &[EntityRef],
    area: [i64; 4],
    grid: GridMap,
) -> Vec<ProposalTarget> {
    let d = step(cp.dir);
    let (ix, iy) = (grid.index(cp.at.0), grid.index(cp.at.1));
    let want = opposite(cp.dir);
    let mut hits = Vec::new();
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
        // Free CPs facing the ray at the walk point, in label order (the rows' order, FT-439).
        for q in &tables.cps {
            if grid.index(q.at.0) == kx
                && grid.index(q.at.1) == ky
                && !own.contains(&EntityRef(q.rep))
                && q.connections.is_empty()
                && q.dir == Some(want)
            {
                hits.push(ProposalTarget::Cp {
                    function: EntityRef(q.function),
                    idx: q.idx,
                    at: q.at,
                    label: q.label.clone(),
                });
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
    }
    hits
}

/// The splits of the ghost: pairs of ghost CPs on one segment of a connection facing its two ends. Returns the
/// proposals of the routed ones and whether a route-less one was met.
fn splits(cps: &[GhostCp], tables: &PageTables, grid: GridMap) -> (Vec<Proposal>, bool) {
    let mut out = Vec::new();
    let mut needs_route = false;
    for c in &tables.connections {
        let line = dedup(&c.polyline);
        for w in line.windows(2) {
            let (a, b) = (w[0], w[1]);
            let horizontal = a.1 == b.1 && a.0 != b.0;
            let vertical_seg = a.0 == b.0 && a.1 != b.1;
            if !(horizontal || vertical_seg) {
                continue;
            }
            let on = |p: (i64, i64)| {
                if horizontal {
                    grid.index(p.1) == grid.index(a.1) && a.0.min(b.0) < p.0 && p.0 < a.0.max(b.0)
                } else {
                    grid.index(p.0) == grid.index(a.0) && a.1.min(b.1) < p.1 && p.1 < a.1.max(b.1)
                }
            };
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
                    if c.stored_route {
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
                        });
                    } else {
                        needs_route = true;
                    }
                }
            }
        }
    }
    (out, needs_route)
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
    let grid = GridMap::of(tables);
    let (split_list, needs_route) = splits(cps, tables, grid);
    let in_split = |c: &GhostCp| {
        split_list.iter().any(|p| {
            p.cp.symbol_cp == c.symbol_cp
                || matches!(&p.target, ProposalTarget::Split { other, .. } if other.0 == c.symbol_cp)
        })
    };
    let mut sorted: Vec<&GhostCp> = cps.iter().collect();
    sorted.sort_by_key(|c| c.symbol_cp);
    let mut list: Vec<Proposal> = Vec::new();
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
            continue;
        }
        list.push(Proposal {
            cp: c.clone(),
            target: hits[cycle % hits.len()].clone(),
        });
    }
    Proposals { list, needs_route }
}

/// The ghost CPs of the representations `reps` of the page moved by the page displacements `delta(rep)`: their table
/// CPs at the moved points, the designation the label prints after its colon.
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
            at: (cp.at.0 + d.0, cp.at.1 + d.1),
            dir,
            connected: !cp.connections.is_empty(),
        });
    }
    out
}

/// Whether `end` is a connection point of the page (a junction end is not).
pub fn is_cp(end: &TableEnd) -> bool {
    matches!(end, TableEnd::Cp { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::{TableConnection, TableCp, TableRep};

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
            plc_address: None,
        }
    }

    fn ghost(symbol_cp: u16, at: (i64, i64), dir: Dir) -> GhostCp {
        GhostCp {
            symbol_cp,
            designation: (symbol_cp + 1).to_string(),
            at,
            dir,
            connected: false,
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
        });
        let g = [ghost(0, (0, 0), Dir::Down)];
        assert!(proposals(&g, &t, &[], AREA, 0).list.is_empty());
        // The ghost's own body does not stop it.
        assert_eq!(
            proposals(&g, &t, &[EntityRef(uuid(0x400))], AREA, 0).list.len(),
            1
        );
        t.reps.clear();
        // A crossing segment is passed.
        t.connections.push(TableConnection {
            id: uuid(0x600),
            a: TableEnd::Junction { id: uuid(1) },
            b: TableEnd::Junction { id: uuid(2) },
            kind: "wire".into(),
            polyline: vec![(-5 * G, 5 * G), (5 * G, 5 * G)],
            stored_route: true,
        });
        assert_eq!(proposals(&g, &t, &[], AREA, 0).list.len(), 1);
        // A segment along the ray occupies the line.
        t.connections.push(TableConnection {
            id: uuid(0x601),
            a: TableEnd::Junction { id: uuid(1) },
            b: TableEnd::Junction { id: uuid(2) },
            kind: "wire".into(),
            polyline: vec![(0, 3 * G), (0, 7 * G)],
            stored_route: true,
        });
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
        t.connections.push(TableConnection {
            id: uuid(0x600),
            a: TableEnd::Junction { id: uuid(1) },
            b: TableEnd::Junction { id: uuid(2) },
            kind: "wire".into(),
            polyline: vec![(0, 0), (0, 0), (0, 20 * G), (0, 20 * G)],
            stored_route: true,
        });
        let g = [ghost(0, (0, 8 * G), Dir::Up), ghost(1, (0, 12 * G), Dir::Down)];
        let p = proposals(&g, &t, &[], AREA, 0);
        assert_eq!(p.list.len(), 1, "{p:?}");
        assert!(matches!(&p.list[0].target, ProposalTarget::Split { .. }));
        assert!(p.hints().is_empty());
        assert_eq!(p.lines(), vec![("\"1\" \"2\"".to_owned(), None)]);
        // Route-less: no split, and the hint asks for a route.
        t.connections[0].stored_route = false;
        let p = proposals(&g, &t, &[], AREA, 0);
        assert!(p.list.is_empty());
        assert!(p.needs_route);
    }
}
