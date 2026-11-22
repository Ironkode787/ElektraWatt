// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The legend as a list page of its own (doc 12 §5.4, doc 09 §2.2, §3.2). The test's own cabinet: on the sample's frame
//! and parts, a second enclosure at the sample's location +FLD, shown on a new panel page 920, with sixty devices
//! placed on its plate, each with a part whose description is one of the sample's longest; and its legend list, made
//! as Add legend makes it (`report_block.create {layout_page}`).

use std::collections::BTreeSet;

use crate::common;

use ew_api::Session;
use ew_api::output::{LEGEND_COLUMNS, page_layer};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{PageOrder, PageRef};
use ew_dl::{ItemKind, Layer};
use ew_schema::{CarrierRef, MountingSurface, Placement, ReportBlock, ReportKind, ReportSource};
use serde_json::json;

/// The devices on the test's plate.
const DEVICES: usize = 60;

/// The parts the test's devices take in turn: three of the sample's with long descriptions.
const PARTS: [&str; 3] = ["EW-GEN-CON-1", "EW-GEN-CPU-2", "EW-GEN-VTM-1"];

/// The sample with the test's cabinet and its legend list; the page 920's ID, its surface and the legend block.
fn cabinet() -> (Session, Uuid, Id<MountingSurface>, Id<ReportBlock>) {
    let mut s = common::sample();
    let q = s.query();
    let field = q
        .structure_nodes()
        .iter()
        .find(|(_, n)| n.code.as_str() == "FLD")
        .map(|(id, _)| id.as_uuid())
        .expect("the sample's location +FLD");
    let part = |number: &str| {
        q.parts()
            .iter()
            .find(|(_, c)| c.item.part_number.as_str() == number)
            .map(|(id, _)| id.as_uuid())
            .unwrap_or_else(|| panic!("the sample's part {number}"))
    };
    let enclosure = part("EW-GEN-ENC-1");
    let parts: Vec<Uuid> = PARTS.iter().map(|n| part(n)).collect();
    // A list form the project holds: the parts list's.
    let form = q
        .report_blocks()
        .iter()
        .find(|(_, b)| matches!(b.source, ReportSource::Kind(ReportKind::PartsSummary)))
        .map(|(_, b)| b.form)
        .expect("the sample's parts list");
    drop(q);
    let before: BTreeSet<Id<MountingSurface>> = s.query().table::<MountingSurface>().ids().collect();
    common::run_cmd(
        &mut s,
        "layout.enclosure.create_from_part",
        json!({"location": field.to_string(), "part": enclosure.to_string()}),
    );
    let surface = s
        .query()
        .table::<MountingSurface>()
        .ids()
        .find(|id| !before.contains(id))
        .expect("the new plate");
    common::run_cmd(
        &mut s,
        "page.create",
        json!({"number": 920, "kind": "panel_layout", "title": "Field cabinet"}),
    );
    let page = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "920")
        .expect("page 920")
        .id;
    common::run_cmd(
        &mut s,
        "page.surface_view.set",
        json!({"page": page.to_string(), "surface": surface.as_uuid().to_string(), "datum_at": [40_000_000, 160_000_000],
            "scale": {"one_to": 10}, "labels": "tags_or_numbers"}),
    );
    for i in 0..DEVICES {
        let receipt = common::run_cmd(
            &mut s,
            "device.create",
            json!({"letter": "K", "counter": 500 + i, "parts": [{"part": {"copy": parts[i % PARTS.len()].to_string()}, "qty": "1"}],
                "structure": {"location": field.to_string()}}),
        );
        let device = receipt.outcome.primary.expect("the new device").1.as_uuid();
        // Ten to a row, 50 mm apart, rows 100 mm apart.
        let (u, v) = ((i % 10) as i64 * 50_000_000, (i / 10) as i64 * 100_000_000);
        common::run_cmd(
            &mut s,
            "layout.place",
            json!({"device": device.to_string(), "carrier": {"surface": surface.as_uuid().to_string()},
                "pose": {"t": [u, v, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        );
    }
    let columns: Vec<_> = LEGEND_COLUMNS
        .iter()
        .map(|(key, weight)| json!({"key": key, "weight": weight}))
        .collect();
    let receipt = common::run_cmd(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "enclosure_legend"}, "form": form.as_uuid().to_string(),
            "layout_page": page.to_string(), "columns": columns,
            "title": {"en-GB": "Legend of layout {layout}", "nl-NL": "Legenda bij layout {layout}"}}),
    );
    let block = receipt.outcome.primary.expect("the legend").1.cast();
    (s, page, surface, block)
}

/// The placements a legend block's rows list.
fn listed(s: &Session, block: Id<ReportBlock>) -> BTreeSet<Id<Placement>> {
    let placements = s.query().table::<Placement>().ids().collect::<BTreeSet<_>>();
    ew_reports::rows_of(&s.query(), block)
        .rows
        .iter()
        .flat_map(|r| r.sources.iter())
        .map(|a| Id::<Placement>::from_uuid(a.as_uuid()))
        .filter(|p| placements.contains(p))
        .collect()
}

/// The placements on `surface`.
fn on(s: &Session, surface: Id<MountingSurface>) -> BTreeSet<Id<Placement>> {
    s.query()
        .table::<Placement>()
        .iter()
        .filter(|(_, p)| p.carrier == CarrierRef::Surface(surface))
        .map(|(id, _)| id)
        .collect()
}

/// The row IDs of a block, in order.
fn row_ids(s: &Session, block: Id<ReportBlock>) -> Vec<String> {
    ew_reports::rows_of(&s.query(), block)
        .rows
        .iter()
        .map(|r| r.id.to_string())
        .collect()
}

/// A legend box of a project made by 0.3.0, moved to its own page as Move the legend to its own page does it (the
/// list made for its layout page, the box deleted), lists every row the box listed, in its order.
#[test]
fn a_legend_box_moved_to_its_own_page_keeps_every_row() {
    let mut s = common::sample_with_legend_box();
    let q = s.query();
    let (boxed, page, form) = q
        .report_blocks()
        .iter()
        .find_map(|(id, b)| Some((id, b.embedded.as_ref()?.page, b.form)))
        .expect("the legend box on 902");
    drop(q);
    let before = row_ids(&s, boxed);
    assert!(before.len() > 20, "{before:?}");
    let columns: Vec<_> = LEGEND_COLUMNS
        .iter()
        .map(|(key, weight)| json!({"key": key, "weight": weight}))
        .collect();
    let receipt = common::run_cmd(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "enclosure_legend"}, "form": form.as_uuid().to_string(),
            "layout_page": page.as_uuid().to_string(), "columns": columns}),
    );
    let list: Id<ReportBlock> = receipt.outcome.primary.expect("the list").1.cast();
    common::run_cmd(
        &mut s,
        "edit.delete",
        json!({"targets": [{"kind": "report_block", "id": boxed.as_uuid().to_string()}]}),
    );
    assert_eq!(row_ids(&s, list), before);
    let order = s.query().get::<PageOrder>(());
    assert_eq!(
        order.by_folio("902.1").map(|e| e.page),
        Some(PageRef::Generated {
            block: list,
            index: 0
        })
    );
}

#[test]
fn a_legend_page_lists_the_rows_of_its_layout_and_no_other() {
    let (s, _, surface, block) = cabinet();
    let mine = on(&s, surface);
    assert_eq!(mine.len(), DEVICES);
    assert_eq!(listed(&s, block), mine, "the legend lists its plate's devices");
    // The sample's own legend of 902 lists none of them.
    let q = s.query();
    let p902 = q
        .pages()
        .iter()
        .find(|(_, p)| p.folio.number == 902)
        .map(|(id, _)| id)
        .expect("902");
    let theirs = q
        .report_blocks()
        .iter()
        .find(|(_, b)| b.layout_page == Some(p902) || b.embedded.as_ref().is_some_and(|e| e.page == p902))
        .map(|(id, _)| id)
        .expect("902's legend");
    drop(q);
    let other = listed(&s, theirs);
    assert!(!other.is_empty());
    assert!(
        other.is_disjoint(&mine),
        "902's legend lists none of 920's devices"
    );
}

#[test]
fn a_legend_of_sixty_devices_runs_on_over_its_pages_with_every_text_whole() {
    let (s, page, _, block) = cabinet();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let pages: Vec<(PageRef, String)> = order
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Generated { block: b, .. } if b == block))
        .map(|e| (e.page, e.folio.clone()))
        .collect();
    // Sixty devices, the enclosure and its plate: more rows than two A4 list pages hold.
    assert!(pages.len() >= 3, "{pages:?}");
    // The run follows the layout page, 920.1 on, each page right after the one before.
    let at = |p: PageRef| order.position(p).expect("in the page order");
    let layout = order.position(PageRef::Drawn(Id::from_uuid(page))).expect("920");
    for (k, (p, folio)) in pages.iter().enumerate() {
        assert_eq!(*folio, format!("920.{}", k + 1));
        assert_eq!(at(*p), layout + 1 + k);
    }
    assert_eq!(
        q.get::<ew_derive::PageTitle>(pages[0].0).as_ref(),
        &ew_derive::ew_mltext::MlText::from_langs([
            ("en-GB".parse().unwrap(), "Legend of layout 920"),
            ("nl-NL".parse().unwrap(), "Legenda bij layout 920"),
        ])
    );
    // Every text the pages print is whole (none cut with an ellipsis), and every device's tag prints once.
    let mut printed = Vec::new();
    for (p, _) in &pages {
        for item in &page_layer(&q, *p, Layer::Tables).list.items {
            if let ItemKind::Glyphs(run) = &item.kind {
                printed.push(run.text.clone());
            }
        }
    }
    let cut: Vec<&String> = printed.iter().filter(|t| t.ends_with('\u{2026}')).collect();
    assert!(cut.is_empty(), "cut texts: {cut:?}");
    // A device drawn on no schematic page prints its tag with the page part unknown ("-?K500").
    for i in 0..DEVICES {
        let tag = format!("K{}", 500 + i);
        assert_eq!(
            printed
                .iter()
                .filter(|t| t.ends_with(&format!("-{tag}")) || t.ends_with(&format!("?{tag}")))
                .count(),
            1,
            "{tag}"
        );
    }
    drop(q);
    // The devices' descriptions are among the texts, whole.
    for number in PARTS {
        let q = s.query();
        let description = q
            .parts()
            .iter()
            .find(|(_, c)| c.item.part_number.as_str() == number)
            .map(|(_, c)| ew_derive::text_in(&c.item.description, None).to_owned())
            .expect("the part");
        assert!(printed.contains(&description), "{description}");
    }
}

/// A legend list is named by its printed folio, and its Properties name its layout page, not the first folio and
/// section that nothing reads (FT-2032).
#[test]
fn a_legend_list_is_named_by_its_printed_folio_and_shows_its_layout_page() {
    let s = common::sample();
    let q = s.query();
    let (block, layout) = q
        .report_blocks()
        .iter()
        .find_map(|(id, b)| b.layout_page.map(|p| (id, p)))
        .expect("the sample's legend list");
    let order = q.get::<PageOrder>(());
    let printed = order
        .entry(PageRef::Generated { block, index: 0 })
        .map(|e| e.folio.clone())
        .expect("the legend's first page");
    let layout_folio = order
        .entry(PageRef::Drawn(layout))
        .map(|e| e.folio.clone())
        .expect("the layout page");
    assert_ne!(printed, layout_folio, "the legend's own folio, not its layout's");
    let (found, _) = ew_api::output::find_block(&q, &printed).expect("a printed folio names its block");
    assert_eq!(found, block);
    drop(q);
    let sheet = s.property_sheet(&[block.as_uuid()]);
    let keys: Vec<&str> = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .map(|r| r.key.as_str())
        .collect();
    let layout_row = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .find(|r| r.key == "report_block/layout_page")
        .unwrap_or_else(|| panic!("no layout page row: {keys:?}"));
    match &layout_row.value {
        ew_api::output::RowValue::One { text, .. } => {
            assert!(
                text.starts_with(&layout_folio),
                "the layout page by its folio: {text}"
            );
        }
        other => panic!("{other:?}"),
    }
    for unread in ["report_block/first_folio", "report_block/section"] {
        assert!(!keys.contains(&unread), "{unread}: {keys:?}");
    }
}
