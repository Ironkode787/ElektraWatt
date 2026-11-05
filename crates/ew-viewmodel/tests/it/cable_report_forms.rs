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
        format!("Created cable {tag} with 5 cores")
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

/// ID-1376 (C51-p4#4): Report block…'s two refusals, a kind that is no report and a folio that is no number, stay
/// under the fields, whole, in English and in Dutch; the kind's refusal lists the kinds the form offers, in words.
#[test]
fn report_block_refuses_a_bad_kind_and_a_bad_folio_in_both_languages() {
    let tmp = Tmp::new("ft1372b");
    let mut s = tmp.shell_in(Mode::Gui, true, "103");
    for (language, kind, folio) in [
        (
            "en",
            "\"bogus\" is not a report; use one of: ",
            "The first folio is a number: \"9x\" is not.",
        ),
        (
            "nl",
            "\"bogus\" is geen rapport; gebruik een van: ",
            "Het eerste folio moet een nummer zijn; \"9x\" is dat niet.",
        ),
    ] {
        done(&mut s, "view.language", json!({ "value": language }));
        done(&mut s, "report_block.new", json!({}));
        let words: Vec<String> = form(&s).fields[0]
            .options
            .iter()
            .map(|(_, w)| w.clone())
            .collect();
        s.set_form_field("kind", "bogus");
        let rev = s.revision();
        let Outcome::Failed { message } = s.submit_form() else {
            panic!("{language}: not a report")
        };
        assert_eq!(message, format!("{kind}{}.", words.join(", ")), "{language}");
        assert_eq!(form(&s).error.as_deref(), Some(message.as_str()), "{language}");
        s.set_form_field("kind", "terminal_diagram");
        s.set_form_field("folio", "9x");
        let Outcome::Failed { message } = s.submit_form() else {
            panic!("{language}: not a number")
        };
        assert_eq!(message, folio, "{language}");
        assert_eq!(form(&s).error.as_deref(), Some(folio), "{language}");
        assert_eq!(s.revision(), rev, "{language}: nothing added");
        done(&mut s, "ui.dismiss", json!({}));
    }
    s.shutdown();
}

/// ID-1389 (W51 finding E): Report block… refuses first folio 102, which a page of Golden A holds, under the fields,
/// whole, in English and in Dutch, as New page… does; nothing is added.
#[test]
fn report_block_refuses_a_folio_a_page_holds_in_both_languages() {
    let tmp = Tmp::new("id1389e");
    let mut s = tmp.shell_in(Mode::Gui, true, "103");
    let pages = s.page_labels().to_vec();
    for (language, refusal) in [
        (
            "en",
            "folio 102 already exists in this section; choose a free folio",
        ),
        ("nl", "folio 102 bestaat al in deze sectie; kies een vrij folio"),
    ] {
        done(&mut s, "view.language", json!({ "value": language }));
        done(&mut s, "report_block.new", json!({}));
        s.set_form_field("kind", "terminal_diagram");
        s.set_form_field("folio", "102");
        let rev = s.revision();
        let Outcome::Failed { message } = s.submit_form() else {
            panic!("{language}: folio 102 is taken")
        };
        assert_eq!(message, refusal, "{language}");
        assert_eq!(form(&s).error.as_deref(), Some(refusal), "{language}");
        assert_eq!(s.revision(), rev, "{language}: nothing added");
        assert_eq!(s.page_labels(), pages, "{language}");
        done(&mut s, "ui.dismiss", json!({}));
    }
    s.shutdown();
}

/// The library tiers of an installation: the content's system libraries, the rest in `root`.
fn tiers(root: &std::path::Path) -> ew_viewmodel::ew_api::library::LibraryTiers {
    ew_viewmodel::ew_api::library::LibraryTiers {
        system: vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/libraries")],
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// Whether the Libraries navigator lists a library copy of the project whose words hold `number`.
fn holds_copy(s: &mut ShellModel, number: &str) -> bool {
    done(s, "navigator.show", json!({"kind": "libraries"}));
    let t = s.nav(NavKind::Libraries).and_then(|n| n.table()).unwrap().clone();
    t.nodes.iter().any(|n| n.label.contains(number))
}

/// ID-1376 (C51-p4#3a): in a new project, New cable… with a cable part of an installed library embeds the part and
/// creates the cable in one transaction: one undo step takes both back, and no copy of the part is left.
#[test]
fn a_library_cable_part_is_embedded_with_its_cable_in_one_undo_step() {
    let tmp = Tmp::new("ft1371c");
    let config = ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test")
        .with_libraries(Some(tiers(&tmp.0)));
    let mut s = ShellModel::new(Mode::Gui, config);
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    const NUMBER: &str = "6XV1830-0EH10";
    assert!(!holds_copy(&mut s, NUMBER), "the new project holds no cable part");
    assert!(!s.can_undo());
    done(&mut s, "cable.new", json!({}));
    let (part, _) = form(&s).fields[0]
        .options
        .iter()
        .find(|(_, w)| w.starts_with(NUMBER))
        .cloned()
        .expect("the library's cable part is offered");
    s.set_form_field("part", &part);
    assert_eq!(s.submit_form(), Outcome::Done);
    assert_eq!(cables(&mut s).len(), 1);
    assert!(holds_copy(&mut s, NUMBER), "the part is embedded");
    done(&mut s, "edit.undo", json!({}));
    assert!(!s.can_undo(), "one undo step");
    assert_eq!(cables(&mut s), Vec::<(String, Vec<String>)>::new());
    assert!(!holds_copy(&mut s, NUMBER), "no copy of the part is left");
    s.shutdown();
}

/// ID-1376 (C51-p4#3b): two new cables both print `-?W?`; Connect cores… names each in its list by its part and its
/// position, and refuses the bare tag in words, in both languages, instead of taking the first.
#[test]
fn connect_cores_tells_two_cables_with_one_tag_apart() {
    let tmp = Tmp::new("ft1371d");
    let mut s = tmp.shell_in(Mode::Gui, true, "103");
    let before = cables(&mut s);
    for _ in 0..2 {
        done(&mut s, "cable.new", json!({"part": "NEBU-G78W5-K-2-N-LE5"}));
    }
    let new: Vec<String> = cables(&mut s)
        .into_iter()
        .map(|(tag, _)| tag)
        .filter(|t| !before.iter().any(|(b, _)| b == t))
        .collect();
    assert_eq!(new, ["-?W?", "-?W?"], "both new cables print the same tag");
    let w = wires(&s);
    done(&mut s, "select.set", json!({"ids": [w[0].to_string()]}));
    done(&mut s, "cable.connect_cores", json!({}));
    let words: Vec<String> = form(&s).fields[0]
        .options
        .iter()
        .map(|(_, w)| w.clone())
        .filter(|w| w.starts_with("-?W?"))
        .collect();
    assert_eq!(
        words,
        [
            "-?W? \u{b7} NEBU-G78W5-K-2-N-LE5 (1)",
            "-?W? \u{b7} NEBU-G78W5-K-2-N-LE5 (2)"
        ]
    );
    done(&mut s, "ui.dismiss", json!({}));
    for (language, refusal) in [
        ("en", "-?W? is the tag of 2 cables: choose one from the list."),
        (
            "nl",
            "-?W? is het kenmerk van 2 kabels: kies er een uit de lijst.",
        ),
    ] {
        done(&mut s, "view.language", json!({ "value": language }));
        let rev = s.revision();
        let Outcome::Failed { message } = s.dispatch("cable.connect_cores", &json!({"cable": "-?W?"})) else {
            panic!("{language}: an ambiguous tag")
        };
        assert_eq!(message, refusal, "{language}");
        assert_eq!(s.revision(), rev, "{language}: nothing assigned");
    }
    done(&mut s, "view.language", json!({ "value": "en" }));
    done(
        &mut s,
        "cable.connect_cores",
        json!({"cable": "-?W? \u{b7} NEBU-G78W5-K-2-N-LE5 (2)"}),
    );
    assert_eq!(
        state(&s)["toast"]["text"],
        "Wires connected to the cores of -?W? \u{b7} NEBU-G78W5-K-2-N-LE5 (2): 1"
    );
    s.shutdown();
}
