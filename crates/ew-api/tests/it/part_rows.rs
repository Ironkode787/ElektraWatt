// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sheet's sections and declared rows (doc 07 §5, picture A): a terminal lists its accessories and
//! variants, not a ferrule's or a wire's section; a row's text is checked by its schema type (a new part's size above zero, a
//! part number a part has, no ferrule as an accessory); a draft left as opened saves nothing; a changed cell writes
//! only its member; `part.delete` deletes a part of the project's own and refuses a library's; Front view and holes
//! reads a hole as its kind and size, checks it in words and saves it, with the front's imported outline.

use crate::common;

use ew_api::{AnyCommand, PartDraftError, Session};
use ew_ids::Uuid;
use serde_json::{Value, json};

fn run(s: &mut Session, cmd: &AnyCommand) -> Result<ew_api::Receipt, ew_api::CmdError> {
    match s.execute(cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(cmd).and_then(|p| s.commit(p)),
        other => other,
    }
}

/// The held copy numbered `number`.
fn copy_of(s: &Session, number: &str) -> Uuid {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("the sample holds {number}"))
}

fn field_key(r: Result<(), PartDraftError>) -> &'static str {
    match r {
        Err(PartDraftError::Field(m)) => m.key,
        other => panic!("a field's message: {other:?}"),
    }
}

#[test]
fn a_terminals_sections_rows_and_tables_are_read_checked_and_saved() {
    let s = common::sample();
    let copy = copy_of(&s, "EW-GEN-TRM-1");
    let mut d = s.part_draft(copy).expect("the terminal's draft");
    let words: Vec<&str> = d.sections().iter().map(|x| x.name()).collect();
    for w in ["identity", "schematic", "cabinet", "connections", "goes_with"] {
        assert!(words.contains(&w), "{words:?}");
    }
    for w in ["wire", "ferrule", "marker"] {
        assert!(!words.contains(&w), "{words:?}");
    }
    // A ferrule lists its own section and none of a mounted part's.
    let ferrule = copy_of(&s, "EW-GEN-FER-1.5");
    let ferrule = s.part_draft(ferrule).expect("the ferrule's draft");
    let theirs: Vec<&str> = ferrule.sections().iter().map(|x| x.name()).collect();
    assert!(theirs.contains(&"ferrule"), "{theirs:?}");
    for w in [
        "schematic",
        "cabinet",
        "connections",
        "goes_with",
        "wire",
        "marker",
    ] {
        assert!(!theirs.contains(&w), "{theirs:?}");
    }
    // The accessories read as part numbers and words.
    let acc = d.table("accessories").expect("the accessories' table").clone();
    let col = |m: &str| acc.columns.iter().position(|(c, _)| *c == m).expect(m);
    let plate = acc
        .rows
        .iter()
        .position(|r| r[col("part")] == "EW-GEN-EP-1")
        .expect("the end plate row");
    assert_eq!(acc.rows[plate][col("per")], "strip");
    // A starter part is never changed: its sheet refuses Save in words that say what to do instead.
    assert!(matches!(
        s.part_edit_command(&d),
        Err(PartDraftError::Refused(k)) if k == "reason-part-edit-not-yours"
    ));
    // A size of zero, a number no part has, a ferrule as an accessory: each refused in words under its field.
    // A starter part's rows are not typed into.
    assert!(matches!(
        s.set_draft_field(&mut d, "mech.width", "9"),
        Err(PartDraftError::Refused(_))
    ));
    // The same part of a colleague's library is his: Save forks it into the project. A new part's size of zero is
    // refused in words.
    let mut s = common::colleague_sample();
    let copy = copy_of(&s, "EW-GEN-TRM-1");
    let mut d = s.part_draft(copy).expect("its sheet");
    let mut new = s.new_part_draft().expect("a new part");
    assert_eq!(
        field_key(s.set_draft_field(&mut new, "mech.width", "0")),
        "error-part-length"
    );
    assert_eq!(
        field_key(s.set_draft_field(&mut d, &format!("accessories[{plate}].part"), "NO-SUCH-PART")),
        "error-part-accessory"
    );
    assert_eq!(
        field_key(s.set_draft_field(&mut d, &format!("accessories[{plate}].part"), "EW-GEN-FER-1.5")),
        "error-part-accessory-kind"
    );
    s.set_draft_field(&mut d, &format!("accessories[{plate}].part"), "EW-GEN-EP-1")
        .expect("the end plate again");
    // Two end plates per strip: the fork carries the changed member, the other members and rows stay.
    s.set_draft_field(&mut d, &format!("accessories[{plate}].qty"), "2")
        .expect("a quantity");
    let held = s
        .query()
        .parts()
        .iter()
        .find(|(id, _)| id.as_uuid() == copy)
        .map(|(_, c)| c.item.clone())
        .unwrap();
    let cmd = s.part_edit_command(&d).expect("the fork");
    run(&mut s, &cmd).expect("saved");
    let q = s.query();
    let fork = q
        .parts()
        .iter()
        .find(|(_, c)| {
            c.item.part_number.as_str() == "EW-GEN-TRM-1"
                && c.library_id.as_uuid() == q.project().id.as_uuid()
        })
        .map(|(_, c)| c.item.clone())
        .expect("the project's own copy");
    assert_eq!(fork.accessories.len(), held.accessories.len());
    assert_eq!(fork.accessories[plate].qty.to_string(), "2");
    assert_eq!(fork.accessories[plate].per, held.accessories[plate].per);
    assert_eq!(fork.variants, held.variants);
    assert_eq!(fork.mech, held.mech);
}

#[test]
fn delete_part_deletes_a_part_of_the_projects_own_and_keeps_a_librarys() {
    let mut s = common::sample();
    let library = copy_of(&s, "EW-GEN-TRM-1");
    let refused = run(
        &mut s,
        &AnyCommand {
            name: "part.delete".into(),
            params: json!({ "copy": library.to_string() }),
        },
    )
    .unwrap_err();
    assert!(refused.to_string().contains("is a library's part"), "{refused}");
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    s.set_draft_field(&mut d, "manufacturer", "Acme")
        .expect("a maker");
    s.set_draft_field(&mut d, "part_number", "OWN-1")
        .expect("a number");
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command).expect("the part is made");
    let own = copy_of(&s, "OWN-1");
    run(
        &mut s,
        &AnyCommand {
            name: "part.delete".into(),
            params: json!({ "copy": own.to_string() }),
        },
    )
    .expect("deleted");
    assert!(s.query().parts().iter().all(|(id, _)| id.as_uuid() != own));
}

/// A new part of the project's own, `number`, with the accessory `accessory` when given.
fn own_part(s: &mut Session, number: &str, accessory: Option<&str>) {
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    s.set_draft_field(&mut d, "manufacturer", "Acme")
        .expect("a maker");
    s.set_draft_field(&mut d, "part_number", number)
        .expect("a number");
    if let Some(a) = accessory {
        assert!(d.add_table_row("accessories"));
        s.set_draft_field(&mut d, "accessories[0].part", a)
            .expect("its accessory");
        s.set_draft_field(&mut d, "accessories[0].qty", "1").expect("one");
        s.set_draft_field(&mut d, "accessories[0].per", "strip")
            .expect("per strip");
    }
    let made = s.new_part_command(&d).expect("the new part's command");
    run(s, &made.command).expect("the part is made");
}

/// Delete part… asks who names the copy: a part of one's own that another part names as its accessory is refused in
/// words that name that part, and goes once nothing names it.
#[test]
fn delete_part_refuses_a_part_another_part_names_as_its_accessory() {
    let mut s = common::sample();
    own_part(&mut s, "OWN-EP", None);
    own_part(&mut s, "OWN-TRM", Some("OWN-EP"));
    let plate = copy_of(&s, "OWN-EP");
    let delete = |copy: Uuid| AnyCommand {
        name: "part.delete".into(),
        params: json!({ "copy": copy.to_string() }),
    };
    let refused = match run(&mut s, &delete(plate)).unwrap_err() {
        ew_api::CmdError::Model(_, Some(words)) => words.to_string(),
        other => panic!("refused in words: {other}"),
    };
    assert_eq!(
        refused,
        "part OWN-EP cannot be removed: part OWN-TRM still names it"
    );
    assert!(s.query().parts().iter().any(|(id, _)| id.as_uuid() == plate));
    let terminal = copy_of(&s, "OWN-TRM");
    run(&mut s, &delete(terminal)).expect("the terminal's part goes");
    run(&mut s, &delete(plate)).expect("then the end plate's");
}

/// A closed 30 × 30 square in a DXF drawing that names no unit.
const SQUARE_DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\n0\n90\n4\n70\n1\n10\n0\n20\n0\n10\n30\n20\n0\n10\n30\n20\n30\n10\n0\n20\n30\n0\nENDSEC\n0\nEOF\n";

/// Front view and holes (doc 07 §5): on a new part a hole is a Kind and a Size read together, a size that does not
/// read for its kind is refused in words under it, a row without its size, a hole off the front and a round front of
/// unequal sides refuse Save in words; the preview draws the front and its holes; Save writes the shape, the holes and
/// an imported outline into `mech`. The project's own part opens with its holes as kind and size and changes them;
/// a library part's fork keeps them. (A part of My parts changes them: `ew-viewmodel`'s `part_front.rs`.)
#[test]
fn a_parts_front_view_and_holes_are_read_checked_and_saved() {
    let mut s = common::sample();
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    assert!(d.sections().iter().any(|x| x.name() == "cabinet"));
    for (f, v) in [
        ("manufacturer", "Acme"),
        ("part_number", "OWN-LAMP"),
        ("mech.width", "30"),
        ("mech.height", "30"),
        ("mech.mount_kind", "door_cutout"),
        ("mech.front_shape", "round"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    assert!(d.add_table_row("mech.holes"));
    for (f, v) in [
        ("mech.holes[0].x", "15"),
        ("mech.holes[0].y", "15"),
        ("mech.holes[0].hole.kind", "fixing"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    // 22 mm is no screw, but it is a cut-out: the kind's change takes the size's message away.
    assert_eq!(
        field_key(s.set_draft_field(&mut d, "mech.holes[0].hole.size", "22 mm")),
        "hole-error-not-a-screw"
    );
    s.set_draft_field(&mut d, "mech.holes[0].hole.kind", "cutout")
        .expect("22 mm is a cut-out");
    assert!(d.messages.is_empty(), "{:?}", d.messages);
    // A second row without its size, then off the 30 mm front.
    assert!(d.add_table_row("mech.holes"));
    for (f, v) in [
        ("mech.holes[1].x", "40"),
        ("mech.holes[1].y", "15"),
        ("mech.holes[1].hole.kind", "fixing"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    let refused = |s: &mut Session, d: &ew_api::PartSheet| match s.new_part_command(d) {
        Err(PartDraftError::Field(m)) => (m.key, m.field),
        other => panic!("refused in words: {:?}", other.map(|_| ())),
    };
    assert_eq!(
        refused(&mut s, &d),
        ("error-part-hole-incomplete", "mech.holes[1].hole.size".to_owned())
    );
    s.set_draft_field(&mut d, "mech.holes[1].hole.size", "M3")
        .expect("a screw");
    assert_eq!(
        refused(&mut s, &d),
        ("error-part-hole-off", "mech.holes[1].x".to_owned())
    );
    s.set_draft_field(&mut d, "mech.holes[1].x", "5")
        .expect("on the front");
    s.set_draft_field(&mut d, "mech.height", "31").expect("a height");
    assert_eq!(
        refused(&mut s, &d),
        ("error-part-front-round", "mech.front_shape".to_owned())
    );
    s.set_draft_field(&mut d, "mech.height", "30")
        .expect("square again");
    // The preview: the round front and its two holes.
    let (list, sheet) = d.front_preview().expect("a front to preview");
    assert_eq!(list.items.len(), 3);
    assert_eq!(sheet.0, sheet.1);
    // An outline of a drawing without a unit, read in millimetres.
    let tmp = ew_testdir::Tmp::new("ew-api-part-outline");
    let dxf = tmp.path("front.dxf");
    std::fs::write(&dxf, SQUARE_DXF).unwrap();
    assert_eq!(s.draft_outline_import(&mut d, &dxf), Ok(1));
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command).expect("the part is made");
    let copy = copy_of(&s, "OWN-LAMP");
    let held = |s: &Session| {
        s.query()
            .parts()
            .iter()
            .find(|(id, _)| id.as_uuid() == copy)
            .and_then(|(_, c)| c.item.mech.clone())
            .expect("its mechanical data")
    };
    let mech = held(&s);
    assert_eq!(mech.front_shape, Some(ew_schema::FrontShape::Round));
    let mm = |n: i64| ew_units::Nm::from_mm(n);
    let holes: Vec<_> = mech.holes.iter().map(|h| (h.x, h.y, h.hole.clone())).collect();
    assert_eq!(
        holes,
        [
            (mm(15), mm(15), ew_schema::Hole::Cutout(ew_schema::Cutout::D22)),
            (mm(5), mm(15), ew_schema::Hole::Fixing(ew_schema::Screw::M3)),
        ]
    );
    assert_eq!(mech.front_2d.len(), 1);
    assert!(mech.front_2d[0].contains(&ew_units::Pt2::new(mm(30), mm(30))));
    // The project's own part opens with its holes; its second fixing becomes M4, and Remove takes the outline away.
    let mut own = s.part_draft(copy).expect("its draft");
    assert!(own.section_editable(ew_derive::part_fields::Group::Cabinet));
    let t = own.table("mech.holes").expect("the holes");
    let at = |m: &str| t.columns.iter().position(|(c, _)| *c == m).expect(m);
    assert_eq!(
        (
            t.rows[0][at(ew_api::HOLE_KIND)].as_str(),
            t.rows[0][at(ew_api::HOLE_SIZE)].as_str()
        ),
        ("cutout", "22 mm")
    );
    s.set_draft_field(&mut own, "mech.holes[1].hole.size", "M4")
        .expect("a bigger screw");
    assert!(own.remove_outline());
    let cmd = s.part_edit_command(&own).expect("the change");
    run(&mut s, &cmd).expect("saved");
    let mech = held(&s);
    assert_eq!(mech.holes[1].hole, ew_schema::Hole::Fixing(ew_schema::Screw::M4));
    assert!(mech.front_2d.is_empty());
    // A library part's fork keeps its front view: its cut-out reads, its rows and its outline are refused.
    let pb = copy_of(&s, "EW-GEN-PB-1");
    let mut lib = s.part_draft(pb).expect("the pushbutton's draft");
    let t = lib.table("mech.holes").expect("the holes");
    assert_eq!(t.rows.len(), 1);
    assert!(matches!(
        s.set_draft_field(&mut lib, "mech.front_shape", "rectangle"),
        Err(PartDraftError::Refused(_))
    ));
    assert!(s.draft_outline_import(&mut lib, &dxf).is_err());
    assert!(!lib.remove_outline());
}

/// The held copy numbered `number` in `s`.
fn held(s: &Session, number: &str) -> ew_schema::Part {
    let q = s.query();
    q.parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(_, c)| c.item.clone())
        .unwrap_or_else(|| panic!("{number} is held"))
}

/// A gland of one's own made in Parts (its Gland section's rows), put first in the workshop stock, is the
/// gland the program proposes for the sample's -101W1 (10.5 mm); a clamping range the wrong way round is refused in
/// words.
#[test]
fn a_gland_of_ones_own_is_made_in_the_editor_and_proposed_for_a_cable() {
    let mut s = common::sample();
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    for (f, v) in [
        ("manufacturer", "Acme"),
        ("part_number", "OWN-GLD"),
        ("kind", "gland"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    assert!(d.sections().iter().any(|x| x.name() == "gland"));
    for (f, v) in [
        ("sections.gland.thread", "m20"),
        ("sections.gland.clamp_min", "10"),
        ("sections.gland.clamp_max", "9"),
        ("sections.gland.nut", "26"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    match s.new_part_command(&d) {
        Err(PartDraftError::Field(m)) => assert_eq!(m.key, "error-part-gland-range"),
        other => panic!("refused in words: {:?}", other.map(|_| ())),
    }
    s.set_draft_field(&mut d, "sections.gland.clamp_max", "11")
        .expect("a range");
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command).expect("the part is made");
    let own = copy_of(&s, "OWN-GLD");
    let q = s.query();
    let stock: Vec<Value> = std::iter::once(json!({"copy": own.to_string()}))
        .chain(
            q.project()
                .settings
                .workshop
                .stock
                .iter()
                .map(|r| json!({"copy": r.copy.to_string()})),
        )
        .collect();
    drop(q);
    run(
        &mut s,
        &AnyCommand {
            name: "workshop.stock".into(),
            params: json!({"stock": stock}),
        },
    )
    .expect("the stock");
    let q = s.query();
    let space = q
        .table::<ew_schema::LayoutSpace>()
        .ids()
        .next()
        .expect("the sample's space");
    let cable = q
        .devices()
        .iter()
        .find(|(id, _)| ew_derive::short_tag(&q, *id) == "-101W1")
        .map(|(id, _)| id)
        .expect("-101W1");
    let glands = q.get::<ew_derive::layout::CableGlands>(space);
    let g = glands
        .iter()
        .find(|g| g.cable == cable)
        .expect("-101W1 leaves the cabinet");
    assert_eq!(g.part.as_ref().map(|p| p.copy.as_uuid()), Ok(own));
}

/// A cable type of one's own: the cable form's Diameter writes `sections.cable.outer_diameter`, which the one reader
/// gives back, and no technical value; a diameter that is no length is refused in words.
#[test]
fn a_cables_outer_diameter_is_kept_in_its_cable_section() {
    let mut s = common::sample();
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    for (f, v) in [
        ("manufacturer", "Acme"),
        ("part_number", "OWN-CBL"),
        ("kind", "cable"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    assert_eq!(
        field_key(s.set_draft_field(&mut d, "cable.diameter", "thick")),
        "error-part-length"
    );
    s.set_draft_field(&mut d, "cable.diameter", "14")
        .expect("a diameter");
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command).expect("the part is made");
    let part = held(&s, "OWN-CBL");
    let fourteen = ew_units::Nm::from_mm(14);
    assert_eq!(
        part.sections.cable.as_ref().and_then(|c| c.outer_diameter),
        Some(fourteen)
    );
    assert!(part.tech_data.iter().all(|t| t.key.as_str() != "outer_diameter"));
    assert_eq!(ew_derive::layout::cable_outer_diameter(&part), Some(fourteen));
    // The editor opens it with its diameter.
    let draft = s.part_draft(copy_of(&s, "OWN-CBL")).expect("its draft");
    assert_eq!(draft.text("cable.diameter"), "14");
}

/// An enclosure of one's own is made in the editor from its declared rows (the cabinet surfaces' data had no place in
/// the window): its outer size, its door and its edge margin are saved into its enclosure section, Enclosure… offers
/// it, and it lacks nothing a cabinet's layout reads but its plate.
#[test]
fn an_enclosure_of_ones_own_takes_its_size_and_door_in_the_editor() {
    use ew_api::part_fields::{Feature, Group, lacking};
    let mut s = common::sample();
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", "ENC-8-10-3"),
        ("kind", "enclosure"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    assert!(d.sections().contains(&Group::Enclosure), "{:?}", d.sections());
    assert!(
        !d.sections().contains(&Group::Cabinet),
        "a cabinet's size is its own"
    );
    for (f, v) in [
        ("sections.enclosure.width", "800"),
        ("sections.enclosure.height", "1000"),
        ("sections.enclosure.depth", "300"),
        ("sections.enclosure.edge_margin", "30"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    assert!(d.add_table_row("sections.enclosure.doors"));
    s.set_draft_field(&mut d, "sections.enclosure.doors[0].hinge", "left")
        .expect("a hinge");
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command).expect("the part is made");
    let own = copy_of(&s, "ENC-8-10-3");
    let q = s.query();
    let part = &q.parts().get(ew_ids::Id::from_uuid(own)).expect("held").item;
    let section = part.sections.enclosure.as_ref().expect("its enclosure section");
    assert_eq!(section.width, Some(ew_units::Nm(800_000_000)));
    assert_eq!(section.height, Some(ew_units::Nm(1_000_000_000)));
    assert_eq!(section.edge_margin, Some(ew_units::Nm(30_000_000)));
    assert_eq!(section.doors.len(), 1);
    assert!(lacking(part, Feature::Cabinet).is_empty());
    assert_eq!(
        lacking(part, Feature::PlateFrame)
            .iter()
            .map(|f| f.path)
            .collect::<Vec<_>>(),
        ["sections.enclosure.plate"]
    );
    drop(q);
    assert!(
        s.layout_part_offers("enclosure")
            .iter()
            .any(|o| o.number == "ENC-8-10-3"),
        "Enclosure… offers it"
    );
}

/// A member that is no field (a GTIN) is shown under Also stored and written back unchanged when the part is saved.
#[test]
fn a_gtin_is_shown_and_kept_when_the_part_is_saved() {
    let mut s = common::colleague_sample();
    let copy = copy_of(&s, "EW-GEN-TRM-1");
    let mut item = serde_json::to_value(
        &s.query()
            .parts()
            .get(ew_ids::Id::from_uuid(copy))
            .expect("held")
            .item,
    )
    .unwrap();
    item["gtin"] = json!("4011111222233");
    let edit = AnyCommand {
        name: "part.edit_copy".into(),
        params: json!({"copy": copy.to_string(), "part": item, "fork": s.next_fork_item(copy).to_string()}),
    };
    run(&mut s, &edit).expect("the part takes a GTIN");
    let with_gtin = |s: &Session, description: Option<&str>| {
        s.query()
            .parts()
            .iter()
            .find(|(_, c)| {
                c.item.gtin.as_ref().map(|g| g.as_str()) == Some("4011111222233")
                    && description.is_none_or(|d| c.item.description.neutral_or_first() == Some(d))
            })
            .map(|(id, _)| id.as_uuid())
    };
    let fork = with_gtin(&s, None).expect("the project's own part with its GTIN");
    let mut d = s.part_draft(fork).expect("its draft");
    assert!(
        d.also_stored().iter().any(|(_, v)| v == "4011111222233"),
        "{:?}",
        d.also_stored()
    );
    s.set_draft_field(&mut d, "description", "Feed-through terminal, grey")
        .expect("a description");
    let cmd = s.part_edit_command(&d).expect("Save");
    run(&mut s, &cmd).expect("saved");
    assert!(
        with_gtin(&s, Some("Feed-through terminal, grey")).is_some(),
        "the saved part keeps its GTIN"
    );
}
