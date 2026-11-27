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
    pub label: String,
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
    /// The points, from the first pin to the other.
    pub points: Vec<Pt2>,
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
        let card = card(&now, &after, space, command, all);
        Some(Ok((card, preview)))
    }
}

/// The card of the plan of `space` on the snapshot `now`, its stored state `after`.
fn card(now: &Q, after: &Q, space: Id<LayoutSpace>, command: AnyCommand, all: bool) -> CabinetRoute {
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
            let CarrierParams::Duct(d) = &carrier.params else {
                continue;
            };
            let (area, inner) = fill.fill();
            out.ducts.push(CardDuct {
                carrier: c.as_uuid(),
                label: carrier_label(after, c).map(|l| l.label()).unwrap_or_default(),
                size: format!(
                    "{}×{}",
                    ew_derive::mm_exact(i128::from(d.width.0)),
                    ew_derive::mm_exact(i128::from(d.height.0))
                ),
                percent: ew_units::arith::div_round_ties_away(area * 100, inner.max(1)),
                inner_mm2: ew_units::arith::div_round_ties_away(inner, 1_000_000_000_000),
                limit: fill.limit,
                over: fill.over(),
            });
        }
        out.ducts.sort_by_key(|d| !d.over);
    }
    out
}
