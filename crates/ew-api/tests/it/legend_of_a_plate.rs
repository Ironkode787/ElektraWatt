// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A legend belongs to the plate, not to the page (doc 12 §5.4, FT-2031): with a second layout page of the sample's
//! plate before its own, the sample's legend list follows that first page, by its folios and its title, both pages
//! name it, and neither is given a second.

use crate::common;

use ew_api::Session;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{PageOrder, PageRef};
use ew_schema::{Page, PageItemKind, ReportBlock};
use serde_json::json;

/// The sample with a new layout page 901 that shows the plate of the sample's layout page; the new page's ID, the
/// sample's layout page and its legend block.
fn two_pages_of_one_plate() -> (Session, Uuid, Id<Page>, Id<ReportBlock>) {
    let mut s = common::sample();
    let q = s.query();
    let (block, layout) = q
        .report_blocks()
        .iter()
        .find_map(|(id, b)| b.layout_page.map(|p| (id, p)))
        .expect("the sample's legend list");
    let view = q
        .page_items()
        .iter()
        .find_map(|(_, i)| match &i.kind {
            PageItemKind::SurfaceView(v) if i.page == layout => Some(v.clone()),
            _ => None,
        })
        .expect("the sample's layout shows its plate");
    drop(q);
    common::run_cmd(
        &mut s,
        "page.create",
        json!({"number": 901, "kind": "panel_layout", "title": "Plate, detail"}),
    );
    let page = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "901")
        .expect("page 901")
        .id;
    common::run_cmd(
        &mut s,
        "page.surface_view.set",
        json!({"page": page.to_string(), "surface": view.surface.as_uuid().to_string(),
            "datum_at": [40_000_000, 160_000_000], "scale": {"one_to": 5}}),
    );
    (s, page, layout, block)
}

#[test]
fn a_legend_follows_the_first_layout_page_of_its_plate() {
    let (s, page, layout, block) = two_pages_of_one_plate();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let first = order
        .entry(PageRef::Generated { block, index: 0 })
        .map(|e| e.folio.clone())
        .expect("the legend's first page");
    assert_eq!(first, "901.1", "the legend follows the plate's first layout page");
    let at = |p: PageRef| order.position(p).expect("in the page order");
    assert_eq!(
        at(PageRef::Generated { block, index: 0 }),
        at(PageRef::Drawn(Id::from_uuid(page))) + 1,
        "right after it"
    );
    assert!(at(PageRef::Drawn(Id::from_uuid(page))) < at(PageRef::Drawn(layout)));
    let title = q.get::<ew_derive::PageTitle>(PageRef::Generated { block, index: 0 });
    let english = title.text_for(&"en-GB".parse().unwrap()).unwrap_or_default();
    assert!(english.ends_with(" 901"), "the title names the page it follows: {english}");
    drop(q);
    // Both pages of the plate name the one legend, and neither is given a second.
    for p in [page, layout.as_uuid()] {
        assert_eq!(s.legend_page_folio(p).as_deref(), Some("901.1"));
        assert_eq!(s.legend_offer(p), ew_api::output::LegendOffer::Paged);
    }
}

#[test]
fn a_second_legend_list_of_one_plate_is_refused_from_its_other_page() {
    let (mut s, page, _, block) = two_pages_of_one_plate();
    let form = s.query().report_blocks().get(block).expect("the legend").form;
    let refused = s
        .execute(&ew_api::AnyCommand {
            name: "report_block.create".into(),
            params: json!({"source": {"kind": "enclosure_legend"}, "form": form.as_uuid().to_string(),
                "layout_page": page.to_string()}),
        })
        .expect_err("a second legend of one plate");
    let words = refused.to_string();
    assert!(
        words.contains("the plate of layout 901 has its legend list already"),
        "{words}"
    );
}
