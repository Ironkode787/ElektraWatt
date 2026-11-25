// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawn boxes of a corrected part follow it (doc 10 §2.6): `part.edit_copy` and `library.update` give
//! each box a device of the part draws the pins and sockets the part has now.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{
    BoxCp, BoxSpec, Connection, Device, Dir4, Endpoint, Function, LibCopy, Page, Part, Representation,
    SymbolDef,
};
use ew_units::{Nm, Orient2, Pt2};

use crate::commands::common::function_cp_indices;
use crate::commands::connection::follow_moved_cps;
use crate::error::CmdError;
use crate::tx::TxBuilder;

/// The boxes of the devices that use part copy `part`, after the part changed in this transaction: each black box
/// (a symbol without CPs drawn with a `box`, not a valve terminal's positions) of a function of such a device keeps
/// its CPs, the index of each taken through `remap` (`library.update`'s pairing of the old and the new pattern by
/// designation), drops a CP the function no longer has unless a connection on its page ends there (that line keeps its
/// end; the update's card names it), and takes a CP the function gains: after the last CP of the edge that holds the
/// CP of the highest index, one pitch on (the pitch of that edge's last two CPs, else one grid step), leaving as it
/// does, the box growing past the edge's far end where a new CP needs it, the CPs of the far edge and their wires with
/// it. A box turned, or not an axis-aligned rectangle, or without a CP left, takes no new CP: it is drawn again by
/// Redraw them or by deleting it and drawing it again, as before.
pub(crate) fn follow_part_boxes(
    tx: &mut TxBuilder<'_>,
    part: Id<Part>,
    remap: &BTreeMap<(Id<Function>, u16), u16>,
) -> Result<(), CmdError> {
    let devices: Vec<std::sync::Arc<Device>> = tx
        .iter::<Device>()
        .into_iter()
        .filter(|d| d.parts.iter().any(|u| u.part.copy == part))
        .collect();
    for dev in devices {
        for f in tx.functions_of_device(dev.id) {
            let Some(func) = tx.get::<Function>(f) else {
                continue;
            };
            let cps = function_cp_indices(tx, &func, &dev);
            for rid in tx.reps_of_function(f) {
                if let Some(rep) = tx.get::<Representation>(rid) {
                    follow_box(tx, &rep, f, &cps, remap)?;
                }
            }
        }
    }
    Ok(())
}

/// One box of [`follow_part_boxes`]: the box `rep` of function `f`, whose CPs are `cps` now.
fn follow_box(
    tx: &mut TxBuilder<'_>,
    rep: &Representation,
    f: Id<Function>,
    cps: &BTreeSet<u16>,
    remap: &BTreeMap<(Id<Function>, u16), u16>,
) -> Result<(), CmdError> {
    let Some(b) = rep.box_.as_ref().filter(|b| b.slots.is_none()) else {
        return Ok(());
    };
    let drawn_by_box = tx
        .get::<LibCopy<SymbolDef>>(rep.symbol)
        .is_some_and(|s| s.item.cps.is_empty());
    if rep.functions.first() != Some(&f) || !drawn_by_box {
        return Ok(());
    }
    let ends_here = |tx: &TxBuilder<'_>, idx: u16| {
        tx.connections_of_function(f).into_iter().any(|cid| {
            tx.get::<Connection>(cid).is_some_and(|c| {
                c.page == Some(rep.page)
                    && [&c.a, &c.b]
                        .into_iter()
                        .any(|e| matches!(e, Endpoint::Cp(g, i) if *g == f && *i == idx))
            })
        })
    };
    let mut kept: Vec<BoxCp> = b
        .cps
        .iter()
        .map(|c| BoxCp {
            idx: remap.get(&(f, c.idx)).copied().unwrap_or(c.idx),
            ..c.clone()
        })
        .filter(|c| cps.contains(&c.idx) || ends_here(tx, c.idx))
        .collect();
    let new: Vec<u16> = cps
        .iter()
        .copied()
        .filter(|i| !kept.iter().any(|c| c.idx == *i))
        .collect();
    if kept == b.cps && new.is_empty() {
        return Ok(());
    }
    let grid = tx.require::<Page>(rep.page)?.grid;
    let mut outline = b.outline.clone();
    let mut moves: BTreeMap<(Id<Function>, u16), Pt2> = BTreeMap::new();
    if rep.orient == Orient2::default()
        && let Some((placed, grown)) = place_new(grid, rep.pos, &outline, &kept, &new)
    {
        if let Some((far, to)) = grown {
            let shift = to.0 - far.0;
            let along_x = placed
                .first()
                .is_some_and(|c| matches!(c.dir, Dir4::Up | Dir4::Down));
            let at = |p: &mut Pt2| {
                let v = if along_x { &mut p.x } else { &mut p.y };
                if *v == far {
                    *v = to;
                    true
                } else {
                    false
                }
            };
            for v in &mut outline {
                at(v);
            }
            for c in &mut kept {
                if at(&mut c.pos) {
                    let d = if along_x {
                        Pt2::new(Nm(shift), Nm::ZERO)
                    } else {
                        Pt2::new(Nm::ZERO, Nm(shift))
                    };
                    moves.insert((f, c.idx), d);
                }
            }
        }
        kept.extend(placed);
    }
    if !moves.is_empty() {
        follow_moved_cps(tx, rep.page, grid, &moves, text!("refusal-move-what-update"))?;
    }
    let mut next = rep.clone();
    next.box_ = Some(BoxSpec {
        outline,
        cps: kept,
        ..b.clone()
    });
    tx.put(next);
    Ok(())
}

/// Where the new CPs `new` of the rectangle `outline` with the CPs `kept` (relative to `pos`, unturned) stand (see
/// [`follow_part_boxes`]), and, when the box must grow for them, its far edge's coordinate along the row's axis and the
/// one it moves to. `None` for an outline that is no axis-aligned rectangle, without a CP to continue from, without a
/// new CP, or off the grid's range.
#[allow(clippy::type_complexity)]
fn place_new(
    grid: ew_units::Grid,
    pos: Pt2,
    outline: &[Pt2],
    kept: &[BoxCp],
    new: &[u16],
) -> Option<(Vec<BoxCp>, Option<(Nm, Nm)>)> {
    let rectangle = outline.len() == 4
        && outline[0].y == outline[1].y
        && outline[1].x == outline[2].x
        && outline[2].y == outline[3].y
        && outline[3].x == outline[0].x;
    if !rectangle || new.is_empty() {
        return None;
    }
    let last = kept.iter().max_by_key(|c| c.idx)?;
    let along_x = matches!(last.dir, Dir4::Up | Dir4::Down);
    let coord = |p: Pt2| if along_x { p.x } else { p.y };
    let base = coord(pos);
    let k = |v: Nm| grid.snap(Nm(base.0 + v.0));
    let at = |k: i64| grid.at(k).map(|n| Nm(n.0 - base.0));
    let mut row: Vec<i64> = kept
        .iter()
        .filter(|c| c.dir == last.dir)
        .map(|c| k(coord(c.pos)))
        .collect();
    row.sort_unstable();
    let end = *row.last()?;
    let pitch = row
        .len()
        .checked_sub(2)
        .and_then(|i| row.get(i))
        .map_or(1, |before| (end - before).max(1));
    let far = outline.iter().map(|p| coord(*p)).max()?;
    let mut kk = end;
    let mut placed = Vec::with_capacity(new.len());
    for idx in new {
        kk += pitch;
        let c = at(kk)?;
        let p = if along_x {
            Pt2::new(c, last.pos.y)
        } else {
            Pt2::new(last.pos.x, c)
        };
        placed.push(BoxCp {
            idx: *idx,
            pos: p,
            dir: last.dir.clone(),
            name: None,
            extra: Default::default(),
        });
    }
    // The far edge one step past the last new CP, where the box ends sooner.
    let grown = if kk + 1 > k(far) {
        Some((far, at(kk + 1)?))
    } else {
        None
    };
    Some((placed, grown))
}
