// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rework list (doc 13 §3.5): what the fitter changes in a built cabinet between a release and now.
//!
//! [`wire_changes`] matches the wires of two states: a wire of now is the wire of the release with the same ID, and a
//! wire whose carrier changed (a junction edit) is the wire of the release that ends at the same two CPs, so it is not
//! pulled and added again. The same matching gives the wire numbers before and after a change that a preview lists
//! (doc 16 §7.6). [`rework`] reads the matched pairs as the fitter's work, one [`ReworkItem`] per thing he does, in the
//! order he does it ([`ReworkAction`]): a wire that only moved on a page or took another route, a new number, a change
//! of source that keeps the value and a renamed potential are no work and give no item.

use std::collections::BTreeMap;

use ew_schema::{ColourSpec, Csa, PartRef};

use crate::joins::cp_label;
use crate::nets::CpRef;
use crate::q::Q;
use crate::wire_record::{WireLength, WireRecord, WireRecordSet};
use crate::wires::EndKind;

/// One wire of two states: its record in the earlier state (`before`, the release) and in the later one (`after`,
/// now); one of them is `None` for a wire that is gone or new.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WirePair<'a> {
    /// The wire as it was.
    pub before: Option<&'a WireRecord>,
    /// The wire as it is.
    pub after: Option<&'a WireRecord>,
}

/// The wires of two states matched (see the module documentation): first the kept wires in the order of `after`, a
/// new wire among them where it stands; then the wires that are gone, in the order of `before`.
pub fn wire_changes<'a>(before: &'a [WireRecord], after: &'a [WireRecord]) -> Vec<WirePair<'a>> {
    let by_id: BTreeMap<_, usize> = before.iter().enumerate().map(|(i, r)| (r.wire.id, i)).collect();
    let mut taken = vec![false; before.len()];
    let mut matched: Vec<Option<usize>> = after
        .iter()
        .map(|r| {
            let i = *by_id.get(&r.wire.id)?;
            taken[i] = true;
            Some(i)
        })
        .collect();
    // A wire whose carrier changed keeps its two ends.
    let mut by_ends: BTreeMap<[CpRef; 2], Vec<usize>> = BTreeMap::new();
    for (i, r) in before.iter().enumerate().filter(|(i, _)| !taken[*i]) {
        by_ends.entry(ends_key(r)).or_default().push(i);
    }
    for (slot, r) in matched.iter_mut().zip(after) {
        if slot.is_none()
            && let Some(free) = by_ends.get_mut(&ends_key(r))
            && !free.is_empty()
        {
            let i = free.remove(0);
            taken[i] = true;
            *slot = Some(i);
        }
    }
    let mut out: Vec<WirePair<'a>> = matched
        .into_iter()
        .zip(after)
        .map(|(i, r)| WirePair {
            before: i.map(|i| &before[i]),
            after: Some(r),
        })
        .collect();
    out.extend(
        before
            .iter()
            .zip(&taken)
            .filter(|(_, t)| !**t)
            .map(|(r, _)| WirePair {
                before: Some(r),
                after: None,
            }),
    );
    out
}

/// A wire's two end CPs in order, the key of a wire matched by its ends.
fn ends_key(r: &WireRecord) -> [CpRef; 2] {
    let mut ends = [r.wire.ends[0].cp, r.wire.ends[1].cp];
    ends.sort();
    ends
}

/// What the fitter does, in the order he does it (doc 13 §3.5); a row with several changes takes the first that
/// applies.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ReworkAction {
    /// A wire of the release is gone.
    Pull,
    /// A kept wire changed its cross-section, colour or wire part.
    Replace,
    /// An end of a kept wire lands on another CP.
    MoveEnd,
    /// The length changed.
    CheckLength,
    /// A wire is new.
    Add,
}

/// One change of a kept wire, as the Change cell names it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReworkChange {
    /// The cross-section, before and after.
    Size(Option<Csa>, Option<Csa>),
    /// The colour, before and after.
    Colour(Option<ColourSpec>, Option<ColourSpec>),
    /// The wire part, by its part number before and after.
    WirePart(Option<String>, Option<String>),
    /// An end that lands on another CP: the CP before and after (`-104A1-A1:2`).
    End(String, String),
    /// The length, before and after.
    Length(WireLength, WireLength),
}

/// One row of the rework list: an action on one wire, the wire as the row prints it (as the release had it for a
/// pulled wire, as it is now for every other), and its changes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReworkItem {
    /// What the fitter does.
    pub action: ReworkAction,
    /// The wire as the row prints it.
    pub wire: WireRecord,
    /// What changed, in the order of [`ReworkAction`]; empty for a wire pulled or added.
    pub changes: Vec<ReworkChange>,
}

/// The rework list between the release `base` and `now` (doc 13 §3.5): the wires of [`wire_changes`] as the fitter's
/// work, by action and then in the numbering order of the state each row prints.
pub fn rework(base: &Q, now: &Q) -> Vec<ReworkItem> {
    let (before, after) = (base.get::<WireRecordSet>(()), now.get::<WireRecordSet>(()));
    let mut items: Vec<ReworkItem> = wire_changes(&before, &after)
        .into_iter()
        .filter_map(|pair| match (pair.before, pair.after) {
            (Some(b), None) => Some(ReworkItem {
                action: ReworkAction::Pull,
                wire: b.clone(),
                changes: Vec::new(),
            }),
            (None, Some(a)) => Some(ReworkItem {
                action: ReworkAction::Add,
                wire: a.clone(),
                changes: Vec::new(),
            }),
            (Some(b), Some(a)) => {
                let changes = changes(base, b, now, a);
                let action = changes.iter().map(action_of).min()?;
                Some(ReworkItem {
                    action,
                    wire: a.clone(),
                    changes,
                })
            }
            (None, None) => None,
        })
        .collect();
    // Stable: within an action the order of `wire_changes`, each state's numbering order.
    items.sort_by_key(|i| i.action);
    items
}

/// The action a change asks for.
fn action_of(change: &ReworkChange) -> ReworkAction {
    match change {
        ReworkChange::Size(..) | ReworkChange::Colour(..) | ReworkChange::WirePart(..) => {
            ReworkAction::Replace
        }
        ReworkChange::End(..) => ReworkAction::MoveEnd,
        ReworkChange::Length(..) => ReworkAction::CheckLength,
    }
}

/// What changed on a kept wire, `b` in the release and `a` now; the values, never their sources.
fn changes(base: &Q, b: &WireRecord, now: &Q, a: &WireRecord) -> Vec<ReworkChange> {
    let mut out = Vec::new();
    let size = |r: &WireRecord| r.csa.as_ref().map(|p| p.value.clone());
    let (size_b, size_a) = (size(b), size(a));
    if size_b != size_a {
        out.push(ReworkChange::Size(size_b, size_a));
    }
    let colour = |r: &WireRecord| r.colour.as_ref().map(|p| p.value.clone());
    let (colour_b, colour_a) = (colour(b), colour(a));
    if colour_b != colour_a {
        out.push(ReworkChange::Colour(colour_b, colour_a));
    }
    let (part_b, part_a) = (
        part_number(base, b.wire_part.as_ref()),
        part_number(now, a.wire_part.as_ref()),
    );
    if part_b != part_a {
        out.push(ReworkChange::WirePart(part_b, part_a));
    }
    // An end is compared with the release's end at the same place of the wire, From with From: orientation follows
    // the page order, so a moved end can turn a wire round; it is compared by whichever pairing keeps an end.
    let [b0, b1] = b.wire.ends.map(|e| e.cp);
    let [a0, a1] = a.wire.ends.map(|e| e.cp);
    let pairs = if a0 == b1 || a1 == b0 {
        [(b1, a0), (b0, a1)]
    } else {
        [(b0, a0), (b1, a1)]
    };
    for (end_b, end_a) in pairs {
        if end_b != end_a {
            out.push(ReworkChange::End(
                end_label(base, b, end_b),
                end_label(now, a, end_a),
            ));
        }
    }
    if b.length != a.length {
        out.push(ReworkChange::Length(b.length, a.length));
    }
    out
}

/// An end as the Change cell names it: its CP's label, `ext. <name>` for an external end.
fn end_label(q: &Q, r: &WireRecord, cp: CpRef) -> String {
    let external = r
        .wire
        .ends
        .iter()
        .any(|e| e.cp == cp && e.kind == EndKind::External);
    let label = cp_label(q, cp);
    if external { format!("ext. {label}") } else { label }
}

/// The part number of a wire part, `None` without one or for a part the project does not hold.
fn part_number(q: &Q, part: Option<&PartRef>) -> Option<String> {
    part.and_then(|p| q.parts().get(p.copy))
        .map(|c| c.item.part_number.as_str().to_owned())
}
