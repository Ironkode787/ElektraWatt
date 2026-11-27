// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, the one place for parts (doc 07 §5): where a part lives (doc 07 §5.8), what the list finds and that its row
//! lacks something, and Keep in my parts, on a project with a user folder and parts made from nothing.

use crate::common;

use ew_api::parts::{Lives, PartScope};
use ew_api::{AnyCommand, Session, SessionOptions};
use ew_ids::Uuid;

fn run(s: &mut Session, cmd: &AnyCommand) {
    match s.execute(cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => {
            let p = s.preview(cmd).expect("a preview");
            s.commit(p).expect("committed");
        }
        other => {
            other.expect("committed");
        }
    }
}

/// A new project whose person has a user folder under `root`.
fn project(root: &std::path::Path) -> Session {
    Session::new_project(
        ew_api::ProjectTemplate::Builtin,
        ew_api::NewProjectKind::Schematic,
        SessionOptions {
            libraries: Some(common::tiers(root)),
            ..SessionOptions::default()
        },
    )
    .expect("a new project")
}

/// The copy the project holds of the part numbered `number`.
fn copy_of(s: &Session, number: &str) -> Uuid {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("the project holds {number}"))
}

/// A terminal made from nothing (no starter part copied) with its maker and number only, kept to the project
/// (`to_library` off) or saved to My parts.
fn terminal(s: &mut Session, number: &str, to_library: bool) {
    let mut sheet = s.new_part_draft().expect("a new part");
    sheet.to_library = to_library;
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", number),
        ("kind", "terminal"),
    ] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    assert_eq!(s.sheet_lives(&sheet), Lives::New);
    let made = s.new_part_command(&sheet).expect("the new part's command");
    run(s, &made.command);
}

#[test]
fn a_part_lives_in_my_parts_in_the_project_alone_or_in_the_starter() {
    let tmp = ew_testdir::Tmp::new("parts-place-lives");
    let mut s = project(&tmp.0);
    terminal(&mut s, "FT-4", true);
    terminal(&mut s, "DL-2.5", false);
    let mine = s.part_draft(copy_of(&s, "FT-4")).expect("its sheet");
    assert_eq!(s.sheet_lives(&mine), Lives::Mine);
    let here = s.part_draft(copy_of(&s, "DL-2.5")).expect("its sheet");
    assert_eq!(s.sheet_lives(&here), Lives::OnlyHere);
    // The list finds both in the project, the one of My parts under My parts, and a starter part as not his.
    let numbers = |scope| -> Vec<String> {
        s.parts_find("", scope, Some("terminal"), false, None, 500)
            .into_iter()
            .map(|f| f.offer.part_number)
            .collect()
    };
    let project_parts = numbers(PartScope::Project);
    assert!(project_parts.contains(&"FT-4".to_owned()) && project_parts.contains(&"DL-2.5".to_owned()));
    let my_parts = numbers(PartScope::Mine);
    assert!(my_parts.contains(&"FT-4".to_owned()), "{my_parts:?}");
    assert!(!my_parts.contains(&"DL-2.5".to_owned()), "{my_parts:?}");
    let starter = s
        .parts_find("EW-GEN-TRM-1", PartScope::All, None, false, None, 50)
        .into_iter()
        .find(|f| f.offer.part_number == "EW-GEN-TRM-1")
        .expect("the starter terminal");
    assert!(
        matches!(starter.lives, Lives::NotMine { starter: true, .. }),
        "{:?}",
        starter.lives
    );
    // A terminal of a maker and a number alone lacks its size, its clamps and how it mounts: its row says so.
    let ft4 = s
        .parts_find("FT-4", PartScope::Mine, None, true, None, 50)
        .into_iter()
        .find(|f| f.offer.part_number == "FT-4")
        .expect("found among the parts that lack something");
    assert!(ft4.lacks);
}

/// Keep in my parts (doc 07 §5.8): a part only in this project is written into My parts and the project's devices are
/// given that copy; with none using it, the project takes the copy by `library.embed`, and it lives in My parts.
#[test]
fn a_part_only_in_this_project_is_kept_in_my_parts() {
    let tmp = ew_testdir::Tmp::new("parts-place-keep");
    let mut s = project(&tmp.0);
    terminal(&mut s, "DL-2.5", false);
    let here = copy_of(&s, "DL-2.5");
    let keep = s.keep_in_my_parts(here).expect("kept");
    assert_eq!(keep.name, "part.keep");
    run(&mut s, &keep);
    let kept: Vec<Uuid> = s
        .query()
        .parts()
        .iter()
        .filter(|(_, c)| c.item.part_number.as_str() == "DL-2.5")
        .map(|(id, _)| id.as_uuid())
        .collect();
    assert_eq!(kept.len(), 2, "the project's own copy stays beside the kept one");
    let lives: Vec<Lives> = kept
        .iter()
        .map(|c| s.sheet_lives(&s.part_draft(*c).expect("a sheet")))
        .collect();
    assert!(
        lives.contains(&Lives::Mine) && lives.contains(&Lives::OnlyHere),
        "{lives:?}"
    );
    // Kept again: My parts has it already, said in words.
    assert!(s.keep_in_my_parts(here).is_err());
}

/// Ready for says what a part lacks only for a feature that reads parts of its kind (doc 07 §5.7), asked of every part
/// of the starter content: the output load of a PLC module or station alone, the coil of a valve with solenoids alone,
/// no footprint or rail of an accessory (a plug, a bridge), and no pins of a piece that has none (an air service unit).
#[test]
fn a_starter_part_lacks_nothing_for_a_feature_that_does_not_read_its_kind() {
    use ew_api::part_fields::{Feature, lacks};
    let s = common::new_project(1);
    for c in s.library_parts() {
        let p = &c.item;
        let number = p.part_number.as_str();
        for l in lacks(p) {
            let fields: Vec<&str> = l.fields.iter().map(|f| f.path).collect();
            match l.feature {
                Feature::OutputLoad => assert!(p.sections.plc.is_some(), "{number}: {fields:?}"),
                Feature::CoilCheck => assert!(
                    p.sections.fluid.as_ref().is_some_and(|f| !f.solenoids.is_empty()),
                    "{number}: {fields:?}"
                ),
                Feature::Footprint | Feature::RailFit => {
                    assert_ne!(p.kind, ew_schema::PartKind::Accessory, "{number}: {fields:?}");
                }
                Feature::Placing => {
                    assert!(
                        !fields.contains(&"function_template[].cps[]"),
                        "{number}: {fields:?}"
                    );
                }
                _ => {}
            }
        }
    }
}

/// A writable session on a copy of the sample whose person has a user folder under `tmp`.
fn sample_with_my_parts(tmp: &ew_testdir::Tmp) -> Session {
    let dir = tmp.sample("sample.ew");
    let mut store = ew_api::StoreOptions::new("0.0.0-test");
    store.state_dir = Some(tmp.0.join("state"));
    let options = SessionOptions {
        libraries: Some(common::tiers(&tmp.0)),
        ..SessionOptions::default()
    };
    Session::open(&dir, store, options, ew_api::Recovery::Keep)
        .expect("the copy opens")
        .0
}

/// Keep in my parts moves every use of the project's copy to the kept one with what it chose, in one undo step: -102F1
/// uses a part only in this project in its variant "B"; kept, it uses the kept copy in the same variant; undone, the
/// project's copy again.
#[test]
fn a_kept_part_keeps_its_devices_and_their_variants_and_undo_brings_them_back() {
    let tmp = ew_testdir::Tmp::new("parts-place-keep-used");
    let mut s = sample_with_my_parts(&tmp);
    let mut sheet = s.new_part_draft().expect("a new part");
    sheet.to_library = false;
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", "MCB-HERE"),
        ("kind", "device"),
    ] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    assert!(sheet.add_table_row("variants"));
    s.set_draft_field(&mut sheet, "variants[0].key", "B")
        .expect("its key");
    s.set_draft_field(&mut sheet, "variants[0].name", "B")
        .expect("its name");
    s.set_draft_field(&mut sheet, "variants[0].part_number", "MCB-HERE-B")
        .expect("its number");
    let made = s.new_part_command(&sheet).expect("the new part's command");
    run(&mut s, &made.command);
    let here = copy_of(&s, "MCB-HERE");
    let device = s.resolve_reference("tag:-102F1").expect("-102F1");
    let starter = {
        let q = s.query();
        q.devices()
            .get(ew_ids::Id::from_uuid(device))
            .and_then(|d| d.parts.first().map(|u| u.part.copy.as_uuid()))
            .expect("-102F1's part")
    };
    run(
        &mut s,
        &AnyCommand {
            name: "part.assign".into(),
            params: serde_json::json!({
                "devices": [device], "part": here, "mode": "replace", "from": starter, "variant": "B",
            }),
        },
    );
    let use_of = |s: &Session| {
        let q = s.query();
        let d = q
            .devices()
            .get(ew_ids::Id::from_uuid(device))
            .expect("the device")
            .clone();
        d.parts
            .iter()
            .map(|u| {
                (
                    u.part.copy.as_uuid(),
                    u.part.variant.as_ref().map(|v| v.as_str().to_owned()),
                )
            })
            .next()
            .expect("a part use")
    };
    assert_eq!(use_of(&s), (here, Some("B".to_owned())));
    let keep = s.keep_in_my_parts(here).expect("kept");
    assert_eq!(keep.name, "part.keep");
    run(&mut s, &keep);
    let kept = keep.params["to"]
        .as_str()
        .expect("the kept copy")
        .parse::<Uuid>()
        .expect("an ID");
    assert_ne!(kept, here);
    assert_eq!(
        use_of(&s),
        (kept, Some("B".to_owned())),
        "the device keeps its variant"
    );
    let lives = s.sheet_lives(&s.part_draft(kept).expect("its sheet"));
    assert_eq!(lives, Lives::Mine);
    s.undo(&ew_api::Origin::Gui).expect("undo");
    assert_eq!(use_of(&s), (here, Some("B".to_owned())), "one undo step");
}

/// A part of a library that is not installed (a colleague's parts, a library removed) is only in this project: his to
/// change and to keep (doc 07 §5.8). The sample's EW-GEN-MCB-1 is read as a copy of a library nobody has.
#[test]
fn a_part_of_a_library_that_is_not_installed_is_only_in_this_project() {
    let (project, records, mut lib) = common::sample_records();
    let gone = ew_ids::Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0000_0001);
    let mut copy_id = None;
    for doc in &mut lib {
        if let ew_schema::LibDocument::Part(c) = doc
            && c.item.part_number.as_str() == "EW-GEN-MCB-1"
        {
            c.library_id = ew_ids::Id::from_uuid(gone);
            copy_id = Some(c.id.as_uuid());
        }
    }
    let s = common::session_of(project, records, lib);
    let sheet = s
        .part_draft(copy_id.expect("the sample holds EW-GEN-MCB-1"))
        .expect("its sheet");
    assert_eq!(s.sheet_lives(&sheet), Lives::OnlyHere);
    assert!(sheet.own, "his to change");
}
