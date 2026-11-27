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

/// After Route a wire's Length row says how its length is made and its Way row where it runs, with Show on 902 (the
/// page, then the wire selected, whose way 902 then draws); a typed length says what Route gives; before Route the row
/// says that Route on 902 gives one. A duct's rows give its inner area and its fill, its Wires row Select them, and its
/// fullest piece is the duct's mark on 902.
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
    let w = wire(&s, "101.9");
    let link = |s: &Session, id: Uuid, key: &str| {
        s.property_sheet(&[id])
            .sections
            .iter()
            .flat_map(|sec| &sec.rows)
            .find(|r| r.key == key)
            .and_then(|r| r.link.clone())
    };
    let show = link(&s, w, "connection/derived.way").expect("Show on 902");
    assert_eq!(show.label.to_string(), "Show on 902");
    assert_eq!(
        show.actions,
        [
            ("page.goto", json!({"folio": "902"})),
            ("select.set", json!({"ids": [w]})),
        ]
    );
    assert_eq!(s.ways_at("902", &[w]).len(), 1, "the selected wire's way on 902");
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
    let select = link(&s, duct, "carrier/derived.wires").expect("Select them");
    assert_eq!(select.label.to_string(), "Select them");
    assert!(
        matches!(&select.actions[..], [("select.set", p)] if p["ids"].as_array().is_some_and(|ids| ids.contains(&json!(w)))),
        "{select:?}"
    );
    let marks = s.duct_marks("902", &[duct]);
    assert!(
        matches!(&marks[..], [m] if m.piece[0] != m.piece[1] && !m.over && m.limit == 60),
        "{marks:?}"
    );
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

/// -X1031 turned half round where it stands on its rail, as the window's X turns it (`layout.move` to the clip point with
/// mode free, which keeps it where it fits): its placement.
fn turn_x1031(s: &mut Session) -> ew_ids::Id<ew_schema::Placement> {
    let q = s.query();
    let (placement, p) = q
        .table::<ew_schema::Placement>()
        .iter()
        .find(|(_, p)| ew_derive::short_tag(&q, p.device) == "-X1031")
        .map(|(id, p)| (id, p.clone()))
        .expect("-X1031 placed");
    drop(q);
    assert!(
        matches!(p.carrier, ew_schema::CarrierRef::Carrier(_)),
        "-X1031 is on a rail"
    );
    let pose = json!({"t": [p.pose.t[0].0, 0, 0], "q": [0, 0, 0, 1_000_000_000]});
    let cmd = json!({"placement": placement.as_uuid(), "pose": pose, "mode": "free"});
    run(s, "layout.move", cmd).expect("turned");
    placement
}

/// After Route, -X1031 turned half round on its rail stales the ways of its wires: the wire list prints 103.5's length
/// after "≈" with Length from "stale", the cut list and the consumables print their pieces and metres with "≈"; Route
/// lays them again, 101.6 and 103.6 too.
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
    turn_x1031(&mut s);
    let stale = row(&s, "103.5");
    assert!(stale.contains(",≈ ") && stale.contains(",stale,"), "{stale}");
    assert!(lines(&s, "cut_list").iter().any(|l| l.contains('≈')));
    assert!(lines(&s, "consumables").iter().any(|l| l.contains('≈')));
    run(&mut s, "layout.route", json!({"space": space})).expect("Route again");
    for n in ["103.5", "101.6", "103.6"] {
        assert!(row(&s, n).contains(",routed,"), "{}", row(&s, n));
    }
}

/// With 103.5 alone routed before -X1031 turns, its consumables row sums a stale length beside wires without one: the
/// metres print after "≈" with the wires without a length beside them.
#[test]
fn a_sum_beside_wires_without_a_length_is_approximate_when_it_holds_a_stale_one() {
    use ew_api::output::{RowExport, row_export};
    let mut s = common::sample_without_ways();
    let space = s.cabinet_of_folio("902").unwrap();
    let w = wire(&s, "103.5");
    run(&mut s, "layout.route", json!({"space": space, "wires": [w]})).expect("Route 103.5");
    turn_x1031(&mut s);
    let q = s.query();
    let csv = row_export(&q, &RowExport::Kind("consumables".to_owned()), None)
        .expect("consumables")
        .csv;
    assert!(
        csv.lines()
            .any(|l| l.contains(",≈ ") && l.contains(" wires without a length")),
        "{csv}"
    );
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
    let placement = turn_x1031(&mut s);
    let f = findings(&s, "EW-LAY-011");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message()
            .contains(" wires at +CAB-X1031 no longer hold: it moved. Route gives them new ones."),
        "{}",
        f[0].message()
    );
    // The strip's Properties say how many of its ways no longer hold, with Route.
    let ways = s
        .property_sheet(&[placement.as_uuid()])
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .find(|r| r.key == "placement/derived.ways")
        .cloned()
        .expect("a Ways row");
    assert!(
        matches!(&ways.value, ew_api::output::RowValue::One { text, .. } if text.ends_with(" no longer hold")),
        "{ways:?}"
    );
    assert_eq!(
        ways.link.map(|l| (l.label.to_string(), l.actions)),
        Some(("Route".to_owned(), vec![("layout.route", Value::Null)]))
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

/// A routed wire its cabinet no longer routes has no way read anywhere (doc 12-3d §8.5): re-wired to a field device, to
/// an external point, or become a core, 101.8 has no length (made on site, or its cable's), its duct no longer counts
/// it, and Route's next run removes its record in its one undo step, its card counting it.
#[test]
fn a_wire_the_cabinet_no_longer_routes_keeps_no_way() {
    use ew_schema::{ConnectionKind, Endpoint, Record};
    let s = common::sample();
    let carrier = wire(&s, "101.8");
    let q = s.query();
    let id = ew_ids::Id::<ew_schema::Connection>::from_uuid(carrier);
    let route = ew_ids::v5::route_id::<_, ew_schema::CommittedRoute>(id);
    assert!(
        q.table::<ew_schema::CommittedRoute>().contains(route),
        "the sample lays 101.8"
    );
    let duct = q
        .table::<ew_schema::Carrier>()
        .iter()
        .find(|(_, c)| c.kind == ew_schema::CarrierKind::Duct)
        .map(|(c, _)| c)
        .unwrap();
    let in_duct = |q: &ew_derive::Q| {
        q.get::<ew_derive::layout::DuctFill>(duct)
            .as_ref()
            .as_ref()
            .is_some_and(|f| f.wires.contains(&id))
    };
    assert!(in_duct(&q));
    // A field device's pin, an external point (the end of -101W1's core at ext. L1), and a core of -101W1.
    let field = q
        .functions()
        .iter()
        .find(|(_, f)| f.device.is_some_and(|d| ew_derive::short_tag(&q, d) == "-104B1"))
        .map(|(f, _)| Endpoint::Cp(f, 0))
        .expect("-104B1");
    let records = q.get::<ew_derive::WireRecordSet>(());
    let core_wire = records
        .iter()
        .find(|r| r.wire.core.is_some() && r.wire.ends.iter().any(|e| e.kind == ew_derive::EndKind::External))
        .expect("a core of -101W1 from an external point");
    let external = core_wire
        .wire
        .ends
        .iter()
        .find(|e| e.kind == ew_derive::EndKind::External)
        .map(|e| Endpoint::Cp(e.cp.function, e.cp.idx))
        .expect("ext. L1");
    let conductor = core_wire.wire.core.expect("a core");
    drop(records);
    drop(q);
    let changed = |change: &dyn Fn(&mut ew_schema::Connection)| {
        let (project, mut records, lib) = common::sample_records();
        for r in &mut records {
            if let Record::Connection(c) = r
                && c.id == id
            {
                change(c);
            }
        }
        common::session_of(project, records, lib)
    };
    for (what, mut s) in [
        ("field", changed(&|c| c.b = field.clone())),
        ("external", changed(&|c| c.b = external.clone())),
        ("core", changed(&|c| c.kind = ConnectionKind::Core(conductor))),
    ] {
        let q = s.query();
        let r = q
            .get::<ew_derive::WireRecordSet>(())
            .iter()
            .find(|r| r.wire.id == id)
            .cloned()
            .expect("101.8");
        assert!(
            !matches!(
                r.length,
                ew_derive::WireLength::Routed { .. } | ew_derive::WireLength::Stale { .. }
            ),
            "{what}: {:?}",
            r.length
        );
        assert_eq!(
            ew_derive::layout::route_status(&q, &r.wire),
            ew_derive::layout::RouteStatus::None,
            "{what}"
        );
        assert!(!in_duct(&q), "{what}: the duct no longer counts it");
        drop(q);
        let (card, preview) = s.cabinet_route("902", false).unwrap().expect("a plan");
        assert!(card.gone > 0, "{what}: the card counts the leftover");
        let before = s.history().cursor();
        s.commit(preview).expect("Apply");
        assert_eq!(s.history().cursor(), before + 1, "{what}: one undo step");
        assert!(
            !s.query().table::<ew_schema::CommittedRoute>().contains(route),
            "{what}: Route removed the leftover"
        );
    }
}

/// A wire's every stretch selected (one row of the Connections table) reads as that wire (doc 12-3d §12.5): headed by
/// its number and its ends, its Length the routed length with the sentence that makes it under the row, its Way row with
/// the way told along its course; on 902 its way carries its chip and the device in the door it leaves for.
#[test]
fn a_wire_selected_with_its_every_stretch_reads_as_that_wire() {
    let s = common::sample();
    let q = s.query();
    let r = q
        .get::<ew_derive::WireRecordSet>(())
        .iter()
        .find(|r| r.number.as_deref() == Some("104.3"))
        .cloned()
        .expect("104.3");
    drop(q);
    assert!(
        r.wire.constituents.len() > 1,
        "104.3 has stretches of its own and a shared one"
    );
    let ids: Vec<Uuid> = r.wire.constituents.iter().map(|c| c.as_uuid()).collect();
    let sheet = s.property_sheet(&ids);
    assert_eq!(sheet.header.title, "104.3 · -104E1:X2 → -104A1-A1:M");
    let row = |key: &str| {
        sheet
            .sections
            .iter()
            .flat_map(|sec| &sec.rows)
            .find(|row| row.key == key)
            .cloned()
            .unwrap_or_else(|| panic!("no row {key}"))
    };
    let length = row("connection/derived.length");
    let length_value = ew_derive::WireLength::value(r.length).expect("a routed length");
    assert!(
        matches!(&length.value, ew_api::output::RowValue::One { json, .. } if json.as_i64() == Some(length_value.0)),
        "{length:?}"
    );
    assert!(
        length
            .note
            .as_deref()
            .is_some_and(|n| n.starts_with("Routed: ") && n.contains("door loop")),
        "{:?}",
        length.note
    );
    let way = row("connection/derived.way");
    assert!(
        way.note
            .as_deref()
            .is_some_and(|n| n.starts_with("From -104E1:X2, across the hinge onto the plate, into duct 1")),
        "{:?}",
        way.note
    );
    let shown = s.ways_at("902", &ids);
    assert!(
        shown.iter().any(|w| w
            .chip
            .as_ref()
            .is_some_and(|(_, n, l)| n == "104.3" && *l == Some(length_value))
            && w.door.as_ref().is_some_and(|(_, tag)| tag == "-104E1")),
        "{shown:?}"
    );
}
