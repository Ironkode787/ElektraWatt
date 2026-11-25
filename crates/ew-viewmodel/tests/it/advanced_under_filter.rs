// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! "Show advanced (n)" counts the advanced rows the Properties filter leaves, and goes when none does
//! ", read "Hide advanced (2)" and saw no row come). The sample's project rows.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::properties::advanced_count;
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_advanced_link_counts_what_the_filter_leaves() {
    let tmp = Tmp::new("advanced-under-filter");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    let sheet = s.properties().sheet().expect("the project's sheet");
    assert!(advanced_count(sheet, "") > 0, "the project has advanced rows");
    assert_eq!(advanced_count(sheet, "wire"), 0, "none of them matches \"wire\"");
    done(&mut s, "props.filter", json!({"text": "wire"}));
    assert!(
        !s.describe().lines().any(|l| l.starts_with("  advanced ")),
        "no advanced line under the filter"
    );
}
