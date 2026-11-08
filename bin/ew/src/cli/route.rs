// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew route [<project-dir>] [--page <folio>] [--selection <ids>] [--dry-run] [--json]` (WP-S2.19 D13): the page
//! router over the CLI. Each page's plan ([`ew_api::Session::route_plan`]) is committed as one
//! `connection.route_many`, the command the window's Route page commits, and the project is saved once.

use std::path::PathBuf;

use ew_api::cli_results as results;
use ew_api::routing::{RoutePlan, RouteScope, Unrouted};
use ew_api::{AnyCommand, Session};

use super::{project_dir, reader_dir};

/// The arguments of `ew route`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RouteArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The page by its folio label (`--page`); `None`: every drawn page but the panel-layout pages.
    pub page: Option<String>,
    /// The connections and representations to route (`--selection`, IDs joined by `,`); empty: the pages' unrouted
    /// connections.
    pub selection: Vec<String>,
    /// Route and print, but save nothing (`--dry-run`).
    pub dry_run: bool,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// The reason's key in the JSON result and its sentence in the text (WP-S2.19 §12a).
fn reason(why: Unrouted, wire: &str) -> (&'static str, String) {
    match why {
        Unrouted::Wires => ("wires", format!("No route keeps clear of other wires: {wire}.")),
        Unrouted::Symbols => ("symbols", format!("No route keeps clear of symbols: {wire}.")),
        Unrouted::Texts => ("texts", format!("No route keeps clear of texts: {wire}.")),
        Unrouted::Points => (
            "points",
            format!("No route keeps clear of other connection points: {wire}."),
        ),
        Unrouted::FourArms => (
            "four_arms",
            format!("The junction would join four wires: {wire}."),
        ),
    }
}

/// `ew route`: routes each page and saves (a reader with `--dry-run`). Exit 0 when every wire it routes is routed, 1
/// when a wire stays unrouted, 2 on usage, open, an unknown folio or selection, a refused command and save errors.
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
        Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "route")
            .map_err(|e| format!("{}: {e}", dir.display()))
    } else {
        let Some(dir) = project_dir(a.project.as_ref()) else {
            return 2;
        };
        let options = ew_api::SessionOptions {
            origin: ew_api::Origin::Cli {
                subcommand: "route".to_owned(),
                confirmed: false,
            },
            ..ew_api::SessionOptions::default()
        };
        Session::open(
            &dir,
            ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
            options,
            ew_api::Recovery::Keep,
        )
        .map_err(|e| format!("{}: {e}", dir.display()))
    };
    let mut session = match opened {
        Ok((session, _)) => session,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let folios = match &a.page {
        Some(f) => {
            if session.page_geometry(f).is_none() {
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
        let Some(plan) = session.route_plan(&folio, &scope) else {
            continue;
        };
        let words = |id| session.connection_words(&folio, id).unwrap_or_default();
        let page = page_result(&folio, &plan, words);
        text.push_str(&page_text(&page, &plan));
        if !a.dry_run
            && let Some(params) = ew_api::routing::route_many_params(&plan)
        {
            let cmd = AnyCommand {
                name: "connection.route_many".to_owned(),
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

/// One page's result: the routed wires and the unrouted ones with their reasons, named by `words`.
fn page_result(
    folio: &str,
    plan: &RoutePlan,
    words: impl Fn(ew_api::output::Uuid) -> String,
) -> results::RoutePageResult {
    results::RoutePageResult {
        folio: folio.to_owned(),
        routed: plan.routes.iter().map(|(id, _)| words(*id)).collect(),
        unrouted: plan
            .unrouted
            .iter()
            .map(|(id, why)| {
                let wire = words(*id);
                results::RouteUnrouted {
                    reason: reason(*why, &wire).0.to_owned(),
                    wire,
                }
            })
            .collect(),
    }
}

/// The text of one page: its summary line (WP-S2.19 §12a, the page as "folio <label>" as the window words it), then
/// one line per unrouted wire.
fn page_text(page: &results::RoutePageResult, plan: &RoutePlan) -> String {
    let (routed, unrouted) = (page.routed.len(), page.unrouted.len());
    let mut out = match routed + unrouted {
        0 => format!("Every wire on folio {} has a route.\n", page.folio),
        1 => format!(
            "Route 1 wire on folio {}: {routed} routed, {unrouted} not routed.\n",
            page.folio
        ),
        n => format!(
            "Route {n} wires on folio {}: {routed} routed, {unrouted} not routed.\n",
            page.folio
        ),
    };
    for ((_, why), u) in plan.unrouted.iter().zip(&page.unrouted) {
        out.push_str(&format!("  {}\n", reason(*why, &u.wire).1));
    }
    out
}

#[cfg(test)]
mod tests {
    use ew_api::routing::{RoutePlan, Unrouted};

    use super::{page_result, page_text};

    #[test]
    fn the_text_has_the_summary_line_and_one_line_per_unrouted_wire() {
        let id = |n: u128| ew_api::output::Uuid::from_u128(n);
        let plan = RoutePlan {
            routes: vec![(id(1), vec![(0, 0), (1, 0)])],
            unrouted: vec![(id(2), Unrouted::Texts), (id(3), Unrouted::FourArms)],
        };
        let words = |i: ew_api::output::Uuid| format!("-K{}:1 → -X1:{}", i.as_u128(), i.as_u128());
        let page = page_result("101", &plan, words);
        assert_eq!(
            page_text(&page, &plan),
            "Route 3 wires on folio 101: 1 routed, 2 not routed.\n  No route keeps clear of texts: -K2:1 → -X1:2.\n  The \
             junction would join four wires: -K3:1 → -X1:3.\n"
        );
        assert_eq!(page.unrouted[1].reason, "four_arms");
        let empty = page_result("102", &RoutePlan::default(), words);
        assert_eq!(
            page_text(&empty, &RoutePlan::default()),
            "Every wire on folio 102 has a route.\n"
        );
    }
}
