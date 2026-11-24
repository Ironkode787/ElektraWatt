// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew layout` on a copy of the sample: the text of folio 101 names no fault and no wire across a text, its line of
//! figures agrees with the `--json` result, and a folio that names no page is refused in words. Named items only,
//! never totals.

use std::path::Path;

use ew_api::cli_results as results;

use crate::support::{Tmp, page_file, stderr, stdout};

#[test]
fn folio_101_routed_names_no_touch_overlap_or_symbol_crossed() {
    let t = Tmp::new("ew-layout");
    let p = t.sample("sample.ew").to_string_lossy().into_owned();
    let o = t.ew(&["layout", &p, "--page", "101"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let text = stdout(&o);
    let lines: Vec<&str> = text.lines().collect();
    // The sample's wires on 101 are drawn as a draughtsman draws them: no line names a fault or a wire across a text.
    for word in [
        "  touch: ",
        "  overlap: ",
        "  through a symbol: ",
        "  false joint: ",
        "  part on a part: ",
        "  through a text: ",
    ] {
        assert!(!lines.iter().any(|l| l.starts_with(word)), "{word}\n{text}");
    }
    // The line of counts, from the `--json` result of the same page.
    let o = t.ew(&["layout", &p, "--page", "101", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: results::LayoutResult = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(json.schema, results::LayoutResult::SCHEMA);
    let [page] = json.pages.as_slice() else {
        panic!("{json:?}");
    };
    assert!(
        lines[0].starts_with(&format!(
            "101: faults {} (through points {}, ",
            page.faults,
            page.touches.len()
        )),
        "{}",
        lines[0]
    );
    assert!(
        lines[0].contains(&format!(
            "; unrouted {}; cost {}; crossings {}, bends {}, straight {} of {}, ",
            page.unrouted,
            page.cost,
            page.crossings.len(),
            page.bend_total(),
            page.straight,
            page.wires
        )),
        "{}",
        lines[0]
    );
}

#[test]
fn every_drawn_page_gets_a_line_of_counts_and_an_unknown_folio_is_refused() {
    let t = Tmp::new("ew-layout");
    let p = t.sample("sample.ew").to_string_lossy().into_owned();
    let o = t.ew(&["layout", &p]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let heads: Vec<String> = stdout(&o)
        .lines()
        .filter(|l| !l.starts_with(' '))
        .map(|l| l.split(':').next().unwrap_or_default().to_owned())
        .collect();
    // The sample's drawn pages: the title page, 101 to 105 and the pneumatics.
    for folio in ["1", "101", "102", "103", "104", "105", "201"] {
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

/// `ew route`: folio 105's one wire, its route cleared first, is routed and saved, so a second run has nothing to do;
/// `--dry-run` writes nothing; an unknown folio is refused in words.
#[test]
fn route_routes_a_page_saves_it_and_then_has_nothing_to_do() {
    let t = Tmp::new("ew-layout");
    let p = t.sample("sample.ew").to_string_lossy().into_owned();
    // The PROFIBUS line from -104A1 to -201K1, the one wire of the bus page 105, without a route.
    let page = std::fs::read_to_string(Path::new(&p).join(page_file(Path::new(&p), 105))).unwrap();
    let line = page
        .lines()
        .find(|l| l.contains("\"$k\":\"connection\""))
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(line).unwrap();
    let log = Path::new(&p).with_file_name("unroute.ewcmd");
    std::fs::write(
        &log,
        format!(
            "{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"folio 105's wire unrouted\"}}\n\
             {{\"cmd\":\"connection.route\",\"params\":{{\"connection\":{},\"route\":[]}}}}\n",
            wire["id"]
        ),
    )
    .unwrap();
    let o = t.ew(&["apply", &p, log.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let before = std::fs::read_dir(Path::new(&p).join("pages")).unwrap().count();
    let o = t.ew(&["route", &p, "--page", "105", "--dry-run"]);
    assert_eq!(
        (o.status.code(), stdout(&o)),
        (Some(0), "Would route 1 wire on folio 105.\n".to_owned()),
        "{}",
        stderr(&o)
    );
    let o = t.ew(&["route", &p, "--page", "105"]);
    assert_eq!(
        (o.status.code(), stdout(&o)),
        (Some(0), "Routed 1 wire on folio 105.\n".to_owned()),
        "{}",
        stderr(&o)
    );
    assert_eq!(
        std::fs::read_dir(Path::new(&p).join("pages")).unwrap().count(),
        before
    );
    let o = t.ew(&["route", &p, "--page", "105", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: results::RouteResult = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(json.schema, results::RouteResult::SCHEMA);
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
