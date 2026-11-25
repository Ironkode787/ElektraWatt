// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A list never ends on a page of one or two rows (doc 09 §3.2): on a copy of the sample, a list of data rows given
//! rows of its own until one row would stand alone on its last page prints three there, the page before ending two rows
//! earlier, and as many pages as before.

use crate::common;

use ew_api::output::{PageFrameOf, PageTableOf};
use ew_derive::{PageOrder, PageRef};
use ew_ids::Id;
use ew_reports::{BlockRows, RowKind};
use serde_json::{Value, json};

#[test]
fn a_lists_last_page_takes_rows_from_the_page_before() {
    let mut s = common::sample();
    let q = s.query();
    // The sample's first list (by folio) whose rows are all data rows in one run of pages, a table of contents aside.
    let mut lists: Vec<_> = q
        .report_blocks()
        .iter()
        .filter(|(_, b)| b.embedded.is_none() && !ew_reports::paginate::is_toc(b))
        .map(|(id, b)| (b.first_folio, id, b.clone()))
        .collect();
    lists.sort_by_key(|(folio, ..)| *folio);
    let (id, block) = lists
        .into_iter()
        .find(|(_, id, _)| {
            let set = q.get::<BlockRows>(*id);
            set.groups.is_empty() && !set.rows.is_empty() && set.rows.iter().all(|r| r.kind == RowKind::Data)
        })
        .map(|(_, id, b)| (id, b))
        .expect("a list of data rows");
    let set = q.get::<BlockRows>(id);
    let rows = set.rows.len();
    // The rows one of its pages holds: its frame's list area.
    let first = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| matches!(e.page, PageRef::Generated { block: b, .. } if b == id))
        .map(|e| e.page)
        .expect("its first page");
    let frame = q.get::<PageFrameOf>(first).as_ref().clone().expect("a frame");
    let per_page = usize::try_from(
        frame
            .form
            .list_area(frame.title_block.as_deref(), false, false)
            .rows,
    )
    .expect("rows");
    let first_key = set.columns[0].key.clone();
    drop(q);
    // Rows of its own until the last page would hold one row, after at least one full page.
    let mut add = (per_page + 1 - rows % per_page) % per_page;
    if rows + add <= per_page {
        add += per_page;
    }
    let mut own: Vec<Value> = block
        .added_rows
        .iter()
        .map(|r| serde_json::to_value(r).expect("a row"))
        .collect();
    own.extend((0..add).map(|i| {
        let mut cells = serde_json::Map::new();
        cells.insert(first_key.clone(), Value::String(format!("~{i:03}")));
        json!({ "cells": cells })
    }));
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "report_block", "targets": [id.as_uuid().to_string()], "path": "added_rows", "value": own}),
    );
    let q = s.query();
    let set = q.get::<BlockRows>(Id::from_uuid(id.as_uuid()));
    let total = set.rows.len();
    assert!(
        set.rows.iter().all(|r| r.kind == RowKind::Data) && total % per_page == 1,
        "{total} rows of {per_page}"
    );
    let printed: Vec<usize> = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Generated { block: b, .. } if b == id))
        .map(|e| {
            q.get::<PageTableOf>(e.page)
                .as_ref()
                .as_ref()
                .map_or(0, |t| t.rows.len())
        })
        .collect();
    let pages = total.div_ceil(per_page);
    let mut want = vec![per_page; pages];
    want[pages - 2] = per_page - 2;
    want[pages - 1] = 3;
    assert_eq!(printed, want, "{total} rows of {per_page}");
}
