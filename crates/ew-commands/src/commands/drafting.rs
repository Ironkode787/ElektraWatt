// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `layout.apply` (docs/design/06a-drafting.md §2.6): a drafting plan stored in one transaction.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{
    Connection, Endpoint, Junction, JunctionStyle, Kind, LibCopy, Page, PageItem, PageItemKind,
    Representation, SymbolDef,
};
use ew_units::{Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::connection::{RouteConnection, point_bevels, run_routes};
use crate::error::CmdError;
use crate::geometry::{rep_body as body, require_on_grid};
use crate::meta::CommandMeta;
use crate::proxy::{Orient2S, Pt2S, UuidS};
use crate::tx::TxBuilder;

/// The most entries of each list one `layout.apply` takes.
pub const LAYOUT_APPLY_MAX: usize = 10_000;

/// A junction's new place in a plan.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JunctionPlace {
    /// The junction.
    #[schemars(with = "UuidS")]
    pub junction: Id<Junction>,
    /// Where it stands, on the page's grid.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// The way a bevel points; absent: as the routes that meet at it ask. A dot takes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Orient2S>")]
    pub orient: Option<Orient2>,
}

/// A symbol's move in a plan, by whole steps of its page's grid.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartMove {
    /// The representation.
    #[schemars(with = "UuidS")]
    pub rep: Id<Representation>,
    /// Grid steps `[kx, ky]`, as `rep.move` takes them.
    pub steps: [i64; 2],
}

/// A wire marker's new anchor in a plan.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkerPlace {
    /// The wire marker.
    #[schemars(with = "UuidS")]
    pub item: Id<PageItem>,
    /// Its new anchor point.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
}

/// `layout.apply` (`ApplyLayout`, docs/design/06a-drafting.md §2.6): the plan of Route or of Arrange and route on one
/// page in one transaction and one undo step: the symbols at their new places, the junctions at theirs, the routes,
/// the wire markers that follow their wires.
///
/// - Every named record must lie on `page`. A symbol moves by whole grid steps as `rep.move`'s `steps` moves it (its
///   position, or a fitted symbol's first pin); a symbol kept in place is refused, as is one moved twice, one whose
///   connection points or anchor would leave the drawing area of a page with a frame grid, and one whose body would
///   stand on another symbol's by a grid step or more each way where it did not before. Every routed wire with an end
///   on a moved symbol must get its route in the same plan.
/// - A junction's place must be on the page's grid; every routed wire of a moved junction must get its route in the
///   same plan.
/// - Each route is checked as `connection.route` checks it, after the junctions have moved (both ends on their
///   connection points or junctions, on the grid), and is marked `computed`.
/// - A bevel junction that moves, or whose wires get routes, points as given or as its routes ask; a dot stays a
///   dot. No member can change a wire's ends, a junction's form, its wires or its sequence.
///
/// Rejected: nothing to apply; more than 10,000 entries in a list; a record on another page; a symbol kept in place,
/// moved twice, out of the drawing area or onto another; a symbol or a junction moved without the routes of its
/// wires. Cost: `Bulk` above 500 entries, else interactive. Outcome: the first routed connection.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyLayout {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The symbols that move.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartMove>,
    /// The junctions that move.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub junctions: Vec<JunctionPlace>,
    /// The new routes, each `{connection, route}` as `connection.route` takes it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<RouteConnection>,
    /// The wire markers that move with their wires.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<MarkerPlace>,
}

impl ApplyLayout {
    /// Moves the symbols of `parts` (the type's documentation).
    fn move_parts(&self, tx: &mut TxBuilder<'_>) -> Result<(), CmdError> {
        let mut seen: BTreeSet<Id<Representation>> = BTreeSet::new();
        let mut moved: Vec<(Representation, Representation)> = Vec::new();
        for (i, m) in self.parts.iter().enumerate() {
            let rep = tx.require::<Representation>(m.rep)?;
            if rep.page != self.page {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/parts/{i}/rep"),
                    text!(
                        "refusal-layout-apply-other-page",
                        crate::words::symbol(tx, m.rep),
                        crate::words::page(tx, self.page)
                    ),
                ));
            }
            if !seen.insert(m.rep) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/parts/{i}/rep"),
                    text!("refusal-layout-apply-twice", crate::words::symbol(tx, m.rep)),
                ));
            }
            if rep.locked {
                return tx.reject(text!(
                    "refusal-layout-apply-kept",
                    crate::words::symbol(tx, m.rep)
                ));
            }
            let next = crate::commands::device::stepped(tx, &rep, m.steps)?;
            moved.push(((*rep).clone(), next));
        }
        if moved.is_empty() {
            return Ok(());
        }
        let page = tx.require::<Page>(self.page)?;
        let frame = crate::numbering::frame_grid(tx, &page);
        let bodies_before: Vec<(Id<Representation>, Option<[i64; 4]>)> = tx
            .reps_on_page(self.page)
            .into_iter()
            .filter_map(|id| tx.get::<Representation>(id))
            .map(|r| (r.id, body(tx, &r)))
            .collect();
        for (_, next) in &moved {
            tx.put(next.clone());
        }
        for (old, next) in &moved {
            // Its connection points and its anchor inside the drawing area of a page with a frame grid.
            if let Some(grid) = &frame {
                let mut points: Vec<Pt2> = crate::geometry::rep_cps(tx, next).iter().map(|c| c.pos).collect();
                points.extend(
                    tx.get::<LibCopy<SymbolDef>>(next.symbol)
                        .and_then(|s| ew_derive::anchor_point_of(&s.item, next)),
                );
                if points
                    .iter()
                    .any(|p| ew_derive::cell_in_grid(grid, page.grid, *p).is_none())
                {
                    return tx.reject(text!(
                        "refusal-layout-apply-outside",
                        crate::words::symbol(tx, next.id),
                        crate::words::page(tx, self.page)
                    ));
                }
            }
            // Its body not onto another's by a grid step or more each way, where they did not overlap so before.
            let step = crate::commands::device::steps_nm(page.grid, 1);
            let (Some(a), Some(a0)) = (body(tx, next), body(tx, old)) else {
                continue;
            };
            for (other, b0) in &bodies_before {
                if *other == next.id {
                    continue;
                }
                let Some(r) = tx.get::<Representation>(*other) else {
                    continue;
                };
                let Some(b) = body(tx, &r) else { continue };
                let deep = |a: [i64; 4], b: [i64; 4]| {
                    a[2].min(b[2]) - a[0].max(b[0]) >= step && a[3].min(b[3]) - a[1].max(b[1]) >= step
                };
                if deep(a, b) && !b0.is_some_and(|b0| deep(a0, b0)) {
                    return tx.reject(text!(
                        "refusal-layout-apply-on-symbol",
                        crate::words::symbol(tx, next.id),
                        crate::words::symbol(tx, *other)
                    ));
                }
            }
        }
        // Every routed wire with an end on a connection point of a moved symbol gets its route in the plan.
        for (old, _) in &moved {
            let mut wires: Vec<Id<Connection>> = crate::geometry::rep_cps(tx, old)
                .iter()
                .filter_map(|c| ew_model::EndpointKey::of(&Endpoint::Cp(c.function, c.idx)))
                .flat_map(|k| tx.connections_at(k))
                .collect();
            wires.sort();
            wires.dedup();
            for c in wires {
                let Some(w) = tx.get::<Connection>(c) else {
                    continue;
                };
                if w.page == Some(self.page)
                    && !w.route.is_empty()
                    && !self.routes.iter().any(|r| r.connection == c)
                {
                    return tx.reject(text!(
                        "refusal-layout-apply-symbol-route",
                        crate::words::symbol(tx, old.id),
                        crate::words::wire(tx, c)
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Command for ApplyLayout {
    const NAME: &'static str = "layout.apply";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, _model: &ew_model::Model) -> crate::meta::Cost {
        let n = self.parts.len() + self.junctions.len() + self.routes.len() + self.markers.len();
        if n > crate::commands::BULK_ABOVE {
            crate::meta::Cost::Bulk
        } else {
            crate::meta::Cost::Interactive
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        tx.require::<Page>(self.page)?;
        if self.parts.is_empty()
            && self.junctions.is_empty()
            && self.routes.is_empty()
            && self.markers.is_empty()
        {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/routes",
                text!("refusal-layout-apply-nothing"),
            ));
        }
        for (field, n) in [
            ("/parts", self.parts.len()),
            ("/junctions", self.junctions.len()),
            ("/routes", self.routes.len()),
            ("/markers", self.markers.len()),
        ] {
            if n > LAYOUT_APPLY_MAX {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    field,
                    text!(
                        "refusal-layout-apply-too-many",
                        n.to_string(),
                        LAYOUT_APPLY_MAX.to_string()
                    ),
                ));
            }
        }
        for (i, r) in self.routes.iter().enumerate() {
            let c = tx.require::<Connection>(r.connection)?;
            if c.page != Some(self.page) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/routes/{i}/connection"),
                    text!(
                        "refusal-layout-apply-other-page",
                        crate::words::wire(tx, r.connection),
                        crate::words::page(tx, self.page)
                    ),
                ));
            }
        }
        // The symbols and the junctions first, so that the routes are checked against their new places.
        self.move_parts(tx)?;
        for (i, place) in self.junctions.iter().enumerate() {
            let j = tx.require::<Junction>(place.junction)?;
            if j.page != self.page {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/junctions/{i}/junction"),
                    text!(
                        "refusal-layout-apply-junction-other-page",
                        crate::words::page(tx, self.page)
                    ),
                ));
            }
            require_on_grid(tx, self.page, place.at, &format!("/junctions/{i}/at"))?;
            if place.at != j.at {
                let left = j.sequence.iter().find(|c| {
                    tx.get::<Connection>(**c).is_some_and(|w| !w.route.is_empty())
                        && !self.routes.iter().any(|r| r.connection == **c)
                });
                if let Some(c) = left {
                    return tx.reject(text!(
                        "refusal-layout-apply-junction-route",
                        crate::words::wire(tx, *c)
                    ));
                }
            }
            tx.update::<Junction>(place.junction, |j| {
                j.at = place.at;
                if let (JunctionStyle::Bevel(b), Some(o)) = (&mut j.style, place.orient) {
                    b.orient = o;
                }
            })?;
        }
        let primary = run_routes(tx, Self::NAME, &self.routes)?;
        // A bevel at a moved junction or at the end of a new route points as its routes ask, unless the plan says how.
        let mut bevels: Vec<Id<Junction>> = self
            .junctions
            .iter()
            .filter(|p| p.orient.is_none())
            .map(|p| p.junction)
            .collect();
        for r in &self.routes {
            if let Some(c) = tx.get::<Connection>(r.connection) {
                for e in [&c.a, &c.b] {
                    if let ew_schema::Endpoint::Junction(j) = e
                        && !self
                            .junctions
                            .iter()
                            .any(|p| p.junction == *j && p.orient.is_some())
                    {
                        bevels.push(*j);
                    }
                }
            }
        }
        bevels.sort();
        bevels.dedup();
        point_bevels(tx, &bevels)?;
        for (i, m) in self.markers.iter().enumerate() {
            let item = tx.require::<PageItem>(m.item)?;
            let PageItemKind::WireMarker(marker) = &item.kind else {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/markers/{i}/item"),
                    text!("refusal-layout-apply-not-a-marker"),
                ));
            };
            if item.page != self.page {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/markers/{i}/item"),
                    text!(
                        "refusal-layout-apply-marker-other-page",
                        crate::words::page(tx, self.page)
                    ),
                ));
            }
            let d = Pt2::new(m.at.x - marker.at.x, m.at.y - marker.at.y);
            let Some(mut next) = crate::commands::items::translate_item(&item, d) else {
                return tx.reject(text!(
                    "refusal-rep-move-item-out-of-range",
                    text!("refusal-rep-move-item-wire-marker"),
                    crate::words::page(tx, self.page)
                ));
            };
            crate::commands::items::clean_marker(tx, &mut next);
            tx.put(next);
        }
        Ok(Outcome {
            primary: primary.or_else(|| {
                self.junctions
                    .first()
                    .map(|p| (Kind::Junction, p.junction.cast()))
            }),
            ..Outcome::default()
        })
    }
}
