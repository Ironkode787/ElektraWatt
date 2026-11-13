// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Underlays (doc 14 §5.5, FR-IMP-030): a DXF drawing and a PDF page placed under a page, embedded, one undo step each;
//! drawn on the canvas, absent from an export unless it prints underlays; locked (no entity to pick); no check reads
//! them; a file that is no PDF refused in words.

use ew_api::dxf::{DxfError, DxfOptions};
use ew_api::output::{ExportOptions, PageSelection, UnderlaySource};
use ew_api::{AnyCommand, Session, SessionOptions};
use ew_dl::{Item, ItemKind};
use ew_schema::{PageItemKind, UnderlayFormat};

/// A project with page 1.
fn project() -> Session {
    let mut s = Session::from_records(
        serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b39","derivation_profile":"2027.1"}"#,
        )
        .unwrap(),
        [],
        [],
        SessionOptions::default(),
    )
    .unwrap();
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": 1, "title": "Plan"}),
    })
    .unwrap();
    s
}

/// A drawing of a 100 × 50 mm rectangle and a text.
fn drawing() -> Vec<u8> {
    let pairs = [
        ("0", "SECTION"),
        ("2", "HEADER"),
        ("9", "$INSUNITS"),
        ("70", "4"),
        ("0", "ENDSEC"),
        ("0", "SECTION"),
        ("2", "ENTITIES"),
        ("0", "LWPOLYLINE"),
        ("8", "0"),
        ("90", "4"),
        ("70", "1"),
        ("10", "20"),
        ("20", "20"),
        ("10", "120"),
        ("20", "20"),
        ("10", "120"),
        ("20", "70"),
        ("10", "20"),
        ("20", "70"),
        ("0", "TEXT"),
        ("8", "0"),
        ("10", "30"),
        ("20", "40"),
        ("40", "3.5"),
        ("1", "Pump room"),
        ("0", "ENDSEC"),
        ("0", "EOF"),
    ];
    pairs
        .iter()
        .map(|(c, v)| format!("{c}\n{v}\n"))
        .collect::<String>()
        .into_bytes()
}

fn page_items(items: &[Item], out: &mut usize, picked: &mut usize) {
    for i in items {
        *out += 1;
        if i.entity.is_some() {
            *picked += 1;
        }
        if let ItemKind::Group(g) = &i.kind {
            page_items(&g.items, out, picked);
        }
    }
}

/// The sheet items of page 1 (and how many carry an entity), with or without underlays.
fn sheet(s: &Session, underlays: bool) -> (usize, usize, Vec<Item>) {
    let pages = s
        .export_pages(&ExportOptions {
            pages: PageSelection::Folios(vec!["1".to_owned()]),
            underlays,
            ..ExportOptions::default()
        })
        .unwrap();
    let items = pages.pages[0].content.items.clone();
    let (mut n, mut picked) = (0, 0);
    page_items(&items, &mut n, &mut picked);
    (n, picked, items)
}

fn underlays(s: &Session) -> Vec<ew_schema::Underlay> {
    s.query()
        .page_items()
        .iter()
        .filter_map(|(_, i)| match &i.kind {
            PageItemKind::Underlay(u) => Some(u.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_dxf_underlay_is_drawn_on_the_canvas_only_locked_and_undone_in_one_step() {
    let mut s = project();
    let (bare, bare_picked, _) = sheet(&s, true);
    // The import run's own report (EW-IMP, such as the SHX font of the drawing) is listed as its findings; no other
    // rule reads the underlay.
    let findings = |s: &Session| -> Vec<String> {
        s.check(&ew_api::checks::FULL, ew_api::checks::CheckMode::Release)
            .findings
            .iter()
            .filter(|f| !f.rule.starts_with("EW-IMP-"))
            .map(|f| format!("{f:?}"))
            .collect()
    };
    let checks_before = findings(&s);
    let source = UnderlaySource::Dxf(DxfOptions::default());
    s.place_underlay("1", "plan.dxf", drawing(), &source, true, Vec::new())
        .unwrap();
    let u = underlays(&s);
    assert_eq!(u.len(), 1);
    assert_eq!(u[0].format, UnderlayFormat::Dxf);
    assert_eq!(u[0].name, "plan.dxf");
    assert!(
        s.asset(&u[0].asset).is_some(),
        "embedded: the source is a project asset"
    );

    let (with, picked, _) = sheet(&s, true);
    assert!(with > bare, "the canvas draws the underlay");
    assert_eq!(picked, bare_picked, "nothing of the underlay can be picked");
    let (printed, _, _) = sheet(&s, false);
    assert_eq!(printed, bare, "an export without underlays leaves it out");
    assert_eq!(findings(&s), checks_before, "no check reads an underlay");

    s.undo(&ew_api::Origin::Gui).unwrap();
    assert!(underlays(&s).is_empty());
}

#[test]
fn a_pdf_page_underlay_covers_the_sheet_as_a_raster_and_a_file_that_is_no_pdf_is_refused() {
    // The PDF of another project's page is the source.
    let other = project();
    let pdf = other.export_pdf(&ExportOptions::default()).unwrap().bytes;
    let mut s = project();
    s.place_underlay(
        "1",
        "eplan.pdf",
        pdf,
        &UnderlaySource::Pdf { page: 1 },
        true,
        Vec::new(),
    )
    .unwrap();
    let (_, _, items) = sheet(&s, true);
    let image = items.iter().find_map(|i| match &i.kind {
        ItemKind::Image(img) => Some(img.image.clone()),
        _ => None,
    });
    let image = image.expect("the underlay's raster");
    assert!(image.bytes.starts_with(b"\x89PNG"), "a PNG raster");

    let e = s
        .place_underlay(
            "1",
            "fake.pdf",
            b"%PDF-1.7 not really".to_vec(),
            &UnderlaySource::Pdf { page: 1 },
            true,
            Vec::new(),
        )
        .unwrap_err();
    assert!(matches!(e, DxfError::Refused(_)), "{e:?}");
    assert!(e.to_string().contains("no PDF that can be read"), "{e}");
    let e = s
        .place_underlay(
            "9",
            "plan.dxf",
            drawing(),
            &UnderlaySource::Dxf(DxfOptions::default()),
            true,
            Vec::new(),
        )
        .unwrap_err();
    assert_eq!(e.to_string(), "The project has no drawn page 9.");
}
