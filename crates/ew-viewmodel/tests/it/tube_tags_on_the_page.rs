// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! rc.5 persona run: with Fluid settings' "Tag tubes by page and counter" on, the tubes carried their tags in the tube
//! table alone; the setting promises them on the page too. Each tube prints its tag beside it; without the setting,
//! none.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// The texts the page shown prints.
fn page_texts(s: &ShellModel) -> Vec<String> {
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(g) => walk(&g.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&s.canvas().expect("a page").page().list.items, &mut out);
    out
}

/// A click at the page point of grid index `k`.
fn click(s: &mut ShellModel, k: (i64, i64), alt: bool) {
    let sn = &s.canvas().unwrap().page().snapper;
    let q = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
    for a in ["canvas.pointer", "canvas.press", "canvas.release"] {
        done(s, a, at(q, json!({"alt": alt})));
    }
}

#[test]
fn a_tube_prints_its_tag_beside_it_when_tubes_are_tagged() {
    let tmp = Tmp::new("tube-tags");
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let dir = tmp.0.join("new.ew");
    done(
        &mut s,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "201", "type": "fluid", "title": "Tubes"}),
    );
    done(&mut s, "page.goto", json!({"folio": "201"}));
    for k in [(20, 20), (40, 30)] {
        done(&mut s, "place.pick", json!({"part": "EW-GEN-FCV-1", "template": 0}));
        click(&mut s, k, true);
        done(&mut s, "tool.select", json!({}));
    }
    // A tube from the first flow control's port 2 to the second's port 1.
    let ports: Vec<(i64, i64)> = {
        let t = tables(&s);
        let mut reps: Vec<_> = t.reps.iter().map(|r| r.id).collect();
        reps.sort_by_key(|id| t.reps.iter().find(|r| r.id == *id).map(|r| r.pos));
        let port = |rep, label: &str| {
            t.cps
                .iter()
                .find(|c| c.rep == rep && c.label.ends_with(label))
                .map(|c| c.at)
                .expect("the port")
        };
        vec![port(reps[0], ":2"), port(reps[1], ":1")]
    };
    let tag = |s: &ShellModel| page_texts(s).into_iter().filter(|t| t == "201.1").count();
    done(&mut s, "fluid.settings", json!({"line_tags": "true"}));
    done(&mut s, "tool.fluid_line", json!({}));
    for p in &ports {
        let q = s.canvas().unwrap().page().snapper.page_to_sheet(*p);
        for a in ["canvas.pointer", "canvas.press", "canvas.release"] {
            done(&mut s, a, at(q, json!({})));
        }
    }
    done(&mut s, "tool.select", json!({}));
    assert_eq!(tables(&s).connections.len(), 1, "the tube");
    assert_eq!(tag(&s), 1, "the tag beside the tube: {:?}", page_texts(&s));
    // Without the setting, no tag.
    done(&mut s, "fluid.settings", json!({"line_tags": "false"}));
    assert_eq!(tag(&s), 0, "no tag without the setting");
}
