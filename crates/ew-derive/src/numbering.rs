// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wire numbers: `wire_numbers()` (doc 08 §5.2, doc 20 §8.1, FR-NUM-019, FR-NUM-028).
//!
//! # Rule
//!
//! The golden scheme is connection-oriented `{page}.{counter}` with the counter scope page: each physical wire
//! gets its own number, the counters run 1, 2, … per page. Cable cores are identified by cable and core and
//! are never numbered (FR-NUM-019); bridges are not wires. The wires are taken in the **numbering order**
//! (FR-NUM-028):
//!
//! 1. the page of the wire's From end (the page order of the drawn pages),
//! 2. the From end's frame cell, column then row,
//! 3. the To end's frame cell, column then row,
//! 4. the wire ID.
//!
//! The From end is the end [`orient_wire`] puts first (doc 08 §3.8). The cell of an end is the cell of the CP's
//! drawn point on the page of the constituent connection that ends there ([`cp_cell`]); an end that no
//! representation draws on that page has no cell and sorts after every cell. Where the two cells cannot
//! separate two wires the wire ID decides; [`WireNumber::tie`] then gives the counter range of the group, so
//! that a table can print "101.2–.3 (tie)" as doc 20 §8.2 does.
//!
//! The query orders **every** physical wire (cores included, so that the wire list can use the order for its
//! rows); only the numbered ones carry a [`WireNumber`]. A wire none of whose constituents is drawn on a page
//! is placed last and not numbered.
//!
//! Numbers are derived, never stored in the model at this stage (the stored `NumberSpec` of doc 08 §5.1 and the
//! `NumberWires` command follow with the numbering schemes of FR-NUM-020); a drawing edit that moves a CP into
//! another cell can renumber, which is why the query is memoised over what it reads.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Connection, Page};

use crate::cp_point::cp_cell;
use crate::pages::DrawnPageOrder;
use crate::query::{Policy, Query};
use crate::wires::{PhysicalWire, PhysicalWiresOf, WireEnd};
use crate::{PageRef, PotentialPartition, Q, RepAnchorCell};

/// The number of one physical wire.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WireNumber {
    /// The page of the From end: the `{page}` token, printed as its folio label.
    pub page: Id<Page>,
    /// The counter on that page, from 1.
    pub counter: u32,
    /// The printed number, `{page}.{counter}` (`102.5`).
    pub label: String,
    /// The counters (first, last) of the group of wires whose From and To cells are the same, when the group
    /// has more than one wire: the wire ID decided among them (doc 20 §8.1 "tie").
    pub tie: Option<(u32, u32)>,
}

/// One physical wire in the numbering order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NumberedWire {
    /// The wire ID (its carrier connection).
    pub wire: Id<Connection>,
    /// Its number; `None` for a cable core and for a wire that is drawn on no page.
    pub number: Option<WireNumber>,
}

/// The value of [`WireNumbers`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WireNumbersV {
    /// Every physical wire of the project in the numbering order.
    pub wires: Vec<NumberedWire>,
    index: BTreeMap<Id<Connection>, usize>,
}

impl WireNumbersV {
    /// The entry of a wire.
    pub fn get(&self, wire: Id<Connection>) -> Option<&NumberedWire> {
        self.index.get(&wire).map(|i| &self.wires[*i])
    }

    /// The number of a wire.
    pub fn number(&self, wire: Id<Connection>) -> Option<&WireNumber> {
        self.get(wire).and_then(|w| w.number.as_ref())
    }

    /// The position of a wire in the numbering order (0-based).
    pub fn position(&self, wire: Id<Connection>) -> Option<usize> {
        self.index.get(&wire).copied()
    }
}

/// `wire_numbers()`: the connection-oriented numbers of the physical wires (see the module documentation).
/// Initial policy Memo.
#[derive(Debug)]
pub struct WireNumbers;

impl Query for WireNumbers {
    type Key = ();
    type Value = WireNumbersV;
    const FAMILY: &'static str = "wire_numbers";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<WireNumbersV> {
        Arc::new(number_wires(q))
    }
}

/// A frame cell as the sort key `(column, row)`; `(MAX, MAX)` when the end has none.
type CellKey = (u32, u32);

/// The sort key of a wire: page position, From cell, To cell, wire ID.
type Key = (usize, CellKey, CellKey, Id<Connection>);

struct Entry {
    key: Key,
    page: Option<Id<Page>>,
    core: bool,
}

fn number_wires(q: &Q) -> WireNumbersV {
    let partition = q.get::<PotentialPartition>(());
    let order = q.get::<DrawnPageOrder>(());
    let mut by_id: BTreeMap<Id<Connection>, Entry> = BTreeMap::new();
    for p in partition.potentials() {
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            by_id.entry(w.id).or_insert_with(|| entry(q, &order, w));
        }
    }
    let mut entries: Vec<(Id<Connection>, Entry)> = by_id.into_iter().collect();
    entries.sort_by_key(|a| a.1.key);

    // Counters per page over the numbered wires, then the tie groups: runs of equal (page, From, To) cells.
    let mut next: BTreeMap<Id<Page>, u32> = BTreeMap::new();
    let mut numbers: Vec<Option<(Id<Page>, u32)>> = Vec::with_capacity(entries.len());
    for (_, e) in &entries {
        numbers.push(match (e.page, e.core) {
            (Some(page), false) => {
                let c = next.entry(page).or_insert(0);
                *c += 1;
                Some((page, *c))
            }
            _ => None,
        });
    }
    let group_of = |i: usize| (entries[i].1.key.0, entries[i].1.key.1, entries[i].1.key.2);
    let mut ties: Vec<Option<(u32, u32)>> = vec![None; entries.len()];
    let mut i = 0;
    while i < entries.len() {
        let mut j = i + 1;
        while j < entries.len() && group_of(j) == group_of(i) {
            j += 1;
        }
        // The counters of the numbered wires of the run (cores sort inside it without a counter).
        let counters: Vec<u32> = numbers[i..j].iter().flatten().map(|(_, c)| *c).collect();
        if counters.len() > 1 {
            let range = (counters[0], counters[counters.len() - 1]);
            for t in &mut ties[i..j] {
                *t = Some(range);
            }
        }
        i = j;
    }

    let mut out = WireNumbersV::default();
    for (n, ((id, _), (number, tie))) in entries.iter().zip(numbers.into_iter().zip(ties)).enumerate() {
        let number = number.map(|(page, counter)| WireNumber {
            page,
            counter,
            label: format!("{}.{counter}", folio_of(&order, page)),
            tie,
        });
        out.index.insert(*id, n);
        out.wires.push(NumberedWire { wire: *id, number });
    }
    out
}

fn folio_of(order: &crate::pages::PageOrderV, page: Id<Page>) -> String {
    order
        .entry(PageRef::Drawn(page))
        .map(|e| e.folio.clone())
        .unwrap_or_default()
}

fn entry(q: &Q, order: &crate::pages::PageOrderV, wire: &PhysicalWire) -> Entry {
    let mut w = wire.clone();
    let swapped = orient_wire(q, &mut w);
    // The constituents run from the end they started at; the From end's constituent is the first one unless
    // the orientation swapped the ends.
    let (from_side, to_side) = if swapped {
        (w.constituents.last(), w.constituents.first())
    } else {
        (w.constituents.first(), w.constituents.last())
    };
    let page_of = |c: Option<&Id<Connection>>| c.and_then(|c| q.connections().get(*c)).and_then(|c| c.page);
    let (from_page, to_page) = (page_of(from_side), page_of(to_side));
    let cell = |end: &WireEnd, page: Option<Id<Page>>| -> CellKey {
        page.and_then(|p| cp_cell(q, end.cp.function, end.cp.idx, p))
            .map_or((u32::MAX, u32::MAX), |c| (c.column, c.row))
    };
    let pos = from_page
        .and_then(|p| order.position(PageRef::Drawn(p)))
        .unwrap_or(usize::MAX);
    Entry {
        key: (pos, cell(&w.ends[0], from_page), cell(&w.ends[1], to_page), w.id),
        page: from_page.filter(|_| pos != usize::MAX),
        core: w.core.is_some(),
    }
}

/// Puts the From end of a wire first (doc 08 §3.8 "Orientation (From/To)", FR-CON-040): on different pages
/// the end on the earlier page in the page order; otherwise the end with the smaller anchor cell (row, then
/// column), then the smaller CP ID. The page of an end is the page of the constituent that ends there.
/// Returns whether the ends were swapped.
///
/// The flow-distance step of the rule (hops over wires, bridges and terminal shorts from the points where
/// the potential enters) is not applied: it needs a per-CP flow distance that this crate does not expose
/// (FT-101). A wire that rule decides differently on one page has its ends the other way round.
pub fn orient_wire(q: &Q, w: &mut PhysicalWire) -> bool {
    let position = |page: Id<Page>| {
        q.get::<DrawnPageOrder>(())
            .position(PageRef::Drawn(page))
            .unwrap_or(usize::MAX)
    };
    let page_of = |c: Option<&Id<Connection>>| c.and_then(|c| q.connections().get(*c)).and_then(|c| c.page);
    let (pa, pb) = (page_of(w.constituents.first()), page_of(w.constituents.last()));
    let swap = match (pa, pb) {
        (Some(a), Some(b)) if a != b => position(b) < position(a),
        _ => {
            let cell = |e: &WireEnd, page: Option<Id<Page>>| -> (u32, u32) {
                let Some(page) = page else {
                    return (u32::MAX, u32::MAX);
                };
                let mut reps: Vec<_> = q
                    .idx()
                    .reps_of_function(e.cp.function)
                    .filter(|r| q.reps().get(*r).is_some_and(|r| r.page == page))
                    .collect();
                reps.sort();
                reps.first()
                    .and_then(|r| q.get::<RepAnchorCell>(*r).as_ref().clone())
                    .map_or((u32::MAX, u32::MAX), |c| (c.row, c.column))
            };
            let page = pa.or(pb);
            (cell(&w.ends[1], page), w.ends[1].cp) < (cell(&w.ends[0], page), w.ends[0].cp)
        }
    };
    if swap {
        w.ends.swap(0, 1);
    }
    swap
}
