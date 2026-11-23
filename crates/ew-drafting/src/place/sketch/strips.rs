// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip over a card that opens as one and the card that steps along its edge (docs/design/06a-drafting.md
//! §4.4), and the order of the rows of terminals that no plan breaks.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use super::{CardSteps, Held, Kind, Lead, Sketch};
use crate::consts::{ROW_GAP, STUB, TICKS_PER_STEP};
use crate::model::{LabelKind, Owner, PartId, Pt, Rect, Role, Side, Tick, grid_away, grid_down};

impl<'a> Sketch<'a> {
    /// The strips that open over or under a card (doc 06a §4.4, FT-2110), each with its units in the order they stand
    /// and its card: a strip is the units joined through mates side by side, three or more and none fixed, of which two
    /// or more hang in line on pins of one card that face up or down (the unit most of them hang on so).
    fn opening(&self) -> Vec<(Vec<usize>, usize)> {
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for u in 0..self.units.len() {
            groups.entry(self.strips[0][u]).or_default().push(u);
        }
        let mut out = Vec::new();
        for mut members in groups.into_values() {
            if members.len() < 3 || members.iter().any(|u| self.units[*u].fixed) {
                continue;
            }
            let mut count: BTreeMap<usize, usize> = BTreeMap::new();
            for &u in &members {
                if let Some(l) = self.in_column(u) {
                    *count.entry(l.host).or_default() += 1;
                }
            }
            let Some((card, k)) = count.into_iter().max_by_key(|(h, k)| (*k, Reverse(*h))) else {
                continue;
            };
            if k < 2 {
                continue;
            }
            members.sort_by_key(|u| (self.shape(*u).map_or(0, |r| r.x0), *u));
            out.push((members, card));
        }
        out
    }

    /// The lead of a unit that hangs in line on a pin facing up or down: a channel's terminal under or over its card.
    fn in_column(&self, u: usize) -> Option<Lead> {
        self.lead[u].filter(|l| {
            l.inline
                && matches!(l.kind, Kind::Facing | Kind::Tap)
                && self.side(l.h).is_some_and(Side::vertical)
        })
    }

    /// `f` and what hangs on it in line (facing it or tapping it; with `corners` round a corner too), but the units of
    /// `stop` and what hangs on them: what moves with it. What hangs beside it (a lamp wired out and back to a terminal)
    /// stands where it is.
    fn in_line(&self, f: usize, stop: &[usize], corners: bool) -> Vec<usize> {
        let mut out = vec![f];
        let mut k = 0;
        while k < out.len() {
            let h = out[k];
            out.extend((0..self.units.len()).filter(|u| {
                !stop.contains(u)
                    && self.lead[*u].is_some_and(|l| {
                        l.host == h
                            && (matches!(l.kind, Kind::Facing | Kind::Tap | Kind::Mate)
                                || (corners && l.kind == Kind::Corner))
                    })
            }));
            k += 1;
        }
        out
    }

    /// The units an opening strip sets anew ([`Sketch::open_strips`]): its terminals that are no channel's. While the
    /// rows are set they stand in no member's way and bind nobody.
    pub(super) fn loose(&self) -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        for (members, card) in self.opening() {
            out.extend(
                members
                    .iter()
                    .copied()
                    .filter(|u| self.in_column(*u).is_none_or(|l| l.host != card)),
            );
        }
        out
    }

    /// Each strip over or under a card opens as one, in its own order (doc 06a §4.4, FT-2110), by
    /// [`Sketch::open_strip`]; where that cannot hold, the strip stays as the rows set it.
    ///
    /// **A strip opens as one** (FT-2110). A strip of terminals over or under a card, two or more of them in line on the
    /// card's pins, is set anew where the arrangement opens it ([`Sketch::open`]; the judge tries it where the strip's
    /// other terminals held a channel's terminal off its column, or the rows set a row of terminals out of its order):
    /// its other terminals bind nobody while the rows are set, and none of them is set beside or round a corner from its
    /// host. Then [`Sketch::open_strip`] sets the whole strip in the order it stands in: the longest run of channels'
    /// terminals in their channels' order with room between takes its columns, every other terminal keeps its place in
    /// the order. A strip that opened goes or stays as one ([`Sketch::with_strips`]), and no row of terminals leaves its
    /// order in any plan ([`Sketch::order_kept`]).
    pub(super) fn open_strips(&mut self) {
        self.opened.clear();
        self.held.clear();
        for (members, card) in self.opening() {
            let start = self.d.clone();
            match self.open_strip(&members, card) {
                Ok(()) => self.opened.push(members),
                Err(why) => {
                    self.d = start;
                    self.held.push((members[0], why));
                }
            }
        }
    }

    /// The strip of `members` (in the order they stand) under or over `card` set anew, in that order (doc 06a §4.4): of
    /// its channels' terminals the longest run that stands in the order of its channels' columns, with room between
    /// each two for the terminals between them (their bodies a row's gap apart), takes its columns (of runs as long,
    /// the first);
    /// every other terminal keeps its place in the order: between two of the run evenly, before the first or after the
    /// last at the strip's pitch, beyond what hangs on them in line. No terminal passes another. All level with the first terminal of the run; what hangs
    /// on a terminal in line moves with it (round a corner too on a channel's terminal). What holds it where no two channels' terminals can take their columns
    /// (their channels in another order, or too little room between their columns), or where a unit that moves would
    /// stand on another or outside the drawing area.
    fn open_strip(&mut self, members: &[usize], card: usize) -> Result<(), Held> {
        let left = |sk: &Self, u: usize| sk.shape(u).map_or(0, |r| r.x0);
        let Some(pitch) = members
            .windows(2)
            .map(|w| left(self, w[1]) - left(self, w[0]))
            .filter(|g| *g > 0)
            .min()
        else {
            return Err(Held::Room);
        };
        // Where each channel's terminal would stand: its pin on its channel's line.
        let column: Vec<Option<Tick>> = members
            .iter()
            .map(|&u| {
                let l = self.in_column(u).filter(|l| l.host == card)?;
                Some(left(self, u) + self.pt(l.h).x - self.pt0(l.f).x)
            })
            .collect();
        let chans: Vec<usize> = (0..members.len()).filter(|i| column[*i].is_some()).collect();
        let col = |i: usize| column[i].unwrap_or(0);
        let steps = |n: usize| Tick::try_from(n).unwrap_or(Tick::MAX / 2);
        // The least pitch between two terminals: their bodies a row's gap apart (two steps for a terminal), or the
        // strip's own where that is less.
        let width = members
            .iter()
            .filter_map(|u| self.shape(*u))
            .map(|r| r.w())
            .max()
            .unwrap_or(0);
        let least = pitch.min(grid_away(width + ROW_GAP));
        // The longest run in order (`room`: with room between each two), each as its last member's best length and
        // the member before it.
        let run = |room: bool| -> Vec<usize> {
            let mut best: Vec<(usize, Option<usize>)> = vec![(1, None); chans.len()];
            for j in 0..chans.len() {
                for i in 0..j {
                    let (a, b) = (chans[i], chans[j]);
                    let fits = col(b) - col(a) >= if room { steps(b - a) * least } else { 1 };
                    if fits && best[i].0 + 1 > best[j].0 {
                        best[j] = (best[i].0 + 1, Some(i));
                    }
                }
            }
            let Some(mut k) = (0..chans.len()).max_by_key(|k| (best[*k].0, Reverse(*k))) else {
                return Vec::new();
            };
            let mut out = vec![chans[k]];
            while let Some(i) = best[k].1 {
                out.push(chans[i]);
                k = i;
            }
            out.reverse();
            out
        };
        let anchors = run(true);
        if anchors.len() < 2 {
            return Err(if run(false).len() < 2 {
                Held::Order
            } else {
                Held::Room
            });
        }
        let (first, last) = (anchors[0], anchors[anchors.len() - 1]);
        // The terminals outside the run stand beyond it at the strip's pitch, and beyond what hangs on the run's outer
        // two in line (a button over the first with its tag at its left), so that their wires to the parts over the run
        // pass clear of those parts and their texts.
        let reach = |i: usize| -> Option<Rect> {
            self.in_line(members[i], members, true)
                .into_iter()
                .skip(1)
                .map(|v| self.hull(v))
                .reduce(Rect::union)
        };
        let near = [
            col(first) - reach(first).map_or(0, |r| grid_away((col(first) - r.x0).max(0))),
            col(last) + reach(last).map_or(0, |r| grid_away((r.x1 - col(last) - width).max(0))),
        ];
        let mut place: Vec<Tick> = (0..members.len())
            .map(|i| {
                if i < first {
                    near[0] - pitch * steps(first - i)
                } else if i > last {
                    near[1] + pitch * steps(i - last)
                } else {
                    col(i)
                }
            })
            .collect();
        for w in anchors.windows(2) {
            let (a, b) = (w[0], w[1]);
            let gaps = steps(b - a);
            for (i, at) in place.iter_mut().enumerate().take(b).skip(a + 1) {
                let even = (col(b) - col(a)) * steps(i - a) / gaps;
                *at = col(a) + grid_down(even);
            }
        }
        if !place.windows(2).all(|w| w[0] + least <= w[1]) {
            return Err(Held::Room);
        }
        // Level with the first terminal of the run, what hangs on each in line with it.
        let line = self.shape(members[first]).map_or(0, |r| r.y0) + self.d[members[first]].y;
        let mut moving: BTreeSet<usize> = members.iter().copied().collect();
        for (i, &u) in members.iter().enumerate() {
            let Some(r) = self.shape(u) else {
                return Err(Held::Room);
            };
            let by = Pt::new(place[i] - r.x0, line - r.y0).minus(self.d[u]);
            // (What hangs round a corner on a terminal that is no channel's, a mark at the sheet's edge, stays.)
            let hung: Vec<usize> = self
                .in_line(u, members, column[i].is_some())
                .into_iter()
                .skip(1)
                .collect();
            self.shift(&[u], by);
            self.shift(&hung, by);
            moving.extend(hung);
        }
        // In the drawing area, on no other unit.
        for u in &moving {
            if !self.inside(*u) {
                return Err(Held::Sheet);
            }
            if let Some(o) = (0..self.units.len()).find(|o| !moving.contains(o) && self.bodies_meet(*u, *o)) {
                return Err(Held::Unit(o));
            }
        }
        Ok(())
    }

    /// Whether a card of the page would take a step along its edge ([`Sketch::card_step`]).
    pub fn cards_step(&self) -> bool {
        (0..self.units.len()).any(|u| self.card_step(u, CardSteps::Columns) != Pt::default())
    }

    /// The step a card takes along its edge as the root of its tree (§4.4), so that its channels keep clear of its
    /// own supply and of another card's channels. Its channel pins face up or down and its supply pins stand on its
    /// left side. (a) Every pin its supply is wired to that lies on the side its channels face (the controller's
    /// supply pin, a mark, the supply pin of a card at the other edge) stands left of the card, where the wire drops
    /// beside the first channel and crosses none. (b) Where the columns of its channels in use interleave with those of
    /// a card whose pins face its own and it has fewer in use, its used columns stand right of the other's, two steps
    /// clear, and a pitch of the strip more for each spare terminal of a strip between them ([`Sketch::spares_between`],
    /// [`CardSteps::Columns`] only). No step where the card would leave the drawing area.
    pub(super) fn card_step(&self, u: usize, steps: CardSteps) -> Pt {
        if steps == CardSteps::None {
            return Pt::default();
        }
        let card = self.units[u]
            .parts
            .iter()
            .map(|i| self.w.parts[*i])
            .find(|p| p.role == Role::Card);
        let Some(card) = card else {
            return Pt::default();
        };
        let facing = |s: Side| card.pins.iter().filter(|p| p.side == Some(s)).count();
        let s = if facing(Side::Up) >= facing(Side::Down) {
            Side::Up
        } else {
            Side::Down
        };
        if self.units[u].fixed || facing(s) < 2 || facing(s.opposite()) > 0 {
            return Pt::default();
        }
        let body = card.bounds();
        let ahead = |p: Pt| {
            if s == Side::Up {
                p.y < body.y0
            } else {
                p.y > body.y1
            }
        };
        let mut left = body.x0;
        for pin in card.pins.iter().filter(|p| p.side == Some(Side::Left)) {
            for net in self.w.nets.iter().filter(|n| n.pins.contains(&pin.id)) {
                for q in &net.pins {
                    let (i, at, side) = self.w.pins[q];
                    if self.unit_of[i] == u || !ahead(at) {
                        continue;
                    }
                    let way = match side {
                        Some(Side::Left) => at.x - STUB,
                        Some(Side::Right) => at.x + STUB,
                        _ => at.x,
                    };
                    left = left.max(way + TICKS_PER_STEP);
                }
            }
        }
        let mut by = grid_away(left - body.x0);
        // The columns of a card's channels in use on `side`.
        let used = |part: &crate::model::Part, side: Side| -> Vec<Tick> {
            let mut xs: Vec<Tick> = part
                .pins
                .iter()
                .filter(|p| p.side == Some(side) && self.wired.contains(&p.id))
                .map(|p| part.at.x + p.at.x)
                .collect();
            xs.sort_unstable();
            xs
        };
        let own = used(card, s);
        let other = self
            .w
            .parts
            .iter()
            .filter(|p| p.role == Role::Card && p.id != card.id && ahead(p.at))
            .map(|p| (p, used(p, s.opposite())))
            .find(|(_, xs)| !xs.is_empty());
        if let (Some((part, xs)), CardSteps::Columns, Some(&lo), Some(&hi)) =
            (other, steps, own.first(), own.last())
        {
            // Where the used columns of the two interleave, the card with fewer channels in use (of two as many, the
            // one further right) steps on until its own stand right of the other's, two steps clear: the parts of the
            // two cards' channels do not stand among each other, and a strip between them keeps its order.
            let (them_lo, them_hi) = (xs[0], xs[xs.len() - 1]);
            let fewer = (own.len(), Reverse(card.at.x), Reverse(card.id))
                < (xs.len(), Reverse(part.at.x), Reverse(part.id));
            if fewer && lo + by <= them_hi && them_lo <= hi + by {
                let spares = self.spares_between((*part, them_hi), (card, lo));
                by = by.max(grid_away(them_hi + 2 * TICKS_PER_STEP + spares - lo));
            }
        }
        let room = self.w.page.area.x1 - TICKS_PER_STEP - body.x1;
        if by > room {
            return Pt::default();
        }
        Pt::new(by, 0)
    }

    /// The room the spare terminals of a strip between two cards ask for when their columns step apart
    /// ([`Sketch::card_step`], FT-2097): where the terminal wired to the pin of card `a` at column `xa` and the one
    /// wired to the pin of card `b` at column `xb` are mates of one row (a strip), each mate between them that is wired
    /// to nothing (a spare, which keeps its place in the strip) takes the row's pitch as it stood; none otherwise.
    fn spares_between(
        &self,
        (a, xa): (&crate::model::Part, Tick),
        (b, xb): (&crate::model::Part, Tick),
    ) -> Tick {
        let terminal = |card: &crate::model::Part, x: Tick| -> Option<usize> {
            let own = self.unit_of_id(card.id)?;
            let pin = card
                .pins
                .iter()
                .find(|p| card.at.x + p.at.x == x && self.wired.contains(&p.id))?;
            let net = self.w.nets.iter().find(|n| n.pins.contains(&pin.id))?;
            net.pins.iter().map(|q| self.unit_of_pin(*q)).find(|v| *v != own)
        };
        let centre = |u: usize| self.shape(u).map(|r| (r.x0 + r.x1) / 2);
        let (Some(ta), Some(tb)) = (terminal(a, xa), terminal(b, xb)) else {
            return 0;
        };
        let (Some(ca), Some(cb)) = (centre(ta), centre(tb)) else {
            return 0;
        };
        if ta == tb || !self.mates(ta, tb, true) {
            return 0;
        }
        let (lo, hi) = (ca.min(cb), ca.max(cb));
        let mut row = vec![ca, cb];
        let mut spares = 0;
        for m in (0..self.units.len()).filter(|m| *m != ta && *m != tb && self.mates(ta, *m, true)) {
            let Some(c) = centre(m).filter(|c| lo < *c && *c < hi) else {
                continue;
            };
            row.push(c);
            let wired = self.units[m]
                .parts
                .iter()
                .any(|i| self.w.parts[*i].pins.iter().any(|p| self.wired.contains(&p.id)));
            spares += Tick::from(!wired);
        }
        row.sort_unstable();
        let pitch = row
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|g| *g > 0)
            .min()
            .unwrap_or(0);
        spares * pitch
    }

    /// The strips opened as one, each by its first terminal.
    pub fn opened(&self) -> Vec<PartId> {
        self.opened
            .iter()
            .filter_map(|s| s.first().map(|u| self.largest(*u)))
            .collect()
    }

    /// `units` with every unit of a strip opened as one that one of them belongs to, and what hangs on those: a strip
    /// that opened goes or stays as one, never in part.
    pub fn with_strips(&self, units: Vec<usize>) -> Vec<usize> {
        let mut out: BTreeSet<usize> = units.into_iter().collect();
        for strip in &self.opened {
            if strip.iter().any(|u| out.contains(u)) {
                out.extend(strip.iter().flat_map(|u| self.subtree(*u)));
            }
        }
        out.into_iter().collect()
    }

    /// The rows of terminals (the units joined through mates side by side, or one under another, whose largest part
    /// carries a terminal's name), two or more, each with whether it stands side by side and its units in the order
    /// they stand.
    pub(super) fn terminal_rows(&self) -> Vec<(bool, Vec<usize>)> {
        let named: BTreeSet<PartId> = self
            .w
            .page
            .labels
            .iter()
            .filter(|l| l.kind == LabelKind::TerminalName)
            .filter_map(|l| match l.owner {
                Owner::Part(p) => Some(p),
                _ => None,
            })
            .collect();
        let mut out = Vec::new();
        for (k, down) in [(0, true), (1, false)] {
            let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for u in (0..self.units.len()).filter(|u| named.contains(&self.largest(*u))) {
                groups.entry(self.strips[k][u]).or_default().push(u);
            }
            for mut row in groups.into_values().filter(|r| r.len() > 1) {
                row.sort_by_key(|u| (self.shape(*u).map_or(0, |r| if down { r.x0 } else { r.y0 }), *u));
                out.push((down, row));
            }
        }
        out
    }

    /// Whether every row of terminals keeps the order it stands in with the moves as they are (doc 06a §4.4): no
    /// terminal passes another, in any step. Two that stood at one place along the row have no order to keep.
    pub fn order_kept(&self) -> bool {
        self.terminal_rows.iter().all(|(down, row)| {
            let along = |u: usize, d: Pt| {
                self.shape(u)
                    .map_or(0, |r| if *down { r.x0 + d.x } else { r.y0 + d.y })
            };
            row.iter().enumerate().all(|(i, &a)| {
                row[i + 1..].iter().all(|&b| {
                    let (was, now) = (
                        along(b, Pt::default()) - along(a, Pt::default()),
                        along(b, self.d[b]) - along(a, self.d[a]),
                    );
                    was == 0 || now.signum() == was.signum()
                })
            })
        })
    }
}
