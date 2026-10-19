// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page output of the golden project (WP-S3.04): page display lists, PNG candidates and the PDF of every
//! folio of `page_order`. It lives here because `ew-scene` reads derived data only through a caller-filled
//! view, and the caller with `Q` is the session layer (see `page_output/view.rs`).

#[path = "page_output/golden_files.rs"]
mod golden_files;
#[path = "page_output/snapshot.rs"]
mod snapshot;
#[path = "page_output/view.rs"]
mod view;

use std::sync::Arc;

use ew_derive::Q;
use ew_dl::{DisplayList, ItemKind, Layer};
use ew_export::{PdfDate, PdfOptions, PdfPage, pdf};
use ew_ids::{Id, Uuid};
use ew_scene::page::{
    ConnectionView, PageFinding, PageKind, PageScene, PageView, RepView, page_display_list, page_scene,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use golden_files::{
    blessing, check_or_bless, content_form, csv_rows, decode_rgba, differing_percent, expected_dir,
    frame_figures, frame_json, page_png, text_csv,
};
use view::{GoldenPage, builtin_symbol, destination, fonts, golden_pages, golden_session};

/// The PDF page list of the golden project.
fn pdf_pages(q: &Q) -> Vec<PdfPage> {
    golden_pages(q)
        .into_iter()
        .map(|g| {
            let scene = page_scene(&g.view, ew_text::bundled());
            PdfPage {
                destination: destination(g.entry.page),
                label: g.entry.folio.clone(),
                title: g.outline_title.clone(),
                section: g.section_title.clone(),
                width: scene.sheet_width,
                height: scene.sheet_height,
                content: scene.sheet_list(),
            }
        })
        .collect()
}

/// The export options of the golden PDF: dates from the model's edited date (FR-OUT-026).
fn pdf_options(q: &Q) -> PdfOptions {
    let m = &q.project().master;
    let date = m.edited.map_or(PdfDate::ymd(2000, 1, 1), |d| {
        PdfDate::ymd(d.year(), d.month(), d.day())
    });
    PdfOptions {
        title: Some(ew_scene::forms::ml_text(&m.description, None).to_owned()),
        creator: Some(format!("ElektraWatt {}", env!("CARGO_PKG_VERSION"))),
        date,
        document_id: format!("golden J-E-2416 {}", q.profile().version),
        ..PdfOptions::default()
    }
}

#[test]
fn the_golden_pdf_has_every_folio_and_is_byte_identical_across_runs_and_record_shuffles() {
    let s = golden_session(None);
    let q = Q::new(&s.world().model);
    let pages = pdf_pages(&q);
    assert_eq!(pages.len(), 16, "page_order of the golden model");
    let f = fonts();
    let a = pdf(&pages, &pdf_options(&q), &f, None).expect("the golden PDF is written");
    assert!(a.bytes.starts_with(b"%PDF-"));
    assert!(a.report.missing_fonts.is_empty(), "{:?}", a.report);
    let b = pdf(&pages, &pdf_options(&q), &f, None).unwrap();
    assert!(a.bytes == b.bytes, "two runs differ");
    let shuffled = golden_session(Some(0x5eed_1234));
    let q2 = Q::new(&shuffled.world().model);
    let c = pdf(&pdf_pages(&q2), &pdf_options(&q2), &f, None).unwrap();
    assert!(a.bytes == c.bytes, "a record shuffle changes the PDF");
}

fn with_golden_pages(f: impl FnOnce(&[GoldenPage])) {
    let s = golden_session(None);
    let q = Q::new(&s.world().model);
    f(&golden_pages(&q));
}

#[test]
fn the_text_csv_of_every_folio_matches_the_committed_file() {
    with_golden_pages(|pages| {
        assert_eq!(pages.len(), 16);
        for g in pages {
            let scene = page_scene(&g.view, ew_text::bundled());
            let csv = text_csv(&scene);
            if std::env::var_os("EW_CSV_ROWS").is_some() {
                eprintln!("{} rows {}", g.entry.folio, csv_rows(&csv));
            }
            assert!(csv_rows(&csv) > 0, "folio {} has texts", g.entry.folio);
            check_or_bless(
                &expected_dir("text").join(format!("{}.csv", g.entry.folio)),
                csv.as_bytes(),
            );
        }
    });
}

#[test]
fn the_frame_geometry_matches_the_committed_file() {
    check_or_bless(
        &expected_dir("geometry").join("frame.json"),
        frame_json().as_bytes(),
    );
}

#[test]
fn the_printed_frame_matches_doc_20_section_17_1() {
    let tb = content_form("ew.titleblock.qet-a4");
    let f = frame_figures(&content_form("ew.frame.qet-a4-landscape-grid"), &tb);
    let n = |v: &serde_json::Value| v.as_f64().unwrap();
    let near = |a: f64, b: f64, what: &str| assert!((a - b).abs() <= 0.01, "{what}: {a} vs {b}");
    near(n(&f["outer"]["x0"]), 9.79, "outer x0");
    near(n(&f["outer"]["x1"]), 281.18, "outer x1");
    near(n(&f["outer"]["y0"]), 9.79, "outer y0");
    near(n(&f["outer"]["y1"]), 199.76, "outer y1");
    near(n(&f["header_band"]), 15.01 - 9.79, "header band thickness");
    let xs = f["column_edges"].as_array().unwrap();
    let ys = f["row_edges"].as_array().unwrap();
    near(n(&xs[1]) - n(&xs[0]), 15.66, "column width");
    near(n(&ys[1]) - n(&ys[0]), 20.88, "row height");
    near(
        n(&xs[0]),
        15.01,
        "first column edge (the left band ends at 15.01)",
    );
    near(n(&f["drawing_area"]["y1"]), 182.02, "drawing area bottom");
}

#[test]
fn the_png_candidates_match_within_the_doc_20_tolerance() {
    let f = fonts();
    with_golden_pages(|pages| {
        for g in pages {
            let scene = page_scene(&g.view, ew_text::bundled());
            let bytes = page_png(&scene, &f);
            let path = expected_dir("png").join(format!("{}.png", g.entry.folio));
            if blessing() {
                check_or_bless(&path, &bytes);
                continue;
            }
            let committed = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("{}: {e} (run with EW_BLESS=1)", path.display()));
            let (w0, h0, a) = decode_rgba(&committed);
            let (w1, h1, b) = decode_rgba(&bytes);
            assert_eq!((w0, h0), (w1, h1), "folio {} size", g.entry.folio);
            let pct = differing_percent(&a, &b, 16);
            assert!(
                pct <= 0.5,
                "folio {}: {pct:.3} % of the pixels differ",
                g.entry.folio
            );
        }
    });
}

#[test]
fn the_png_of_a_folio_is_deterministic() {
    let f = fonts();
    with_golden_pages(|pages| {
        let g = folio(pages, "101");
        let scene = page_scene(&g.view, ew_text::bundled());
        let a = Sha256::digest(page_png(&scene, &f));
        let b = Sha256::digest(page_png(&scene, &f));
        assert_eq!(a, b);
    });
}

#[test]
fn every_committed_png_is_at_most_300_kb() {
    if blessing() {
        return;
    }
    let mut n = 0;
    for e in std::fs::read_dir(expected_dir("png")).unwrap() {
        let p = e.unwrap().path();
        let len = std::fs::metadata(&p).unwrap().len();
        assert!(len <= 300 * 1024, "{}: {len} bytes", p.display());
        n += 1;
    }
    assert_eq!(n, 16);
}

fn folio<'a>(pages: &'a [GoldenPage], folio: &str) -> &'a GoldenPage {
    pages
        .iter()
        .find(|g| g.entry.folio == folio)
        .expect("the folio exists")
}

#[test]
fn the_display_list_of_folio_101_is_stable() {
    with_golden_pages(|pages| {
        let scene = page_scene(&folio(pages, "101").view, ew_text::bundled());
        let text = snapshot::display_list_text(&scene, &[Layer::Symbols, Layer::Wires, Layer::Labels]);
        insta::assert_snapshot!(text, @r#"
        symbols instance 20d847a1 sym=6a8491f2 variant=A translate=(235479167,79375000)
        symbols instance 23a4ce3c sym=a144580f variant=A translate=(50270833,79375000)
        symbols instance 313e2bc4 sym=f8e64dbf variant=A translate=(267229167,169333333)
        symbols instance 57543b91 sym=88ca8636 variant=A translate=(44979167,37041667)
        symbols instance 5e0e0f25 sym=4f6bee27 variant=A translate=(7937500,142875000)
        symbols instance 6112da7c sym=b0f8e0f4 variant=A translate=(235479167,100541667)
        symbols instance 66c620c1 sym=13704217 variant=A translate=(39687500,79375000)
        symbols instance 682aa707 sym=e2cae815 variant=A translate=(203729167,58208333)
        symbols instance 7b76f7a3 sym=d5dded4b variant=A translate=(137583333,31750000)
        symbols instance 8c4dbb72 sym=d5dded4b variant=A translate=(201083333,31750000)
        symbols instance 92878d9b sym=4f6bee27 variant=A translate=(7937500,148166667)
        symbols instance 9945b5aa sym=13704217 variant=A translate=(44979167,79375000)
        symbols instance 9b2316f2 sym=f8e64dbf variant=A translate=(251354167,142875000)
        symbols instance dc21e14c sym=f8e64dbf variant=A translate=(267229167,164041667)
        symbols instance de795e26 sym=9a687b31 variant=A translate=(203729167,95250000)
        symbols instance ec75639e sym=f8b44164 variant=A translate=(92604167,79375000)
        symbols instance f4fdc202 sym=4f6bee27 variant=A translate=(7937500,137583333)
        wires path 09ed5beb els=3 first=(44979167,82020833) last=(50270834,44979167) stroke=260000 fill=-
        wires path 13c75a2c els=3 first=(7937500,142875000) last=(44979167,76729167) stroke=260000 fill=-
        wires path 17b31bbc els=3 first=(50270834,37041667) last=(145520833,31750000) stroke=260000 fill=-
        wires path 2b9b8f17 els=2 first=(137583333,31750000) last=(201083333,31750000) stroke=260000 fill=-
        wires path 2be33590 els=2 first=(203729167,66145833) last=(203729167,105833333) stroke=260000 fill=-
        wires path 301459db els=2 first=(145520833,31750000) last=(209020833,31750000) stroke=260000 fill=-
        wires path 35faab6c els=3 first=(206375000,100541667) last=(251354167,140229167) stroke=260000 fill=-
        wires path 48f5a362 els=2 first=(235479167,87312500) last=(235479167,100541667) stroke=260000 fill=-
        wires path 522a3985 els=3 first=(145520833,42333333) last=(267229167,166687500) stroke=260000 fill=-
        wires path 5945bf6c els=3 first=(7937500,148166667) last=(50270833,76729167) stroke=260000 fill=-
        wires path 5e9d92b4 els=3 first=(219604167,58208333) last=(235479167,79375000) stroke=260000 fill=-
        wires path 628881aa els=3 first=(39687500,82020833) last=(44979167,44979167) stroke=260000 fill=-
        wires path 6d50c972 els=3 first=(7937500,137583333) last=(39687500,76729167) stroke=260000 fill=-
        wires path 8d27ba7c els=3 first=(137583333,42333333) last=(267229167,161395834) stroke=260000 fill=-
        wires path a697efb2 els=3 first=(209020833,42333333) last=(219604167,58208333) stroke=260000 fill=-
        wires path aba94b57 els=3 first=(50270833,82020833) last=(92604167,79375000) stroke=260000 fill=-
        wires path b5382424 els=3 first=(187854167,58208333) last=(203729167,95250000) stroke=260000 fill=-
        wires path c851008e els=3 first=(187854167,58208333) last=(235479167,108479167) stroke=260000 fill=-
        wires path ce52ac00 els=3 first=(44979167,37041667) last=(137583333,31750000) stroke=260000 fill=-
        wires path d48db15f els=3 first=(201083333,42333333) last=(187854167,58208333) stroke=260000 fill=-
        wires path f40a98e3 els=2 first=(219604167,58208333) last=(203729167,58208333) stroke=260000 fill=-
        wires instance 0c0b22d7 sym=a3e1ca3e variant=A translate=(187854167,58208333)
        wires instance 1f9afba6 sym=a3e1ca3e variant=A translate=(219604167,58208333)
        labels glyphs 20d847a1 "S56" origin=(237463542,83910223) size=3439583 angle=0
        labels glyphs 20d847a1 "13" origin=(233304292,80508739) size=1322917 angle=0
        labels glyphs 20d847a1 "14" origin=(233304292,87123323) size=1322917 angle=0
        labels glyphs 23a4ce3c "1X1:PE" origin=(37420815,80602931) size=3439583 angle=0
        labels glyphs 57543b91 "S51" origin=(36510120,42238348) size=3439583 angle=0
        labels glyphs 57543b91 "1" origin=(45640625,38836865) size=1322917 angle=0
        labels glyphs 57543b91 "2" origin=(45640625,44128531) size=1322917 angle=0
        labels glyphs 57543b91 "3" origin=(50932292,38836865) size=1322917 angle=0
        labels glyphs 57543b91 "4" origin=(50932292,44128531) size=1322917 angle=0
        labels glyphs 6112da7c "E56" origin=(226324584,105738348) size=3439583 angle=0
        labels glyphs 6112da7c "X1" origin=(236140625,101675406) size=1322917 angle=0
        labels glyphs 6112da7c "X2" origin=(236140625,108289990) size=1322917 angle=0
        labels glyphs 66c620c1 "1X1:1" origin=(28863397,80602931) size=3439583 angle=0
        labels glyphs 682aa707 "T55" origin=(205713542,62082097) size=3439583 angle=0
        labels glyphs 682aa707 "5" origin=(202311000,59342072) size=1322917 angle=0
        labels glyphs 682aa707 "3" origin=(202311000,65956656) size=1322917 angle=0
        labels glyphs 7b76f7a3 "-101F1" origin=(124175045,38269598) size=3439583 angle=0
        labels glyphs 7b76f7a3 "1" origin=(138244791,33545198) size=1322917 angle=0
        labels glyphs 7b76f7a3 "2" origin=(138244791,41482698) size=1322917 angle=0
        labels glyphs 7b76f7a3 "N" origin=(146182291,33545198) size=1322917 angle=0
        labels glyphs 7b76f7a3 "N" origin=(146182291,41482698) size=1322917 angle=0
        labels glyphs 8c4dbb72 "F55" origin=(192717473,38269598) size=3439583 angle=0
        labels glyphs 8c4dbb72 "1" origin=(201744791,33545198) size=1322917 angle=0
        labels glyphs 8c4dbb72 "2" origin=(201744791,41482698) size=1322917 angle=0
        labels glyphs 8c4dbb72 "N" origin=(209682291,33545198) size=1322917 angle=0
        labels glyphs 8c4dbb72 "N" origin=(209682291,41482698) size=1322917 angle=0
        labels glyphs 9945b5aa "1X1:2" origin=(34155064,80602931) size=3439583 angle=0
        labels glyphs de795e26 "E55" origin=(195236043,101769598) size=3439583 angle=0
        labels glyphs de795e26 "L" origin=(204390625,97045198) size=1322917 angle=0
        labels glyphs de795e26 "N" origin=(204390625,104982698) size=1322917 angle=0
        labels glyphs de795e26 "PE" origin=(207036459,99492593) size=1322917 angle=0
        labels instance 09ed5beb sym=04b0176e variant=D translate=(44979167,63500000)
        labels glyphs 09ed5beb "2.5" origin=(45451448,64433979) size=1322917 angle=90000
        labels instance 17b31bbc sym=04b0176e variant=A translate=(97895833,31750000)
        labels glyphs 17b31bbc "2.5" origin=(96961854,32222281) size=1322917 angle=0
        labels instance 628881aa sym=04b0176e variant=D translate=(39687500,63500000)
        labels glyphs 628881aa "2.5" origin=(40159781,64433979) size=1322917 angle=90000
        labels instance ce52ac00 sym=04b0176e variant=A translate=(91281250,31750000)
        labels glyphs ce52ac00 "2.5" origin=(90347271,32222281) size=1322917 angle=0
        "#);
    });
}

/// Paths and glyph runs of a display list, groups included.
fn count(list: &DisplayList) -> (usize, usize) {
    fn go(items: &[ew_dl::Item], paths: &mut usize, runs: &mut usize) {
        for i in items {
            match &i.kind {
                ItemKind::Path(_) => *paths += 1,
                ItemKind::Glyphs(_) => *runs += 1,
                ItemKind::Group(g) => go(&g.items, paths, runs),
                _ => {}
            }
        }
    }
    let (mut paths, mut runs) = (0, 0);
    go(&list.items, &mut paths, &mut runs);
    (paths, runs)
}

/// Observed counts of the frame layer of folio 101, derived in the test below.
const FRAME_PATHS: usize = 38;
const FRAME_RUNS: usize = 41;

#[test]
fn the_frame_layer_of_folio_101_has_the_items_the_frame_rules_give() {
    with_golden_pages(|pages| {
        let v = &folio(pages, "101").view;
        let scene = page_scene(v, ew_text::bundled());
        let (paths, runs) = count(scene.layer(Layer::Frame).unwrap());
        // Frame rules of `forms/mod.rs`: outer rectangle 1, title-block separator 1, 2 header-band lines,
        // 16 column ticks and 7 row ticks = 27 paths; grid labels 17 columns + 8 rows = 25 runs. Title
        // block (`ew.titleblock.qet-a4`): 11 line graphics = 11 paths, and 18 text cells of which 16 print
        // text (the logo and revision cells are not bound yet and print nothing) = 16 runs.
        // Total: 27 + 11 = 38 paths and 25 + 16 = 41 runs.
        assert_eq!(
            (paths, runs),
            (FRAME_PATHS, FRAME_RUNS),
            "frame layer of folio 101"
        );
        // The frame reads only the frame: a wire edit never rebuilds the title block.
        let mut bare = v.clone();
        bare.reps.clear();
        bare.connections.clear();
        bare.junctions.clear();
        let bare = page_display_list(&bare, Layer::Frame, ew_text::bundled());
        assert!(bare.list == **scene.layer(Layer::Frame).unwrap());
    });
}

#[test]
fn the_display_lists_do_not_depend_on_the_order_of_the_view_records() {
    with_golden_pages(|pages| {
        for g in pages {
            let mut r = g.view.clone();
            r.reps.reverse();
            r.connections.reverse();
            r.junctions.reverse();
            for &layer in g.view.kind.layers() {
                let a = page_display_list(&g.view, layer, ew_text::bundled());
                let b = page_display_list(&r, layer, ew_text::bundled());
                assert!(a == b, "folio {} layer {}", g.entry.folio, layer.name());
            }
        }
    });
}

fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The hyphenated text of [`uuid`], the form of a record's ID member.
fn uid(n: u128) -> String {
    uuid(n).hyphenated().to_string()
}

/// A page of 100 terminals and 10 050 connections between pseudo-random CPs (50 with unknown ends).
fn hostile_view() -> PageView {
    let symbol = builtin_symbol("ew.iec.terminal@qet10").expect("the terminal symbol");
    let page = Id::from_uuid(uuid(0xAA));
    let mut reps = Vec::new();
    for i in 0..100u128 {
        let (col, row) = (i64::try_from(i % 10).unwrap(), i64::try_from(i / 10).unwrap());
        let (x, y) = ((col * 25 + 15) * 1_000_000, (row * 18 + 25) * 1_000_000);
        let rep = serde_json::from_value(json!({
            "id": uid(0x1000 + i),
            "functions": [uid(0x2000 + i)],
            "page": uid(0xAA),
            "symbol": symbol.id.as_uuid().hyphenated().to_string(),
            "pos": [x, y],
        }))
        .expect("a representation record");
        reps.push(RepView {
            rep: Arc::new(rep),
            symbol: Some(symbol.def.clone()),
            slots: Vec::new(),
        });
    }
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move |m: u64| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % m
    };
    let mut connections = Vec::new();
    for i in 0..10_050u128 {
        let mut end = || (0x2000 + u128::from(next(100)), next(2));
        let (mut a, mut b) = (end(), end());
        if i >= 10_000 {
            // Unresolvable: a function no representation draws.
            b.0 = 0x9000 + i;
        }
        if a == b {
            a.1 = 1 - a.1;
        }
        let c = serde_json::from_value(json!({
            "id": uid(0x10_0000 + i),
            "a": {"cp": [uid(a.0), a.1]},
            "b": {"cp": [uid(b.0), b.1]},
            "page": uid(0xAA),
        }))
        .expect("a connection record");
        connections.push(ConnectionView {
            connection: Arc::new(c),
            marker: None,
        });
    }
    PageView {
        page,
        kind: PageKind::Schematic,
        language: None,
        frame: None,
        reps,
        connections,
        junctions: Vec::new(),
        wire_marker: None,
    }
}

#[test]
fn a_hostile_page_renders_and_exports_without_panic() {
    let view = hostile_view();
    let scene: PageScene = page_scene(&view, ew_text::bundled());
    let unresolved = scene
        .findings
        .iter()
        .filter(|f| matches!(f, PageFinding::UnresolvedEnd { .. }))
        .count();
    assert_eq!(unresolved, 50);
    let wires = count(scene.layer(Layer::Wires).unwrap()).0;
    assert!(wires <= 10_000, "{wires} wire paths");
    let list = scene.sheet_list();
    let f = fonts();
    let sheet = ew_render_cpu::SheetSize {
        width: scene.sheet_width,
        height: scene.sheet_height,
    };
    let options = ew_render_cpu::RenderOptions {
        dpi: 72,
        ..Default::default()
    };
    ew_render_cpu::render(&list, sheet, &options, &f, None).expect("the hostile page renders");
    let page = PdfPage {
        destination: ew_dl::Destination::Page(uuid(0xAA)),
        label: "1".to_owned(),
        title: "hostile".to_owned(),
        section: None,
        width: scene.sheet_width,
        height: scene.sheet_height,
        content: list,
    };
    let options = PdfOptions {
        date: PdfDate::ymd(2000, 1, 1),
        ..PdfOptions::default()
    };
    let out = pdf(&[page], &options, &f, None).expect("the hostile PDF is written");
    assert!(out.bytes.starts_with(b"%PDF-"));
}
