// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The connectivity report kinds over the golden project (WP-S3.03; doc 20 §8.2, §8.3, §9, §10, §16):
//! `wire_list` (907), `potential_overview` (909), `terminal_diagram` (903 to 905) and
//! `cable_overview_diagrams` (906).
//!
//! The rows come from a `Q` without the report engine (`Q::new`) for the row tests, which keeps them apart from
//! the pagination; the connectivity queries they read (`potential_partition`, `physical_wires_of`,
//! `wire_props_of`) read the drawn page order, so a `Q` with the engine derives the same rows while it
//! paginates (the last two tests).
//!
//! Tests:
//! - `wire_list_has_44_wires_then_six_bridges` (doc 20 §8.2, §8.3) and its pages (§16: 23, 23, 5 rows).
//! - `wire_rows_carry_the_effective_properties_and_cable_cores` (AC-09 values through the report).
//! - `potential_overview_lists_every_potential_and_the_bond` (doc 20 §7.3, §7.4).
//! - `terminal_diagrams_list_the_strips_terminals` (doc 20 §9: 3, 20 and 5 rows).
//! - `cable_overview_has_three_cables_thirteen_cores_and_four_headings` (doc 20 §10, §16: 20 rows).
//! - `the_row_texts_match_the_committed_files`: the printed rows of the six blocks as
//!   `golden/J-E-2416/expected/rows/<block>.csv` (`EW_BLESS=1` rewrites them), over the golden tables with
//!   the colour and size profiles of doc 20 §7.1, §7.2, which the page goldens do not have yet. Candidates
//!   for the FT-10 approval, compared with doc 20 by hand (README, "Connectivity report rows").
//! - `a_query_with_the_report_engine_paginates_the_connectivity_kinds`: the four kinds get their derived rows and
//!   pages (907 in three) inside the pagination, equal to the rows of a `Q` without the engine.
//! - `rows_and_pages_do_not_depend_on_the_order_they_are_read_in`: the order the queries and the records are read in
//!   (NFR-REL-07).

// Test target: bless and diagnostic environment flags (EW_*), which only choose whether a committed file is rewritten or a line is printed; the compared result never depends on them.
#![allow(clippy::disallowed_methods)]

mod golden_common;

use ew_derive::{GeneratedPage, PageOrder, Q};
use ew_reports::{BlockPages, BlockRows, Cell, RowKind, RowSet, RowSetStatus};
use golden_common::{block, plain, rows_csv, rows_of, texts};

/// The row kinds (`data` / `heading`) of a committed row file `golden/J-E-2416/expected/rows/<key>.csv`.
fn committed_kinds(key: &str) -> Vec<String> {
    let path = golden_common::repo().join(format!("golden/J-E-2416/expected/rows/{key}.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .skip(1)
        .map(|l| l.split(',').next().unwrap_or_default().to_owned())
        .collect()
}

fn col(set: &RowSet, key: &str) -> usize {
    set.column(key).unwrap_or_else(|| panic!("no column `{key}`"))
}

fn pages(
    q: &Q,
    batch: &ew_import_tables::ew_interop_api::ImportBatch,
    key: &str,
) -> Vec<(String, std::ops::Range<u32>)> {
    q.get::<BlockPages>((block(batch, key), 0))
        .iter()
        .map(|p: &GeneratedPage| (p.label.clone(), p.rows.clone()))
        .collect()
}

#[test]
fn wire_list_has_44_wires_then_six_bridges() {
    let batch = golden_common::batch();
    let s = golden_common::session_with_profiles(&batch);
    let q = plain(&s);
    let set = rows_of(&q, &batch, "907");

    // The physical wires (W1 to W44), a heading, the six bridges BR1 to BR6, as in the committed rows.
    let committed = committed_kinds("907");
    let wires = committed
        .iter()
        .position(|k| k == "heading")
        .expect("a heading row");
    assert_eq!(set.rows.len(), committed.len());
    let kinds: Vec<RowKind> = set.rows.iter().map(|r| r.kind).collect();
    assert!(kinds[..wires].iter().all(|k| *k == RowKind::Data));
    assert_eq!(kinds[wires], RowKind::Heading);
    assert!(kinds[wires + 1..].iter().all(|k| *k == RowKind::Data));
    let no = col(&set, "no");
    let via = col(&set, "via");
    for (i, (label, span)) in [
        ("BR1", "1\u{2013}4"),
        ("BR2", "5\u{2013}6"),
        ("BR3", "7\u{2013}8"),
        ("BR4", "9\u{2013}10"),
        ("BR5", "11\u{2013}20"),
        ("BR6", "1\u{2013}5"),
    ]
    .into_iter()
    .enumerate()
    {
        let t = texts(&q, &set, wires + 1 + i);
        assert_eq!(t[no], label);
        assert!(
            t[via].contains(&format!("terminals {span}")),
            "{label}: {}",
            t[via]
        );
        assert_eq!(t[col(&set, "part")], "TBD", "{label}: plug-in, TBD (doc 20 §8.3)");
    }
    // Bridges are never wire rows: the wire rows and the bridge rows have disjoint IDs.
    let mut ids: Vec<String> = set.rows.iter().map(|r| r.id.to_string()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), committed.len());

    // Doc 20 §16: 50 rows over three pages (907, 907.1, 907.2); the heading takes a row of its own.
    assert_eq!(
        pages(&q, &batch, "907"),
        vec![
            ("907".to_owned(), 0..23),
            ("907.1".to_owned(), 23..46),
            ("907.2".to_owned(), 46..51),
        ]
    );
}

#[test]
fn wire_rows_carry_the_effective_properties_and_cable_cores() {
    let batch = golden_common::batch();
    let s = golden_common::session_with_profiles(&batch);
    let q = plain(&s);
    let set = rows_of(&q, &batch, "907");
    let (from, from_cp, to, to_cp) = (
        col(&set, "from"),
        col(&set, "from_cp"),
        col(&set, "to"),
        col(&set, "to_cp"),
    );
    let (pot, colour, csa, source, via) = (
        col(&set, "potential"),
        col(&set, "colour"),
        col(&set, "cross_section"),
        col(&set, "source"),
        col(&set, "via"),
    );
    let wires: Vec<Vec<String>> = (0..44).map(|i| texts(&q, &set, i)).collect();
    // A wire by its two ends, in either orientation.
    let find = |a: &str, b: &str| -> &Vec<String> {
        let hits: Vec<&Vec<String>> = wires
            .iter()
            .filter(|t| {
                let (x, y) = (
                    format!("{}:{}", t[from], t[from_cp]),
                    format!("{}:{}", t[to], t[to_cp]),
                );
                (x == a && y == b) || (x == b && y == a)
            })
            .collect();
        assert_eq!(hits.len(), 1, "wire {a} {b}: {hits:?}");
        hits[0]
    };

    // W4: 1X1:1 to S51:2, explicit 2.5 mm² (CDP), colour from the profile (doc 20 §8.2).
    let w4 = find("S51:2", "1X1:1:1");
    assert_eq!(
        (w4[pot].as_str(), w4[colour].as_str(), w4[csa].as_str()),
        ("L", "BK", "2.5 mm\u{00B2}")
    );
    assert_eq!(w4[source], "cdp/rule");
    // W19: PE to the strip -X1032, 1.5 mm² GNYE from the profile rule.
    let w19 = find("E55:PE", "-X1032:1:b");
    assert_eq!(
        (w19[pot].as_str(), w19[colour].as_str(), w19[csa].as_str()),
        ("PE", "GNYE", "1.5 mm\u{00B2}")
    );
    // W27: the bond wire joins 0V and PE (doc 20 §7.4), 0.75 mm² GNYE by the bond rule.
    let w27 = find("-102G1:\u{2212}", "-X1032:2:a");
    assert_eq!(w27[pot], "0V \u{21C4} PE");
    assert_eq!(w27[source], "bond/rule");
    assert_eq!(
        (w27[colour].as_str(), w27[csa].as_str()),
        ("GNYE", "0.75 mm\u{00B2}")
    );
    // W39: a core of -106W1: colour BN from the conductor, the cable and core in `via`.
    let w39 = find("-X1031:1:b", "-201U1:5");
    assert_eq!(
        (w39[pot].as_str(), w39[colour].as_str(), w39[source].as_str()),
        ("24VDC-1", "BN", "rule/core")
    );
    assert!(w39[via].starts_with("-106W1 BN"), "{}", w39[via]);
    // W1: the external end prints as `ext. <name>` and has no CP.
    let w1 = find("1X1:1:2", "ext. J-E-206-L:");
    assert_eq!(
        (w1[pot].as_str(), w1[colour].as_str(), w1[csa].as_str()),
        ("L", "BK", "1.5 mm\u{00B2}")
    );

    // Every wire has two ends, a potential and pages.
    for t in &wires {
        assert!(!t[from].is_empty() && !t[to].is_empty(), "{t:?}");
        assert!(!t[pot].is_empty() && !t[col(&set, "pages")].is_empty(), "{t:?}");
    }
    // Wire numbers (doc 08 §5.2, doc 20 §8.1): `{page}.{counter}` on the 34 wires, none on the 10 cable cores,
    // and the rows are in the numbering order, so the counters of a page rise down the list (FT-100).
    let no = col(&set, "no");
    let mut last: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let mut numbered = 0;
    for t in &wires {
        if t[via].contains("-106W") {
            assert!(t[no].is_empty(), "a cable core is not numbered: {t:?}");
            continue;
        }
        let (page, counter) = t[no]
            .split_once('.')
            .unwrap_or_else(|| panic!("no number: {t:?}"));
        let counter: u32 = counter.parse().expect("a counter");
        let prev = last.insert(page.to_owned(), counter).unwrap_or(0);
        assert_eq!(counter, prev + 1, "counters rise by one on folio {page}: {t:?}");
        numbered += 1;
    }
    assert_eq!(numbered, 34);
}

#[test]
fn potential_overview_lists_every_potential_and_the_bond() {
    let batch = golden_common::batch();
    let s = golden_common::session_with_profiles(&batch);
    let q = plain(&s);
    let set = rows_of(&q, &batch, "909");
    // Doc 20 §7.3: 19 potentials (the PROFIBUS `DP` line through its `Bus` connections, FT-105) and the
    // bond B1.
    assert_eq!(set.rows.len(), 20);
    let rows: Vec<Vec<String>> = (0..set.rows.len()).map(|i| texts(&q, &set, i)).collect();
    let pot = col(&set, "potential");
    let named: Vec<&str> = rows.iter().map(|t| t[pot].as_str()).collect();
    for want in [
        "L", "N", "L-S", "N-S", "L-1", "N-1", "L-55", "N-55", "N-55T", "N-55S", "PE", "+24V", "+24V-F",
        "24VDC-1", "24VDC-2", "24VDC-3", "24VDC-4", "0V", "DP",
    ] {
        assert!(named.contains(&want), "potential {want}");
    }
    assert!(
        named.last().is_some_and(|n| n.starts_with("B1 (0V \u{21C4} PE)")),
        "{named:?}"
    );
    let row = |name: &str| {
        rows.iter()
            .find(|t| t[pot] == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    let (kind, volt, class, colour, csa, aliases, members, pg) = (
        col(&set, "kind"),
        col(&set, "voltage"),
        col(&set, "class"),
        col(&set, "colour"),
        col(&set, "cross_section"),
        col(&set, "aliases"),
        col(&set, "members"),
        col(&set, "pages"),
    );
    // L: 230 V AC mains, BK 1.5 mm², alias J-E-206-L, folio 101 (doc 20 §7.3).
    let l = row("L");
    assert_eq!(
        (l[kind].as_str(), l[volt].as_str(), l[class].as_str()),
        ("L", "230 V AC", "mains")
    );
    assert_eq!(
        (
            l[colour].as_str(),
            l[csa].as_str(),
            l[aliases].as_str(),
            l[pg].as_str()
        ),
        ("BK", "1.5 mm\u{00B2}", "1", "101")
    );
    // 24VDC-1: DBU 0.75 mm², four aliases (24VDC-AIR1 to 4), pages 102, 103, 106 (a range, FR-REP-046).
    let v = row("24VDC-1");
    assert_eq!(
        (v[colour].as_str(), v[csa].as_str(), v[aliases].as_str()),
        ("DBU", "0.75 mm\u{00B2}", "4")
    );
    assert_eq!(v[pg], "102\u{2013}106");
    // PE: GNYE, seven aliases, folios 101 to 106.
    let pe = row("PE");
    assert_eq!((pe[colour].as_str(), pe[aliases].as_str()), ("GNYE", "7"));
    assert_eq!(pe[pg], "101\u{2013}106");
    // 0V: WH 0.75 mm², members counted, never listed.
    let m = row("0V");
    assert_eq!((m[colour].as_str(), m[csa].as_str()), ("WH", "0.75 mm\u{00B2}"));
    assert!(m[members].parse::<usize>().is_ok_and(|n| n >= 14));
    assert_eq!(pages(&q, &batch, "909"), vec![("909".to_owned(), 0..20)]);
}

#[test]
fn terminal_diagrams_list_the_strips_terminals() {
    let batch = golden_common::batch();
    let s = golden_common::session_with_profiles(&batch);
    let q = plain(&s);
    let (x1, x1031, x1032) = (
        rows_of(&q, &batch, "903"),
        rows_of(&q, &batch, "904"),
        rows_of(&q, &batch, "905"),
    );
    // Doc 20 §16: 3, 20 and 5 rows, one page each.
    assert_eq!((x1.rows.len(), x1031.rows.len(), x1032.rows.len()), (3, 20, 5));
    for key in ["903", "904", "905"] {
        assert_eq!(pages(&q, &batch, key).len(), 1, "{key}");
    }
    let t = |set: &RowSet, i: usize| texts(&q, set, i);
    let c = |set: &RowSet, k: &str| col(set, k);

    // -X1031 terminal 1: "1+", Pneumatics, PT 2.5-TWIN, 24VDC-1, fed from -102F2:Out1, BR1, used (doc 20 §9).
    let r = t(&x1031, 0);
    assert_eq!(r[c(&x1031, "terminal")], "1");
    assert_eq!(r[c(&x1031, "label")], "1+");
    assert_eq!(r[c(&x1031, "group")], "Pneumatics");
    assert_eq!(r[c(&x1031, "part")], "PT 2.5-TWIN");
    assert_eq!(r[c(&x1031, "potential")], "24VDC-1");
    assert_eq!(r[c(&x1031, "side1")], "-102F2:Out1");
    assert_eq!(r[c(&x1031, "side2")], "-201U1:5 via -106W1 BN");
    assert_eq!(
        (r[c(&x1031, "bridge")].as_str(), r[c(&x1031, "state")].as_str()),
        ("BR1", "used")
    );
    // Terminals 5 to 10 and 15 to 20 are spares: twelve (doc 08 §6.6); 5, 7 and 9 receive the feeds W30-W32.
    let spare = (0..20)
        .filter(|i| t(&x1031, *i)[c(&x1031, "state")] == "spare")
        .count();
    assert_eq!(spare, 12);
    assert_eq!(t(&x1031, 4)[c(&x1031, "side1")], "-102F2:Out2");
    assert_eq!(t(&x1031, 4)[c(&x1031, "group")], "Spare");
    // Terminal 11: 0V from -102G1, BR5.
    let r = t(&x1031, 10);
    assert_eq!(
        (r[c(&x1031, "label")].as_str(), r[c(&x1031, "bridge")].as_str()),
        ("1-", "BR5")
    );
    assert!(
        r[c(&x1031, "side2")].ends_with("via -106W1 BU"),
        "{}",
        r[c(&x1031, "side2")]
    );

    // 1X1: field side 2 is the supply (`ext.`), side 1 the internal wire to S51 / -XPE (doc 20 §9).
    assert_eq!(t(&x1, 0)[c(&x1, "side1")], "S51:2");
    assert_eq!(t(&x1, 0)[c(&x1, "side2")], "ext. J-E-206-L");
    assert_eq!(t(&x1, 2)[c(&x1, "terminal")], "PE");
    assert_eq!(t(&x1, 2)[c(&x1, "part")], "USLKG 5");

    // -X1032: five PE terminals, BR6; terminal 5 is a spare with no wire.
    assert!((0..5).all(|i| t(&x1032, i)[c(&x1032, "bridge")] == "BR6"));
    assert_eq!(t(&x1032, 4)[c(&x1032, "state")], "spare");
    assert_eq!(t(&x1032, 3)[c(&x1032, "side2")], "-202U1:FE");
}

#[test]
fn cable_overview_has_three_cables_thirteen_cores_and_four_headings() {
    let batch = golden_common::batch();
    let s = golden_common::session_with_profiles(&batch);
    let q = plain(&s);
    let set = rows_of(&q, &batch, "906");
    // Doc 20 §16: 3 + 13 cores + 4 headings = 20 rows, as in the committed rows.
    let committed = committed_kinds("906");
    assert_eq!(set.rows.len(), committed.len());
    let headings = set.rows.iter().filter(|r| r.kind == RowKind::Heading).count();
    assert_eq!(headings, committed.iter().filter(|k| *k == "heading").count());
    let cores = set
        .rows
        .iter()
        .filter(|r| r.kind == RowKind::Data && r.cells[col(&set, "core")] != Cell::Empty)
        .count();
    assert_eq!(cores, golden_common::table("conductors.csv").len());
    assert_eq!(pages(&q, &batch, "906"), vec![("906".to_owned(), 0..20)]);

    let (cable, part, core, colour, csa, from, to, length) = (
        col(&set, "cable"),
        col(&set, "part"),
        col(&set, "core"),
        col(&set, "colour"),
        col(&set, "cross_section"),
        col(&set, "from"),
        col(&set, "to"),
        col(&set, "length"),
    );
    let all: Vec<Vec<String>> = (0..set.rows.len()).map(|i| texts(&q, &set, i)).collect();
    // -3W1: no length (EF-08, D-19 "length missing"), no core with a wire (the bus line is not derived).
    let w3 = all
        .iter()
        .find(|t| t[cable] == "-3W1" && !t[part].is_empty())
        .expect("the -3W1 row");
    assert_eq!(w3[length], "length missing");
    // -106W1: ends -X1031/-X1032 and -201U1, length from the part, five cores.
    let w1 = all
        .iter()
        .find(|t| t[cable] == "-106W1" && !t[part].is_empty())
        .expect("the -106W1 row");
    assert_eq!(w1[part], "NEBU-G78W5-K-2-N-LE5");
    assert!(
        w1[from].contains("-X1031") && w1[from].contains("-X1032"),
        "{}",
        w1[from]
    );
    assert_eq!(w1[to], "-201U1");
    assert_eq!(w1[length], "from part");
    let w1_cores: Vec<&Vec<String>> = all
        .iter()
        .filter(|t| t[cable] == "-106W1" && !t[core].is_empty())
        .collect();
    assert_eq!(w1_cores.len(), 5);
    // Core BN: colour BN (never "BR", D-17), 0.75 mm², from the terminal -X1031:1/b to -201U1:5.
    let bn = w1_cores.iter().find(|t| t[core] == "BN").expect("core BN");
    assert_eq!((bn[colour].as_str(), bn[csa].as_str()), ("BN", "0.75 mm\u{00B2}"));
    assert_eq!((bn[from].as_str(), bn[to].as_str()), ("-X1031:1/b", "-201U1:5"));
    // -3W1 has cores A, B and the shield SH without colour or cross-section.
    let w3_cores: Vec<&str> = all
        .iter()
        .filter(|t| t[cable] == "-3W1" && !t[core].is_empty())
        .map(|t| t[core].as_str())
        .collect();
    assert_eq!(w3_cores, ["A", "B", "SH"]);
}

#[test]
fn the_row_texts_match_the_committed_files() {
    let batch = golden_common::batch();
    let s = golden_common::session_with_profiles(&batch);
    let q = plain(&s);
    let dir = golden_common::repo().join("golden/J-E-2416/expected/rows");
    let bless = std::env::var_os("EW_BLESS").is_some_and(|v| v == "1");
    for key in ["903", "904", "905", "906", "907", "909"] {
        let csv = rows_csv(&q, &rows_of(&q, &batch, key));
        let path = dir.join(format!("{key}.csv"));
        if bless {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, csv.as_bytes()).unwrap();
            continue;
        }
        let committed = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 generates it)", path.display()));
        assert!(
            committed == csv.as_bytes(),
            "{} differs from the generated rows (EW_BLESS=1 rewrites it)",
            path.display()
        );
    }
}

#[test]
fn a_query_with_the_report_engine_paginates_the_connectivity_kinds() {
    let batch = golden_common::batch();
    let s = golden_common::session_with_profiles(&batch);
    let q = s.query();
    let order = q.get::<PageOrder>(());
    for (key, title, folios) in [
        ("903", "Terminal Diagram 1X1", &["903"][..]),
        ("906", "Cable Overview and Diagrams", &["906"][..]),
        ("907", "Wire List 1", &["907", "907.1", "907.2"][..]),
        ("909", "Potential Overview", &["909"][..]),
    ] {
        let id = block(&batch, key);
        let pages: Vec<&GeneratedPage> = order
            .entries
            .iter()
            .filter(|e| matches!(e.page, ew_derive::PageRef::Generated { block, .. } if block == id))
            .filter_map(|e| order.generated(e.page))
            .collect();
        let labels: Vec<&str> = pages.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, folios, "{key}");
        assert_eq!(pages[0].title.as_neutral(), Some(title));
        assert!(
            pages.iter().all(|g| g.note.is_none()),
            "{key}: no placeholder note"
        );
        // The rows read inside the pagination are the derived ones, the same as in a `Q` without the engine.
        assert_eq!(q.get::<BlockRows>(id).status, RowSetStatus::Derived, "{key}");
        assert_eq!(*q.get::<BlockRows>(id), *plain(&s).get::<BlockRows>(id), "{key}");
    }
}

/// What a fresh `Q` with the engine shows of block `key` after its rows and its pages were read in the given
/// order: the rows, the notes of the block's pages and the derivation notes.
fn observe(batch: &ew_import_tables::ew_interop_api::ImportBatch, key: &str, rows_first: bool) -> String {
    let s = golden_common::session_with_profiles(batch);
    let mut q = s.query();
    let id = block(batch, key);
    let (rows, order) = if rows_first {
        let rows = q.get::<BlockRows>(id);
        (rows, q.get::<PageOrder>(()))
    } else {
        let order = q.get::<PageOrder>(());
        (q.get::<BlockRows>(id), order)
    };
    let pages: Vec<(String, Option<String>)> = order
        .entries
        .iter()
        .filter(|e| matches!(e.page, ew_derive::PageRef::Generated { block, .. } if block == id))
        .filter_map(|e| order.generated(e.page))
        .map(|g| {
            (
                g.label.clone(),
                g.note.as_ref().and_then(|n| n.as_neutral()).map(str::to_owned),
            )
        })
        .collect();
    let notes = q.diagnostics().notes();
    format!("{rows:?}\n{pages:?}\n{notes:?}")
}

#[test]
fn rows_and_pages_do_not_depend_on_the_order_they_are_read_in() {
    let batch = golden_common::batch();
    for key in ["903", "904", "905", "906", "907", "909"] {
        let rows_first = observe(&batch, key, true);
        let pages_first = observe(&batch, key, false);
        assert_eq!(rows_first, pages_first, "block {key}");
        // Derived rows, no placeholder note on a page, and no derivation note.
        assert!(!rows_first.contains("NotDerived"), "{key}");
        assert!(!rows_first.contains("Not derived"), "{key}");
        assert!(rows_first.ends_with("[]"), "{key}: no notes, {rows_first}");
    }
    // The order the records are read in: a session of the reversed records gives the same rows.
    let a = golden_common::session_with_profiles(&batch);
    let b = golden_common::session_with(&batch, |p, records| {
        golden_common::with_profiles(p);
        records.reverse();
    });
    let (qa, qb) = (plain(&a), plain(&b));
    for key in ["903", "904", "905", "906", "907", "909"] {
        let id = block(&batch, key);
        let (ra, rb) = (qa.get::<BlockRows>(id), qb.get::<BlockRows>(id));
        assert_eq!(*ra, *rb, "block {key}, reversed records");
    }
}
