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
}

/// An emitted page.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PageInfo {
    /// The record ID.
    pub(crate) id: Id<Page>,
    /// The page grid.
    pub(crate) grid: Grid,
}

/// A part copy as the device emitters see it.
#[derive(Clone, Debug)]
pub(crate) struct PartInfo {
    /// The copy ID.
    pub(crate) id: Id<Part>,
    /// The CP pattern, by pattern index.
    pub(crate) pattern: BTreeMap<u16, CpInfo>,
    /// The function templates: FnDef key and pattern indexes.
    pub(crate) templates: Vec<(String, Vec<u16>)>,
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

    /// EW-IMP-002 (warning): row `row` is kept with a corrected value.
    pub(crate) fn corrected(&mut self, row: Row<'_>, message: &str) {
        self.findings.at(
            Code::MALFORMED_ITEM,
            Severity::Warning,
            row.file(),
            Some(row.key()),
            Some(row.line()),
            message.to_owned(),
        );
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

/// `target` minus `anchor`, snapped to `grid`; the flag is set when `target` was not on the grid.
pub(crate) fn place_on_grid(grid: Grid, target: Pt2, anchor: Pt2) -> Option<(Pt2, bool)> {
    let kx = grid.snap(target.x);
    let ky = grid.snap(target.y);
    let off = grid.at(kx)? != target.x || grid.at(ky)? != target.y;
    let ax = grid.snap(anchor.x);
    let ay = grid.snap(anchor.y);
    let p = Pt2::new(grid.at(kx.checked_sub(ax)?)?, grid.at(ky.checked_sub(ay)?)?);
    Some((p, off))
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
        let p = cell_point((0, 0), 0, 0).unwrap();
        let (q, off) = place_on_grid(Grid::QET_10PX, p, Pt2::ORIGIN).unwrap();
        assert!(!off);
        assert_eq!(
            q,
            Pt2::new(Grid::QET_10PX.at(5).unwrap(), Grid::QET_10PX.at(6).unwrap())
        );
        // 5 px off the grid is snapped and flagged.
        let p = cell_point((0, 0), 5, 0).unwrap();
        assert!(place_on_grid(Grid::QET_10PX, p, Pt2::ORIGIN).unwrap().1);
    }
}
