// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part taken out of My parts (doc 07 §2.5): the part editor of a part of My parts offers Remove from my
//! parts library; the next project no longer offers the part, and the project that uses it keeps its copy, which its
//! device keeps and the toast names.

use crate::common::{Tmp, cursor, done, grid_point, refused, toast};

use ew_viewmodel::dialogs::{DialogAnswer, spec};
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::{Value, json};

/// A shell in the window's mode with the libraries `tiers`, on a new project `name` from the built-in template with a
/// schematic page 101 shown.
fn project(tmp: &Tmp, tiers: LibraryTiers, name: &str) -> ShellModel {
    let config = tmp.config().with_libraries(Some(tiers));
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
    done(&mut s, "parts.new", json!({"plc": "module"}));
    for (field, value) in [
        ("manufacturer", "Example"),
        ("part_number", "DI-4"),
        ("plc.kind", "di"),
        ("plc.count", "4"),
        ("plc.pin_of", "{n+1}"),
    ] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
    // A device of it on the page.
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "DI-4"}));
    for k in [(20, 20), (32, 28)] {
        let p = grid_point(&s, k);
        cursor(&mut s, p);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "ui.dismiss", json!({}));
    // Parts offers its removal; it asks first, and its Remove takes the part out.
    done(&mut s, "parts.open", json!({"part": "DI-4"}));
    done(&mut s, "parts.remove", json!({}));
    let danger = |s: &ShellModel| {
        let dialog = s.dialog().cloned().expect("a dialog");
        let b = spec(&dialog, s).danger.expect("its danger button");
        let DialogAnswer::Action(action, params) = b.answer else {
            panic!("an action: {:?}", b.answer);
        };
        (b.label, action, params)
    };
    let (label, action, params) = danger(&s);
    assert_eq!((label.as_str(), action), ("Remove", "parts.remove"));
    done(&mut s, action, params);
    assert!(s.dialog().is_none(), "the confirmation gives way");
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
    // Removing it again is refused in words: the project's copy is no part of My parts now.
    done(&mut s, "parts.open", json!({"part": "DI-4"}));
    assert_eq!(refused(&mut s, "parts.remove", json!({})), "reason-part-not-own");
    s.shutdown();
    drop(s);
    // The next project, in the same person's folders, no longer finds it.
    let mut next = project(&tmp, mine, "q.ew");
    let numbers = offered(&mut next);
    assert!(!numbers.iter().any(|n| n == "DI-4"), "{numbers:?}");
    next.shutdown();
}
