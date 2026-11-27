// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shop's hole settings in Project properties ▸ Workshop (doc 12-3d §10.3, doc 12 §2.8): Fixing holes, Rail fixing,
//! Duct fixing, Web between holes and Measured from are rows a person sets, a value that cannot be refused in words; a
//! changed fixing resets the holes moved or left out that its new holes no longer match, on a rail and a duct of the
//! test's own, and says so before it is applied; Measured from changes the numbers of the hole table, not the holes.

use crate::common;

use ew_api::output::{RowExport, RowValue, row_export};
use ew_api::{AnyCommand, CmdError, Origin, Session};
use ew_ids::Id;
use ew_schema::{Carrier, MountingSurface, SurfaceKind};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

fn set(member: &str, value: Value) -> AnyCommand {
    AnyCommand {
        name: "project.settings.set".into(),
        params: json!({"member": format!("workshop.{member}"), "value": value}),
    }
}

fn run(s: &mut Session, cmd: &AnyCommand) -> Result<(), CmdError> {
    match s.execute(cmd) {
        Err(CmdError::NeedsPreview { .. }) => s.preview(cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .map(drop)
}

/// The refusal of `cmd` in words.
fn refusal(s: &mut Session, cmd: &AnyCommand) -> String {
    match run(s, cmd) {
        Err(CmdError::InvalidParams { text, .. }) => text.to_string(),
        other => panic!("{}: {other:?}", cmd.params),
    }
}

/// The project's row `settings.workshop.<path>`: its text, whether a person edits it and its chip.
fn row(s: &Session, path: &str) -> (String, bool, String) {
    let sheet = s.property_sheet(&[s.query().project().id.as_uuid()]);
    let key = format!("project/settings.workshop.{path}");
    let r = (sheet.sections.iter().flat_map(|x| x.rows.iter()))
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("the row {key}"));
    let text = match &r.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    };
    (text, r.route.is_editable(), common::chip(&r.provenance).1)
}

/// Every hole row stands at its default with the chip "project default" and is one a person sets; each set value is
/// stored, a value that cannot be is refused in words, and a fixing set back to its default is stored as none.
#[test]
fn the_shops_hole_settings_are_rows_a_person_sets() {
    let mut s = common::sample();
    for (path, default) in [
        ("fixing", "tapped"),
        ("rail_fixing.screw", "m5"),
        ("rail_fixing.pitch", "300 mm"),
        ("rail_fixing.from_end", "25 mm"),
        ("duct_fixing.screw", "m4"),
        ("duct_fixing.pitch", "300 mm"),
        ("duct_fixing.from_end", "25 mm"),
        ("min_web", "5 mm"),
        ("datum", "bottom_left"),
    ] {
        assert_eq!(
            row(&s, path),
            (default.to_owned(), true, "project default".to_owned()),
            "{path}"
        );
    }
    for (member, value) in [
        ("fixing", json!("through")),
        ("datum", json!("top_left")),
        ("min_web", json!(8 * MM)),
        ("rail_fixing.pitch", json!(200 * MM)),
        ("duct_fixing.screw", json!("m5")),
    ] {
        run(&mut s, &set(member, value)).unwrap_or_else(|e| panic!("{member}: {e:?}"));
    }
    let workshop = s.query().project().settings.workshop.clone();
    assert_eq!(workshop.fixing, ew_schema::FixingRule::Through);
    assert_eq!(workshop.datum, Some(ew_schema::Corner::TopLeft));
    assert_eq!(workshop.min_web, Some(ew_units::Nm(8 * MM)));
    let rail = workshop.rail_fixing.expect("the rail fixing set");
    assert_eq!(
        (rail.screw, rail.pitch.0, rail.from_end.0),
        (ew_schema::Screw::M5, 200 * MM, 25 * MM)
    );
    assert_eq!(row(&s, "rail_fixing.pitch").0, "200 mm");
    assert!(refusal(&mut s, &set("min_web", json!(0))).contains("0 mm is no web between holes"));
    assert!(
        refusal(&mut s, &set("rail_fixing.pitch", json!(0))).contains("is no distance between fixing holes")
    );
    assert!(
        refusal(&mut s, &set("rail_fixing.pitch", json!(5_000 * MM)))
            .contains("is longer than the longest rail"),
        "a pitch no rail of the project holds"
    );
    assert!(refusal(&mut s, &set("duct_fixing.from_end", json!(-MM))).contains("less than nothing"));
    for (member, value) in [("rail_fixing.screw", json!("m7")), ("datum", json!("middle"))] {
        assert!(!refusal(&mut s, &set(member, value)).is_empty(), "{member}");
    }
    run(&mut s, &set("rail_fixing.pitch", Value::Null)).expect("the pitch back to its default");
    assert_eq!(s.query().project().settings.workshop.rail_fixing, None);
}

/// The plate's surface.
fn plate(s: &Session) -> Id<MountingSurface> {
    let q = s.query();
    q.table::<MountingSurface>()
        .iter()
        .find(|(_, m)| m.kind == SurfaceKind::Plate)
        .map(|(id, _)| id)
        .expect("the plate")
}

/// A carrier of the test's own on the plate, 470 mm long at `v` mm (three fixing holes by default: 25, 235 and 445 mm
/// from its start), with its fixing hole 0 left out and 1 moved by 5 mm.
fn own_carrier(s: &mut Session, kind: &str, v: i64) -> Id<Carrier> {
    let params = match kind {
        "rail" => json!({"rail": {"profile": "ts35x7_5"}}),
        _ => json!({"duct": {"width": 40 * MM, "height": 60 * MM}}),
    };
    let add = AnyCommand {
        name: "layout.carrier.add".into(),
        params: json!({"surface": plate(s), "kind": kind, "path": [[10 * MM, v * MM], [480 * MM, v * MM]],
                       "params": params}),
    };
    let receipt = s.execute(&add).expect("the carrier");
    let carrier: Id<Carrier> = Id::from_uuid(receipt.outcome.primary.expect("the new carrier").1.as_uuid());
    for choice in [
        json!({"index": 0, "omit": true}),
        json!({"index": 1, "offset": [5 * MM, 0]}),
    ] {
        let mut params = choice;
        params["carrier"] = json!(carrier);
        run(
            s,
            &AnyCommand {
                name: "layout.hole_choice".into(),
                params,
            },
        )
        .expect("the choice");
    }
    carrier
}

/// The indices of the carrier's hole choices.
fn choices(s: &Session, c: Id<Carrier>) -> Vec<u16> {
    let q = s.query();
    let carrier = q.table::<Carrier>().get(c).cloned().expect("the carrier");
    carrier.hole_choices.iter().map(|h| h.index).collect()
}

/// A 200 mm rail pitch moves the second of the own rail's three holes (235 → 165 mm), not the first: the choice that
/// moved hole 1 is reset, the one that left out hole 0 stays, and the card's preview says so before it is applied;
/// the duct keeps its choices, and Undo brings the rail's back. A new screw moves no hole and keeps every choice; a
/// duct's ends at 40 mm move its first hole (25 → 40 mm) and leave its middle one at 235 mm.
#[test]
fn a_changed_fixing_resets_the_holes_moved_on_its_rails_and_says_so() {
    let mut s = common::sample();
    let rail = own_carrier(&mut s, "rail", 40);
    let duct = own_carrier(&mut s, "duct", 15);
    let pitch = set("rail_fixing.pitch", json!(200 * MM));
    let preview = s.preview(&pitch).expect("a preview");
    let notices: Vec<String> = preview
        .outcome()
        .notices
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        notices
            .iter()
            .any(|n| n.starts_with("1 hole moved or left out on rail ") && n.contains(" is reset")),
        "{notices:?}"
    );
    let before = s.history().cursor();
    s.commit(preview).expect("applied");
    assert_eq!(s.history().cursor(), before + 1, "one undo step");
    assert_eq!(choices(&s, rail), [0], "the hole left out stays where it was");
    assert_eq!(choices(&s, duct), [0, 1], "a duct is fixed by its own setting");
    s.undo(&Origin::Gui).expect("undo");
    assert_eq!(choices(&s, rail), [0, 1]);
    run(&mut s, &set("rail_fixing.screw", json!("m6"))).expect("a new screw");
    assert_eq!(choices(&s, rail), [0, 1], "a new screw moves no hole");
    run(&mut s, &set("duct_fixing.from_end", json!(40 * MM))).expect("the duct's ends");
    assert_eq!(choices(&s, duct), [1], "the middle hole stays where it was");
}

/// Measured from changes where the hole table measures from, not where a hole is: the plate's holes and their numbers
/// stay, the table's X and Y change.
#[test]
fn measured_from_changes_the_numbers_not_the_holes() {
    let mut s = common::sample();
    let surface = plate(&s);
    let holes = |s: &Session| {
        s.query()
            .get::<ew_derive::layout::SurfaceFeatures>(surface)
            .as_ref()
            .clone()
    };
    let table = |s: &Session| {
        row_export(&s.query(), &RowExport::Kind("hole_table".to_owned()), None)
            .expect("the hole table")
            .csv
    };
    let (was, was_table) = (holes(&s), table(&s));
    assert!(!was.is_empty(), "the plate has holes");
    run(&mut s, &set("datum", json!("top_right"))).expect("measured from the top right");
    assert_eq!(
        holes(&s),
        was,
        "the holes stay where they are, with their numbers"
    );
    let now_table = table(&s);
    assert_eq!(now_table.lines().count(), was_table.lines().count());
    assert_ne!(now_table, was_table, "the numbers change");
}
