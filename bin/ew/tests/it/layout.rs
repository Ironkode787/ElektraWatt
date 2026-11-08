// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew layout` (WP-S2.19 part 1, decision log ID-1498) on a copy of Golden A: the text of folio 101 names the
//! heater's PE wire touching the lamp's X1 and the lamp's feed along the switch's wires, its line of counts agrees
//! with the `--json` result, and a folio that names no page is refused in words. Named items only, never totals.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ew_api::cli_results as results;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        Tmp(ew_testdir::fresh_dir("ew-layout"))
    }

    /// `ew` in this directory with its user directories inside it and no starter content.
    fn ew(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(bin_exe());
        cmd.args(args.iter().map(OsString::from))
            .current_dir(&self.0)
            .env_remove("EW_CONTENT_DIR")
            .env("EW_DATA_DIR", self.0.join("data"))
            .env("EW_CACHE_DIR", self.0.join("cache"));
        cmd.output().unwrap()
    }

    /// A copy of Golden A, by its path as text.
    fn golden(&self) -> String {
        let copy = self.0.join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &copy);
        copy.to_string_lossy().into_owned()
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The path of the `ew` binary under test (nextest's run-time path, else the compile-time one).
// The program boundary of a test: the path only chooses which binary runs; no compared result depends on it.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_ew")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ew"))) // bin-exe-fallback
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn folio_101_routed_names_no_touch_overlap_or_symbol_crossed() {
    let t = Tmp::new();
    let p = t.golden();
    let o = t.ew(&["layout", &p, "--page", "101"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let text = stdout(&o);
    let lines: Vec<&str> = text.lines().collect();
    // Golden A's routes are the router's (WP-S2.19 RG1, RG2): no line names a broken hard rule.
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
    let t = Tmp::new();
    let p = t.golden();
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

/// `ew route` (WP-S2.19 D13): folio 3's one wire, its route cleared first (Golden A's routes are the router's), is
/// routed and saved, so a second run has nothing to do; `--dry-run` writes nothing; an unknown folio is refused in words.
#[test]
fn route_routes_a_page_saves_it_and_then_has_nothing_to_do() {
    let t = Tmp::new();
    let p = t.golden();
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
