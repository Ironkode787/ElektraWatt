// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.19 part 1 (ID-1498): a page's geometry and its layout measures on a temporary copy of Golden A. Folio 101
//! draws its connections along the provisional path, and the measures name what the owner saw there: the heater's
//! PE wire through the lamp's X1, and the lamp's L feed along the switch's wires. Named items only, never totals.

use std::path::{Path, PathBuf};

use ew_api::Session;
use ew_api::output::LayoutError;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// A temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A read-only session on a temporary copy of Golden A (the committed project is only copied).
fn golden_copy() -> (Tmp, Session) {
    let tmp = Tmp(ew_testdir::fresh_dir("ew-api-layout"));
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let session = Session::open_read_only(&dir, "ElektraWatt test").unwrap().0;
    (tmp, session)
}

/// WP-S2.19 RG2 on Golden A's routed folio 101 (rc.1's provisional drawing broke all three): the heater's PE wire does
/// not touch the lamp's X1, the lamp's L feed runs along none of the switch's wires, and the heater's L feed does not
/// pass through T55; the page breaks no hard rule.
#[test]
fn rg2_folio_101_s_pe_wire_lamp_feed_and_heater_feed_keep_clear() {
    let (_tmp, s) = golden_copy();
    let pages = s.layout_report(Some("101")).unwrap();
    assert_eq!(pages.len(), 1);
    let p = &pages[0];
    assert_eq!(p.folio, "101");
    assert!(
        !p.touches
            .iter()
            .any(|t| t.connection == "E55:PE → PE-1.1" && t.point == "E56:X1"),
        "{:#?}",
        p.touches
    );
    let feed = "the junction in C11 → E56:X2";
    assert!(
        !p.overlaps.iter().any(|o| o.connection == feed || o.other == feed),
        "{:#?}",
        p.overlaps
    );
    assert!(
        !p.through_symbols
            .iter()
            .any(|t| t.connection.ends_with("E55:L") && t.symbol == "T55"),
        "{:#?}",
        p.through_symbols
    );
    assert_eq!(p.hard(), 0, "{p:#?}");
    // The page as the whole-project report measures it.
    let all = s.layout_report(None).unwrap();
    assert_eq!(all.iter().find(|q| q.folio == "101"), Some(p));
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

/// ID-1502 (3): a free text and a structure box's caption are text boxes of the page's geometry, so measure (d) and
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
