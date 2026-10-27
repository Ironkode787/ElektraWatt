// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of the scoring (WP-S6.13 BD15): fixtures of a run folder, canned outputs of the copied `ew`, each status and
//! each outcome.

use serde_json::json;

use super::*;
use crate::blind::run::{Kind, script_header, step_line, turns_header};
use crate::blind::testkit::{Tmp, new_run_json};

// ---- A small task: folio 101 with two reps, folio 102 with one, folio 103 the demo variant.

fn row(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|s| (*s).to_owned()).collect()
}

fn count(folio: &str, reps: u32, connections: u32, junctions: u32) -> FolioCount {
    FolioCount {
        folio: folio.to_owned(),
        reps,
        connections,
        junctions,
        source: "test".to_owned(),
        ..FolioCount::default()
    }
}

fn task() -> TaskDef {
    let mut counts = vec![
        count("101", 2, 1, 0),
        count("102", 1, 0, 1),
        count("103", 2, 0, 0),
    ];
    counts[0].budget_turns = Some(250);
    counts[2].terminals = Some(1);
    counts[2].strips = Some(1);
    let mut rows = BTreeMap::new();
    rows.insert(
        "cells".to_owned(),
        vec![
            row(&["101", "S51", "sym.a", "101-B2"]),
            row(&["101", "L-1", "sym.ip", "101-H16"]),
            row(&["102", "-102G1", "sym.box", "102-B6"]),
            // The golden strip segment of folio 103 is left out of S1 on both sides.
            row(&["103", "-X1031:1", "ew.gen.strip-segment@qet10", "103-A3"]),
            row(&["103", "L-1", "sym.ip", "103-A1"]),
        ],
    );
    rows.insert("ends".to_owned(), vec![row(&["101", "S51:1", "ext L-1", "101"])]);
    rows.insert(
        "junctions".to_owned(),
        vec![row(&[
            "102",
            "bevel",
            "chain",
            "-102G1:+#1 | -102F1:1 | -102G1:+#2",
        ])],
    );
    rows.insert(
        "xrefs".to_owned(),
        vec![row(&["101", "L-1", "102-A1"]), row(&["101", "PE-1.1", "106-A1"])],
    );
    rows.insert(
        "tags".to_owned(),
        vec![
            row(&["101", "normal", "S51"]),
            row(&["101", "ip", "L-1"]),
            row(&["102", "black_box", "-102G1"]),
        ],
    );
    rows.insert(
        "pages".to_owned(),
        vec![row(&["101", "Main"]), row(&["102", "Control"])],
    );
    rows.insert("markers".to_owned(), vec![row(&["101", "2.5 BK BU"])]);
    rows.insert("findings".to_owned(), vec![row(&["EW-NUM-001", "info", "S51"])]);
    let entry = |action: &str| PreludeEntry {
        action: action.to_owned(),
        class: "setup".to_owned(),
        gap: None,
    };
    let gap = |check: &str, gap: &str, reason: &str| {
        (
            check.to_owned(),
            GapEntry {
                check: check.to_owned(),
                gap: gap.to_owned(),
                reason: reason.to_owned(),
            },
        )
    };
    TaskDef {
        counts,
        bom: vec![
            BomRow {
                part: "533356".to_owned(),
                description: "Main switch".to_owned(),
                qty: "1".to_owned(),
            },
            BomRow {
                part: "TBD".to_owned(),
                description: "Plug-in bridge".to_owned(),
                qty: "3".to_owned(),
            },
        ],
        rows,
        prelude: vec![entry("file.new"), entry("command.run")],
        gaps: BTreeMap::from([
            gap("H4", "WP-S2.07", "part selection"),
            gap("S7", "FT-999", "markers"),
        ]),
        queries: BTreeMap::new(),
    }
}

fn page_101() -> String {
    [
        "page \"101\" \"Main\" reps 2 connections 1 junctions 0",
        "  rep \"S51\" cell \"101-B2\" symbol \"sym.a\" variant \"A\" at 1.000 2.000 mm",
        "    cp \"S51:1\" at 1.000 2.000 mm wires 1",
        "  rep ip \"L-1\" direction source cell \"101-H16\" symbol \"sym.ip\" variant \"A\" at 3.000 4.000 mm",
        "  wire \"S51:1\" -> \"L-1\" route stored bends 0",
        "  item marker cell \"101-C3\" \"2.5 BK BU\"",
    ]
    .join("\n")
}

fn page_102() -> String {
    [
        "page \"102\" \"Control\" reps 1 connections 0 junctions 1",
        "  rep \"-102G1\" cell \"102-B6\" symbol \"sym.box\" variant \"A\" at 1.000 2.000 mm",
        "  junction 1 cell \"102-C6\" at 1.000 2.000 mm style bevel wiring chain sequence \"-102G1:+#1\" \"-102F1:1\" \"-102G1:+#2\"",
    ]
    .join("\n")
}

fn page_103() -> String {
    [
        "page \"103\" \"Dist\" reps 2 connections 0 junctions 0",
        "  rep ip \"L-1\" direction target cell \"103-A1\" symbol \"sym.ip\" variant \"A\" at 1.000 2.000 mm",
        "  rep \"-X1031:1\" cell \"103-A9\" symbol \"ew.iec.terminal@qet10\" variant \"A\" at 1.000 2.000 mm",
    ]
    .join("\n")
}

fn sql_json(columns: &[&str], rows: &[&[&str]]) -> String {
    json!({"$schema": "ew.sql-result/1", "columns": columns, "rows": rows}).to_string()
}

fn turn(kind: &str, n: u32, folio: Option<&str>, steps: Option<[u32; 2]>) -> Turn {
    Turn {
        n,
        kind: kind.to_owned(),
        turn: Some(n),
        steps,
        folio: folio.map(str::to_owned),
        ..Turn::default()
    }
}

/// A drafter run that did everything right on all three folios: the evidence of a `pass`.
fn perfect(work: &Path) -> Evidence {
    let mut rj = new_run_json("gpp-r1", Kind::Gate);
    rj.task = "gpp".to_owned();
    rj.folios = vec!["101".to_owned(), "102".to_owned(), "103".to_owned()];
    rj.end = Some("handback".to_owned());
    rj.handed_back = true;
    rj.clean_before_epilogue = Some(true);
    rj.child_exit = Some(0);
    let mut ev = Evidence::new(rj);
    ev.agent_steps = vec![
        AgentStep {
            action: "file.save".to_owned(),
            params: json!({}),
            outcome: "done".to_owned(),
        },
        AgentStep {
            action: "file.export_pdf".to_owned(),
            params: json!({"path": "set.pdf"}),
            outcome: "done".to_owned(),
        },
    ];
    ev.turns = vec![
        turn("do", 1, Some("101"), Some([1, 2])),
        turn("do", 2, Some("102"), Some([3, 3])),
        turn("do", 3, Some("103"), Some([4, 4])),
    ];
    ev.work = work.to_path_buf();
    std::fs::write(work.join("set.pdf"), b"%PDF-").unwrap();
    ev.epilogue = BTreeMap::from([
        ("101".to_owned(), page_101()),
        ("102".to_owned(), page_102()),
        ("103".to_owned(), page_103()),
    ]);
    ev.bom_csv = Some(
        "row,Pos,Qty,Unit,Manufacturer,Part number,Description,Tags,Folios\n\
         data,1,1,pc,Eaton,533356,Main switch,S51,101\n\
         data,2,3,pc,TBD,TBD,Plug-in bridge,\"-X1031\",103\n"
            .to_owned(),
    );
    ev.check_json = Some(
        json!({"findings": [
            {"rule": "EW-NUM-001", "severity": "info", "subject": "=P01+J-E-2416-S51", "state": "open"},
            {"rule": "EW-CORE-033", "severity": "error", "subject": "=P01+J-E-2416-3W1", "state": "open"},
        ]})
        .to_string(),
    );
    ev.sql.insert(
        "ends".to_owned(),
        Ok(sql_json(
            &["folio", "end_1", "end_2", "span"],
            &[&["101", "S51:1", "ext L-1", "101"]],
        )),
    );
    ev.sql.insert(
        "xrefs".to_owned(),
        Ok(sql_json(&["folio", "tag", "xref"], &[&["101", "L-1", "102-A1"]])),
    );
    ev.sql.insert(
        "tags".to_owned(),
        Ok(sql_json(
            &["folio", "kind", "tag"],
            &[
                &["101", "normal", "S51"],
                &["101", "ip", "L-1"],
                &["102", "black_box", "-102G1"],
            ],
        )),
    );
    ev.sql.insert(
        "pages".to_owned(),
        Ok(sql_json(
            &["folio", "title"],
            &[&["101", "Main"], &["102", "Control"]],
        )),
    );
    ev.sql.insert(
        "terminals".to_owned(),
        Ok(sql_json(&["terminals", "strips"], &[&["1", "1"]])),
    );
    ev.prelude_actions = vec!["file.new".to_owned(), "command.run".to_owned()];
    ev
}

fn by_id<'a>(s: &'a Score, id: &str) -> &'a Check {
    s.hard
        .iter()
        .chain(&s.soft)
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id}"))
}

/// Every status and every outcome of the scoring on a fixture task: `ok`, `miss`, `gap`; `pass`, `pass-with-gaps`,
/// `fail`, `incomplete`.
#[test]
fn a_run_scores_each_status_and_each_outcome() {
    let tmp = Tmp::new("score-outcomes");
    let t = task();
    // A perfect run: every check ok (the xref of folio 101 whose partner lies on folio 106 is left out of S4).
    let ev = perfect(&tmp.0);
    let s = score(Some(&t), &ev);
    for c in s.hard.iter().chain(&s.soft) {
        assert_eq!(c.status, Status::Ok, "{}: {:?}", c.id, c.misses);
    }
    assert_eq!(s.outcome, Some(Outcome::Pass));
    assert_eq!(s.hard.len(), 7, "H1 to H3 by the folios, then H4 to H7");
    assert_eq!(s.soft.len(), 8);
    assert_eq!(s.protocol.prelude, PreludeUse { setup: 2, drawing: 0 });
    assert_eq!(
        s.protocol.turns_by_folio["101"],
        FolioUse {
            turns: 1,
            budget: Some(250)
        }
    );

    // `miss` and `fail`: folio 102 has two representations where the oracle wants one.
    let mut bad = perfect(&tmp.0);
    bad.epilogue
        .insert("102".to_owned(), page_102().replace("reps 1 ", "reps 2 "));
    let s = score(Some(&t), &bad);
    let h2 = by_id(&s, "H2");
    assert_eq!(h2.status, Status::Miss);
    assert_eq!(h2.misses, ["reps: want 1, got 2"]);
    assert_eq!(h2.want, "reps 1 connections 0 junctions 1");
    assert_eq!(h2.got, "reps 2 connections 0 junctions 1");
    assert_eq!(s.outcome, Some(Outcome::Fail));

    // `gap` and `pass-with-gaps`: no parts list, and the task lists H4 as blocked by WP-S2.07.
    let mut gapped = perfect(&tmp.0);
    gapped.bom_csv = None;
    let s = score(Some(&t), &gapped);
    let h4 = by_id(&s, "H4");
    assert_eq!((h4.status, h4.gap.as_deref()), (Status::Gap, Some("WP-S2.07")));
    assert!(h4.misses[0].contains("no parts list"), "{:?}", h4.misses);
    assert_eq!(s.outcome, Some(Outcome::PassWithGaps));
    // A miss next to a gap still fails.
    let mut both = bad.clone();
    both.bom_csv = None;
    assert_eq!(score(Some(&t), &both).outcome, Some(Outcome::Fail));

    // `incomplete`: every cause the spec names, and a folio that was never started.
    for end in ["cap", "quit", "orchestrator", "lost", "diverged"] {
        let mut e = perfect(&tmp.0);
        e.rj.end = Some(end.to_owned());
        assert_eq!(score(Some(&t), &e).outcome, Some(Outcome::Incomplete), "{end}");
    }
    let mut unstarted = perfect(&tmp.0);
    unstarted.turns.retain(|t| t.folio.as_deref() != Some("103"));
    assert_eq!(score(Some(&t), &unstarted).outcome, Some(Outcome::Incomplete));
}

/// A gap listed for a check that is `ok` is `ok`.
#[test]
fn a_gap_listed_for_a_check_that_is_ok_is_ok() {
    let tmp = Tmp::new("score-gap-ok");
    let ev = perfect(&tmp.0);
    // H4 is listed as a gap in the task and the parts list is right: ok, no gap named.
    let s = score(Some(&task()), &ev);
    let h4 = by_id(&s, "H4");
    assert_eq!((h4.status, h4.gap.clone()), (Status::Ok, None));
    // S7 is listed too, and the markers match.
    assert_eq!(by_id(&s, "S7").status, Status::Ok);
    // The same check, not ok, is a gap with the listed follow-up.
    let mut ev = perfect(&tmp.0);
    ev.epilogue.insert(
        "101".to_owned(),
        page_101().replace("  item marker cell \"101-C3\" \"2.5 BK BU\"", ""),
    );
    let s = score(Some(&task()), &ev);
    let s7 = by_id(&s, "S7");
    assert_eq!((s7.status, s7.gap.as_deref()), (Status::Gap, Some("FT-999")));
    assert_eq!(s7.misses, ["missing: 101 | 2.5 BK BU"]);
}

/// H5 and H6 read the agent's own steps: an epilogue save makes H6 `miss`, a refused export or a missing file makes H5
/// `miss`.
#[test]
fn h5_and_h6_read_the_agents_own_steps() {
    let tmp = Tmp::new("score-h5-h6");
    let t = task();
    // The agent saved and exported: both ok.
    let ev = perfect(&tmp.0);
    let s = score(Some(&t), &ev);
    assert_eq!(by_id(&s, "H5").status, Status::Ok);
    assert_eq!(by_id(&s, "H6").status, Status::Ok);
    assert_eq!(by_id(&s, "H6").got, "1 saves done, project clean");
    // The agent never saved: the project was dirty and the epilogue saved it. H6 is a miss.
    let mut e = perfect(&tmp.0);
    e.agent_steps.retain(|s| s.action != "file.save");
    e.rj.clean_before_epilogue = Some(false);
    let s = score(Some(&t), &e);
    let h6 = by_id(&s, "H6");
    assert_eq!(h6.status, Status::Miss);
    assert_eq!(
        h6.misses.len(),
        2,
        "no save step, and dirty before the epilogue: {:?}",
        h6.misses
    );
    assert!(h6.misses[1].contains("the epilogue saved it"));
    // The agent saved, then changed something and left it dirty: the epilogue's save does not count.
    let mut e = perfect(&tmp.0);
    e.rj.clean_before_epilogue = Some(false);
    let s = score(Some(&t), &e);
    assert_eq!(by_id(&s, "H6").status, Status::Miss);
    assert_eq!(by_id(&s, "H6").misses.len(), 1);
    // A refused save is not a save.
    let mut e = perfect(&tmp.0);
    e.agent_steps[0].outcome = "refused".to_owned();
    assert_eq!(by_id(&score(Some(&t), &e), "H6").status, Status::Miss);
    // The export: failed, absent from work/, present under another name, and without a path.
    let mut e = perfect(&tmp.0);
    e.agent_steps[1].outcome = "failed".to_owned();
    assert_eq!(by_id(&score(Some(&t), &e), "H5").status, Status::Miss);
    let mut e = perfect(&tmp.0);
    e.agent_steps[1].params = json!({"path": "other.pdf"});
    let s = score(Some(&t), &e);
    assert_eq!(by_id(&s, "H5").status, Status::Miss);
    assert!(by_id(&s, "H5").misses[0].contains("not in the work folder"));
    let mut e = perfect(&tmp.0);
    e.agent_steps[1].params = json!({});
    assert_eq!(
        by_id(&score(Some(&t), &e), "H5").status,
        Status::Ok,
        "a PDF in work/ stands for the default name"
    );
    // An export made by the harness (not among the agent's steps) is no export of the agent's.
    let mut e = perfect(&tmp.0);
    e.agent_steps.retain(|s| s.action != "file.export_pdf");
    assert_eq!(by_id(&score(Some(&t), &e), "H5").status, Status::Miss);
}

/// H7 counts the prelude's `drawing` lines, and reports a prelude.toml that does not fit the prelude.
#[test]
fn h7_counts_the_drawing_lines_of_the_prelude() {
    let tmp = Tmp::new("score-h7");
    let mut t = task();
    t.prelude[1] = PreludeEntry {
        action: "command.run".to_owned(),
        class: "drawing".to_owned(),
        gap: Some("FT-408".to_owned()),
    };
    let s = score(Some(&t), &perfect(&tmp.0));
    let h7 = by_id(&s, "H7");
    assert_eq!(h7.status, Status::Miss);
    assert_eq!(h7.got, "1 drawing lines");
    assert!(
        h7.misses[0].contains("command.run") && h7.misses[0].contains("FT-408"),
        "{:?}",
        h7.misses
    );
    assert_eq!(s.protocol.prelude, PreludeUse { setup: 1, drawing: 1 });
    // Listed as a gap, the drawing line is a gap, not a miss.
    t.gaps.insert(
        "H7".to_owned(),
        GapEntry {
            check: "H7".to_owned(),
            gap: "FT-408".to_owned(),
            reason: "pages".to_owned(),
        },
    );
    let s = score(Some(&t), &perfect(&tmp.0));
    assert_eq!(
        (by_id(&s, "H7").status, by_id(&s, "H7").gap.as_deref()),
        (Status::Gap, Some("FT-408"))
    );
    assert_eq!(s.outcome, Some(Outcome::PassWithGaps));
    // A prelude with another number of steps than prelude.toml has entries.
    let mut e = perfect(&tmp.0);
    e.prelude_actions.pop();
    let s = score(Some(&task()), &e);
    assert!(by_id(&s, "H7").misses[0].contains("2 entries for 1 prelude steps"));
}

/// S1 reads the cell of every representation from the epilogue's `page.describe` text (an interruption point and a
/// device alike, as many as the page has) and leaves the terminals of folio 103 out on both sides.
#[test]
fn s1_reads_every_representations_cell_from_page_describe() {
    let tmp = Tmp::new("score-s1");
    let t = task();
    let s = score(Some(&t), &perfect(&tmp.0));
    let s1 = by_id(&s, "S1");
    assert_eq!(s1.status, Status::Ok);
    // 5 expected rows less the strip segment; the agent's terminal of folio 103 is left out too.
    assert_eq!(s1.want, "4 rows");
    assert_eq!(s1.got, "4 rows");
    // A device in the wrong cell and an interruption point in the wrong cell: one missing and one extra row each.
    let mut e = perfect(&tmp.0);
    e.epilogue.insert(
        "101".to_owned(),
        page_101()
            .replace("101-B2", "101-B3")
            .replace("101-H16", "101-H15"),
    );
    let s = score(Some(&t), &e);
    let s1 = by_id(&s, "S1");
    assert_eq!(s1.status, Status::Miss);
    // The rows sort by their text (the named tie-break), a wrong cell is a pair.
    assert_eq!(
        s1.misses,
        [
            "extra: 101 | L-1 | sym.ip | 101-H15",
            "missing: 101 | L-1 | sym.ip | 101-H16",
            "missing: 101 | S51 | sym.a | 101-B2",
            "extra: 101 | S51 | sym.a | 101-B3",
        ]
    );
    // A representation that has no tag prints `rep none`: it is a row with the label `none`.
    let rows = cell_rows(
        "102",
        "page \"102\" \"x\" reps 1 connections 0 junctions 0\n  rep none cell \"102-D5\" symbol \"sym.x\" variant \"A\" at 1.000 2.000 mm",
    );
    assert_eq!(rows, [row(&["102", "none", "sym.x", "102-D5"])]);
    // The other page-text readers: junction sequences and marker texts.
    assert_eq!(
        junction_rows("102", &page_102()),
        [row(&[
            "102",
            "bevel",
            "chain",
            "-102G1:+#1 | -102F1:1 | -102G1:+#2"
        ])]
    );
    assert_eq!(marker_rows("101", &page_101()), [row(&["101", "2.5 BK BU"])]);
    // A missing epilogue text for a folio: H2 says so.
    let mut e = perfect(&tmp.0);
    e.epilogue.remove("102");
    let s = score(Some(&t), &e);
    assert_eq!(by_id(&s, "H2").got, "no page text");
    assert_eq!(by_id(&s, "H2").status, Status::Miss);
}

/// `--folios` leaves folios out: their counts are `gap` "not in this run", the soft rows of the other folios are not
/// asked for, the parts list is a gap, and the outcome can still be `pass-with-gaps`.
#[test]
fn folios_left_out_of_the_run_are_gaps_not_misses() {
    let tmp = Tmp::new("score-folios");
    let t = task();
    let mut e = perfect(&tmp.0);
    e.rj.folios = vec!["101".to_owned()];
    e.epilogue.retain(|f, _| f == "101");
    e.turns.retain(|t| t.folio.as_deref() == Some("101"));
    e.bom_csv = Some(
        "row,Pos,Qty,Unit,Manufacturer,Part number,Description,Tags,Folios\ndata,1,1,pc,Eaton,533356,Main switch,S51,101\n"
            .to_owned(),
    );
    let s = score(Some(&t), &e);
    for id in ["H2", "H3"] {
        let c = by_id(&s, id);
        assert_eq!(
            (c.status, c.gap.as_deref()),
            (Status::Gap, Some("not in this run")),
            "{id}"
        );
    }
    // The parts list of one folio cannot equal the oracle's: a gap, whatever gaps.toml says.
    let h4 = by_id(&s, "H4");
    assert_eq!(
        (h4.status, h4.gap.as_deref()),
        (Status::Gap, Some("not in this run"))
    );
    assert_eq!(by_id(&s, "H1").status, Status::Ok);
    // S1, S3, S5 and S6 ask for folio 101 only, and hold.
    for id in ["S1", "S3", "S5", "S6"] {
        assert_eq!(
            by_id(&s, id).status,
            Status::Ok,
            "{id}: {:?}",
            by_id(&s, id).misses
        );
    }
    assert_eq!(s.outcome, Some(Outcome::PassWithGaps));
    assert_eq!(s.protocol.folios, ["101"]);
}

/// The soft checks list at most 50 differing rows, then a count; the ordering is by row text.
#[test]
fn soft_misses_are_capped_at_fifty_and_counted() {
    let tmp = Tmp::new("score-cap");
    let mut ev = perfect(&tmp.0);
    let rows: Vec<Vec<String>> = (0..60)
        .map(|i| vec!["101".to_owned(), format!("T{i:02}"), format!("{i}")])
        .collect();
    let strs: Vec<Vec<&str>> = rows
        .iter()
        .map(|r| r.iter().map(String::as_str).collect())
        .collect();
    let refs: Vec<&[&str]> = strs.iter().map(Vec::as_slice).collect();
    ev.sql
        .insert("xrefs".to_owned(), Ok(sql_json(&["folio", "tag", "xref"], &refs)));
    let s = score(Some(&task()), &ev);
    let s4 = by_id(&s, "S4");
    assert_eq!(s4.status, Status::Miss);
    assert_eq!(s4.misses.len(), MAX_MISSES);
    assert_eq!(s4.more, 11, "61 differing rows: 1 missing, 60 extra");
    assert!(s4.misses[0].starts_with("missing: ") && s4.misses[1] == "extra: 101 | T00 | 0");
    let text = score_json(&s).unwrap();
    assert!(text.contains("\"more\": 11"), "{text}");
    // A check that is `ok` carries no `more` member.
    assert!(
        !score_json(&score(Some(&task()), &perfect(&tmp.0)))
            .unwrap()
            .contains("\"more\"")
    );
}

/// S2 and S4 keep the rows of the folios of the run; a wire that spans a folio the run does not draw, and a text whose
/// partner is on such a folio, are left out (AC-05's rule).
#[test]
fn s2_and_s4_leave_out_what_ends_on_a_folio_that_is_not_drawn() {
    let tmp = Tmp::new("score-s2-s4");
    let mut t = task();
    t.rows
        .get_mut("ends")
        .unwrap()
        .push(row(&["101", "A:1", "B:2", "101,102"]));
    let mut e = perfect(&tmp.0);
    e.rj.folios = vec!["101".to_owned()];
    e.epilogue.retain(|f, _| f == "101");
    e.turns.retain(|t| t.folio.as_deref() == Some("101"));
    let s = score(Some(&t), &e);
    // Expected: the wire on 101 only (the span 101,102 is out); the xref L-1 -> 102-A1 is out too (partner on 102).
    assert_eq!(by_id(&s, "S2").status, Status::Ok, "{:?}", by_id(&s, "S2").misses);
    assert_eq!(by_id(&s, "S2").want, "1 rows");
    let s4 = by_id(&s, "S4");
    assert_eq!(s4.want, "0 rows");
    assert_eq!(
        s4.status,
        Status::Miss,
        "the agent's own row is extra: {:?}",
        s4.misses
    );
    assert_eq!(xref_folios("∞ 3-B2, 106-G3"), ["3", "106"]);
    assert_eq!(xref_folios("+J-E-206 / JE-N1000-03"), Vec::<String>::new());
    assert_eq!(xref_folios("102-A1"), ["102"]);
    assert_eq!(xref_folios("24VDC-1 0VDC-AIR1"), Vec::<String>::new());
}

/// S8 reads `out/check.json` through the tags of the drawn devices: a finding on a device that is not drawn, or on a
/// cable, is not a row.
#[test]
fn s8_reads_the_check_report_through_the_drawn_tags() {
    let tmp = Tmp::new("score-s8");
    let mut ev = perfect(&tmp.0);
    let s = score(Some(&task()), &ev);
    assert_eq!(by_id(&s, "S8").status, Status::Ok);
    // A new finding on a drawn device is an extra row.
    ev.check_json = Some(
        json!({"findings": [
            {"rule": "EW-NUM-001", "severity": "info", "subject": "=P01+J-E-2416-S51", "state": "open"},
            {"rule": "EW-CORE-041", "severity": "warning", "subject": "=P01+J-E-2416-S51", "state": "open"},
        ]})
        .to_string(),
    );
    let s = score(Some(&task()), &ev);
    assert_eq!(by_id(&s, "S8").misses, ["extra: EW-CORE-041 | warning | S51"]);
    // No check report: the check says so.
    ev.check_json = None;
    let s = score(Some(&task()), &ev);
    assert!(by_id(&s, "S8").misses[0].contains("no check report"));
    assert_eq!(
        finding_tag(
            "=P01+J-E-2416-101F1",
            &BTreeSet::from(["-101F1".to_owned(), "F1".to_owned()])
        ),
        Some("-101F1".to_owned())
    );
    assert_eq!(
        finding_tag("=P01+J-E-2416-X", &BTreeSet::from(["S51".to_owned()])),
        None
    );
}

/// A query that failed is a miss with its message, not a crash; H3's terminal count comes from its query.
#[test]
fn a_failed_query_is_a_miss_with_its_message() {
    let tmp = Tmp::new("score-sql");
    let mut ev = perfect(&tmp.0);
    ev.sql
        .insert("pages".to_owned(), Err("exit 2: denied".to_owned()));
    ev.sql.insert(
        "terminals".to_owned(),
        Ok(sql_json(&["terminals", "strips"], &[&["2", "1"]])),
    );
    let s = score(Some(&task()), &ev);
    assert_eq!(
        by_id(&s, "S6").misses,
        ["the `pages` query failed: exit 2: denied"]
    );
    let h3 = by_id(&s, "H3");
    assert_eq!(h3.status, Status::Miss);
    assert_eq!(h3.misses, ["terminals: want 1, got 2"]);
    assert_eq!(h3.got, "reps 2 connections 0 junctions 0 terminals 2 strips 1");
    ev.sql.remove("terminals");
    assert!(by_id(&score(Some(&task()), &ev), "H3").misses[0].contains("not run"));
}

/// A free run (no oracle) holds only its protocol.
#[test]
fn a_free_run_holds_only_its_protocol() {
    let tmp = Tmp::new("score-free");
    let mut ev = perfect(&tmp.0);
    ev.rj.kind = "free".to_owned();
    ev.rj.folios.clear();
    let s = score(None, &ev);
    assert_eq!(s.outcome, None);
    assert!(s.hard.is_empty() && s.soft.is_empty());
    assert_eq!(s.protocol.end.as_deref(), Some("handback"));
    let md = score_markdown(&s);
    assert!(md.contains("holds no oracle"), "{md}");
    let json = score_json(&s).unwrap();
    assert!(
        json.contains("\"$schema\": \"ew.blindscore/1\"") && json.contains("\"outcome\": null"),
        "{json}"
    );
    // The round trip.
    let back: Score = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

/// The Markdown of a scored run names the outcome, the checks and the misses.
#[test]
fn the_score_as_markdown_names_the_outcome_the_gaps_and_the_misses() {
    let tmp = Tmp::new("score-md");
    let mut ev = perfect(&tmp.0);
    ev.bom_csv = None;
    ev.epilogue
        .insert("102".to_owned(), page_102().replace("reps 1 ", "reps 5 "));
    let md = score_markdown(&score(Some(&task()), &ev));
    assert!(md.contains("Outcome: **fail**"), "{md}");
    assert!(md.contains("| H4 |") && md.contains("gap (WP-S2.07)"), "{md}");
    assert!(md.contains("- reps: want 1, got 5"), "{md}");
    assert!(md.contains("### Soft checks"), "{md}");
}

// ---- The files of a run: the agent's steps and the epilogue are read from the run's folder.

#[test]
fn the_agents_steps_and_the_epilogue_are_read_from_the_runs_files() {
    let tmp = Tmp::new("score-files");
    let run = crate::blind::testkit::test_run(&tmp);
    let steps = [
        ("ui.describe", json!({})),
        ("file.save", json!({})),
        ("file.export_pdf", json!({"path": "set.pdf"})),
        ("page.describe", json!({"folio": "101"})),
        ("file.save", json!({})),
        ("app.quit", json!({"discard": true})),
    ];
    let mut script = format!("{}\n", script_header("steps"));
    for (a, p) in &steps {
        script.push_str(&step_line(a, p));
        script.push('\n');
    }
    std::fs::write(run.path("steps.uiscript"), script).unwrap();
    // The transcript: the prelude's line (file.new), then one line per step.
    let line = |action: &str, outcome: &str, describe: Option<&str>| {
        let mut v = json!({"step": 1, "action": action, "outcome": outcome});
        if let Some(d) = describe {
            v["describe"] = json!(d);
        }
        v.to_string()
    };
    let mut transcript = String::from("{\"$schema\":\"ew.uitranscript/1\",\"version\":\"0\"}\n");
    for l in [
        line("file.new", "done", None),
        line("ui.describe", "done", Some("title: x")),
        line("file.save", "done", None),
        line("file.export_pdf", "refused", None),
        line("page.describe", "done", Some(&page_101())),
        line("file.save", "done", None),
        line("app.quit", "done", None),
    ] {
        transcript.push_str(&l);
        transcript.push('\n');
    }
    std::fs::write(run.path("transcript.jsonl"), transcript).unwrap();
    let mut turns = format!("{}\n", turns_header());
    for t in [
        turn("look", 1, Some("101"), Some([1, 1])),
        turn("do", 1, Some("101"), Some([2, 3])),
        turn("epilogue", 2, None, Some([4, 6])),
    ] {
        turns.push_str(&serde_json::to_string(&t).unwrap());
        turns.push('\n');
    }
    std::fs::write(run.path("turns.jsonl"), turns).unwrap();
    std::fs::create_dir_all(run.path("out")).unwrap();
    std::fs::write(run.path("out/bom.csv"), "row,Qty\n").unwrap();
    let ev = evidence_of(
        &run,
        None,
        ExportFacts {
            pdfs_identical: Some(true),
            ew_check_exit: Some(1),
        },
    )
    .unwrap();
    // Only the steps of the agent's `do` turns: not the harness's look, not the epilogue's save.
    let got: Vec<(&str, &str)> = ev
        .agent_steps
        .iter()
        .map(|s| (s.action.as_str(), s.outcome.as_str()))
        .collect();
    assert_eq!(got, [("file.save", "done"), ("file.export_pdf", "refused")]);
    assert_eq!(ev.epilogue.keys().collect::<Vec<_>>(), ["101"]);
    assert!(ev.epilogue["101"].starts_with("page \"101\""));
    assert_eq!(ev.prelude_actions, ["file.new"]);
    assert_eq!(ev.bom_csv.as_deref(), Some("row,Qty\n"));
    assert!(ev.check_json.is_none());
    assert_eq!((ev.pdfs_identical, ev.ew_check_exit), (Some(true), Some(1)));
    // And `score_run` with no oracle writes the protocol-only score.
    let s = score_run(&tmp.0, &run, ExportFacts::default()).unwrap();
    assert_eq!(s.outcome, None);
    let written = std::fs::read_to_string(run.path("out/score.json")).unwrap();
    assert_eq!(serde_json::from_str::<Score>(&written).unwrap(), s);
}

// ---- The committed task and the text helpers.

#[test]
fn the_committed_task_loads_and_its_counts_are_the_documents() {
    let t = TaskDef::load(&crate::workspace_root(), "gpp-101-103").unwrap();
    let counts: Vec<_> = t
        .counts
        .iter()
        .map(|c| {
            (
                c.folio.as_str(),
                c.reps,
                c.connections,
                c.junctions,
                c.terminals,
                c.strips,
            )
        })
        .collect();
    // WP-S2.04 §13.6: 17, 12 and 23 reps less the 3 strip segments plus the 15 terminals of the demo variant.
    assert_eq!(
        counts,
        [
            ("101", 17, 21, 2, None, None),
            ("102", 12, 19, 4, None, None),
            ("103", 35, 20, 0, Some(15), Some(2)),
        ]
    );
    assert_eq!(
        t.bom.len(),
        14,
        "doc 20 §14.1 restricted to the devices on 101 to 103"
    );
    assert_eq!(t.prelude.len(), 5);
    assert!(t.prelude.iter().all(|p| p.class == "setup"));
    for (_, set, _) in SOFT {
        assert!(!t.rows[set].is_empty(), "{set}");
    }
    assert_eq!(t.rows["cells"].len(), 52, "17 + 12 + 23 representations");
    for q in QUERIES {
        assert!(t.queries.contains_key(q), "{q}");
    }
    assert!(t.queries.contains_key("placed_terminals"));
    assert_eq!(t.gaps.keys().collect::<Vec<_>>(), ["H4"]);
}

#[test]
fn csv_and_the_query_file_round_trip() {
    let rows = vec![row(&["a", "b,c", "say \"x\""]), row(&["multi\nline", "", "end"])];
    let text = write_csv(&rows);
    assert_eq!(text, "a,\"b,c\",\"say \"\"x\"\"\"\n\"multi\nline\",,end\n");
    assert_eq!(parse_csv(&text), rows);
    assert_eq!(parse_csv("x,y\r\n1,2"), [row(&["x", "y"]), row(&["1", "2"])]);
    assert!(parse_csv("").is_empty());
    // The parts list: the export's columns by name, only the data rows, sorted.
    let rows = parse_csv(
        "row,Pos,Qty,Part number,Description\nheading,,,,Bridges\ndata,2,1,B,Two\ndata,1,2,A,One\n",
    );
    let bom = bom_rows(&rows).unwrap();
    assert_eq!(
        bom.iter().map(|b| b.part.as_str()).collect::<Vec<_>>(),
        ["A", "B"]
    );
    assert!(bom_rows(&parse_csv("Pos,Qty\n1,1\n")).is_err());
    // The queries: a marker line starts each, comments and the closing semicolon go.
    let q =
        parse_queries("-- header\n-- query: one\n-- a comment\nSELECT 1;\n\n-- query: two\nSELECT\n  2\n;\n")
            .unwrap();
    assert_eq!(q["one"], "SELECT 1");
    assert_eq!(q["two"], "SELECT\n  2");
    for bad in [
        "SELECT 1",
        "-- query:\nSELECT 1",
        "-- query: a\n-- query: b\nSELECT 1",
        "-- query: a\nSELECT 1\n-- query: a\nSELECT 2",
    ] {
        assert!(parse_queries(bad).is_err(), "{bad}");
    }
    // The `ew sql` JSON: every cell as text, null empty.
    let sql = sql_json(&["a", "b"], &[&["1", "x"]]);
    assert_eq!(parse_sql_json(&sql).unwrap(), [row(&["1", "x"])]);
    assert_eq!(
        parse_sql_json("{\"rows\": [[null, 3, true, \"t\"]]}").unwrap(),
        [row(&["", "3", "true", "t"])]
    );
    assert!(parse_sql_json("nope").is_err());
    // The header counts of a describe text, with a title that holds the word `reps`.
    assert_eq!(
        header_counts("page \"1\" \"The reps of it\" reps 3 connections 4 junctions 5 items 1 boxes 0"),
        Some((3, 4, 5))
    );
    assert_eq!(header_counts("not a page"), None);
}
