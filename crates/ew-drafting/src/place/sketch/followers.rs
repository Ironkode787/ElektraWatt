// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The followers of a rank (docs/design/06a-drafting.md §4.3, §4.4): a row with its packing, a stack of corner
//! followers with its pitch and the texts it is judged by, and the level followers beside their hosts.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use super::{Block, Kind, Lead, Sketch, across_vec, along_vec, out, rising, span_across, span_along};
use crate::consts::{
    BODY_GAP, CHAIN_GAP, CHAIN_GAP_MIN, HALF, MARK_LEAD, MARK_PITCHES, ROW_GAP, ROW_PULL, TICKS_PER_STEP,
    TRACK_PITCH, TRACK_PITCH_NARROW, TRIAL_STUB,
};
use crate::measure::measure;
use crate::model::{
    DraftPage, End, Groups, Junction, JunctionId, Owner, PageKind, PartId, PinId, Pt, Rect, Side, Tick, Wire,
    WireId, grid_away, grid_down,
};
use crate::route::plain_line;

impl<'a> Sketch<'a> {
    /// Sets each of `members` with what hangs on it, by itself: a block that the caller then sets as one.
    pub(super) fn blocks(&mut self, members: &[usize]) {
        for m in members {
            let units: BTreeSet<usize> = self.subtree(*m).into_iter().collect();
            let outer = self.within.replace(units.clone());
            self.placed[*m] = true;
            self.lay_from(*m);
            self.within = outer;
            for u in units {
                self.placed[u] = false;
            }
        }
    }

    /// The members of a row whose extents meet along side `s` packed across in their order with [`ROW_GAP`] between:
    /// by their bodies (`tight`) or their footprints. Of three ways to pack them (by the least total move from where
    /// they stand, their wishes; the first at its wish and the others beyond it; the last at its wish and the others
    /// before it) the one whose stepping wires cross the fewest of `risers` is taken: the wires that leave the hosts'
    /// rank on this side beside the row's own (a strip's supply terminal beside the terminals of a row of sensors).
    fn pack(&mut self, s: Side, items: &[(usize, Lead)], wished: &[Tick], tight: bool, risers: &[Tick]) {
        let extent = |sk: &Self, m: usize| {
            if tight {
                sk.bodies(m).into_iter().reduce(Rect::union).unwrap_or_default()
            } else {
                sk.hull(m)
            }
        };
        let n = items.len();
        let mut groups = Groups::new(n);
        let spans: Vec<(Tick, Tick)> = items
            .iter()
            .map(|(m, _)| span_along(extent(self, *m), s))
            .collect();
        for i in 0..n {
            for j in i + 1..n {
                if spans[i].0 < spans[j].1 + ROW_GAP && spans[j].0 < spans[i].1 + ROW_GAP {
                    groups.join(i, j);
                }
            }
        }
        let group = groups.roots();
        let mut names: Vec<usize> = group.clone();
        names.sort_unstable();
        names.dedup();
        for g in names {
            let of: Vec<usize> = (0..n).filter(|k| group[*k] == g).collect();
            let edges: Vec<(Tick, Tick)> = of
                .iter()
                .map(|k| span_across(extent(self, items[*k].0), s))
                .collect();
            let mut offset: Vec<Tick> = vec![0];
            for k in 1..of.len() {
                let need = grid_away(edges[k - 1].1 + ROW_GAP - edges[k].0);
                offset.push(offset[k - 1] + need);
            }
            let want: Vec<Tick> = offset.iter().map(|o| -o).collect();
            let mut middle: Vec<Tick> = rising(&want)
                .into_iter()
                .enumerate()
                .map(|(k, v)| v + offset[k])
                .collect();
            // (Stepped back into the drawing area as one where an end of it leaves it.)
            let (first, last) = span_across(self.w.page.area.grown(-HALF), s);
            let under = (0..of.len())
                .map(|k| first - (edges[k].0 + middle[k]))
                .max()
                .unwrap_or(0);
            let over = (0..of.len())
                .map(|k| edges[k].1 + middle[k] - last)
                .max()
                .unwrap_or(0);
            if under > 0 && over + grid_away(under) <= 0 {
                middle.iter_mut().for_each(|by| *by += grid_away(under));
            } else if over > 0 && under + grid_away(over) <= 0 {
                middle.iter_mut().for_each(|by| *by -= grid_away(over));
            }
            // Members that keep their places and their distances stay: none steps aside for a wire.
            if middle.iter().all(|by| *by == 0) && of.iter().all(|k| wished[*k] == 0) {
                continue;
            }
            // A member's bodies across, and whether they would stand in a riser's way (within a step of it) when the
            // member is stepped by `by`.
            let body: Vec<(Tick, Tick)> = of
                .iter()
                .map(|k| {
                    let all = self.bodies(items[*k].0).into_iter().reduce(Rect::union);
                    span_across(all.unwrap_or_default(), s)
                })
                .collect();
            let in_way = |k: usize, by: Tick| {
                risers
                    .iter()
                    .any(|r| body[k].0 + by - BODY_GAP < *r && *r < body[k].1 + by + BODY_GAP)
            };
            let reach = 2 * ROW_PULL;
            let mut forward: Vec<Tick> = vec![0; of.len()];
            for k in 0..of.len() {
                if k > 0 {
                    forward[k] = (forward[k - 1] + offset[k] - offset[k - 1]).max(0);
                }
                let free = (0..=reach / TICKS_PER_STEP)
                    .map(|n| forward[k] + n * TICKS_PER_STEP)
                    .find(|by| !in_way(k, *by));
                forward[k] = free.unwrap_or(forward[k]);
            }
            let mut backward: Vec<Tick> = vec![0; of.len()];
            for k in (0..of.len()).rev() {
                if k + 1 < of.len() {
                    backward[k] = (backward[k + 1] - (offset[k + 1] - offset[k])).min(0);
                }
                let free = (0..=reach / TICKS_PER_STEP)
                    .map(|n| backward[k] - n * TICKS_PER_STEP)
                    .find(|by| !in_way(k, *by));
                backward[k] = free.unwrap_or(backward[k]);
            }
            // The risers each way of packing would have its stepping wires cross, or its members stand in the way of.
            let crossed = |sk: &Self, by: &[Tick]| -> usize {
                of.iter()
                    .zip(by)
                    .enumerate()
                    .map(|(n, (k, b))| {
                        let l = items[*k].1;
                        let (h, f) = (s.across(sk.pt(l.h)), s.across(sk.pt(l.f)) + b);
                        risers.iter().filter(|r| h.min(f) < **r && **r < h.max(f)).count()
                            + usize::from(in_way(n, *b))
                    })
                    .sum()
            };
            // (A way that sets a member outside the drawing area comes last.)
            let outside = |by: &[Tick]| -> usize {
                (0..of.len())
                    .filter(|k| edges[*k].0 + by[*k] < first || edges[*k].1 + by[*k] > last)
                    .count()
            };
            let ways = [middle, forward, backward];
            let best = (0..ways.len())
                .min_by_key(|w| (outside(&ways[*w]), crossed(self, &ways[*w]), *w))
                .unwrap_or(0);
            for (k, by) in ways[best].iter().enumerate() {
                if *by != 0 {
                    let m = items[of[k]].0;
                    self.d[m] = self.d[m].plus(across_vec(s, *by));
                }
            }
        }
    }

    /// Whether the straight way between two points keeps clear of every unit but `but`: no body within a step of it.
    fn line_free(&self, a: Pt, b: Pt, but: &[usize]) -> bool {
        let line = Rect::new(a, b).grown(1);
        !(0..self.units.len()).any(|o| {
            !but.contains(&o)
                && self
                    .bodies(o)
                    .iter()
                    .any(|body| body.grown(BODY_GAP - 2).overlaps(line))
        })
    }

    /// A row of followers on side `s` of their hosts, which stand in the rank `hosts` (§4.4).
    ///
    /// **A row** (facing and tap leads on one side of a rank). The members that stand more than [`ROW_PULL`] beyond the
    /// nearest wait until what hangs on the near ones is set: a device wired straight to a card does not take the place
    /// of the row over the terminals.
    /// - *Across*, a member wants its pin on its host pin's line. It takes the line where the straight way between the
    ///   two pins is free of other parts, and where that does not carry it past a member that stays, nor past a unit
    ///   of its row that has no line of its own to take or further from such a unit than it stood by more than
    ///   [`ROW_PULL`]. A unit of the row with a line of its own binds nobody: the terminals of one strip that are wired
    ///   to two cards each stand at their own card's pin.
    /// - *Along*, a member keeps its place when it stands at least [`CHAIN_GAP_MIN`] beyond its host pin and clear of its
    ///   host (one its host left more than a row's pull behind keeps the distance it had; a keeper one step off the
    ///   distance most keepers of its shape keep takes that). Else it takes the row's line: [`CHAIN_GAP`], a step more
    ///   than the tracks its stepping wires need, or the line the keepers stand on within [`ROW_PULL`] of that. A member
    ///   a rail taps takes the rail's row where its lead leaves it the room. Where a member that takes the row's line
    ///   would stand on a unit that is set, those members step back together to the first line that is clear, no
    ///   nearer than [`CHAIN_GAP_MIN`] and their tracks (sensors under a box at the head).
    /// - *Packed*: the members whose extents meet along the side stand in the order of their host pins with [`ROW_GAP`]
    ///   between: by their bodies where the text placer then sets every text of the row clear and within a step of
    ///   its part (a strip's terminals two steps apart), else by their footprints (bodies and texts at their first
    ///   places). Of
    ///   three ways to pack them (by the least total move from their wishes: neighbours that would overlap are pooled
    ///   at their median; from the first member onward; from the last member back) the one is taken whose stepping wires
    ///   cross the fewest of the wires that pass the row (those that leave the rank on that side and do not end at a
    ///   pin of the row that faces the rank: a supply's riser beside a row of sensors), and none that sets a member
    ///   outside the drawing area while another does not. Members that keep their places and distances stay.
    /// - A member that then stands on a unit already set steps further out on its line where the line is free that far
    ///   (two parts wider than their pins' pitch stand staggered, both in line); else it stands where it stood across,
    ///   and steps further out from there.
    pub(super) fn row(&mut self, hosts: &[usize], s: Side, members: &[usize]) {
        let mut items: Vec<(usize, Lead)> = members
            .iter()
            .filter_map(|m| self.lead[*m].map(|l| (*m, l)))
            .collect();
        items.sort_by_key(|(m, l)| (s.across(self.pt(l.h)), s.across(self.pt(l.f)), self.id(*m)));
        // The wires that leave the rank on this side and do not end at a pin of the row that faces the rank (a second
        // pole of a member is the row's own; a supply that passes the row to its far pins is not), but the rank's own
        // leads.
        let mut rank: Vec<usize> = hosts
            .iter()
            .copied()
            .chain(items.iter().map(|(_, l)| l.host))
            .collect();
        rank.sort_unstable();
        rank.dedup();
        let mine: BTreeSet<usize> = items.iter().flat_map(|(m, _)| self.subtree(*m)).collect();
        let own: BTreeSet<PinId> = self
            .w
            .nets
            .iter()
            .filter(|n| {
                n.pins
                    .iter()
                    .any(|p| mine.contains(&self.unit_of_pin(*p)) && self.side(*p) == Some(s.opposite()))
            })
            .flat_map(|n| n.pins.iter().copied())
            .chain(rank.iter().filter_map(|u| self.lead[*u].map(|l| l.f)))
            .collect();
        let risers: Vec<Tick> = rank
            .iter()
            .flat_map(|u| self.units[*u].parts.iter())
            .flat_map(|i| self.w.parts[*i].pins.iter())
            .filter(|pin| pin.side == Some(s) && self.wired.contains(&pin.id) && !own.contains(&pin.id))
            // (Of the units that stand in the hosts' line.)
            .filter(|pin| {
                items
                    .iter()
                    .any(|(_, l)| s.beyond(self.pt(l.h), self.pt(pin.id)).abs() <= ROW_PULL)
            })
            .map(|pin| s.across(self.pt(pin.id)))
            .collect();
        // Across: in line, where the straight way between the two pins is then free (a part of another circuit on it
        // leaves the member where it stands).
        let mut wished: Vec<Tick> = Vec::with_capacity(items.len());
        for (m, l) in &items {
            let by = if l.inline {
                s.across(self.pt(l.h)) - s.across(self.pt(l.f))
            } else {
                0
            };
            self.d[*m] = self.d[*m].plus(across_vec(s, by));
            wished.push(by);
        }
        // The units of the members' strips that are not of the row, with what hangs on them, and the terminals an
        // opening strip sets anew ([`Sketch::loose`]): where the strip opens (`open`) they are set with it
        // afterwards ([`Sketch::open_strips`]), so they stand in no member's way and bind nobody.
        let strip = &self.strips[usize::from(!s.vertical())];
        let here: BTreeSet<usize> = items.iter().map(|(m, _)| *m).collect();
        let mut others: Vec<usize> = (0..self.units.len())
            .filter(|u| !here.contains(u) && items.iter().any(|(m, _)| strip[*u] == strip[*m]))
            .flat_map(|u| self.subtree(u))
            .collect();
        others.extend(self.loose.iter().copied());
        for (k, (m, l)) in items.iter().enumerate() {
            let far = self.pt(l.f).plus(along_vec(
                s,
                (CHAIN_GAP - s.beyond(self.pt(l.h), self.pt(l.f))).max(0),
            ));
            // (The row's other members are packed clear of it afterwards.)
            let mut own = self.subtree(*m);
            own.push(l.host);
            own.extend(items.iter().flat_map(|(o, _)| self.subtree(*o)));
            if wished[k] != 0 && !self.line_free(self.pt(l.h), far, &own) {
                own.extend(&others);
                let strip_only = self.line_free(self.pt(l.h), far, &own);
                if self.open && strip_only {
                    continue;
                }
                self.bound |= strip_only;
                self.d[*m] = self.d[*m].plus(across_vec(s, -wished[k]));
                wished[k] = 0;
            }
        }
        // A member whose line would carry it past one that stays, where the two stood in the order of their pins, stays
        // too: it is not drawn up against the one that could not go. Nor does a member take a line that carries it
        // past a unit of its row that has no line of its own to take (it is fixed, or hangs beside its host), or
        // further from it than it stood by more than the row's pull: such a row is not torn. A unit of the row with a
        // line of its own (a strip's terminal that faces another card) binds nobody: each stands in its channel's
        // column (rule E4). Where the strip opens as one (`open`), none binds: the strip is set anew afterwards.
        let elsewhere = |sk: &Self, m: usize| -> Vec<usize> {
            (0..sk.units.len())
                .filter(|u| {
                    *u != m
                        && strip[*u] == strip[m]
                        && !here.contains(u)
                        && sk.lead[*u]
                            .is_none_or(|l| !matches!(l.kind, Kind::Mate | Kind::Facing | Kind::Tap))
                })
                .collect()
        };
        let place = |sk: &Self, u: usize, moved: bool| {
            let at = sk.shape(u).map_or(0, |r| span_across(r, s).0);
            if moved { at + s.across(sk.d[u]) } else { at }
        };
        let binds = |sk: &Self, wished: &[Tick], k: usize| {
            let m = items[k].0;
            wished[k] != 0
                && elsewhere(sk, m).into_iter().any(|u| {
                    let (then, now) = (
                        place(sk, m, false) - place(sk, u, false),
                        place(sk, m, true) - place(sk, u, true),
                    );
                    then.signum() != now.signum() || now.abs() > then.abs() + ROW_PULL
                })
        };
        if !self.open && (0..items.len()).any(|k| binds(self, &wished, k)) {
            self.bound = true;
        }
        loop {
            let at = |sk: &Self, k: usize| s.across(sk.pt(items[k].1.f));
            let was = |sk: &Self, k: usize| s.across(sk.pt0(items[k].1.f));
            let past = (0..items.len()).find(|k| {
                wished[*k] != 0
                    && ((0..items.len()).any(|j| {
                        wished[j] == 0
                            && (*k < j) == (was(self, *k) < was(self, j))
                            && was(self, *k) != was(self, j)
                            && (*k < j) != (at(self, *k) < at(self, j))
                    }) || (!self.open && binds(self, &wished, *k)))
            });
            let Some(k) = past else { break };
            self.d[items[k].0] = self.d[items[k].0].plus(across_vec(s, -wished[k]));
            wished[k] = 0;
        }
        // Along: a keeper keeps its place; one that stood too near, or on the wrong side, takes the chain gap for now;
        // one its host left far behind keeps the distance it had.
        let dist = |sk: &Self, l: &Lead| s.beyond(sk.pt(l.h), sk.pt(l.f));
        let mut movers: Vec<bool> = Vec::with_capacity(items.len());
        for (m, l) in &items {
            let was = s.beyond(self.pt0(l.h), self.pt0(l.f));
            let now = dist(self, l);
            let (least, most) = (was.clamp(CHAIN_GAP_MIN, CHAIN_GAP), was.max(CHAIN_GAP) + ROW_PULL);
            let keeps = was >= CHAIN_GAP_MIN && now >= least && !self.meets(*m, l.host);
            if keeps && now > most {
                self.d[*m] = self.d[*m].plus(along_vec(s, was - now));
            } else if !keeps {
                self.d[*m] = self.d[*m].plus(along_vec(s, CHAIN_GAP - now));
            }
            movers.push(!keeps);
        }
        // A keeper a step off the distance that most keepers of its shape keep takes it: a row set down a step crooked
        // is levelled.
        let mut kept: BTreeMap<(Tick, Tick), BTreeMap<Tick, usize>> = BTreeMap::new();
        for (k, (m, l)) in items.iter().enumerate() {
            if let (false, Some(shape)) = (movers[k], self.shape(*m)) {
                *kept
                    .entry(shape.size())
                    .or_default()
                    .entry(dist(self, l))
                    .or_default() += 1;
            }
        }
        for (k, (m, l)) in items.iter().enumerate() {
            let Some(counts) = self.shape(*m).and_then(|x| kept.get(&x.size())) else {
                continue;
            };
            let now = dist(self, l);
            if let (false, Some((most, n))) = (movers[k], counts.iter().max_by_key(|(d, n)| (**n, **d)))
                && *n >= 2
                && (*most - now).abs() == TICKS_PER_STEP
            {
                self.d[*m] = self.d[*m].plus(along_vec(s, *most - now));
            }
        }
        // Across: packed by their bodies where the text placer then finds every text of the row a clear place (a
        // strip's terminals two steps apart with their names); else by their footprints, texts at their first places.
        let start = self.d.clone();
        self.pack(s, &items, &wished, true, &risers);
        // (Asked once for each way the row stands to its hosts.)
        let key: (Side, Vec<(usize, Pt)>) = (
            s,
            items
                .iter()
                .map(|(m, l)| (*m, self.d[*m].minus(self.d[l.host])))
                .collect(),
        );
        let tight = match self.tight.get(&key) {
            Some(t) => *t,
            None => {
                let mut all: Vec<usize> = items.iter().flat_map(|(m, l)| [*m, l.host]).collect();
                all.sort_unstable();
                all.dedup();
                let wires: Vec<(PinId, PinId)> = items.iter().map(|(_, l)| (l.h, l.f)).collect();
                // No text more on a text, and none more away from its part, than each unit has by itself (a mark set
                // on a pin has its texts on that pin's part): texts that lay on a neighbour's as the row stood do not
                // count for it, a row whose texts clash is spread.
                let now = self.text_clashes(&all, &wires);
                let own = all.iter().fold((0, 0), |(a, b), u| {
                    let (c, d) = self.text_clashes(&[*u], &[]);
                    (a + c, b + d)
                });
                let t = { now.0 <= own.0 && now.1 <= own.1 };
                self.tight.insert(key, t);
                t
            }
        };
        if !tight {
            self.d = start;
            self.pack(s, &items, &wished, false, &risers);
        }
        // Along: the row's line, with room for the tracks of its stepping wires.
        let lanes: Vec<(Tick, Tick)> = items
            .iter()
            .filter(|(_, l)| l.inline)
            .map(|(_, l)| (s.across(self.pt(l.h)), s.across(self.pt(l.f))))
            .collect();
        let tracks = crate::tracks::tracks_needed(&lanes) as Tick;
        let room = tracks
            .saturating_mul(TRACK_PITCH)
            .saturating_add(TICKS_PER_STEP)
            .max(CHAIN_GAP);
        let narrow = tracks
            .saturating_mul(TRACK_PITCH_NARROW)
            .saturating_add(CHAIN_GAP_MIN);
        for (k, (_, l)) in items.iter().enumerate() {
            let steps = l.inline && s.across(self.pt(l.h)) != s.across(self.pt(l.f));
            if !movers[k] && steps && dist(self, l) < narrow {
                movers[k] = true;
            }
        }
        let line = items
            .iter()
            .enumerate()
            .filter(|(k, _)| !movers[*k])
            .map(|(_, (_, l))| dist(self, l))
            .filter(|d| (room..=room + ROW_PULL).contains(d))
            .max()
            .unwrap_or(room);
        for (k, (m, l)) in items.iter().enumerate() {
            if movers[k] {
                // (A chain opens by a row's pull at most, rule A5: its tracks then take the narrow pitch.)
                let was = s.beyond(self.pt0(l.h), self.pt0(l.f));
                let most = if was >= CHAIN_GAP_MIN {
                    was.max(CHAIN_GAP) + ROW_PULL
                } else {
                    line
                };
                let by = grid_away(line.min(most) - dist(self, l));
                self.d[*m] = self.d[*m].plus(along_vec(s, by));
            }
        }
        // A member a rail taps joins the rail's row where its lead leaves it the room.
        for (m, l) in &items {
            let Some((step, must)) = self.steps.get(m).copied() else {
                continue;
            };
            if s.vertical() != (step.y != 0) {
                continue;
            }
            let by = s.beyond(Pt::default(), step) - s.beyond(Pt::default(), self.d[*m]);
            if by != 0 && dist(self, l) + by >= CHAIN_GAP_MIN {
                self.d[*m] = self.d[*m].plus(along_vec(s, by));
            } else if by != 0 && must {
                self.failed = true;
            }
        }
        // A member whose place in line is taken stands where it stood across, when that is free; one that moved onto a
        // unit already set steps further out.
        let reach = self.w.page.area.w().max(self.w.page.area.h());
        // (A row packed by its bodies stands by its bodies: its texts found their places.)
        let on = |sk: &Self, m: usize| {
            if tight {
                (0..sk.units.len()).any(|o| {
                    o != m
                        && sk.placed[o]
                        && sk.within.as_ref().is_none_or(|w| w.contains(&o))
                        && sk.bodies_meet(m, o)
                })
            } else {
                sk.blocked(m, &[])
            }
        };
        // A row whose line is taken stands nearer: the members that take the row's line step back together, a step at a
        // time and no nearer than their tracks leave them, to the first line on which none of them stands on a unit
        // that is set (a row of sensors under a box at the head).
        let taking: Vec<usize> = (0..items.len()).filter(|k| movers[*k]).collect();
        if taking.iter().any(|k| on(self, items[*k].0)) {
            let start = self.d.clone();
            let nearer = loop {
                for k in &taking {
                    let m = items[*k].0;
                    self.d[m] = self.d[m].plus(along_vec(s, -TICKS_PER_STEP));
                }
                if taking.iter().any(|k| dist(self, &items[*k].1) < narrow) {
                    break false;
                }
                if !taking.iter().any(|k| on(self, items[*k].0)) {
                    break true;
                }
            };
            if !nearer {
                self.d = start;
            }
        }
        for (k, (m, l)) in items.iter().enumerate() {
            if wished[k] != 0 && on(self, *m) {
                // Further out on its line where the line is free that far (two flow controls wider than their ports'
                // pitch stand staggered); else where it stood across.
                let start = self.d[*m];
                let mut more = 0;
                while on(self, *m) && more < 2 * ROW_PULL {
                    self.d[*m] = self.d[*m].plus(along_vec(s, TICKS_PER_STEP));
                    more += TICKS_PER_STEP;
                }
                let mut own = self.subtree(*m);
                own.push(l.host);
                if on(self, *m) || !self.line_free(self.pt(l.h), self.pt(l.f), &own) {
                    self.d[*m] = start.plus(across_vec(s, -wished[k]));
                }
            }
            let mut more = 0;
            while self.d[*m] != Pt::default() && on(self, *m) && more < reach {
                self.d[*m] = self.d[*m].plus(along_vec(s, TICKS_PER_STEP));
                more += TICKS_PER_STEP;
            }
            if more >= reach {
                // No room: back to where it stood.
                self.d[*m] = Pt::default();
                self.failed = true;
            }
            self.placed[*m] = true;
        }
    }

    /// The wires of a block towards the rank it hangs on, on side `s` of the rank, its own pins facing `sf`: its lead
    /// and the relations left out of the tree, each as (host pin, own pin).
    fn entries(
        &self,
        hosts: &[usize],
        s: Side,
        sf: Side,
        units: &[usize],
        lead: Lead,
    ) -> Vec<(PinId, PinId)> {
        let mut out = vec![(lead.h, lead.f)];
        for r in &self.extra {
            if r.kind != Kind::Corner {
                continue;
            }
            for (h, f) in [(r.pa, r.pb), (r.pb, r.pa)] {
                if hosts.contains(&self.unit_of_pin(h))
                    && units.contains(&self.unit_of_pin(f))
                    && self.side(h) == Some(s)
                    && self.side(f) == Some(sf)
                {
                    out.push((h, f));
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// A stack of corner followers on side `s` of `hosts`, their own pins facing `sf` (§4.4).
    ///
    /// **A stack** (corner leads on one side of a rank that turn one way): each member is set first with what hangs on
    /// it, as one block. A block heavier than the unit it hangs on keeps its place (a card with its channels does not
    /// step for the corner of one supply wire). The blocks are taken in nesting order (the drop nearest the side they stand on turns first) and
    /// each keeps its place when every one of its wires has its corner (the drop at least [`CHAIN_GAP_MIN`] long, its own
    /// pin at least [`MARK_LEAD`] before the drop), it lies beyond the blocks before it and stands clear; else it takes
    /// the nearest such place: in the column most single marks of the stack stand in, or in the column a mark's lead
    /// before the nearest drop. The pitch is [`MARK_PITCH`](crate::consts::MARK_PITCH), or the next larger one at which the text placer finds
    /// every text of the stack a place as clear as it had. A stack that leaves the drawing area slides back towards its
    /// hosts.
    pub(super) fn stack(&mut self, hosts: &[usize], s: Side, sf: Side, members: &[usize]) {
        let way = sf.opposite();
        let mut blocks: Vec<Block> = members
            .iter()
            .filter_map(|m| {
                let units = self.subtree(*m);
                let entries = self.entries(hosts, s, sf, &units, self.lead[*m]?);
                Some((units, entries))
            })
            .collect();
        // Innermost first: the drop nearest the side the blocks stand on.
        let inner = |sk: &Self, e: &[(PinId, PinId)]| e.iter().map(|(h, _)| out(sk.pt(*h), way)).max();
        blocks.sort_by_key(|(units, e)| (Reverse(inner(self, e)), self.id(units[0])));
        let Some(key) = blocks.first().map(|(units, _)| (units[0], s, sf)) else {
            return;
        };
        let start = self.d.clone();
        let pitches: Vec<Tick> = match self.pitch.get(&key) {
            Some(p) => vec![*p],
            None => MARK_PITCHES.to_vec(),
        };
        let mut best: Option<(usize, Tick, Vec<Pt>)> = None;
        for pitch in pitches {
            self.d.clone_from(&start);
            self.stacked(s, way, &blocks, pitch);
            if self.pitch.contains_key(&key) {
                best = Some((0, pitch, self.d.clone()));
                break;
            }
            let mut all: Vec<usize> = blocks.iter().flat_map(|(u, _)| u.iter().copied()).collect();
            all.extend(hosts.iter().copied());
            let wires: Vec<(PinId, PinId)> = blocks.iter().flat_map(|(_, e)| e.iter().copied()).collect();
            let clashes = self.text_clashes(&all, &wires).0;
            if best.as_ref().is_none_or(|(c, _, _)| clashes < *c) {
                best = Some((clashes, pitch, self.d.clone()));
            }
            // As clear as the texts stood before (a mark set on a pin has its texts on that part wherever it stands).
            let now = std::mem::replace(&mut self.d, start.clone());
            let before = self.clashes_before(&all).0;
            self.d = now;
            if clashes <= before {
                break;
            }
        }
        if let Some((_, pitch, d)) = best {
            self.d = d;
            self.pitch.insert(key, pitch);
        }
        for (units, _) in &blocks {
            for u in units {
                self.placed[*u] = true;
            }
        }
    }

    /// Whether a body of `u` stands within [`ROW_GAP`] of a body of `o`.
    pub(super) fn bodies_meet(&self, u: usize, o: usize) -> bool {
        let (a, b) = (self.bodies(u), self.bodies(o));
        u != o
            && a.iter()
                .any(|x| b.iter().any(|y| x.grown(ROW_GAP - 2).overlaps(y.grown(1))))
    }

    /// One pass over the blocks of a stack at `pitch`: each keeps its place or takes the nearest good one. The blocks
    /// keep their bodies clear of each other and of what is set; their texts are the text placer's (the caller asks it).
    fn stacked(&mut self, s: Side, way: Side, blocks: &[Block], pitch: Tick) {
        let reach = self.w.page.area.w().max(self.w.page.area.h());
        // A block takes its place at `floor` or beyond; one that stands at `least` or beyond keeps its own.
        let (mut floor, mut least) = (CHAIN_GAP_MIN, TICKS_PER_STEP);
        let mut set: Vec<usize> = Vec::new();
        let short = |sk: &Self, entries: &[(PinId, PinId)]| {
            entries
                .iter()
                .map(|(h, f)| MARK_LEAD - way.beyond(sk.pt(*h), sk.pt(*f)))
                .max()
                .unwrap_or(0)
        };
        // The near column: the mark's lead before the drop nearest the side the blocks stand on.
        let near = blocks
            .iter()
            .flat_map(|(_, e)| e.iter().map(|(h, _)| out(self.pt(*h), way)))
            .max()
            .map(|x| x + MARK_LEAD);
        // The column the person made: where most of the blocks of one unit stand that have their lead (the one
        // furthest out of several).
        let mut stand: BTreeMap<Tick, usize> = BTreeMap::new();
        for (units, entries) in blocks {
            if let ([_], [(_, f)]) = (units.as_slice(), entries.as_slice())
                && short(self, entries) <= 0
            {
                *stand.entry(out(self.pt(*f), way)).or_default() += 1;
            }
        }
        let made = stand
            .iter()
            .filter(|(_, k)| **k >= 2)
            .max_by_key(|(c, k)| (**k, **c))
            .map(|(c, _)| *c);
        for (units, entries) in blocks {
            let on = |sk: &Self, set: &[usize]| {
                units.iter().any(|u| {
                    (0..sk.units.len()).any(|o| {
                        if units.contains(&o) {
                            false
                        } else if set.contains(&o) {
                            // Blocks of one stack stand as near as their pitch sets them: their bodies only must
                            // not lie on each other.
                            sk.bodies(*u)
                                .iter()
                                .any(|a| sk.bodies(o).iter().any(|b| a.grown(-1).overlaps(*b)))
                        } else {
                            sk.placed[o]
                                && sk.within.as_ref().is_none_or(|w| w.contains(&o))
                                && sk.bodies_meet(*u, o)
                        }
                    })
                })
            };
            // How far the block is from a column.
            let to_column = |sk: &Self, column: Option<Tick>| {
                entries
                    .iter()
                    .map(|(_, f)| column.unwrap_or(0) - out(sk.pt(*f), way))
                    .max()
                    .unwrap_or(0)
            };
            // Across: every pin of its own at least the mark's lead before its drop. A block that has to move, and a
            // block of one unit within the row's pull of it, takes the column the person made; without one, a block
            // that has to move takes the near column where that is within the pull of its own lead.
            let lacks = short(self, entries);
            let single = units.len() == 1 && entries.len() == 1;
            let to_made = to_column(self, made);
            if made.is_some() && to_made >= lacks && (lacks > 0 || (single && to_made.abs() <= ROW_PULL)) {
                self.shift(units, along_vec(way, grid_away(to_made)));
            } else if lacks > 0 {
                let to = to_column(self, near);
                let by = if to >= lacks && to - lacks <= ROW_PULL {
                    to
                } else {
                    lacks
                };
                self.shift(units, along_vec(way, grid_away(by)));
            }
            // Along: beyond the blocks before it.
            let level = |sk: &Self| {
                entries
                    .iter()
                    .map(|(h, f)| s.beyond(sk.pt(*h), sk.pt(*f)))
                    .min()
                    .unwrap_or(0)
            };
            if level(self) < least {
                let by = grid_away(floor - level(self));
                self.shift(units, along_vec(s, by));
            }
            // Its place taken: the near column, where that is free; else further out.
            if on(self, &set) {
                let to = grid_away(to_column(self, near));
                if to != 0 {
                    self.shift(units, along_vec(way, to));
                    let lead = entries
                        .iter()
                        .map(|(h, f)| way.beyond(self.pt(*h), self.pt(*f)))
                        .min()
                        .unwrap_or(0);
                    if on(self, &set) || lead < MARK_LEAD {
                        self.shift(units, along_vec(way, -to));
                    }
                }
            }
            let mut more = 0;
            while on(self, &set) && more < reach {
                self.shift(units, along_vec(s, TICKS_PER_STEP));
                more += TICKS_PER_STEP;
            }
            if more >= reach {
                self.failed = true;
            }
            let far = entries
                .iter()
                .map(|(h, f)| s.beyond(self.pt(*h), self.pt(*f)))
                .max()
                .unwrap_or(floor);
            // The next block's wire runs beyond this block's bodies too.
            let base = entries.first().map_or(0, |(h, _)| out(self.pt(*h), s));
            let bodies = units
                .iter()
                .flat_map(|u| self.bodies(*u))
                .map(|b| span_along(b, s).1 - base + BODY_GAP)
                .max()
                .unwrap_or(0);
            floor = (far + pitch).max(grid_away(bodies));
            least = floor;
            set.extend(units.iter().copied());
        }
        // A stack that leaves the drawing area slides back towards its hosts as far as its first block has room.
        let edge = span_along(self.w.page.area.grown(-HALF), s).1;
        let over = set
            .iter()
            .flat_map(|u| self.bodies(*u))
            .map(|b| span_along(b, s).1 - edge)
            .max()
            .unwrap_or(0);
        let slack = blocks
            .first()
            .and_then(|(_, e)| e.iter().map(|(h, f)| s.beyond(self.pt(*h), self.pt(*f))).min())
            .map_or(0, |l| l - CHAIN_GAP_MIN);
        let slide = grid_away(over).min(grid_down(slack));
        if over > 0 && slide > 0 {
            self.shift(&set, along_vec(s, -slide));
            let on = set.iter().any(|u| {
                (0..self.units.len()).any(|o| {
                    !set.contains(&o)
                        && self.placed[o]
                        && self.within.as_ref().is_none_or(|w| w.contains(&o))
                        && self.bodies_meet(*u, o)
                })
            });
            if on {
                self.shift(&set, along_vec(s, slide));
            }
        }
    }

    /// [`Sketch::text_clashes`] of `units` where they stand on the page as it is: what the texts of these parts cannot
    /// help (a mark set on a pin has its texts on that pin's part).
    fn clashes_before(&mut self, units: &[usize]) -> (usize, usize) {
        let now = self.d.clone();
        for u in units {
            self.d[*u] = Pt::default();
        }
        // No wires: where a part stands on the wrong side of its pin its wire has no plain shape.
        let clashes = self.text_clashes(units, &[]);
        self.d = now;
        clashes
    }

    /// How many texts of `units` the text placer cannot set clear with the units where they now stand, each of `wires`
    /// (a host's pin and a follower's) drawn in its plain shape (straight, with its one corner, or stepping at the
    /// middle), and how many it sets more than a step away from their part: asked on a page of those parts alone.
    fn text_clashes(&self, units: &[usize], wires: &[(PinId, PinId)]) -> (usize, usize) {
        let page = self.w.page;
        // No drawing area: where the stack stands on the sheet is settled afterwards.
        let far = Tick::MAX / 4;
        let mut mini = DraftPage {
            kind: page.kind,
            area: Rect::new(Pt::new(-far, -far), Pt::new(far, far)),
            ..DraftPage::default()
        };
        let mut ids: BTreeSet<PartId> = BTreeSet::new();
        for u in units {
            for i in &self.units[*u].parts {
                let mut part = self.w.parts[*i].clone();
                part.at = part.at.plus(self.d[*u]);
                ids.insert(part.id);
                for t in page.fixed_texts.iter().filter(|t| t.owner == part.id.0) {
                    let mut t = t.clone();
                    t.rect = t.rect.moved(self.d[*u]);
                    mini.fixed_texts.push(t);
                }
                mini.parts.push(part);
            }
        }
        mini.labels = page
            .labels
            .iter()
            .filter(|l| matches!(l.owner, Owner::Part(p) if ids.contains(&p)))
            .cloned()
            .collect();
        for (h, f) in wires {
            let (a, b) = (self.pt(*f), self.pt(*h));
            let (Some(sf), Some(sh)) = (self.side(*f), self.side(*h)) else {
                continue;
            };
            let Some(route) = plain_line((a, sf), (b, sh)) else {
                continue;
            };
            mini.wires.push(Wire {
                id: WireId(f.0),
                ends: [End::Pin(*f), End::Pin(*h)],
                route,
                computed: true,
                stale: false,
                class: crate::model::WireClass::Wire,
                supply: None,
            });
        }
        // Every other wired pin of these parts with its first steps of wire.
        let cornered: BTreeSet<PinId> = mini
            .wires
            .iter()
            .flat_map(|w| w.ends)
            .filter_map(|e| match e {
                End::Pin(p) => Some(p),
                End::Junction(_) => None,
            })
            .collect();
        let wired: BTreeSet<PinId> = self.w.nets.iter().flat_map(|n| n.pins.iter().copied()).collect();
        for part in mini.parts.clone() {
            for pin in &part.pins {
                let Some(s) = pin.side else { continue };
                if cornered.contains(&pin.id) || !wired.contains(&pin.id) {
                    continue;
                }
                let at = part.at.plus(pin.at);
                let end = at.plus(along_vec(s, TRIAL_STUB));
                let j = JunctionId(pin.id.0);
                mini.junctions.push(Junction { id: j, at: end });
                mini.wires.push(Wire {
                    id: WireId(pin.id.0),
                    ends: [End::Pin(pin.id), End::Junction(j)],
                    route: vec![at, end],
                    computed: true,
                    stale: false,
                    class: crate::model::WireClass::Wire,
                    supply: None,
                });
            }
        }
        let q = measure(&mini);
        // (A label within a step of what it names reads as its own, §1.3.)
        let away = q
            .labels
            .iter()
            .filter(|l| {
                mini.parts
                    .iter()
                    .find(|part| part.id.0 == l.id.owner)
                    .is_some_and(|part| crate::geom::gap(l.rect, part.bounds()) > TICKS_PER_STEP)
            })
            .count();
        (q.counts.label_clashes + q.counts.across_texts, away)
    }

    /// Whether the way out of pin `p` along its side runs into a body of unit `o`.
    fn walled(&self, p: PinId, o: usize) -> bool {
        let Some(s) = self.side(p) else { return false };
        let at = self.pt(p);
        self.bodies(o).iter().any(|b| {
            let (lo, hi) = span_across(*b, s);
            let (_, far) = span_along(*b, s);
            lo - BODY_GAP < s.across(at) && s.across(at) < hi + BODY_GAP && far > out(at, s)
        })
    }

    /// **Level followers.** One whose wire has no way out and back (it stands under or over its host) is set beside the
    /// host, pins level, the pin nearest that side innermost, where that side or the other has the room. One that stands
    /// neither beside its host nor near beyond its pins takes the row beyond them: two steps clear of the host and off
    /// the pin's line by its own width, so that the wire passes it, on the side where the host has the fewer pins of
    /// that edge. On a bus page two stations whose ports face one side stand level (rule B2). A level follower that
    /// keeps its place joins the next rank: what hangs on a strip of terminals is one row.
    ///
    /// The level followers on side `s` of their hosts (§4.3). Those that keep their place: what hangs
    /// on them is then set with the rest of their rank.
    pub(super) fn beside(&mut self, s: Side, members: &[usize]) -> Vec<usize> {
        let mut stay: Vec<usize> = Vec::new();
        let mut items: Vec<(usize, Lead)> = members
            .iter()
            .filter_map(|m| self.lead[*m].map(|l| (*m, l)))
            .collect();
        items.sort_by_key(|(m, l)| (s.across(self.pt(l.h)), self.id(*m)));
        // The ones whose wire has no way out and back, by host.
        let mut stuck: BTreeMap<usize, Vec<(usize, Lead)>> = BTreeMap::new();
        for (m, l) in items {
            let units = self.subtree(m);
            let walled = units.iter().any(|u| self.walled(l.h, *u)) || self.walled(l.f, l.host);
            if walled {
                stuck.entry(l.host).or_default().push((m, l));
            } else {
                // On a bus page (rule B2) two stations whose ports face one side stand level.
                let by = s.beyond(self.pt(l.f), self.pt(l.h));
                if self.w.page.kind == PageKind::Bus && by != 0 {
                    self.shift(&units, along_vec(s, by));
                    if units.iter().any(|u| self.blocked(*u, &units)) {
                        self.shift(&units, along_vec(s, -by));
                        self.failed = true;
                    }
                } else if self.w.page.kind != PageKind::Bus && !self.hairpin(s, l, &units) {
                    stay.push(m);
                    continue;
                }
                for u in units {
                    self.placed[u] = true;
                }
            }
        }
        for (host, list) in stuck {
            let (lo, hi) = span_across(self.hull(host), s);
            // One side for them all: the one their pins lie nearer to.
            let (to_lo, to_hi) = list.iter().fold((0i64, 0i64), |(a, b), (_, l)| {
                let x = s.across(self.pt(l.h));
                (a + i64::from(x - lo), b + i64::from(hi - x))
            });
            let sides: [bool; 2] = if to_lo < to_hi {
                [true, false]
            } else {
                [false, true]
            };
            let start = self.d.clone();
            let mut done = false;
            for low in sides {
                self.d.clone_from(&start);
                // The pin nearest that side innermost.
                let mut order = list.clone();
                order.sort_by_key(|(m, l)| {
                    let x = s.across(self.pt(l.h));
                    (if low { x } else { -x }, self.id(*m))
                });
                let mut edge = if low { lo } else { hi };
                let mut ok = true;
                let mut set: Vec<usize> = Vec::new();
                for (m, l) in &order {
                    let units = self.subtree(*m);
                    // Pins level.
                    let by = s.beyond(self.pt(l.f), self.pt(l.h));
                    self.shift(&units, along_vec(s, by));
                    let hull = units
                        .iter()
                        .map(|u| self.hull(*u))
                        .reduce(Rect::union)
                        .unwrap_or_default();
                    let (a, b) = span_across(hull, s);
                    let by = if low {
                        -grid_away(b + TRACK_PITCH - edge)
                    } else {
                        grid_away(edge + TRACK_PITCH - a)
                    };
                    self.shift(&units, across_vec(s, by));
                    let on = units
                        .iter()
                        .any(|u| self.blocked(*u, &units) || set.iter().any(|o| self.meets(*u, *o)));
                    if on {
                        ok = false;
                        break;
                    }
                    let hull = units
                        .iter()
                        .map(|u| self.hull(*u))
                        .reduce(Rect::union)
                        .unwrap_or_default();
                    edge = if low {
                        span_across(hull, s).0
                    } else {
                        span_across(hull, s).1
                    };
                    set.extend(units);
                }
                if ok {
                    done = true;
                    break;
                }
            }
            if !done {
                // No room beside: they stay where they stand.
                self.d = start;
                stay.extend(list.iter().map(|(m, _)| *m));
                continue;
            }
            for (m, _) in &list {
                for u in self.subtree(*m) {
                    self.placed[u] = true;
                }
            }
        }
        // What hangs on one that stays was set by itself: it is set anew with its rank.
        for m in &stay {
            for u in self.subtree(*m) {
                if u != *m {
                    self.d[u] = Pt::default();
                }
            }
            self.placed[*m] = true;
        }
        stay
    }

    /// A level follower (`units`: it and what hangs on it) that stands neither beside its host nor near beyond its
    /// pins takes the row beyond them: two steps clear of the host, and off the pin's line by its own width so that
    /// the wire passes it, on the side where the host has the fewer pins of that edge (the side it stands on where
    /// they are as many). Where neither side has the room it stays. Whether it moved.
    fn hairpin(&mut self, s: Side, l: Lead, units: &[usize]) -> bool {
        let bodies = |sk: &Self, us: &[usize]| {
            us.iter()
                .flat_map(|u| sk.bodies(*u))
                .reduce(Rect::union)
                .unwrap_or_default()
        };
        let (host, own) = (bodies(self, &[l.host]), bodies(self, units));
        let ((h0, h1), (o0, o1)) = (span_along(host, s), span_along(own, s));
        let (lo, hi) = span_across(own, s);
        let (h, f) = (s.across(self.pt(l.h)), s.across(self.pt(l.f)));
        let clear = hi + BODY_GAP <= h || h <= lo - BODY_GAP;
        let beside = o0 < h1 && h0 < o1;
        let near = (h1 + CHAIN_GAP_MIN..=h1 + CHAIN_GAP + ROW_PULL).contains(&o0);
        if beside || (near && clear) {
            return false;
        }
        let pins = |sk: &Self, low: bool| {
            sk.units[l.host]
                .parts
                .iter()
                .flat_map(|i| sk.w.parts[*i].pins.iter())
                .filter(|p| p.side == Some(s) && p.id != l.h && (s.across(sk.pt(p.id)) < h) == low)
                .count()
        };
        let low = (pins(self, true), f > h) < (pins(self, false), f <= h);
        let along = grid_away(h1 + CHAIN_GAP_MIN - o0);
        let start = self.d.clone();
        for low in [low, !low] {
            let by = if low {
                -grid_away((hi + BODY_GAP - h).max(f + TRACK_PITCH - h))
            } else {
                grid_away((h + BODY_GAP - lo).max(h + TRACK_PITCH - f))
            };
            // (Further out where the texts of the two need it.)
            for more in 0..=ROW_PULL / TICKS_PER_STEP {
                let step = along_vec(s, along + more * TICKS_PER_STEP).plus(across_vec(s, by));
                self.shift(units, step);
                let ok = units.iter().all(|u| self.inside(*u) && !self.blocked(*u, units));
                if ok {
                    return true;
                }
                self.d.clone_from(&start);
            }
        }
        false
    }
}
