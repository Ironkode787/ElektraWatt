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
use crate::properties::{Card, CardColumns, CardGroup, CardGroupRow, CardOrigin, CardTick};
use crate::strings::{self, Key};
use crate::{count_key, key};

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
    /// The card's groups as the design lays them out (doc 12-3d §12.5): Ducts; Lengths; Typed by hand, Made on site,
    /// Without a way and the tick Route every wire again. A row of a wire picks it, which shows its way alone.
    pub fn columns(&self) -> CardColumns {
        let r = &self.route;
        let mm = |n: ew_units::Nm| format!("{} mm", super::layout::mm_text(i128::from(n.0), false));
        let name = |w: &ew_api::cabinet_route::CardWire| {
            if w.number.is_empty() {
                w.ends.clone()
            } else {
                w.number.clone()
            }
        };
        let row = |w: &ew_api::cabinet_route::CardWire, cells: Vec<String>| CardGroupRow {
            id: Some(w.wire),
            cells,
            note: None,
            picked: self.picked == Some(w.wire),
            action: Some((
                "layout.route_card",
                serde_json::json!({ "wire": w.wire.to_string() }),
            )),
        };
        let group = |heading: Key<'static>, rows: Vec<CardGroupRow>| CardGroup {
            heading: strings::text(heading).to_owned(),
            count: Some(rows.len()),
            rows,
            tick: None,
        };
        let ducts = r
            .ducts
            .iter()
            .map(|d| {
                let fill = chip_words(d.percent, d.limit, d.over);
                let wires = strings::format_counted(
                    count_key!("cabinet-route-duct-wires"),
                    d.wires,
                    &[&d.fullest.to_string()],
                );
                let mut note = format!(
                    "{} · {}",
                    strings::render(&d.inner),
                    strings::format(key!("cabinet-route-duct-limit"), &[&d.limit.to_string()])
                );
                for n in &d.notes {
                    note = format!("{note} · {}", strings::render(n));
                }
                CardGroupRow {
                    id: None,
                    cells: vec![strings::render(&d.label), d.size.clone(), wires, fill],
                    note: Some(note),
                    picked: false,
                    action: None,
                }
            })
            .collect();
        let lengths = r
            .lengths
            .iter()
            .map(|w| {
                let length = w.length.map(mm).unwrap_or_default();
                let length = match w.was {
                    Some(was) => format!("{} → {length}", mm(was)),
                    None => length,
                };
                row(w, vec![name(w), w.ends.clone(), length])
            })
            .collect();
        let typed = r
            .typed
            .iter()
            .map(|w| {
                let words = strings::format(
                    key!("cabinet-route-typed-row"),
                    &[
                        &w.typed.map(mm).unwrap_or_default(),
                        &w.length.map(mm).unwrap_or_default(),
                    ],
                );
                row(w, vec![name(w), words])
            })
            .collect();
        let site = r
            .site
            .iter()
            .map(|w| {
                let why = w
                    .site_end
                    .as_ref()
                    .map_or_else(|| w.ends.clone(), strings::render);
                row(w, vec![name(w), why])
            })
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
                row(w, vec![name(w), why])
            })
            .collect();
        let tick = CardTick {
            words: if r.all || r.holding == 0 {
                strings::text(key!("cabinet-route-all-on")).to_owned()
            } else {
                strings::format_count(count_key!("cabinet-route-all"), r.holding)
            },
            on: r.all,
            action: "layout.route_card",
            param: "all",
        };
        CardColumns(vec![
            vec![group(key!("cabinet-route-ducts"), ducts)],
            vec![group(key!("cabinet-route-lengths"), lengths)],
            vec![
                group(key!("cabinet-route-typed"), typed),
                group(key!("cabinet-route-site-heading"), site),
                group(key!("cabinet-route-without"), without),
                CardGroup {
                    heading: String::new(),
                    count: None,
                    rows: Vec::new(),
                    tick: Some(tick),
                },
            ],
        ])
    }
}

/// A duct's fill as its chip and its card row say it: "9 %", "72 % · over 60 %".
fn chip_words(percent: i128, limit: u8, over: bool) -> String {
    if over {
        strings::format(
            key!("cabinet-route-chip-over"),
            &[&percent.to_string(), &limit.to_string()],
        )
    } else {
        strings::format(key!("cabinet-route-chip"), &[&percent.to_string()])
    }
}

/// The marks of an open cabinet card on its page: each way in page nm and whether it is the picked one; each duct's
/// chip at its page point with its words, and the note of the wires that leave for the door where they leave the plate.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CabinetMarks {
    /// The ways: their points (page nm) and whether the row is picked.
    pub ways: Vec<(Vec<(i64, i64)>, bool)>,
    /// The ducts' chips: where (page nm) and their words ("9 %", "72 % · over 60 %").
    pub chips: Vec<((i64, i64), String)>,
    /// The notes beside the ways: where (page nm) and their words ("9 wires to the door").
    pub notes: Vec<((i64, i64), String)>,
}

/// What a selection's marks were worked out for: the revision, the page's folio and the selected IDs.
pub(super) type MarksKey = (u64, String, Vec<Uuid>);

/// What the selection shows on a panel page: the ways of its wires (page nm, and whether each no longer holds) and the
/// fullest pieces of its ducts.
#[derive(Debug)]
pub(super) struct SelectionMarks {
    ways: Vec<(Vec<(i64, i64)>, bool)>,
    ducts: Vec<DuctPieceMark>,
}

/// The fullest piece of a duct selected on its panel page (doc 12-3d §12.5): its corners (page nm), hatched, and its fill
/// chip where (page nm) and with its words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DuctPieceMark {
    /// The piece's two corners.
    pub piece: [(i64, i64); 2],
    /// The chip's place.
    pub chip: (i64, i64),
    /// The chip's words.
    pub words: String,
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
        // Nothing is due: the toast says so and the card opens with its tick set, every wire planned again.
        if !all && route.lengths.is_empty() && route.typed.is_empty() && route.without.is_empty() {
            let text = strings::format(key!("toast-cabinet-all-hold"), &[&route.cabinet]);
            self.notify(text, false);
            return self.cabinet_route_card(folio, true);
        }
        // One text with its arguments; the words for one wire made on site are a text of their own.
        let lead_key = if route.site.len() == 1 {
            count_key!("cabinet-route-lead-one-site")
        } else {
            count_key!("cabinet-route-lead")
        };
        let lead = strings::format_counted(
            lead_key,
            route.lengths.len() + route.typed.len(),
            &[&route.cabinet, &route.site.len().to_string()],
        );
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

    /// The stored ways of the wires at the placements selected on the panel page of canvas `group`, and of the wires
    /// selected (in the Connections table, by a Way row's Show on), each with whether it no longer holds (doc 12
    /// §12.5); none while a cabinet card is open.
    pub fn selected_ways(&self, group: usize) -> Vec<(Vec<(i64, i64)>, bool)> {
        if self
            .props
            .card
            .as_ref()
            .is_some_and(|c| matches!(c.origin, CardOrigin::Cabinet(_)))
        {
            return Vec::new();
        }
        self.selection_marks(group)
            .map(|m| m.ways.clone())
            .unwrap_or_default()
    }

    /// The fullest pieces of the ducts selected on the panel page of canvas `group`, hatched, with their fill chips
    /// (doc 12-3d §12.5).
    pub fn selected_duct_marks(&self, group: usize) -> Vec<DuctPieceMark> {
        self.selection_marks(group)
            .map(|m| m.ducts.clone())
            .unwrap_or_default()
    }

    /// What the selection shows on the panel page of canvas `group`, worked out once per revision, page and selection
    /// (the window asks for it on every frame).
    fn selection_marks(&self, group: usize) -> Option<std::sync::Arc<SelectionMarks>> {
        let (Some(_), Some(folio), Some(rev)) =
            (self.canvas_of(group), self.current_folio(), self.revision())
        else {
            return None;
        };
        let ids: Vec<Uuid> = self.tools.selection.items.iter().map(|e| e.0).collect();
        if ids.is_empty() {
            return None;
        }
        let key: MarksKey = (rev, folio.to_owned(), ids);
        if let Some((k, marks)) = &*self.marks_cache.borrow()
            && *k == key
        {
            return Some(marks.clone());
        }
        let session = self.project_session()?;
        let marks = std::sync::Arc::new(SelectionMarks {
            ways: session.ways_at(&key.1, &key.2),
            ducts: session
                .duct_marks(&key.1, &key.2)
                .into_iter()
                .map(|m| DuctPieceMark {
                    piece: m.piece,
                    chip: m.chip,
                    words: chip_words(m.percent, m.limit, m.over),
                })
                .collect(),
        });
        *self.marks_cache.borrow_mut() = Some((key, marks.clone()));
        Some(marks)
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
                .map(|d| (d.chip, chip_words(d.percent, d.limit, d.over)))
                .collect(),
            notes: door_note(&c.route.ways).into_iter().collect(),
        }
    }
}

/// The note of the wires a plan lays into the doors, where the first of them leaves the plate: "9 wires to the door".
fn door_note(ways: &[ew_api::cabinet_route::CardWay]) -> Option<((i64, i64), String)> {
    let at: Vec<(i64, i64)> = ways.iter().filter_map(|w| w.door_at).collect();
    let first = *at.first()?;
    Some((
        first,
        strings::format_count(count_key!("cabinet-route-to-door"), at.len()),
    ))
}
