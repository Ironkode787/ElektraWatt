// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part taken out of My parts (FT-1766, doc 07 §2.5): the part editor of a part of My parts offers Remove from my
//! parts library; the next project no longer offers the part, and the project that uses it keeps its copy, which its
//! device keeps and the toast names.

use crate::common::{Tmp, cursor, done, grid_point, refused, toast};

use ew_viewmodel::dialogs::{DialogAnswer, spec};
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A shell in the window's mode with the libraries `tiers`, on a new project `name` from the built-in template with a
/// schematic page 101 shown.
fn project(tmp: &Tmp, tiers: LibraryTiers, name: &str) -> ShellModel {
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test").with_libraries(Some(tiers));
    let mut s = ShellModel::new(Mode::Gui, config);
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join(name), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Cards"}),
    );
    s
}

/// The part numbers the device browser of the page shown lists, opened for it.
fn offered(s: &mut ShellModel) -> Vec<String> {
    done(s, "tool.place", json!({}));
    let numbers = s
        .picker()
        .expect("the device browser")
        .catalogue
        .parts
        .iter()
        .map(|p| p.part_number.clone())
        .collect();
    done(s, "ui.dismiss", json!({}));
    numbers
}

#[test]
fn a_part_removed_from_my_parts_stays_in_the_project_that_uses_it() {
    let tmp = Tmp::new("my-parts-remove");
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = project(&tmp, mine.clone(), "p.ew");
    done(&mut s, "part.new", json!({"plc": "module"}));
    for (field, value) in [
        ("manufacturer", "Example"),
        ("part_number", "DI-4"),
        ("plc.kind", "di"),
        ("plc.count", "4"),
        ("plc.pin_of", "{n+1}"),
    ] {
        done(&mut s, "part.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "part.save", json!({}));
    // A device of it on the page.
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "DI-4"}));
    for k in [(20, 20), (32, 28)] {
        let p = grid_point(&s, k);
        cursor(&mut s, p);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "ui.dismiss", json!({}));
    // Its part editor offers the removal, apart from Save.
    done(&mut s, "part.edit", json!({"part": "DI-4"}));
    let dialog = s.dialog().cloned().expect("the part editor");
    let danger = spec(&dialog, &s).danger.expect("Remove from my parts library");
    assert_eq!(danger.label, "Remove from my parts library");
    let DialogAnswer::Action(action, params) = danger.answer else {
        panic!("an action: {:?}", danger.answer);
    };
    assert_eq!(action, "part.remove_own");
    done(&mut s, action, params.clone());
    assert!(s.dialog().is_none(), "the part editor gives way");
    let said = toast(&s);
    assert!(
        said.starts_with("Removed DI-4 from your parts library")
            && said.contains("This project keeps its copy for -"),
        "{said}"
    );
    // The project keeps the copy and its device keeps it.
    assert!(offered(&mut s).iter().any(|n| n == "DI-4"), "held by the project");
    let world = s.world_for_tests().expect("a project");
    let lines: Vec<Value> = ew_commands::rows::model_lines(&world.model)
        .expect("every record has a JSON form")
        .into_iter()
        .filter_map(|(_, line)| serde_json::from_str(&line).ok())
        .collect();
    let copy = lines
        .iter()
        .find(|v| v["part_number"] == "DI-4")
        .and_then(|v| v["id"].as_str())
        .expect("the project's copy")
        .to_owned();
    assert!(
        lines.iter().any(|v| v["parts"].to_string().contains(&copy)),
        "the device keeps its part"
    );
    // Removing it again is refused in words.
    assert_eq!(
        refused(&mut s, "part.remove_own", json!({"part": "DI-4"})),
        "reason-part-not-own"
    );
    s.shutdown();
    drop(s);
    // The next project, in the same person's folders, no longer finds it.
    let mut next = project(&tmp, mine, "q.ew");
    let numbers = offered(&mut next);
    assert!(!numbers.iter().any(|n| n == "DI-4"), "{numbers:?}");
    next.shutdown();
}
