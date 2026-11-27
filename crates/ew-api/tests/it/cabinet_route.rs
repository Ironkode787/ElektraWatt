// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route on the sample's panel page 902 (doc 12 §8.5, §12.5): the card of the cabinet's plan, Apply in one step, a
//! wire's Length and Way rows, a duct's fill rows, `layout.route`'s refusals and the shop's numbers of
//! `project.settings.set`. Named wires only, never totals.

use crate::common;

use ew_api::output::Uuid;
use ew_api::{AnyCommand, CmdError, Session};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) -> Result<(), CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    match s.execute(&cmd) {
        Err(CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .map(|_| ())
}

/// The carrier connection of the wire numbered `number`.
fn wire(s: &Session, number: &str) -> Uuid {
    let q = s.query();
    q.get::<ew_derive::WireRecordSet>(())
        .iter()
        .find(|r| r.number.as_deref() == Some(number))
        .map(|r| r.wire.id.as_uuid())
        .unwrap_or_else(|| panic!("no wire {number}"))
}

/// The wire's row `name` of its Properties: its text and its chip.
fn wire_row(s: &Session, number: &str, name: &str) -> Option<(String, String)> {
    let sheet = s.property_sheet(&[wire(s, number)]);
    sheet.sections.iter().flat_map(|sec| &sec.rows).find_map(|r| {
        (r.key == format!("connection/derived.{name}")).then(|| {
            let text = match &r.value {
                ew_api::output::RowValue::One { text, .. } => text.clone(),
                _ => String::new(),
            };
            let chip = match &r.provenance {
                ew_api::output::Provenance::Derived { via } => via.clone(),
                ew_api::output::Provenance::Overridden { note } => note.clone(),
                _ => String::new(),
            };
            (text, chip)
        })
    })
}

/// Route's card on 902 names a wire with its new length, keeps 101.1's typed 400 mm with what Route gives, makes the
/// field wire 104.4 on site and shows duct 1's fill; Apply stores the plan in one step, after which Route finds nothing
/// due and counts the ways that hold.
#[test]
fn routes_card_on_902_and_its_apply() {
    let mut s = common::sample_without_ways();
    let (card, preview) = s
        .cabinet_route("902", false)
        .expect("902 shows a cabinet")
        .expect("a plan");
    assert_eq!(card.cabinet, "+CAB");
    let row =
        |rows: &[ew_api::cabinet_route::CardWire], n: &str| rows.iter().find(|w| w.number == n).cloned();
    let w = row(&card.lengths, "101.8").expect("101.8 gets a way");
    assert!(
        w.length.is_some() && w.was.is_none() && w.ends == "-101Q1:2 → -103F1:1",
        "{w:?}"
    );
    let typed = row(&card.typed, "101.1").expect("101.1 keeps its typed length");
    assert_eq!(typed.typed, Some(ew_units::Nm::from_mm(400)));
    assert!(typed.length.is_some());
    assert!(row(&card.site, "104.4").is_some(), "a field wire is made on site");
    let duct = card.ducts.iter().find(|d| d.label == "Duct 1").expect("duct 1");
    assert!(duct.size == "40×60" && !duct.over && duct.limit == 60, "{duct:?}");
    assert!(card.ways.iter().any(|w| w.wire == wire(&s, "101.8")));
    assert!(
        s.cabinet_route("101", false).is_none(),
        "a schematic page shows no cabinet"
    );

    let before = s.history().cursor();
    s.commit(preview).expect("Apply");
    assert_eq!(s.history().cursor(), before + 1, "one undo step");
    let (again, _) = s.cabinet_route("902", false).unwrap().unwrap();
    assert!(
        again.lengths.is_empty() && again.typed.is_empty() && again.holding > 0,
        "{again:?}"
    );
    let (all, _) = s.cabinet_route("902", true).unwrap().unwrap();
    assert!(
        row(&all.lengths, "101.9").is_some(),
        "the tick plans every wire again"
    );
}

/// After Route a wire's Length row says how its length is made and its Way row where it runs; a typed length says what
/// Route gives; before Route the row says that Route on 902 gives one. A duct's rows give its inner area and its fill.
#[test]
fn a_routed_wires_length_and_way_rows_and_a_ducts_fill() {
    let mut s = common::sample_without_ways();
    let (_, chip) = wire_row(&s, "101.9", "length").expect("a Length row");
    assert!(
        chip.starts_with("No way yet: Route on page 902 gives it one"),
        "{chip}"
    );
    assert!(wire_row(&s, "101.9", "way").is_none(), "no way before Route");
    let space = s.cabinet_of_folio("902").unwrap();
    run(&mut s, "layout.route", json!({"space": space})).expect("Route");
    let (text, chip) = wire_row(&s, "101.9", "length").unwrap();
    assert!(
        text.ends_with(" mm") && chip.starts_with("Routed: "),
        "{text} · {chip}"
    );
    assert!(
        chip.contains("50 mm reserve at each end, rounded up to "),
        "{chip}"
    );
    let (way, chip) = wire_row(&s, "101.9", "way").expect("a Way row");
    assert_eq!((way.as_str(), chip.as_str()), ("Duct 1", "routed"));
    let (text, chip) = wire_row(&s, "101.1", "length").unwrap();
    assert!(
        text == "400 mm" && chip.starts_with("Typed by hand. Route gives it "),
        "{text} · {chip}"
    );

    let q = s.query();
    let duct = q
        .table::<ew_schema::Carrier>()
        .iter()
        .find(|(_, c)| c.kind == ew_schema::CarrierKind::Duct)
        .map(|(id, _)| id.as_uuid())
        .expect("the sample's duct");
    let rows = s.property_sheet(&[duct]);
    let text = |key: &str| {
        rows.sections
            .iter()
            .flat_map(|sec| &sec.rows)
            .find(|r| r.key == key)
            .and_then(|r| match &r.value {
                ew_api::output::RowValue::One { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no row {key}"))
    };
    assert_eq!(
        text("carrier/derived.inner_area"),
        "2040 mm² · 0.85 × 40 × 60: the part gives no inner size"
    );
    let fill = text("carrier/derived.fill");
    assert!(fill.contains("% where fullest · the limit is 60 %"), "{fill}");
    assert!(fill.ends_with("with default diameters (unverified)"), "{fill}");
}

/// `layout.route` refuses a space that does not exist and a wire Route does not lay (the field wire 104.4), in words;
/// `project.settings.set` refuses a reserve below nothing and a fill limit outside 1 … 100, and a reserve it takes
/// changes a routed length at once.
#[test]
fn route_and_the_shops_numbers_refuse_in_words() {
    let mut s = common::sample();
    let space = s.cabinet_of_folio("902").unwrap();
    let refusal = |r: Result<(), CmdError>| match r {
        Err(CmdError::InvalidParams { text, .. }) => text.to_string(),
        other => panic!("{other:?}"),
    };
    let gone = refusal(run(&mut s, "layout.route", json!({"space": Uuid::from_u128(7)})));
    assert_eq!(gone, "the layout space is not in the project");
    let field = wire(&s, "104.4");
    let not = refusal(run(
        &mut s,
        "layout.route",
        json!({"space": space, "wires": [field]}),
    ));
    assert_eq!(
        not,
        "It is not a wire of the cabinet +CAB that Route lays: its ends must both be in the cabinet"
    );
    let set = |member: &str, value: Value| json!({"member": member, "value": value});
    let below = refusal(run(
        &mut s,
        "project.settings.set",
        set("workshop.reserve", json!(-1)),
    ));
    assert_eq!(below, "A length cannot be less than nothing");
    for limit in [0, 101] {
        let out = refusal(run(
            &mut s,
            "project.settings.set",
            set("workshop.fill_limit", json!(limit)),
        ));
        assert_eq!(out, "A fill limit is a percentage from 1 to 100");
    }
    let w = wire(&s, "101.8");
    run(&mut s, "layout.route", json!({"space": space, "wires": [w]})).expect("Route 101.8");
    let length = |s: &Session| {
        let q = s.query();
        q.get::<ew_derive::WireRecordSet>(())
            .iter()
            .find(|r| r.number.as_deref() == Some("101.8"))
            .and_then(|r| r.length.value())
            .expect("a length")
    };
    let was = length(&s);
    run(
        &mut s,
        "project.settings.set",
        set("workshop.reserve", json!(60_000_000)),
    )
    .expect("a reserve");
    assert_eq!(length(&s).0, was.0 + 20_000_000, "10 mm more at each end");
}

/// After Route, -X1031 turned half round on its rail stales the ways of its wires: the wire list prints 103.5's length
/// after "≈" with Length from "stale", the cut list and the consumables print their pieces and metres with "≈"; Route
/// lays them again.
#[test]
fn a_turned_strip_stales_its_wires_and_the_lists_say_so() {
    use ew_api::output::{RowExport, row_export};
    let mut s = common::sample();
    let space = s.cabinet_of_folio("902").unwrap();
    run(&mut s, "layout.route", json!({"space": space})).expect("Route");
    let lines = |s: &Session, kind: &str| {
        let q = s.query();
        row_export(&q, &RowExport::Kind(kind.to_owned()), None)
            .unwrap_or_else(|e| panic!("{kind}: {e}"))
            .csv
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let row = |s: &Session, number: &str| {
        lines(s, "wire_list")
            .into_iter()
            .find(|l| l.starts_with(&format!("data,{number},")))
            .unwrap_or_else(|| panic!("no row {number}"))
    };
    assert!(row(&s, "103.5").contains(",routed,"));
    assert!(!lines(&s, "cut_list").iter().any(|l| l.contains('≈')));
    let q = s.query();
    let (placement, p) = q
        .table::<ew_schema::Placement>()
        .iter()
        .find(|(_, p)| ew_derive::short_tag(&q, p.device) == "-X1031")
        .map(|(id, p)| (id, p.clone()))
        .expect("-X1031 placed");
    drop(q);
    let t = p.pose.t.map(|n| n.0);
    run(
        &mut s,
        "layout.move",
        json!({"placement": placement.as_uuid(), "pose": {"t": t, "q": [0, 0, 0, 1_000_000_000]}}),
    )
    .expect("turned");
    let stale = row(&s, "103.5");
    assert!(stale.contains(",≈ ") && stale.contains(",stale,"), "{stale}");
    assert!(lines(&s, "cut_list").iter().any(|l| l.contains('≈')));
    assert!(lines(&s, "consumables").iter().any(|l| l.contains('≈')));
    run(&mut s, "layout.route", json!({"space": space})).expect("Route again");
    assert!(row(&s, "103.5").contains(",routed,"));
}

/// The findings of `rule` on the session.
fn findings(s: &Session, rule: &str) -> Vec<ew_checks::Finding> {
    let q = s.query();
    ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

/// EW-MFG-001: before Route the sample's cabinet has wires without a length, one finding naming +CAB with them as its
/// targets (101.9, not the field wire 104.4 nor a core of -101W1); after Route none.
#[test]
fn ew_mfg_001_counts_the_cabinets_wires_without_a_length() {
    let mut s = common::sample_without_ways();
    let f = findings(&s, "EW-MFG-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message()
            .contains(" wires of +CAB have no length: Route gives them one, or type it"),
        "{}",
        f[0].message()
    );
    let has = |n: &str| f[0].targets.iter().any(|t| t.id.as_uuid() == wire(&s, n));
    assert!(
        has("101.9") && !has("104.4"),
        "the field wire 104.4 is the site's"
    );
    let q = s.query();
    let cores: Vec<_> = q
        .get::<ew_derive::WireRecordSet>(())
        .iter()
        .filter(|r| r.wire.core.is_some())
        .map(|r| r.wire.id.as_uuid())
        .collect();
    assert!(
        !cores.is_empty() && !f[0].targets.iter().any(|t| cores.contains(&t.id.as_uuid())),
        "a core is its cable's"
    );
    drop(q);
    let space = s.cabinet_of_folio("902").unwrap();
    run(&mut s, "layout.route", json!({"space": space})).expect("Route");
    assert!(findings(&s, "EW-MFG-001").is_empty());
}

/// EW-LAY-011: -X1031 turned after Route stales the ways of its wires: one finding at -X1031 with Route as its fix;
/// one of them routed again leaves the finding with the same fingerprint.
#[test]
fn ew_lay_011_is_one_finding_per_cause_whatever_its_wires() {
    let mut s = common::sample();
    let space = s.cabinet_of_folio("902").unwrap();
    run(&mut s, "layout.route", json!({"space": space})).expect("Route");
    assert!(findings(&s, "EW-LAY-011").is_empty());
    let q = s.query();
    let (placement, p) = q
        .table::<ew_schema::Placement>()
        .iter()
        .find(|(_, p)| ew_derive::short_tag(&q, p.device) == "-X1031")
        .map(|(id, p)| (id, p.clone()))
        .unwrap();
    drop(q);
    let t = p.pose.t.map(|n| n.0);
    run(
        &mut s,
        "layout.move",
        json!({"placement": placement.as_uuid(), "pose": {"t": t, "q": [0, 0, 0, 1_000_000_000]}}),
    )
    .expect("turned");
    let f = findings(&s, "EW-LAY-011");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message()
            .contains(" wires at +CAB-X1031 no longer hold: it moved. Route gives them new ones."),
        "{}",
        f[0].message()
    );
    assert_eq!(
        f[0].fix.as_ref().map(|x| x.command.as_str()),
        Some("layout.route")
    );
    let w = wire(&s, "103.5");
    run(&mut s, "layout.route", json!({"space": space, "wires": [w]})).expect("103.5 again");
    let again = findings(&s, "EW-LAY-011");
    assert_eq!(again.len(), 1);
    assert_eq!(
        again[0].fingerprint, f[0].fingerprint,
        "the cause keeps its finding"
    );
    assert!(again[0].targets.len() < f[0].targets.len());
}

/// EW-LAY-008: with a fill limit of 1 % duct 1 is over it once Route has laid its wires, in words.
#[test]
fn ew_lay_008_reports_a_duct_over_the_fill_limit() {
    let mut s = common::sample_without_ways();
    let set = json!({"member": "workshop.fill_limit", "value": 1});
    run(&mut s, "project.settings.set", set).expect("a limit");
    assert!(
        findings(&s, "EW-LAY-008").is_empty(),
        "nothing is stored before Route"
    );
    let space = s.cabinet_of_folio("902").unwrap();
    run(&mut s, "layout.route", json!({"space": space, "all": true})).expect("Route");
    let f = findings(&s, "EW-LAY-008");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message().starts_with("Duct 1 (40×60) is ")
            && f[0]
                .message()
                .ends_with("; the limit is 1 %: widen it or add a duct"),
        "{}",
        f[0].message()
    );
}

/// EW-LAY-007 on a part of one's own: -101G1 given a power supply part made here that keeps 100 mm free below it and
/// 10 mm in front: duct 1, 75 mm below it, is 25 mm too close; its own rail is not judged; the placement reaches
/// 127.5 mm from the plate (7.5 of rail, 110 deep, 10 in front). A device of an unknown footprint keeps no zone.
#[test]
fn ew_lay_007_reports_a_duct_inside_a_parts_free_room() {
    use ew_schema::{LibDocument, Record};
    let (project, mut records, mut lib) = common::sample_records();
    let psu = lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(c) if c.item.part_number.as_str() == "EW-GEN-PSU-1" => Some(c.clone()),
            _ => None,
        })
        .expect("the starter power supply");
    let mut own = psu.clone();
    own.id = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(0x0057_0001));
    own.item_id = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(0x0057_0002));
    own.item.part_number = "MY-PSU-24".parse().unwrap();
    let mech = own.item.mech.get_or_insert_with(Default::default);
    mech.clearance = Some(ew_schema::Clearance {
        bottom: ew_units::Nm::from_mm(100),
        front: ew_units::Nm::from_mm(10),
        ..Default::default()
    });
    lib.push(LibDocument::Part(own.clone()));
    for r in &mut records {
        match r {
            Record::Device(d) if d.parts.iter().any(|u| u.part.copy == psu.id) => {
                for u in &mut d.parts {
                    u.part.copy = own.id;
                }
            }
            Record::Placement(p) if p.part.as_ref().is_some_and(|u| u.copy == psu.id) => {
                if let Some(u) = &mut p.part {
                    u.copy = own.id;
                }
            }
            _ => {}
        }
    }
    let s = common::session_of(project, records, lib);
    let f = findings(&s, "EW-LAY-007");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message().ends_with("-101G1 keeps 100 mm free below it (its part's clearance); duct 1 is 75 mm from it, 25 mm too close"),
        "{}",
        f[0].message()
    );
    let q = s.query();
    let placement = q
        .table::<ew_schema::Placement>()
        .iter()
        .find(|(_, p)| ew_derive::short_tag(&q, p.device) == "-101G1")
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(
        ew_derive::layout::reach(&q, placement),
        Some(ew_units::Nm(127_500_000))
    );
}

/// EW-LAY-006's closed door on a cabinet part of one's own: the sample's enclosure given a copy made here with 100 mm
/// between the plate and the door. -101G1, 117.5 mm deep with its rail, reaches the closed door; a device of the plate
/// and one of the door that stand in front of each other and together take more than 100 mm meet. Properties of
/// -101G1 say what it takes and the room; with the sample's own 200 mm nothing reaches the door.
#[test]
fn ew_lay_006_reports_what_the_closed_door_meets() {
    use ew_schema::{LibDocument, Record};
    let s = common::sample();
    assert!(
        !findings(&s, "EW-LAY-006")
            .iter()
            .any(|f| f.discriminator.starts_with("door_")),
        "the sample's door closes"
    );
    let reaches = |s: &Session, tag: &str| {
        let q = s.query();
        let placement = q
            .table::<ew_schema::Placement>()
            .iter()
            .find(|(_, p)| ew_derive::short_tag(&q, p.device) == tag)
            .map(|(id, _)| id.as_uuid())
            .unwrap();
        drop(q);
        let sheet = s.property_sheet(&[placement]);
        sheet
            .sections
            .iter()
            .flat_map(|sec| &sec.rows)
            .find(|r| r.key.ends_with(".reaches"))
            .map(|r| match &r.value {
                ew_api::output::RowValue::One { text, .. } => text.clone(),
                _ => String::new(),
            })
            .unwrap_or_default()
    };
    assert_eq!(
        reaches(&s, "-101G1"),
        "117.5 mm from the plate; the closed door is 200 mm away"
    );

    let (project, mut records, mut lib) = common::sample_records();
    let enc = lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(c) if c.item.part_number.as_str() == "EW-GEN-ENC-1" => Some(c.clone()),
            _ => None,
        })
        .expect("the starter enclosure");
    let mut own = enc.clone();
    own.id = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(0x0057_0011));
    own.item_id = ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(0x0057_0012));
    own.item.part_number = "MY-CAB-600".parse().unwrap();
    own.item
        .sections
        .enclosure
        .as_mut()
        .expect("an enclosure section")
        .usable_depth = Some(ew_units::Nm::from_mm(100));
    lib.push(LibDocument::Part(own.clone()));
    for r in &mut records {
        if let Record::Device(d) = r {
            for u in &mut d.parts {
                if u.part.copy == enc.id {
                    u.part.copy = own.id;
                }
            }
        }
    }
    let s = common::session_of(project, records, lib);
    let f = findings(&s, "EW-LAY-006");
    let reach: Vec<String> = f
        .iter()
        .filter(|f| f.discriminator == "door_reach")
        .map(|f| f.message())
        .collect();
    assert!(
        reach
            .iter()
            .any(|m| m.ends_with("-101G1 reaches 117.5 mm from the plate; the closed door is 100 mm away")),
        "{reach:#?}"
    );
    assert!(
        f.iter().any(|f| f.discriminator == "door_meet"
            && f.message()
                .contains(" in the door meet when the door is closed: they take ")
            && f.message()
                .ends_with(" of the 100 mm between the plate and the door")),
        "{f:#?}"
    );
    assert_eq!(
        reaches(&s, "-101G1"),
        "117.5 mm from the plate; the closed door is 100 mm away"
    );
}
