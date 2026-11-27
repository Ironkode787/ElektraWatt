// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Making a part (doc 07 §5.5): the journeys of the parts place's design, each on a new project from nothing. New
//! part… asks What is it? first; a card's start presets the sheet; the person types what the journey names and saves.
//! The part is then offered where its kind is offered, and Ready for lacks nothing for what the journey makes it for.

use crate::common::{Tmp, cursor, done, form, grid_point, tables};

use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::ew_api::part_fields::Feature;
use ew_viewmodel::shell::parts_place::Asking;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A shell in the window's mode on a new project from the built-in template with a schematic page 101, the person's
/// user folder (My parts) under `tmp`.
fn project(tmp: &Tmp) -> ShellModel {
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = ShellModel::new(Mode::Gui, tmp.config().with_libraries(Some(mine)));
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    s
}

/// Types `fields` into the open sheet in order: `+<table>` adds a row of that table, `pin` a pin of that name, any
/// other a field.
fn fill(s: &mut ShellModel, fields: &[(&str, &str)]) {
    for (field, value) in fields {
        if let Some(table) = field.strip_prefix('+') {
            done(s, "parts.row", json!({"table": table}));
        } else if *field == "pin" {
            done(s, "part.pin", json!({"add": true, "name": value}));
        } else {
            done(s, "parts.field", json!({"field": field, "value": value}));
        }
    }
}

/// Makes part `number` of maker Example from the card `start`: New part…, the card, the fields, Save. Returns the
/// features Ready for names of the saved part, opened again.
fn make(s: &mut ShellModel, start: &str, number: &str, fields: &[(&str, &str)]) -> Vec<Feature> {
    done(s, "parts.new", json!({}));
    let place = s.parts_place().expect("Parts");
    assert!(
        place.sheet.is_none(),
        "New part… opens no sheet before its question"
    );
    assert_eq!(place.asking, Some(Asking::WhatIsIt));
    done(s, "parts.new", json!({"start": start}));
    fill(s, &[("manufacturer", "Example"), ("part_number", number)]);
    fill(s, fields);
    done(s, "parts.save", json!({}));
    saved(s, number)
}

/// The features Ready for names of part `number`, opened in Parts.
fn saved(s: &mut ShellModel, number: &str) -> Vec<Feature> {
    done(s, "parts.open", json!({"part": number}));
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("the part opens");
    assert_eq!(open.sheet.text("part_number"), number);
    open.lacks.iter().map(|l| l.feature).collect()
}

#[track_caller]
fn lacks_none(lacks: &[Feature], of: &[Feature]) {
    for f in of {
        assert!(!lacks.contains(f), "it lacks something for {f:?}: {lacks:?}");
    }
}

/// Places part `number` from the device browser on page 101 at grid point `at`; whether the page draws it with its
/// symbols, as the pages draw a part drawn as its pieces or a terminal: no box on the page.
fn placed_with_symbols(s: &mut ShellModel, number: &str, at: (i64, i64)) -> bool {
    // Back to the page, wherever the previous step left the window.
    let _ = s.dispatch("parts.back", &json!({}));
    done(s, "page.goto", json!({"folio": "101"}));
    done(s, "tool.place", json!({}));
    done(s, "place.pick", json!({ "part": number }));
    let p = grid_point(s, at);
    cursor(s, p);
    done(s, "canvas.activate", json!({}));
    done(s, "ui.dismiss", json!({}));
    let t = tables(s);
    !t.reps.is_empty() && t.reps.iter().all(|r| r.black_box.is_none())
}

/// Whether the device browser of page 101 offers part `number`.
fn browser_offers(s: &mut ShellModel, number: &str) -> bool {
    done(s, "parts.back", json!({}));
    done(s, "page.goto", json!({"folio": "101"}));
    done(s, "tool.place", json!({}));
    let offered = s
        .picker()
        .expect("the device browser")
        .catalogue
        .parts
        .iter()
        .any(|p| p.part_number == number);
    done(s, "ui.dismiss", json!({}));
    offered
}

/// Whether the form `action` opens offers part `number` in one of its lists.
fn form_offers(s: &mut ShellModel, action: &str, number: &str) -> bool {
    done(s, "parts.back", json!({}));
    done(s, action, json!({}));
    let offered = form(s).fields.iter().any(|f| {
        f.options
            .iter()
            .any(|(v, w)| v.contains(number) || w.contains(number))
    });
    done(s, "ui.dismiss", json!({}));
    offered
}

/// Whether the project's workshop stock takes part `number`.
fn stock_takes(s: &mut ShellModel, number: &str) -> bool {
    done(s, "parts.back", json!({}));
    done(s, "workshop.edit_stock", json!({}));
    let took =
        s.dispatch("workshop.stock_add", &json!({"part": number})) == ew_viewmodel::shell::Outcome::Done;
    done(s, "ui.dismiss", json!({}));
    took
}

/// Journey a: a cabinet with its mounting plate, made in passing from the plate's field (New plate…: Save returns to
/// the cabinet with the plate chosen), and one door; Enclosure… offers it.
#[test]
fn a_cabinet_with_its_plate_made_in_passing_and_a_door() {
    let tmp = Tmp::new("journey-a");
    let mut s = project(&tmp);
    done(&mut s, "parts.new", json!({"start": "enclosure"}));
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "ENC-8-10-3"),
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
    let place = s.parts_place().expect("Parts");
    assert_eq!(place.under.len(), 1, "the cabinet waits under the plate");
    assert!(
        place.strip().is_some_and(|t| t.contains("ENC-8-10-3")),
        "{:?}",
        place.strip()
    );
    fill(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "MP-8-10"),
            ("sections.mounting_plate.width", "760"),
            ("sections.mounting_plate.height", "960"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("the cabinet again");
    assert_eq!(open.sheet.text("part_number"), "ENC-8-10-3");
    assert_eq!(
        open.sheet
            .row("sections.enclosure.plate")
            .map(|r| r.text.as_str()),
        Some("MP-8-10"),
        "the plate chosen"
    );
    fill(
        &mut s,
        &[
            ("+sections.enclosure.doors", ""),
            ("sections.enclosure.doors[0].hinge", "left"),
            ("sections.enclosure.doors[0].width", "800"),
            ("sections.enclosure.edge_margin", "30"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    let cabinet = saved(&mut s, "ENC-8-10-3");
    lacks_none(
        &cabinet,
        &[Feature::Cabinet, Feature::PlateFrame, Feature::Surfaces],
    );
    let plate = saved(&mut s, "MP-8-10");
    lacks_none(&plate, &[Feature::PlateFrame]);
    assert!(form_offers(&mut s, "layout.enclosure_new", "ENC-8-10-3"));
}

/// Journey b: a cabinet with two doors made like the first, its size and its doors changed.
#[test]
fn a_cabinet_like_another_with_two_doors() {
    let tmp = Tmp::new("journey-b");
    let mut s = project(&tmp);
    make(
        &mut s,
        "enclosure",
        "ENC-8-10-3",
        &[
            ("sections.enclosure.width", "800"),
            ("sections.enclosure.height", "1000"),
            ("sections.enclosure.depth", "300"),
            ("+sections.enclosure.doors", ""),
            ("sections.enclosure.doors[0].hinge", "left"),
            ("sections.enclosure.edge_margin", "30"),
        ],
    );
    done(&mut s, "parts.new", json!({"like": true}));
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("the new part");
    assert!(open.sheet.new && open.sheet.text("part_number").is_empty());
    assert_eq!(
        open.sheet.text("manufacturer"),
        "Example",
        "everything but its number"
    );
    fill(
        &mut s,
        &[
            ("part_number", "ENC-12-10-3-2D"),
            ("sections.enclosure.width", "1200"),
            ("sections.enclosure.doors[0].width", "600"),
            ("+sections.enclosure.doors", ""),
            ("sections.enclosure.doors[1].hinge", "right"),
            ("sections.enclosure.doors[1].width", "600"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    let lacks = saved(&mut s, "ENC-12-10-3-2D");
    lacks_none(&lacks, &[Feature::Cabinet, Feature::Surfaces]);
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("opened");
    let doors = open.sheet.table("sections.enclosure.doors").expect("its doors");
    assert_eq!(doors.rows.len(), 2, "two doors");
    assert!(form_offers(&mut s, "layout.enclosure_new", "ENC-12-10-3-2D"));
}

/// Journeys c and d: a feed-through terminal with one level and a terminal made without a starter with two, each with
/// its clamps, its size and its rail; the device browser offers them, and each is drawn by the starter terminal, not as a
/// box. The second's pins stand at their levels: opened again, it has its two levels.
#[test]
fn terminals_with_their_levels_clamps_size_and_rail() {
    let tmp = Tmp::new("journey-c");
    let mut s = project(&tmp);
    for (number, levels, width, clamp, max) in [
        ("FT-4", 1, "6.2", "screw", "6"),
        ("DL-2.5", 2, "5.2", "push_in", "2.5"),
    ] {
        let mut fields: Vec<(String, String)> = vec![("levels".into(), levels.to_string())];
        for i in 0..2 * levels {
            for (member, value) in [
                ("clamp_type", clamp),
                ("csa_min", "0.5"),
                ("csa_max", max),
                ("max_conductors", "1"),
            ] {
                fields.push((format!("cp_pattern[{i}].{member}"), value.to_owned()));
            }
        }
        for (f, v) in [
            ("mech.width", width),
            ("mech.height", "65"),
            ("mech.depth", "45"),
            ("mech.mount_kind", "ts35x7_5"),
        ] {
            fields.push((f.into(), v.into()));
        }
        let typed: Vec<(&str, &str)> = fields.iter().map(|(f, v)| (f.as_str(), v.as_str())).collect();
        let lacks = make(&mut s, "terminal", number, &typed);
        lacks_none(
            &lacks,
            &[
                Feature::Placing,
                Feature::Footprint,
                Feature::RailFit,
                Feature::ClampFit,
            ],
        );
        let open = s
            .parts_place()
            .and_then(|p| p.sheet.as_ref())
            .expect("opened again");
        assert_eq!(open.sheet.levels.len(), levels, "{number}: its levels");
        assert!(!open.sheet.box_part, "{number} is drawn as a terminal, not a box");
        assert!(browser_offers(&mut s, number), "{number}");
    }
}

/// Journey e: a cable with seven numbered cores and its outer diameter; New cable… offers it.
#[test]
fn a_cable_with_its_cores_and_diameter() {
    let tmp = Tmp::new("journey-e");
    let mut s = project(&tmp);
    let lacks = make(
        &mut s,
        "cable",
        "CY-7x1.5",
        &[
            ("cable.cores", "7"),
            ("cable.csa", "1.5"),
            ("cable.diameter", "9.6"),
        ],
    );
    lacks_none(&lacks, &[Feature::CableCores, Feature::GlandChoice]);
    assert!(form_offers(&mut s, "cable.new", "CY-7x1.5"));
}

/// A journey of one part: its start, its number, the fields typed and the feature that must lack nothing.
type Journey = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
    Feature,
);

/// Journeys f and k: a wire, a ferrule, a marker and a cable gland; the workshop stock takes each.
#[test]
fn a_wire_a_ferrule_a_marker_and_a_gland_for_the_stock() {
    let tmp = Tmp::new("journey-f");
    let mut s = project(&tmp);
    let parts: [Journey; 4] = [
        (
            "wire",
            "H07-1.5-BU",
            &[
                ("sections.wire.csa", "1.5"),
                ("sections.wire.colour", "BU"),
                ("sections.wire.class", "5"),
            ],
            Feature::StockMatch,
        ),
        (
            "ferrule",
            "FER-1.5-BK",
            &[
                ("sections.ferrule.csa_min", "1.5"),
                ("sections.ferrule.csa_max", "1.5"),
                ("sections.ferrule.colour", "BK"),
            ],
            Feature::FerruleChoice,
        ),
        (
            "marker",
            "WM-12",
            &[
                ("sections.marker.marks", "wire"),
                ("sections.marker.lines", "1"),
                ("sections.marker.chars", "12"),
            ],
            Feature::MarkerFit,
        ),
        (
            "gland",
            "CG-M25",
            &[
                ("sections.gland.thread", "m25"),
                ("sections.gland.clamp_min", "9"),
                ("sections.gland.clamp_max", "17"),
                ("sections.gland.nut", "33"),
            ],
            Feature::GlandChoice,
        ),
    ];
    for (start, number, fields, feature) in parts {
        let lacks = make(&mut s, start, number, fields);
        lacks_none(&lacks, &[feature]);
        assert!(stock_takes(&mut s, number), "{number}");
    }
}

/// Journey g: a rail of its profile and a wiring duct of its size; Rail and Duct offer them.
#[test]
fn a_rail_and_a_duct() {
    let tmp = Tmp::new("journey-g");
    let mut s = project(&tmp);
    let rail = make(&mut s, "rail", "TS35-15", &[("sections.rail.profile", "ts35x15")]);
    lacks_none(&rail, &[Feature::RailFit]);
    let duct = make(
        &mut s,
        "duct",
        "WD-60x80",
        &[("mech.width", "60"), ("mech.height", "80")],
    );
    lacks_none(&duct, &[Feature::DuctSize]);
    let numbers = |s: &ShellModel, kind: &str| -> Vec<String> {
        s.draw_part_offers(kind).into_iter().map(|o| o.number).collect()
    };
    assert!(numbers(&s, "rail").contains(&"TS35-15".to_owned()));
    assert!(numbers(&s, "duct").contains(&"WD-60x80".to_owned()));
}

/// Journeys i and j: a contactor drawn with symbols that exist, its pins, size and rail; a pushbutton in a door with
/// its round front and its cut-out. The device browser offers both.
#[test]
fn a_contactor_and_a_pushbutton_in_a_door() {
    let tmp = Tmp::new("journey-i");
    let mut s = project(&tmp);
    let contactor = make(
        &mut s,
        "device",
        "CON-18-24",
        &[
            ("description", "Contactor 18 A, coil 24 V DC"),
            ("+pieces", ""),
            ("pieces[0].what", "ew.fn.coil"),
            ("pieces[0].pins", "A1, A2"),
            ("+pieces", ""),
            ("pieces[1].what", "ew.fn.contact-no"),
            ("pieces[1].pins", "13, 14"),
            ("tech:rated_current", "18 A"),
            ("mech.width", "45"),
            ("mech.height", "85"),
            ("mech.depth", "80"),
            ("mech.mount_kind", "ts35x7_5"),
        ],
    );
    lacks_none(
        &contactor,
        &[
            Feature::PartsList,
            Feature::Placing,
            Feature::Footprint,
            Feature::RailFit,
        ],
    );
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("opened again");
    let whats: Vec<&str> = open.sheet.pieces.iter().map(|p| p.what.as_str()).collect();
    assert_eq!(
        whats,
        ["ew.fn.coil", "ew.fn.contact-no"],
        "a coil and its contact"
    );
    assert!(browser_offers(&mut s, "CON-18-24"));
    assert!(
        placed_with_symbols(&mut s, "CON-18-24", (10, 10)),
        "drawn as its coil, not a box"
    );
    let button = make(
        &mut s,
        "door",
        "PB-22-GN",
        &[
            ("+pieces", ""),
            ("pieces[0].what", "ew.fn.pushbutton-no"),
            ("pieces[0].pins", "13, 14"),
            ("mech.width", "30"),
            ("mech.height", "30"),
            ("mech.front_shape", "round"),
            ("+mech.holes", ""),
            ("mech.holes[0].x", "15"),
            ("mech.holes[0].y", "15"),
            ("mech.holes[0].hole.kind", "cutout"),
            ("mech.holes[0].hole.size", "22"),
        ],
    );
    lacks_none(
        &button,
        &[Feature::Placing, Feature::HoleTable, Feature::Footprint],
    );
    assert!(browser_offers(&mut s, "PB-22-GN"));
}

/// Journey l: a valve with its coil and its ports, and a tube; the device browser offers the valve.
#[test]
fn a_valve_with_its_coil_and_a_tube() {
    let tmp = Tmp::new("journey-l");
    let mut s = project(&tmp);
    let valve = make(
        &mut s,
        "valve",
        "VLV-52-24",
        &[
            ("+pieces", ""),
            ("pieces[0].what", "ew.fn.valve-5-2-sol-spring"),
            ("pieces[0].pins", "1, 2, 3, 4, 5"),
            ("sections.fluid.coil.voltage", "24"),
            ("sections.fluid.coil.env", "dc"),
            ("+sections.fluid.ports", ""),
            ("sections.fluid.ports[0].designation", "1"),
            ("sections.fluid.ports[0].push_in_od", "8"),
        ],
    );
    lacks_none(&valve, &[Feature::Placing, Feature::CoilCheck]);
    assert!(browser_offers(&mut s, "VLV-52-24"));
    let tube = make(
        &mut s,
        "tube",
        "PU-8",
        &[
            ("sections.fluid.tube.od", "8"),
            ("sections.fluid.tube.id", "5.5"),
            ("sections.fluid.tube.material", "PU"),
            ("sections.fluid.tube.colour", "BU"),
            ("sections.fluid.tube.p_rating", "10"),
        ],
    );
    lacks_none(&tube, &[Feature::TubeFit]);
}

/// A new part needs a maker and a part number, and one of a maker and a number that exists is refused in words that
/// name it.
#[test]
fn a_part_that_exists_is_refused_by_its_maker_and_number() {
    let tmp = Tmp::new("journey-refused");
    let mut s = project(&tmp);
    make(&mut s, "device", "CON-18-24", &[]);
    done(&mut s, "parts.new", json!({"start": "device"}));
    fill(&mut s, &[("manufacturer", "Example")]);
    assert!(matches!(
        s.dispatch("parts.save", &json!({})),
        ew_viewmodel::shell::Outcome::Refused { .. } | ew_viewmodel::shell::Outcome::Failed { .. }
    ));
    fill(&mut s, &[("part_number", "CON-18-24")]);
    assert!(s.dispatch("parts.save", &json!({})) != ew_viewmodel::shell::Outcome::Done);
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("still open");
    let words = open.foot_message().expect("the refusal in words");
    assert!(words.contains("Example CON-18-24"), "{words}");
}

/// Back with changes not saved asks, as every program does: Stay keeps the sheet, Save saves and leaves (doc 07 §5.2).
#[test]
fn back_with_changes_not_saved_asks_save_discard_or_stay() {
    let tmp = Tmp::new("journey-back");
    let mut s = project(&tmp);
    done(&mut s, "parts.new", json!({"start": "device"}));
    fill(&mut s, &[("manufacturer", "Example"), ("part_number", "SW-6")]);
    done(&mut s, "parts.back", json!({}));
    assert!(
        matches!(
            s.dialog(),
            Some(ew_viewmodel::shell::Dialog::ConfirmLeavePart { .. })
        ),
        "Back asks first"
    );
    done(&mut s, "ui.dismiss", json!({}));
    assert!(
        s.parts_place().and_then(|p| p.sheet.as_ref()).is_some(),
        "Stay keeps the sheet as it was"
    );
    done(&mut s, "parts.back", json!({ "save": true }));
    assert!(s.parts_place().is_none(), "saved, and back on the page");
    saved(&mut s, "SW-6");
}
