// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the page draws its connection points (see the module documentation of `page`): the drawn CPs, the
//! tag's anchor, a connection's polyline between its drawn ends.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A drawn CP: its position and the direction a wire leaves it.
pub(crate) type CpMap = BTreeMap<(Uuid, u16), (Pt2, Dir4)>;

/// The point of every function CP the page draws, by `(function, CP index)`: the CP positions of the page scene
/// without the directions. `ew-derive`'s `cp_point` restates these rules over `Q` (the wire numbers order
/// wires by the cell of their ends), and a test holds the two equal on the sample's pages.
pub fn cp_points(view: &PageView) -> BTreeMap<(Uuid, u16), Pt2> {
    cp_map(view).into_iter().map(|(k, (pos, _))| (k, pos)).collect()
}

pub(crate) fn sorted_reps(view: &PageView) -> Vec<&RepView> {
    let mut reps: Vec<&RepView> = view.reps.iter().collect();
    reps.sort_by_key(|r| r.rep.id);
    reps
}

pub(crate) fn sorted_connections(view: &PageView) -> Vec<&ConnectionView> {
    let mut c: Vec<&ConnectionView> = view.connections.iter().collect();
    c.sort_by_key(|c| c.connection.id);
    c
}

/// The drawn function CPs of the page (see the module documentation).
pub(crate) fn cp_map(view: &PageView) -> CpMap {
    page_drawn_cps(view)
        .into_iter()
        .map(|(k, (pos, dir, _))| (k, (pos, dir)))
        .collect()
}

/// [`cp_map`] with the representation that draws each CP.
pub(super) fn page_drawn_cps(view: &PageView) -> BTreeMap<(Uuid, u16), (Pt2, Dir4, Uuid)> {
    drawn_cps(view.grid, &view.reps)
}

/// The function CPs that the representations `reps` draw on a page of `grid`, by `(function, CP index)`: each CP's
/// point, the direction a wire leaves it and the representation that draws it (see the module documentation). The
/// page tables of `ew-api` read them.
pub fn drawn_cps(grid: Grid, reps: &[RepView]) -> BTreeMap<(Uuid, u16), (Pt2, Dir4, Uuid)> {
    let mut sorted: Vec<&RepView> = reps.iter().collect();
    sorted.sort_by_key(|r| r.rep.id);
    let mut map: BTreeMap<(Uuid, u16), (Pt2, Dir4, Uuid)> = BTreeMap::new();
    for rv in sorted {
        let Some(def) = &rv.symbol else { continue };
        let rep = &rv.rep;
        // The symbol's CPs, else a black box's CPs of `Representation.box` (the derivation's rule). A direction of a
        // newer format stays unknown.
        for cp in ew_derive::rep_drawn_cps(rep, def) {
            let Some(at) = rep
                .orient
                .checked_apply(cp.local)
                .and_then(|p| rep.pos.checked_add(p))
            else {
                continue;
            };
            let dir = cp.dir.unwrap_or(Dir4::Unknown(serde_json::Value::Null));
            map.entry((cp.function.as_uuid(), cp.idx)).or_insert((
                grid.settle_point(at),
                dir,
                rep.id.as_uuid(),
            ));
        }
        // A strip segment: one point per drawn terminal and side.
        if !rv.terminals.is_empty() {
            let seg_grid = def.grid;
            let bottom = seg_grid.snap(symbol::lower(def, Orient2::R0).bounds.max().y);
            for (k, t) in rv.terminals.iter().enumerate() {
                let Ok(step) = i64::try_from(k + 1) else { break };
                let points = (
                    ew_derive::segment_point(seg_grid, rep, step, 0),
                    ew_derive::segment_point(seg_grid, rep, step, bottom),
                );
                let (Some(side1), Some(side2)) = points else { break };
                for (idx, at, dir) in t
                    .side1
                    .iter()
                    .map(|&i| (i, side1, Dir4::Up))
                    .chain(t.side2.iter().map(|&i| (i, side2, Dir4::Down)))
                {
                    let dir = symbol::transform_dir(&dir, rep.orient);
                    map.entry((t.function.as_uuid(), idx)).or_insert((
                        grid.settle_point(at),
                        dir,
                        rep.id.as_uuid(),
                    ));
                }
            }
        }
    }
    map
}

/// The page point of the tag text `rep` prints with `def`, by the scene's rules (the labels
/// layer's): the first tag slot, visible by the representation's override, else by its named arrangement, else by the
/// slot's default; the override's position, else the slot's place (`symbol::slot_places`), in the
/// representation's text frame (`ew_derive::text_frame`: on a half-turned black box, as on the box unturned). With it
/// the em height (nm) of the slot's text style in the project's table `styles` (the terminal style for the tag of a
/// symbol that draws a terminal, `terminal`, `ew_derive::terminal_symbol`).
pub fn tag_anchor(
    rep: &Representation,
    def: &SymbolDef,
    terminal: bool,
    styles: &TextStyleTable,
) -> Option<(Pt2, i64)> {
    let (i, slot) = def
        .text_slots
        .iter()
        .enumerate()
        .find(|(_, s)| matches!(s.kind, ew_schema::SlotKind::Tag))?;
    let index = u16::try_from(i).ok()?;
    let ov = rep.text_slots.iter().find(|o| o.slot == index);
    let visible = ew_derive::slot_shown(rep, def, index);
    let (style, _) = symbol::slot_style_key(terminal, slot, styles);
    let em = styles.resolve(style).map_or(0, |t| t.size.0.max(0));
    let frame = ew_derive::text_frame(rep, def);
    let at = match ov.and_then(|o| o.pos) {
        Some(pos) => frame.place(pos)?,
        None if ov.and_then(|o| o.angle).is_some() => frame.place(slot.pos)?,
        // Where the symbol puts it: on a symbol turned a quarter, placed anew round its body.
        None => frame.origin.checked_add(
            symbol::slot_places(def, frame.orient, |n| n == i)
                .swap_remove(i)
                .0,
        )?,
    };
    visible.then_some((at, em))
}

pub(crate) fn polyline_of(view: &PageView, cps: &CpMap, c: &Connection, page: Rect) -> Option<Vec<Pt2>> {
    let junction_at = |j: Id<Junction>| {
        view.junctions
            .iter()
            .find(|jv| jv.junction.id == j)
            .map(|jv| jv.junction.at)
    };
    let end = |e: &Endpoint| match e {
        Endpoint::Cp(f, idx) => cps.get(&(f.as_uuid(), *idx)).map(|(pos, dir)| RouteEnd {
            pos: *pos,
            dir: Some(dir.clone()),
        }),
        Endpoint::Junction(j) => junction_at(*j).map(|pos| RouteEnd { pos, dir: None }),
        Endpoint::Unknown(_) => None,
    };
    let (a, b) = (end(&c.a)?, end(&c.b)?);
    Some(connection_polyline(&a, &b, &c.route, page))
}

/// A drawn function CP: its function, its index, its point and the direction a wire leaves it.
pub(crate) type DrawnCp = (Id<Function>, u16, Pt2, Dir4);

/// The function CPs each representation draws, by representation ID. A CP that several representations draw is the
/// first one's by ID, as [`cp_map`] has it.
pub(crate) fn cp_owners(view: &PageView) -> BTreeMap<Uuid, Vec<DrawnCp>> {
    let mut out: BTreeMap<Uuid, Vec<DrawnCp>> = BTreeMap::new();
    for ((function, idx), (pos, dir, rep)) in page_drawn_cps(view) {
        out.entry(rep)
            .or_default()
            .push((Id::from_uuid(function), idx, pos, dir));
    }
    out
}
