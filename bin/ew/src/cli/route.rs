// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew route [<project-dir>] [--page <folio>] [--selection <ids>] [--arrange] [--dry-run] [--json]`: Route over the
//! CLI (docs/design/06a-drafting.md §7.4), and with `--arrange` Arrange and route. Each page's plan
//! ([`ew_api::Session::route_plan`], [`ew_api::Session::arrange_plan`]) is stored as one `layout.apply`, the command
//! the window's buttons store, and the project is saved once; `--dry-run` prints the plan.

use ew_api::AnyCommand;
use ew_api::cli_results as results;
use ew_api::routing::{Reason, RoutePlan, RouteScope};

use super::{RouteArgs, open_for_writing, open_reader, project_dir, reader_dir};

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
pub(super) fn run_route(a: &RouteArgs) -> i32 {
    let mut selection = Vec::with_capacity(a.selection.len());
    for s in &a.selection {
        match ew_api::output::Uuid::parse_str(s.trim()) {
            Ok(id) => selection.push(id),
            Err(_) => {
                eprintln!("error: `{s}` in --selection is not a record ID");
                return 2;
            }
        }
    }
    let opened = if a.dry_run {
        let Some(dir) = reader_dir(a.project.as_ref()) else {
            return 2;
        };
        open_reader(&dir, "route").map(|(s, _)| s).map_err(|e| {
            eprintln!("error: {}: {e}", dir.display());
            2
        })
    } else {
        let Some(dir) = project_dir(a.project.as_ref()) else {
            return 2;
        };
        let origin = ew_api::Origin::Cli {
            subcommand: "route".to_owned(),
            confirmed: false,
        };
        open_for_writing(&dir, origin)
    };
    let mut session = match opened {
        Ok(session) => session,
        Err(code) => return code,
    };
    let folios = match &a.page {
        Some(f) => {
            if session.page_draft(f).is_none() {
                eprintln!("error: the page order has no drawn page with the folio `{f}`");
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
        page.left = plan.left.iter().map(|(p, _)| name(p.0)).collect();
        text.push_str(&page_text(&page, &plan, a.dry_run));
        if !a.dry_run
            && let Some(params) = plan.params.clone()
        {
            let cmd = AnyCommand {
                name: "layout.apply".to_owned(),
                params,
            };
            if let Err(e) = session.execute(&cmd) {
                eprintln!("error: folio {folio}: {e}; nothing was saved");
                return 2;
            }
            changed = true;
        }
        pages.push(page);
    }
    let saved = changed && !a.dry_run;
    if saved && let Err(e) = session.save() {
        eprintln!("error: {e}; nothing was saved");
        return 2;
    }
    session.close();
    let unrouted = pages.iter().any(|p| !p.unrouted.is_empty());
    if a.json {
        print!(
            "{}",
            results::to_text(&results::RouteResult {
                schema: results::ROUTE.to_owned(),
                pages,
                saved,
            })
        );
    } else {
        print!("{text}");
    }
    i32::from(unrouted)
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
/// Route would do), then one line per wire left without a way and per wire across a text, and the number of wires a
/// page-wide Route leaves as the person's.
fn page_text(page: &results::RoutePageResult, plan: &RoutePlan, dry_run: bool) -> String {
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
    for symbol in &page.left {
        out.push_str(&format!(
            "  No better place for {symbol} on the side its pin faces: it is left where it stands.\n"
        ));
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

#[cfg(test)]
mod tests {
    use ew_api::routing::{Plan, Reason, RoutePlan, WireId};

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
        };
        let words = |i: ew_api::output::Uuid| format!("-K{}:1 → -X1:{}", i.as_u128(), i.as_u128());
        let page = page_result("101", &plan, words);
        assert_eq!(
            page_text(&page, &plan, false),
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
        };
        assert_eq!(
            page_text(&page_result("102", &empty, words), &empty, false),
            "Nothing to route on folio 102: no wire would run better.\n"
        );
        let his = RoutePlan {
            kept: vec![id(4), id(5)],
            ..empty
        };
        assert_eq!(
            page_text(&page_result("102", &his, words), &his, false),
            "Nothing changed on folio 102.\n  2 wires keep the routes they have: name them with --selection to have \
             them routed anew.\n"
        );
    }
}
