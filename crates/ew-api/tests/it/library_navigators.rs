// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The data half: the Libraries navigator on the sample project shows its copies without libraries and
//! `UpdateAvailable` with a fixture library, tiers and states in words; the Macros navigator
//! lists a saved macro under its tier and its detail lists the devices; a new macro instance ID is
//! deterministic and skips held IDs.

use crate::common::{self, Dir, tiers};

use ew_api::library::{LibraryTiers, Registry};
use ew_api::output::{NavKind, NavNode, NavRole, NavTable};
use ew_api::{AnyCommand, InsertAt, MacroScope, MacroSpec, MacroValues, Session, SessionOptions};
use ew_ids::Uuid;
use ew_mltext::MlText;
use ew_schema::LibDocument;
use serde_json::json;

/// The sample with the libraries of `tiers`, in memory.
fn sample(tiers: Option<LibraryTiers>) -> Session {
    let (project, records, lib) = common::sample_records();
    let options = SessionOptions {
        libraries: tiers,
        ids: Some(ew_testids::seeded_ids(38)),
        ..SessionOptions::default()
    };
    Session::from_records(project, records, lib, options).unwrap()
}

/// The sample's copy of its power supply's part EW-GEN-PSU-1, as JSON.
fn psu_part() -> serde_json::Value {
    let (_, _, lib) = common::sample_records();
    let doc = lib
        .iter()
        .find(|d| matches!(d, LibDocument::Part(p) if p.item.part_number.as_str() == "EW-GEN-PSU-1"))
        .expect("the power supply's part");
    serde_json::to_value(doc).unwrap()
}

/// The sample's copy of the power supply part's manufacturer, as JSON.
fn psu_manufacturer() -> serde_json::Value {
    let item = psu_part()["manufacturer"]["item"].clone();
    let (_, _, lib) = common::sample_records();
    let doc = lib
        .iter()
        .find(|d| matches!(d, LibDocument::Manufacturer(m) if json!(m.item_id.as_uuid().to_string()) == item))
        .expect("the part's manufacturer");
    serde_json::to_value(doc).unwrap()
}

/// A document of the sample moved into the library `lib` (the registry recomputes the copy ID and hash on write).
fn into_library(mut v: serde_json::Value, lib: Uuid) -> LibDocument {
    v["library_id"] = json!(lib.to_string());
    if v.get("manufacturer").is_some() {
        v["manufacturer"]["library"] = json!(lib.to_string());
        v["manufacturer"]["version_req"] = json!("*");
    }
    LibDocument::from_value(v).expect("a library document")
}

/// A user library "Fixture parts" holding EW-GEN-PSU-1 with its manufacturer: its ID and the library's tiers.
fn fixture_library(name: &str) -> (Dir, LibraryTiers, Uuid) {
    let root = Dir(ew_testdir::fresh_dir(name));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(
            &t.user.join("fixture.ewlib"),
            "fixture",
            "Fixture parts",
            "CC0-1.0",
        )
        .unwrap();
    reg.write(
        lib,
        &[
            into_library(psu_part(), lib),
            into_library(psu_manufacturer(), lib),
        ],
    )
    .unwrap();
    (root, t, lib)
}

fn row<'t>(t: &'t NavTable, role: NavRole, label: &str) -> &'t NavNode {
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .unwrap_or_else(|| {
            let labels: Vec<&str> = t
                .nodes
                .iter()
                .filter(|n| n.role == role)
                .map(|n| n.label.as_str())
                .collect();
            panic!("no {role:?} row {label} in {labels:?}")
        })
}

fn cell<'t>(t: &'t NavTable, n: &'t NavNode, key: &str) -> &'t str {
    let i = t.columns.iter().position(|c| c.key == key).unwrap();
    &n.cells[i]
}

/// UI-4: without libraries (the compiled-in ones only), the sample's symbols are current; the compiled-in libraries
/// stand under "System", read-only. Tiers and the group of the project's copies are words.
#[test]
fn the_sample_s_copies_without_libraries() {
    let s = sample(None);
    let t = s.navigator_table(NavKind::Libraries);
    let system = row(&t, NavRole::Tier, "System");
    assert_eq!(system.key, "tier:system");
    let lib = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Library)
        .expect("a compiled-in library");
    assert_eq!(cell(&t, lib, "tier"), "System");
    assert_eq!(cell(&t, lib, "access"), "Read-only");
    assert_eq!(cell(&t, lib, "kind"), "Library");
    assert!(lib.key.starts_with("tier:system/library:"));
    let group = row(&t, NavRole::ProjectCopies, "In this project");
    let lamp = row(&t, NavRole::LibCopy, "Lamp");
    assert_eq!(t.nodes[lamp.parent.unwrap() as usize].key, group.key);
    assert!(lamp.badges.is_empty());
    assert_eq!(cell(&t, lamp, "state"), "Current");
    assert_eq!(cell(&t, lamp, "kind"), "Symbol");
    assert!(!cell(&t, lamp, "version").is_empty(), "{lamp:?}");
    assert!(!cell(&t, lamp, "used_by").is_empty(), "{lamp:?}");
    // The copy is listed by its name in words, not its key
    let copy = row(&t, NavRole::LibCopy, "Title block QET A4");
    assert_eq!(copy.badges, ["source_missing"]);
    assert_eq!(cell(&t, copy, "state"), "Only in this project");
    assert_eq!(cell(&t, copy, "kind"), "Form");
    assert!(
        copy.facets
            .contains(&("state".to_owned(), "source_missing".to_owned()))
    );
    // The copy's detail names it in words, its key secondary, with its kind, version and state.
    let detail = s.library_copy_detail(&copy.key).expect("a copy row's detail");
    assert_eq!(detail.name, "Title block QET A4");
    assert_eq!(detail.kind, "Form");
    assert_eq!(detail.key, "ew.titleblock.qet-a4");
    assert_eq!(detail.state.to_string(), "Only in this project");
    assert!(!detail.version.is_empty());
    assert_eq!(
        s.library_copy_detail(&system.key),
        None,
        "a tier row has no copy detail"
    );
    // No copy row is listed by an `ew.` key; a function definition reads as its kind.
    for n in t.nodes.iter().filter(|n| n.role == NavRole::LibCopy) {
        assert!(!n.label.starts_with("ew."), "{}", n.label);
    }
    row(&t, NavRole::LibCopy, "Black box");
    // A copy row stands for the copy and the devices that use it (marking it selects them).
    assert!(lamp.records.len() > 1);
}

/// UI-4: the power supply's part assigned to -101G1 from a fixture user library is current; once the library holds
/// its next patch version the copy has an update available. The library stands under "User", writable.
#[test]
fn a_fixture_library_offers_an_update() {
    let (_root, t, lib) = fixture_library("ew-api-ui4");
    let mut s = sample(Some(t));
    let item: Uuid = psu_part()["item_id"].as_str().unwrap().parse().unwrap();
    let reference: ew_schema::LibRef = serde_json::from_value(
        json!({"library": lib.to_string(), "item": item.to_string(), "version_req": "*"}),
    )
    .unwrap();
    let g1 = s.resolve_reference("tag:-101G1").unwrap();
    let held: Uuid = psu_part()["id"].as_str().unwrap().parse().unwrap();
    let cmd = s
        .assign_part(
            &[g1],
            &reference,
            ew_api::AssignMode::Replace { from: held },
            ew_units::Dec3::ONE,
        )
        .unwrap();
    s.execute(&cmd).unwrap();
    let table = s.navigator_table(NavKind::Libraries);
    row(&table, NavRole::Tier, "User");
    let library = row(&table, NavRole::Library, "Fixture parts");
    assert_eq!(cell(&table, library, "access"), "Writable");
    assert_eq!(cell(&table, library, "tier"), "User");
    let fixture_copy = |t: &NavTable| -> NavNode {
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::LibCopy && n.label == "EW-GEN-PSU-1" && cell(t, n, "used_by") == "-101G1")
            .cloned()
            .expect("the fixture library's copy, used by -101G1")
    };
    let copy = fixture_copy(&table);
    assert!(copy.badges.is_empty());
    assert_eq!(cell(&table, &copy, "state"), "Current");
    // The library's next version: a description change.
    let mut next = serde_json::to_value(s.libraries().resolve(&reference).unwrap()).unwrap();
    // Its next patch version.
    let held = next["version"].as_str().expect("a version").to_owned();
    let (major_minor, patch) = held.rsplit_once('.').expect("a three-part version");
    let bumped = format!("{major_minor}.{}", patch.parse::<u32>().unwrap() + 1);
    next["version"] = json!(bumped);
    next["description"] = json!("Power supply unit, AC to DC, 24 V");
    s.libraries_mut()
        .write(lib, &[LibDocument::from_value(next).unwrap()])
        .unwrap();
    let table = s.navigator_table(NavKind::Libraries);
    let copy = fixture_copy(&table);
    assert_eq!(copy.badges, ["update_available"]);
    assert_eq!(cell(&table, &copy, "state"), format!("Update available ({bumped})"));

    assert!(
        copy.facets
            .contains(&("state".to_owned(), "update_available".to_owned()))
    );
}

fn free_point() -> ew_units::Pt2 {
    let g = ew_units::Grid::QET_10PX;
    ew_units::Pt2::new(g.at(20).unwrap(), g.at(20).unwrap())
}

/// UI-1 (data): the PSU macro (the sample's breaker -101F1, the power supply -101G1 it feeds and their two wires) saved
/// into a user library is listed under "User" with its kind, version, library and device count; the detail lists its
/// two devices, has no thumbnail and names the macro by reference. A new instance ID is the same twice and moves on
/// after an insertion.
#[test]
fn the_macros_navigator_lists_a_saved_macro_and_its_detail() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-ui1"));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    drop(reg);
    let mut s = sample(Some(t));
    assert!(s.navigator_table(NavKind::Macros).nodes.is_empty());
    let page = s.page_tables("101").unwrap();
    let devices = [
        s.resolve_reference("-101F1").unwrap(),
        s.resolve_reference("-101G1").unwrap(),
    ];
    let functions: Vec<Uuid> = page
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .flat_map(|r| r.functions.clone())
        .collect();
    let inside = |e: &ew_api::output::TableEnd| {
        matches!(e, ew_api::output::TableEnd::Cp { function, .. } if functions.contains(function))
    };
    let ids: Vec<Uuid> = page
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .map(|r| r.id)
        .chain(
            page.connections
                .iter()
                .filter(|c| inside(&c.a) && inside(&c.b))
                .map(|c| c.id),
        )
        .collect();
    let spec = MacroSpec {
        key: "psu-24vdc".into(),
        name: MlText::neutral("PSU 24 V DC"),
        description: MlText::neutral("Power supply with its input breaker"),
        scope: MacroScope::Window(ids),
        origin: None,
        keep_tags: Vec::new(),
        variables: Vec::new(),
        groups: Vec::new(),
        value_sets: Vec::new(),
    };
    let saved = s.save_macro(lib, &spec).unwrap();
    let table = s.navigator_table(NavKind::Macros);
    let tier = row(&table, NavRole::Tier, "User");
    let m = row(&table, NavRole::Macro, "PSU 24 V DC");
    assert_eq!(table.nodes[m.parent.unwrap() as usize].key, tier.key);
    assert_eq!(cell(&table, m, "kind"), "Window");
    assert_eq!(cell(&table, m, "version"), "1.0.0");
    assert_eq!(cell(&table, m, "library"), "Mine");
    assert_eq!(cell(&table, m, "devices"), "2");
    assert!(m.facets.contains(&("kind".to_owned(), "window".to_owned())));
    let detail = s.macro_detail(&m.key).expect("the detail");
    assert_eq!(detail.name, "PSU 24 V DC");
    assert_eq!(detail.description, "Power supply with its input breaker");
    assert_eq!(detail.library, "Mine");
    assert_eq!(detail.thumbnail, None);
    assert_eq!(detail.devices.len(), 2, "{:?}", detail.devices);
    assert!(
        detail.devices.iter().any(|d| d.starts_with("-G")),
        "{:?}",
        detail.devices
    );
    assert!(detail.devices.iter().filter(|d| d.starts_with("-F")).count() == 1);
    assert_eq!(detail.copy, saved.copy);
    let reference = detail.reference.clone().expect("installed");
    assert_eq!(reference.item, saved.reference.item);
    assert!(detail.extent.is_some());
    // The thumbnail draws the macro on a blank page of a throwaway copy; the project is not touched.
    let pages = s.query().pages().iter().count();
    let thumb = s.macro_thumbnail(&m.key).expect("a thumbnail");
    assert_eq!(
        s.query().pages().iter().count(),
        pages,
        "the session is not touched"
    );
    let [x0, y0, x1, y1] = thumb.region;
    assert!(x0 < x1 && y0 < y1, "{:?}", thumb.region);
    assert!(
        x1 > 0 && y1 > 0 && x0 < thumb.sheet.0 && y0 < thumb.sheet.1,
        "{:?} on {:?}",
        thumb.region,
        thumb.sheet
    );
    assert!(
        !thumb.content.entity_bounds().is_empty(),
        "the macro's records are drawn"
    );
    assert!(s.macro_thumbnail("tier:user/macro:nonsense").is_none());
    assert_eq!(s.macro_detail("tier:user/macro:nonsense"), None);

    // The instance: deterministic, and the next one after an insertion differs.
    let first = s.next_macro_instance(saved.copy);
    assert_eq!(first, s.next_macro_instance(saved.copy));
    let p101 = s.page_tables("101").unwrap().page;
    let q = s.query();
    let page = serde_json::to_value(q.pages().get(ew_ids::Id::from_uuid(p101)).unwrap().as_ref()).unwrap();
    drop(q);
    let mut params = json!({"number": 106, "title": "Power Supply 6"});
    if let Some(section) = page["folio"].get("section") {
        params["section"] = section.clone();
    }
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params,
    })
    .unwrap();
    let p106 = s.page_tables("106").unwrap().page;
    let cmd = s
        .insert_macro(
            &reference,
            &InsertAt {
                page: p106,
                at: Some(free_point()),
                orient: ew_units::Orient2::R0,
            },
            &MacroValues::default(),
            first,
        )
        .unwrap();
    let preview = s.preview(&cmd).unwrap();
    s.commit(preview).unwrap();
    let second = s.next_macro_instance(saved.copy);
    assert_ne!(first, second);
    // The project now holds the macro's copy, which the installed row names: no "In this project" row for it.
    let table = s.navigator_table(NavKind::Macros);
    assert!(!table.nodes.iter().any(|n| n.role == NavRole::ProjectCopies));
    let m = row(&table, NavRole::Macro, "PSU 24 V DC");
    assert!(m.badges.is_empty(), "a current copy has no badge: {:?}", m.badges);
    assert!(m.facets.contains(&("state".to_owned(), "current".to_owned())));

    // Re-saving the macro with a new description bumps its version; the held copy then reads Update
    // available in both navigators, and Update… plans the macro copy. (Equal content keeps the hash: no update.)
    let spec = MacroSpec {
        description: MlText::neutral("Power supply, its input breaker"),

        ..spec
    };
    let resaved = s.save_macro(lib, &spec).unwrap();
    assert_eq!(resaved.version, "1.1.0");
    let table = s.navigator_table(NavKind::Macros);
    let m = row(&table, NavRole::Macro, "PSU 24 V DC");
    assert_eq!(m.badges, ["update_available"]);
    let libs = s.navigator_table(NavKind::Libraries);
    let copy = row(&libs, NavRole::LibCopy, "PSU 24 V DC");
    assert_eq!(cell(&libs, copy, "kind"), "Macro");
    assert_eq!(cell(&libs, copy, "state"), "Update available (1.1.0)");
    assert!(
        !cell(&libs, copy, "used_by").is_empty(),
        "the inserted devices use the macro"
    );
    // The thumbnail of the new version draws although the project holds the old one.
    assert!(s.macro_thumbnail(&m.key).is_some(), "a thumbnail of 1.1.0");
    let plan = s.update_from_library(&[saved.copy], &[]).expect("an update plan");
    assert_eq!(plan.copies.len(), 1);
    assert_eq!(plan.copies[0].to, "1.1.0");
}
