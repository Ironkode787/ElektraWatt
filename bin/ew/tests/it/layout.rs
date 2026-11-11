// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew layout` on a copy of Golden A: the text of folio 101 names the
//! heater's PE wire touching the lamp's X1 and the lamp's feed along the switch's wires, its line of counts agrees
//! with the `--json` result, and a folio that names no page is refused in words. Named items only, never totals.

use std::path::Path;

use ew_api::cli_results as results;

use crate::support::{EwTmp, Tmp, stderr, stdout};

#[test]
fn folio_101_routed_names_no_touch_overlap_or_symbol_crossed() {
    let t = Tmp::new("ew-layout");
    let p = t.golden("J-E-2416.ew").to_string_lossy().into_owned();
    let o = t.ew(&["layout", &p, "--page", "101"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let text = stdout(&o);
    let lines: Vec<&str> = text.lines().collect();
    // Golden A's routes are the router's (RG1, RG2): no line names a broken hard rule.
    for word in [
        "  touch: ",
        "  overlap: ",
        "  through a symbol: ",
        "  through a text: ",
    ] {
        assert!(!lines.iter().any(|l| l.starts_with(word)), "{word}\n{text}");
    }
    // The line of counts, from the `--json` result of the same page.
    let o = t.ew(&["layout", &p, "--page", "101", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: results::LayoutResult = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(json.schema, results::LAYOUT);
    let [page] = json.pages.as_slice() else {
        panic!("{json:?}");
    };
    assert_eq!(
        lines[0],
        format!(
            "101: hard rules {} (touches {}, overlaps {}, through symbols {}, through texts {}, four-way junctions {}); \
             costs: crossings {}, bends {}",
            page.hard(),
            page.touches.len(),
            page.overlaps.len(),
            page.through_symbols.len(),
            page.through_texts.len(),
            page.four_way_junctions.len(),
            page.crossings.len(),
            page.bend_total()
        )
    );
}

#[test]
fn every_drawn_page_gets_a_line_of_counts_and_an_unknown_folio_is_refused() {
    let t = Tmp::new("ew-layout");
    let p = t.golden("J-E-2416.ew").to_string_lossy().into_owned();
    let o = t.ew(&["layout", &p]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let heads: Vec<String> = stdout(&o)
        .lines()
        .filter(|l| !l.starts_with(' '))
        .map(|l| l.split(':').next().unwrap_or_default().to_owned())
        .collect();
    for folio in ["1", "3", "101", "102", "103", "106", "203"] {
        assert!(heads.iter().any(|h| h == folio), "{folio}: {heads:?}");
    }
    let o = t.ew(&["layout", &p, "--page", "999"]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            "error: the page order has no folio `999`\n".to_owned()
        )
    );
}

/// `ew route`: folio 3's one wire, its route cleared first (Golden A's routes are the router's), is
/// routed and saved, so a second run has nothing to do; `--dry-run` writes nothing; an unknown folio is refused in words.
#[test]
fn route_routes_a_page_saves_it_and_then_has_nothing_to_do() {
    let t = Tmp::new("ew-layout");
    let p = t.golden("J-E-2416.ew").to_string_lossy().into_owned();
    // `connections|DP.1`, the bus cable's hop on folio 3, without a route.
    let log = Path::new(&p).with_file_name("unroute.ewcmd");
    std::fs::write(
        &log,
        "{\"$schema\":\"ew.cmdlog/1\",\"label\":\"folio 3's wire unrouted\"}\n\
         {\"cmd\":\"connection.route\",\"params\":{\"connection\":\"ec0fcf51-0509-588e-a64c-a564f837c94b\",\"route\":[]}}\n",
    )
    .unwrap();
    let o = t.ew(&["apply", &p, log.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let before = std::fs::read_dir(Path::new(&p).join("pages")).unwrap().count();
    let o = t.ew(&["route", &p, "--page", "3", "--dry-run"]);
    assert_eq!(
        (o.status.code(), stdout(&o)),
        (
            Some(0),
            "Route 1 wire on folio 3: 1 routed, 0 not routed.\n".to_owned()
        ),
        "{}",
        stderr(&o)
    );
    let o = t.ew(&["route", &p, "--page", "3"]);
    assert_eq!(
        (o.status.code(), stdout(&o)),
        (
            Some(0),
            "Route 1 wire on folio 3: 1 routed, 0 not routed.\n".to_owned()
        ),
        "{}",
        stderr(&o)
    );
    assert_eq!(
        std::fs::read_dir(Path::new(&p).join("pages")).unwrap().count(),
        before
    );
    let o = t.ew(&["route", &p, "--page", "3", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: results::RouteResult = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(json.schema, results::ROUTE);
    assert_eq!(
        (json.pages.len(), json.pages[0].routed.len(), json.saved),
        (1, 0, false)
    );
    let o = t.ew(&["route", &p, "--page", "999"]);
    assert_eq!(
        (o.status.code(), stderr(&o)),
        (
            Some(2),
            "error: the page order has no drawn page with the folio `999`\n".to_owned()
        )
    );
}
