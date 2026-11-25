// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What an older build (0.3.0) leaves in a project of this one, which this build still opens (ADR-0011 amendment 4):
//! 0.3.0 keeps a member it does not know in `extra` and does not apply its link, so a target it deletes can stay named.

use crate::common;

use ew_ids::{Id, Uuid};
use ew_derive::{PageOrder, PageRef};
use ew_schema::{Record, ReportBlock};

/// The sample's legend list (`layout_page`, on 902) after 0.3.0 deleted its layout page and kept the block: the project
/// opens, and the list has no pages.
#[test]
fn a_legend_list_whose_layout_page_an_older_build_deleted_opens_and_lists_no_pages() {
    let (project, mut records, lib) = common::sample_records();
    let mut legend: Option<Id<ReportBlock>> = None;
    for r in &mut records {
        if let Record::ReportBlock(b) = r
            && b.layout_page.is_some()
        {
            b.layout_page = Some(Id::from_uuid(Uuid::from_u128(0x0d1e_7e0d)));
            legend = Some(b.id);
        }
    }
    let legend = legend.expect("the sample's legend list");
    let s = common::session_of(project, records, lib);
    let order = s.query().get::<PageOrder>(());
    assert!(
        !order
            .entries
            .iter()
            .any(|e| matches!(e.page, PageRef::Generated { block, .. } if block == legend)),
        "a legend list without its layout page has no pages"
    );
}
