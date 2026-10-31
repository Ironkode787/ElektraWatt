// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 5 (acceptance IM1, FR-SYM-025): an image placed on a page of Golden A's import. An SVG logo is
//! vector paths in the page's display list and in the exported PDF (no raster image object for it); a PNG is an image
//! object; an unknown hash is refused. The assets are placeholders written here (no company's artwork).

mod golden_common;

use ew_api::output::{ExportOptions, PageSelection};
use ew_api::{AnyCommand, Session};
use ew_dl::{Color, Item, ItemKind};
use golden_common::{batch, ids_by_key, session};
use serde_json::{Value, json};
use sha2::Digest;

const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10" fill="#123456"/></svg>"##;

fn cmd(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// A PNG of `w × h` opaque red pixels (synthetic).
fn red_png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer
        .write_image_data(&[255, 0, 0, 255].repeat((w * h) as usize))
        .unwrap();
    writer.finish().unwrap();
    out
}

/// Brings `bytes` into the session's asset store through the company logo command (the one command that stores an
/// asset), takes the logo off again and returns the asset's hash text: the asset stays in the store.
fn hold(s: &mut Session, bytes: &[u8]) -> String {
    for logo in [ew_api::base64_encode(bytes), String::new()] {
        let p = s
            .preview(&cmd("project.company.set", json!({ "logo": logo })))
            .unwrap();
        s.commit(p).unwrap();
    }
    let hash = ew_api::Sha256(sha2::Sha256::digest(bytes).into());
    assert!(s.asset(&hash).is_some(), "the asset stays in the store");
    hash.to_string()
}

fn place(s: &mut Session, page: &str, asset: &str) -> Result<ew_api::Receipt, ew_api::CmdError> {
    s.execute(&cmd(
        "item.place",
        json!({"page": page, "kind": {"image": {"asset": asset,
            "rect": [[20_000_000, 40_000_000], [100_000_000, 60_000_000]]}}}),
    ))
}

fn folio_101() -> ExportOptions {
    ExportOptions {
        pages: PageSelection::Folios(vec!["101".to_owned()]),
        ..ExportOptions::default()
    }
}

/// Every item of `items`, groups flattened (pre-order).
fn flat(items: &[Item], out: &mut Vec<Item>) {
    for i in items {
        out.push(i.clone());
        if let ItemKind::Group(g) = &i.kind {
            flat(&g.items, out);
        }
    }
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

/// IM1: an SVG image item reaches the page's display list as paths (its fill, no image item is left) and the PDF
/// as drawing operators (no image XObject); the item itself is the record's image with the asset's hash.
#[test]
fn im1_an_svg_image_item_is_vector_paths_in_the_display_list_and_the_pdf() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page = ids["pages|101"].clone();
    let mut s = session(&batch);
    let before = s.export_pdf(&folio_101()).unwrap();
    let hash = hold(&mut s, SVG);
    place(&mut s, &page, &hash).expect("placed");

    let pages = s.export_pages(&folio_101()).unwrap();
    assert!(
        !pages
            .findings
            .iter()
            .any(|f| matches!(f.finding, ew_scene::page::PageFinding::Image(_))),
        "no image finding"
    );
    let mut all = Vec::new();
    flat(&pages.pages[0].content.items, &mut all);
    assert!(
        !all.iter().any(|i| matches!(i.kind, ItemKind::Image(_))),
        "no image item is left in the display list"
    );
    let navy = Color::from_rgb8(0x12, 0x34, 0x56);
    let painted = all
        .iter()
        .filter(|i| matches!(&i.kind, ItemKind::Path(p) if p.fill == Some(ew_dl::Brush::Solid(navy))))
        .count();
    assert_eq!(painted, 1, "the logo's rectangle as one filled path");
    // The group of the image carries the record's entity: the entity rectangles pick it.
    let item = {
        let q = s.query();
        q.page_items().ids().next().expect("the item").as_uuid()
    };
    assert!(
        all.iter()
            .any(|i| i.entity.is_some_and(|e| e.0 == item) && matches!(i.kind, ItemKind::Group(_))),
        "the vector group is tagged with the item"
    );

    let pdf = s.export_pdf(&folio_101()).unwrap();
    assert_eq!((pdf.report.images, pdf.report.skipped_images), (0, 0));
    assert_eq!(count(&pdf.bytes, b"/Subtype/Image"), 0, "no raster image object");
    assert_eq!(count(&before.bytes, b"/Subtype/Image"), 0);
    assert_ne!(pdf.bytes, before.bytes, "the logo is drawn");
    let svg = s.export_svg(&folio_101()).unwrap();
    assert_eq!((svg.report.images, svg.report.skipped_images), (0, 0));
    assert!(
        svg.files[0].text.contains("#123456"),
        "the SVG writer draws the paths"
    );
}

/// IM1: a PNG image item stays an image item with the asset's bytes and is one image XObject in the PDF.
#[test]
fn im1_a_png_image_item_is_an_image_object() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page = ids["pages|101"].clone();
    let mut s = session(&batch);
    let png = red_png(8, 2);
    let hash = hold(&mut s, &png);
    place(&mut s, &page, &hash).expect("placed");

    let pages = s.export_pages(&folio_101()).unwrap();
    let mut all = Vec::new();
    flat(&pages.pages[0].content.items, &mut all);
    let images: Vec<_> = all
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Image(img) => Some(img.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 1, "one image item");
    assert_eq!(images[0].image.format, ew_dl::ImageFormat::Png);
    assert_eq!(&*images[0].image.bytes, png.as_slice(), "with the asset's bytes");
    let pdf = s.export_pdf(&folio_101()).unwrap();
    assert_eq!((pdf.report.images, pdf.report.skipped_images), (1, 0));
    assert_eq!(count(&pdf.bytes, b"/Subtype/Image"), 1, "one image XObject");
    assert_eq!(count(&pdf.bytes, b"/Width 8/Height 2"), 1, "the PNG's size");
}

/// IM1: an asset the project does not hold is refused, and nothing is placed.
#[test]
fn im1_an_unknown_hash_is_refused() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page = ids["pages|101"].clone();
    let mut s = session(&batch);
    // The golden project holds page items of its own (folio 902's surface view, WP-S4.01 part 3).
    let before = s.query().page_items().ids().count();
    let unknown = ew_api::Sha256([0x5a; 32]).to_string();
    let err = place(&mut s, &page, &unknown).unwrap_err().to_string();
    assert!(err.contains(&unknown) && err.contains("holds no asset"), "{err}");
    assert_eq!(s.query().page_items().ids().count(), before, "nothing was placed");
}

/// The page tables list the image with its rectangle as `image`.
#[test]
fn the_page_tables_list_an_image_item() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page = ids["pages|101"].clone();
    let mut s = session(&batch);
    let hash = hold(&mut s, SVG);
    place(&mut s, &page, &hash).expect("placed");
    let tables = s.page_tables("101").expect("tables");
    assert_eq!(tables.items.len(), 1);
    assert_eq!(tables.items[0].kind, "image");
    assert_eq!(
        tables.items[0].bounds,
        [20_000_000, 40_000_000, 100_000_000, 60_000_000]
    );
}

/// Cycle-29 review 4: a page's views are memoised, and an image's format comes from the asset the project holds. An
/// image item whose asset the project does not hold (its file is missing) reads as a PNG; the same file placed on
/// another page with `data` brings the asset in the same transaction without touching the first page's rows, so the
/// first page's memoised views must follow: they equal the cold ones (an SVG now: vector paths, no raster image).
#[test]
fn an_asset_that_arrives_with_an_item_on_another_page_reaches_the_memoised_views() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let (page_a, page_b) = (ids["pages|101"].clone(), ids["pages|102"].clone());
    let hash = ew_api::Sha256(sha2::Sha256::digest(SVG).into());
    let mut s = golden_common::session_with(&batch, |_, records| {
        let item: ew_schema::PageItem = serde_json::from_value(json!({
            "id": "0192f3a0-0000-7000-8000-00000000a001", "page": page_a,
            "kind": {"image": {"asset": hash.to_string(),
                "rect": [[20_000_000, 40_000_000], [100_000_000, 60_000_000]]}}}))
        .unwrap();
        records.push(ew_schema::Record::PageItem(item));
    });
    assert!(s.asset(&hash).is_none(), "the file is missing");
    // The memo store holds the views of every page, page 101's with the image as a PNG of unknown bytes.
    let report = ew_api::verify::verify_derived(&s).expect("memo on");
    assert!(report.is_clean(), "{:?}", report.differences);
    let before = s.export_svg(&folio_101()).unwrap();
    assert_eq!(before.report.skipped_images, 1, "the missing file is reported");
    assert!(!before.files[0].text.contains("#123456"));

    s.execute(&cmd(
        "item.place",
        json!({"page": page_b, "kind": {"image": {"data": ew_api::base64_encode(SVG),
            "rect": [[20_000_000, 40_000_000], [100_000_000, 60_000_000]]}}}),
    ))
    .expect("placed with the file itself");
    assert!(s.asset(&hash).is_some());

    // Page 101's memoised views equal their cold values, and its image is the SVG now.
    let report = ew_api::verify::verify_derived(&s).expect("memo on");
    assert!(report.is_clean(), "{:?}", report.differences);
    let after = s.export_svg(&folio_101()).unwrap();
    assert_eq!(after.report.skipped_images, 0);
    assert!(
        after.files[0].text.contains("#123456"),
        "page 101 draws the logo's paths"
    );
}

/// Cycle-29 review 2, item 2: an asset can arrive in a transaction that changes no row at all. The project names its
/// company logo by the full reference, the logo's file is missing, and an image item names the same hash; the same
/// logo bytes set again change no row (the project row already names them) but bring the asset. The memo store must
/// follow (it is advanced although the revision stayed): the image's views equal the cold ones, an SVG now.
#[test]
fn an_asset_that_arrives_without_any_row_reaches_the_memoised_views() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page_a = ids["pages|101"].clone();
    let reference = ew_api::asset_ref_of(SVG).expect("the placeholder is an SVG");
    let mut s = golden_common::session_with(&batch, |project, records| {
        project.extra.insert(
            ew_schema::COMPANY_MEMBER.into(),
            json!({"logo": serde_json::to_value(reference).unwrap()}),
        );
        let item: ew_schema::PageItem = serde_json::from_value(json!({
            "id": "0192f3a0-0000-7000-8000-00000000a001", "page": page_a,
            "kind": {"image": {"asset": reference.sha256.to_string(),
                "rect": [[20_000_000, 40_000_000], [100_000_000, 60_000_000]]}}}))
        .unwrap();
        records.push(ew_schema::Record::PageItem(item));
    });
    assert!(s.asset(&reference.sha256).is_none(), "the logo's file is missing");
    // The memo store holds every page's views, page 101's with the image as a PNG of unknown bytes.
    let report = ew_api::verify::verify_derived(&s).expect("memo on");
    assert!(report.is_clean(), "{:?}", report.differences);
    let before = s.export_svg(&folio_101()).unwrap();
    assert!(before.report.skipped_images >= 1, "the missing file is reported");

    let rev = s.world().rev;
    let p = s
        .preview(&cmd(
            "project.company.set",
            json!({ "logo": ew_api::base64_encode(SVG) }),
        ))
        .unwrap();
    s.commit(p).unwrap();
    assert!(s.asset(&reference.sha256).is_some(), "the asset arrived");
    assert_eq!(s.world().rev, rev, "no row changed");

    let report = ew_api::verify::verify_derived(&s).expect("memo on");
    assert!(report.is_clean(), "{:?}", report.differences);
    let after = s.export_svg(&folio_101()).unwrap();
    assert!(
        after.files[0].text.contains("#123456"),
        "page 101 draws the logo's paths"
    );
}
