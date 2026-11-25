// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's geometry and its layout measures on the sample project: its drawn pages break no hard layout rule, a
//! wire's words name its ends. Named items only, never totals. The layout rules (EW-CORE-067 to -070, -072 and
//! -073) on hand-built pages and on the sample (RK1, RK2).

use crate::common;

use ew_api::Session;
use ew_api::output::LayoutError;

/// The sample's supply page 101, drawn as a draughtsman would: the page breaks no hard rule, and the whole-project
/// report measures it alike.
#[test]
fn folio_101_breaks_no_hard_rule() {
    let s = common::sample();
    let pages = s.layout_report(Some("101")).unwrap();
    assert_eq!(pages.len(), 1);
    let p = &pages[0];
    assert_eq!(p.folio, "101");
    assert_eq!(p.hard(), 0, "{p:#?}");
    // The page as the whole-project report measures it.
    let all = s.layout_report(None).unwrap();
    assert_eq!(all.iter().find(|q| q.folio == "101"), Some(p));
}

/// A wire's words name each end by its device's tag with its prefix: on folio 101 the wire from the terminal
/// -X1011:L1's clamp a to the main switch's first pin.
#[test]
fn a_wire_s_words_name_each_device_with_its_tag_s_prefix() {
    let s = common::sample();
    let tables = s.page_tables("101").unwrap();
    let switch = s.resolve_reference("-101Q1").unwrap();
    let terminal = s.resolve_reference("-X1011:L1").unwrap();
    let function = |d| {
        tables
            .reps
            .iter()
            .find(|r| r.device == Some(d))
            .unwrap()
            .functions[0]
    };
    let (q1, l1) = (function(switch), function(terminal));
    let end = |e: &ew_api::output::TableEnd, f| matches!(e, ew_api::output::TableEnd::Cp { function, .. } if *function == f);
    let wire = tables
        .connections
        .iter()
        .find(|c| (end(&c.a, l1) && end(&c.b, q1)) || (end(&c.a, q1) && end(&c.b, l1)))
        .expect("the wire from -X1011:L1 to -101Q1")
        .id;
    assert_eq!(
        s.connection_words("101", wire).map(|t| t.to_string()).as_deref(),
        Some("-X1011:L1/a → -101Q1:1")
    );
}

#[test]
fn the_project_report_measures_the_drawn_pages_in_page_order() {
    let s = common::sample();
    let folios: Vec<String> = s
        .layout_report(None)
        .unwrap()
        .into_iter()
        .map(|p| p.folio)
        .collect();
    let pos = |f: &str| folios.iter().position(|x| x == f);
    for (a, b) in [("1", "101"), ("101", "105"), ("105", "201")] {
        assert!(pos(a).expect(a) < pos(b).expect(b), "{folios:?}");
    }
    // The dividers and the generated pages are not measured.
    for f in ["2", "100", "900"] {
        assert_eq!(pos(f), None, "{folios:?}");
    }
}

#[test]
fn a_folio_that_names_no_drawn_page_is_refused_in_words() {
    let s = common::sample();
    let e = s.layout_report(Some("999")).unwrap_err();
    assert_eq!(e, LayoutError::NoFolio("999".to_owned()));
    assert_eq!(e.to_string(), "the page order has no folio `999`");
    let e = s.layout_report(Some("2")).unwrap_err();
    assert_eq!(e, LayoutError::NotDrawn("2".to_owned()));
    assert_eq!(
        e.to_string(),
        "folio `2` is not a drawn page: a section divider or a generated page has no wires"
    );
    assert_eq!(s.page_draft("2"), None);
}

#[test]
fn the_draft_of_folio_101_holds_its_parts_wires_and_frame_bands() {
    let s = common::sample();
    let t = s.page_tables("101").unwrap();
    let d = s.page_draft("101").unwrap();
    // Every part is a representation of the page; every connection the page draws is a wire, routed where it has a
    // stored route.
    let reps: Vec<_> = t.reps.iter().map(|r| r.id).collect();
    assert!(d.parts.iter().all(|p| reps.contains(&p.id.0)));
    for c in &t.connections {
        let w = d
            .wires
            .iter()
            .find(|w| w.id.0 == c.id)
            .expect("a wire of the page");
        assert_eq!(w.route.is_empty(), !c.stored_route);
    }
    // The frame's bands keep clear of every pin of the page, and the QET frame has some.
    assert!(!d.keep_out.is_empty());
    for b in &d.keep_out {
        for p in &d.parts {
            for pin in &p.pins {
                let at = p.at.plus(pin.at);
                assert!(
                    !(b.x0 < at.x && at.x < b.x1 && b.y0 < at.y && at.y < b.y1),
                    "{b:?}"
                );
            }
        }
    }
}

/// A free text and a structure box's caption are fixed texts of the page as the layout sees it, so the measure and the
/// router see them.
#[test]
fn free_texts_and_structure_box_captions_are_fixed_texts() {
    let mut s = common::sample();
    let page = s.page_tables("101").expect("folio 101").page.to_string();
    let at = [100_541_667, 31_750_000];
    let placed = s
        .execute(&ew_api::AnyCommand {
            name: "item.place".into(),
            params: serde_json::json!({"page": page, "kind": {"free_text": {"text": "FT", "at": at}}}),
        })
        .expect("item.place");
    let item = placed.outcome.primary.expect("the item").1.as_uuid();
    let q = 7_937_500 / 3;
    let outline = [
        [30 * q, 30 * q],
        [40 * q, 30 * q],
        [40 * q, 35 * q],
        [30 * q, 35 * q],
    ];
    let boxed = s
        .execute(&ew_api::AnyCommand {
            name: "box.structure".into(),
            params: serde_json::json!({"page": page, "outline": outline, "style": "dotted", "caption": "Box", "assigns": {}}),
        })
        .expect("box.structure");
    let structure = boxed.outcome.primary.expect("the box").1.as_uuid();
    let draft = s.page_draft("101").expect("folio 101");
    let has = |id| draft.fixed_texts.iter().any(|t| t.owner == id);
    assert!(has(item), "the free text");
    assert!(has(structure), "the structure box's caption");
}

// The layout rules EW-CORE-067 to -070, -072 and -073 on hand-built pages (RK1) and on the sample (RK2).

/// The point of the QET grid at column `c` and row `r` of grid steps (7 937 500 / 3 nm, rounded to the nm).
fn grid(c: i64, r: i64) -> serde_json::Value {
    serde_json::json!([(c * 7_937_500 + 1) / 3, (r * 7_937_500 + 1) / 3])
}

/// A route through the grid points `points`.
fn route(points: &[(i64, i64)]) -> serde_json::Value {
    serde_json::Value::Array(points.iter().map(|&(c, r)| grid(c, r)).collect())
}

/// RK1's page: the sample's motor page 103 with a coil (`-103K1`, `-103K2`, … in this order) whose upper connection
/// point A1 stands at each grid point of `coils`, on the empty stretch of columns 50 to 75 and rows 20 to 35 (right of
/// the motor circuit); a coil's A2 is four steps below its A1. The session, the page's ID and each coil's function.
fn rk1_page(coils: &[(i64, i64)]) -> (Session, String, Vec<String>) {
    let mut s = common::sample();
    let page = s.page_tables("103").expect("folio 103").page.to_string();
    if let Some(embed) = s.starter_embed("ew.iec.coil@qet10") {
        run(&mut s, "library.embed", embed);
    }
    let coil = s
        .starter_symbol_copy("ew.iec.coil@qet10")
        .expect("the starter coil")
        .to_string();
    let mut functions = Vec::new();
    for &(c, r) in coils {
        let before: Vec<_> = s.query().functions().ids().collect();
        run(
            &mut s,
            "device.place",
            serde_json::json!({"page": page, "symbol": coil, "pos": grid(c, r), "target": {"new_device": {"letter": "K"}}}),
        );
        let placed = s
            .query()
            .functions()
            .ids()
            .find(|f| !before.contains(f))
            .expect("the coil's function");
        functions.push(placed.to_string());
    }
    (s, page, functions)
}

/// Runs `name` with `params` on `s`; the primary record's ID.
fn run(s: &mut Session, name: &str, params: serde_json::Value) -> String {
    let receipt = s
        .execute(&ew_api::AnyCommand {
            name: name.into(),
            params,
        })
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    receipt
        .outcome
        .primary
        .map(|(_, id)| id.to_string())
        .unwrap_or_default()
}

/// Connection point `idx` (0: A1, 1: A2) of the coil `function`, as `connection.draw` takes an end.
fn pin(function: &str, idx: u16) -> serde_json::Value {
    serde_json::json!({"cp": {"function": function, "idx": idx}})
}

/// Draws a wire on `page` from `a` to `b` along `route` (empty: unrouted); its ID.
fn draw(
    s: &mut Session,
    page: &str,
    a: serde_json::Value,
    b: serde_json::Value,
    route: serde_json::Value,
) -> String {
    run(
        s,
        "connection.draw",
        serde_json::json!({"a": a, "b": b, "page": page, "route": route}),
    )
}

/// The findings of `rule` on RK1's page (folio 103) in the online run of the drafting scheme: each message in English
/// and in Dutch, with its severity.
fn findings(s: &Session, rule: &str) -> Vec<(String, String, ew_api::checks::Severity)> {
    let page = s.page_tables("103").map(|t| t.page);
    let report = s.check(&ew_api::checks::DRAFTING, ew_api::checks::CheckMode::Online);
    report
        .findings
        .iter()
        .filter(|f| f.rule == rule && f.page.map(|p| p.as_uuid()) == page)
        .map(|f| {
            (
                f.message().clone(),
                ew_api::checks::render(f, "nl-NL"),
                f.severity,
            )
        })
        .collect()
}

use ew_api::checks::Severity::{Info, Warning};

/// `ew layout` measured an unrouted connection's dashed line for bends and crossings, while the check rules count it
/// only as unrouted: the report leaves it out as they do, and measures it once it has a route.
#[test]
fn the_layout_report_leaves_an_unrouted_wire_out() {
    let (mut s, page, k) = rk1_page(&[(60, 20), (70, 28)]);
    let w = draw(&mut s, &page, pin(&k[0], 1), pin(&k[1], 0), serde_json::json!([]));
    let bends = |s: &Session| -> Vec<String> {
        s.layout_report(Some("103")).unwrap()[0]
            .bends
            .iter()
            .map(|b| b.connection.clone())
            .filter(|c| c.contains("-103K1"))
            .collect()
    };
    assert_eq!(bends(&s), Vec::<String>::new(), "the dashed L is no wire");
    run(
        &mut s,
        "connection.route",
        serde_json::json!({"connection": w, "route": route(&[(60, 24), (60, 26), (70, 26), (70, 28)])}),
    );
    assert_eq!(bends(&s), ["-103K1:A2 → -103K2:A1"], "routed, it is measured");
}

/// RK1, EW-CORE-067: a wire routed through a coil's A1 that it does not join; its fixed twin runs a step above the
/// point's stub.
#[test]
fn a_wire_through_a_connection_point_fires_ew_core_067() {
    let (mut s, page, k) = rk1_page(&[(60, 20), (65, 28), (70, 20)]);
    let w = draw(
        &mut s,
        &page,
        pin(&k[0], 1),
        pin(&k[2], 1),
        route(&[(60, 24), (60, 28), (70, 28), (70, 24)]),
    );
    assert_eq!(
        findings(&s, "EW-CORE-067"),
        [(
            "Wire -103K1:A2 → -103K3:A2 crosses -103K2:A1 without joining it, so on paper it looks connected. Route it \
             again so that it passes beside."
                .to_owned(),
            "Draad -103K1:A2 → -103K3:A2 kruist -103K2:A1 zonder ermee verbonden te zijn, dus op papier lijkt hij \
             aangesloten. Routeer hem opnieuw zodat hij ernaast loopt."
                .to_owned(),
            Warning
        )]
    );
    run(
        &mut s,
        "connection.route",
        serde_json::json!({"connection": w, "route": route(&[(60, 24), (60, 26), (70, 26), (70, 24)])}),
    );
    assert_eq!(findings(&s, "EW-CORE-067"), []);
}

/// RK1, EW-CORE-068: two wires on one line for six grid steps; the fixed twin runs the second a step lower.
#[test]
fn a_wire_along_another_fires_ew_core_068() {
    let (mut s, page, k) = rk1_page(&[(60, 20), (62, 20), (68, 20), (70, 20)]);
    draw(
        &mut s,
        &page,
        pin(&k[0], 1),
        pin(&k[3], 1),
        route(&[(60, 24), (60, 26), (70, 26), (70, 24)]),
    );
    let w = draw(
        &mut s,
        &page,
        pin(&k[1], 1),
        pin(&k[2], 1),
        route(&[(62, 24), (62, 26), (68, 26), (68, 24)]),
    );
    assert_eq!(
        findings(&s, "EW-CORE-068"),
        [(
            "Wire -103K1:A2 → -103K4:A2 lies on top of -103K2:A2 → -103K3:A2 on page 103, so on paper the two cannot \
             be told apart. Route one of them again."
                .to_owned(),
            "Draad -103K1:A2 → -103K4:A2 ligt over -103K2:A2 → -103K3:A2 op pagina 103, dus op papier zijn de twee \
             niet uit elkaar te houden. Routeer een van beide opnieuw."
                .to_owned(),
            Warning
        )]
    );
    run(
        &mut s,
        "connection.route",
        serde_json::json!({"connection": w, "route": route(&[(62, 24), (62, 27), (68, 27), (68, 24)])}),
    );
    assert_eq!(findings(&s, "EW-CORE-068"), []);
}

/// RK1, EW-CORE-069: a wire through a coil's body; the fixed twin runs below the coil.
#[test]
fn a_wire_through_a_symbol_fires_ew_core_069() {
    let (mut s, page, k) = rk1_page(&[(60, 20), (65, 25), (70, 20)]);
    let w = draw(
        &mut s,
        &page,
        pin(&k[0], 1),
        pin(&k[2], 1),
        route(&[(60, 24), (60, 26), (70, 26), (70, 24)]),
    );
    assert_eq!(
        findings(&s, "EW-CORE-069"),
        [(
            "Wire -103K1:A2 → -103K3:A2 runs through -103K2.".to_owned(),
            "Draad -103K1:A2 → -103K3:A2 loopt door -103K2.".to_owned(),
            Warning
        )]
    );
    run(
        &mut s,
        "connection.route",
        serde_json::json!({"connection": w, "route": route(&[(60, 24), (60, 31), (70, 31), (70, 24)])}),
    );
    assert_eq!(findings(&s, "EW-CORE-069"), []);
}

/// RK1, EW-CORE-070: a wire through a free text (a device's tag keeps clear of the wires the scene draws, so the page
/// test takes a text that stays put); the fixed twin runs round it. The wording of a symbol's text in both languages:
/// [`ew_api::checks::rules::page::wire_through_text`] with a device.
#[test]
fn a_wire_through_a_text_fires_ew_core_070() {
    use ew_api::checks::rules::page::{LayoutWire, TextOwner, wire_through_text};
    let (mut s, page, k) = rk1_page(&[(60, 20), (60, 32)]);
    run(
        &mut s,
        "item.place",
        serde_json::json!({"page": page, "kind": {"free_text": {"text": "Note", "at": grid(59, 28)}}}),
    );
    let w = draw(
        &mut s,
        &page,
        pin(&k[0], 1),
        pin(&k[1], 0),
        route(&[(60, 24), (60, 32)]),
    );
    assert_eq!(
        findings(&s, "EW-CORE-070"),
        [(
            "Wire -103K1:A2 → -103K2:A1 runs through the free text \"Note\".".to_owned(),
            "Draad -103K1:A2 → -103K2:A1 loopt door de vrije tekst \"Note\".".to_owned(),
            Warning
        )]
    );
    run(
        &mut s,
        "connection.route",
        serde_json::json!({"connection": w, "route": route(&[(60, 24), (60, 25), (66, 25), (66, 31), (60, 31), (60, 32)])}),
    );
    assert_eq!(findings(&s, "EW-CORE-070"), []);
    let rep = s.page_tables("103").expect("folio 103").reps[0].id;
    let wire = LayoutWire {
        id: w.parse().expect("an ID"),
        words: &ew_api::checks::rules::Named::from("-103K1:A2 → -103K2:A1"),
        tube: false,
    };
    let device = TextOwner::Symbol {
        rep: ew_ids::Id::from_uuid(rep),
        device: &ew_api::checks::rules::Named::from("-103K3"),
    };
    let f = wire_through_text(ew_ids::Id::from_uuid(rep), wire, device, "-103K3");
    assert_eq!(
        (f.message().as_str(), ew_api::checks::render(&f, "nl-NL")),
        (
            "Wire -103K1:A2 → -103K2:A1 runs through the text \"-103K3\" of -103K3.",
            "Draad -103K1:A2 → -103K2:A1 loopt door de tekst \"-103K3\" van -103K3.".to_owned()
        )
    );
}

/// EW-CORE-070 quotes the line of a text that the wire runs through, not the text's first line.
#[test]
fn ew_core_070_quotes_the_crossed_line() {
    let (mut s, page, k) = rk1_page(&[(60, 20), (70, 32)]);
    run(
        &mut s,
        "item.place",
        serde_json::json!({"page": page, "kind": {"free_text": {"text": "Upper\nLower", "at": grid(58, 29)}}}),
    );
    draw(
        &mut s,
        &page,
        pin(&k[0], 1),
        pin(&k[1], 0),
        route(&[(60, 24), (60, 25), (55, 25), (55, 30), (70, 30), (70, 32)]),
    );
    let found = findings(&s, "EW-CORE-070");
    assert_eq!(
        found
            .iter()
            .map(|f| f.0.as_str())
            .filter(|m| m.contains("-103K1"))
            .collect::<Vec<_>>(),
        ["Wire -103K1:A2 → -103K2:A1 runs through the free text \"Lower\"."]
    );
}

/// docs/design/06a-drafting.md §9.3 row 7: marks of the starter library stacked two grid steps apart, incoming and
/// outgoing, each with its name and its cross-reference, stand at their first places without their texts meeting.
#[test]
fn starter_marks_stack_two_steps_apart_without_their_texts_meeting() {
    let (mut s, page, _) = rk1_page(&[]);
    for key in ["ew.gen.ip@qet10", "ew.gen.ip-target@qet10"] {
        if let Some(embed) = s.starter_embed(key) {
            run(&mut s, "library.embed", embed);
        }
    }
    let (source, target) = (
        s.starter_symbol_copy("ew.gen.ip@qet10").unwrap().to_string(),
        s.starter_symbol_copy("ew.gen.ip-target@qet10")
            .unwrap()
            .to_string(),
    );
    for (i, row) in [20, 22, 24].into_iter().enumerate() {
        for (symbol, x, direction) in [(&target, 52, "target"), (&source, 72, "source")] {
            run(
                &mut s,
                "ip.place",
                serde_json::json!({"page": page, "symbol": symbol, "pos": grid(x, row),
                    "name": format!("24V-{i}"), "direction": direction}),
            );
        }
    }
    use ew_api::drafting::LabelKind::{MarkName, MarkXref};
    let ours: Vec<_> = s
        .page_tables("103")
        .unwrap()
        .reps
        .iter()
        .filter(|r| r.ip.as_ref().is_some_and(|ip| ip.name.starts_with("24V-")))
        .map(|r| r.id)
        .collect();
    let page = s.page_draft("103").unwrap();
    let settled = ew_api::drafting::settle_labels(&page);
    let marks: Vec<_> = page
        .labels
        .iter()
        .filter(|l| matches!(l.kind, MarkName | MarkXref) && ours.contains(&l.id.owner))
        .filter_map(|l| settled.iter().find(|x| x.id == l.id))
        .collect();
    assert_eq!(marks.len(), 12, "six marks, a name and a cross-reference each");
    // Each name in line with its wire, where its symbol puts it.
    assert!(
        marks.iter().filter(|m| m.id.n == 0).all(|m| m.rank == 0),
        "the names at their first places: {marks:?}"
    );
    for (i, a) in marks.iter().enumerate() {
        for b in &marks[i + 1..] {
            assert!(!a.rect.overlaps(b.rect), "{a:?} meets {b:?}");
        }
    }
}

/// FT-2207: two outgoing marks one grid step apart (a supply page's 24 V and 0 V), each continued at three points:
/// each mark's references stand clear of its own name, of the other mark's texts and of their arrows, and the check
/// finds no text on a text there.
#[test]
fn marks_one_step_apart_with_several_continuations_keep_their_texts_apart() {
    let (mut s, page, _) = rk1_page(&[]);
    for key in ["ew.gen.ip@qet10", "ew.gen.ip-target@qet10"] {
        if let Some(embed) = s.starter_embed(key) {
            run(&mut s, "library.embed", embed);
        }
    }
    let (source, target) = (
        s.starter_symbol_copy("ew.gen.ip@qet10").unwrap().to_string(),
        s.starter_symbol_copy("ew.gen.ip-target@qet10")
            .unwrap()
            .to_string(),
    );
    // The upper mark's references take two lines (five of them, three a line), the lower one's one.
    let continued = [
        ("0V-S", 20, vec![(52, 23), (52, 26), (52, 29), (52, 32), (52, 35)]),
        ("24V-S", 21, vec![(62, 26), (62, 29), (62, 32)]),
    ];
    for (name, row, targets) in continued {
        run(
            &mut s,
            "ip.place",
            serde_json::json!({"page": page, "symbol": source, "pos": grid(72, row),
                "name": name, "direction": "source"}),
        );
        for &(c, r) in &targets {
            run(
                &mut s,
                "ip.place",
                serde_json::json!({"page": page, "symbol": target, "pos": grid(c, r),
                    "name": name, "direction": "target"}),
            );
        }
    }
    use ew_api::drafting::LabelKind::{MarkName, MarkXref};
    let ours: Vec<_> = s
        .page_tables("103")
        .unwrap()
        .reps
        .iter()
        .filter(|r| r.ip.as_ref().is_some_and(|ip| ip.name.ends_with("V-S")))
        .map(|r| r.id)
        .collect();
    let draft = s.page_draft("103").unwrap();
    let settled = ew_api::drafting::settle_labels(&draft);
    let texts: Vec<_> = draft
        .labels
        .iter()
        .filter(|l| matches!(l.kind, MarkName | MarkXref) && ours.contains(&l.id.owner))
        .filter_map(|l| settled.iter().find(|x| x.id == l.id))
        .collect();
    let bodies: Vec<_> = draft
        .parts
        .iter()
        .filter(|p| ours.contains(&p.id.0))
        .map(|p| p.bounds())
        .collect();
    for (i, a) in texts.iter().enumerate() {
        for b in &texts[i + 1..] {
            assert!(!a.rect.overlaps(b.rect), "{a:?} meets {b:?}");
        }
        for body in &bodies {
            assert!(!a.rect.overlaps(*body), "{a:?} lies on a mark's arrow {body:?}");
        }
    }
    let on_text: Vec<_> = findings(&s, "EW-CORE-075")
        .into_iter()
        .map(|f| f.0)
        .filter(|m| m.contains("V-S"))
        .collect();
    assert_eq!(on_text, Vec::<String>::new());
}

/// Doc 06a §2.4: a symbol kept in place (`fields.set` of `locked`) is a locked part of the page's model, its
/// neighbour is not; letting it go unlocks it again.
#[test]
fn a_symbol_kept_in_place_is_a_locked_part_of_the_page_model() {
    let (mut s, _, functions) = rk1_page(&[(52, 22), (60, 22)]);
    let coils: Vec<_> = s
        .page_tables("103")
        .unwrap()
        .reps
        .iter()
        .filter(|r| r.functions.iter().any(|f| functions.contains(&f.to_string())))
        .map(|r| r.id)
        .collect();
    let [kept, other] = coils[..] else {
        panic!("two coils: {coils:?}")
    };
    let locked = |s: &Session, id| {
        s.page_draft("103")
            .unwrap()
            .parts
            .iter()
            .find(|p| p.id.0 == id)
            .expect("the coil's part")
            .locked
    };
    for keep in [true, false] {
        run(
            &mut s,
            "fields.set",
            serde_json::json!({"kind": "representation", "targets": [kept.to_string()], "path": "locked", "value": keep}),
        );
        assert_eq!(locked(&s, kept), keep);
        assert!(!locked(&s, other));
    }
}

/// A wire through an interruption point names the point in each language's words, never
/// "interruption point" in a Dutch message: EW-CORE-069 on a page, and EW-CORE-070's wording of a point's text.
#[test]
fn a_wire_through_an_interruption_point_names_it_in_each_language() {
    use ew_api::checks::rules::page::{LayoutWire, TextOwner, wire_through_text};
    let (mut s, page, k) = rk1_page(&[(52, 20), (70, 20)]);
    if let Some(embed) = s.starter_embed("ew.gen.ip@qet10") {
        run(&mut s, "library.embed", embed);
    }
    let ip = s
        .starter_symbol_copy("ew.gen.ip@qet10")
        .expect("the starter interruption point")
        .to_string();
    // A source point whose arrow (its tip at 61, 28) the wire along row 28 runs through.
    run(
        &mut s,
        "ip.place",
        serde_json::json!({"page": page, "symbol": ip, "pos": grid(61, 28), "name": "X-9", "direction": "source"}),
    );
    draw(
        &mut s,
        &page,
        pin(&k[0], 1),
        pin(&k[1], 1),
        route(&[(52, 24), (52, 28), (70, 28), (70, 24)]),
    );
    assert_eq!(
        findings(&s, "EW-CORE-069"),
        [(
            "Wire -103K1:A2 → -103K2:A2 runs through interruption point X-9.".to_owned(),
            "Draad -103K1:A2 → -103K2:A2 loopt door onderbrekingspunt X-9.".to_owned(),
            Warning
        )]
    );
    let rep = s.page_tables("103").expect("folio 103").reps[0].id;
    let wire = LayoutWire {
        id: ew_ids::Id::from_uuid(rep),
        words: &ew_api::checks::rules::Named::from("-103K1:A2 → -103K2:A2"),
        tube: false,
    };
    let owner = TextOwner::Ip {
        rep: ew_ids::Id::from_uuid(rep),
        name: "X-9",
    };
    let f = wire_through_text(ew_ids::Id::from_uuid(rep), wire, owner, "X-9");
    assert_eq!(
        (f.message().as_str(), ew_api::checks::render(&f, "nl-NL")),
        (
            "Wire -103K1:A2 → -103K2:A2 runs through the text \"X-9\" of interruption point X-9.",
            "Draad -103K1:A2 → -103K2:A2 loopt door de tekst \"X-9\" van onderbrekingspunt X-9.".to_owned()
        )
    );
}

/// RK1, EW-CORE-072: wires from above, below, the left and the right into one junction; its twin, the junction before
/// the fourth wire, joins three.
#[test]
fn a_four_way_junction_fires_ew_core_072() {
    let (mut s, page, k) = rk1_page(&[(60, 20), (60, 32), (52, 20), (68, 20)]);
    let v = draw(
        &mut s,
        &page,
        pin(&k[0], 1),
        pin(&k[1], 0),
        route(&[(60, 24), (60, 32)]),
    );
    let on_v = serde_json::json!({"on_connection": {"connection": v, "at": grid(60, 28)}});
    draw(
        &mut s,
        &page,
        pin(&k[2], 1),
        on_v,
        route(&[(52, 24), (52, 28), (60, 28)]),
    );
    assert_eq!(findings(&s, "EW-CORE-072"), []);
    let junction = s
        .page_tables("103")
        .expect("folio 103")
        .junctions
        .iter()
        .find(|j| serde_json::json!([j.at.0, j.at.1]) == grid(60, 28))
        .expect("the junction")
        .id
        .to_string();
    draw(
        &mut s,
        &page,
        pin(&k[3], 1),
        serde_json::json!({"junction": {"junction": junction}}),
        route(&[(68, 24), (68, 28), (60, 28)]),
    );
    assert_eq!(
        findings(&s, "EW-CORE-072"),
        [(
            "The junction at D9 joins four wires: draw two T junctions instead.".to_owned(),
            "De aftakking in D9 verbindt vier draden: teken in plaats daarvan twee T-aftakkingen.".to_owned(),
            Warning
        )]
    );
}

/// On a page without a frame grid EW-CORE-072 names the junction by its folio, in both
/// languages, with the folio as its subject.
#[test]
fn a_four_way_junction_off_a_frame_grid_is_named_by_its_folio() {
    use ew_api::checks::rules::page::four_way_junction;
    let page: ew_ids::Id<ew_schema::Page> = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(7));
    let f = four_way_junction(page, "7", page.cast(), "");
    assert_eq!(
        (
            f.subject_english().as_str(),
            f.message().as_str(),
            ew_api::checks::render(&f, "en-GB"),
            ew_api::checks::render(&f, "nl-NL")
        ),
        (
            "folio 7",
            "A junction on folio 7 joins four wires: draw two T junctions instead.",
            "A junction on folio 7 joins four wires: draw two T junctions instead.".to_owned(),
            "Een aftakking op folio 7 verbindt vier draden: teken in plaats daarvan twee T-aftakkingen."
                .to_owned()
        )
    );
}

/// RK1, EW-CORE-073: a wire drawn without a route between two points (info online, an error in release mode); its
/// fixed twin has the straight route. A waiver of the finding waives it, and EW-CORE-081 does not report the waiver.
#[test]
fn an_unrouted_wire_fires_ew_core_073_at_its_severities_and_takes_a_waiver() {
    use ew_api::checks::{CheckMode, FindingState, PRE_DELIVERY, Severity};
    let (mut s, page, k) = rk1_page(&[(60, 20), (60, 30)]);
    let w = draw(&mut s, &page, pin(&k[0], 1), pin(&k[1], 0), serde_json::json!([]));
    assert_eq!(
        findings(&s, "EW-CORE-073"),
        [(
            "Wire -103K1:A2 → -103K2:A1 has no route: it is drawn as a dashed line.".to_owned(),
            "Draad -103K1:A2 → -103K2:A1 heeft geen route: hij is als gestreepte lijn getekend.".to_owned(),
            Info
        )]
    );
    let release = s.check(&PRE_DELIVERY, CheckMode::Release);
    let found: Vec<_> = release
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-073")
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    let fingerprint = found[0].fingerprint.to_string();
    s.execute_with(
        &ew_api::AnyCommand {
            name: "check.waive".into(),
            params: serde_json::json!({
                "rule": "EW-CORE-073", "fingerprint": fingerprint,
                "reason": {"en-GB": "A draft page."}, "author": "engineer", "date": "2026-11-08"
            }),
        },
        ew_api::Submit::from(ew_api::Origin::Gui),
    )
    .expect("the waiver commits from the GUI");
    let release = s.check(&PRE_DELIVERY, CheckMode::Release);
    let states: Vec<bool> = release
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-073")
        .map(|f| matches!(f.state, FindingState::Waived { .. }))
        .collect();
    assert_eq!(states, [true]);
    assert_eq!(
        release
            .findings
            .iter()
            .filter(|f| f.rule == "EW-CORE-081")
            .count(),
        0
    );
    run(
        &mut s,
        "connection.route",
        serde_json::json!({"connection": w, "route": route(&[(60, 24), (60, 30)])}),
    );
    assert_eq!(findings(&s, "EW-CORE-073"), []);
}

/// RK2: the sample's check report, online and in release mode, holds none of EW-CORE-067 to -070 and -073.
#[test]
fn the_sample_s_check_report_holds_no_layout_finding() {
    use ew_api::checks::{CheckMode, DRAFTING, PRE_DELIVERY};
    let s = common::sample();
    for report in [
        s.check(&DRAFTING, CheckMode::Online),
        s.check(&PRE_DELIVERY, CheckMode::Release),
    ] {
        let layout: Vec<String> = report
            .findings
            .iter()
            .filter(|f| {
                [
                    "EW-CORE-067",
                    "EW-CORE-068",
                    "EW-CORE-069",
                    "EW-CORE-070",
                    "EW-CORE-073",
                ]
                .contains(&f.rule)
            })
            .map(|f| f.message())
            .collect();
        assert_eq!(layout, Vec::<String>::new());
        assert!(
            report.rules.iter().any(|m| m.id == "EW-CORE-073"),
            "the run evaluated the rules"
        );
    }
}
