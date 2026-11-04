// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sample project's catalogue parts in a new project (WP-S7.03 D3, SC-3, SC-5): with the installation's system
//! tier (`content/libraries`), every sample part with a function template is placed from the picker on the starter
//! symbol of its main function, and its device carries the part after saving and reopening; the place tool's picker
//! offers a sample part, and the Libraries navigator lists the library in the system tier.

use crate::common;

use std::path::{Path, PathBuf};

use common::{Tmp, at, click, done};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::ew_api::library::starter;
use ew_viewmodel::ew_api::output::{TableRep, Uuid};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::tools::PickerRow;
use serde_json::{Value, json};

const MULTI: &str = "multi_line";
const FLUID: &str = "fluid";
const OVERVIEW: &str = "overview";
const BUS: &str = "bus_topology";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The installation's library tiers: the content's system libraries, the rest in `root`.
fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: vec![repo().join("content/libraries")],
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// The sample parts with a function template, as `(part number, main FnDef key)`, from the shipped files.
fn templated_parts() -> Vec<(String, String)> {
    let dir = repo().join("content/libraries/ew-sample-parts.ewlib/parts");
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        for line in std::fs::read_to_string(e.unwrap().path()).unwrap().lines() {
            let v: Value = serde_json::from_str(line).unwrap();
            let Some(t) = v["function_template"].as_array() else {
                continue;
            };
            let main = t.iter().find(|t| t["main"] == true).unwrap_or(&t[0]);
            out.push((
                v["part_number"].as_str().unwrap().to_owned(),
                main["fndef"]["key"].as_str().unwrap().to_owned(),
            ));
        }
    }
    out.sort();
    out
}

/// A new project with one page of each type the parts use (101 multi-line, 102 fluid, 103 overview, 104 bus
/// topology) and the strip `-X`, its sessions reading `tiers`.
fn new_project(tmp: &Tmp) -> ShellModel {
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test").with_libraries(Some(tiers(&tmp.0)));
    let mut s = ShellModel::new(Mode::Headless, config);
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    for (folio, ty) in [("101", MULTI), ("102", FLUID), ("103", OVERVIEW), ("104", BUS)] {
        done(&mut s, "page.new", json!({"folio": folio, "type": ty, "title": ty}));
    }
    done(&mut s, "strip.new", json!({"letter": "X"}));
    s
}

/// The page type a function definition is drawn on.
fn page_of(fndef: &str) -> &'static str {
    match fndef {
        "ew.fn.overview" => OVERVIEW,
        "ew.fn.dp-port" | "ew.fn.dp-connector" => BUS,
        "ew.fn.valve-5-2-sol-spring" => FLUID,
        _ => MULTI,
    }
}

fn goto(s: &mut ShellModel, ty: &str) {
    let folio = match ty {
        MULTI => "101",
        FLUID => "102",
        OVERVIEW => "103",
        _ => "104",
    };
    done(s, "page.goto", json!({ "folio": folio }));
    done(s, "view.zoom_actual", json!({}));
}

fn grid(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
}

fn place_at(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid(s, k);
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
}

fn reps(s: &ShellModel) -> Vec<TableRep> {
    s.canvas().unwrap().page().tables.reps.clone()
}

/// The FnDef key of starter symbol `key`.
fn symbol_fndef(key: &str) -> String {
    let v: Value = serde_json::from_str(starter::item(key).unwrap_or_else(|| panic!("{key}")).json).unwrap();
    v["fndef"]["key"].as_str().unwrap().to_owned()
}

/// SC-3: each sample part with a template places on the starter symbol of its main function from the picker (the box
/// tool for a black box or an overview, the strip chooser for a terminal), and its device carries the part, held by
/// the project after saving and reopening.
#[test]
fn every_templated_sample_part_places_on_its_starter_symbol() {
    let parts = templated_parts();
    assert!(parts.iter().any(|(n, _)| n == "A9P52610"), "{parts:?}");
    let tmp = Tmp::new("sample-parts");
    let mut s = new_project(&tmp);
    let mut placed: Vec<(String, Uuid)> = Vec::new();
    for (k, (number, fndef)) in (0i64..).zip(&parts) {
        let page = page_of(fndef);
        goto(&mut s, page);
        let before = reps(&s);
        done(&mut s, "place.pick", json!({"part": number, "template": 0}));
        let cell = (8 + (k % 6) * 24, 10 + (k / 6) * 24);
        if s.tool().name() == "black_box" {
            let (a, b) = (grid(&s, cell), grid(&s, (cell.0 + 12, cell.1 + 8)));
            click(&mut s, a, json!({}));
            click(&mut s, b, json!({}));
        } else {
            if let Some(picker) = s.picker()
                && let Some(row) = picker.rows.iter().position(|r| matches!(r, PickerRow::Strip(_)))
            {
                done(&mut s, "picker.choose", json!({ "row": row }));
            }
            place_at(&mut s, cell);
        }
        done(&mut s, "ui.dismiss", json!({}));
        done(&mut s, "ui.dismiss", json!({}));
        let new: Vec<TableRep> = reps(&s)
            .into_iter()
            .filter(|r| before.iter().all(|b| b.id != r.id))
            .collect();
        let rep = new
            .iter()
            .find(|r| symbol_fndef(&r.symbol_key) == *fndef)
            .unwrap_or_else(|| panic!("{number}: no symbol of {fndef} placed: {new:?}"));
        placed.push((number.clone(), rep.device.unwrap_or_else(|| panic!("{number}: a device"))));
    }
    done(&mut s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.0.join("new.ew"), "test").unwrap();
    let q = session.query();
    for (number, device) in &placed {
        let (_, d) = q
            .devices()
            .iter()
            .find(|(id, _)| id.as_uuid() == *device)
            .unwrap_or_else(|| panic!("{number}: the device after reopening"));
        let numbers: Vec<String> = d
            .parts
            .iter()
            .filter_map(|u| {
                q.parts()
                    .iter()
                    .find(|(id, _)| id.as_uuid() == u.part.copy.as_uuid())
                    .map(|(_, c)| c.item.part_number.as_str().to_owned())
            })
            .collect();
        assert_eq!(numbers, [number.as_str()], "{number}");
    }
}

/// SC-5: in a new project the place tool's picker lists the sample parts' template functions (A9P52610's MCB), and
/// the Libraries navigator lists "Sample project parts" under the system tier.
#[test]
fn the_picker_and_the_libraries_navigator_offer_the_sample_parts() {
    let tmp = Tmp::new("sample-parts-picker");
    let mut s = new_project(&tmp);
    goto(&mut s, MULTI);
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "picker.filter", json!({"text": "A9P52610"}));
    let lines: Vec<String> = {
        let p = s.picker().expect("the picker");
        p.rows.iter().map(|r| p.row_line(*r)).collect()
    };
    assert_eq!(lines, ["row part \"A9P52610\" function 1 of 1 \"ew.fn.mcb-1pn\" main"]);
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "navigator.show", json!({"kind": "libraries"}));
    let t = s
        .nav(NavKind::Libraries)
        .and_then(|n| n.table())
        .expect("the Libraries navigator");
    let key = |role: NavRole, label: &str| {
        t.nodes
            .iter()
            .find(|n| n.role == role && n.label == label)
            .map(|n| n.key.clone())
            .unwrap_or_else(|| panic!("no {role:?} row {label}"))
    };
    let tier = key(NavRole::Tier, "System");
    let lib = key(NavRole::Library, "Sample project parts");
    assert!(lib.starts_with(&format!("{tier}/")), "{lib} under {tier}");
}
