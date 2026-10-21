// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The record emitters and their shared context: the key maps of every emitted row, the CP lists of every
//! function and the endpoint notation of doc 20 §1.2 (README "CP and endpoint notation").

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::Ident;
use ew_schema::{
    ClampKey, Conductor, Connection, Device, Endpoint, ExternalRef, Function, Junction, LibDocument, Page,
    Part, Project, Record, Section, Side, StructureNode,
};
use ew_units::{Grid, Nm, Pt2};

use crate::library::BuiltinLibrary;
use crate::table::{Findings, Row};

pub(crate) mod connect;
pub(crate) mod devices;
pub(crate) mod parts;
pub(crate) mod profiles;
pub(crate) mod project;

/// One CP of a function: its index and what the endpoint notation matches on.
#[derive(Clone, Debug)]
pub(crate) struct CpInfo {
    /// The CP index (`Endpoint::Cp(_, idx)`).
    pub(crate) idx: u16,
    /// The designation.
    pub(crate) designation: Ident,
    /// The terminal side.
    pub(crate) side: Option<Side>,
    /// The terminal clamp.
    pub(crate) clamp: Option<ClampKey>,
}

/// An emitted function.
#[derive(Clone, Debug)]
pub(crate) struct FnInfo {
    /// The record ID.
    pub(crate) id: Id<Function>,
    /// The device key; `None` for an interruption point.
    pub(crate) device: Option<String>,
    /// Its CPs in index order.
    pub(crate) cps: Vec<CpInfo>,
    /// Whether it is an interruption point.
    pub(crate) ip: bool,
    /// Whether it was emitted with `kind = black_box`.
    pub(crate) black_box: bool,
}

/// An emitted device (terminals included).
#[derive(Clone, Debug)]
pub(crate) struct DeviceInfo {
    /// The record ID.
    pub(crate) id: Id<Device>,
    /// The strip's field side, when the device is a strip.
    pub(crate) strip_field_side: Option<Side>,
    /// Whether the device has the cable aspect.
    pub(crate) cable: bool,
    /// The `parts` keys of its part uses, in `Device.parts` order.
    pub(crate) parts: Vec<String>,
}

/// An emitted page.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PageInfo {
    /// The record ID.
    pub(crate) id: Id<Page>,
    /// The page grid.
    pub(crate) grid: Grid,
}

/// One function of a part's template as the device emitters see it.
#[derive(Clone, Debug)]
pub(crate) struct TemplateInfo {
    /// The FnDef key.
    pub(crate) fndef: String,
    /// `TemplateFn.main`: the role the template function has, and that a function must have to match it.
    pub(crate) main: bool,
    /// Its pattern indexes, in template order (never empty).
    pub(crate) cps: Vec<u16>,
}

/// A part copy as the device emitters see it.
#[derive(Clone, Debug)]
pub(crate) struct PartInfo {
    /// The copy ID.
    pub(crate) id: Id<Part>,
    /// The CP pattern, by pattern index.
    pub(crate) pattern: BTreeMap<u16, CpInfo>,
    /// The function templates, in `n` order.
    pub(crate) templates: Vec<TemplateInfo>,
}

/// An emitted connection: its ID and endpoints (the junction emitter checks sequences against them).
#[derive(Clone, Debug)]
pub(crate) struct ConnInfo {
    /// The record ID.
    pub(crate) id: Id<Connection>,
    /// Endpoint `a`.
    pub(crate) a: Endpoint,
    /// Endpoint `b`.
    pub(crate) b: Endpoint,
}

/// Checks the key of `row`: non-empty and not yet in `seen`; reports EW-IMP-002 and returns `false` otherwise.
/// A free function, so that `seen` may be a map of the context while its findings are borrowed mutably.
pub(crate) fn fresh<V>(findings: &mut Findings, row: Row<'_>, seen: &BTreeMap<String, V>) -> bool {
    let problem = if row.key().is_empty() {
        "row dropped: empty key"
    } else if seen.contains_key(row.key()) {
        "row dropped: duplicate key"
    } else {
        return true;
    };
    findings.at(
        Code::MALFORMED_ITEM,
        Severity::Error,
        row.file(),
        Some(row.key()),
        Some(row.line()),
        problem,
    );
    false
}

/// The emitter context.
pub(crate) struct Cx<'l> {
    /// The built-in library.
    pub(crate) lib: &'l BuiltinLibrary,
    /// The findings.
    pub(crate) findings: Findings,
    /// The project ID (the library ID of project-local parts).
    pub(crate) project: Id<Project>,
    /// The emitted records (sorted at the end).
    pub(crate) records: Vec<Record>,
    /// Project-local library documents (parts).
    pub(crate) local_lib: Vec<LibDocument>,
    /// Keys of the built-in documents referenced so far.
    pub(crate) used_lib: BTreeSet<String>,
    /// `parts` keys.
    pub(crate) parts: BTreeMap<String, PartInfo>,
    /// `structure` keys.
    pub(crate) structure: BTreeMap<String, Id<StructureNode>>,
    /// `external_refs` keys.
    pub(crate) external_refs: BTreeMap<String, Id<ExternalRef>>,
    /// `sections` keys.
    pub(crate) sections: BTreeMap<String, Id<Section>>,
    /// `pages` keys.
    pub(crate) pages: BTreeMap<String, PageInfo>,
    /// `devices` and `terminals` keys.
    pub(crate) devices: BTreeMap<String, DeviceInfo>,
    /// `functions` keys.
    pub(crate) functions: BTreeMap<String, FnInfo>,
    /// Device key → its function keys, in table order.
    pub(crate) device_functions: BTreeMap<String, Vec<String>>,
    /// `junctions` keys (registered before the connections, which may end at them).
    pub(crate) junctions: BTreeMap<String, Id<Junction>>,
    /// `conductors` keys.
    pub(crate) conductors: BTreeMap<String, Id<Conductor>>,
    /// `connections` keys.
    pub(crate) connections: BTreeMap<String, ConnInfo>,
}

impl<'l> Cx<'l> {
    /// A fresh context.
    pub(crate) fn new(lib: &'l BuiltinLibrary, project: Id<Project>) -> Cx<'l> {
        Cx {
            lib,
            findings: Findings::default(),
            project,
            records: Vec::new(),
            local_lib: Vec::new(),
            used_lib: BTreeSet::new(),
            parts: BTreeMap::new(),
            structure: BTreeMap::new(),
            external_refs: BTreeMap::new(),
            sections: BTreeMap::new(),
            pages: BTreeMap::new(),
            devices: BTreeMap::new(),
            functions: BTreeMap::new(),
            device_functions: BTreeMap::new(),
            junctions: BTreeMap::new(),
            conductors: BTreeMap::new(),
            connections: BTreeMap::new(),
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Findings about rows

    /// EW-IMP-002 (error): row `row` is malformed and dropped.
    pub(crate) fn malformed(&mut self, row: Row<'_>, message: &str) {
        self.findings.at(
            Code::MALFORMED_ITEM,
            Severity::Error,
            row.file(),
            Some(row.key()),
            Some(row.line()),
            format!("row dropped: {message}"),
        );
    }

    /// EW-IMP-003 (error): a reference of row `row` does not resolve; the row is dropped.
    pub(crate) fn unresolved(&mut self, row: Row<'_>, message: &str) {
        self.findings.at(
            Code::UNRESOLVED_REFERENCE,
            Severity::Error,
            row.file(),
            Some(row.key()),
            Some(row.line()),
            format!("row dropped: {message}"),
        );
    }

    /// Reports the rows of a parent column dropped by [`parent_fates`]: EW-IMP-002 for a row on a cycle,
    /// EW-IMP-003 for a row whose parent was dropped. `rows[i]` and `fates[i]` belong together.
    pub(crate) fn report_fates(&mut self, rows: &[Row<'_>], fates: &[Fate], what: &str) {
        for (row, fate) in rows.iter().zip(fates) {
            match fate {
                Fate::Kept | Fate::Dropped => {}
                Fate::Cycle => self.malformed(*row, &format!("parent chain is a cycle ({what})")),
                Fate::Orphaned(p) => {
                    let parent = rows.get(*p).map_or("", |r| r.key());
                    self.unresolved(*row, &format!("its parent `{parent}` was dropped"));
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Library references

    /// The FnDef copy ID of built-in `key`, marking it used.
    pub(crate) fn fndef_id(&mut self, key: &str) -> Option<Id<ew_schema::FnDef>> {
        let id = self.lib.fndef(key)?.id;
        self.used_lib.insert(key.to_owned());
        Some(id)
    }

    /// The form copy ID of built-in `key`, marking it used.
    pub(crate) fn form_id(&mut self, key: &str) -> Option<Id<ew_schema::FormDef>> {
        let id = self.lib.form(key)?.id;
        self.used_lib.insert(key.to_owned());
        Some(id)
    }

    // -----------------------------------------------------------------------------------------------------
    // Endpoints

    /// The endpoint named by `text` in the notation of doc 20 §1.2.
    pub(crate) fn endpoint(&self, text: &str) -> Result<Endpoint, String> {
        if let Some(j) = text.strip_prefix('@') {
            return self
                .junctions
                .get(j)
                .map(|id| Endpoint::Junction(*id))
                .ok_or_else(|| format!("unknown junction `{j}`"));
        }
        if let Some(f) = text.strip_prefix("ip:") {
            let info = self
                .functions
                .get(f)
                .ok_or_else(|| format!("unknown interruption point `{f}`"))?;
            if !info.ip {
                return Err(format!("`{f}` is not an interruption point"));
            }
            let cp = info.cps.first().ok_or_else(|| format!("`{f}` has no CP"))?;
            return Ok(Endpoint::Cp(info.id, cp.idx));
        }
        let (owner, rest) = text.split_once(':').ok_or_else(|| {
            format!("`{text}` is not an endpoint (`<device>:<cp>`, `ip:<point>` or `@<junction>`)")
        })?;
        let device = self
            .devices
            .get(owner)
            .ok_or_else(|| format!("unknown device `{owner}`"))?;
        if let Some(field_side) = &device.strip_field_side {
            let (designation, sel) = rest.split_once('/').ok_or_else(|| {
                format!("`{text}`: a terminal CP needs a selector (`/a`, `/s1`, `/field` …)")
            })?;
            let terminal = format!("{owner}:{designation}");
            if !self.devices.contains_key(&terminal) {
                return Err(format!("unknown terminal `{terminal}`"));
            }
            return self.terminal_cp(&terminal, sel, field_side);
        }
        let (designation, nth) = match rest.rsplit_once('#') {
            Some((d, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                let n: usize = n.parse().map_err(|_| format!("`{text}`: bad occurrence"))?;
                if n == 0 {
                    return Err(format!("`{text}`: occurrences count from 1"));
                }
                (d, Some(n))
            }
            _ => (rest, None),
        };
        let mut hits = Vec::new();
        for f in self.device_functions.get(owner).into_iter().flatten() {
            if let Some(info) = self.functions.get(f) {
                for cp in &info.cps {
                    if cp.designation.as_str() == designation {
                        hits.push(Endpoint::Cp(info.id, cp.idx));
                    }
                }
            }
        }
        match (nth, hits.len()) {
            (_, 0) => Err(format!("device `{owner}` has no CP `{designation}`")),
            (None, 1) => Ok(hits.remove(0)),
            (None, n) => Err(format!(
                "`{text}` is ambiguous ({n} CPs `{designation}`; add `#<n>`)"
            )),
            (Some(k), n) if k <= n => Ok(hits.swap_remove(k - 1)),
            (Some(k), n) => Err(format!("`{text}`: occurrence {k} of {n}")),
        }
    }

    fn terminal_cp(&self, terminal: &str, sel: &str, field_side: &Side) -> Result<Endpoint, String> {
        let matches = |cp: &CpInfo| -> bool {
            match sel {
                "field" => cp.side.as_ref() == Some(field_side),
                "internal" => cp.side.is_some() && cp.side.as_ref() != Some(field_side),
                "rail" => cp.clamp == Some(ClampKey::RailFoot),
                _ => {
                    if let Some(n) = sel.strip_prefix('s').and_then(|n| n.parse::<u8>().ok()) {
                        cp.clamp == Some(ClampKey::Shaft(n))
                    } else {
                        let mut chars = sel.chars();
                        match (chars.next(), chars.next()) {
                            (Some(c), None) if c.is_ascii_lowercase() => cp.clamp == Some(ClampKey::Clamp(c)),
                            _ => false,
                        }
                    }
                }
            }
        };
        let mut hits = Vec::new();
        for f in self.device_functions.get(terminal).into_iter().flatten() {
            if let Some(info) = self.functions.get(f) {
                hits.extend(
                    info.cps
                        .iter()
                        .filter(|cp| matches(cp))
                        .map(|cp| Endpoint::Cp(info.id, cp.idx)),
                );
            }
        }
        match hits.len() {
            0 => Err(format!("terminal `{terminal}` has no CP `/{sel}`")),
            1 => Ok(hits.remove(0)),
            n => Err(format!("terminal `{terminal}`: `/{sel}` is ambiguous ({n} CPs)")),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Positions on the QET-compatible A4 frame (doc 20 §17.1)

/// Header band, column width and row height of the frame, in QET px.
const HEADER_PX: i64 = 20;
const COLUMN_PX: i64 = 60;
const ROW_PX: i64 = 80;

/// The cell `<row><column>` (`B8`: row B, column 8) as `(column, row)` indexes, columns 0–16, rows A–H.
pub(crate) fn parse_cell(cell: &str) -> Result<(i64, i64), String> {
    let mut chars = cell.chars();
    let row = chars.next().ok_or("empty cell")?;
    let row = match row {
        'A'..='H' => i64::from(u32::from(row) - u32::from('A')),
        _ => return Err(format!("cell `{cell}`: row must be A–H")),
    };
    let col: &str = chars.as_str();
    if col.is_empty() || col.len() > 2 || !col.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("cell `{cell}`: column must be 0–16"));
    }
    let col: i64 = col.parse().map_err(|_| format!("cell `{cell}`: bad column"))?;
    if col > 16 {
        return Err(format!("cell `{cell}`: column must be 0–16"));
    }
    Ok((col, row))
}

/// The centre of `cell` plus `(dx, dy)` QET px, in nm, rounded to the nm: `round(px × 793 750 / 3)`.
pub(crate) fn cell_point(cell: (i64, i64), dx: i64, dy: i64) -> Option<Pt2> {
    let x = HEADER_PX + COLUMN_PX * cell.0 + COLUMN_PX / 2 + dx;
    let y = HEADER_PX + ROW_PX * cell.1 + ROW_PX / 2 + dy;
    let px = Grid::QET_1PX;
    Some(Pt2::new(px.at(x)?, px.at(y)?))
}

/// Why a table position cannot be placed. The bootstrap never snaps (doc 06 §5.7, decision log ID-54; doc 14
/// §9 strict mode): the row is dropped with EW-IMP-002.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PlaceError {
    /// The target point is not a point of the page grid.
    OffGrid,
    /// No grid position puts the anchor on the target: the anchor lies within 1 nm of a half-grid tie that
    /// the nm rounding of the position tips to the wrong side.
    Unreachable,
    /// An overflow.
    OutOfRange,
}

impl PlaceError {
    /// The finding text.
    pub(crate) fn message(self) -> &'static str {
        match self {
            PlaceError::OffGrid => "position off the page grid (positions are never snapped, doc 06 §5.7)",
            PlaceError::Unreachable => {
                "no grid position puts the symbol's anchor exactly on this cell position (anchor at a half-grid tie)"
            }
            PlaceError::OutOfRange => "position out of range",
        }
    }
}

/// The grid index of `x` when `x` is a point of `grid` (the nm rounding of `k · grid`), else `None`.
pub(crate) fn grid_index(grid: Grid, x: Nm) -> Option<i64> {
    let k = grid.snap(x);
    (grid.at(k)? == x).then_some(k)
}

/// The position on `grid` whose anchor point `pos + anchor` lies at `target` exactly as
/// `ew_derive::rep_anchor_cell` reads it: the derived anchor point is snapped to the page grid before its cell is
/// looked up (doc 06 §5.1), so `snap(pos + anchor) = index(target)` is checked per axis, not assumed.
///
/// `target` must be a grid point ([`PlaceError::OffGrid`] otherwise). `anchor` is the symbol's anchor in symbol
/// coordinates ([`ew_schema::SymbolDef::anchor_point`]) under the identity orientation; it need not lie on the
/// grid (a graphic centre may fall between grid points). `pos` is a grid point: `index(target) − snap(anchor)`,
/// else its neighbour, whichever makes the derived anchor snap onto the target (both can differ only when the
/// anchor sits at a half-grid tie, where the 0.5 nm rounding of `pos` decides);
/// [`PlaceError::Unreachable`] if neither does.
pub(crate) fn place(grid: Grid, target: Pt2, anchor: Pt2) -> Result<Pt2, PlaceError> {
    let axis = |t: Nm, a: Nm| -> Result<Nm, PlaceError> {
        let kt = grid_index(grid, t).ok_or(PlaceError::OffGrid)?;
        let k0 = kt.checked_sub(grid.snap(a)).ok_or(PlaceError::OutOfRange)?;
        for k in [Some(k0), k0.checked_sub(1), k0.checked_add(1)]
            .into_iter()
            .flatten()
        {
            let pos = grid.at(k).ok_or(PlaceError::OutOfRange)?;
            let derived = pos.0.checked_add(a.0).ok_or(PlaceError::OutOfRange)?;
            if grid.snap(Nm(derived)) == kt {
                return Ok(pos);
            }
        }
        Err(PlaceError::Unreachable)
    };
    Ok(Pt2::new(axis(target.x, anchor.x)?, axis(target.y, anchor.y)?))
}

/// What happens to one row of a table with a parent column ([`parent_fates`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Fate {
    /// Kept.
    Kept,
    /// Dropped before the walk (an unknown parent, already reported).
    Dropped,
    /// On a parent cycle, a row naming itself included: dropped with EW-IMP-002.
    Cycle,
    /// Its parent (the row index) is dropped, so it is dropped too, with EW-IMP-003.
    Orphaned(usize),
}

/// The fate of every row of a parent forest: `parents[i]` is the row index of row `i`'s parent, `pre[i]`
/// whether row `i` is dropped already. Every row on a parent cycle is a [`Fate::Cycle`]; every row whose parent
/// is dropped (before or by the walk) is [`Fate::Orphaned`] by that parent.
///
/// One iterative depth-first walk with three states per row (unvisited, on the current path, decided): every
/// row is pushed, popped and decided once, so the walk is O(N) in time and memory, without recursion (a deep
/// chain needs no call stack).
pub(crate) fn parent_fates(parents: &[Option<usize>], pre: &[bool]) -> Vec<Fate> {
    const UNVISITED: u8 = 0;
    const ON_PATH: u8 = 1;
    const DECIDED: u8 = 2;
    let n = parents.len();
    let mut fate: Vec<Fate> = (0..n)
        .map(|i| {
            if pre.get(i).copied().unwrap_or(false) {
                Fate::Dropped
            } else {
                Fate::Kept
            }
        })
        .collect();
    let mut state: Vec<u8> = fate
        .iter()
        .map(|f| if *f == Fate::Dropped { DECIDED } else { UNVISITED })
        .collect();
    let mut path: Vec<usize> = Vec::new();
    for start in 0..n {
        if state[start] != UNVISITED {
            continue;
        }
        // Walk up until a root, a decided row or a row on the current path (a cycle).
        let mut cur = start;
        let end = loop {
            state[cur] = ON_PATH;
            path.push(cur);
            match parents[cur].filter(|p| *p < n) {
                None => break None,
                Some(p) if state[p] == UNVISITED => cur = p,
                Some(p) => break Some(p),
            }
        };
        // `kept`: whether the row above the current top of the path is kept.
        let mut kept = match end {
            None => true,
            Some(p) if state[p] == DECIDED => fate[p] == Fate::Kept,
            Some(p) => {
                // `p` is on the path: the rows from `p` to the top form the cycle.
                let at = path.iter().rposition(|x| *x == p).unwrap_or(0);
                for &c in &path[at..] {
                    fate[c] = Fate::Cycle;
                    state[c] = DECIDED;
                }
                path.truncate(at);
                false
            }
        };
        // Unwind: each row takes its parent's outcome.
        while let Some(c) = path.pop() {
            if !kept {
                fate[c] = Fate::Orphaned(parents[c].unwrap_or(c));
            }
            kept = fate[c] == Fate::Kept;
            state[c] = DECIDED;
        }
    }
    fate
}

/// The page grid of `pages.csv`'s `grid` column.
pub(crate) fn grid_named(name: &str) -> Option<Grid> {
    match name {
        "qet10" => Some(Grid::QET_10PX),
        "mm4" => Some(Grid::MM4),
        _ => None,
    }
}

/// `Nm` of a whole number of mm given as Dec3 thousandths of a mm (`Dec3(2500)` = 2.5 mm).
pub(crate) fn dec3_mm_to_nm(d: ew_units::Dec3) -> Option<Nm> {
    d.0.checked_mul(1_000).map(Nm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells() {
        assert_eq!(parse_cell("B8"), Ok((8, 1)));
        assert_eq!(parse_cell("H16"), Ok((16, 7)));
        assert!(parse_cell("I1").is_err());
        assert!(parse_cell("A17").is_err());
        assert!(parse_cell("A").is_err());
    }

    #[test]
    fn cell_centres_lie_on_the_qet_grid() {
        // Cell A0: centre at (50, 60) px = grid index (5, 6) of the 10 px grid.
        let g = Grid::QET_10PX;
        let p = cell_point((0, 0), 0, 0).unwrap();
        assert_eq!(
            place(g, p, Pt2::ORIGIN),
            Ok(Pt2::new(g.at(5).unwrap(), g.at(6).unwrap()))
        );
        // 5 px off the grid is refused, never snapped.
        let p = cell_point((0, 0), 5, 0).unwrap();
        assert_eq!(place(g, p, Pt2::ORIGIN), Err(PlaceError::OffGrid));
    }

    #[test]
    fn placement_puts_the_anchor_on_the_target() {
        let g = Grid::QET_10PX;
        let target = cell_point((8, 1), 0, 0).unwrap();
        // A named anchor on the grid (20 px down), an off-grid graphic centre (25 px right, between two grid
        // points: a tie) and a negative one off the ties: the derived anchor point snaps back onto the target.
        for anchor in [
            Pt2::new(Nm(0), g.at(2).unwrap()),
            Pt2::new(Nm(6_614_583), Nm(0)),
            Pt2::new(Nm(-6_000_000), Nm(-13_229_167)),
        ] {
            let pos = place(g, target, anchor).unwrap();
            assert!(grid_index(g, pos.x).is_some() && grid_index(g, pos.y).is_some());
            let derived = Pt2::new(Nm(pos.x.0 + anchor.x.0), Nm(pos.y.0 + anchor.y.0));
            assert_eq!(g.snap(derived.x), g.snap(target.x), "{anchor:?}");
            assert_eq!(g.snap(derived.y), g.snap(target.y), "{anchor:?}");
        }
        // −25 px (−6,614,583 nm) is a tie at −2.5 grid steps: from column index 53 the derived points are
        // 52.5 → 52 (ties toward zero), 51.5 + 1 nm → 52 and 53.5 + 1 nm → 54, so 53 cannot be reached.
        assert_eq!(
            place(g, target, Pt2::new(Nm(-6_614_583), Nm(0))),
            Err(PlaceError::Unreachable)
        );
        // Whatever the anchor, a placement either lands exactly or is refused.
        for a in (-20_000_000..20_000_000).step_by(333_331) {
            for col in 0..17 {
                let t = cell_point((col, 3), 0, 0).unwrap();
                if let Ok(pos) = place(g, t, Pt2::new(Nm(a), Nm(a))) {
                    assert_eq!(g.snap(Nm(pos.x.0 + a)), g.snap(t.x), "{a} {col}");
                    assert_eq!(g.snap(Nm(pos.y.0 + a)), g.snap(t.y), "{a} {col}");
                }
            }
        }
    }

    #[test]
    fn parent_fates_find_cycles_and_orphans() {
        // 0 ← 1 ← 2 (a chain), 3 ↔ 4 (a cycle) ← 5, 6 names itself, 7 under the pre-dropped 8.
        let parents = [
            None,
            Some(0),
            Some(1),
            Some(4),
            Some(3),
            Some(4),
            Some(6),
            Some(8),
            None,
        ];
        let pre = [false, false, false, false, false, false, false, false, true];
        assert_eq!(
            parent_fates(&parents, &pre),
            [
                Fate::Kept,
                Fate::Kept,
                Fate::Kept,
                Fate::Cycle,
                Fate::Cycle,
                Fate::Orphaned(4),
                Fate::Cycle,
                Fate::Orphaned(8),
                Fate::Dropped,
            ]
        );
    }

    #[test]
    fn parent_fates_walk_a_deep_chain_without_recursion() {
        // Row i's parent is row i + 1; the last two rows form a cycle. Every other row is orphaned.
        let n = 200_000;
        let mut parents: Vec<Option<usize>> = (0..n).map(|i| Some(i + 1)).collect();
        parents[n - 1] = Some(n - 2);
        let f = parent_fates(&parents, &vec![false; n]);
        assert_eq!(f.iter().filter(|x| **x == Fate::Cycle).count(), 2);
        assert!(
            f[..n - 2]
                .iter()
                .enumerate()
                .all(|(i, x)| *x == Fate::Orphaned(i + 1))
        );
    }
}
