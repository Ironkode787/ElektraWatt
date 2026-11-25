// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw on a page's default puts each card of a station on a page of its own, the station's head on the first
//! (doc 10 §2.3): the pages are made after the page the view stands on, in its section, titled from their cards,
//! and one Undo takes the pages and the drawings back.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn each_card_goes_on_a_page_of_its_own_the_head_on_the_first_in_one_undo_step() {
    let tmp = Tmp::new("station-draw-own-pages");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    let pages_before: Vec<String> = s.page_rows().iter().map(|r| r.folio.clone()).collect();
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DO-1"}));
    let station = s.station_tab().and_then(|t| t.station).expect("the station");
    done(&mut s, "plc.rack_next", json!({"step": "draw"}));
    let new: Vec<_> = s
        .page_rows()
        .iter()
        .filter(|r| !pages_before.contains(&r.folio))
        .map(|r| (r.folio.clone(), r.title.clone(), r.section.clone()))
        .collect();
    let section_104 = s
        .page_rows()
        .iter()
        .find(|r| r.folio == "104")
        .and_then(|r| r.section.clone());
    assert_eq!(new.len(), 2, "a page for each card: {new:?}");
    assert!(
        new.iter().all(|(_, _, section)| *section == section_104),
        "made in the section of the page the view stands on: {new:?}"
    );
    assert_eq!(
        new.iter().map(|(_, t, _)| t.as_str()).collect::<Vec<_>>(),
        ["PLC 8 DI", "PLC 8 DO"],
        "titled from their cards"
    );
    // The head and the first card on the first page, the second card on the second.
    let view = s.rack_view(station).expect("the station's rack");
    let on: Vec<Option<String>> = std::iter::once(&view.head)
        .chain(&view.rows)
        .filter(|r| r.module.is_some())
        .map(|r| r.page.clone())
        .collect();
    assert_eq!(
        on,
        [
            Some(new[0].0.clone()),
            Some(new[0].0.clone()),
            Some(new[1].0.clone())
        ],
        "the head and its first card on the first page, the second card on the second"
    );
    // One Undo takes it all back: the pages and the drawings.
    done(&mut s, "edit.undo", json!({}));
    let pages_after: Vec<String> = s.page_rows().iter().map(|r| r.folio.clone()).collect();
    assert_eq!(pages_after, pages_before, "the pages go with the drawings");
    let view = s.rack_view(station).expect("the station's rack");
    assert!(
        std::iter::once(&view.head)
            .chain(&view.rows)
            .all(|r| r.page.is_none()),
        "nothing of the station stays drawn"
    );
}
