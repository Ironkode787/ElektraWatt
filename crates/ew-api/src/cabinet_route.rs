// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route on a panel page (doc 12 §8.5, §12.5): the cabinet's plan as Route's preview shows it, from a preview of the
//! `layout.route` that stores it (ADR-0008's fork), so the card's lengths and fills are those the stored ways give.

use ew_derive::layout::{
    DuctFill, Planned, RouteScope, RouteStatus, cabinet_name, carrier_label, carriers_in_order, page_surface,
    plan_routes, route_status, routed_length, way_points,
};
use ew_derive::{PageOrder, PageRef, Q, WireLength, WireRecordSet, cp_label};
use ew_ids::{Id, Uuid};
use ew_l10n::Text;
use ew_schema::{CarrierParams, Enclosure, LayoutSpace, MountingSurface, Page};
use ew_units::{Nm, Pt2};
use serde_json::json;

use crate::{AnyCommand, CmdError, Preview, Session};

/// One duct of Route's card: its label, size and fill where fullest.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CardDuct {
    /// The duct.
    pub carrier: Uuid,
    /// Its label ("Duct 1").
    pub label: crate::Text,
    /// Its nominal size ("40×60").
    pub size: String,
    /// Its fill where fullest, in whole percent.
    pub percent: i128,
    /// Its inner area in whole square millimetres.
    pub inner_mm2: i128,
    /// The fill limit, in percent.
    pub limit: u8,
    /// Whether the fullest piece is over the limit.
    pub over: bool,
    /// Where its chip stands on the page: the middle of its fullest piece (page nm).
    pub chip: (i64, i64),
}

/// One wire of Route's card.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CardWire {
    /// The wire, by its carrier connection.
    pub wire: Uuid,
    /// Its number (`101.8`); empty without one.
    pub number: String,
    /// Its ends (`-101Q1:2 → -103F1:1`).
    pub ends: String,
    /// The length its stored way gave before, for a wire whose way changes.
    pub was: Option<Nm>,
    /// The length the plan gives it (for a wire with a typed length: what Route gives).
    pub length: Option<Nm>,
    /// The typed length a wire keeps.
    pub typed: Option<Nm>,
    /// Why it gets no way (Without a way), and its words.
    pub reason: Option<(ew_derive::layout::NoWay, Text)>,
}

impl CardWire {
    /// Why the wire gets no way, as `ew route --json` names it: `unplaced`, `no_duct` or `off_plate`.
    pub fn reason_key(&self) -> Option<&'static str> {
        use ew_derive::layout::NoWay;
        self.reason.as_ref().map(|(why, _)| match why {
            NoWay::Unplaced(_) => "unplaced",
            NoWay::NoDuct { .. } => "no_duct",
            NoWay::OffPlate(_) => "off_plate",
        })
    }
}

/// One way the canvas draws while the card is open: its points in its surface's frame.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CardWay {
    /// The wire.
    pub wire: Uuid,
    /// The points, from the first pin to the other, in the surface's frame.
    pub points: Vec<Pt2>,
    /// The same on the page (page nm), each way in a duct on a lane of its own across the duct's width, so that the
    /// ways in a duct stand side by side.
    pub page: Vec<(i64, i64)>,
    /// Whether it crosses into a door.
    pub door: bool,
}

/// Route's card of a cabinet (doc 12 §12.5): what the plan does, before anything changes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CabinetRoute {
    /// The layout space.
    pub space: Uuid,
    /// The cabinet's name (`+CAB`).
    pub cabinet: String,
    /// The plate the panel page shows.
    pub surface: Uuid,
    /// The command that stores the plan.
    pub command: AnyCommand,
    /// Whether every wire is planned again (the tick Route every wire again).
    pub all: bool,
    /// The wires whose way still holds and that the plan keeps (the tick's count).
    pub holding: usize,
    /// The ducts, one over the limit first, else in the legend's order.
    pub ducts: Vec<CardDuct>,
    /// The wires that get a way, in the wire list's order (Lengths).
    pub lengths: Vec<CardWire>,
    /// The wires that keep their typed length, with what Route gives (Typed by hand).
    pub typed: Vec<CardWire>,
    /// The wires made on site.
    pub site: Vec<CardWire>,
    /// The wires without a way, each with why.
    pub without: Vec<CardWire>,
    /// The ways the plan lays.
    pub ways: Vec<CardWay>,
}

/// The layout space and plate of the panel page `page`: the space of the enclosure whose plate it shows.
fn cabinet_of_page(q: &Q, page: Id<Page>) -> Option<(Id<LayoutSpace>, Id<MountingSurface>)> {
    let surface = page_surface(q, page)?;
    let s = q.table::<MountingSurface>().get(surface)?;
    let space = q.table::<Enclosure>().get(s.enclosure)?.space;
    Some((space, surface))
}

/// The ends of a wire in words.
fn ends_text(q: &Q, r: &ew_derive::WireRecord) -> String {
    let [a, b] = r.wire.ends.map(|e| cp_label(q, e.cp));
    format!("{a} → {b}")
}

impl Session {
    /// The layout space the panel page labelled `folio` lays out; `None` for an unknown folio or a page that shows no
    /// cabinet's plate.
    pub fn cabinet_of_folio(&self, folio: &str) -> Option<Uuid> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let PageRef::Drawn(page) = order.by_folio(folio)?.page else {
            return None;
        };
        cabinet_of_page(&q, page).map(|(space, _)| space.as_uuid())
    }

    /// The stored ways of the wires at the placements `placements` on the panel page labelled `folio` (doc 12 §12.5: a
    /// device selected on the panel page shows the ways of its wires): each way's points on the page and whether it no
    /// longer holds (drawn dashed).
    pub fn ways_at(&self, folio: &str, placements: &[Uuid]) -> Vec<(Vec<(i64, i64)>, bool)> {
        let q = self.query();
        let Some(PageRef::Drawn(page)) = q.get::<PageOrder>(()).by_folio(folio).map(|e| e.page) else {
            return Vec::new();
        };
        let Some(view) = q.get::<crate::output::PageSurfaceView>(page).as_ref().clone() else {
            return Vec::new();
        };
        let records = q.get::<WireRecordSet>(());
        let mut out = Vec::new();
        for r in records.iter() {
            let Some(way) = ew_derive::layout::stored_way(&q, r.wire.id) else {
                continue;
            };
            let at = way.ends.iter().any(|e| {
                ew_derive::layout::placement_of_function(&q, e.cp.0)
                    .is_some_and(|p| placements.contains(&p.as_uuid()))
            });
            if !at {
                continue;
            }
            let planned = ew_derive::layout::Way {
                ends: way.ends.clone(),
                steps: way.way.clone(),
                run: way.run,
            };
            let points = page_points(&q, Some(&view.projection), &planned, |_| 0);
            let stale = matches!(route_status(&q, &r.wire), RouteStatus::Stale { .. });
            out.push((points, stale));
        }
        out
    }

    /// What Route has to do on the panel page labelled `folio` (doc 12 §12.5, the HUD's next step): the wires of its
    /// cabinet Route lays that have no length yet, and those whose way no longer holds; `None` for a page that shows no
    /// cabinet's plate.
    pub fn cabinet_due(&self, folio: &str) -> Option<(usize, usize)> {
        let space = Id::<LayoutSpace>::from_uuid(self.cabinet_of_folio(folio)?);
        let q = self.query();
        let wires = ew_derive::layout::cabinet_wires(&q, space);
        let routed = wires
            .iter()
            .filter(|(_, k)| *k == ew_derive::layout::CabinetWire::Routed);
        let (mut missing, mut stale) = (0, 0);
        for (r, _) in routed {
            match r.length {
                WireLength::Missing => missing += 1,
                WireLength::Stale { .. } => stale += 1,
                _ => {}
            }
        }
        Some((missing, stale))
    }

    /// Route on the panel page labelled `folio` (doc 12 §8.5, §12.5): the card of the cabinet's plan and the preview
    /// of the `layout.route` that stores it, which [`Session::commit`] applies in one step; `all` plans every wire of
    /// the cabinet again. `None` for a page that shows no cabinet's plate.
    ///
    /// # Errors
    ///
    /// The command's refusal.
    pub fn cabinet_route(
        &mut self,
        folio: &str,
        all: bool,
    ) -> Option<Result<(CabinetRoute, Preview), CmdError>> {
        let space = Id::<LayoutSpace>::from_uuid(self.cabinet_of_folio(folio)?);
        let command = AnyCommand {
            name: "layout.route".to_owned(),
            params: if all {
                json!({"space": space.as_uuid(), "all": true})
            } else {
                json!({"space": space.as_uuid()})
            },
        };
        let preview = match self.preview(&command) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };
        let now = self.query();
        let after = self.query_over(preview.candidate());
        let page = match now.get::<PageOrder>(()).by_folio(folio).map(|e| e.page) {
            Some(PageRef::Drawn(p)) => Some(p),
            _ => None,
        };
        let projection = page
            .and_then(|p| now.get::<crate::output::PageSurfaceView>(p).as_ref().clone())
            .map(|v| v.projection);
        let card = card(&now, &after, space, command, all, projection.as_ref());
        Some(Ok((card, preview)))
    }
}

/// A surface point on the page (`(0, 0)` without a projection).
fn on_page(projection: Option<&ew_scene::panel::Projection>, (u, v): (i128, i128)) -> (i64, i64) {
    projection.map_or((0, 0), |p| crate::output::panel::page_of_surface(p, (u, v)))
}

/// A way's points on the page, each duct stretch shifted across its duct by `lane(carrier)`; a direct way across, then
/// along.
fn page_points(
    q: &Q,
    projection: Option<&ew_scene::panel::Projection>,
    way: &ew_derive::layout::Way,
    lane: impl Fn(Id<ew_schema::Carrier>) -> i128,
) -> Vec<(i64, i64)> {
    use ew_derive::layout::DuctLine;
    use ew_schema::WayStep;
    let at = |p: Pt2| (i128::from(p.x.0), i128::from(p.y.0));
    let (a, b) = (at(way.ends[0].at), at(way.ends[1].at));
    let mut pts: Vec<(i128, i128)> = vec![a];
    let mut any = false;
    for step in &way.steps {
        let WayStep::Duct(d) = step else { continue };
        let Some(line) = DuctLine::of(q, d.carrier) else {
            continue;
        };
        let stretch = line.stretch(i128::from(d.from.0), i128::from(d.to.0));
        let off = lane(d.carrier);
        // A horizontal neighbour moves a point up or down, a vertical one left or right.
        for (i, p) in stretch.iter().enumerate() {
            let near = [i.checked_sub(1).and_then(|j| stretch.get(j)), stretch.get(i + 1)];
            let (mut dx, mut dy) = (0, 0);
            for n in near.into_iter().flatten() {
                if n.1 == p.1 && n.0 != p.0 {
                    dy = off;
                }
                if n.0 == p.0 && n.1 != p.1 {
                    dx = off;
                }
            }
            pts.push((p.0 + dx, p.1 + dy));
        }
        any = true;
    }
    if !any {
        pts.push((b.0, a.1));
    }
    pts.push(b);
    let mut out: Vec<(i64, i64)> = pts.into_iter().map(|p| on_page(projection, p)).collect();
    out.dedup();
    out
}

/// The card of the plan of `space` on the snapshot `now`, its stored state `after`; its marks on the page of
/// `projection`.
fn card(
    now: &Q,
    after: &Q,
    space: Id<LayoutSpace>,
    command: AnyCommand,
    all: bool,
    projection: Option<&ew_scene::panel::Projection>,
) -> CabinetRoute {
    let scope = if all { RouteScope::All } else { RouteScope::Due };
    let plan = plan_routes(now, space, &scope);
    let surface = ew_derive::layout::plate_of_space(now, space);
    let records = after.get::<WireRecordSet>(());
    let after_length = |wire| records.iter().find(|r| r.wire.id == wire).map(|r| r.length);
    let mut out = CabinetRoute {
        space: space.as_uuid(),
        cabinet: cabinet_name(now, space),
        surface: surface.map_or(Uuid::nil(), |s| s.as_uuid()),
        command,
        all,
        holding: 0,
        ducts: Vec::new(),
        lengths: Vec::new(),
        typed: Vec::new(),
        site: Vec::new(),
        without: Vec::new(),
        ways: Vec::new(),
    };
    // The lanes: each duct's planned ways side by side across its width, in the plan's order.
    let mut lanes: std::collections::BTreeMap<
        Id<ew_schema::Carrier>,
        Vec<ew_ids::Id<ew_schema::Connection>>,
    > = std::collections::BTreeMap::new();
    for w in &plan.wires {
        if let Planned::Way(way) = &w.planned {
            for step in &way.steps {
                if let ew_schema::WayStep::Duct(d) = step {
                    lanes.entry(d.carrier).or_default().push(w.wire.wire.id);
                }
            }
        }
    }
    let width = |c: Id<ew_schema::Carrier>| match now.table::<ew_schema::Carrier>().get(c).map(|c| &c.params)
    {
        Some(CarrierParams::Duct(d)) => i128::from(d.width.0),
        _ => 0,
    };
    let lane_of = |wire: ew_ids::Id<ew_schema::Connection>, c: Id<ew_schema::Carrier>| {
        let all = lanes.get(&c).map_or(&[][..], Vec::as_slice);
        let (k, n) = (all.iter().position(|w| *w == wire).unwrap_or(0), all.len());
        let n = i128::try_from(n).unwrap_or(1);
        let k = i128::try_from(k).unwrap_or(0);
        width(c) * (2 * k + 2 - (n + 1)) / (2 * (n + 1))
    };
    for w in &plan.wires {
        let r = &w.wire;
        let row = CardWire {
            wire: r.wire.id.as_uuid(),
            number: r.number.clone().unwrap_or_default(),
            ends: ends_text(now, r),
            was: None,
            length: None,
            typed: None,
            reason: None,
        };
        match &w.planned {
            Planned::Way(way) => {
                out.ways.push(CardWay {
                    wire: r.wire.id.as_uuid(),
                    points: way_points(now, &way.ends, &way.steps),
                    page: page_points(now, projection, way, |c| lane_of(r.wire.id, c)),
                    door: way.door(),
                });
                if let WireLength::Typed(n) = r.length {
                    out.typed.push(CardWire {
                        typed: Some(n),
                        length: Some(routed_length(now, way.run, way.door())),
                        ..row
                    });
                    continue;
                }
                let was = match route_status(now, &r.wire) {
                    RouteStatus::Holds { run, door } | RouteStatus::Stale { run, door, .. } => {
                        Some(routed_length(now, run, door))
                    }
                    RouteStatus::None => None,
                };
                let length = after_length(r.wire.id).and_then(WireLength::value);
                out.lengths.push(CardWire {
                    was: was.filter(|w| Some(*w) != length),
                    length,
                    ..row
                });
            }
            Planned::Kept => out.holding += 1,
            Planned::NoWay(why) => out.without.push(CardWire {
                reason: Some((*why, ew_derive::text::no_way(now, *why))),
                ..row
            }),
            Planned::Site => out.site.push(row),
            Planned::Core => {}
        }
    }
    // Every wire with a stored way that holds counts for the tick, also one the plan lays again with it.
    if all {
        out.holding = plan
            .wires
            .iter()
            .filter(|w| matches!(route_status(now, &w.wire.wire), RouteStatus::Holds { .. }))
            .count();
    }
    if let Some(plate) = surface {
        for c in carriers_in_order(after, plate) {
            let (Some(fill), Some(carrier)) = (
                after.get::<DuctFill>(c).as_ref().clone(),
                after.table::<ew_schema::Carrier>().get(c).cloned(),
            ) else {
                continue;
            };
            let (CarrierParams::Duct(d), Some(label)) = (&carrier.params, carrier_label(after, c)) else {
                continue;
            };
            let (area, inner) = fill.fill();
            let chip = ew_derive::layout::DuctLine::of(after, c)
                .and_then(|l| {
                    let piece = fill.fullest.and_then(|i| fill.pieces.get(i))?;
                    l.at((piece.from + piece.to) / 2)
                        .map(|(_, p)| on_page(projection, p))
                })
                .unwrap_or_default();
            out.ducts.push(CardDuct {
                carrier: c.as_uuid(),
                label: ew_derive::text::carrier_label(label),
                size: format!(
                    "{}×{}",
                    ew_derive::mm_exact(i128::from(d.width.0)),
                    ew_derive::mm_exact(i128::from(d.height.0))
                ),
                percent: ew_units::arith::div_round_ties_away(area * 100, inner.max(1)),
                inner_mm2: ew_units::arith::div_round_ties_away(inner, 1_000_000_000_000),
                limit: fill.limit,
                over: fill.over(),
                chip,
            });
        }
        out.ducts.sort_by_key(|d| !d.over);
    }
    out
}
