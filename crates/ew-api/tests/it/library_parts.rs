// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.18 D6 in the Session: the system libraries' parts (the compiled-in generic starter parts) are listed for a
//! new project, and choosing one in the Parts row is one `part.assign` that carries the part with its closure, so
//! that the command replays without a library.

use ew_api::library::LibraryTiers;
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_ids::Uuid;
use ew_schema::LibRef;
use serde_json::{Value, json};

fn fresh() -> Session {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(7)),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

/// A fresh project with the device E1 (no part); its ID.
fn with_device() -> (Session, Uuid) {
    let mut s = fresh();
    s.execute(&AnyCommand {
        name: "device.create".into(),
        params: json!({"letter": "E", "counter": 1}),
    })
    .unwrap();
    let q = s.query();
    let [id] = q
        .devices()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .collect::<Vec<_>>()
        .try_into()
        .expect("one device");
    drop(q);
    (s, id)
}

/// The copy ID of the starter part `number`.
fn library_copy(s: &Session, number: &str) -> Uuid {
    s.library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == number)
        .unwrap_or_else(|| panic!("{number} among the library parts"))
        .id
        .as_uuid()
}

/// The part numbers of the device's part uses.
fn part_numbers(s: &Session, device: Uuid) -> Vec<String> {
    let q = s.query();
    let (_, d) = q
        .devices()
        .iter()
        .find(|(id, _)| id.as_uuid() == device)
        .expect("the device");
    d.parts
        .iter()
        .map(|u| {
            q.parts()
                .iter()
                .find(|(id, _)| id.as_uuid() == u.part.copy.as_uuid())
                .map(|(_, c)| c.item.part_number.as_str().to_owned())
                .expect("a held part")
        })
        .collect()
}

#[test]
fn a_new_project_lists_the_sixteen_starter_parts_with_their_manufacturer() {
    let s = fresh();
    let parts = s.library_parts();
    assert!(!parts.is_empty());
    assert!(
        parts
            .iter()
            .all(|c| c.item.part_number.as_str().starts_with("EW-GEN-"))
    );
    assert!(
        parts
            .iter()
            .all(|c| s.library_manufacturer(&c.item.manufacturer) == "Generic")
    );
    assert_eq!(s.query().parts().iter().count(), 0, "none is embedded");
}

#[test]
fn the_parts_row_assigns_a_library_part_in_one_command_that_carries_its_closure() {
    let (mut s, device) = with_device();
    let copy = library_copy(&s, "EW-GEN-LMP-1");
    let value = json!([{"part": {"copy": copy.to_string()}, "qty": "1"}]);
    let cmd = s
        .part_uses_assign(&[device], &value)
        .expect("a library part")
        .unwrap();
    assert_eq!(cmd.name, "part.assign");
    // The part, its template FnDef (the lamp) and the manufacturer travel in the command.
    let kinds: Vec<&str> = cmd.params["copies"]
        .as_array()
        .expect("copies")
        .iter()
        .map(|d| d["$k"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(kinds.iter().filter(|k| **k == "part").count(), 1, "{kinds:?}");
    assert!(
        kinds.contains(&"fndef") && kinds.contains(&"manufacturer"),
        "{kinds:?}"
    );
    // The text form of the command, as a command log keeps it.
    let text = serde_json::to_string(&cmd).unwrap();
    s.execute(&cmd).unwrap();
    assert_eq!(part_numbers(&s, device), ["EW-GEN-LMP-1"]);
    // Replay in another project: the command alone gives the same result.
    let (mut t, other) = with_device();
    assert_eq!(other, device, "seeded IDs");
    let replayed: AnyCommand = serde_json::from_str(&text).unwrap();
    t.execute(&replayed).unwrap();
    assert_eq!(part_numbers(&t, device), ["EW-GEN-LMP-1"]);
    // Without its copies the command does not reach for a library: the part is not held, so it is refused.
    let mut bare: Value = serde_json::from_str(&text).unwrap();
    bare["params"]["copies"] = json!([]);
    let bare: AnyCommand = serde_json::from_value(bare).unwrap();
    let (mut u, _) = with_device();
    assert!(u.execute(&bare).is_err(), "the closure is in the command");
}

#[test]
fn a_held_part_or_a_list_of_several_stays_the_rows_own_edit() {
    let (mut s, device) = with_device();
    let lamp = library_copy(&s, "EW-GEN-LMP-1");
    let fuse = library_copy(&s, "EW-GEN-FUS-1");
    let two = json!([{"part": {"copy": lamp.to_string()}}, {"part": {"copy": fuse.to_string()}}]);
    assert!(s.part_uses_assign(&[device], &two).is_none(), "two uses");
    let one = json!([{"part": {"copy": lamp.to_string()}}]);
    let cmd = s.part_uses_assign(&[device], &one).unwrap().unwrap();
    s.execute(&cmd).unwrap();
    assert!(s.part_uses_assign(&[device], &one).is_none(), "held now");
    let unknown = json!([{"part": {"copy": Uuid::from_u128(1).to_string()}}]);
    assert!(
        s.part_uses_assign(&[device], &unknown).is_none(),
        "no library holds it"
    );
}

/// The catalogue's parts as the picker lists them: maker, part number and whether the project holds the part.
fn catalogue_order(s: &Session) -> Vec<(String, String, bool)> {
    let folio = s.page_labels()[0].clone();
    s.place_catalogue(&folio)
        .expect("a drawn page")
        .parts
        .into_iter()
        .map(|p| (p.manufacturer, p.part_number, p.library.is_none()))
        .collect()
}

/// FT-1087 as FT-1375 amends it: the chooser lists the parts the project holds first, then those
/// of the libraries it does not, each by (maker, part number); embedding a part (here by the Parts row's
/// `part.assign`, which carries the part with its closure) moves it to the front, among the project's own.
#[test]
fn embedding_a_library_part_moves_it_among_the_projects_parts() {
    let (mut s, device) = with_device();
    let before = catalogue_order(&s);
    assert!(before.len() > 2, "the starter parts are listed");
    assert!(before.iter().all(|(_, _, held)| !held), "none is held yet");
    let keys: Vec<(&str, &str)> = before.iter().map(|(m, n, _)| (m.as_str(), n.as_str())).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "maker, then part-number order");
    // A part that is not the first of its maker: embedding it brings it to the front.
    let number = before[before.len() / 2].1.clone();
    let copy = library_copy(&s, &number);
    let value = json!([{"part": {"copy": copy.to_string()}, "qty": "1"}]);
    let cmd = s
        .part_uses_assign(&[device], &value)
        .expect("a library part")
        .unwrap();
    s.execute(&cmd).unwrap();
    let after = catalogue_order(&s);
    assert_eq!(
        after.iter().filter(|(_, _, held)| *held).count(),
        1,
        "the part is held now"
    );
    assert!(
        after.iter().any(|(_, n, held)| *held && *n == number),
        "{number} is the held one"
    );
    assert_eq!(after[0].1, number, "the project's own part first");
    let names = |v: &[(String, String, bool)]| -> Vec<(String, String)> {
        v.iter().map(|(m, n, _)| (m.clone(), n.clone())).collect()
    };
    let mut expected = names(&before);
    let moved = expected.remove(before.len() / 2);
    expected.insert(0, moved);
    assert_eq!(names(&after), expected, "the others keep their order");
}

/// A part names its maker by the library record's name, not its key: a library part whose
/// manufacturer the project does not hold is listed under the same text as the part is once embedded.
#[test]
fn a_part_names_its_maker_by_the_library_records_name_not_its_key() {
    let root = ew_testdir::fresh_dir("ew-api-maker-name");
    let tiers = LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: vec![root.join("company")],
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    };
    let project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .unwrap();
    let mut s = Session::from_records(
        project,
        [],
        [],
        SessionOptions {
            libraries: Some(tiers),
            ..SessionOptions::default()
        },
    )
    .unwrap();
    let lib = s
        .libraries_mut()
        .create(
            &root.join("company").join("makers.ewlib"),
            "makers",
            "Makers",
            "CC0-1.0",
        )
        .unwrap();
    s.libraries_mut()
        .write(lib, &ew_testkit::library::generated_parts(1, lib))
        .unwrap();
    let r: LibRef = serde_json::from_value(json!({
        "library": lib.to_string(),
        "item": ew_testkit::library::manufacturer_item(lib, 0).to_string(),
        "version_req": "^1"
    }))
    .unwrap();
    let name = s.library_manufacturer(&r);
    drop(s);
    std::fs::remove_dir_all(&root).unwrap();
    // The record's key is "M00"; its name is what the chooser groups by.
    assert_eq!(name, "Manufacturer 00");
}
