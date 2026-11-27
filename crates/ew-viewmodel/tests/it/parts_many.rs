// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Many parts at once in Parts (doc 07 §5.9): his own terminals selected together show the fields they share, "mixed"
//! where they differ; a value typed goes to all of them and one Save writes them, the project's copies following in
//! one undo step after the card named them; a starter part among them is refused by name; Export as a table… writes
//! them, and Import… reads the table back as the same.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A shell on a new project whose person has a user folder under `tmp`.
fn shell(tmp: &Tmp) -> ShellModel {
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = ShellModel::new(Mode::Headless, tmp.config().with_libraries(Some(mine)));
    common::make_project(&mut s, &tmp.0.join("p.ew"), &[]);
    s
}

/// A terminal of his own, made on its sheet and saved: into My parts and this project.
fn terminal(s: &mut ShellModel, number: &str, height: &str) {
    done(s, "parts.new", json!({"start": "terminal"}));
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", number),
        ("mech.width", "6.2"),
        ("mech.height", height),
    ] {
        done(s, "parts.field", json!({"field": f, "value": v}));
    }
    done(s, "parts.save", json!({}));
}

/// The list's row of `number`, as `parts.open` names it.
fn row(s: &mut ShellModel, number: &str) -> String {
    done(s, "parts.find", json!({"text": "", "scope": "project"}));
    s.parts_place()
        .and_then(|p| p.found.iter().find(|f| f.offer.part_number == number))
        .map(ew_viewmodel::ew_api::parts::PartFound::param)
        .unwrap_or_else(|| panic!("{number} in the list"))
}

fn lines_with(s: &ShellModel, prefix: &str) -> Vec<String> {
    s.describe()
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_value_typed_into_two_of_his_parts_is_saved_into_both_in_one_step() {
    let tmp = Tmp::new("parts-many-vm");
    let mut s = shell(&tmp);
    done(&mut s, "parts.open", json!({}));
    terminal(&mut s, "FT-4", "60");
    terminal(&mut s, "FT-6", "70");
    let (a, b) = (row(&mut s, "FT-4"), row(&mut s, "FT-6"));
    done(&mut s, "parts.open", json!({"part": a}));
    done(&mut s, "parts.open", json!({"part": b, "add": true}));
    assert_eq!(lines_with(&s, "many "), ["many \"FT-4\" \"FT-6\""]);
    assert!(lines_with(&s, "field mech.width ").contains(&"field mech.width \"6.2\"".to_owned()));
    assert!(lines_with(&s, "field mech.height ").contains(&"field mech.height mixed".to_owned()));
    // Both have the same pins: the Connections table is shared, a cell each.
    assert!(
        !lines_with(&s, "field cp_pattern[0].csa_max").is_empty(),
        "{}",
        s.describe()
    );
    let undo_before = s.can_undo();
    done(
        &mut s,
        "parts.field",
        json!({"field": "mech.depth", "value": "45"}),
    );
    done(
        &mut s,
        "parts.field",
        json!({"field": "cp_pattern[0].csa_max", "value": "2.5"}),
    );
    done(&mut s, "parts.save", json!({}));
    // The project holds both: the card names the two parts before its Apply, which is one undo step.
    let card = s.properties().card().expect("the card of the project's copies");
    assert_eq!(card.preview.name, "library.update");
    done(&mut s, "preview.apply", json!({}));
    assert!(s.can_undo());
    for (number, height) in [("FT-4", "60"), ("FT-6", "70")] {
        let part = row(&mut s, number);
        done(&mut s, "parts.open", json!({"part": part}));
        let open = &s.parts_place().unwrap().sheet.as_ref().unwrap().sheet;
        assert_eq!(open.text("mech.depth"), "45", "{number}");
        assert_eq!(open.text("cp_pattern[0].csa_max"), "2.5", "{number}");
        assert_eq!(
            open.text("mech.height"),
            height,
            "{number}: mixed is never written"
        );
    }
    // One undo step takes both back.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.can_undo(), undo_before);
    s.shutdown();
}

#[test]
fn a_starter_part_among_the_selected_is_refused_by_name() {
    let tmp = Tmp::new("parts-many-starter");
    let mut s = shell(&tmp);
    done(&mut s, "parts.open", json!({}));
    terminal(&mut s, "FT-4", "60");
    let mine = row(&mut s, "FT-4");
    done(
        &mut s,
        "parts.find",
        json!({"text": "EW-GEN-TRM-1", "scope": "all"}),
    );
    let starter = s
        .parts_place()
        .and_then(|p| p.found.iter().find(|f| f.offer.part_number == "EW-GEN-TRM-1"))
        .map(ew_viewmodel::ew_api::parts::PartFound::param)
        .expect("the starter terminal");
    done(&mut s, "parts.open", json!({"part": mine}));
    done(&mut s, "parts.open", json!({"part": starter, "add": true}));
    assert_eq!(
        refused(
            &mut s,
            "parts.field",
            json!({"field": "mech.depth", "value": "45"})
        ),
        "reason-parts-many-not-yours"
    );
    s.shutdown();
}

/// A table exported from Parts and read back unchanged is the same, row for row.
#[test]
fn an_exported_table_imported_unchanged_is_the_same_for_every_row() {
    let tmp = Tmp::new("parts-many-export");
    let mut s = shell(&tmp);
    done(&mut s, "parts.open", json!({}));
    terminal(&mut s, "FT-4", "60");
    terminal(&mut s, "FT-6", "70");
    let (a, b) = (row(&mut s, "FT-4"), row(&mut s, "FT-6"));
    done(&mut s, "parts.open", json!({"part": a}));
    done(&mut s, "parts.open", json!({"part": b, "add": true}));
    for file in ["family.csv", "family.xlsx"] {
        let path = tmp.0.join(file);
        done(&mut s, "parts.export", json!({"path": path.to_string_lossy()}));
        done(&mut s, "parts.import", json!({"path": path.to_string_lossy()}));
        assert_eq!(
            lines_with(&s, "set \""),
            [format!("set \"{file}\" new 0 changed 0 same 2 refused 0")],
            "{}",
            s.describe()
        );
        done(&mut s, "parts.discard", json!({}));
        done(&mut s, "parts.open", json!({"part": a}));
        done(&mut s, "parts.open", json!({"part": b, "add": true}));
    }
    s.shutdown();
}
