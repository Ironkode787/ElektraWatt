// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrange and route (docs/design/06a-drafting.md §7.1, §7.3): the plan of the selection or the page
//! (`Session::arrange_plan`) always opens a preview card first. The card lists the symbols that move (the far moves
//! first), the wires the person drew that get a new route, and what is left as it is; unticking a row plans again with
//! that symbol where it stands, or that wire as it runs. The canvas shows the page as the plan would leave it, the
//! moved symbols tinted with their old places outlined (`ShellModel::arrange_page`, `ShellModel::arrange_moves`); the
//! window shows the page as it is while B is held. Apply stores the plan's one `layout.apply`: one undo step.
//!
//! **Reads.** One `Session::arrange_plan` and one `Session::preview_page` when the card opens and on each tick; the
//! canvas reads the card's page, nothing per frame.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ew_api::output::Uuid;
use ew_api::routing::{InTheWay, RoutePlan, RouteScope, Side, StripHeld};
use serde_json::{Map, Value};

use super::{Outcome, ShellModel};
use crate::canvas::CanvasPage;
use crate::notifications::{Note, NoteTone};
use crate::properties::{ArrangeCard, ArrangeRow, Card, CardOrigin};
use crate::strings;
use crate::{count_key, key};

/// Gives each forked page of a card its own revision number for the window's tile cache: counted down from the top
/// of the range, which no revision of a project reaches.
static FORKS: AtomicU64 = AtomicU64::new(0);

impl ShellModel {
    /// `layout.arrange` (Arrange and route): the plan of the selected symbols and wires, or with nothing of the page
    /// selected of the page, as a preview card; nothing changes until Apply. Nothing to arrange: a toast says so.
    pub(crate) fn act_layout_arrange(&mut self) -> Outcome {
        let scope = self.route_selection_ids();
        let fitted = self.canvas().is_some_and(|c| c.is_fitted());
        let out = self.arrange_with(scope, Vec::new(), Vec::new(), Vec::new());
        self.tools.refit = self
            .area
            .as_ref()
            .map(|a| a.focused)
            .filter(|_| fitted && self.arrange_card().is_some());
        // The card's proposal is the whole page: it is fitted, and so fits again into what the Preview pane leaves.
        if self.arrange_card().is_some()
            && let Some(g) = self.area.as_ref().map(|a| a.focused)
            && let Some(c) = self.canvas_mut(g)
        {
            c.fit_again();
        }
        out
    }

    /// Plans Arrange and route over `scope` (empty: the page) with the rows `unticked` (symbols and wires by ID) and
    /// the wires `unlisted` kept as they stand, and opens its card; `listed` are the symbols the card listed before,
    /// which stay listed unticked. Nothing left to apply after an untick: the card closes and a toast says that.
    fn arrange_with(
        &mut self,
        scope: Vec<Uuid>,
        unticked: Vec<Uuid>,
        unlisted: Vec<Uuid>,
        listed: Vec<ArrangeRow>,
    ) -> Outcome {
        let Some(folio) = self.current_folio().map(str::to_owned) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let range = if scope.is_empty() {
            RouteScope::Page
        } else {
            RouteScope::Selection(scope.clone())
        };
        let kept: Vec<Uuid> = unticked.iter().chain(&unlisted).copied().collect();
        let Some(plan) = session.arrange_plan(&folio, &range, &kept) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let Some(params) = plan.params.clone() else {
            self.props.card = None;
            let why = in_the_way(session, &folio, &plan);
            if !unticked.is_empty() {
                self.notify(strings::text(key!("toast-arrange-none-left")).to_owned(), false);
            } else if why.is_empty() {
                self.notify_toned(
                    strings::text(key!("toast-arrange-nothing")).to_owned(),
                    NoteTone::Info,
                );
            } else {
                // A result toast (doc 16a §6.10): "Nothing to arrange" its title, what stands in the way its line. Where
                // several things do, the toast shows the first and how many more; its tooltip and the log show all.
                let text = strings::format(key!("arrange-blocked"), &[&why.join(" ")]);
                let brief = (why.len() > 1).then(|| {
                    let more = strings::format_count(count_key!("arrange-blocked-more"), why.len() - 1);
                    strings::format(key!("arrange-blocked"), &[&format!("{} {more}", why[0])])
                });
                self.notes.push(Note {
                    brief,
                    ..Note::new("edit", text).toned(NoteTone::Info)
                });
            }
            return Outcome::Done;
        };
        let card = arrange_card(session, &folio, scope, &plan, &unticked, &listed);
        let out = self.preview_host_card_as(
            "layout.apply",
            params,
            key!("reason-arrange"),
            CardOrigin::Arrange(Box::new(card)),
        );
        self.tools.arranged = self.forked_page(&folio);
        out
    }

    /// The page the open card's preview would leave, as the canvas shows it.
    fn forked_page(&self, folio: &str) -> Option<Arc<CanvasPage>> {
        let card = self.props.card.as_ref()?;
        let preview = card.preview.result.as_ref().ok()?;
        let session = &self.project.as_ref()?.session;
        let shown = self.canvas()?.page();
        let page = session.preview_page(preview, folio)?;
        let fork = FORKS.fetch_add(1, Ordering::Relaxed);
        let mut out = CanvasPage::new(
            u64::MAX - fork,
            folio,
            &shown.title,
            (page.width.0, page.height.0),
            Arc::new(page.content),
            shown.grid,
        );
        out.session = shown.session;
        Some(Arc::new(out))
    }

    /// The open Arrange and route card, while it is current.
    fn arrange_card(&self) -> Option<&ArrangeCard> {
        let card = self.props.card.as_ref()?;
        let CardOrigin::Arrange(a) = &card.origin else {
            return None;
        };
        (self.revision() == Some(card.preview.base_rev)).then_some(a.as_ref())
    }

    /// The page as an open Arrange and route card would leave it, on the canvas of `group` when that shows the card's
    /// page: the window paints it in place of the page (but while B is held).
    pub fn arrange_page(&self, group: usize) -> Option<Arc<CanvasPage>> {
        let card = self.arrange_card()?;
        let canvas = self.canvas_of(group)?;
        (canvas.page().folio == card.folio).then(|| self.tools.arranged.clone())?
    }

    /// The symbols an open Arrange and route card moves on the canvas of `group`, each as its body where it stands and
    /// where it goes, in sheet nm `[x0, y0, x1, y1]`: the window tints the new place, outlines the old one faintly and
    /// joins them by a dotted line.
    pub fn arrange_moves(&self, group: usize) -> Vec<([i64; 4], [i64; 4])> {
        let (Some(card), Some(canvas)) = (self.arrange_card(), self.canvas_of(group)) else {
            return Vec::new();
        };
        let page = canvas.page();
        if page.folio != card.folio {
            return Vec::new();
        }
        let sheet = |r: [i64; 4]| {
            let (a, b) = (
                page.snapper.page_to_sheet((r[0], r[1])),
                page.snapper.page_to_sheet((r[2], r[3])),
            );
            [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
        };
        card.shifts.iter().map(|(a, b)| (sheet(*a), sheet(*b))).collect()
    }

    /// `layout.arrange_tick {row?, all?}`: ticks or unticks row `row` of the open card's symbols and wires (in that
    /// order), or with `all` ticks (`true`) or unticks (`false`) every row of the wires you drew, and plans again with
    /// every unticked one kept as it stands. A plan made again brings in no wire you drew that the card did not list:
    /// those keep their routes too, so that an unticked row is the last word on your wires (untick all: none of them
    /// changes). Refused without an Arrange and route card or with neither parameter.
    pub(crate) fn act_layout_arrange_tick(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(card) = self.props.card.as_ref().and_then(|c| match &c.origin {
            CardOrigin::Arrange(a) => Some(a.as_ref().clone()),
            _ => None,
        }) else {
            return Outcome::refused(key!("reason-arrange-tick"));
        };
        let rows: Vec<&ArrangeRow> = card.parts.iter().chain(&card.wires).collect();
        let mut unticked: Vec<Uuid> = rows.iter().filter(|r| !r.ticked).map(|r| r.id).collect();
        if let Some(all) = p.get("all").and_then(Value::as_bool) {
            unticked.retain(|id| !card.wires.iter().any(|w| w.id == *id));
            if !all {
                unticked.extend(card.wires.iter().map(|w| w.id));
            }
        } else {
            let row = p
                .get("row")
                .and_then(Value::as_u64)
                .map_or(usize::MAX, |r| usize::try_from(r).unwrap_or(usize::MAX));
            let Some(hit) = rows.get(row) else {
                return Outcome::refused(key!("reason-arrange-tick"));
            };
            if hit.ticked {
                unticked.push(hit.id);
            } else {
                unticked.retain(|id| *id != hit.id);
            }
        }
        let listed = card.parts.clone();
        let unlisted: Vec<Uuid> = self
            .project
            .as_ref()
            .and_then(|p| p.session.page_draft(&card.folio))
            .map(|d| {
                d.wires
                    .iter()
                    .filter(|w| !w.computed && !w.route.is_empty())
                    .map(|w| w.id.0)
                    .filter(|id| !card.wires.iter().any(|r| r.id == *id))
                    .collect()
            })
            .unwrap_or_default();
        self.arrange_with(card.scope, unticked, unlisted, listed)
    }

    /// `layout.arrange_as_is`: what B does while an Arrange and route card is open (the window shows the page as it is
    /// while the key is held); it changes nothing.
    pub(crate) fn act_layout_arrange_as_is(&mut self) -> Outcome {
        Outcome::Done
    }

    /// Whether B shows the page as it is: an Arrange and route card is open.
    pub(crate) fn arrange_shown(&self) -> bool {
        self.arrange_card().is_some()
    }

    /// Apply on an Arrange and route card: the plan's `layout.apply` commits as a bulk card's command does, and the
    /// toast says what moved and what was routed.
    pub(super) fn apply_arrange_card(&mut self, card: Card) -> Outcome {
        let toast = match &card.origin {
            CardOrigin::Arrange(a) => strings::format(
                key!("toast-arrange-applied"),
                &[
                    &a.page,
                    &strings::format_count(count_key!("arrange-moved"), a.moved),
                    &strings::format_count(count_key!("arrange-routed"), a.routed),
                ],
            ),
            _ => String::new(),
        };
        let fresh = self.revision() == Some(card.preview.base_rev) && card.preview.result.is_ok();
        let out = self.apply_bulk_card(card);
        if fresh && out == Outcome::Done {
            self.notify(toast, true);
        }
        out
    }
}

/// The words of a move by `d` ticks: "6 steps right, 31 steps up".
fn move_words(d: ew_api::routing::Pt) -> String {
    let step = ew_api::routing::TICKS_PER_STEP;
    let (x, y) = (i64::from(d.x / step), i64::from(d.y / step));
    let mut parts = Vec::new();
    let n = |v: i64| usize::try_from(v.unsigned_abs()).unwrap_or(usize::MAX);
    if x > 0 {
        parts.push(strings::format_count(count_key!("arrange-right"), n(x)));
    } else if x < 0 {
        parts.push(strings::format_count(count_key!("arrange-left"), n(x)));
    }
    if y < 0 {
        parts.push(strings::format_count(count_key!("arrange-up"), n(y)));
    } else if y > 0 {
        parts.push(strings::format_count(count_key!("arrange-down"), n(y)));
    }
    parts.join(", ")
}

/// The words of a symbol left against a rule (doc 06a §4.7): what was in the way of its place on `side` of its host's
/// pin, `name` naming a symbol by its ID.
fn left_words(side: Side, why: InTheWay, name: impl Fn(Uuid) -> String) -> String {
    let place = strings::text(match side {
        Side::Up => key!("arrange-place-up"),
        Side::Down => key!("arrange-place-down"),
        Side::Left => key!("arrange-place-left"),
        Side::Right => key!("arrange-place-right"),
    });
    match why {
        InTheWay::Room => strings::format(key!("arrange-in-way-room"), &[place]),
        InTheWay::Part(p) => strings::format(key!("arrange-in-way-part"), &[place, &name(p.0)]),
        InTheWay::Edge => strings::format(key!("arrange-in-way-edge"), &[place]),
        InTheWay::Text(Some(p)) => strings::format(key!("arrange-in-way-text-of"), &[place, &name(p.0)]),
        InTheWay::Text(None) => strings::format(key!("arrange-in-way-text"), &[place]),
        InTheWay::Wire => strings::format(key!("arrange-in-way-wire"), &[place]),
        InTheWay::Texts => strings::format(key!("arrange-in-way-texts"), &[place]),
        InTheWay::Crossings => strings::format(key!("arrange-in-way-crossings"), &[place]),
        InTheWay::NoGain => strings::format(key!("arrange-in-way-no-gain"), &[place]),
    }
}

/// What stands in the way of a plan that changes nothing (doc 06a §4.9), one sentence each: the symbols left against a
/// rule, the strips of terminals that did not open to their card's channels, the wires without a way (with what is in it,
/// where the router found that), and the faults the page keeps; empty for a page that is in order.
fn in_the_way(session: &ew_api::Session, folio: &str, plan: &RoutePlan) -> Vec<String> {
    let name = |id: Uuid| {
        session
            .symbol_name(id)
            .unwrap_or_else(|| strings::text(key!("arrange-a-symbol")).to_owned())
    };
    let mut out: Vec<String> = plan
        .left
        .iter()
        .map(|(id, side, why)| {
            strings::format(
                key!("arrange-blocked-symbol"),
                &[&name(id.0), &left_words(*side, *why, name)],
            )
        })
        .collect();
    // The strips of terminals that did not open to their card's channels, with what held each (doc 06a §4.4).
    for (first, why) in &plan.held {
        let strip = name(first.0);
        out.push(match why {
            StripHeld::Order => strings::format(key!("arrange-held-order"), &[&strip]),
            StripHeld::Room => strings::format(key!("arrange-held-room"), &[&strip]),
            StripHeld::Part(p) => strings::format(key!("arrange-held-part"), &[&strip, &name(p.0)]),
            StripHeld::Sheet => strings::format(key!("arrange-held-sheet"), &[&strip]),
            StripHeld::Worse => strings::format(key!("arrange-held-worse"), &[&strip]),
        });
    }
    out.extend(super::wiring::route_rows(session, folio, plan));
    for w in &plan.plan.after.findings.unrouted {
        if !plan.plan.unrouted.iter().any(|(x, _)| x == w) {
            let wire = session
                .connection_words(folio, w.0)
                .map(|t| super::tools::rejected_words("", Some(&t)))
                .unwrap_or_default();
            out.push(strings::format(key!("arrange-blocked-unrouted"), &[&wire]));
        }
    }
    let faults = plan.plan.after.faults;
    if faults > 0 {
        out.push(strings::format_count(
            count_key!("arrange-blocked-faults"),
            faults,
        ));
    }
    out
}

/// The card of `plan` on page `folio` (module documentation).
fn arrange_card(
    session: &ew_api::Session,
    folio: &str,
    scope: Vec<Uuid>,
    plan: &RoutePlan,
    unticked: &[Uuid],
    listed: &[ArrangeRow],
) -> ArrangeCard {
    let name = |id: Uuid| {
        session
            .symbol_name(id)
            .unwrap_or_else(|| strings::text(key!("arrange-a-symbol")).to_owned())
    };
    let wire = |id: Uuid| {
        session
            .connection_words(folio, id)
            .map(|t| super::tools::rejected_words("", Some(&t)))
            .unwrap_or_default()
    };
    let area = session.page_draft(folio).map(|d| d.area).unwrap_or_default();
    let far = |d: ew_api::routing::Pt| 3 * d.x.abs() > area.w() || 3 * d.y.abs() > area.h();
    // The symbols that move, the far ones first, then those unticked.
    let mut parts: Vec<(bool, String, ArrangeRow)> = plan
        .moves
        .iter()
        .map(|(id, d)| {
            let row = ArrangeRow {
                id: id.0,
                name: name(id.0),
                words: move_words(*d),
                ticked: true,
                kept: false,
            };
            (!far(*d), row.name.clone(), row)
        })
        .collect();
    parts.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| super::natural_cmp(&a.1, &b.1))
            .then_with(|| a.2.id.cmp(&b.2.id))
    });
    let mut parts: Vec<ArrangeRow> = parts.into_iter().map(|(_, _, r)| r).collect();
    for r in listed.iter().filter(|r| unticked.contains(&r.id)) {
        parts.push(ArrangeRow {
            ticked: false,
            words: strings::text(key!("arrange-unticked")).to_owned(),
            ..r.clone()
        });
    }
    // The wires the person drew that get a new route, then those unticked.
    let mut wires: Vec<ArrangeRow> = plan
        .plan
        .his
        .iter()
        .map(|w| ArrangeRow {
            id: w.0,
            name: wire(w.0),
            words: strings::text(key!("arrange-his")).to_owned(),
            ticked: true,
            kept: false,
        })
        .collect();
    let tables = session.page_tables(folio);
    for id in unticked {
        if tables
            .as_ref()
            .is_some_and(|t| t.connections.iter().any(|c| c.id == *id))
        {
            wires.push(ArrangeRow {
                id: *id,
                name: wire(*id),
                words: strings::text(key!("arrange-his-unticked")).to_owned(),
                ticked: false,
                kept: false,
            });
        }
    }
    // Left as it is: the symbols kept in place, those without room, the wires without a way.
    let mut left: Vec<ArrangeRow> = Vec::new();
    if let Some(t) = &tables {
        for r in t
            .reps
            .iter()
            .filter(|r| r.locked && (scope.is_empty() || scope.contains(&r.id)))
        {
            left.push(ArrangeRow {
                id: r.id,
                name: name(r.id),
                words: strings::text(key!("arrange-kept")).to_owned(),
                ticked: false,
                kept: true,
            });
        }
    }
    for (id, side, why) in &plan.left {
        left.push(ArrangeRow {
            id: id.0,
            name: name(id.0),
            words: left_words(*side, *why, name),
            ticked: false,
            kept: false,
        });
    }
    let mut route = plan.clone();
    route.plan.his.clear();
    let ways = super::wiring::route_rows(session, folio, &route);
    // The bodies of the moved symbols where they stand and where they go, in page nm.
    let shifts = tables
        .as_ref()
        .map(|t| {
            plan.moves
                .iter()
                .filter_map(|(id, d)| {
                    let r = t.reps.iter().find(|r| r.id == id.0)?;
                    let (dx, dy) = ew_api::routing::Ticks::new(t.grid).move_nm(*d);
                    let b = r.body;
                    Some((b, [b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy]))
                })
                .collect()
        })
        .unwrap_or_default();
    let page = strings::format(key!("route-folio"), &[folio]);
    let moved = plan.moves.len();
    let across = plan.moves.iter().filter(|(_, d)| far(*d)).count();
    let mut symbols = strings::format_count(count_key!("arrange-symbols"), moved);
    if across > 0 {
        symbols = strings::format(key!("arrange-across"), &[&symbols, &across.to_string()]);
    }
    let routed = plan.plan.routes.iter().filter(|(_, r)| !r.is_empty()).count();
    let mut routes = strings::format_count(count_key!("arrange-wires"), routed);
    // The wires the person drew that get a new route, counted in the lead.
    let yours = plan.plan.his.len();
    if yours > 0 {
        routes = strings::format(key!("arrange-yours"), &[&routes, &yours.to_string()]);
    }
    let lead = strings::format(key!("arrange-lead"), &[&page, &symbols, &routes]);
    let figures = |q: &ew_api::routing::Quality| {
        strings::format(
            key!("arrange-figures"),
            &[
                &strings::format_count(count_key!("arrange-crossings"), q.counts.crossings),
                &strings::format_count(count_key!("arrange-bends"), q.counts.bends),
                &strings::format_count(count_key!("arrange-straight"), q.counts.straight),
                &strings::format_count(count_key!("arrange-through"), q.counts.across_texts),
                &strings::format_count(count_key!("arrange-faults"), q.faults),
            ],
        )
    };
    ArrangeCard {
        folio: folio.to_owned(),
        page,
        scope,
        lead,
        now: strings::format(key!("arrange-now"), &[&figures(&plan.plan.before)]),
        then: strings::format(key!("arrange-then"), &[&figures(&plan.plan.after)]),
        parts,
        wires,
        left,
        ways,
        shifts,
        moved,
        routed,
    }
}
