// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route on a panel page (doc 12 §8.5, §12.5): the card of the cabinet's plan in the Preview tab, its tick Route every
//! wire again, a row picked to show its way alone, its Apply with the toast, and the marks the canvas draws while the
//! card is open: every planned way side by side in its duct, the picked one in the accent, a chip with each duct's
//! fill.

use ew_api::cabinet_route::CabinetRoute;
use ew_api::output::Uuid;
use serde_json::{Map, Value};

use super::{Outcome, ShellModel};
use crate::properties::{Card, CardOrigin};
use crate::strings::{self, Key};
use crate::{count_key, key};

/// One row of a cabinet's Route card: its wire (none for a duct's row) and its words.
pub type CardRow = (Option<Uuid>, String);

/// What a Route card of a cabinet shows (doc 12 §12.5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CabinetCard {
    /// The plan, as the session gives it.
    pub route: CabinetRoute,
    /// The panel page's folio.
    pub folio: String,
    /// "Route: cabinet +CAB. 41 wires get a way through the ducts; 3 are made on site. Nothing has changed yet."
    pub lead: String,
    /// The row picked to show its way alone.
    pub picked: Option<Uuid>,
}

impl CabinetCard {
    /// The card's rows by section, each with its wire and its words: Ducts, Lengths, Typed by hand, Made on site,
    /// Without a way; a section without rows is left out.
    pub fn sections(&self) -> Vec<(String, Vec<CardRow>)> {
        let r = &self.route;
        let mm = |n: ew_units::Nm| format!("{} mm", super::layout::mm_text(i128::from(n.0), false));
        let name = |w: &ew_api::cabinet_route::CardWire| {
            if w.number.is_empty() {
                w.ends.clone()
            } else {
                w.number.clone()
            }
        };
        let ducts = r
            .ducts
            .iter()
            .map(|d| {
                let label = strings::render(&d.label);
                let words = if d.over {
                    strings::format(
                        key!("cabinet-route-duct-over"),
                        &[&label, &d.size, &d.percent.to_string(), &d.limit.to_string()],
                    )
                } else {
                    strings::format(
                        key!("cabinet-route-duct"),
                        &[&label, &d.size, &d.percent.to_string(), &d.inner_mm2.to_string()],
                    )
                };
                (None, words)
            })
            .collect();
        let lengths = r
            .lengths
            .iter()
            .map(|w| {
                let length = w.length.map(mm).unwrap_or_default();
                let words = match w.was {
                    Some(was) => format!("{} · {} → {length}", name(w), mm(was)),
                    None => format!("{} · {} · {length}", name(w), w.ends),
                };
                (Some(w.wire), words)
            })
            .collect();
        let typed = r
            .typed
            .iter()
            .map(|w| {
                let words = strings::format(
                    key!("cabinet-route-typed-row"),
                    &[
                        &name(w),
                        &w.typed.map(mm).unwrap_or_default(),
                        &w.length.map(mm).unwrap_or_default(),
                    ],
                );
                (Some(w.wire), words)
            })
            .collect();
        let site = r
            .site
            .iter()
            .map(|w| (Some(w.wire), format!("{} · {}", name(w), w.ends)))
            .collect();
        let without = r
            .without
            .iter()
            .map(|w| {
                let why = w
                    .reason
                    .as_ref()
                    .map(|(_, t)| strings::render(t))
                    .unwrap_or_default();
                (Some(w.wire), format!("{}: {why}", name(w)))
            })
            .collect();
        [
            (key!("cabinet-route-ducts"), ducts),
            (key!("cabinet-route-lengths"), lengths),
            (key!("cabinet-route-typed"), typed),
            (key!("cabinet-route-site-heading"), site),
            (key!("cabinet-route-without"), without),
        ]
        .into_iter()
        .filter(|(_, rows): &(Key<'_>, Vec<_>)| !rows.is_empty())
        .map(|(k, rows)| (strings::text(k).to_owned(), rows))
        .collect()
    }

    /// The tick's words: "Route every wire again (24 hold)".
    pub fn tick(&self) -> String {
        strings::format_count(count_key!("cabinet-route-all"), self.route.holding)
    }
}

/// The marks of an open cabinet card on its page: each way in page nm and whether it is the picked one; each duct's
/// chip at its page point with its words.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CabinetMarks {
    /// The ways: their points (page nm) and whether the row is picked.
    pub ways: Vec<(Vec<(i64, i64)>, bool)>,
    /// The ducts' chips: where (page nm) and their words ("9 %", "72 % · over 60 %").
    pub chips: Vec<((i64, i64), String)>,
}

impl ShellModel {
    /// Route on the panel page labelled `folio` that shows a cabinet (doc 12 §12.5): its card in the Preview tab, `all`
    /// planning every wire again; a cabinet with nothing to route says so in a toast.
    pub(super) fn cabinet_route_card(&mut self, folio: &str, all: bool) -> Outcome {
        let project = or_return!(super::gated_mut(&mut self.project));
        let route = match project.session.cabinet_route(folio, all) {
            Some(Ok((route, _))) => route,
            Some(Err(e)) => return super::tools::command_failed(&e, &Value::Null),
            None => return Outcome::refused(key!("reason-route-page")),
        };
        if !all && route.lengths.is_empty() && route.typed.is_empty() && route.without.is_empty() {
            let text = strings::format(key!("toast-cabinet-all-hold"), &[&route.cabinet]);
            self.notify(text, false);
            return Outcome::Done;
        }
        let ways = strings::format_count(
            count_key!("cabinet-route-ways"),
            route.lengths.len() + route.typed.len(),
        );
        let site = strings::format_count(count_key!("cabinet-route-site"), route.site.len());
        let lead = strings::format(key!("cabinet-route-lead"), &[&route.cabinet, &ways, &site]);
        let params = route.command.params.clone();
        self.preview_host_card_as(
            "layout.route",
            params,
            key!("reason-route"),
            CardOrigin::Cabinet(Box::new(CabinetCard {
                route,
                folio: folio.to_owned(),
                lead,
                picked: None,
            })),
        )
    }

    /// The card's tick and its rows (`layout.route_card {all?, wire?}`): `all` plans every wire again or the due ones
    /// only; `wire` picks a row to show its way alone (the picked one again shows them all).
    pub(crate) fn act_layout_route_card(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(card) = self.props.card.as_mut().and_then(|c| match &mut c.origin {
            CardOrigin::Cabinet(c) => Some(c),
            _ => None,
        }) else {
            return Outcome::refused(key!("reason-route-card"));
        };
        if let Some(wire) = p
            .get("wire")
            .and_then(Value::as_str)
            .and_then(|w| Uuid::parse_str(w).ok())
        {
            card.picked = (card.picked != Some(wire)).then_some(wire);
            return Outcome::Done;
        }
        let all = p.get("all").and_then(Value::as_bool).unwrap_or(!card.route.all);
        let folio = card.folio.clone();
        self.cabinet_route_card(&folio, all)
    }

    /// Apply on a cabinet card: the plan is stored in one step, and the toast says what it routed.
    pub(super) fn apply_cabinet_card(&mut self, card: Card) -> Outcome {
        let CardOrigin::Cabinet(c) = &card.origin else {
            return Outcome::Done;
        };
        let routed = c.route.lengths.len() + c.route.typed.len();
        let cabinet = c.route.cabinet.clone();
        let out = self.apply_bulk_card(card);
        if out == Outcome::Done {
            let text = strings::format_counted(count_key!("toast-cabinet-routed"), routed, &[&cabinet]);
            self.notify(text, true);
        }
        out
    }

    /// The stored ways of the wires at the placements selected on the panel page of canvas `group` (doc 12 §12.5), each
    /// with whether it no longer holds; none while a cabinet card is open.
    pub fn selected_ways(&self, group: usize) -> Vec<(Vec<(i64, i64)>, bool)> {
        if self
            .props
            .card
            .as_ref()
            .is_some_and(|c| matches!(c.origin, CardOrigin::Cabinet(_)))
        {
            return Vec::new();
        }
        let (Some(canvas), Some(folio)) = (self.canvas_of(group), self.current_folio()) else {
            return Vec::new();
        };
        let page = canvas.page();
        let placements: Vec<Uuid> = self
            .tools
            .selection
            .items
            .iter()
            .filter(|e| {
                crate::tools::page_entity_kind(page, **e) == Some(crate::tools::EntityKind::Placement)
            })
            .map(|e| e.0)
            .collect();
        if placements.is_empty() {
            return Vec::new();
        }
        self.project_session()
            .map(|s| s.ways_at(folio, &placements))
            .unwrap_or_default()
    }

    /// The wire of the `i`-th way of an open cabinet card (the order of [`CabinetMarks::ways`]).
    pub fn cabinet_way_wire(&self, group: usize, i: usize) -> Option<Uuid> {
        let _ = group;
        match &self.props.card.as_ref()?.origin {
            CardOrigin::Cabinet(c) => c.route.ways.get(i).map(|w| w.wire),
            _ => None,
        }
    }

    /// The marks an open cabinet card draws on the canvas of `group` while it shows the card's page and the card is
    /// current.
    pub fn cabinet_marks(&self, group: usize) -> CabinetMarks {
        let Some((card, c)) = self
            .props
            .card
            .as_ref()
            .filter(|c| self.revision() == Some(c.preview.base_rev))
            .and_then(|card| match &card.origin {
                CardOrigin::Cabinet(c) => Some((card, c)),
                _ => None,
            })
        else {
            return CabinetMarks::default();
        };
        let _ = card;
        if self.canvas_of(group).is_none() || self.current_folio() != Some(c.folio.as_str()) {
            return CabinetMarks::default();
        }
        CabinetMarks {
            ways: c
                .route
                .ways
                .iter()
                .map(|w| (w.page.clone(), c.picked == Some(w.wire)))
                .collect(),
            chips: c
                .route
                .ducts
                .iter()
                .map(|d| {
                    let words = if d.over {
                        strings::format(
                            key!("cabinet-route-chip-over"),
                            &[&d.percent.to_string(), &d.limit.to_string()],
                        )
                    } else {
                        strings::format(key!("cabinet-route-chip"), &[&d.percent.to_string()])
                    };
                    (d.chip, words)
                })
                .collect(),
        }
    }
}
