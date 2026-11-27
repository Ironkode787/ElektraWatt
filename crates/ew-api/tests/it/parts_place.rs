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
    assert_eq!(keep.name, "library.embed");
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
