// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1371 and FT-1372 (ID-1362) on copies of Golden A, through the forms as the window shows them: Draw ▸ Connect ▸
//! New cable… creates a cable from its part with the part's cores, Connect cores… makes the selected wires its cores
//! by order (one undo step; too few free cores refused in words), and Output ▸ Report block… adds a report block
//! whose pages join the page order.

use crate::common;

use common::{Tmp, done, state};
use ew_viewmodel::actions::{Menu, MenuEntry, menu_model};
use ew_viewmodel::ew_api::output::{NavRole, Uuid};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Dialog, FormDialog, Mode, Outcome, ShellModel};
use serde_json::json;

/// The open form.
#[track_caller]
fn form(s: &ShellModel) -> FormDialog {
    match s.dialog() {
        Some(Dialog::Form(f)) => f.clone(),
        other => panic!("no form: {other:?}"),
    }
}

/// The cables of the Cables navigator: (tag, its cores in row order).
fn cables(s: &mut ShellModel) -> Vec<(String, Vec<String>)> {
    done(s, "navigator.show", json!({"kind": "cables"}));
    let t = s.nav(NavKind::Cables).and_then(|n| n.table()).unwrap().clone();
    t.nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.role == NavRole::Cable)
        .map(|(i, n)| {
            let cores = t
                .nodes
                .iter()
                .filter(|c| c.role == NavRole::Conductor && c.parent == Some(i as u32))
                .map(|c| c.label.clone())
                .collect();
            (n.label.clone(), cores)
        })
        .collect()
}

/// The `kind` of each connection of the page shown, by ID.
fn kinds(s: &ShellModel, ids: &[Uuid]) -> Vec<String> {
    let t = &s.canvas().unwrap().page().tables;
    ids.iter()
        .map(|id| t.connections.iter().find(|c| c.id == *id).unwrap().kind.clone())
        .collect()
}

/// The wires of the page shown, left to right, then top to bottom (the order Connect cores… takes them).
fn wires(s: &ShellModel) -> Vec<Uuid> {
    let t = &s.canvas().unwrap().page().tables;
    let mut w: Vec<((i64, i64), Uuid)> = t
        .connections
        .iter()
        .filter(|c| c.kind == "wire")
        .map(|c| {
            let x = c.polyline.iter().map(|p| p.0).min().unwrap();
            let y = c.polyline.iter().map(|p| p.1).min().unwrap();
            ((x, y), c.id)
        })
        .collect();
    w.sort();
    w.into_iter().map(|(_, id)| id).collect()
}

/// The labels of the items of `menu`, through its submenus and groups, as (group or submenu label, item label).
fn menu_items(s: &ShellModel, menu: Menu) -> Vec<(String, String)> {
    let model = menu_model(s);
    let m = model.iter().find(|m| m.menu == menu).unwrap();
    let mut out = Vec::new();
    for e in &m.entries {
        match e {
            MenuEntry::Item(i) => out.push((String::new(), i.label.clone())),
            MenuEntry::Submenu { label, items, .. } => {
                out.extend(items.iter().map(|i| (label.clone(), i.label.clone())))
            }
            MenuEntry::Group { label, entries, .. } => {
                for e in entries {
                    out.extend(e.items().into_iter().map(|i| (label.clone(), i.label.clone())));
                }
            }
            MenuEntry::Separator => {}
        }
    }
    out
}

/// FT-1371: New cable… sits in Draw ▸ Connect; its form offers the cable parts and creates the cable with the part's
/// five cores in one undo step; Connect cores… sits beside it and makes the two leftmost selected wires the cable's BK
/// and BU cores in one undo step; four wires more than its three free cores are refused in words, nothing assigned.
#[test]
fn a_new_cable_takes_its_parts_cores_and_the_selected_wires_become_its_cores() {
    let tmp = Tmp::new("ft1371");
    let mut s = tmp.shell_in(Mode::Gui, true, "103");
    let draw = menu_items(&s, Menu::Draw);
    for label in ["New cable…", "Connect cores…"] {
        assert!(
            draw.contains(&("Connect".to_owned(), label.to_owned())),
            "{label} in Draw ▸ Connect: {draw:?}"
        );
    }
    let before = cables(&mut s);
    // The form: one field, the cable parts of the project and the libraries.
    done(&mut s, "cable.new", json!({}));
    let f = form(&s);
    assert_eq!(f.action, "cable.new");
    assert_eq!(f.title_key.as_str(), "cable-new-title");
    assert_eq!(f.fields.len(), 1);
    assert_eq!(f.fields[0].label_key.as_str(), "form-cable-part");
    let (part, _) = f.fields[0]
        .options
        .iter()
        .find(|(_, w)| w.starts_with("NEBU-G78W5-K-2-N-LE5"))
        .cloned()
        .expect("Golden A's cable part is offered");
    s.set_form_field("part", &part);
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none(), "the form closes");
    let after = cables(&mut s);
    assert_eq!(after.len(), before.len() + 1);
    let (tag, cores) = after
        .iter()
        .find(|c| !before.contains(c))
        .cloned()
        .expect("the new cable");
    let mut sorted = cores.clone();
    sorted.sort();
    assert_eq!(sorted, ["BK", "BN", "BU", "GNYE", "WH"], "the part's cores");
    assert_eq!(
        state(&s)["toast"]["text"],
        format!("Created cable {tag}; its cores: 5")
    );
    // Connect cores…: the two leftmost wires of the page become BK and BU.
    let w = wires(&s);
    assert!(w.len() >= 6, "folio 103 has wires: {}", w.len());
    done(
        &mut s,
        "select.set",
        json!({"ids": [w[1].to_string(), w[0].to_string()]}),
    );
    done(&mut s, "cable.connect_cores", json!({}));
    let f = form(&s);
    assert_eq!(f.action, "cable.connect_cores");
    assert_eq!(f.fields[0].label_key.as_str(), "form-cable");
    assert_eq!(f.ok_key.as_str(), "dialog-connect");
    let (cable, _) = f.fields[0]
        .options
        .iter()
        .find(|(_, words)| *words == tag)
        .cloned()
        .expect("the new cable is offered");
    s.set_form_field("cable", &cable);
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none());
    assert_eq!(kinds(&s, &w[..2]), ["core", "core"]);
    assert_eq!(
        state(&s)["toast"]["text"],
        format!("Wires connected to the cores of {tag}: 2")
    );
    done(&mut s, "navigator.show", json!({"kind": "cables"}));
    let from = |s: &mut ShellModel, core: &str| {
        let t = s.nav(NavKind::Cables).and_then(|n| n.table()).unwrap().clone();
        let col = t.columns.iter().position(|c| c.key == "from").unwrap();
        let cable_row = t.nodes.iter().position(|n| n.label == tag).unwrap() as u32;
        t.nodes
            .iter()
            .find(|n| n.parent == Some(cable_row) && n.label == core)
            .map(|n| n.cells[col].clone())
            .unwrap()
    };
    for core in ["BK", "BU"] {
        assert!(!from(&mut s, core).is_empty(), "{core} is connected");
    }
    assert_eq!(from(&mut s, "GNYE"), "", "GNYE stays spare");
    // Four wires more for the three free cores: refused under the fields, nothing assigned.
    done(
        &mut s,
        "select.set",
        json!({"ids": w[2..6].iter().map(ToString::to_string).collect::<Vec<_>>()}),
    );
    done(&mut s, "cable.connect_cores", json!({}));
    s.set_form_field("cable", &cable);
    let rev2 = s.revision();
    let Outcome::Failed { message } = s.submit_form() else {
        panic!("too few cores")
    };
    assert_eq!(
        message,
        format!("{tag} has 3 free cores for 4 wires: select fewer wires, or choose another cable.")
    );
    assert_eq!(form(&s).error.as_deref(), Some(message.as_str()));
    assert_eq!(s.revision(), rev2);
    done(&mut s, "ui.dismiss", json!({}));
    // One undo takes the connection back.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(kinds(&s, &w[..2]), ["wire", "wire"]);
    s.shutdown();
}

/// FT-1371: without a selected wire Connect cores… is disabled with its reason; headless, New cable… needs its part.
#[test]
fn connect_cores_needs_selected_wires_and_new_cable_its_part() {
    let tmp = Tmp::new("ft1371b");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    assert_eq!(
        common::refused(&mut s, "cable.connect_cores", json!({"cable": "-106W1"})),
        "reason-nothing-selected"
    );
    let Outcome::Failed { message } = s.dispatch("cable.new", &json!({})) else {
        panic!("needs its part")
    };
    assert_eq!(message, "Missing parameter `part`.");
    let Outcome::Failed { message } = s.dispatch("cable.new", &json!({"part": "NO-SUCH-CABLE"})) else {
        panic!("no such part")
    };
    assert_eq!(
        message,
        "No cable part has the part number NO-SUCH-CABLE: choose one from the list."
    );
    s.shutdown();
}

/// FT-1372: Report block… sits in the Output menu; its form offers the report kinds in words and proposes the folio
/// after the last page; a terminal diagram from folio 950 adds its pages in one undo step.
#[test]
fn a_report_block_from_its_form_adds_its_pages() {
    let tmp = Tmp::new("ft1372");
    let mut s = tmp.shell_in(Mode::Gui, true, "103");
    let output = menu_items(&s, Menu::Output);
    assert!(
        output.iter().any(|(_, l)| l == "Report block…"),
        "Report block… in Output: {output:?}"
    );
    done(&mut s, "report_block.new", json!({}));
    let f = form(&s);
    assert_eq!(f.action, "report_block.new");
    assert_eq!(f.title_key.as_str(), "report-block-new-title");
    let names: Vec<&str> = f.fields.iter().map(|f| f.name).collect();
    assert_eq!(names, ["kind", "folio", "title"]);
    let kind = &f.fields[0];
    assert!(
        kind.options
            .contains(&("terminal_diagram".to_owned(), "Terminal diagram".to_owned())),
        "{:?}",
        kind.options
    );
    let last = s
        .page_labels()
        .iter()
        .filter_map(|l| l.split('.').next()?.parse::<u32>().ok())
        .max()
        .unwrap();
    assert_eq!(
        f.fields[1].value,
        (last + 1).to_string(),
        "the folio after the last page"
    );
    s.set_form_field("kind", "terminal_diagram");
    s.set_form_field("folio", "950");
    s.set_form_field("title", "Terminals {strip}");
    assert!(!s.page_labels().iter().any(|l| l.starts_with("950")));
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none());
    assert!(
        s.page_labels().iter().any(|l| l == "950"),
        "{:?}",
        s.page_labels()
    );
    assert_eq!(
        state(&s)["toast"]["text"],
        "Added the report block Terminal diagram from folio 950"
    );
    done(&mut s, "edit.undo", json!({}));
    assert!(!s.page_labels().iter().any(|l| l.starts_with("950")));
    // A folio that is no number stays under the fields.
    done(&mut s, "report_block.new", json!({}));
    s.set_form_field("folio", "9x");
    let Outcome::Failed { message } = s.submit_form() else {
        panic!("not a number")
    };
    assert_eq!(message, "The first folio is a number: \"9x\" is not.");
    assert_eq!(form(&s).error.as_deref(), Some(message.as_str()));
    s.shutdown();
}
