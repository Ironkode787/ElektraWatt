// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A location does not crowd the drawing (a longer persona round of the second 0.3.0 candidate: a card given the
//! location +CAB1 printed "+CAB1-102A1-A1" into the next card's tag): on a page without a location of its own whose
//! devices all have one location, the tags leave it out; a device of another location brings it back.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The texts the page on show prints.
fn texts(shell: &ShellModel) -> Vec<String> {
    use ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(group) => walk(&group.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&shell.canvas().unwrap().page().list.items, &mut out);
    out
}

/// A miniature circuit breaker placed at grid index `k` of the page shown; its representation.
fn mcb_at(s: &mut ShellModel, k: (i64, i64)) -> String {
    let before: Vec<_> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id)
        .collect();
    done(s, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
    let _ = s.dispatch("ui.dismiss", &json!({}));
    let _ = s.dispatch("ui.dismiss", &json!({}));
    s.canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| !before.contains(&r.id))
        .expect("a new symbol")
        .id
        .to_string()
}

/// The location `code` given to each of `reps`' devices, as Properties gives it (its Effective location row).
fn locate(s: &mut ShellModel, reps: &[String], code: &str) {
    for rep in reps {
        done(s, "select.set", json!({"ids": [rep]}));
        done(
            s,
            "props.set",
            json!({"row": "device/derived.structure.location", "value": code}),
        );
    }
}

#[test]
fn a_page_whose_devices_share_a_location_prints_their_tags_without_it() {
    let tmp = Tmp::new("page-location");
    let mut s = common::new_project(&tmp);
    let a = mcb_at(&mut s, (20, 20));
    let b = mcb_at(&mut s, (40, 20));
    for code in ["CAB1", "CAB2"] {
        done(
            &mut s,
            "structure.new",
            json!({"kind": "location", "designation": code}),
        );
    }
    locate(&mut s, &[a, b.clone()], "+CAB1");
    // The title block of the frame names the page's location; the tags leave it out.
    let shown = texts(&s);
    assert!(
        !shown.iter().any(|t| t.contains("+CAB1-")),
        "the shared location is left out: {shown:?}"
    );
    // A device of another location: the page's devices no longer share one, so each tag says its own.
    locate(&mut s, &[b], "+CAB2");
    let shown = texts(&s);
    assert!(
        shown.iter().any(|t| t.contains("+CAB1-")) && shown.iter().any(|t| t.contains("+CAB2-")),
        "{shown:?}"
    );
    s.shutdown();
}
