// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 37 window check (W37 q01): in a new project, two make contacts of -101K1 placed 9 grid steps apart on
//! folio 101 print with the project's text styles; the first contact's back-reference "(101-D6)" must neither run
//! into the second contact's tag nor overlap its own CP designation "14" (the drop below the tag meets it, so the
//! text drops below that too). Checked on the page display list the canvas paints.

mod common;

use common::{Tmp, done};
use ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A layout box in page nm: (min x, min y, max x, max y).
type Box4 = (i64, i64, i64, i64);

/// Every glyph run of the list (groups opened) with its layout box.
fn runs(items: &[Item], out: &mut Vec<(String, Box4)>) {
    for i in items {
        match &i.kind {
            ItemKind::Glyphs(run) => {
                let q = run.layout_quad();
                let xs = q.iter().map(|p| p.x.0);
                let ys = q.iter().map(|p| p.y.0);
                let b = (
                    xs.clone().min().unwrap(),
                    ys.clone().min().unwrap(),
                    xs.max().unwrap(),
                    ys.max().unwrap(),
                );
                out.push((run.text.clone(), b));
            }
            ItemKind::Group(g) => runs(&g.items, out),
            _ => {}
        }
    }
}

fn overlap(a: Box4, b: Box4) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

#[test]
fn a_back_reference_clears_the_next_contacts_tag_and_its_own_designation() {
    let tmp = Tmp::new("c37-backref");
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 101, "title": "Control circuit"}}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let place = |s: &mut ShellModel, pick: Value, x: f64, y: f64| {
        done(s, "tool.place", json!({}));
        done(s, "place.pick", pick);
        done(s, "cursor.set", json!({"x_mm": x, "y_mm": y}));
        done(s, "canvas.activate", json!({"alt": true}));
        let _ = s.dispatch("ui.dismiss", &json!({}));
        let _ = s.dispatch("ui.dismiss", &json!({}));
    };
    place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        100.0,
        80.0,
    );
    let contact = json!({"symbol": "ew.iec.contact-no@qet10", "device": "tag:-101K1"});
    place(&mut s, contact.clone(), 100.0, 150.0);
    place(&mut s, contact, 125.0, 150.0);
    let mut all = Vec::new();
    runs(&s.canvas().unwrap().page().list.items, &mut all);
    // The two contacts' texts (below 120 mm), left to right.
    let at = |text: &str| -> Vec<Box4> {
        let mut v: Vec<Box4> = all
            .iter()
            .filter(|(t, b)| t == text && b.1 > 120_000_000)
            .map(|(_, b)| *b)
            .collect();
        v.sort_unstable();
        v
    };
    let (backs, tags, ends) = (at("(101-D6)"), at("-101K1"), at("14"));
    assert_eq!((backs.len(), tags.len(), ends.len()), (2, 2, 2), "{all:?}");
    let (back, next_tag, own_end) = (backs[0], tags[1], ends[0]);
    let beside = back.1 < next_tag.3 && next_tag.1 < back.3;
    assert!(
        // `ew_scene::page::SLOT_TEXT_GAP`, 0.5 mm.
        !beside || next_tag.0 - back.2 >= 500_000 - 1_000,
        "{back:?} runs into {next_tag:?}"
    );
    assert!(!overlap(back, next_tag), "{back:?} overlaps {next_tag:?}");
    assert!(!overlap(back, own_end), "{back:?} overlaps its 14 {own_end:?}");
}

/// A new project's folio 101 in a headless shell.
fn new_folio(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 101, "title": "Valves"}}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    s
}

/// Cycle 37 visual play-test: two starter valves (EW-GEN-VLV-1) side by side at one column and a little more: M1's
/// "24 V" never overlaps or runs into M2's tag, and never lands on one of M1's own texts.
#[test]
fn a_valves_data_clears_the_next_valves_tag() {
    // The insertion points on the QET 10 px grid (2.645833 mm): 6, 7 and 8 steps apart.
    for steps in [6.0, 7.0, 8.0] {
        let tmp = Tmp::new("c37-valves");
        let mut s = new_folio(&tmp);
        for x in [100.0, 100.0 + steps * 2.645_833] {
            done(&mut s, "tool.place", json!({}));
            done(
                &mut s,
                "place.pick",
                json!({"part": "EW-GEN-VLV-1", "template": 0, "letter": "M"}),
            );
            done(&mut s, "cursor.set", json!({"x_mm": x, "y_mm": 80.0}));
            done(&mut s, "canvas.activate", json!({"alt": true}));
            let _ = s.dispatch("ui.dismiss", &json!({}));
            let _ = s.dispatch("ui.dismiss", &json!({}));
        }
        let mut all = Vec::new();
        runs(&s.canvas().unwrap().page().list.items, &mut all);
        let mut data: Vec<Box4> = all.iter().filter(|(t, _)| t == "24 V").map(|(_, b)| *b).collect();
        data.sort_unstable();
        let next = all
            .iter()
            .find(|(t, _)| t == "-101M2")
            .map(|(_, b)| *b)
            .unwrap_or_else(|| panic!("{steps}: no -101M2 in {all:?}"));
        assert_eq!(data.len(), 2, "{steps}: {all:?}");
        let first = data[0];
        let beside = first.1 < next.3 && next.1 < first.3;
        assert!(!overlap(first, next), "{steps}: {first:?} overlaps {next:?}");
        assert!(
            // `ew_scene::page::SLOT_TEXT_GAP`, 0.5 mm.
            !beside || next.0 - first.2 >= 500_000 - 1_000,
            "{steps}: {first:?} runs into {next:?}"
        );
        // M1's own texts: everything left of M2's tag's right end that is not the data itself or M2's.
        for (t, b) in &all {
            if t != "24 V" && t != "-101M2" && b.2 <= first.0 + 2_000_000 && b.0 >= first.0 - 15_000_000 {
                assert!(!overlap(first, *b), "{steps}: {first:?} overlaps {t} {b:?}");
            }
        }
    }
}
