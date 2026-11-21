// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A legend list's pages follow its layout page (doc 09 §3.2): N.1, N.2 … after N, the first run of sub-pages that no
//! drawn page of the section holds, never split by one.

use ew_reports::paginate::legend_first_sub;

fn held(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|l| (*l).to_owned()).collect()
}

#[test]
fn a_legend_follows_its_layout_page_at_the_first_free_run_of_sub_pages() {
    // 902.1 and 902.2 after 902.
    assert_eq!(legend_first_sub(902, "902", &held(&["901", "902", "903"]), 2), 1);
    // A drawn 902.1 moves the run on to 902.2.
    assert_eq!(legend_first_sub(902, "902", &held(&["902", "902.1"]), 2), 2);
    // A drawn 902.2 would split a run of two from 902.1: it starts at 902.3.
    assert_eq!(legend_first_sub(902, "902", &held(&["902", "902.2"]), 2), 3);
    // One page fits before the drawn 902.2.
    assert_eq!(legend_first_sub(902, "902", &held(&["902", "902.2"]), 1), 1);
    // After a layout page printed 902.3 the run starts at 902.4.
    assert_eq!(legend_first_sub(902, "902.3", &held(&["902.3"]), 1), 4);
}
