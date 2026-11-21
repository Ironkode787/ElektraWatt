// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1768: the datum Enclosure… and the Surface row centre a layout with stands the enclosure in the middle of the
//! frame's drawing area on a frame whose page-to-sheet map has an offset (the starter frames' binding margin). The
//! drawing area is in page coordinates already; mapping it through the sheet map once more moved the plate off
//! centre by about the offset.

use crate::common;

use ew_api::AnyCommand;
use ew_api::output::PageFrameOf;
use ew_derive::PageRef;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_schema::{MountingSurface, Page};
use serde_json::json;

const MM: i64 = 1_000_000;

#[test]
fn a_new_layout_stands_in_the_middle_of_a_frame_with_an_offset() {
    let mut s = common::sample();
    let p902 = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "902")
        .expect("902")
        .id;
    let plate = s
        .query()
        .table::<MountingSurface>()
        .ids()
        .next()
        .expect("the sample's plate")
        .as_uuid();
    let before: Vec<Uuid> = s.page_rows().into_iter().map(|r| r.id).collect();
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params: json!({"after": p902.to_string(), "kind": "panel_layout"}),
    })
    .expect("a new panel page");
    let row = s
        .page_rows()
        .into_iter()
        .find(|r| !before.contains(&r.id))
        .expect("the new page");

    // The new page's frame: the starter A4 frame, whose map puts the page 9.79 mm in on the sheet, and its grid (a
    // header band of 15.875/3 mm, 17 columns of 15.875 mm, 8 rows of 63.5/3 mm).
    let q = s.query();
    let frame = q.get::<PageFrameOf>(PageRef::Drawn(Id::<Page>::from_uuid(row.id)));
    let form = &frame.as_ref().as_ref().expect("the page's frame").form;
    let map = form.page_to_sheet.as_ref().expect("a page-to-sheet map");
    assert_eq!(map.offset, Pt2::new(Nm(9_790_000), Nm(9_790_000)));
    let grid = form.grid.as_ref().expect("a grid");
    assert_eq!(
        (grid.columns.count, grid.rows.count),
        (17, 8),
        "the starter frame's grid"
    );
    drop(q);
    // The drawing area in page coordinates, each edge rounded once: x from 5.291667 to 5.291667 + 17 × 15.875 =
    // 275.166667 mm, y from 5.291667 to 5.291667 + 8 × 21.166667 = 174.625 mm; its middle in whole nanometres.
    let middle = Pt2::new(
        Nm((5_291_667 + 275_166_667) / 2),
        Nm((5_291_667 + 174_625_000) / 2),
    );

    let (x, y) = s.centred_surface_datum(row.id, plate).expect("a centred datum");
    s.execute(&AnyCommand {
        name: "page.surface_view.set".into(),
        params: json!({"page": row.id.to_string(), "surface": plate.to_string(), "datum_at": [x, y],
            "scale": {"one_to": 5}}),
    })
    .expect("the view");
    // The sample's 600 × 800 mm enclosure runs (-40, -40)–(560, 760) mm in the plate's frame; drawn by the page's
    // projection its middle is the drawing area's.
    let projection = s.panel_tables(&row.folio).expect("the view").projection;
    let enclosure = projection
        .rect(Rect::from_points(
            Pt2::new(Nm(-40 * MM), Nm(-40 * MM)),
            Pt2::new(Nm(560 * MM), Nm(760 * MM)),
        ))
        .expect("in range");
    let centre = Pt2::new(
        Nm((enclosure.min().x.0 + enclosure.max().x.0) / 2),
        Nm((enclosure.min().y.0 + enclosure.max().y.0) / 2),
    );
    let off = ((centre.x.0 - middle.x.0).abs(), (centre.y.0 - middle.y.0).abs());
    assert!(
        off.0 <= 1 && off.1 <= 1,
        "the enclosure's middle {centre:?} is the drawing area's {middle:?}"
    );
}
