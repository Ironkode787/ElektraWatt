// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer's reads on Golden A (WP-S2.03 part 1, acceptance check V1): `DisplayList::link_sources` against the
//! PDF writer's link annotations, `Session::page_rows`, `Session::page_cells`, `Session::resolve_goto`, and
//! `Session::link_targets` against the PDF's named destinations and the cross-reference texts.

mod golden_common;

use std::collections::BTreeMap;
use std::str::FromStr;

use ew_api::Session;
use ew_api::output::{CellLabel, ExportOptions, GotoError, PageSelection, page_uuid};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::Representation;
use ew_derive::{PageOrder, XrefKind, XrefRelationOf, XrefTextOf, anchor_point};
use ew_dl::{Destination, DisplayList};
use golden_common::{batch, ids_by_key, run_log};

fn rep(ids: &BTreeMap<String, String>, key: &str) -> Id<Representation> {
    let k = format!("representations|{key}");
    Id::from_str(ids.get(&k).unwrap_or_else(|| panic!("no representation {k}"))).unwrap()
}

/// The sheet display list of one folio.
fn sheet(s: &Session, folio: &str) -> DisplayList {
    s.export_pages(&ExportOptions {
        pages: PageSelection::Folios(vec![folio.to_owned()]),
        ..ExportOptions::default()
    })
    .unwrap()
    .pages
    .remove(0)
    .content
}

/// nm to PDF points (72 per inch).
fn pt(nm: i64) -> f64 {
    nm as f64 * 72.0 / 25_400_000.0
}

/// The A4 landscape sheet's height in points: the PDF's y axis runs up from the bottom.
const SHEET_H: i64 = 210_000_000;

/// The objects of a PDF whose structure is uncompressed (krilla writes the dictionaries in plain text): number →
/// body.
fn objects(text: &str) -> BTreeMap<u32, &str> {
    let mut out = BTreeMap::new();
    let mut rest = text;
    let mut base = 0;
    while let Some(i) = rest.find(" 0 obj") {
        let head = &text[..base + i];
        let start = head.rfind(|c: char| !c.is_ascii_digit()).map_or(0, |k| k + 1);
        let body_start = base + i + " 0 obj".len();
        let Some(end) = text[body_start..].find("endobj") else {
            break;
        };
        if let Ok(n) = text[start..base + i].parse::<u32>() {
            out.insert(n, &text[body_start..body_start + end]);
        }
        base = body_start + end;
        rest = &text[base..];
    }
    out
}

/// The object numbers of a `[a 0 R b 0 R …]` reference list that starts after `key` in `body`.
fn refs(body: &str, key: &str) -> Vec<u32> {
    let Some(i) = body.find(key) else {
        return Vec::new();
    };
    let rest = &body[i + key.len()..];
    let list = &rest[..rest.find(']').unwrap()];
    let words: Vec<&str> = list.split_whitespace().collect();
    words
        .chunks(3)
        .filter_map(|w| w.first().and_then(|n| n.parse().ok()))
        .collect()
}

/// The numbers of a `[...]` array that starts right after `key` in `body`.
fn numbers(body: &str, key: &str) -> Vec<f64> {
    let i = body.find(key).unwrap() + key.len();
    let rest = &body[i..];
    rest[..rest.find(']').unwrap()]
        .split(|c: char| c.is_whitespace() || c == '/')
        .filter_map(|n| n.parse().ok())
        .collect()
}

/// What the PDF says: per page (in page order) its link annotations as (destination name, rectangle), and every
/// named destination as name → (page index, x, y) in points.
struct Pdf {
    links: Vec<Vec<(String, [f64; 4])>>,
    dests: BTreeMap<String, (usize, f64, f64)>,
}

fn read_pdf(bytes: &[u8]) -> Pdf {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let objs = objects(&text);
    let tree = objs
        .values()
        .find(|b| b.contains("/Type/Pages"))
        .expect("a page tree");
    let pages = refs(tree, "/Kids[");
    let name_of = |body: &str| -> String {
        let d = body.find("/Dest(").unwrap() + "/Dest(".len();
        body[d..d + body[d..].find(')').unwrap()].to_owned()
    };
    let links = pages
        .iter()
        .map(|p| {
            refs(objs[p], "/Annots[")
                .into_iter()
                .map(|a| {
                    let r = numbers(objs[&a], "/Rect[");
                    (name_of(objs[&a]), [r[0], r[1], r[2], r[3]])
                })
                .collect()
        })
        .collect();
    let names = &text[text.find("/Dests<</Names[").unwrap() + "/Dests<</Names[".len()..];
    let names = &names[..names.find("]>>").unwrap()];
    let mut dests = BTreeMap::new();
    for part in names.split('(').skip(1) {
        let (name, tail) = part.split_once(')').unwrap();
        let obj: u32 = tail.split_whitespace().next().unwrap().parse().unwrap();
        let body = objs[&obj];
        let page: u32 = body
            .trim()
            .trim_start_matches('[')
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let xy = numbers(body, "/XYZ");
        let index = pages.iter().position(|p| *p == page).unwrap();
        dests.insert(name.to_owned(), (index, xy[0], xy[1]));
    }
    Pdf { links, dests }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.01
}

/// The PDF of every page, its reading, and the page order's folios.
fn golden_pdf(s: &Session) -> (Pdf, Vec<String>) {
    let pdf = s.export_pdf(&ExportOptions::default()).unwrap();
    (read_pdf(&pdf.bytes), s.page_labels())
}

#[test]
fn v1_link_sources_are_the_pdf_link_annotations_on_folios_101_to_103() {
    let s = golden_common::session(&batch());
    let (pdf, folios) = golden_pdf(&s);
    let mut matched = 0;
    for folio in ["101", "102", "103"] {
        let index = folios.iter().position(|f| f == folio).unwrap();
        let mut ours: Vec<(String, [f64; 4])> = sheet(&s, folio)
            .link_sources()
            .into_iter()
            .filter(|(d, _)| pdf.dests.contains_key(&d.pdf_name()))
            .map(|(d, r)| {
                let h = pt(SHEET_H);
                let (a, b) = (r.min(), r.max());
                (d.pdf_name(), [pt(a.x.0), h - pt(b.y.0), pt(b.x.0), h - pt(a.y.0)])
            })
            .collect();
        let mut theirs = pdf.links[index].clone();
        let key = |l: &(String, [f64; 4])| {
            (
                l.0.clone(),
                (l.1[0] * 100.0).round() as i64,
                (l.1[1] * 100.0).round() as i64,
            )
        };
        ours.sort_by_key(key);
        theirs.sort_by_key(key);
        assert_eq!(ours.len(), theirs.len(), "folio {folio}: {ours:?}\n{theirs:?}");
        assert!(!ours.is_empty(), "folio {folio} has links");
        for (o, t) in ours.iter().zip(&theirs) {
            assert_eq!(o.0, t.0, "folio {folio}");
            for k in 0..4 {
                assert!(
                    close(o.1[k], t.1[k]),
                    "folio {folio} {}: {:?} != {:?}",
                    o.0,
                    o.1,
                    t.1
                );
            }
            matched += 1;
        }
    }
    assert!(matched >= 30, "{matched}");
}

#[test]
fn v1_link_targets_are_the_pdf_named_destinations_and_print_the_xref_cells() {
    let b = batch();
    let ids = ids_by_key(&b);
    let s = golden_common::session(&b);
    let (pdf, folios) = golden_pdf(&s);
    let mut checked = 0;
    for folio in ["101", "102", "103"] {
        let sources = sheet(&s, folio).link_sources();
        let dests: Vec<Destination> = sources.iter().map(|(d, _)| *d).collect();
        let targets = s.link_targets(&dests);
        assert_eq!(targets.len(), dests.len());
        for (d, t) in dests.iter().zip(&targets) {
            let t = t.as_ref().unwrap_or_else(|| panic!("{folio}: {d:?} resolves"));
            let (page, x, y) = pdf.dests[&d.pdf_name()];
            assert_eq!(folios[page], t.folio, "{folio}: {d:?}");
            match d {
                Destination::Page(_) => assert_eq!((t.at_nm, &t.cell), (None, &None)),
                _ => {
                    // Ruling 5: a mismatch here stops the lane.
                    let at = t.at_nm.expect("a point");
                    assert!(
                        close(pt(at.0), x) && close(pt(SHEET_H) - pt(at.1), y),
                        "{d:?}: {at:?} vs {x} {y}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 30, "{checked}");
    // Each representation target prints its cell as the source's cross-reference prints it.
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let mut exact = 0;
    for folio in ["101", "102", "103"] {
        let page = order.entries.iter().find(|e| e.folio == folio).unwrap().page;
        let ew_derive::PageRef::Drawn(page) = page else {
            panic!()
        };
        for (key, id) in &ids {
            let Some(_) = key.strip_prefix("representations|") else {
                continue;
            };
            let rid: Id<Representation> = Id::from_str(id).unwrap();
            if q.reps().get(rid).is_none_or(|r| r.page != page) {
                continue;
            }
            let text = q.get::<XrefTextOf>(rid);
            let kind = q.get::<XrefRelationOf>(rid).kind.clone();
            for link in &text.links {
                let item = &text.text[link.range.clone()];
                let t = s.link_targets(&[Destination::Representation(link.rep.as_uuid())])[0]
                    .clone()
                    .unwrap();
                let cell = t.cell.unwrap();
                match kind {
                    XrefKind::Ip { .. } | XrefKind::MainToAux => {
                        assert_eq!(item, cell, "{key}");
                        exact += 1;
                    }
                    _ => assert!(item.contains(&cell), "{key}: {item} {cell}"),
                }
            }
        }
    }
    assert!(exact >= 20, "{exact}");
    // FR-XRF-019: the "103-G6" of PE-1.1 on folio 101 targets folio 103, cell G6.
    let pe = s.link_targets(&[Destination::Representation(rep(&ids, "PE-1.1@103").as_uuid())]);
    let pe = pe[0].clone().unwrap();
    assert_eq!((pe.folio.as_str(), pe.cell.as_deref()), ("103", Some("103-G6")));
    // External and unknown destinations resolve to nothing; a page destination to its page.
    let none = Uuid::from_u128(0xdead_beef);
    assert_eq!(
        s.link_targets(&[
            Destination::Representation(none),
            Destination::Cp(none),
            Destination::Page(none)
        ]),
        vec![None, None, None]
    );
    let first = order.entries[0].page;
    let page = s.link_targets(&[Destination::Page(page_uuid(first))])[0]
        .clone()
        .unwrap();
    assert_eq!(
        (page.folio.as_str(), page.at_nm, page.cell),
        (folios[0].as_str(), None, None)
    );
}

#[test]
fn v1_link_targets_print_the_eplan_preset() {
    let b = batch();
    let ids = ids_by_key(&b);
    let mut s = golden_common::session(&b);
    eplan(&mut s);
    let t = s.link_targets(&[Destination::Representation(rep(&ids, "24VDC-1@103").as_uuid())]);
    assert_eq!(t[0].as_ref().unwrap().cell.as_deref(), Some("/103.A1"));
    let text = s.query().get::<XrefTextOf>(rep(&ids, "24VDC-1@102"));
    assert_eq!(text.text, "/103.A1");
}

/// Switches the `ip` format to the EPLAN cell preset (doc 06 §2.8) through `project.settings.set`.
fn eplan(s: &mut Session) {
    let log = concat!(
        r#"{"$schema":"ew.cmdlog/1","label":"eplan"}"#,
        "\n",
        r#"{"cmd":"project.settings.set","params":{"member":"xref_format","value":{"key":"eplan-cell","ip":"/{page}.{row}{column}","aux_to_main":"(/{page}.{row}{column})","main_to_aux":"∞ {list}","structure":"when_different","same_page":"full","matrix":{"per_row":3,"max":9,"overflow":"…"}}}}"#,
        "\n"
    );
    run_log(s, log, "eplan.ewcmd").unwrap();
}

/// `page_index_of` is the navigator's copy of the xref engine's private `{page_index}` (the engine's function is
/// not public, so the copy stays): with `{page_index}` in the `ip` format, the cell that `Session::link_targets`
/// prints for a representation is the one the engine prints in the source's cross-reference text, for every link of
/// the golden project. The count skips the continuation page 2.1 before folio 101, so the index of 101 is 6 and not 7
/// (FR-XRF-003).
#[test]
fn v1_the_navigators_page_index_is_the_xref_engines() {
    let b = batch();
    let ids = ids_by_key(&b);
    let mut s = golden_common::session(&b);
    let log = concat!(
        r#"{"$schema":"ew.cmdlog/1","label":"page-index"}"#,
        "\n",
        r#"{"cmd":"project.settings.set","params":{"member":"xref_format","value":{"key":"page-index","ip":"{page_index}.{row}{column}","aux_to_main":"({page_index}.{row}{column})","main_to_aux":"∞ {list}","structure":"when_different","same_page":"full","matrix":{"per_row":3,"max":9,"overflow":"…"}}}}"#,
        "\n"
    );
    run_log(&mut s, log, "page-index.ewcmd").unwrap();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    // The order has a continuation page (a generated page with an index above 0) before the drawn pages.
    assert!(
        order
            .entries
            .iter()
            .any(|e| matches!(e.page, ew_derive::PageRef::Generated { index, .. } if index > 0)),
        "the golden page order has a continuation page"
    );
    let (mut compared, mut indices) = (0, std::collections::BTreeSet::new());
    for (key, id) in &ids {
        if !key.starts_with("representations|") {
            continue;
        }
        let text = q.get::<XrefTextOf>(Id::from_str(id).unwrap());
        for link in &text.links {
            let item = &text.text[link.range.clone()];
            let cell = s.link_targets(&[Destination::Representation(link.rep.as_uuid())])[0]
                .clone()
                .unwrap()
                .cell
                .unwrap();
            // The IP and main-to-aux items are the cell alone; the aux-to-main item wraps it in parentheses.
            assert!(item.contains(&cell), "{key}: {item} vs {cell}");
            indices.insert(cell.split('.').next().unwrap().to_owned());
            compared += 1;
        }
    }
    assert!(compared >= 30, "{compared}");
    assert!(
        indices.contains("6"),
        "folio 101 is the sixth page record: {indices:?}"
    );
    assert!(indices.len() >= 3, "{indices:?}");
}

#[test]
fn v1_page_rows_follow_the_page_order() {
    let s = golden_common::session(&batch());
    let rows = s.page_rows();
    let labels = s.page_labels();
    assert_eq!(rows.len(), labels.len());
    assert_eq!(rows.iter().map(|r| r.folio.clone()).collect::<Vec<_>>(), labels);
    let row = |f: &str| rows.iter().find(|r| r.folio == f).unwrap();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    for (r, e) in rows.iter().zip(&order.entries) {
        assert_eq!(r.id, page_uuid(e.page));
        assert_eq!(
            r.generated,
            matches!(e.page, ew_derive::PageRef::Generated { .. }),
            "{}",
            r.folio
        );
        assert_eq!(r.generated, r.kind == "report", "{}", r.folio);
        assert_eq!(r.section.is_none(), e.section.is_none(), "{}", r.folio);
        assert!(!r.title.starts_with(&r.folio), "{}: {}", r.folio, r.title);
    }
    assert!(rows.iter().any(|r| r.generated));
    let p101 = row("101");
    assert_eq!((p101.kind.as_str(), p101.kind_rank), ("multi_line", 5));
    assert!(p101.qet_grid);
    assert_eq!(p101.section.as_ref().map(|s| s.0), Some(1));
    assert!(p101.title.starts_with("Power Supply 1"), "{}", p101.title);
    let title = row("1");
    assert_eq!(
        (title.kind.as_str(), title.kind_rank, title.section.clone()),
        ("title_page", 0, None)
    );
    // Sorting by type keeps page order inside a type: kind_rank is the schema's declaration order.
    assert!(rows.iter().all(|r| r.kind_rank <= 14));
}

#[test]
fn v1_page_cells_on_the_qet_frame() {
    let b = batch();
    let ids = ids_by_key(&b);
    let s = golden_common::session(&b);
    let cells = s.page_cells("102").expect("folio 102 has cells");
    let grid = s.page_grid("101").unwrap();
    // The L-1 anchor on 101 (page coordinates) mapped to the sheet lies in H16.
    let q = s.query();
    let anchor = anchor_point(&q, rep(&ids, "L-1@101")).unwrap();
    let to_sheet =
        |v: i64, off: i64| off + (i128::from(v) * i128::from(grid.sheet_scale_ppb) / 1_000_000_000) as i64;
    let at = (
        to_sheet(anchor.x.0, grid.sheet_offset_nm.0),
        to_sheet(anchor.y.0, grid.sheet_offset_nm.1),
    );
    let c101 = s.page_cells("101").unwrap();
    assert_eq!(c101.cell_at(at).map(|c| c.to_string()), Some("H16".to_owned()));
    // Column 15 spans x 249.9 to 265.6 mm on the golden frame (doc 16 §3.6, doc 20 §17.1, whose 265.6 is
    // 15.01 + 16 × 15.66 rounded; the exact edge prints at 265.525 mm).
    let h15 = CellLabel {
        row: "h".into(),
        column: "15".into(),
        row_index: 0,
        column_index: 0,
    };
    let [x0, y0, x1, y1] = cells.cell_rect(&h15).unwrap();
    assert_eq!((x0 + 50_000) / 100_000, 2_499, "{x0}");
    assert!((x1 - 265_600_000).abs() < 100_000, "{x1}");
    let (pw, ph) = cells.pitch_nm();
    assert!(
        (x1 - x0 - pw).abs() <= 1 && (y1 - y0 - ph).abs() <= 1,
        "each edge rounds on its own"
    );
    // A boundary point belongs to the right and lower cell; just left of it is column 14.
    let h15_at = cells.cell_at((x0, y0)).unwrap();
    assert_eq!((h15_at.row.as_str(), h15_at.column.as_str()), ("H", "15"));
    assert_eq!((h15_at.row_index, h15_at.column_index), (7, 15));
    assert_eq!(cells.cell_at((x0 - 2_000, y0 + 1_000_000)).unwrap().column, "14");
    assert_eq!(cells.cell_at((x0 + 1_000_000, y0 - 2_000)).unwrap().row, "G");
    assert_eq!(cells.text(&h15_at), "102-H15");
    // Outside the drawing area: nothing.
    assert_eq!(cells.cell_at((1_000_000, 1_000_000)), None);
    assert_eq!(cells.cell_at((150_000_000, 190_000_000)), None, "the title block");
    // Every cell's rectangle round-trips through cell_at; indices order column 10 after column 2.
    for row in ["A", "B", "C", "D", "E", "F", "G", "H"] {
        for column in 0..17 {
            let label = CellLabel {
                row: row.into(),
                column: column.to_string(),
                row_index: 0,
                column_index: 0,
            };
            let [a, b2, c, d] = cells.cell_rect(&label).unwrap();
            let found = cells.cell_at(((a + c) / 2, (b2 + d) / 2)).unwrap();
            assert_eq!(found.to_string(), format!("{row}{column}"));
            assert_eq!(found.column_index, column);
        }
    }
    let at = |x: i64| cells.cell_at((x, y0 + 1_000_000)).unwrap();
    let (c2, c10) = (
        at(15_010_000 + 2 * 15_660_000 + 1_000_000),
        at(15_010_000 + 10 * 15_660_000 + 1_000_000),
    );
    assert_eq!((c2.column.as_str(), c10.column.as_str()), ("2", "10"));
    assert!(
        c10.column_index > c2.column_index && c10.column < c2.column,
        "labels misorder, indices do not"
    );
    assert_eq!(
        cells.cell_rect(&CellLabel {
            row: "Z".into(),
            ..h15.clone()
        }),
        None
    );
    // Pages without a frame grid and unknown folios have no cells.
    let generated = s.page_rows().into_iter().find(|r| r.generated).unwrap().folio;
    assert!(s.page_cells(&generated).is_none(), "{generated}");
    assert!(s.page_cells("no such folio").is_none());
}

#[test]
fn v1_resolve_goto() {
    let mut s = golden_common::session(&batch());
    let cells = s.page_cells("103").unwrap();
    let page = s.resolve_goto(" 103 ").unwrap();
    assert_eq!(
        (page.folio.as_str(), page.cell, page.rect_nm),
        ("103", None, None)
    );
    let a1 = s.resolve_goto("103-A1").unwrap();
    assert_eq!(a1.folio, "103");
    assert_eq!(a1.cell.as_ref().map(ToString::to_string), Some("A1".to_owned()));
    let rect = a1.rect_nm.unwrap();
    assert_eq!(
        cells.cell_at(((rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2)),
        a1.cell
    );
    assert_eq!(
        s.resolve_goto("103-a1").unwrap(),
        a1,
        "labels match case-insensitively"
    );
    assert_eq!(
        a1.page,
        s.page_rows().iter().find(|r| r.folio == "103").unwrap().id
    );
    // The three errors.
    assert_eq!(s.resolve_goto("  "), Err(GotoError::Empty));
    assert_eq!(s.resolve_goto("999-A1"), Err(GotoError::NotFound));
    assert_eq!(
        s.resolve_goto("103-A"),
        Err(GotoError::NotFound),
        "no row and column to read"
    );
    assert_eq!(
        s.resolve_goto("103-A99"),
        Err(GotoError::NoSuchCell {
            folio: "103".into(),
            cell: "A99".into()
        })
    );
    assert_eq!(
        s.resolve_goto("103-Z9"),
        Err(GotoError::NoSuchCell {
            folio: "103".into(),
            cell: "Z9".into()
        })
    );
    let generated = s.page_rows().into_iter().find(|r| r.generated).unwrap().folio;
    assert_eq!(
        s.resolve_goto(&format!("{generated}-A1")),
        Err(GotoError::NoSuchCell {
            folio: generated.clone(),
            cell: "A1".into()
        }),
        "a page without a frame grid"
    );
    // The EPLAN cell preset: "/103.A1" is accepted and "103-A1" no longer is.
    eplan(&mut s);
    let e = s.resolve_goto("/103.A1").unwrap();
    assert_eq!((e.folio.as_str(), e.rect_nm), ("103", Some(rect)));
    assert_eq!(s.resolve_goto("103-A1"), Err(GotoError::NotFound));
    // A `{page_index}` format (QET `%f`, 1-based): folio 102 is the sixth page record.
    let indexed = concat!(
        r#"{"$schema":"ew.cmdlog/1","label":"index"}"#,
        "\n",
        r#"{"cmd":"project.settings.set","params":{"member":"xref_format","value":{"key":"qet","ip":"%f-%l%c","aux_to_main":"({page}-{row}{column})","main_to_aux":"∞ {list}","structure":"when_different","same_page":"full","matrix":{"per_row":3,"max":9,"overflow":"…"}}}}"#,
        "\n"
    );
    run_log(&mut s, indexed, "index.ewcmd").unwrap();
    let six = s.resolve_goto("6-A1").unwrap();
    assert_eq!(six.folio, "102");
    assert_eq!(
        s.page_cells("102").unwrap().text(six.cell.as_ref().unwrap()),
        "6-A1"
    );
    // A continuation page ("2.1") has no index of its own: with `%f`, "3-Z9" reads once, as folio 3 (the third
    // page record after 1 and 2), and names no cell of it.
    assert!(s.page_labels().contains(&"2.1".to_owned()));
    assert_eq!(
        s.resolve_goto("3-Z9"),
        Err(GotoError::NoSuchCell {
            folio: "3".into(),
            cell: "Z9".into()
        })
    );
}

/// Every end point of every path item of `items` on the sheet (paths under group transforms; instances are inlined).
fn path_ends(items: &[ew_dl::Item], stack: &[ew_dl::Transform], out: &mut Vec<(i64, i64)>) {
    for item in items {
        match &item.kind {
            ew_dl::ItemKind::Path(p) => {
                let pts: Vec<_> = p.path.points().collect();
                for q in [pts.first(), pts.last()].into_iter().flatten() {
                    let s = stack.iter().rev().fold(*q, |a, t| t.apply(a));
                    out.push((s.x.0, s.y.0));
                }
            }
            ew_dl::ItemKind::Group(g) => {
                let mut s = stack.to_vec();
                s.push(g.transform.unwrap_or(ew_dl::Transform::IDENTITY));
                path_ends(&g.items, &s, out);
            }
            _ => {}
        }
    }
}

#[test]
fn t5_page_cps_on_folio_102() {
    let s = golden_common::session(&batch());
    let cps = s.page_cps("102");
    assert!(cps.len() >= 10, "{}", cps.len());
    // -102F1:2 is drawn where a wire of the sheet display list ends.
    let f1 = cps.iter().find(|c| c.label == "-102F1:2").unwrap_or_else(|| {
        panic!(
            "-102F1:2 in {:?}",
            cps.iter().map(|c| &c.label).collect::<Vec<_>>()
        )
    });
    let mut ends = Vec::new();
    path_ends(&sheet(&s, "102").items, &[], &mut ends);
    assert!(
        ends.iter()
            .any(|e| (e.0 - f1.at_nm.0).abs() <= 1 && (e.1 - f1.at_nm.1).abs() <= 1),
        "{:?} is no wire end",
        f1.at_nm
    );
    assert!(f1.connected);
    // FR-SYM-009: every symbol CP lies on the page's connection grid (the QET grid on 102), and the grid point
    // maps to the CP's sheet point.
    let grid = s.page_grid("102").unwrap();
    assert_eq!((grid.num_nm, grid.den), (7_937_500, 3));
    for c in &cps {
        let (kx, ky) = c.grid_index.unwrap_or_else(|| panic!("{} on the grid", c.label));
        let sheet = |k: i64, off: i64| {
            off as f64 + k as f64 * 7_937_500.0 / 3.0 * f64::from(grid.sheet_scale_ppb) / 1e9
        };
        assert!(
            (sheet(kx, grid.sheet_offset_nm.0) - c.at_nm.0 as f64).abs() < 2.0,
            "{}",
            c.label
        );
        assert!(
            (sheet(ky, grid.sheet_offset_nm.1) - c.at_nm.1 as f64).abs() < 2.0,
            "{}",
            c.label
        );
    }
    // Reading order: by the cell (column, then row, by index), then (y, x).
    let cells = s.page_cells("102").unwrap();
    let key = |c: &ew_api::output::PageCp| {
        let cell = cells.cell_at(c.at_nm);
        (
            cell.as_ref().map_or(u32::MAX, |l| l.column_index),
            cell.as_ref().map_or(u32::MAX, |l| l.row_index),
            c.at_nm.1,
            c.at_nm.0,
        )
    };
    assert!(cps.windows(2).all(|w| key(&w[0]) <= key(&w[1])), "reading order");
    // No CP twice; some CPs are free.
    let ids: std::collections::BTreeSet<_> = cps.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), cps.len());
    assert!(cps.iter().any(|c| !c.connected), "a free CP on 102");
    assert!(s.page_cps("no such folio").is_empty());
    assert!(s.page_cps("2").is_empty(), "a generated page draws no CP");
}
