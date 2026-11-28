// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The life of a part of one's own (doc 07 §3.1, §5.8 to §5.11), as a person who brings his own parts in drives the
//! window: each journey through the actions a click sends, on a copy of the sample in the window's mode with the
//! settings folder of the tests and nothing else configured, so My parts is the one that folder gives. A part he makes
//! goes to My parts with the parts of his own it names; a device takes a part with what it names; a part is one row
//! wherever it is listed; no message names a part by an internal identifier.

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::ew_api::parts::Lives;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;

/// The sample's copy on page 102 in the window's mode, the settings folder of the tests its only configuration.
fn sample(tmp: &Tmp) -> ShellModel {
    tmp.sample_in(Mode::Gui, true, "102")
}

/// Types `fields` into the open sheet in order: `+<table>` adds a row of that table, any other a field.
fn fill(s: &mut ShellModel, fields: &[(&str, &str)]) {
    for (field, value) in fields {
        match field.strip_prefix('+') {
            Some(table) => done(s, "parts.row", json!({"table": table})),
            None => done(s, "parts.field", json!({"field": field, "value": value})),
        }
    }
}

/// The rows of Parts' list whose part number is `number`, each with where it lives, after a search for it in `scope`.
fn rows(s: &mut ShellModel, number: &str, scope: &str) -> Vec<Lives> {
    done(s, "parts.find", json!({"text": number, "scope": scope}));
    let place = s.parts_place().expect("Parts");
    place
        .found
        .iter()
        .filter(|f| f.offer.part_number == number)
        .map(|f| f.lives.clone())
        .collect()
}

/// Part `number` is one row of Parts' list, a part of My parts, and the list of My parts holds it.
#[track_caller]
fn one_row_mine(s: &mut ShellModel, number: &str) {
    assert_eq!(rows(s, number, "all"), [Lives::Mine], "{number} in All");
    assert_eq!(rows(s, number, "mine"), [Lives::Mine], "{number} in My parts");
}

/// The open sheet's part number and where it lives.
fn open(s: &ShellModel) -> (String, Lives) {
    let o = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("a part is open");
    (o.sheet.text("part_number").to_owned(), o.lives.clone())
}

/// Every text a person reads of the last outcome: the toasts and the open sheet's message.
fn said(s: &ShellModel) -> String {
    let notes: Vec<String> = s.notifications().entries().map(|n| n.text.clone()).collect();
    let foot = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .and_then(|o| o.foot_message())
        .unwrap_or_default();
    format!("{} | {foot}", notes.join(" | "))
}

/// No text a person reads holds a UUID, a version requirement or the error of a library that holds nothing.
#[track_caller]
fn no_identifier(text: &str) {
    let uuid = text.split(|c: char| !c.is_ascii_hexdigit() && c != '-').any(|w| {
        w.len() == 36 && w.chars().filter(|c| *c == '-').count() == 4
    });
    assert!(!uuid, "an identifier: {text}");
    for raw in ["@*", "@^", "no installed library holds it", "Version req"] {
        assert!(!text.contains(raw), "{raw}: {text}");
    }
}

/// Imports `rows` into the sample's copy as the project's own parts (as a colleague's project carries them): parts
/// only in this project.
fn project_parts(s: &mut ShellModel, tmp: &Tmp, rows: &str) {
    common::import_parts(s, tmp, rows);
}

/// An end plate and a one-level terminal only in this project.
const HERE: &str = "EX-EP-1,Example,EX-EP-1,accessory,,End plate,piece,,,,,,,,,,,\n\
                    EX-TB-1,Example,EX-TB-1,terminal,,Terminal 2.5,piece,,,5.2,60,40,ts35x7_5,,,,,\n\
                    EX-TB-2,Example,EX-TB-2,terminal,,Terminal 4,piece,,,6.2,60,40,ts35x7_5,,,,,\n";

/// Journey a: a cabinet whose mounting plate is made in passing from its sheet (New plate…); Save. Both are parts of
/// My parts, each one row; the toast says where the cabinet is; the list's head counts them as his.
#[test]
fn a_cabinet_and_its_plate_made_in_passing_are_his() {
    let tmp = Tmp::new("life-a");
    let mut s = sample(&tmp);
    done(&mut s, "parts.new", json!({"start": "enclosure"}));
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "EX-CAB-8"),
            ("sections.enclosure.width", "800"),
            ("sections.enclosure.height", "1000"),
            ("sections.enclosure.depth", "300"),
        ],
    );
    done(
        &mut s,
        "parts.new",
        json!({"start": "plate", "for_field": "sections.enclosure.plate"}),
    );
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "EX-CAB-8-MP"),
            ("sections.mounting_plate.width", "760"),
            ("sections.mounting_plate.height", "960"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    assert_eq!(open(&s).0, "EX-CAB-8", "back on the cabinet");
    done(&mut s, "parts.save", json!({}));
    assert_eq!(open(&s), ("EX-CAB-8".to_owned(), Lives::Mine));
    assert_eq!(toast(&s), "Part EX-CAB-8 is in the project and in My parts.");
    one_row_mine(&mut s, "EX-CAB-8");
    one_row_mine(&mut s, "EX-CAB-8-MP");
    done(&mut s, "parts.find", json!({"text": "EX-CAB-8", "scope": "all"}));
    let counts = s.parts_place().unwrap().counts;
    assert_eq!((counts.all, counts.mine), (2, 2), "the head counts them as his");
    no_identifier(&said(&s));
    s.shutdown();
}

/// Journey b: a two-level terminal that comes with an end plate only in this project; Save keeps the end plate in My
/// parts with it in one write and says so by maker and number; Keep in my parts then has nothing left to keep. A
/// terminal only in this project that comes with that end plate is kept with it the same way.
#[test]
fn a_terminal_is_kept_with_the_end_plate_it_comes_with() {
    let tmp = Tmp::new("life-b");
    let mut s = sample(&tmp);
    project_parts(&mut s, &tmp, HERE);
    done(&mut s, "parts.open", json!({"part": "EX-EP-1"}));
    assert_eq!(open(&s).1, Lives::OnlyHere);
    done(&mut s, "parts.new", json!({"start": "terminal"}));
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "EX-TB-2L"),
            ("levels", "2"),
            ("+accessories", ""),
            ("accessories[0].part", "EX-EP-1"),
            ("accessories[0].qty", "1"),
            ("accessories[0].per", "strip"),
            ("accessories[0].kind", "end_plate"),
        ],
    );
    assert_eq!(s.dispatch("parts.save", &json!({})), Outcome::Done, "{}", said(&s));
    assert_eq!(open(&s), ("EX-TB-2L".to_owned(), Lives::Mine));
    assert_eq!(
        toast(&s),
        "Part EX-TB-2L is in the project and in My parts, kept with Example EX-EP-1."
    );
    one_row_mine(&mut s, "EX-TB-2L");
    one_row_mine(&mut s, "EX-EP-1");
    no_identifier(&said(&s));
    for number in ["EX-TB-2L", "EX-EP-1"] {
        done(&mut s, "parts.open", json!({"part": number}));
        assert_ne!(s.dispatch("parts.keep", &json!({})), Outcome::Done, "{number} is his already");
    }
    // A terminal only in this project that comes with the end plate, now his: Keep in my parts keeps it, and it names
    // the end plate of My parts.
    done(&mut s, "parts.open", json!({"part": "EX-TB-1"}));
    fill(
        &mut s,
        &[
            ("+accessories", ""),
            ("accessories[0].part", "EX-EP-1"),
            ("accessories[0].qty", "1"),
            ("accessories[0].per", "strip"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(s.dispatch("parts.keep", &json!({})), Outcome::Done, "{}", said(&s));
    assert_eq!(open(&s), ("EX-TB-1".to_owned(), Lives::Mine));
    one_row_mine(&mut s, "EX-TB-1");
    one_row_mine(&mut s, "EX-EP-1");
    no_identifier(&said(&s));
    s.shutdown();
}

/// Journey b, the other way: a terminal only in this project that comes with an end plate only in this project; Keep
/// in my parts keeps both in one write, and the toast names the end plate.
#[test]
fn keep_in_my_parts_keeps_the_parts_it_names_only_here() {
    let tmp = Tmp::new("life-b-keep");
    let mut s = sample(&tmp);
    project_parts(&mut s, &tmp, HERE);
    done(&mut s, "parts.open", json!({"part": "EX-TB-1"}));
    fill(
        &mut s,
        &[
            ("+accessories", ""),
            ("accessories[0].part", "EX-EP-1"),
            ("accessories[0].qty", "1"),
            ("accessories[0].per", "strip"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(s.dispatch("parts.keep", &json!({})), Outcome::Done, "{}", said(&s));
    assert_eq!(open(&s), ("EX-TB-1".to_owned(), Lives::Mine));
    assert_eq!(
        toast(&s),
        "EX-TB-1 is kept in My parts with Example EX-EP-1."
    );
    one_row_mine(&mut s, "EX-TB-1");
    one_row_mine(&mut s, "EX-EP-1");
    no_identifier(&said(&s));
    s.shutdown();
}

/// Journey c: a second terminal made with Like this from the first saves as the first did.
#[test]
fn a_terminal_like_the_first_saves_as_the_first_did() {
    let tmp = Tmp::new("life-c");
    let mut s = sample(&tmp);
    project_parts(&mut s, &tmp, HERE);
    done(&mut s, "parts.new", json!({"start": "terminal"}));
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "EX-TB-2L"),
            ("levels", "2"),
            ("+accessories", ""),
            ("accessories[0].part", "EX-EP-1"),
            ("accessories[0].qty", "1"),
            ("accessories[0].per", "strip"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.new", json!({"like": true}));
    fill(&mut s, &[("part_number", "EX-TB-2L-B")]);
    assert_eq!(s.dispatch("parts.save", &json!({})), Outcome::Done, "{}", said(&s));
    assert_eq!(open(&s), ("EX-TB-2L-B".to_owned(), Lives::Mine));
    one_row_mine(&mut s, "EX-TB-2L-B");
    one_row_mine(&mut s, "EX-EP-1");
    no_identifier(&said(&s));
    s.shutdown();
}

/// Journey d: two terminals only in this project selected, the rail set for both; Save keeps them in My parts and
/// applies the value to both (its label says so), no dead end.
#[test]
fn several_terminals_only_here_take_one_value_and_are_kept() {
    let tmp = Tmp::new("life-d");
    let mut s = sample(&tmp);
    project_parts(&mut s, &tmp, HERE);
    done(&mut s, "parts.open", json!({}));
    done(&mut s, "parts.find", json!({"text": "EX-TB-", "scope": "all"}));
    let params: Vec<String> = s
        .parts_place()
        .unwrap()
        .found
        .iter()
        .filter(|f| f.offer.part_number.starts_with("EX-TB-"))
        .map(ew_viewmodel::ew_api::parts::PartFound::param)
        .collect();
    assert_eq!(params.len(), 2);
    done(&mut s, "parts.open", json!({"part": params[0]}));
    done(&mut s, "parts.open", json!({"part": params[1], "add": true}));
    assert!(s.many_sheet().is_some(), "the sheet of several");
    fill(&mut s, &[("mech.mount_kind", "ts35x15")]);
    let label = s.parts_place().unwrap().save_label();
    assert_eq!(label, "Keep in my parts and save");
    assert_eq!(s.dispatch("parts.save", &json!({})), Outcome::Done, "{}", said(&s));
    if s.properties().card().is_some() {
        done(&mut s, "preview.apply", json!({}));
    }
    for number in ["EX-TB-1", "EX-TB-2"] {
        one_row_mine(&mut s, number);
        done(&mut s, "parts.open", json!({"part": number}));
        let o = s.parts_place().and_then(|p| p.sheet.as_ref()).unwrap();
        assert_eq!(
            o.sheet.row("mech.mount_kind").map(|r| r.text.as_str()),
            Some("ts35x15"),
            "{number} took the value"
        );
    }
    no_identifier(&said(&s));
    s.shutdown();
}

/// Journey e: parts made in My parts with no project open (a terminal that comes with an end plate of his); in the
/// project the terminals of a strip are replaced by his terminal through the part dialog, which offers it once: the
/// project takes the terminal with its end plate in the same step.
#[test]
fn the_terminals_of_a_strip_take_his_terminal_with_its_end_plate() {
    let tmp = Tmp::new("life-e");
    let mut s = tmp.empty_shell(Mode::Gui);
    done(&mut s, "parts.open", json!({}));
    for (start, fields) in [
        ("accessory", &[("manufacturer", "Example"), ("part_number", "EX-EP-9")][..]),
        (
            "terminal",
            &[
                ("manufacturer", "Example"),
                ("part_number", "EX-TB-9"),
                ("mech.width", "5.2"),
                ("mech.height", "60"),
                ("mech.depth", "40"),
                ("mech.mount_kind", "ts35x7_5"),
                ("+accessories", ""),
                ("accessories[0].part", "EX-EP-9"),
                ("accessories[0].qty", "1"),
                ("accessories[0].per", "strip"),
            ][..],
        ),
    ] {
        done(&mut s, "parts.new", json!({"start": start}));
        fill(&mut s, fields);
        assert_eq!(s.dispatch("parts.save", &json!({})), Outcome::Done, "{}", said(&s));
        assert_eq!(open(&s).1, Lives::Mine);
        let number = open(&s).0;
        assert_eq!(rows(&mut s, &number, "mine"), [Lives::Mine], "{number} in My parts");
    }
    done(&mut s, "parts.back", json!({}));
    let dir = tmp.sample_copy();
    common::copy_dir(std::path::Path::new(common::SAMPLE), &dir);
    done(&mut s, "file.open", json!({"dir": dir.to_string_lossy()}));
    done(&mut s, "strip.edit", json!({ "strip": "-X1021" }));
    let (k1, k2) = (
        common::strip_row(&s, "Terminal", "1"),
        common::strip_row(&s, "Terminal", "2"),
    );
    done(&mut s, "table.cell", json!({ "kind": "strips", "row": k1 }));
    done(
        &mut s,
        "navigator.select",
        json!({ "kind": "strips", "keys": [k1, k2] }),
    );
    done(&mut s, "part.choose", json!({ "target": "parts" }));
    done(&mut s, "part.search", json!({ "text": "EX-TB-9" }));
    let Some(Dialog::PartChoose(c)) = s.dialog() else {
        panic!("the part dialog")
    };
    let offered = c.offers.iter().filter(|o| o.part_number == "EX-TB-9").count();
    assert_eq!(offered, 1, "offered once");
    assert_eq!(
        s.dispatch("part.pick", &json!({ "part": "EX-TB-9" })),
        Outcome::Done,
        "{}",
        said(&s)
    );
    let parts = common::strip_texts(&s, "Part");
    assert_eq!(parts.iter().filter(|p| *p == "EX-TB-9").count(), 2, "{parts:?}");
    // The project took the terminal with its end plate, each one row of his.
    done(&mut s, "parts.open", json!({}));
    assert_eq!(rows(&mut s, "EX-TB-9", "project"), [Lives::Mine]);
    assert_eq!(rows(&mut s, "EX-EP-9", "project"), [Lives::Mine], "its end plate");
    no_identifier(&said(&s));
    s.shutdown();
}

/// Journey f: Create enclosure on a location that has its enclosure says so before anything is chosen, names the part
/// that enclosure has and offers no other part: it draws the enclosure as it is.
#[test]
fn create_enclosure_on_a_location_with_one_offers_no_part_it_ignores() {
    let tmp = Tmp::new("life-f");
    let mut s = sample(&tmp);
    done(&mut s, "parts.new", json!({"start": "enclosure"}));
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "EX-CAB-8"),
            ("sections.enclosure.width", "800"),
            ("sections.enclosure.height", "1000"),
            ("sections.enclosure.depth", "300"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
    done(
        &mut s,
        "page.new",
        json!({"folio": "926", "type": "panel_layout", "title": "Layout"}),
    );
    done(&mut s, "layout.enclosure_new", json!({}));
    let form = common::form(&s);
    let location = form.fields.iter().find(|f| f.name == "location").unwrap();
    let held = s
        .enclosure_choice()
        .expect("the location has its enclosure");
    assert!(held.0.starts_with("Draw -UC1 of "), "{held:?}");
    let part = form.fields.iter().find(|f| f.name == "part").unwrap();
    assert_eq!(part.options.len(), 1, "only the part -UC1 has: {:?}", part.options);
    assert!(
        part.options[0].1.starts_with("EW-GEN-ENC-1"),
        "{:?} for {}",
        part.options,
        location.value
    );
    let notes = s.enclosure_form_notes().join(" ");
    assert!(notes.contains("EW-GEN-ENC-1"), "{notes}");
    // A script that names another part is refused in words, not drawn as if it took it.
    let code = location
        .options
        .iter()
        .find(|(v, _)| *v == location.value)
        .map(|(_, w)| w.clone())
        .unwrap();
    done(&mut s, "ui.dismiss", json!({}));
    let out = s.dispatch(
        "layout.enclosure_new",
        &json!({"location": code, "part": "EX-CAB-8"}),
    );
    assert!(matches!(out, Outcome::Refused { .. } | Outcome::Failed { .. }), "{out:?}");
    s.shutdown();
}

/// Journey g: his ferrule added to the workshop stock stands first of the ferrules (he adds it because he prefers it);
/// a row of the stock's list opens that part.
#[test]
fn his_ferrule_added_to_the_stock_stands_first_of_its_kind() {
    let tmp = Tmp::new("life-g");
    let mut s = sample(&tmp);
    done(&mut s, "parts.new", json!({"start": "ferrule"}));
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "EX-FER-1.5"),
            ("sections.ferrule.csa_min", "1.5"),
            ("sections.ferrule.csa_max", "1.5"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.stock", json!({"op": "add"}));
    done(&mut s, "parts.stock", json!({}));
    let place = s.parts_place().unwrap();
    let ferrules: Vec<String> = place
        .found
        .iter()
        .filter(|f| f.offer.kind == "ferrule")
        .map(|f| f.offer.part_number.clone())
        .collect();
    assert_eq!(ferrules.first().map(String::as_str), Some("EX-FER-1.5"), "{ferrules:?}");
    let second = place
        .found
        .iter()
        .find(|f| f.offer.kind == "ferrule" && f.offer.part_number != "EX-FER-1.5")
        .unwrap()
        .param();
    done(&mut s, "parts.open", json!({"part": second}));
    let mine = s
        .parts_place()
        .unwrap()
        .found
        .iter()
        .find(|f| f.offer.part_number == "EX-FER-1.5")
        .unwrap()
        .param();
    done(&mut s, "parts.open", json!({"part": mine}));
    assert_eq!(open(&s).0, "EX-FER-1.5", "the row opens its part");
    s.shutdown();
}

/// Journey b, the way he went: Comes with makes the end plate in passing (New part… in its row, as a cabinet's plate is
/// made), and Save returns to the terminal's row with it named; the end plate lacks nothing that only a device needs.
#[test]
fn comes_with_makes_its_part_in_passing() {
    use ew_viewmodel::ew_api::part_fields::Feature;
    let tmp = Tmp::new("life-b-passing");
    let mut s = sample(&tmp);
    done(&mut s, "parts.new", json!({"start": "terminal"}));
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "EX-TB-2L"),
            ("+accessories", ""),
        ],
    );
    done(
        &mut s,
        "parts.new",
        json!({"start": "accessory", "for_field": "accessories[0].part"}),
    );
    fill(&mut s, &[("manufacturer", "Example"), ("part_number", "EX-EP-2L")]);
    let lacks: Vec<Feature> = (s.parts_place().and_then(|p| p.sheet.as_ref()))
        .map(|o| o.lacks.iter().map(|l| l.feature).collect())
        .unwrap_or_default();
    for only_a_device in [Feature::Placing, Feature::Footprint] {
        assert!(!lacks.contains(&only_a_device), "an end plate: {lacks:?}");
    }
    // Ready for says that the schematic does not draw it, rather than that it is ready for a page.
    let placing = (s.parts_place().and_then(|p| p.sheet.as_ref()))
        .and_then(|o| o.ready.iter().find(|r| r.feature == Feature::Placing).cloned())
        .expect("the schematic's row");
    assert!(
        matches!(placing.state, ew_viewmodel::ew_api::part_fields::Readiness::NotHere(_)),
        "{placing:?}"
    );
    done(&mut s, "parts.save", json!({}));
    let o = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the terminal again");
    assert_eq!(o.sheet.text("part_number"), "EX-TB-2L");
    let named = o.sheet.table("accessories").and_then(|t| t.rows.first()).and_then(|r| r.first());
    assert_eq!(named.map(String::as_str), Some("EX-EP-2L"), "the row names it");
    fill(&mut s, &[("accessories[0].qty", "1"), ("accessories[0].per", "strip")]);
    assert_eq!(s.dispatch("parts.save", &json!({})), Outcome::Done, "{}", said(&s));
    one_row_mine(&mut s, "EX-TB-2L");
    one_row_mine(&mut s, "EX-EP-2L");
    s.shutdown();
}

/// A two-level terminal's picture draws both levels, wider than one level's; Ready for keeps what it said while a
/// Comes with row is added and not filled in yet.
#[test]
fn a_two_level_terminal_shows_its_levels_and_ready_for_holds() {
    use ew_viewmodel::ew_api::part_fields::Feature;
    let tmp = Tmp::new("life-levels");
    let mut s = sample(&tmp);
    done(&mut s, "parts.new", json!({"start": "terminal"}));
    fill(&mut s, &[("manufacturer", "Example"), ("part_number", "EX-TB-L")]);
    let width = |s: &ShellModel| {
        let o = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the sheet");
        s.part_schematic(&o.sheet).map(|(_, size)| size.0.0).expect("a picture")
    };
    let one = width(&s);
    fill(&mut s, &[("levels", "2")]);
    assert!(width(&s) > one, "two levels are drawn: {} against {one}", width(&s));
    let placing = |s: &ShellModel| {
        (s.parts_place().and_then(|p| p.sheet.as_ref()))
            .and_then(|o| o.ready.iter().find(|r| r.feature == Feature::Placing).cloned())
    };
    let before = placing(&s);
    done(&mut s, "parts.row", json!({"table": "accessories"}));
    assert_eq!(placing(&s), before, "Ready for holds while the row is empty");
    s.shutdown();
}

/// Journey e, the other way in: his terminal the project holds is given Comes with an end plate of My parts that the
/// project does not hold; the Save's card brings the end plate into the project with the update, so that no reference
/// names a part the project lacks.
#[test]
fn his_terminals_update_brings_the_end_plate_it_comes_with() {
    let tmp = Tmp::new("life-e-update");
    let mut s = tmp.empty_shell(Mode::Gui);
    done(&mut s, "parts.open", json!({}));
    done(&mut s, "parts.new", json!({"start": "accessory"}));
    fill(&mut s, &[("manufacturer", "Example"), ("part_number", "EX-EP-7")]);
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
    let dir = tmp.sample_copy();
    common::copy_dir(std::path::Path::new(common::SAMPLE), &dir);
    done(&mut s, "file.open", json!({"dir": dir.to_string_lossy()}));
    done(&mut s, "parts.new", json!({"start": "terminal"}));
    fill(&mut s, &[("manufacturer", "Example"), ("part_number", "EX-TB-7")]);
    done(&mut s, "parts.save", json!({}));
    fill(
        &mut s,
        &[
            ("+accessories", ""),
            ("accessories[0].part", "EX-EP-7"),
            ("accessories[0].qty", "1"),
            ("accessories[0].per", "strip"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    assert_eq!(s.dispatch("preview.apply", &json!({})), Outcome::Done, "{}", said(&s));
    assert_eq!(rows(&mut s, "EX-EP-7", "project"), [Lives::Mine], "the end plate came with it");
    s.shutdown();
}
