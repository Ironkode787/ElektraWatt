// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's geometry and its layout measures on a temporary copy of Golden A. Folio 101
//! draws its connections along the provisional path, and the measures name what the owner saw there: the heater's
//! PE wire through the lamp's X1, and the lamp's L feed along the switch's wires. Named items only, never totals. The
//! layout rules of §9 (EW-CORE-067 to -070, -072 and -073) on hand-built pages and on Golden A (RK1, RK2).

use std::path::Path;

use ew_api::Session;
use ew_api::output::LayoutError;
use ew_testdir::Tmp;

/// A read-only session on a temporary copy of Golden A (the committed project is only copied).
fn golden_copy() -> (Tmp, Session) {
    let tmp = Tmp::new("ew-api-layout");
    let dir = tmp.0.join("J-E-2416.ew");
    ew_testdir::copy_dir(Path::new(ew_testdir::GOLDEN_A), &dir);
    let session = Session::open_read_only(&dir, "ElektraWatt test").unwrap().0;
    (tmp, session)
}

/// On Golden A's routed folio 101 (rc.1's provisional drawing broke all three): the heater's PE wire does
/// not touch the lamp's X1, the lamp's L feed runs along none of the switch's wires, and the heater's L feed does not
/// pass through T55; the page breaks no hard rule.
#[test]
fn folio_101_s_pe_wire_lamp_feed_and_heater_feed_keep_clear() {
    let (_tmp, s) = golden_copy();
    let pages = s.layout_report(Some("101")).unwrap();
    assert_eq!(pages.len(), 1);
    let p = &pages[0];
    assert_eq!(p.folio, "101");
    assert!(
        !p.touches
            .iter()
            .any(|t| t.connection == "-E55:PE → PE-1.1" && t.point == "-E56:X1"),
        "{:#?}",
        p.touches
    );
    let feed = "the junction in C11 → -E56:X2";
    assert!(
        !p.overlaps.iter().any(|o| o.connection == feed || o.other == feed),
        "{:#?}",
        p.overlaps
    );
    assert!(
        !p.through_symbols
            .iter()
            .any(|t| t.connection.ends_with("-E55:L") && t.symbol == "-T55"),
        "{:#?}",
        p.through_symbols
    );
    assert_eq!(p.hard(), 0, "{p:#?}");
    // The page as the whole-project report measures it.
    let all = s.layout_report(None).unwrap();
    assert_eq!(all.iter().find(|q| q.folio == "101"), Some(p));
}

/// A wire's words name each device by its tag with its prefix, also where a
/// frozen tag hides its sign on the page (folio 101's S51 and terminals 1X1).
#[test]
fn a_wire_s_words_name_each_device_with_its_tag_s_prefix() {
    use crate::golden_common;
    let (_tmp, s) = golden_copy();
    let w4: ew_derive::ew_ids::Uuid = golden_common::ids_by_key(&golden_common::batch())["connections|W4"]
        .parse()
        .expect("an ID");
    assert_eq!(
        s.connection_words("101", w4).as_deref(),
        Some("-1X1:1/a → -S51:2")
    );
}

#[test]
fn the_project_report_measures_the_drawn_pages_in_page_order() {
    let (_tmp, s) = golden_copy();
    let folios: Vec<String> = s
        .layout_report(None)
        .unwrap()
        .into_iter()
        .map(|p| p.folio)
        .collect();
    let pos = |f: &str| folios.iter().position(|x| x == f);
    for (a, b) in [("1", "3"), ("3", "101"), ("103", "106"), ("106", "201")] {
        assert!(pos(a).unwrap() < pos(b).unwrap(), "{folios:?}");
    }
    // The dividers and the generated pages are not measured.
    for f in ["2", "100", "900"] {
        assert_eq!(pos(f), None, "{folios:?}");
    }
}

#[test]
fn a_folio_that_names_no_drawn_page_is_refused_in_words() {
    let (_tmp, s) = golden_copy();
    let e = s.layout_report(Some("999")).unwrap_err();
    assert_eq!(e, LayoutError::NoFolio("999".to_owned()));
    assert_eq!(e.to_string(), "the page order has no folio `999`");
    let e = s.layout_report(Some("2")).unwrap_err();
    assert_eq!(e, LayoutError::NotDrawn("2".to_owned()));
    assert_eq!(
        e.to_string(),
        "folio `2` is not a drawn page: a section divider or a generated page has no wires"
    );
    assert_eq!(s.page_geometry("2"), None);
}

#[test]
fn the_geometry_of_folio_101_holds_its_bodies_texts_and_frame_bands() {
    let (_tmp, s) = golden_copy();
    let t = s.page_tables("101").unwrap();
    let g = s.page_geometry("101").unwrap();
    // Every body, text box and CP belongs to a representation of the page; each connection keeps its drawn polyline.
    let reps: Vec<_> = t.reps.iter().map(|r| r.id).collect();
    assert!(g.bodies.iter().all(|b| reps.contains(&b.rep)));
    assert!(
        g.texts
            .iter()
            .all(|x| reps.contains(&x.rep) && x.rect[0] < x.rect[2] && x.rect[1] < x.rect[3])
    );
    assert!(g.cps.iter().all(|c| reps.contains(&c.rep)));
    assert_eq!(
        g.connections
            .iter()
            .map(|c| (c.id, c.polyline.clone()))
            .collect::<Vec<_>>(),
        t.connections
            .iter()
            .map(|c| (c.id, c.polyline.clone()))
            .collect::<Vec<_>>()
    );
    // The lamp's pin text X1 beside its point: a text box of E56's representation, once.
    let lamp = t.cps.iter().find(|c| c.label == "E56:X1").unwrap();
    let x1 = [236_140_625, 100_261_208, 237_672_563, 102_063_021];
    assert_eq!(
        g.texts
            .iter()
            .filter(|x| x.rep == lamp.rep && x.rect == x1)
            .count(),
        1
    );
    // The frame's bands keep clear of every CP of the page, and the QET frame has some.
    assert!(!g.frame_bands.is_empty());
    for b in &g.frame_bands {
        let inside = |p: (i64, i64)| b[0] < p.0 && p.0 < b[2] && b[1] < p.1 && p.1 < b[3];
        assert!(g.cps.iter().all(|c| !inside(c.at)), "{b:?}");
    }
}

/// A free text and a structure box's caption are text boxes of the page's geometry, so measure (d) and
/// the router's rule 4 see them.
#[test]
fn free_texts_and_structure_box_captions_are_text_boxes() {
    use crate::golden_common;
    let batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let mut s = golden_common::session(&batch);
    let page = ids["pages|101"].clone();
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
    let geometry = s.page_geometry("101").expect("folio 101");
    let has = |id| geometry.texts.iter().any(|t| t.rep == id);
    assert!(has(item), "the free text");
    assert!(has(structure), "the structure box's caption");
}

// The layout rules EW-CORE-067 to -070, -072 and -073 on hand-built pages (RK1) and on Golden A (RK2).

/// The point of the QET grid at column `c` and row `r` of grid steps (7 937 500 / 3 nm, rounded to the nm).
fn grid(c: i64, r: i64) -> serde_json::Value {
    serde_json::json!([(c * 7_937_500 + 1) / 3, (r * 7_937_500 + 1) / 3])
}

/// A route through the grid points `points`.
fn route(points: &[(i64, i64)]) -> serde_json::Value {
    serde_json::Value::Array(points.iter().map(|&(c, r)| grid(c, r)).collect())
}

/// RK1's page: Golden A's folio 103 with a coil (`-103K1`, `-103K2`, … in this order) whose upper connection point A1
/// stands at each grid point of `coils`, on the empty stretch of columns 50 to 75 and rows 20 to 35; a coil's A2 is
/// four steps below its A1. The session, the page's ID and each coil's function.
fn rk1_page(coils: &[(i64, i64)]) -> (Session, String, Vec<String>) {
    use crate::golden_common;
    let batch = golden_common::batch();
    let page = golden_common::ids_by_key(&batch)["pages|103"].clone();
    let mut s = golden_common::session(&batch);
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

/// The findings of `rule` in the online run of the drafting scheme: each message in English and in Dutch, with its
/// severity.
fn findings(s: &Session, rule: &str) -> Vec<(String, String, ew_api::checks::Severity)> {
    let report = s.check(&ew_api::checks::DRAFTING, ew_api::checks::CheckMode::Online);
    report
        .findings
        .iter()
        .filter(|f| f.rule == rule)
        .map(|f| (f.message.clone(), ew_api::checks::render(f, "nl-NL"), f.severity))
        .collect()
}

use ew_api::checks::Severity::{Info, Warning};

/// RK1, EW-CORE-067: a wire routed through a coil's A1 that it does not join; its fixed twin runs a step above the
/// point's stub.
#[test]
fn rk1_a_wire_through_a_connection_point_fires_ew_core_067() {
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
            "Wire -103K1:A2 → -103K3:A2 runs through -103K2:A1, which it does not join.".to_owned(),
            "Draad -103K1:A2 → -103K3:A2 loopt door -103K2:A1, waarmee hij niet verbonden is.".to_owned(),
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
fn rk1_a_wire_along_another_fires_ew_core_068() {
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
            "Wire -103K1:A2 → -103K4:A2 runs along -103K2:A2 → -103K3:A2 on folio 103.".to_owned(),
            "Draad -103K1:A2 → -103K4:A2 loopt over -103K2:A2 → -103K3:A2 op folio 103.".to_owned(),
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
fn rk1_a_wire_through_a_symbol_fires_ew_core_069() {
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
fn rk1_a_wire_through_a_text_fires_ew_core_070() {
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
        words: "-103K1:A2 → -103K2:A1",
    };
    let device = TextOwner::Symbol {
        rep: ew_derive::ew_ids::Id::from_uuid(rep),
        device: "-103K3",
    };
    let f = wire_through_text(ew_derive::ew_ids::Id::from_uuid(rep), wire, device, "-103K3");
    assert_eq!(
        (f.message.as_str(), ew_api::checks::render(&f, "nl-NL")),
        (
            "Wire -103K1:A2 → -103K2:A1 runs through the text \"-103K3\" of -103K3.",
            "Draad -103K1:A2 → -103K2:A1 loopt door de tekst \"-103K3\" van -103K3.".to_owned()
        )
    );
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
        id: ew_derive::ew_ids::Id::from_uuid(rep),
        words: "-103K1:A2 → -103K2:A2",
    };
    let owner = TextOwner::Ip {
        rep: ew_derive::ew_ids::Id::from_uuid(rep),
        name: "X-9",
    };
    let f = wire_through_text(ew_derive::ew_ids::Id::from_uuid(rep), wire, owner, "X-9");
    assert_eq!(
        (f.message.as_str(), ew_api::checks::render(&f, "nl-NL")),
        (
            "Wire -103K1:A2 → -103K2:A2 runs through the text \"X-9\" of interruption point X-9.",
            "Draad -103K1:A2 → -103K2:A2 loopt door de tekst \"X-9\" van onderbrekingspunt X-9.".to_owned()
        )
    );
}

/// RK1, EW-CORE-072: wires from above, below, the left and the right into one junction; its twin, the junction before
/// the fourth wire, joins three.
#[test]
fn rk1_a_four_way_junction_fires_ew_core_072() {
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
    let page: ew_derive::ew_ids::Id<ew_derive::ew_schema::Page> =
        ew_derive::ew_ids::Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(7));
    let f = four_way_junction(page, "7", page.cast(), "");
    assert_eq!(
        (
            f.subject.as_str(),
            f.message.as_str(),
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
fn rk1_an_unrouted_wire_fires_ew_core_073_at_its_severities_and_takes_a_waiver() {
    use ew_api::checks::{CheckMode, FindingState, PRE_DELIVERY, Severity};
    let (mut s, page, k) = rk1_page(&[(60, 20), (60, 30)]);
    let w = draw(&mut s, &page, pin(&k[0], 1), pin(&k[1], 0), serde_json::json!([]));
    assert_eq!(
        findings(&s, "EW-CORE-073"),
        [(
            "Wire -103K1:A2 → -103K2:A1 has no route: Route page draws it.".to_owned(),
            "Draad -103K1:A2 → -103K2:A1 heeft geen route: Pagina routeren tekent hem.".to_owned(),
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

/// RK2: Golden A's check report, online and in release mode, holds none of EW-CORE-067 to -070 and -073.
#[test]
fn rk2_golden_a_s_check_report_holds_no_layout_finding() {
    use ew_api::checks::{CheckMode, DRAFTING, PRE_DELIVERY};
    let (_tmp, s) = golden_copy();
    for report in [
        s.check(&DRAFTING, CheckMode::Online),
        s.check(&PRE_DELIVERY, CheckMode::Release),
    ] {
        let layout: Vec<&str> = report
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
            .map(|f| f.message.as_str())
            .collect();
        assert_eq!(layout, Vec::<&str>::new());
        assert!(
            report.rules.iter().any(|m| m.id == "EW-CORE-073"),
            "the run evaluated the rules"
        );
    }
}
