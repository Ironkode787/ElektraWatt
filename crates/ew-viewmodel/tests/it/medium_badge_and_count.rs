// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1761: the Fluid navigator's badge of a medium counted its lines ("1 lines") beside the medium table's Count of
//! its consumers. On a copy of the sample, a new medium defined at a tube of its pneumatics: the badge names the
//! consumers, as many as the table's Count.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::navigators::{NavKind, cell_text};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_mediums_badge_counts_what_the_medium_table_counts() {
    let tmp = Tmp::new("medium-badge");
    let mut s = tmp.sample_in(Mode::Headless, true, "201");
    done(&mut s, "fluid.medium_new", json!({"name": "Air", "p_nom": "6"}));
    done(&mut s, "navigator.show", json!({"kind": "tubes", "on": true}));
    let line = {
        let nav = s.nav(NavKind::Tubes).expect("the tube table");
        let t = nav.table().expect("its rows");
        t.nodes
            .iter()
            .find_map(|n| n.records.first().copied())
            .expect("a tube of the sample")
    };
    done(&mut s, "navigator.show", json!({"kind": "media", "on": true}));
    let medium = {
        let nav = s.nav(NavKind::Media).expect("the medium table");
        let t = nav.table().expect("its rows");
        t.nodes
            .iter()
            .find(|n| n.label == "Air")
            .and_then(|n| n.records.first().copied())
            .expect("the medium")
    };
    done(
        &mut s,
        "fluid.set_medium",
        json!({"line": line.to_string(), "medium": medium.to_string()}),
    );
    done(&mut s, "navigator.show", json!({"kind": "media", "on": true}));
    let count = {
        let nav = s.nav(NavKind::Media).expect("the medium table");
        let t = nav.table().expect("its rows");
        let col = t
            .columns
            .iter()
            .position(|c| c.key == "consumer_count")
            .expect("the Count column");
        let node = t.nodes.iter().find(|n| n.label == "Air").expect("the medium's row");
        cell_text(t, node, col)
    };
    done(&mut s, "navigator.show", json!({"kind": "fluid", "on": true}));
    let row = s
        .shown_rows(NavKind::Fluid)
        .into_iter()
        .find(|r| r.label == "Air")
        .expect("the medium's row in the Fluid navigator");
    let want = if count == "1" {
        "1 consumer".to_owned()
    } else {
        format!("{count} consumers")
    };
    assert_eq!(row.badges, [want]);
}
