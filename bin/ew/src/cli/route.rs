// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew route [<project-dir>] [--page <folio>] [--selection <ids>] [--arrange] [--dry-run] [--json]`: Route over the
//! CLI (docs/design/06a-drafting.md §7.4), and with `--arrange` Arrange and route. Each page's plan
//! ([`ew_api::Session::route_plan`], [`ew_api::Session::arrange_plan`]) is stored as one `layout.apply`, the command
//! the window's buttons store, and the project is saved once; `--dry-run` prints the plan. On a panel page that shows a
//! cabinet's plate (doc 12 §8.5) it routes the cabinet: its plan ([`ew_api::Session::cabinet_route`]) is stored as
//! one `layout.route`.

use ew_api::AnyCommand;
use ew_api::cli_results as results;
use ew_api::drafting::{InTheWay, Reason, Side};
use ew_api::routing::{RoutePlan, RouteScope};

use super::io::{Io, out};
use super::tree::Leaf;
use super::{RouteArgs, open_for_writing, open_reader, project_dir, reader_dir};
use crate::catalogue::{Exit, exit};

impl Leaf for RouteArgs {
    const EXITS: &'static [Exit] = &[
        exit(0, "every wire it routes is routed"),
        exit(1, "a wire stays unrouted"),
        exit(
            2,
            "usage, open, an unknown folio, a refused command and save errors",
        ),
    ];
    const RESULT: Option<&'static str> = Some(results::RouteResult::SCHEMA);
}

/// The reason's key in the JSON result and its sentence in the text.
fn reason(why: Reason, wire: &str) -> (&'static str, String) {
    match why {
        Reason::Wires => ("wires", format!("No way for {wire}: other wires are in its way.")),
        Reason::Parts => ("symbols", format!("No way for {wire}: symbols are in its way.")),
        Reason::Points => (
            "points",
            format!("No way for {wire}: connection points of other wires are in its way."),
        ),
        Reason::FourArms => (
            "four_arms",
            format!("No way for {wire}: it would be a fourth arm at a junction."),
        ),
    }
}

/// `ew route`: routes each page and saves (a reader with `--dry-run`). Exit 0 when no wire it routes is left without
/// a way, 1 when one is, 2 on usage (a `--selection` item that is no record ID), open, an unknown folio, a refused
/// command and save errors. A selection ID that names nothing on a page routes nothing there.
pub(super) fn run_route(a: &RouteArgs, io: &mut Io<'_>) -> i32 {
    let mut selection = Vec::with_capacity(a.selection.len());
    for s in &a.selection {
        match ew_api::output::Uuid::parse_str(s.trim()) {
            Ok(id) => selection.push(id),
            Err(_) => {
                io.error(format_args!("`{s}` in --selection is not a record ID"));
                return 2;
            }
        }
    }
    let opened = if a.dry_run {
        let Some(dir) = reader_dir(a.project.as_ref(), io) else {
            return 2;
        };
        open_reader(&dir, "route", io).map(|(s, _)| s).map_err(|e| {
            io.error(format_args!("{}: {e}", dir.display()));
            2
        })
    } else {
        let Some(dir) = project_dir(a.project.as_ref(), io) else {
            return 2;
        };
        let origin = ew_api::Origin::Cli {
            subcommand: "route".to_owned(),
            confirmed: false,
        };
        open_for_writing(&dir, origin, io)
    };
    let mut session = match opened {
        Ok(session) => session,
        Err(code) => return code,
    };
    let folios = match &a.page {
        Some(f) if session.cabinet_of_folio(f).is_some() => {
            return route_cabinet(session, f, a, io);
        }
        Some(f) => {
            if session.page_draft(f).is_none() {
                io.error(format_args!(
                    "the page order has no drawn page with the folio `{f}`"
                ));
                return 2;
            }
            vec![f.clone()]
        }
        None => session.schematic_folios(),
    };
    let scope = if selection.is_empty() {
        RouteScope::Page
    } else {
        RouteScope::Selection(selection)
    };
    let mut pages = Vec::new();
    let mut text = String::new();
    let mut changed = false;
    for folio in folios {
        let planned = if a.arrange {
            session.arrange_plan(&folio, &scope, &[])
        } else {
            session.route_plan(&folio, &scope)
        };
        let Some(plan) = planned else {
            continue;
        };
        let words = |id| {
            session
                .connection_words(&folio, id)
                .map(|t| t.to_string())
                .unwrap_or_default()
        };
        let mut page = page_result(&folio, &plan, words);
        let name = |id| session.symbol_name(id).unwrap_or_else(|| "a symbol".to_owned());
        page.moved = plan.moves.iter().map(|(p, _)| name(p.0)).collect();
        page.left = plan.left.iter().map(|(p, _, _)| name(p.0)).collect();
        let left: Vec<String> = plan
            .left
            .iter()
            .map(|(p, side, why)| format!("{}: {}", name(p.0), left_words(*side, *why, name)))
            .collect();
        text.push_str(&page_text(&page, &plan, &left, a.dry_run));
        if !a.dry_run
            && let Some(params) = plan.params.clone()
        {
            let cmd = AnyCommand {
                name: "layout.apply".to_owned(),
                params,
            };
            if let Err(e) = session.execute(&cmd) {
                io.error(format_args!("folio {folio}: {e}; nothing was saved"));
                return 2;
            }
            changed = true;
        }
        pages.push(page);
    }
    let saved = changed && !a.dry_run;
    if saved && let Err(e) = session.save() {
        io.error(format_args!("{e}; nothing was saved"));
        return 2;
    }
    session.close();
    let unrouted = pages.iter().any(|p| !p.unrouted.is_empty());
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::RouteResult {
                schema: results::RouteResult::SCHEMA.to_owned(),
                pages,
                saved,
            })
        );
    } else {
        out!(io, "{text}");
    }
    i32::from(unrouted)
}

/// `ew route --page <folio>` on a panel page: the cabinet's plan stored as one `layout.route` and saved (printed with
/// `--dry-run`). Exit 0 when every wire of the cabinet Route lays has a way, 1 when one has none, 2 on a refused command
/// and save errors.
fn route_cabinet(mut session: ew_api::Session, folio: &str, a: &RouteArgs, io: &mut Io<'_>) -> i32 {
    let (card, preview) = match session.cabinet_route(folio, false) {
        Some(Ok(planned)) => planned,
        Some(Err(e)) => {
            io.error(format_args!("folio {folio}: {e}; nothing was saved"));
            return 2;
        }
        None => return 2,
    };
    let name = |w: &ew_api::cabinet_route::CardWire| {
        if w.number.is_empty() {
            w.ends.clone()
        } else {
            w.number.clone()
        }
    };
    let routed: Vec<String> = card.lengths.iter().chain(&card.typed).map(name).collect();
    let unrouted: Vec<results::RouteUnrouted> = card
        .without
        .iter()
        .map(|w| results::RouteUnrouted {
            wire: name(w),
            reason: w.reason_key().unwrap_or("no_duct").to_owned(),
        })
        .collect();
    let verb = if a.dry_run { "Would route" } else { "Routed" };
    let wires = |n: usize| {
        if n == 1 {
            "1 wire".to_owned()
        } else {
            format!("{n} wires")
        }
    };
    // The lead says what Route does; "every wire has its way" only where no wire is left without one.
    let mut text = if !routed.is_empty() {
        format!(
            "{verb} {} of {} on folio {folio}.
",
            wires(routed.len()),
            card.cabinet
        )
    } else if card.without.is_empty() {
        format!(
            "Every wire of {} has its way.
",
            card.cabinet
        )
    } else {
        format!(
            "No wire of {} gets a way on folio {folio}; {} without one:
",
            card.cabinet,
            wires(card.without.len())
        )
    };
    for w in &card.without {
        let why = w.reason.as_ref().map(|(_, t)| t.to_string()).unwrap_or_default();
        text.push_str(&format!(
            "  No way for {}: {why}.
",
            name(w)
        ));
    }
    // The ways of wires the cabinet no longer lays go with this run, whatever else it does.
    if let Some(words) = card.gone_words() {
        // In English, as the command line prints, by the text's own catalogue.
        let words = words
            .home()
            .map_or_else(|| words.to_string(), |c| c.text_for(&words, None));
        text.push_str(&format!("{words}.\n"));
    }
    let mut saved = false;
    if !a.dry_run && (!routed.is_empty() || card.gone > 0) {
        if let Err(e) = session.commit(preview) {
            io.error(format_args!("folio {folio}: {e}; nothing was saved"));
            return 2;
        }
        if let Err(e) = session.save() {
            io.error(format_args!("{e}; nothing was saved"));
            return 2;
        }
        saved = true;
    }
    session.close();
    let unrouted_any = !unrouted.is_empty();
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::RouteResult {
                schema: results::RouteResult::SCHEMA.to_owned(),
                pages: vec![results::RoutePageResult {
                    folio: folio.to_owned(),
                    routed,
                    unrouted,
                    across_texts: Vec::new(),
                    moved: Vec::new(),
                    left: Vec::new(),
                }],
                saved,
            })
        );
    } else {
        out!(io, "{text}");
    }
    i32::from(unrouted_any)
}

/// One page's result: the wires the plan changes and those it left without a way with their reasons, named by `words`.
fn page_result(
    folio: &str,
    plan: &RoutePlan,
    words: impl Fn(ew_api::output::Uuid) -> String,
) -> results::RoutePageResult {
    let p = &plan.plan;
    let changed: Vec<ew_api::output::Uuid> = plan.changed();
    let across: std::collections::BTreeSet<ew_api::output::Uuid> = p
        .after
        .findings
        .across_texts
        .iter()
        .map(|(w, _, _)| w.0)
        // (And a wire the plan leaves as it was that a text now lies across: a text that had to step aside.)
        .filter(|w| changed.contains(w) || !p.before.findings.across_texts.iter().any(|(b, _, _)| b.0 == *w))
        .collect();
    results::RoutePageResult {
        folio: folio.to_owned(),
        routed: changed.iter().map(|id| words(*id)).collect(),
        unrouted: p
            .unrouted
            .iter()
            .map(|(id, why)| {
                let wire = words(id.0);
                results::RouteUnrouted {
                    reason: reason(*why, &wire).0.to_owned(),
                    wire,
                }
            })
            .collect(),
        across_texts: across.into_iter().map(words).collect(),
        moved: Vec::new(),
        left: Vec::new(),
    }
}

/// The text of one page: its summary line (the page as `folio <label>`, as the window words it; with `--dry-run` what
/// Route would do), then one line per symbol left against a rule (`left`, each with what was in its way), per wire
/// left without a way and per wire across a text, and the number of wires a page-wide Route leaves as the person's.
fn page_text(page: &results::RoutePageResult, plan: &RoutePlan, left: &[String], dry_run: bool) -> String {
    let (routed, unrouted, kept) = (page.routed.len(), page.unrouted.len(), plan.kept.len());
    let moved = page.moved.len();
    let verb = if dry_run { "Would route" } else { "Routed" };
    let wires = |n: usize| {
        if n == 1 {
            "1 wire".to_owned()
        } else {
            format!("{n} wires")
        }
    };
    let mut out = match (moved, routed, unrouted, kept) {
        (0, 0, 0, 0) => format!(
            "Nothing to route on folio {}: no wire would run better.\n",
            page.folio
        ),
        (0, 0, 0, _) => format!("Nothing changed on folio {}.\n", page.folio),
        (0, n, _, _) => format!("{verb} {} on folio {}.\n", wires(n), page.folio),
        (m, n, _, _) => {
            let symbols = if m == 1 {
                "1 symbol".to_owned()
            } else {
                format!("{m} symbols")
            };
            let (move_verb, route_verb) = if dry_run {
                ("Would move", "route")
            } else {
                ("Moved", "routed")
            };
            format!(
                "{move_verb} {symbols} and {route_verb} {} on folio {}.\n",
                wires(n),
                page.folio
            )
        }
    };
    for symbol in left {
        out.push_str(&format!("  {symbol}; it is left where it stands.\n"));
    }
    for ((_, why), u) in plan.plan.unrouted.iter().zip(&page.unrouted) {
        out.push_str(&format!("  {}\n", reason(*why, &u.wire).1));
    }
    for wire in &page.across_texts {
        out.push_str(&format!(
            "  Routed across a text that has no other place: {wire}.\n"
        ));
    }
    match kept {
        0 => {}
        1 => out
            .push_str("  1 wire keeps the route it has: name it with --selection to have it routed anew.\n"),
        n => out.push_str(&format!(
            "  {n} wires keep the routes they have: name them with --selection to have them routed anew.\n"
        )),
    }
    out
}

/// The words of a symbol left against a rule (docs/design/06a-drafting.md §4.7): what was in the way of its place on
/// `side` of its host's pin, `name` naming a symbol by its ID.
fn left_words(side: Side, why: InTheWay, name: impl Fn(ew_api::output::Uuid) -> String) -> String {
    let place = match side {
        Side::Up => "above its pin",
        Side::Down => "below its pin",
        Side::Left => "left of its pin",
        Side::Right => "right of its pin",
    };
    match why {
        InTheWay::Room => format!("no place {place} has room for it"),
        InTheWay::Part(p) => format!("{} stands in its place {place}", name(p.0)),
        InTheWay::Edge => format!("its place {place} lies outside the drawing area"),
        InTheWay::Text(Some(p)) => format!(
            "in its place {place} a wire would run through a text of {}",
            name(p.0)
        ),
        InTheWay::Text(None) => format!("in its place {place} a wire would run through a text"),
        InTheWay::Wire => {
            format!("in its place {place} a wire would run through a connection point or along another wire")
        }
        InTheWay::Texts => format!("in its place {place} a text would lie on another text or a symbol"),
        InTheWay::Crossings => format!("in its place {place} the wires would cross more than they do now"),
        InTheWay::NoGain => format!("in its place {place} the page would be no better"),
    }
}

#[cfg(test)]
mod tests {
    use ew_api::drafting::{Plan, Reason, WireId};
    use ew_api::routing::RoutePlan;

    use super::{page_result, page_text};

    #[test]
    fn the_text_has_the_summary_line_and_one_line_per_wire_without_a_way() {
        let id = |n: u128| ew_api::output::Uuid::from_u128(n);
        let plan = RoutePlan {
            plan: Plan {
                routes: vec![(WireId(id(1)), Vec::new())],
                unrouted: vec![(WireId(id(2)), Reason::Parts), (WireId(id(3)), Reason::FourArms)],
                ..Plan::default()
            },
            moves: Vec::new(),
            left: Vec::new(),
            params: None,
            kept: Vec::new(),
            held: Vec::new(),
        };
        let words = |i: ew_api::output::Uuid| format!("-K{}:1 → -X1:{}", i.as_u128(), i.as_u128());
        let page = page_result("101", &plan, words);
        assert_eq!(
            page_text(&page, &plan, &[], false),
            "Routed 1 wire on folio 101.\n  No way for -K2:1 → -X1:2: symbols are in its way.\n  No way for -K3:1 → \
             -X1:3: it would be a fourth arm at a junction.\n"
        );
        assert_eq!(page.unrouted[1].reason, "four_arms");
        let empty = RoutePlan {
            plan: Plan::default(),
            moves: Vec::new(),
            left: Vec::new(),
            params: None,
            kept: Vec::new(),
            held: Vec::new(),
        };
        assert_eq!(
            page_text(&page_result("102", &empty, words), &empty, &[], false),
            "Nothing to route on folio 102: no wire would run better.\n"
        );
        let his = RoutePlan {
            kept: vec![id(4), id(5)],
            ..empty
        };
        assert_eq!(
            page_text(&page_result("102", &his, words), &his, &[], false),
            "Nothing changed on folio 102.\n  2 wires keep the routes they have: name them with --selection to have \
             them routed anew.\n"
        );
    }
}
