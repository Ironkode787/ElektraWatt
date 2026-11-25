// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The units of the text placer (docs/design/06a-drafting.md §5.2): the blocks of the block rule and every other
//! label alone, and the rules that change their places before the choice: the stacked marks and the two rows.

use std::collections::BTreeMap;

use crate::consts::{LABEL_CLEAR, LINE_GAP};
use crate::geom::{self};
use crate::model::{LabelId, LabelKind, Owner, PartId, Places, Pt, Rect, Side, Tick};

use super::places::mark_name_beyond;
use super::{Ground, Unit};

/// The units of the page: the blocks of the block rule and every other label alone (§5.2).
///
/// **The block rule.** The texts of one part that join a block ([`LabelKind::blocks`]: every one but a pin's number)
/// and stand on one side of its body (left of its middle, right of it, or across it) and one under the other are one block: it is placed as one
/// label of their joined box, with the places of its first member's kind, and its members keep their places in it. A
/// member that overlaps another at its first place, or stands beside the block, is placed on its own.
pub(super) fn units(ground: &Ground<'_>) -> Vec<Unit> {
    let page = ground.page;
    let mut order: Vec<usize> = (0..page.labels.len()).collect();
    order.sort_by_key(|&i| page.labels[i].id);
    let origin = |owner: Owner| match owner {
        Owner::Part(p) => ground.parts.get(&p).map(|p| p.at),
        Owner::Mark(m) => page.marks.iter().find(|x| x.id == m).map(|x| x.line.top_left()),
        Owner::Wire(_) => Some(Pt::default()),
    };
    let alone = |i: usize| {
        let l = &page.labels[i];
        let (given, along) = match &l.places {
            Places::At(at) => (
                origin(l.owner).map_or_else(Vec::new, |o| {
                    at.iter().map(|p| Rect::sized(o.plus(*p), l.size)).collect()
                }),
                false,
            ),
            Places::AlongWire => (Vec::new(), true),
        };
        Unit {
            members: vec![(i, Pt::default())],
            size: l.size,
            kind: l.kind,
            owner: l.owner,
            given,
            along,
            stagger: false,
        }
    };
    // The block candidates by part and side: their first places on the page.
    // Per part and side: (priority, top, ID, index, first place) of each text there.
    type Side = (u8, Tick, LabelId, usize, Rect);
    let mut sides: BTreeMap<(PartId, i8), Vec<Side>> = BTreeMap::new();
    let mut out: Vec<Unit> = Vec::new();
    for &i in &order {
        let l = &page.labels[i];
        let first = match (&l.places, l.owner) {
            (Places::At(at), Owner::Part(p)) if l.kind.blocks() => at.first().and_then(|a| {
                let part = ground.parts.get(&p)?;
                Some((p, Rect::sized(part.at.plus(*a), l.size), part.bounds()))
            }),
            _ => None,
        };
        match first {
            Some((p, r, body)) => {
                let cx = body.centre().x;
                let side = if r.x1 <= cx {
                    -1
                } else if r.x0 >= cx {
                    1
                } else {
                    0
                };
                sides
                    .entry((p, side))
                    .or_default()
                    .push((l.kind.priority(), r.y0, l.id, i, r));
            }
            None => out.push(alone(i)),
        }
    }
    for (_, mut members) in sides {
        members.sort();
        let mut block: Vec<(usize, Rect)> = Vec::new();
        let mut rest: Vec<usize> = Vec::new();
        for (_, _, _, i, r) in members {
            let joins = match block.iter().map(|(_, b)| *b).reduce(Rect::union) {
                None => true,
                // Line under line: beside none of the block, under or over it with at most a quarter step between.
                Some(u) => {
                    let line = LINE_GAP;
                    !block.iter().any(|(_, b)| geom::texts_meet(*b, r))
                        && r.x0 < u.x1
                        && u.x0 < r.x1
                        && r.y0 <= u.y1 + line
                        && u.y0 <= r.y1 + line
                }
            };
            if joins {
                block.push((i, r));
            } else {
                rest.push(i);
            }
        }
        if block.len() == 1 {
            out.push(alone(block[0].0));
        } else if let Some(union) = block.iter().map(|(_, b)| *b).reduce(Rect::union) {
            let lead = &page.labels[block[0].0];
            out.push(Unit {
                members: block
                    .iter()
                    .map(|(i, b)| (*i, b.top_left().minus(union.top_left())))
                    .collect(),
                size: union.size(),
                kind: lead.kind,
                owner: lead.owner,
                given: vec![union],
                along: false,
                stagger: false,
            });
        }
        out.extend(rest.into_iter().map(alone));
    }
    out
}

/// **The stacked-marks rule.** The name of a mark (a part of one pin that leaves sideways) whose texts, where their
/// author put them, reach another mark or its texts, or leave the drawing area, has "in line with the wire beyond the
/// arrow" as its first place: marks stacked two steps apart read as a list, each name beside its arrow. A mark that
/// stands alone, and a name its author set in line already, keep the author's place. `first_places` gives the placer these first places, so that
/// it sets a stack by the texts as they will stand.
/// A name set in line already is one the wire's line runs through.
pub(super) fn stacked_marks(ground: &Ground<'_>, units: &mut [Unit]) {
    // Each mark with a name: its part, the texts' first places and its body.
    let mut marks: BTreeMap<PartId, (Vec<Rect>, Rect)> = BTreeMap::new();
    for u in units.iter() {
        let Owner::Part(p) = u.owner else { continue };
        let Some(part) = ground.parts.get(&p) else {
            continue;
        };
        let level = matches!(
            part.pins.as_slice(),
            [pin] if matches!(pin.side, Some(Side::Left | Side::Right))
        );
        if level
            && matches!(u.kind, LabelKind::MarkName | LabelKind::MarkXref)
            && let Some(first) = u.given.first()
        {
            marks
                .entry(p)
                .or_insert_with(|| (Vec::new(), part.bounds()))
                .0
                .push(*first);
        }
    }
    let crowded = |p: PartId| {
        let Some((mine, _)) = marks.get(&p) else {
            return false;
        };
        mine.iter().any(|r| {
            let g = r.grown(LABEL_CLEAR);
            !ground.inside(*r)
                || marks.iter().any(|(o, (texts, body))| {
                    *o != p && (body.overlaps(g) || texts.iter().any(|t| t.overlaps(g)))
                })
        })
    };
    for u in units.iter_mut() {
        let Owner::Part(p) = u.owner else { continue };
        if u.kind != LabelKind::MarkName || u.given.is_empty() || !crowded(p) {
            continue;
        }
        let Some(part) = ground.parts.get(&p) else {
            continue;
        };
        let b = part.bounds();
        let Some((q, side)) = part.pins.first().map(|pin| (part.at.plus(pin.at), pin.side)) else {
            continue;
        };
        if u.given.first().is_some_and(|r| r.y0 < q.y && q.y < r.y1) {
            continue;
        }
        let in_line = match side {
            Some(Side::Right | Side::Left) => mark_name_beyond(b, (q, side), u.size),
            _ => None,
        };
        let Some(in_line) = in_line else {
            continue;
        };
        u.given.retain(|r| *r != in_line);
        u.given.insert(0, in_line);
    }
}

/// **The two-rows rule.** A tag (of a symbol, a drawn box or a terminal) or a contact list whose first place reaches
/// within [`LABEL_CLEAR`] of the first place of a label of its kind level with it, on another part, has "one line
/// lower" (its height and [`LABEL_CLEAR`] down) as its second place: the first of a row keeps its place, the second
/// steps down, the third finds its place free again: two rows.
pub(super) fn stagger(units: &mut [Unit]) {
    let firsts: Vec<Option<(LabelKind, Owner, Rect)>> = units
        .iter()
        .map(|u| {
            u.given
                .first()
                .filter(|_| u.kind.two_rows())
                .map(|r| (u.kind, u.owner, *r))
        })
        .collect();
    for (i, unit) in units.iter_mut().enumerate() {
        let Some((kind, owner, r)) = firsts[i] else {
            continue;
        };
        let reach = r.grown(LABEL_CLEAR);
        unit.stagger = firsts.iter().enumerate().any(|(j, o)| {
            j != i
                && o.is_some_and(|(k, w, s)| {
                    k == kind && w != owner && s.y0 == r.y0 && s.h() == r.h() && s.overlaps(reach)
                })
        });
    }
}
