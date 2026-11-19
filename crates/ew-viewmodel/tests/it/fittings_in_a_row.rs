// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! rc.5 persona run: four one-way flow controls placed in a row on a fluid page, each beside the last, were joined by
//! tubes, port 1 of each to port 2 of the one before it, which he did not want. Two fittings set side by side are a
//! row of branches: placing one never joins it to another; a fitting placed facing a cylinder's port still is.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::ew_api::output::TableRep;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// A part placed with its anchor at grid index `k` by one click, joined to what it plainly faces.
fn place(s: &mut ShellModel, part: &str, k: (i64, i64)) -> TableRep {
    let before = tables(s).reps.clone();
    done(s, "place.pick", json!({"part": part, "template": 0}));
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
    for a in ["canvas.pointer", "canvas.press", "canvas.release"] {
        done(s, a, at(p, json!({})));
    }
    done(s, "tool.select", json!({}));
    tables(s)
        .reps
        .iter()
        .find(|r| before.iter().all(|b| b.id != r.id))
        .cloned()
        .expect("the new symbol")
}

#[test]
fn flow_controls_set_side_by_side_are_not_joined_and_one_facing_a_cylinder_is() {
    let tmp = Tmp::new("fittings-row");
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
        json!({"folio": "201", "type": "fluid", "title": "Fluid"}),
    );
    done(&mut s, "page.goto", json!({"folio": "201"}));
    // The persona's row: four flow controls, eight and fourteen grid steps apart, on one line.
    let row: Vec<TableRep> = [20, 28, 42, 50]
        .into_iter()
        .map(|x| place(&mut s, "EW-GEN-FCV-1", (x, 30)))
        .collect();
    // How far below its anchor a flow control's ports stand, in grid steps.
    let below = {
        let port = tables(&s).cps.iter().find(|c| c.rep == row[0].id).map(|c| c.at.1);
        s.canvas().unwrap().page().snapper.grid_index_of(port.expect("a port")) - 30
    };
    assert_eq!(tables(&s).reps.len(), 4);
    assert!(
        tables(&s).connections.is_empty(),
        "no tube between two flow controls: {:?}",
        tables(&s).connections
    );
    // A cylinder, then a flow control placed left of it with its port 2 facing the cylinder's port: joined.
    let cylinder = place(&mut s, "EW-GEN-CYL-1", (80, 50));
    let port = tables(&s)
        .cps
        .iter()
        .filter(|c| c.rep == cylinder.id)
        .min_by_key(|c| c.at.1)
        .cloned()
        .expect("the cylinder's upper port");
    let line = s.canvas().unwrap().page().snapper.grid_index_of(port.at.1);
    let fitting = place(&mut s, "EW-GEN-FCV-1", (60, line - below));
    let t = tables(&s);
    let joined = t.connections.iter().any(|c| {
        let ends = [&c.a, &c.b].map(|e| match e {
            ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx } => t
                .cps
                .iter()
                .find(|p| p.function == *function && p.idx == *idx)
                .map(|p| p.rep),
            ew_viewmodel::ew_api::output::TableEnd::Junction { .. } => None,
        });
        ends.contains(&Some(cylinder.id)) && ends.contains(&Some(fitting.id))
    });
    assert!(joined, "the flow control facing the cylinder's port is joined to it");
}
