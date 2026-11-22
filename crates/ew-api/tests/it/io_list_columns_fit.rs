// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1923: an I/O list block on the form of the sample's PLC overview prints every CPU tag and part number whole: a
//! column whose text would be cut takes room from the columns that print smaller than their room, down to the smallest
//! size (doc 09 §7 item 5).

use crate::common;

use ew_api::AnyCommand;
use ew_api::output::page_layer;
use ew_derive::PageOrder;
use ew_dl::Layer;
use ew_scene::page::PageFinding;
use ew_scene::table::TableFinding;
use ew_schema::{ReportKind, ReportSource};
use serde_json::json;

/// The I/O list's columns of the CPU tag and of the part number (`ew_reports`' `io_list` columns).
const CPU_AND_PART: [usize; 2] = [1, 6];

#[test]
fn an_io_list_on_the_plc_overviews_form_cuts_no_cpu_tag_or_part_number() {
    let mut s = common::sample();
    let form = {
        let q = s.query();
        q.report_blocks()
            .iter()
            .find(|(_, b)| b.source == ReportSource::Kind(ReportKind::PlcOverview))
            .map(|(_, b)| b.form)
            .expect("the PLC overview's block")
    };
    let cmd = AnyCommand {
        name: "report_block.create".into(),
        params: json!({"source": {"kind": "io_list"}, "form": form, "first_folio": 950, "title": "I/O List {n}"}),
    };
    let preview = s.preview(&cmd).expect("the block");
    s.commit(preview).expect("committed");
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let pages: Vec<_> = order
        .entries
        .iter()
        .filter(|e| e.folio == "950" || e.folio.starts_with("950."))
        .collect();
    assert!(!pages.is_empty());
    for e in pages {
        let layer = page_layer(&q, e.page, Layer::Tables);
        let cut: Vec<&PageFinding> = layer
            .findings
            .iter()
            .filter(|f| {
                matches!(f, PageFinding::Table(TableFinding::Overflow { column, .. })
                    if CPU_AND_PART.contains(column))
            })
            .collect();
        assert!(cut.is_empty(), "folio {}: {cut:?}", e.folio);
    }
}
